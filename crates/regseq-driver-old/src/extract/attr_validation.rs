//! Cross-crate attribute visibility validation.
//!
//! Gate test for Phase 4: validates that `#[rseq::intrinsic("...")]`,
//! `#[rseq::framework_type("...")]`, `#[rseq::abstraction("...")]`, and
//! `#[rseq::pin_state(id = "...")]` attributes placed on facade crate types
//! are visible when extracting a dependent crate.

use std::collections::HashSet;

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{CrateNum, DefId, CRATE_DEF_INDEX};
use rustc_hir::{AttrArgs, Attribute, ItemKind};
use rustc_middle::ty::TyCtxt;

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Summary of cross-crate attribute visibility validation.
#[derive(Debug, Default)]
pub struct AttrValidationReport {
    /// `#[rseq::intrinsic("...")]` attributes found.
    pub intrinsics: Vec<FoundAttr>,
    /// `#[rseq::abstraction("...")]` attributes found.
    pub abstractions: Vec<FoundAttr>,
    /// `#[rseq::framework_type("...")]` attributes found.
    pub framework_types: Vec<FoundAttr>,
    /// `#[rseq::pin_state(id = "...")]` attributes found.
    pub pin_states: Vec<FoundAttr>,
}

impl AttrValidationReport {
    /// Total number of attributes found across all categories.
    pub fn total(&self) -> usize {
        self.intrinsics.len()
            + self.abstractions.len()
            + self.framework_types.len()
            + self.pin_states.len()
    }

    /// Log the report to stderr for diagnostic output.
    pub fn log(&self) {
        eprintln!(
            "attr-validation: intrinsics found: {}",
            self.intrinsics.len()
        );
        for a in &self.intrinsics {
            eprintln!(
                "  {} -> {:?} (from {})",
                a.def_path, a.value, a.source_crate
            );
        }
        eprintln!(
            "attr-validation: abstractions found: {}",
            self.abstractions.len()
        );
        for a in &self.abstractions {
            eprintln!(
                "  {} -> {:?} (from {})",
                a.def_path, a.value, a.source_crate
            );
        }
        eprintln!(
            "attr-validation: framework_types found: {}",
            self.framework_types.len()
        );
        for a in &self.framework_types {
            eprintln!(
                "  {} -> {:?} (from {})",
                a.def_path, a.value, a.source_crate
            );
        }
        eprintln!(
            "attr-validation: pin_states found: {}",
            self.pin_states.len()
        );
        for a in &self.pin_states {
            eprintln!(
                "  {} -> {:?} (from {})",
                a.def_path, a.value, a.source_crate
            );
        }
        eprintln!("attr-validation: total attributes found: {}", self.total());
    }
}

/// A single attribute discovered during validation.
#[derive(Debug)]
pub struct FoundAttr {
    /// Fully-qualified path of the item carrying the attribute.
    pub def_path: String,
    /// Attribute value (e.g. `"mmio_read"` from `#[rseq::intrinsic("mmio_read")]`).
    pub value: Option<String>,
    /// Name of the crate the item belongs to.
    pub source_crate: String,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Validate cross-crate visibility of `#[rseq::*]` attributes.
///
/// Scans the `regseq` facade crate for `intrinsic`, `abstraction`, and
/// `framework_type` attributes, and the local crate for `pin_state` attributes.
pub fn validate_cross_crate_attrs<'tcx>(tcx: TyCtxt<'tcx>) -> AttrValidationReport {
    let mut report = AttrValidationReport::default();

    // Scan the facade crate for intrinsic/abstraction/framework_type attrs.
    if let Some(regseq_crate) = find_crate_by_name(tcx, "regseq") {
        let crate_name = tcx.crate_name(regseq_crate).to_string();
        let root = DefId {
            krate: regseq_crate,
            index: CRATE_DEF_INDEX,
        };
        let mut visited = HashSet::new();
        walk_module(tcx, root, &crate_name, &mut visited, &mut report);
    } else {
        eprintln!("attr-validation: WARNING: regseq crate not found in dependencies");
    }

    // Scan local crate for pin_state attrs.
    scan_local_pin_states(tcx, &mut report);

    report
}

// ---------------------------------------------------------------------------
// External crate scanning
// ---------------------------------------------------------------------------

/// Find an external crate by name.
fn find_crate_by_name<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Option<CrateNum> {
    tcx.crates(())
        .iter()
        .copied()
        .find(|&krate| tcx.crate_name(krate).as_str() == name)
}

/// Recursively walk a module and check attributes on all items.
fn walk_module(
    tcx: TyCtxt<'_>,
    module_def_id: DefId,
    crate_name: &str,
    visited: &mut HashSet<DefId>,
    report: &mut AttrValidationReport,
) {
    let children = tcx.module_children(module_def_id);
    for child in children {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };

        // Only process items from this crate (skip re-exports from other crates).
        if def_id.krate != module_def_id.krate {
            continue;
        }

        // Skip already-visited items (avoids duplicates from re-exports).
        if !visited.insert(def_id) {
            continue;
        }

        // Check attributes on this item.
        check_def_attrs(tcx, def_id, crate_name, report);

        // Check methods in inherent impls for struct/enum types.
        let def_kind = tcx.def_kind(def_id);
        if matches!(def_kind, DefKind::Struct | DefKind::Enum) {
            check_inherent_methods(tcx, def_id, crate_name, visited, report);
        }

        // Recurse into submodules.
        if matches!(def_kind, DefKind::Mod) {
            walk_module(tcx, def_id, crate_name, visited, report);
        }
    }
}

/// Check attributes on a single DefId (works for both local and external items).
fn check_def_attrs(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    crate_name: &str,
    report: &mut AttrValidationReport,
) {
    let attrs = tcx.get_all_attrs(def_id);
    let def_path = tcx.def_path_str(def_id);

    for attr in attrs {
        let Some(name) = rseq_attr_name(attr) else {
            continue;
        };
        let value = rseq_attr_string_value(attr);
        let found = FoundAttr {
            def_path: def_path.clone(),
            value,
            source_crate: crate_name.to_string(),
        };

        match name {
            "intrinsic" => report.intrinsics.push(found),
            "abstraction" => report.abstractions.push(found),
            "framework_type" => report.framework_types.push(found),
            "pin_state" => report.pin_states.push(found),
            _ => {}
        }
    }
}

/// Check inherent methods of a struct/enum for rseq attributes.
fn check_inherent_methods(
    tcx: TyCtxt<'_>,
    type_def_id: DefId,
    crate_name: &str,
    visited: &mut HashSet<DefId>,
    report: &mut AttrValidationReport,
) {
    let impls = tcx.inherent_impls(type_def_id);
    for &impl_def_id in impls.iter() {
        for &item_id in tcx.associated_item_def_ids(impl_def_id) {
            if visited.insert(item_id) {
                check_def_attrs(tcx, item_id, crate_name, report);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Local crate scanning (pin_state)
// ---------------------------------------------------------------------------

/// Scan the local crate for `#[rseq::pin_state(id = "...")]` attributes.
///
/// Uses `tcx.get_all_attrs(def_id)` (the same DefId-based API used for
/// cross-crate items) to validate that the attribute access path is
/// consistent across all four attribute namespaces.
fn scan_local_pin_states(tcx: TyCtxt<'_>, report: &mut AttrValidationReport) {
    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        if !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..)) {
            continue;
        }

        let def_id = item.owner_id.def_id.to_def_id();
        let attrs = tcx.get_all_attrs(def_id);
        for attr in attrs {
            if rseq_attr_name(attr) == Some("pin_state") {
                let def_path = tcx.def_path_str(def_id);
                let value = rseq_attr_kv_value(attr, "id");
                report.pin_states.push(FoundAttr {
                    def_path,
                    value,
                    source_crate: "local".to_string(),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

/// Return the tool name if this is an `#[rseq::*]` tool attribute.
fn rseq_attr_name(attr: &Attribute) -> Option<&str> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let segs = &item.path.segments;
    if segs.len() == 2 && segs[0].as_str() == "rseq" {
        Some(segs[1].as_str())
    } else {
        None
    }
}

/// Extract the string value from `#[rseq::xxx("value")]`.
fn rseq_attr_string_value(attr: &Attribute) -> Option<String> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let AttrArgs::Delimited(delimited) = &item.args else {
        return None;
    };

    for tt in delimited.tokens.iter() {
        if let TokenTree::Token(token, _) = tt {
            if let TokenKind::Literal(lit) = &token.kind {
                if lit.kind == LitKind::Str {
                    return Some(lit.symbol.as_str().to_string());
                }
            }
        }
    }
    None
}

/// Extract the value for a named key from `#[rseq::xxx(key = "value")]`.
fn rseq_attr_kv_value(attr: &Attribute, key: &str) -> Option<String> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let AttrArgs::Delimited(delimited) = &item.args else {
        return None;
    };

    let trees: Vec<&TokenTree> = delimited.tokens.iter().collect();
    for window in trees.windows(3) {
        if let (
            TokenTree::Token(key_tok, _),
            TokenTree::Token(_, _),
            TokenTree::Token(val_tok, _),
        ) = (window[0], window[1], window[2])
        {
            if let TokenKind::Ident(sym, _) = key_tok.kind {
                if sym.as_str() == key {
                    if let TokenKind::Literal(lit) = &val_tok.kind {
                        if lit.kind == LitKind::Str {
                            return Some(lit.symbol.as_str().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}
