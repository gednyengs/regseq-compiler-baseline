//! External modeled item discovery and materialization.
//!
//! Discovers external nominal types from supported crates (currently
//! `regseq`) that appear in the analyzed MIR slice, and materializes
//! them as real `ItemStore` entries with real `ItemId`s.
//!
//! This is a dedicated prepass that runs between authored hierarchy
//! construction and semantic lowering. It ensures that framework types
//! like `RxChar` and `RegseqError` receive honest item identity.
//!
//! Discovery is code-driven (scans MIR types actually referenced),
//! not a hardcoded inventory.

use std::collections::{BTreeMap, BTreeSet};

use regseq_analysis::AnalysisInput;
use regseq_model::declaration::mir_body::{
    MirConstValue, MirFnBodyDecl, MirOperand, MirRvalue, MirStatementKind, MirTerminatorKind, MirTy,
};
use regseq_model::declaration::Provenance;
use regseq_model::item::constant::{ConstValue, ConstantDef};
use regseq_model::item::function::{FunctionDef, FunctionKind, FunctionRole};
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::impl_def::ImplDef;
use regseq_model::item::module::Module;
use regseq_model::item::type_ref::TypeRef;
use regseq_model::item::types::{EnumDef, EnumVariantDef, Item, StructDef};
use regseq_model::semantic::SemanticBody;

use super::SemanticLoweringError;
use crate::lowering::hierarchy::ItemHierarchy;

// ---------------------------------------------------------------------------
// Supported external type set
// ---------------------------------------------------------------------------

/// Classification of an external type's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTypeKind {
    Struct,
    Enum,
}

/// The supported external type set for the current slice.
///
/// Derived from the actual `regseq` crate source. If a discovered
/// `regseq::` ADT path is not in this table (after alias normalization),
/// materialization fails with a hard error.
fn supported_external_types() -> BTreeMap<&'static str, ExternalTypeKind> {
    let mut m = BTreeMap::new();
    m.insert("regseq::RxChar", ExternalTypeKind::Struct);
    m.insert("regseq::RegseqError", ExternalTypeKind::Enum);
    m.insert("regseq::SerialErrors", ExternalTypeKind::Struct);
    m.insert("regseq::SerialConfig", ExternalTypeKind::Struct);
    m.insert("regseq::IrqResult", ExternalTypeKind::Enum);
    m
}

/// Known variant definitions for supported external enums.
///
/// MIR `Aggregate { Adt { variant_index } }` only gives numeric indices,
/// not variant names. This table provides variant names and discriminants
/// for the supported external enum set. Consistent with
/// `supported_external_types()` and the design doc's allowance for
/// rule-driven classification.
fn external_enum_variants() -> BTreeMap<&'static str, Vec<(&'static str, Option<u64>)>> {
    let mut m: BTreeMap<&str, Vec<(&str, Option<u64>)>> = BTreeMap::new();
    m.insert(
        "regseq::IrqResult",
        vec![("Handled", Some(0)), ("NotHandled", Some(1))],
    );
    m.insert(
        "regseq::RegseqError",
        vec![
            ("Io", Some(0)),
            ("InvalidConfig", Some(1)),
            ("Timeout", Some(2)),
        ],
    );
    m
}

// ---------------------------------------------------------------------------
// External function metadata
// ---------------------------------------------------------------------------

/// Shape-reconstruction metadata for external functions from supported crates.
///
/// MIR does not carry ownership or self-receiver information for external
/// functions. This table provides the data needed to materialize them as
/// coherently-owned `FunctionDef` items.
#[derive(Debug, Clone)]
struct ExternalFunctionMeta {
    canonical_path: &'static str,
    name: &'static str,
    /// Owning type for inherent methods (e.g., `"regseq::RxChar"`).
    /// `None` for free functions.
    owner_type: Option<&'static str>,
}

/// Shape-reconstruction table for the supported external function set.
fn supported_external_functions() -> BTreeMap<&'static str, ExternalFunctionMeta> {
    let mut m = BTreeMap::new();
    m.insert(
        "regseq::RxChar::new",
        ExternalFunctionMeta {
            canonical_path: "regseq::RxChar::new",
            name: "new",
            owner_type: Some("regseq::RxChar"),
        },
    );
    m.insert(
        "regseq::SerialErrors::empty",
        ExternalFunctionMeta {
            canonical_path: "regseq::SerialErrors::empty",
            name: "empty",
            owner_type: Some("regseq::SerialErrors"),
        },
    );
    m
}

// ---------------------------------------------------------------------------
// External constant metadata
// ---------------------------------------------------------------------------

/// Shape-reconstruction metadata for external constants from supported crates.
#[derive(Debug, Clone)]
struct ExternalConstantMeta {
    canonical_path: &'static str,
    name: &'static str,
    owner_type: &'static str,
    value: Option<i64>,
}

/// Shape-reconstruction table for the supported external constant set.
fn supported_external_constants() -> BTreeMap<&'static str, ExternalConstantMeta> {
    let mut m = BTreeMap::new();
    m.insert(
        "regseq::SerialErrors::OVERRUN",
        ExternalConstantMeta {
            canonical_path: "regseq::SerialErrors::OVERRUN",
            name: "OVERRUN",
            owner_type: "regseq::SerialErrors",
            value: Some(1), // 1 << 0
        },
    );
    m.insert(
        "regseq::SerialErrors::BREAK",
        ExternalConstantMeta {
            canonical_path: "regseq::SerialErrors::BREAK",
            name: "BREAK",
            owner_type: "regseq::SerialErrors",
            value: Some(8), // 1 << 3
        },
    );
    m.insert(
        "regseq::SerialErrors::PARITY",
        ExternalConstantMeta {
            canonical_path: "regseq::SerialErrors::PARITY",
            name: "PARITY",
            owner_type: "regseq::SerialErrors",
            value: Some(2), // 1 << 1
        },
    );
    m.insert(
        "regseq::SerialErrors::FRAMING",
        ExternalConstantMeta {
            canonical_path: "regseq::SerialErrors::FRAMING",
            name: "FRAMING",
            owner_type: "regseq::SerialErrors",
            value: Some(4), // 1 << 2
        },
    );
    m
}

// ---------------------------------------------------------------------------
// External callable predicate (reused by semantic walker)
// ---------------------------------------------------------------------------

/// Check if a path is a supported external callable that should be modeled.
///
/// True when the path passes through the supported external crate filter
/// AND is not already handled by a dedicated semantic classification
/// (register ops, resource ops, barriers, etc.).
///
/// This is the same predicate used by discovery/materialization (Step 2a)
/// and by the semantic walker's `Unknown` call recovery (Step 5).
pub fn is_supported_external_callable(path: &str) -> bool {
    is_supported_external_crate(path) && !is_already_classified_infrastructure(path)
}

/// Check if a path belongs to infrastructure already handled by dedicated
/// call classifications (register wrappers, resource handles, barriers,
/// duration constructors, serial trait methods).
fn is_already_classified_infrastructure(path: &str) -> bool {
    // Register wrapper methods
    if path.contains("ReadWrite::") || path.contains("ReadOnly::") || path.contains("WriteOnly::") {
        return true;
    }
    // Resource handle methods
    if path.contains("Clock::enable")
        || path.contains("Clock::disable")
        || path.contains("Clock::get_rate")
    {
        return true;
    }
    if path.contains("ResetLine::") {
        return true;
    }
    if path.contains("PinCtrl::") {
        return true;
    }
    // Barrier intrinsics
    if path.contains("memory_barrier") {
        return true;
    }
    // Duration constructors
    if path.contains("Duration::") {
        return true;
    }
    // Serial trait methods
    if path.contains("Serial::") {
        return true;
    }
    false
}

/// Normalize external type alias paths to their real target type.
///
/// `regseq::ErrorKind` is `pub type ErrorKind = SerialErrors` — a type
/// alias, not a real nominal type. Alias paths are normalized before
/// hierarchy lookup/materialization.
pub fn normalize_external_alias(path: &str) -> &str {
    match path {
        "regseq::ErrorKind" => "regseq::SerialErrors",
        other => other,
    }
}

/// Check if a path belongs to a supported external crate.
fn is_supported_external_crate(path: &str) -> bool {
    path.starts_with("regseq::")
        && !path.starts_with("regseq::ReadWrite")
        && !path.starts_with("regseq::ReadOnly")
        && !path.starts_with("regseq::WriteOnly")
        && !path.starts_with("regseq::Mmio")
        && !path.starts_with("regseq::Field")
        && !path.starts_with("regseq::FieldValue")
        && !path.starts_with("regseq::Clock")
        && !path.starts_with("regseq::ResetLine")
        && !path.starts_with("regseq::PinCtrl")
        && !path.starts_with("regseq::Duration")
        && !path.starts_with("regseq::intrinsics")
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover and materialize external items from MIR bodies.
///
/// Scans all MIR bodies in scope for external references from supported
/// crates:
/// - `MirTy::Adt` paths (types)
/// - `MirTy::FnDef` paths in Call terminators (functions)
/// - `MirConstValue::Unsupported { "unevaluated:regseq::..." }` (constants)
///
/// Materializes discovered items as real `ItemStore` entries with
/// coherent ownership (ImplDef, FunctionDef, ConstantDef).
///
/// Returns an augmented `ItemHierarchy` with the external items added.
pub fn discover_and_materialize(
    hierarchy: &ItemHierarchy,
    input: &AnalysisInput,
) -> Result<ItemHierarchy, SemanticLoweringError> {
    let supported = supported_external_types();
    let variant_meta = external_enum_variants();
    let fn_meta = supported_external_functions();
    let const_meta = supported_external_constants();
    let mut discovered_types: BTreeSet<String> = BTreeSet::new();
    let mut discovered_fns: BTreeSet<String> = BTreeSet::new();
    let mut discovered_consts: BTreeSet<String> = BTreeSet::new();

    // Scan all MIR bodies for external references.
    let reachable = input.mir_reachable_helper_names();
    let callbacks = input.mir_callback_bodies();
    let isrs = input.mir_isr_bodies();
    let helpers = input.mir_helper_bodies();
    let free_fns = input.mir_free_function_bodies();

    let all_bodies: Vec<_> = callbacks
        .iter()
        .chain(isrs.iter())
        .chain(helpers.iter().filter(|f| reachable.contains(&f.name)))
        .chain(free_fns.iter().filter(|f| reachable.contains(&f.name)))
        .collect();

    for func in &all_bodies {
        // Scan local declarations for external ADT types.
        for decl in &func.body.local_decls {
            collect_external_adts(&decl.ty, &mut discovered_types);
        }
        // Scan statement rvalues for field projection types.
        for bb in &func.body.basic_blocks {
            for stmt in &bb.statements {
                if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                    collect_external_adts_from_rvalue(rvalue, &mut discovered_types);
                }
            }
        }
    }

    // Discover external function calls and constants from MIR bodies.
    collect_external_function_calls(&all_bodies, &mut discovered_fns);
    collect_external_constants(&all_bodies, &mut discovered_consts);

    // --- Phase A: Materialize external types ---

    let to_materialize_types: Vec<String> = discovered_types
        .into_iter()
        .filter(|p| is_supported_external_crate(p))
        .map(|p| normalize_external_alias(&p).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    for path in &to_materialize_types {
        if !supported.contains_key(path.as_str())
            && hierarchy
                .cross_link
                .resolve_item(&CanonicalPath::new(path))
                .is_none()
        {
            return Err(SemanticLoweringError::UnsupportedExternalType {
                path: path.clone(),
                context: "not in supported external type set".into(),
            });
        }
    }

    let mut augmented = clone_hierarchy(hierarchy);
    let mut external_item_ids = Vec::new();

    for path in &to_materialize_types {
        if augmented
            .cross_link
            .resolve_item(&CanonicalPath::new(path))
            .is_some()
        {
            continue; // Already in hierarchy (dedup).
        }
        if let Some(&kind) = supported.get(path.as_str()) {
            let variants = variant_meta.get(path.as_str());
            let id = materialize_external_type(
                &mut augmented.store,
                &mut augmented.cross_link,
                path,
                kind,
                variants,
            );
            external_item_ids.push(id);
        }
    }

    // --- Phase B: Materialize external functions ---

    // Filter to supported callable paths (not already classified infrastructure).
    let to_materialize_fns: Vec<String> = discovered_fns
        .into_iter()
        .filter(|p| is_supported_external_callable(p))
        .collect();

    // Validate all discovered external callables have metadata.
    for path in &to_materialize_fns {
        if !fn_meta.contains_key(path.as_str()) {
            return Err(SemanticLoweringError::UnsupportedExternalType {
                path: path.clone(),
                context: "discovered external callable has no shape-reconstruction metadata".into(),
            });
        }
    }

    for path in &to_materialize_fns {
        if augmented
            .cross_link
            .resolve_item(&CanonicalPath::new(path))
            .is_some()
        {
            continue;
        }
        if let Some(meta) = fn_meta.get(path.as_str()) {
            let ids = materialize_external_function(
                &mut augmented.store,
                &mut augmented.cross_link,
                meta,
            );
            external_item_ids.extend(ids);
        }
    }

    // --- Phase C: Materialize external constants ---

    let to_materialize_consts: Vec<String> = discovered_consts
        .into_iter()
        .filter(|p| is_supported_external_crate(p))
        .collect();

    for path in &to_materialize_consts {
        if !const_meta.contains_key(path.as_str()) {
            return Err(SemanticLoweringError::UnsupportedExternalType {
                path: path.clone(),
                context: "discovered external constant has no shape-reconstruction metadata".into(),
            });
        }
    }

    for path in &to_materialize_consts {
        if augmented
            .cross_link
            .resolve_item(&CanonicalPath::new(path))
            .is_some()
        {
            continue;
        }
        if let Some(meta) = const_meta.get(path.as_str()) {
            let ids = materialize_external_constant(
                &mut augmented.store,
                &mut augmented.cross_link,
                meta,
            );
            external_item_ids.extend(ids);
        }
    }

    // --- Create synthetic external module ---

    if !external_item_ids.is_empty() {
        augmented.modules.push(Module {
            name: "regseq".into(),
            canonical_path: CanonicalPath::new("regseq"),
            items: external_item_ids,
            provenance: Provenance::new("external:regseq"),
        });
    }

    Ok(augmented)
}

// ---------------------------------------------------------------------------
// MIR type scanning
// ---------------------------------------------------------------------------

/// Recursively collect external ADT paths from a MIR type.
fn collect_external_adts(ty: &MirTy, discovered: &mut BTreeSet<String>) {
    match ty {
        MirTy::Adt { path, args } => {
            if path.starts_with("regseq::") {
                discovered.insert(path.clone());
            }
            for arg in args {
                collect_external_adts(arg, discovered);
            }
        }
        MirTy::Ref { ty: inner, .. } => collect_external_adts(inner, discovered),
        MirTy::RawPtr { ty: inner, .. } => collect_external_adts(inner, discovered),
        MirTy::Array { ty: inner, .. } => collect_external_adts(inner, discovered),
        MirTy::Slice { ty: inner } => collect_external_adts(inner, discovered),
        MirTy::Tuple { elements } => {
            for e in elements {
                collect_external_adts(e, discovered);
            }
        }
        _ => {}
    }
}

/// Collect external ADT paths from rvalue field projection types.
fn collect_external_adts_from_rvalue(rvalue: &MirRvalue, discovered: &mut BTreeSet<String>) {
    if let MirRvalue::Aggregate {
        agg_kind: regseq_model::declaration::mir_body::MirAggregateKind::Adt { path, args, .. },
        ..
    } = rvalue
    {
        if path.starts_with("regseq::") {
            discovered.insert(path.clone());
        }
        for arg in args {
            collect_external_adts(arg, discovered);
        }
    }
}

// ---------------------------------------------------------------------------
// External function call discovery
// ---------------------------------------------------------------------------

/// Discover external function calls from MIR Call terminators.
///
/// Scans all reachable MIR bodies for `MirTy::FnDef { path }` in Call
/// terminators where the path belongs to a supported external crate and
/// is not already handled by a dedicated semantic classification.
fn collect_external_function_calls(
    all_bodies: &[&&MirFnBodyDecl],
    discovered: &mut BTreeSet<String>,
) {
    for func in all_bodies {
        for bb in &func.body.basic_blocks {
            if let Some(ref term) = bb.terminator {
                if let MirTerminatorKind::Call {
                    func: MirOperand::Constant { constant },
                    ..
                } = &term.kind
                {
                    if let MirTy::FnDef { path, .. } = &constant.ty {
                        if path.starts_with("regseq::") {
                            discovered.insert(path.clone());
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// External constant discovery
// ---------------------------------------------------------------------------

/// Discover external unevaluated constants from MIR operands.
///
/// Scans all reachable MIR bodies for `MirConstValue::Unsupported`
/// values with `"unevaluated:regseq::..."` descriptions.
fn collect_external_constants(all_bodies: &[&&MirFnBodyDecl], discovered: &mut BTreeSet<String>) {
    for func in all_bodies {
        for bb in &func.body.basic_blocks {
            // Scan statement operands.
            for stmt in &bb.statements {
                if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                    collect_constants_from_rvalue(rvalue, discovered);
                }
            }
            // Scan terminator operands (call args).
            if let Some(ref term) = bb.terminator {
                if let MirTerminatorKind::Call { args, .. } = &term.kind {
                    for arg in args {
                        collect_constant_from_operand(arg, discovered);
                    }
                }
            }
        }
    }
}

/// Collect external constant paths from an rvalue.
fn collect_constants_from_rvalue(rvalue: &MirRvalue, discovered: &mut BTreeSet<String>) {
    match rvalue {
        MirRvalue::Use { operand } => collect_constant_from_operand(operand, discovered),
        MirRvalue::BinaryOp { lhs, rhs, .. } => {
            collect_constant_from_operand(lhs, discovered);
            collect_constant_from_operand(rhs, discovered);
        }
        MirRvalue::UnaryOp { operand, .. } => {
            collect_constant_from_operand(operand, discovered);
        }
        _ => {}
    }
}

/// Collect external constant path from a single operand.
fn collect_constant_from_operand(operand: &MirOperand, discovered: &mut BTreeSet<String>) {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Unsupported { description } = &constant.value {
            if let Some(path) = description.strip_prefix("unevaluated:") {
                if path.starts_with("regseq::") {
                    discovered.insert(path.to_string());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Materialization
// ---------------------------------------------------------------------------

/// Materialize a single external type item into the store.
///
/// If variant metadata is provided for an enum, populates
/// `EnumDef.variants` with real `EnumVariantDef` entries.
fn materialize_external_type(
    store: &mut regseq_model::item::identity::ItemStore,
    cross_link: &mut regseq_model::item::cross_link::CrossLinkTable,
    path: &str,
    kind: ExternalTypeKind,
    variant_defs: Option<&Vec<(&str, Option<u64>)>>,
) -> ItemId {
    let canonical = CanonicalPath::new(path);
    let name = path.rsplit("::").next().unwrap_or(path).to_string();

    let item = match kind {
        ExternalTypeKind::Struct => Item::Struct(StructDef {
            id: ItemId(0), // Overwritten by store.push().
            canonical_path: canonical.clone(),
            name,
            fields: vec![],
            attributes: vec![],
            provenance: Provenance::new("external:regseq"),
        }),
        ExternalTypeKind::Enum => {
            let variants = variant_defs
                .map(|defs| {
                    defs.iter()
                        .map(|(vname, disc)| EnumVariantDef {
                            name: vname.to_string(),
                            discriminant: *disc,
                            fields: vec![],
                        })
                        .collect()
                })
                .unwrap_or_default();
            Item::Enum(EnumDef {
                id: ItemId(0),
                canonical_path: canonical.clone(),
                name,
                variants,
                provenance: Provenance::new("external:regseq"),
            })
        }
    };

    let id = store.push(item);
    cross_link.insert_item(canonical, id);
    id
}

/// Materialize an external function with coherent ownership.
///
/// Creates the owning type (if not already materialized), an `ImplDef`
/// for the owning type, and the `FunctionDef` as an inherent method.
///
/// Returns the list of newly created item IDs.
fn materialize_external_function(
    store: &mut regseq_model::item::identity::ItemStore,
    cross_link: &mut regseq_model::item::cross_link::CrossLinkTable,
    meta: &ExternalFunctionMeta,
) -> Vec<ItemId> {
    let mut ids = Vec::new();

    // Ensure the owning type exists (may already be materialized as a type item).
    let owner_id = if let Some(owner_path) = meta.owner_type {
        let owner_canonical = CanonicalPath::new(owner_path);
        match cross_link.resolve_item(&owner_canonical) {
            Some(id) => id,
            None => {
                // Materialize the owner as a struct (best default for external types
                // whose exact shape we don't know beyond ownership).
                let owner_name = owner_path.rsplit("::").next().unwrap_or(owner_path);
                let item = Item::Struct(StructDef {
                    id: ItemId(0),
                    canonical_path: owner_canonical.clone(),
                    name: owner_name.to_string(),
                    fields: vec![],
                    attributes: vec![],
                    provenance: Provenance::new("external:regseq"),
                });
                let id = store.push(item);
                cross_link.insert_item(owner_canonical, id);
                ids.push(id);
                id
            }
        }
    } else {
        ItemId(0) // Placeholder for free functions (no owner).
    };

    // Create the FunctionDef.
    let fn_canonical = CanonicalPath::new(meta.canonical_path);
    let kind = if meta.owner_type.is_some() {
        FunctionKind::InherentMethod { owner: owner_id }
    } else {
        FunctionKind::FreeFunction
    };

    let func_item = Item::Function(FunctionDef {
        id: ItemId(0),
        canonical_path: fn_canonical.clone(),
        name: meta.name.to_string(),
        kind,
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: Provenance::new("external:regseq"),
    });
    let fn_id = store.push(func_item);
    cross_link.insert_item(fn_canonical, fn_id);
    ids.push(fn_id);

    // Create an ImplDef if we have an owner.
    if let Some(owner_path) = meta.owner_type {
        let impl_path = format!("{owner_path}::<impl>");
        let impl_canonical = CanonicalPath::new(&impl_path);
        // Check if an impl already exists for this owner.
        if cross_link.resolve_item(&impl_canonical).is_none() {
            let impl_item = Item::Impl(ImplDef {
                id: ItemId(0),
                canonical_path: impl_canonical.clone(),
                self_type: TypeRef::Item { id: owner_id },
                trait_ref: None,
                methods: vec![fn_id],
                constants: vec![],
                provenance: Provenance::new("external:regseq"),
            });
            let impl_id = store.push(impl_item);
            cross_link.insert_item(impl_canonical, impl_id);
            ids.push(impl_id);
        } else {
            // ImplDef already exists — we would need to add the method to it.
            // For the current slice, each owner type has at most one function
            // materialized per discover_and_materialize call, so this case
            // is handled by materializing all functions for the same owner
            // together. If needed, extend to update existing ImplDef.
        }
    }

    ids
}

/// Materialize an external constant with ownership.
///
/// Ensures the owning type exists and creates a `ConstantDef`.
///
/// Returns the list of newly created item IDs.
fn materialize_external_constant(
    store: &mut regseq_model::item::identity::ItemStore,
    cross_link: &mut regseq_model::item::cross_link::CrossLinkTable,
    meta: &ExternalConstantMeta,
) -> Vec<ItemId> {
    let mut ids = Vec::new();

    // Ensure the owning type exists.
    let owner_canonical = CanonicalPath::new(meta.owner_type);
    let owner_id = match cross_link.resolve_item(&owner_canonical) {
        Some(id) => id,
        None => {
            let owner_name = meta
                .owner_type
                .rsplit("::")
                .next()
                .unwrap_or(meta.owner_type);
            let item = Item::Struct(StructDef {
                id: ItemId(0),
                canonical_path: owner_canonical.clone(),
                name: owner_name.to_string(),
                fields: vec![],
                attributes: vec![],
                provenance: Provenance::new("external:regseq"),
            });
            let id = store.push(item);
            cross_link.insert_item(owner_canonical, id);
            ids.push(id);
            id
        }
    };

    // Create the ConstantDef.
    let const_canonical = CanonicalPath::new(meta.canonical_path);
    let stable_name = meta.canonical_path.replace("::", "_").to_ascii_uppercase();
    let const_item = Item::Constant(ConstantDef {
        id: ItemId(0),
        canonical_path: const_canonical.clone(),
        name: meta.name.to_string(),
        owner: Some(owner_id),
        ty: TypeRef::Item { id: owner_id },
        value: meta.value.map(ConstValue::Integer),
        stable_name,
        provenance: Provenance::new("external:regseq"),
    });
    let const_id = store.push(const_item);
    cross_link.insert_item(const_canonical, const_id);
    ids.push(const_id);

    ids
}

/// Clone an ItemHierarchy for augmentation.
///
/// Preserves item ordering so all existing ItemId references remain valid.
fn clone_hierarchy(hierarchy: &ItemHierarchy) -> ItemHierarchy {
    let mut new_store = regseq_model::item::identity::ItemStore::new();
    for i in 0..hierarchy.store.len() {
        let id = ItemId(i as u32);
        let item = hierarchy.store.get(id).expect("contiguous store").clone();
        new_store.push(item);
    }
    ItemHierarchy {
        store: new_store,
        modules: hierarchy.modules.clone(),
        cross_link: hierarchy.cross_link.clone(),
        interrupt_classes: hierarchy.interrupt_classes.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_alias_error_kind() {
        assert_eq!(
            normalize_external_alias("regseq::ErrorKind"),
            "regseq::SerialErrors"
        );
    }

    #[test]
    fn normalize_alias_passthrough() {
        assert_eq!(normalize_external_alias("regseq::RxChar"), "regseq::RxChar");
    }

    #[test]
    fn supported_set_contains_expected_types() {
        let set = supported_external_types();
        assert!(set.contains_key("regseq::RxChar"));
        assert!(set.contains_key("regseq::RegseqError"));
        assert!(set.contains_key("regseq::SerialErrors"));
        assert!(set.contains_key("regseq::IrqResult"));
    }

    #[test]
    fn register_wrapper_types_are_not_external_items() {
        assert!(!is_supported_external_crate("regseq::ReadWrite"));
        assert!(!is_supported_external_crate("regseq::Mmio"));
        assert!(!is_supported_external_crate("regseq::Clock"));
    }

    #[test]
    fn discovery_collects_from_option_inner() {
        let ty = MirTy::Adt {
            path: "core::option::Option".into(),
            args: vec![MirTy::Adt {
                path: "regseq::RxChar".into(),
                args: vec![],
            }],
        };
        let mut discovered = BTreeSet::new();
        collect_external_adts(&ty, &mut discovered);
        assert!(discovered.contains("regseq::RxChar"));
    }

    // -----------------------------------------------------------------------
    // Enum variant metadata tests
    // -----------------------------------------------------------------------

    #[test]
    fn irq_result_has_two_variants() {
        let variants = external_enum_variants();
        let irq = variants
            .get("regseq::IrqResult")
            .expect("IrqResult in table");
        assert_eq!(irq.len(), 2);
        assert_eq!(irq[0].0, "Handled");
        assert_eq!(irq[0].1, Some(0));
        assert_eq!(irq[1].0, "NotHandled");
        assert_eq!(irq[1].1, Some(1));
    }

    #[test]
    fn materialized_enum_has_populated_variants() {
        use regseq_model::item::cross_link::CrossLinkTable;
        use regseq_model::item::identity::ItemStore;

        let mut store = ItemStore::new();
        let mut cl = CrossLinkTable::new();
        let variants = external_enum_variants();
        let variant_defs = variants.get("regseq::IrqResult");
        let id = materialize_external_type(
            &mut store,
            &mut cl,
            "regseq::IrqResult",
            ExternalTypeKind::Enum,
            variant_defs,
        );
        let item = store.get(id).expect("item in store");
        match item {
            Item::Enum(e) => {
                assert_eq!(e.name, "IrqResult");
                assert_eq!(e.variants.len(), 2);
                assert_eq!(e.variants[0].name, "Handled");
                assert_eq!(e.variants[0].discriminant, Some(0));
                assert_eq!(e.variants[1].name, "NotHandled");
                assert_eq!(e.variants[1].discriminant, Some(1));
            }
            other => panic!("expected Enum, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // External function materialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn rxchar_new_materialized_as_inherent_method() {
        use regseq_model::item::cross_link::CrossLinkTable;
        use regseq_model::item::identity::ItemStore;

        let mut store = ItemStore::new();
        let mut cl = CrossLinkTable::new();
        let meta = supported_external_functions();
        let rxchar_meta = meta.get("regseq::RxChar::new").expect("meta");
        let ids = materialize_external_function(&mut store, &mut cl, rxchar_meta);
        assert!(!ids.is_empty());

        // Find the FunctionDef.
        let fn_id = cl
            .resolve_item(&CanonicalPath::new("regseq::RxChar::new"))
            .expect("function in cross-link");
        let fn_item = store.get(fn_id).expect("item");
        match fn_item {
            Item::Function(f) => {
                assert_eq!(f.name, "new");
                assert!(matches!(f.kind, FunctionKind::InherentMethod { .. }));
                if let FunctionKind::InherentMethod { owner } = &f.kind {
                    // Owner should be the RxChar struct.
                    let owner_item = store.get(*owner).expect("owner");
                    match owner_item {
                        Item::Struct(s) => assert_eq!(s.name, "RxChar"),
                        other => panic!("expected Struct owner, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Function, got {other:?}"),
        }
    }

    #[test]
    fn external_impl_def_created_for_rxchar() {
        use regseq_model::item::cross_link::CrossLinkTable;
        use regseq_model::item::identity::ItemStore;

        let mut store = ItemStore::new();
        let mut cl = CrossLinkTable::new();
        let meta = supported_external_functions();
        let rxchar_meta = meta.get("regseq::RxChar::new").expect("meta");
        materialize_external_function(&mut store, &mut cl, rxchar_meta);

        let impl_id = cl
            .resolve_item(&CanonicalPath::new("regseq::RxChar::<impl>"))
            .expect("impl in cross-link");
        let impl_item = store.get(impl_id).expect("impl item");
        match impl_item {
            Item::Impl(i) => {
                assert!(i.trait_ref.is_none()); // Inherent impl.
                assert!(!i.methods.is_empty());
            }
            other => panic!("expected Impl, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // External constant materialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn serial_errors_overrun_materialized_as_constant() {
        use regseq_model::item::cross_link::CrossLinkTable;
        use regseq_model::item::identity::ItemStore;

        let mut store = ItemStore::new();
        let mut cl = CrossLinkTable::new();
        let meta = supported_external_constants();
        let overrun = meta.get("regseq::SerialErrors::OVERRUN").expect("meta");
        let ids = materialize_external_constant(&mut store, &mut cl, overrun);
        assert!(!ids.is_empty());

        let const_id = cl
            .resolve_item(&CanonicalPath::new("regseq::SerialErrors::OVERRUN"))
            .expect("constant in cross-link");
        let const_item = store.get(const_id).expect("item");
        match const_item {
            Item::Constant(c) => {
                assert_eq!(c.name, "OVERRUN");
                assert!(c.owner.is_some());
                assert_eq!(
                    c.value,
                    Some(regseq_model::item::constant::ConstValue::Integer(1))
                );
            }
            other => panic!("expected Constant, got {other:?}"),
        }
    }

    #[test]
    fn all_serial_errors_constants_have_metadata() {
        let meta = supported_external_constants();
        assert!(meta.contains_key("regseq::SerialErrors::OVERRUN"));
        assert!(meta.contains_key("regseq::SerialErrors::BREAK"));
        assert!(meta.contains_key("regseq::SerialErrors::PARITY"));
        assert!(meta.contains_key("regseq::SerialErrors::FRAMING"));
    }

    // -----------------------------------------------------------------------
    // Supported external callable predicate tests
    // -----------------------------------------------------------------------

    #[test]
    fn rxchar_new_is_supported_external_callable() {
        assert!(is_supported_external_callable("regseq::RxChar::new"));
    }

    #[test]
    fn serial_errors_empty_is_supported_external_callable() {
        assert!(is_supported_external_callable(
            "regseq::SerialErrors::empty"
        ));
    }

    #[test]
    fn infrastructure_methods_are_not_supported_external_callable() {
        assert!(!is_supported_external_callable("regseq::Clock::enable"));
        assert!(!is_supported_external_callable("regseq::ReadWrite::write"));
        assert!(!is_supported_external_callable("regseq::Serial::rx_char"));
        assert!(!is_supported_external_callable("regseq::Duration::from_us"));
    }

    #[test]
    fn non_regseq_paths_are_not_supported_external_callable() {
        assert!(!is_supported_external_callable("core::ops::BitOr::bitor"));
        assert!(!is_supported_external_callable("std::vec::Vec::new"));
    }
}
