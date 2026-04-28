//! Abstraction type registry extraction.
//!
//! Builds a mapping from [`DefId`] to canonical abstraction identifier for
//! every `#[rseq::abstraction("...")]`-annotated type reachable through the
//! `regseq` facade crate.
//!
//! This lets the compiler recognize Regseq abstraction types (e.g. `Clock`,
//! `Mmio`, `SpinLock`) by attribute rather than by stringified path heuristics.

use std::collections::{HashMap, HashSet};

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{CrateNum, DefId, CRATE_DEF_INDEX};
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::TyCtxt;

/// Registry mapping `DefId`s of abstraction-annotated types to their
/// canonical abstraction identifiers.
#[derive(Debug, Default)]
pub struct AbstractionTypeRegistry {
    entries: HashMap<DefId, String>,
}

impl AbstractionTypeRegistry {
    /// Construct the registry directly from an iterator of
    /// `(DefId, canonical_name)` pairs.
    ///
    /// Used by integration tests that need to stand up a synthetic
    /// registry around arbitrary local ADTs without rebuilding the
    /// real `regseq` facade. The production entry point is
    /// [`Self::build`].
    //
    // Marked `allow(dead_code)` because `tests/abstraction_types.rs`
    // includes this file via `#[path = ...]` to test the production
    // walker in isolation; that test harness does not call the
    // synthetic constructor, which would otherwise flag a dead-code
    // warning inside the test-target build.
    #[allow(dead_code)]
    pub fn from_entries<I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (DefId, String)>,
    {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Build the registry by scanning all items in the `regseq` facade crate
    /// for `#[rseq::abstraction("...")]` attributes.
    pub fn build(tcx: TyCtxt<'_>) -> Self {
        let mut entries: HashMap<DefId, String> = HashMap::new();

        if let Some(regseq_crate) = find_crate_by_name(tcx, "regseq") {
            let root = DefId {
                krate: regseq_crate,
                index: CRATE_DEF_INDEX,
            };
            let mut visited = HashSet::new();
            collect_abstractions(tcx, root, &mut visited, &mut entries);
        }

        Self { entries }
    }

    /// Look up the canonical abstraction identifier for a type's `DefId`.
    pub fn lookup(&self, def_id: DefId) -> Option<&str> {
        self.entries.get(&def_id).map(String::as_str)
    }

    /// Return the number of registered abstraction types.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over `(DefId, identifier)` pairs in the registry.
    pub fn iter(&self) -> impl Iterator<Item = (DefId, &str)> + '_ {
        self.entries.iter().map(|(&id, name)| (id, name.as_str()))
    }
}

// ---------------------------------------------------------------------------
// Crate discovery
// ---------------------------------------------------------------------------

fn find_crate_by_name(tcx: TyCtxt<'_>, name: &str) -> Option<CrateNum> {
    tcx.crates(())
        .iter()
        .copied()
        .find(|&krate| tcx.crate_name(krate).as_str() == name)
}

// ---------------------------------------------------------------------------
// Module walking
// ---------------------------------------------------------------------------

/// Recursively walk a module, collecting abstraction annotations on items.
fn collect_abstractions(
    tcx: TyCtxt<'_>,
    module_def_id: DefId,
    visited: &mut HashSet<DefId>,
    entries: &mut HashMap<DefId, String>,
) {
    for child in tcx.module_children(module_def_id) {
        let Some(def_id) = child.res.opt_def_id() else {
            continue;
        };

        if def_id.krate != module_def_id.krate {
            continue;
        }

        if !visited.insert(def_id) {
            continue;
        }

        record_abstraction(tcx, def_id, entries);

        if matches!(tcx.def_kind(def_id), DefKind::Mod) {
            collect_abstractions(tcx, def_id, visited, entries);
        }
    }
}

/// If `def_id` carries `#[rseq::abstraction("name")]`, record the mapping.
fn record_abstraction(tcx: TyCtxt<'_>, def_id: DefId, entries: &mut HashMap<DefId, String>) {
    for attr in tcx.get_all_attrs(def_id) {
        if rseq_attr_name(attr) != Some("abstraction") {
            continue;
        }
        if let Some(name) = rseq_attr_string_value(attr) {
            entries.insert(def_id, name);
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

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
