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

### Stage 4 (constants and literals)

- **D-4-1** [LOCKED] — Closed `Const` enum at 11 variants:
  `Bool(bool)`, `Char(u32)`, `Int(IntKind, u128)`,
  `Uint(UintKind, u128)`, `Float(FloatKind, u128)`, `ZST`,
  `Bytes(Vec<Byte>)`, `Aggregate(Vec<ConstId>)`,
  `EnumValue { variant_idx, payload }`, `NullPtr(TyId)`,
  `Reloc(InstanceRef)`. Plus three-state `Byte` (`Uninit |
  Value(u8) | Reloc(InstanceRef)`) and the cross-build-stable
  `InstanceRef` 4-tuple `(regseq_kind_rank, def_path_hash,
  args_key, fn_shim_disambig)`. Type-level vs value-level
  partition resolves stage 3's deferred dependency cycle via
  the §3.3 Phase B step-4 / step-9 split.
  - **D-4-1.1** [LOCKED] — Scalar grouping: per-type variants
    (`Bool` / `Char` / `Int` / `Uint` / `Float`); bit-pattern
    in `u128`; high-order zero for narrow widths; small-width
    sign/zero extension into `u128`.
  - **D-4-1.2** [LOCKED] — ZST canonicalisation: single `ZST`
    variant; type from consumer context.
  - **D-4-1.3** [LOCKED] — Aggregate field elements:
    `Aggregate(Vec<ConstId>)` with interned children.
  - **D-4-1.4** [LOCKED] — Enum payload:
    `EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> }`;
    discriminant lives on the consumer's `Ty::Enum`.
  - **D-4-1.5** [LOCKED] — `InstanceRef` 4-tuple replaces local
    `InstanceId` in `Reloc`; cross-build stability via
    `def_path_hash` + `args_key`.
  - **D-4-1.6** [LOCKED] — Null pointer typing: `NullPtr(TyId)`
    per pointee type.
  - **D-4-1.7** [LOCKED] — Inline-only type tag placement; no
    outer `Const { ty, kind }` wrapper.
  - **D-4-1.8** [LOCKED] — Three-state `Byte` for uninit /
    initialised / pointer-bearing memory; maximal-relocation-
    run well-formedness invariant.
  - **D-4-1.9** [LOCKED] — Aggregate first-class (not
    demote-to-static); bridge picks `Bytes` vs `Aggregate`
    based on rustc's const-eval shape.

- **D-4-2** [LOCKED] — `ConstIdentityKey` per-variant byte-form
  schema. Tagged-tree byte stream parallel to §12.3's per-`Ty`
  table; `u8` variant tags `0..=10` (append-only); per-variant
  payload byte forms (scalars: `u128` LE bit-patterns; `Bytes`:
  three-state per-`Byte` sub-tag; `Aggregate` / `EnumValue`:
  recursive `canonical_const(ConstId)`); `u32` LE length
  prefixes; `InstanceRef` 34-byte layout with
  `args_key.to_be_bytes()` BE exception. §3.3 step 4 scoped
  to type-level scalars; step 9 extended with three-state-DFS
  acyclicity check + value-level lookup-injectivity check
  ordered after acyclicity.
  - **D-4-2.1** [LOCKED] — Variant tag: `u8` width,
    contiguous `0..=10`, append-only.
  - **D-4-2.2** [LOCKED] — Scalar payload byte forms.
  - **D-4-2.3** [LOCKED] — Length prefix: `u32` LE.
  - **D-4-2.4** [LOCKED] — `EnumValue.variant_idx` encoding:
    `u32` LE before payload length.
  - **D-4-2.5** [LOCKED] — `Byte` sub-discriminator:
    `u8` (Uninit=0 / Value=1 / Reloc=2).
  - **D-4-2.6** [LOCKED] — Aggregate / EnumValue recursion
    via `canonical_const(ConstId)` + three-state DFS
    acyclicity validation.
  - **D-4-2.7** [LOCKED] — `NullPtr(TyId)` via recursive
    `canonical_ty(TyId)`.
  - **D-4-2.8** [LOCKED] — `Reloc(InstanceRef)` 34-byte layout
    with `args_key.to_be_bytes()` BE exception.
  - **D-4-2.9** [LOCKED] — `ZST` identity: tag-only.

- **D-4-3** [LOCKED] — Reference-typed const handling
  (`&str` / `&[T]` literals) via the new
  `SyntheticItemKind::ConstMemoryStatic { content_hash:
  ContentHash }` family. Bridge synthesises an immutable
  byte-array Static per unique memory-content blob with a
  content-derived `DefPathHash`; reference-typed const value
  composes from D-4-1's existing variants as
  `Aggregate([Reloc(synth_inst_ref), Uint(Usize, len)])`
  (wide pointer) or `Reloc(synth_inst_ref)` (thin pointer).
  No new `Const` variant needed. Whole-family scope:
  stage-4 + stage-12 territory closed under one decision.
  Non-zero offsets into Memory bridge-rejected.
  - **D-4-3.1** [LOCKED] — Family name + payload:
    `ConstMemoryStatic { content_hash: ContentHash }`.
  - **D-4-3.2** [LOCKED] — `ContentHash` construction:
    SHA-256-truncated `u128` parallel to `ArgsKey` /
    `ProjectionBindingsKey`; canonical input = D-4-2
    `Const::Bytes` byte form.
  - **D-4-3.3** [LOCKED] — §13.3 enum extension append-only.
  - **D-4-3.4** [LOCKED] — §13.4 segments
    `["<regseq_synth>", "const_memory_static", <hash hex>]`.
  - **D-4-3.5** [LOCKED] — §13.4 DefPathHash payload =
    16-byte content hash.
  - **D-4-3.6** [LOCKED] — Bridge synthesis rule: lazy
    on-demand; per-kind rich data (Static's `ty` and `init`)
    on the Instance, not the Item.
  - **D-4-3.7** [LOCKED] — Synthesised Static carries
    `Ty::Array(Ty::Uint(U8), N)` + `Mutability::Not`;
    type-erased to bytes for content-keyed dedup.
  - **D-4-3.8** [LOCKED] — Bridge dispatch arms:
    `Slice { data, meta }` → `Aggregate([Reloc, Uint])`;
    `Indirect { offset = 0 }` → `Reloc`; non-zero offset
    rejected.
  - **D-4-3.9** [LOCKED] — §3.3 Phase B step 9
    `ConstMemoryStatic` dedup sub-bullet ordered after
    value-level lookup injectivity.
  - **D-4-3.10** [LOCKED] — Stage-12 outbound flipped to
    `[CLOSED by D-4-3]`.

- **D-4-K** [DEFERRED to stage 14] — `pretty.rs::render_const`
  renderer-style decision deferred to stage 14 (extending the
  D14 → stage-13 precedent for `render_ty` one stage further
  because `render_const`'s dependency chain is longer:
  5 of 11 variants depend on `render_ty` (stage 13);
  `Reloc(InstanceRef)` additionally depends on
  `render_def_path(ItemId)` (stage 12) and `render_instance`
  (stage 14)). Stage 14 is the latest dependency. Outbound
  expectation in `stage-14/expectations.md` "From stage 4."
  No specific rendering style pre-committed by stage 4.
