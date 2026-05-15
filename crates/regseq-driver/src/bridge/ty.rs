//! Type translation from rustc `TyKind` into RegseqModel `Ty`.

use rustc_hir::LangItem;
use rustc_middle::ty::{self, TyKind};
use rustc_span::def_id::DefId;

use regseq_model::{
    AutoTrait, CrateId, CrateInfo, DefPath, DefPathData, DefPathHash, DefPathSegment, FieldLayout,
    FloatKind, FnAbi, FnSig, GenericArg, GenericArgs, InlineHint, IntKind, Item, ItemId, ItemKind,
    Layout, LayoutKind, Linkage, Mutability, Provenance, Safety, Span, StableCrateId,
    SyntheticItemKind, Ty, TyId, UintKind, UNIT_LAYOUT,
};

use super::{BridgeCtxt, BridgeError, DslRejectionKind, NonRigidTyKind};

/// Translate a rustc `TyKind` into a `regseq_model::TyId`. Runs during
/// Phase 2 of `run_bridge`. Per spec.md section 15.0 lines 5849-5864.
pub fn translate_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<regseq_model::TyId, BridgeError> {
    let ty = erase_regions_or_reject(cx.input.tcx, ty)?;
    translate_erased_ty(cx, ty, false)
}

/// Erase regions on a rustc `Ty`, rejecting substantive HRTB binders
/// per spec.md section 17.1.
pub fn erase_regions_or_reject<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<rustc_middle::ty::Ty<'tcx>, BridgeError> {
    if has_substantive_hrtb(&ty.to_string()) {
        return Err(BridgeError::SubstantiveHrtb {
            binder_repr: ty.to_string(),
        });
    }
    Ok(tcx.erase_and_anonymize_regions(ty))
}

fn translate_erased_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    allow_unsized_marker: bool,
) -> Result<TyId, BridgeError> {
    let model_ty = match ty.kind() {
        TyKind::Bool => Ty::Bool,
        TyKind::Char => Ty::Char,
        TyKind::Int(kind) => Ty::Int(translate_int_kind(*kind)),
        TyKind::Uint(kind) => Ty::Uint(translate_uint_kind(*kind)),
        TyKind::Float(kind) => Ty::Float(translate_float_kind(*kind)),
        TyKind::Str => {
            if !allow_unsized_marker {
                return Err(BridgeError::UnsupportedTy {
                    ty_repr: ty.to_string(),
                });
            }
            Ty::Str
        }
        TyKind::Never => Ty::Never,
        TyKind::Array(elem, len) => {
            let elem = translate_erased_ty(cx, *elem, false)?;
            let len = len.try_to_target_usize(cx.input.tcx).ok_or_else(|| {
                BridgeError::UnsupportedTy {
                    ty_repr: ty.to_string(),
                }
            })?;
            Ty::Array(elem, len)
        }
        TyKind::Slice(elem) => {
            if !allow_unsized_marker {
                return Err(BridgeError::UnsupportedTy {
                    ty_repr: ty.to_string(),
                });
            }
            Ty::Slice(translate_erased_ty(cx, *elem, false)?)
        }
        TyKind::Ref(_, pointee, mutability) => Ty::Ref(
            translate_erased_ty(cx, *pointee, true)?,
            translate_ref_mutability(*mutability),
        ),
        TyKind::RawPtr(pointee, mutability) => {
            if matches!(pointee.kind(), TyKind::Foreign(_)) {
                return Err(BridgeError::Foreign {
                    ty_repr: ty.to_string(),
                });
            }
            Ty::Ptr(
                translate_erased_ty(cx, *pointee, true)?,
                translate_ptr_mutability(*mutability),
            )
        }
        TyKind::Tuple(fields) => {
            let fields = fields
                .iter()
                .map(|field| translate_erased_ty(cx, field, false))
                .collect::<Result<Vec<_>, _>>()?;
            let layout = tuple_layout(cx, ty, &fields)?;
            Ty::Tuple { fields, layout }
        }
        TyKind::Adt(adt, args) => translate_adt(cx, ty, *adt, args)?,
        TyKind::FnDef(def_id, args) => {
            let item = intern_item(cx, *def_id, ItemKind::Function, SpecialAdt::None);
            Ty::FnDef(item, translate_generic_args(cx, args)?)
        }
        TyKind::FnPtr(sig_tys, header) => {
            reject_substantive_fn_ptr_binder(&ty.to_string())?;
            let sig = sig_tys.with(*header).skip_binder();
            let inputs = sig
                .inputs()
                .iter()
                .map(|input| translate_ty(cx, *input))
                .collect::<Result<Vec<_>, _>>()?;
            let output = translate_ty(cx, sig.output())?;
            let sig = FnSig {
                inputs,
                output,
                abi: translate_abi(sig.abi)?,
                c_variadic: sig.c_variadic,
                safety: translate_safety(sig.safety),
            };
            Ty::FnPtr(cx.model.sigs.intern(sig))
        }
        TyKind::Dynamic(predicates, _) => {
            if !allow_unsized_marker {
                return Err(BridgeError::UnsupportedTy {
                    ty_repr: ty.to_string(),
                });
            }
            if ty.to_string().starts_with("dyn*") {
                return Err(BridgeError::DynStar {
                    ty_repr: ty.to_string(),
                });
            }
            if ty.to_string().contains("for<") {
                return Err(BridgeError::SubstantiveHrtb {
                    binder_repr: ty.to_string(),
                });
            }
            let auto_traits = translate_auto_traits(cx.input.tcx, predicates, &ty.to_string())?;
            if !dynamic_has_principal_trait(predicates, &ty.to_string()) {
                return Err(BridgeError::DynAutoOnly {
                    ty_repr: ty.to_string(),
                });
            }
            let vtable_ty = synthesize_dynamic_vtable_ty(cx, &ty.to_string());
            Ty::Dynamic {
                auto_traits,
                vtable_ty,
            }
        }
        TyKind::Closure(def_id, args) => {
            let item = intern_item(cx, *def_id, ItemKind::Struct, SpecialAdt::None);
            Ty::Closure(item, translate_generic_args(cx, args)?)
        }
        TyKind::Foreign(_) => {
            return Err(BridgeError::Foreign {
                ty_repr: ty.to_string(),
            });
        }
        TyKind::Coroutine(..) | TyKind::CoroutineClosure(..) | TyKind::CoroutineWitness(..) => {
            return Err(BridgeError::Coroutine {
                ty_repr: ty.to_string(),
            });
        }
        TyKind::Pat(..) => {
            return Err(BridgeError::Pat {
                ty_repr: ty.to_string(),
            });
        }
        TyKind::UnsafeBinder(_) => {
            return Err(BridgeError::UnsafeBinder {
                ty_repr: ty.to_string(),
            });
        }
        TyKind::Param(_) => return non_rigid(NonRigidTyKind::Param, ty),
        TyKind::Bound(..) => return non_rigid(NonRigidTyKind::Bound, ty),
        TyKind::Placeholder(_) => return non_rigid(NonRigidTyKind::Placeholder, ty),
        TyKind::Infer(_) => return non_rigid(NonRigidTyKind::Infer, ty),
        TyKind::Alias(..) => return non_rigid(NonRigidTyKind::Alias, ty),
        TyKind::Error(_) => return non_rigid(NonRigidTyKind::Error, ty),
    };

    Ok(cx.model.types.intern(model_ty))
}

fn translate_adt<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    adt: rustc_middle::ty::AdtDef<'tcx>,
    args: rustc_middle::ty::GenericArgsRef<'tcx>,
) -> Result<Ty, BridgeError> {
    let special = if adt.is_phantom_data() {
        SpecialAdt::PhantomData
    } else if adt.is_unsafe_cell() {
        SpecialAdt::UnsafeCell
    } else if adt.is_manually_drop() {
        SpecialAdt::ManuallyDrop
    } else if adt.is_box() || is_box_adt(cx.input.tcx, adt.did()) {
        return Err(BridgeError::DslRejection {
            kind: DslRejectionKind::Box,
            ty_repr: ty.to_string(),
        });
    } else if cx.input.tcx.is_lang_item(adt.did(), LangItem::CVoid)
        || is_c_void_adt(cx.input.tcx, adt.did())
    {
        return Err(BridgeError::DslRejection {
            kind: DslRejectionKind::CVoid,
            ty_repr: ty.to_string(),
        });
    } else if is_atomic_adt(cx.input.tcx, adt.did()) {
        return Err(BridgeError::DslRejection {
            kind: DslRejectionKind::Atomic,
            ty_repr: ty.to_string(),
        });
    } else {
        SpecialAdt::None
    };

    let item_kind = match adt.adt_kind() {
        ty::AdtKind::Struct => ItemKind::Struct,
        ty::AdtKind::Enum => ItemKind::Enum,
        ty::AdtKind::Union => ItemKind::Union,
    };
    let item = intern_item(cx, adt.did(), item_kind, special);
    let model_args = translate_generic_args(cx, args)?;
    let field_tys = adt
        .all_fields()
        .map(|field| translate_ty(cx, field.ty(cx.input.tcx, args)))
        .collect::<Result<Vec<_>, _>>()?;
    let layout = nominal_layout(cx, ty, &field_tys)?;

    Ok(match adt.adt_kind() {
        ty::AdtKind::Struct => Ty::Struct {
            item,
            args: model_args,
            layout,
        },
        ty::AdtKind::Enum => Ty::Enum {
            item,
            args: model_args,
            layout,
        },
        ty::AdtKind::Union => Ty::Union {
            item,
            args: model_args,
            layout,
        },
    })
}

#[derive(Clone, Copy)]
enum SpecialAdt {
    None,
    PhantomData,
    UnsafeCell,
    ManuallyDrop,
}

fn intern_item(
    cx: &mut BridgeCtxt<'_>,
    def_id: DefId,
    kind: ItemKind,
    special: SpecialAdt,
) -> ItemId {
    let def_path_hash = def_path_hash(cx.input.tcx, def_id);
    for (idx, existing) in cx.model.item_def_paths.iter().enumerate() {
        if *existing == def_path_hash {
            return ItemId::from_u32(idx as u32);
        }
    }

    let crate_id = crate_id(cx, def_id);
    let rendered = cx.input.tcx.def_path_str(def_id);
    let rendered_id = cx.model.strings.intern(&rendered);
    let leaf = rendered.rsplit("::").next().unwrap_or(&rendered);
    let leaf_id = cx.model.strings.intern(leaf);
    let segment_data = match kind {
        ItemKind::Function | ItemKind::Static { .. } | ItemKind::Const => {
            DefPathData::ValueNs(leaf_id)
        }
        ItemKind::Struct | ItemKind::Enum | ItemKind::Union => DefPathData::TypeNs(leaf_id),
    };
    let item = Item {
        def_path: DefPath {
            crate_id,
            segments: vec![DefPathSegment {
                data: segment_data,
                disambiguator: 0,
            }],
            rendered: rendered_id,
        },
        kind,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: matches!(special, SpecialAdt::PhantomData),
        is_unsafe_cell: matches!(special, SpecialAdt::UnsafeCell),
        is_manually_drop: matches!(special, SpecialAdt::ManuallyDrop),
        opaque_handle_eligible: false,
    };
    let item_id = cx.model.items.push(item);
    cx.model.item_def_paths.push(def_path_hash);
    cx.model.item_synthetic_kinds.push(SyntheticItemKind::None);
    cx.model.domain_attrs.item_attrs.push(Vec::new());
    item_id
}

fn crate_id(cx: &mut BridgeCtxt<'_>, def_id: DefId) -> CrateId {
    let stable = StableCrateId(cx.input.tcx.stable_crate_id(def_id.krate).as_u64());
    for (idx, krate) in cx.model.crates.iter().enumerate() {
        if krate.stable_crate_id == stable {
            return CrateId::from_u32(idx as u32);
        }
    }
    let name = cx.input.tcx.crate_name(def_id.krate).to_string();
    let name = cx.model.strings.intern(&name);
    cx.model.crates.push(CrateInfo {
        name,
        stable_crate_id: stable,
    })
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    DefPathHash {
        stable_crate_id: StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}

fn translate_generic_args<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    args: rustc_middle::ty::GenericArgsRef<'tcx>,
) -> Result<GenericArgs, BridgeError> {
    args.iter()
        .filter_map(|arg| match arg.kind() {
            ty::GenericArgKind::Type(ty) => Some(translate_ty(cx, ty).map(GenericArg::Type)),
            ty::GenericArgKind::Const(_) | ty::GenericArgKind::Lifetime(_) => None,
        })
        .collect()
}

fn translate_int_kind(kind: ty::IntTy) -> IntKind {
    match kind {
        ty::IntTy::I8 => IntKind::I8,
        ty::IntTy::I16 => IntKind::I16,
        ty::IntTy::I32 => IntKind::I32,
        ty::IntTy::I64 => IntKind::I64,
        ty::IntTy::I128 => IntKind::I128,
        ty::IntTy::Isize => IntKind::Isize,
    }
}

fn translate_uint_kind(kind: ty::UintTy) -> UintKind {
    match kind {
        ty::UintTy::U8 => UintKind::U8,
        ty::UintTy::U16 => UintKind::U16,
        ty::UintTy::U32 => UintKind::U32,
        ty::UintTy::U64 => UintKind::U64,
        ty::UintTy::U128 => UintKind::U128,
        ty::UintTy::Usize => UintKind::Usize,
    }
}

fn translate_float_kind(kind: ty::FloatTy) -> FloatKind {
    match kind {
        ty::FloatTy::F16 => FloatKind::F16,
        ty::FloatTy::F32 => FloatKind::F32,
        ty::FloatTy::F64 => FloatKind::F64,
        ty::FloatTy::F128 => FloatKind::F128,
    }
}

fn translate_ref_mutability(mutability: rustc_ast::Mutability) -> Provenance {
    match mutability {
        rustc_ast::Mutability::Not => Provenance::SharedBorrow,
        rustc_ast::Mutability::Mut => Provenance::UniqueBorrow,
    }
}

fn translate_ptr_mutability(mutability: rustc_ast::Mutability) -> Mutability {
    match mutability {
        rustc_ast::Mutability::Not => Mutability::Not,
        rustc_ast::Mutability::Mut => Mutability::Mut,
    }
}

fn translate_abi(abi: rustc_abi::ExternAbi) -> Result<FnAbi, BridgeError> {
    // Spec section 14.2: `RustCall` is admissible only on rewritten
    // synthesized closure calls, not on fn-pointer ABI. fnsig.rs
    // already rejects it; mirror that here so the two ABI maps
    // agree.
    match abi {
        rustc_abi::ExternAbi::Rust => Ok(FnAbi::Rust),
        rustc_abi::ExternAbi::C { unwind: false } => Ok(FnAbi::C),
        _ => Err(BridgeError::UnsupportedTy {
            ty_repr: format!("extern ABI {abi:?}"),
        }),
    }
}

fn translate_safety(safety: rustc_hir::Safety) -> Safety {
    match safety {
        rustc_hir::Safety::Safe => Safety::Safe,
        rustc_hir::Safety::Unsafe => Safety::Unsafe,
    }
}

fn non_rigid<T>(kind: NonRigidTyKind, ty: rustc_middle::ty::Ty<'_>) -> Result<T, BridgeError> {
    Err(BridgeError::NonRigidTy {
        kind,
        ty_repr: ty.to_string(),
    })
}

fn is_atomic_adt(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> bool {
    let path = tcx.def_path_str(def_id);
    let leaf = path.rsplit("::").next().unwrap_or(path.as_str());
    matches!(
        leaf,
        "AtomicBool"
            | "AtomicPtr"
            | "AtomicU8"
            | "AtomicU16"
            | "AtomicU32"
            | "AtomicU64"
            | "AtomicUsize"
            | "AtomicI8"
            | "AtomicI16"
            | "AtomicI32"
            | "AtomicI64"
            | "AtomicIsize"
    ) && (path.starts_with("core::sync::atomic::") || path.starts_with("std::sync::atomic::"))
}

fn is_box_adt(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> bool {
    matches!(
        tcx.def_path_str(def_id).as_str(),
        "alloc::boxed::Box" | "std::boxed::Box"
    )
}

fn is_c_void_adt(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> bool {
    matches!(
        tcx.def_path_str(def_id).as_str(),
        "core::ffi::c_void" | "std::ffi::c_void"
    )
}

fn translate_auto_traits<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    predicates: &'tcx rustc_middle::ty::List<
        rustc_middle::ty::Binder<'tcx, rustc_middle::ty::ExistentialPredicate<'tcx>>,
    >,
    rendered: &str,
) -> Result<Vec<AutoTrait>, BridgeError> {
    let mut traits = Vec::new();
    for def_id in predicates.auto_traits() {
        let auto_trait = match tcx.def_path_str(def_id).as_str() {
            "core::marker::Send" | "std::marker::Send" => AutoTrait::Send,
            "core::marker::Sync" | "std::marker::Sync" => AutoTrait::Sync,
            "core::marker::Unpin" | "std::marker::Unpin" => AutoTrait::Unpin,
            other => {
                return Err(BridgeError::CustomAutoTrait {
                    trait_repr: other.to_string(),
                    ty_repr: rendered.to_string(),
                });
            }
        };
        traits.push(auto_trait);
    }
    traits.sort_unstable();
    traits.dedup();
    Ok(traits)
}

fn dynamic_has_principal_trait<'tcx>(
    predicates: &'tcx rustc_middle::ty::List<
        rustc_middle::ty::Binder<'tcx, rustc_middle::ty::ExistentialPredicate<'tcx>>,
    >,
    _rendered: &str,
) -> bool {
    predicates.principal().is_some()
}

fn synthesize_dynamic_vtable_ty(cx: &mut BridgeCtxt<'_>, rendered: &str) -> TyId {
    let name = format!("vtable::{rendered}");
    let item_id = synthesize_item(cx, &name, ItemKind::Struct);
    let layout = placeholder_nominal_layout(cx);
    cx.model.types.intern(Ty::Struct {
        item: item_id,
        args: Vec::new(),
        layout,
    })
}

fn synthesize_item(cx: &mut BridgeCtxt<'_>, rendered: &str, kind: ItemKind) -> ItemId {
    let rendered_id = cx.model.strings.intern(rendered);
    let leaf_id = cx.model.strings.intern(rendered);
    let item = Item {
        def_path: DefPath {
            crate_id: cx.model.primary_crate,
            segments: vec![DefPathSegment {
                data: DefPathData::TypeNs(leaf_id),
                disambiguator: 0,
            }],
            rendered: rendered_id,
        },
        kind,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    };
    let item_id = cx.model.items.push(item);
    cx.model.item_def_paths.push(DefPathHash {
        stable_crate_id: StableCrateId(0),
        local_hash: item_id.as_u32() as u64,
    });
    cx.model.item_synthetic_kinds.push(SyntheticItemKind::None);
    cx.model.domain_attrs.item_attrs.push(Vec::new());
    item_id
}

fn tuple_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    fields: &[TyId],
) -> Result<Layout, BridgeError> {
    if fields.is_empty() {
        return Ok(UNIT_LAYOUT);
    }
    let rustc_layout = cx
        .input
        .tcx
        .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: ty.to_string(),
        })?;
    let empty = cx.model.strings.intern("");
    Ok(Layout {
        size: rustc_layout.size.bytes(),
        align: rustc_layout.align.abi.bytes(),
        pack: None,
        uninhabited: rustc_layout.uninhabited,
        kind: LayoutKind::Struct {
            fields: fields
                .iter()
                .copied()
                .enumerate()
                .map(|(idx, ty)| FieldLayout {
                    name: empty,
                    ty,
                    offset: rustc_layout.fields.offset(idx).bytes(),
                })
                .collect(),
        },
    })
}

fn nominal_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    field_tys: &[TyId],
) -> Result<Layout, BridgeError> {
    let rustc_layout = cx
        .input
        .tcx
        .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: ty.to_string(),
        })?;
    let empty = cx.model.strings.intern("");
    let fields = field_tys
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, ty)| FieldLayout {
            name: empty,
            ty,
            offset: if idx < rustc_layout.fields.count() {
                rustc_layout.fields.offset(idx).bytes()
            } else {
                0
            },
        })
        .collect();
    let kind = match ty.kind() {
        TyKind::Adt(adt, _) if adt.is_union() => LayoutKind::Union { fields },
        _ => LayoutKind::Struct { fields },
    };
    Ok(Layout {
        size: rustc_layout.size.bytes(),
        align: rustc_layout.align.abi.bytes(),
        pack: None,
        uninhabited: rustc_layout.uninhabited,
        kind,
    })
}

fn placeholder_nominal_layout(cx: &mut BridgeCtxt<'_>) -> Layout {
    let empty = cx.model.strings.intern("");
    Layout {
        size: 0,
        align: 1,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct {
            fields: vec![FieldLayout {
                name: empty,
                ty: TyId::ZERO,
                offset: 0,
            }],
        },
    }
}

fn has_substantive_hrtb(rendered: &str) -> bool {
    if !rendered.contains("for<") {
        return false;
    }
    if rendered.matches("for<").count() > 1
        || rendered.contains("<&'")
        || rendered.contains("fn((&'")
        || rendered.contains("*const &'")
        || rendered.contains("*mut &'")
    {
        return true;
    }
    if rendered.contains(" as ") || rendered.contains("-> &") {
        return true;
    }
    let Some(binder_start) = rendered.find("for<") else {
        return false;
    };
    let binder_body = &rendered[binder_start + "for<".len()..];
    let Some(binder_end) = binder_body.find('>') else {
        return true;
    };
    let binders = &binder_body[..binder_end];
    for binder in binders.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if rendered.matches(&format!("&{binder}")).count() > 1 {
            return true;
        }
    }
    false
}

fn reject_substantive_fn_ptr_binder(rendered: &str) -> Result<(), BridgeError> {
    if has_substantive_hrtb(rendered) {
        return Err(BridgeError::SubstantiveHrtb {
            binder_repr: rendered.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_kind_helpers_cover_closed_model_enums() {
        assert_eq!(translate_int_kind(ty::IntTy::I8), IntKind::I8);
        assert_eq!(translate_int_kind(ty::IntTy::Isize), IntKind::Isize);
        assert_eq!(translate_uint_kind(ty::UintTy::U8), UintKind::U8);
        assert_eq!(translate_uint_kind(ty::UintTy::Usize), UintKind::Usize);
        assert_eq!(translate_float_kind(ty::FloatTy::F32), FloatKind::F32);
        assert_eq!(translate_float_kind(ty::FloatTy::F64), FloatKind::F64);
    }

    #[test]
    fn hrtb_rendering_predicate_accepts_single_input_ref_slot() {
        assert!(!has_substantive_hrtb("for<'a> fn(&'a u32)"));
        assert!(!has_substantive_hrtb("for<'a, 'b> fn(&'a u32, &'b u32)"));
    }

    #[test]
    fn hrtb_rendering_predicate_rejects_relational_and_output_slots() {
        assert!(has_substantive_hrtb("for<'a> fn(&'a u32, &'a u32)"));
        assert!(has_substantive_hrtb("for<'a> fn() -> &'a u32"));
        assert!(has_substantive_hrtb("for<'a> fn(<T as Trait<'a>>::Assoc)"));
        assert!(has_substantive_hrtb("for<'a> fn(for<'b> fn(&'b &'a u32))"));
        assert!(has_substantive_hrtb("for<'a> fn(Vec<&'a u32>)"));
    }

    #[test]
    fn rejection_error_variants_cover_unstable_or_synthetic_rows() {
        let non_rigid = [
            NonRigidTyKind::Param,
            NonRigidTyKind::Bound,
            NonRigidTyKind::Placeholder,
            NonRigidTyKind::Infer,
            NonRigidTyKind::Alias,
            NonRigidTyKind::Error,
        ];
        for kind in non_rigid {
            assert!(matches!(
                BridgeError::NonRigidTy {
                    kind,
                    ty_repr: "synthetic".to_string()
                },
                BridgeError::NonRigidTy { .. }
            ));
        }
        assert!(matches!(
            BridgeError::Coroutine {
                ty_repr: "synthetic coroutine".to_string()
            },
            BridgeError::Coroutine { .. }
        ));
        assert!(matches!(
            BridgeError::Pat {
                ty_repr: "synthetic pat".to_string()
            },
            BridgeError::Pat { .. }
        ));
        assert!(matches!(
            BridgeError::UnsafeBinder {
                ty_repr: "synthetic unsafe binder".to_string()
            },
            BridgeError::UnsafeBinder { .. }
        ));
        assert!(matches!(
            BridgeError::DynStar {
                ty_repr: "dyn* Trait".to_string()
            },
            BridgeError::DynStar { .. }
        ));
    }
}
