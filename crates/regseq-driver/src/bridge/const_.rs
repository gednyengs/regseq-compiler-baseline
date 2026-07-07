//! Const translation from rustc MIR constants into RegseqModel consts.

use regseq_model::{
    canonical_const_bytes, render_def_path, synthetic_def_path_hash, synthetic_def_path_segments,
    ArgsKey, Byte, Const, ConstId, ContentHash, DefPath, FloatKind, InlineHint, Instance,
    InstanceKind, InstanceRef, IntKind, Item, ItemKind, Linkage, Mutability, Span, StableCrateId,
    SyntheticItemKind, Ty, UintKind,
};
use rustc_middle::mir;
use rustc_middle::mir::interpret::{alloc_range, Allocation, GlobalAlloc, Scalar};
use rustc_middle::ty::{self, ScalarInt, TyKind};
use rustc_span::def_id::DefId;
use rustc_span::DUMMY_SP;
use sha2::{Digest, Sha256};

use super::{ty::translate_ty, BridgeCtxt, BridgeError, ItemHandle, PendingItem};

/// Phase-2 cache entry for a bridge-synthesised ConstMemoryStatic item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstMemoryStaticHandle {
    /// Canonical content hash for the byte-level allocation.
    pub content_hash: ContentHash,
    /// ConstId of the byte-level initializer.
    pub pointee_const: ConstId,
    /// Phase-2 item handle remapped to final ItemId during Phase 4.
    pub handle: ItemHandle,
}

/// Translate a rustc `mir::Const` into a `regseq_model::ConstId`. Per
/// spec.md section 9.7 dispatch table. Reference-typed constants trigger
/// ConstMemoryStatic Item synthesis via `synthesise_const_memory_static`.
pub fn translate_const<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    konst: mir::Const<'tcx>,
) -> Result<ConstId, BridgeError> {
    if let mir::Const::Ty(_, ty_const) = konst {
        if let Some(value) = ty_const.try_to_value() {
            return translate_ty_const_value(cx, value);
        }
    }

    let ty = konst.ty();
    let value = konst
        .eval(cx.input.tcx, ty::TypingEnv::fully_monomorphized(), DUMMY_SP)
        .map_err(|error| BridgeError::UnsupportedConst {
            const_repr: format!("const eval failed for {konst:?}: {error:?}"),
        })?;
    translate_const_value(cx, value, ty)
}

fn translate_ty_const_value<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    value: ty::Value<'tcx>,
) -> Result<ConstId, BridgeError> {
    let translated = if let Some(leaf) = value.try_to_leaf() {
        translate_scalar_int(cx, leaf, value.ty)?
    } else if value.valtree.is_zst() {
        Const::ZST
    } else {
        match value.ty.kind() {
            TyKind::Tuple(_) => {
                let children = value
                    .to_branch()
                    .iter()
                    .copied()
                    .map(|child| translate_ty_const(cx, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Const::Aggregate(children)
            }
            TyKind::Array(elem, _) if matches!(elem.kind(), TyKind::Uint(ty::UintTy::U8)) => {
                let bytes = value
                    .try_to_raw_bytes(cx.input.tcx)
                    .ok_or_else(|| BridgeError::UnsupportedConst {
                        const_repr: format!("non-byte array valtree {value:?}"),
                    })?
                    .iter()
                    .copied()
                    .map(Byte::Value)
                    .collect();
                Const::Bytes(bytes)
            }
            TyKind::Array(_, _) => {
                let children = value
                    .to_branch()
                    .iter()
                    .copied()
                    .map(|child| translate_ty_const(cx, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Const::Aggregate(children)
            }
            TyKind::Adt(def, _) if def.is_enum() => {
                let destructured = value.destructure_adt_const();
                let payload = destructured
                    .fields
                    .iter()
                    .copied()
                    .map(|child| translate_ty_const(cx, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Const::EnumValue {
                    variant_idx: regseq_model::VariantIdx::from_u32(destructured.variant.as_u32()),
                    payload,
                }
            }
            TyKind::Adt(_, _) => {
                let destructured = value.destructure_adt_const();
                let children = destructured
                    .fields
                    .iter()
                    .copied()
                    .map(|child| translate_ty_const(cx, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Const::Aggregate(children)
            }
            _ => {
                return Err(BridgeError::UnsupportedConst {
                    const_repr: format!("valtree {value:?}"),
                });
            }
        }
    };
    Ok(cx.model.consts.intern(translated))
}

fn translate_ty_const<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    value: ty::Const<'tcx>,
) -> Result<ConstId, BridgeError> {
    let Some(value) = value.try_to_value() else {
        return Err(BridgeError::UnsupportedConst {
            const_repr: format!("{value:?}"),
        });
    };
    translate_ty_const_value(cx, value)
}

/// Synthesise a ConstMemoryStatic Item for a reference-typed const.
///
/// Records a Phase-2 ItemHandle in the working cache; the handle is
/// remapped to the final ItemId during Phase 4 and the synthetic Item
/// enters `RegseqModel.items` during Phase 5.
pub fn synthesise_const_memory_static<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    pointee_const: ConstId,
) -> Result<ItemHandle, BridgeError> {
    let content_hash = content_hash_for_const(cx, pointee_const);
    if let Some(entry) = cx
        .const_memory_static_cache
        .iter()
        .find(|entry| entry.content_hash == content_hash)
    {
        return Ok(entry.handle);
    }

    let handle = cx.handles.fresh_handle();
    let synthetic_kind = SyntheticItemKind::ConstMemoryStatic {
        content_hash: content_hash.clone(),
    };
    let segments = synthetic_def_path_segments(&synthetic_kind, &mut cx.model.strings);
    let rendered = render_def_path(
        &DefPath {
            crate_id: cx.model.primary_crate,
            segments: segments.clone(),
            rendered: regseq_model::StringId::ZERO,
        },
        &cx.model.crates,
        &cx.model.strings,
    );
    let def_path = DefPath {
        crate_id: cx.model.primary_crate,
        segments,
        rendered: cx.model.strings.intern(&rendered),
    };
    let item = Item {
        def_path,
        kind: ItemKind::Static {
            mutability: Mutability::Not,
        },
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
        is_result: false,
    };
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash: synthetic_def_path_hash(&synthetic_kind),
        synthetic_kind,
        bridge_flags: regseq_model::BridgeItemFlags::default(),
    });
    cx.const_memory_static_cache.push(ConstMemoryStaticHandle {
        content_hash,
        pointee_const,
        handle,
    });
    Ok(handle)
}

/// Construct Phase-5 Static instances for cached ConstMemoryStatic items.
pub fn construct_const_memory_static_instances(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let u8_ty = cx.model.types.intern(Ty::Uint(UintKind::U8));
    for entry in &cx.const_memory_static_cache {
        let item = cx.handles.resolve(entry.handle).ok_or_else(|| {
            BridgeError::Phase4(format!(
                "ConstMemoryStatic handle {} was not assigned an ItemId",
                entry.handle.as_u32()
            ))
        })?;
        let byte_len = match cx.model.consts.resolve(entry.pointee_const) {
            Const::Bytes(bytes) => bytes.len() as u64,
            other => {
                return Err(BridgeError::UnsupportedConst {
                    const_repr: format!("ConstMemoryStatic initializer is {other:?}"),
                });
            }
        };
        let array_ty = cx.model.types.intern(Ty::Array(u8_ty, byte_len));
        let emission_name = cx.model.items[item].def_path.rendered;
        cx.model.instances.push(Instance {
            item,
            generic_args: Vec::new(),
            emission_name,
            span: Span::dummy(),
            kind: InstanceKind::Static {
                ty: array_ty,
                init: Some(entry.pointee_const),
            },
        });
    }
    Ok(())
}

/// Translate a source `static`'s initialiser into an
/// `Option<ConstId>` for `InstanceKind::Static { init }` per
/// regseq-driver-55 Change C.
///
/// Returns `Ok(None)` for foreign statics
/// (`extern "C" { static FOO: u32; }`): they carry no initialiser
/// per spec.md section 9.4's Static-init biconditional. Foreign
/// statics are out of scope per regseq-driver-55 Non-Goals; the
/// `None` arm exists only so the foreign carve-out is explicit and
/// is not exercised by the in-scope fixtures.
///
/// For source-defined statics the initialiser is rustc's evaluated
/// `eval_static_initializer` allocation. It is translated to match
/// the static's type per spec.md section 9.7's per-`Ty` const
/// dispatch (which the pretty-printer's `render_const` enforces):
///
///   * scalar-typed statics (`bool` / `char` / int / uint / float
///     and thin `&T` / `*T` / `fn`) read the leading `Scalar` from
///     the allocation and reuse the existing `translate_scalar`
///     path, so a `u32` static renders as the scalar `Const`
///     variant the spec requires (NOT `Const::Bytes`);
///   * every other (aggregate / array / union) static keeps the
///     byte-level `allocation_bytes` translation -- the same path
///     `construct_const_memory_static_instances` and the `Indirect`
///     const dispatch use -- so relocation-bearing statics
///     round-trip as `Const::Bytes` with embedded `Byte::Reloc`.
pub fn translate_static_init<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    def_id: DefId,
) -> Result<Option<ConstId>, BridgeError> {
    if cx.input.tcx.is_foreign_item(def_id) {
        return Ok(None);
    }
    let alloc = cx
        .input
        .tcx
        .eval_static_initializer(def_id)
        .map_err(|err| BridgeError::UnsupportedConst {
            const_repr: format!("static initializer eval failed for {def_id:?}: {err:?}"),
        })?;
    let rustc_ty = cx.input.tcx.type_of(def_id).instantiate_identity();
    let read_provenance = matches!(
        rustc_ty.kind(),
        TyKind::RawPtr(..) | TyKind::Ref(..) | TyKind::FnPtr(..)
    );
    let is_scalar = read_provenance
        || matches!(
            rustc_ty.kind(),
            TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_)
        );
    let value = if is_scalar {
        let layout = cx
            .input
            .tcx
            .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(rustc_ty))
            .map_err(|_| BridgeError::Layout {
                ty_repr: rustc_ty.to_string(),
            })?;
        let scalar = alloc
            .inner()
            .read_scalar(
                &cx.input.tcx,
                alloc_range(rustc_abi::Size::ZERO, layout.size),
                read_provenance,
            )
            .map_err(|err| BridgeError::UnsupportedConst {
                const_repr: format!("static {def_id:?} scalar read failed: {err:?}"),
            })?;
        translate_scalar(cx, scalar, rustc_ty)?
    } else {
        Const::Bytes(allocation_bytes(cx, alloc.inner())?)
    };
    Ok(Some(cx.model.consts.intern(value)))
}

/// Translate a source `const` item's `(ty, value)` for
/// `InstanceKind::Const` per regseq-driver-55 Change D.
///
/// The value is evaluated through the existing `translate_const`
/// path by wrapping the const's `DefId` in a
/// `mir::Const::Unevaluated` and letting `mir::Const::eval` drive
/// rustc's CTFE machinery, so the full spec.md section 9.7 const
/// dispatch (scalars, ZSTs, reference-typed const memory) is reused
/// verbatim. Consts cannot be generic in the supported subset, so
/// identity args are correct.
pub fn translate_const_item<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    def_id: DefId,
) -> Result<(regseq_model::TyId, ConstId), BridgeError> {
    let rustc_ty = cx.input.tcx.type_of(def_id).instantiate_identity();
    let ty = translate_ty(cx, rustc_ty)?;
    let identity_args = ty::GenericArgs::identity_for_item(cx.input.tcx, def_id);
    let konst =
        mir::Const::Unevaluated(mir::UnevaluatedConst::new(def_id, identity_args), rustc_ty);
    let value = translate_const(cx, konst)?;
    Ok((ty, value))
}

fn translate_const_value<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    value: mir::ConstValue,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<ConstId, BridgeError> {
    let value = match value {
        mir::ConstValue::Scalar(scalar) => translate_scalar(cx, scalar, ty)?,
        mir::ConstValue::ZeroSized => Const::ZST,
        mir::ConstValue::Slice { alloc_id, meta } => {
            let alloc = cx.input.tcx.global_alloc(alloc_id);
            let GlobalAlloc::Memory(memory) = alloc else {
                return Err(BridgeError::UnsupportedConst {
                    const_repr: format!("Slice const over {alloc:?}"),
                });
            };
            let bytes = allocation_bytes(cx, memory.inner())?;
            let pointee = cx.model.consts.intern(Const::Bytes(bytes));
            synthesise_const_memory_static(cx, pointee)?;
            let inner = Const::Reloc(instance_ref_for_const(cx, pointee));
            wrap_reference_const(cx, ty, inner, meta as u128)?
        }
        mir::ConstValue::Indirect { alloc_id, offset } => {
            if offset.bytes() != 0 {
                return Err(BridgeError::UnsupportedConst {
                    const_repr: format!("Indirect const with non-zero offset {}", offset.bytes()),
                });
            }
            let alloc = cx.input.tcx.global_alloc(alloc_id);
            match alloc {
                GlobalAlloc::Memory(memory) => {
                    let bytes = allocation_bytes(cx, memory.inner())?;
                    if matches!(ty.kind(), TyKind::Ref(..)) {
                        let pointee = cx.model.consts.intern(Const::Bytes(bytes));
                        synthesise_const_memory_static(cx, pointee)?;
                        let inner = Const::Reloc(instance_ref_for_const(cx, pointee));
                        wrap_reference_const(cx, ty, inner, 0)?
                    } else if matches!(ty.kind(), TyKind::RawPtr(..)) {
                        let pointee = cx.model.consts.intern(Const::Bytes(bytes));
                        synthesise_const_memory_static(cx, pointee)?;
                        Const::Reloc(instance_ref_for_const(cx, pointee))
                    } else {
                        Const::Bytes(bytes)
                    }
                }
                other => {
                    return Err(BridgeError::UnsupportedConst {
                        const_repr: format!("Indirect const over {other:?}"),
                    });
                }
            }
        }
    };
    Ok(cx.model.consts.intern(value))
}

/// Wrap an inner `Const::Reloc` for a Rust reference-typed const into the
/// `Const::Aggregate([Reloc, Uint(meta)])` carrier mandated by spec
/// section 9.7's reference-const dispatch + the audit-finding refinement
/// requiring a uniform Aggregate carrier for thin references (with
/// `Uint(0)` as the metadata placeholder).
fn wrap_reference_const<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    inner_reloc: Const,
    meta: u128,
) -> Result<Const, BridgeError> {
    intern_fat_pointer_auxiliary_tys(cx, ty)?;
    let reloc = cx.model.consts.intern(inner_reloc);
    let len = cx.model.consts.intern(Const::Uint(UintKind::Usize, meta));
    Ok(Const::Aggregate(vec![reloc, len]))
}

fn translate_scalar<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    scalar: Scalar,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<Const, BridgeError> {
    match scalar {
        Scalar::Int(value) => translate_scalar_int(cx, value, ty),
        Scalar::Ptr(ptr, _) => {
            let alloc_id = ptr.provenance.alloc_id();
            let alloc = cx.input.tcx.global_alloc(alloc_id);
            let inner_reloc = match alloc {
                GlobalAlloc::Memory(memory) => {
                    let bytes = allocation_bytes(cx, memory.inner())?;
                    let pointee = cx.model.consts.intern(Const::Bytes(bytes));
                    synthesise_const_memory_static(cx, pointee)?;
                    Const::Reloc(instance_ref_for_const(cx, pointee))
                }
                GlobalAlloc::Static(def_id) => {
                    Const::Reloc(instance_ref_for_static(cx.input.tcx, def_id))
                }
                GlobalAlloc::Function { instance } => {
                    Const::Reloc(instance_ref_for_function_provenance(cx, instance)?)
                }
                _ => {
                    return Err(BridgeError::UnsupportedConst {
                        const_repr: format!("pointer scalar over {alloc:?}"),
                    });
                }
            };
            // Per the audit-finding refinement, Rust reference-typed
            // consts (`TyKind::Ref`) emit the uniform Aggregate carrier
            // even when the pointer is thin (`Uint(0)` placeholder).
            // Raw pointers (`TyKind::RawPtr`) keep the bare `Const::Reloc`
            // shape.
            if matches!(ty.kind(), TyKind::Ref(..)) {
                wrap_reference_const(cx, ty, inner_reloc, 0)
            } else {
                Ok(inner_reloc)
            }
        }
    }
}

fn intern_fat_pointer_auxiliary_tys<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<(), BridgeError> {
    let pointee = match ty.kind() {
        TyKind::Ref(_, pointee, _) | TyKind::RawPtr(pointee, _) => *pointee,
        _ => {
            return Err(BridgeError::UnsupportedConst {
                const_repr: format!("fat-pointer const with non-pointer type {ty}"),
            });
        }
    };
    let pointee = translate_unsized_pointee_ty(cx, pointee)?;
    cx.model.types.intern(Ty::Ptr(pointee, Mutability::Not));
    cx.model.types.intern(Ty::Uint(UintKind::Usize));
    Ok(())
}

fn translate_unsized_pointee_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<regseq_model::TyId, BridgeError> {
    match ty.kind() {
        TyKind::Str => Ok(cx.model.types.intern(Ty::Str)),
        TyKind::Slice(elem) => {
            let elem = translate_ty(cx, *elem)?;
            Ok(cx.model.types.intern(Ty::Slice(elem)))
        }
        _ => translate_ty(cx, ty),
    }
}

fn translate_scalar_int<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    value: ScalarInt,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<Const, BridgeError> {
    let layout = cx
        .input
        .tcx
        .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: ty.to_string(),
        })?;
    let bits = value
        .try_to_bits(layout.size)
        .map_err(|_| BridgeError::UnsupportedConst {
            const_repr: format!("scalar int {value:?} for {}", ty),
        })?;

    Ok(match ty.kind() {
        TyKind::Bool => Const::Bool(bits != 0),
        TyKind::Char => Const::Char(bits as u32),
        TyKind::Int(kind) => Const::Int(
            translate_int_kind(*kind),
            sign_extend(bits, layout.size.bits()),
        ),
        TyKind::Uint(kind) => Const::Uint(translate_uint_kind(*kind), bits),
        TyKind::Float(kind) => Const::Float(translate_float_kind(*kind), bits),
        TyKind::RawPtr(pointee, _) | TyKind::Ref(_, pointee, _) if bits == 0 => {
            Const::NullPtr(translate_ty(cx, *pointee)?)
        }
        _ => {
            match translate_scalar_int_aggregate(cx, value, ty)? {
                Some(aggregate) => aggregate,
                None => {
                    return Err(BridgeError::UnsupportedConst {
                        const_repr: format!("scalar int {value:?} for {}", ty),
                    });
                }
            }
        }
    })
}

/// Reconstruct the `Const::Aggregate` carrier for a scalar-ABI aggregate
/// whose MIR `ConstValue` is a bare `Scalar` (e.g. a single-field newtype
/// carrier such as `ErrBits { bits: u8 }` or `Fv(u32)`, or a recursion
/// of such). rustc collapses a struct/tuple whose layout is dominated by one
/// non-ZST scalar field down to that field's scalar, so the const reaches
/// `translate_scalar_int` with an aggregate *type* but a scalar *value*. The
/// scalar maps onto the single non-ZST field (its bits equal the aggregate's
/// by the scalar-ABI invariant); ZST siblings lower to `Const::ZST`. This
/// mirrors the `Const::Aggregate` shape the valtree path already emits for
/// `TyKind::Adt` / `TyKind::Tuple` in `translate_ty_const_value`, so both
/// const-translation paths produce the same model shape. Returns `None` when
/// `ty` is not a struct/tuple aggregate, so genuinely unsupported scalar
/// consts are still rejected by the caller.
fn translate_scalar_int_aggregate<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    value: ScalarInt,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<Option<Const>, BridgeError> {
    let field_tys: Vec<rustc_middle::ty::Ty<'tcx>> = match ty.kind() {
        TyKind::Adt(def, args) if def.is_struct() => def
            .non_enum_variant()
            .fields
            .iter()
            .map(|field| field.ty(cx.input.tcx, args))
            .collect(),
        TyKind::Tuple(elems) => elems.iter().collect(),
        _ => return Ok(None),
    };

    let mut children = Vec::with_capacity(field_tys.len());
    let mut non_zst_fields = 0usize;
    for field_ty in field_tys {
        let field_layout = cx
            .input
            .tcx
            .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(field_ty))
            .map_err(|_| BridgeError::Layout {
                ty_repr: field_ty.to_string(),
            })?;
        // The scalar-ABI invariant guarantees exactly one non-ZST field; it
        // carries the same bits as the aggregate scalar, so recurse with the
        // same value. ZST siblings (PhantomData, `()`, ...) carry no bits.
        let child = if field_layout.is_zst() {
            Const::ZST
        } else {
            non_zst_fields += 1;
            translate_scalar_int(cx, value, field_ty)?
        };
        children.push(cx.model.consts.intern(child));
    }
    // A bare `Scalar` ConstValue only reaches here for a scalar-ABI aggregate,
    // which has exactly one non-ZST field. Reject (rather than silently
    // duplicate the scalar across fields) if that invariant does not hold — a
    // multi-scalar aggregate would arrive as `ScalarPair`/`Indirect`, not here.
    if non_zst_fields != 1 {
        return Ok(None);
    }
    Ok(Some(Const::Aggregate(children)))
}

fn allocation_bytes<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    allocation: &Allocation,
) -> Result<Vec<Byte>, BridgeError> {
    let raw = allocation.inspect_with_uninit_and_ptr_outside_interpreter(0..allocation.len());
    let mut bytes = Vec::with_capacity(raw.len());
    let mut idx = 0;
    while idx < raw.len() {
        let offset = rustc_abi::Size::from_bytes(idx as u64);
        if let Some(prov) = allocation.provenance().get_ptr(offset) {
            let alloc = cx.input.tcx.global_alloc(prov.alloc_id());
            let reloc = match alloc {
                GlobalAlloc::Memory(memory) => {
                    let pointee_bytes = allocation_bytes(cx, memory.inner())?;
                    let pointee = cx.model.consts.intern(Const::Bytes(pointee_bytes));
                    synthesise_const_memory_static(cx, pointee)?;
                    Byte::Reloc(instance_ref_for_const(cx, pointee))
                }
                GlobalAlloc::Static(def_id) => {
                    Byte::Reloc(instance_ref_for_static(cx.input.tcx, def_id))
                }
                GlobalAlloc::Function { instance } => {
                    Byte::Reloc(instance_ref_for_function_provenance(cx, instance)?)
                }
                _ => {
                    return Err(BridgeError::UnsupportedConst {
                        const_repr: format!("allocation provenance over {alloc:?}"),
                    });
                }
            };
            bytes.extend(std::iter::repeat_n(
                reloc,
                cx.input.tcx.data_layout.pointer_size().bytes_usize(),
            ));
            idx += cx.input.tcx.data_layout.pointer_size().bytes_usize();
            continue;
        }

        let byte_range = alloc_range(offset, rustc_abi::Size::from_bytes(1));
        if allocation
            .init_mask()
            .is_range_initialized(byte_range)
            .is_ok()
        {
            bytes.push(Byte::Value(raw[idx]));
        } else {
            bytes.push(Byte::Uninit);
        }
        idx += 1;
    }
    Ok(bytes)
}

/// Materialize the function instance carried by a
/// `GlobalAlloc::Function { instance }` provenance and return the
/// canonical `InstanceRef` to it, so a function pointer baked into a
/// `const` or `static` lowers to a `Const::Reloc` / `Byte::Reloc`
/// targeting that instance.
///
/// Mirrors the `bind_closure_once_shim_instance` materialization
/// recipe in `bridge/body/terminator.rs`: build the callee `FnDef`
/// `Ty` from the instance identity, translate it to
/// `Ty::FnDef(item, args)`, map the rustc instance kind to a
/// `FunctionInstanceFlavor` (so a `#[track_caller]` reify keeps its
/// `ReifyShim` flavor), then materialize and dedup the Function
/// instance through `lookup_or_create_direct_instance`. The returned
/// reference is read back from the materialized instance via
/// `RegseqModel::instance_ref_of`, so it cannot drift from the
/// reference set the validator accepts in `check_instance_ref`.
pub(crate) fn instance_ref_for_function_provenance<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance: ty::Instance<'tcx>,
) -> Result<InstanceRef, BridgeError> {
    let callee_ty = ty::Ty::new_fn_def(cx.input.tcx, instance.def_id(), instance.args);
    let model_ty = translate_ty(cx, callee_ty)?;
    let Ty::FnDef(item, model_args) = cx.model.types.resolve(model_ty).clone() else {
        return Err(BridgeError::UnsupportedConst {
            const_repr: format!("function provenance over non-FnDef type {callee_ty}"),
        });
    };
    let flavor = super::function_instance_flavor_for_kind(instance.def);
    let def_id = instance.def_id();
    let args = instance.args;
    let id = super::body::terminator::lookup_or_create_direct_instance(
        cx,
        item,
        model_args,
        flavor,
        None,
        move |cx| {
            let poly = cx.input.tcx.fn_sig(def_id).instantiate(cx.input.tcx, args);
            // Concrete instance signature: resolve associated-type projections
            // (e.g. a `BitOr::Output` return reached via a fn-pointer const /
            // `ReifyFnPointer`) before translation.
            super::fnsig::translate_fn_sig_monomorphic(cx, poly)
        },
    )?;
    Ok(cx.model.instance_ref_of(id))
}

fn instance_ref_for_const(cx: &BridgeCtxt<'_>, pointee_const: ConstId) -> InstanceRef {
    let content_hash = content_hash_for_const(cx, pointee_const);
    let kind = SyntheticItemKind::ConstMemoryStatic { content_hash };
    InstanceRef {
        regseq_kind_rank: 1,
        def_path_hash: synthetic_def_path_hash(&kind),
        args_key: ArgsKey::EMPTY,
        fn_shim_disambig: 0,
    }
}

fn instance_ref_for_static(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> InstanceRef {
    InstanceRef {
        regseq_kind_rank: 1,
        def_path_hash: def_path_hash(tcx, def_id),
        args_key: ArgsKey::EMPTY,
        fn_shim_disambig: 0,
    }
}

fn def_path_hash(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: DefId) -> regseq_model::DefPathHash {
    let hash = tcx.def_path_hash(def_id);
    regseq_model::DefPathHash {
        stable_crate_id: StableCrateId(hash.stable_crate_id().as_u64()),
        local_hash: hash.local_hash().as_u64(),
    }
}

fn content_hash_for_const(cx: &BridgeCtxt<'_>, pointee_const: ConstId) -> ContentHash {
    let bytes = canonical_const_bytes(&cx.model, pointee_const);
    let digest = Sha256::digest(bytes);
    let mut head = [0; 16];
    head.copy_from_slice(&digest[..16]);
    ContentHash(u128::from_be_bytes(head))
}

fn sign_extend(bits: u128, width: u64) -> u128 {
    if width == 128 || width == 0 {
        return bits;
    }
    let sign_bit = 1_u128 << (width - 1);
    if bits & sign_bit == 0 {
        bits
    } else {
        bits | (!0_u128 << width)
    }
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
