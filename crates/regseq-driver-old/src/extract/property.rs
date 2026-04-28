use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute, ItemKind};
use rustc_middle::ty::TyCtxt;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Record types
// ---------------------------------------------------------------------------

/// Top-level extraction result for the firmware-property declaration surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertySurface {
    pub fields: Vec<PropertyField>,
    pub lints: Vec<PropertyLint>,
}

/// A field annotated with `#[rseq::property(...)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyField {
    /// Name of the driver struct containing this field.
    pub driver_type: String,
    /// Field name.
    pub field_name: String,
    /// Recoverable field type path (e.g. "u32", "Option < u32 >").
    pub field_type: String,
    /// Value of the `id = "..."` key.
    pub property_id: String,
    /// Raw literal payload of `default = ...`, if present.
    pub default_literal: Option<String>,
    /// Value of `vendor_prefix = "..."`, if present.
    pub vendor_prefix: Option<String>,
    /// `true` when the field type is `T` (required), `false` when `Option<T>`.
    pub required: bool,
}

/// Non-fatal declaration-side lint facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyLint {
    /// A required field (`T`) also declares a `default`.
    RequiredWithDefault {
        driver_type: String,
        field_name: String,
        property_id: String,
    },
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from property declaration extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    /// The `#[rseq::property(...)]` attribute is syntactically malformed.
    MalformedAttribute(String),
    /// Missing required `id = "..."` key.
    MissingPropertyId(String),
    /// Duplicate property ids within one driver type.
    DuplicatePropertyId {
        driver_type: String,
        property_id: String,
    },
}

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Extract the firmware-property declaration surface from HIR after analysis.
pub fn extract_property_surface<'tcx>(tcx: TyCtxt<'tcx>) -> Result<PropertySurface, PropertyError> {
    let mut fields = Vec::new();
    let mut lints = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);

        if let ItemKind::Struct(ident, _generics, variant_data) = &item.kind {
            let driver_type = ident.name.as_str().to_string();
            extract_property_fields(tcx, &driver_type, variant_data, &mut fields, &mut lints)?;
        }
    }

    Ok(PropertySurface { fields, lints })
}

// ---------------------------------------------------------------------------
// Property field extraction
// ---------------------------------------------------------------------------

fn extract_property_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    driver_type: &str,
    variant_data: &rustc_hir::VariantData<'tcx>,
    fields: &mut Vec<PropertyField>,
    lints: &mut Vec<PropertyLint>,
) -> Result<(), PropertyError> {
    let mut seen_ids: HashSet<String> = HashSet::new();

    for field in variant_data.fields() {
        let attrs = tcx.hir_attrs(field.hir_id);
        let prop_attr = attrs.iter().find(|a| rseq_attr_name(a) == Some("property"));

        let Some(attr) = prop_attr else {
            continue;
        };

        let field_name = field.ident.name.as_str().to_string();
        let parsed = parse_property_attr(attr, &field_name)?;

        let property_id = parsed.id.ok_or_else(|| {
            PropertyError::MissingPropertyId(format!("{}.{}", driver_type, field_name))
        })?;

        if !seen_ids.insert(property_id.clone()) {
            return Err(PropertyError::DuplicatePropertyId {
                driver_type: driver_type.to_string(),
                property_id,
            });
        }

        let (required, field_type) = classify_field_type(field.ty);

        if required && parsed.default_literal.is_some() {
            lints.push(PropertyLint::RequiredWithDefault {
                driver_type: driver_type.to_string(),
                field_name: field_name.clone(),
                property_id: property_id.clone(),
            });
        }

        fields.push(PropertyField {
            driver_type: driver_type.to_string(),
            field_name,
            field_type,
            property_id,
            default_literal: parsed.default_literal,
            vendor_prefix: parsed.vendor_prefix,
            required,
        });
    }

    Ok(())
}

/// Classify a field type as required (`T` -> true) or optional (`Option<T>` -> false).
/// Returns `(required, type_path_string)`.
fn classify_field_type(ty: &rustc_hir::Ty<'_>) -> (bool, String) {
    let type_str = hir_ty_to_string(ty);

    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return (true, type_str);
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return (true, type_str),
    };
    let Some(last_seg) = path.segments.last() else {
        return (true, type_str);
    };

    if last_seg.ident.name.as_str() == "Option" {
        (false, type_str)
    } else {
        (true, type_str)
    }
}

/// Produce a recoverable string representation of a HIR type.
fn hir_ty_to_string(ty: &rustc_hir::Ty<'_>) -> String {
    let rustc_hir::TyKind::Path(qpath) = &ty.kind else {
        return "<unsupported>".to_string();
    };
    let path = match qpath {
        rustc_hir::QPath::Resolved(_, path) => path,
        _ => return "<unsupported>".to_string(),
    };

    let mut parts = Vec::new();
    for seg in path.segments.iter() {
        let mut s = seg.ident.name.as_str().to_string();
        if let Some(args) = seg.args {
            let inner: Vec<String> = args
                .args
                .iter()
                .filter_map(|arg| {
                    if let rustc_hir::GenericArg::Type(inner_ty) = arg {
                        if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p)) =
                            &inner_ty.kind
                        {
                            Some(
                                p.segments
                                    .iter()
                                    .map(|s| s.ident.name.as_str().to_string())
                                    .collect::<Vec<_>>()
                                    .join("::"),
                            )
                        } else {
                            Some("<unsupported>".to_string())
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if !inner.is_empty() {
                s.push('<');
                s.push_str(&inner.join(", "));
                s.push('>');
            }
        }
        parts.push(s);
    }

    parts.join("::")
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

struct ParsedPropertyAttr {
    id: Option<String>,
    default_literal: Option<String>,
    vendor_prefix: Option<String>,
}

fn parse_property_attr(
    attr: &Attribute,
    field_name: &str,
) -> Result<ParsedPropertyAttr, PropertyError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(malformed(&format!(
            "expected tool attribute on field '{field_name}'"
        )));
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => {
            return Err(malformed(&format!(
                "expected delimited args on property attribute for field '{field_name}'"
            )));
        }
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut result = ParsedPropertyAttr {
        id: None,
        default_literal: None,
        vendor_prefix: None,
    };
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = extract_ident(trees[i]).ok_or_else(|| {
            malformed(&format!(
                "expected key in property attribute for field '{field_name}'"
            ))
        })?;
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            return Err(malformed(&format!(
                "expected '=' after '{key}' in property attribute for field '{field_name}'"
            )));
        }
        i += 1;

        if i >= trees.len() {
            return Err(malformed(&format!(
                "expected value for '{key}' in property attribute for field '{field_name}'"
            )));
        }

        match key.as_str() {
            "id" => {
                result.id = Some(extract_string_literal(trees[i]).ok_or_else(|| {
                    malformed(&format!(
                        "expected string literal for 'id' in property attribute for field '{field_name}'"
                    ))
                })?);
            }
            "default" => {
                result.default_literal = Some(extract_literal_raw(trees[i]).ok_or_else(|| {
                    malformed(&format!(
                        "expected literal for 'default' in property attribute for field '{field_name}'"
                    ))
                })?);
            }
            "vendor_prefix" => {
                result.vendor_prefix = Some(extract_string_literal(trees[i]).ok_or_else(|| {
                    malformed(&format!(
                        "expected string literal for 'vendor_prefix' in property attribute for field '{field_name}'"
                    ))
                })?);
            }
            _ => {
                // Unknown keys are tolerated for forward-compat; skip the value.
            }
        }
        i += 1;
    }

    Ok(result)
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

fn malformed(msg: &str) -> PropertyError {
    PropertyError::MalformedAttribute(msg.to_string())
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

/// Extract a raw literal representation (integer, string, bool) as a string.
fn extract_literal_raw(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Literal(lit) => Some(lit.symbol.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}
