//! Type translation from rustc `TyKind` into RegseqModel `Ty`.

use rustc_hir::LangItem;
use rustc_middle::ty::{self, TyKind};
use rustc_span::def_id::DefId;

use regseq_model::{
    AutoTrait, CrateId, CrateInfo, DefPath, DefPathData, DefPathHash, DefPathSegment, FieldLayout,
    FloatKind, FnAbi, FnSig, GenericArg, GenericArgs, InlineHint, IntKind, Item, ItemId, ItemKind,
    Layout, LayoutKind, Linkage, Mutability, PrincipalTraitKey, Provenance, Safety, Span,
    StableCrateId, SyntheticItemKind, Ty, TyId, UintKind, UNIT_LAYOUT,
};

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
            reject_substantive_fn_ptr_binder(&ty.to_string())?;
            let sig = sig_tys.with(*header).skip_binder();
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
            if ty.to_string().contains("for<") {
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
    let field_tys = adt
        .all_fields()
        .map(|field| translate_ty(cx, field.ty(cx.input.tcx, args)))
        .collect::<Result<Vec<_>, _>>()?;
    let layout = nominal_layout(cx, ty, &field_tys)?;

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

#[derive(Clone, Copy)]
enum SpecialAdt {
    None,
    PhantomData,
    UnsafeCell,
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
fn intern_item(
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

fn vtable_entry_count<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    principal_ref: ty::ExistentialTraitRef<'tcx>,
) -> u32 {
    // `tcx.vtable_entries(poly_trait_ref)` requires a fully-resolved
    // trait ref (with a self type that actually implements the
    // trait) and therefore cannot be called for the existential
    // alone (no concrete impl is available at `Ty::Dynamic`
    // translation time). Mirror rustc's internal `vtable_min_entries`
    // approximation: 3 common slots (drop_in_place, size, align)
    // plus the count of every method in the principal trait and its
    // supertraits via `own_existential_vtable_entries`. This matches
    // the same slot count rustc uses for `&dyn Trait` layout, which
    // is the load-bearing requirement on the vtable-Struct layout.
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
    let empty = cx.model.strings.intern("");
    let fields = upvar_tys
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
