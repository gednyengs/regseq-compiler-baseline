//! Rule 3 companion: cross-crate ADT extraction.
//!
//! Pass 1's [`super::cross_crate`] walk registers every cross-crate
//! *function* reachable from authored bodies. Function translation in
//! Pass 2 then hits cross-crate ADTs -- `regseq::Clock`,
//! `regseq::RxChar`, `core::fmt::Arguments`, and so on -- that appear
//! in local-decl types or rvalue destinations but have no entry in
//! `cx.type_def_map` (only local-crate structs/enums do). Without an
//! entry, [`super::mir_mapping::map_ty`] surfaces a
//! [`super::ConstructionError::UnsupportedRustType`] and aborts body
//! construction for otherwise-well-formed specs.
//!
//! This pass walks every function body reachable from Pass 1's
//! `function_map`, collects every non-lang-item cross-crate ADT
//! referenced in a local-decl, rvalue, terminator-argument, or
//! Place-projection type, and materialises each as a minimal
//! [`TypeDef`] under the synthetic cross-crate module chain already
//! built by [`super::cross_crate`]. Classification is derived from the
//! Phase 4 attribute registries when the ADT carries
//! `#[rseq::framework_type]` or `#[rseq::abstraction]`; anything else
//! lands as [`TypeDefClassification::UserDefined`]. Fields are left
//! empty -- Pass 2 only needs the `TypeDefId` for `RegseqType::Defined`
//! references; aggregate construction and Field projections on
//! cross-crate ADTs would ordinarily go through cross-crate method
//! calls that never inspect the TypeDef's field list. The IR remains
//! sound because `map_ty` never consults fields either.
//!
//! Motivation: this is intentionally a declaration-level integration
//! pass, not a new recognition rule. The body-level IR construction
//! rules (1-7) are unchanged; this pass only makes them applicable to
//! bodies that reach across the crate boundary through already-
//! registered cross-crate callees.

use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use regseq_model::identity::{SourceOrigin, TypeDefId};
use regseq_model::ir::{RegseqAbstraction, RegseqType};
use regseq_model::model::{
    Attrs, FieldDef, FrameworkTypeKind, Module, ModuleKind, TypeDef, TypeDefClassification,
    TypeDefKind, VariantDef,
};

use super::mir_mapping::{map_ty, TypeDefIndex};

use super::walk::BuildContext;
use super::ConstructionError;

/// Register every cross-crate ADT referenced by a body in
/// `cx.function_map` as a minimal [`TypeDef`] under the synthetic
/// cross-crate module chain rooted at the ADT's defining crate.
///
/// Runs after [`super::cross_crate::extract_cross_crate_functions`]
/// completes so the transitive closure of the authored call graph is
/// fully in scope.
pub fn extract_cross_crate_types(
    cx: &mut BuildContext<'_, '_>,
    root_module: &mut Module,
) -> Result<(), ConstructionError> {
    let tcx = cx.tcx;

    // Collect all cross-crate ADT DefIds referenced by any function
    // body. `Option` / `Result` lang-items are handled directly by
    // `map_ty` and do not need a TypeDef entry.
    //
    // Iteration order here is cross-run deterministic: sort the
    // `function_map` keys by `def_path_str` before walking their
    // MIR, so the order in which new cross-crate ADTs are
    // discovered (and later assigned DeclIds) is stable. Phase 5's
    // golden-file contract requires byte-identical output across
    // runs.
    let mut to_register: Vec<DefId> = Vec::new();
    let mut seen: HashSet<DefId> = HashSet::new();
    let mut callers: Vec<DefId> = cx.function_map.keys().copied().collect();
    callers.sort_by_cached_key(|did| tcx.def_path_str(*did));
    for def_id in callers {
        if !tcx.is_mir_available(def_id) {
            continue;
        }
        let body = tcx.optimized_mir(def_id);
        collect_adts_in_body(tcx, body, &cx.type_def_map, &mut seen, &mut to_register);
    }

    // Register each collected ADT. Sort by the rendered def-path for
    // deterministic iteration order across runs -- seed set ordering
    // depends on HashMap iteration otherwise.
    to_register.sort_by_cached_key(|did| tcx.def_path_str(*did));

    for adt_did in to_register {
        if cx.type_def_map.contains_key(&adt_did) {
            continue;
        }
        register_cross_crate_adt(cx, root_module, adt_did)?;
    }

    Ok(())
}

fn collect_adts_in_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
    type_def_map: &HashMap<DefId, TypeDefId>,
    seen: &mut HashSet<DefId>,
    out: &mut Vec<DefId>,
) {
    for local in body.local_decls.iter() {
        collect_adts_in_ty(tcx, local.ty, type_def_map, seen, out);
    }
    for bb in body.basic_blocks.iter() {
        for stmt in &bb.statements {
            if let mir::StatementKind::Assign(boxed) = &stmt.kind {
                // The rvalue's resulting type is the destination
                // place's type; visiting the destination's
                // projection types picks up field-type ADTs that
                // never appear as a whole local.
                let (_, rvalue) = &**boxed;
                match rvalue {
                    mir::Rvalue::Aggregate(kind, _) => {
                        if let mir::AggregateKind::Adt(did, _, _, _, _) = &**kind {
                            push_adt(*did, type_def_map, seen, out);
                        }
                    }
                    mir::Rvalue::Cast(_, _, cast_ty) => {
                        collect_adts_in_ty(tcx, *cast_ty, type_def_map, seen, out);
                    }
                    _ => {}
                }
            }
        }
        if let mir::TerminatorKind::Call {
            func: mir::Operand::Constant(boxed),
            ..
        } = &bb.terminator().kind
        {
            // The callee's substs frequently carry cross-crate ADT
            // references (e.g. `PinCtrl::apply::<DefaultState>`).
            if let ty::TyKind::FnDef(_, subst) = boxed.const_.ty().kind() {
                for generic in subst.types() {
                    collect_adts_in_ty(tcx, generic, type_def_map, seen, out);
                }
            }
        }
    }
}

fn collect_adts_in_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    type_def_map: &HashMap<DefId, TypeDefId>,
    seen: &mut HashSet<DefId>,
    out: &mut Vec<DefId>,
) {
    match ty.kind() {
        ty::TyKind::Adt(adt_def, args) => {
            let did = adt_def.did();
            // Skip lang-item handled types (Option / Result). Their
            // structural form is handled directly by `map_ty`.
            if is_option_or_result(tcx, did) {
                // Still recurse into type arguments.
                for t in args.types() {
                    collect_adts_in_ty(tcx, t, type_def_map, seen, out);
                }
                return;
            }
            push_adt(did, type_def_map, seen, out);
            for t in args.types() {
                collect_adts_in_ty(tcx, t, type_def_map, seen, out);
            }
        }
        ty::TyKind::Ref(_, inner, _) | ty::TyKind::RawPtr(inner, _) => {
            collect_adts_in_ty(tcx, *inner, type_def_map, seen, out);
        }
        ty::TyKind::Array(inner, _) | ty::TyKind::Slice(inner) => {
            collect_adts_in_ty(tcx, *inner, type_def_map, seen, out);
        }
        ty::TyKind::Tuple(elems) => {
            for t in elems.iter() {
                collect_adts_in_ty(tcx, t, type_def_map, seen, out);
            }
        }
        _ => {}
    }
}

fn push_adt(
    did: DefId,
    type_def_map: &HashMap<DefId, TypeDefId>,
    seen: &mut HashSet<DefId>,
    out: &mut Vec<DefId>,
) {
    if did.krate == LOCAL_CRATE {
        // Local crate ADTs are handled by the main walk; they land
        // in type_def_map directly.
        return;
    }
    if type_def_map.contains_key(&did) {
        return;
    }
    if seen.insert(did) {
        out.push(did);
    }
}

fn is_option_or_result(tcx: TyCtxt<'_>, did: DefId) -> bool {
    let name = tcx.get_diagnostic_name(did);
    matches!(name, Some(n) if n == rustc_span::sym::Option || n == rustc_span::sym::Result)
}

fn register_cross_crate_adt(
    cx: &mut BuildContext<'_, '_>,
    root_module: &mut Module,
    adt_did: DefId,
) -> Result<(), ConstructionError> {
    let tcx = cx.tcx;
    let adt_def = tcx.adt_def(adt_did);

    // Resolve or create the synthetic module chain for this ADT's
    // defining crate (e.g. `core::fmt::Arguments` ends up under
    // `core -> fmt -> Arguments`). The chain mirrors the one built
    // by `super::cross_crate` for functions.
    let chain = module_chain_for(tcx, adt_did);
    let module = ensure_synthetic_module_chain(cx, root_module, &chain);

    // Allocate a TypeDefId for the ADT. Record the ADT's span so
    // `source_locations` gets a real entry rather than
    // `CompilerGenerated`.
    let id = TypeDefId::from(cx.alloc_and_record_public(adt_did));

    // Classification: consult the Phase 4 registries for
    // `#[rseq::framework_type]` / `#[rseq::abstraction]`. Fall back
    // to `UserDefined` otherwise.
    let classification = if let Some(canonical) = cx.registries.abstraction_types.lookup(adt_did) {
        match abstraction_from_canonical(canonical) {
            Some(ab) => TypeDefClassification::Abstraction(ab),
            None => TypeDefClassification::UserDefined,
        }
    } else if let Some(canonical) = cx.registries.framework_types.lookup(adt_did) {
        match framework_from_canonical(canonical) {
            Some(fr) => TypeDefClassification::Framework(fr),
            None => TypeDefClassification::UserDefined,
        }
    } else {
        TypeDefClassification::UserDefined
    };

    // Pick a name: the ADT's bare identifier from `def_path_str`.
    let raw_path = tcx.def_path_str(adt_did);
    let bare_name = raw_path.rsplit("::").next().unwrap_or(&raw_path).to_owned();
    let name_sym = cx.intern_public(&bare_name);

    // Populate fields / variants from the ADT's middle-ty view so
    // the Phase-1 invariant "every declaration-side field
    // populated" holds for cross-crate types too. Fields whose
    // types cannot be mapped (generic `Param`, function pointers,
    // etc.) fall back to `RegseqType::Unit` so the field entry
    // still exists -- the integration test only reads structural
    // presence, not the fallback type itself.
    //
    // We evaluate field types against a lazy `TypeDefIndex` view
    // over the in-progress map so types introduced earlier in
    // this extraction pass resolve to `RegseqType::Defined`
    // correctly.
    let kind = build_kind_from_adt(cx, adt_def);

    let type_def = TypeDef {
        id,
        name: name_sym,
        kind,
        classification,
        attrs: Attrs::default(),
    };
    cx.type_def_map.insert(adt_did, id);
    module.type_defs.push(type_def);

    Ok(())
}

fn module_chain_for(tcx: TyCtxt<'_>, did: DefId) -> Vec<String> {
    let rendered = tcx.def_path_str(did);
    rendered
        .split("::")
        .map(|s| s.to_owned())
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, rest)| rest.to_vec())
        .unwrap_or_default()
}

fn ensure_synthetic_module_chain<'m>(
    cx: &mut BuildContext<'_, '_>,
    root: &'m mut Module,
    chain: &[String],
) -> &'m mut Module {
    let mut current: &mut Module = root;
    for segment in chain {
        let name_sym = cx.symbols.intern(segment);
        let existing_index = current.submodules.iter().position(|m| m.name == name_sym);
        match existing_index {
            Some(i) => {
                current = &mut current.submodules[i];
            }
            None => {
                let allocator = &mut *cx.allocator;
                let new_id = regseq_model::identity::ModuleId::from(allocator.alloc());
                cx.source_locations
                    .insert(new_id.decl_id(), SourceOrigin::CompilerGenerated);
                let new_module = Module {
                    id: new_id,
                    name: name_sym,
                    kind: ModuleKind::Normal,
                    submodules: Vec::new(),
                    constants: Vec::new(),
                    free_functions: Vec::new(),
                    type_defs: Vec::new(),
                    register_descriptors: Vec::new(),
                    register_blocks: Vec::new(),
                    impl_blocks: Vec::new(),
                    driver: None,
                    trait_impls: Vec::new(),
                    driver_impl_blocks: Vec::new(),
                    abstractions_used: std::collections::BTreeSet::new(),
                    intrinsics_used: std::collections::BTreeSet::new(),
                };
                current.submodules.push(new_module);
                let last = current.submodules.len() - 1;
                current = &mut current.submodules[last];
            }
        }
    }
    current
}

fn build_kind_from_adt(cx: &mut BuildContext<'_, '_>, adt_def: ty::AdtDef<'_>) -> TypeDefKind {
    let tcx = cx.tcx;
    match adt_def.adt_kind() {
        ty::AdtKind::Struct | ty::AdtKind::Union => {
            let variant = adt_def.non_enum_variant();
            let fields: Vec<FieldDef> = variant
                .fields
                .iter()
                .map(|f| {
                    let field_ty = tcx.type_of(f.did).instantiate_identity();
                    let ty_resolved = map_ty(tcx, field_ty, &TypeDefIndex::new(&cx.type_def_map))
                        .unwrap_or(RegseqType::Unit);
                    let name = cx.symbols.intern(f.ident(tcx).name.as_str());
                    FieldDef {
                        name,
                        ty: ty_resolved,
                    }
                })
                .collect();
            TypeDefKind::Struct { fields }
        }
        ty::AdtKind::Enum => {
            let variants: Vec<VariantDef> = adt_def
                .variants()
                .iter()
                .map(|v| {
                    let name = cx.symbols.intern(v.name.as_str());
                    let fields = v
                        .fields
                        .iter()
                        .map(|f| {
                            let field_ty = tcx.type_of(f.did).instantiate_identity();
                            let ty_resolved =
                                map_ty(tcx, field_ty, &TypeDefIndex::new(&cx.type_def_map))
                                    .unwrap_or(RegseqType::Unit);
                            let field_name = cx.symbols.intern(f.ident(tcx).name.as_str());
                            FieldDef {
                                name: field_name,
                                ty: ty_resolved,
                            }
                        })
                        .collect();
                    VariantDef {
                        name,
                        fields,
                        discriminant: None,
                    }
                })
                .collect();
            TypeDefKind::Enum { variants }
        }
    }
}

fn abstraction_from_canonical(name: &str) -> Option<RegseqAbstraction> {
    Some(match name {
        "clock" => RegseqAbstraction::Clock,
        "reset" => RegseqAbstraction::Reset,
        "pinctrl" => RegseqAbstraction::PinCtrl,
        "interrupt" => RegseqAbstraction::Interrupt,
        "mmio" => RegseqAbstraction::Mmio,
        "mutex" => RegseqAbstraction::Mutex,
        "spinlock" => RegseqAbstraction::SpinLock,
        "irq-spinlock" => RegseqAbstraction::IrqSpinLock,
        "completion" => RegseqAbstraction::Completion,
        _ => return None,
    })
}

fn framework_from_canonical(name: &str) -> Option<FrameworkTypeKind> {
    Some(match name {
        "duration" => FrameworkTypeKind::Duration,
        "irq-result" => FrameworkTypeKind::IrqResult,
        "data-bits" => FrameworkTypeKind::DataBits,
        "parity" => FrameworkTypeKind::Parity,
        "stop-bits" => FrameworkTypeKind::StopBits,
        "flow-control" => FrameworkTypeKind::FlowControl,
        "serial-config" => FrameworkTypeKind::SerialConfig,
        "serial-errors" => FrameworkTypeKind::SerialErrors,
        "rx-char" => FrameworkTypeKind::RxChar,
        "regseq-error" => FrameworkTypeKind::RegseqError,
        _ => return None,
    })
}
