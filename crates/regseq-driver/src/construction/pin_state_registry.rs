//! Registry mapping [`DefId`]s of `#[rseq::pin_state(id = "...")]`-annotated
//! types to their authored pin-state identifier strings.
//!
//! Construction consumes this registry directly rather than the broader
//! `PinCtrlSurface::pin_states` list so that Pass 1 lookups are
//! `DefId`-keyed and O(1).

use std::collections::HashMap;

use rustc_ast::token::{Delimiter, LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::TyCtxt;

/// Registry mapping `DefId`s of pin-state marker types to their
/// declared `state_id` strings.
#[derive(Debug, Default)]
pub struct PinStateRegistry {
    entries: HashMap<DefId, String>,
}

impl PinStateRegistry {
    /// Build the registry by scanning the local crate for
    /// `#[rseq::pin_state(id = "...")]`-annotated types.
    pub fn build(tcx: TyCtxt<'_>) -> Self {
        let mut entries: HashMap<DefId, String> = HashMap::new();

        for item_id in tcx.hir_free_items() {
            let item = tcx.hir_item(item_id);
            if !matches!(
                item.kind,
                rustc_hir::ItemKind::Struct(..) | rustc_hir::ItemKind::Enum(..)
            ) {
                continue;
            }
            let def_id = item.owner_id.def_id.to_def_id();
            record(tcx, def_id, &mut entries);
        }
        let _ = LOCAL_CRATE;

        Self { entries }
    }

    /// Look up the canonical pin-state identifier for a type's `DefId`.
    pub fn lookup(&self, def_id: DefId) -> Option<&str> {
        self.entries.get(&def_id).map(String::as_str)
    }

    /// Number of registered pin-state markers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over `(DefId, state_id)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (DefId, &str)> + '_ {
        self.entries.iter().map(|(&id, name)| (id, name.as_str()))
    }
}

fn record(tcx: TyCtxt<'_>, def_id: DefId, entries: &mut HashMap<DefId, String>) {
    for attr in tcx.get_all_attrs(def_id) {
        if rseq_attr_name(attr) != Some("pin_state") {
            continue;
        }
        if let Some(id) = parse_state_id(attr) {
            entries.insert(def_id, id);
            return;
        }
    }
}

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

fn parse_state_id(attr: &Attribute) -> Option<String> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return None,
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut i = 0;
    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }
        let key = extract_ident(trees[i])?;
        i += 1;
        if i >= trees.len() || !is_eq(trees[i]) {
            return None;
        }
        i += 1;
        if i >= trees.len() {
            return None;
        }
        if key == "id" {
            return extract_string_literal(trees[i]);
        }
        i += 1;
    }
    None
}

fn is_comma(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Comma))
}

fn is_eq(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Eq))
}

fn extract_ident(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Ident(sym, _) => Some(sym.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn extract_string_literal(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Literal(lit) if lit.kind == LitKind::Str => {
                Some(lit.symbol.as_str().to_string())
            }
            _ => None,
        },
        _ => None,
    }
}

// Keep Delimiter referenced so `use` stays warning-free if later added.
const _: Option<Delimiter> = None;
