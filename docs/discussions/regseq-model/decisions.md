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

- **D14** [CLOSED-BY-ABSORPTION] — Renderer style for `Ty`
  (absorbed into D-13-1 per stage 13). Originally reserved
  at stage 3 as the open decision for `pretty.rs::render_ty`
  style choice; deferred to stage 13 (type-system
  pretty-printing) because the renderer's input depends on
  `Ty` content references that only become fully populated
  at stage 11 (`FnSig`) and stage 12 (`Item` / DefPath). The
  Rust-source-style lean (Charon precedent) was preserved as
  input for stage 13 in `stage-3/discussions.md` (D14 entry).
  Stage 13's D-13-1 (master rendering style + method shape)
  locks Rust-source-style with sigil decorations + methods
  on `PrettyPrinter<'a>` returning `String`; D14's
  load-bearing question is fully answered by D-13-1, so D14
  closes by absorption per D-13-1.3.

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

- **D-4-K** [CLOSED-BY-ABSORPTION] — `pretty.rs::render_const`
  renderer-style decision was deferred to stage 14 (extending
  the D14 → stage-13 precedent for `render_ty` one stage
  further because `render_const`'s dependency chain is longer:
  5 of 11 variants depend on `render_ty` (stage 13);
  `Reloc(InstanceRef)` additionally depends on
  `render_def_path(ItemId)` (stage 12) and `render_instance`
  (stage 14)). Stage 14 D-14-4 locked the
  `render_const(ConstId, TyId) -> String` composer renderer
  + 12 sub-decisions covering the per-arm specs for the closed
  11-variant `Const` enum; D-4-K closes by absorption into
  D-14-4.

### Stage 5 (MIR places and projections)

- **D-5-1** [LOCKED] -- `Place` and `PlaceElem` core
  vocabulary (6-variant closed set + structural invariants +
  storage shape; sub-decisions D-5-1.1 through D-5-1.6).
  LOCKED at R10 MINOR-with-NIT-only; residual cross-reference
  cleanup deferred to top-level merger Codex review per the
  methodology-deviation rationale recorded in
  `stage-5/review-log.md`.
  - **D-5-1.1** [LOCKED] -- Closed `PlaceElem` vocabulary
    count: 6 variants (`Deref`, `Field`, `Index`,
    `ConstantIndex`, `Subslice`, `Downcast`); reject
    `OpaqueCast` (absent under both solver paths per H1
    REFINED) and `UnwrapUnsafeBinder` (no driver use case
    per H2 REFINED).
  - **D-5-1.2** [LOCKED] -- `Field` payload: keep inline
    `TyId` (`Field(FieldIdx, TyId)`) for O(1) narrowing.
  - **D-5-1.3** [LOCKED] -- `Downcast` payload: drop rustc's
    `Option<Symbol>` slot; `Downcast(VariantIdx)` only.
    Variant names recovered render-side from
    `VariantLayout.name`.
  - **D-5-1.4** [LOCKED] -- Projection list storage:
    `Box<[PlaceElem]>` (immutable owned slab, modulo rustc's
    `'tcx` arena interning).
  - **D-5-1.5** [LOCKED] -- Codify I-Deref-first invariant
    strictly (`Deref` appears at most once and only at
    `projection[0]`).
  - **D-5-1.6** [LOCKED] -- Codify I-Downcast-followed-by-
    Field invariant strictly (`Downcast` is non-last AND
    immediately followed by `Field`).

- **D-5-2** [LOCKED] -- Place validation phase placement:
  stage 5 owns the five validation rules (type-coherence
  walk, head-local existence, FieldIdx range, VariantIdx
  range, structural invariants); walk implementation defers
  to stage 11. LOCKED at R6 APPROVE.

- **D-5-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_place` deferred to stage 13
  (originally stage 12; retargeted to stage 11 on D-5-2/D-5-K
  R1 BLOCK; subsequently retargeted to stage 13 per stage-11
  D-11-K's categorical rendering deferral).

### Stage 6 (MIR operands)

- **D-6-1** [LOCKED] -- `Operand` core vocabulary
  (parent). 3-variant closed enum reached as the parent
  decision; sub-decisions break out the per-aspect detail.

  - **D-6-1.1** [LOCKED] -- Variant cardinality: 3
    variants (`Copy(Place)`, `Move(Place)`,
    `Constant(ConstId)`); rustc's 4th `RuntimeChecks`
    variant dropped from the IR (lowered at bridge time;
    see D-6-1.2).
  - **D-6-1.2** [LOCKED] -- `RuntimeChecks` bridge-side
    lowering rule: resolve via `checks.value(tcx.sess)`
    against the bridge-host's session (per Q4: one
    consuming session per build, even under Q6's
    multi-crate strategy), intern as `Const::Bool(b)` via
    stage-4 `ConstInterner`, emit as
    `Operand::Constant(ConstId)`. Charon precedent for
    option (c) lower-and-eliminate.
  - **D-6-1.3** [LOCKED] -- `Constant` payload shape:
    `Constant(ConstId)`, no `Box<ConstOperand>` wrapper.
    Drops `user_ty` (NLL-only, dead in `optimized_mir`)
    and operand-internal `span` (recoverable from
    Statement `SourceInfo` at stage 8 for non-Call uses;
    Call/TailCall args carry per-arg span via
    `Spanned<Operand>` at the Terminator level captured
    by stage 9).
  - **D-6-1.4** [LOCKED] -- Copy/Move preservation: keep
    `Copy(Place)` and `Move(Place)` as distinct variants
    (peer-validated by mrustc's regretted
    `Use(LValue)` collapse, Charon's preservation,
    rustc's preservation; backend optionality at zero
    spec cost).

- **D-6-2** [LOCKED] -- Operand storage: inline at use
  sites in stages 7-9 payloads. No `OperandId`, no
  operand-level interning. No downstream consumer of
  operand-content-uniqueness (contrast `Const` with
  `GenericArg::Const(ConstId)` etc.).

- **D-6-3** [LOCKED] -- Operand-site invariants: one
  invariant only (I-Operand-Place-Validity, discharged by
  inheritance from §19.7 Place validation). No
  type-`Copy` enforcement on `Operand::Copy(p)` (rustc's
  pre-drop-elaboration invariant is documented as gone
  post-drop-elaboration; `optimized_mir` is post-drop-
  elaboration).

- **D-6-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_operand` deferred to stage 13
  (originally stage 11; retargeted to stage 13 per stage-11
  D-11-K's categorical rendering deferral). The `Constant`
  arm late-binds to stage-14 `render_const` once filled.

### Stage 7 (MIR rvalues)

- **D-7-1** [LOCKED] -- `Rvalue` core vocabulary (parent).
  9-variant closed enum reached as the parent decision;
  sub-decisions break out per-variant payload shape detail.

  - **D-7-1.1** [LOCKED] -- Variant cardinality: 9 closed
    variants (`Use`, `Repeat`, `Ref`, `RawPtr`, `Cast`,
    `BinaryOp`, `UnaryOp`, `Discriminant`, `Aggregate`).
    Drop `CopyForDeref`, `ShallowInitBox` (cleanup-pass-
    discharged by required `EraseDerefTemps` /
    `ElaborateBoxDerefs` runtime-lowering passes; rustc
    codegen `bug!()`s); drop `ThreadLocalRef` (DSL-surface
    claim: TLS unavailable to Linux/Zephyr drivers); drop
    `WrapUnsafeBinder` (§15.1 type-rejection of `Ty::UnsafeBinder`
    cascades; nightly-experimental; no C-shape;
    no corpus precedent).
  - **D-7-1.2** [LOCKED] -- `Use(Operand)` direct embed
    of stage-6 Operand. No curation.
  - **D-7-1.3** [LOCKED] -- `Repeat(Operand, u64)` direct
    `u64` count, parallel to stage-3 `Ty::Array(elem_ty, n)`
    canonical-form rule (post-mono concrete count; no
    `ConstId` interning).
  - **D-7-1.4** [LOCKED] -- `Ref(Provenance, Place)`. Drop
    rustc's `Region<'tcx>` slot (compositional with stage-3
    `Ty::Ref` region erasure). Reuse stage-3's
    `Provenance { SharedBorrow, UniqueBorrow }` enum
    (`spec.md:2458`). Drop rustc's `BorrowKind::Fake`
    (cleanup-discharged -> D-7-3) and `MutBorrowKind` sub-
    discriminator (codegen-collapsed via
    `bk.to_mutbl_lossy()`).
  - **D-7-1.5** [LOCKED] -- `RawPtr(Mutability, Place)`.
    Reuse stage-3's `Mutability { Not, Mut }` enum
    (`spec.md:2461`). Bridge-side collapse:
    `RawPtrKind::FakeForPtrMetadata -> Mutability::Not`
    (matches rustc codegen's `to_mutbl_lossy()` mapping at
    `rustc_middle/src/mir/statement.rs:994-1003` -- "no type
    corresponding to a fake borrow, use *const as
    approximation").
  - **D-7-1.6** [LOCKED] -- `Cast(CastKind, Operand, TyId)`.
    11-variant `CastKind`; 4-variant `PointerCoercion`
    sub-enum (drop `MutToConstPointer` and `ArrayToPointer`
    -- rewritten to `PtrToPtr` by required
    `CleanupPostBorrowck`). Drop `CoercionSource`
    (diagnostic-only); drop `hir::Safety` from
    `ReifyFnPointer`/`ClosureFnPointer` (structurally
    duplicated with destination `Ty::FnPtr(FnSigId)`'s
    signature).
  - **D-7-1.7** [LOCKED] -- `BinaryOp(BinOp, Operand,
    Operand)` flat (Charon precedent; rustc's `Box` was a
    size-budget micro-opt we don't need); `UnaryOp(UnOp,
    Operand)`. Preserve all 26 BinOp variants and 3 UnOp
    variants (including `PtrMetadata`) verbatim.
  - **D-7-1.8** [LOCKED] -- `Discriminant(Place)` direct;
    `Aggregate(AggregateKind, Box<[Operand]>)` with
    7-variant `AggregateKind` (`Array(TyId)`, `Tuple(TyId)`,
    `Struct(TyId)`, `Enum(TyId, VariantIdx)`,
    `Union(TyId, FieldIdx)`, `Closure(TyId)` -- identity
    view per §13.1, `RawPtr(TyId, Mutability)`). Adt split
    three-way (Struct/Enum/Union) per stage-3 NominalKind
    taxonomy. Drop `Coroutine`/`CoroutineClosure` (type-
    rejected in §15.1 + StateTransform-discharged).
    Drop `UserTypeAnnotationIndex` slot (D-6-1.3 precedent
    + cleanup-pass clearing).

- **D-7-2** [LOCKED] -- Rvalue storage: inline at use
  sites in stage-8 `Statement::Assign(Place, Rvalue)`. No
  `RvalueId`, no rvalue-level interning. Compositional
  with D-5-2 (Place) and D-6-2 (Operand).

- **D-7-3** [LOCKED] -- Rvalue-site invariants. Part A:
  7-row rejection table (cleanup-pass-discharged + DSL-
  surface + experimental + type-rejected). Part B: positive
  invariants specified at stage 7 (Operand-Wellformed,
  TyId-Interned, Aggregate-Operand-Count per AggregateKind
  variant, Repeat-Count-Wellformed, VariantIdx / FieldIdx
  in-range). Part C: walk-enforcement implementation
  deferred to stage 11 (same split as D-5-2). Part D:
  F4 / F4-2 cross-build identity OUT of scope (body-local
  content).

- **D-7-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_rvalue` deferred to stage 13
  (originally stage 11; retargeted per stage-11 D-11-K's
  categorical rendering deferral). Aggregate field/variant
  names resolve directly from the per-variant `TyId` via
  the §12 type interner; Cast late-binds to stage-13
  `render_ty` (now co-located).

### Stage 8 (MIR statements)

- **D-8-1** [LOCKED] -- Closed `Statement` surface at
  3 variants: `Assign(Place, Rvalue)`,
  `SetDiscriminant(Place, VariantIdx)`,
  `Intrinsic(NonDivergingIntrinsic)`. Rustc's 13-variant
  `mir::StatementKind<'tcx>` is reduced via the cleanup-
  pass + build-flag discharge cascade (10 distinct
  dropped variants); reject-on-encounter at the bridge
  with diagnostics naming the responsible pass / absent
  flag. Sub-decisions D-8-1.1-1.4 cover variant
  cardinality and per-variant payload shapes.
  - **D-8-1.1** [LOCKED] -- 3-variant cardinality;
    12-row reject-on-encounter table covering 10
    distinct rustc variants (with split rows for
    `Coverage(BlockMarker | SpanMarker)` vs
    `Coverage(non-marker)` and for
    `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))`
    vs the surviving `Assign` general case). Mrustc-
    tighter Option D (fold `SetDiscriminant` into Rvalue
    + lift `Intrinsic` to Call) rejected on
    representational-fidelity + Q7-namespace grounds.
  - **D-8-1.2** [LOCKED] -- `Assign(Place, Rvalue)`
    flat tuple; no Box (rustc's Box is the 16-byte
    `StatementKind` size-budget artefact, not
    inherited). Type-equality invariant
    `place_ty(p) == rvalue_ty(rv)` lands as D-8-4
    Part B-1; walk-enforcement deferred to stage 11.
    No drop-suppression flag (drop is the explicit
    Drop terminator's job at stage 9).
  - **D-8-1.3** [LOCKED] -- `SetDiscriminant(Place,
    VariantIdx)` flat tuple; struct-with-named-fields
    rejected per stage-7 2-3-field-tuple precedent.
    Target-type rule `place_ty(p) = Ty::Enum { item,
    args, layout }` per §10 (compositional with
    stage-3 type taxonomy; coroutines stage-3-
    rejected; opaque types normalized by
    `PostAnalysisNormalize` required pass). Variant_index
    in-range invariant lands as D-8-4 Part B-2,
    reading `LayoutKind::Enum.shape` per §11.1
    cases. No redundant Item / Layout slot on the
    variant.
  - **D-8-1.4** [LOCKED] -- `Intrinsic(NonDivergingIntrinsic)`
    nested with sub-enum `Assume(Operand) |
    CopyNonOverlapping(CopyNonOverlapping)` and named-
    fields sub-struct `CopyNonOverlapping { src:
    Operand, dst: Operand, count: Operand }` (named
    fields are correctness-essential -- all three
    fields are Operand and order-confusable in a tuple
    shape). Flat throughout the sub-tree (no Box).
    Fanout-into-top-level-Statement-variants rejected
    on semantic-grouping + closed-surface-size +
    future-proofing grounds. Per-arm operand-type
    invariants land as D-8-4 Part B-4 (Assume:
    `operand_ty == Ty::Bool`) + B-5 (CopyNonOverlapping
    deref-able-pointer-like with identical pointees +
    count `Ty::Uint(Usize)`; Box accepted by rustc
    validator but not RegseqModel-representable per
    §15.1).

- **D-8-2** [LOCKED] -- No `Statement` interning. Inline
  storage in stage-10 basic-block storage. F4 / F4-2
  cross-build identity OUT of scope (body-local
  content). Compositional with D-5-2 / D-6-2 / D-7-2.

- **D-8-3** [LOCKED] -- `Statement` wrapper struct shape:
  `Statement { kind: StatementKind, span: Span }`. Drop
  `source_info.scope` (debuginfo-only; no C-shape
  meaning). Drop `debuginfos: StmtDebugInfos`
  entirely (per-statement DWARF instrumentation hints
  with no C-emission target). Field order `kind, span`
  matches rustc_public's analog. Charon's
  `comments_before` rejected (Aeneas-specific source-
  fidelity for verification artefacts). **Outbound to
  stage 11**: named-local preservation
  (`LocalDecl.debug_name` populated from rustc's
  body-level `var_debug_info`, distinct from
  `StmtDebugInfos`).

- **D-8-4** [LOCKED] -- Statement-site invariants +
  rejection rules. Five-part registry. Part A: 12-row
  rejection table grouped by discharge mechanism
  (Group 1 CleanupPostBorrowck-NOPed; Group 2 always-
  running cleanup-pass strip; Group 3 build-flag-
  gated). Part B: 8 positive invariants -- B-0a Place
  wellformedness inherited from D-5-2; B-0b Rvalue
  wellformedness inherited from D-7-3; B-0c Operand
  wellformedness inherited from §20.5
  (`I-Operand-Place-Validity`) for operand positions
  in `Intrinsic` payloads not enclosed by an Rvalue;
  B-1 Assign type-equality; B-2 SetDiscriminant
  variant_index in-range; B-3 SetDiscriminant target-
  type `Ty::Enum`; B-4 Intrinsic::Assume operand_ty
  bool; B-5 Intrinsic::CopyNonOverlapping deref-able-
  pointer + pointee-equality + count usize. Part C:
  walk implementation deferred to stage 11
  (compositional with D-5-2 / D-7-3 walk-impl
  deferral). Part D: F4 / F4-2 cross-build identity
  OUT of scope. Part E: peer comparison informational
  (RegseqModel sides with rustc + mrustc on validator
  discipline).

- **D-8-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_statement` deferred to stage 13
  (originally stage 11; retargeted per stage-11 D-11-K's
  categorical rendering deferral). The Statement-walk
  implementation (D-8-4 Part C) and named-local
  preservation (D-8-3) close at stage 11 per D-11-6 +
  D-11-2 (only RENDERING moves; structural concerns stay
  at stage 11).

### Stage 9 (MIR terminators)

- **D-9-1** [LOCKED] -- `Terminator` core vocabulary
  (parent decision; sub-decisions D-9-1.1-1.6).
  Closed 6-variant `TerminatorKind` (Goto, SwitchInt,
  Return, Unreachable, Call, Assert).

- **D-9-1.1** [LOCKED] -- TerminatorKind cardinality
  fixed at 6 variants. Smallest closed surface
  covering well-formed driver MIR post-pipeline; all
  rejected variants are pipeline-discharged or
  DSL-only (per D-9-9).

- **D-9-1.2** [LOCKED] -- `Goto(BlockId)` flat-tuple
  payload. Universal across rustc / rustc_public /
  Charon / mrustc.

- **D-9-1.3** [LOCKED] -- `SwitchInt { discr: Operand,
  targets: SwitchTargets }` payload (storage shape per
  D-9-6).

- **D-9-1.4** [LOCKED] -- `Return` and `Unreachable`
  unit variants (no payload).

- **D-9-1.5** [LOCKED] -- `Call { callee, args,
  destination, target }` payload, fully determined by
  D-9-3 (callee) + D-9-4 (args) + D-9-8 (drop
  unwind/call_source/fn_span). `target: Option<BlockId>`
  with None for divergent calls.

- **D-9-1.6** [LOCKED] -- `Assert { cond, expected,
  msg, target }` classified at the Terminator level
  (not Statement Charon-style). Three load-bearing
  reasons: bridge structural transparency + CFG-shape
  stability + no information advantage to demotion.
  AssertKind subset per D-9-5; drop unwind per D-9-8.

- **D-9-2** [LOCKED] -- Drop-as-Call collapse at the
  bridge. `TerminatorKind::Drop` -> synthetic
  `TerminatorKind::Call` to drop-glue Instance with
  appended `&raw mut place` Statement + fresh unit-
  typed return slot + copied target. Concrete
  `place_ty` -> `flavor=DropGlue`; `place_ty =
  Ty::Dynamic` -> `flavor=Virtual + virtual_idx=Some(0)`
  (mirrors rustc `InstanceKind::Virtual(drop_fn.def_id(),
  0)`). Direct precedent: rustc-internal codegen-SSA
  `block.rs:599-693`; Charon `desugar_drops.rs:23-72`.
  Outbound to stage 11: fresh LocalIds for `&raw mut`
  temp + unit return slot.

- **D-9-3** [LOCKED] -- Call callee shape supporting
  vtable-indirect dispatch. **Locked at candidate (b)**:
  `CallCallee::{Direct(InstanceId), Indirect(Operand)}`.
  Five load-bearing reasons: required for completeness
  (function-pointer calls), preserves emission-only
  InstanceKind design, proper IR variant per compiler
  theory, C-codegen shape match, cleans invariant 9.
  **Cross-stage schema extension** (R3 surfaced):
  added `virtual_idx: Option<u32>` to
  `InstanceKind::Function` per §9.4 with construction
  invariant `virtual_idx.is_some() iff flavor == Virtual`
  (projected at Direct virtual call sites by
  §23.6.2 B-8; global biconditional lives at §9.4 +
  §3.3 step 8). Closes stage-1 inbound
  "Vtable-indirect dispatch" + stage-3 D5 inbound;
  closes stage-1 inbound "Possibly extend
  `DefPathData`" only as it relates to stage-9
  call-callee shape (stage-12 vtable-Item DefPathData
  encoding remains stage-12-owned). Outbound to
  stage 11: vtable-indirect rewrite pass.

- **D-9-4** [LOCKED] -- Per-arg span preservation:
  drop entirely. `args: Box<[Operand]>` (no per-arg
  span). Per-arg diagnostic granularity falls back to
  Terminator-level `span`. rustc_public-aligned (per
  `mir.rs:752` `arg.node.stable(...)` strip).
  Supersedes (rather than satisfies) stage-6 D-6-1.3
  inbound (which listed only preserve-via-wrapper or
  parallel-slabs); D-9-4 introduces a third drop-
  entirely option on consumer-landscape grounds.

- **D-9-5** [LOCKED] -- `AssertKind` 8-variant subset
  (drop 3 coroutine-only variants:
  ResumedAfterReturn / ResumedAfterPanic /
  ResumedAfterDrop). Type-rejection-cascade
  consistency with stages 5-8; preserves diagnostic
  quality for driver-relevant cases (BoundsCheck,
  Overflow, etc.). rustc_public-aligned with single
  deliberate divergence.

- **D-9-6** [LOCKED] -- `SwitchTargets` explicit-split
  storage: `cases: Box<[(u128, BlockId)]>` plus
  `otherwise: BlockId`. Three reasons: explicit >
  implicit; no runtime invariant to enforce;
  compositional with zero-cases edge case. Aligned
  with rustc_public's
  `SwitchTargets { branches, otherwise }` shape.

- **D-9-7** [LOCKED] -- No Terminator interning.
  Inline storage at `BasicBlock.terminator: Terminator`
  (stage-10 vocabulary). Universal precedent across
  rustc / rustc_public / Charon / mrustc; pattern
  consistent with stages 5-8 inline-storage rule.
  F4/F4-2 OUT of scope.

- **D-9-8** [LOCKED] -- Wrapper struct shape:
  `Terminator { kind: TerminatorKind, span: Span }`.
  Direct precedent: rustc_public `body.rs:140`. Drops
  `source_info.scope` (debuginfo-only),
  `Call.call_source` (diagnostic-only),
  `Call.fn_span` (subsumed), Drop's
  `replace`/`drop`/`async_fut` (subsumed by D-9-2),
  `unwind: UnwindAction` on all variants (under
  panic=abort the surviving values carry no
  driver-relevant information).

- **D-9-9** [REOPENED] -- 9-row rejection table
  (8 rejected + 1 collapsed via D-9-2). (revised in
  stage 14 D-14-7: rejection vocabulary extended
  with `#[naked]` Item-translation row at §15.1,
  sibling to InlineAsm terminator-translation row
  at §23.6.1; §23.6.1 9-row table itself stays at
  9 rows; §15.1 table grows by one row.) Two-category
  framing:
  - **Pipeline-discharged (rows 1-6):** FalseEdge,
    FalseUnwind, Yield, CoroutineDrop, UnwindResume,
    UnwindTerminate. Discharged by required passes
    (CleanupPostBorrowck, StateTransform,
    AbortUnwindingCalls + SimplifyCfg).
  - **DSL-only (rows 7-8):** TailCall, InlineAsm.
    rustc supports under feature gates; DSL surface
    excludes both.
  - **Bridge-collapsed (row 9):** Drop per D-9-2.
  Walk-pass enforcement deferred to stage 11.

- **D-9-10** [REOPENED] -- 4-part invariants registry.
  (revised in stage 14 D-14-2.5: B-4 and B-6 carved
  out for `IntrinsicKind::Log(_)` callees per the
  `I-DiagLog-CallArity` invariant -- bridge
  translation contract for the diagnostic-logging
  intrinsic vocabulary admits flat-arg shape after
  DiagLogPeel.) Part A: rejection table cross-ref
  (D-9-9). Part B:
  2 inherited (B-0a Place, B-0c Operand) + 9
  stage-9-introduced positive invariants (11 total)
  (B-1 SwitchInt discr type; B-2 Assert cond bool;
  B-3 Call destination type matches return type
  **when target: Some(_)**; B-4 Call args arity
  matches callee; B-5 callee callable; B-6 per-arg
  type match; B-7 closes construction invariant 9;
  B-8 Terminator-site projection of §9.4 virtual_idx
  biconditional at Direct-virtual call sites
  [global biconditional lives at §9.4 + §3.3 step 8;
  B-8 added at R4 alongside the §9.4 schema
  extension]; B-9 target/callee-output coherence
  [target.is_none iff callee output == Ty::Never;
  added at post-merge R5]). Part C: walk-impl
  deferred to stage 11. Part D: F4/F4-2 OUT of
  scope.

- **D-9-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_terminator` + 3 public sub-
  renderers + 1 internal helper deferred to stage 13
  (originally stage 11; retargeted per stage-11 D-11-K's
  categorical rendering deferral). CallCallee::Direct
  rendering late-binds to stage-12 `render_def_path`.
  Non-rendering items from the original D-9-K outbound
  (Terminator walk implementation D-9-10 Part C; body-
  level vtable-indirect rewrite pass D-9-3 (b); fresh
  LocalIds D-9-2) close at stage 11 per D-11-6 + D-11-7 +
  D-11-8.

### Stage 10 (MIR basic blocks)

- **D-10-1** [LOCKED] -- `BasicBlock` per-block
  struct shape. 2-field closed structure
  (`statements: Box<[Statement]>`, `terminator:
  Terminator`); drops rustc's `is_cleanup` (panic=abort
  discharge) and `after_last_stmt_debuginfos`
  (debuginfo discharge); collapses
  `Option<Terminator>` to non-Optional at the type
  level. Stage-9 §23.5 preview formalised. 3-of-4
  peer alignment (rustc_public, Charon, mrustc all
  use 2-field shape).
  - **D-10-1.1** [LOCKED] -- Cardinality: 2 fields.
  - **D-10-1.2** [LOCKED] -- `statements: Box<[Statement]>`
    (RegseqModel internal pattern: stage-7
    `Aggregate(Box<[Operand]>)` + stage-9
    `SwitchTargets.cases: Box<[(u128, BlockId)]>`
    finalized payload precedent).
  - **D-10-1.3** [LOCKED] -- `terminator: Terminator`
    non-Optional (rustc_public/Charon/mrustc all do
    this; type-level encoding of post-construction
    invariant).
  - **D-10-1.4** [LOCKED] -- Drop `is_cleanup`
    (panic=abort discharge from stage-9 D-9-9 rows
    5-6).
  - **D-10-1.5** [LOCKED] -- Drop
    `after_last_stmt_debuginfos` (debuginfo
    discharge from stage-8 D-8-3).
  - **D-10-1.6** [LOCKED] -- No `BasicBlock`-level
    metadata (no span, no scope; spans live on
    Statement and Terminator individually).

- **D-10-2** [LOCKED] -- Body-level basic-block
  container shape. Bare `IndexVec<BlockId, BasicBlock>`
  storage; no wrapper, no CFG cache, no type alias.
  CFG analyses (predecessors, dominators, RPO
  iteration) live in `regseq-analysis` per Q1
  architectural crate split. **Principled rule**: the
  model stores facts the bridge owns or rustc has
  resolved (types, layouts, const values, instance
  identities); the model does not store analyses
  derivable from those facts via standard CFG/SSA
  algorithms. The single exception -- RPO ordering
  of `basic_blocks` storage -- is justified by being
  the layout itself, not a separate field.
  - **D-10-2.1** [LOCKED] -- Storage:
    `IndexVec<BlockId, BasicBlock>` (typed-IndexVec;
    stage-1 §2.6 canonical).
  - **D-10-2.2** [LOCKED] -- No wrapper struct;
    bare type at field-declaration sites.
  - **D-10-2.3** [LOCKED] -- No CFG cache; analyses
    live in `regseq-analysis`. Principled rule
    documented in §spec.
  - **D-10-2.4** [LOCKED] -- No type alias.

- **D-10-3** [LOCKED] -- `BlockId` body-local
  typed-index introduction via `newtype_index!` (per
  stage-1 ID conventions). Closes 4 inbound stage-1
  expectations: BlockId definition, const-body macro
  arm, sigil convention, size/niche assertions.
  Rejects rustc_public's bare `BasicBlockIdx = usize`
  on stage-1 ID-discipline grounds (no niche, no
  sigil, no type-safety).
  - **D-10-3.1** [LOCKED] -- `BlockId` introduced
    via `newtype_index!` in
    `crates/regseq-model/src/idx.rs` (mechanical
    inheritance from stage-1 ID conventions).
  - **D-10-3.2** [LOCKED] -- `newtype_index!` macro
    extended with const-body arm grammar
    `pub struct $Name { const NAME = INT_LITERAL; }`
    (multi-const supported; integer-literal-only;
    empty-body arm preserved). Direct grammar match
    to rustc's
    `pub struct BasicBlock { const START_BLOCK = 0; }`.
    `LocalId::RETURN_PLACE` migration deferred to
    stage 11.
  - **D-10-3.3** [LOCKED] -- Static assertions
    `size_of::<BlockId>() == 4` and
    `size_of::<Option<BlockId>>() == 4` appended to
    `crates/regseq-model/src/lib.rs` per existing
    crate-level-ID pattern.
  - **D-10-3.4** [LOCKED] -- Sigil textual
    convention `BlockId(N)` -> `bbN` (universal peer
    match: rustc, rustc_public, Charon, mrustc). The
    macro's auto-generated `Debug` impl unchanged
    (renders `BlockId(N)`); the `bbN` sigil belongs
    to RegseqModel's custom textual format and is
    renderer-injected at the BasicBlock-rendering
    site (deferred to stage 11 under D-10-K.2).

- **D-10-4** [LOCKED] -- Block-ordering invariants:
  B-RPO storage order (inherited from
  `ReorderBasicBlocks` in cleanup whitelist) +
  B-Entry `START_BLOCK = BlockId(0)` (inherited from
  rustc's START_BLOCK const + `prettify.rs:33`
  fixed-point). No defensive fallback for non-RPO
  inputs (per pinned-toolchain methodology).
  - **D-10-4.1** [LOCKED] -- B-RPO: storage in
    reverse-postorder of body's CFG rooted at
    `BlockId::START_BLOCK`.
  - **D-10-4.2** [LOCKED] -- B-Entry:
    `BlockId::START_BLOCK == BlockId(0)` is the
    entry block of every Body.
  - **D-10-4.3** [LOCKED] -- Walk-implementation
    deferred to stage 11; jointly enforces B-RPO +
    B-Reachable + B-Entry via the §24.4.3 DFS-RPO
    procedure (steps 3-6), preceded by B-NonEmpty +
    B-Successors-Valid pre-checks (steps 1-2;
    pre-checks prevent indexing-panic from
    out-of-range successors during DFS).
  - **D-10-4.4** [LOCKED] -- No defensive fallback
    for non-RPO inputs (per "no defensive fallbacks"
    methodology rule + pinned-toolchain contract).

- **D-10-5** [LOCKED] -- Per-stage invariants
  registry (5-part). Part A: 3-row rejection table
  (bridge-enforced at construction; rows 1-2
  defensive `is_cleanup=true` + `terminator: None`,
  row 3 silent-drop `after_last_stmt_debuginfos`).
  Part B: 8 positive invariants (2 inherited B-0a
  Statement / B-0b Terminator + 6 stage-10-introduced:
  B-RPO, B-Reachable, B-Entry, B-Terminator-Some,
  B-Successors-Valid, B-NonEmpty). B-Reachable is
  verified at the bridge boundary by the stage-11
  DFS-RPO walk; rustc's standard pipeline scheduling
  (`SimplifyCfg::PreOptimizations` +
  `remove_dead_blocks`) prunes unreachable blocks in
  practice but the spec does not lean on that
  scheduling for soundness. Part C: walk-impl
  deferred to stage 11 (Part B only; RegseqModel's
  `BasicBlock` shape makes Part-A conditions
  structurally inexpressible post-construction).
  Part D: F4/F4-2 OUT of scope. Part E:
  forward-looking considerations (no stage-10
  non-invariants; per-instance const-folding noted
  as a future-stage concern).

- **D-10-K** [REOPENED] (retargeted to stage 13 per D-11-K)
  -- `pretty.rs::render_basic_block` deferred to stage 13
  (originally stage 11; retargeted per stage-11 D-11-K's
  categorical rendering deferral). Non-rendering items
  from the original D-10-K outbound (body walk
  implementation D-10-5 Part C; body-level basic-block
  table integration; `LocalId::RETURN_PLACE` migration)
  close at stage 11 per D-11-1 + D-11-4 + D-11-6.
  - **D-10-K.1** [REOPENED] (retargeted to stage 13 per
    D-11-K) -- `render_basic_block(&BasicBlock, &Body)`
    materializes at stage 13.
  - **D-10-K.2** [REOPENED] (retargeted to stage 13 per
    D-11-K) -- `BlockId` sigil-rendering helper
    introduced at stage 13 alongside first consumer;
    textual convention `bbN` pinned in §24.3.4 stays
    stage-10-owned.
  - **D-10-K.3** [REOPENED] (retargeted to stage 13 per
    D-11-K) -- §18.2 per-method ownership table row
    retargeted to stage 13 (rows remain in §18.2 with
    new ownership).

### Stage 11 (MIR bodies)

- **D-11-1** [LOCKED] -- `Body` 4-field flat shape `{ basic_blocks, local_decls, arg_count, span }`; ~14 rustc Body fields rejected (post-mono / no DWARF / borrowck-discharged / coroutine-rejected / collector-discharged / instrumentation / debuginfo-lifted / Item-side-linkage); no Locals substructure; no body-level fn_sig; no body-level item back-pointer; no spread_arg.
  - **D-11-1.1** [LOCKED] -- Cardinality: 4 fields.
  - **D-11-1.2** [LOCKED] -- `basic_blocks` inherited from D-10-2.
  - **D-11-1.3** [LOCKED] -- `local_decls` flat (no Locals substructure).
  - **D-11-1.4** [LOCKED] -- `arg_count: usize` flat; positional layout convention.
  - **D-11-1.5** [LOCKED] -- `span: Span` whole-body diagnostic.
  - **D-11-1.6** [LOCKED] -- Rejection table for ~14 dropped rustc Body fields.
  - **D-11-1.7** [LOCKED] -- No body-level `item: ItemId` back-pointer.

- **D-11-2** [LOCKED] -- `LocalDecl` 4-field shape `{ ty: TyId, mutability: Mutability, span: Span, debug_name: Option<StringId> }`; named-local lift from `var_debug_info` per D-8-3; rejected: `local_info`, `user_ty`, `source_info.scope`.
  - **D-11-2.1** [LOCKED] -- Cardinality: 4 fields.
  - **D-11-2.2** [LOCKED] -- `ty: TyId`.
  - **D-11-2.3** [LOCKED] -- `mutability: Mutability` (info-preservation).
  - **D-11-2.4** [LOCKED] -- `span: Span`; `Span::dummy()` for synthetic.
  - **D-11-2.5** [LOCKED] -- `debug_name: Option<StringId>` per D-8-3.
  - **D-11-2.6** [LOCKED] -- Rejection table for 3 dropped LocalDecl fields.

- **D-11-3** [REOPENED] -- FnSig location: Item-side, NOT on Body. Stage-12 absorbs FnSig as `Item.kind = Function { fn_sig: FnSigId, ... }`. Stage-1 inbound's Body-side framing superseded ("Spec is mutable"). Body's signature reachable via `local_decls[1..=arg_count]` (inputs) + `local_decls[RETURN_PLACE]` (return). (revised in stage 12 per D-12-1.4: FnSig moves to Instance-side `InstanceKind::Function { fn_sig, body, flavor, virtual_idx }` per Option B; Items are pre-mono per F4 sort + §13.4 source DefPathHash + §9.3 RegseqInstanceKey, but stage-3 §15.1 rejects Param/Bound TyKind variants so a stored FnSig must be already-substituted post-mono -- placement on InstanceKind restores symmetry with non-Function variants.)
  - **D-11-3.1** [LOCKED] -- Body has no `fn_sig` field.
  - **D-11-3.2** [LOCKED] -- FnSig location: Item-side per (e); 4-peer convergence.
  - **D-11-3.3** [LOCKED] -- Argument types via `local_decls[1..=arg_count].ty`.
  - **D-11-3.4** [LOCKED] -- Return type via `local_decls[RETURN_PLACE].ty`.
  - **D-11-3.5** [LOCKED] -- Outbound to stage 12: §14.3 5-field schema (preserve `safety`).
  - **D-11-3.6** [LOCKED] -- Stage-1 inbound's Body-side framing superseded.

- **D-11-4** [LOCKED] -- `LocalId` typed-index via `newtype_index!` with `const RETURN_PLACE = 0` (using stage-10 D-10-3.2 const-body arm); niche/size assertions; sigil `_N` (rendering site to stage 13 per D-11-K).
  - **D-11-4.1** [LOCKED] -- `LocalId` introduced via `newtype_index!`.
  - **D-11-4.2** [LOCKED] -- `const RETURN_PLACE = 0` (stage-10 const-body arm).
  - **D-11-4.3** [LOCKED] -- Size + niche assertions.
  - **D-11-4.4** [LOCKED] -- Sigil `_N` pinned; rendering site to stage 13 per D-11-K.

- **D-11-5** [REOPENED] -- No `BodyId`; bodies as structural field of Item per `Item.body: Option<Body>` (placement finalized by stage 12). 4-peer convergence; walkers reach Body via Item. (revised in stage 12 per D-12-1.4: bodies move to Instance-side `InstanceKind::Function { body: Option<Body>, ... }` per Option B; per-monomorphization Instance carries its own substituted body. The "no BodyId" half of the decision is preserved -- bodies still don't have a separate ID type; only the placement changes from per-Item to per-Instance.)
  - **D-11-5.1** [LOCKED] -- No `BodyId` allocation.
  - **D-11-5.2** [LOCKED] -- Bodies as Item structural field.
  - **D-11-5.3** [LOCKED] -- `Option<Body>` accommodates foreign decls + non-Function kinds.
  - **D-11-5.4** [LOCKED] -- Body-fetch via Item; no reverse lookup.
  - **D-11-5.5** [LOCKED] -- Outbound to stage 12: absorb Body as Item structural component.

- **D-11-6** [LOCKED] -- Joint validation walk: visitor with override hooks at `crates/regseq-model/src/visit.rs`; 10 visit/super hook pairs (body, basic_block, local_decl, statement, terminator, place, rvalue, operand, ty, call_callee); read-only `Visitor` at stage 11 (`MutVisitor` deferred); absorbs 5 deferred walks (D-5-2, D-7-3, D-8-4, D-9-10, D-10-5).
  - **D-11-6.1** [LOCKED] -- Construction invariants 1+9 fold into visitor hooks; no separate pre-pass.
  - **D-11-6.2** [LOCKED] -- Visitor location: `crates/regseq-model/src/visit.rs`.
  - **D-11-6.3** [LOCKED] -- Hook surface: 10 visit/super pairs.
  - **D-11-6.4** [LOCKED] -- Mutability: read-only `Visitor` at stage 11.
  - **D-11-6.5** [LOCKED] -- §24.4.3 6-step DFS-RPO with B-NonEmpty + B-Successors-Valid pre-checks.
  - **D-11-6.6** [LOCKED] -- 5 deferred walks close at joint visitor.

- **D-11-7** [LOCKED] -- Hybrid (c) timing: Drop-as-Call at bridge construction time (forced by D-9-1.1's Drop-variant rejection); vtable-indirect rewrite as post-construction pass. Cross-stage friction with D-5-1: multi-statement rewrite shape forced by D-5-1's PlaceElem set lacking `PtrMetadata`. Pass scheduling: rewrite precedes joint validation walk.
  - **D-11-7.1** [LOCKED master] -- Hybrid (c) timing.
  - **D-11-7.2** [LOCKED] -- Drop-as-Call at bridge time per stage-9 §23.4 step 4.
  - **D-11-7.3** [LOCKED] -- Vtable-rewrite as post-construction pass.
  - **D-11-7.4** [LOCKED] -- 6-step rewrite procedure per stage-9 §23.3.
  - **D-11-7.5** [LOCKED] -- Pass scheduling: rewrite before validation.
  - **D-11-7.6** [LOCKED] -- Cross-stage friction with D-5-1; D-5-1 reopening out of scope.
  - **D-11-7.7** [LOCKED] -- Stage-12 dependency notice for vtable Item synthesis.

- **D-11-8** [LOCKED] -- LocalDecls allocation API: single push method on Body. `Body::push_local(decl) -> LocalId` + helper `Body::push_synthetic_local(ty) -> LocalId`. No BodyBuilder; no buffering.
  - **D-11-8.1** [LOCKED master] -- Single push API on Body.
  - **D-11-8.2** [LOCKED] -- `Body::push_local(decl) -> LocalId`.
  - **D-11-8.3** [LOCKED] -- `Body::push_synthetic_local(ty) -> LocalId` helper.
  - **D-11-8.4** [LOCKED] -- No BodyBuilder.
  - **D-11-8.5** [LOCKED] -- No buffering.
  - **D-11-8.6** [LOCKED] -- Consumer enumeration (bridge + Drop-as-Call + vtable-rewrite).

- **D-11-K** [LOCKED] -- Categorical rendering deferral: all body-level renderers + sigil wirings move to stage 13. Single master decision; D-5-K, D-6-K, D-7-K, D-8-K, D-9-K, D-10-K + D-10-K.1/.2/.3 flip to [REOPENED] with annotation `(retargeted to stage 13 per D-11-K)`. §18.2 rows retargeted in place. Stage-11 retains structural ownership.
  - **D-11-K.1** [LOCKED master] -- Categorical rendering deferral.
  - **D-11-K.2** [LOCKED] -- 12-item bundle (9 public renderers + 1 internal helper + 2 sigil wirings).
  - **D-11-K.3** [LOCKED] -- Predecessor K-tags flip to [REOPENED]; cross-stage bookkeeping to stage-13/expectations.md.
  - **D-11-K.4** [LOCKED] -- §18.2 row retargeting in place.
  - **D-11-K.5** [LOCKED] -- Stage-11 retains structural spec ownership; only RENDERING moves.
  - **D-11-K.6** [LOCKED] -- Cross-references; stage-13 inherits predecessor K-tag dependency analysis.

### Stage 12 (Rust Items)

- **D-12-1** [LOCKED] -- Item schema + ItemKind shape: 6-variant flat ItemKind; per-mono data on InstanceKind (Option B reframe); Item-level typed metadata per stage-1 R5.2 (linkage, link_section, inline_hint, is_unsafe + lang-item flags + opaque_handle_eligible; `is_naked` removed by stage-14 D-14-7 -- v1 rejects `#[naked]` at Item translation).
  - **D-12-1.1** [LOCKED] -- Cardinality: closed 6-variant flat `{ Function, Static, Const, Struct, Enum, Union }`. Aligns with rustc-internal first-class ADT representation.
  - **D-12-1.2** [LOCKED] -- `ItemKind::Static { mutability: Mutability }`; Const/Struct/Enum/Union stay unit. Mutability moves from InstanceKind::Static to ItemKind::Static (option c) per stage-1 intent.
  - **D-12-1.3** [REOPENED] -- Item-level typed fields: `linkage: Linkage` (closed 4-variant), `link_section: Option<StringId>`, `inline_hint: InlineHint` (closed 4-variant), `is_unsafe: bool`, `is_naked: bool`. Visibility omitted in v1. (revised in stage 14 D-14-7: `is_naked` field removed; v1 rejects `#[naked]`-attributed Items at Item translation.)
  - **D-12-1.4** [LOCKED] -- **Option B reframe**: `InstanceKind::Function { fn_sig: FnSigId, body: Option<Body>, flavor, virtual_idx }`. Restores symmetry across all 6 InstanceKind variants. Reopens stage-11 D-11-3 + D-11-5.
  - **D-12-1.5** [LOCKED] -- DomainAttr forward-pointer commentary only at §8.0 + §3; no stage-12 schema. Stage-14 owns property vocabulary on RegseqModel sibling tables per stage-4 §1287-1299.
  - **D-12-1.6** [LOCKED] -- Hybrid accessor: `InstanceKind::as_function() -> Option<&FunctionInstanceData>` + match-arm-readable; named `FunctionInstanceData` struct.
  - **D-12-1.7** [LOCKED] -- Stage-11 D-11-3 + D-11-5 reopening process: flip to [REOPENED] in decisions.md at stage-12 merger close per README §5.5 step 4 with annotation `(revised in stage 12 per D-12-1.4)`.
  - **D-12-1.8** [LOCKED] -- Instance-side pull-through helpers: methods on `RegseqModel` (13-helper v1 inventory). Composes with §23.6.2 callee_fn_sig pattern.

- **D-12-2** [LOCKED] -- Foreign / extern declaration dispatch.
  - **D-12-2.1** [LOCKED] -- `InstanceKind::Function.body: None ⟺ tcx.is_foreign_item(did)`. fn_sig populated for foreign fns. flavor: Item; virtual_idx: None.
  - **D-12-2.2** [LOCKED] -- `InstanceKind::Static.init: None ⟺ tcx.is_foreign_item(did)`. Mutability lives on `ItemKind::Static` per D-12-1.2 (c); InstanceKind::Static is now `{ ty, init }` (2-field).
  - **D-12-2.3** [LOCKED] -- Linkage translation: is_foreign->Foreign; NO_MANGLE/export_name->NoMangle; WeakAny/WeakODR->Weak; otherwise Default. Other rustc Linkage variants rejected with BridgeError::UnsupportedLinkage.
  - **D-12-2.4** [LOCKED] -- `link_name` stays Instance-side (folded into `Instance.emission_name` per §9.2); informational restatement of D-12-1.3 omit.

- **D-12-3** [LOCKED] -- Lang-item recognition mechanism.
  - **D-12-3.1** [LOCKED] -- Closed 3-flag set carries (`is_phantom_data`, `is_unsafe_cell`, `is_manually_drop`); no expansion in v1. is_pin/is_c_void/is_box deferred per documented criteria.
  - **D-12-3.2** [LOCKED] -- Recognition via AdtFlags predicates: `adt_def.is_phantom_data()` etc. at bridge type-translation time. O(1) bitflag tests.
  - **D-12-3.3** [LOCKED] -- Non-flagged lang-items queried on-demand at consumer sites: `tcx.is_lang_item(did, LangItem::*)` / `tcx.lang_items().X()`. Never stored on Item.
  - **D-12-3.4** [LOCKED] -- Reject opaque `LangItem` tag and Charon-style string-based `lang_item: Option<String>`. Closed-vocabulary discipline per §17.4.

- **D-12-4** [LOCKED] -- Construction-invariant closure for stage-1 deferrals (3 invariants).
  - **D-12-4.1** [LOCKED] -- Invariant 3 closure: F4 sort monotonicity walk at validate; non-strict `key[i] <= key[i+1]`; landing site Phase B step 3 (d).
  - **D-12-4.2** [LOCKED] -- Invariant 6 closure: `item.def_path.crate_id.index() < crates.len()` walk; landing site Phase B step 3 (c).
  - **D-12-4.3** [LOCKED] -- Invariant 4 closure: `BridgeInput { spec_crate_stable_id }` parameter on validate; `validate(&self, &BridgeInput)` signature change; landing site Phase B step 3 (e).

- **D-12-5** [LOCKED] -- Bridge construction-pipeline order: 5-phase. New §3.4 subsection.
  - **D-12-5.1** [LOCKED] -- Canonical 5-phase pipeline: Discovery -> Translation+Synthesis -> F4 Sort -> ItemId Assignment + side-tables -> Instance Construction (with Phase 5 body work: materialisation -> Drop-as-Call collapse -> vtable rewrite -> validation).
  - **D-12-5.2** [LOCKED] -- Synthesis interleaved during Phase 2 translation; family-specific Phase-2 working caches keyed by stable Phase-2-available handles (PrincipalTraitKey/closure_did/ContentHash -> Phase2Handle); RegseqModel.vtables populated in Phase 5.
  - **D-12-5.3** [LOCKED] -- Post-sort ItemId assignment. Cross-reference handling during construction (placeholder ItemIds vs DefPathHash key) is bridge-implementation choice.
  - **D-12-5.4** [LOCKED] -- New §3.4 subsection "Bridge construction-pipeline order" lands at stage-12 merger.

- **D-12-6** [LOCKED] -- `pretty.rs::render_def_path(ItemId)` stub fill.
  - **D-12-6.1** [LOCKED] -- One-line body: `self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()`. Cache lookup via eager `Item.def_path.rendered`.
  - **D-12-6.2** [LOCKED] -- `render_def_path(ItemId)` (ItemId convenience) and `render_def_path_value(&DefPath)` (value-level renderer) coexist with documented division of labor.
  - **D-12-6.3** [LOCKED] -- §18.2 per-method ownership table updates: `render_def_path` row flips from "deferred to stage 12" to "stage 12; D-12-6".

- **D-12-K** [LOCKED] -- Categorical Item-level rendering deferral to stage 13, compositional with D-11-K cascade. Only `render_def_path` (D-12-6) stays at stage 12.
  - **D-12-K.1** [LOCKED] -- Deferral list: render_item, ItemKind variant rendering, Linkage rendering, InlineHint rendering, Function-flag rendering, link_section rendering, lang-item flag rendering, opaque_handle_eligible.
  - **D-12-K.2** [LOCKED] -- Outbound bundle of 7 entries to file in `stage-13/expectations.md` "From stage 12 (per D-12-K)" at merger close. No predecessor K-tag retargeting (purely additive).
  - **D-12-K.3** [LOCKED] -- §18.2 per-method ownership table gets new "deferred to stage 13 per D-12-K" rows for the 7-entry bundle.

## Stage 13 (Pretty-printing)

- **D-13-1** [LOCKED] -- Master rendering style + method shape: Rust-source-style with sigil decorations + PrettyPrinter methods returning String.
  - **D-13-1.1** [LOCKED] -- Textual style = Rust-source-style with sigil decorations; 4-peer prior-art convergence (rustc-internal, rustc_public, Charon, project's render_def_path_value); mrustc dual-rendering rejected as anti-pattern.
  - **D-13-1.2** [LOCKED] -- Method shape = methods on `PrettyPrinter<'a>` returning `String`; shipped at stage 1; Q5 context-required.
  - **D-13-1.3** [LOCKED] -- Stage-3 D14 closes by absorption into D-13-1.

- **D-13-2** [LOCKED] -- Body envelope + indentation + sigil placement: rustc-converged envelope, 4-space INDENT, inline sigil wirings.
  - **D-13-2.1** [LOCKED] -- Body envelope = `fn $path(_1: T, ...) -> Ret { let _N: T; ...; bbN: { stmt; stmt; terminator -> [edges]; } }`.
  - **D-13-2.2** [LOCKED] -- Indentation = 4-space INDENT, 8-space stmt indent (rustc-converged).
  - **D-13-2.3** [LOCKED] -- Sigil wirings inline at first consumer; `format!("_{}", id.index())` for LocalId, `format!("bb{}", id.index())` for BlockId.
  - **D-13-2.4** [LOCKED] -- D-13-2 / D-13-4 boundary: function header (`fn $path(...) -> Ret` + Item-level attributes) belongs to D-13-4; D-13-2 owns body content only.
  - **D-13-2.5** [LOCKED] -- Locals block = one decl per line, `let mut _N: T` / `let _N: T` rustc-converged; debug names render as `debug NAME => _N;` standalone lines after locals (gated by D-13-3.4).

- **D-13-3** [LOCKED] -- Span-rendering mode + debuginfo gating: default-off, opt-in via `PrettyPrintOptions { include_spans, include_debug_names }` constructed via `PrettyPrinter::with_options`.
  - **D-13-3.1** [LOCKED] -- Default mode = default-off, opt-in via toggle.
  - **D-13-3.2** [LOCKED] -- Toggle mechanism = `PrettyPrintOptions` struct + `with_options` constructor; existing `new(model)` preserved.
  - **D-13-3.3** [LOCKED] -- Span shape (when on) = trailing inline `// <render_span>` aligned to col 40 (rustc `ALIGN = 40`).
  - **D-13-3.4** [LOCKED] -- Debug-name shape (when on) = standalone `debug NAME => _N;` lines after locals declarations.
  - **D-13-3.5** [LOCKED] -- Synthetic-span suppression: `SourceFileId::SYNTHESIZED` spans suppress when toggle on (no trailing comment).

- **D-13-4** [LOCKED] -- Item-level rendering shape: Rust-source `#[...]` attribute syntax, default-suppression, per-flag/per-ItemKind tables, multi-line layout, building-block-vs-composer architectural division.
  - **D-13-4.1** [LOCKED] -- Attribute syntax = Rust-source `#[...]` form.
  - **D-13-4.2** [LOCKED] -- Default-suppression for all default/false-valued fields.
  - **D-13-4.3** [REOPENED] -- Per-flag rendering table: source-faithful where rustc has the attribute (`#[no_mangle]`, `#[inline(always)]`, `#[link_section = "..."]`, `#[lang = "..."]`, `#[naked]`, `unsafe` keyword on `fn`); `#[opaque_handle]` for the bridge-outcome flag. (revised in stage 14 D-14-7: `#[naked]` row removed -- v1 rejects `#[naked]`-attributed Items at Item translation; §5.6 row-order recap renumbered.)
  - **D-13-4.4** [LOCKED] -- Per-ItemKind kind-keyword + DefPath: `[unsafe ]fn`, `static [mut ]`, `const`, `struct`, `enum`, `union`.
  - **D-13-4.5** [LOCKED] -- Layout: always multi-line when ≥1 attribute renders, one-line bare `kind $path` otherwise.
  - **D-13-4.6** [LOCKED] -- Building-block-vs-composer architectural division: stage 13 owns NEW BUILDING BLOCKS (`render_ty`, `render_item`, body-level renderers); stage 14 owns COMPOSERS (`render_const`, `render_instance`, top-level `render`). Inherited building blocks: `render_def_path` (stage 12 per D-12-6), `render_def_path_value` + `render_span` (stage 1, shipped). `render_item` stays a public stage-13 API entry per D-12-K.1.

- **D-13-5** [LOCKED] -- `render_ty` per-arm specifications across the closed 19-variant Ty enum.
  - **D-13-5.1** [LOCKED] -- Per-variant rendering table for all 19 Ty variants.
  - **D-13-5.2** [LOCKED] -- `Ty::Dynamic` = `dyn <render_def_path(vtable_item)> + AutoTrait + AutoTrait`; auto-traits in §10.1 sorted order; empty `auto_traits` omits `+` suffix.
  - **D-13-5.3** [LOCKED] -- `render_generic_args` = bare `<...>` non-empty, empty string when args empty; per-arg dispatch Type → render_ty, Const → render_const (stage-14 forward-call); GenericArgs is closed 2-variant per spec.md §9.1, no Lifetime arm.
  - **D-13-5.4** [LOCKED] -- Lifetime suppression carve-out: non-issue (RegseqModel carries Provenance, not regions).
  - **D-13-5.5** [LOCKED] -- Forward-call to stage-14 `render_const` for `GenericArg::Const` arms.
  - **D-13-5.6** [LOCKED] -- GenericArgs structural verification (verified at R1 against spec.md §9.1).

- **D-13-6** [REOPENED] -- Body-level renderer per-arm specifications across 9 public renderers + 1 internal helper. (revised in stage 14 D-14-4.9: `render_operand` and `render_rvalue` signatures extended with `expected_ty: TyId`; `render_generic_args` signature extended with `item: ItemId`. Operand-callsite wiring threads consumer-context Ty through to the pair-form `render_const(id, expected_ty)` per D-14-4.1; the per-arm specs unchanged otherwise.)
  - **D-13-6.1** [LOCKED] -- `render_place` = rustc pre/post-wrap projection chain preserving operator precedence; field/variant name resolution via place_ty walk through layout.kind; FieldLayout.name + VariantLayout.name non-optional StringId per §11.
  - **D-13-6.2** [LOCKED] -- `render_operand` = bare `<render_const>` for `Constant` (rustc-converged).
  - **D-13-6.3** [LOCKED] -- `render_rvalue` = 9-variant table + AggregateKind 7-variant sub-table; CastKind ambiguous-only annotation (§7.3.1); RawPtr aggregate two-operand form per §21.3.6.
  - **D-13-6.4** [LOCKED] -- `render_statement` = 3-variant table; SetDiscriminant variant name from place_ty's LayoutKind::Enum.shape.
  - **D-13-6.5** [LOCKED] -- `render_terminator` + sub-renderers: 6-variant TerminatorKind; output ends with `]` (no trailing `;`); `render_call_callee.Direct(id)` = `<path><instance.generic_args>` via Instance lookup; `render_assert_kind(&AssertKind, &Body)` direct adoption of rustc_public's 8-template patterns (BoundsCheck, Overflow, OverflowNeg, DivisionByZero, RemainderByZero, MisalignedPointerDereference, NullPointerDereference, InvalidEnumConstruction).
  - **D-13-6.6** [LOCKED] -- `render_basic_block` = `bbN: { stmt; ...; terminator; }` envelope; 4-/8-space indentation; trailing `;` after terminator appended at this level.

- **D-13-7** [LOCKED] -- Worked-example specimens for snapshot stability: paired ctor-input + output-text format; ~75 specimens covering ≥1 per public renderer + ≥1 per closed-enum-variant at per-arm spec level (with policy-bucket exception for decoration-only variants); inline in pretty-printer.md §9; 1:1 mirrored to insta snapshots at end-of-phase-1 implementation sweep.
  - **D-13-7.1** [LOCKED] -- Specimen format = paired blocks (ctor input + rendered output).
  - **D-13-7.2** [LOCKED] -- Specimen budget ~75 covering per-arm spec level.
  - **D-13-7.3** [LOCKED] -- Specimen location = inline in pretty-printer.md §9 per D-13-9.
  - **D-13-7.4** [LOCKED] -- Insta-snapshot 1:1 mirroring at end-of-phase-1 implementation sweep.

- **D-13-8** [CLOSED-BY-ABSORPTION] -- §18.1 sigil-table review + §18.2 ownership-table consolidation (absorbed into D-13-9 §3 + §8).

- **D-13-9** [LOCKED] -- Pretty-printer spec location: full pretty-printer spec moves to top-level companion file `pretty-printer.md`; `spec.md` §18 becomes one-paragraph pointer; methodology rule extension to README §5.9 rule 6 + §5.5 step 5/6/7 + §4 folder layout.
  - **D-13-9.1** [LOCKED] -- Scope = full pretty-printer spec moves to `pretty-printer.md`.
  - **D-13-9.2** [LOCKED] -- Cross-references: drop deferred-renderer rows in spec.md §15 / §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9 / §25.x; replace with one-line "Rendering: see pretty-printer.md §<N>" pointers.
  - **D-13-9.3** [LOCKED] -- Methodology rule extension: README §5.9 rule 6 (pretty-printer spec lives in pretty-printer.md); §5.5 step 5 amendment (pretty-printer content directs to pretty-printer.md); §5.5 step 6 amendment (post-merge review covers every co-equal artifact); §5.5 step 7 + §5.9 freeze rule extension (stage-N/pretty-printer-snapshot.md freezes alongside spec-snapshot.md); §4 folder layout adds pretty-printer.md entry.
  - **D-13-9.4** [LOCKED] -- pretty-printer.md initial 10-section structure (Mission / Master style / Body envelope+sigils / Span-debuginfo modes / Item-level shape / render_ty / Body-level rendering / Ownership table / Worked-example specimens / Cross-references).
  - **D-13-9.5** [LOCKED] -- Working surface = stage-13/pretty-printer-snapshot.md for the new top-level file's draft + minimal in-place edits to existing spec-snapshot.md for cross-references.
  - **D-13-9.6** [LOCKED] -- D-13-8 closes by absorption.

## Stage 14 (Phase-1 foundation extensions)

- **D-14-1** [LOCKED] -- Q7 intrinsic recognition mechanism + storage. Recognition via `tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])` (multi-segment tool-attribute walker); resolved `IntrinsicKind` stored on `RegseqModel.domain_attrs` (the DomainAttr sibling-table architecture per D-14-3) under the `item_attrs: IndexVec<ItemId, Vec<AttrData>>` table as `AttrData::Intrinsic(IntrinsicKind)` entries per Q2 eager-translation; closed vocabulary; unknown rejected. Folds external-review Finding #1 (corrected `intrinsic_raw` mis-attribution + the "not stored on Item" framing).
  - **D-14-1.1** [LOCKED] -- Recognition codepath (`tcx.get_attrs_by_path`).
  - **D-14-1.2** [LOCKED] -- `register_tool(rseq)` validation: declaration is source-author-side per README §2 "Source declares its own compiler-relevant attributes"; bridge does not inject.
  - **D-14-1.3** [LOCKED] -- NAME parsing: `Attribute::value_str()` extraction; closed-set validation against the `IntrinsicKind` enum.
  - **D-14-1.4** [LOCKED] -- `IntrinsicKind` closed enum, diagnostic-logging entries only at stage 14; phase-2 stages 15-19+ append per-domain entries.
  - **D-14-1.5** [LOCKED] -- Storage shape: resolved `IntrinsicKind` lifts into the unified DomainAttr architecture (D-14-3) rather than a separate per-Item field — entries land in `RegseqModel.domain_attrs.item_attrs: IndexVec<ItemId, Vec<AttrData>>` as `AttrData::Intrinsic(IntrinsicKind)` variants (resolved kind only; no `source_item` redundant with IndexVec key; no `name: StringId` needed since `kind` is sufficient).
  - **D-14-1.6** [LOCKED] -- Instance-side pull-through helper `instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind>` added to §3.5.2 (13th helper post-D-14-7 deletion of `instance_is_naked`); returns the borrowed `AttrData::Intrinsic` payload via the DomainAttr `domain_attrs.item_attrs` lookup.
  - **D-14-1.7** [LOCKED] -- Population timing: stage-12 D-12-5 Phase 2 Translation+Synthesis, attached to the Item discovery walk.
  - **D-14-1.8** [LOCKED] -- Spec home: new `RegseqModel` field declared in §3 + new §3.6 covering DomainAttr architecture (shared with D-14-3).

- **D-14-2** [LOCKED] -- Diagnostic-logging intrinsic vocabulary + bridge contract.
  - **D-14-2.1** [LOCKED] -- Per-level intrinsics (`log_debug`, `log_info`, `log_warn`, `log_error`) rather than single intrinsic with a level operand: IR-side dispatch on `InstanceId` without operand inspection; backends are free to consolidate at the C-emission layer (the generic-C backend uses a single `printf` emitter with the severity rendered as a leading bracketed format-string tag per `generic-c-backend.md` §8.1).
  - **D-14-2.2** [LOCKED] -- Argument shape: variadic `&str` + value-args (`i32`, `u32`, `i64`, `u64`, `usize`, `isize`, `&str`); format-string convention is rustc-`format_args!`-style.
  - **D-14-2.3** [LOCKED] -- Vocabulary admission via `IntrinsicKind::Log(LogLevel)` variant.
  - **D-14-2.4** [LOCKED] -- Backend lowering contract: generic-C lowers to `printf("[<LEVEL>] " <fmt> "\n", <args>...)` (single emitter, stdout, severity rendered as a leading bracketed tag); Linux backend lowers to `printk(...)` (phase-2 stage 15+ per the `stage-15/expectations.md` inbound binding).
  - **D-14-2.5** [LOCKED] -- DiagLogPeel bridge translation contract: at Call-translation phase, peel the `format_args!` macro-expanded `&[ArgumentV1]` slice argument into the flat-arg shape the diagnostic-logging C lowering admits. **REOPENS D-9-10** (B-4 and B-6 invariants carved out for `IntrinsicKind::Log(_)` callees per `I-DiagLog-CallArity`).
  - **D-14-2.6 through D-14-2.9** [LOCKED] -- Macro-vs-direct DSL surface, format-string parsing constraints, lookup table population, validator hook integration.

- **D-14-3** [LOCKED] -- DomainAttr sibling-table architecture.
  - **D-14-3.0** [LOCKED] -- Operational-intrinsic + declarative-property surfaces share one DomainAttr architecture (unified, not parallel): single sibling-table machinery dispatched by `AttrData` variant.
  - **D-14-3.1** [LOCKED] -- Key-kind set: `ItemId` (intrinsics + per-Item DSL attributes), `FieldPath` (per-field MMIO/atomic attributes), `Span` (call-site MMIO/atomic markers); `InstanceId` not used (Item-side pull-through covers all Instance-level access).
  - **D-14-3.2** [LOCKED] -- Storage shape: `DomainAttrTables { item_attrs: IndexVec<ItemId, Vec<AttrData>>, field_attrs: HashMap<FieldPath, Vec<AttrData>>, span_attrs: HashMap<Span, Vec<AttrData>> }` -- each sibling-table entry holds a `Vec<AttrData>` to allow multiple domain attributes per key; per-family duplicate invariants handled by validation.
  - **D-14-3.3** [LOCKED] -- One sibling table per key kind with a polymorphic `AttrData` payload (rather than per-family-x-key-kind tables): three tables on `DomainAttrTables` (`item_attrs`, `field_attrs`, `span_attrs`); each carries `Vec<AttrData>` so multiple domain attributes can attach to a single key; phase-2 stages append new `AttrData` variants per family.
  - **D-14-3.4** [LOCKED] -- Population timing: stage-12 D-12-5 Phase 2 Translation+Synthesis. Validation contract: referenced-key existence, no-duplicate entries, per-family attribute-shape checks.

- **D-14-4** [LOCKED] -- `render_const(ConstId, TyId) -> String` composer renderer per-arm specs.
  - **D-14-4.1** [LOCKED] -- API signature: pair-form `(ConstId, TyId)`.
  - **D-14-4.2 / D-14-4.10 / D-14-4.11 / D-14-4.12** [LOCKED] -- Scalar variants: underscore-decimal convention + sentinel collapsing (T::MIN / T::MAX) + decimal-only + non-finite forms (NaN, infinity).
  - **D-14-4.3 / D-14-4.7 / D-14-4.8** [LOCKED] -- ZST + NullPtr + Reloc per-arm dispatches.
  - **D-14-4.4 / D-14-4.5 / D-14-4.6** [LOCKED] -- Aggregate (Array/Tuple/Struct) + EnumValue per-variant + Bytes (uninit / byte-array / structural fallback) multi-arm dispatches.
  - **D-14-4.9** [LOCKED] -- Type-context threading via `expected_ty: TyId` parameter on `render_operand` and `render_rvalue`; `item: ItemId` parameter on `render_generic_args`. **REOPENS D-13-6** for the body-level renderer signature extensions.

- **D-14-5** [LOCKED] -- `render_instance(InstanceId) -> String` composer renderer per-variant specs.
  - **D-14-5.1** [LOCKED] -- API signature: single-argument (`InstanceKind` carries enough context).
  - **D-14-5.2** [LOCKED] -- Function variant envelope + body-vs-declaration dispatch.
  - **D-14-5.3 / D-14-5.4** [LOCKED] -- Static / Const variant forms.
  - **D-14-5.5** [LOCKED] -- ADT variants (Struct/Enum/Union) Option B (full definition body from §11 Layout); EnumShape dispatch.
  - **D-14-5.6** [LOCKED] -- DomainAttr attribute weaving: two-stream Stream-1 (Item-flag) + Stream-2 (`domain_attrs.item_attrs`) interleaving in canonical order.
  - **D-14-5.7** [LOCKED] -- Layering boundary: composer-from-scratch (no `render_item` wrapper); §5.6 shared per-flag helper.
  - **D-14-5.8** [LOCKED] -- Cross-stage friction outcome: zero new stage-13 D-tag REOPENs.

- **D-14-6** [LOCKED] -- Top-level `render(&self) -> String` composer.
  - **D-14-6.1** [LOCKED] -- API signature: zero-argument.
  - **D-14-6.2** [LOCKED] -- Iteration over `model.instances.iter_enumerated()` in canonical F4-2 order per `spec.md` §16.3.
  - **D-14-6.3** [LOCKED] -- Per-Instance separator `"\n\n"`; first-flag pattern; no leading/trailing separator.
  - **D-14-6.4** [LOCKED] -- File-level envelope: no header/trailer/wrapper; no trailing newline.
  - **D-14-6.5** [LOCKED] -- Empty-model case: empty `String` `""`.
  - **D-14-6.6** [LOCKED] -- Cross-stage friction outcome: zero new stage-13 D-tag REOPENs.

- **D-14-7** [LOCKED] -- `#[naked]` rejection in v1 (external-review Finding #2).
  - **D-14-7.1** [LOCKED] -- Reject `#[naked]` at Item translation; diagnostic recommends phase-2 stage-17 `#[rseq::interrupt(...)]` deferral path.
  - **D-14-7.2** [LOCKED] -- Recognition API: `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)` during stage-12 D-12-5 Phase 2.
  - **D-14-7.3** [LOCKED] -- Rejection-table placement: new "Item translation" phase row at §15.1; sibling to stage-9 D-9-9 §23.6.1 row 8.
  - **D-14-7.4 + D-14-7.5** [LOCKED] -- `Item.is_naked: bool` field + `instance_is_naked` helper deletions.
  - **D-14-7.6** [LOCKED] -- Pretty-printer per-flag table edits (§5.3 + §5.6 + §5.1 + §5.2).
  - **D-14-7.7** [LOCKED] -- Specimen alignment at §9.9 (Specimens 69 + 70 + 75).
  - **D-14-7.8** [LOCKED] -- §3.6.6 paragraph 3 finalisation.
  - **D-14-7.9** [LOCKED] -- Cross-stage friction outcome: three REOPENs at merger close (**REOPENS D-12-1.3** + **D-9-9** + **D-13-4.3**).

- **D-14-8** [LOCKED] -- Stale `emission_name` forward-pointer retarget (external-review Finding #3). §9.2 paragraph 3's `Struct / Enum / Union` `emission_name` bullet retargets the stale "stage 13 (codegen)" forward pointer to "phase-1 commits to determinism + uniqueness only; the full normative algorithm ... is the C-backend spec's responsibility and is out of phase-1 scope." Pure spec hygiene; no sub-decisions; no D-tag flip.

- **D-14-9** [LOCKED] -- Specimen 70 split (external-review Finding #4). Pretty-printer §9.9 Specimen 70 downshifts `opaque_handle_eligible: true` to `false` (Function ItemKind structurally ineligible per §8.0 invariant 19); new Specimen 70b carries `opaque_handle_eligible: true` on a non-generic ZST `repr(C)` Struct. Pure pretty-printer-spec hygiene; no sub-decisions; no D-tag flip on D-13-7.

- **D-14-10** [LOCKED] -- Generic-C backend codegen carve-out: separate co-equal merger artifact `generic-c-backend.md` per README §5.9 rule 7 (companion to `spec.md` + `pretty-printer.md`). Phase-1 backend artifact covers hosted userspace, libc, `printf`, `main()`, host-platform integer conventions; intrinsic C-emission contracts for the diagnostic-logging surface; per-`Ty` C typedef tables.
  - **D-14-10.1 through D-14-10.7** [LOCKED] -- Carve-out scope + §5.9 rule 7 amendment + initial 9-section structure for `generic-c-backend.md` (`## 1`-`## 9`).

- **`core::ffi::c_void` rejection enforcement** [LOCKED, no D-tag] -- Stage-3 D16 already specifies the rejection at the decision level; stage 14 adds the §15.1 introduction enforcement-narrative paragraph (naming §15.0 dispatch-ordering gate as the algorithmic enforcement for ADT-backed special-ADT rejections; non-Adt TyKind rejections enforced at top-level TyKind match-arm dispatch) plus an inline opaque-struct-pattern definition (`#[repr(C)]` ZST struct with single private zero-length-array field). No D-tag allocated; no D-tag flip.
