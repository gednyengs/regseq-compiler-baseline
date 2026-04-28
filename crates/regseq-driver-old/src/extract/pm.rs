use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute, ItemKind};
use rustc_middle::ty::TyCtxt;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the PM gate surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmGateSurface {
    pub gates: Vec<PmGate>,
}

/// A `#[rseq::pm(...)]` gate on a driver type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmGate {
    /// Driver type name bearing the gate.
    pub driver_type: String,
    /// Properties extracted from the attribute.
    pub properties: PmGateProperties,
    /// Whether an accepted `impl PowerManagement` exists for this driver type.
    pub has_pm_impl: bool,
}

/// Extracted PM gate properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmGateProperties {
    pub idle: bool,
    pub wakeup_capable: bool,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from PM gate extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PmError {
    /// The `#[rseq::pm(...)]` attribute is syntactically malformed.
    MalformedAttribute(String),
    /// Duplicate key in one PM attribute.
    DuplicateKey { driver_type: String, key: String },
    /// Multiple `#[rseq::pm(...)]` attributes on the same driver type.
    DuplicateGate { driver_type: String },
    /// PM gate is present but no `impl PowerManagement for DriverType` found.
    MissingPmImpl { driver_type: String },
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the PM gate surface from HIR after analysis.
///
/// `pm_impl_types` is the set of driver type names that have an accepted
/// `impl PowerManagement for DriverType`.
pub fn extract_pm_surface<'tcx>(
    tcx: TyCtxt<'tcx>,
    pm_impl_types: &HashSet<String>,
) -> Result<PmGateSurface, PmError> {
    let mut gates = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        if let ItemKind::Struct(ident, _generics, _variant_data) = &item.kind {
            let driver_type = ident.name.as_str().to_string();
            let attrs = tcx.hir_attrs(item.hir_id());
            let pm_attrs: Vec<&Attribute> = attrs
                .iter()
                .filter(|a| rseq_attr_name(a) == Some("pm"))
                .collect();

            if pm_attrs.is_empty() {
                continue;
            }

            if pm_attrs.len() > 1 {
                return Err(PmError::DuplicateGate {
                    driver_type: driver_type.clone(),
                });
            }

            let properties = parse_pm_attr(pm_attrs[0], &driver_type)?;
            let has_pm_impl = pm_impl_types.contains(&driver_type);

            if !has_pm_impl {
                return Err(PmError::MissingPmImpl {
                    driver_type: driver_type.clone(),
                });
            }

            gates.push(PmGate {
                driver_type,
                properties,
                has_pm_impl,
            });
        }
    }

    Ok(PmGateSurface { gates })
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

fn parse_pm_attr(attr: &Attribute, driver_type: &str) -> Result<PmGateProperties, PmError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed(&format!(
            "expected tool attribute on type '{driver_type}'"
        )));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => {
            return Err(malformed(&format!(
                "expected delimited args on pm attribute for type '{driver_type}'"
            )));
        }
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut idle: Option<bool> = None;
    let mut wakeup_capable: Option<bool> = None;
    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i]).ok_or_else(|| {
            malformed(&format!(
                "expected key in pm attribute for type '{driver_type}'"
            ))
        })?;

        if !seen_keys.insert(key.clone()) {
            return Err(PmError::DuplicateKey {
                driver_type: driver_type.to_string(),
                key,
            });
        }

        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!(
                "expected '=' after '{key}' in pm attribute for type '{driver_type}'"
            )));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!(
                "expected value for '{key}' in pm attribute for type '{driver_type}'"
            )));
        }

        let value = extract_ident(trees[i]).ok_or_else(|| {
            malformed(&format!(
                "expected boolean value for '{key}' in pm attribute for type '{driver_type}'"
            ))
        })?;

        let bool_val = match value.as_str() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(malformed(&format!(
                    "expected 'true' or 'false' for '{key}' in pm attribute for type '{driver_type}', got '{value}'"
                )));
            }
        };

        match key.as_str() {
            "idle" => idle = Some(bool_val),
            "wakeup_capable" => wakeup_capable = Some(bool_val),
            _ => { /* unknown keys tolerated for forward-compat */ }
        }

        i += 1;
    }

    let idle = idle.ok_or_else(|| {
        malformed(&format!(
            "missing 'idle' in pm attribute for type '{driver_type}'"
        ))
    })?;
    let wakeup_capable = wakeup_capable.ok_or_else(|| {
        malformed(&format!(
            "missing 'wakeup_capable' in pm attribute for type '{driver_type}'"
        ))
    })?;

    Ok(PmGateProperties {
        idle,
        wakeup_capable,
    })
}

// ---------------------------------------------------------------------------
// Token helpers
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

fn malformed(msg: &str) -> PmError {
    PmError::MalformedAttribute(msg.to_string())
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
