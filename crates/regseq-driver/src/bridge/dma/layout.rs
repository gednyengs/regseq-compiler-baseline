//! The `DmaPod` layout predicate + the C03 layout verify rule (dma-management
//! construct, M4/T19; GRILL C03/D08).
//!
//! Two halves, split by where the rustc `TyCtxt` is in hand (the M4 division
//! Codex confirmed):
//!
//!   * [`dma_pod_reason`] — the REAL repr(C)-POD predicate over a rustc `Ty`. A
//!     coherent DMA buffer must hold ALL-BIT-PATTERNS-VALID, fixed-layout bytes
//!     (the `bytemuck::Pod` distinction): a DMA device writes ARBITRARY bytes into
//!     the buffer, so the only sound view is one for which every byte pattern is a
//!     valid value. So `DmaPod` admits only: INTEGER/FLOAT scalar leaves (every bit
//!     pattern valid, NaNs included); arrays (recurse on the element); `#[repr(C)]`
//!     / `#[repr(transparent)]` structs and unions whose every field is itself
//!     `DmaPod` (a `PhantomData`/ZST marker is allowed). It REJECTS: any `Drop`
//!     glue; `bool` (only 0x00/0x01 valid) and `char` (must be a valid Unicode
//!     scalar) — validity invariants a device byte can violate (BLOCKING-1); ALL
//!     enums (even a fieldless `#[repr(int)]` one — only its declared discriminants
//!     are valid); references `&T`, raw pointers `*T`, and function pointers (a host
//!     pointer is never a device-visible descriptor field — descriptors carry
//!     integer bus addresses); DSTs (`[T]`/`str`/`dyn`); unresolved generics
//!     (`Ty::Param`/`Ty::Alias`/infer/bound/placeholder/error); tuples; and
//!     Rust-layout (non-`repr(C)`) aggregates whose field order/padding the ABI does
//!     not pin. The M1 `is_pod = !needs_drop` was a conservative PROXY; `!needs_drop`
//!     is necessary but NOT sufficient (a `bool`/enum field, a `#[repr(Rust)]` POD
//!     struct, or a struct with a `&T` field has no Drop glue yet is not a sound DMA
//!     view).
//!
//!   * [`verify_dma_layout`] — the C03 clause verifier (a `BridgeCtxt` pass with
//!     `tcx` in hand). For each recognized region it checks the DECLARATION (over
//!     `T`) and every per-`element_access` VIEW (over `V`, the turbofish reinterpret
//!     or `T` by default): `V: Sized + DmaPod`, `size_of::<V>() <= element_size`,
//!     `align_of::<V>() <= region_alignment`, `element_size % align_of::<V>() == 0`,
//!     and `i in [0, element_count)` where the index is a static const. Each
//!     violation is a compile-fail naming the violated clause (the `DmaDiagnostic`
//!     family). `region_alignment = alignment?` if given, else `align_of::<T>()`
//!     (NOT 1, GRILL D08). A `V` strictly smaller than `element_size` is allowed
//!     (the borrow exposes only `size_of::<V>()` bytes).
//!
//! Runs in `materialise_bodies` after `recognise_dma_region_sites` (the per-access
//! view facts exist) — see `bridge::mod`.

use regseq_model::{
    CallCallee, DmaOp, InstanceKind, IntrinsicKind, ItemId, StringId, TerminatorKind, TyId,
};
use rustc_middle::ty::{Ty, TyCtxt, TyKind, TypingEnv};

use super::super::{BridgeCtxt, BridgeError, DmaDiagnostic};

/// The set of rustc `Ty`s currently being inspected, to break the recursion on a
/// self-referential ADT (which a non-`Box`/non-pointer DMA descriptor cannot
/// legally form, but the guard keeps the predicate total).
type Seen<'tcx> = Vec<Ty<'tcx>>;

/// The reason a type is NOT `DmaPod`, or `None` when it IS `DmaPod`. The reason
/// string names the violated POD sub-clause so the C03 diagnostic can cite it
/// (GRILL C03: "a diagnostic naming the violated clause"). A bare bool could only
/// say "not DmaPod" — the Codex-recommended `pod_reason` carries which rule failed.
pub(crate) fn dma_pod_reason<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<String> {
    let typing_env = TypingEnv::fully_monomorphized();
    // Hardening (NON-BLOCKING-1): reject a non-monomorphic top-level type
    // (`Ty::Param`/`Ty::Alias`/infer/bound/placeholder/error) BEFORE the
    // `needs_drop` query — `needs_drop` under a `fully_monomorphized` env would
    // ICE/misbehave on a stray free parameter. A resolved `V` never hits this; a
    // genuinely-unresolved view is cleanly rejected as an unresolved generic. The
    // recursive walk applies the same guard to every reached field.
    if let Some(reason) = unresolved_generic_reason(ty) {
        return Some(reason);
    }
    // A type with Drop glue is never safe to memcpy through a coherent backing
    // store (it owns a resource the bitwise copy would alias/double-free). This is
    // the necessary-but-not-sufficient floor the M1 proxy used alone. (Now reached
    // only for a monomorphic type, after the unresolved-generic guard above.)
    if ty.needs_drop(tcx, typing_env) {
        return Some("has a `Drop` impl (a DMA view must be trivially copyable)".to_string());
    }
    let mut seen = Seen::new();
    pod_reason_rec(tcx, typing_env, ty, &mut seen)
}

/// The "unresolved generic" rejection reason for a non-monomorphic type kind
/// (`Param`/`Alias`/infer/bound/placeholder/error), or `None` for a concrete kind.
/// Pulled out so `dma_pod_reason` can apply it BEFORE the `needs_drop` query
/// (NON-BLOCKING-1 hardening) and `pod_reason_rec` can reuse it per field.
fn unresolved_generic_reason(ty: Ty<'_>) -> Option<String> {
    match ty.kind() {
        TyKind::Param(_)
        | TyKind::Alias(..)
        | TyKind::Bound(..)
        | TyKind::Placeholder(_)
        | TyKind::Infer(_)
        | TyKind::Error(_) => Some(
            "is an unresolved generic type (a DMA view must be a concrete monomorphic type)"
                .to_string(),
        ),
        _ => None,
    }
}

/// The recursive POD walk. Returns the first violated sub-clause, or `None` when
/// every reachable leaf/field is `DmaPod`.
fn pod_reason_rec<'tcx>(
    tcx: TyCtxt<'tcx>,
    typing_env: TypingEnv<'tcx>,
    ty: Ty<'tcx>,
    seen: &mut Seen<'tcx>,
) -> Option<String> {
    if seen.contains(&ty) {
        return None; // already being inspected up the stack; not a fresh violation
    }
    match ty.kind() {
        // Scalar leaves: ONLY integer/float types, for which EVERY bit pattern is a
        // valid value (BLOCKING-1, the `bytemuck::Pod` distinction). A DMA device
        // writes ARBITRARY bytes into the coherent buffer; reinterpreting a
        // device-written byte through a type with a validity invariant is UB. Floats
        // are included (every bit pattern is a valid `f32`/`f64`, incl. NaNs).
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) => None,
        // `bool` (only 0x00/0x01 valid) and `char` (must be a valid Unicode scalar)
        // have validity invariants a device-written byte can violate — REJECT.
        TyKind::Bool => Some(
            "is `bool` (only 0x00/0x01 are valid; a DMA device could write any byte — use an \
             integer type)"
                .to_string(),
        ),
        TyKind::Char => Some(
            "is `char` (must be a valid Unicode scalar; a DMA device could write any 4 bytes — \
             use `u32`)"
                .to_string(),
        ),
        // Arrays: a fixed-count run of a `DmaPod` element. Recurse on the element
        // (a zero-length array is a ZST and trivially fine).
        TyKind::Array(elem, _len) => {
            seen.push(ty);
            let r = pod_reason_rec(tcx, typing_env, *elem, seen);
            seen.pop();
            r
        }
        // References, raw pointers, and function pointers are NOT device-visible
        // descriptor fields — a coherent descriptor carries an INTEGER bus/physical
        // address, never a host virtual pointer. Reject them outright.
        TyKind::Ref(..) => {
            Some("contains a reference `&T` (a DMA view holds no host references; use an integer bus address)".to_string())
        }
        TyKind::RawPtr(..) => {
            Some("contains a raw pointer `*T` (a DMA view holds no host pointers; use an integer bus address)".to_string())
        }
        TyKind::FnPtr(..) => {
            Some("contains a function pointer (not a device-visible descriptor field)".to_string())
        }
        // DSTs have no statically-known size — they cannot be a `Sized` DMA view.
        TyKind::Slice(_) => Some("is a slice `[T]` (a DST; a DMA view must be `Sized`)".to_string()),
        TyKind::Str => Some("is `str` (a DST; a DMA view must be `Sized`)".to_string()),
        TyKind::Dynamic(..) => {
            Some("is a `dyn` trait object (a DST; a DMA view must be a concrete `Sized` type)".to_string())
        }
        // Unresolved generics (`Param`/`Alias`/infer/bound/placeholder/error) never
        // reach a monomorphic layout; the verify rule requires a fully-resolved `V`.
        TyKind::Param(_)
        | TyKind::Alias(..)
        | TyKind::Bound(..)
        | TyKind::Placeholder(_)
        | TyKind::Infer(_)
        | TyKind::Error(_) => unresolved_generic_reason(ty),
        TyKind::Adt(adt, args) => {
            // A `PhantomData<T>` marker is a ZST that carries no bytes — admit it
            // without recursing into `T` (recursing would false-reject a legitimate
            // marker field whose `T` is e.g. a lifetime-carrying non-POD).
            if adt.is_phantom_data() {
                return None;
            }
            // A coherent descriptor aggregate must pin its field layout: `repr(C)`
            // or `repr(transparent)`. A `repr(Rust)` struct's field order/padding is
            // unspecified, so the device and the CPU would disagree on byte offsets.
            let repr = adt.repr();
            // ALL enums are rejected (BLOCKING-1). Even a fieldless `#[repr(int)]`
            // enum has a VALIDITY INVARIANT — only its declared discriminants are
            // valid values. A DMA device can write any byte into the slot, so
            // reinterpreting a device-written byte as such an enum is UB (an
            // undeclared discriminant). Use the underlying integer type instead.
            if adt.is_enum() {
                return Some(
                    "is an enum (only its declared discriminants are valid; a DMA device could \
                     write any byte — use the underlying integer type)"
                        .to_string(),
                );
            }
            if !repr.c() && !repr.transparent() {
                return Some(format!(
                    "aggregate `{}` is not `#[repr(C)]`/`#[repr(transparent)]` (its field layout is unpinned)",
                    tcx.item_name(adt.did())
                ));
            }
            // Recurse into every field of every variant (a struct/union has one
            // variant). A field whose type is not `DmaPod` taints the aggregate.
            seen.push(ty);
            for variant in adt.variants() {
                for field in &variant.fields {
                    let field_ty = field.ty(tcx, args);
                    if let Some(reason) = pod_reason_rec(tcx, typing_env, field_ty, seen) {
                        seen.pop();
                        return Some(format!(
                            "field `{}` of `{}` {}",
                            field.name,
                            tcx.item_name(adt.did()),
                            reason
                        ));
                    }
                }
            }
            seen.pop();
            None
        }
        // Tuples: Rust-layout aggregates with no `repr(C)` guarantee — reject (a
        // descriptor should be a named `#[repr(C)]` struct, not a tuple).
        TyKind::Tuple(elems) if elems.is_empty() => None, // the unit `()` ZST is fine
        TyKind::Tuple(_) => {
            Some("is a tuple (no `#[repr(C)]` layout guarantee; use a `#[repr(C)]` struct)".to_string())
        }
        // Anything else (closures, generators, never, foreign, …) is not a DMA view.
        _ => Some("is not a fixed-layout POD type".to_string()),
    }
}

/// The C03 layout verify pass (M4/T19). Runs after `recognise_dma_region_sites`
/// recorded the per-access view facts, with the `BridgeCtxt`'s `tcx` in hand so
/// the `DmaPod` predicate over a view `V` reads the real rustc `Ty` (the M1
/// `view_layout.is_pod` placeholder is a conservative stand-in, not the authority).
///
/// For each recognized region:
///   * DECLARATION (over `T`): `T: Sized + DmaPod`, `size_of::<T>() <= element_size`,
///     `align_of::<T>() <= region_alignment`, `element_size % align_of::<T>() == 0`.
///     (`materialise_dma_regions` already runs the size/align/stride numerics over
///     `T`'s resolved layout; this pass adds the `DmaPod` clause over `T`.)
///   * each `element_access` VIEW (over `V` = the turbofish reinterpret, or `T`):
///     `V: Sized + DmaPod`, `size_of::<V>() <= element_size`,
///     `align_of::<V>() <= region_alignment`, `element_size % align_of::<V>() == 0`,
///     and `i in [0, element_count)` where the index is a static const.
///
/// `region_alignment = alignment?` if given, else `align_of::<T>()` (NOT 1, D08).
pub(crate) fn verify_dma_layout(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let typing_env = TypingEnv::fully_monomorphized();

    // The declared element rustc `Ty` for each region id (recovered from the region
    // field's `DmaRegion<T>` type). Resolved up-front so a per-site `V = T` default
    // (no turbofish) reads it, and the declaration POD clause runs over it. Every
    // clause runs within this single `tcx`-scoped pass; no rustc `Ty` is persisted.
    let region_decl_ty = collect_region_decl_tys(cx);

    // Region declarations: the `DmaPod` clause over `T` (the size/align/stride were
    // checked in `materialise_dma_regions`; this adds POD over the declared T).
    for tables in cx.model.dma.drivers.values() {
        for region in &tables.regions {
            let region_str = cx.model.strings.resolve(region.id).to_string();
            // FAIL-CLOSED (BLOCKING-3): a RECOGNIZED region whose declared `T` cannot
            // be recovered as a rustc `Ty` is a REJECT, not a silent skip — for a
            // soundness check, "could not verify" must not pass. The E1000/PL011
            // fixtures' declared types are concrete repr(C)/scalar types that recover
            // cleanly, so failing closed causes no false reject.
            let Some(t) = region_decl_ty.get(&region.id).copied() else {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::LayoutVerify {
                    region: region_str.clone(),
                    clause: "V: DmaPod (declaration `T`)",
                    detail: "the declared element type `T` could not be resolved to a concrete \
                             type — the layout verify rule fails closed (cannot prove DmaPod)"
                        .to_string(),
                }));
            };
            if let Some(reason) = dma_pod_reason(tcx, t) {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::LayoutVerify {
                    region: region_str.clone(),
                    clause: "V: DmaPod (declaration `T`)",
                    detail: format!("the declared element type `T` {reason}"),
                }));
            }
        }
    }

    // Per-access view sites: collect (region id, view rustc Ty, index, the region's
    // element_size/element_count/region_alignment) from the model + the body walk,
    // then run the per-view clauses.
    //
    // The view rustc `Ty` is read off the `element_access` closure argument's
    // rust-call signature `fn((&[mut] V,))` (NOT a positional generic arg — see
    // `element_access_view_rustc`); a `V = T` default (no reinterpret) falls back to
    // the declared `T`.
    let mut violations: Vec<(String, &'static str, String)> = Vec::new();
    for instance in cx.model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            let Some(IntrinsicKind::Dma(DmaOp::ElementAccess { .. })) =
                cx.model.instance_intrinsic(*callee_inst)
            else {
                continue;
            };
            // Recover the region this access targets (the receiver field marker).
            let Some(recv) = args.first() else {
                continue;
            };
            let Some((_driver, region_id)) =
                super::recognition::recover_region_for_layout(&cx.model, body, recv)
            else {
                continue;
            };
            // The region's declared constants (element_size/count) + region_alignment.
            let Some((element_size, element_count, region_alignment, decl_t)) =
                region_constants(cx, region_id, &region_decl_ty)
            else {
                continue;
            };
            let region_str = cx.model.strings.resolve(region_id).to_string();
            // The view `V`'s rustc `Ty`, read off the closure argument's rust-call
            // signature `fn((&[mut] V,))` (NOT a positional generic arg — see
            // `element_access_view_rustc`). When the closure sig is unrecoverable the
            // view IS the declared `T` (the V=T default with no reinterpret).
            let view_ty = element_access_view_rustc(cx, *callee_inst).or(decl_t);

            // FAIL-CLOSED (BLOCKING-3): an `element_access` site whose view type
            // cannot be resolved (neither the reinterpret `V` nor the `decl_t`
            // fallback) is a REJECT — the POD/size/align clauses cannot be proved, so
            // the soundness check fails closed. The E1000 fixture's `write::<T>` view
            // resolves cleanly (V via the closure sig, T via the region field), so no
            // false reject.
            let Some(v) = view_ty else {
                violations.push((
                    region_str.clone(),
                    "V: DmaPod (access view)",
                    "the access view type `V` could not be resolved to a concrete type — the \
                     layout verify rule fails closed (cannot prove DmaPod)"
                        .to_string(),
                ));
                continue;
            };

            // The static index `i` (args[1]). BLOCKING-2: an element_access index
            // that is PRESENT but NOT a recoverable const is a REJECT — generic-c
            // emits `base + i*element_size` runtime pointer arithmetic with NO runtime
            // bounds guard, so a non-const index cannot be statically bounded. This
            // mirrors the M3 "reject non-const span ranges this cycle" precedent. A
            // MISSING index arg (args[1] absent) is treated as unresolved too (an
            // element_access always carries an index; an absent one is a malformed
            // shape that also fails closed).
            match args.get(1) {
                None => {
                    violations.push((
                        region_str.clone(),
                        "i in [0, element_count)",
                        "the element index operand is missing — element access requires a \
                         statically-known index this cycle"
                            .to_string(),
                    ));
                    continue;
                }
                Some(op) => match super::recognition::const_uint_for_layout(&cx.model, op) {
                    Some(i) => {
                        if element_count != 0 && i >= element_count {
                            violations.push((
                                region_str.clone(),
                                "i in [0, element_count)",
                                format!(
                                    "static index {i} is out of bounds for element_count = \
                                     {element_count}"
                                ),
                            ));
                            continue;
                        }
                    }
                    None => {
                        violations.push((
                            region_str.clone(),
                            "i in [0, element_count)",
                            "a non-const element index is not supported this cycle — coherent \
                             element access requires a statically-known index (no runtime bounds \
                             guard is emitted)"
                                .to_string(),
                        ));
                        continue;
                    }
                },
            }

            {
                // Clause: V: Sized + DmaPod.
                if let Some(reason) = dma_pod_reason(tcx, v) {
                    violations.push((
                        region_str.clone(),
                        "V: DmaPod (access view)",
                        format!("the access view type `V` {reason}"),
                    ));
                    continue;
                }
                // Clause: size_of::<V>() <= element_size; align_of::<V>() <=
                // region_alignment; element_size % align_of::<V>() == 0.
                if let Ok(layout) = tcx.layout_of(typing_env.as_query_input(v)) {
                    let vsize = layout.size.bytes();
                    let valign = layout.align.abi.bytes();
                    if vsize > element_size {
                        violations.push((
                            region_str.clone(),
                            "size_of::<V>() <= element_size",
                            format!(
                                "size_of::<V>() = {vsize} exceeds element_size = {element_size} \
                                 (the reinterpret view would read past the slot)"
                            ),
                        ));
                        continue;
                    }
                    if valign > region_alignment {
                        violations.push((
                            region_str.clone(),
                            "align_of::<V>() <= region_alignment",
                            format!(
                                "align_of::<V>() = {valign} exceeds region_alignment = {region_alignment} \
                                 (the slot base is not guaranteed V-aligned)"
                            ),
                        ));
                        continue;
                    }
                    if valign != 0 && element_size % valign != 0 {
                        violations.push((
                            region_str.clone(),
                            "element_size % align_of::<V>() == 0",
                            format!(
                                "element_size = {element_size} is not a multiple of align_of::<V>() = {valign} \
                                 (slot base + i*element_size would not be V-aligned)"
                            ),
                        ));
                        continue;
                    }
                }
            }
        }
    }
    if let Some((region, clause, detail)) = violations.into_iter().next() {
        return Err(BridgeError::DmaValidation(DmaDiagnostic::LayoutVerify {
            region,
            clause,
            detail,
        }));
    }
    Ok(())
}

/// The region's `(element_size, element_count, region_alignment, declared rustc T)`
/// for a region id. `region_alignment = alignment?` else `align_of::<T>()` (NOT 1,
/// D08) — read off the stored region facts (the bridge already recovered the
/// default alignment into `DmaRegionFacts.alignment`).
fn region_constants<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    region_id: StringId,
    decl_tys: &std::collections::HashMap<StringId, Ty<'tcx>>,
) -> Option<(u64, u64, u64, Option<Ty<'tcx>>)> {
    for tables in cx.model.dma.drivers.values() {
        if let Some(region) = tables.regions.iter().find(|r| r.id == region_id) {
            // `region_alignment`: the stored `alignment` (which the bridge already
            // defaulted to `align_of::<T>()` when omitted), else the element layout's
            // align, else 1 only as a last resort (T unresolved).
            let region_alignment = region
                .alignment
                .or_else(|| region.element_layout.map(|l| l.align))
                .unwrap_or(1)
                .max(1);
            let decl_t = decl_tys.get(&region_id).copied();
            return Some((
                region.element_size,
                region.element_count,
                region_alignment,
                decl_t,
            ));
        }
    }
    None
}

/// Resolve each region's declared element rustc `Ty` (`DmaRegion<T>`'s `T`) by
/// walking the driver structs' `DmaRegion<T>` fields, mirroring
/// `materialise_dma_regions`' field walk but reading the rustc element type for
/// the POD predicate. Keyed by the region resource id.
fn collect_region_decl_tys<'tcx>(
    cx: &BridgeCtxt<'tcx>,
) -> std::collections::HashMap<StringId, Ty<'tcx>> {
    use rustc_span::Symbol as RustcSymbol;
    let tcx = cx.input.tcx;
    let mut out = std::collections::HashMap::new();
    // The region ids already interned by `materialise_dma_regions`, paired with
    // their resolved strings, so a parsed attr id can be matched to its `StringId`
    // WITHOUT mutating the string table (this pass is read-only over the model).
    let interned_ids: Vec<(String, StringId)> = cx
        .model
        .dma
        .drivers
        .values()
        .flat_map(|d| {
            d.regions
                .iter()
                .map(|r| (cx.model.strings.resolve(r.id).to_string(), r.id))
        })
        .collect();
    // The driver-struct DefIds, mirroring the region materialisation walk root.
    let drivers: Vec<(ItemId, rustc_span::def_id::DefId)> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item_id| {
            let def_id = super::super::framework_type::recover_source_def_id(
                tcx,
                cx.model.item_def_paths[item_id],
            )?;
            Some((item_id, def_id))
        })
        .collect();
    let region_path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("dma_region")];
    for (_item_id, def_id) in drivers {
        let adt = tcx.adt_def(def_id);
        for variant in adt.variants() {
            for field in &variant.fields {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(t) = super::recognition::dma_region_element_ty_pub(tcx, field_ty) else {
                    continue;
                };
                // Read the field's `#[rseq::dma_region(id = "…")]` id to key by it.
                let Some(attr) = tcx.get_attrs_by_path(field.did, &region_path).next() else {
                    continue;
                };
                if let Some(id) = super::recognition::dma_region_attr_id(attr) {
                    if let Some((_, sid)) = interned_ids.iter().find(|(s, _)| *s == id) {
                        out.insert(*sid, t);
                    }
                }
            }
        }
    }
    out
}

/// The element-access view type `V` of a `region.<verb>::<V>(i, |&[mut] V| …)`
/// call, as a rustc `Ty`. `None` when `V` is unrecoverable; the caller then falls
/// back to the declared `T` (`decl_t`), and if THAT is also `None` the access fails
/// closed (a DMA-ERR-LAYOUT-VERIFY reject, BLOCKING-3 — a soundness check must not
/// silently pass a view it cannot analyze).
///
/// `V` is recovered from the **closure argument's rust-call signature**, NOT from a
/// positional generic arg: a `DmaRegion::<T>::write::<V, F>` monomorphic instance
/// carries `generic_args = [T (region), V (method), F = the closure]`, and the
/// closure `F` reifies as `extern "rust-call" fn((&[mut] V,))`. Reading the closure
/// sig's single tuple input and peeling the `&[mut]` yields `V` unambiguously —
/// whereas the first type arg is the region's `T` and the last is the closure `F`,
/// so neither position is `V`. (This is the same closure-reified-sig shape the T13
/// HRTB carve-out matches; the verify rule reads `V` off it for C03.)
fn element_access_view_rustc<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    callee: regseq_model::InstanceId,
) -> Option<Ty<'tcx>> {
    use rustc_middle::ty::TyKind;
    // Walk the callee's type generic args; the closure arg is the one whose rustc
    // `Ty` is a `Closure`. Resolve each model `TyId` to its rustc `Ty` (cache, else
    // reconstruct), and read `V` off the closure's rust-call signature.
    for arg in &cx.model.instances[callee].generic_args {
        let regseq_model::GenericArg::Type(ty_id) = arg else {
            continue;
        };
        let Some(rustc_ty) = model_ty_to_rustc(cx, *ty_id) else {
            continue;
        };
        if let TyKind::Closure(_, closure_args) = rustc_ty.kind() {
            // The rust-call closure sig is `fn((&[mut] V,))`: one tuple input whose
            // first element is `&[mut] V`. Peel the reference to reach `V`.
            let sig = closure_args.as_closure().sig();
            let inputs = sig.inputs().skip_binder();
            if let Some(tuple) = inputs.first() {
                if let TyKind::Tuple(elems) = tuple.kind() {
                    if let Some(first) = elems.iter().next() {
                        return Some(match first.kind() {
                            TyKind::Ref(_, inner, _) => *inner,
                            _ => first,
                        });
                    }
                }
            }
        }
    }
    None
}

/// Resolve a model `TyId` to a rustc `Ty` (the M4/T19 layout-verify resolver):
/// first the bridge ty-forward cache (the common case), then — when that misses —
/// a reconstruction from the model `Ty`. Handles a concrete (non-generic) ADT (via
/// its def-path → rustc DefId → `tcx.type_of(def_id).instantiate_identity()`), the
/// scalar leaves, a closure (reconstructed from its def-path + cached generic args),
/// and a reference (recurse on the pointee). `None` for anything else.
fn model_ty_to_rustc<'tcx>(cx: &BridgeCtxt<'tcx>, view_ty_id: TyId) -> Option<Ty<'tcx>> {
    use regseq_model::{IntKind, Ty as ModelTy, UintKind};
    let tcx = cx.input.tcx;
    if let Some(t) = cx.ty_forward_cache.get(&view_ty_id).copied() {
        return Some(t);
    }
    match &cx.model.types[view_ty_id] {
        ModelTy::Struct { item, args, .. }
        | ModelTy::Enum { item, args, .. }
        | ModelTy::Union { item, args, .. }
            if args.is_empty() =>
        {
            let def_id = super::super::framework_type::recover_source_def_id(
                tcx,
                cx.model.item_def_paths[*item],
            )?;
            Some(tcx.type_of(def_id).instantiate_identity())
        }
        ModelTy::Bool => Some(tcx.types.bool),
        ModelTy::Char => Some(tcx.types.char),
        ModelTy::Int(k) => Some(match k {
            IntKind::I8 => tcx.types.i8,
            IntKind::I16 => tcx.types.i16,
            IntKind::I32 => tcx.types.i32,
            IntKind::I64 => tcx.types.i64,
            IntKind::I128 => tcx.types.i128,
            IntKind::Isize => tcx.types.isize,
        }),
        ModelTy::Uint(k) => Some(match k {
            UintKind::U8 => tcx.types.u8,
            UintKind::U16 => tcx.types.u16,
            UintKind::U32 => tcx.types.u32,
            UintKind::U64 => tcx.types.u64,
            UintKind::U128 => tcx.types.u128,
            UintKind::Usize => tcx.types.usize,
        }),
        _ => None,
    }
}
