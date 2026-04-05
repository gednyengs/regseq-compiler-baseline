use rustc_ast::token::{Delimiter, LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::TyCtxt;

/// Accepted portable bus taxonomy values for `#[rseq::driver(bus = "...")]`.
const ACCEPTED_BUS_VALUES: &[&str] = &["integrated", "amba", "pci"];

/// Extracted driver identity attributes from HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverIdentity {
    /// From `#[rseq::driver(name = "...")]`.
    pub driver_name: String,
    /// From `#[rseq::driver(compatible = [...])]`.
    pub compatible: Vec<String>,
    /// From `#[rseq::device_class(...)]`.
    pub device_class: String,
    /// From `#[rseq::driver(bus = "...")]`. Optional portable bus/fabric identity.
    pub bus: Option<String>,
}

/// Errors during driver identity extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverIdentityError {
    MissingDriverName,
    MissingCompatible,
    MissingDeviceClass,
    MalformedAttribute(String),
    DuplicateAttribute(String),
    UnknownBusValue(String),
}

/// Extract driver identity information from all `#[rseq::driver(...)]`-annotated
/// items in the crate.
pub fn extract_driver_identities<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<DriverIdentity>, DriverIdentityError> {
    let mut identities = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let attrs = tcx.hir_attrs(item.hir_id());

        let driver_attrs: Vec<_> = attrs
            .iter()
            .filter(|a| rseq_attr_name(a) == Some("driver"))
            .collect();

        if driver_attrs.is_empty() {
            continue;
        }

        if driver_attrs.len() > 1 {
            return Err(DriverIdentityError::DuplicateAttribute("driver".into()));
        }

        let parsed = parse_driver_attr(driver_attrs[0])?;
        let driver_name = parsed.name.ok_or(DriverIdentityError::MissingDriverName)?;
        let compatible = parsed
            .compatible
            .ok_or(DriverIdentityError::MissingCompatible)?;

        let class_attrs: Vec<_> = attrs
            .iter()
            .filter(|a| rseq_attr_name(a) == Some("device_class"))
            .collect();

        if class_attrs.is_empty() {
            return Err(DriverIdentityError::MissingDeviceClass);
        }
        if class_attrs.len() > 1 {
            return Err(DriverIdentityError::DuplicateAttribute(
                "device_class".into(),
            ));
        }

        let device_class = parse_device_class_attr(class_attrs[0])?;

        identities.push(DriverIdentity {
            driver_name,
            compatible,
            device_class,
            bus: parsed.bus,
        });
    }

    Ok(identities)
}

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

/// Intermediate result from parsing `#[rseq::driver(...)]` key-value pairs.
struct ParsedDriverAttr {
    name: Option<String>,
    compatible: Option<Vec<String>>,
    bus: Option<String>,
}

/// Parse `#[rseq::driver(name = "...", compatible = [...], bus = "...")]`.
fn parse_driver_attr(attr: &Attribute) -> Result<ParsedDriverAttr, DriverIdentityError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => {
            return Err(malformed(
                "driver attribute requires parenthesized arguments",
            ))
        }
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut driver_name = None;
    let mut compatible = None;
    let mut bus = None;
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i])
            .ok_or_else(|| malformed("expected identifier key in driver attribute"))?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!("expected '=' after '{key}'")));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!("expected value for '{key}'")));
        }

        match key.as_str() {
            "name" => {
                if driver_name.is_some() {
                    return Err(DriverIdentityError::DuplicateAttribute("name".into()));
                }
                let val = extract_string_literal(trees[i])
                    .ok_or_else(|| malformed("name must be a string literal"))?;
                driver_name = Some(val);
            }
            "compatible" => {
                if compatible.is_some() {
                    return Err(DriverIdentityError::DuplicateAttribute("compatible".into()));
                }
                let list = extract_string_list(trees[i])?;
                compatible = Some(list);
            }
            "bus" => {
                if bus.is_some() {
                    return Err(DriverIdentityError::DuplicateAttribute("bus".into()));
                }
                let val = extract_string_literal(trees[i])
                    .ok_or_else(|| malformed("bus must be a string literal"))?;
                if !ACCEPTED_BUS_VALUES.contains(&val.as_str()) {
                    return Err(DriverIdentityError::UnknownBusValue(val));
                }
                bus = Some(val);
            }
            other => {
                return Err(malformed(&format!(
                    "unknown driver attribute key: '{other}'"
                )));
            }
        }
        i += 1;
    }

    Ok(ParsedDriverAttr {
        name: driver_name,
        compatible,
        bus,
    })
}

/// Parse `#[rseq::device_class(serial)]`.
fn parse_device_class_attr(attr: &Attribute) -> Result<String, DriverIdentityError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed("expected tool attribute"));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return Err(malformed("device_class requires parenthesized arguments")),
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    if trees.len() != 1 {
        return Err(malformed(
            "device_class attribute expects exactly one identifier",
        ));
    }

    extract_ident(trees[0]).ok_or_else(|| malformed("device_class value must be an identifier"))
}

fn malformed(msg: &str) -> DriverIdentityError {
    DriverIdentityError::MalformedAttribute(msg.to_string())
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

fn extract_string_list(tree: &TokenTree) -> Result<Vec<String>, DriverIdentityError> {
    match tree {
        TokenTree::Delimited(_, _, Delimiter::Bracket, inner) => {
            let mut items = Vec::new();
            for t in inner.iter() {
                if is_comma(t) {
                    continue;
                }
                let s = extract_string_literal(t)
                    .ok_or_else(|| malformed("compatible list items must be string literals"))?;
                items.push(s);
            }
            Ok(items)
        }
        _ => Err(malformed(
            "compatible value must be a bracket-delimited list",
        )),
    }
}
