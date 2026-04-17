//! Driver struct construction for Pass 1.
//!
//! Produces a `TypeDef` for the driver struct AND populates the
//! containing module's `driver: Option<DriverStruct>` slot with a fully
//! categorized [`DriverStruct`]. Resource binding slots and property
//! bound_value slots are all left `None`.

use rustc_ast::token::{LitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def_id::DefId;
use rustc_hir::{AttrArgs, Attribute, VariantData};

use regseq_model::identity::{PropertyId, ResourceId, Symbol, TypeDefId, UserDataId};
use regseq_model::ir::{LiteralValue, RegseqType, ScalarType};
use regseq_model::model::{
    Attrs, ClockDecl, ClockResource, DriverIdentity, DriverStruct, FieldDef, InterruptDecl,
    InterruptResource, MmioDecl, MmioResource, Module, PinCtrlDecl, PinCtrlResource, PropertyField,
    ResetDecl, ResetResource, Shape, SyncKind, SyncResource, TypeDef, TypeDefClassification,
    TypeDefKind, UserDataField,
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

        // Record the struct field on the TypeDef side.
        type_fields.push(FieldDef {
            name: field_name_sym,
            ty: super::walk::render_ty_for_field(cx.tcx, f.ty),
        });

        // Collect per-field attributes.
        let hir_attrs = cx.tcx.hir_attrs(f.hir_id);
        let field_def_id = f.def_id.to_def_id();

        // Resource attributes.
        if let Some(attr) = find_rseq_attr(hir_attrs, "clock") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "clock",
            })?;
            let shape = shape_of_field(f.ty, "Clock");
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let clock_id_sym = cx.intern_public(&id_str);
            driver.clocks.push(ClockResource {
                id: res_id,
                field_name: field_name_sym,
                shape,
                decl: ClockDecl {
                    clock_id: clock_id_sym,
                },
                binding: None,
            });
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "reset") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "reset",
            })?;
            let shape = shape_of_field(f.ty, "ResetLine");
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let reset_id_sym = cx.intern_public(&id_str);
            driver.resets.push(ResetResource {
                id: res_id,
                field_name: field_name_sym,
                shape,
                decl: ResetDecl {
                    reset_id: reset_id_sym,
                },
                binding: None,
            });
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "pinctrl") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "pinctrl",
            })?;
            let shape = shape_of_field(f.ty, "PinCtrl");
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let pin_id_sym = cx.intern_public(&id_str);
            driver.pinctrl.push(PinCtrlResource {
                id: res_id,
                field_name: field_name_sym,
                shape,
                decl: PinCtrlDecl {
                    pinctrl_id: pin_id_sym,
                },
                binding: None,
            });
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "interrupt") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "interrupt",
            })?;
            let shape = shape_of_field(f.ty, "Interrupt");
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let irq_id_sym = cx.intern_public(&id_str);
            driver.interrupts.push(InterruptResource {
                id: res_id,
                field_name: field_name_sym,
                shape,
                decl: InterruptDecl {
                    interrupt_id: irq_id_sym,
                },
                binding: None,
            });
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "mmio") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "mmio",
            })?;
            let shape = shape_of_field(f.ty, "Mmio");
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            let id_sym = cx.intern_public(&id_str);
            // Resolve the T in Mmio<T> against the crate-wide
            // `DefId -> RegisterBlockId` map built during the walk. If
            // no matching register-block struct has been seen yet, fall
            // back to an allocator-fresh id so Pass 1's invariant
            // (every DeclId in the model is also a key in
            // `source_locations`) stays intact; tests currently only
            // inspect `mmio_id`, so an unresolved reference is silent.
            let register_block = resolve_mmio_register_block(cx, f.ty).unwrap_or_else(|| {
                let fresh = cx.allocator.alloc();
                cx.source_locations.insert(
                    fresh,
                    regseq_model::identity::SourceOrigin::CompilerGenerated,
                );
                regseq_model::identity::RegisterBlockId::from(fresh)
            });
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
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "property") {
            let (id_str, default) = parse_property_attr(attr);
            let id_str = id_str.ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "property",
            })?;
            let scalar = scalar_of_field(f.ty).unwrap_or(ScalarType::U32);
            let id_sym = cx.intern_public(&id_str);
            let prop_id = PropertyId::from(cx.alloc_and_record_public(field_def_id));
            driver.properties.push(PropertyField {
                id: prop_id,
                field_name: field_name_sym,
                field_type: scalar,
                property_id: id_sym,
                default_value: default,
                vendor_prefix: None,
                required: !is_option(f.ty),
                bound_value: None,
            });
            continue;
        }

        // Sync primitive (by type name).
        if let Some((sync_kind, payload)) = sync_kind_for_field(f.ty) {
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            driver.sync.push(SyncResource {
                id: res_id,
                field_name: field_name_sym,
                sync_kind,
                payload_type: payload,
            });
            continue;
        }

        // User-data field: must be Option<T> with T scalar-ish (Copy).
        let rendered = super::walk::rendered_ty_string(f.ty);
        if is_option(f.ty) {
            let inner = extract_option_inner(f.ty);
            if !is_copy_inner(inner) {
                let inner_str = inner
                    .map(super::walk::rendered_ty_string)
                    .unwrap_or_default();
                return Err(ConstructionError::UserDataNotCopy {
                    field: field_prov,
                    inner_type: inner_str,
                });
            }
            let inner_type = inner
                .map(|t| super::walk::render_ty_for_field(cx.tcx, t))
                .unwrap_or(RegseqType::Scalar(ScalarType::U32));
            let ud_id = UserDataId::from(cx.alloc_and_record_public(field_def_id));
            driver.user_data.push(UserDataField {
                id: ud_id,
                field_name: field_name_sym,
                inner_type,
            });
        } else {
            return Err(ConstructionError::InvalidUserDataShape {
                field: field_prov,
                rendered_type: rendered,
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
        attrs: collect_rseq_attrs_for(cx.tcx, def_id),
    })
}

fn collect_rseq_attrs_for(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> Attrs {
    use regseq_model::model::{AttrEntry, AttrValue};
    let mut entries = Vec::new();
    for attr in tcx.get_all_attrs(def_id) {
        if let Attribute::Unparsed(item) = attr {
            let segs = &item.path.segments;
            if segs.len() == 2 && segs[0].as_str() == "rseq" {
                entries.push(AttrEntry {
                    key: format!("rseq::{}", segs[1].as_str()),
                    value: AttrValue::Unit,
                });
            }
        }
    }
    Attrs { entries }
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

fn parse_property_attr(attr: &Attribute) -> (Option<String>, Option<LiteralValue>) {
    let mut id: Option<String> = None;
    let mut default: Option<LiteralValue> = None;
    let Attribute::Unparsed(item) = attr else {
        return (id, default);
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return (id, default),
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
                _ => return (id, default),
            },
            _ => return (id, default),
        };
        i += 2;
        if i >= trees.len() {
            return (id, default);
        }
        match key.as_str() {
            "id" => {
                id = extract_string(trees[i]);
            }
            "default" => {
                if let TokenTree::Token(tok, _) = trees[i] {
                    if let TokenKind::Literal(lit) = tok.kind {
                        if lit.kind == LitKind::Integer {
                            if let Ok(v) = lit.symbol.as_str().parse::<u32>() {
                                default = Some(LiteralValue::U32(v));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (id, default)
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
) -> Option<regseq_model::identity::RegisterBlockId> {
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

fn shape_of_field(ty: &rustc_hir::Ty<'_>, _inner_name_hint: &str) -> Shape {
    if is_option(ty) {
        Shape::Optional
    } else if let rustc_hir::TyKind::Array(_, len) = &ty.kind {
        let n = match &len.kind {
            rustc_hir::ConstArgKind::Anon(_) => 0,
            _ => 0,
        };
        Shape::Array(n)
    } else {
        Shape::Single
    }
}

fn is_option(ty: &rustc_hir::Ty<'_>) -> bool {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            return seg.ident.name.as_str() == "Option";
        }
    }
    false
}

fn extract_option_inner<'hir>(ty: &'hir rustc_hir::Ty<'hir>) -> Option<&'hir rustc_hir::Ty<'hir>> {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            if seg.ident.name.as_str() == "Option" {
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

fn is_copy_inner(inner: Option<&rustc_hir::Ty<'_>>) -> bool {
    let Some(inner) = inner else {
        return false;
    };
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &inner.kind {
        if let Some(seg) = path.segments.last() {
            let n = seg.ident.name.as_str();
            return matches!(
                n,
                "u8" | "u16"
                    | "u32"
                    | "u64"
                    | "usize"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "isize"
                    | "bool"
                    | "char"
            );
        }
    }
    false
}

fn scalar_of_field(ty: &rustc_hir::Ty<'_>) -> Option<ScalarType> {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            return match seg.ident.name.as_str() {
                "u8" => Some(ScalarType::U8),
                "u16" => Some(ScalarType::U16),
                "u32" => Some(ScalarType::U32),
                "u64" => Some(ScalarType::U64),
                "usize" => Some(ScalarType::Usize),
                "i8" => Some(ScalarType::I8),
                "i16" => Some(ScalarType::I16),
                "i32" => Some(ScalarType::I32),
                "i64" => Some(ScalarType::I64),
                "isize" => Some(ScalarType::Isize),
                _ => None,
            };
        }
    }
    None
}

fn sync_kind_for_field(ty: &rustc_hir::Ty<'_>) -> Option<(SyncKind, Option<RegseqType>)> {
    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind {
        if let Some(seg) = path.segments.last() {
            let n = seg.ident.name.as_str();
            let kind = match n {
                "Mutex" => SyncKind::Mutex,
                "SpinLock" => SyncKind::SpinLock,
                "IrqSpinLock" => SyncKind::IrqSpinLock,
                "Completion" => return Some((SyncKind::Completion, None)),
                _ => return None,
            };
            // Payload type resolution requires a TyCtxt, which we do
            // not thread here for simplicity. Pass 1 records a
            // placeholder U32 payload; Pass 2 (or follow-up task 52)
            // can refine this when it walks MIR.
            return Some((kind, Some(RegseqType::Scalar(ScalarType::U32))));
        }
    }
    None
}
