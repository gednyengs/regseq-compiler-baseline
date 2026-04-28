//! Driver struct construction for Pass 1.
//!
//! Produces a `TypeDef` for the driver struct and seeds the
//! containing module's `driver: Option<DriverStruct>` slot.
//!
//! C2 moves resource population out of this HIR walker and into
//! `construction/resources.rs`, which consumes the extraction surfaces
//! directly after the walk completes. The only resource family that
//! still needs HIR-walk-time handling here is MMIO because the
//! `Mmio<T> -> RegisterBlockId` lookup depends on the crate-wide
//! preallocated register-block map.

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def_id::DefId;
use rustc_hir::{AttrArgs, Attribute, VariantData};

use regseq_model_old::identity::{ResourceId, Symbol, TypeDefId};
use regseq_model_old::model::{
    Attrs, DriverIdentity, DriverStruct, FieldDef, MmioDecl, MmioResource, Module, Shape, TypeDef,
    TypeDefClassification, TypeDefKind,
};

use super::walk::BuildContext;
use super::{ConstructionError, ErrorProvenance};

pub fn build_driver_struct(
    cx: &mut BuildContext<'_, '_>,
    target: &mut Module,
    def_id: DefId,
    name: &str,
    variant_data: &VariantData<'_>,
) -> Result<TypeDef, ConstructionError> {
    let type_id = TypeDefId::from(cx.alloc_and_record_public(def_id));
    let name_sym: Symbol = cx.intern_public(name);

    let identity = parse_driver_identity(cx, def_id);

    let mut driver = DriverStruct {
        type_id,
        name: name_sym,
        identity,
        device_class: None,
        pm_info: None,
        platform_metadata: None,
        clocks: Vec::new(),
        resets: Vec::new(),
        pinctrl: Vec::new(),
        interrupts: Vec::new(),
        mmio: Vec::new(),
        sync: Vec::new(),
        properties: Vec::new(),
        user_data: Vec::new(),
    };

    let mut type_fields: Vec<FieldDef> = Vec::new();
    for f in variant_data.fields() {
        let field_prov = ErrorProvenance::for_span(f.span);
        let field_name_str = f.ident.name.as_str().to_owned();
        let field_name_sym = cx.intern_public(&field_name_str);

        // Collect per-field attributes and middle-ty.
        let hir_attrs = cx.tcx.hir_attrs(f.hir_id);
        let field_def_id = f.def_id.to_def_id();

        // Record the struct field on the TypeDef side using the field's
        // middle-ty so locally-defined ADTs (e.g. Mmio<MyBlock>) resolve
        // through the DefId -> TypeDefId map instead of collapsing to Unit.
        type_fields.push(FieldDef {
            name: field_name_sym,
            ty: super::walk::resolve_field_ty(cx, field_def_id),
        });

        if let Some(attr) = find_rseq_attr(hir_attrs, "mmio") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "mmio",
            })?;
            let shape = shape_of_field(cx, field_def_id, f.ty);
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let id_sym = cx.intern_public(&id_str);
            // Resolve the T in Mmio<T> against the crate-wide
            // `DefId -> RegisterBlockId` map. The register-block pre-pass
            // in `walk.rs` populates this map before any driver struct
            // is visited, so unresolved references here indicate a real
            // authoring error (Mmio<T> where T is not a
            // `#[rseq::register_block]` struct in the crate).
            let register_block = resolve_mmio_register_block(cx, f.ty).ok_or(
                ConstructionError::MissingResourceId {
                    field: field_prov.clone(),
                    kind: "mmio-register-block",
                },
            )?;
            driver.mmio.push(MmioResource {
                id: res_id,
                field_name: field_name_sym,
                shape,
                decl: MmioDecl {
                    mmio_id: id_sym,
                    register_block,
                },
                binding: None,
            });
        }
    }

    target.driver = Some(driver);

    Ok(TypeDef {
        id: type_id,
        name: name_sym,
        kind: TypeDefKind::Struct {
            fields: type_fields,
        },
        classification: TypeDefClassification::UserDefined,
        attrs: collect_rseq_attrs_for(cx.tcx, def_id)?,
    })
}

fn collect_rseq_attrs_for(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    def_id: DefId,
) -> Result<Attrs, ConstructionError> {
    super::attrs::collect_rseq_attrs(tcx, def_id)
}

fn parse_driver_identity(cx: &mut BuildContext<'_, '_>, def_id: DefId) -> DriverIdentity {
    let mut name: Option<String> = None;
    let mut compatible: Vec<String> = Vec::new();
    let mut bus: Option<String> = None;
    for attr in cx.tcx.get_all_attrs(def_id) {
        if let Attribute::Unparsed(item) = attr {
            let segs = &item.path.segments;
            if !(segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == "driver") {
                continue;
            }
            let tokens = match &item.args {
                AttrArgs::Delimited(d) => &d.tokens,
                _ => continue,
            };
            let trees: Vec<&TokenTree> = tokens.iter().collect();
            let mut i = 0;
            while i < trees.len() {
                if is_comma(trees[i]) {
                    i += 1;
                    continue;
                }
                let key = match &trees[i] {
                    TokenTree::Token(tok, _) => match tok.kind {
                        TokenKind::Ident(sym, _) => sym.as_str().to_owned(),
                        _ => break,
                    },
                    _ => break,
                };
                i += 2;
                if i >= trees.len() {
                    break;
                }
                match key.as_str() {
                    "name" => {
                        if let Some(s) = extract_string(trees[i]) {
                            name = Some(s);
                        }
                    }
                    "compatible" => {
                        if let Some(list) = extract_string_list(trees[i]) {
                            compatible = list;
                        }
                    }
                    "bus" => {
                        if let Some(s) = extract_string(trees[i]) {
                            bus = Some(s);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
    }
    let name_sym = cx.intern_public(&name.unwrap_or_default());
    let compat_syms: Vec<Symbol> = compatible.iter().map(|c| cx.intern_public(c)).collect();
    let bus_sym = bus.as_deref().map(|b| cx.intern_public(b));
    DriverIdentity {
        name: name_sym,
        compatible: compat_syms,
        bus: bus_sym,
    }
}

fn find_rseq_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|a| {
        if let Attribute::Unparsed(item) = a {
            let segs = &item.path.segments;
            segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == name
        } else {
            false
        }
    })
}

fn parse_resource_id(attr: &Attribute) -> Option<String> {
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
        let key = match &trees[i] {
            TokenTree::Token(tok, _) => match tok.kind {
                TokenKind::Ident(sym, _) => sym.as_str().to_owned(),
                _ => return None,
            },
            _ => return None,
        };
        i += 2;
        if i >= trees.len() {
            return None;
        }
        if key == "id" {
            return extract_string(trees[i]);
        }
        i += 1;
    }
    None
}

fn extract_string(tree: &TokenTree) -> Option<String> {
    match tree {
        TokenTree::Token(tok, _) => match tok.kind {
            TokenKind::Literal(lit) if lit.kind == LitKind::Str => {
                Some(lit.symbol.as_str().to_owned())
            }
            _ => None,
        },
        _ => None,
    }
}

fn extract_string_list(tree: &TokenTree) -> Option<Vec<String>> {
    match tree {
        TokenTree::Delimited(_, _, _, stream) => {
            let mut out = Vec::new();
            for t in stream.iter() {
                if let Some(s) = extract_string(t) {
                    out.push(s);
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn is_comma(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Token(tok, _) if matches!(tok.kind, TokenKind::Comma))
}

// --- type shape helpers ---

/// Pull out the T in `Mmio<T>` as a HIR type, resolve T's `DefId`, and
/// look up the corresponding [`RegisterBlockId`] in the crate-wide
/// register-block map built during the walk. Returns `None` when T is
/// not a resolved ADT path or when no register block has been assigned
/// for its `DefId` yet.
fn resolve_mmio_register_block(
    cx: &BuildContext<'_, '_>,
    ty: &rustc_hir::Ty<'_>,
) -> Option<regseq_model_old::identity::RegisterBlockId> {
    use rustc_hir::def::Res;
    let inner = mmio_inner_ty(ty)?;
    let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &inner.kind else {
        return None;
    };
    let target_def = match path.res {
        Res::Def(_, def_id) => def_id,
        _ => return None,
    };
    cx.register_block_map.get(&target_def).copied()
}

fn mmio_inner_ty<'hir>(ty: &'hir rustc_hir::Ty<'hir>) -> Option<&'hir rustc_hir::Ty<'hir>> {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            if seg.ident.name.as_str() == "Mmio" {
                let args = seg.args?;
                for arg in args.args.iter() {
                    if let rustc_hir::GenericArg::Type(inner) = arg {
                        return Some(inner.as_unambig_ty());
                    }
                }
            }
        }
    }
    None
}

fn shape_of_field(cx: &BuildContext<'_, '_>, field_def_id: DefId, ty: &rustc_hir::Ty<'_>) -> Shape {
    if matches!(
        &ty.kind,
        rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path))
            if path
                .segments
                .last()
                .map(|seg| seg.ident.name.as_str() == "Option")
                .unwrap_or(false)
    ) {
        return Shape::Optional;
    }
    let middle_ty = cx.tcx.type_of(field_def_id).instantiate_identity();
    if let rustc_middle::ty::TyKind::Array(_, len_const) = middle_ty.kind() {
        let n = len_const
            .try_to_target_usize(cx.tcx)
            .map(|n| n as usize)
            .unwrap_or(0);
        return Shape::Array(n);
    }
    Shape::Single
}
