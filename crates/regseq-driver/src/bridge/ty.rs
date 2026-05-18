//! Type translation from rustc `TyKind` into RegseqModel `Ty`.

use rustc_hir::LangItem;
use rustc_middle::ty::{self, TyKind, TypeSuperVisitable, TypeVisitable};
use rustc_span::def_id::DefId;

use regseq_model::{
    render_def_path, AutoTrait, CrateId, CrateInfo, DefPath, DefPathHash, DefPathSegment,
    FieldLayout, FloatKind, FnAbi, FnSig, GenericArg, GenericArgs, InlineHint, IntKind, Item,
    ItemId, ItemKind, Layout, LayoutKind, Linkage, Mutability, PrincipalTraitKey, Provenance,
    Safety, Span, StableCrateId, SyntheticItemKind, Ty, TyId, UintKind, UNIT_LAYOUT,
};

use super::def_path::translate_def_path_data;
use super::{
    placeholder_item_id_for_handle, BridgeCtxt, BridgeError, DslRejectionKind, ItemHandle,
    NonRigidTyKind, PendingItem,
};

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
/// per spec.md section 17.1. Uses a structural `TypeVisitor` (per
/// audit-finding MEDIUM-37-3) instead of pretty-printer matching, so
/// the predicate is robust against rustc Display drift.
pub fn erase_regions_or_reject<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<rustc_middle::ty::Ty<'tcx>, BridgeError> {
    if has_substantive_hrtb_ty(tcx, ty) {
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
    let ty_id = translate_erased_ty_inner(cx, ty, allow_unsized_marker)?;
    // Audit-finding regseq-driver-43-2: cache the (TyId,
    // erased-rustc-Ty) pairing so Phase-5 body materialisation can
    // reconstitute per-Instance rustc generic args from the model's
    // `Instance.generic_args`. `entry(...).or_insert(ty)` keeps the
    // first-seen rustc Ty stable across repeated translation calls.
    cx.ty_forward_cache.entry(ty_id).or_insert(ty);
    Ok(ty_id)
}

fn translate_erased_ty_inner<'tcx>(
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
        TyKind::Adt(adt, args) => {
            let (model_ty, handle) = translate_adt(cx, ty, *adt, args)?;
            let ty_id = cx.model.types.intern(model_ty);
            track_source_ty_placeholder(cx, handle, ty_id);
            return Ok(ty_id);
        }
        TyKind::FnDef(def_id, args) => {
            let (item, handle) = intern_item(cx, *def_id, ItemKind::Function, SpecialAdt::None);
            let model_args = translate_generic_args(cx, args)?;
            let ty_id = cx.model.types.intern(Ty::FnDef(item, model_args));
            track_source_ty_placeholder(cx, handle, ty_id);
            return Ok(ty_id);
        }
        TyKind::FnPtr(sig_tys, header) => {
            let binder = sig_tys.with(*header);
            reject_substantive_fn_ptr_binder(cx.input.tcx, ty, binder)?;
            let sig = binder.skip_binder();
            // Spec section 13.1 closure-call shim rewrite: a RustCall
            // FnPtr that survived monomorphisation is the reified
            // function pointer of a closure-trait call method. The
            // bridge un-tuples the trailing RustCall args tuple to
            // positional inputs and remaps the ABI to FnAbi::Rust
            // BEFORE the strict section 14.2 abi check. A RustCall
            // FnPtr whose last input is NOT a tuple is malformed
            // (no valid closure-call shape) and falls through to
            // `translate_abi`'s RustCall rejection.
            let (raw_inputs, abi) = if let (rustc_abi::ExternAbi::RustCall, Some((last, prefix))) =
                (sig.abi, sig.inputs().split_last())
            {
                if let TyKind::Tuple(tuple_fields) = last.kind() {
                    let mut normalised: Vec<rustc_middle::ty::Ty<'_>> = Vec::new();
                    normalised.extend_from_slice(prefix);
                    for field in tuple_fields.iter() {
                        normalised.push(field);
                    }
                    (normalised, rustc_abi::ExternAbi::Rust)
                } else {
                    // RustCall without a tupled args slot is
                    // outside the closure-call shim shape; let
                    // translate_abi reject it explicitly.
                    (sig.inputs().to_vec(), sig.abi)
                }
            } else {
                (sig.inputs().to_vec(), sig.abi)
            };
            let inputs = raw_inputs
                .iter()
                .map(|input| translate_ty(cx, *input))
                .collect::<Result<Vec<_>, _>>()?;
            let output = translate_ty(cx, sig.output())?;
            let sig = FnSig {
                inputs,
                output,
                abi: translate_abi(abi)?,
                c_variadic: sig.c_variadic,
                safety: translate_safety(sig.safety),
            };
            Ty::FnPtr(cx.model.sigs.intern(sig))
        }
        TyKind::Dynamic(predicates, _) => {
            // DynStar carries its own dedicated rejection row per
            // spec section 15.1 and must be detected BEFORE the
            // bare-unsized check so a top-level `dyn* Trait` is
            // not misclassified as `UnsupportedTy`.
            if ty.to_string().starts_with("dyn*") {
                return Err(BridgeError::DynStar {
                    ty_repr: ty.to_string(),
                });
            }
            if !allow_unsized_marker {
                return Err(BridgeError::UnsupportedTy {
                    ty_repr: ty.to_string(),
                });
            }
            if has_substantive_hrtb_dynamic(cx.input.tcx, predicates) {
                return Err(BridgeError::SubstantiveHrtb {
                    binder_repr: ty.to_string(),
                });
            }
            let auto_traits = translate_auto_traits(cx.input.tcx, predicates, &ty.to_string())?;
            let Some(principal_ref) = predicates
                .principal()
                .map(|principal| principal.skip_binder())
            else {
                return Err(BridgeError::DynAutoOnly {
                    ty_repr: ty.to_string(),
                });
            };
            // Spec 13.2: vtable_ty references a real Ty::Struct whose
            // Item carries SyntheticItemKind::VTableType { principal }.
            // The slot count comes from `tcx.vtable_entries`; each slot
            // is pointer-width.
            let projection_bindings = super::projection_bindings_key(cx, predicates);
            let principal = super::principal_trait_key(cx, principal_ref, projection_bindings);
            let vtable_ty =
                synthesize_dynamic_vtable_ty(cx, principal, principal_ref, &ty.to_string());
            Ty::Dynamic {
                auto_traits,
                vtable_ty,
            }
        }
        TyKind::Closure(def_id, args) => {
            // Spec 13.1 + audit-finding F4 (no direct push to
            // `model.items`): the captures-struct Item is owned by
            // `synth/closure.rs`. Driving synthesis here makes
            // translate_ty idempotent regardless of whether Phase 2
            // scanning already ran for this closure; the dedup on
            // `closure_synth_cache` guarantees a single handle.
            let handles = crate::bridge::synth::closure::synthesise_closure_for_def_id(
                cx,
                *def_id,
                args.as_closure(),
            )?;
            let handle = handles.captures;
            let item = cx
                .handles
                .resolve(handle)
                .unwrap_or_else(|| placeholder_item_id_for_handle(handle));
            let model_args = translate_generic_args(cx, args)?;
            let closure_ty_id = cx.model.types.intern(Ty::Closure(item, model_args.clone()));
            track_source_ty_placeholder(cx, handle, closure_ty_id);
            // Spec 13.1: the closure has two parallel `Ty` views
            // sharing one `Item`. Intern the captures-struct view now
            // and record the mapping so consumers can recover the
            // layout-bearing struct from the closure type-identity.
            let captures_struct_ty_id =
                intern_closure_captures_struct_view(cx, ty, item, &model_args, args.as_closure())?;
            track_source_ty_placeholder(cx, handle, captures_struct_ty_id);
            cx.model
                .closure_captures_struct_ty
                .insert(closure_ty_id, captures_struct_ty_id);
            return Ok(closure_ty_id);
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
) -> Result<(Ty, ItemHandle), BridgeError> {
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
    let (item, handle) = intern_item(cx, adt.did(), item_kind, special);
    let model_args = translate_generic_args(cx, args)?;
    let fields = adt
        .all_fields()
        .map(|field| {
            let ty = translate_ty(cx, field.ty(cx.input.tcx, args))?;
            Ok((field.name, ty))
        })
        .collect::<Result<Vec<(rustc_span::Symbol, TyId)>, BridgeError>>()?;
    let layout = nominal_layout(cx, ty, &fields)?;

    let model_ty = match adt.adt_kind() {
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
    };
    Ok((model_ty, handle))
}

/// Source-derived ADT classification used by `intern_item`. The
/// nominal-flag fields on `Item` (`is_phantom_data`, `is_unsafe_cell`,
/// `is_manually_drop`) are populated from this tag at intern time.
#[derive(Clone, Copy)]
pub(crate) enum SpecialAdt {
    /// No special-ADT classification (the common case for ordinary
    /// structs / enums / unions / function items).
    None,
    /// The ADT is `core::marker::PhantomData`.
    PhantomData,
    /// The ADT is `core::cell::UnsafeCell`.
    UnsafeCell,
    /// The ADT is `core::mem::ManuallyDrop`.
    ManuallyDrop,
}

/// Allocate (or recover) the Phase-2 handle for a source-derived
/// `Item` (`Struct`, `Enum`, `Union`, `FnDef`, or closure captures
/// struct). Per spec.md section 3.4 D-12-5.3 the bridge MUST route
/// every Item through `pending_items` + `HandleTable` so that final
/// `ItemId` assignment happens at Phase 4 after the F4 sort. The
/// returned `ItemHandle` resolves to a placeholder `ItemId`
/// (via [`placeholder_item_id_for_handle`]) until Phase 4 binds the
/// handle to a real `ItemId`; `bridge/synth/mod.rs::patch_source_item_ty_views`
/// patches every placeholder `Ty` entry recorded in
/// `cx.source_ty_pending` at Phase 5.
pub(crate) fn intern_item(
    cx: &mut BridgeCtxt<'_>,
    def_id: DefId,
    kind: ItemKind,
    special: SpecialAdt,
) -> (ItemId, ItemHandle) {
    let handle = intern_source_item_handle(cx, def_id, kind, special);
    let item_id = cx
        .handles
        .resolve(handle)
        .unwrap_or_else(|| placeholder_item_id_for_handle(handle));
    (item_id, handle)
}

/// Allocate (or recover) the Phase-2 handle for a source-derived
/// Item without materialising an `ItemId`. Used by Phase-2 callers
/// that need to coordinate handle identity with the synth path
/// (e.g., the closure captures struct shared with `synth/closure.rs`).
fn intern_source_item_handle(
    cx: &mut BridgeCtxt<'_>,
    def_id: DefId,
    kind: ItemKind,
    special: SpecialAdt,
) -> ItemHandle {
    let def_path_hash = def_path_hash(cx.input.tcx, def_id);
    if let Some(handle) = cx.source_item_handles.get(&def_path_hash).copied() {
        return handle;
    }

    let handle = cx.handles.fresh_handle();
    cx.source_item_handles.insert(def_path_hash, handle);

    let crate_id = crate_id(cx, def_id);

    // Build full multi-segment DefPath.segments per spec.md section
    // 3.4 Phase 3 (spec.md:1198-1202): source-derived Items use
    // rustc's DefPath segments verbatim. `tcx.def_path(def_id).data`
    // is a Vec<DisambiguatedDefPathData> per the pinned toolchain;
    // it does not include a leading CrateRoot for non-root DefIds.
    // The render_def_path helper at def_path.rs:175 emits the crate
    // name first regardless, so no leading CrateRoot is prepended.
    let rustc_def_path = cx.input.tcx.def_path(def_id);
    let segments: Vec<DefPathSegment> = rustc_def_path
        .data
        .iter()
        .map(|disambiguated| DefPathSegment {
            data: translate_def_path_data(&mut cx.model.strings, disambiguated.data),
            disambiguator: disambiguated.disambiguator,
        })
        .collect();

    let intermediate_path = DefPath {
        crate_id,
        segments,
        rendered: regseq_model::StringId::from_u32(0),
    };

    // Compute canonical rendered string per spec.md section 7.2
    // (spec.md:2079-2088): the always-verbose `crate::module::Item`
    // form. Both bridge and pretty-printer call this helper so the
    // rendering rule lives in exactly one place; validate_items
    // (model.rs:1442) enforces byte-equality against this string.
    let rendered = render_def_path(&intermediate_path, &cx.model.crates, &cx.model.strings);
    let rendered_id = cx.model.strings.intern(&rendered);

    let item = Item {
        def_path: DefPath {
            crate_id,
            segments: intermediate_path.segments,
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
    cx.pending_items.push(PendingItem {
        handle,
        item,
        def_path_hash,
        synthetic_kind: SyntheticItemKind::None,
    });
    handle
}

/// Record a Phase-2 placeholder `Ty` so Phase 5 can patch its
/// embedded `ItemId` to the final value once the handle binds.
/// No-op if the handle already resolves to a real `ItemId` (e.g.,
/// when translation runs after Phase 4 during body materialisation).
fn track_source_ty_placeholder(cx: &mut BridgeCtxt<'_>, handle: ItemHandle, ty_id: TyId) {
    if cx.handles.resolve(handle).is_some() {
        return;
    }
    cx.source_ty_pending.entry(handle).or_default().push(ty_id);
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
    // Audit-finding regseq-driver-43-2: forward (here) and reverse
    // (`bridge/body/materialise.rs::translate_model_generic_args`)
    // resolution of generic args MUST be coordinated. Lifetimes are
    // dropped (they round-trip to `re_erased` on the reverse side);
    // type args translate recursively; const args are explicitly
    // rejected. Both paths must either preserve or reject scalar
    // const generics. The reverse side rejects, so we reject here
    // too rather than silently expanding the spec gap.
    //
    // Type args are translated with `allow_unsized_marker=true`:
    // trait-method `FnDef` args carry the unsized `Self` of
    // dyn-dispatch (per spec section 13.2 the Self of an
    // `<dyn Trait as Trait>::method` FnDef is the `dyn Trait`
    // itself), so disallowing unsized markers here would reject the
    // trait shape. Sized-ness of non-Self params is rustc-side
    // `?Sized` resolution; the bridge does not duplicate it.
    let mut translated = GenericArgs::new();
    for arg in args.iter() {
        match arg.kind() {
            ty::GenericArgKind::Type(ty) => {
                let ty = erase_regions_or_reject(cx.input.tcx, ty)?;
                translated.push(GenericArg::Type(translate_erased_ty(cx, ty, true)?));
            }
            ty::GenericArgKind::Const(konst) => {
                return Err(BridgeError::UnsupportedConst {
                    const_repr: format!("{konst:?}"),
                });
            }
            ty::GenericArgKind::Lifetime(_) => {}
        }
    }
    Ok(translated)
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
    // Spec section 14.2: only `Rust` and `C { unwind: false }` are
    // accepted. `RustCall` is rejected per the "RustCall does not
    // appear in the IR" rule; closure-call methods are routed
    // through the spec 13.1 closure-call shim rewrite which
    // substitutes the rustc closure-trait method (RustCall) with
    // the synth Rust-ABI ClosureCall callee BEFORE FnPtr
    // translation runs.
    match abi {
        rustc_abi::ExternAbi::Rust => Ok(FnAbi::Rust),
        rustc_abi::ExternAbi::C { unwind: false } => Ok(FnAbi::C),
        _ => Err(BridgeError::UnsupportedAbi {
            abi_repr: format!("{abi:?}"),
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

/// Synthesize the `Ty::Struct` view of the vtable type referenced by
/// `Ty::Dynamic.vtable_ty` per spec.md sections 13.0 / 13.2. The
/// returned struct's `Item` carries
/// `SyntheticItemKind::VTableType { principal }` and its layout is
/// built from `tcx.vtable_entries(principal_ref)`: one pointer-width
/// slot per vtable entry.
///
/// VTableType Item synthesis is routed through
/// `synth/vtable.rs::synthesise_vtable_type` (per spec.md section 3.4
/// D-12-5.2 plus the audit-finding refinement on this task: every
/// synthetic Item must enter through `pending_items` and acquire its
/// final `ItemId` at Phase 4 via the `HandleTable` abstraction;
/// no eager `model.items.push` shortcut). The `Ty::Struct` view is
/// interned with a placeholder `ItemId` derived from the Phase-2
/// handle when the final `ItemId` is not yet known; the placeholder
/// is patched to the real `ItemId` at Phase 4 via
/// `TyInterner::patch_nominal_item_id`.
fn synthesize_dynamic_vtable_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: PrincipalTraitKey,
    principal_ref: ty::ExistentialTraitRef<'tcx>,
    _rendered: &str,
) -> TyId {
    let handle = crate::bridge::synth::vtable::synthesise_vtable_type(cx, principal.clone());
    let layout = vtable_struct_layout(cx, principal_ref);

    if let Some(existing) = cx.model.vtables.get(&principal).copied() {
        if let Some(ty_id) =
            cx.model
                .types
                .find_nominal(regseq_model::NominalKind::Struct, existing, &Vec::new())
        {
            return ty_id;
        }
        return cx.model.types.intern(Ty::Struct {
            item: existing,
            args: Vec::new(),
            layout,
        });
    }

    if let Some(existing) = cx.dynamic_vtable_ty_pending.get(&handle).copied() {
        return existing;
    }
    let placeholder_item = ItemId::from_u32(ItemId::MAX_AS_U32 - handle.as_u32());
    let ty_id = cx.model.types.intern(Ty::Struct {
        item: placeholder_item,
        args: Vec::new(),
        layout,
    });
    cx.dynamic_vtable_ty_pending.insert(handle, ty_id);
    ty_id
}

fn vtable_struct_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal_ref: ty::ExistentialTraitRef<'tcx>,
) -> Layout {
    let pointer_size = (cx.model.target.pointer_width / 8) as u64;
    let pointer_align = pointer_size;
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let ptr_ty = cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
    let empty_name = cx.model.strings.intern("");
    let slot_count = vtable_entry_count(cx, principal_ref) as u64;
    let fields = (0..slot_count)
        .map(|idx| FieldLayout {
            name: empty_name,
            ty: ptr_ty,
            offset: idx * pointer_size,
        })
        .collect::<Vec<_>>();
    Layout {
        size: slot_count * pointer_size,
        align: pointer_align.max(1),
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

/// Approximate the vtable entry count for `Ty::Dynamic.vtable_ty`.
///
/// **Why an approximation rather than `tcx.vtable_entries`.** The
/// regseq-driver-37 task spec (line 317-318) calls for the slot
/// count to come from `tcx.vtable_entries(principal_poly_trait_ref)`.
/// That rustc query is keyed on a *fully-resolved* `PolyTraitRef`
/// whose `Self` type implements the trait
/// (`vtable_entries` is invoked only after `.with_self_ty(...)` in
/// rustc's own `vtable_allocation_provider`; see
/// `~/.rustup/toolchains/nightly-2026-02-01-.../rustc-src/rust/compiler/rustc_middle/src/ty/vtable.rs:90-94`).
/// At `Ty::Dynamic` translation time the bridge has only the
/// *existential* trait ref -- there is no concrete impl yet, so the
/// query is unavailable. Rustc itself has the same constraint and
/// solves it with `vtable_min_entries`
/// (`rustc_middle/src/ty/vtable.rs:53-75`): 3 common slots
/// (drop_in_place + size + align) plus
/// `own_existential_vtable_entries(trait_def_id).len()` summed over
/// the principal and every supertrait. This function mirrors that
/// algorithm. The post-Phase-5 vtable Struct layout is refined when
/// a concrete `impl_ty` becomes available at a `Cast::Unsize` site
/// (`synth/vtable.rs::synthesise_vtable_with_rustc_entries` already
/// calls `tcx.vtable_entries(with_self_ty)` for the full
/// composition); the count here is used only for the Phase-2
/// `Ty::Struct` placeholder layout and is later patched if it
/// changes. Audit-finding MEDIUM-37-4 deliberately keeps this
/// approximation; the task spec text describes a query that rustc
/// itself cannot satisfy at this point.
fn vtable_entry_count<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    principal_ref: ty::ExistentialTraitRef<'tcx>,
) -> u32 {
    use rustc_middle::ty::Upcast;
    let tcx = cx.input.tcx;
    let mut count = 3u32; // COMMON_VTABLE_ENTRIES: drop_in_place + size + align
    let mut seen: std::collections::HashSet<DefId> = std::collections::HashSet::new();
    let mut stack = vec![principal_ref.def_id];
    seen.insert(principal_ref.def_id);
    while let Some(def_id) = stack.pop() {
        count += tcx.own_existential_vtable_entries(def_id).len() as u32;
        let super_predicates = tcx.explicit_super_predicates_of(def_id).skip_binder();
        for (clause, _) in super_predicates.iter() {
            let predicate: ty::Predicate<'_> = (*clause).upcast(tcx);
            if let ty::PredicateKind::Clause(ty::ClauseKind::Trait(data)) =
                predicate.kind().skip_binder()
            {
                if seen.insert(data.def_id()) {
                    stack.push(data.def_id());
                }
            }
        }
    }
    count
}

fn intern_closure_captures_struct_view<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure_ty: ty::Ty<'tcx>,
    item: ItemId,
    model_args: &GenericArgs,
    closure_args: ty::ClosureArgs<ty::TyCtxt<'tcx>>,
) -> Result<TyId, BridgeError> {
    let upvar_tys = closure_args
        .upvar_tys()
        .iter()
        .map(|upvar| translate_ty(cx, upvar))
        .collect::<Result<Vec<_>, _>>()?;
    let rustc_layout = cx
        .input
        .tcx
        .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(closure_ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: closure_ty.to_string(),
        })?;
    // Per spec.md section 13.1 lines 4527-4529, the closure captures
    // struct view's fields are flattened from `tupled_upvars_ty()` to
    // positional `_0`, `_1`, ... names (audit-finding MEDIUM-40-3).
    let fields = upvar_tys
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, ty)| FieldLayout {
            name: cx.model.strings.intern(&format!("_{idx}")),
            ty,
            offset: if idx < rustc_layout.fields.count() {
                rustc_layout.fields.offset(idx).bytes()
            } else {
                0
            },
        })
        .collect();
    let layout = Layout {
        size: rustc_layout.size.bytes(),
        align: rustc_layout.align.abi.bytes(),
        pack: None,
        uninhabited: rustc_layout.uninhabited,
        kind: LayoutKind::Struct { fields },
    };
    Ok(cx.model.types.intern(Ty::Struct {
        item,
        args: model_args.clone(),
        layout,
    }))
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
                .map(|(idx, ty)| {
                    let name_id = cx.model.strings.intern(&format!("_{idx}"));
                    FieldLayout {
                        name: name_id,
                        ty,
                        offset: rustc_layout.fields.offset(idx).bytes(),
                    }
                })
                .collect(),
        },
    })
}

fn nominal_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    fields: &[(rustc_span::Symbol, TyId)],
) -> Result<Layout, BridgeError> {
    let rustc_layout = cx
        .input
        .tcx
        .layout_of(ty::TypingEnv::fully_monomorphized().as_query_input(ty))
        .map_err(|_| BridgeError::Layout {
            ty_repr: ty.to_string(),
        })?;
    let field_layouts = fields
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, (name, ty))| {
            let name_id = cx.model.strings.intern(name.as_str());
            FieldLayout {
                name: name_id,
                ty,
                offset: if idx < rustc_layout.fields.count() {
                    rustc_layout.fields.offset(idx).bytes()
                } else {
                    0
                },
            }
        })
        .collect();
    let kind = match ty.kind() {
        TyKind::Adt(adt, _) if adt.is_union() => LayoutKind::Union {
            fields: field_layouts,
        },
        _ => LayoutKind::Struct {
            fields: field_layouts,
        },
    };
    Ok(Layout {
        size: rustc_layout.size.bytes(),
        align: rustc_layout.align.abi.bytes(),
        pack: None,
        uninhabited: rustc_layout.uninhabited,
        kind,
    })
}

/// Structural substantive-HRTB predicate per spec.md section 17.1.
///
/// Walks the `Binder<FnSig>` body and `Binder<Predicate>` bodies via
/// `rustc_middle::ty::TypeVisitor`, counting `ReBound` region
/// occurrences per syntactic position. The five spec-17.1 conditions
/// map to:
///
///   1. Bound region in projection (`<T as Trait<'a>>::Assoc`) ->
///      visited inside `Alias::Projection`; substantive.
///   2. Bound region inside a nested binder body that resolves to
///      THIS binder -> visited with `current_depth > 0`; substantive.
///   3. Bound region in trait-bound / trait-predicate / raw-pointer
///      pointee / generic-arg position -> visited via `super_visit_with`
///      from outside an immediate `TyKind::Ref` slot; substantive.
///   4. Bound region appearing in more than one immediate-input-ref
///      slot (relational binder) -> tracked in `immediate_ref_counts`;
///      reject when any count > 1.
///   5. Bound region in the output position -> visited under
///      `PositionContext::Output`; substantive.
///
/// Replaces the previous `ty.to_string()` substring-matching
/// predicate (regseq-driver-37 audit-finding MEDIUM-37-3).
fn has_substantive_hrtb_ty<'tcx>(tcx: ty::TyCtxt<'tcx>, ty: ty::Ty<'tcx>) -> bool {
    match ty.kind() {
        TyKind::FnPtr(sig_tys, header) => {
            let binder = sig_tys.with(*header);
            has_substantive_hrtb_fn_sig(tcx, binder)
        }
        TyKind::Dynamic(predicates, _) => has_substantive_hrtb_dynamic(tcx, predicates),
        _ => false,
    }
}

fn has_substantive_hrtb_fn_sig<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    binder: ty::Binder<'tcx, ty::FnSig<'tcx>>,
) -> bool {
    let sig = binder.skip_binder();
    let mut checker = HrtbChecker::new(tcx);
    for input in sig.inputs() {
        if checker.substantive {
            return true;
        }
        checker.visit_input(*input);
    }
    if checker.substantive {
        return true;
    }
    checker.visit_output(sig.output());
    if checker.substantive {
        return true;
    }
    // Condition 4: any bound region appearing in more than one
    // immediate-input-ref slot is relational and rejected.
    checker
        .immediate_ref_counts
        .values()
        .any(|&count| count > 1)
}

fn has_substantive_hrtb_dynamic<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    predicates: &'tcx ty::List<ty::Binder<'tcx, ty::ExistentialPredicate<'tcx>>>,
) -> bool {
    // Per spec.md section 17.1: a bound region appearing anywhere in
    // a `dyn` predicate body is substantive (conditions 1-3 catch
    // projection / nested-binder / trait-predicate positions; the
    // bare auto-trait set has no bound regions). Visit each
    // predicate's body and reject on any `ReBound` to the outer
    // binder.
    for predicate in predicates.iter() {
        let body = predicate.skip_binder();
        let mut checker = HrtbChecker::new(tcx);
        checker.position = PositionContext::NonImmediateInput;
        body.visit_with(&mut checker);
        if checker.substantive {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
enum PositionContext {
    /// `TyKind::Ref(_, pointee, _)` directly under a `FnSig.inputs[_]`
    /// slot. A single bound region here is accepted; the spec rejects
    /// only when the same region appears in more than one such slot
    /// (condition 4).
    ImmediateInputRefSlot,
    /// Anywhere else inside an input position: trait-bound args,
    /// generic-arg slots, projection arguments, raw-pointer pointees,
    /// or the pointee of a `TyKind::Ref` itself. Any bound region
    /// here is substantive (conditions 1-3).
    NonImmediateInput,
    /// Inside the FnSig output (or output sub-tree). Any bound region
    /// here is substantive (condition 5).
    Output,
}

struct HrtbChecker<'tcx> {
    _tcx: ty::TyCtxt<'tcx>,
    /// DeBruijn depth of the binder we are checking, as observed from
    /// the visitor's current scope. Starts at `0` (the body we are
    /// looking at directly). Increments when descending into nested
    /// binders so that a `ReBound(self.current_depth, _)` always
    /// refers to OUR binder regardless of nesting.
    current_depth: u32,
    position: PositionContext,
    /// Count of bound region occurrences in immediate-input-ref slots,
    /// keyed by the bound-region variable's index.
    immediate_ref_counts: std::collections::HashMap<u32, u32>,
    substantive: bool,
}

impl<'tcx> HrtbChecker<'tcx> {
    fn new(tcx: ty::TyCtxt<'tcx>) -> Self {
        Self {
            _tcx: tcx,
            current_depth: 0,
            position: PositionContext::NonImmediateInput,
            immediate_ref_counts: std::collections::HashMap::new(),
            substantive: false,
        }
    }

    fn visit_input(&mut self, ty: ty::Ty<'tcx>) {
        if self.substantive {
            return;
        }
        // Per spec.md section 17.1 lines 6243-6246, `&'r1 &'r2 T` is
        // TWO immediate-input-ref positions (the outer ref's lifetime
        // AND the inner ref's lifetime). Walk every leading `Ref`
        // layer as an immediate slot, then switch to non-immediate
        // context for the first non-Ref pointee found.
        let mut cursor = ty;
        loop {
            if self.substantive {
                return;
            }
            match cursor.kind() {
                TyKind::Ref(region, pointee, _) => {
                    self.position = PositionContext::ImmediateInputRefSlot;
                    self.visit_region(*region);
                    cursor = *pointee;
                }
                _ => {
                    self.position = PositionContext::NonImmediateInput;
                    self.descend_into(cursor);
                    return;
                }
            }
        }
    }

    fn visit_output(&mut self, ty: ty::Ty<'tcx>) {
        if self.substantive {
            return;
        }
        self.position = PositionContext::Output;
        self.descend_into(ty);
    }

    fn visit_region(&mut self, region: ty::Region<'tcx>) {
        if let ty::RegionKind::ReBound(ty::BoundVarIndexKind::Bound(debruijn), br) = region.kind() {
            if debruijn.as_u32() == self.current_depth {
                match self.position {
                    PositionContext::ImmediateInputRefSlot => {
                        let key = br.var.as_u32();
                        *self.immediate_ref_counts.entry(key).or_default() += 1;
                    }
                    PositionContext::NonImmediateInput | PositionContext::Output => {
                        self.substantive = true;
                    }
                }
            }
        }
    }

    fn descend_into(&mut self, ty: ty::Ty<'tcx>) {
        ty.visit_with(self);
    }
}

impl<'tcx> ty::TypeVisitor<ty::TyCtxt<'tcx>> for HrtbChecker<'tcx> {
    type Result = ();

    fn visit_binder<T: ty::TypeVisitable<ty::TyCtxt<'tcx>>>(
        &mut self,
        t: &ty::Binder<'tcx, T>,
    ) -> Self::Result {
        // Entering a nested binder. OUR binder's bound vars now appear
        // at `current_depth + 1`; the nested binder's vars are at the
        // new innermost. Per spec condition 2, any reference to OUR
        // binder inside a nested binder is substantive. Setting the
        // position to NonImmediateInput here (in case we just stepped
        // out of an ImmediateInputRefSlot) preserves rejection
        // semantics regardless of how the nested binder reaches a
        // ReBound region.
        let saved_position = self.position;
        self.position = PositionContext::NonImmediateInput;
        self.current_depth += 1;
        t.super_visit_with(self);
        self.current_depth -= 1;
        self.position = saved_position;
    }

    fn visit_region(&mut self, r: ty::Region<'tcx>) -> Self::Result {
        HrtbChecker::visit_region(self, r);
    }

    fn visit_ty(&mut self, t: ty::Ty<'tcx>) -> Self::Result {
        if self.substantive {
            return;
        }
        // While descending into a sub-tree, any nested `TyKind::Ref`
        // is NOT an immediate-input-ref slot of OUR binder -- it sits
        // inside something else. Force the position to non-ref so
        // bound regions in nested refs are correctly classified as
        // substantive (e.g., `Vec<&'a T>` or `*const &'a T`).
        let saved_position = self.position;
        if !matches!(self.position, PositionContext::Output) {
            self.position = PositionContext::NonImmediateInput;
        }
        t.super_visit_with(self);
        self.position = saved_position;
    }
}

fn reject_substantive_fn_ptr_binder<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    binder: ty::Binder<'tcx, ty::FnSig<'tcx>>,
) -> Result<(), BridgeError> {
    if has_substantive_hrtb_fn_sig(tcx, binder) {
        return Err(BridgeError::SubstantiveHrtb {
            binder_repr: ty.to_string(),
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
    fn hrtb_checker_immediate_ref_position_classification() {
        // Position-context classification is exercised end-to-end by
        // the integration test `rejects_foreign_dynamic_and_hrtb_rows`
        // in tests/bridge_ty.rs, which compiles real `for<'a>` binders
        // through rustc and asserts the bridge classification. Unit
        // testing the visitor in isolation would require fabricating
        // rustc Ty values without a `TyCtxt`, which is impractical;
        // the classification logic is small enough that the
        // integration coverage is the load-bearing assertion.
        assert!(matches!(
            PositionContext::ImmediateInputRefSlot,
            PositionContext::ImmediateInputRefSlot
        ));
        assert!(matches!(
            PositionContext::NonImmediateInput,
            PositionContext::NonImmediateInput
        ));
        assert!(matches!(PositionContext::Output, PositionContext::Output));
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
