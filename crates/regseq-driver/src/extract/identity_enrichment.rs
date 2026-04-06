//! Identity enrichment extraction for the semantic procedural IR.
//!
//! Extracts canonical identity and ownership facts from HIR that later phases
//! need to build the item hierarchy, cross-link table, and semantic model.
//! This is the Phase 0 extraction enrichment: it adds upstream truth about
//! item identity, module containment, impl membership, field identity, and
//! ISR markers.
//!
//! All canonical paths are derived from `tcx.def_path_str(DefId)`. Field
//! resource classification uses `#[rseq::*]` attributes, never type-name
//! string matching.
//!
//! Types are defined in the companion [`super::identity_enrichment_types`]
//! module and re-exported here.

pub use super::identity_enrichment_types::*;

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute, ImplItemKind, ItemKind, QPath};
use rustc_middle::ty::TyCtxt;

// ---------------------------------------------------------------------------
// Main extraction entry point
// ---------------------------------------------------------------------------

/// Mutable accumulator for extracted facts, passed through recursive module
/// traversal to avoid excessive function parameters.
struct Accumulator {
    modules: Vec<ModuleFact>,
    structs: Vec<StructFact>,
    enums: Vec<EnumFact>,
    impl_blocks: Vec<ImplBlockFact>,
    free_functions: Vec<FunctionFact>,
}

/// Extract identity and ownership facts from HIR for the Phase 0 enrichment.
///
/// Walks the full HIR item tree structurally: modules (including crate root),
/// structs, enums, impl blocks, and free functions. Produces canonical paths,
/// module containment, impl membership, field identity, and ISR markers.
pub fn extract_identity_enrichment<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<IdentityEnrichmentSurface, IdentityEnrichmentError> {
    let mut acc = Accumulator {
        modules: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        impl_blocks: Vec::new(),
        free_functions: Vec::new(),
    };

    // Build crate root module fact from top-level items.
    let crate_name = tcx
        .crate_name(rustc_hir::def_id::LOCAL_CRATE)
        .as_str()
        .to_string();
    let crate_root_path = CanonicalPath(crate_name.clone());
    let mut crate_root_children = Vec::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let item_path = CanonicalPath(tcx.def_path_str(item.owner_id.def_id.to_def_id()));

        match &item.kind {
            ItemKind::Mod(ident, mod_ref) => {
                crate_root_children.push(item_path.clone());
                extract_module(tcx, ident.name.as_str(), &item_path, mod_ref, &mut acc)?;
            }
            ItemKind::Struct(ident, _generics, variant_data) => {
                crate_root_children.push(item_path.clone());
                extract_struct(
                    tcx,
                    ident.name.as_str(),
                    &item_path,
                    &crate_root_path,
                    item,
                    variant_data,
                    &mut acc.structs,
                )?;
            }
            ItemKind::Enum(ident, _generics, enum_def) => {
                crate_root_children.push(item_path.clone());
                acc.enums.push(EnumFact {
                    canonical_path: item_path,
                    name: ident.name.as_str().to_string(),
                    module_path: crate_root_path.clone(),
                    variants: enum_def
                        .variants
                        .iter()
                        .map(|v| v.ident.name.as_str().to_string())
                        .collect(),
                });
            }
            ItemKind::Impl(impl_block) => {
                crate_root_children.push(item_path.clone());
                extract_impl_block(tcx, &item_path, item, impl_block, &mut acc.impl_blocks)?;
            }
            ItemKind::Fn { ident, .. } => {
                crate_root_children.push(item_path.clone());
                acc.free_functions.push(FunctionFact {
                    name: ident.name.as_str().to_string(),
                    canonical_path: item_path,
                    module_path: crate_root_path.clone(),
                });
            }
            _ => {}
        }
    }

    acc.modules.insert(
        0,
        ModuleFact {
            canonical_path: crate_root_path,
            name: crate_name,
            child_items: crate_root_children,
        },
    );

    // Verify the driver type exists after the full recursive walk so nested
    // modules are included in the check.
    if !acc.structs.iter().any(|s| s.is_driver) {
        return Err(IdentityEnrichmentError::NoDriverType);
    }

    Ok(IdentityEnrichmentSurface {
        modules: acc.modules,
        structs: acc.structs,
        enums: acc.enums,
        impl_blocks: acc.impl_blocks,
        free_functions: acc.free_functions,
    })
}

// ---------------------------------------------------------------------------
// Module extraction
// ---------------------------------------------------------------------------

fn extract_module<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
    module_path: &CanonicalPath,
    mod_ref: &rustc_hir::Mod<'tcx>,
    acc: &mut Accumulator,
) -> Result<(), IdentityEnrichmentError> {
    let mut child_items = Vec::new();

    for &child_id in mod_ref.item_ids {
        let item = tcx.hir_item(child_id);
        let item_path = CanonicalPath(tcx.def_path_str(item.owner_id.def_id.to_def_id()));

        match &item.kind {
            ItemKind::Mod(ident, nested_mod) => {
                child_items.push(item_path.clone());
                extract_module(tcx, ident.name.as_str(), &item_path, nested_mod, acc)?;
            }
            ItemKind::Struct(ident, _generics, variant_data) => {
                child_items.push(item_path.clone());
                extract_struct(
                    tcx,
                    ident.name.as_str(),
                    &item_path,
                    module_path,
                    item,
                    variant_data,
                    &mut acc.structs,
                )?;
            }
            ItemKind::Enum(ident, _generics, enum_def) => {
                child_items.push(item_path.clone());
                acc.enums.push(EnumFact {
                    canonical_path: item_path,
                    name: ident.name.as_str().to_string(),
                    module_path: module_path.clone(),
                    variants: enum_def
                        .variants
                        .iter()
                        .map(|v| v.ident.name.as_str().to_string())
                        .collect(),
                });
            }
            ItemKind::Impl(impl_block) => {
                child_items.push(item_path.clone());
                extract_impl_block(tcx, &item_path, item, impl_block, &mut acc.impl_blocks)?;
            }
            ItemKind::Fn { ident, .. } => {
                child_items.push(item_path.clone());
                acc.free_functions.push(FunctionFact {
                    name: ident.name.as_str().to_string(),
                    canonical_path: item_path,
                    module_path: module_path.clone(),
                });
            }
            _ => {}
        }
    }

    acc.modules.push(ModuleFact {
        canonical_path: module_path.clone(),
        name: name.to_string(),
        child_items,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Struct extraction
// ---------------------------------------------------------------------------

fn extract_struct<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
    struct_path: &CanonicalPath,
    module_path: &CanonicalPath,
    item: &'tcx rustc_hir::Item<'tcx>,
    variant_data: &rustc_hir::VariantData<'tcx>,
    structs: &mut Vec<StructFact>,
) -> Result<(), IdentityEnrichmentError> {
    let attrs = tcx.hir_attrs(item.hir_id());
    let is_driver = has_rseq_attr(attrs, "driver");
    let is_register_block = has_rseq_attr(attrs, "register_block");

    let mut fields = Vec::new();
    for field in variant_data.fields() {
        let field_name = field.ident.name.as_str().to_string();
        let field_path = CanonicalPath(format!("{}::{}", struct_path.0, field_name));
        let field_attrs = tcx.hir_attrs(field.hir_id);
        let rseq_kind = extract_rseq_field_kind(field_attrs, &field_name)?;
        fields.push(FieldFact {
            name: field_name,
            canonical_path: field_path,
            rseq_kind,
        });
    }

    structs.push(StructFact {
        canonical_path: struct_path.clone(),
        name: name.to_string(),
        module_path: module_path.clone(),
        is_driver,
        is_register_block,
        fields,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Impl block extraction
// ---------------------------------------------------------------------------

fn extract_impl_block<'tcx>(
    tcx: TyCtxt<'tcx>,
    impl_path: &CanonicalPath,
    item: &'tcx rustc_hir::Item<'tcx>,
    impl_block: &'tcx rustc_hir::Impl<'tcx>,
    impl_blocks: &mut Vec<ImplBlockFact>,
) -> Result<(), IdentityEnrichmentError> {
    let impl_span = format!("{:?}", item.span);

    let self_type_path = resolve_self_type_path(tcx, impl_block.self_ty, &impl_span)?;
    let self_type_name =
        extract_self_type_short_name(impl_block.self_ty).unwrap_or_else(|| "<unknown>".to_string());

    let (trait_path, trait_name, is_inherent) = match &impl_block.of_trait {
        Some(trait_header) => {
            let tp = resolve_trait_path(tcx, trait_header, &impl_span)?;
            let short_name = tp.0.rsplit("::").next().unwrap_or(&tp.0).to_string();
            (Some(tp), Some(short_name), false)
        }
        None => (None, None, true),
    };

    let mut methods = Vec::new();
    let mut constants = Vec::new();

    for item_ref in impl_block.items {
        let impl_item = tcx.hir_impl_item(rustc_hir::ImplItemId {
            owner_id: item_ref.owner_id,
        });
        let item_name = impl_item.ident.name.as_str().to_string();
        let item_canonical = CanonicalPath(tcx.def_path_str(impl_item.owner_id.def_id.to_def_id()));

        match &impl_item.kind {
            ImplItemKind::Fn(..) => {
                let attrs = tcx.hir_attrs(impl_item.hir_id());
                let is_isr = has_rseq_attr(attrs, "isr");
                methods.push(ImplMethodFact {
                    name: item_name,
                    canonical_path: item_canonical,
                    is_isr,
                });
            }
            ImplItemKind::Const(ty, _) => {
                constants.push(AssociatedConstantFact {
                    name: item_name,
                    canonical_path: item_canonical,
                    ty: render_hir_ty(ty),
                    owner_impl_path: impl_path.clone(),
                });
            }
            ImplItemKind::Type(_) => {
                // Type aliases in impls are not required by the Phase 0 contract.
            }
        }
    }

    impl_blocks.push(ImplBlockFact {
        canonical_path: impl_path.clone(),
        self_type_path,
        self_type_name,
        trait_path,
        trait_name,
        is_inherent,
        methods,
        constants,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Self-type and trait path resolution
// ---------------------------------------------------------------------------

/// Resolve the canonical path of an impl block's self type from HIR.
fn resolve_self_type_path<'tcx>(
    tcx: TyCtxt<'tcx>,
    self_ty: &rustc_hir::Ty<'tcx>,
    impl_span: &str,
) -> Result<CanonicalPath, IdentityEnrichmentError> {
    if let rustc_hir::TyKind::Path(QPath::Resolved(_, path)) = &self_ty.kind {
        if let rustc_hir::def::Res::Def(_, def_id) = path.res {
            return Ok(CanonicalPath(tcx.def_path_str(def_id)));
        }
    }
    Err(IdentityEnrichmentError::UnresolvedSelfType {
        impl_span: impl_span.to_string(),
    })
}

/// Resolve the canonical path of a trait impl's trait reference.
fn resolve_trait_path<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_header: &rustc_hir::TraitImplHeader<'tcx>,
    impl_span: &str,
) -> Result<CanonicalPath, IdentityEnrichmentError> {
    if let rustc_hir::def::Res::Def(_, def_id) = trait_header.trait_ref.path.res {
        return Ok(CanonicalPath(tcx.def_path_str(def_id)));
    }
    Err(IdentityEnrichmentError::UnresolvedTraitPath {
        impl_span: impl_span.to_string(),
    })
}

/// Extract the short name of the self type from HIR.
fn extract_self_type_short_name(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    if let rustc_hir::TyKind::Path(QPath::Resolved(_, path)) = &ty.kind {
        path.segments
            .last()
            .map(|s| s.ident.name.as_str().to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Field attribute classification
// ---------------------------------------------------------------------------

/// Classify a field's resource kind from its `#[rseq::*]` attributes.
///
/// Returns `None` if no rseq attribute is present. Returns an error if an
/// rseq attribute is present but malformed (e.g., missing `id = "..."`).
fn extract_rseq_field_kind(
    attrs: &[Attribute],
    field_name: &str,
) -> Result<Option<RseqFieldKind>, IdentityEnrichmentError> {
    for attr in attrs {
        let attr_name = match rseq_attr_name(attr) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let kind = match attr_name.as_str() {
            "mmio" => {
                let id = parse_id_arg(attr, field_name, &attr_name)?;
                RseqFieldKind::Mmio { id }
            }
            "clock" => {
                let id = parse_id_arg(attr, field_name, &attr_name)?;
                RseqFieldKind::Clock { id }
            }
            "pinctrl" => {
                let id = parse_id_arg(attr, field_name, &attr_name)?;
                RseqFieldKind::PinCtrl { id }
            }
            "interrupt" => {
                let id = parse_id_arg(attr, field_name, &attr_name)?;
                RseqFieldKind::Interrupt { id }
            }
            "property" => {
                let id = parse_id_arg(attr, field_name, &attr_name)?;
                RseqFieldKind::Property { id }
            }
            _ => continue,
        };

        return Ok(Some(kind));
    }
    Ok(None)
}

/// Parse the `id = "..."` argument from an `#[rseq::*(...)]` attribute.
fn parse_id_arg(
    attr: &Attribute,
    field_name: &str,
    attr_name: &str,
) -> Result<String, IdentityEnrichmentError> {
    let Attribute::Unparsed(item) = attr else {
        return Err(IdentityEnrichmentError::MalformedRseqAttribute {
            field_name: field_name.to_string(),
            attr_name: attr_name.to_string(),
            reason: "expected tool attribute".to_string(),
        });
    };

    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => {
            return Err(IdentityEnrichmentError::MalformedRseqAttribute {
                field_name: field_name.to_string(),
                attr_name: attr_name.to_string(),
                reason: "missing attribute arguments".to_string(),
            });
        }
    };

    let trees: Vec<&TokenTree> = tokens.iter().collect();
    let mut i = 0;

    while i < trees.len() {
        if is_comma(trees[i]) {
            i += 1;
            continue;
        }

        let key = match extract_ident(trees[i]) {
            Some(k) => k,
            None => {
                i += 1;
                continue;
            }
        };
        i += 1;

        if i >= trees.len() || !is_eq(trees[i]) {
            continue;
        }
        i += 1;

        if i >= trees.len() {
            continue;
        }

        if key == "id" {
            if let Some(val) = extract_string_literal(trees[i]) {
                return Ok(val);
            }
        }
        i += 1;
    }

    Err(IdentityEnrichmentError::MalformedRseqAttribute {
        field_name: field_name.to_string(),
        attr_name: attr_name.to_string(),
        reason: "missing required id = \"...\" argument".to_string(),
    })
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

fn has_rseq_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| rseq_attr_name(a) == Some(name))
}

// ---------------------------------------------------------------------------
// Token tree helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// HIR type rendering
// ---------------------------------------------------------------------------

fn render_hir_ty<A>(ty: &rustc_hir::Ty<'_, A>) -> String {
    match &ty.kind {
        rustc_hir::TyKind::Path(qpath) => render_qpath(qpath),
        rustc_hir::TyKind::Ref(_, mut_ty) => {
            let m = if mut_ty.mutbl.is_mut() { "mut " } else { "" };
            format!("&{m}{}", render_hir_ty(mut_ty.ty))
        }
        rustc_hir::TyKind::Slice(elem) => format!("[{}]", render_hir_ty(elem)),
        rustc_hir::TyKind::Tup([]) => "()".to_string(),
        rustc_hir::TyKind::Tup(elems) => {
            let inner: Vec<_> = elems.iter().map(|e| render_hir_ty(e)).collect();
            format!("({})", inner.join(", "))
        }
        rustc_hir::TyKind::Never => "!".to_string(),
        _ => "<type>".to_string(),
    }
}

fn render_qpath(qpath: &QPath<'_>) -> String {
    match qpath {
        QPath::Resolved(_, path) => path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str())
            .collect::<Vec<_>>()
            .join("::"),
        QPath::TypeRelative(ty, segment) => {
            format!("{}::{}", render_hir_ty(ty), segment.ident.name.as_str())
        }
    }
}
