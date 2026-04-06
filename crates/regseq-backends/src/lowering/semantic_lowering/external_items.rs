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
use regseq_model::declaration::mir_body::{MirRvalue, MirStatementKind, MirTy};
use regseq_model::declaration::Provenance;
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::module::Module;
use regseq_model::item::types::{EnumDef, Item, StructDef};

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
/// Scans all MIR bodies in scope for `MirTy::Adt` paths from supported
/// external crates. Materializes discovered items as real `ItemStore`
/// entries in a synthetic external module.
///
/// Returns an augmented `ItemHierarchy` with the external items added.
pub fn discover_and_materialize(
    hierarchy: &ItemHierarchy,
    input: &AnalysisInput,
) -> Result<ItemHierarchy, SemanticLoweringError> {
    let supported = supported_external_types();
    let mut discovered: BTreeSet<String> = BTreeSet::new();

    // Scan all MIR bodies for external ADT paths.
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
        // Scan local declarations.
        for decl in &func.body.local_decls {
            collect_external_adts(&decl.ty, &mut discovered);
        }
        // Scan statement rvalues for field projection types.
        for bb in &func.body.basic_blocks {
            for stmt in &bb.statements {
                if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                    collect_external_adts_from_rvalue(rvalue, &mut discovered);
                }
            }
        }
    }

    // Filter to supported external crate paths and normalize aliases.
    let to_materialize: Vec<String> = discovered
        .into_iter()
        .filter(|p| is_supported_external_crate(p))
        .map(|p| normalize_external_alias(&p).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // Validate all discovered types are in the supported set.
    for path in &to_materialize {
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

    // Clone the hierarchy and add external items.
    let mut augmented = clone_hierarchy(hierarchy);

    let mut external_item_ids = Vec::new();
    for path in &to_materialize {
        if augmented
            .cross_link
            .resolve_item(&CanonicalPath::new(path))
            .is_some()
        {
            continue; // Already in hierarchy (dedup).
        }
        if let Some(&kind) = supported.get(path.as_str()) {
            let id = materialize_external_item(
                &mut augmented.store,
                &mut augmented.cross_link,
                path,
                kind,
            );
            external_item_ids.push(id);
        }
    }

    // Create a synthetic external module for the regseq crate.
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
// Materialization
// ---------------------------------------------------------------------------

/// Materialize a single external item into the store.
fn materialize_external_item(
    store: &mut regseq_model::item::identity::ItemStore,
    cross_link: &mut regseq_model::item::cross_link::CrossLinkTable,
    path: &str,
    kind: ExternalTypeKind,
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
        ExternalTypeKind::Enum => Item::Enum(EnumDef {
            id: ItemId(0),
            canonical_path: canonical.clone(),
            name,
            variants: vec![],
            provenance: Provenance::new("external:regseq"),
        }),
    };

    let id = store.push(item);
    cross_link.insert_item(canonical, id);
    id
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
}
