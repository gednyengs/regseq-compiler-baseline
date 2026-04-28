use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::{DefId, LocalModDefId, CRATE_DEF_ID};
use rustc_hir::{ItemKind, TyKind};
use rustc_middle::ty::{Ty, TyCtxt};

use regseq_model_old::identity::{PropertyId, ResourceId, UserDataId};
use regseq_model_old::ir::{RegseqType, ScalarType};
use regseq_model_old::model::{
    ClockDecl, ClockResource, DriverStruct, InterruptDecl, InterruptResource, Module, PinCtrlDecl,
    PinCtrlResource, PmInfo, PropertyField, ResetDecl, ResetResource, Shape, SyncKind,
    SyncResource, UserDataField, UserDataKind,
};

use crate::extract::clock::ClockSurface;
use crate::extract::interrupts::{InterruptFieldShape, InterruptSurface};
use crate::extract::pinctrl::PinCtrlSurface;
use crate::extract::pm::PmGateSurface;
use crate::extract::property::PropertySurface;
use crate::extract::reset::ResetSurface;
use crate::extract::synchronization::{SyncFieldKind, SynchronizationSurface};

use super::walk::BuildContext;
use super::{ConstructionError, ErrorProvenance};

pub fn populate_clocks<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &ClockSurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for field in &surface.fields {
        let Some(field_info) = driver_hir.fields.get(field.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(field.field_name.as_str()) {
            continue;
        }
        driver.clocks.push(ClockResource {
            id: ResourceId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&field.field_name),
            shape: shape_from_required_and_array_len(field.required, field.array_len),
            decl: ClockDecl {
                clock_id: cx.intern_public(&field.clock_id),
            },
            binding: None,
        });
    }

    Ok(())
}

pub fn populate_resets<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &ResetSurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for field in &surface.fields {
        let Some(field_info) = driver_hir.fields.get(field.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(field.field_name.as_str()) {
            continue;
        }
        driver.resets.push(ResetResource {
            id: ResourceId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&field.field_name),
            shape: shape_from_required_and_array_len(field.required, field.array_len),
            decl: ResetDecl {
                reset_id: cx.intern_public(&field.field_name),
            },
            binding: None,
        });
    }

    Ok(())
}

pub fn populate_pinctrl<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &PinCtrlSurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for field in &surface.fields {
        let Some(field_info) = driver_hir.fields.get(field.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(field.field_name.as_str()) {
            continue;
        }
        let pinctrl_id = field
            .pinctrl_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or(field.field_name.as_str());
        driver.pinctrl.push(PinCtrlResource {
            id: ResourceId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&field.field_name),
            shape: shape_from_required_and_array_len(field.required, field.array_len),
            decl: PinCtrlDecl {
                pinctrl_id: cx.intern_public(pinctrl_id),
            },
            binding: None,
        });
    }

    Ok(())
}

pub fn populate_interrupts<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &InterruptSurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for resource in &surface.resources {
        let Some(field_info) = driver_hir.fields.get(resource.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(resource.field_name.as_str()) {
            continue;
        }
        driver.interrupts.push(InterruptResource {
            id: ResourceId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&resource.field_name),
            shape: shape_from_interrupt(resource.shape),
            decl: InterruptDecl {
                interrupt_id: cx.intern_public(&resource.interrupt_id),
            },
            binding: None,
        });
    }

    Ok(())
}

pub fn populate_sync<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &SynchronizationSurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for field in &surface.sync_fields {
        let Some(field_info) = driver_hir.fields.get(field.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(field.field_name.as_str()) {
            continue;
        }
        let payload_type = field
            .payload_type
            .as_deref()
            .filter(|payload| !payload.is_empty())
            .map(|_| RegseqType::Scalar(ScalarType::U32));
        driver.sync.push(SyncResource {
            id: ResourceId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&field.field_name),
            sync_kind: match field.kind {
                SyncFieldKind::Mutex => SyncKind::Mutex,
                SyncFieldKind::SpinLock => SyncKind::SpinLock,
                SyncFieldKind::IrqSpinLock => SyncKind::IrqSpinLock,
                SyncFieldKind::Completion => SyncKind::Completion,
            },
            payload_type,
        });
    }

    Ok(())
}

pub fn populate_properties<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
    surface: &PropertySurface,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    let mut seen = HashSet::new();

    for field in &surface.fields {
        if field.driver_type != driver_hir.type_name {
            continue;
        }
        let Some(field_info) = driver_hir.fields.get(field.field_name.as_str()) else {
            continue;
        };
        if !seen.insert(field.field_name.as_str()) {
            continue;
        }
        let Some(field_type) = parse_scalar_type(field.field_type.as_str()) else {
            continue;
        };
        driver.properties.push(PropertyField {
            id: PropertyId::from(cx.alloc_and_record_public(field_info.def_id)),
            field_name: cx.intern_public(&field.field_name),
            field_type,
            property_id: cx.intern_public(&field.property_id),
            default_value: None,
            vendor_prefix: field
                .vendor_prefix
                .as_deref()
                .filter(|vendor| !vendor.is_empty())
                .map(|vendor| cx.intern_public(vendor)),
            required: field.required,
            bound_value: None,
        });
    }

    Ok(())
}

pub fn populate_user_data<'tcx>(
    cx: &mut BuildContext<'_, 'tcx>,
    root_module: &mut Module,
) -> Result<(), ConstructionError> {
    let driver_hir = find_driver_hir(cx.tcx)?;
    let consumed = consumed_driver_fields(cx, root_module);
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");

    for field_name in &driver_hir.field_order {
        if consumed.contains(field_name.as_str()) {
            continue;
        }
        let Some(field_info) = driver_hir.fields.get(field_name.as_str()) else {
            continue;
        };
        let field_prov = ErrorProvenance::for_def_id(cx.tcx, field_info.def_id);
        let rendered = super::walk::rendered_ty_string(field_info.ty);
        let middle_field_ty = cx.tcx.type_of(field_info.def_id).instantiate_identity();

        if is_option(field_info.ty) {
            let inner_middle = option_inner_middle_ty(middle_field_ty);
            let inner_hir = extract_option_inner(field_info.ty);
            let inner_is_copy = inner_middle
                .map(|inner| type_implements_copy(cx.tcx, inner))
                .unwrap_or(false);
            if !inner_is_copy {
                let inner_str = inner_hir
                    .map(super::walk::rendered_ty_string)
                    .unwrap_or_default();
                return Err(ConstructionError::UserDataNotCopy {
                    field: field_prov,
                    inner_type: inner_str,
                });
            }
            let inner_type = inner_middle
                .and_then(|inner| super::walk::ty_to_regseq(cx.tcx, &cx.type_def_map, inner))
                .unwrap_or(RegseqType::Scalar(ScalarType::U32));
            driver.user_data.push(UserDataField {
                id: UserDataId::from(cx.alloc_and_record_public(field_info.def_id)),
                field_name: cx.intern_public(field_name),
                kind: UserDataKind::Option { inner_type },
            });
            continue;
        }

        if is_unresolved_generic_or_projection(middle_field_ty) {
            return Err(ConstructionError::UserDataNotZeroInitWhitelisted {
                field: field_prov,
                rejected_type: rendered,
            });
        }
        if !type_implements_copy(cx.tcx, middle_field_ty) {
            return Err(ConstructionError::UserDataNotCopy {
                field: field_prov,
                inner_type: rendered,
            });
        }
        let mut visited: HashSet<Ty<'tcx>> = HashSet::new();
        match rule1_whitelist_ty(cx.tcx, &cx.type_def_map, middle_field_ty, &mut visited) {
            Rule1Outcome::Accepted(inner_type) => {
                driver.user_data.push(UserDataField {
                    id: UserDataId::from(cx.alloc_and_record_public(field_info.def_id)),
                    field_name: cx.intern_public(field_name),
                    kind: UserDataKind::ZeroInit { inner_type },
                });
            }
            Rule1Outcome::Rejected => {
                return Err(ConstructionError::UserDataNotZeroInitWhitelisted {
                    field: field_prov,
                    rejected_type: rendered,
                });
            }
        }
    }

    Ok(())
}

pub fn populate_pm_info(
    root_module: &mut Module,
    surface: &PmGateSurface,
) -> Result<(), ConstructionError> {
    let driver = driver_struct_mut(root_module).expect("driver module was already validated");
    if driver.pm_info.is_some() {
        return Ok(());
    }
    if let Some(gate) = surface.gates.first() {
        driver.pm_info = Some(PmInfo {
            idle: gate.properties.idle,
            wakeup_capable: gate.properties.wakeup_capable,
        });
    }
    Ok(())
}

struct DriverFieldInfo<'tcx> {
    def_id: DefId,
    ty: &'tcx rustc_hir::Ty<'tcx>,
}

struct DriverHir<'tcx> {
    type_name: String,
    field_order: Vec<String>,
    fields: HashMap<String, DriverFieldInfo<'tcx>>,
}

fn find_driver_hir<'tcx>(tcx: TyCtxt<'tcx>) -> Result<DriverHir<'tcx>, ConstructionError> {
    let driver_item =
        find_driver_item_in_module(tcx, CRATE_DEF_ID.to_def_id()).ok_or_else(|| {
            ConstructionError::NoDriverStruct {
                crate_root: ErrorProvenance::for_def_id(tcx, CRATE_DEF_ID.to_def_id()),
            }
        })?;
    let ItemKind::Struct(ident, _generics, variant_data) = &driver_item.kind else {
        unreachable!("driver item must be a struct");
    };
    let mut field_order = Vec::new();
    let mut fields = HashMap::new();
    for field in variant_data.fields() {
        let name = field.ident.name.as_str().to_owned();
        field_order.push(name.clone());
        fields.insert(
            name,
            DriverFieldInfo {
                def_id: field.def_id.to_def_id(),
                ty: field.ty,
            },
        );
    }
    Ok(DriverHir {
        type_name: ident.name.as_str().to_owned(),
        field_order,
        fields,
    })
}

fn find_driver_item_in_module<'tcx>(
    tcx: TyCtxt<'tcx>,
    module_def_id: DefId,
) -> Option<&'tcx rustc_hir::Item<'tcx>> {
    for item_id in children_of(tcx, module_def_id) {
        let item = tcx.hir_item(item_id);
        let def_id = item.owner_id.def_id.to_def_id();
        match &item.kind {
            ItemKind::Struct(..) if has_rseq_attr(tcx, def_id, "driver") => return Some(item),
            ItemKind::Mod(..) => {
                if let Some(found) = find_driver_item_in_module(tcx, def_id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn children_of(tcx: TyCtxt<'_>, module_def_id: DefId) -> Vec<rustc_hir::ItemId> {
    let Some(local) = module_def_id.as_local() else {
        return Vec::new();
    };
    let mod_id = LocalModDefId::new_unchecked(local);
    tcx.hir_module_items(mod_id).free_items().collect()
}

fn has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, attr_name: &str) -> bool {
    tcx.get_all_attrs(def_id).iter().any(|attr| {
        let rustc_hir::Attribute::Unparsed(item) = attr else {
            return false;
        };
        let segs = &item.path.segments;
        segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == attr_name
    })
}

fn driver_struct(module: &Module) -> Option<&DriverStruct> {
    if let Some(driver) = module.driver.as_ref() {
        return Some(driver);
    }
    for submodule in &module.submodules {
        if let Some(driver) = driver_struct(submodule) {
            return Some(driver);
        }
    }
    None
}

fn driver_struct_mut(module: &mut Module) -> Option<&mut DriverStruct> {
    if let Some(driver) = module.driver.as_mut() {
        return Some(driver);
    }
    for submodule in &mut module.submodules {
        if let Some(driver) = driver_struct_mut(submodule) {
            return Some(driver);
        }
    }
    None
}

fn consumed_driver_fields(cx: &BuildContext<'_, '_>, root_module: &Module) -> HashSet<String> {
    let Some(driver) = driver_struct(root_module) else {
        return HashSet::new();
    };
    driver
        .clocks
        .iter()
        .map(|resource| cx.symbols.resolve(resource.field_name).to_owned())
        .chain(
            driver
                .resets
                .iter()
                .map(|resource| cx.symbols.resolve(resource.field_name).to_owned()),
        )
        .chain(
            driver
                .pinctrl
                .iter()
                .map(|resource| cx.symbols.resolve(resource.field_name).to_owned()),
        )
        .chain(
            driver
                .interrupts
                .iter()
                .map(|resource| cx.symbols.resolve(resource.field_name).to_owned()),
        )
        .chain(
            driver
                .mmio
                .iter()
                .map(|resource| cx.symbols.resolve(resource.field_name).to_owned()),
        )
        .chain(
            driver
                .sync
                .iter()
                .map(|resource| cx.symbols.resolve(resource.field_name).to_owned()),
        )
        .chain(
            driver
                .properties
                .iter()
                .map(|field| cx.symbols.resolve(field.field_name).to_owned()),
        )
        .collect()
}

fn shape_from_required_and_array_len(required: bool, array_len: Option<usize>) -> Shape {
    match array_len {
        Some(len) => Shape::Array(len),
        None if required => Shape::Single,
        None => Shape::Optional,
    }
}

fn shape_from_interrupt(shape: InterruptFieldShape) -> Shape {
    match shape {
        InterruptFieldShape::Single => Shape::Single,
        InterruptFieldShape::Optional => Shape::Optional,
        InterruptFieldShape::Array(len) => Shape::Array(len),
    }
}

fn parse_scalar_type(raw: &str) -> Option<ScalarType> {
    match raw {
        "u8" => Some(ScalarType::U8),
        "u16" => Some(ScalarType::U16),
        "u32" => Some(ScalarType::U32),
        "u64" => Some(ScalarType::U64),
        "usize" => Some(ScalarType::Usize),
        "i8" => Some(ScalarType::I8),
        "i16" => Some(ScalarType::I16),
        "i32" => Some(ScalarType::I32),
        "i64" => Some(ScalarType::I64),
        "isize" => Some(ScalarType::Isize),
        _ => None,
    }
}

fn is_option(ty: &rustc_hir::Ty<'_>) -> bool {
    matches!(
        &ty.kind,
        TyKind::Path(rustc_hir::QPath::Resolved(_, path))
            if path
                .segments
                .last()
                .map(|seg| seg.ident.name.as_str() == "Option")
                .unwrap_or(false)
    )
}

fn extract_option_inner<'hir>(ty: &'hir rustc_hir::Ty<'hir>) -> Option<&'hir rustc_hir::Ty<'hir>> {
    let TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind else {
        return None;
    };
    let seg = path.segments.last()?;
    if seg.ident.name.as_str() != "Option" {
        return None;
    }
    let args = seg.args?;
    for arg in args.args.iter() {
        if let rustc_hir::GenericArg::Type(inner) = arg {
            return Some(inner.as_unambig_ty());
        }
    }
    None
}

fn option_inner_middle_ty<'tcx>(ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    if let rustc_middle::ty::TyKind::Adt(_, args) = ty.kind() {
        return args.types().next();
    }
    None
}

fn type_implements_copy<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let typing_env = rustc_middle::ty::TypingEnv::fully_monomorphized();
    tcx.type_is_copy_modulo_regions(typing_env, ty)
}

fn is_unresolved_generic_or_projection(ty: Ty<'_>) -> bool {
    use rustc_middle::ty::TyKind;
    matches!(
        ty.kind(),
        TyKind::Param(_) | TyKind::Alias(..) | TyKind::Bound(..) | TyKind::Placeholder(_)
    )
}

enum Rule1Outcome {
    Accepted(RegseqType),
    Rejected,
}

fn rule1_whitelist_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    type_def_map: &HashMap<DefId, regseq_model_old::identity::TypeDefId>,
    ty: Ty<'tcx>,
    visited: &mut HashSet<Ty<'tcx>>,
) -> Rule1Outcome {
    use rustc_middle::ty::TyKind;

    match ty.kind() {
        TyKind::Param(_) | TyKind::Alias(..) | TyKind::Bound(..) | TyKind::Placeholder(_) => {
            return Rule1Outcome::Rejected;
        }
        _ => {}
    }

    if !visited.insert(ty) {
        return Rule1Outcome::Accepted(RegseqType::Unit);
    }
    let outcome = rule1_whitelist_ty_inner(tcx, type_def_map, ty, visited);
    visited.remove(&ty);
    outcome
}

fn rule1_whitelist_ty_inner<'tcx>(
    tcx: TyCtxt<'tcx>,
    type_def_map: &HashMap<DefId, regseq_model_old::identity::TypeDefId>,
    ty: Ty<'tcx>,
    visited: &mut HashSet<Ty<'tcx>>,
) -> Rule1Outcome {
    use rustc_middle::ty::TyKind;

    match ty.kind() {
        TyKind::Bool => Rule1Outcome::Accepted(RegseqType::Bool),
        TyKind::Uint(u) => match u {
            rustc_middle::ty::UintTy::U8 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U8))
            }
            rustc_middle::ty::UintTy::U16 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U16))
            }
            rustc_middle::ty::UintTy::U32 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U32))
            }
            rustc_middle::ty::UintTy::U64 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U64))
            }
            _ => Rule1Outcome::Rejected,
        },
        TyKind::Array(inner, len_const) => {
            let Some(len) = len_const.try_to_target_usize(tcx) else {
                return Rule1Outcome::Rejected;
            };
            match rule1_whitelist_ty(tcx, type_def_map, *inner, visited) {
                Rule1Outcome::Accepted(elem_ty) => {
                    Rule1Outcome::Accepted(RegseqType::Array(Box::new(elem_ty), len as usize))
                }
                Rule1Outcome::Rejected => Rule1Outcome::Rejected,
            }
        }
        TyKind::Adt(adt, args) => {
            if !adt.is_struct() {
                return Rule1Outcome::Rejected;
            }
            let did = adt.did();
            if !did.is_local() {
                return Rule1Outcome::Rejected;
            }
            let Some(type_id) = type_def_map.get(&did).copied() else {
                return Rule1Outcome::Rejected;
            };
            for field in &adt.non_enum_variant().fields {
                match rule1_whitelist_ty(tcx, type_def_map, field.ty(tcx, args), visited) {
                    Rule1Outcome::Accepted(_) => {}
                    Rule1Outcome::Rejected => return Rule1Outcome::Rejected,
                }
            }
            Rule1Outcome::Accepted(RegseqType::Defined(type_id))
        }
        _ => Rule1Outcome::Rejected,
    }
}
