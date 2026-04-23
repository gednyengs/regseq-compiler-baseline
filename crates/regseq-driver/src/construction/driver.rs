//! Driver struct construction for Pass 1.
//!
//! Produces a `TypeDef` for the driver struct AND populates the
//! containing module's `driver: Option<DriverStruct>` slot with a fully
//! categorized [`DriverStruct`]. Resource binding slots and property
//! bound_value slots are all left `None`.

use std::collections::HashSet;

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
    TypeDefKind, UserDataField, UserDataKind,
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

        // Resource attributes.
        if let Some(attr) = find_rseq_attr(hir_attrs, "clock") {
            let id_str = parse_resource_id(attr).ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "clock",
            })?;
            let shape = shape_of_field(cx, field_def_id, f.ty);
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
            let shape = shape_of_field(cx, field_def_id, f.ty);
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
            let shape = shape_of_field(cx, field_def_id, f.ty);
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
            let shape = shape_of_field(cx, field_def_id, f.ty);
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
            continue;
        }
        if let Some(attr) = find_rseq_attr(hir_attrs, "property") {
            let (id_str, default, vendor_prefix) = parse_property_attr(attr);
            let id_str = id_str.ok_or(ConstructionError::MissingResourceId {
                field: field_prov.clone(),
                kind: "property",
            })?;
            let scalar = scalar_of_field(f.ty).unwrap_or(ScalarType::U32);
            let id_sym = cx.intern_public(&id_str);
            let vendor_prefix_sym = vendor_prefix.as_deref().map(|vp| cx.intern_public(vp));
            let prop_id = PropertyId::from(cx.alloc_and_record_public(field_def_id));
            driver.properties.push(PropertyField {
                id: prop_id,
                field_name: field_name_sym,
                field_type: scalar,
                property_id: id_sym,
                default_value: default,
                vendor_prefix: vendor_prefix_sym,
                required: !is_option(f.ty),
                bound_value: None,
            });
            continue;
        }

        // Sync primitive (by type name).
        if let Some((sync_kind, payload)) = sync_kind_for_field(cx, field_def_id, f.ty) {
            let res_id = ResourceId::from(cx.alloc_and_record_public(field_def_id));
            driver.sync.push(SyncResource {
                id: res_id,
                field_name: field_name_sym,
                sync_kind,
                payload_type: payload,
            });
            continue;
        }

        // User-data field: two acceptance rules per Path A.
        //
        // Rule 2: `Option<T>` where `T: Copy`. Admits any user type
        // that genuinely implements `Copy`.
        // Rule 1: zero-init-whitelisted types. The outer field type
        // must be `Copy` AND belong to a compiler-controlled
        // whitelist of `u8`, `u16`, `u32`, `u64`, `bool`, arrays of
        // accepted types, and `Copy` structs recursively composed
        // of accepted fields.
        let rendered = super::walk::rendered_ty_string(f.ty);
        let middle_field_ty = cx.tcx.type_of(field_def_id).instantiate_identity();
        if is_option(f.ty) {
            let inner_middle = option_inner_middle_ty(middle_field_ty);
            let inner_hir = extract_option_inner(f.ty);
            let inner_is_copy = inner_middle
                .map(|inner| type_implements_copy(cx.tcx, inner))
                .unwrap_or(false);
            if !inner_is_copy {
                let inner_str = inner_hir
                    .map(super::walk::rendered_ty_string)
                    .unwrap_or_default();
                return Err(ConstructionError::UserDataNotCopy {
                    field: field_prov,
                    inner_type: inner_str,
                });
            }
            let inner_type = inner_middle
                .and_then(|inner| super::walk::ty_to_regseq(cx.tcx, &cx.type_def_map, inner))
                .unwrap_or(RegseqType::Scalar(ScalarType::U32));
            let ud_id = UserDataId::from(cx.alloc_and_record_public(field_def_id));
            driver.user_data.push(UserDataField {
                id: ud_id,
                field_name: field_name_sym,
                kind: UserDataKind::Option { inner_type },
            });
        } else {
            // Rule 1 path. Reject unresolved generics / projections
            // FIRST so the Rule-1 whitelist variant surfaces with
            // the expected diagnostic shape. The Copy trait query
            // under `fully_monomorphized` would otherwise fail on
            // `TyKind::Param` and `TyKind::Alias` with a generic
            // not-Copy verdict even when the field's trait bound
            // (e.g. `T: Copy`) would structurally admit it.
            if is_unresolved_generic_or_projection(middle_field_ty) {
                return Err(ConstructionError::UserDataNotZeroInitWhitelisted {
                    field: field_prov,
                    rejected_type: rendered,
                });
            }
            // The outer field type must be `Copy` (reuses the same
            // helper that Rule 2 uses for the inner type); otherwise
            // surface `UserDataNotCopy` with the bare rendered type
            // so non-`Copy` shapes like `Box<u32>`, `Rc<u32>`,
            // `Arc<u32>`, `Cell<u32>`, `RefCell<u32>`,
            // `UnsafeCell<u32>`, `AtomicU32` fail with the
            // familiar variant rather than the Rule-1 whitelist
            // variant.
            if !type_implements_copy(cx.tcx, middle_field_ty) {
                return Err(ConstructionError::UserDataNotCopy {
                    field: field_prov,
                    inner_type: rendered,
                });
            }
            let mut visited: HashSet<rustc_middle::ty::Ty<'_>> = HashSet::new();
            match rule1_whitelist_ty(cx.tcx, &cx.type_def_map, middle_field_ty, &mut visited) {
                Rule1Outcome::Accepted(inner_type) => {
                    let ud_id = UserDataId::from(cx.alloc_and_record_public(field_def_id));
                    driver.user_data.push(UserDataField {
                        id: ud_id,
                        field_name: field_name_sym,
                        kind: UserDataKind::ZeroInit { inner_type },
                    });
                }
                Rule1Outcome::Rejected => {
                    return Err(ConstructionError::UserDataNotZeroInitWhitelisted {
                        field: field_prov,
                        rejected_type: rendered,
                    });
                }
            }
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

fn parse_property_attr(attr: &Attribute) -> (Option<String>, Option<LiteralValue>, Option<String>) {
    let mut id: Option<String> = None;
    let mut default: Option<LiteralValue> = None;
    let mut vendor_prefix: Option<String> = None;
    let Attribute::Unparsed(item) = attr else {
        return (id, default, vendor_prefix);
    };
    let tokens = match &item.args {
        AttrArgs::Delimited(d) => &d.tokens,
        _ => return (id, default, vendor_prefix),
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
                _ => return (id, default, vendor_prefix),
            },
            _ => return (id, default, vendor_prefix),
        };
        i += 2;
        if i >= trees.len() {
            return (id, default, vendor_prefix);
        }
        match key.as_str() {
            "id" => {
                id = extract_string(trees[i]);
            }
            "vendor_prefix" => {
                vendor_prefix = extract_string(trees[i]);
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
    (id, default, vendor_prefix)
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

fn shape_of_field(cx: &BuildContext<'_, '_>, field_def_id: DefId, ty: &rustc_hir::Ty<'_>) -> Shape {
    if is_option(ty) {
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

/// Return the middle-ty of the inner `T` in a `Option<T>` type.
fn option_inner_middle_ty<'tcx>(
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Option<rustc_middle::ty::Ty<'tcx>> {
    if let rustc_middle::ty::TyKind::Adt(_, args) = ty.kind() {
        return args.types().next();
    }
    None
}

/// Query the trait system to decide whether `ty` implements `Copy`. This
/// replaces the earlier hardcoded primitive-name whitelist so any
/// user-defined type that derives `Copy` is accepted as user-data inner.
fn type_implements_copy<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> bool {
    let typing_env = rustc_middle::ty::TypingEnv::fully_monomorphized();
    tcx.type_is_copy_modulo_regions(typing_env, ty)
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

fn sync_kind_for_field(
    cx: &BuildContext<'_, '_>,
    field_def_id: DefId,
    ty: &rustc_hir::Ty<'_>,
) -> Option<(SyncKind, Option<RegseqType>)> {
    let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = &ty.kind else {
        return None;
    };
    let seg = path.segments.last()?;
    let kind = match seg.ident.name.as_str() {
        "Mutex" => SyncKind::Mutex,
        "SpinLock" => SyncKind::SpinLock,
        "IrqSpinLock" => SyncKind::IrqSpinLock,
        "Completion" => return Some((SyncKind::Completion, None)),
        _ => return None,
    };

    // Resolve the type parameter of `Mutex<T>` / `SpinLock<T>` /
    // `IrqSpinLock<T>` through middle-ty so `T` surfaces as the correct
    // [`RegseqType`] (including `Defined(TypeDefId)` for user ADTs)
    // instead of a hardcoded `U32` placeholder.
    let middle_ty = cx.tcx.type_of(field_def_id).instantiate_identity();
    let payload = if let rustc_middle::ty::TyKind::Adt(_, args) = middle_ty.kind() {
        args.types()
            .next()
            .and_then(|inner| super::walk::ty_to_regseq(cx.tcx, &cx.type_def_map, inner))
    } else {
        None
    };
    Some((kind, payload.or(Some(RegseqType::Unit))))
}

/// Decide whether `ty` is an unresolved generic parameter or
/// associated-type projection that Rule 1 must reject up-front.
/// Covers every `TyKind` variant for which the whitelist walker
/// cannot make a safe decision without a concrete monomorphized
/// type.
fn is_unresolved_generic_or_projection(ty: rustc_middle::ty::Ty<'_>) -> bool {
    use rustc_middle::ty::TyKind;
    matches!(
        ty.kind(),
        TyKind::Param(_) | TyKind::Alias(..) | TyKind::Bound(..) | TyKind::Placeholder(_)
    )
}

// --- Rule-1 zero-init whitelist predicate ---------------------------

/// Classification outcome for the Rule-1 whitelist walker. `Accepted`
/// carries the faithful [`RegseqType`] representation of the accepted
/// type; `Rejected` signals the caller should raise
/// [`ConstructionError::UserDataNotZeroInitWhitelisted`].
enum Rule1Outcome {
    Accepted(RegseqType),
    Rejected,
}

/// Rule-1 zero-init whitelist walker.
///
/// Admits exactly:
/// - Primitive scalars `u8`, `u16`, `u32`, `u64`, `bool`.
/// - Arrays `[T; N]` whose element `T` is itself accepted.
/// - Structs whose fields are all accepted (recursively) and that
///   the type system treats as `Copy`.
///
/// Every other Rust type (signed integers, `usize`/`isize`, floats,
/// `char`, `u128`/`i128`, tuples, enums, unions, `NonZeroU*`,
/// `NonNull`, references, function pointers, raw pointers,
/// unresolved generic parameters / associated-type projections,
/// slices, trait objects, etc.) is rejected.
///
/// The visited set uses monomorphized [`rustc_middle::ty::Ty`] keys
/// so `Boxed<u32>` and `Boxed<NonZeroU32>` (sharing a `DefId`) are
/// walked independently. A type already on the stack is treated as
/// accepted so a hypothetical self-referential struct does not
/// spin; the outer `Copy` check the caller applies before this
/// walker still blocks genuinely unsound shapes.
fn rule1_whitelist_ty<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    type_def_map: &std::collections::HashMap<DefId, regseq_model::identity::TypeDefId>,
    ty: rustc_middle::ty::Ty<'tcx>,
    visited: &mut HashSet<rustc_middle::ty::Ty<'tcx>>,
) -> Rule1Outcome {
    use rustc_middle::ty::TyKind;

    // Reject generic parameters and unresolved projections BEFORE
    // applying the whitelist -- Rule 1 requires fully monomorphized
    // types, and accepting these would be unsound.
    match ty.kind() {
        TyKind::Param(_) | TyKind::Alias(..) | TyKind::Bound(..) | TyKind::Placeholder(_) => {
            return Rule1Outcome::Rejected;
        }
        _ => {}
    }

    if !visited.insert(ty) {
        // Cycle -- accept at the recursion re-entry site. The outer
        // Copy check the caller applies before invoking this walker
        // guarantees we only reach cycle nodes via a Copy-bearing
        // field, so accepting here cannot admit a non-Copy shape.
        return Rule1Outcome::Accepted(RegseqType::Unit);
    }
    let outcome = rule1_whitelist_ty_inner(tcx, type_def_map, ty, visited);
    visited.remove(&ty);
    outcome
}

fn rule1_whitelist_ty_inner<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    type_def_map: &std::collections::HashMap<DefId, regseq_model::identity::TypeDefId>,
    ty: rustc_middle::ty::Ty<'tcx>,
    visited: &mut HashSet<rustc_middle::ty::Ty<'tcx>>,
) -> Rule1Outcome {
    use rustc_middle::ty::TyKind;
    match ty.kind() {
        TyKind::Bool => Rule1Outcome::Accepted(RegseqType::Bool),
        TyKind::Uint(u) => match u {
            rustc_middle::ty::UintTy::U8 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U8))
            }
            rustc_middle::ty::UintTy::U16 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U16))
            }
            rustc_middle::ty::UintTy::U32 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U32))
            }
            rustc_middle::ty::UintTy::U64 => {
                Rule1Outcome::Accepted(RegseqType::Scalar(ScalarType::U64))
            }
            // u128, usize: rejected by Stage-1 whitelist narrowing.
            _ => Rule1Outcome::Rejected,
        },
        TyKind::Array(inner, len_const) => {
            let len = match len_const.try_to_target_usize(tcx) {
                Some(n) => n as usize,
                None => return Rule1Outcome::Rejected,
            };
            match rule1_whitelist_ty(tcx, type_def_map, *inner, visited) {
                Rule1Outcome::Accepted(elem_ty) => {
                    Rule1Outcome::Accepted(RegseqType::Array(Box::new(elem_ty), len))
                }
                Rule1Outcome::Rejected => Rule1Outcome::Rejected,
            }
        }
        TyKind::Adt(adt, args) => {
            // Only structs qualify. Enums and unions are explicitly
            // rejected (Stage-1 whitelist narrowing).
            if !adt.is_struct() {
                return Rule1Outcome::Rejected;
            }
            // Reject non-local ADTs (NonZeroU*, NonNull, Box, Rc,
            // Arc, Cell, RefCell, UnsafeCell, AtomicU*, etc.). The
            // Rule-1 whitelist admits only user-defined Copy
            // structs whose fields are themselves accepted; the
            // `TypeDef` map only carries local ADTs, so non-local
            // ones surface here as missing.
            let did = adt.did();
            if !did.is_local() {
                return Rule1Outcome::Rejected;
            }
            let tid = match type_def_map.get(&did) {
                Some(tid) => *tid,
                None => return Rule1Outcome::Rejected,
            };
            // Walk every field under the substituted args so nested
            // generics monomorphize correctly. Structs only have one
            // variant.
            let variant = adt.non_enum_variant();
            for field in &variant.fields {
                let field_ty = field.ty(tcx, args);
                match rule1_whitelist_ty(tcx, type_def_map, field_ty, visited) {
                    Rule1Outcome::Accepted(_) => {}
                    Rule1Outcome::Rejected => return Rule1Outcome::Rejected,
                }
            }
            Rule1Outcome::Accepted(RegseqType::Defined(tid))
        }
        // Every other TyKind is rejected. This covers:
        // - Int (i8..i64, i128), Float (f32, f64), Char
        // - Tuple (including the unit tuple)
        // - Enum, Union (handled above via !adt.is_struct())
        // - Ref, RawPtr, FnPtr, FnDef, Closure, Coroutine
        // - Dynamic (trait objects), Slice, Str
        // - Foreign, Error, and the generic-param kinds already
        //   filtered upstream.
        _ => Rule1Outcome::Rejected,
    }
}
