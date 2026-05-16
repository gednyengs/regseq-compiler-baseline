//! Phase-5 function body materialisation.

use regseq_model::{
    validate_body_places, Body, BodyValidationError, GenericArg as ModelGenericArg, InstanceId,
    InstanceKind, LocalDecl, Mutability, Visitor,
};
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::{self, GenericArgs, Instance, TyCtxt};

use crate::bridge::{ty::translate_ty, BridgeCtxt, BridgeError};

use super::{
    statement::translate_span, translate_basic_block, vtable_indirect::rewrite_vtable_indirect,
    BodyCtxt,
};

/// Phase-5 handle for a function instance whose body is being
/// materialised.
pub type InstanceHandle = InstanceId;

/// Materialise the body of one Function Instance during Phase 5 per
/// spec.md section 25.7. Walks rustc basic blocks, populating
/// LocalDecls via the section 25.8 allocation API and translating
/// each block via `translate_basic_block`.
pub fn materialise_function_body<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance_handle: InstanceHandle,
) -> Result<Body, BridgeError> {
    if instance_handle.index() >= cx.model.instances.len() {
        return Err(BridgeError::Phase5(format!(
            "missing instance {}",
            instance_handle.as_u32()
        )));
    }
    let instance = &cx.model.instances[instance_handle];
    let InstanceKind::Function(_) = &instance.kind else {
        return Err(BridgeError::Phase5(format!(
            "instance {} is not a function",
            instance_handle.as_u32()
        )));
    };
    let item = instance.item;
    let model_args = instance.generic_args.clone();
    let local_def_id = local_def_id_for_item(cx.input.tcx, &cx.model, item)?;
    let args = translate_model_generic_args(cx, local_def_id.to_def_id(), &model_args)?;
    let rustc_instance = Instance::new_raw(local_def_id.to_def_id(), args);
    let rustc_body = cx.input.tcx.instance_mir(rustc_instance.def);

    let mut body = Body {
        basic_blocks: regseq_model::IndexVec::new(),
        local_decls: regseq_model::IndexVec::new(),
        arg_count: rustc_body.arg_count,
        span: translate_span(cx, rustc_body.span),
    };

    for local in rustc_body.local_decls.iter() {
        body.push_local(LocalDecl {
            ty: translate_ty(cx, local.ty)?,
            mutability: translate_local_mutability(local.mutability),
            span: translate_span(cx, local.source_info.span),
            debug_name: None,
        });
    }

    let mut body_cx = BodyCtxt::new(&rustc_body.local_decls);
    for block in rustc_body.basic_blocks.iter() {
        body.basic_blocks
            .push(translate_basic_block(cx, &mut body_cx, block)?);
    }

    Ok(body)
}

/// Phase-5 driver: materialise every Function body, run
/// vtable-indirect rewrite, run joint validation. Per spec.md section
/// 25.7 pass ordering.
pub fn run_phase5_body_passes(_cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let handles = _cx
        .model
        .instances
        .iter()
        .enumerate()
        .filter_map(|(idx, instance)| {
            instance
                .kind
                .as_function()
                .map(|_| InstanceId::from_u32(idx as u32))
        })
        .collect::<Vec<_>>();

    for instance in handles {
        let mut body = materialise_function_body(_cx, instance)?;
        rewrite_vtable_indirect(_cx, &mut body)?;
        joint_validation_walk(&_cx.model, instance, &body)?;
        let function = _cx.model.instances[instance]
            .kind
            .as_function_mut()
            .expect("collected only function instances");
        function.body = Some(body);
    }

    Ok(())
}

fn joint_validation_walk(
    model: &regseq_model::RegseqModel,
    instance: InstanceId,
    body: &Body,
) -> Result<(), BridgeError> {
    let mut visitor = JointValidationVisitor::default();
    visitor.visit_body(body);
    if !visitor.body_errors.is_empty() {
        return Err(BridgeError::Phase5(format!(
            "joint validation body pre-check failed for instance {}: {:?}",
            instance.as_u32(),
            visitor.body_errors
        )));
    }
    validate_body_places(model, body).map_err(|error| {
        BridgeError::Phase5(format!(
            "joint validation place walk failed for instance {}: {error:?}",
            instance.as_u32()
        ))
    })
}

#[derive(Default)]
struct JointValidationVisitor {
    body_errors: Vec<BodyValidationError>,
}

impl Visitor for JointValidationVisitor {
    fn emit_body_error(&mut self, error: BodyValidationError) {
        self.body_errors.push(error);
    }
}

fn local_def_id_for_item(
    tcx: TyCtxt<'_>,
    model: &regseq_model::RegseqModel,
    item: regseq_model::ItemId,
) -> Result<LocalDefId, BridgeError> {
    let wanted = model.item_def_paths[item];
    tcx.hir_body_owners()
        .find(|local_def_id| def_path_hash_eq(tcx, local_def_id.to_def_id(), wanted))
        .ok_or_else(|| {
            BridgeError::Phase5(format!(
                "no rustc body owner found for item {}",
                item.as_u32()
            ))
        })
}

fn def_path_hash_eq(
    tcx: TyCtxt<'_>,
    def_id: rustc_span::def_id::DefId,
    wanted: regseq_model::DefPathHash,
) -> bool {
    let actual = tcx.def_path_hash(def_id);
    actual.stable_crate_id().as_u64() == wanted.stable_crate_id.0
        && actual.local_hash().as_u64() == wanted.local_hash
}

fn translate_local_mutability(mutability: rustc_middle::mir::Mutability) -> Mutability {
    match mutability {
        rustc_middle::mir::Mutability::Not => Mutability::Not,
        rustc_middle::mir::Mutability::Mut => Mutability::Mut,
    }
}

/// Translate the model-side `Instance.generic_args` back into a rustc
/// `GenericArgs<'tcx>` ref for the given rustc `DefId`.
///
/// The model stores only Type arguments (lifetimes are erased per
/// section 17.1; const generics are filtered by `translate_generic_args`
/// at the bridge ingress, mirrored here). To rebuild a complete
/// rustc args vector we walk the rustc generic-param sequence and:
///
/// - For `Lifetime` params, substitute `tcx.lifetimes.re_erased`.
/// - For `Type` params, consume the next model `Type` arg by resolving
///   its `TyId` through the bridge's forward type cache.
/// - For `Const` params, substitute the param default (rustc's
///   `identity_for_item` equivalent for that slot) because the bridge
///   currently filters const generics at translation time.
fn translate_model_generic_args<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    def_id: rustc_span::def_id::DefId,
    model_args: &[ModelGenericArg],
) -> Result<&'tcx ty::List<ty::GenericArg<'tcx>>, BridgeError> {
    let tcx = cx.input.tcx;
    let identity = GenericArgs::identity_for_item(tcx, def_id);
    let mut model_type_iter = model_args.iter().filter_map(|arg| match arg {
        ModelGenericArg::Type(ty_id) => Some(*ty_id),
        ModelGenericArg::Const(_) => None,
    });

    let args = ty::GenericArgs::for_item(tcx, def_id, |param, _| match param.kind {
        ty::GenericParamDefKind::Lifetime => tcx.lifetimes.re_erased.into(),
        ty::GenericParamDefKind::Type { .. } => match model_type_iter.next() {
            Some(model_ty) => match cx.ty_forward_cache.get(&model_ty) {
                Some(rustc_ty) => (*rustc_ty).into(),
                None => identity[param.index as usize],
            },
            None => identity[param.index as usize],
        },
        ty::GenericParamDefKind::Const { .. } => identity[param.index as usize],
    });

    Ok(args)
}
