# RegseqModel Specification

Live snapshot of the RegseqModel spec — what RegseqModel IS today,
organised by topic. History (audit trail of decisions, alternatives
considered, reasoning) lives in `discussions.md`. Per-stage
contributions are recorded in `stage-N/spec-snapshot.md` (frozen
at stage close) and merged here by the merger contract.

This document is updated when a stage closes and the merger
integrates that stage's contribution. See `README.md` Section 5.9
for the update guidelines and Section 5.5 for the merger contract.

Currently reflects: stage 1 (foundation) + stage 3 (type system)
contributions.

**Snapshot scope.** This document fully specifies the stage-1
foundation and the stage-3 type-system shape. Several IR
constructs reference value-level constants (vtable instance
statics, string literals backing `&str`, static initialisers,
aggregate constants, function-pointer relocations) whose full
canonicalisation depends on stage-4's `Const` /
`ConstIdentityKey` schema (see §9.7). A `RegseqModel`
containing only stage-3-scope content (type definitions,
function signatures, scalar const generics, scalar / array-
length constants) is fully serialisable and validatable by
this document alone. Models containing the richer value-level
constants listed above are partial until stage 4 lands.

---

## 1. Mission and IR posture

RegseqModel is the internal IR of the Regseq Rust-to-C compiler
for OS-agnostic device drivers. The bridge consumes rustc's
`optimized_mir` (with `mir_opt_level=0` and a curated cleanup-pass
whitelist) and produces RegseqModel: an owned, rustc-free,
single-level IR that downstream consumers (analyses, codegen)
walk to produce C output for target OSes (Linux, Zephyr, ...).

### 1.1 Architectural posture (Q1-Q7)

The architectural decisions binding the spec live in
`architecture.md`. One-line summaries:

- **Q1.** Two-crate split: bridge inside `regseq-driver`;
  RegseqModel in `regseq-model` with zero rustc deps.
- **Q2.** Eager translation. Bridge produces a fully-owned
  RegseqModel; `'tcx` does not survive past the bridge call.
- **Q3.** Single-level IR; stages refine the same IR, not a
  layered ladder.
- **Q4.** rustc integration via `rustc_driver::Callbacks`.
- **Q5.** Tests + serialization: `insta` snapshots + custom
  textual MIR-inspired pretty-printer (primary) + serde JSON
  (secondary).
- **Q6.** `regseq` facade is a separate crate; three-tier wrapper
  dispatch (`PassthroughCallbacks` / `RegseqCallbacks` /
  `RegseqAwareCallbacks`); regseq-aware crates declared via
  `[package.metadata.regseq]`.
- **Q7.** Intrinsic recognition by `#[rseq::intrinsic("NAME")]`
  attribute; closed vocabulary; unknown rejected.

### 1.2 Methodology rules (binding)

These rules govern every part of the IR. Full text in `README.md`
Section 2:

- `core` is opaque (lang-item / canonical-name recognition).
- No defensive fallbacks (toolchain pinned).
- Re-derive from evidence; do not import baggage.
- Codex review for non-trivial decisions.
- Spec is mutable.
- Compiler-theory-first.
- Source declares its own compiler-relevant attributes.
- C semantics with information preservation (store rustc's
  resolved result, not the input).
- Code does not reference discussion-folder paths.
- Forward expectations live in `stage-N/expectations.md`.
- Per-stage prior-art research uses primary sources.

---

## 2. ID system

RegseqModel uses typed integer IDs uniformly: every IR cross-
reference is an opaque ID indexing into a table on the model. IDs
are u32-backed with niche optimisation; the model holds one table
per ID kind.

### 2.1 The `Idx` trait

```rust
pub trait Idx: Copy + Eq + Hash + Ord + Debug + 'static {
    fn new(idx: usize) -> Self;
    fn index(self) -> usize;
}
```

Mirrors `rustc_index::Idx`. `Ord` is required for stable dump
ordering. `'static` keeps the owned-IR invariant (Q2) explicit at
the trait boundary.

### 2.2 Backing representation

All IDs are `u32` with niche optimisation so `Option<Id>` is 4
bytes. The implementation may use `NonZeroU32` shifted by one, a
`NonMaxU32` newtype, or rustc-style top-256 reservation. 4 billion
items / types / consts is far beyond any realistic driver crate;
`u16` would be too tight (basic blocks of large bodies routinely
exceed 16K).

### 2.3 The `newtype_index!` analogue

A macro generates each typed ID:

```rust
newtype_index! {
    pub struct ItemId {}
}
```

Expansion: a struct with a `u32` (or niche-equivalent) field; an
`Idx` impl; derives for `Copy, Clone, Eq, PartialEq, Ord,
PartialOrd, Hash, Debug`; an associated `pub const ZERO: Self =
Self::from_u32(0);` constant for the index-0 sentinel pattern;
and an inherent `pub const fn from_u32(idx: u32) -> Self` for use
in const contexts.

The IR-side stability contract is that all IDs satisfy `Idx +
Serialize + Deserialize + Hash + Ord` and expose the `ZERO`
const.

#### Serde rules

`Serialize` / `Deserialize` must NOT be field-derived (would leak
the niche-encoded inner) and must NOT use `#[serde(transparent)]`.
Each ID newtype implements serde with the on-wire shape `u32 =
Idx::index()` via either:

1. Hand-rolled `impl Serialize` / `impl Deserialize`.
2. `#[serde(into = "u32", try_from = "u32")]` plus `From<Id> for
   u32` and `TryFrom<u32, Error = E> for Id` where `E: Display`.
   The infallible `from = "u32"` form is NOT acceptable.

Both routes must round-trip the logical index and reject niche-
violating input at deserialize time with a clear error.

### 2.4 ID inventory

| ID             | Width | Indexes into           | Scope          |
| -------------- | ----- | ---------------------- | -------------- |
| `CrateId`      | u32   | `model.crates`         | crate-level    |
| `StringId`     | u32   | `model.strings`        | crate-level    |
| `SourceFileId` | u32   | `model.source_files`   | crate-level    |
| `TyId`         | u32   | `model.types`          | crate-level    |
| `ConstId`      | u32   | `model.consts`         | crate-level    |
| `ItemId`       | u32   | `model.items`          | crate-level    |
| `BodyId`       | u32   | `model.bodies`         | crate-level    |
| `FnSigId`      | u32   | `model.sigs`           | crate-level    |
| `InstanceId`   | u32   | `model.instances`      | crate-level    |
| `LocalId`      | u32   | `Body.local_decls`     | **body-local** |
| `BlockId`      | u32   | `Body.basic_blocks`    | **body-local** |
| `VariantIdx`   | u32   | `VariantLayout` arrays | layout-local   |

Body-local IDs (`LocalId`, `BlockId`) are scoped to a single
`Body`. Crate-level IDs are unique within the RegseqModel.

### 2.5 Body-local ID constants

Two body-local IDs are reserved by rustc convention:

```rust
impl LocalId  { pub const RETURN_PLACE: LocalId = LocalId::from_u32(0); }
impl BlockId  { pub const START_BLOCK: BlockId = BlockId::from_u32(0); }
```

`LocalId(0)` is the implicit return slot of every body;
`BlockId(0)` is the entry block.

### 2.6 `IndexVec<I, T>`

Canonical dense storage indexed by a typed ID. Mirrors rustc's
`IndexVec`. Spec requires:

- `IndexVec<I: Idx, T>::push(value: T) -> I` returns the assigned
  index.
- `iter()`, `iter_mut()`, indexing by `I`.
- Length-as-`I::index` semantics.
- `Serialize` / `Deserialize` produce a flat array of `T` on the
  wire (no per-element key, no `PhantomData<I>` field).

---

## 3. Top-level `RegseqModel`

The owned root struct returned by `bridge::extract(tcx) ->
RegseqModel`. All other tables hang off it.

```rust
pub struct CrateInfo {
    pub name:            StringId,        // e.g., "pl011-spec", "regseq"
    pub stable_crate_id: StableCrateId,   // rustc's StableCrateId (u64 wrapper)
}

pub struct TargetInfo {
    pub triple:          StringId,    // e.g., "x86_64-unknown-linux-gnu"
    pub pointer_width:   u8,          // bits; one of {16, 32, 64}
    pub endian:          Endian,
    pub data_layout:     StringId,    // LLVM data-layout string
    pub scalar_layouts:  ScalarLayouts,
}

pub enum Endian { Little, Big }

pub struct ScalarLayouts {
    pub bool_:    SizeAlign,                       // §11.5
    pub char_:    SizeAlign,
    pub int:      [SizeAlign; INT_KIND_COUNT],     // indexed by IntKind discriminant
    pub uint:     [SizeAlign; UINT_KIND_COUNT],    // indexed by UintKind discriminant
    pub float:    [SizeAlign; FLOAT_KIND_COUNT],   // indexed by FloatKind discriminant
    pub fn_ptr:   SizeAlign,                       // function pointer width/align
    pub data_ptr: SizeAlign,                       // data pointer width/align
}

pub struct BuildInfo {
    /// Pinned toolchain identifier carrying enough information to
    /// identify the exact rustc whose layout, MIR, and type
    /// behaviour produced this model. At minimum: channel + date
    /// (for nightlies) or version string; commit hash when
    /// available; host triple. Example: the project's
    /// `nightly-2026-02-01-aarch64-apple-darwin`. Stable across
    /// builds satisfying the build-equivalence quad (§12.4).
    pub toolchain:           StringId,
    /// 128-bit content hash of the normalised extraction options.
    /// Construction algorithm pinned below.
    pub bridge_options_hash: u128,
}

/// Tag carried per-`ItemId` distinguishing source-level items
/// from bridge-synthesised items by family. Side table on
/// `RegseqModel` (parallel to `items`).
///
/// Normative `SyntheticItemKind`, `ClosureCallKind`, and
/// `PrincipalTraitKey` definitions live in §13.3. This file
/// references the type by name; the enum body is **not**
/// duplicated here to avoid drift.
pub enum SyntheticItemKind { /* ... see §13.3 ... */ }

pub struct RegseqModel {
    // Crate inventory.
    pub crates:        IndexVec<CrateId, CrateInfo>,     // crate inventory
    pub primary_crate: CrateId,                          // points at the primary spec crate

    // Target context. Layout values stored in `types` are valid
    // only for this target. Determinism (§16) is parameterised
    // over `target` equality.
    pub target: TargetInfo,

    // Build provenance. Records the pinned-toolchain identifier
    // and a hash of the normalised bridge extraction options.
    // Lets a deserialised consumer verify the build-equivalence
    // quad (§12.4) post-hoc.
    pub build: BuildInfo,

    // Interning tables. `source_files[0]` is the synthesised-source sentinel.
    pub strings:      StringInterner,
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    // Content-keyed interners. Same BiMap pattern as `types` and
    // `strings`: identity is the value's canonical content, not
    // allocation order. See §12 (TyInterner), §14.4 (FnSigInterner),
    // §9.7 (ConstInterner).
    pub types:  TyInterner,
    pub consts: ConstInterner,
    pub sigs:   FnSigInterner,    // every `InstanceKind::Function` has an
                                  // entry; foreign decls have `body: None`
                                  // but still have a sig

    // Source-level item alphabet. Holds the alphabet for source-
    // level traceback (DefPath, kind, attributes, span). NOT the
    // emission table; emission walks `instances`.
    // `item_def_paths` is the F4 cross-build-identity side table.
    // `item_synthetic_kinds` distinguishes source-level items
    // (`None`) from bridge-synthesised items by family (§13.3).
    pub items:                IndexVec<ItemId, Item>,
    pub item_def_paths:       IndexVec<ItemId, DefPathHash>,
    pub item_synthetic_kinds: IndexVec<ItemId, SyntheticItemKind>,

    // C ABI lowering side table, parallel to `sigs`. Present iff
    // `sigs[id].abi == FnAbi::C` — records how each
    // input/output position is *passed/returned* under C ABI,
    // independent of the IR-level type identity carried in
    // `sigs[id].inputs/output`. See §14.5.
    pub sig_c_abi_lowerings: IndexVec<FnSigId, Option<CAbiLowering>>,

    // Bodies for body-carrying Instances.
    pub bodies: IndexVec<BodyId, Body>,

    // Primary monomorphised table; iteration target for analyses
    // and C codegen.
    pub instances: IndexVec<InstanceId, Instance>,

    // Bridge-side vtable dedup table. `#[serde(skip)]`; rebuilt on
    // deserialise from `items` by scanning `item_synthetic_kinds`
    // for `SyntheticItemKind::VTableType { principal }` entries
    // (see §13.2). Keyed by full `PrincipalTraitKey` (§13.3) so
    // generic trait objects like `dyn Register<u32>` and
    // `dyn Register<u64>` get distinct vtable types. Zero rustc deps.
    pub vtables: HashMap<PrincipalTraitKey, ItemId>,
}
```

`StableCrateId` is rustc's 64-bit type, mirrored as
`pub struct StableCrateId(pub u64);` since `regseq-model` has zero
rustc deps (Q1).

`TargetInfo` records the target context that made the stored
`Layout` values true. Two `RegseqModel`s built from the same
source for different targets carry different `target` and
generally different `Layout`s; cross-build determinism (§16)
holds only when both `target` values agree. `data_layout` is the
canonical LLVM data-layout string (it subsumes `pointer_width`
and `endian`; the latter two are denormalised for cheap consumer
access). The bridge populates from `tcx.sess.target` at
extraction time. No rustc handle is retained.

`BuildInfo` records the **provenance** of the model so a
deserialised consumer can verify the build-equivalence quad
(§12.4) post-hoc without needing access to the original build
environment. `toolchain` is a stable string identifier (e.g.,
the rust-toolchain.toml channel + commit, or the pinned nightly
date). `bridge_options_hash` is a stable hash over the normalised
extraction options that are not otherwise observable from the
model: the cleanup-pass whitelist (`+RemoveStorageMarkers`,
`+RemoveZsts`, etc.), `mir_opt_level=0`, and the UB-check
force-off flags. Two builds satisfying the quad have equal
`build` values; consumers comparing models check `target` and
`build` for quad equality.

`TargetInfo.scalar_layouts` stores the bridge-resolved
`SizeAlign` for every primitive scalar type the IR admits,
populated from rustc at extraction time. This is "store the
result, not the input": consumers query
`model.target.scalar_layouts.*` directly rather than parsing
the LLVM data-layout string. `data_layout` remains for
diagnostics and target identity; `scalar_layouts` is the
authoritative source for `size_align_of` (§11.5) on primitive
types. The `INT_KIND_COUNT`, `UINT_KIND_COUNT`, and
`FLOAT_KIND_COUNT` constants match the variant counts of
`IntKind`, `UintKind`, and `FloatKind` (§10).

**`bridge_options_hash` construction.** The hash is computed
identically to `ArgsKey` (§12.3): the leading 16 bytes
(big-endian) of `SHA-256(canonical_bridge_options_bytes)`,
loaded as `u128` big-endian. The canonical byte encoding is:

```text
canonical_bridge_options_bytes =
    u32 LE option_count
    repeated entries, sorted ascending by option_name byte order:
        u32 LE option_name_byte_len
        option_name UTF-8 bytes
        u32 LE option_value_byte_len
        option_value UTF-8 bytes
```

The option set covered (must be exhaustive of any extraction
flag that affects MIR shape or model contents):

- `mir_opt_level` (decimal value, e.g., `"0"`).
- `cleanup_pass_whitelist` (comma-separated normalised pass
  names sorted alphabetically, e.g.,
  `"+RemoveStorageMarkers,+RemoveZsts,+SimplifyLocals-final,+ReorderBasicBlocks,+ReorderLocals"`).
- `ub_check_force_offs` (sorted comma-separated names of
  disabled UB checks).
- Any future extraction flag added by the bridge — appending
  changes `bridge_options_hash` for builds that toggle the
  flag, satisfying the build-equivalence quad invariant (§12.4).

Same hash family (SHA-256 truncated) used for `ArgsKey` and
`bridge_options_hash` ensures portability and one normative
construction.

### 3.1 Construction invariants

The bridge guarantees:

1. **Tables are populated before they are referenced.** Every
   `CrateId`, `TyId`, `ConstId`, `ItemId`, `BodyId`, `InstanceId`,
   `FnSigId`, `SourceFileId`, `StringId`, `VariantIdx` resolves to
   a valid index in its table. Debug-build assertion.
2. **`item_def_paths` and `item_synthetic_kinds` mirror `items`.**
   Both side tables have `len() == items.len()`. For source-level
   items, `item_def_paths[item_id]` is rustc's `DefPathHash` of
   the originating `DefId` and `item_synthetic_kinds[item_id] ==
   SyntheticItemKind::None`. For bridge-synthesised items,
   `item_def_paths` carries the bridge-derived stable hash
   defined in §13.4, and `item_synthetic_kinds` carries the
   structured kind tag (§13.3). **Carve-out:**
   `SyntheticItemKind::ClosureCaptures` is a synthetic-kind
   discriminator on a *source-derived* Item (the closure
   expression has its own rustc `DefId` per §13.1), so for
   `ClosureCaptures` Items, `item_def_paths[item_id]` is rustc's
   `tcx.def_path_hash(closure_did)`, **not** a synthetic hash.
   All other non-`None` synthetic kinds use synthetic
   `DefPathHash`es from §13.4.
3. **Items are sorted at construction.** Per F4 (§16), items are
   placed in `items` and `item_def_paths` in the order produced
   by the F4 sort comparator. Two builds satisfying the
   build-equivalence quad (§12.4 — equal source, pinned toolchain,
   equal `target`, equal bridge options) produce identical
   `ItemId` assignments. Builds outside the quad are aligned by
   `DefPathHash` (§7.3), not by sequential `ItemId`.
4. **`primary_crate` is valid and corresponds to the spec
   crate.** `primary_crate.index() < crates.len()` and
   `crates[primary_crate].stable_crate_id` matches the bridge
   input's `spec_crate_stable_id`.
5. **`crates[c].stable_crate_id` is unique across `c`.** No two
   `CrateInfo` entries share a `StableCrateId`. Additionally,
   `crates` is sorted by `(crate_name, stable_crate_id)` before
   `CrateId` assignment — two builds satisfying the
   build-equivalence quad (§12.4) produce identical `CrateId`
   values. Builds outside the quad are aligned by
   `StableCrateId`, not by sequential `CrateId`.
6. **Every `DefPath.crate_id` resolves to a valid entry.** For
   every item, `item.def_path.crate_id.index() < crates.len()`.
7. **No `'tcx` references.** The model is fully owned and
   `'static`-compatible; serialisation round-trips losslessly.
8. **Synthesised source-file sentinel.** `source_files[0]` is
   the synthesised sentinel: `path` interned as `"<synthesized>"`,
   `content: None`, `line_starts: vec![0]`. Real source files
   start at `SourceFileId(1)`.
9. **Instance closure-completeness.** The bridge computes the
   monomorphisation closure from a settled root set (full rustc
   mono-item closure for the primary crate plus all
   regseq-aware dependency crates per Q6, plus all synthesised
   artefacts those mono-items require). Every `InstanceId`
   referenced anywhere in the model resolves to a valid entry.
10. **Body presence matches kind.** Every `Instance` with `kind:
    InstanceKind::Function { body: Some(b) }` resolves `b` to a
    valid `Body`. `body: None` is reserved for foreign
    declarations.
11. **Content-keyed interner uniqueness.** `types` (§12), `sigs`
    (§14.4), and `consts` (§9.7) are content-keyed: no two
    distinct IDs map to canonically-equal values. This is what
    makes `Ty::FnPtr(FnSigId)` and `GenericArg::Const(ConstId)`
    sound under structural type identity (§12.1) — equal canonical
    content always implies equal ID. Strings follow the same
    rule via `StringInterner` (§4.1).
12. **Unsized marker placement.** `Ty::Str`, `Ty::Slice`, and
    `Ty::Dynamic` appear only as the pointee of a `Ty::Ref` or
    `Ty::Ptr`. The bridge rejects bare unsized markers in any
    other position (struct field, tuple element, fn argument or
    return slot, generic arg, etc.) at type translation.
13. **`Ty::Dynamic` well-formedness.** `auto_traits` is sorted
    ascending by `AutoTrait` discriminant order with no
    duplicates. A principal trait is **required** (auto-only
    trait objects like `dyn Send` are rejected at §15.1).
    `vtable_ty` resolves to the synthesised vtable `Ty::Struct`
    for the trait object's full `PrincipalTraitKey` (§13.3 —
    `(trait_path, trait_args, projection_bindings)`), whose
    Item carries `SyntheticItemKind::VTableType { principal:
    PrincipalTraitKey }`.
14. **`InstanceKind` layout consistency.** For
    `InstanceKind::{Struct, Enum, Union} { layout }`, the
    referenced `TyId` resolves to a `Ty` of the matching nominal
    variant (`Ty::Struct` / `Ty::Enum` / `Ty::Union`) whose
    `(item, args)` equals `(Instance.item, Instance.generic_args)`.
15. **`TargetInfo` well-formedness.** `target.triple` and
    `target.data_layout` are non-empty interned strings;
    `target.pointer_width` is one of {16, 32, 64}; the
    `target.pointer_width` and `target.endian` values agree with
    the parsed `target.data_layout`. `target.scalar_layouts`
    populated for every primitive scalar (Bool, Char, all
    Int/Uint/Float widths, fn ptr, data ptr) with sizes and
    alignments matching `target.data_layout` and rustc's
    target-specific scalar layouts. All values powers of two
    where required by ABI. Bridge takes everything from
    `tcx.sess.target` at extraction; no consumer-side recomputation.
16. **C ABI admissibility outcome and lowering record.** For
    every `FnSig` with `abi: FnAbi::C`, every type referenced
    by `inputs` and `output` was C-ABI-admissible at bridge
    time per the §14.5 rustc-side predicate.
    `FnSig.inputs`/`output` preserve the **original Rust
    `TyId`s** — this keeps rustc-distinct function types
    distinct in the IR per D15 (e.g.,
    `extern "C" fn(Handle) -> Handle` and
    `extern "C" fn(u32) -> u32` have distinct `FnSigId`s even
    when `Handle` is `repr(transparent)` over `u32`).
    The C-side lowering shape per input/output position
    (`Direct` or `Substituted { emit_as, reason }`) is recorded
    in `sig_c_abi_lowerings[id]: Some(CAbiLowering)` (§14.5).
    For `FnAbi::Rust` FnSigs, `sig_c_abi_lowerings[id]` is
    `None`. `sig_c_abi_lowerings.len() == sigs.len()`.
17. **`BuildInfo` well-formedness.** `build.toolchain` is a
    non-empty interned string identifying the pinned toolchain
    used at extraction time. `build.bridge_options_hash` is
    deterministic over the normalised extraction options (§3
    description) and identical across builds satisfying the
    build-equivalence quad (§12.4).
18. **`item_def_paths` global uniqueness.** No two distinct
    `ItemId`s map to the same `DefPathHash`. Source-derived
    items use rustc's `tcx.def_path_hash(did)` (always
    non-zero `stable_crate_id`); synthetic items use the §13.4
    construction algorithm (always `stable_crate_id == 0`).
    The two spaces are domain-separated by their
    `stable_crate_id` value and cannot collide.
19. **`Item.opaque_handle_eligible` consistency (non-generic
    only).** When `Item.opaque_handle_eligible == true`, **all**
    of the following hold:
    - `Item.kind == ItemKind::Struct` (model-checkable).
    - The Item's source-side rustc `adt_def.repr()` is `C`
      (**bridge certificate** — the source repr is not stored
      in the model per §11.3, D11; presence of
      `opaque_handle_eligible == true` is the bridge's
      attestation that this fact held at construction time per
      the §3.3 validation trust model).
    - The Item is **non-generic** (model-checkable). Model-checkable criterion:
      `model.types` contains exactly one `Ty::Struct { item:
      this, args, layout }` entry, and that entry's
      `args.is_empty()`. (Generic Items would produce one
      `Ty::Struct` entry per monomorphisation, with non-empty
      `args`.) v1 restricts opaque-handle eligibility to
      non-generic Items; generic opaque-handle candidates
      (e.g., `Handle<T>`) are rejected at the bridge with
      `BridgeError::GenericOpaqueHandle`. Future revisions may
      lift the eligibility flag to a per-monomorphization side
      table (`HashMap<TyId, bool>` or per-`InstanceKind::Struct`)
      to support generic opaque handles; v1 takes the simpler
      non-generic path.
    - The Item's monomorphic `Ty::Struct.layout` (the unique
      monomorphisation, since the Item is non-generic) is ZST:
      `size: 0`, `align: 1` (**model-checkable** from the
      inline `Layout` on the unique `Ty::Struct` entry).
    - Every reachable use of the Item's monomorphic `TyId`
      across the model is behind `Ref` or `Ptr` (no by-value,
      sized-field, or `sizeof` use). **Model-checkable** via
      the formal reachable-use traversal pinned in §14.5;
      validation walks the same closed traversal domain at
      Phase B time.
    The bridge computes the eligibility outcome at construction
    time and stores it on the Item so consumers can read it
    directly without re-running the global reachability
    analysis. See §14.5 "Opaque-handle convention" for the
    full rule and §3.3 Phase B for the validation split
    between bridge-certified facts (source-side `repr(C)`)
    and model-checkable conditions (the other four).
20. **`StableCrateId(0)` reserved as synthetic-domain
    sentinel.** No real source crate carries
    `StableCrateId(0)`. The bridge validates this at extraction
    time: if `tcx.stable_crate_id(crate_num) == StableCrateId(0)`
    for any real crate, the bridge rejects with
    `BridgeError::StableCrateIdZeroCollision`. Synthetic-item
    `DefPathHash`es use `stable_crate_id == 0` (§13.4) — the
    domain separation between real and synthetic is rejection-
    enforced, not hash-probabilistic.

### 3.2 Local IDs vs stable identity

The model uses two parallel ID schemes:

| Concern | Local addressing (cheap, per-build) | Stable cross-build identity                                            |
| ------- | ----------------------------------- | ---------------------------------------------------------------------- |
| Items   | `ItemId(u32)` indexes `items`       | `DefPathHash` stored at `item_def_paths[ItemId]` (parallel side table) |
| Crates  | `CrateId(u32)` indexes `crates`     | `StableCrateId(u64)` stored on each `CrateInfo` (embedded field)       |

Local IDs are NOT stable across builds: two RegseqModels for the
same source can have identical `DefPath`s mapped to different
sequential IDs if the reachable set differs (incremental builds,
partial extraction). Stable forms (`DefPathHash`,
`StableCrateId`) are needed for cross-build alignment.

### 3.3 Model validation order

Deserialise proceeds in two phases — **parsing** loads all
storage arrays into raw memory; **validation** runs ordered
checks afterwards. Splitting the phases avoids the chicken-and-
egg of validating cross-table references before their target
tables exist.

**Validation authority and trust model.** RegseqModel is
produced by the bridge from rustc state, then consumed
downstream. JSON serialisation (per Q5) is *secondary* — the
primary serialisation is the pretty-printer, which is
output-only. There is no adversarial-input trust boundary in
the v1 pipeline: the bridge produces, codegen / analyses
consume. Deserialise validation therefore enforces:

- **Internal well-formedness**: all stored cross-references
  resolve; content-keyed interner uniqueness holds; structural
  invariants (sorted, bounded, length-matching) are preserved;
  enum and layout values are within their declared bounds.
- **Bridge-certificate shape**: stored bridge outcomes
  (`sig_c_abi_lowerings`, `Item.opaque_handle_eligible`, etc.)
  are present where required and structurally valid.

Validation **does not re-prove** rustc-side facts the bridge
already verified at construction (e.g., source-side
`adt_def.repr().c()`, the recursive C-ABI admissibility
predicate's per-step decisions). Those facts are *certificates*
in the model: their presence is the bridge's attestation that
the predicate held. A serialised RegseqModel is trusted as
bridge-produced. If a future deployment introduces an
adversarial-input boundary (e.g., an external tool consuming
JSON from untrusted sources), it must perform its own
verification on top of this validation; the v1 model does not
store the rustc-side input facts needed to re-derive bridge
certificates.

**Phase A — parsing (deserialise raw storage).**

1. Parse `strings` storage, `source_files` storage, `crates`,
   `items`, `item_def_paths`, `item_synthetic_kinds`, `target`,
   `build` into raw in-memory state.
2. Parse `consts` storage, `sigs` storage,
   `sig_c_abi_lowerings`, `types` storage, `bodies`,
   `instances` into raw in-memory state.
3. Defer rebuild of `#[serde(skip)]` transient maps (`vtables`)
   to Phase C.

After Phase A, all storage lengths are known but no cross-table
references have been checked.

**Phase B — validation (ordered by inter-table dependencies).**

Identity uniqueness in `TyInterner` (§12) depends on `FnSigId`
and `ConstId` being content-unique (invariant 11), so those
inner interners are validated first.

1. Rebuild `strings` lookup; validate content uniqueness +
   sentinel at `StringId(0) == ""`.
2. Validate `source_files`: synthesised sentinel at
   `SourceFileId(0)`; line-table consistency for non-sentinel
   entries.
3. Validate `crates`, `items` shape, `item_def_paths.len() ==
   items.len()`, `item_synthetic_kinds.len() == items.len()`;
   `target` well-formedness (invariant 15), including
   `scalar_layouts` populated; `build` well-formedness
   (invariant 17).
4. Validate `ConstInterner` content uniqueness (no duplicate
   canonical values).
5. Validate `FnSigInterner` content uniqueness (no duplicate
   canonical `FnSig`s); validate every `TyId` referenced
   inside `FnSig.inputs` / `FnSig.output` is in-bounds
   against the parsed `types` storage length. Validate
   `sig_c_abi_lowerings.len() == sigs.len()` and per-entry
   presence rule (`Some` iff `FnAbi::C`). For each present
   `CAbiLowering` entry:
   - **Arity**: `lowering.inputs.len() ==
     sigs[id].inputs.len()` (one shape per parameter slot).
   - **Shape-target reachability**: every
     `Substituted.emit_as` TyId is in-bounds.
   - **Tag-scalar primitiveness**: when
     `Substituted.reason == TagScalar` or
     `TransparentChainAndTagScalar`, the resolved
     `Substituted.emit_as` `Ty` must be `Ty::Int(_)` or
     `Ty::Uint(_)`. Other reasons admit any C-ABI-admissible
     `emit_as` type.
6. Validate `TyInterner` identity uniqueness (§12.1
   nominal/structural rule); validate every nested
   `TyId` / `ItemId` / `FnSigId` / `ConstId` / `StringId` /
   `SourceFileId` / `VariantIdx` resolves.
7. Validate `Layout` sanity per §12.5 mechanical checks.
8. Validate `instances` and `bodies`: every `Instance.item`
   and `Instance.kind`-payload IDs resolve; `InstanceKind`
   layout consistency (invariant 14); for
   `Function { sig, body, flavor }`, the
   `(InstanceKey)` projection is unique across all instances
   (invariant 16-derived) and `flavor` matches the bridge's
   F4-2 sort key (§9.3).

**Phase B (continued) — invariant enforcement beyond reachability.**

Validation is not merely bounds checking. It enforces every
construction invariant codegen relies on and that can be
checked from `RegseqModel` alone. Invariants whose underlying
rustc-side facts are not stored in the model
(e.g., source-side `adt_def.repr()` classification — the only
remaining bridge-certified-only conditions are now narrowly
the rustc repr classifications used by §14.5 admissibility
predicates and the source-side `repr(C)` certificate in
invariant 19) are represented by stored bridge outcomes;
validation checks the shape and presence of the certificate,
not the underlying rustc-side computation. Where an invariant
condition IS computable from the closed model (as the formal
reachable-use traversal in §14.5 is), validation re-runs that
condition mechanically — it is not bridge-certified-only.
Specifically, Phase B also enforces:

- **Unsized marker placement (invariant 12).** Walk every
  `Ty` and reject any `Ty::Str` / `Ty::Slice(_)` /
  `Ty::Dynamic{..}` appearing outside a `Ty::Ref` /
  `Ty::Ptr` pointee position (struct field, tuple element,
  fn argument or return slot, generic-arg position, etc.).
- **`Ty::Dynamic` well-formedness (invariant 13).** Validate
  `auto_traits` is sorted ascending by AutoTrait discriminant
  with no duplicates; validate `vtable_ty` resolves to a
  `Ty::Struct` whose Item carries
  `SyntheticItemKind::VTableType { principal }`.
- **`SyntheticItemKind` ↔ `ItemKind` consistency.** Validate
  per-family:
  - `ClosureCaptures` → `ItemKind::Struct`.
  - `ClosureCall { .. }` → `ItemKind::Function`.
  - `ClosureDropGlue` → `ItemKind::Function`.
  - `VTableType { .. }` → `ItemKind::Struct`.
  - `VTableInstance { .. }` → `ItemKind::Static`.
  - `VTableMethodShim { .. }` → `ItemKind::Function`.
  - `VTableDropShim { .. }` → `ItemKind::Function`.
  - `None` → any source-derivable `ItemKind`.
- **`item_def_paths` global uniqueness (invariant 18).** No
  two distinct `ItemId`s map to the same `DefPathHash`.
  HashSet sweep across `item_def_paths`.
- **Opaque-handle consistency (invariant 19, model-checkable
  subset).** For every `Item` with `opaque_handle_eligible
  == true`, validation enforces the **model-checkable**
  subset of invariant 19's conditions per the trust model
  above:
  - `Item.kind == ItemKind::Struct` (model-checkable).
  - **Non-generic**: exactly one `Ty::Struct { item: this,
    args, layout }` entry in `model.types` with
    `args.is_empty()` (model-checkable per the §14.5
    "Model-checkable non-generic criterion").
  - **ZST monomorphic layout**: that single `Ty::Struct`
    entry's `layout.size == 0 && layout.align == 1`
    (model-checkable from the inline `Layout`).
  - **Reachable-use traversal**: every reachable use of the
    monomorphic `TyId` is behind `Ref`/`Ptr` per the formal
    traversal in §14.5 (model-checkable — the traversal
    domain is closed and listed there).
  The remaining condition — *source-side `repr(C)`* — is a
  rustc-side fact not stored in the model (per §11.3, D11).
  Validation **does not re-prove** it; it is a bridge
  certificate per the trust model. Presence of
  `opaque_handle_eligible == true` is the bridge's attestation
  that the source repr was `C`. A consumer that needs to
  re-verify this fact (e.g., an external adversarial-input
  tool) must perform its own check outside Phase B.
- **`FnSig` invariants (§14.3).** `c_variadic == true` implies
  `abi == FnAbi::C` and `inputs.len() >= 1`; no `inputs[i] ==
  Ty::Never` for any FnSig.
- **`CAbiLowering` shape.** For each present entry:
  arity `inputs.len() == FnSig.inputs.len()` (§3.3 step 5);
  `Substituted.emit_as` resolves; `TagScalar` /
  `TransparentChainAndTagScalar` reasons require resolved
  `emit_as` Ty to be `Ty::Int(_)` or `Ty::Uint(_)`; `Direct`
  on output unit (`Ty::Tuple { fields: [] }`) and on `Never`
  is the codegen-recognised void/noreturn-void shape.
- **Layout invariants (§11.5, §12.5).** Mechanical layout
  sanity from §12.5 step 4 (variant ↔ kind matching, tuple
  field-name convention, field reachability, field-fits-in-
  layout, alignment respected); plus **non-overlap of non-ZST
  fields** (no two non-ZST `FieldLayout`s in the same struct,
  tuple, or `VariantLayout` payload have overlapping byte
  intervals); plus **packed alignment formula correctness**:
  `required_field_align = match layout.pack { None =>
  align_of(field.ty), Some(n) => min(align_of(field.ty), n) };
  field.offset % required_field_align == 0`.
- **Tag-scalar width-fit (§11.2).** Per the §11.2 width-fit
  invariant, every `valid_range.{start,end}`, `niche_start`,
  and per-`VariantLayout.tag_value` fits the bit width of
  `TagScalar.scalar_ty` (Bool=8, Char=32, Int(Ix)/Uint(Ux)
  natural, Ptr/Ref `target.pointer_width`).
- **F4-2 instance order.** Validate `instances` is sorted by
  `RegseqInstanceKey` (§9.3, §16.3) — the serialised order is
  contractual for snapshot-equivalence and deterministic
  traversal. A model with out-of-order instances is rejected.
- **`Ty::Tuple { fields: [] }` carries `UNIT_LAYOUT`.** The
  empty tuple's layout is exactly `UNIT_LAYOUT` (§11): `size:
  0, align: 1, pack: None, uninhabited: false, kind:
  LayoutKind::Struct { fields: vec![] }`. Tuple identity
  excludes layout per §12.1, so this validation is the only
  defence against a serialised model assigning a non-unit
  layout to `()`.
- **`DefPath.rendered` matches canonical rendering.** For each
  `Item.def_path`, validation re-derives the canonical
  rendered form from `crate_id` + `segments` (per §7.2 verbose
  rendering rules) and checks it equals the stored
  `rendered: StringId`. Prevents a deserialised model from
  presenting misleading diagnostic / snapshot output.
- **`VTableInstance` Items are immutable statics.** For every
  Item with `item_synthetic_kinds[id] ==
  SyntheticItemKind::VTableInstance { .. }`, validation
  checks that the corresponding Static Instance has
  `kind: InstanceKind::Static { mutability: Mutability::Not,
  init: Some(_), .. }`. Vtable instances must be immutable
  with a present initialiser; mutable or missing-init
  vtables are model corruption.
- **Source-file span bounds (sentinel-aware).** For
  `SourceFile { content: Some(_), .. }`, validation checks
  every `Span { file, lo, hi }` referencing this file has
  `lo <= hi <= content.len()` and that `lo`/`hi` lie at valid
  UTF-8 byte boundaries. For `SourceFile { content: None, ..
  }` (upstream-crate sources, the synthesised sentinel),
  validation cannot check `lo`/`hi` against content length
  but DOES check that `line_starts` is monotonic and starts
  at 0; ranges through such files are accepted as-is by
  validation but resolve to `None` via `Span::slice` per §5.4.

**Phase C — rebuild transient maps.**

9. Rebuild `vtables` by scanning `item_synthetic_kinds` for
   `SyntheticItemKind::VTableType { principal }` entries and
   re-keying as `principal -> ItemId` (§13.2). **Reject
   duplicate `principal` keys**: two `VTableType` Items
   sharing a `PrincipalTraitKey` is a model-corruption error
   (the bridge dedupes at construction, so duplicates can only
   arise from tampered serialised data). Phase C raises an
   error rather than silently overwriting.

The dependency arrows are: `consts` ← `sigs` ← `types` ←
(layout, instances). Validating in any other order can miss
invalid serialised states (e.g., a duplicate `FnSigId` would
be overlooked if `FnSigInterner` uniqueness is checked after
`TyInterner`).

---

## 4. StringInterner

```rust
pub struct StringInterner {
    storage: Vec<Arc<str>>,
    lookup:  HashMap<Arc<str>, StringId>,
}

impl StringInterner {
    pub fn new() -> Self;             // pre-interns "" at StringId(0)
    pub fn intern(&mut self, s: &str) -> StringId;
    pub fn resolve(&self, id: StringId) -> &str;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (StringId, &str)>;
}
```

Two collections (rather than one) for amortized-O(1) on both
`resolve` (vector indexing) and `intern` (hash lookup).
`Arc<str>` lets `lookup` keys and `storage` entries share
allocations.

### 4.1 Invariants

1. **Append-only.** No removal API. `StringId` assignments are
   permanent.
2. **`StringId(0) == ""`.** Pre-interned by `new()`. Used as a
   sentinel where a name is absent.
3. **Hash by content for cross-session stability.** Numeric IDs
   are allocation-order; cross-run equality is by content, not
   `StringId` value.

### 4.2 Serialisation

Custom `Serialize` / `Deserialize`. On-wire shape: a flat JSON
array of strings (`storage` only, in order). On deserialise, the
implementation reads the array, populates `storage`, and rebuilds
`lookup` by re-interning each string in order. Round-trip
preserves all `StringId` values.

Deserialise-time validation:
1. Index 0 must be the empty string.
2. No duplicate entries (interner is content-unique by
   construction).

---

## 5. Source files and spans

### 5.1 `SourceFile`

```rust
pub struct SourceFile {
    pub path:        StringId,         // file path (interned)
    pub content:     Option<Arc<str>>, // None for upstream-crate sources
    pub line_starts: Vec<u32>,         // file-local byte offsets of line starts
}
```

`content` is populated for crates Regseq extracts (the spec
crate, the `regseq` facade, regseq-aware deps per Q6). `None` for
upstream-crate sources whose spans we surface but whose source
text we do not embed.

### 5.2 `Position`

```rust
pub struct Position {
    pub line: u32,        // 1-based
    pub col:  u32,        // 1-based byte column
}
```

Byte-column. Display-column (multi-byte / full-width chars) is
deferred; for driver source code (overwhelmingly ASCII) byte-col
matches display-col.

### 5.3 `Span`

```rust
pub struct Span {
    pub file: SourceFileId,
    pub lo:   u32,   // file-local byte offset, inclusive
    pub hi:   u32,   // file-local byte offset, exclusive
}
```

12 bytes total. `[lo, hi)` end-exclusive. `lo` and `hi` are
file-local byte offsets into `SourceFile.content` (not rustc's
global `BytePos`). `Span` values appear inline in IR nodes — there
is no `SpanId`, no span-deduplication table.

### 5.4 Resolution methods

```rust
impl SourceFile {
    pub fn position(&self, byte: u32) -> Position;
    pub fn line_text(&self, line: u32) -> Option<&str>;
}

impl Span {
    pub fn start(&self, files: &IndexVec<SourceFileId, SourceFile>) -> Position;
    pub fn end(&self,   files: &IndexVec<SourceFileId, SourceFile>) -> Position;
    pub fn slice<'a>(&self, files: &'a IndexVec<SourceFileId, SourceFile>) -> Option<&'a str>;
}
```

`Span::slice` returns `None` when `content` is `None`.

### 5.5 Synthesised source-file sentinel

`source_files[0]` (`SourceFileId::ZERO`) is reserved for the
synthesised-source sentinel:

```rust
SourceFile {
    path:        strings.intern("<synthesized>"),
    content:     None,
    line_starts: vec![0],
}
```

`SourceFileId::SYNTHESIZED` is a documentary alias for
`SourceFileId::ZERO`. Spans on bridge-synthesised items use
`Span { file: SourceFileId::SYNTHESIZED, lo: 0, hi: 0 }`.
Consumers don't need `Option<Span>` checks — synthesised spans
resolve naturally through `Span::slice` to `None` because the
sentinel's `content` is `None`.

---

## 6. CrateInfo

The normative `CrateInfo` and `StableCrateId` definitions live
in §3 (alongside the `RegseqModel` struct that contains the
`crates: IndexVec<CrateId, CrateInfo>` field). This section is
explanatory; the type bodies are **not** duplicated here to
avoid drift.

The `crates` table holds one `CrateInfo` per crate that
contributes items to the model. Per Q6, this includes the spec
crate, the `regseq` facade, and any regseq-aware dep declared via
`[package.metadata.regseq]`. `primary_crate` points at the spec
crate.

The `(CrateId, StableCrateId)` pair parallels `(ItemId,
DefPathHash)`: `CrateId` is local addressing (this-build-only),
`StableCrateId` is cross-build stable identity.

---

## 7. DefPath

A structured per-item identifier that distinguishes items across
crates, disambiguates sibling impls and anon-consts, renders
cleanly to `crate::module::Item` strings, and supports the F4
deterministic sort comparator.

### 7.1 `DefPathData`

A closed enum mirroring rustc's `DefPathData`, mapped through the
interner:

```rust
pub enum DefPathData {
    // Root marker
    CrateRoot,

    // Items and item-like things
    Impl,                       // inherent or trait impl
    ForeignMod,                 // `extern "C"` block
    Use,                        // `use` item
    GlobalAsm,                  // `global_asm!`
    TypeNs(StringId),           // type, module
    ValueNs(StringId),          // function, const, static
    MacroNs(StringId),
    LifetimeNs(StringId),
    Closure,                    // closure expression

    // Subportions of items
    Ctor,                       // implicit tuple-struct / unit-variant ctor
    AnonConst,                  // const expr in source
    LateAnonConst,              // const expr created during AST->HIR lowering
    DesugaredAnonymousLifetime, // elided-lifetime placeholder
    OpaqueTy,                   // existential `impl Trait` node
    OpaqueLifetime(StringId),   // RPITIT captured lifetime
    AnonAssocTy(StringId),      // RPITIT anonymous associated type
    SyntheticCoroutineBody,     // coroutine by-move body
    NestedStatic,               // additional static data referred to by a static
}
```

19 variants. Closed against the pinned toolchain. A toolchain bump
adding a variant is a methodology event (Codex review of the new
variant's semantic placement).

### 7.2 `DefPathSegment` and `DefPath`

```rust
pub struct DefPathSegment {
    pub data:          DefPathData,
    pub disambiguator: u32,
}

pub struct DefPath {
    pub crate_id: CrateId,
    pub segments: Vec<DefPathSegment>,
    pub rendered: StringId,     // canonical verbose rendering
}
```

`crate_id` records the originating crate — Q6 places items from
multiple crates in one RegseqModel.

`segments` is the structured form for F4 sort comparator and
analysis pattern-matching.

`rendered` is the eagerly-computed canonical
`crate::module::Item` string. **Always verbose**: every
`disambiguator != 0` segment carries a `#N` suffix, every
anonymous namespace renders as `{namespace#N}` (`{impl#0}`,
`{closure#1}`, `{anonconst#0}`, ...). The crate prefix is the
resolved `crates[crate_id].name`.

Construction invariant: `rendered` is byte-identical for two
RegseqModels of the same source. The bridge must call the
helper, not selectively pick non-verbose mode.

### 7.3 `DefPathHash`

```rust
pub struct DefPathHash {
    pub stable_crate_id: StableCrateId,
    pub local_hash:      u64,
}
```

Mirrors rustc's `DefPathHash = (StableCrateId, Hash64)` exactly.
Stable across builds. Computed by the bridge from
`tcx.def_path_hash(def_id)` and stored in the
`item_def_paths: IndexVec<ItemId, DefPathHash>` side table on
`RegseqModel`.

### 7.4 Where `DefPath` lives

`DefPath` is part of `Item`. The cross-build identity table
`item_def_paths` lives at the `RegseqModel` level.

---

## 8. Item

`Item` is the source-level item alphabet. Generic forms live
here for source-level traceback (DefPath, kind, attributes,
span); generic items themselves are not emitted (Q2:
monomorphisation is baked at the bridge).

### 8.0 Schema

```rust
pub struct Item {
    pub def_path: DefPath,
    pub kind:     ItemKind,
    pub span:     Span,

    // Lang-item flags (§8.2). Load-bearing for codegen
    // lowering decisions; populated by the bridge from rustc's
    // `AdtFlags` cache via `adt_def.is_*()` predicates.
    pub is_phantom_data:  bool,
    pub is_unsafe_cell:   bool,
    pub is_manually_drop: bool,

    // Bridge-computed global-use outcome for the opaque-handle
    // C lowering convention (§14.5). v1 restricts eligibility
    // to **non-generic** Items: when `true`, the Item is
    // `ItemKind::Struct`, `repr(C)`, has no generic parameters,
    // its (unique) monomorphic `Ty::Struct.layout` is ZST
    // (`size: 0, align: 1`), and every reachable use of that
    // monomorphic `TyId` across the model is behind `Ref` /
    // `Ptr`. False otherwise (most Items). When `true`, codegen
    // MAY emit the Item as a C incomplete struct declaration;
    // when `false`, codegen MUST emit a complete declaration.
    // Generic opaque-handle candidates (`Handle<T>`-style) are
    // rejected at the bridge with `BridgeError::GenericOpaqueHandle`
    // — opaque-handle eligibility is per-monomorphization in
    // general, and a single Item-level bool cannot represent
    // it precisely. A future revision may lift this to a
    // per-monomorphization side table.
    pub opaque_handle_eligible: bool,
}

pub enum ItemKind {
    Function,
    Static,
    Const,
    Struct,
    Enum,
    Union,
}
```

Per-`ItemKind` rich data lives on `Instance` (§9): function
signatures via `FnSigId` on `InstanceKind::Function`,
const/static values via `ConstId`, ADT layout via `TyId` on
`InstanceKind::{Struct, Enum, Union}`. The Item itself carries
only the source-level alphabet.

**Attribute storage is scoped to the stage-3 type system.**
For the type-system shape pinned in this document, no general
attribute table is stored on `Item`. The only attribute-driven
recognition this stage's translation needs is
`#[rseq::intrinsic("NAME")]` (Q7), identified at the callee's
`DefId` during call translation, not via per-Item attribute
storage.

This claim is **scoped to stage 3**. The Regseq DSL has three
mechanisms (intrinsics + properties + traits — see
`regseq-background.md`); declarative property attributes
(`#[rseq::mmio(access_width = 32)]`,
`#[rseq::interrupt(trigger_type = "...")]`, etc.) are a
core DSL mechanism that the type system does not yet
represent. Property metadata is a forward expectation for
**stage 14** (domain-specific extensions): a sibling table
keyed by `ItemId` / `InstanceId` / `FieldPath` / `Span` (the
exact key kind chosen per property family) will record
property values that downstream domain analyses and codegen
consume. Stage 14 owns the property vocabulary; stage 3
intentionally carries no attribute storage.

### 8.1 Lifting `impl` items

`impl` blocks are source-level scopes, not emittable units.
RegseqModel does not represent them as Items. Their contents lift
to the top-level `items` / `instances` tables:

- **Inherent impl methods** lift to top-level Function Items with
  an explicit receiver parameter (`&self` becomes `&Foo` after
  substitution).
- **Trait impl methods** lift the same way; the trait association
  survives only via DefPath
  (`crate::Foo::{impl#0 as Trait}::bar`) and — for dynamic
  dispatch only — via vtables synthesised as Static Instances.
- **Associated constants** lift to top-level Const Items.
- **Associated types** are erased; uses are substituted at
  monomorphisation.
- **Trait declarations** are erased; static dispatch resolves
  directly to the implementing function's Instance, dynamic
  dispatch goes through a vtable.
- **Default trait method bodies** are owned by the trait, not by
  any impl. The body lives at the trait's DefPath.

Consequences:

- `ItemKind` has six variants — `Function`, `Static`, `Const`,
  `Struct`, `Enum`, `Union` — mirroring `InstanceKind`. No
  `Impl`, `Trait`, or `AssocType` variants.
- `DefPathData` keeps its full 19-variant inventory. `Impl`
  remains a path-segment variant for source-level traceback; it
  is not an Item kind.

### 8.2 Lang-item flags on `Item`

The bridge populates flags on `Item` for lang-items where the
flag is load-bearing for codegen lowering. Recognition is via
`adt_def.is_*()` predicates, all backed by `AdtFlags` bits in
rustc:

| Flag               | Recognition                  | rustc cache                  |
| ------------------ | ---------------------------- | ---------------------------- |
| `is_phantom_data`  | `adt_def.is_phantom_data()`  | `AdtFlags::IS_PHANTOM_DATA`  |
| `is_unsafe_cell`   | `adt_def.is_unsafe_cell()`   | `AdtFlags::IS_UNSAFE_CELL`   |
| `is_manually_drop` | `adt_def.is_manually_drop()` | `AdtFlags::IS_MANUALLY_DROP` |

Lang items NOT flagged on `Item` (recognised at call/use time
instead, or not in v1 scope):

| Lang item                                           | Disposition                                                                    |
| --------------------------------------------------- | ------------------------------------------------------------------------------ |
| `MaybeUninit`                                       | not flagged; composition (Union no-auto-drop + inner `ManuallyDrop`) suffices  |
| `CVoid`                                             | not flagged; recognised via `tcx.is_lang_item(LangItem::CVoid)` only to reject |
| `Pin`                                               | reintroduce `is_pin` when async support lands                                  |
| `OwnedBox`, `VaList`, `GlobalAlloc`, `UnsafePinned` | not in v1 scope                                                                |

---

## 9. Instance and InstanceKind

Per Q2, the bridge bakes monomorphisation. **Instance is the
primary monomorphised unit in RegseqModel.** Every emittable
construct — functions, statics, consts, structs, enums, unions
— appears in `instances` as a fully-resolved monomorphisation.
Generic forms live in `items` only as the alphabet for source-
level traceback; they are not emitted.

Downstream consumers (analyses, C codegen) traverse `instances`,
not `items`. The `(item, generic_args)` back-reference exists
for diagnostics, custom naming, source navigation, and cross-
build identity — not for emission.

### 9.1 `GenericArg`

```rust
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
    // Lifetimes erased post-monomorphisation; not represented.
}

pub type GenericArgs = Vec<GenericArg>;
```

Lifetimes are erased at the bridge (rustc's `erasing_regions`
step). They never appear in `GenericArgs`.

### 9.2 `Instance`

```rust
pub struct Instance {
    pub item:          ItemId,        // origin: source-level item, always present
    pub generic_args:  GenericArgs,   // resolved args; empty for non-generic items
    pub emission_name: StringId,      // C-side identifier; uniqueness guaranteed across `instances`
    pub span:          Span,          // source span; SYNTHESIZED for bridge-synthesised items
    pub kind:          InstanceKind,
}

newtype_index! { pub struct InstanceId {} }
```

- `item` is always present; every rustc Instance variant carries
  a `DefId`, so an originating Item exists for every Instance.
  For Instances whose origin is a recognised `core` lang-item
  (e.g., `drop_in_place` for drop glue), the `Item` entry is
  metadata-only: DefPath, kind, span, attributes — captured by
  the bridge from rustc-side metadata, with no body in `bodies`.
- `emission_name` is the C-side identifier under which this
  instance is emitted. Bridge construction rule by `InstanceKind`:
  - `Function` / `Static` / `Const`: rustc's
    `tcx.symbol_name(MonoItem::*)` mangled name (callable/static
    instances have a natural rustc symbol).
  - `Struct` / `Enum` / `Union`: bridge-constructed name derived
    deterministically from the rendered `DefPath` plus the
    canonical `ArgsKey` (§12.3), encoded into a C-identifier-safe
    form. Type-definition instances have no rustc symbol; the
    construction rule is pinned in stage 13 (codegen).
  Uniqueness across `instances` is guaranteed by construction.
  Custom naming strategies that walk `(item.def_path, generic_args)`
  must include `InstanceId` or `emission_name` as a disambiguating
  suffix because rustc shims can share a `DefId` (e.g., `Item(foo)`
  vs `ReifyShim(foo)`).
- `span` is `SYNTHESIZED` for bridge-synthesised items.

### 9.3 Instance interning key

The bridge interns instances during construction with a
**Regseq-level instance key**:

```text
RegseqInstanceKey = (
    regseq_kind_rank:    u8,    // Regseq InstanceKind variant rank
    item:                ItemId,
    generic_args_key:    ArgsKey,   // see §12 for ArgsKey
    fn_shim_disambig:    u8,    // 0 for non-Function variants;
                                // for Function: rustc InstanceKind rank
)
```

Two ranks because Regseq's `InstanceKind` covers six emission
kinds (Function/Static/Const/Struct/Enum/Union) while rustc's
`InstanceKind` is a callable-only taxonomy with shim variants
(`Item`, `DropGlue`, `VTableShim`, `ReifyShim`, etc.) that can
share a `DefId`. The `regseq_kind_rank` separates emission
kinds; `fn_shim_disambig` separates rustc shims within the
`Function` Regseq kind. Without the second discriminator,
`Item(foo)` and `ReifyShim(foo)` would collapse into one
`Instance` despite distinct semantics.

The shim discriminator **is** exposed as a public field on
`Instance` via `InstanceKind::Function.flavor`
(`FunctionInstanceFlavor`, §9.4) — added in Round 7 to give
deserialised consumers a structured way to reconstruct stable
instance identity for shims sharing `(item, args)` without
parsing `emission_name`. The mapping
`FunctionInstanceFlavor` → `fn_shim_disambig: u8` is the rank
table at §9.3 (`rustc_instance_kind_sort_rank`). For non-
`Function` Regseq kinds, no `flavor` field is needed because
`fn_shim_disambig` is unconditionally `0`.

#### Stable rank tables

`regseq_instance_kind_rank` (append-only):

```rust
fn regseq_instance_kind_rank(kind: &InstanceKind) -> u8 {
    match kind {
        InstanceKind::Function { .. } => 0,
        InstanceKind::Static   { .. } => 1,
        InstanceKind::Const    { .. } => 2,
        InstanceKind::Struct   { .. } => 3,
        InstanceKind::Enum     { .. } => 4,
        InstanceKind::Union    { .. } => 5,
    }
}
```

`rustc_instance_kind_sort_rank` (append-only; pinned for the
toolchain `nightly-2026-02-01-aarch64-apple-darwin`): rank for
rustc's `ty::InstanceKind` variant, used as `fn_shim_disambig`
when the Regseq kind is `Function`:

```rust
fn rustc_instance_kind_sort_rank(kind: &rustc_ty::InstanceKind<'_>) -> u8 {
    match kind {
        InstanceKind::Item(..)                               => 0,
        InstanceKind::Intrinsic(..)                          => 1,
        InstanceKind::VTableShim(..)                         => 2,
        InstanceKind::ReifyShim(..)                          => 3,
        InstanceKind::FnPtrShim(..)                          => 4,
        InstanceKind::Virtual(..)                            => 5,
        InstanceKind::ClosureOnceShim { .. }                 => 6,
        InstanceKind::ConstructCoroutineInClosureShim { .. } => 7,
        InstanceKind::ThreadLocalShim(..)                    => 8,
        InstanceKind::FutureDropPollShim(..)                 => 9,
        InstanceKind::DropGlue(..)                           => 10,
        InstanceKind::CloneShim(..)                          => 11,
        InstanceKind::FnPtrAddrShim(..)                      => 12,
        InstanceKind::AsyncDropGlueCtorShim(..)              => 13,
        InstanceKind::AsyncDropGlue(..)                      => 14,
    }
}
```

For non-`Function` Regseq kinds (Static/Const/Struct/Enum/Union),
`fn_shim_disambig` is unconditionally `0` — they have no
rustc-shim discrimination to encode.

**Verification requirement.** The bridge MUST verify that
`(regseq_kind_rank, item, generic_args_key, fn_shim_disambig)`
is injective over the admitted `InstanceKind` set under the
pinned toolchain. Two distinct rustc `InstanceKind` values that
share the same `(rank, did, args)` would collapse into one
RegseqModel `Instance` — breaking the same identity-preservation
contract D15 enforces for `FnAbi`. Verification is a debug-build
assertion at instance interning. If a future toolchain bump
introduces a non-injective shim variant (i.e., two distinct
rustc `InstanceKind` carrying additional discriminating payload
beyond what `fn_shim_disambig` captures), the rank table
extends with a payload-key encoding rather than silently
admitting the collision.

### 9.4 `InstanceKind`

```rust
pub enum InstanceKind {
    /// Function instantiation. `sig` is always present, even
    /// for foreign declarations (`extern "C" { fn foo(); }`
    /// where `body` is `None`). `flavor` records which rustc
    /// `InstanceKind` variant the bridge translated from
    /// (§9.3 `fn_shim_disambig`); stored on the model so
    /// deserialised consumers can reconstruct stable instance
    /// identity for shims sharing `(item, args)` without
    /// parsing `emission_name`.
    Function { sig: FnSigId, body: Option<BodyId>, flavor: FunctionInstanceFlavor },

    /// Static. `mutability` distinguishes `static` (Not) from
    /// `static mut` (Mut) — affects emitted C storage class
    /// (`const` vs mutable) and aliasing assumptions. `init`
    /// is `None` for foreign statics (`extern "C" { static FOO:
    /// u32; }`) and `Some(c)` for source-defined statics with
    /// an initial value.
    Static { ty: TyId, init: Option<ConstId>, mutability: Mutability },

    /// Const item.
    Const { ty: TyId, value: ConstId },

    /// Nominal struct. `layout` -> `Ty::Struct`.
    Struct { layout: TyId },

    /// Nominal enum. `layout` -> `Ty::Enum`.
    Enum { layout: TyId },

    /// Nominal C-style union. `layout` -> `Ty::Union`.
    Union { layout: TyId },
}

/// Mirror of rustc's `ty::InstanceKind` discriminants for the
/// Function Regseq kind. Append-only (mirrors the
/// `rustc_instance_kind_sort_rank` table at §9.3) — adding a
/// new rustc variant on a toolchain bump appends a new flavor
/// at the end; existing flavors and ranks remain immutable.
pub enum FunctionInstanceFlavor {
    Item,
    Intrinsic,
    VTableShim,
    ReifyShim,
    FnPtrShim,
    Virtual,
    ClosureOnceShim,
    ConstructCoroutineInClosureShim,
    ThreadLocalShim,
    FutureDropPollShim,
    DropGlue,
    CloneShim,
    FnPtrAddrShim,
    AsyncDropGlueCtorShim,
    AsyncDropGlue,
}
```

Each variant carries only the IDs needed to find kind-specific
rich data in the appropriate table (`sigs`, `consts`, `types`,
`bodies`).

`FunctionInstanceFlavor` is the structured form of the
`fn_shim_disambig: u8` rank used inside `RegseqInstanceKey`
(§9.3). Storing it on `InstanceKind::Function` (rather than
deriving it from `emission_name`) avoids the brittleness the
Round 6 SyntheticItemKind change avoided for synthetic Items
(§13.3). The rank-table mapping
`rustc_instance_kind_sort_rank(flavor) -> u8` is stable per
the toolchain pinning (§9.3).

**Bridge-synthesised Function Items use `flavor =
FunctionInstanceFlavor::Item` by convention.** Vtable method
shims, vtable drop shims, closure call methods, and closure
drop glue (synthesised per §13.1, §13.2) are bridge-
constructed Function Items with no rustc `ty::InstanceKind`
origin — the `FunctionInstanceFlavor` enum mirrors rustc's
taxonomy, which doesn't apply to bridge-synthetic functions.
Rather than add a `FunctionInstanceOrigin { Rustc(...),
RegseqSynthetic }` discriminator (which would carry no
identity information not already present), v1 pins the
sentinel: synthetic functions use `flavor =
FunctionInstanceFlavor::Item`. Identity disambiguation is
already provided by the synthetic `ItemId` (each synthetic
function has a unique synthetic `ItemId` per §13.4), so
`(regseq_kind_rank=Function, item, args, fn_shim_disambig=0)`
is injective across synthetic functions even with all of them
sharing `flavor = Item`. Future revisions may distinguish
synthetic function families if a real need surfaces (e.g.,
sort-order tweaks for diagnostics); v1 takes the simpler
sentinel path.

The six variants above are the current closed set. Any future
extension follows the Q3 single-level IR rule: append to
`InstanceKind` with a corresponding append-only entry in
`regseq_instance_kind_rank` (existing ranks remain immutable).

### 9.5 Where `InstanceId` references appear

Call sites in MIR carry `InstanceId` identifying the callee.
This includes:

- Regular function calls.
- Direct dispatch on trait methods (resolved at monomorphisation).
- Drop-glue calls: `Drop` terminators lower to regular `Call`
  terminators targeting the drop-glue `Instance` for the type.
  There is no `Drop` variant in MIR terminators.

Dynamic dispatch through trait objects becomes an indirect call
through a vtable. The vtable is a synthesised Static `Instance`
whose value is a mixed aggregate matching rustc's
`tcx.vtable_entries(trait_ref)` sequence: drop-glue fn pointer +
size + alignment + per-method fn pointers + supertrait vtable
pointers + `Vacant` slots for non-dispatchable methods. Both the
drop-glue slot and each method fn pointer reference synthesised
**vtable shim Function Items** (see §13.2).

### 9.6 Bridge-synthesised Items

Some Instances arise from constructs the bridge synthesises
rather than from source-level definitions. The principal cases
are trait-object vtables and closure captures-structs (see §13).
Bridge-synthesised Items live in `items` alongside source-level
items, with construction rules:

- **`def_path`**: synthetic, under a reserved canonical form
  per family. Vtable family rendered examples: vtable type
  `<vtable_ty for Trait[<trait_args>][<projection_bindings>]>`
  and vtable instance `<vtable for Trait for SelfTy>` (§13.2;
  the structured `DefPath.segments` encoding lives in §13.4).
- **`def_path_hash`**: derived deterministically from the
  synthetic components. Stable across builds. Identity does
  **not** include any "triggering crate" — synthetic Items
  are deduped by semantic identity (e.g., `PrincipalTraitKey`
  for vtables), not by which crate first required them; see
  §13.4 "No triggering crate in identity" for the deterministic
  ownership rule (used only for cross-references and display,
  not identity).
- **`crate_id`**: deterministic per family per §13.4 ownership
  rule (closure family: closure expression's
  `DefPath.crate_id`; vtable family: principal trait's
  originating crate, falling back to `primary_crate`). This is
  cross-reference / display metadata only; it is not part of
  the synthetic `DefPathHash` identity.

Construction invariant 2 (§3.1) extends to synthetic items
(with the ClosureCaptures carve-out): `item_def_paths[item_id]`
for a synthetic item is the bridge-derived `DefPathHash`
(§13.4); for the source-derived ClosureCaptures Item it is
rustc's `tcx.def_path_hash(closure_did)`.

### 9.7 `ConstInterner`

Const values participate in nominal type identity via
`GenericArg::Const(ConstId)` (§9.1) and in static / const
instances via `InstanceKind::Static.init` / `Const.value`. Sound
nominal-type identity therefore requires `ConstId` to be content-
unique: two builds (or two construction paths within one build)
that produce the same canonical const value must produce the
same `ConstId`.

```rust
pub struct ConstInterner {
    storage: IndexVec<ConstId, Const>,
    lookup:  HashMap<ConstIdentityKey, ConstId>,
}

impl ConstInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, c: Const) -> ConstId;
    pub fn resolve(&self, id: ConstId) -> &Const;
    pub fn find(&self, key: &ConstIdentityKey) -> Option<ConstId>;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (ConstId, &Const)>;
}
```

Same BiMap pattern as `TyInterner` (§12) and `StringInterner`
(§4). Custom serde mirrors the same shape.

`Const` and `ConstIdentityKey` schemas are **pinned in stage 4**.
Stage 3's commitment is the content-uniqueness invariant
(construction invariant 11, §3.1) and the canonicalisation
contract: `ConstIdentityKey` is computed from the value's
const-eval result (`Discr.val: u128` for scalar consts, fully-
evaluated bytes for aggregate consts), not from any
allocation-order rustc handle. Unevaluated or symbolic consts
that survive monomorphisation are a bridge bug per the no-
defensive-fallbacks methodology.

For stage-3 type-system purposes, the minimum shape `Const` must
support:

- scalar primitives (`Bool`, `Char`, `Int(IntKind, value)`,
  `Uint(UintKind, value)`, `Float(FloatKind, bits)`),
- evaluated array lengths (used in `Ty::Array(elem, n: u64)` —
  the `u64` is sourced from the const-eval result, not a separate
  `ConstId`).

Generic-arg const positions (`GenericArg::Const(ConstId)`) reach
the canonicaliser via §12.3's "ConstId → canonical const-eval
result" rule; the ConstInterner ensures equal canonical results
share a `ConstId`.

#### Stage-3 minimum vs stage-4 requirement

Stage 3 commits to ConstId content-uniqueness (invariant 11) and
the scalar-primitive + array-length stub above. Several stage-3
spec constructs *reference* `ConstId` for richer values that
stage 4 must canonicalise:

- `InstanceKind::Static { init: Option<ConstId>, .. }` —
  initial-value constant (may be aggregate, may carry
  relocations to other Instances). `None` for foreign statics
  (no init expression).
- `InstanceKind::Const { value: ConstId }` — const item value.
- Vtable instance values (§13.2) — aggregate constants
  containing function pointers (relocations to vtable shim
  Function Items), size/align scalars, vacant slots.
- String literals backing `&str` — byte-array constants.
- Aggregate constants for arrays/structs/tuples/enums.
- Reference-typed constants — null pointer, function pointer,
  vtable pointer, static address.

These reference shapes are pinned at stage 3 by virtue of being
mentioned in the IR; stage 4's full `Const` and
`ConstIdentityKey` schemas must support canonical equality over
all of them. Implementations that materialise vtable instances
or static initialisers are gated on stage-4 const-vocabulary
expansion. The minimum surface stage 4 must cover:

- scalar values with type tag (so `1u32` and `1u64` differ);
- ZST values (single canonical representative);
- byte arrays / raw initialised bytes;
- aggregate field sequences (preserving order);
- enum discriminant + payload constants;
- references to `InstanceId` (function/static/vtable pointer
  relocations); the **cross-build** canonical encoding of such
  a relocation must use stable instance identity, not local
  `InstanceId` integer — encoded as
  `(regseq_instance_kind_rank, item_def_path_hash,
  canonical_generic_args, fn_shim_disambig_or_None)` so that
  two builds satisfying the build-equivalence quad produce
  identical canonical const keys for relocations to the same
  Instance. Stage-4's `ConstIdentityKey` schema is responsible
  for this encoding;
- typed null pointers (one per pointee type);
- padding/uninitialised bytes if they appear in constants
  (canonicalisation must distinguish "uninit" from any specific
  byte pattern).

##### Stage-4 const-validation phase split (forward expectation)

Stage 3's `ConstInterner` is validated **before** `TyInterner`
(per §3.3 Phase B), because nominal-type identity depends on
`GenericArg::Const(ConstId)` content uniqueness. That ordering
is sound for stage-3 const usage (scalar primitives + array
lengths), which can be canonicalised without resolving any
`Ty` or `Instance`.

Richer const values listed above (vtable instances, static
initialisers, typed null pointers, function-pointer
relocations) need access to type identity (for typed-null
canonicalisation) and instance identity (for relocation
encoding). That introduces a dependency cycle if all const
values share a single validation phase.

Stage 4 will resolve this with a **two-phase const validation**
discipline:

1. **Type-level const phase** (validated early, before
   `TyInterner`): the subset of `Const` values reachable from
   `GenericArg::Const(ConstId)` — scalar primitives only in
   v1. Canonicalisation uses a type-independent key.
2. **Value-level const phase** (validated late, after
   `TyInterner` and `instances`): the full `Const` value
   universe (statics, vtable instances, aggregates,
   relocations). Canonicalisation may reference type and
   instance identity.

Implementation may use a single `ConstInterner` table with a
two-pass validation discipline, or two separate interner
tables (`TypeConstInterner` + `ConstInterner`) — the spec
deferral is the validation phase split, not the interner
shape.

---

## 10. The `Ty` enum

The `Ty` enum is **closed** at 19 variants. Every variant is a
*rigid* post-monomorphisation type per `TyKind::is_known_rigid()`.
Domain extensions extend `InstanceKind` / `Item` flags / tags on
existing nodes — never `Ty` itself (Q3 single-level IR rule).

| Variant                                                      | Carries                                           | Layout?          | Identity bucket                                      | C-side intent                                                                                                                                              |
| ------------------------------------------------------------ | ------------------------------------------------- | ---------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Bool`                                                       | —                                                 | —                | structural                                           | `_Bool` / `uint8_t`                                                                                                                                        |
| `Char`                                                       | —                                                 | —                | structural                                           | `uint32_t` (Unicode scalar value)                                                                                                                          |
| `Int(IntKind)`                                               | `I8`/`I16`/`I32`/`I64`/`I128`/`Isize`             | —                | structural                                           | `intN_t` / `intptr_t`                                                                                                                                      |
| `Uint(UintKind)`                                             | `U8`/`U16`/`U32`/`U64`/`U128`/`Usize`             | —                | structural                                           | `uintN_t` / `size_t`                                                                                                                                       |
| `Float(FloatKind)`                                           | `F16`/`F32`/`F64`/`F128`                          | —                | structural                                           | `float` / `double` / `_Float16` / `_Float128`                                                                                                              |
| `Str`                                                        | marker                                            | no (unsized)     | structural                                           | only behind `Ref`/`Ptr`; fat pointer                                                                                                                       |
| `Never`                                                      | marker                                            | no (uninhabited) | structural                                           | `void` + `__attribute__((noreturn))` in fn-return; accepted as `Ref`/`Ptr` pointee (codegen may reject); divergent-expression positions elided (see §10.2) |
| `Array(TyId, u64)`                                           | element type + length                             | —                | structural                                           | C array `T[N]` in field/local context only; rejected at `FnAbi::C` parameter/return per §14.5 (C arrays decay to pointers in those positions)              |
| `Slice(TyId)`                                                | element type                                      | no (unsized)     | structural                                           | only behind `Ref`/`Ptr`; fat pointer                                                                                                                       |
| `Ref(TyId, Provenance)`                                      | pointee + borrow flavor                           | —                | structural                                           | `T*` / `const T*` / `T* restrict`                                                                                                                          |
| `Ptr(TyId, Mutability)`                                      | pointee + `Not`/`Mut`                             | —                | structural                                           | `T*` / `const T*` (raw)                                                                                                                                    |
| `FnDef(ItemId, GenericArgs)`                                 | callable item + post-mono args                    | —                | **nominal**                                          | ZST — function identity                                                                                                                                    |
| `FnPtr(FnSigId)`                                             | sig handle (carries `FnAbi`, etc.)                | —                | structural (FnSig content expanded by canonicalizer) | C function pointer `T (*)(args)`                                                                                                                           |
| `Dynamic { auto_traits, vtable_ty }`                         | sorted `AutoTrait` set + vtable's `TyId`          | —                | structural                                           | only behind `Ref`/`Ptr`; fat pointer (codegen-emitted)                                                                                                     |
| `Closure(ItemId, GenericArgs)`                               | synthesised captures-struct Item + post-mono args | —                | **nominal**                                          | identity view; layout via parallel `Ty::Struct`                                                                                                            |
| `Tuple { fields: Vec<TyId>, layout: Layout }`                | element types + resolved layout                   | **inline**       | structural-with-layout-excluded                      | anonymous C struct (synthetic field names `_0`, `_1`, ...) for non-empty tuples; the **empty tuple** `Ty::Tuple { fields: [] }` is Rust unit `()` — ZST with `size: 0, align: 1`, lowered to C `void` in function return position (per §14.5) and treated as a value-less marker elsewhere |
| `Struct { item: ItemId, args: GenericArgs, layout: Layout }` | item + args + resolved layout                     | **inline**       | **nominal**                                          | C `struct`                                                                                                                                                 |
| `Enum { item: ItemId, args: GenericArgs, layout: Layout }`   | item + args + resolved layout                     | **inline**       | **nominal**                                          | tagged C struct or niche-encoded                                                                                                                           |
| `Union { item: ItemId, args: GenericArgs, layout: Layout }`  | item + args + resolved layout                     | **inline**       | **nominal**                                          | C `union`                                                                                                                                                  |

### 10.0 `Ty` enum and primitive scalar enums (Rust shapes)

The variant table above is the reference; the concrete Rust
shape implementations target is:

```rust
pub enum Ty {
    Bool,
    Char,
    Int(IntKind),
    Uint(UintKind),
    Float(FloatKind),
    Str,
    Never,
    Array(TyId, u64),
    Slice(TyId),
    Ref(TyId, Provenance),
    Ptr(TyId, Mutability),
    FnDef(ItemId, GenericArgs),
    FnPtr(FnSigId),
    Dynamic { auto_traits: Vec<AutoTrait>, vtable_ty: TyId },
    Closure(ItemId, GenericArgs),
    Tuple { fields: Vec<TyId>, layout: Layout },
    Struct { item: ItemId, args: GenericArgs, layout: Layout },
    Enum   { item: ItemId, args: GenericArgs, layout: Layout },
    Union  { item: ItemId, args: GenericArgs, layout: Layout },
}
```

Primitive scalar discriminants are **append-only** — adding a
new scalar variant on a future toolchain bump appends at the
end; existing discriminants remain immutable so serialised
`ScalarLayouts` indices and `ArgsKey` byte streams stay stable.

```rust
#[repr(u8)]
pub enum IntKind {
    I8    = 0,
    I16   = 1,
    I32   = 2,
    I64   = 3,
    I128  = 4,
    Isize = 5,
}

#[repr(u8)]
pub enum UintKind {
    U8    = 0,
    U16   = 1,
    U32   = 2,
    U64   = 3,
    U128  = 4,
    Usize = 5,
}

#[repr(u8)]
pub enum FloatKind {
    F16  = 0,
    F32  = 1,
    F64  = 2,
    F128 = 3,
}

pub const INT_KIND_COUNT:   usize = 6;
pub const UINT_KIND_COUNT:  usize = 6;
pub const FLOAT_KIND_COUNT: usize = 4;

#[repr(u8)]
pub enum Safety {
    Safe   = 0,
    Unsafe = 1,
}
```

`Safety` mirrors rustc's `hir::Safety` and is used in
`FnSig.safety` (§14.3).

### 10.1 Auxiliary closed enums on `Ty`

```rust
#[repr(u8)]
pub enum Provenance { SharedBorrow = 0, UniqueBorrow = 1 }

#[repr(u8)]
pub enum Mutability { Not = 0, Mut = 1 }      // mirrors rustc

#[repr(u8)]
pub enum AutoTrait  { Send = 0, Sync = 1, Unpin = 2 }
//                    sorted ascending by discriminant in `Ty::Dynamic.auto_traits`

#[repr(u8)]
pub enum FnAbi      { Rust = 0, C = 1 }
```

| Enum         | Variants                                       |
| ------------ | ---------------------------------------------- |
| `Provenance` | `SharedBorrow`, `UniqueBorrow`                 |
| `Mutability` | `Not`, `Mut` (rustc's enum, mirrored)          |
| `AutoTrait`  | `Send`, `Sync`, `Unpin` (sorted in this order) |
| `FnAbi`      | `Rust`, `C`                                    |

Advisory pointer-kind bits (`frozen`, `unpin`, `global`) are
dropped from `Provenance`. Custom auto-traits (declared via
unstable `feature(auto_traits)`) are rejected at the bridge.

Naming note: `Provenance` here means *borrow flavor* — the source
reference's shared/unique distinction that drives C qualification
and the `restrict` decision. It is **not** rustc-style pointer
provenance (Stacked Borrows, Tree Borrows, alias history); the
IR carries no aliasing-history metadata (per D7/D8).

### 10.2 Unsized variants

`Ty::Str` and `Ty::Slice` are unsized DST markers; they do NOT
carry a `Layout` and `tcx.layout_of` is not called on them
directly. They appear only inside `Ty::Ref(...)` or
`Ty::Ptr(...)` pointee positions; codegen emits the fat-pointer
shape on demand.

`Ty::Never` is the uninhabited type `!`. It carries no `Layout`.
It is accepted at type level in three positions:

- **Function-return type** (`fn diverge() -> !`): codegen emits
  `void` plus `__attribute__((noreturn))`.
- **Divergent-expression type**: positions are unreachable by
  construction and codegen elides them.
- **Pointee inside `Ty::Ref(...)` / `Ty::Ptr(...)`** (`&!`,
  `*const !`, `*mut !`): accepted at type level (the type is
  well-formed); codegen may reject when emitted, since
  uninhabited pointees have no value representation.

### 10.3 Wide-scalar portability

`Int(I128)`, `Uint(U128)`, `Float(F16)`, and `Float(F128)` are
admitted unconditionally by the bridge — they are well-formed
post-monomorphisation rigid types. Their C lowerings depend on
the target C compiler:

- `__int128` / `unsigned __int128` (gcc/clang on 64-bit targets).
- `_Float16` (gcc/clang extension, target-dependent).
- `_Float128` (target-dependent).

Codegen rejects these scalars per-target when the selected C
compiler lacks faithful support, raising
`BackendError::UnsupportedScalar { kind, target }`. This is
target-dependent rejection at the codegen boundary, not bridge-
side defensive guarding (the bridge never speculates about
backend capability). Rejection applies wherever the unsupported
scalar appears in a position the backend would emit: top-level
parameters/returns, struct/union fields, array elements, **and
nested FnPtr declarators** (the inner FnSig's
`sig_c_abi_lowerings` is walked at extern-declaration emission;
unsupported scalars there reject the outer declaration too).

---

## 11. Layout

Resolved layout copied from `tcx.layout_of(ty)`. The bridge does
not recompute layout; rustc owns the algorithm, RegseqModel owns
preserving the answer. Layout lives inline on `Ty::Struct`,
`Ty::Enum`, `Ty::Union`, and `Ty::Tuple`.

```rust
pub struct Layout {
    pub size:        u64,
    pub align:       u64,
    pub pack:        Option<u32>,   // None = not packed; Some(N) = repr(packed(N))
    pub uninhabited: bool,          // Result<T, !>, enum E {}, struct S(!), etc.
    pub kind:        LayoutKind,
}

pub enum LayoutKind {
    Struct { fields: Vec<FieldLayout> },   // for Ty::Struct AND Ty::Tuple
    Union  { fields: Vec<FieldLayout> },   // for Ty::Union (offsets all 0)
    Enum   { shape:  EnumShape       },    // for Ty::Enum
}

pub struct FieldLayout {
    pub name:   StringId,    // interned via RegseqModel.strings
    pub ty:     TyId,
    pub offset: u64,         // see §11.0 for offset reference frame
}
```

`pack` is the single repr-driven knob carried structurally. Every
other repr effect (alignment boost, transparent newtype, repr-int
tag width, struct-field reordering) falls out of resolved bytes.

#### Canonical unit layout

Rust unit `()` is `Ty::Tuple { fields: vec![], layout: UNIT_LAYOUT }`.
The canonical layout is pinned as a constant so the bridge,
synthesised vtable receivers (§13.2), and validation (§3.3
Phase B) all share one shape:

```rust
pub const UNIT_LAYOUT: Layout = Layout {
    size:        0,
    align:       1,
    pack:        None,
    uninhabited: false,
    kind:        LayoutKind::Struct { fields: Vec::new() },
};
```

Validation enforces that any `Ty::Tuple { fields: [] }` carries
exactly `UNIT_LAYOUT` — tuple identity excludes layout from
nominal/structural equality (§12.1), so this validation is the
only defence against a serialised model assigning a non-unit
layout to `()`.

### 11.0 Field and variant index conventions

Two indexing conventions on `Layout` vectors are normative:

- **`fields: Vec<FieldLayout>` is indexed by source / MIR field
  index** (matching `PlaceElem::Field(FieldIdx, _)` projections).
  This holds for `LayoutKind::Struct.fields`,
  `LayoutKind::Union.fields`, and `VariantLayout.fields`. The
  vector's index is the projection key body lowering uses; it is
  *not* memory order.
- **`FieldLayout.offset` is the byte offset from the start of the
  *containing aggregate*.** For struct/union fields, the
  containing aggregate is the struct/union itself. For variant
  payload fields (inside `VariantLayout.fields`), the containing
  aggregate is the **whole enum object**, not the variant payload
  start. This matches rustc's `tcx.layout_of` convention and lets
  codegen lower `SetDiscriminant`, `Discriminant`, and field
  store/load to absolute offsets uniformly.
- **`EnumShape::Multiple.variants: Vec<VariantLayout>` is indexed
  by `VariantIdx`** (rustc's variant index, mirrored as a typed
  ID per the §2.4 inventory).

C emission may reorder emitted fields by `offset` and synthesise
explicit padding fields, but the IR's field index remains stable
for projection lowering. ZST fields (sizeof == 0) remain in the
IR for projection / identity / drop-tracking purposes; emission
may omit them from the C struct.

### 11.1 `EnumShape` and `VariantLayout`

```rust
pub enum EnumShape {
    /// Zero-variant uninhabited enum (`enum Never {}`).
    Empty,

    /// Single-variant enum. `scalar` is `Some` for fieldless
    /// `repr(int)` enums (a tag scalar IS allocated even with
    /// one variant: e.g., `#[repr(u8)] enum Only { A = 5 }`
    /// stores `5` as a `u8`); `None` for ordinary
    /// single-variant enums (no tag bytes).
    Single { variant: VariantLayout, scalar: Option<TagScalar> },

    Multiple {
        tag:      TagScalar,
        encoding: TagEncoding,
        variants: Vec<VariantLayout>,
    },
}

pub struct VariantLayout {
    pub name:         StringId,
    pub fields:       Vec<FieldLayout>,
    pub uninhabited:  bool,
    pub discriminant: u128,         // Rust-level semantic discriminant
                                    // (what Discriminant(place) returns)
    pub tag_value:    Option<u128>, // layout-level tag bytes
                                    // (what SetDiscriminant writes)
}
```

`tag_value` semantics:

| Case                                                                       | `tag_value`                                                                                  |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `EnumShape::Single { scalar: None }` (ordinary single-variant enum)        | `None` (no tag bytes)                                                                        |
| `EnumShape::Single { scalar: Some(_) }` (fieldless `repr(int)` enum)       | `Some(v)`, `v == discriminant` (the variant's `repr(int)` value, e.g., `5` for `A = 5`)      |
| Direct-encoded multiple variant                                            | `Some(v)`, `v == discriminant`                                                               |
| Niche-encoded niched variant                                               | `Some(v)` per the niche tag-value formula (§11.2)                                            |
| Niche-encoded **untagged** variant (data IS the tag)                       | **`None`** — payload written via the normal field-assignment path, NOT via `SetDiscriminant` |

**Signed-discriminant interpretation.** `discriminant: u128` and
`tag_value: u128` carry the *bit pattern* of the resolved value,
truncated/zero-extended into 128 bits — this matches rustc's own
`Discr.val: u128` representation. For signed `repr(i*)` enums
(e.g., `Status::Err = -1` lowering to bits `0xFF` for `repr(i8)`,
`0xFFFFFFFFu32` for `repr(i32)`), the value stored is the unsigned
reinterpretation of the two's-complement encoding at the tag
scalar's width. Codegen reads `TagScalar.scalar_ty` to decide
whether to emit the constant as a signed C literal: `Int(Ix)` →
sign-extend the low `Ix` bits to `intmax_t` and emit signed;
`Uint(Ux)` → zero-extend and emit unsigned. The storage shape
(`u128`) is value-equality friendly without committing to a
sign-aware comparison at the IR level.

### 11.2 `TagScalar`, `TagEncoding`, `WrappingRange`

```rust
pub struct TagScalar {
    pub offset:      u64,
    pub scalar_ty:   TyId,            // logical scalar type — see policy below
    pub valid_range: WrappingRange,   // always present
}

pub enum TagEncoding {
    Direct,
    Niche {
        untagged_variant: VariantIdx,
        niche_variants:   VariantRange,   // inclusive
        niche_start:      u128,
    },
}

newtype_index! { pub struct VariantIdx {} }

pub struct VariantRange {
    pub start: VariantIdx,
    pub end:   VariantIdx,    // inclusive
}

pub struct WrappingRange {
    pub start: u128,
    pub end:   u128,                  // inclusive; wraps if start > end
}
```

Bridge invariant: `TagScalar.valid_range` is always present (the
bridge populates from rustc's `Scalar::Initialized`). If the
bridge encounters `Scalar::Union` at the tag position of a
`Variants::Multiple`, it rejects the layout shape rather than
fabricating a `valid_range`.

**`TagScalar.scalar_ty` policy: logical scalar type.**
`scalar_ty` is the **logical** rustc-resolved tag type, not its
storage integer. Admitted scalar variants (per the v1 type
universe): `Bool`, `Char`, `Int(_)`, `Uint(_)`, `Ptr(_, _)`,
`Ref(_, _)`. Niche-optimised enums whose tag is carried by a
non-integer scalar position (e.g., `Option<bool>`, where Bool's
invalid bit patterns serve as the `None` niche) preserve the
logical type. `Float` is **not** admitted as a tag scalar —
floats have no usable niche/validity invariants for niche
encoding under v1 (rustc agrees).

**Niche tag-value formula (normative).** For
`TagEncoding::Niche { untagged_variant, niche_variants,
niche_start }`, each variant's `tag_value` is computed as:

```text
For v.idx in niche_variants (inclusive range):
  raw = niche_start + (v.idx - niche_variants.start.idx)
  v.tag_value = Some(raw mod 2^scalar_width)

For v.idx == untagged_variant:
  v.tag_value = None             // payload IS the tag

For v.idx outside niche_variants and != untagged_variant:
  // unreachable: niche_variants must enumerate every non-untagged
  // variant; bridge construction enforces this.
```

`scalar_width` is the bit width of `TagScalar.scalar_ty`
(Bool = 8, Char = 32, Int(Ix)/Uint(Ux) natural width, Ptr/Ref
`target.pointer_width`). The `mod 2^scalar_width` step
implements wrapping behaviour at the scalar boundary;
combined with the `valid_range` invariant on `TagScalar`,
codegen knows exactly which bit pattern to write for each
variant. `valid_range` describes the range of *valid niche
encodings* — every `tag_value` produced by the formula must
fall inside `valid_range` (modulo wrapping for ranges where
`start > end`).

**Pointer/reference tag scalars.** When `TagScalar.scalar_ty`
resolves to `Ty::Ptr(_, _)` or `Ty::Ref(_, _)`, `valid_range`,
`niche_start`, and `tag_value` are stored as `u128` and
interpreted at codegen time as truncated to `target.pointer_width`
bits (§3 `TargetInfo`). The canonical encoding for niche-
optimised null-pointer cases (e.g., `Option<&T>` with
`niche_start = 0`, `valid_range = 1..=usize::MAX`) emits the
discriminant-write as integer-zero cast to the pointer's C
type rather than a typed pointer constant.

**Width-fit invariant.** `valid_range.start`,
`valid_range.end`, `niche_start`, and per-`VariantLayout.tag_value`
must all fit the bit width of `scalar_ty` (modulo
wrapping-range semantics for `valid_range`). Bit width per
`scalar_ty` variant: `Bool` is 8 (storage); `Char` is 32
(Unicode scalar value width); `Int(Ix)`/`Uint(Ux)` is the
natural integer width; `Ptr`/`Ref` is `target.pointer_width`.
Validation rejects out-of-range stored values.

**Codegen rule for invalid-bit-pattern niche storage.** When
`TagScalar.scalar_ty` is `Bool` or `Char`, niche encoding may
write bit patterns outside the *logical value set* (e.g.,
`Option<bool>` uses byte values `2..=255` as `None` niche;
C `_Bool` would normalise these to `0` or `1` on assignment).
For tag/niche read and write paths, codegen emits the storage
lvalue as an **unsigned integer of the scalar's storage
width** — `uint8_t` for `Bool`, `uint32_t` for `Char` — not
as the logical C type (`_Bool` / Unicode scalar). The
*logical* type remains `Bool` / `Char` in the IR for type-
identity and structural purposes; only the *storage emission*
at niche read/write sites uses the unsigned-integer alias.
This applies only to enum tag/niche operations; ordinary
field reads/writes of `Bool` / `Char` use their logical C
type.

### 11.3 What's dropped from rustc's `LayoutData`

| Dropped                                                    | Why                                                                                                                            |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `BackendRepr`                                              | Rust calling-convention hint for layout reproduction; ABI consumers query the bridge's C-ABI predicate at translation (§14.5)  |
| `max_repr_align`, `unadjusted_abi_align`                   | layout-algorithm intermediates                                                                                                 |
| `randomization_seed`                                       | nightly perf knob                                                                                                              |
| `largest_niche`                                            | parent-only consumer; already baked into `tag_encoding`                                                                        |
| `ReprOptions::align`, `int`, `IS_C`/`IS_TRANSPARENT` flags | resolved into the layout bytes for layout reproduction; the bridge consults `adt_def.repr()` directly at C-ABI translation     |
| `IS_SIMD`/`IS_LINEAR`/`IS_SCALABLE`                        | DSL out of scope; rejected at bridge                                                                                           |

### 11.5 Size and alignment query

`Layout` lives inline only on `Ty::Struct`/`Enum`/`Union`/`Tuple`.
Validation (§12.5 layout sanity), codegen, and consumer-side
analyses also need sizes/alignments for non-ADT types. The model
exposes:

```rust
pub struct SizeAlign {
    pub size:  u64,    // bytes
    pub align: u64,    // bytes; power of two
}

impl RegseqModel {
    pub fn size_align_of(&self, ty: TyId) -> SizeAlign;
}
```

Rules per `Ty` variant:

| Variant                                              | Rule                                                                                       |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `Struct`/`Enum`/`Union`/`Tuple`                      | read inline `Layout.size` and `Layout.align`                                               |
| `Array(elem, n)`                                     | `stride(elem) * n`, `align(elem)`. Stride rounds element size up to a multiple of element alignment: `stride = round_up_to_multiple(size: size(elem), boundary: align(elem))`. Validation: `checked_mul(stride, n)` must not overflow `u64` and the product must not exceed `target.pointer_width`-bit `usize::MAX` for the model's target; otherwise the model is rejected. |
| `Ref(t, _)` / `Ptr(t, _)` to **sized** pointee       | read `target.scalar_layouts.data_ptr` — thin pointer                                       |
| `Ref(t, _)` / `Ptr(t, _)` to **unsized** pointee     | `(2 * data_ptr.size, data_ptr.align)` where `data_ptr = target.scalar_layouts.data_ptr` — fat pointer (two slots) |
| `FnPtr(_)`                                           | read `target.scalar_layouts.fn_ptr`                                                        |
| `FnDef(_, _)`                                        | `(0, 1)` — ZST                                                                             |
| `Bool`                                               | read `target.scalar_layouts.bool_`                                                         |
| `Char`                                               | read `target.scalar_layouts.char_`                                                         |
| `Int(k)`                                             | read `target.scalar_layouts.int[k as usize]`                                               |
| `Uint(k)`                                            | read `target.scalar_layouts.uint[k as usize]`                                              |
| `Float(k)`                                           | read `target.scalar_layouts.float[k as usize]`                                             |
| `Never`                                              | `(0, 1)` — uninhabited; never instantiated                                                 |
| `Str`, `Slice(_)`, `Dynamic{..}` (queried directly)  | **panic / debug-assert**; unsized markers must only be queried via the enclosing `Ref`/`Ptr` |

"Sized pointee" / "unsized pointee" classification: the pointee
is unsized iff it is `Str`, `Slice(_)`, or `Dynamic{..}`. All
other types (including `Array(_, _)` and any sized ADT) are sized.

All primitive sizes/aligns come from
`target.scalar_layouts` (§3, populated by the bridge from
rustc at extraction time). Consumers do **not** parse
`target.data_layout` to derive scalar layouts;
`scalar_layouts` is the authoritative source. `data_layout`
remains for diagnostics and target-identity verification.

---

## 12. Type interning

**Identity model (private interner key + public full-value
equality).** R2.9 declined a *spec-level* `TyIdentityKey` type
(no extracted public identity surface). The Round 9 refinement
preserves that decision while removing the API hazard of
identity-only public `Eq`/`Hash` on `Ty`:

- The interner uses a **private** `TyIdentityKey` for its
  `lookup: HashMap<TyIdentityKey, TyId>` map. `TyIdentityKey`
  is internal to `regseq-model` (not exposed in the public
  spec surface — consistent with R2.9's lock against a public
  identity type). It encodes the §12.1 nominal/structural
  identity rules (layout excluded from nominal ADTs and
  tuples).
- Public `Ty` derives ordinary `PartialEq` / `Eq` / `Hash` —
  **full-value equality**, including inline `Layout`. Two
  `Ty::Struct { item, args, layout }` values with the same
  `(item, args)` but different `layout` are **not** publicly
  equal under `==`. This matches the equality semantics
  downstream consumers naturally expect from Rust
  `#[derive(Eq, Hash)]` enums; no surprising
  identity-only-equality behaviour leaks across the crate
  boundary.
- Self-referential ADT reservation still works (§12.2): the
  bridge constructs a `TyIdentityKey` from `(item, args)`
  pre-layout-resolution, reserves the `TyId`, then patches
  the stored `Ty.layout` once computed. Because
  `TyIdentityKey` excludes layout, the reservation key is
  stable across the patch.
- Identity-keyed lookup remains available through
  `TyInterner::find` and `TyInterner::find_nominal` (§12);
  consumers do not need to construct a `TyIdentityKey`
  directly. Identity comparisons use `TyId` equality (`a == b`
  for `TyId` is identity by construction, since the
  content-addressing interner ensures equal canonical content
  → equal `TyId`).

This refinement is **consistent with R2.9**, not a relitigation:
R2.9 was about whether the public spec exposes a
`TyIdentityKey` type; the answer is still no. The internal
`TyInterner.lookup` mechanism is implementation detail.

```rust
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup:  HashMap<TyIdentityKey, TyId>,    // private key per Round 9 refinement
}

// Private to the regseq-model crate. Encodes the §12.1
// nominal/structural identity rules: ADT-shaped variants
// hash by `(variant_kind_tag, item, args)` (layout excluded);
// `Tuple` hashes by `(variant_kind_tag, fields)` (layout
// excluded); other variants hash structurally over their fields.
struct TyIdentityKey { /* implementation-private */ }

pub enum NominalKind { Struct, Enum, Union, FnDef, Closure }

impl TyInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, ty: Ty) -> TyId;
    pub fn resolve(&self, id: TyId) -> &Ty;
    pub fn find(&self, ty: &Ty) -> Option<TyId>;   // identity lookup; non-mutating
    pub fn find_nominal(
        &self,
        kind: NominalKind,
        item: ItemId,
        args: &GenericArgs,
    ) -> Option<TyId>;                              // (kind, item, args)-keyed lookup
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (TyId, &Ty)>;
}
```

Two collections (BiMap) for amortized-O(1) on both `resolve` and
`intern`. The bridge holds an additional transient
`HashMap<rustc::Ty<'tcx>, TyId>` translation cache (forced bridge-
side by Q1; dies with the bridge call per Q2).

`find_nominal` is the layout-agnostic reverse lookup for nominal
identities — needed by callers who know `(kind, item, args)` but
do not yet hold a concrete `Ty` (notably the closure
identity-view-to-layout-view navigation, §13.1, and vtable type-
Item lookup, §13.2). Implementation constructs a private
`TyIdentityKey` from `(kind, item, args)` directly and looks
it up in the interner; callers do not need to materialise a
concrete `Ty` (or its `Layout`) for the lookup to succeed.

### 12.1 Identity rules (private `TyIdentityKey` shape)

The private `TyIdentityKey` (introduced in Round 9 per F8) is
the interner's lookup key. It encodes the per-variant identity
rule per the buckets below. Public `Ty` has derived
`PartialEq`/`Eq`/`Hash` (full-value equality including
`Layout`); identity equality is *not* exposed on `Ty` itself.

| Bucket                                      | Variants                                                             | `TyIdentityKey` shape                 | Layout role                                                          |
| ------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------- |
| **Nominal**                                 | `Struct`, `Enum`, `Union`, `FnDef`, `Closure`                        | `(variant_kind_tag, item, args)`      | inline `Layout` excluded (functionally determined by `(item, args)`) |
| **Tuple** (structural-with-layout-excluded) | `Tuple`                                                              | `(variant_kind_tag, fields)`          | inline `Layout` excluded (functionally determined by `fields`)       |
| **Other structural**                        | Bool/Char/Int/Uint/Float/Str/Array/Slice/Ptr/Ref/FnPtr/Dynamic/Never | full structural recursion over fields | no inline layout                                                     |

The `variant_kind_tag` is load-bearing: it makes
`Ty::Closure(item, args)` and `Ty::Struct { item, args, layout }`
for the same closure **distinct entries** in the interner
(closures have an identity view + a layout-bearing view sharing
one `Item`).

### 12.2 Self-referential ADTs

Self-referential ADTs intern via TyId reservation: parent's
`TyId` is reserved before its layout is computed; recursive
references hit the reservation via the nominal hash. E.g., for
`struct Node { next: *mut Node }`:

1. Reserve `Ty::Struct { item: NodeItem, args: [], layout:
   PENDING }` at `TyId(K)`.
2. Compute layout for `Node`. Field `next: *mut Node` recurses.
3. The recursive `Node` lookup hits the nominal-identity
   reservation at `TyId(K)`.
4. Wrap as `Ty::Ptr(TyId(K), Mutability::Mut)` at `TyId(K+1)`.
5. `next` field's `FieldLayout.ty = TyId(K+1)`.
6. Patch `storage[K].layout = computed_layout`.

### 12.3 `ArgsKey` deep canonicaliser

F4-2 instance ordering depends on a deterministic generic-args
fingerprint. `ArgsKey` is also stored in serialised model state
(`SyntheticItemKind` payloads, §13.3), so it must be a concrete,
serializable value type with stable cross-build identity.

#### `ArgsKey` value type

```rust
pub struct ArgsKey(pub u128);    // 128-bit content hash
```

`ArgsKey` is a **128-bit content hash** of the canonical byte-
encoding of a `GenericArgs` value (or any other tree of `Ty` /
`Const` content the bridge canonicalises). The hash function is
**SHA-256 of the canonical byte-encoding, truncated to the
leading 16 bytes (the first 128 bits of the SHA-256 digest in
big-endian byte order)**, then loaded as a `u128` in big-endian
order. SHA-256 is chosen for portability (every conforming
implementation produces identical digests), cryptographic-grade
collision resistance (well beyond what hash-collision
adversaries can mount), and zero implementation ambiguity. The
same digest is used by the §13.4 synthetic `DefPathHash`
construction (also truncated to its respective width).

**Collision model (honesty note).** `ArgsKey` is a **stable
content fingerprint**, not a proof of equality. The serialised
model stores only the `u128`, not the canonical bytes. A
deserialised consumer cannot mechanically validate that two
syntactically-distinct canonical forms did not collide; collision
detection requires retaining canonical bytes (which the IR does
not). At 128 bits with SHA-256 truncation, the collision
probability for any realistic driver-crate volume is below
`2^-50` (birthday-bound estimate: ~`2^64` distinct canonicalisations
to reach 50% collision probability). Regseq treats truncated
SHA-256 collision as outside the threat model. Implementations
MAY (in debug builds, for testing only) retain canonical bytes
in a side table and validate collision-freedom at construction
time; this is not part of the serialised IR contract.

**Canonical byte form of `ArgsKey` (for embedding in other hash
inputs).** When `ArgsKey` appears as a payload component of
another hash (e.g., the synthetic `DefPathHash` construction in
§13.4), its byte form is the **16 leading bytes of the SHA-256
digest in their original digest order** — equivalently
`ArgsKey.0.to_be_bytes()` (the `u128` serialised big-endian).
The `ArgsKey hex` rendering used in §13.4 DefPath segments is
the lower-case hex of the same 16 bytes. This pins the encoding
so two implementations produce byte-identical synthetic payloads
and DefPath segments.

#### Canonical byte-encoding

The encoding is a tagged-tree byte stream. Each node emits a
1-byte variant tag followed by tag-specific payload bytes; all
multi-byte integers are little-endian; lengths emit as
little-endian `u32` prefixes; nested nodes recurse.

Per-IR-identifier termination rules (the canonicaliser's
walk):

| Inside             | Rule                                                                                                                         |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| ADT-shaped variant | encode as `(variant_kind_tag, DefPathHash, canonical_args)`; layout NOT traversed (cycle-breaking + functionally determined) |
| Other variant      | structural recursion over fields                                                                                             |
| `TyId` field       | resolve via `model.types.resolve(...)` and recurse on the resolved `Ty`                                                      |
| `ItemId`           | replace with `DefPathHash` from `model.item_def_paths` (cross-build terminator)                                              |
| `FnSigId`          | resolve and **expand** FnSig content (see §14.3 minimum schema)                                                              |
| `StringId`         | resolve to content (UTF-8 bytes, length-prefixed)                                                                            |
| `ConstId`          | canonical const-eval result (stage-4 schema)                                                                                 |
| `DefPathHash`      | terminal: emit the 16-byte content (StableCrateId u64 LE + local_hash u64 LE)                                                |

Per-`Ty`-variant byte-encoding tags (for the
`canonical_args.recurse(TyId)` step) — these are append-only
constants pinned for the v1 type universe:

```text
variant_kind_tag = u8
  0  = Bool
  1  = Char
  2  = Int(IntKind)        followed by u8 IntKind discriminant
  3  = Uint(UintKind)      followed by u8 UintKind discriminant
  4  = Float(FloatKind)    followed by u8 FloatKind discriminant
  5  = Str
  6  = Never
  7  = Array               followed by canonical_args(elem) + u64 LE n
  8  = Slice               followed by canonical_args(elem)
  9  = Ref                 followed by canonical_args(pointee) + u8 Provenance discriminant (§10.1)
 10  = Ptr                 followed by canonical_args(pointee) + u8 Mutability
 11  = FnDef               followed by DefPathHash(item) + canonical_args(GenericArgs)
 12  = FnPtr               followed by canonical FnSig content (§14.3)
 13  = Dynamic             followed by sorted Vec<u8 AutoTrait> + canonical_args(vtable_ty)
 14  = Closure             followed by DefPathHash(item) + canonical_args(GenericArgs)
 15  = Tuple               followed by u32 LE field_count + per-field canonical_args(TyId)
 16  = Struct              followed by DefPathHash(item) + canonical_args(GenericArgs)
 17  = Enum                followed by DefPathHash(item) + canonical_args(GenericArgs)
 18  = Union               followed by DefPathHash(item) + canonical_args(GenericArgs)
```

The encoding is **append-only**: new `Ty` variants admitted by
future spec revisions append at the next free tag value;
existing tags remain immutable so cross-build hashes for
already-admitted types never change.

#### Cross-build determinism

Two builds satisfying the build-equivalence quad (§12.4)
produce identical `ArgsKey` values for canonically-equal
generic-args trees. The byte-encoding never references local
allocation-order IDs (TyId / FnSigId / ConstId / StringId
integers are resolved to their content before encoding).

#### Distinct from `TyInterner` identity (per-build) lookup

`TyInterner.lookup` (§12) uses the private `TyIdentityKey`
(§12.1) which compares nested `TyId` integers directly via
the per-variant identity rules — sound within a single build
because content-equal Tys share TyIds (the interner's
content-addressing property). `ArgsKey` is the **deep**
content-addressed identity used for F4-2 sort keys (§16.3)
and synthetic-item identity (§13.3) — it walks `Ty` content
recursively, resolving every nested ID to its content per
the termination rules above. Both are correct for their
respective scopes: `TyIdentityKey` for interner dedup
(per-build, shallow because TyIds are content-addressed
within a build); `ArgsKey` for cross-build identity (deep,
content-addressed across builds).

### 12.4 Cross-build property

All claims below hold under the **build-equivalence quad**: equal
source, equal pinned toolchain, equal `RegseqModel.target`
(§3 `TargetInfo`), equal bridge options (cleanup-pass whitelist
and the methodology-bound `mir_opt_level=0`). Two builds that
disagree on any quad component are not "the same build" for
determinism purposes.

| Property                         | Status                                                                     |
| -------------------------------- | -------------------------------------------------------------------------- |
| Set of distinct `Ty` values      | **identical** across builds under the build-equivalence quad               |
| Each `Ty`'s structural shape     | **identical**                                                              |
| Each `Layout`                    | **identical** (target-dependent; quad's `target` equality is load-bearing) |
| `InstanceId` / `ItemId` ordering | **deterministic** per F4 / F4-2 (computed over `Ty` content, not integers) |
| `TyId` integer values            | **NOT guaranteed stable** — bridge discovery walk may differ between runs  |
| Cross-build identity check       | use the deep `ArgsKey` canonicaliser (compares `Ty` content)               |
| Snapshot diffing                 | content-based via the pretty-printer (resolves `TyId` to `Ty`)             |

A bridge implementation MAY produce reproducible `TyId`
integers as a recommended deterministic property, but no IR
contract relies on it.

### 12.5 Serialisation

`TyInterner` implements `Serialize` / `Deserialize` manually,
mirroring `StringInterner`. The on-wire form is `storage` only
(a flat array of `Ty` values); `lookup` is reconstructed at
deserialise by direct insertion (NOT via the public `intern`
API), preserving the original `TyId` values.

Deserialise validation:

1. Read full `storage` array first. Forward references permitted
   (ADT layouts may reference TyIds at higher indices than the
   ADT itself).
2. Validate references after the full array is loaded. Every
   ID referenced inside any `Ty` satisfies its table bound:
   `TyId` against `storage.len()`; `ItemId` against
   `model.items.len()`; `FnSigId`, `ConstId`, `StringId`,
   `SourceFileId` against their respective interners; `VariantIdx`
   against the parent enum's variant count.
3. Validate content-uniqueness under the nominal/structural
   identity rule (§12.1).
4. Validate layout sanity (mechanical checks):
   - **Variant ↔ kind matching**: `Ty::Struct`'s `layout.kind`
     must be `LayoutKind::Struct`; `Ty::Tuple` likewise.
     `Ty::Union`'s `layout.kind` must be `LayoutKind::Union`
     and every union field offset is `0`. `Ty::Enum`'s
     `layout.kind` must be `LayoutKind::Enum`.
   - **Tuple field-name convention**: for `Ty::Tuple`, every
     `FieldLayout.name` resolves to `_0`, `_1`, ..., `_{n-1}`
     in MIR-index order (matching the synthetic-name
     convention in §10).
   - **Field reachability**: every `FieldLayout.ty` resolves
     to a valid `TyId`; every `FieldLayout.name` resolves via
     `StringInterner`; every `VariantIdx` referenced by an
     `EnumShape` is within the variant count of that enum's
     `Vec<VariantLayout>`.
   - **Field-fits-in-layout**: for every non-ZST
     `FieldLayout`, `checked_add(offset, size_of(ty))` must
     not overflow `u64` AND the result must be `<=
     layout.size` (modulo permitted ZST tail-pad). The
     `size_of` is per `size_align_of` (§11.5).
   - **Alignment respected** (mirroring §3.3 Phase B's
     packed-aware rule, which is the normative formula):
     `required_field_align = match layout.pack { None =>
     align_of(field.ty), Some(n) => min(align_of(field.ty),
     n) }`; every `FieldLayout.offset` must satisfy
     `field.offset % required_field_align == 0`. Validation
     rejects unaligned offsets. Note: a `repr(packed(2))`
     `u32` field requires `offset % 2 == 0`, **not** any
     arbitrary byte offset — the earlier wording allowing
     `pack <= align_of(field.ty)` was too weak.
5. Rebuild `lookup` by direct insertion preserving original
   `TyId` values.

---

## 13. Synthesised Items (shape)

The bridge synthesises Items for constructs without source-level
`DefId`s. Two principal families: closure family and vtable
family. Shape commitments are pinned below.

### 13.1 Closure family

Per closure encountered in MIR, the bridge synthesises:

- **One captures-struct Item** (`ItemKind::Struct`). The closure
  expression has its own rustc `DefId` (with the
  `DefPathData::Closure` segment carrying a disambiguator that
  distinguishes sibling closures within one function), so the
  captures Item is **source-derived**: `def_path` is rustc's
  closure DefPath verbatim and `item_def_paths[id]` is rustc's
  `tcx.def_path_hash(closure_did)`. The Item is marked in
  `item_synthetic_kinds` as
  `SyntheticItemKind::ClosureCaptures` — but this is purely a
  discriminator (so consumers can recognise closure captures
  without walking `DefPath.segments`); the underlying identity
  is rustc's, not synthetic. This is the only `SyntheticItemKind`
  variant whose Item carries a rustc-derived (rather than
  synthetic) `DefPathHash`; invariant 2 (§3.1) carves out this
  exception. Fields = the captured upvars (per rustc's
  `tupled_upvars_ty()`), flattened to named fields (`_0`,
  `_1`, ...). Layout: `tcx.layout_of(closure_ty)`.

- **1-3 call-method Function Items** based on the closure's
  `ClosureKind`:
  - `FnOnce`: `call_once` only.
  - `FnMut`: `call_mut` plus inherited `call_once`.
  - `Fn`: `call` plus inherited `call_mut` and `call_once`.
  Each Function Item's signature is substituted from the
  closure's `closure_sig_as_fn_ptr_ty`, with a per-call-kind
  receiver shape:
  - `Call`     → receiver `Ref(captures_struct, SharedBorrow)`
                 (i.e., `&Captures`).
  - `CallMut`  → receiver `Ref(captures_struct, UniqueBorrow)`
                 (i.e., `&mut Captures`).
  - `CallOnce` → receiver `captures_struct` by value.
  These are the **type-system signatures**. A C-codegen lowering
  may choose a pointer receiver for by-value `FnOnce` for
  implementation convenience, but that is a body/codegen
  decision and does not change the type-system signature carried
  in `model.sigs[id]`.

- **A drop-glue Function Item** iff the captures need dropping.

- **Call-method ABI is `FnAbi::Rust`.** Rustc's source-level ABI
  for closure call methods is `RustCall` (the closure-trait
  tuple-receiver convention), which §14's `FnAbi` alphabet
  rejects. The bridge **rewrites** synthesised call-method
  Function Items to `FnAbi::Rust` after the receiver/argument
  tuple normalisation: the captures-struct is the receiver
  directly (not wrapped in a `RustCall` tuple) and call
  arguments are ordinary positional parameters (not packaged
  into a tuple). Post-rewrite, `FnAbi::Rust` is the correct
  calling convention; type-identity is preserved because
  `RustCall` does not appear in the IR.

- `Coroutine` / `CoroutineClosure` / `CoroutineWitness` are
  rejected.

#### Closure `Ty` views

A closure has two `Ty` entries sharing one `Item`:

- `Ty::Closure(item, args)` — closure-type-identity view (used
  in fn signatures and Place types).
- `Ty::Struct { item, args, layout }` — captures-struct
  layout-bearing view.

Reached from `Ty::Closure(item, args)` to the parallel
`Ty::Struct` via
`model.types.find_nominal(NominalKind::Struct, item, args)`
(§12). `find_nominal` is layout-agnostic — nominal identity
ignores the `Layout` field per §12.1, so the caller does not
need a placeholder `Ty::Struct` value to perform the lookup.
The two views remain distinct entries in the BiMap because
`variant_kind_tag` is part of nominal identity.

### 13.2 Vtable family

For trait objects (`Ty::Dynamic`), the bridge synthesises:

- **Vtable type Item** (`ItemKind::Struct`), per
  `PrincipalTraitKey` (§13.3 — `(trait_path, trait_args,
  projection_bindings)`) reachable via `Ty::Dynamic`
  construction. Generic trait objects like `dyn Register<u32>`
  and `dyn Register<u64>` differ only in `trait_args` and
  therefore get distinct vtable type Items. Marked in
  `item_synthetic_kinds` as
  `SyntheticItemKind::VTableType { principal: PrincipalTraitKey }`
  (§13.3). DefPath rendered form (display only):
  `<vtable_ty for Trait[<trait_args>][<projection_bindings>]>`.
  Field sequence is rustc's `tcx.vtable_entries(trait_ref)`
  order: drop-glue ptr field + size + align + per-method
  fn-ptr fields + supertrait vtable ptrs + Vacant slots for
  non-dispatchable methods.
  - **Erased receiver type is uniformly `*mut ErasedPointee`**,
    regardless of the source borrow flavor (where
    `ErasedPointee` is defined below). Shared `&self` methods
    on impls whose `Self` contains `UnsafeCell` legitimately
    mutate through `&self` — emitting `*const ErasedPointee`
    for the shared receiver would be C undefined behaviour on
    the actual write. The Rust borrow flavor is preserved at
    the type-system level (the source `Ref(t, SharedBorrow)`
    is still visible), but the C-level erased dispatch surface
    drops the `const` qualifier to be sound across all impls.
    See §17.2.1 "Trait-object dispatch carve-out."
  - **Canonical erased pointee.** The erased receiver and
    trait-object `data` field reference the canonical erased
    unit type: `Ty::Tuple { fields: vec![], layout:
    UNIT_LAYOUT }` (§11). When this exact pointer-to-unit
    pattern occurs in vtable-erased positions, codegen emits
    `void *` (per the §17.2.1 trait-object dispatch carve-out)
    rather than a typed pointer to an empty synthetic struct.
    This avoids reintroducing `core::ffi::c_void` (rejected
    per §15.1) while giving the bridge a stable `Ty` to point to.
  - **Vacant slot canonical type.** Vacant vtable slots are
    explicit placeholder fn-pointer fields with a fixed null
    initialiser, NOT omitted. Each vacant slot's field type is
    the **canonical vacant signature**:
    `Ty::FnPtr(canonical_vacant_sig_id)` where
    `canonical_vacant_sig_id` references a single shared
    `FnSig` entry with `inputs: [Ty::Ptr(unit, Mutability::Mut)]`,
    `output: unit`, `abi: FnAbi::C`, `c_variadic: false`,
    `safety: Safety::Safe`. (Equivalent to `extern "C" fn(*mut ()) -> ()`.)
    Pinning the canonical signature ensures vacant slots have a
    well-typed shape for layout, the pretty-printer, and constant
    schemas. Slot size/align match other fn-pointer fields
    (target pointer width). Method-index rewriting reads the slot
    type uniformly regardless of vacant/non-vacant status.

  - **Method shim and drop shim ABI is `FnAbi::C` uniformly.**
    All bridge-synthesised vtable shim Function Items (method
    shims and the drop-in-place shim) use `FnAbi::C`,
    matching the vacant-slot canonical signature and the C
    examples in Appendix A.6. Their erased signatures are
    checked with `CAbiPositionAdmissible` (§14.5) **after**
    receiver erasure to `*mut ()`. A trait whose dispatchable
    method's signature (with the source receiver replaced by
    the erased receiver `*mut ()`) is not C-ABI-position-
    admissible is **rejected at trait-object translation**
    with `BridgeError::UnsupportedTraitObjectMethod { trait_path,
    method_def_path, ty }`. This pins the v1 contract that
    Regseq traits used as `dyn Trait` are C-shaped surfaces
    (matching the OS-integration role of the trait family).

    **Regseq facade trait-surface design rule (consequence).**
    Trait families intended for `dyn Trait` use at OS
    integration boundaries (e.g., `Lifecycle`, callback tables)
    must use C-ABI-admissible erased method surfaces:
    fieldless `repr(int)` enums for status codes, primitive
    return types, `repr(C)` aggregates, raw pointers / refs to
    sized pointees. Default Rust enums (`Result<T, E>`,
    `Option<T>`) are *rejected* at the trait-object boundary
    by the predicate above — facade authors targeting `dyn`
    use must define explicit `repr(int)` status types or use
    primitive status codes. Static-dispatch trait usage
    (non-`dyn`) is unrestricted: ordinary Rust signatures
    work because the call lifts to a direct Function Item
    invocation, not through a vtable shim. The constraint
    applies only at the trait-object boundary.

#### Synthetic vtable layout algorithm

Vtable type Items have **no rustc `Ty`** — they are
bridge-synthesised. The §11 rule "Layout copied from
`tcx.layout_of`" therefore does not apply to them. Vtable
`Layout` is computed by the bridge per the following algorithm
(closed; deterministic; rustc-free post-construction):

1. **Representation:** `repr(C)`-like aggregate (sequential
   field order, no Rust reordering).
2. **Field order:** exactly `tcx.vtable_entries(trait_ref)`
   order. Vacant entries occupy their slot in source order
   (no compaction).
3. **Per-field `(size, align)`:** read from the field's `Ty`
   via `size_align_of` (§11.5):
   - drop-glue ptr field: pointer-width slot
     (`target.scalar_layouts.fn_ptr`).
   - size and align fields: target-width unsigned slot
     (`target.scalar_layouts.uint[Usize]`).
   - per-method fn-ptr fields: pointer-width slot.
   - supertrait vtable ptr fields: pointer-width slot.
   - Vacant slots (canonical fn-ptr type above):
     pointer-width slot.
4. **Field offsets:** computed by standard C struct layout
   under `TargetInfo` — each field placed at the next offset
   that is a multiple of `align(field.ty)`, with explicit
   padding bytes inserted as needed.
5. **Aggregate alignment:** `max(align(field.ty)) for each
   field`.
6. **Aggregate size:** the offset after the last field, rounded
   up to aggregate alignment.
7. **`pack` field:** `None` (vtable types are not packed).
8. **`uninhabited` flag:** `false` (vtable instances are
   inhabited static memory).
9. **Field naming convention** (`FieldLayout.name` for each
   slot, in `tcx.vtable_entries` order):
   - drop-glue slot: `"__drop_in_place"`.
   - size slot: `"__size"`.
   - alignment slot: `"__align"`.
   - method slots (per dispatchable method): the source
     method's name (e.g., `"read"`, `"write"`); for two
     methods with the same name (e.g., supertrait shadowing
     edge cases), append `"#N"` disambiguator with N the
     vtable_entry_index.
   - supertrait vtable pointer slots: `"__super_TraitName"`
     where `TraitName` is the rendered short name of the
     supertrait; ambiguity disambiguated by appending the
     supertrait's vtable_entry_index.
   - vacant slots: `"__vacant_N"` where `N` is the
     vtable_entry_index.
   The names are stable per `(PrincipalTraitKey,
   ConcreteImpl)`; consumers (codegen, pretty-printer,
   snapshot tests) get reproducible field identifiers.

The synthesised `Layout` is fully resolved and rustc-free —
identical in shape and content to a layout produced by
`tcx.layout_of` for a hand-written `repr(C)` struct with the
same field sequence. The "provenance" (synthesised vs
rustc-derived) is location-pinned: §11 governs source-derived
types; §13.2 governs vtable types. No explicit
`LayoutProvenance` tag is stored; the bridge knows by
construction site.

- **Vtable instance Item** (`ItemKind::Static`), per
  `(PrincipalTraitKey, ConcreteImpl)` reachable (via
  `Cast::Unsize` rvalues at coercion sites). Marked in
  `item_synthetic_kinds` as
  `SyntheticItemKind::VTableInstance { principal:
  PrincipalTraitKey, self_ty: ArgsKey }` (§13.3). DefPath
  rendered form (display only): `<vtable for Trait for
  SelfTy>`. The instance value references vtable shim
  Function Items plus const size/align values, in field
  order.

- **Vtable shim Function Items** in two families, both
  motivated by the same erased-vs-concrete receiver mismatch:

  - **Method shims**, per `(PrincipalTraitKey, ConcreteImpl,
    method)`. Marked in `item_synthetic_kinds` as
    `SyntheticItemKind::VTableMethodShim { principal:
    PrincipalTraitKey, self_ty: ArgsKey, vtable_entry_index:
    u32, method_def_path: DefPathHash }` (§13.3). The
    `vtable_entry_index` is the position in the
    `tcx.vtable_entries(trait_ref)` sequence (the actual
    vtable struct field index — including drop-glue/size/align
    slots and supertrait pointers, NOT just dispatchable
    methods); body-level dispatch indexes into the vtable by
    this value. The `method_def_path` carries the source
    method's stable identity for diagnostics and cross-build
    alignment. Body: takes erased receiver `*mut ()` (uniform
    per the vtable receiver rule above), casts to `&mut SelfTy`
    (or `&SelfTy` per the source method receiver), tail-calls
    the lifted impl method. **Safety pinning:** the shim's
    `FnSig.safety` mirrors the source method's safety
    (`Safe` for ordinary methods; `Unsafe` for source `unsafe
    fn` methods). Two trait objects whose shims differ only by
    safety would otherwise share a `FnSigId` and collapse.
  - **Drop-in-place shim**, per `(PrincipalTraitKey,
    ConcreteImpl)`. Marked as
    `SyntheticItemKind::VTableDropShim { principal:
    PrincipalTraitKey, self_ty: ArgsKey }` (§13.3). Body: takes
    erased receiver `*mut ()`, casts to `*mut SelfTy`,
    tail-calls `core::ptr::drop_in_place::<SelfTy>` (rustc's
    `DropGlue` Instance for `SelfTy`). Signature:
    `fn(*mut ())` matching the vtable's `drop_in_place` slot.

  Without method shims, vtable method-slot initialisers cannot
  type-check. Without the drop-in-place shim, the vtable's drop
  slot has the same problem: rustc's `tcx.vtable_entries`
  resolves the drop entry to concrete `drop_in_place::<SelfTy>`,
  whose `fn(*mut SelfTy)` signature does not type-equal the
  vtable slot's `fn(*mut ())`. Both shim families bridge the
  erased-receiver gap with identical synthesis machinery; only
  the tail-call target differs.

#### Vtable dedup table

`RegseqModel.vtables: HashMap<PrincipalTraitKey, ItemId>`.
Bridge-side; dedupes vtable type Items per
`PrincipalTraitKey` (§13.3 — captures `trait_path`,
`trait_args`, and `projection_bindings` so generic trait
objects like `dyn Register<u32>` and `dyn Register<u64>` get
distinct vtable types). Marked `#[serde(skip)]`; rebuilt on
deserialise (§3.3 Phase C) by walking `item_synthetic_kinds`
for entries matching `SyntheticItemKind::VTableType
{ principal }` and re-keying each as `principal -> ItemId`.
**Duplicate detection is mandatory:** two `VTableType` Items
sharing a `PrincipalTraitKey` is a model-corruption error (the
bridge dedupes at construction, so duplicates can only arise
from tampered serialised data); Phase C raises an error
rather than silently overwriting. The structured-metadata
side table is the round-trip anchor; the rendered DefPath
form (`<vtable_ty for ...>`) remains as a display-only
convention for snapshots and diagnostics.

### 13.3 `SyntheticItemKind` and `PrincipalTraitKey`

Per-`ItemId` discriminator carried in the `item_synthetic_kinds`
side table on `RegseqModel` (§3, §3.1 invariant 2), plus the
trait-object identity carrier used by the vtable family.

```rust
pub struct PrincipalTraitKey {
    /// rustc `DefPathHash` for the trait `DefId`. This need
    /// **not** correspond to an `ItemId` in `model.items`:
    /// trait declarations are erased from the source-level
    /// Item alphabet (§8.1, no `ItemKind::Trait` variant), so
    /// `trait_path` is a stable rustc identifier used for
    /// trait-object identity and vtable dedup but not for
    /// Item table lookup. Implementers must not validate
    /// `trait_path` through `item_def_paths`.
    pub trait_path:          DefPathHash,
    /// Generic args to the trait itself, e.g., the `<u32>`
    /// in `Register<u32>`. Same canonical encoding as any
    /// `GenericArgs` (§12.3 ArgsKey).
    pub trait_args:          ArgsKey,
    /// Associated-type and associated-const projection equality
    /// payload, e.g., the `<Item = u32>` in
    /// `dyn Iter<Item = u32>`. **Distinct type from `ArgsKey`**
    /// because projection bindings are *named* (assoc_item
    /// identity is load-bearing), not anonymous positional args.
    /// See `ProjectionBindingsKey` below.
    pub projection_bindings: ProjectionBindingsKey,
}

pub struct ProjectionBindingsKey(pub u128);    // 128-bit content hash

pub enum SyntheticItemKind {
    /// Source-level item from rustc; `item_def_paths[id]` is
    /// `tcx.def_path_hash(did)` for the originating `DefId`.
    None,

    // Closure family (§13.1, §13.4). Each variant carries the
    // closure expression's rustc DefPathHash inline so the
    // synthetic identity payload is self-contained — consumers
    // and validators do not have to parse `DefPath.segments`
    // hex to recover identity (matching the vtable family's
    // self-contained PrincipalTraitKey pattern).
    ClosureCaptures  { closure: DefPathHash },                              // source-derived; see §13.1 carve-out
    ClosureCall      { closure: DefPathHash, call_kind: ClosureCallKind },  // synthetic
    ClosureDropGlue  { closure: DefPathHash },                              // synthetic

    // Vtable family (§13.2). Identity does NOT include any
    // "triggering crate" — vtable Items are deduped by their
    // semantic identity (PrincipalTraitKey, plus self_ty and
    // vtable_entry_index/method_def_path where relevant), not
    // by discovery order.
    VTableType       { principal: PrincipalTraitKey },
    VTableInstance   { principal: PrincipalTraitKey, self_ty: ArgsKey },
    VTableMethodShim { principal: PrincipalTraitKey, self_ty: ArgsKey, vtable_entry_index: u32, method_def_path: DefPathHash },
    VTableDropShim   { principal: PrincipalTraitKey, self_ty: ArgsKey },
}

pub enum ClosureCallKind { Call, CallMut, CallOnce }
```

**`ProjectionBindingsKey` canonical encoding.** Same hash
construction as `ArgsKey` (SHA-256 truncated to leading 16
bytes, big-endian, loaded as `u128`), but different canonical
input grammar — projection bindings are *named*
`(assoc_item, value)` pairs:

```text
ProjectionBindings canonical byte encoding:
    u32 LE binding_count
    repeated entries, sorted lexicographically (ascending) over the
    16 canonical bytes of `assoc_item DefPathHash` (StableCrateId
    u64 LE + local_hash u64 LE per §7.3 byte form):
        16 bytes  assoc_item DefPathHash (StableCrateId u64 LE
                  + local_hash u64 LE per §7.3 byte form)
        u8        binding_kind:
                    0 = TypeEq      (associated type = T)
                    1 = ConstEq     (associated const = c)
                    // future kinds append
        canonical value bytes:
                    if TypeEq:  ArgsKey-style canonical Ty bytes (§12.3)
                    if ConstEq: ConstIdentityKey bytes (stage 4)
```

Sort order on `assoc_item DefPathHash` ensures that
`dyn Two<A = u32, B = u64>` and `dyn Two<B = u64, A = u32>`
produce identical `ProjectionBindingsKey`s (rustc may present
projection predicates in either order). Conversely,
`dyn Trait<AssocA = u32>` and `dyn Trait<AssocB = u32>` differ
because the assoc_item DefPathHash is part of the key.

Empty projection bindings (no `<Assoc = ...>` clauses) encode
as `u32 LE binding_count = 0`; the resulting hash is the
canonical "empty bindings" `ProjectionBindingsKey`.

**Collision model (honesty note).** Same framing as `ArgsKey`
(§12.3): `ProjectionBindingsKey` is a **stable content
fingerprint**, not a proof of equality. The serialised model
stores only the `u128`, not the canonical projection-binding
bytes. At 128 bits with SHA-256 truncation, collision
probability for any realistic driver-crate volume is below
`2^-50`. Regseq treats truncated SHA-256 collision as outside
the threat model.

**Why `PrincipalTraitKey` not just `trait_path`.** A trait
identity at the trait-object boundary is `(path, generic_args,
projection_bindings)`, not just `path`. For generic traits like
`Register<T>`, the trait's own type arguments distinguish
otherwise-identically-named trait objects:

- `dyn Register<u32>` and `dyn Register<u64>` share `trait_path`
  but have distinct `trait_args` (`[u32]` vs `[u64]`).
- `dyn Iter<Item = u32>` and `dyn Iter<Item = u64>` share
  `trait_path` and `trait_args` but have distinct
  `projection_bindings`.

A key of just `(trait_path, projection_bindings)` would collapse
these into one vtable type — a real identity bug. `PrincipalTraitKey`
captures the full identity. Used in all four vtable-family
synthetic kinds and in `RegseqModel.vtables`.

**Why structured rather than DefPath-prefix parsing.** The vtable
dedup table rebuild (§13.2) and any future analysis that
distinguishes synthesised families needs a stable, machine-
checkable discriminator. Parsing `def_path.rendered` for marker
prefixes conflates display and semantic recovery — `rendered`
is human-facing and may evolve. The structured tag is the
load-bearing recovery mechanism; the DefPath rendered prefix
remains as a display convention.

**Identity carriers in payload fields.** Variants that carry
trait/self-type identity use cross-build-stable forms:
`DefPathHash` for the trait's path; `ArgsKey` (§12.3 — 128-bit
content hash) for the trait args (`PrincipalTraitKey.trait_args`)
and self-ty canonicalisation; `ProjectionBindingsKey` (this
section, distinct from `ArgsKey`) for
`PrincipalTraitKey.projection_bindings` because projection
bindings are *named* `(assoc_item, value)` pairs rather than
positional args. `vtable_entry_index` (on `VTableMethodShim`)
is the position of the slot in the
`tcx.vtable_entries(trait_ref)` sequence (the actual vtable
struct field index, including drop/size/align/supertrait/vacant
slots — not just dispatchable-method rank). `method_def_path`
is the source method's `DefPathHash`, the semantic identity
carrier (rustc-stable per the trait's `def_id`); it survives
slot-reordering edge cases.

**Closed enumeration.** Adding new bridge-synthesis families
(e.g., async-drop shims if Async support lands) extends
`SyntheticItemKind` with new variants append-only — existing
variants and their payload shapes remain stable.

### 13.4 Synthetic `DefPath` segments and `DefPathHash` construction

Synthetic items have structured `SyntheticItemKind`, but they
also need structured `DefPath.segments` (used by F4 sort
comparator, §16.1) and deterministic `DefPathHash` values
(used by `item_def_paths`, §3.1 invariant 2). Both are
specified per-family.

**Scope.** This subsection specifies the synthetic encoding for
items whose Item identity is bridge-synthesised. The closure
captures-struct (§13.1) is **excluded**: it is source-derived
(rustc closure expression has its own `DefId`), so its `DefPath`
and `DefPathHash` come from rustc directly. The
`SyntheticItemKind::ClosureCaptures` discriminator marks the
shape but does **not** trigger synthetic-DefPath construction.
The remaining six synthetic kinds (`ClosureCall`,
`ClosureDropGlue`, `VTableType`, `VTableInstance`,
`VTableMethodShim`, `VTableDropShim`) use the encoding below.

#### Synthetic `DefPath.segments` encoding

Synthetic items use `DefPathData::TypeNs(StringId)` segments
with reserved string contents. The first segment is always
`TypeNs(strings.intern("<regseq_synth>"))` — this is the
domain-separator marker so synthetic Items never alias with
source-derived items even if a future toolchain bump adds a
crate named `regseq_synth`. Subsequent segments encode the
synthetic kind and its identifying payload.

Per-family canonical segments:

```text
Closure family (call methods + drop glue only;
captures are source-derived)
  segments = [
    TypeNs("<regseq_synth>"),
    TypeNs("closure"),
    TypeNs(<closure expression's def_path_hash hex>),    // not the enclosing fn's;
                                                          // each closure expression has
                                                          // its own rustc DefId
    TypeNs("call" | "call_mut" | "call_once" | "drop_glue"),
  ]

Vtable family
  segments = [
    TypeNs("<regseq_synth>"),
    TypeNs("vtable_ty" | "vtable" | "vtable_method_shim" | "vtable_drop_shim"),
    TypeNs(<principal.trait_path hex>),
    TypeNs(<principal.trait_args ArgsKey hex>),
    TypeNs(<principal.projection_bindings ProjectionBindingsKey hex>),
    // Vtable instance / shims only:
    TypeNs(<self_ty ArgsKey hex>),
    // Method shim only:
    TypeNs(<vtable_entry_index decimal>),
    TypeNs(<method_def_path hex>),
  ]
```

The canonical segments give F4 a deterministic per-segment sort
comparator and let the rendered DefPath form (e.g., `<vtable_ty
for Trait[..][..]>`) be derived from the same data. Non-synthetic
items have no `<regseq_synth>` first segment; the
domain-separation is at-a-glance.

**No "triggering crate" in identity.** Synthetic-item identity is
**not** parameterised by which crate "caused synthesis" — that
would make vtable identity depend on discovery order, since the
same vtable can be required by multiple crates (primary spec, the
`regseq` facade, regseq-aware deps). `RegseqModel.vtables` already
dedupes by `PrincipalTraitKey`; the synthetic hash must agree.
Per-family ownership for *cross-references and display*
(non-identity-bearing) is computed deterministically:

- Closure family: owner crate = the closure expression's
  `DefPath.crate_id`'s crate.
- Vtable family: owner crate = `principal.trait_path`'s
  originating crate when present in `model.crates`; otherwise
  `primary_crate` (deterministic per `(principal, self_ty)`,
  not per discovery order).

#### Synthetic `DefPathHash` construction

The `DefPathHash` for a synthetic item is computed as:

```text
synth.local_hash = u64 from leading 8 bytes (big-endian) of SHA-256(
    bytes "regseq-synthetic-item-v1\0"     // domain separator
        ++ u8 synthetic_kind_discriminant  // SyntheticItemKind variant tag
        ++ canonical payload bytes per family   // see below
)

synth.stable_crate_id = StableCrateId(0)   // reserved synthetic-crate marker;
                                            // bridge validates no real crate
                                            // has stable_crate_id == 0 (inv 20)
```

(`local_hash` is a `u64` per `DefPathHash` schema §7.3; SHA-256
truncated to the leading 8 bytes (big-endian) is the
deterministic 64-bit construction. Same hash family as `ArgsKey`
in §12.3; both truncate SHA-256 of a canonical byte-encoding.
**Note:** synthetic identity does **not** include any "triggering
crate" — see "No triggering crate in identity" above.)

Per-family canonical payload bytes:

```text
ClosureCall / ClosureDropGlue          // captures: source-derived, not synthetic
  ++ DefPathHash bytes of `closure` payload field (16 B;
     StableCrateId u64 LE + local_hash u64 LE per §7.3 byte form;
     payload field is on `SyntheticItemKind::ClosureCall { closure, .. }`
     / `ClosureDropGlue { closure }` per §13.3)
  ++ for ClosureCall: u8 ClosureCallKind discriminant
                       (Call=0, CallMut=1, CallOnce=2)

VTableType
  ++ DefPathHash bytes of the principal trait                  (16 B)
  ++ ArgsKey bytes of trait_args                               (16 B)
  ++ ProjectionBindingsKey bytes of projection_bindings        (16 B)

VTableInstance / VTableMethodShim / VTableDropShim
  ++ DefPathHash bytes of the principal trait                  (16 B)
  ++ ArgsKey bytes of trait_args                               (16 B)
  ++ ProjectionBindingsKey bytes of projection_bindings        (16 B)
  ++ ArgsKey bytes of self_ty                                  (16 B)
  ++ for VTableMethodShim:
       u32 LE vtable_entry_index
       ++ DefPathHash bytes of method_def_path (16 B; StableCrateId
          u64 LE + local_hash u64 LE per §7.3 byte form)
```

`DefPathHash` bytes = the 16-byte content (StableCrateId u64 LE
+ local_hash u64 LE per §7.3). `ArgsKey` and
`ProjectionBindingsKey` byte form per §12.3 / §13.3 (leading 16
SHA-256 digest bytes in digest order, equivalently
`<key>.0.to_be_bytes()`).

The reserved `StableCrateId(0)` for synthetic items
domain-separates synthetic `DefPathHash` values from real-crate
`DefPathHash` values. Per construction invariant 20 (§3.1), the
bridge validates at extraction time that no real crate has
`stable_crate_id == 0`; if it ever encounters one (a rustc
hashing edge case), the bridge rejects with
`BridgeError::StableCrateIdZeroCollision`. The two spaces are
**rejection-enforced** disjoint, not hash-probabilistic.

Construction invariant: `item_def_paths` is unique across all
`ItemId`s (invariant 18). Source-derived hashes use rustc's
`tcx.def_path_hash(did)` (non-zero `stable_crate_id`);
synthetic hashes use the algorithm above
(`stable_crate_id == 0`). The
`SyntheticItemKind::ClosureCaptures` carve-out (§13.1) means
ClosureCaptures Items use rustc's `DefPathHash` for the closure
expression (non-zero `stable_crate_id`), placing them in the
source-derived space.

**Collision model (honesty note).** Synthetic
`DefPathHash.local_hash` is a 64-bit fingerprint. The
intra-model uniqueness invariant (invariant 18) detects
collisions within a single constructed model and is enforced
by validation; cross-model synthetic hash collision is
**outside the v1 threat model** — bridge implementations
running on different sources may produce colliding 64-bit
synthetic hashes (probability ~`2^-32` at hundreds of millions
of synthetic items per bridge run, well above driver-crate
realistic volumes). For cross-build identity comparisons that
must distinguish synthetic items, use the structured
`SyntheticItemKind` payload (which is collision-proof — it
carries the structured identity directly).

---

## 14. `FnAbi` and `FnSig` minimum canonical schema

### 14.1 `FnAbi` enum

The normative `FnAbi` definition (with `#[repr(u8)]`
discriminants) lives in §10.1 alongside the other
auxiliary-closed-enums-on-`Ty`. This section is explanatory;
the enum body is **not** duplicated here to avoid drift.

`FnAbi` is embedded as a structural field on `FnSig` (single
source of truth; no side-channel `HashMap<FnSigId, FnAbi>`, no
post-translation rustc query). Q2 (eager translation;
fully-owned IR) forces this — omitting the ABI would conflate
rustc-distinct types in the interner.

### 14.2 Bridge translation map

```
ExternAbi::Rust                       ->  FnAbi::Rust  (accept)
ExternAbi::C { unwind: false }        ->  FnAbi::C     (accept)
ExternAbi::C { unwind: true }         ->  reject (BridgeError::UnsupportedAbi)
[all non-Rust / non-C ExternAbi variants]  ->  reject (BridgeError::UnsupportedAbi)
```

Hard rejection: bridge raises a diagnostic carrying the source
span and the unsupported ABI name; refuses to translate the
function (or the `FnSig` referenced by a `FnPtr`/`FnDef` type).
No defensive fallback to `FnAbi::C` for unsupported ABIs.

`RustCall` (closure-trait method ABI) is rejected at the
top-level rule but does NOT appear in the IR — closures are
accepted and their synthesised call-method Function Items are
rewritten to `FnAbi::Rust` after tuple-receiver/argument
normalisation (§13.1).

### 14.3 `FnSig` minimum canonical schema

The current pinned `FnSig` schema for canonicalisation (§12.3):

| Field        | Type                      | Carries                                               |
| ------------ | ------------------------- | ----------------------------------------------------- |
| `inputs`     | `Vec<TyId>`               | positional argument types                             |
| `output`     | `TyId`                    | return type                                           |
| `abi`        | `FnAbi`                   | `Rust` or `C` (load-bearing for type identity per Q2) |
| `c_variadic` | `bool`                    | trailing `...`; distinct types in rustc               |
| `safety`     | `Safety { Safe, Unsafe }` | `unsafe fn` marker; distinct types in rustc           |

This is the canonicalisation surface — the fields the
canonicaliser walks. Diagnostic-only data (parameter names,
declaration spans) is **not** carried on `FnSig` itself; if the
implementation needs such metadata, it lives in a sibling table
keyed by `FnSigId`, not as additional `FnSig` fields. This keeps
the canonical schema and the interner's identity rule one and
the same.

**`FnSig` construction invariants.**

- **Never-input DSL rejection (all FnSigs).** `inputs[i]` MUST
  NOT be `Ty::Never` for any FnSig. A function type with an
  uninhabited parameter is well-formed in type theory (just
  uncallable except from divergent code), but v1 has no useful
  C lowering for uninhabited parameters. The bridge **rejects**
  such signatures with `BridgeError::UnsupportedNeverInput
  { fn_sig_id_or_callee_def_id, input_idx }` — this is an
  explicit DSL-scope rejection (with a real diagnostic), not
  a bridge internal bug. `output` MAY be `Ty::Never` (divergent
  functions are well-formed and lower to `__attribute__((noreturn))
  void`).
- **Unit return.** `output` MAY be `Ty::Tuple { fields: [] }`
  (Rust unit `()`); this is the standard no-value return shape
  and lowers to C `void` at `FnAbi::C` boundaries (§14.5).

**`c_variadic` construction invariants.**

- `c_variadic == true` implies `abi == FnAbi::C` (rustc's C
  variadic restriction; `extern "Rust" fn(...)` cannot be
  variadic).
- `c_variadic == true` implies `inputs.len() >= 1` — the
  rustc-accepted prefix shape requires at least one fixed
  parameter before the `...`.

### 14.4 `FnSigInterner`

`Ty::FnPtr(FnSigId)` participates in structural type identity
(§12.1: `FnPtr` is "structural — FnSig content expanded by
canonicaliser"). Sound identity requires `FnSigId` to be
content-unique under the §14.3 schema: two builds (or two
construction paths within one build) producing the same
`(inputs, output, abi, c_variadic, safety)` tuple must produce
the same `FnSigId`.

```rust
pub struct FnSigInterner {
    storage: IndexVec<FnSigId, FnSig>,
    lookup:  HashMap<FnSig, FnSigId>,        // Hash + Eq over the §14.3 fields exactly
}

impl FnSigInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, sig: FnSig) -> FnSigId;
    pub fn resolve(&self, id: FnSigId) -> &FnSig;
    pub fn find(&self, sig: &FnSig) -> Option<FnSigId>;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (FnSigId, &FnSig)>;
}
```

Same BiMap pattern as `TyInterner` (§12) and `StringInterner`
(§4). Custom serde mirrors the same shape: on-wire is `storage`
only; `lookup` is reconstructed at deserialise by direct
insertion preserving original `FnSigId` values. Construction
invariant 11 (§3.1) pins the content-uniqueness requirement.

### 14.5 C ABI admissibility

A `FnSig` with `abi: FnAbi::C` is accepted only if every
parameter type and the return type has a defined C ABI lowering
under the v1 type universe. Two things distinguish "C ABI
admissible" from "structurally lowerable as C": (a) the call
contract must match (passing convention, register/stack
allocation), and (b) certain Rust shapes are ABI-equivalent to
a *different* type at the boundary, requiring the bridge to
record both the original Rust type (for IR-level identity) and
the C-side lowering shape (for codegen).

**Where the check runs.** Admissibility is **bridge-side at
`FnSig` translation**, not a model-walked predicate. The bridge
consults rustc directly for repr classification
(`adt_def.repr().c()`, `adt_def.repr().transparent()`,
`adt_def.repr().int`) which are *not* stored on the IR
(intentionally, per §11.3 — D11 dropped `IS_C` /
`IS_TRANSPARENT` from `Layout` because they are not needed for
layout reproduction). The model carries two bridge outcomes:
(1) the *fact* that the predicate held (presence of the FnSig
in `model.sigs` with `abi: FnAbi::C` per construction invariant
16), and (2) the per-position lowering shape in
`sig_c_abi_lowerings[id]: Option<CAbiLowering>` (§3).

**Lowering shape (`CAbiShape`).** The boundary lowering per
input/output position is one of two shapes:

```rust
pub enum CAbiShape {
    /// Use the position's own `TyId` in its natural C
    /// representation: scalars, pointers, fn-pointers, repr(C)
    /// ADTs without further substitution.
    Direct,

    /// Substitute a different `TyId` at the C boundary. The
    /// substitution is **fully composed** — `emit_as` is the
    /// final terminal `TyId` after applying repr(transparent)
    /// unwrapping and / or fieldless repr(int) enum
    /// substitution recursively to a fixed point. `reason`
    /// records why the substitution occurred (for diagnostics
    /// and snapshot inspection).
    Substituted {
        emit_as: TyId,
        reason:  SubstReason,
    },
}

pub enum SubstReason {
    /// One or more repr(transparent) unwraps. `emit_as` is the
    /// deepest non-transparent non-fieldless-enum field type.
    TransparentChain,

    /// Fieldless repr(int) enum (every variant unit), no
    /// transparent wrapping. `emit_as` is the tag scalar type
    /// (`Ty::Int(_)` or `Ty::Uint(_)`).
    TagScalar,

    /// Composition of transparent wrappers and fieldless enum:
    /// the chain ultimately resolves to a tag scalar.
    /// `emit_as` is the final tag scalar type.
    TransparentChainAndTagScalar,
}

pub struct CAbiLowering {
    pub inputs: Vec<CAbiShape>,    // one per FnSig.inputs entry
    pub output: CAbiShape,
}
```

**Why composition is captured in `emit_as`, not in a recursive
shape.** The shape is a *result*, not a ladder: codegen reads
`emit_as` directly to know which `TyId`'s natural C
representation to emit. For `extern "C" fn(W(Status))` where
`W(Status)` is `repr(transparent)` over `Status` which is
`#[repr(u32)] enum Status { ... }` (fieldless), the predicate
walks W → Status → u32 to a fixed point and records `Substituted
{ emit_as: TyId(u32), reason: TransparentChainAndTagScalar }`.
Without composition into `emit_as`, codegen would have to re-
derive the unwrap chain — but repr facts are intentionally not
stored in the IR (§11.3, D11). Storing the final emit target
keeps codegen executable from the model alone.

**Type identity preservation (D15).** `FnSig.inputs/output`
preserve the **original Rust `TyId`s**. The lowering is
*recorded alongside* in `sig_c_abi_lowerings`, not substituted
into the FnSig. This keeps rustc-distinct function types
distinct in the IR:

- `extern "C" fn(Handle) -> Handle` (with `Handle(u32)`
  `repr(transparent)`) has `FnSig { inputs: [TyId(Handle)],
  output: TyId(Handle), abi: C, ... }` and
  `sig_c_abi_lowerings[id] = Some(CAbiLowering {
  inputs: [Substituted { emit_as: TyId(u32), reason:
  TransparentChain }],
  output: Substituted { emit_as: TyId(u32), reason:
  TransparentChain } })`.
- `extern "C" fn(u32) -> u32` has `FnSig { inputs: [TyId(u32)],
  output: TyId(u32), abi: C, ... }` and
  `sig_c_abi_lowerings[id] = Some(CAbiLowering { inputs:
  [Direct], output: Direct })`.

Both lowerings emit identical C function signatures
(`uint32_t f(uint32_t)`), but the FnSigs are distinct in the
model — preserving the D15 type-identity contract for rustc-
distinct types.

**`CAbiLowering` is sibling-only metadata.** Per the §14.3
convention, diagnostic / sibling data does not affect FnSig
identity. `sig_c_abi_lowerings` is keyed by FnSigId but does
not appear in the FnSigInterner's `Hash + Eq` — two FnSigs
with identical canonical schemas (§14.3) collapse to one
FnSigId regardless of their CAbiLowering. (This case does not
arise in practice: identical FnSigs reference identical TyIds,
which carry identical repr facts, hence identical lowerings.)

**Two predicates: position vs field admissibility.** "C-ABI-
admissible" splits into two recursive predicates with different
acceptance criteria:

- **`CAbiPositionAdmissible(ty, position)`** — for top-level
  `FnSig.inputs[i]` and `FnSig.output` positions. Concerned
  with whether `ty` has a defined by-value C calling
  convention.
- **`CFieldAdmissible(ty)`** — for fields inside C-representable
  aggregates (`repr(C)` structs, unions, enum payload variants).
  Concerned with whether `ty` can appear inside a C struct
  field declaration.

Some types differ between the two: arrays and ZST markers
admitted as fields but rejected at top-level positions; tuples
rejected in both. Conflating the two predicates (Round 6's
single-recursive rule) would either reject `repr(C) struct
{ bytes: [u8; 16] }` at the boundary (because Array is rejected
at positions and the recursion uses one rule for both) or admit
arrays at boundary positions (which has no by-value C ABI).
Splitting fixes both.

**Recursion-cycle handling (memoised checking).** The two
predicates are mutually recursive through aggregate fields
(`Struct`/`Union`/`Enum` recurse to `CFieldAdmissible` on
fields) and through nested function pointers (`FnPtr(sig)`
recurses to `CAbiPositionAdmissible` on the nested sig's
inputs/output). Driver callback structs use exactly this
pattern (Linux's `struct file_operations`, Zephyr's device
callback tables): a `repr(C)` struct containing function
pointers whose signatures reference the same struct or another
recursive callback type. Without cycle handling, the bridge
loops on real driver fixtures.

The bridge's checker MUST implement a memoised / coinductive
discipline:

- Maintain three visiting sets:
  `visiting_position: Set<(TyId, PositionKind)>`,
  `visiting_field: Set<TyId>`, and
  `visiting_sig: Set<FnSigId>`.
- On entry to a check, insert the key into the corresponding
  visiting set; on exit (accept or reject), remove it.
- On re-entry through a `FnPtr(sig)` edge (the only way a
  cycle can close in a finite-sized type — function pointers
  are the indirection that makes recursive callback structs
  finite), treat the in-flight check as **provisionally
  accepted**. Resolve the provisional acceptance once the
  outer check completes: if the outer check accepts, the
  in-flight cycle is real-accepted; if it rejects, the cycle
  is rejected too.
- Re-entering the same aggregate through a *by-value field
  edge* without a pointer/fnptr indirection is **impossible**
  for valid Rust sized types (rustc rejects such infinite-size
  types at definition). The bridge rejects this case as model
  corruption (`BridgeError::CyclicAggregateField`), since it
  cannot arise from valid rustc input.
- Successful checks may be cached in `accepted_position` /
  `accepted_field` / `accepted_sig` to avoid redundant work
  on shared subgraphs.

This terminates because the visiting-set guard ensures every
edge is traversed at most once per outer check, and the
position/field/sig spaces are finite per `RegseqModel`.

#### `CAbiPositionAdmissible(ty, position)`

The bridge runs the predicate at each `FnSig.inputs[i]` and
`FnSig.output` position; the result is both a yes/no decision
(rejection raises an error) and a `CAbiShape` that gets recorded
in `sig_c_abi_lowerings`. Shape rules:

- `Bool`, `Int(_)`, `Uint(_)`, `Float(_)` — admissible at any
  position; shape: `Direct`.
- **Unit return**: at `position = Output`,
  `Ty::Tuple { fields: [], .. }` — admissible; shape: `Direct`.
  Codegen reads `Direct` on the unit-tuple output position and
  emits C `void`. (At `position = Input`, unit is rejected in
  v1 — see rejected list.)
- `Ptr(_, _)`, `Ref(_, _)` to any sized pointee — admissible;
  shape: `Direct`. (The pointee itself need not be
  C-ABI-admissible — pointers are passed by-value as machine
  words.) **Exception:** `Ref(Never, _)` and `Ptr(Never, _)`
  are rejected — `Never` is sized (size 0, align 1) but has
  no value representation; pointers to `!` have no v1 C
  lowering. Reject at C boundary with
  `BridgeError::UnsupportedCAbiType` and
  `position`-appropriate diagnostic.
- `Ptr(t, _)`, `Ref(t, _)` where `t` is `Ty::Str`,
  `Ty::Slice(_)`, or `Ty::Dynamic{..}` (fat pointer to unsized
  pointee) — **rejected** in v1. The wide-pointer aggregate is
  not idiomatic at extern "C" boundaries; driver code passes
  data + length/vtable as separate parameters.
- `FnPtr(sig)` — admissible iff `model.sigs[sig].abi ==
  FnAbi::C` AND the nested sig's `inputs` and `output` are
  themselves C-ABI-position-admissible (recursive check via
  `CAbiPositionAdmissible`); shape: `Direct`. Nested
  `FnAbi::Rust` function pointers crossing a C boundary lose
  the call contract and are rejected. **Exception:**
  `FnPtr(sig)` where `model.sigs[sig].output == Ty::Never` is
  rejected at C boundaries in v1 — top-level `extern "C" fn()
  -> !` lowers to `__attribute__((noreturn)) void`, but nested
  noreturn function-pointer declarators have platform-specific
  C syntax and placement rules. Reject at the boundary; defer
  to a future revision if a real driver use case surfaces.
- `Struct { item, args, .. }` and `Union { item, args, .. }`
  whose rustc `adt_def.repr()` is `C` *and* whose every field
  type is recursively `CFieldAdmissible` — admissible; shape:
  `Direct`. (Note: field-level recursion uses
  `CFieldAdmissible`, not `CAbiPositionAdmissible` — this is
  the split that allows array fields inside `repr(C)` structs.)
- `Struct { item, args, .. }` whose rustc `adt_def.repr()` is
  `transparent` — admissible iff the inner non-ZST field type
  is recursively `CAbiPositionAdmissible` (because the
  transparent wrapper unwraps to the inner field's *boundary*
  representation, not its field-level representation). Skip
  ZST marker fields like `PhantomData<U>` during inspection.
  Shape composition:
  - If the recursive admission produced `Direct` for the inner
    field: shape is `Substituted { emit_as: <inner-field-ty>,
    reason: TransparentChain }`.
  - If the recursive admission produced
    `Substituted { emit_as, reason: TagScalar }` for the inner
    field: shape is `Substituted { emit_as, reason:
    TransparentChainAndTagScalar }`.
  - If the inner field is itself a `repr(transparent)` wrapper
    that further unwraps: recursion continues to a fixed
    point; `emit_as` records the final terminal TyId.
  The original `Struct` TyId stays in `FnSig.inputs/output`.
- `Enum { item, args, .. }` with payload variants whose rustc
  `adt_def.repr()` is `C` *and* whose every variant's field
  types are recursively `CFieldAdmissible` — admissible; shape:
  `Direct`.
- Fieldless `Enum { item, args, .. }` with rustc
  `adt_def.repr()` being `repr(<int>)` *and* every variant
  unit (no payload fields) — admissible; shape:
  `Substituted { emit_as: <tag-scalar-ty>, reason: TagScalar }`,
  where `<tag-scalar-ty>` is the underlying `Ty::Int(_)` or
  `Ty::Uint(_)`. The original `Enum` TyId stays in `FnSig`.

**Rejected at C boundary positions**:

- `Char` — Rust `char` carries a Unicode-scalar-value validity
  invariant; emitting as `uint32_t` preserves width but not the
  validity contract. Use `u32` and validate explicitly. (Char
  is also rejected as a *field* of any aggregate that crosses
  a C ABI boundary by way of `CFieldAdmissible` — see below —
  because the validity invariant cannot be enforced when
  external C populates the field.)
- `Tuple { fields: [...] }` with `fields.len() >= 1` — Rust
  multi-field tuples have no canonical C ABI.
- `Tuple { fields: [] }` (Rust unit) — at `position = Input`,
  rejected in v1 (no v1 ZST-parameter omission policy);
  accepted at `position = Output` per the unit-return rule
  above.
- `Array(_, _)` — C arrays decay to pointers in parameter
  position and cannot be returned by value. Pass as
  `Ptr(Array(...), _)` (admissible), or wrap in a `repr(C)`
  struct field (admissible per `CFieldAdmissible`).
- `Closure { .. }` — `Ty::Closure` is the captures-struct
  identity view; it has no `repr(C)` claim.
- `FnDef(_, _)` — function-item ZSTs do not appear at ABI
  boundaries by construction. Bridge bug if encountered.
- `Dynamic { .. }`, `Str`, `Slice` (bare) — invalid outside
  `Ref`/`Ptr` per construction invariant 12.
- `Never` as a parameter — pointless. Accepted as the return
  slot (`fn() -> !` lowers to `__attribute__((noreturn)) void`).
- `Struct`/`Enum`/`Union` whose rustc `adt_def.repr()` is
  default `Rust` — no canonical C ABI.
- `FnPtr(sig)` whose `model.sigs[sig].abi == FnAbi::Rust`.

#### `CFieldAdmissible(ty)`

For fields inside C-representable aggregates (`repr(C)` structs,
unions, enum payload variants). Different acceptance set from
`CAbiPositionAdmissible`:

- All scalars (`Bool`, `Int(_)`, `Uint(_)`, `Float(_)`) —
  admissible. (`Char` is rejected — see below.)
- `Ptr(_, _)`, `Ref(_, _)` to any sized pointee — admissible.
- `FnPtr(sig)` — admissible iff `model.sigs[sig].abi ==
  FnAbi::C` (same recursion guard as positions; nested
  declarators must also be C-ABI emittable).
- `Array(elem, _)` — admissible iff `CFieldAdmissible(elem)`
  (recurse on element; arrays-of-arrays accepted).
- `Tuple { fields: [] }` (unit ZST marker) — admissible if
  codegen treats the field as zero-sized (omits or pads per
  the ZST handling rule, §11.0).
- `Struct { item, .. }` with `model.items[item].is_phantom_data
  == true` AND `layout.size == 0` AND `layout.align == 1`
  (e.g., `core::marker::PhantomData<T>`) — admissible
  regardless of the wrapper's source repr (PhantomData is
  default `repr(Rust)`, but its lang-item identity makes it
  the canonical zero-cost marker). The recursion does **not**
  inspect `T` (the marker's type parameter is for
  variance/phantom semantics only; it does not appear in
  emitted C). Codegen omits PhantomData fields from emitted
  C aggregates. This rule is restricted to lang-item ZST
  markers; non-lang-item ZSTs without `is_phantom_data` go
  through the ordinary ADT rule below.
- `Struct`/`Union` with `repr(C)` *and* every field
  `CFieldAdmissible` — admissible.
- `Struct` with `repr(transparent)` — admissible iff the inner
  non-ZST field is `CFieldAdmissible`.
- `Enum` with `repr(C)` or fieldless `repr(int)` — admissible
  if every variant's payload fields are `CFieldAdmissible`.

**Rejected as fields**:

- `Char` — same validity-invariant argument as boundary
  positions; external C populating the field cannot enforce
  the Unicode-scalar-value range. Driver code uses `u32`
  fields and validates on access.
- Any `Ty` with default `repr(Rust)`.
- `Tuple` with `fields.len() >= 1` — non-unit tuples have no
  canonical C field shape.
- `Closure`, `FnDef` — same as boundary positions.
- `Str`, `Slice`, `Dynamic` (bare) — invalid placement
  (invariant 12).
- `Never` — uninhabited; cannot exist as a field of a
  reachable value.

The two predicates are mutually recursive only through
`Struct`/`Union`/`Enum`'s field-level recursion (which uses
`CFieldAdmissible`) and `FnPtr`'s nested-sig recursion (which
uses `CAbiPositionAdmissible` on the nested FnSig's inputs/
output, since those are themselves boundary positions).

**Nested `FnPtr` declarator emission.** When emitting
`Ty::FnPtr(sig)` as a C declarator (e.g., as a parameter type
in an extern "C" decl, or inside a `repr(C)` struct field),
codegen reads `sig_c_abi_lowerings[sig]` if `sigs[sig].abi ==
FnAbi::C` and applies those per-position shapes inside the
pointed-to function type. The outer `CAbiShape::Direct` for
the FnPtr value means only that the pointer value itself is
passed directly as a machine word; it does not suppress
nested-signature lowering. For example, `extern "C" fn(cb:
extern "C" fn(Handle) -> Handle)` emits the cb declarator as
`uint32_t (*cb)(uint32_t)` (Handle substituted via the nested
sig's `Substituted { emit_as: u32, reason: TransparentChain }`),
not as `Handle (*cb)(Handle)`.

Validation invariant: any `FnPtr(sig)` admitted at a C boundary
or as a field of a C-representable aggregate has
`sig_c_abi_lowerings[sig] == Some(_)` — the nested lowering
must exist for codegen to emit the declarator.

**Diagnostics on rejection.** The bridge raises
`BridgeError::UnsupportedCAbiType { ty, position }` (where
`position` is `Input(idx)`, `Output`, or `Field { parent_ty,
field_idx }`). No defensive fallback to `FnAbi::Rust`. Codegen
reads `sig_c_abi_lowerings[id]` to know whether to emit each
position as the position's own `TyId`'s natural C representation
(`Direct`) or substitute to the `Substituted.emit_as` TyId.
The C compiler computes ABI correctly because the emitted
declaration uses the C-side shape per `CAbiShape`, while the IR
still distinguishes rustc-distinct function types by preserving
original Rust `TyId`s in `FnSig`. Rejection at the bridge plus
side-table lowering record is the discipline that keeps both
invariants honest.

**v1 nested-field C ABI assumption.** For aggregate parameters
crossing a `FnAbi::C` boundary (admissible per the
`CAbiPositionAdmissible.Direct` rule above), v1 codegen relies
on the C compiler classifying layout-equivalent C aggregates
identically to rustc's view of the source aggregate. This
holds for the v1 target set (Linux + Zephyr on SystemV x86_64
and AAPCS aarch64) with the type set the bridge admits at
field positions (scalars, pointers, fn pointers, `repr(C)`
aggregates of those, `repr(transparent)` wrappers in field
position substituted via `CFieldAdmissible`). The assumption
specifically does **not** cover ABI-classification edge cases
that don't appear in driver kernel APIs: HFA/HFV float-array
aggregates on AAPCS, scalar-pair vs union-on-128-bit
classification on SystemV, etc. Future targets requiring
per-field ABI distinctions beyond layout-equivalence would
extend this contract with a recorded per-ADT C ABI lowering
certificate. For v1, the assumption is methodology-aligned
("don't add schema for hypotheticals") and verified against
actual driver use cases at fixture-pressure time.

**Opaque-handle convention.** A **non-generic** `repr(C)` ZST
struct (typically `#[repr(C)] struct OpaqueCtx { _private: [u8;
0] }`) used only behind `Ref`/`Ptr` is admitted: the boundary
type is the pointer to it, and codegen MAY lower the struct as
a C *incomplete struct declaration* (`struct OpaqueCtx;`). The
bridge tracks this via `Item.opaque_handle_eligible` (§8.0):
**true** iff **all five** conditions hold (matching invariant
19, §3.1):

1. `Item.kind == ItemKind::Struct`.
2. The Item's source-side rustc `adt_def.repr()` is `C`.
3. The Item is **non-generic** — see "Model-checkable
   non-generic criterion" below.
4. The Item's monomorphic `Ty::Struct.layout` is ZST
   (`size: 0, align: 1`).
5. Every reachable use of the Item's monomorphic `TyId` across
   the model is behind `Ref` or `Ptr`.

Generic opaque-handle candidates (`Handle<T>`-style) are
**rejected at the bridge** with
`BridgeError::GenericOpaqueHandle`. Computing conditions 4-5
requires a global reachability analysis at bridge construction;
the outcome is stored on the Item so codegen and validation
can read it directly. When `opaque_handle_eligible == true`,
codegen emits the incomplete-struct form; when false, the
bridge has already rejected any incompatible use site at type
translation, so codegen emits a complete declaration.

**Model-checkable non-generic criterion.** A non-generic Item
is one for which there exists exactly one `Ty::Struct { item:
this, args, layout }` entry in `model.types` and that entry's
`args.is_empty()`. (Generic Items would produce one
`Ty::Struct` entry per monomorphisation, with non-empty
`args`.) Validation (§3.3 Phase B) checks this directly from
`model.types` — no need to inspect `DefPath.segments` or
rustc-side generic parameter lists.

**Reachable-use traversal (formal definition).** "Every
reachable use of the Item's monomorphic `TyId` is behind
`Ref` or `Ptr`" is formalised as: walk the model's reference
graph from each `TyId` occurrence, count it as a use, and
classify each occurrence as either *behind a pointer* (good)
or *direct* (disqualifying). The traversal domain is closed
and listed below; bridge implementations and validation
(§3.3 Phase B) walk these positions exactly.

| Position kind                                                     | Traversal            | Disqualifies eligibility?                          |
| ----------------------------------------------------------------- | -------------------- | -------------------------------------------------- |
| Direct `Ty::Struct { item: this, .. }` mention as a `FnSig.inputs[i]` / `FnSig.output` | by-value             | **yes**                                            |
| `Ref(this_ty, _)` / `Ptr(this_ty, _)` anywhere in the model       | behind pointer       | no                                                 |
| `Ty::Struct { item: this }` as a `FieldLayout.ty` of any `Layout` | by-value field       | **yes** (sized-field rule)                         |
| `Ty::Struct { item: this }` as an `Array` element type            | by-value (array elem)| **yes**                                            |
| `Ty::Struct { item: this }` as a `Tuple.fields` element           | by-value             | **yes**                                            |
| `GenericArg::Type(TyId(this_ty))` in any `GenericArgs`            | by-value (generic arg)| **yes**                                           |
| Nested `FnPtr(sig)` whose `sigs[sig].inputs/output` reach `this_ty` directly | recurse via sig    | applies the same rules to the nested signature    |
| `Const` value referencing the type (stage-4-pinned shape)         | TBD per stage 4      | stage-4 forward expectation; stage 3 conservatively rejects eligibility for any Item appearing in a `Const` value |
| Body local types, place projection types (stages 9-11)            | by-value             | stage-9/11 forward expectation; stage 3 reserves the right to disqualify based on body-stage occurrences |

For stage 3, the traversal walks `model.types`, `model.sigs`,
and `model.items` (lang-item flag fields, `def_path`
attribute references). Stages 9-11 (body lowering) and stage 4
(consts) extend the traversal as forward expectations. The
bridge stores the **outcome** as `Item.opaque_handle_eligible`
after running the traversal at construction time; consumers
read the bool, not re-run the walk.

This formal traversal makes opaque-handle eligibility
**deterministic across bridge implementations** — two bridges
satisfying the build-equivalence quad produce identical
`opaque_handle_eligible` values.

---

## 15. Bridge translation rules

What the bridge does for each rustc `TyKind` after
`instantiate_mir_and_normalize_erasing_regions`:

| rustc `TyKind`                                           | RegseqModel                              | Notes                                                                                        |
| -------------------------------------------------------- | ---------------------------------------- | -------------------------------------------------------------------------------------------- |
| `Bool`, `Char`, `Int(_)`, `Uint(_)`, `Float(_)`, `Never` | `Ty::Bool` / `Char` / `Int(...)` / etc.  | direct                                                                                       |
| `Str`                                                    | `Ty::Str`                                | only valid behind `Ref`/`Ptr` (invariant 12)                                                 |
| `Array(t, n)`                                            | `Ty::Array(translate(t), n)`             | `n` is post-mono `u64`                                                                       |
| `Slice(t)`                                               | `Ty::Slice(translate(t))`                | only valid behind `Ref`/`Ptr` (invariant 12)                                                 |
| `Ref(_region, t, Mutability::Not)`                       | `Ty::Ref(translate(t), SharedBorrow)`    | non-HRTB parent only; HRTB parent rejected per §17.1                                         |
| `Ref(_region, t, Mutability::Mut)`                       | `Ty::Ref(translate(t), UniqueBorrow)`    | same                                                                                         |
| `RawPtr(t, mutbl)`                                       | `Ty::Ptr(translate(t), mutbl)`           |                                                                                              |
| `Tuple([t0..tn])`                                        | `Ty::Tuple { fields, layout }`           | layout from `tcx.layout_of`                                                                  |
| `Adt(_)`                                                 | special-ADT dispatch (see below)         |                                                                                              |
| `FnDef(did, args)`                                       | `Ty::FnDef(item, args)`                  | ZST                                                                                          |
| `FnPtr(binder)`                                          | `Ty::FnPtr(intern(FnSig))`               | substantive-HRTB-bearing rejected per §17.1                                                  |
| `Dynamic(preds, _region, DynKind::Dyn)`                  | `Ty::Dynamic { auto_traits, vtable_ty }` | substantive-HRTB-bearing rejected per §17.1; `vtable_ty` resolves to a synthesised `Ty::Struct` |
| `Dynamic(_, _, DynKind::DynStar)`                        | reject (`BridgeError::UnsupportedDynKind`) | `dyn*` is structurally distinct (thin pointer with embedded vtable, not fat pointer); not in v1 scope |
| `Closure(did, args)`                                     | `Ty::Closure(synthesised_item, args)`    | parallel `Ty::Struct(synthesised_item, args, layout)` interned for layout view               |

**Special-ADT dispatch order.** When the bridge encounters
`TyKind::Adt(adt_def, args)`, it checks the closed special-ADT
set *before* generic struct/enum/union translation:

1. **Lang-item flagged ADTs** (D1.5): `PhantomData`, `UnsafeCell`,
   `ManuallyDrop`. Recognition via `adt_def.is_*()`. Translation
   continues as ordinary `Ty::Struct`; the corresponding flag
   on the resulting `Item` is set per §8.2.
2. **DSL-rejected ADTs** (D1.4 + D10 + D16): `Box` (via
   `adt_def.is_box()` / `IS_BOX`), `core::ffi::c_void` (via
   `tcx.is_lang_item(LangItem::CVoid)`),
   `core::sync::atomic::Atomic*` (via canonical-name match),
   `Pat`, `UnsafeBinder`, `Foreign`, `Coroutine` family — see
   §15.1 rejection table for the full set.
3. **Generic Adt translation** runs only if no special rule
   above applies, dispatching on the rustc `AdtDef::adt_kind()`
   to one of:
   - `Adt(struct)`  → `Ty::Struct { item, args, layout }`
   - `Adt(enum)`    → `Ty::Enum { item, args, layout }`
   - `Adt(union)`   → `Ty::Union { item, args, layout }`

This ordering is normative: implementation must check the
special set first, otherwise generic translation accidentally
admits rejected core types.

### 15.1 Rejections

The bridge rejects the following at type or call translation. Two
categories overall: **non-rigid `TyKind`** (should not survive
`instantiate_mir_and_normalize_erasing_regions`; encountering one
is a bridge bug) and **rigid but DSL-rejected** (types that
survive normalisation but lie outside the v1 driver-DSL scope).

| Rejection                                                                 | Phase            | Reason                                                                                |
| ------------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------- |
| Non-rigid `TyKind` (Param, Bound, Placeholder, Infer, Alias, Error)       | type translation | bridge bug if encountered post-normalisation                                          |
| `Foreign(_)`                                                              | type translation | `extern type T`; opaque-struct pattern recommended                                    |
| `Box<T>`                                                                  | type translation | `alloc` construct; outside the no-alloc driver DSL                                    |
| `Coroutine` / `CoroutineClosure` / `CoroutineWitness`                     | type translation | async / coroutine state machines; outside `#![no_std]` driver DSL                     |
| `Pat(...)`                                                                | type translation | unstable pattern-newtype types                                                        |
| `UnsafeBinder`                                                            | type translation | unstable for-binders feature                                                          |
| `core::ffi::c_void`                                                       | type translation | recognised via `tcx.is_lang_item(LangItem::CVoid)`; opaque-struct pattern recommended |
| `core::sync::atomic::Atomic*` types                                       | type translation | use `regseq::Atomic<T>`                                                               |
| `core::sync::atomic::Atomic*::*()` methods + `core::intrinsics::atomic_*` | call translation | use `#[rseq::intrinsic("atomic_*")]`                                                  |
| `core::sync::atomic::{fence, compiler_fence}`                             | call translation | outside the v1 admitted intrinsic vocabulary; **stage-14 forward expectation:** Regseq intrinsic vocabulary will admit barrier intrinsics (`#[rseq::intrinsic("compiler_fence" \| "device_fence" \| ...)]`) when the domain intrinsic surface is specified. Until then, drivers requiring explicit ordering barriers between MMIO and normal memory are out of v1 scope |
| `core::ptr::{read,write}_volatile`                                        | call translation | use `#[rseq::intrinsic("mmio_*")]`                                                    |
| `extern "C" { unwind: true }` and other `ExternAbi` variants              | type translation | only `Rust` and `C` accepted (§14)                                                    |
| `FnAbi::C` `FnSig` with non-C-ABI-admissible parameter or return type     | type translation | C ABI admissibility predicate (§14.5); `BridgeError::UnsupportedCAbiType`              |
| `Ref`/`Ptr` to unsized pointee (`Str`, `Slice(_)`, `Dynamic{..}`) at `FnAbi::C` parameter/return | type translation | wide-pointer aggregate not idiomatic at extern "C" boundary; pass data + length/vtable as separate parameters (§14.5) |
| `Ty::Char` at `FnAbi::C` parameter/return                                 | type translation | Rust `char` validity invariant not preserved by C `uint32_t`; use `u32` and validate (§14.5) |
| `Ty::Char` as a field of a C-ABI-crossing aggregate                       | type translation | same validity-invariant argument; rejected by `CFieldAdmissible` (§14.5)                 |
| `Ty::Tuple { fields: [] }` at `FnAbi::C` parameter (input position)       | type translation | no v1 ZST-parameter omission policy; unit accepted at output position only (§14.5)        |
| Use of a `repr(C)` ZST struct by-value or as a sized field                | type translation | conflicts with the opaque-handle incomplete-struct lowering convention (§14.5, §8.0)      |
| Generic opaque-handle candidate (`Handle<T>`-style `repr(C)` ZST struct)  | type translation | v1 restricts opaque-handle eligibility to non-generic Items per invariant 19; `BridgeError::GenericOpaqueHandle` |
| Trait-object dispatchable method whose erased signature is non-C-ABI-admissible | type translation | vtable shims use `FnAbi::C` per §13.2; `BridgeError::UnsupportedTraitObjectMethod`        |
| `Ty::Never` in any `FnSig.inputs[i]` position                             | type translation | uninhabited parameters have no v1 C lowering; `BridgeError::UnsupportedNeverInput`        |
| `Ref(Never, _)` / `Ptr(Never, _)` at `FnAbi::C` parameter/return          | type translation | `&!` / `*const !` / `*mut !` have no value representation; no v1 C lowering for pointers to `!` (§14.5) |
| `FnPtr(sig)` with `model.sigs[sig].output == Ty::Never` at `FnAbi::C` boundary | type translation | nested noreturn fn-pointer declarators have platform-specific C syntax; not in v1 (§14.5)         |
| `Ty::FnPtr(sig)` with `sig.abi == FnAbi::Rust` at `FnAbi::C` boundary     | type translation | Rust call convention crossing C boundary loses call contract (§14.5)                   |
| `Ty::FnDef(_, _)` at `FnAbi::C` parameter/return                          | type translation | function-item ZSTs do not appear at ABI boundaries; bridge bug if encountered (§14.5)  |
| `Ty::Dynamic` with no principal trait (auto-only, e.g., `dyn Send`)       | type translation | vtable synthesis requires a principal trait + projection bindings; not in v1 scope     |
| **`Ty::FnPtr` failing the §17.1 binder predicate**                        | type translation | rejects substantive HRTB (projection / nested / non-ref-slot trait predicate) AND relational binders (a bound region used in more than one immediate-ref position) — both classes would collapse rustc-distinct types under post-erasure `Ty::FnPtr` shape |
| **`Ty::Dynamic` failing the §17.1 binder predicate** (`dyn for<'a> Trait<&'a T>`) | type translation | same reason                                                                           |
| `Dynamic(_, _, DynKind::DynStar)`                                         | type translation | `dyn*` representation distinct from ordinary `dyn`; not in v1 scope                   |
| Bare `Ty::Str`, `Ty::Slice`, `Ty::Dynamic` outside `Ref`/`Ptr` pointee    | type translation | construction invariant 12 (§3.1)                                                      |
| Custom auto-traits (via `feature(auto_traits)`)                           | type translation | closed v1 set is `{Send, Sync, Unpin}`                                                |

---

## 16. Determinism (F4 / F4-2)

Two RegseqModels built under the same **build-equivalence quad**
(equal source, equal pinned toolchain, equal `RegseqModel.target`,
equal bridge options — see §12.4) produce stable `ItemId` and
`InstanceId` assignments. Builds that disagree on any quad
component are not subject to F4 / F4-2 cross-build alignment;
`DefPathHash` (§7.3, §3.1 invariant 2) is the cross-quad
identity carrier.

### 16.1 Item sort comparator (F4)

The bridge sorts items before assigning `ItemId`s. Sort key per
item:

```text
(crate_name:           &str,            // crates[item.def_path.crate_id].name (resolved)
 crate_stable_id:      StableCrateId,
 segments:             Vec<SegmentKey>)

SegmentKey = (data_kind:    u8 (stable rank from def_path_data_sort_rank),
              symbol_text:  &str,    // empty when DefPathData carries no name
              disambiguator: u32)
```

Comparison: `crate_name` (lexicographic) → `crate_stable_id`
(numeric tiebreak) → `segments` (lexicographic, per-segment).
Per segment: `data_kind` (stable rank table) → `symbol_text`
(lexicographic) → `disambiguator` (numeric).

#### `def_path_data_sort_rank` table

Append-only. Adding a new variant during a toolchain bump means
appending a new rank at the end; never re-ranking earlier
entries:

```rust
fn def_path_data_sort_rank(data: &DefPathData) -> u8 {
    match data {
        DefPathData::CrateRoot                  => 0,
        DefPathData::Impl                       => 1,
        DefPathData::ForeignMod                 => 2,
        DefPathData::Use                        => 3,
        DefPathData::GlobalAsm                  => 4,
        DefPathData::TypeNs(_)                  => 5,
        DefPathData::ValueNs(_)                 => 6,
        DefPathData::MacroNs(_)                 => 7,
        DefPathData::LifetimeNs(_)              => 8,
        DefPathData::Closure                    => 9,
        DefPathData::Ctor                       => 10,
        DefPathData::AnonConst                  => 11,
        DefPathData::LateAnonConst              => 12,
        DefPathData::DesugaredAnonymousLifetime => 13,
        DefPathData::OpaqueTy                   => 14,
        DefPathData::OpaqueLifetime(_)          => 15,
        DefPathData::AnonAssocTy(_)             => 16,
        DefPathData::SyntheticCoroutineBody     => 17,
        DefPathData::NestedStatic               => 18,
    }
}
```

The comparator does NOT compare on `Symbol` index, `SymbolIndex`,
or any allocation-order numeric ID. Allocation-order numeric IDs
are not deterministic across runs.

### 16.2 Cross-build identity

`item_def_paths: IndexVec<ItemId, DefPathHash>` lets a consumer
align two RegseqModels for the same source by hash. Two builds
produce identical sequential `ItemId`s only if their reachable
item sets are identical; for incremental/partial builds where
the reachable set differs, `DefPathHash` is the stable identifier
mapping "this `ItemId` in build A" to "that `ItemId` in build B."

`DefPathHash` identifies the *path*, not the body content.

### 16.3 Instance ordering (F4-2)

Instances are sorted by `RegseqInstanceKey` (§9.3):

```text
(regseq_kind_rank:  u8,        // see regseq_instance_kind_rank
 item_id:           ItemId,    // already F4-sorted
 generic_args_key:  ArgsKey,   // see §12.3
 fn_shim_disambig:  u8)        // 0 for non-Function; rustc kind rank for Function
```

Both rank tables (`regseq_instance_kind_rank`,
`rustc_instance_kind_sort_rank`) are append-only. `ArgsKey` is
the deep canonicaliser defined in §12.3.

---

## 17. Methodology cross-cutting

### 17.1 Region erasure (with substantive-HRTB rejection)

No `Ty` variant carries any region slot for free regions. Free
regions become `ReErased` under rustc's
`erase_and_anonymize_regions` and leave no IR trace.

Bound regions, however, **survive** rustc's region erasure as
`ReBound`. The naive predicate "reject any `Binder<FnSig>` whose
body contains `ReBound`" over-rejects: in rustc, every fn-pointer
type carries an *implicit* binder over its elided lifetimes
(`fn(&u32)` is internally `for<'r> fn(&'r u32)`, with `'r` an
anonymised late-bound region after `erase_and_anonymize_regions`).
A naive predicate would reject every callback signature with a
reference parameter, which is most of them.

The same type-identity argument that locks the `FnAbi` field
(rustc-distinct types must remain distinct in the IR) does
require rejection of substantive HRTB structures, where bound
regions appear in positions that participate in trait selection
or projection — collapsing those would merge rustc-distinct
types.

**v1 predicate (substantive-HRTB rejection).** The bridge rejects
`Ty::FnPtr` over `Binder<FnSig>` and `Ty::Dynamic` over
`Binder<Predicate>` with `BridgeError::UnsupportedHRTB` when
**any** of the following holds:

1. a bound region appears in an associated-type / projection
   position (e.g., `<T as Trait<'a>>::Assoc`); or
2. a bound region appears inside a nested binder
   (HRTB-inside-HRTB); or
3. a bound region appears in a trait-bound or trait-predicate
   position other than the immediate lifetime slot of a `&T` /
   `&mut T` reference type in the binder body; or
4. **a bound region appears in more than one position in the
   binder body** (i.e., the binder relates two or more reference
   slots through a shared lifetime); or
5. **a bound region appears in the binder's output position**
   (return-type ref slot). Output-only single-use bound
   regions like `for<'a> fn() -> &'a T` would post-erasure
   collapse with `fn() -> &'static T` and any other
   single-output-ref-bound shape; rejecting them avoids the
   collapse without needing an output-position `binder_shape`
   carrier. Driver fn-pointer types `fn() -> &T` are uncommon
   (returning a reference from a parameterless fn-pointer
   requires `'static` or closure environment); the restriction
   has minimal practical impact.

Condition 4 is what prevents the type-identity collapse D15
forbids: rustc treats `for<'a> fn(&'a A, &'a B)` (relational —
single bound region used twice) and
`for<'a, 'b> fn(&'a A, &'b B)` (independent — two bound regions
used once each) as **distinct types**. Both have the same
post-erasure `Ty::FnPtr` shape `(inputs: [Ref(A), Ref(B)],
output: ())` because no region material survives in `Ty::Ref`.
Accepting both would collapse them into one `FnSigId`. Rejecting
the relational case keeps the identity correspondence one-to-one.

**Accepted (each bound region used exactly once in immediate
input ref slot, no output-position bound regions):**

- `fn(&T)` — single anonymised bound region in input position.
- `fn(&A, &B)` — two distinct anonymised bound regions
  (elision creates one per `&_` parameter), each used once.
  Driver callbacks routinely take multiple references; this is
  the common case.
- `extern "C" fn(*const T)` — no bound regions at all.
- `fn(u32, &T) -> u32` — single bound region used once in
  input; output is `u32` (not a ref).

**Rejected (relational binder; condition 4):**

- `for<'a> fn(&'a A, &'a B)` — explicit relational lifetime.

**Rejected (output-position bound region; condition 5):**

- `for<'a> fn(&'a A) -> &'a A` — input-output relational by
  elision (also fails condition 4).
- `for<'a> fn() -> &'a T` — output-only single-use bound
  region; rejected by condition 5.
- Any `fn(...) -> &T` whose return ref carries a bound region.

**Rejected (substantive HRTB; conditions 1-3):**

- `for<'a> fn(<T as Trait<'a>>::Assoc)` — projection.
- `dyn for<'a> Trait<&'a T>` — trait-predicate argument.
- `for<'a> fn(for<'b> fn(&'b &'a T))` — nested binder.

#### `'static` and free-region collapse policy

A separate v1 question: how does the bridge treat fn-pointer
types with explicit `'static` or other free (non-bound)
reference lifetimes? Examples:

- `type A = fn(&u32);` — `for<'r> fn(&'r u32)` (anonymised
  late-bound region post-erasure).
- `type B = fn(&'static u32);` — explicit `'static`; free
  region (`ReFree` becoming `ReErased`).

After erasure, both lower to
`FnSig { inputs: [Ref(u32, SharedBorrow)], output: <unit | other>, abi, ... }`.
The bound vs free distinction does not survive the type-system
representation (`Ty::Ref` carries no region slot).

**v1 policy: accept both, document the collapse.** The IR does
not distinguish `fn(&u32)` from `fn(&'static u32)`. Rationale:

- The C output is byte-identical (`void f(const uint32_t *)`).
- The runtime / machine behaviour is identical.
- Rustc treats them as subtype-related (the `'static` form is
  a subtype of the binder form), not type-equal; the only
  rustc-internal information lost in the IR is the
  subtyping-only distinction, which has no codegen consequence.
- A `binder_shape` carrier on `FnSig` would preserve the
  distinction at the cost of an extra field; v1 takes the
  simpler path. If a future driver use case demonstrates a
  load-bearing need for the distinction, the relaxation path
  is to add `binder_shape` (parallel to the relaxation path
  for relational binders documented above).

This is **not** a methodology-violating identity collapse: the
distinct rustc types are subtype-related (one accepts a
strict superset of values), so collapsing them is sound at the
machine level. D15's lock applies to types with *distinct* C
outputs — `fn(&u32)` and `fn(&'static u32)` have identical C
output, so the collapse preserves the codegen contract D15
guards.

The predicate is checked at the parent `Ty::FnPtr` /
`Ty::Dynamic` level *before* any nested `TyKind::Ref` translates.
For accepted binders, every `TyKind::Ref` the bridge then
translates has a region slot that is either `ReErased` (free)
or anonymous `ReBound` in a unique immediate-ref position; both
are discarded with no `RegionKind` recording. **No region
survives in `Ty::Ref` for any type that enters the IR.** The
admitted subset is intended to avoid known binder-shape
collapses; the bridge rejects relational and substantive
binders whose erased form would lose type identity. Stage-13
will add a formal binder-shape test suite to mechanically
verify the predicate on the admitted subset.

#### Position traversal definition

A bound region's *position* in the binder body is determined by
walking the body and identifying every `TyKind::Ref` node whose
lifetime slot binds a region. Raw pointers (`TyKind::RawPtr`)
have no lifetime slot in rustc and are not "immediate ref
positions" — bound regions inside the pointee of a raw pointer
fall under condition 3 (non-ref-slot structural position) and
cause rejection.

- Each `&'r T` or `&'r mut T` directly under the binder body
  counts as **one position**.
- `&'r1 &'r2 T` is **two positions** (the outer ref's lifetime
  and the inner ref's lifetime, both bound). Both bound regions
  must satisfy condition 4 individually — neither may appear in
  any *other* position elsewhere in the body.
- A reference appearing inside a generic-type argument (e.g.,
  `Vec<&'a T>` as a function parameter) is *not* an "immediate
  ref slot" of the binder body — the bound region appears
  inside a structural type argument, which falls under
  condition 3 and causes rejection.
- A reference appearing inside a nested `FnPtr` binder body
  (e.g., `for<'a> fn(for<'b> fn(&'b T))`) falls under condition
  2 (nested binder) and causes rejection.
- A reference appearing inside the *output* slot of the binder
  body counts as one position. Output-position bound regions
  are **rejected** by condition 5 in v1, regardless of whether
  the same region appears elsewhere — even output-only
  single-use bindings (e.g., `for<'a> fn() -> &'a T`) collapse
  with `'static`/free-region forms post-erasure and are
  rejected to preserve identity. Cross-input/output use
  additionally fails condition 4.

#### Test cases (worked examples for the predicate)

| Source                                            | Erased binder                                     | Accept/Reject | Reason                                                          |
| ------------------------------------------------- | ------------------------------------------------- | ------------- | --------------------------------------------------------------- |
| `fn(&u32)` (Test A)                               | `for<'r> fn(&'r u32)`                             | **Accept**    | single bound region, single immediate input ref position        |
| `for<'a> fn(&'a u32)` (Test B)                    | `for<'a> fn(&'a u32)`                             | **Accept**    | source-elided and explicit single-bound-region equivalent       |
| `fn(&'static u32)` (Test C)                       | `fn(&u32)` (free region erased)                   | **Accept**    | no bound region; collapses with Test A per `'static` policy     |
| `for<'a> fn() -> &'a u32` (Test D)                | `for<'a> fn() -> &'a u32`                         | **Reject**    | condition 5 — output-position bound region                      |
| `for<'a> fn(&'a u32) -> u32` (Test E)             | `for<'a> fn(&'a u32) -> u32`                      | **Accept**    | single bound region used once in input; `u32` output            |
| `for<'a> fn(&'a u32) -> &'a u32` (Test F)         | `for<'a> fn(&'a u32) -> &'a u32`                  | **Reject**    | condition 4 (used twice) AND condition 5 (output-position)      |
| `fn(&u32, &u32)`                                  | `for<'r1, 'r2> fn(&'r1 u32, &'r2 u32)`            | **Accept**    | elision creates distinct bound regions, each used once          |
| `for<'a> fn(&'a u32, &'a u32)` (Test G)           | `for<'a> fn(&'a u32, &'a u32)`                    | **Reject**    | condition 4 — `'a` used in two positions (relational)           |
| `for<'a, 'b> fn(&'a u32, &'b u32)`                | `for<'a, 'b> fn(&'a u32, &'b u32)`                | **Accept**    | each bound region used once                                     |
| `fn(&u32) -> &u32`                                | `for<'r> fn(&'r u32) -> &'r u32`                  | **Reject**    | conditions 4 + 5 — elision ties output to input                 |
| `extern "C" fn(*const u32)` (Test H)              | `fn(*const u32)` (no binder)                      | **Accept**    | no bound regions                                                |

**Driver impact and future relaxation.** Condition 4 rejects fn-
pointer types that return a reference derived from an input
reference (a common shape for accessor callbacks). When such a
function exists in driver code, it can be expressed as a
*Function Item* (`Ty::FnDef`, called directly via
`InstanceId`) — the restriction is only on the *fn-pointer*
type position, not on the function itself. If a future driver
fixture surfaces a real need for relational fn-pointer types,
the relaxation path is to add a `binder_shape` carrier field
on `FnSig` (a canonical encoding of the bound-region linkage
partition) so distinct binder shapes get distinct `FnSigId`s.
This preserves rustc-distinct identity at the cost of an
extra `FnSig` field; D15's locking argument applies. Stage-3
v1 takes the simpler path (reject) per the
no-defensive-fallbacks methodology and lazy-vocabulary-
expansion pattern.

### 17.2 No QualType layer (S0 framing)

C uses qualifiers (`const`, `volatile`, `restrict`); Rust uses
type-level distinctions and per-operation semantics. RegseqModel
does NOT introduce a "QualType" intermediate that mirrors C
qualifiers as type-level constructs. Instead:

- **`const`** is derived from the C-side `data` qualification
  rule on `Ty::Ref` / `Ty::Ptr` (inferred from `Provenance` and
  the recursive `contains_unsafe_cell` predicate, §17.2.1).
- **`volatile`** is per-operation: encoded on the intrinsic
  `InstanceId` for MMIO / atomic operations, not as a type-level
  flag. There is no `Ty::Volatile<T>` and no `is_volatile` bit.
- **`restrict`** is per-context at codegen time, with a
  conservative emission rule: codegen emits `restrict` only on
  direct function parameters whose MIR use does not copy the
  pointer into an aliasing location before all accesses. It
  is **never** emitted for erased trait-object data pointers
  (the concrete `Self` may store the pointer for later access)
  or for raw pointers (`Ty::Ptr`, which carry no uniqueness
  contract). Absence of `restrict` is the safe default; the
  type system carries no `restrict` flag and `Ty::Ref(_,
  UniqueBorrow)` does not mechanically imply `restrict` at
  codegen.

This keeps the type vocabulary minimal and the C-side
qualification logic local to codegen.

#### 17.2.1 Recursive interior-mutability detection

The `is_unsafe_cell` flag (§8.2) is item-level — it marks the
`UnsafeCell` `Item` itself. Codegen `const`-derivation requires
*recursive* detection: any wrapper that transitively contains
`UnsafeCell<T>` (e.g., `Cell<T>`, `RefCell<T>`, the regseq DSL
`Atomic<T>` facade) must drop `const` from enclosing shared
references just as a bare `&UnsafeCell<T>` would.

```
contains_unsafe_cell(ty: TyId, model: &RegseqModel) -> bool
```

Recursion rules:

| `Ty` variant                         | Rule                                                                                                                          |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `Struct { item, .. }`                | `true` if `model.items[item].is_unsafe_cell`; else recurse into every `FieldLayout.ty` of the inline `Layout`.                |
| `Enum { item, .. }`                  | recurse into every `FieldLayout.ty` of every `VariantLayout`. (Enums are not lang-item-flagged at the wrapper level in v1.)   |
| `Union { item, .. }`                 | recurse into every `FieldLayout.ty`.                                                                                          |
| `Tuple { fields, .. }`               | recurse into each element (the inline `Layout`'s field types).                                                                |
| `Array(elem, _)`                     | recurse into `elem`.                                                                                                          |
| `Closure(item, args)`                | recurse via the parallel `Ty::Struct` (captures-struct) found via `find_nominal(NominalKind::Struct, item, args)`.            |
| `Ref(_, _)`, `Ptr(_, _)`, `FnPtr(_)` | `false` — interior mutability through a pointer does not propagate to the qualifier of the *outer* reference's pointee.       |
| `FnDef(_, _)`                        | `false` (ZST, no contained data).                                                                                             |
| `Bool`, `Char`, `Int`, `Uint`, `Float`, `Str`, `Slice`, `Dynamic`, `Never` | `false`.                                                                                |

Termination: `TyInterner` is acyclic at the value level
(self-referential ADTs cycle through `Ref`/`Ptr` per §12.2,
which the rule terminates on). Implementations memoise via a
visited `HashSet<TyId>` for cost.

C-qualification rule (codegen) — applies when the pointee `t` is
**sized**:

- `Ref(t, SharedBorrow)` lowers to `const T*` if
  `!contains_unsafe_cell(t)`, else `T*`.
- `Ref(t, UniqueBorrow)` lowers to `T*` (mutable borrow already
  drops `const`; `restrict` is a separate per-context decision).
- `Ptr(t, Mutability::Not)` lowers to `const T*` (raw pointers
  do not consult `is_unsafe_cell`; D9 / S0 framing).
- `Ptr(t, Mutability::Mut)` lowers to `T*`.

#### Wide-pointee carve-out

When the pointee `t` is **unsized** — `Ty::Str`, `Ty::Slice(_)`,
or `Ty::Dynamic{..}` — the rules above do **not** apply. The
`Ref`/`Ptr` value lowers as a fat-pointer aggregate (two-slot
struct), not as `T*`. The qualification applies to the
aggregate's `data` field per the table below; the second slot
(`len` for slice/str, `vtable` for dyn) is independent.

| Pointee + borrow                       | `data` field type                                                                                                                                              | Second slot                          |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `Ref(Str, SharedBorrow)`               | `const uint8_t *`                                                                                                                                              | `len` — target-width unsigned        |
| `Ref(Str, UniqueBorrow)`               | `uint8_t *`                                                                                                                                                    | `len` — target-width unsigned        |
| `Ptr(Str, Mutability::Not)`            | `const uint8_t *`                                                                                                                                              | `len` — target-width unsigned        |
| `Ptr(Str, Mutability::Mut)`            | `uint8_t *`                                                                                                                                                    | `len` — target-width unsigned        |
| `Ref(Slice(T), SharedBorrow)`          | `const T*` if `!contains_unsafe_cell(T)`, else `T*`                                                                                                            | `len` — target-width unsigned        |
| `Ref(Slice(T), UniqueBorrow)`          | `T*`                                                                                                                                                           | `len` — target-width unsigned        |
| `Ptr(Slice(T), Mutability::Not)`       | `const T*`                                                                                                                                                     | `len` — target-width unsigned        |
| `Ptr(Slice(T), Mutability::Mut)`       | `T*`                                                                                                                                                           | `len` — target-width unsigned        |
| `Ref(Dynamic{..}, _)` or `Ptr(Dynamic{..}, _)` | `void *` (unqualified) — see "Trait-object dispatch carve-out" below                                                                                          | `vtable` — `const VTable *`          |

The literal C struct typedef shapes (`struct rust_str`,
`struct rust_slice_T`, `struct trait_obj_Trait`) are codegen
contract; this section pins the *qualification and slot count*
the codegen must respect. See Appendix A.2 for worked examples.

**C boundary rejection.** The fat-pointer lowering rules above
apply to **Rust-internal** uses (Rust-ABI function calls,
value-context locals, struct fields). At `FnAbi::C`
parameter/return positions, `Ref`/`Ptr` to unsized pointees are
**rejected** by the C ABI admissibility predicate (§14.5).
Driver code crossing extern "C" passes the data pointer and
length / vtable pointer as separate parameters; the wide-
pointer aggregate is not idiomatic at the C boundary.

#### Trait-object dispatch carve-out

For `Ref(Dynamic{..}, _)` and `Ptr(Dynamic{..}, _)`, the
fat-pointer's `data` field is **unqualified `void *`**
regardless of the source borrow's `Provenance` or `Mutability`.

Reason: trait-object dispatch erases the concrete `Self` type.
The vtable shim's receiver is uniformly `*mut ()` (§13.2),
because shared `&self` methods on impls whose `Self` contains
`UnsafeCell` may legitimately mutate through the shared
reference — promising C `const` through type-erased dispatch
would be C undefined behaviour for those impls. The
`contains_unsafe_cell` predicate (above) cannot be evaluated
on `Dynamic{..}` (the concrete `Self` is not in the type), so
the only sound rule is to drop `const` uniformly at the
trait-object boundary.

The Rust borrow flavor is preserved at the type-system level
(the source `Ref(Dynamic{..}, SharedBorrow)` is still visible
in the IR); what's dropped is the C-level `const` promise on
the *erased data pointer*. The vtable pointer slot itself
remains `const VTable *` — vtable instances are immutable
statics.

### 17.3 C-shape over Rust-shape

RegseqModel uses C-shaped lowerings rather than mirroring
Rust-internal compiler shapes. The bridge does the desugaring:

- Drop-glue calls become regular `Call` terminators.
- Trait objects become vtables.
- Closures become structs + methods (§13.1).
- Generics get monomorphised away.
- `impl` blocks are lifted (§8.1).

Consumers see C-shape. Where rustc has done the work (layout
resolution, monomorphisation, region erasure), the bridge
**stores the result, not the input**.

### 17.4 Closed `Ty` enum

The `Ty` enum is closed at 19 variants for the v1 Regseq DSL
type universe. **Domain-specific features must extend
`InstanceKind` / `Item` flags / tags on existing IR nodes —
never `Ty` itself (Q3 single-level IR rule).** New Rust
*source-language* type forms (introduced by toolchain evolution
adding new rigid `TyKind` variants) are rejected unless
deliberately admitted by a future spec revision. The closed set
is binding for v1; revision is the path for source-language
expansion, not for domain extension.

### 17.5 Type-identity preservation contract (with explicit exception)

RegseqModel preserves Rust type identity end-to-end: rustc-
distinct types remain distinct in the IR. This is the D15 lock
that drove `FnAbi` storage on `FnSig`, the
`sig_c_abi_lowerings` side table (so `extern "C" fn(Handle)`
and `extern "C" fn(u32)` remain distinct), and the
`PrincipalTraitKey` shape (so `dyn Register<u32>` and
`dyn Register<u64>` remain distinct).

**Single exception: erased region-only distinctions accepted by
§17.1.** Free-region versus single-use input-bound-region
distinctions in fn-pointer types (e.g., `fn(&u32)` vs
`fn(&'static u32)`) are intentionally collapsed under the
admitted-binder subset of §17.1. They carry no runtime
representation, no codegen consequence (identical C output and
machine behaviour), and are subtype-related rather than
type-equal in rustc; the collapse is sound.

**This exception does not apply to:**

- *Nominal distinctions.* `Handle` and `u32` (when `Handle` is
  `repr(transparent)` over `u32`) remain distinct nominal
  types in the IR even though their C boundary lowering is
  identical (§14.5 `Substituted { emit_as }`).
- *ABI distinctions.* `fn() -> u32` and `extern "C" fn() -> u32`
  remain distinct (`FnAbi` is stored on `FnSig`).
- *Safety distinctions.* `fn()` and `unsafe fn()` remain
  distinct (`Safety` is stored on `FnSig`).
- *Const-generic distinctions.* `Buf<3>` and `Buf<4>` remain
  distinct (`ConstId` content uniqueness via `ConstInterner`).
- *Non-region structural distinctions.* Anything else.

The single region-only collapse is a *deliberate v1 trade-off*
documented in §17.1 with a future-relaxation path
(`binder_shape` carrier on `FnSig`).

### 17.6 Spec is mutable

If hand-lowering or specification surfaces awkward source-spec
constructs, the spec is revised — not worked around in the
compiler. This applies equally to the bridge (Q2), the
methodology (`README.md` §2), and the spec (this document).

---

## 18. Pretty-printer scaffold

The pretty-printer is the primary serialisation (Q5). Per-type
`Display` impls are insufficient because rendering requires
context (the model's tables for ID resolution).

```rust
pub struct PrettyPrinter<'a> {
    model: &'a RegseqModel,
    // optional: indentation level, span-rendering mode, etc.
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(model: &'a RegseqModel) -> Self;
    pub fn render(&self) -> String;
    pub fn render_ty(&self, ty: TyId) -> String;
    pub fn render_span(&self, span: Span) -> String;
    pub fn render_def_path(&self, item: ItemId) -> String;
    // ...
}
```

### 18.1 Sigil conventions

| ID             | Sigil                                                            |
| -------------- | ---------------------------------------------------------------- |
| `ItemId`       | `Item/N`                                                         |
| `BodyId`       | `Body/N` (rendered as the path of the owning item when in scope) |
| `TyId`         | rendered as the type itself (`u32`, `*const u8`, ...)            |
| `ConstId`      | rendered as the const value                                      |
| `LocalId`      | `_N` (zero-based; `_0` is `RETURN_PLACE`)                        |
| `BlockId`      | `bbN`                                                            |
| `SourceFileId` | rendered as the file path                                        |
| `StringId`     | rendered as the resolved string                                  |
| `Span`         | `path:line:col: line:col` (matches rustc's MIR-dump form)        |
| `InstanceId`   | `Item::<args>` (path of the item plus generic args)              |

### 18.2 Renderer status

The interface is pinned; only `render_span` is currently
implemented. Other renderer methods are stubs that panic with
messages indicating they are not yet filled. **Full-model
rendering is not yet available**; snapshot tests for the
complete stage-3 model surface are blocked on filling the
remaining renderers (§13 stage 13 owns this work).

Until then, snapshot tests covering the spec's structural
invariants use the secondary JSON serialisation (per Q5)
where readability is acceptable; tests that depend on
human-readable structural output are deferred to stage 13.
Calling the textual pretty-printer the "primary serialisation"
remains accurate as a *contract direction*, not a current
capability.

The textual pretty-printer is one-way (no parser); JSON
serialisation handles structured round-trip per Q5.

---

## Appendix A: Worked C-lowering examples

These examples illustrate the live spec.

### A.1 Reference and pointer types

**Rust:**
```rust
let a: &u32        = ...;   // shared borrow
let b: &mut u32    = ...;   // unique borrow
let c: *const u32  = ...;   // raw const
let d: *mut u32    = ...;   // raw mut
```

**Translated (post-bridge):**
```text
a: Ty::Ref(t_u32, Provenance::SharedBorrow)
b: Ty::Ref(t_u32, Provenance::UniqueBorrow)
c: Ty::Ptr(t_u32, Mutability::Not)
d: Ty::Ptr(t_u32, Mutability::Mut)
```

**C:**
```c
const uint32_t *a;            // SharedBorrow
uint32_t       *b;            // UniqueBorrow (codegen MAY add `restrict` in declaration contexts)
const uint32_t *c;            // raw const
uint32_t       *d;            // raw mut
```

### A.2 Fat-pointer types: `&str`, `&[T]`, `&dyn Trait`

**Rust:**
```rust
let s: &str = "hello";
let xs: &[u32] = &[1, 2, 3];
let d: &dyn Driver = &my_driver;
```

**C:**
```c
typedef struct rust_str { const uint8_t *data; size_t len; } rust_str_t;

typedef struct rust_slice_u32 {
    const uint32_t *data;
    size_t len;
} rust_slice_u32_t;

typedef struct trait_obj_Driver {
    void *data;                          /* unqualified per §17.2.1 trait-object dispatch carve-out */
    const Driver_VTable *vtable;
} trait_obj_Driver_t;

static const uint8_t __str_lit_0[] = { 'h', 'e', 'l', 'l', 'o' };  // no NUL
static const rust_str_t HELLO = { __str_lit_0, 5 };
```

### A.3 Niche-encoded enum: `Option<&u32>`

**Rust:**
```rust
let p: Option<&u32> = ...;   // 8 bytes; niche-encoded (None = 0, Some(p) = p)
```

**C:**
```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;
```

### A.4 UnsafeCell aliasing

**Rust:**
```rust
let x: &UnsafeCell<u32> = ...;
```

The `is_unsafe_cell` flag on the `UnsafeCell<u32>` `Item`
triggers `const`-drop on enclosing shared references.

**C:**
```c
uint32_t *x;   // NOT const, despite the source-side &T
```

### A.5 Packed struct

**Rust:**
```rust
#[repr(packed)]
struct Frame {
    kind: u8,
    addr: u32,
}
```

**Layout:** `pack: Some(1)`, `size: 5`, `align: 1`.

**C:**
```c
struct Frame {
    uint8_t  kind;
    uint32_t addr;
} __attribute__((packed));
```

### A.6 `&dyn Driver` lowering

**Rust:**
```rust
fn process(d: &dyn Driver) -> u32 { d.read() }
```

**C:**
```c
/* Vtable type: erased-receiver fn-pointer fields. Per §13.2,
   all receiver pointers are uniformly `*mut ()` (`void *`)
   regardless of the source borrow flavor — see the
   trait-object dispatch carve-out in §17.2.1 for the
   UnsafeCell soundness rationale. */
typedef struct Driver_VTable {
    void (*drop_in_place)(void *self);
    size_t size;
    size_t align;
    uint32_t (*read)(void *self);
} Driver_VTable_t;

/* Trait object: data field is unqualified `void *` per the
   trait-object dispatch carve-out (§17.2.1). The Rust
   SharedBorrow flavor is preserved at the type-system level
   but the C-level `const` is dropped at the erased boundary. */
typedef struct trait_obj_Driver {
    void *data;
    const Driver_VTable_t *vtable;
} trait_obj_Driver_t;

/* Method shim (§13.2). */
static uint32_t Driver_read__shim_PL011(void *self) {
    return PL011_read((PL011_t *)self);
}

/* Drop-in-place shim (§13.2). */
static void Driver__drop_shim_PL011(void *self) {
    PL011_drop_in_place((PL011_t *)self);
}

/* Vtable instance: both drop slot and method slot use shims. */
extern const Driver_VTable_t __vtable_PL011_for_Driver;
const Driver_VTable_t __vtable_PL011_for_Driver = {
    .drop_in_place = Driver__drop_shim_PL011,
    .size          = sizeof(PL011_t),
    .align         = _Alignof(PL011_t),
    .read          = Driver_read__shim_PL011,
};

uint32_t process(trait_obj_Driver_t d) {
    return d.vtable->read(d.data);
}
```

### A.7 Closure lowering

**Rust:**
```rust
struct Token { handle: u32 }
impl Drop for Token {
    fn drop(&mut self) { /* release the handle; details elided */ }
}
impl Token {
    fn into_handle(self) -> u32 {
        let h = self.handle;
        core::mem::forget(self);
        h
    }
}

fn make_consumer(t: Token) -> impl FnOnce() -> u32 {
    move || t.into_handle()
}

let c = make_consumer(Token { handle: 42 });
let value = c();
```

**C:**
```c
typedef struct Token { uint32_t handle; } Token_t;

void Token_drop(Token_t *self) {
    /* release the handle */
}

uint32_t Token_into_handle(Token_t self) {
    uint32_t h = self.handle;
    /* core::mem::forget suppresses Drop; no Token_drop call here. */
    return h;
}

typedef struct closure_make_consumer_0 {
    Token_t t;
} closure_make_consumer_0_t;

uint32_t closure_make_consumer_0__call_once(closure_make_consumer_0_t self) {
    return Token_into_handle(self.t);
}

void closure_make_consumer_0__drop(closure_make_consumer_0_t *self) {
    Token_drop(&self->t);
}

closure_make_consumer_0_t make_consumer(Token_t t) {
    return (closure_make_consumer_0_t){ .t = t };
}

closure_make_consumer_0_t c = make_consumer((Token_t){ .handle = 42 });
uint32_t value = closure_make_consumer_0__call_once(c);
```

### A.8 MMIO read via intrinsic

**Rust:**
```rust
#[repr(C)]
struct UartFlagsReg(u32);

#[rseq::intrinsic("mmio_read")]
fn mmio_read_u32(base: *mut u8, offset: usize) -> u32 {
    todo!("facade stub: lowered by the compiler backend")
}

fn poll_busy(base: *mut u8) -> bool {
    let flags = mmio_read_u32(base, 0x18);
    (flags & (1 << 5)) != 0
}
```

**C (Linux target):**
```c
bool poll_busy(uint8_t *base) {
    uint32_t flags = readl((volatile void __iomem *)(base + 0x18));
    return (flags & (1u << 5)) != 0;
}
```

The volatility lives on the intrinsic `InstanceId`'s codegen, not
as an IR-level flag.

### A.9 `Atomic<u32>` with acquire-load (Linux target)

**C:**
```c
/* NO `const` on the parameter, because Atomic_u32_t transitively contains
   UnsafeCell<u32> and the `is_unsafe_cell` flag drops `const`. */
static inline uint32_t read_count(Atomic_u32_t *counter) {
    return smp_load_acquire(&counter->value);
}
```

`smp_load_acquire` rather than `atomic_read_acquire` because
`atomic_read_acquire` takes `const atomic_t *` (an opaque
struct), and type-punning a `const uint32_t *` to that is not
valid in general.

### A.10 `repr(u32)` enum with user-assigned tags

**Rust:**
```rust
#[repr(u32)]
pub enum Status {
    Idle   = 0,
    Active = 1,
    Error  = 0xFF,
    Reset  = 0xDEAD,
}

fn priority(s: Status) -> u32 {
    match s {
        Status::Idle   => 0,
        Status::Active => 1,
        Status::Error  => 100,
        Status::Reset  => 200,
    }
}
```

**Layout:** `EnumShape::Multiple` with `TagEncoding::Direct`,
`tag.scalar_ty: Uint(U32)`,
`valid_range: WrappingRange { start: 0, end: 0xDEAD }`. Each
`VariantLayout` carries `discriminant` (semantic) and
`tag_value: Some(discriminant)` (Direct encoding writes the
discriminant verbatim).

**C:**
```c
typedef struct Status {
    uint32_t tag;
} Status_t;

#define STATUS_IDLE   0u
#define STATUS_ACTIVE 1u
#define STATUS_ERROR  0xFFu
#define STATUS_RESET  0xDEADu

uint32_t priority(Status_t s) {
    switch (s.tag) {
        case STATUS_IDLE:   return 0;
        case STATUS_ACTIVE: return 1;
        case STATUS_ERROR:  return 100;
        case STATUS_RESET:  return 200;
    }
}

// SetDiscriminant(Status::Error) -> place->tag = STATUS_ERROR;
//   codegen reads VariantLayout.tag_value = Some(0xFF) and emits
//   0xFF as the tag value, NOT 2 (variant index).
```

### A.11 Niche-encoded `Option<&u32>` with discriminant fields

**Rust:**
```rust
fn f(x: Option<&u32>) -> u32 {
    match x {
        Some(p) => *p,
        None    => 0,
    }
}
```

**Layout:** `EnumShape::Multiple` with `TagEncoding::Niche {
untagged_variant: Some, niche_variants: None..=None,
niche_start: 0 }`.

| Variant | `discriminant` | `tag_value` |
| ------- | -------------- | ----------- |
| `None`  | `0`            | `Some(0)`   |
| `Some`  | `1`            | **`None`**  |

`tag_value: None` for `Some` because `Some` is the untagged
variant — the inner pointer's bit pattern IS the tag.

**C:**
```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;

uint32_t f(Option_ref_u32_t x) {
    if (x.p == NULL) return 0;        // None
    return *x.p;                      // Some
}

// SetDiscriminant(Option::None) -> place->p = (const uint32_t *)0;
//   /* tag_value = Some(0) */
// SetDiscriminant(Option::Some) -> /* no-op */
//   /* tag_value = None: untagged variant; payload IS the tag. */
// Construction of Option::Some(p) -> place->p = p;
//   /* Separate MIR field-write statement, NOT a SetDiscriminant. */
```

### A.12 Rejected `*mut c_void` usage

**Driver source (rejected counter-example):**
```rust
use core::ffi::c_void;

fn register_callback(cb: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    todo!("hypothetical")
}
```

The bridge rejects this at type translation:

- Translating `*mut c_void` encounters the `c_void` ADT.
- `tcx.is_lang_item(c_void_def_id, LangItem::CVoid)` returns
  `true`.
- Bridge raises a diagnostic naming `core::ffi::c_void` as
  out-of-DSL-scope in v1, recommends the opaque-struct pattern.

**Suggested rewrite:**
```rust
#[repr(C)]
struct OpaqueCtx { _private: [u8; 0] }

fn register_callback(cb: extern "C" fn(*mut OpaqueCtx), ctx: *mut OpaqueCtx) {
    todo!("hypothetical")
}
```

The opaque-struct pattern is structurally an `Adt` (an ordinary
`Ty::Struct`) and lowers to a forward-declared struct pointer
(`struct OpaqueCtx *`), a valid opaque-handle shape on both
Linux and Zephyr targets.
