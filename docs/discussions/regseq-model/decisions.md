# Decisions

Project-wide registry of all per-stage decisions. Single-writer
(the merger). Lightweight index — full audit trail (Decision /
Reasoning / Methodology / Prior-art / Worked example) lives in
`discussions.md` (top-level merged) and `stage-N/discussions.md`
(per-stage).

Status legend:
- `[LOCKED]` — decision is firm and the spec reflects it.
- `[DEFERRED]` — decision was reserved at stage open but moved
  to a later stage; pre-empted before locking.
- `[REOPENED]` — a later stage revised an earlier locked
  decision; the current spec content reflects the latest
  refinement.

Tag conventions:
- Stages 1 and 3 pre-date the `D-{stage}-{decision}` convention
  and use legacy un-prefixed forms (stage 1: `F` for
  "foundation"; stage 3: `D` un-prefixed).
- Stage 4+ uses `D-{stage}-{decision}` from day one. Sub-
  decisions follow the same prefix (e.g., `D-4-1.1`).

Useful greps: `[LOCKED]` / `[DEFERRED]` / `[REOPENED]` for
status; `D-3-` for stage-3 decisions in the prefixed form (none
yet); `^- F` for stage-1 F-tags; `^- D[0-9]` for stage-3 D-tags.

---

## Stage 1 (foundation, legacy F-tag form)

- **F1** [LOCKED] — Span byte representation: 12-byte struct
  (`SourceFileId` + 4-byte `lo` + 4-byte `hi`); `[lo, hi)`
  end-exclusive; file-local byte offsets (not rustc's global
  `BytePos`).
- **F2** [LOCKED] — Span placement: inline on IR nodes (no
  `SpanId`, no span-deduplication table).
- **F3** [LOCKED] — Source content population:
  `SourceFile.content` is `Some(Arc<str>)` for crates Regseq
  extracts (spec crate, `regseq` facade, regseq-aware deps per
  Q6); `None` for upstream-crate sources.
- **F4** [LOCKED] — ItemId determinism: bridge sorts items by
  `(crate_name, crate_stable_id, segments)` before assigning
  `ItemId(0..n)`, with `def_path_data_sort_rank` providing
  stable ranks for `DefPathData` variants. Cross-build identity
  via parallel `item_def_paths: IndexVec<ItemId, DefPathHash>`
  side table.
  - **F4-2** [LOCKED] — Instance ordering: bridge sorts
    instances by `RegseqInstanceKey = (regseq_kind_rank,
    item_id, generic_args_key, fn_shim_disambig)`. The
    `generic_args_key` (`ArgsKey`) is the deep canonicaliser
    pinned by stage 3 D3.

---

## Stage 3 (type system, legacy un-prefixed D-tag form)

- **D1** [LOCKED] — `Ty` variant scope: rigid
  post-monomorphisation. The 19-variant enum mirrors the rigid
  subset of rustc's `TyKind`.
  - **D1.1** [LOCKED] — Variant set (19 variants).
  - **D1.2** [LOCKED] — ADT kind separation: `Struct`, `Enum`,
    `Union` are sibling variants (not a unified `Adt`).
  - **D1.3** [LOCKED] — `Option<T>` / `Result<T, E>` represented
    as ordinary `Ty::Enum` (no special-cased variants).
  - **D1.4** [LOCKED] — Rejection list: non-rigid `TyKind`
    variants (Param/Bound/Placeholder/Infer/Alias/Error/Pat),
    `Foreign(_)`, `Box<T>`, `Coroutine` family, `UnsafeBinder`
    rejected at type translation.
  - **D1.5** [LOCKED] — Lang-item recognition flags on `Item`:
    `is_phantom_data`, `is_unsafe_cell`, `is_manually_drop`
    (cached via rustc's `AdtFlags`). `MaybeUninit`, `CVoid`,
    `Pin` not flagged in v1.
  - **D1.6** [LOCKED] — Auxiliary variant decisions: `Str`
    marker (no payload); `FnDef` vs `FnPtr` distinct;
    `Never` separate from uninhabited Adt and from `c_void`.

- **D2** [LOCKED] — Layout position: each
  `Ty::Struct/Enum/Union/Tuple` carries a `Layout` field
  inline. `Layout` is the resolved result of
  `tcx.layout_of(ty)`, copied at type construction.

- **D3** [LOCKED] — Interning strategy: structural BiMap. Three
  identity buckets (nominal for ADT-shaped, structural-with-
  layout-excluded for `Tuple`, full structural for the rest).
  ADT-shaped identity is `(variant_kind_tag, item, args)` —
  variant tag is load-bearing so `Ty::Closure(item, args)` and
  `Ty::Struct{item, args, layout}` for the same closure remain
  distinct. F4-2 `ArgsKey` deep canonicaliser distinct from the
  BiMap's `Hash + Eq`.

- **D4** [LOCKED] — Interning location:
  `RegseqModel.types: TyInterner` (BiMap inside the model,
  mirroring `StringInterner`). Public API: `new`, `intern`,
  `resolve`, `find`, `len`, `iter`. Bridge holds an additional
  bridge-local `HashMap<rustc::Ty<'tcx>, TyId>` short-circuit
  cache.

- **D5** [LOCKED] — `Dynamic` lowering: `Ty::Dynamic {
  auto_traits: Vec<AutoTrait>, vtable_ty: TyId }` with closed
  v1 `AutoTrait::{Send, Sync, Unpin}`. Lowering work
  distributed across stages 3 (Ty shape), 9 (Call terminator
  shape), 11 (body-level dispatch rewrite), 12 (Item synthesis
  including method shims and drop-in-place shims). Codegen
  emits fat-pointer typedefs on demand. HRTB-bearing
  `Ty::Dynamic` rejected per the D7/D8 refinement.

- **D6** [LOCKED] — Closure synthesis: one captures-struct
  Item + 1-3 call-method Function Items per `ClosureKind`
  (FnOnce/FnMut/Fn) + drop-glue Function Item iff captures
  need dropping. `Ty::Closure(item, args)` and parallel
  `Ty::Struct { item, args, layout }` share one Item. Closure
  call-method ABI rewritten from `RustCall` to `FnAbi::Rust`
  after receiver/argument tuple normalisation.

- **D7** [LOCKED] — `Provenance` enum shape: closed two-variant
  `Provenance := { SharedBorrow, UniqueBorrow }` carried on
  `Ty::Ref` only. `Raw` provenance encoded by variant choice
  (`Ty::Ptr`), not by an enum value. Advisory bits
  (`frozen`/`unpin`/`global`) dropped.
  - **D7/D8 refinement** [LOCKED] — HRTB-bearing `Ty::FnPtr`
    and `Ty::Dynamic` rejected at the bridge in v1: `ReBound`
    survives rustc's region erasure, and uniform IR erasure
    would collapse rustc-distinct types under the same
    type-identity argument that locks `FnAbi`.

- **D8** [LOCKED] — `Ty::Ref` shape: `Ref(TyId, Provenance)`.
  `Mutability` field dropped (was 1:1 redundant with
  `Provenance`). `Ty::RawPtr` renamed to `Ty::Ptr` for
  symmetric pairing with `Ty::Ref`.

- **D9** [LOCKED] — Volatile semantics: encoded on the
  intrinsic `InstanceId` (no `Ty` bit, no Statement flag, no
  `Call` field). MMIO via `#[rseq::intrinsic("mmio_*")]`
  exclusively; bridge rejects raw
  `core::ptr::{read,write}_volatile`. Establishes the **S0
  framing principle** ("no QualType layer").

- **D10** [LOCKED] — Atomics as intrinsics: no `Ty` bit; atomic
  types are ordinary `Ty::Struct` carrying inner
  `is_unsafe_cell` flag (D1.5); atomic operations are stage-14
  intrinsics on the regseq-DSL `Atomic<T>` facade. Bridge
  rejects `core::sync::atomic::Atomic*` types and methods +
  `core::intrinsics::atomic_*`. `core::sync::atomic::Ordering`
  (value-level enum) NOT rejected.
  `core::sync::atomic::{fence, compiler_fence}` rejected at
  the bridge in v1 (Call-translation phase) pending future
  barrier-construct decision.

- **D11** [LOCKED] — Layout content subset: full `Layout`
  schema with `size`, `align`, `pack: Option<u32>`,
  `uninhabited`, `LayoutKind` enum (Struct / Union / Enum).
  Drops `BackendRepr`, `max_repr_align`,
  `unadjusted_abi_align`, `randomization_seed`,
  `largest_niche`. Full-fidelity tag encoding. Tuple reuses
  `LayoutKind::Struct` with synthetic field names.

- **D12** [LOCKED] — Niche optimisation preservation: formal
  corollary of D11 via `TagEncoding::Niche` carrying
  `untagged_variant`, `niche_variants`, `niche_start`.

- **D13** [LOCKED] — Discriminant level: discriminant *type*
  settled by D11 (`TagScalar.scalar_ty`); per-variant
  discriminant *values* added via two new fields on
  `VariantLayout`: `discriminant: u128` (Rust-level semantic
  discriminant, always present) and `tag_value: Option<u128>`
  (layout-level tag bytes; `None` for niche-encoded untagged
  variants and `EnumShape::Single` variants).

- **D14** [DEFERRED to stage 13] — Renderer style for `Ty`.
  Originally reserved at stage 3 as the open decision for
  `pretty.rs::render_ty` style choice. Deferred to a dedicated
  stage 13 (type-system pretty-printing) because the
  renderer's input depends on `Ty` content references that
  only become fully populated at stage 11 (`FnSig`) and stage
  12 (`Item` / DefPath). The Rust-source-style lean (Charon
  precedent) is preserved as input for stage 13 in
  `stage-3/discussions.md` (D14 entry, marked DEFERRED). Stage
  13 mints a fresh `D-13-X` tag when it makes the call; the
  D14 slot in stage 3's namespace remains DEFERRED
  indefinitely.

- **D15** [LOCKED] — fn-pointer ABI subset: `FnAbi { Rust, C }`
  embedded as a structural field on `FnSig` (load-bearing for
  type identity per Q2). Rustc's other 26 `ExternAbi` variants
  (including `C { unwind: true }`) bridge-rejected with
  `BridgeError::UnsupportedAbi`. `FnSig` minimum canonical
  schema for F4-2 pinned in stage 3 (M-11 post-iteration
  refinement): `inputs`, `output`, `abi`, `c_variadic`,
  `safety`.

- **D16** [LOCKED] — `c_void` recognition: bridge rejects
  `core::ffi::c_void` at type translation in v1 (recognition
  via `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5);
  diagnostic recommends opaque-struct pattern. D1.5's "no
  flag" lock unchanged. Reopens when a v1 driver fixture or
  stage-14 intrinsic surfaces a stable need for FFI-canonical
  pointer-to-c_void lowering.

- **S0** [LOCKED] — Framing principle: no QualType layer.
  C qualifiers (`const`, `volatile`, `restrict`) are NOT
  introduced as type-level constructs in RegseqModel. `const`
  derives from C-side qualification rule on `Ty::Ref` /
  `Ty::Ptr`; `volatile` is per-operation (encoded on
  intrinsic `InstanceId`); `restrict` is per-context at
  codegen time. Surfaced as a named principle by D9; applies
  cross-cutting.

---

## Stage 4+ (going forward, prefixed `D-{stage}-{decision}` format)

(none yet)
