//! Shared parser for `#[rseq::<name>(...)]` attribute payloads.
//!
//! Consumed by Pass 1 constructors in `walk.rs`, `driver.rs`, and `impls.rs`
//! when populating the `Attrs` bag on types, driver-struct fields, functions,
//! and ISR markers. Preserves positional strings, keyword-arg maps, and
//! presence-only markers so downstream passes (task 52 PinCtrl metadata,
//! task 43 MIR lowering of ISR metadata) can read payloads faithfully.

use regseq_model::model::{AttrEntry, AttrValue, Attrs};
use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;

use super::{ConstructionError, ErrorProvenance};

/// The set of `#[rseq::<name>]` attribute names Pass 1 recognizes. Any
/// attribute outside this set on an item visited by Pass 1 triggers
/// [`ConstructionError::UnknownRseqAttribute`] so typos and stale names
/// fail loudly instead of being silently dropped.
const KNOWN_RSEQ_ATTRS: &[&str] = &[
    "driver",
    "register_block",
    "pin_state",
    "pinctrl",
    "clock",
    "reset",
    "interrupt",
    "mmio",
    "property",
    "isr",
    "framework_type",
    "abstraction",
];

/// Collect every `#[rseq::<name>(...)]` attribute on `def_id` into an
/// [`Attrs`] bag, preserving argument payloads. Errors on any
/// `#[rseq::<name>]` whose short name is not in [`KNOWN_RSEQ_ATTRS`].
pub(super) fn collect_rseq_attrs(
    tcx: TyCtxt<'_>,
    def_id: DefId,
) -> Result<Attrs, ConstructionError> {
    let mut entries = Vec::new();
    for attr in tcx.get_all_attrs(def_id) {
        let Some(entry) = parse_rseq_attr(attr) else {
            continue;
        };
        let short = entry
            .key
            .strip_prefix("rseq::")
            .expect("parse_rseq_attr emits rseq::-prefixed keys");
        if !KNOWN_RSEQ_ATTRS.contains(&short) {
            return Err(ConstructionError::UnknownRseqAttribute {
                provenance: ErrorProvenance::for_def_id(tcx, def_id),
                attr: entry.key,
            });
        }
        entries.push(entry);
    }
    Ok(Attrs { entries })
}

/// Parse a single attribute into an [`AttrEntry`] if it is an `#[rseq::...]`
/// tool attribute. Returns `None` for non-rseq attributes.
fn parse_rseq_attr(attr: &Attribute) -> Option<AttrEntry> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let segs = &item.path.segments;
    if segs.len() != 2 || segs[0].as_str() != "rseq" {
        return None;
    }
    let key = format!("rseq::{}", segs[1].as_str());
    let value = parse_attr_args(&item.args);
    Some(AttrEntry { key, value })
}

/// Parse the argument list of an `#[rseq::name(...)]` attribute into an
/// [`AttrValue`]. Recognizes:
/// - No args / non-delimited args: [`AttrValue::Unit`]
/// - Single positional literal: [`AttrValue::String`], [`AttrValue::Int`],
///   or [`AttrValue::Bool`]
/// - Keyword-arg map: [`AttrValue::KeyValues`]
///
/// Unrecognized tokens fall back to [`AttrValue::Unit`] for the whole
/// attribute. This is permissive by design -- Pass 1 captures authored
/// attrs faithfully but does not validate them.
fn parse_attr_args(args: &AttrArgs) -> AttrValue {
    let AttrArgs::Delimited(delimited) = args else {
        return AttrValue::Unit;
    };
    let trees: Vec<&TokenTree> = delimited.tokens.iter().collect();
    if trees.is_empty() {
        return AttrValue::Unit;
    }

    // Peek: if the first meaningful token is a literal and it is not followed
    // by '=', treat the whole payload as a single positional value.
    if let Some(value) = try_parse_positional(&trees) {
        return value;
    }

    parse_keyword_args(&trees)
}

/// If the argument list is a single positional literal (e.g.
/// `#[rseq::framework_type("clock_controller")]`), return it. Returns
/// `None` when the list starts with a keyword (`ident = ...`).
fn try_parse_positional(trees: &[&TokenTree]) -> Option<AttrValue> {
    let first = trees.first()?;
    let lit_value = token_to_literal(first)?;
    // Positional form: the literal must be the only meaningful token.
    // Anything following (other than trailing commas) rules out positional.
    for t in &trees[1..] {
        if !is_comma(t) {
            return None;
        }
    }
    Some(lit_value)
}

fn parse_keyword_args(trees: &[&TokenTree]) -> AttrValue {
    let mut entries: Vec<AttrEntry> = Vec::new();
    let mut i = 0;
    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }
        // Require `ident = <value>`. If the structure doesn't hold at this
        // position, skip to the next top-level comma and try again so one
        // unrecognized keyword (e.g. a list literal) doesn't discard the
        // entire attribute.
        let Some(key) = token_to_ident(trees[i]) else {
            i = advance_past_next_comma(trees, i + 1);
            continue;
        };
        if i + 1 >= trees.len() || !is_eq(trees[i + 1]) {
            i = advance_past_next_comma(trees, i + 1);
            continue;
        }
        if i + 2 >= trees.len() {
            break;
        }
        if let Some(value) = token_to_literal(trees[i + 2]) {
            entries.push(AttrEntry { key, value });
        }
        // Whether or not we parsed the value, scan to the next comma so we
        // skip past multi-token values like `[lit, lit]`.
        i = advance_past_next_comma(trees, i + 3);
    }
    if entries.is_empty() {
        AttrValue::Unit
    } else {
        AttrValue::KeyValues(entries)
    }
}

fn advance_past_next_comma(trees: &[&TokenTree], mut i: usize) -> usize {
    while i < trees.len() && !is_comma(trees[i]) {
        i += 1;
    }
    if i < trees.len() {
        i += 1;
    }
    i
}

fn token_to_literal(tree: &TokenTree) -> Option<AttrValue> {
    let TokenTree::Token(tok, _) = tree else {
        return None;
    };
    match &tok.kind {
        TokenKind::Literal(lit) => match lit.kind {
            LitKind::Str => Some(AttrValue::String(lit.symbol.as_str().to_owned())),
            LitKind::Integer => lit.symbol.as_str().parse::<i64>().ok().map(AttrValue::Int),
            _ => None,
        },
        // `true` / `false` arrive as bare identifiers in tool-attribute token
        // streams, not as `LitKind::Bool`.
        TokenKind::Ident(sym, _) => match sym.as_str() {
            "true" => Some(AttrValue::Bool(true)),
            "false" => Some(AttrValue::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

fn token_to_ident(tree: &TokenTree) -> Option<String> {
    let TokenTree::Token(tok, _) = tree else {
        return None;
    };
    match &tok.kind {
        TokenKind::Ident(sym, _) => Some(sym.as_str().to_owned()),
        _ => None,
    }
}

fn is_comma(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Comma))
}

fn is_eq(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Eq))
}
