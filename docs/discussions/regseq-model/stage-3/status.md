# Stage 3 Status

Status: **CLOSED**. All locked decisions firm. D14 (renderer
style) deferred to stage 13 (type-system pretty-printing).

Last updated: 2026-04-30.

For top-level project status, see `../status.md`. For project-wide
invariants, see `../README.md`. For stage-3 invariants (workflow,
principles, read order, Codex prompt template), see `memory.md`.
For the spec, see `spec-snapshot.md`.

## Closure summary

Stage 3 specifies the `Ty` enum the bridge fills from rustc's
`TyKind` and codegen consumes for C type declarations. Closed
with **15 locked decisions** (D1, D2, D3, D4, D5, D6, D7, D8,
D9, D10, D11, D12, D13, D15, D16) plus the **S0 framing
principle** ("no QualType layer"). D14 was reserved at stage
open but deferred to a dedicated stage 13 because the
renderer's input depends on `Ty` content references that only
become fully populated at stage 11 (`FnSig`) and stage 12
(`Item` / DefPath); see `../stage-13/expectations.md`.

## Locked decisions

- **D1** (1.1-1.6): variant set (19 variants), ADT kind separation
  (Struct/Enum/Union as siblings), Option/Result handling
  (ordinary `Ty::Enum`), rejection list, lang-item flags
  (PhantomData/UnsafeCell/ManuallyDrop on Item), auxiliary
  variant decisions (Str marker, FnDef vs FnPtr distinct, Never
  separate). Locked via 6-round Codex review (verdict
  trajectory: BLOCK -> MINOR -> MINOR -> MINOR -> MINOR ->
  APPROVE; issue counts 9 -> 7 -> 4 -> 1 -> 1 -> 0).

- **D2**: layout position (explicit `Layout` field on
  `Ty::Struct/Enum/Union/Tuple`, populated by copying
  `tcx.layout_of(...)`'s answer).

- **D3**: interning strategy -- structural BiMap. `TyInterner`
  carries `storage: IndexVec<TyId, Ty>` +
  `lookup: HashMap<Ty, TyId>`. Identity rule is the
  nominal/structural split: ADT-shaped variants
  (Struct/Enum/Union/FnDef/Closure) hash and compare by
  `(variant_kind_tag, item, args)` (variant tag is
  load-bearing); non-ADT variants derive structurally except
  `Ty::Tuple` which excludes its inline `Layout` from
  `Hash + Eq`. `TyId` is u32-backed (`NonMaxU32` niche). F4-2
  `ArgsKey` uses a deep canonicalizer distinct from the BiMap's
  in-build `Hash + Eq`. Self-referential ADTs intern via TyId
  reservation.

- **D4**: interning location -- `RegseqModel.types: TyInterner`,
  refining the stage-1 `IndexVec<TyId, Ty>` placeholder.
  Mirrors the stage-1 `StringInterner` precedent. Public API:
  `new`, `intern`, `resolve`, `find`, `len`, `iter`. Bridge
  holds an additional bridge-local
  `HashMap<rustc::Ty<'tcx>, TyId>` short-circuit cache.

- **D5** (SESSION-A): `Ty::Dynamic { auto_traits: Vec<AutoTrait>,
  vtable_ty: TyId }` with closed v1
  `AutoTrait::{Send, Sync, Unpin}`; `vtable_ty` resolves to a
  synthesised `Ty::Struct` (synthesis machinery in
  `../stage-12/expectations.md`). Lowering work distributed
  across four stages: stage 3 (this commitment), stage 9 (Call
  terminator shape for vtable-indirect dispatch), stage 11
  (body-level dispatch rewriting), stage 12 (Item synthesis
  including method shims and drop-in-place shims). HRTB-bearing
  `Ty::Dynamic` rejected at the bridge per the D7/D8
  refinement. New principle lifted to a named methodology rule:
  region erasure as a RegseqModel-wide invariant
  (`discussions.md` Section 2.6).

- **D6** (SESSION-A): closures synthesised as one captures-struct
  `ItemKind::Struct` Item + 1-3 call-method `ItemKind::Function`
  Items (per `ClosureKind`: 1 for `FnOnce`, 2 for `FnMut`, 3
  for `Fn`) + drop-glue Function Item iff the captures need
  dropping. DefPath: rustc verbatim. `Ty::Closure(item, args)`
  and parallel `Ty::Struct { item, args, layout }` share one
  Item. Closure call-method ABI rewritten from `RustCall` to
  `FnAbi::Rust` after receiver/argument tuple normalization.

- **D7/D8** (SESSION-B): `Provenance := { SharedBorrow,
  UniqueBorrow }` carried on `Ty::Ref` only; `Ty::RawPtr`
  renamed to `Ty::Ptr`; `Mutability` field dropped from `Ref`;
  advisory bits dropped. HRTB binders rejected at the bridge
  in v1 (refinement to D7/D8).

- **D9** (SESSION-C): volatility encoded on the intrinsic
  `InstanceId` (no `Ty` bit, no Statement flag, no `Call`
  field). MMIO via `#[rseq::intrinsic("mmio_*")]` exclusively;
  bridge rejects raw `core::ptr::{read,write}_volatile`.
  Lifted the implicit "no QualType layer" pattern to the
  named **S0** framing principle.

- **D10** (SESSION-C): no `Ty` bit; atomic types are ordinary
  `Ty::Struct` carrying the inner `is_unsafe_cell` flag; atomic
  operations are stage-14 intrinsics on the regseq-DSL
  `Atomic<T>` facade. Bridge rejects
  `core::sync::atomic::Atomic*` types and methods +
  `core::intrinsics::atomic_*`. `core::sync::atomic::{fence,
  compiler_fence}` rejected at the bridge in v1 (Call-
  translation phase).

- **D11**: layout content subset. Full `Layout` schema with
  `size`, `align`, `pack: Option<u32>`, `uninhabited`,
  `LayoutKind` enum (Struct / Union / Enum). Tuple reuses
  `LayoutKind::Struct` with synthetic field names. Drops
  `BackendRepr`, `max_repr_align`, `unadjusted_abi_align`,
  `randomization_seed`, `largest_niche`. Full-fidelity tag
  encoding.

- **D12**: niche optimization preservation -- formal corollary
  of D11 via `TagEncoding::Niche`.

- **D13** (SESSION-C): per-variant discriminant values added
  via two new fields on `VariantLayout`: `discriminant: u128`
  (Rust-level semantic discriminant, always present) and
  `tag_value: Option<u128>` (layout-level tag bytes; `None` for
  niche-encoded untagged variants and `EnumShape::Single`
  variants).

- **D15** (SESSION-B): `FnAbi { Rust, C }` enum embedded in
  `FnSig` (stage 11 wiring). All other `ExternAbi` variants --
  including `C { unwind: true }` -- bridge-rejected with
  `BridgeError::UnsupportedAbi`. `FnSig` minimum canonical
  schema for F4-2 pinned in stage 3 per M-11 (post-iteration
  refinement): `inputs`, `output`, `abi`, `c_variadic`,
  `safety`.

- **D16** (SESSION-C): bridge rejects `core::ffi::c_void` at
  type translation in v1 (recognition via
  `tcx.is_lang_item(did, LangItem::CVoid)`); diagnostic
  recommends opaque-struct pattern. Reopens when a v1 driver
  fixture or stage-14 intrinsic surfaces a stable need for
  FFI-canonical pointer-to-c_void lowering.

## Deferred decisions

- **D14** (renderer style) -- DEFERRED to **stage 13** (type-
  system pretty-printing). The renderer's input depends on
  `Ty` content references that only become fully populated at
  stage 11 (`FnSig`) and stage 12 (`Item` / DefPath); locking
  the renderer style at stage 3 would have produced a spec
  that changes shape every time later stages fill in content.
  See `../stage-13/expectations.md` for the deferred-decision
  context. The historical D14 entry in `discussions.md`
  preserves the Rust-source-style lean as input for stage 13.

## Codex review cadence (record)

- Round 1 (D1): **closed at APPROVE** in 6 sub-rounds.
- Round 2 (D2 + D3 + D4 + D11 + D12 -- structural foundation):
  **closed at APPROVE** in 9 sub-rounds (2026-04-29).
  Trajectory: 8 -> 7 -> 4 -> 3 -> 2 -> 1 -> 2 -> 3 -> 0.
- Round 3, SESSION-B (D7 + D8 + D15): **closed at APPROVE** in
  5 sub-rounds (2026-04-29). Trajectory: 7 -> 1 -> 2 -> 1 -> 0.
- Round 3, SESSION-A (D5 + D6): **closed at APPROVE** in 6
  sub-rounds (2026-04-29). Trajectory: 13 -> 7 -> 5 -> 1 -> 1
  -> 0.
- Round 3, SESSION-C (D9 + D10 + D13 + D16 plus S0 framing):
  **closed at APPROVE** in 21 sub-rounds across two
  sub-clusters (2026-04-29). D9 + D10 + S0: 16 rounds; D13 +
  D16: 5 rounds.
- Post-iteration cross-cluster sweep (external review +
  Codex R1-R10): **closed at APPROVE** at R10 (2026-04-30).
  Trajectory: external BLOCK 13 -> R1 MINOR 8 -> R2 BLOCK 9
  -> R3 BLOCK 5 -> R4 BLOCK 3 -> R5 BLOCK 3 -> R6 MINOR 1 ->
  R7 BLOCK 1 -> R8 MINOR 1 -> R9 MINOR 1 -> R10 APPROVE 0.
  Total: 52 findings closed across the iteration. This sweep
  served the role originally planned as "Round 4
  (comprehensive cross-cluster sweep)" -- run on the
  integrated shared spec after all SESSION-A/B/C merged.

Round-by-round audit trail with verdicts, issues, and
corrections: `review-log.md`.

## Stage-3-specific deferred items

(See also project-level deferred items in `../status.md`.)

- **`Pin<P>` reintroduction.** When async support is added
  (i.e., Coroutine rejection lifted), reintroduce `is_pin` flag
  recognition via `AdtFlags::IS_PIN`.
- **`MaybeUninit` flag.** If a future codegen consumer
  materializes that the union+ManuallyDrop composition does not
  satisfy, revisit whether `is_maybe_uninit` deserves a flag.
  For v1 the composition is sufficient.
- **`c_void` recognition flag.** D16; revisit when intrinsic
  signatures begin using `*mut c_void`.
- **HRTB encoding.** D7/D8 refinement; revisit if a future
  fixture or stage-14 intrinsic surfaces a stable need for
  higher-ranked function pointers or trait-object binders.
- **Rename `findings-rustc-draft.md`.** Currently labelled
  "draft" but is authoritative primary-source evidence.
  Optional rename to `findings-rustc.md`; small follow-up.
- **`StringInterner::find` symmetry.** D4 added
  `find(&self, &Ty) -> Option<TyId>` to `TyInterner`'s public
  API; `StringInterner` has no corresponding method. If a
  consumer materialises the use case, mirror the API.

## Closure ready

Stage 3 is ready to be merged into top-level docs by the merger
contract (see `../README.md` Section 5.5). Pending the merge
run, top-level `spec.md`, `discussions.md`, `review-log.md`,
and `decisions.md` will be created/extended with the stage-3
contributions.
