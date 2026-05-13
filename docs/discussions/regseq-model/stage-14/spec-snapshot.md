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
+ stage 4 (constants and literals) + stage 5 (MIR places and
projections) + stage 6 (operands) + stage 7 (rvalues) +
stage 8 (statements) + stage 9 (terminators) + stage 10 (basic
blocks) + stage 11 (MIR bodies; with stage-12 D-12-1.4 / Option B
reframe of D-11-3 + D-11-5 absorbed in §9.4 / §14.3 / §23.6.2 /
§25.4 / §25.5) + stage 12 (Rust Items) + stage 13 (pretty-printer
spec split: §18 stub + §19.8 / §20.6 / §21.8 / §22.8 / §23.9 /
§24.9 cross-references to `pretty-printer.md` per D-13-9.2;
new §10.4 rendering pointer; §24 / §25 inline cleanup)
contributions. The pretty-printer surface itself lives in the
co-equal merger artifact `pretty-printer.md` per D-13-9.

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
- Spec before implementation (phase-1 implementation deferred
  to a single end-of-phase-1 sweep; no per-stage interleaving).
- Compiler-theory-first.
- Compositional behavior beats special-case flags (when a
  behavior derives from existing rules, do not introduce a
  parallel flag-style marker; variant tags + the existing
  rule chain carry the distinction).
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
    pub sigs:   FnSigInterner,    // per stage-12 D-12-1.4 (Option B),
                                  // every Function Instance carries a
                                  // substituted FnSigId via the Instance-side
                                  // schema (`InstanceKind::Function { fn_sig:
                                  // FnSigId, body: Option<Body>, ... }` per
                                  // §9.4). Foreign decls have body: None but
                                  // still carry a sig (rustc's tcx.fn_sig is
                                  // callable for foreign fns). Different
                                  // substituted signatures get distinct
                                  // FnSigIds; identical substituted signatures
                                  // share one FnSigId via FnSigInterner's
                                  // content-keyed identity rule per §14.4.

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

    // No separate BodyId. Per stage-12 D-12-1.4 (Option B;
    // supersedes stage-11 D-11-5's Item-side framing), bodies
    // live as a structural field on `InstanceKind::Function {
    // body: Option<Body>, ... }` per §9.4. Per-monomorphization
    // Instance carries its own substituted body. See
    // `discussions.md` D-12-1.4 for the Option-B reframe
    // rationale.

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

    // Domain attribute sibling tables per stage-14 D-14-3
    // (architecture) + D-14-1 (intrinsic recognition + storage).
    // Hosts structured DSL property attributes derived from the
    // `#[rseq::*]` tool namespace. Three sibling tables keyed by
    // `ItemId` / `FieldPath` / `Span` -- `instance_attrs` was
    // considered (per stage-4's original 4-key-kind enumeration)
    // but dropped per D-14-3.1 because Instance access to Item-
    // level metadata goes through the §3.5.2 pull-through pattern
    // (stage-12 D-12-1.8). See §3.6 for the full architecture
    // (the `AttrData` discriminated union, recognition codepath,
    // population rules, validation invariants, attribute-
    // namespace storage rule, per-family target-table mapping).
    pub domain_attrs: DomainAttrTables,
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
   `CrateId`, `TyId`, `ConstId`, `ItemId`, `InstanceId`,
   `FnSigId`, `SourceFileId`, `StringId`, `VariantIdx` resolves to
   a valid index in its table. Debug-build assertion. (No
   `BodyId` per stage-12 D-12-1.4 / Option B (supersedes stage-11
   D-11-5); bodies live structurally on `InstanceKind::Function`
   per §9.4.)
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
10. **Body presence matches `InstanceKind::Function` shape.**
    Per stage-12 D-12-1.4 (Option B; supersedes stage-11 D-11-5):
    bodies live structurally on `InstanceKind::Function { body:
    Option<Body>, ... }` per §9.4. Every `Instance` with
    `kind = Function { body: Some(b), ... }` has its body via the
    Instance-side field; `body: None` is reserved for foreign
    declarations (`extern "C" fn` with `tcx.is_foreign_item(did)`
    returning true) per D-12-2.1. Non-Function InstanceKinds
    (Static / Const / Struct / Enum / Union) have no `body`
    field at all. See `discussions.md` D-12-1.4 for the
    Option-B reframe rationale.
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
    distinct in the IR (e.g.,
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
      in the model per §11.3; presence of
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
   `sig_c_abi_lowerings`, `types` storage, `instances`
   into raw in-memory state. (Bodies live structurally
   inside Function Instances on
   `InstanceKind::Function { body: Option<Body>, ... }`
   per stage-12 D-12-1.4 / Option B; no separate
   `bodies` table.)
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
3. **Items / crates / target / build / construction-invariants
   3, 4, 6 closure** (sub-step ordering per stage-12 D-12-4).
   Sub-steps:
   - **(a)** Validate `crates`: stable_crate_id uniqueness
     + sorted (invariant 5; existing).
   - **(b)** Validate `items` shape: `item_def_paths.len() ==
     items.len()`, `item_synthetic_kinds.len() == items.len()`
     (invariant 2 length parity; existing).
   - **(c)** Validate `item.def_path.crate_id.index() <
     crates.len()` for every item (D-12-4.2 closes invariant 6).
     Bound check; out-of-bounds is
     `ValidationError::DefPathCrateIdOutOfBounds`. Must precede
     (d) because (d)'s F4 sort key reads
     `crates[item.def_path.crate_id].name`.
   - **(d)** Validate F4 sort monotonicity over `items`
     (D-12-4.1 closes invariant 3). Walk index-pair-wise; for
     each adjacent pair, compute the F4 sort key per §16.1 and
     check non-strict monotonicity (`key[i] <= key[i+1]`).
     Out-of-order is `ValidationError::F4OutOfOrder`.
     Adjacent items with identical sort keys are NOT a
     violation -- F4 admits ties on identical DefPath; check
     is `>` (strict descent), not `>=`.
   - **(e)** Validate `crates[primary_crate].stable_crate_id ==
     bridge_input.spec_crate_stable_id` (D-12-4.3 closes the
     spec-crate-stable-id half of invariant 4). The
     `validate(&self, bridge_input: &BridgeInput)` signature
     change at stage 12 absorbs `BridgeInput` per §3.5
     (`pub struct BridgeInput { pub spec_crate_stable_id:
     StableCrateId }`). Mismatch is
     `ValidationError::SpecCrateStableIdMismatch`. The other
     half of invariant 4 (`primary_crate.index() <
     crates.len()`) is already model-checkable per stage-1.
   - **(f)** Validate `target` well-formedness (invariant 15;
     existing), including `scalar_layouts` populated.
   - **(g)** Validate `build` well-formedness (invariant 17;
     existing).
4. Validate `ConstInterner` type-level content — the
   type-level const phase per §9.7 "Const validation phase
   split." Validates: (a) for every type-level scalar entry
   (`Bool` / `Char` / `Int` / `Uint` / `Float`), recompute the
   canonical-content key per §9.7's per-`Const`-variant
   byte-encoding tags and verify it matches the stored
   `ConstInterner.lookup` key (canonical-form
   well-formedness); (b) injectivity over the type-level
   subset of `ConstInterner.lookup` (no two type-level entries
   share a canonical key); (c) every `ConstId` appearing in
   `GenericArg::Const(_)` positions resolves to a type-level
   scalar variant. Value-level entries are NOT recomputed at
   this step — their canonical-content keys involve recursion
   through `Aggregate.0` / `EnumValue.payload` and require
   const-graph acyclicity (verified at step 9) to terminate.
   Value-level sub-reference resolution (e.g., `NullPtr`'s
   pointee `TyId`, `Reloc`'s `InstanceRef` projection) and
   value-level lookup injectivity defer to step 9 after
   `TyInterner` and `instances` validate.
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
8. Validate `instances`: every `Instance.item` and
   `Instance.kind`-payload IDs resolve; `InstanceKind`
   layout consistency (invariant 14); for
   `Function { fn_sig, body, flavor, virtual_idx }` (per
   stage-12 D-12-1.4 / Option B; supersedes stage-11
   D-11-3 + D-11-5):
   - `fn_sig` resolves to a valid entry in
     `model.sigs` (§14.4 FnSigInterner).
   - `body.is_none()` iff the parent Item's `linkage ==
     Linkage::Foreign` (model-side biconditional). This is
     the validation-time check; the bridge's source predicate
     at construction is `tcx.is_foreign_item(did)` per
     D-12-2.1, but validation does NOT call rustc -- it
     verifies the stored `Item.linkage` and `Instance.body`
     are consistent. `body.is_some()` for source-defined
     functions, bridge-synthesized shims (drop glue, vtable
     shims, closure call methods), and any other Item with
     `linkage != Linkage::Foreign`.
   - The `(RegseqInstanceKey)` projection is unique
     across all instances (invariant 16-derived).
   - `flavor` matches the bridge's F4-2 sort key (§9.3).
   - The §9.4 construction invariant `virtual_idx.is_some()
     iff flavor == FunctionInstanceFlavor::Virtual` holds.

   NOTE: body **content** validation (Place validation per
   §19.7's five rules; statement / terminator
   well-formedness; basic-block edge consistency) lands at
   stage 11's body-validation step. Stage-1 bodies are
   empty; the body-content walk is non-vacuous only once
   stage 11 materialises `LocalDecls` and the body's
   basic-blocks. Place validation rules are stage-5-owned
   (§19.7); the walk is stage-11-owned (see
   `discussions.md` D-5-2 for the deferral rationale).
   Under Option B, the joint validator iterates over
   `model.instances`; for each Function Instance with
   `body: Some(b)`, runs the body visitor against `b`.

   For `InstanceKind::Static { ty, init }` (per
   stage-12 D-12-1.2 (c) 2-field schema; mutability lives
   on parent `ItemKind::Static`):
   - `ty` resolves in `model.types`.
   - `init.is_none()` iff the parent Item's `linkage ==
     Linkage::Foreign` (model-side biconditional; matches
     the Function `body` biconditional above per D-12-2.2).
     Bridge's source predicate is
     `tcx.is_foreign_item(static_did)` at construction;
     validation reads `Item.linkage` only.
   - When `init: Some(c)`, `c` resolves in `model.consts`.
9. Validate value-level `Const` content sub-references per the
   §9.7 invariants (value-level const phase per §9.7 "Const
   validation phase split"):
   - **`NullPtr(t)`**: `t` resolves in `types`; `t` is a
     pointer-pointee-admissible type per the consumer's
     context.
   - **`Reloc(r)`** and **`Bytes::Reloc(r)`**: each
     `InstanceRef r` projects to exactly one `Instance` in
     the `instances` table. The projection is by re-deriving
     `InstanceRef` from each Instance's
     `(regseq_kind_rank, def_path_hash, args_key,
     fn_shim_disambig)` and checking equality with `r`.
   - **`Bytes(vec)`**: byte-sequence well-formedness per §9.7
     (maximal-relocation-run rule, length matches the
     consumer's expected size, etc.).
   - **`Aggregate(children)`**: every `child: ConstId`
     resolves; type-level scalar variants are admissible as
     children of value-level aggregates (the type-level /
     value-level boundary is variant-intrinsic and
     non-transitive — a value-level aggregate may carry
     type-level scalar children without becoming type-level
     itself).
   - **`EnumValue { variant_idx, payload }`**: `variant_idx`
     is in-bounds for the consumer-context's `Ty::Enum`'s
     `EnumShape.variants` (per §11.1); every
     `payload[i]: ConstId` resolves and is field-type
     admissible per the variant's `VariantLayout.fields`.
   - **Const-graph acyclicity.** The `Aggregate.0` and
     `EnumValue.payload` recursion edges (the only `ConstId`-
     valued positions inside `Const`; `Bytes::Reloc` and
     `Reloc` carry `InstanceRef`, not `ConstId`, so they do
     not extend the const-recursion graph) form a DAG: no
     `ConstId` appears in its own transitive `Aggregate` /
     `EnumValue` payload reachability set. Validation uses
     **three-state DFS marks** (`Unseen`, `Visiting`, `Done`):
     entering a node sets it to `Visiting`; leaving sets it
     to `Done`; an edge to a `Visiting` node is a back-edge
     and a model-corruption error; an edge to a `Done` node
     is skipped (shared DAG children are admissible). A
     single-bit visited-set is insufficient — it cannot
     distinguish "currently on the recursion stack" (cycle)
     from "already fully validated" (DAG sharing). Well-typed
     Rust + const-eval termination guarantee acyclicity at
     construction time; the explicit check defends snapshot
     integrity against tampered serialised models.
   - **Value-level lookup injectivity and canonical-form
     well-formedness.** *Ordering note: this sub-check runs
     after acyclicity above so that recursive
     `canonical_const(ConstId)` computation terminates.* For
     every value-level entry (`ZST`, `Bytes`, `Aggregate`,
     `EnumValue`, `NullPtr`, `Reloc`), recompute the
     canonical-content key per §9.7's per-`Const`-variant
     byte-encoding tags (the recursion into `Aggregate.0` /
     `EnumValue.payload` is now safe: the DAG verified above
     bounds the call depth to the topological-order length).
     Verify the recomputed key matches the stored
     `ConstInterner.lookup` key (canonical-form
     well-formedness); verify injectivity over the
     value-level subset of `ConstInterner.lookup` (no two
     value-level entries share a canonical key). Combined
     with step 4's type-level check, this completes
     `ConstInterner.lookup` injectivity over the full key set.
   - **`ConstMemoryStatic` dedup.** For every Item `I` with
     `item_synthetic_kinds[I] = SyntheticItemKind::ConstMemoryStatic
     { content_hash }`, recompute `ContentHash` from the
     resolved Static's `init: Some(c)` content (where `c` is
     the `ConstId` for the `Const::Bytes(Vec<Byte>)` value).
     For each such Item, this sub-check first verifies the
     `ConstMemoryStatic` Static-shape invariants inline:
     (a) the parent Item has `kind: ItemKind::Static
     { mutability: Mutability::Not }` (mutability lives on
     ItemKind per stage-12 D-12-1.2 (c)) and the corresponding
     Instance has `kind: InstanceKind::Static
     { ty: array_ty, init: Some(c) }` (no `init: None`;
     2-field Instance-side schema per Option B);
     (b) `array_ty` resolves to
     `Ty::Array(elem_ty, n)` with `elem_ty` resolving to
     `Ty::Uint(UintKind::U8)` (byte-array shape);
     (c) `c` resolves to a `Const::Bytes(Vec<Byte>)`; (d) `n`
     equals the resolved `Const::Bytes` sequence length. After the shape check,
     recompute `ContentHash` from the resolved
     `Const::Bytes(Vec<Byte>)` per the canonical byte form per
     §9.7's per-`Const`-variant byte-encoding tags, verify it
     matches the stored `content_hash` field, and verify no
     two `ConstMemoryStatic` Items share a `content_hash`
     value (content-keyed Item uniqueness — two
     anonymous-memory allocations with identical canonical
     byte form must collapse to one synthesised Static). The
     "Phase B continued — invariant enforcement" subsection
     below restates the same Static-shape invariants
     standalone (so consumers walking the invariant list
     find the same rule); the two presentations are
     equivalent. *Ordering note: this sub-check depends on
     the value-level lookup injectivity pass above (it
     recomputes `Const::Bytes` canonical bytes for each synth
     Static's `init`), so it runs after acyclicity and after
     the value-level recomputation pass.*
10. Validate `domain_attrs` per §3.6 (stage-14 D-14-1 +
    D-14-3). Four checks per sibling table (`item_attrs`,
    `field_attrs`, `span_attrs`):
    - **Top-level well-formedness.**
      `domain_attrs.item_attrs.len() == model.items.len()`
      (the `IndexVec` backbone covers exactly the assigned
      ItemId range; Phase 4 initialises this per §3.4).
      Every `FieldPath` key in `domain_attrs.field_attrs`
      satisfies: the embedded `ItemId` resolves to an Item
      whose `kind` is `Struct` / `Enum` / `Union`; the
      `VariantIdx` is in-bounds for the parent ADT's
      variant set (`0` for `Struct` / `Union`; in-range
      per `EnumShape.variants` for `Enum`); the `FieldIdx`
      is in-bounds for the indicated variant's
      `VariantLayout.fields`. Every `Span` key in
      `domain_attrs.span_attrs` is a structurally valid
      `Span` (per §5.3) and inherits the existing
      sentinel-aware **Source-file span bounds** rule
      enforced earlier in Phase B (this same §3.3 section):
      for `SourceFile { content: Some(_), .. }`, the key
      must satisfy `lo <= hi <= content.len()` with `lo`
      and `hi` at valid UTF-8 byte boundaries; for
      `SourceFile { content: None, .. }` (the synthesised
      sentinel or upstream-crate sources), the key is
      accepted as-is and would resolve to `None` via
      `Span::slice` per §5.4 if consumed.
    - **Nested ID well-formedness.** Every nested ID
      inside an `AttrData` payload (`ItemId`, `FnSigId`,
      `StringId`, embedded `FieldPath` components, etc.,
      depending on the per-family payload type) resolves
      against the relevant model table.
    - **Per-family duplicate invariants.** Stage-14
      commits **I-DomainAttr-Intrinsic-Singleton**: for any
      `ItemId id`, the set
      `domain_attrs.item_attrs[id].iter()
      .filter(|a| matches!(a, AttrData::Intrinsic(_)))`
      has cardinality at most 1. Phase-2 stages specify
      analogous per-family invariants for their `AttrData`
      variants at their merger close.
    - **`Vec<AttrData>` ordering.** Bridge-walk-order
      deterministic per D-14-3.3; the validator verifies
      structural consistency only and does NOT re-derive or
      re-sort entries.

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
  - `ConstMemoryStatic { .. }` → `ItemKind::Static`.
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
  rustc-side fact not stored in the model (per §11.3; see
  `discussions.md` D11 for rationale).
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
  checks: (a) the parent Item has `kind: ItemKind::Static
  { mutability: Mutability::Not }` (mutability lives on
  ItemKind per stage-12 D-12-1.2 (c)); (b) the corresponding
  Static Instance has `kind: InstanceKind::Static { ty:
  vtable_ty, init: Some(_) }` (Instance-side ty + init per
  Option B and the 2-field InstanceKind::Static schema).
  Vtable instances must be immutable (Item-side check) with
  a present initialiser (Instance-side check); mutable or
  missing-init vtables are model corruption.
- **`ConstMemoryStatic` Items are immutable byte-array
  statics with `Bytes` initialiser.** For every Item with
  `item_synthetic_kinds[id] == SyntheticItemKind::ConstMemoryStatic
  { content_hash }`, validation checks: (a) the parent Item
  has `kind: ItemKind::Static { mutability: Mutability::Not }`
  (Item-side mutability check per D-12-1.2 (c)); (b) the
  corresponding Static Instance has `kind: InstanceKind::Static
  { ty: array_ty, init: Some(c) }` with a present initialiser
  (Instance-side ty + init check; mutability moved to Item
  per Option B); (c) the `array_ty` resolves to `Ty::Array(elem,
  n)` with `elem` resolving to `Ty::Uint(UintKind::U8)`
  (byte-array shape); (d) the `c` resolves to a
  `Const::Bytes(Vec<Byte>)` value (per the type-erase-to-bytes
  discipline pinned in §9.7); (e) the `n` field of the array
  Ty equals the resolved `Const::Bytes` sequence length.
  Mutable (per Item-side), missing-init, non-byte-array, or
  non-Bytes-init `ConstMemoryStatic` Items are model
  corruption.
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

### 3.4 Bridge construction-pipeline order

Per stage-12 D-12-5. The bridge produces a `RegseqModel` in
five sequential phases. Phases 1-4 establish the model-side
state (post-sort F4-deterministic ItemIds); Phase 5
constructs Instances and resolves cross-references. The
contract is that downstream consumers see post-sort
ItemIds with the F4-determinism property (§16); how the
bridge reaches that state internally (placeholder ItemIds
during construction, DefPathHash-keyed cross-references,
etc.) is bridge-implementation choice.

**Phase 1 -- DISCOVERY.** Bridge calls rustc's
`collect_crate_mono_items(tcx, strategy)` once to get the
mono-item closure: roots (`#[no_mangle]` exports, lang-item
entries, etc.) + transitive closure through MIR mentions.
Result: `Vec<MonoItem>` covering primary crate + all
regseq-aware dependency crates per Q6. Cite:
`rustc_monomorphize/src/collector.rs:1803`
(`collect_crate_mono_items` entry point).

**Phase 2 -- TRANSLATION (with interleaved synthesis).**
Bridge walks the discovered mono-item set. For each
MonoItem, translates rustc state into RegseqModel content.
During translation, synthesis triggers fire and produce
additional bridge-synthesised Items per §13:

| Trigger | Synthesis | §13 reference |
|---|---|---|
| `Ty::Dynamic` translation | VTableType Item | §13.2 (D5) |
| `Cast::Unsize` rvalue | VTableInstance + VTableShim Items | §13.2 (D5) |
| `Ty::Closure` translation | ClosureCaptures Item | §13.1 (D6) |
| Closure call site | ClosureCall Function Items | §13.1 (D6) |
| Non-trivial drop on closure | ClosureDropGlue Function Item | §13.1 (D6) |
| Reference-typed const | ConstMemoryStatic Item | §13.3 (D-4-3) |
| `Drop` terminator -> drop-glue Function Item synthesis | drop-glue Function Item (the call target for the eventual Phase-5 Drop-as-Call rewrite) | stage-9 §23.4 |

Note: the **Drop-as-Call collapse itself** (rewriting a
rustc `Drop` terminator into a `Statement::Assign +
TerminatorKind::Call` inside a Body) is not a Phase-2
Item-synthesis trigger -- it is a Phase-5 body-construction
step (see §25.7.1 + the "Per-phase body work in Phase 5"
note below). Phase 2 only synthesises the drop-glue
Function Item that the Phase-5 rewrite's Call terminator
will target.

Synthesised Items are interleaved with source-derived Items
in the accumulating items collection. Each Item carries its
DefPathHash (rustc's for source-derived; synthetic per §13.4
for synthetic).

Synthesis is cached during Phase 2 to make repeat triggers
idempotent. **The Phase-2 cache cannot key on final
`ItemId`** because final ItemIds are not assigned until
Phase 4 (post-sort). The bridge instead uses a Phase-2
working cache keyed by a stable Phase-2-available handle:

- For vtable types: `PrincipalTraitKey ->
  Phase2Handle` (DefPathHash of the synthesised Item, or
  registration index, or a placeholder ItemId-shaped
  handle -- bridge implementation choice per D-12-5.3).
- For closure captures: `closure_did -> Phase2Handle`
  (rustc's closure DefPathHash is stable and Phase-2-
  available, since the closure expression has its own
  rustc DefId).
- For ConstMemoryStatic: `ContentHash -> Phase2Handle`.

The model-side `RegseqModel.vtables: HashMap<PrincipalTraitKey,
ItemId>` (and analogous final-form caches) is populated
in Phase 5 after final ItemIds exist (per the §3 root
struct comment on `vtables`). Phase 4's ItemId-assignment
pass establishes the
`Phase2Handle -> ItemId` mapping; Phase 5 walks the
Phase-2 cache and re-keys to final ItemIds. Idempotent:
revisiting the same trigger during Phase 2 yields the
previously-synthesised Item via the Phase-2 cache.

**Phase 3 -- F4 SORT.** Once all Items (source + synthetic)
are accumulated, the bridge sorts them by the F4 comparator
per §16.1: `(crate_name, crate_stable_id, segments)`.
Source-derived Items use rustc's DefPath segments verbatim;
synthesised Items use the §13.4 reserved-prefix segments.
The two families coexist in one sort because the F4
comparator treats `<regseq_synth>` (synthetic-domain marker)
as an ordinary `TypeNs(StringId)` segment that sorts
lexicographically.

**Phase 4 -- ItemId ASSIGNMENT + side-table population.**
Sorted Items get sequential `ItemId(0..items.len())`. In
parallel:
- `item_def_paths[ItemId]` populated with each Item's
  DefPathHash (rustc's for source-derived; synthetic per
  §13.4 for synthetic; with the ClosureCaptures carve-out
  per §3.1 invariant 2).
- `item_synthetic_kinds[ItemId]` populated with the per-Item
  `SyntheticItemKind` tag per §13.3 (`None` for
  source-derived).
- `domain_attrs.item_attrs` initialised to length
  `model.items.len()` (each slot starts as an empty
  `Vec<AttrData>`); per-family Phase-2 caches keyed by
  `DefPathHash` (and analogous handles for `field_attrs`
  / `span_attrs`) get re-keyed and inserted in this same
  pass (per stage-14 D-14-1.7; see §3.6.4 for the
  Phase-2-recognition → Phase-4-materialization protocol).

**Phase 5 -- INSTANCE construction + cross-reference
resolution.** Bridge populates `instances` per §9.2 (one
Instance per (Item, GenericArgs, rustc shim flavor) tuple).
Instances are sorted by F4-2 `RegseqInstanceKey` per §16.3.

During Instance construction, each Function Instance is
populated per stage-12 D-12-1.4 (Option B):
- `fn_sig`: substituted post-mono `FnSig` from rustc's
  `tcx.fn_sig(did).instantiate(tcx, args)`, interned via
  `FnSigInterner`.
- `body`: substituted post-mono MIR body from rustc's
  `tcx.instance_mir(InstanceKind)`. `None` if
  `tcx.is_foreign_item(did)` (per D-12-2.1).
- `flavor`: `FunctionInstanceFlavor` from rustc's
  `ty::InstanceKind` discriminant per §9.3
  `rustc_instance_kind_sort_rank`.
- `virtual_idx`: rustc's vtable-slot index for Virtual
  flavor; `None` otherwise (per the §9.4 biconditional).

Cross-references that referenced Items by an inter-Phase
placeholder (DefPathHash, registration index, or whatever
the bridge implementation chose) are resolved to final
ItemIds. `RegseqModel.vtables` (PrincipalTraitKey -> ItemId
dedup table) populated as Phase 5 walks Items.

**Per-phase body work in Phase 5 (body-content rewrites
sequenced before validation).** Phase 5 body materialisation
(populating `InstanceKind::Function.body: Option<Body>`)
includes two body-content rewrites that produce the canonical
Body shape downstream consumers see:

1. **Drop-as-Call collapse** (per §25.7.1): the rustc body's
   `TerminatorKind::Drop` terminators are rewritten in place
   into `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut,
   place)) + TerminatorKind::Call { callee:
   Direct(drop_glue_instance_id), ... }`. The drop-glue
   Function Item that this Call targets was synthesised in
   Phase 2 (per the Phase-2 trigger table above) and got its
   final ItemId in Phase 4; Phase 5 just inserts the Call
   terminator referencing that already-final ItemId. The
   collapse runs inside the bridge's atomic per-rustc-
   terminator translation envelope.
2. **Vtable-indirect rewrite** (per §25.7.2): a body-level
   pass over the materialised Body that detects `Call`
   terminators with virtual-flavor callees and rewrites them
   into projection-chain Statements + an indirect Call.
   Sequenced **after** Drop-as-Call collapse and **before**
   the joint validation walk (§25.6).

After Phase 5 body materialisation + the body-level passes,
the joint validator (§25.6) runs over `model.instances`; for
each Function Instance with `body: Some(b)`, it runs the body
visitor against `b`. §25.7.4 records the full pass-scheduling
order (Phase 5 body-materialisation with Drop-as-Call collapse
and DiagLogPeel both interleaved per-terminator ->
vtable-indirect rewrite -> joint validation).

**Synthesis interleaving rule (D-12-5.2).** Bridge-
synthesised Items are produced **during** Phase 2
translation, NOT in a separate pre-pass before discovery
or post-pass after. Synthesis triggers are
construct-driven (`Ty::Dynamic` encountered -> vtable type
Item; `Cast::Unsize` rvalue encountered -> vtable instance
Item). The bridge can't pre-enumerate triggers without
first translating; trying to pre-enumerate would
re-implement type translation. Synthesis must complete
before Phase 3 F4 sort.

**Post-sort ItemId assignment rule (D-12-5.3).** Final
`ItemId` values are assigned at Phase 4 (after F4 sort),
not incrementally during Phase 2 translation. F4 + the
build-equivalence quad (§12.4) commit to deterministic
ItemIds across builds with the same source. Incremental
assignment in discovery order would tie ItemIds to rustc's
collector traversal order, which can vary with incremental
compilation cache state. Cross-reference handling during
construction (placeholder ItemIds + remap, vs DefPathHash
canonical key) is bridge-implementation choice; not
externally observable. Spec only commits to **post-sort
F4-deterministic ItemIds** as the externally-observable
property.

### 3.5 `BridgeInput` and Instance-side helpers

Per stage-12 D-12-4.3 + D-12-1.8.

#### 3.5.1 `BridgeInput` struct

```rust
/// Bridge-supplied facts that are NOT stored in the model
/// itself but are needed by validation. Currently a single
/// field for the spec-crate-stable-id half of construction
/// invariant 4; future stages may extend (open-extension)
/// when new bridge-supplied facts surface.
pub struct BridgeInput {
    /// StableCrateId of the spec crate the bridge was invoked
    /// on. Validation checks this matches
    /// `crates[primary_crate].stable_crate_id`, closing
    /// stage-1 construction invariant 4 (D-12-4.3).
    pub spec_crate_stable_id: StableCrateId,
}

impl BridgeInput {
    /// Test ergonomic helper: construct a `BridgeInput`
    /// with `spec_crate_stable_id` matching the model's
    /// `crates[primary_crate].stable_crate_id`. For tests
    /// that don't care about invariant 4 specifically.
    pub fn matching(model: &RegseqModel) -> Self { /* ... */ }
}
```

The `RegseqModel::validate` signature changes from
`validate(&self) -> Result<...>` to
`validate(&self, bridge_input: &BridgeInput) ->
Result<...>` per D-12-4.3.

#### 3.5.2 Instance-side pull-through helpers

Every Instance has `Instance.item: ItemId`; Item-level
metadata is reachable via simple pull-through. Per stage-12
D-12-1.8, RegseqModel exposes convenience helpers:

```rust
impl RegseqModel {
    // Identity / source-level traceback
    pub fn instance_def_path(&self, id: InstanceId) -> &DefPath {
        &self.items[self.instances[id].item].def_path
    }
    pub fn instance_span(&self, id: InstanceId) -> Span {
        self.items[self.instances[id].item].span
    }

    // Source-level metadata fields per D-12-1.3
    pub fn instance_linkage(&self, id: InstanceId) -> Linkage { /* ... */ }
    pub fn instance_link_section(&self, id: InstanceId) -> Option<StringId> { /* ... */ }
    pub fn instance_inline_hint(&self, id: InstanceId) -> InlineHint { /* ... */ }
    pub fn instance_is_unsafe(&self, id: InstanceId) -> bool { /* ... */ }
    pub fn instance_is_naked(&self, id: InstanceId) -> bool { /* ... */ }

    // Lang-item flags
    pub fn instance_is_phantom_data(&self, id: InstanceId) -> bool { /* ... */ }
    pub fn instance_is_unsafe_cell(&self, id: InstanceId) -> bool { /* ... */ }
    pub fn instance_is_manually_drop(&self, id: InstanceId) -> bool { /* ... */ }

    // Bridge-computed outcome
    pub fn instance_opaque_handle_eligible(&self, id: InstanceId) -> bool { /* ... */ }

    // Post-Option-B InstanceKind::Function pull-throughs
    pub fn instance_fn_sig(&self, id: InstanceId) -> Option<&FnSig> { /* ... */ }
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body> { /* ... */ }

    // Domain-attribute pull-through per stage-14 D-14-1.6.
    // Walks `domain_attrs.item_attrs[item]` for the requested
    // family variant. Stage 14 introduces the intrinsic helper;
    // phase-2 stages append per-family helpers as their
    // `AttrData` variants land per §3.6 D-14-3.6 target-table
    // mapping.
    pub fn instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind> {
        let item_id = self.instances[id].item;
        self.domain_attrs.item_attrs[item_id]
            .iter()
            .find_map(|attr| match attr {
                AttrData::Intrinsic(k) => Some(k),
            })
    }
}
```

14-helper v1 baseline inventory (13 from stage-12 D-12-1.8
+ 1 domain-attribute pull-through from stage-14 D-14-1.6).
Composes with §23.6.2 `callee_fn_sig` helper (existing
convention). The `instance_<field>()` naming prefixes the
input domain; field names mirror the underlying Item field
names (no synonyms). Phase-2 stages append per-family
domain-attribute pull-through helpers (e.g.,
`field_mmio_attr` at stage 16, `instance_interrupt` at
stage 17, ...) as their `AttrData` variants land, per the
§3.6 D-14-3.6 target-table mapping.

Methods-on-`RegseqModel` shape chosen over methods-on-`Instance`
(which would clutter call sites with `&model` parameter) and
wrapper-type `InstanceRef<'m>` (which would introduce a new
type). The wrapper-type design can be added as a thin layer
later if call-site ergonomics demand; methods-on-model is the
foundation.

### 3.6 Domain attribute tables

Sibling tables on `RegseqModel` for structured DSL property
attributes derived from the `#[rseq::*]` tool namespace.
Architecture locked at stage 14 (D-14-3); the intrinsic
table is the first concrete occupant (D-14-1.6). Phase-2
stages append `AttrData` variants and commit per-family
target-table mappings as their domain extensions land.

#### 3.6.1 `DomainAttrTables`

```rust
pub struct DomainAttrTables {
    pub item_attrs:  IndexVec<ItemId, Vec<AttrData>>,
    pub field_attrs: HashMap<FieldPath, Vec<AttrData>>,
    pub span_attrs:  HashMap<Span, Vec<AttrData>>,
}

pub struct FieldPath {
    pub item:    ItemId,      // parent ADT Item (Struct / Enum / Union)
    pub variant: VariantIdx,  // 0 for Struct / Union; per-variant for Enum
    pub field:   FieldIdx,    // field index within the variant
}
```

Three sibling tables, one per source-level entity key kind:

- **`item_attrs`** -- attributes attached to source-level
  Items (functions, statics, consts, structs, enums,
  unions). Indexed by `ItemId`; sparse via empty `Vec`.
  Stage 14's `AttrData::Intrinsic` lives here.
- **`field_attrs`** -- attributes attached to fields of
  ADT Items. Indexed by `FieldPath` (the
  `(ItemId, VariantIdx, FieldIdx)` triple). Sparse via
  absence in the `HashMap`. First v1 occupant arrives at
  stage 16 (`AttrData::Mmio` for `#[rseq::mmio(...)]` on
  register-bank fields).
- **`span_attrs`** -- attributes attached to source-code
  Spans, for future use cases (diagnostic annotations,
  lint-style markers, etc.). Indexed by `Span`; sparse via
  absence. No v1 occupants; reserved per the stage-4
  §1287-1299 forward-pointer's `Span` key-kind commitment.

**No `instance_attrs` table.** Instance access to Item-level
domain attributes goes through the §3.5.2 pull-through
pattern (`self.items[self.instances[id].item]` + the
per-family `instance_<family>` helper). Per-Instance state
that genuinely depends on monomorphization context lives
inline in `InstanceKind` (precedent: `virtual_idx` on
`InstanceKind::Function` per §9.4), not in a sibling table.
If a future need surfaces for per-Instance bridge-computed
domain attributes, "Spec is mutable" applies; v1 commits to
three tables. See `discussions.md` D-14-3.1 for the
rationale.

#### 3.6.2 `AttrData`

```rust
pub enum AttrData {
    Intrinsic(IntrinsicKind),        // discriminant 0; stage 14
    // phase-2 stages append at discriminant 1, 2, 3, ...:
    //   Mmio(MmioAttr)              -- stage 16
    //   Interrupt(InterruptAttr)    -- stage 17
    //   Atomic(AtomicAttr)          -- stage 18
}
```

`AttrData` is a **closed, append-only** discriminated union.
The variant ordering is normative: stage 14 fixes
`Intrinsic` at discriminant 0; phase-2 stages append new
variants at the end. Existing discriminants remain immutable
so serialized RegseqModels stay stable across stage closures
-- the same append-only discipline applied to
`regseq_instance_kind_rank` (stage-1 R5.1 / R5.2) and to
the primitive-scalar discriminants (§10).

A single sibling-table entry (`item_attrs[id]`,
`field_attrs[path]`, or `span_attrs[sp]`) is a
`Vec<AttrData>`: multiple domain attributes may attach to
the same key (e.g., an Item that is both an intrinsic and
has some future domain marker). Per-family duplicate
invariants are validator-enforced; see §3.6.5.

`Vec<AttrData>` ordering is bridge-walk-order deterministic
(D-14-3.3). The order is established in Phase 2 (recognition
into the per-family working caches per §3.6.4) and
materialised verbatim into the final sibling tables in
Phase 4: for each key (`ItemId`, `FieldPath`, `Span`) the
Phase-4 materialisation pass walks the per-family caches in
their deterministic Phase-2 encounter order and pushes
`AttrData` entries onto the destination `Vec` in that same
order. The combined ordering inherits its determinism from
upstream stage-12 D-12-5's Phase-2 walk-order commitment.

#### 3.6.3 `IntrinsicKind`

```rust
pub enum IntrinsicKind {
    Log(LogLevel),                  // discriminant 0; stage 14
    // phase-2 stages append at discriminant 1, 2, 3, ...:
    //   Mmio(MmioOp)                -- stage 16
    //   Interrupt(InterruptOp)      -- stage 17
    //   Atomic(AtomicOp)            -- stage 18
}

pub enum LogLevel { Debug, Info, Warn, Error }
```

Closed, append-only enum following the **category +
sub-discriminant** pattern: one variant per domain family;
each variant carries a per-family sub-enum of operations.
Each `IntrinsicKind` variant corresponds 1:1 to a fixed
attribute-NAME string. Stage 14 ships the diagnostic-logging
family only; phase-2 stages own their respective sub-enums
(`MmioOp`, `InterruptOp`, `AtomicOp`, ...) and append at
their merger close.

The closed-vocabulary lookup function maps the
attribute-NAME `Symbol` to the resolved `IntrinsicKind`:

```rust
impl IntrinsicKind {
    pub fn from_name(name: Symbol) -> Option<Self> {
        match name.as_str() {
            "log-debug" => Some(IntrinsicKind::Log(LogLevel::Debug)),
            "log-info"  => Some(IntrinsicKind::Log(LogLevel::Info)),
            "log-warn"  => Some(IntrinsicKind::Log(LogLevel::Warn)),
            "log-error" => Some(IntrinsicKind::Log(LogLevel::Error)),
            // phase-2 stages append per-family arms here.
            _ => None,
        }
    }
}
```

Intrinsic NAMEs use **dash form** (`log-debug` /
`log-info` / `log-warn` / `log-error`); the Rust-side
function items that carry the `#[rseq::intrinsic("...")]`
attribute use **underscore form** (`log_debug` /
`log_info` / `log_warn` / `log_error`). The form
divergence is intentional: NAMEs are taxonomic
identifiers that may include domain-family hyphenation
(`mmio-read32`, `atomic-load-acquire`, ...); Rust
function names follow Rust's identifier convention.
See §26.2 for the locked `LogLevel` + NAME table and
the full vocabulary contract.

#### 3.6.4 Recognition and population

The bridge populates `domain_attrs` across **two phases**
of stage-12 D-12-5's canonical construction pipeline.
Recognition (walking rustc attributes) happens in Phase 2;
final-table materialization waits for Phase 4 because
`DomainAttrTables.item_attrs` is keyed by **final**
`ItemId`s, which are assigned only after the Phase-3 F4
sort. This mirrors the established pattern §3.4 documents
for `vtables` and the other side-tables.

**Phase 2 -- Recognition into a working cache.**

During the Phase-2 Item-translation walk, the bridge runs
the per-family attribute-recognition helper against each
discovered entity's source-level handle (`DefId` for
Items; `(parent_def_path_hash, VariantIdx, FieldIdx)` for
fields; the raw `Span` for span-keyed attributes). Results
are stored in per-family Phase-2 working caches keyed by
stable Phase-2-available handles -- never by final
`ItemId`. The exact handle shape per family is bridge-
implementation choice per D-12-5.3.

**Operational-intrinsic recognition** (the stage-14
codepath; D-14-1.1 + D-14-1.3 + D-14-1.4):

```rust
fn recognize_intrinsic_attr(tcx: TyCtxt<'tcx>, did: DefId)
    -> Result<Option<IntrinsicKind>, BridgeError>
{
    let attrs: Vec<&Attribute> = tcx
        .get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])
        .collect();

    match attrs.len() {
        0 => Ok(None),
        1 => {
            let list = attrs[0].meta_item_list()
                .ok_or(BridgeError::IntrinsicAttrNotDelimited)?;
            if list.len() != 1 {
                return Err(BridgeError::IntrinsicAttrWrongArgCount);
            }
            let lit = list[0].lit()
                .ok_or(BridgeError::IntrinsicAttrNotALiteral)?;
            let name = match lit.kind {
                LitKind::Str(sym, _) => sym,
                _ => return Err(BridgeError::IntrinsicAttrNonStringLiteral),
            };
            IntrinsicKind::from_name(name)
                .map(Some)
                .ok_or(BridgeError::IntrinsicAttrUnknownName(name))
        }
        _ => Err(BridgeError::IntrinsicAttrMultiple),
    }
}
```

Recognized intrinsics enter a Phase-2 working cache keyed
by `DefPathHash` (rustc's stable per-Item identity carrier;
available at Phase 2 via `tcx.def_path_hash(did)`):

```rust
// Phase-2 working cache, lives in the bridge construction
// state, not in `RegseqModel`:
intrinsic_cache: HashMap<DefPathHash, IntrinsicKind>
```

The cache is idempotent: re-encountering the same
`DefPathHash` during translation (which would happen for
generic instantiations of the same source Item) yields the
previously-cached `IntrinsicKind`.

**Phase 4 -- Materialization into `domain_attrs`.**

Phase 4 assigns final `ItemId`s in F4-sorted order
(§3.4 D-12-5.3) and, in the same pass, populates
`RegseqModel`'s side-tables: `item_def_paths[id]`,
`item_synthetic_kinds[id]`, and -- per stage-14 D-14-1.7
-- `domain_attrs.item_attrs[id]`. For each Item being
assigned its final `ItemId`:

```rust
// Inside Phase 4's per-Item population pass, after
// item_def_paths[id] has been written:
let dph = model.item_def_paths[id];
if let Some(&kind) = intrinsic_cache.get(&dph) {
    model.domain_attrs.item_attrs[id].push(AttrData::Intrinsic(kind));
}
```

Phase 4 also sizes the sibling-table backbone:
- `domain_attrs.item_attrs` is initialised to
  `IndexVec::from_elem(Vec::new(), model.items.len())`
  before the per-Item pass begins. Items with no
  domain-attribute occupants keep the empty `Vec`.
- `domain_attrs.field_attrs` and `domain_attrs.span_attrs`
  start empty; per-family Phase-2 caches keyed by
  `(parent_def_path_hash, VariantIdx, FieldIdx)` /
  `Span` get re-keyed (where needed) and inserted in
  Phase 4 alongside `item_attrs`.

`field_attrs` keys require an `ItemId` (the `FieldPath`
struct embeds the parent ADT's `ItemId`), so the re-key
step looks up the parent's final `ItemId` via the
`DefPathHash -> ItemId` mapping that Phase 4 produces.
`span_attrs` keys are independent of ItemId and can be
inserted verbatim from the Phase-2 cache.

Phase-2 stages introduce analogous per-family
`recognize_<family>_attr` helpers walking different tail
segments (`&[sym::rseq, sym::mmio]`, `&[sym::rseq,
sym::interrupt]`, etc.) and producing the appropriate
per-family payload type. The recognition primitive
(`get_attrs_by_path` + the `meta_item_list -> .lit() ->
LitKind::Str` extraction chain) is shared; the per-family
parsing of the inner payload differs. Each family's
Phase-2 cache is materialized into the appropriate
sibling table at Phase 4 per the §3.6.7 target-table
mapping.

**Notes on the recognition codepath.**

- `tcx.get_attrs_by_path` is rustc's canonical
  multi-segment tool-namespace attribute walker
  (`rustc_middle/src/ty/mod.rs:1728-1739`). Tool attributes
  always land in `Attribute::Unparsed`
  (`rustc_hir/src/hir.rs:1265-1267`), the arm
  `path_matches` (`rustc_hir/src/hir.rs:1351-1356`)
  compares against.
- `tcx.intrinsic_raw` is NOT the right hook: it gates on
  `tcx.has_attr(def_id, sym::rustc_intrinsic)`
  (`rustc_middle/src/ty/util.rs:1685-1701`) and matches
  rustc-internal compiler intrinsics only -- never
  tool-namespace attributes.
- The bridge does NOT verify
  `tcx.registered_tools(())` contains `sym::rseq` before
  walking attributes. rustc's name-resolution machinery
  enforces the `#![register_tool(rseq)]` registration
  (`rustc_resolve/src/ident.rs:691-694`); any tool-attribute
  that survives to MIR has already passed.
- Malformed-payload validation is wholly the bridge's
  responsibility. rustc has no schema for custom
  tool-attribute payloads (the attribute is `Unparsed`
  precisely because of this). The single check "exactly
  one inner item, and it's `Lit` with `LitKind::Str`"
  covers all four malformed shapes uniformly: empty args
  (`#[rseq::intrinsic]`), eq-style (`#[rseq::intrinsic =
  "NAME"]`), non-string literal (`#[rseq::intrinsic(42)]`),
  multi-arg (`#[rseq::intrinsic("NAME", extra)]`).

#### 3.6.5 Validation invariants

`RegseqModel::validate` (Phase B; see §3.3 step 10) walks
`domain_attrs` and enforces:

- **Top-level table / key well-formedness.**
  `domain_attrs.item_attrs.len() == model.items.len()`
  (the `IndexVec` backbone covers exactly the assigned
  ItemId range, established by Phase 4 per §3.4). Every
  `FieldPath` key in `field_attrs` resolves: its embedded
  `ItemId` points to an ADT Item (`kind` is
  `Struct` / `Enum` / `Union`); its `VariantIdx` is
  in-bounds for that ADT's variant set; its `FieldIdx` is
  in-bounds for the indicated variant's
  `VariantLayout.fields`. Every `Span` key in `span_attrs`
  is a structurally valid `Span` (per §5.3) and inherits
  the existing sentinel-aware **Source-file span bounds**
  rule enforced in Phase B (§3.3): for
  `SourceFile { content: Some(_), .. }`, the key satisfies
  `lo <= hi <= content.len()` with `lo` and `hi` at valid
  UTF-8 byte boundaries; for
  `SourceFile { content: None, .. }` (the synthesised
  sentinel or upstream-crate sources), the key is accepted
  as-is and would resolve to `None` via `Span::slice` per
  §5.4 if consumed.

- **Nested ID well-formedness.** Every nested ID inside
  an `AttrData` payload (`ItemId`, `FnSigId`, `StringId`,
  embedded `FieldPath` components, etc., depending on the
  per-family payload type) resolves against the relevant
  model table.

- **Per-family duplicate invariants** (`I-DomainAttr-*`).
  Stage 14 commits:

  > **I-DomainAttr-Intrinsic-Singleton.** For any `ItemId
  > id`, the set
  > `domain_attrs.item_attrs[id].iter()
  > .filter(|attr| matches!(attr, AttrData::Intrinsic(_)))`
  > has cardinality at most 1. An Item is either an
  > intrinsic or it isn't; multiple `AttrData::Intrinsic`
  > entries on the same Item are a bridge bug.

  Phase-2 stages specify analogous per-family invariants
  for their `AttrData` variants at their merger close
  (e.g., stage 16 may commit
  `I-DomainAttr-Mmio-Singleton` per `FieldPath`).

- **`Vec<AttrData>` ordering.** The validator does NOT
  re-derive or re-sort `Vec<AttrData>` entries. Ordering
  is bridge-walk-order deterministic per D-14-3.3,
  inherited from upstream stage-12 D-12-5 Phase 2 walk-
  order determinism.

#### 3.6.6 Attribute-namespace storage rule

Binding rule for where attribute-derived metadata lands in
the model. **This rule is normative for stage 14 and all
phase-2 stages**: per-family attribute design decisions
follow the namespace, not the family's semantics.

| Source attribute namespace                                                                                | Storage                                              |
| --------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `#[rseq::*]` tool-namespace custom attributes                                                             | `DomainAttrTables` sibling-table `AttrData` entry    |
| Recognized Rust attributes (`#[inline]`, `#[link_section]`, `#[no_mangle]`, ...) and language built-ins (lang-items) | Typed Item field per D-12-1.3                        |

Consequences:

1. **For phase-2 stage authors.** When introducing a new
   domain family, the per-family attribute payload
   (`MmioAttr`, `InterruptAttr`, etc.) is always a
   sibling-table `AttrData` variant -- never a typed Item
   field. The choice is settled by the namespace, not by
   the family's semantics.

2. **For future Rust-attribute admissions.** If v1 later
   admits, e.g., `#[cold]` or `#[target_feature(...)]`, the
   metadata lands as a new typed Item field per D-12-1.3
   -- never in `DomainAttrTables`. Retroactive consistency
   check: the existing typed-Item-field surface
   (`linkage`, `link_section`, `inline_hint`, `is_unsafe`,
   plus lang-item flags `is_phantom_data` /
   `is_unsafe_cell` / `is_manually_drop`) all derive from
   Rust attributes or language built-ins -- none from
   `#[rseq::*]`.

3. **External-review Finding #2 (`#[naked]` rejection in
   v1; D-14-7 territory) is consistent with this rule.**
   `#[naked]` is a Rust attribute (not `#[rseq::*]`); its
   typed-Item-field representation was on the
   typed-fields side; the rejection removes the field
   from D-12-1.3 without migrating it to
   `DomainAttrTables`.

#### 3.6.7 Per-family target-table mapping

Each phase-2 stage's spec section MUST name which sibling
table (`item_attrs`, `field_attrs`, `span_attrs`) its
`AttrData` variant populates. The mapping is part of the
per-family attribute spec; consumers query the appropriate
sibling table by knowing the family's mapping.

**Stage 14 commits the first mapping:**

| AttrData variant          | Target sibling table | Source attribute                                       |
| ------------------------- | -------------------- | ------------------------------------------------------ |
| `Intrinsic(IntrinsicKind)` | `item_attrs`         | `#[rseq::intrinsic("NAME")]` on a function Item        |

**Anticipated phase-2 entries** (not binding; phase-2
stages own the actual commitments at their merger close):

| AttrData variant          | Target sibling table | Source attribute                                          |
| ------------------------- | -------------------- | --------------------------------------------------------- |
| `Mmio(MmioAttr)` (stage 16) | `field_attrs`      | `#[rseq::mmio(...)]` on register-bank fields              |
| `Interrupt(InterruptAttr)` (stage 17) | `item_attrs` | `#[rseq::interrupt(...)]` on handler function Items   |
| `Atomic(AtomicAttr)` (stage 18) | (TBD by stage 18) | `#[rseq::*]` markup on atomic-resource fields / items  |

#### 3.6.8 Cross-references

- §3.5.2 -- Instance-side pull-through helpers; stage 14
  introduces the 14th helper `instance_intrinsic(...)`.
  Phase-2 stages append per-family pull-through helpers.
- §3.3 step 10 -- Phase B validation walk for
  `domain_attrs`.
- §8.0 (Item schema) -- the `#[rseq::intrinsic(...)]`
  recognition note in §8.0 references this section as the
  storage home; the §8.0 paragraph was corrected from the
  original "`tcx.intrinsic_raw(did)` + not stored on Item"
  forward-pointer text per external-review Finding #1.
- `discussions.md` D-14-1 + D-14-3 -- the audit-shape
  rationale for the architecture, with all alternatives
  considered and rejected (per-family typed tables,
  `Option<AttrData>` slots, instance_attrs key kind,
  IntrinsicInfo wrapper struct, etc.).
- `prior-art/rustc.md` Q1-Q4 -- rustc-source evidence
  for the recognition codepath, intrinsic_raw verification,
  register_tool handling, and attribute-payload parsing
  chain.

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

`Item` is the source-level item alphabet, **one Item per
rustc DefId** (1:1 with source-level definitions). For
generic items (e.g., `fn foo<T>(...)`), the Item carries
the source-level traceback (DefPath, kind, attributes,
span); per-monomorphisation data (substituted FnSig,
substituted Body, substituted layout) lives on `Instance`
per Q2 (monomorphisation is baked at the bridge). The
Items table is therefore the **pre-mono source-level
alphabet**; the Instances table is the **per-mono emission
unit**.

### 8.0 Schema

```rust
pub struct Item {
    pub def_path: DefPath,
    pub kind:     ItemKind,
    pub span:     Span,

    // Cross-variant Item-level metadata (D-12-1.3).
    //
    // `linkage` classifies the symbol-emission shape (default
    // mangled, no-mangle override, foreign declaration, weak
    // linkage). Bridge populates from the per-DefId mapping
    // in §3.4 D-12-2.3 (rustc's is_foreign_item / NO_MANGLE /
    // export_name / WeakAny|WeakODR / default external).
    pub linkage:        Linkage,

    // ELF section placement (e.g., ".init_array",
    // ".driver_data"). Common in kernel drivers for
    // auto-discovery sections. Bridge populates from rustc's
    // `CodegenFnAttrs.link_section`.
    pub link_section:   Option<StringId>,

    // Function-relevant flags (Item-level with sensible
    // defaults for non-fn variants per D-12-1.3 -- a `static`
    // is never naked or inline; defaults are well-defined).
    //
    // `inline_hint` corresponds to `#[inline]` /
    // `#[inline(always)]` / `#[inline(never)]` per rustc's
    // `CodegenFnAttrs.inline: InlineAttr`. C-codegen uses
    // these for `static inline` / `__attribute__((always_inline))`
    // / `__attribute__((noinline))` decoration.
    pub inline_hint:    InlineHint,

    // `is_unsafe` corresponds to `unsafe fn`. Consumed by
    // analyses (e.g., "all unsafe fn calls must be inside
    // controlled boundaries"); not directly load-bearing for
    // C codegen (C has no `unsafe` keyword).
    pub is_unsafe:      bool,

    // `is_naked` corresponds to `#[naked]` per rustc's
    // `CodegenFnAttrFlags::NAKED`. C-codegen emits naked
    // functions without prologue/epilogue (raw asm body
    // only); load-bearing for ISR entry stubs in kernel
    // drivers.
    pub is_naked:       bool,

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
    // `Ptr`. False otherwise (most Items). When `true`, the
    // Item is admissible for the opaque-handle lowering
    // convention (emission form per backend); when `false`,
    // it requires a complete representation in codegen.
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
    Static { mutability: Mutability },
    Const,
    Struct,
    Enum,
    Union,
}

/// Symbol-emission classifier (D-12-1.3 + D-12-2.3).
/// Closed 4-variant set covering the v1 driver-fixture
/// surface; other rustc Linkage variants (LinkOnceAny,
/// LinkOnceODR, Appending, Common, Internal, Private,
/// External-when-distinct-from-default) are LLVM-specific
/// and rejected at the bridge with
/// `BridgeError::UnsupportedLinkage` per §17.4
/// closed-vocabulary rule. Phase-2 device pressure can
/// reopen.
#[repr(u8)]
pub enum Linkage {
    /// Ordinary defined fn/static; mangled name.
    Default,
    /// `#[no_mangle]` or `#[export_name]`; symbol_name overrides.
    NoMangle,
    /// `extern "C" { ... }` declaration; never defined locally.
    Foreign,
    /// `#[linkage = "weak"]`; tentative definition.
    Weak,
}

/// Inline hint (D-12-1.3). 4-variant closed mirror of
/// rustc's `InlineAttr`. Defaults to `None` for non-Function
/// ItemKind variants (where the concept doesn't apply).
#[repr(u8)]
pub enum InlineHint {
    /// No `#[inline]` annotation.
    None,
    /// `#[inline]`.
    Hint,
    /// `#[inline(always)]`.
    Always,
    /// `#[inline(never)]`.
    Never,
}
```

**Per-`ItemKind` data placement: source-level alphabet on
Item; per-monomorphisation rich data on Instance.** Items
are pre-mono (1:1 with rustc DefIds); Instances are
per-mono (one per (Item, GenericArgs, rustc shim flavor)
tuple). Per-`InstanceKind` rich data per §9.4:
- `InstanceKind::Function { fn_sig: FnSigId, body:
  Option<Body>, flavor: FunctionInstanceFlavor, virtual_idx:
  Option<u32> }` carries the post-mono substituted FnSig
  + post-mono substituted Body + post-mono dispatch
  metadata. Per stage-12 D-12-1.4 (Option B, restoring
  symmetry with non-Function variants); supersedes
  stage-11 D-11-3 + D-11-5's Item-side framing. See
  `discussions.md` D-12-1.4 for the contradiction
  Option B resolves.
- `InstanceKind::Static { ty: TyId, init: Option<ConstId> }`
  carries the post-mono ty and initializer; mutability
  lives on `ItemKind::Static { mutability }` per
  D-12-1.2 (c) (source-level keyword choice; statics
  can't be generic so 1:1 Item:Instance always).
- `InstanceKind::Const { ty: TyId, value: ConstId }`
  carries the post-mono ty and const value.
- `InstanceKind::{Struct, Enum, Union} { layout: TyId }`
  carries the per-mono `Ty::Struct/Enum/Union` reference.

**No general attribute table on `Item` (stage-3 scope);
typed Item fields per stage-12; structured DSL property
attributes per stage-14.** The 8 Item-level fields above
(`linkage`, `link_section`, `inline_hint`, `is_unsafe`,
`is_naked` + 3 lang-item flags + `opaque_handle_eligible`)
are the typed Item-level metadata stage 12 commits to per
stage-1 R5.2. Declarative DSL property attributes
(`#[rseq::mmio(access_width = 32)]`,
`#[rseq::interrupt(trigger_type = "...")]`, etc.) are
**stage-14 territory (now locked at §3.6)**: stage 14
introduces a sibling-table architecture on `RegseqModel`
keyed by `ItemId` / `FieldPath` / `Span` (three sibling
tables in `DomainAttrTables`; per D-14-3.1,
`instance_attrs` was dropped from stage-4 §1287-1299's
original 4-key-kind enumeration because Instance access
to Item-level metadata goes through the §3.5.2
pull-through pattern locked at stage-12 D-12-1.8). See
§3.6 for the full architecture (the `DomainAttrTables`
struct, `AttrData` discriminated union, recognition +
materialisation protocol, validation invariants,
attribute-namespace storage rule per D-14-3.5, and the
per-family target-table mapping per D-14-3.6). Stage 12
added NO schema for DomainAttr; the architecture was a
forward expectation that stage 14 has now resolved. The
previous merged-spec wording "No general attribute table
on `Item`" was a stage-3-scoped commentary that erased
the stage-3-vs-stage-14 split when absorbed; D-12-1.5
restores the framing.

The `#[rseq::intrinsic("NAME")]` operations attribute (Q7)
is recognized per stage-14 D-14-1: the bridge walks
callee attributes via
`tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`
(rustc's tool-namespace machinery; `tcx.intrinsic_raw` is
rustc-internal-only and does NOT match tool attributes).
The resolved intrinsic identity lives on `RegseqModel` as
an `AttrData::Intrinsic(IntrinsicKind)` entry in
`domain_attrs.item_attrs[item_id]` per the unified
domain-attribute storage architecture (§3.6 -- the
`AttrData` discriminated union, `IntrinsicKind`
closed-vocabulary enum, recognition pseudocode, population
rules, validation invariants, attribute-namespace storage
rule, per-family target-table mapping). Per Q2's
eager-translation rule, `'tcx` does not survive past the
bridge call; downstream codegen reads the resolved
identity directly from `model.domain_attrs.item_attrs`
(or via the §3.5.2 pull-through helper
`instance_intrinsic`).

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
  metadata-only at the Item layer: DefPath, kind, span,
  source-level attributes per §8 (linkage, inline_hint,
  lang-item flags, etc.) — captured by the bridge from
  rustc-side metadata, since `core` lang-items lack a
  source-level body in the spec crate. Per stage-12 D-12-1.4
  (Option B; supersedes stage-11 D-11-5), any body lives on
  the Function `Instance`'s
  `InstanceKind::Function { body: Option<Body>, ... }`, NOT
  on the Item. For drop-glue and similar `core` lang-items,
  the Function Instance's body is **populated**
  (`body: Some(b)`) at Phase 5 body construction from rustc's
  `tcx.instance_mir(InstanceKind::DropGlue(...))` (or
  analogous shim-body fetcher). `body: None` is reserved
  exclusively for foreign declarations
  (`tcx.is_foreign_item(did) == true`) per §3.1 invariant 10
  and D-12-2.1; the metadata-only Item case is orthogonal
  to the Instance-side body=None case (the metadata-only
  framing describes the Item's source-level metadata
  layer, not the per-Instance body slot). No separate
  `bodies` table.
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
(`FunctionInstanceFlavor`, §9.4), giving deserialised
consumers a structured way to reconstruct stable instance
identity for shims sharing `(item, args)` without parsing
`emission_name`. The mapping
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
contract that the `FnAbi` shape enforces (see `discussions.md`
D15 for rationale). Verification is a debug-build
assertion at instance interning. If a future toolchain bump
introduces a non-injective shim variant (i.e., two distinct
rustc `InstanceKind` carrying additional discriminating payload
beyond what `fn_shim_disambig` captures), the rank table
extends with a payload-key encoding rather than silently
admitting the collision.

### 9.4 `InstanceKind`

```rust
pub enum InstanceKind {
    /// Function instantiation. Per stage-12 D-12-1.4 (Option B,
    /// restoring symmetry with non-Function variants):
    /// post-mono substituted `fn_sig` and `body` live HERE on
    /// `InstanceKind::Function`, NOT on the parent Item.
    /// Supersedes stage-11 D-11-3 + D-11-5's Item-side framing.
    /// See `discussions.md` D-12-1.4 for the contradiction
    /// (Items are pre-mono per §16.1 F4 sort + §13.4 source
    /// DefPathHash + §9.3 RegseqInstanceKey; stage-3 §15.1
    /// rejects Param/Bound TyKind variants -- so a stored
    /// FnSig must be already-substituted post-mono; the two
    /// views were incompatible) Option B resolves.
    ///
    /// `fn_sig` is the substituted post-mono signature for
    /// this monomorphization; populated even for foreign
    /// declarations (rustc's `tcx.fn_sig(did)` is callable for
    /// foreign fns even though MIR is unavailable). Two
    /// monomorphizations of a generic fn `fn foo<T>(x: T) -> T`
    /// produce two Instances with distinct `fn_sig` values
    /// (e.g., `fn(u32) -> u32` and `fn(u64) -> u64`).
    ///
    /// `body` is the substituted post-mono MIR body, monomorphized
    /// per the Instance's `generic_args`. `None` for foreign
    /// declarations (`extern "C" { fn foo(); }` where
    /// `tcx.is_foreign_item(did)` returns true and
    /// `tcx.is_mir_available(did)` returns false). `Some(b)` for
    /// source-defined fns and bridge-synthesized shims (drop
    /// glue, vtable shims, closure call methods).
    ///
    /// `flavor` records which rustc `InstanceKind` variant the
    /// bridge translated from (§9.3 `fn_shim_disambig`); stored
    /// on the model so deserialised consumers can reconstruct
    /// stable instance identity for shims sharing `(item, args)`
    /// without parsing `emission_name`. `virtual_idx` stores
    /// rustc's vtable-slot index for Virtual-flavor instances
    /// (per `ty::InstanceKind::Virtual(_, idx)`); `Some(idx)`
    /// exactly when `flavor == Virtual`, `None` otherwise (§9.4
    /// construction invariant). Consumed by the stage-11
    /// vtable-indirect rewrite pass (per §23.6.2 B-8 + the
    /// stage-11 outbound expectation "From stage 9").
    Function {
        fn_sig:      FnSigId,
        body:        Option<Body>,
        flavor:      FunctionInstanceFlavor,
        virtual_idx: Option<u32>,
    },

    /// Static. `init` is `None` for foreign statics
    /// (`extern "C" { static FOO: u32; }`) and `Some(c)` for
    /// source-defined statics with an initial value. Mutability
    /// lives on `ItemKind::Static { mutability }` per stage-12
    /// D-12-1.2 (c) (source-level keyword choice; statics can't
    /// be generic so 1:1 Item:Instance always). Supersedes
    /// stage-9's denormalized placement of mutability on
    /// InstanceKind. See `discussions.md` D-12-1.2 for
    /// rationale.
    Static { ty: TyId, init: Option<ConstId> },

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

Each variant carries the post-mono per-monomorphisation rich
data: `Function` carries `fn_sig` + `body` (per Option B
D-12-1.4) plus dispatch metadata (`flavor`, `virtual_idx`);
`Static` / `Const` carry `ty` + value; `Struct` / `Enum` /
`Union` carry the per-mono `Ty::Struct/Enum/Union`
reference. **No separate `bodies` table** -- bodies live
structurally inside `InstanceKind::Function` per Option B.

**Per-`InstanceKind::Function` accessor.** D-12-1.6 hybrid
pattern: convenience method `as_function() ->
Option<&FunctionInstanceData>` extracts the struct-variant
into a named `FunctionInstanceData` struct for ergonomic
destructuring + accessor return types.

```rust
impl InstanceKind {
    pub fn as_function(&self) -> Option<&FunctionInstanceData> { /* ... */ }
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionInstanceData> { /* ... */ }
}

pub struct FunctionInstanceData {
    pub fn_sig:      FnSigId,
    pub body:        Option<Body>,
    pub flavor:      FunctionInstanceFlavor,
    pub virtual_idx: Option<u32>,
}
```

Match arms remain available for exhaustive handling; the
accessor is for the common single-variant query (e.g., the
§23.6.2 `callee_fn_sig` helper uses this pattern).

**Construction invariant: virtual_idx ↔ flavor=Virtual.** For
`InstanceKind::Function { flavor, virtual_idx, .. }` instances,
`virtual_idx.is_some()` iff `flavor ==
FunctionInstanceFlavor::Virtual`. Equivalently:
- `flavor == Virtual` requires `virtual_idx == Some(idx)` for
  some `idx: u32` (the vtable slot index recovered from rustc's
  `ty::InstanceKind::Virtual(_, idx)`).
- All non-Virtual flavors require `virtual_idx == None`.

`RegseqModel::validate` walks every Function Instance and
checks this biconditional. Encountering a violation (e.g.,
`Some(_)` with `flavor != Virtual`, or `None` with `flavor ==
Virtual`) is a model-construction bug, not a driver-source
error. The invariant is **projected at Direct virtual call
sites** at §23.6.2 B-8 for stage-11 walk-pass enforcement.

`FunctionInstanceFlavor` is the structured form of the
`fn_shim_disambig: u8` rank used inside `RegseqInstanceKey`
(§9.3). Storing it on `InstanceKind::Function` (rather than
deriving it from `emission_name`) avoids the brittleness of
parsing structured discriminators back out of name strings —
the same brittleness the structured `SyntheticItemKind` shape
in §13.3 avoids for synthetic Items. The rank-table mapping
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

### 9.7 `ConstInterner` and the `Const` vocabulary

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

`ConstId` is `u32`-backed via `NonMaxU32` so that
`size_of::<Option<ConstId>>() == 4`. Variant choices below
preserve this invariant by routing all variant-internal
references to other consts through nested `ConstId` (no
`Box<Const>` payloads).

#### Identity contract

Construction invariant 11 (§3.1) requires `ConstId`
content-uniqueness. The `ConstIdentityKey` is computed from
the value's canonical content (the variant tag + per-variant
canonical payload form below), never from any allocation-order
rustc handle (`AllocId`, `DefId`, `mir::AllocId`) or any local
RegseqModel handle (`InstanceId`, `ItemId`). Cross-build
identity relies exclusively on the closed canonical encodings
defined here.

Unevaluated or symbolic consts that survive monomorphisation
are a bridge bug per the no-defensive-fallbacks methodology —
the bridge rejects them at construction, never widens to
"Other" / "Opaque" variants.

The canonical-form rules below close the F4-2 fingerprint
(`ArgsKey` traversal in §12.3 reaches const arguments via
`GenericArg::Const(ConstId)` and queries `canonical_const(ConstId)`
per the per-`Const`-variant byte-encoding tags subsection
below).

#### `Const` enum (closed, 11 variants)

```rust
pub enum Const {
    /// Boolean scalar.
    Bool(bool),

    /// Unicode scalar value as 32-bit code point. Bridge has
    /// already validated the value is a valid `char` at
    /// construction; storage is `u32` for deterministic
    /// canonicalisation independent of Rust's `char` validity
    /// invariant in serialisation paths.
    Char(u32),

    /// Signed integer scalar. Smaller widths sign-extend to
    /// `u128` (e.g. `Int(I8, -1)` stores as `u128::MAX`).
    Int(IntKind, u128),

    /// Unsigned integer scalar. Smaller widths zero-extend.
    Uint(UintKind, u128),

    /// IEEE-754 bit-pattern. The bit-pattern occupies the
    /// low-order bits of `u128`; the high-order bits are zero
    /// for narrow widths (e.g., `1.0f32` stores as
    /// `0x3f800000` per the IEEE-754 binary32 encoding, which
    /// is `Float(F32, 0x3f800000_u128)` with the upper 96 bits
    /// of the `u128` field zero). Per-`FloatKind` natural-width
    /// storage; no lossy `f64` collapse.
    Float(FloatKind, u128),

    /// Zero-sized value. Single canonical representative for
    /// all ZSTs (`()`, fieldless structs, `PhantomData<T>`,
    /// etc.). Type comes from the consumer's context (the
    /// owning Item's `ty` or the InstanceKind's payload).
    ZST,

    /// Byte-level value: a sequence of bytes mixing initialised
    /// payload, uninit bytes, and pointer-bearing relocations.
    /// Used for vtable instance values (§13.2),
    /// `ConstMemoryStatic` synthesised statics (§13.3), and
    /// any aggregate whose rustc-evaluated form is a flat
    /// allocation with `provenance` entries or an `init_mask`
    /// other than all-init. `&str` / `&[T]` reference-typed
    /// const handling backs the slice's data pointer with a
    /// `ConstMemoryStatic` synthesised Static — see
    /// "Reference-typed const handling" below.
    Bytes(Vec<Byte>),

    /// Structured aggregate: struct / tuple / array literals
    /// where rustc's const-eval produced a `ValTree::Branch`
    /// shape. Field-aligned children, each interned for
    /// sharing. Does NOT take program-image storage on its own
    /// (contrast with vtable-instance Static synthesis,
    /// §13.2); per-target use-site rendering is
    /// backend-codegen content.
    Aggregate(Vec<ConstId>),

    /// Enum value: explicit variant index plus payload children.
    /// The discriminant value is implicit via the consumer's
    /// `Ty::Enum`'s `VariantLayout.discriminant` (§11.1);
    /// `EnumValue` does NOT carry the discriminant byte form
    /// — that lives on the type, per the layout/value
    /// separation pinned in §11.1.
    EnumValue {
        variant_idx: VariantIdx,
        payload: Vec<ConstId>,
    },

    /// Typed null pointer. Per-pointee-type for canonical
    /// equality: `NullPtr(t1) != NullPtr(t2)` whenever
    /// `t1 != t2`. Vacant vtable slots and source-level
    /// `core::ptr::null::<T>()` / `core::ptr::null_mut::<T>()`
    /// canonicalise here. (Reference-typed consts pointing at
    /// non-null anonymous-memory blobs are handled by the
    /// `ConstMemoryStatic` synthesised-Static path below, not
    /// as `NullPtr`.)
    NullPtr(TyId),

    /// Whole-pointer relocation: this const is exactly a
    /// pointer to that Instance (function / static / vtable
    /// pointer). The `Bytes::Reloc` byte-level case applies
    /// when the relocation is one field of an aggregate
    /// already encoded as bytes.
    Reloc(InstanceRef),
}
```

The eleven variants are exhaustive: a rustc post-mono const
value must canonicalise into exactly one variant (the bridge
construction rule below names the dispatch). There is no
"escape" variant; non-conforming inputs are bridge-rejected.

#### `Byte` enum (three-state, used by `Const::Bytes`)

```rust
pub enum Byte {
    /// Uninitialised byte. Canonicalises distinct from any
    /// initialised byte value, including `Value(0)`. Sourced
    /// from rustc's `Allocation::init_mask` ranges absent from
    /// the materialised mask.
    Uninit,

    /// Initialised payload byte with a specific value.
    Value(u8),

    /// Pointer-bearing byte: this byte is part of a stored
    /// pointer to the named Instance. A run of consecutive
    /// `Reloc(r)` bytes whose length equals the target's
    /// pointer width and whose start offset is pointer-aligned
    /// represents one whole pointer to `r`; no other
    /// pattern is admissible.
    Reloc(InstanceRef),
}
```

**Byte-sequence well-formedness invariants** (validated in
§3.3 Phase B):

- *Maximal relocation runs are pointer-shaped.* Define a
  *relocation run* as a maximal contiguous subsequence of
  `Byte::Reloc(r)` values sharing the same `InstanceRef r`
  (a run cannot extend into a neighbouring `Byte::Reloc(r')`
  with `r != r'`, nor into `Byte::Uninit` / `Byte::Value(_)`).
  Every relocation run has length exactly `ptr_size` and starts
  at an offset aligned to `ptr_align`. (`ptr_size`, `ptr_align`
  come from `target.scalar_layouts.fn_ptr`.) No relocation run
  may straddle a `ptr_align` boundary, and two adjacent runs
  cannot share the same `InstanceRef` (i.e., any apparent
  "double pointer" of the same target is actually one run; if
  two distinct same-target pointers appear in a sequence they
  occupy non-adjacent slots separated by at least one
  non-Reloc byte or by a same-target run break). This single
  rule replaces a per-byte test: it suffices to enumerate runs
  and check the length and start-alignment of each.
- The sequence length matches the consumer-context's expected
  byte size (the owning Item's `Layout.size`, or the
  vtable-instance `Layout.size`, etc.). Length-mismatch is a
  bridge bug.

#### `InstanceRef` (cross-build stable identity)

```rust
pub struct InstanceRef {
    /// Regseq `InstanceKind` variant rank, per §9.3's
    /// `regseq_instance_kind_rank` table:
    /// `Function`=0, `Static`=1, `Const`=2, `Struct`=3,
    /// `Enum`=4, `Union`=5. Stable per F4-2.
    pub regseq_kind_rank: u8,

    /// rustc's stable cross-build hash of the source-side
    /// DefPath (or the bridge-synthesised DefPath for synthetic
    /// Items per §13.4). Build-stable; carries cross-build
    /// instance identity per §7.3.
    pub def_path_hash: DefPathHash,

    /// Canonical encoding of the Instance's generic arguments
    /// per the §12.3 deep canonicaliser. `ArgsKey` is the same
    /// type used by F4-2 instance ordering; the same canonical
    /// form is reused here to avoid duplication. For
    /// non-generic statics and for synthetic Items whose
    /// generic-args identity is fully captured in
    /// `def_path_hash` (per §13.4 segment encoding),
    /// `ArgsKey::EMPTY` is the canonical value.
    pub args_key: ArgsKey,

    /// rustc-shim flavor disambiguator, mirroring §9.3's
    /// `fn_shim_disambig: u8`. For `Function` Regseq kind, this
    /// is `rustc_instance_kind_sort_rank(flavor)` per §9.3's
    /// table (`Item`=0, `Intrinsic`=1, `VTableShim`=2,
    /// `ReifyShim`=3, `FnPtrShim`=4, `Virtual`=5,
    /// `ClosureOnceShim`=6, ..., `DropGlue`=10, ...). For
    /// non-`Function` Regseq kinds, unconditionally `0`. For
    /// bridge-synthesised Function Items (vtable method shims,
    /// vtable drop shims, closure call methods, closure drop
    /// glue) where there is no rustc `InstanceKind` origin,
    /// also `0` — the synthesised `def_path_hash` (per §13.4)
    /// already disambiguates these.
    pub fn_shim_disambig: u8,
}
```

The 4-tuple matches the same shape used by §9.3's
`RegseqInstanceKey` (which is the in-build instance interning
key); `InstanceRef` is the build-stable projection of that key
where the local `ItemId` is replaced with `def_path_hash` (and
optionally where `ArgsKey` collapses to `ArgsKey::EMPTY` for
synthetic Items whose identity §13.4 already encodes in the
DefPath segments). Each component is build-stable:
`regseq_kind_rank` is the §9.3-fixed Regseq InstanceKind rank;
`def_path_hash` is rustc's stable cross-build hash (or the
deterministic bridge-synthesised hash per §13.4); `args_key`
is the §12.3 deep canonicaliser; `fn_shim_disambig` is the
§9.3-fixed rustc shim rank. No local `InstanceId` or `ItemId`
ever appears in any `InstanceRef`, neither in
`Const::Reloc(_)` nor in `Byte::Reloc(_)`. Two builds
satisfying the build-equivalence quad (§12.4) produce identical
`InstanceRef` for relocations to the same Instance.

#### Type-level vs value-level partition

The `Const` variants partition into two phase-membership classes
based on which RegseqModel positions admit them:

| Variant       | Type-level admissible (`GenericArg::Const`) | Value-level admissible (`InstanceKind`, `Bytes::Reloc`, etc.) |
| ------------- | ------------------------------------------- | ------------------------------------------------------------- |
| `Bool`        | yes                                         | yes                                                           |
| `Char`        | yes                                         | yes                                                           |
| `Int(_, _)`   | yes                                         | yes                                                           |
| `Uint(_, _)`  | yes                                         | yes                                                           |
| `Float(_, _)` | yes                                         | yes                                                           |
| `ZST`         | no (v1 bridge-rejected at GenericArg slot)  | yes                                                           |
| `Bytes`       | no (would force AllocId churn)              | yes                                                           |
| `Aggregate`   | no                                          | yes                                                           |
| `EnumValue`   | no                                          | yes                                                           |
| `NullPtr`     | no (requires resolved `TyId`)               | yes                                                           |
| `Reloc`       | no (requires resolved `InstanceRef`)        | yes                                                           |

The five scalar variants (`Bool` / `Char` / `Int` / `Uint` /
`Float`) form the **type-level subset**: they canonicalise
without resolving any `TyId` or `InstanceRef`, so they are
admissible from `GenericArg::Const(_)` positions and validate
in §3.3 Phase B step 4 before `TyInterner` (step 6).

The remaining six variants form the **value-level remainder**:
they reach the IR only through `InstanceKind::Static.init`,
`InstanceKind::Const.value`, `Const::Bytes::Reloc`, and
`Const::Aggregate` / `Const::EnumValue` payload children. Their
canonical form references resolved `TyId` / `InstanceRef`, so
they validate after `TyInterner` and the `instances` table.

A `Const::Aggregate` whose children are all type-level scalars
is itself value-level — the type-level boundary is structural,
not transitive. The bridge admits aggregates only at value-level
positions.

#### Per-`Const`-variant byte-encoding tags (`ConstIdentityKey` byte form)

The `ConstIdentityKey` byte form is a tagged-tree byte stream
parallel to §12.3's per-`Ty` encoding. The same global rules
apply: each node emits a 1-byte variant tag followed by
tag-specific payload bytes; all multi-byte integers are
little-endian; lengths emit as little-endian `u32` prefixes;
nested nodes recurse via `canonical_const(ConstId)` (defined
below). The encoding is consumed by
`canonical_args.recurse(ConstId)` (the `ConstId` walk-rule row
in §12.3) and by `ConstInterner.lookup`'s key computation.

Per-`Const`-variant byte-encoding tags — append-only constants
pinned for the v1 const universe:

```text
const_variant_tag = u8
  0  = Bool                followed by u8 (0x00 | 0x01)
  1  = Char                followed by u32 LE Unicode scalar value
  2  = Int(IntKind, u128)  followed by u8 IntKind discriminant + u128 LE value
                           (small widths sign-extended into u128)
  3  = Uint(UintKind, u128) followed by u8 UintKind discriminant + u128 LE value
                           (small widths zero-extended)
  4  = Float(FloatKind, u128) followed by u8 FloatKind discriminant + u128 LE
                           bit-pattern (IEEE-754 bit-pattern occupies low-
                           order bits; high-order bits zero for narrow widths)
  5  = ZST                 (no payload — type comes from consumer context)
  6  = Bytes(Vec<Byte>)    followed by u32 LE byte_count + byte_count * Byte
                           encoding (see byte_sub_tag table below)
  7  = Aggregate(Vec<ConstId>) followed by u32 LE child_count + child_count *
                           canonical_const(ConstId)
  8  = EnumValue           followed by u32 LE variant_idx + u32 LE
                           payload_count + payload_count * canonical_const(ConstId)
  9  = NullPtr(TyId)       followed by canonical_ty(TyId) (recursive per
                           §12.3 walk rules)
 10  = Reloc(InstanceRef)  followed by InstanceRef bytes (see InstanceRef
                           encoding below)
```

`IntKind` / `UintKind` / `FloatKind` discriminants reuse the
`#[repr(u8)]` numeric assignments pinned in the §10 enum
definitions (the normative source for the `u8` discriminant
values: `IntKind { I8=0, I16=1, I32=2, I64=3, I128=4, Isize=5 }`,
`UintKind { U8=0, ..., Usize=5 }`, `FloatKind { F16=0, F32=1,
F64=2, F128=3 }`). §12.3's per-`Ty` table reuses the same
values; this per-`Const` table reuses them again — the single
source of truth is §10.

The encoding is **append-only**: any new `Const` variant
admitted by a future spec revision appends at the next free
tag value (`11+`); existing tags remain immutable so cross-build
hashes for already-admitted consts never change. No tag values
are reserved for future use. Reference-typed const handling did
NOT introduce a new `Const` variant — it composes from the
existing 11 variants via `Aggregate([Reloc, Uint])` for wide
pointers and `Reloc(synth_static_inst_ref)` for thin pointers
(per the compositional-behavior-beats-special-case-flags
methodology).

**`Byte` sub-encoding** (used inside `Bytes(Vec<Byte>)`'s payload):

```text
byte_sub_tag = u8
  0  = Uninit             (no payload)
  1  = Value(u8)          followed by 1 byte (the initialised payload byte)
  2  = Reloc(InstanceRef) followed by InstanceRef bytes (see InstanceRef
                          encoding below)
```

**`InstanceRef` byte encoding** (used by `const_variant_tag = 10`
and by `byte_sub_tag = 2`):

```text
InstanceRef encoding (34 bytes total):
  u8       regseq_kind_rank
  16 bytes def_path_hash       (StableCrateId u64 LE + local_hash u64 LE
                                per §12.3 termination rules)
  16 bytes args_key            (emitted as `ArgsKey.0.to_be_bytes()` —
                                the canonical big-endian byte serialisation
                                of the `u128` field, which by §12.3's
                                BE-load convention reproduces the original
                                SHA-256-leading-16-bytes sequence; native
                                memory layout / `transmute` MUST NOT be
                                used)
  u8       fn_shim_disambig
```

The 16-byte `args_key` field is emitted as `ArgsKey.0.to_be_bytes()`
(the standard big-endian `u128` serialisation). This pinning is
load-bearing: the serialised model stores `ArgsKey` only as a
`u128` (§12.3), so the canonical byte form must be specified by
operation on the `u128` value, not by appeal to "underlying
bytes" (which leaves a native-endian / `transmute` interpretation
open). `to_be_bytes()` is consistent with the BE-load convention
in §12.3 (`ArgsKey::EMPTY.0` is "loaded as a big-endian u128")
and reproduces the original SHA-256-leading-16-bytes sequence
that the bridge computed before storing into `ArgsKey.0`.
The general "multi-byte integers are little-endian" rule in
§12.3 governs other multi-byte fields (`StableCrateId`,
`local_hash`, `u128` value-bits in scalars, lengths); `args_key`
is the documented BE exception, anchored on the BE-load
convention already pinned in §12.3.

**`canonical_const(ConstId)` recursion.** When the encoding above
calls `canonical_const(ConstId)` (in `Aggregate` and
`EnumValue.payload`), the rule is: resolve the `ConstId` against
`model.consts.resolve(...)`, recursively encode the resolved
`Const` per the per-variant rules above, and inline the
resulting bytes into the parent's stream. Local `ConstId`
integers are **never** emitted — they are build-local indices
and would defeat cross-build stability. This is the const-side
analogue of §12.3's "`TyId` field — resolve via
`model.types.resolve(...)` and recurse on the resolved `Ty`"
rule.

The recursion is bounded by const-graph acyclicity: well-typed
Rust + const-eval termination guarantee that no `Const`
transitively contains itself. §3.3 Phase B step 9 validates
this property explicitly (three-state DFS marks --
`Unseen` / `Visiting` / `Done` -- over the `Aggregate` /
`EnumValue` recursion edges; back-edge to `Visiting` is
corruption, edge to `Done` is skipped as a legitimate shared
DAG child) so that tampered serialised models cannot cause
infinite recursion at canonical-key computation time.

**Worked examples.**

```text
Const::Bool(true)
  -> 0x00 0x01                         (tag=0, payload=true)

Const::Int(I32, -7_i32 as u128)
  -> 0x02 <IntKind::I32 disc>
        0xF9 0xFF 0xFF 0xFF  0xFF 0xFF 0xFF 0xFF
        0xFF 0xFF 0xFF 0xFF  0xFF 0xFF 0xFF 0xFF
        (tag=2, IntKind disc, sign-extended -7 as u128 LE)

Const::Float(F32, 0x3f800000_u128)     (= 1.0f32 bit-pattern)
  -> 0x04 <FloatKind::F32 disc>
        0x00 0x00 0x80 0x3F  0x00 0x00 0x00 0x00
        0x00 0x00 0x00 0x00  0x00 0x00 0x00 0x00
        (tag=4, FloatKind disc, IEEE bits in low 4 bytes, high 12 bytes zero)

Const::ZST
  -> 0x05                              (tag=5, no payload)

Const::Aggregate(vec![c1, c2])         (e.g., a 2-tuple)
  -> 0x07 0x02 0x00 0x00 0x00          (tag=7, child_count=2 LE)
     <canonical_const(c1)>
     <canonical_const(c2)>

Const::EnumValue { variant_idx: 1, payload: vec![c1] }
  -> 0x08 0x01 0x00 0x00 0x00          (tag=8, variant_idx=1 LE)
        0x01 0x00 0x00 0x00            (payload_count=1 LE)
        <canonical_const(c1)>

Const::NullPtr(t_i32)                  (where t_i32 is the TyId of *const i32)
  -> 0x09 <canonical_ty(t_i32)>        (tag=9, recursive Ty encoding)

Const::Reloc(InstanceRef { regseq_kind_rank: 0, def_path_hash: H,
                            args_key: A, fn_shim_disambig: 0 })
  -> 0x0A 0x00                         (tag=10, regseq_kind_rank=0)
        <16 bytes H>
        <16 bytes A>
        0x00                           (fn_shim_disambig=0)
```

#### Bridge construction (rustc → `Const` dispatch)

Given a rustc `mir::ConstValue` post-monomorphisation (per the
post-mono guarantee surfaced at codegen — see also §15
translation rules), the bridge dispatches:

| Rustc shape (`mir::ConstValue` arm)                        | RegseqModel `Const` variant                                                                                      |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `Scalar(Scalar::Int(scalar_int))` over scalar `Ty::Bool`   | `Bool(b)`                                                                                                        |
| `Scalar(Scalar::Int(_))` over `Ty::Char`                   | `Char(value as u32)`                                                                                             |
| `Scalar(Scalar::Int(_))` over `Ty::Int(k)`                 | `Int(k, sign_extend_to_u128(value))`                                                                             |
| `Scalar(Scalar::Int(_))` over `Ty::Uint(k)`                | `Uint(k, value as u128)`                                                                                         |
| `Scalar(Scalar::Int(_))` over `Ty::Float(k)`               | `Float(k, bits as u128)` (bit-pattern in low-order bits; high-order bits zero for narrow widths)                 |
| `Scalar(Scalar::Int(_))` over a pointer type, all-zero     | `NullPtr(pointee_ty)` (pointee derived from the consumer's `Ty`)                                                 |
| `Scalar(Scalar::Ptr(ptr, _))`                              | `Reloc(InstanceRef::from_alloc_id(ptr.alloc_id))`                                                                |
| `ZeroSized`                                                | `ZST`                                                                                                            |
| `Indirect { alloc, .. }` with structural valtree available | walk valtree → `Aggregate(_)` or `EnumValue { .. }`                                                              |
| `Indirect { alloc_id, offset = 0 }` over `GlobalAlloc::Memory(_)`, sized pointee Ty | `Reloc(synth_static_inst_ref(alloc_id))` (thin pointer to synthesised `ConstMemoryStatic` Item per "Reference-typed const handling" subsection below)                                          |
| `Indirect { alloc_id, offset != 0 }` over `GlobalAlloc::Memory(_)` | bridge-rejected (no-defensive-fallbacks rule; see "Reference-typed const handling" subsection below)                          |
| `Indirect { alloc, .. }` otherwise                         | `Bytes(Vec<Byte>)` from `alloc.bytes`, `alloc.provenance`, `alloc.init_mask`                                     |
| `Slice { data, meta }`                                     | `Aggregate([Reloc(synth_static_inst_ref(data)), Uint(Usize, meta)])` (wide pointer per "Reference-typed const handling" subsection below)                                                       |

The bridge constructs `InstanceRef` from a rustc `AllocId` by
resolving via `tcx.global_alloc(alloc_id)` to a `GlobalAlloc`,
then projecting per §9.3's `regseq_instance_kind_rank` and
`rustc_instance_kind_sort_rank` tables:

| `GlobalAlloc` variant            | `InstanceRef` projection                                                                                                                                                                                                                                                                            |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Function { instance }`          | `regseq_kind_rank: 0` (`Function`); `def_path_hash: def_path_hash(<source DefId underlying instance>)` (extracted from `instance.def_id()` per rustc's `Instance` API — the source-side DefId, distinct from the rustc-shim flavor); `args_key: argskey_of(instance.args)` (or `ArgsKey::EMPTY` if `instance.args.is_empty()`); `fn_shim_disambig`: the `u8` rank looked up in §9.3's `rustc_instance_kind_sort_rank` table for the `FunctionInstanceFlavor` corresponding to `instance.def` (§9.4's `FunctionInstanceFlavor` is the structured Regseq-side wrapper for each admitted rustc `InstanceKind` variant; the §9.3 rank table assigns the same `u8` to a flavor and to the rustc `InstanceKind` variant it wraps). The shim rank distinguishes shims sharing `(DefId, args)` — e.g., `Item(foo)` (rank `0`) vs `ReifyShim(foo)` (rank `3`) vs `DropGlue(foo)` (rank `10`). |
| `Static(def_id)`                 | `regseq_kind_rank: 1` (`Static`), `def_path_hash: def_path_hash(def_id)`, `args_key: ArgsKey::EMPTY` (statics are non-generic), `fn_shim_disambig: 0`.                                                                                                                                  |
| `VTable(ty, preds)`              | `regseq_kind_rank: 1` (`Static` — vtable instances are bridge-synthesised Static Items per §13.2, marked in `item_synthetic_kinds` as `SyntheticItemKind::VTableInstance { principal, self_ty }`); `def_path_hash: synthesised_def_path_hash(per §13.4)`; `args_key: ArgsKey::EMPTY` (vtable instance Items are non-generic — `principal` and `self_ty` are baked into the synthesised DefPath segments per §13.4); `fn_shim_disambig: 0`. |
| `Memory(memory)`                 | `regseq_kind_rank: 1` (`Static` — `ConstMemoryStatic` Items are bridge-synthesised Static Items per the "Reference-typed const handling" subsection below, marked in `item_synthetic_kinds` as `SyntheticItemKind::ConstMemoryStatic { content_hash }`); `def_path_hash: synthesised_def_path_hash(per §13.4 const-memory-family)`; `args_key: ArgsKey::EMPTY` (the synthesised Static is non-generic — `content_hash` is baked into the synthesised DefPath segments per §13.4); `fn_shim_disambig: 0`.                                              |
| `TypeId { ty }`                  | bridge-rejected (`feature(type_id)` const generic; out of v1 driver-DSL scope per the no-defensive-fallbacks methodology rule).                                                                                                                                                                     |

The bridge does **not** route value-level aggregates through
`valtree_to_const_val`: that path materialises fresh `AllocId`s
for non-primitive aggregates inside rustc, which would propagate
build-local handle churn into the canonical const key. The
bridge walks `ValTree::Branch` directly to `Aggregate` /
`EnumValue` for structured aggregates, and walks
`mir::ConstValue::Indirect` for byte-level values.

#### Reference-typed const handling

Reference-typed const values whose storage is backed by an
anonymous rustc `GlobalAlloc::Memory` allocation — primarily
`&str` and `&[T]` literals — are handled by **bridge-synthesising
an immutable `Static` Item per unique memory-content blob**, with
a content-derived `DefPathHash` that is cross-build stable. The
const value is then composed compositionally from the existing
`Const` variants — no new `Const` variant is required.

**Bridge synthesis rule (lazy).** When the bridge dispatches a
`mir::ConstValue::Slice { data, meta }` or
`mir::ConstValue::Indirect { alloc_id, offset = 0 }` arm
targeting `GlobalAlloc::Memory(memory)`:

1. Look up the underlying `Allocation` (`memory.bytes`,
   `memory.init_mask`, `memory.provenance`).
2. Compute the canonical `Const::Bytes(Vec<Byte>)` byte-encoding
   of the allocation per §9.7's per-`Const`-variant byte-encoding
   tags (the three-state `Byte` form captures all three sources
   of allocation distinctness uniformly).
3. Compute `content_hash: ContentHash` by hashing the canonical
   bytes per §13.3's `ContentHash` definition (SHA-256-truncated
   leading 16 bytes BE-loaded into `u128`).
4. **Phase 2** (translation + synthesis interleaving per §3.4):
   look up `content_hash` in the bridge's Phase-2 working cache
   `ContentHash -> Phase2Handle` (the same cache shape §3.4
   pins for synthesis triggers; stable Phase-2-available handle
   such as DefPathHash or placeholder ItemId-shaped handle, NOT
   final ItemId). If present (a prior synth Item with the same
   content), reuse the Phase2Handle. If absent, register a new
   synthetic Item shape:
   - `item.def_path` = synthesised per §13.4 const-memory-family
     segments (`["<regseq_synth>", "const_memory_static",
     <content_hash hex>]`); the synthesised `DefPathHash` per
     §13.4 with payload bytes `content_hash.0.to_be_bytes()`.
     This DefPathHash is the natural Phase-2 stable key (final
     ItemId is not yet assigned).
   - `item.kind = ItemKind::Static { mutability: Mutability::Not }`
     (mutability lives on ItemKind per stage-12 D-12-1.2 (c)).
   - The `SyntheticItemKind::ConstMemoryStatic { content_hash }`
     tag is recorded for later Phase-4 population of
     `item_synthetic_kinds`.

5. **Phase 4** (post-F4-sort ItemId assignment + side-table
   population per §3.4): the Phase-2-registered synthetic Item
   gets its final `ItemId(N)` after sorting. In parallel:
   - `item_def_paths[ItemId(N)]` populated with the synthesised
     `DefPathHash` from step 4.
   - `item_synthetic_kinds[ItemId(N)]` populated with
     `SyntheticItemKind::ConstMemoryStatic { content_hash }`.

6. **Phase 5** (Instance construction per §3.4): the
   bridge synthesises a corresponding `Instance` with:
     `kind = InstanceKind::Static { ty: array_ty, init: Some(c) }`
   (2-field Instance-side schema per Option B; mutability lives
   on the parent Item),
   where `array_ty` is the `TyId` for `Ty::Array(elem_ty, N)`
   with `elem_ty` the `TyId` for `Ty::Uint(UintKind::U8)`
   and `N = memory.bytes.len() as u64` (both resolved against
   the `TyInterner`; created if absent), and `c` is the
   `ConstId` for the `Const::Bytes(Vec<Byte>)` value computed
   in step 2.
     The Static is **type-erased to bytes**: two allocations
     with identical bytes share the synth Item regardless of
     their original Rust element type. The wrapping
     `Aggregate([Reloc, Uint(Usize, len)])` (for `Slice`)
     carries the typed-pointer side; the Static itself is
     bytes. `init: None` is invalid for `ConstMemoryStatic`
     Items (validated in §3.3 Phase B per the
     `ConstMemoryStatic` immutable-statics rule). Per the §8
     `Item` schema, per-kind rich data (including the
     Static's `ty`) lives on the `Instance`, not the `Item`;
     `Item` itself carries only `def_path`, `kind`, `span`,
     flags, and `opaque_handle_eligible`.
7. Construct the dispatch result (post-Phase-5; the
   resulting `Const::Aggregate` / `Const::Reloc` value
   references the now-final synthesised `Instance` via
   `synth_static_inst_ref(alloc_id)`):

```text
mir::ConstValue::Slice { data, meta }                 ->  Aggregate([
                                                            Reloc(synth_static_inst_ref(data)),
                                                            Uint(Usize, meta),
                                                          ])
mir::ConstValue::Indirect { alloc_id, offset = 0 }    ->  Reloc(synth_static_inst_ref(alloc_id))
  over GlobalAlloc::Memory, sized pointee Ty
```

`synth_static_inst_ref(alloc_id)` denotes the `InstanceRef`
4-tuple for the synthesised Static: `regseq_kind_rank: 1`
(`Static`); `def_path_hash`: the `DefPathHash` synthesised
per §13.4 with `content_hash.0.to_be_bytes()` payload;
`args_key: ArgsKey::EMPTY`; `fn_shim_disambig: 0`.

**Pointee type at the use site.** For both Slice and
Indirect-thin dispatch, the synthesised Static is type-erased
to a byte-array `Ty::Array(elem_ty, N)` whose `elem_ty`
resolves to `Ty::Uint(UintKind::U8)`. The original Rust pointee
type (e.g., `[u32]`, `[u32; 3]`, `MyStruct`) is **supplied by
the consumer's context** — the `Ty` of the `Const`'s position
in the IR (e.g., the `Ty` of the `InstanceKind::Static.init`
slot's pointer field, or of the `InstanceKind::Const.value`).
C codegen casts from the byte static to the pointee type at
the use site (e.g., emits `(const struct Foo *)mem_<hash>`
rather than synthesising a re-typed copy of the static). This
mirrors how `&str` lowers to a wide pointer with a
`(const char *)mem_<hash>` data field — the bytes are shared;
the type lives on the use.

**Reference-typed const examples.**

```text
&str literal "hello":   (memory blob = b"hello", N = 5)
  Const::Aggregate([
    Reloc(InstanceRef {
      regseq_kind_rank: 1,                   // Static
      def_path_hash: <synth DefPathHash for content_hash(b"hello")>,
      args_key: ArgsKey::EMPTY,
      fn_shim_disambig: 0,
    }),
    Uint(Usize, 5),
  ])

&[u32; 3] literal &[1, 2, 3]:   (memory blob = bytes-of-three-u32s, thin pointer)
  Const::Reloc(InstanceRef {
    regseq_kind_rank: 1,
    def_path_hash: <synth DefPathHash for content_hash(memory_bytes)>,
    args_key: ArgsKey::EMPTY,
    fn_shim_disambig: 0,
  })

&[u32] slice &[1, 2, 3]:        (memory blob = bytes-of-three-u32s, wide pointer)
  Const::Aggregate([
    Reloc(<same synth Static as the &[u32; 3] case if bytes match>),
    Uint(Usize, 3),
  ])
```

The third example illustrates the type-erasure dedup: a `&[u32; 3]`
literal and a `&[u32]` slice over identical bytes share the
synthesised Static — only the wrapping `Const` differs (thin
pointer vs wide pointer aggregate).

**Out-of-scope cases.**

- **Non-zero offsets into Memory** (`Indirect { alloc_id, offset != 0 }`
  over `GlobalAlloc::Memory`): bridge-rejected per the
  no-defensive-fallbacks rule. These would arise from
  taking a reference into the middle of a larger anonymous
  allocation (e.g., `&FOO.field` where `FOO` is itself
  anonymous-memory-backed). Driver code in practice does not
  produce these; named statics carry shared structures where
  field-pointers might be needed. The current spec is closed
  at `offset = 0`; non-zero offsets remain rejected unless a
  future spec revision admits them (no slot is reserved in
  the dispatch table or `InstanceRef` shape).
- **`feature(type_id)`** (`GlobalAlloc::TypeId { ty }`):
  bridge-rejected.

These rejections are **rejection-enforced**, not silent fallback:
the bridge raises a diagnostic naming the rustc shape that was
encountered. No future-extension slot is reserved; if the case
becomes load-bearing, a future spec revision can add an
admitting rule append-only.

**Cross-build stability.** Two builds satisfying the
build-equivalence quad (§12.4) produce identical
`Allocation.bytes` / `init_mask` / `provenance` for the same
source-level `&str` / `&[T]` literal, hence identical canonical
byte form, hence identical `ContentHash`, hence identical
synthesised `DefPathHash`, hence identical `InstanceRef` values
in the `Reloc` field of the resulting `Const` value. The
synthesised Static's `ItemId` is build-local; cross-build
identity flows through `def_path_hash`, the same discipline
applied to vtable instance Items per §13.2.

#### Const validation phase split

`ConstInterner` validation is split across two §3.3 Phase B
steps:

- **Type-level const phase** at step 4 (before `TyInterner` at
  step 6): validate `ConstInterner.lookup` injectivity over the
  type-level subset of canonical-content keys; recompute
  canonical keys for type-level scalar entries (`Bool` / `Char`
  / `Int` / `Uint` / `Float`) and verify they match stored
  keys; validate that every `ConstId` appearing in
  `GenericArg::Const(_)` positions resolves to a type-level
  variant. Type-level scalar key computation is bounded
  (no recursion through `ConstId`), so this is safe before
  acyclicity is established.

- **Value-level const phase** at step 9 (after `instances` at
  step 8; before Phase C transient-map rebuild at step 10):
  validate value-level `Const` content sub-references per the
  invariants enumerated in §3.3 step 9 (`Byte` sequence
  well-formedness; `NullPtr` pointee resolves; `Reloc` /
  `Bytes::Reloc` `InstanceRef` projects to a unique Instance;
  `Aggregate` children resolve, with type-level scalar
  children admissible inside value-level aggregates per the
  non-transitive-boundary rule; `EnumValue.variant_idx` is
  in-bounds for the consumer's `Ty::Enum`). The step-9 sub-
  bullets run in this order: (1) `NullPtr` / `Reloc` /
  `Bytes::Reloc` sub-reference resolution against `types` /
  `instances`; (2) `Bytes` / `Aggregate` / `EnumValue` content
  checks (sequence well-formedness, child resolution,
  variant_idx in-bounds); (3) const-graph acyclicity via
  three-state DFS; (4) value-level canonical-content key
  recomputation and lookup-map injectivity over the value-level
  subset; (5) `ConstMemoryStatic` dedup (per-Item Static-shape
  check + recompute `ContentHash` from each synth Static's
  `init` and verify against the stored `content_hash`; verify
  no two `ConstMemoryStatic` Items share a `content_hash`).
  Acyclicity must run before step (4) so recursive
  `canonical_const(ConstId)` calls into `Aggregate.0` /
  `EnumValue.payload` terminate; steps (1) and (2) are
  recursion-free (they check resolution, not key recomputation),
  so they can run before acyclicity. Step (5) depends on the
  value-level recomputation in step (4) (it re-canonicalises
  each synth Static's `init: Const::Bytes`), so it runs after.
  Combined with the step-4 type-level pass, step (4) completes
  injectivity over the full `ConstInterner.lookup` key set.

The split is a contract on the validation order and the
membership rule — not on the interner shape. A single
`ConstInterner` table with the two-pass discipline above and a
variant-discriminant-keyed phase classifier satisfies the
contract; the v1 implementation uses one table.

---

## 10. The `Ty` enum

The `Ty` enum is **closed** at 19 variants. Every variant is a
*rigid* post-monomorphisation type per `TyKind::is_known_rigid()`.
Domain extensions extend `InstanceKind` / `Item` flags / tags on
existing nodes — never `Ty` itself (Q3 single-level IR rule).

| Variant                                                      | Carries                                           | Layout?          | Identity bucket                                      |
| ------------------------------------------------------------ | ------------------------------------------------- | ---------------- | ---------------------------------------------------- |
| `Bool`                                                       | —                                                 | —                | structural                                           |
| `Char`                                                       | —                                                 | —                | structural                                           |
| `Int(IntKind)`                                               | `I8`/`I16`/`I32`/`I64`/`I128`/`Isize`             | —                | structural                                           |
| `Uint(UintKind)`                                             | `U8`/`U16`/`U32`/`U64`/`U128`/`Usize`             | —                | structural                                           |
| `Float(FloatKind)`                                           | `F16`/`F32`/`F64`/`F128`                          | —                | structural                                           |
| `Str`                                                        | marker                                            | no (unsized)     | structural                                           |
| `Never`                                                      | marker                                            | no (uninhabited) | structural                                           |
| `Array(TyId, u64)`                                           | element type + length                             | —                | structural                                           |
| `Slice(TyId)`                                                | element type                                      | no (unsized)     | structural                                           |
| `Ref(TyId, Provenance)`                                      | pointee + borrow flavor                           | —                | structural                                           |
| `Ptr(TyId, Mutability)`                                      | pointee + `Not`/`Mut`                             | —                | structural                                           |
| `FnDef(ItemId, GenericArgs)`                                 | callable item + post-mono args                    | ZST              | **nominal**                                          |
| `FnPtr(FnSigId)`                                             | sig handle (carries `FnAbi`, etc.)                | —                | structural (FnSig content expanded by canonicalizer) |
| `Dynamic { auto_traits, vtable_ty }`                         | sorted `AutoTrait` set + vtable's `TyId`          | no (unsized)     | structural                                           |
| `Closure(ItemId, GenericArgs)`                               | synthesised captures-struct Item + post-mono args | via parallel `Ty::Struct` | **nominal**                                  |
| `Tuple { fields: Vec<TyId>, layout: Layout }`                | element types + resolved layout                   | **inline**       | structural-with-layout-excluded                      |
| `Struct { item: ItemId, args: GenericArgs, layout: Layout }` | item + args + resolved layout                     | **inline**       | **nominal**                                          |
| `Enum { item: ItemId, args: GenericArgs, layout: Layout }`   | item + args + resolved layout                     | **inline**       | **nominal**                                          |
| `Union { item: ItemId, args: GenericArgs, layout: Layout }`  | item + args + resolved layout                     | **inline**       | **nominal**                                          |

Per-variant C lowerings (per-target rendering of each variant
as a C type or value) are backend-codegen content; see
`generic-c-backend.md` §2 for the generic-C target.

Variant-level invariants not already captured above or
documented elsewhere in §10 / §17:

- **`FnDef` is a ZST** carrying function identity; no value
  representation is emitted.
- **`Closure` carries identity-view semantics**; its layout is
  derived from a parallel synthetic `Ty::Struct` (the
  captures-struct Item), so the layout column reads "via
  parallel `Ty::Struct`" rather than `inline`.
- **The empty tuple `Ty::Tuple { fields: [] }` is Rust `()`**
  -- a ZST with `size: 0, align: 1` that serves as the unit /
  value-less marker (per §14.5 it is the only ZST admitted at
  `FnAbi::C` return position; backend renders per its
  unit-return convention -- see `generic-c-backend.md` §2).
- **`Str` / `Slice(_)` / `Dynamic{..}` are unsized** and never
  appear inline -- they always sit behind `Ref`/`Ptr` (fat
  pointers; see §17 for the IR-side fat-pointer-aggregate
  shape).
- **`Never` is accepted as a `Ref`/`Ptr` pointee** (codegen
  MAY reject); divergent-expression positions are elided per
  §10.2.
- **`Array(t, n)`** is rejected at `FnAbi::C` parameter/return
  positions per §14.5 (C arrays decay to pointers in those
  positions; the IR rejects the mismatch rather than silently
  decay).

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
IR carries no aliasing-history metadata (see `discussions.md` D7 / D8).

### 10.2 Unsized variants

`Ty::Str` and `Ty::Slice` are unsized DST markers; they do NOT
carry a `Layout` and `tcx.layout_of` is not called on them
directly. They appear only inside `Ty::Ref(...)` or
`Ty::Ptr(...)` pointee positions; codegen emits the fat-pointer
shape on demand.

`Ty::Never` is the uninhabited type `!`. It carries no `Layout`.
It is accepted at type level in three positions:

- **Function-return type** (`fn diverge() -> !`): accepted at
  the type level for divergent functions; backend renders the
  noreturn function per its emission convention (see
  `generic-c-backend.md` §2 for the generic-C rendering).
- **Divergent-expression type**: positions are unreachable by
  construction; codegen elides them.
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

### 10.4 Rendering

See `pretty-printer.md` §6 (per-variant `render_ty`
specifications across the closed 19-variant `Ty` enum).

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

**Storage-lvalue rule for invalid-bit-pattern niche storage.**
When `TagScalar.scalar_ty` is `Bool` or `Char`, niche encoding
may write bit patterns outside the *logical value set* (e.g.,
`Option<bool>` uses byte values `2..=255` as `None` niche;
target-level "smart" scalar types like C `_Bool` may normalise
these to `0` or `1` on assignment, which would corrupt the
niche). The bridge records this niche-storage requirement: for
tag/niche read and write paths, the storage lvalue is treated
as an **unsigned-integer cell of the scalar's storage width**
— width-equivalent to the value type but without the
value-set normalisation. Per-target C type rendering of this
storage lvalue is backend-codegen content; see
`generic-c-backend.md` §2 + §4 for the generic-C target. Not
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
equality).** No public `TyIdentityKey` type is exposed; the
spec-surface model uses a private interner key paired with
public full-value equality on `Ty`:

- The interner uses a **private** `TyIdentityKey` for its
  `lookup: HashMap<TyIdentityKey, TyId>` map. `TyIdentityKey`
  is internal to `regseq-model` (not exposed in the public
  spec surface). It encodes the §12.1 nominal/structural
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

The public spec does not expose a `TyIdentityKey` type; the
`TyInterner.lookup` mechanism is implementation detail.

```rust
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup:  HashMap<TyIdentityKey, TyId>,    // private key (not part of public API)
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

The private `TyIdentityKey` is the interner's lookup key. It
encodes the per-variant identity rule per the buckets below.
Public `Ty` has derived `PartialEq` / `Eq` / `Hash`
(full-value equality including `Layout`); identity equality is
*not* exposed on `Ty` itself.

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
| `ConstId`          | resolve via `model.consts.resolve(...)` and recurse on the resolved `Const` per §9.7's per-`Const`-variant byte-encoding tags |
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

#### `GenericArgs` container encoding

Wherever `canonical_args(GenericArgs)` appears in the per-`Ty`
encodings above (`FnDef`, `Closure`, `Struct`, `Enum`, `Union`),
the container grammar is:

```text
GenericArgs encoding:
  u32 LE arg_count
  followed by arg_count * GenericArg entry

GenericArg entry:
  u8 disc     // 0 = Type(TyId), 1 = Const(ConstId)
  followed by disc-specific payload:
    disc = 0:  canonical Ty bytes per the per-`Ty` rules above
               (i.e., recursively encode the resolved Ty content,
               not the local TyId integer)
    disc = 1:  canonical Const bytes per the per-`Const`-variant
               byte-encoding rules in §9.7 (resolved Const
               content, not the local ConstId integer).
               `GenericArg::Const(_)` positions resolve to
               type-level scalar variants only per §3.3 step 4
               and §9.7's type-level / value-level partition.
```

Per §9.1, `GenericArg` has no lifetime variant (regions are
erased pre-bridge); the closed two-tag set above is
exhaustive. The grammar is **append-only**: a future
`GenericArg` variant (e.g., the unstable `feature(adt_const_params)`-
extended forms, if v2 admits them) appends at the next free
discriminant value.

**`ArgsKey::EMPTY`.** The well-known constant value of `ArgsKey`
for an empty `GenericArgs`:

```rust
impl ArgsKey {
    /// Canonical `ArgsKey` for an empty `GenericArgs` value
    /// (no type / lifetime / const arguments). Every reference
    /// to a non-generic Item — including non-generic statics,
    /// non-generic `Const` Items, and bridge-synthesised Items
    /// whose generic-args identity is fully captured in the
    /// synthesised `DefPath` segments per §13.4 — uses
    /// `ArgsKey::EMPTY` rather than re-canonicalising at every
    /// reference site.
    pub const EMPTY: ArgsKey = ArgsKey(<canonical SHA-256-truncated hash of the
        empty-GenericArgs canonical byte form>);
}
```

The canonical byte form of an empty `GenericArgs` is a single
4-byte little-endian `u32` length prefix `0x00_00_00_00`
followed by zero entries. `ArgsKey::EMPTY.0` is therefore the
leading 16 bytes of `SHA-256(0x00 0x00 0x00 0x00)` loaded as
a big-endian `u128`. This value is fixed by the canonical
encoding and the SHA-256 function; an implementation computes
it once at compile time (or at startup) and shares the
constant across all uses.

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
  For each call-method Function Item, the corresponding
  Function `Instance`'s `InstanceKind::Function { fn_sig,
  ... }` (per stage-12 D-12-1.4 / Option B; §9.4) carries
  the substituted signature derived from the closure's
  `closure_sig_as_fn_ptr_ty`, with a per-call-kind receiver
  shape:
  - `Call`     → receiver `Ref(captures_struct, SharedBorrow)`
                 (i.e., `&Captures`).
  - `CallMut`  → receiver `Ref(captures_struct, UniqueBorrow)`
                 (i.e., `&mut Captures`).
  - `CallOnce` → receiver `captures_struct` by value.
  These are the **type-system signatures**. A C-codegen lowering
  may choose a pointer receiver for by-value `FnOnce` for
  implementation convenience, but that is a body/codegen
  decision and does not change the type-system signature
  carried by the Function Instance's `fn_sig` (resolved
  through `model.sigs[fn_sig_id]`).

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
  - **Erased receiver type is uniformly
    `Ty::Ptr(ErasedPointee, Mutability::Mut)`** in the IR,
    regardless of the source borrow flavor (where
    `ErasedPointee` is defined below). Shared `&self` methods
    on impls whose `Self` contains `UnsafeCell` legitimately
    mutate through `&self`; a shared-receiver erasure
    (`Ty::Ptr(_, Mutability::Not)`) would not be sound across
    all admissible impls. The Rust borrow flavor is preserved
    at the type-system level (the source `Ref(t, SharedBorrow)`
    is still visible in the source Ty), but the erased
    dispatch surface uniformly carries `Mutability::Mut` to be
    sound across all impls. Per-target rendering of this
    erased receiver in C is backend-codegen content; see
    §17.2.3 "Trait-object dispatch carve-out" for the IR-side
    rule and `generic-c-backend.md` §3.3 for the generic-C
    target's rendering.
  - **Canonical erased pointee.** The erased receiver and
    trait-object `data` field reference the canonical erased
    unit type: `Ty::Tuple { fields: vec![], layout:
    UNIT_LAYOUT }` (§11). This canonical IR-side anchor avoids
    reintroducing `core::ffi::c_void` (rejected per §15.1)
    while giving the bridge a stable `Ty` to point to.
    Per-target rendering of the erased pointee (e.g., the
    generic-C target's unqualified void-pointer convention)
    is backend-codegen content; see `generic-c-backend.md`
    §3.3.
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

/// Content fingerprint for an anonymous-memory `GlobalAlloc::Memory`
/// allocation backing a reference-typed const (`&str` / `&[T]`).
/// Same hash family as `ArgsKey` and `ProjectionBindingsKey`:
/// SHA-256 truncated to the leading 16 bytes loaded as a
/// big-endian `u128`. Canonical input bytes = the canonical
/// `Const::Bytes(Vec<Byte>)` byte-encoding of the memory
/// allocation per §9.7's per-`Const`-variant byte-encoding tags
/// (per-`Byte` sub-tag + payload, including any internal
/// `Byte::Reloc(InstanceRef)` for relocation-bearing memory).
/// This inherits the three-state `Byte` discipline so
/// allocations with `init_mask` ranges or `provenance` entries
/// are identity-distinguished from pure-byte allocations.
pub struct ContentHash(pub u128);              // 128-bit content hash

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

    // Const-memory family (§13.4, §9.7). A bridge-synthesised
    // immutable `Static` Item per unique anonymous-memory blob
    // backing a reference-typed const (`&str` / `&[T]` literals).
    // Identity is content-keyed: two allocations with identical
    // canonical byte form produce identical `content_hash`,
    // hence identical synthesised `DefPathHash`, hence one
    // de-duplicated synth Item per content blob. The synthesised
    // Item carries `ItemKind::Static { mutability:
    // Mutability::Not }` (mutability lives on ItemKind per
    // stage-12 D-12-1.2 (c)); its `Instance` has
    // `kind = InstanceKind::Static { ty: Ty::Array(elem_ty, N), init }`
    // (Option-B 2-field Instance schema), where `elem_ty` is
    // the `TyId` for `Ty::Uint(UintKind::U8)`, N is the
    // allocation's byte length, and `init` resolves to a
    // `Const::Bytes(Vec<Byte>)` carrying the three-state byte
    // encoding.
    ConstMemoryStatic { content_hash: ContentHash },
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
                    if ConstEq: ConstIdentityKey bytes per §9.7
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

**`ContentHash` canonical encoding.** Same hash construction
as `ArgsKey` and `ProjectionBindingsKey` (SHA-256 truncated
to leading 16 bytes, big-endian, loaded as `u128`), but
different canonical input grammar — anonymous-memory blob
content:

```text
ContentHash canonical byte input:
    canonical `Const::Bytes(Vec<Byte>)` byte-encoding of the
    memory allocation per §9.7's per-`Const`-variant byte-encoding tags
    (variant tag(6) + u32 LE byte_count + byte_count * Byte-sub-tag-encoded
    payload, where each Byte is encoded per the `byte_sub_tag` table:
    Uninit=0 (no payload), Value(b)=1 + 1 byte, Reloc(r)=2 + 34-byte
    InstanceRef encoding).
```

Reusing the `Bytes` byte form means `ContentHash` naturally
captures all three sources of allocation distinctness:
(a) initialised payload bytes; (b) `init_mask` ranges (encoded
as `Byte::Uninit` runs); (c) `provenance` entries (encoded as
`Byte::Reloc(InstanceRef)` runs). Two allocations with
identical `(bytes, init_mask, provenance)` produce identical
`ContentHash`; any difference in any of the three produces
distinct hashes.

**Collision model (honesty note for `ContentHash`).** Same
framing as `ArgsKey` and `ProjectionBindingsKey`. The
serialised model stores only the `u128`, not the canonical
allocation bytes. At 128 bits with SHA-256 truncation,
collision probability for any realistic driver-crate volume
is below `2^-50`; treated as outside the threat model.

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
The remaining seven synthetic kinds (`ClosureCall`,
`ClosureDropGlue`, `VTableType`, `VTableInstance`,
`VTableMethodShim`, `VTableDropShim`, `ConstMemoryStatic`)
use the encoding below.

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

Const-memory family
  segments = [
    TypeNs("<regseq_synth>"),
    TypeNs("const_memory_static"),
    TypeNs(<content_hash hex>),
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
- Const-memory family: owner crate = `primary_crate`. The
  underlying `GlobalAlloc::Memory` allocation has no
  source-side `DefPath`, so there is no rustc-derived
  ownership; assigning to `primary_crate` keeps display /
  cross-references deterministic (the same `ConstMemoryStatic`
  Item across builds with identical content lands at the same
  owner crate).

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

ConstMemoryStatic
  ++ ContentHash bytes (16 B; equivalently
     `content_hash.0.to_be_bytes()` per §13.3 ContentHash byte form)
```

`DefPathHash` bytes = the 16-byte content (StableCrateId u64 LE
+ local_hash u64 LE per §7.3). `ArgsKey`,
`ProjectionBindingsKey`, and `ContentHash` byte form per §12.3
/ §13.3 (leading 16 SHA-256 digest bytes in digest order,
equivalently `<key>.0.to_be_bytes()`).

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
  functions are well-formed at the IR level; per-target
  noreturn rendering is backend-codegen content).
- **Unit return.** `output` MAY be `Ty::Tuple { fields: [] }`
  (Rust unit `()`); this is the standard no-value return
  shape at `FnAbi::C` boundaries (§14.5); per-target rendering
  per backend.

**`c_variadic` construction invariants.**

- `c_variadic == true` implies `abi == FnAbi::C` (rustc's C
  variadic restriction; `extern "Rust" fn(...)` cannot be
  variadic).
- `c_variadic == true` implies `inputs.len() >= 1` — the
  rustc-accepted prefix shape requires at least one fixed
  parameter before the `...`.

**FnSig placement in the IR.** `FnSig` instances live
**Instance-side** on `InstanceKind::Function { fn_sig:
FnSigId, body: Option<Body>, ... }` per stage-12 D-12-1.4
(Option B). Per-monomorphization function instantiations
each carry their substituted (post-mono) signature directly
on the InstanceKind variant. Bodies live structurally on
the same InstanceKind::Function variant (also per Option B).

Argument types remain reachable from a Body via the §25.1
layout convention (`Body.local_decls[1..=arg_count].ty`);
return type via `Body.local_decls[RETURN_PLACE].ty`.
`Ty::FnPtr(FnSigId)` references through the interner per
§14.4 below; identity is structural over FnSig content.

**Stage-11 D-11-3 + D-11-5 supersession.** Stage-11 placed
`fn_sig` and `body` on Item under the Function ItemKind
variant (Item-side framing), citing 4-peer convergence
(rustc-internal, rustc_public, Charon, mrustc). Stage-12
D-12-1.4 reopens both decisions (flip to `[REOPENED]` in
`decisions.md` at stage-12 merger contract close): the
4-peer evidence actually supports per-Instance body
(rustc_public's `Instance::body() -> Option<Body>` is the
cited pattern; Charon's `FunDecl` is post-mono under
`--monomorphize`). The Item-side framing was incompatible
with the rest of the spec's pre-mono Item model (§16.1
F4 sort + §13.4 source DefPathHash + §9.3
RegseqInstanceKey all imply Items are 1:1 with rustc DefIds
/ pre-mono; stage-3 §15.1 rejects Param/Bound TyKind so a
stored FnSig must be already-substituted post-mono). See
`discussions.md` D-12-1.4 for the full reasoning.

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
(intentionally, per §11.3 dropped `IS_C` /
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
stored in the IR (§11.3). Storing the final emit target
keeps codegen executable from the model alone.

**Type identity preservation.** `FnSig.inputs/output`
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

Both lowerings produce identical post-`CAbiLowering` C-ABI
surfaces (the `Handle` chain unfolds to the same scalar shape
as the direct `u32` signature), but the FnSigs remain distinct
in the model — preserving the type-identity contract for
rustc-distinct types (see `discussions.md` D15 for rationale).
The per-target C-type rendering of the lowered surface is
backend-codegen content; see `generic-c-backend.md` §2 for the
generic-C target.

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
rejected in both. Conflating the two predicates into a single
recursive rule would either reject `repr(C) struct
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
  rejected at C boundaries in v1 — top-level divergent
  `extern "C" fn() -> !` is admissible (per the §14.5 return
  rule), but nested noreturn function-pointer declarators
  have platform-specific
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
  slot (`fn() -> !` is admissible at the IR level; per-target
  noreturn rendering per backend).
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
the Item is admissible for the opaque-handle lowering
convention (per-target emission form lives in the backend
artifact); when false, the bridge has already rejected any
incompatible use site at type translation, so codegen has a
complete representation to work from.

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
| `Ty::Struct { item: this }` reached via a `Const` value's recursive walk (`Aggregate.0`, `EnumValue.payload`, `NullPtr(TyId)`, or as the resolved pointee of a `Reloc(InstanceRef)` whose target Item references the type by-value) | by-value | **yes** — `Const` values may carry by-value Ty references that disqualify the type from opaque-handle eligibility |

The traversal walks `model.types`, `model.sigs`, `model.items`
(lang-item flag fields, `def_path` attribute references), and
`model.consts` (per the `Const` row above). The bridge stores
the **outcome** as `Item.opaque_handle_eligible` after running
the traversal at construction time; consumers read the bool,
not re-run the walk.

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

1. **Lang-item flagged ADTs**: `PhantomData`, `UnsafeCell`,
   `ManuallyDrop`. Recognition via `adt_def.is_*()`. Translation
   continues as ordinary `Ty::Struct`; the corresponding flag
   on the resulting `Item` is set per §8.2.
2. **DSL-rejected ADTs**: `Box` (via
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

The bridge rejects the following at type, call, or place
translation. Three categories overall: **non-rigid `TyKind`**
(should not survive
`instantiate_mir_and_normalize_erasing_regions`; encountering
one is a bridge bug); **rigid but DSL-rejected** (types that
survive normalisation but lie outside the v1 driver-DSL scope);
and **closed `PlaceElem` vocabulary rejections** (rustc
projection variants that are absent from our `optimized_mir`
input or gated behind unstable features and have no v1 driver
use case; see §19.1).

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
| `core::sync::atomic::{fence, compiler_fence}`                             | call translation | outside the v1 admitted intrinsic vocabulary; drivers requiring explicit ordering barriers between MMIO and normal memory are out of v1 scope |
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
| `mir::PlaceElem::OpaqueCast(_)` in any `Place.projection` slot            | place translation | absent from `optimized_mir` under both solver paths (next-solver: never constructed; legacy: stripped by `PostAnalysisNormalize`); see §19.1 closed-vocabulary rule |
| `mir::PlaceElem::UnwrapUnsafeBinder(_)` in any `Place.projection` slot    | place translation | gated behind unstable `unsafe_binders` feature; no v1 driver use case; see §19.1 closed-vocabulary rule |

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

Condition 4 is what prevents the type-identity collapse the
type-identity-preservation contract forbids (see
`discussions.md` D15 for rationale): rustc treats
`for<'a> fn(&'a A, &'a B)` (relational —
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
machine level. The type-identity-preservation contract (see
`discussions.md` D15) applies to types with *distinct* C
outputs — `fn(&u32)` and `fn(&'static u32)` have identical C
output, so the collapse preserves the codegen contract.

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
extra `FnSig` field. The current spec takes the simpler path
(reject) per the no-defensive-fallbacks methodology and
lazy-vocabulary-expansion pattern.

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

Mutability rule for **sized** pointees (IR-side; backend
rendering follows):

- `Ref(t, SharedBorrow)` carries shared (immutable)
  mutability if `!contains_unsafe_cell(t)`; otherwise the
  mutability discriminant is dropped per the UnsafeCell rule
  (§17.2.1 recursive interior-mutability detection).
- `Ref(t, UniqueBorrow)` carries unique (mutable) mutability.
- `Ptr(t, Mutability::Not)` carries shared (immutable)
  mutability (raw pointers do not consult `is_unsafe_cell`;
  raw-pointer framing).
- `Ptr(t, Mutability::Mut)` carries unique (mutable)
  mutability.

Per-target C-type rendering of these mutability discriminants
(e.g., the generic-C target emitting `const T*` / `T*` /
optional `restrict` per its convention) is backend-codegen
content; see `generic-c-backend.md` §3.1.

#### 17.2.2 Wide-pointee carve-out

When the pointee `t` is **unsized** — `Ty::Str`, `Ty::Slice(_)`,
or `Ty::Dynamic{..}` — the sized-pointee rules above do **not**
apply. The `Ref`/`Ptr` value lowers as a **fat-pointer
aggregate** (two-slot struct), not as a thin pointer. The
abstract slot composition:

- **`data` slot** (slot[0]): a thin pointer to the element
  type (`u8` for `Str`, `T` for `Slice(T)`, erased for
  `Dynamic{..}`). The source mutability / `Provenance` rule
  applies to this slot per the table below, with the
  trait-object carve-out (§17.2.3 below) erasing the
  mutability discriminant for `Dynamic{..}` pointees.
- **second slot** (slot[1]): `len` (target-width unsigned
  scalar) for `Str` and `Slice(_)`; `vtable` (immutable
  vtable handle) for `Dynamic{..}`.

The data-slot mutability is independent of the second slot.

| Pointee + borrow                              | `data` slot mutability                                | Second slot |
| --------------------------------------------- | ----------------------------------------------------- | ----------- |
| `Ref(Str, SharedBorrow)`                      | shared (immutable)                                    | length      |
| `Ref(Str, UniqueBorrow)`                      | unique (mutable)                                      | length      |
| `Ptr(Str, Mutability::Not)`                   | shared (immutable)                                    | length      |
| `Ptr(Str, Mutability::Mut)`                   | unique (mutable)                                      | length      |
| `Ref(Slice(T), SharedBorrow)`                 | shared if `!contains_unsafe_cell(T)`, else mutable    | length      |
| `Ref(Slice(T), UniqueBorrow)`                 | unique (mutable)                                      | length      |
| `Ptr(Slice(T), Mutability::Not)`              | shared (immutable)                                    | length      |
| `Ptr(Slice(T), Mutability::Mut)`              | unique (mutable)                                      | length      |
| `Ref(Dynamic{..}, _)` / `Ptr(Dynamic{..}, _)` | erased (mutability not carried; see §17.2.3)          | vtable      |

This section pins the abstract **qualification (data-slot
mutability bit under the §17.1 / §17.2.3 erasure rules)** and
**slot count** of each fat-pointer flavor. Per-target C
typedef shapes (e.g., `struct rust_str`, `struct
rust_slice_T`, `struct trait_obj_Trait` for the generic-C
target) are backend-codegen content; see
`generic-c-backend.md` §3 for the generic-C target's
fat-pointer C struct table and worked examples.

**C boundary rejection.** The fat-pointer lowering rules
above apply to **Rust-internal** uses (Rust-ABI function
calls, value-context locals, struct fields). At `FnAbi::C`
parameter/return positions, `Ref`/`Ptr` to unsized pointees
are **rejected** by the C ABI admissibility predicate
(§14.5). Driver code crossing extern "C" passes the data
pointer and length / vtable pointer as separate parameters;
the fat-pointer aggregate is not admissible at the C
boundary.

#### 17.2.3 Trait-object dispatch carve-out

For `Ref(Dynamic{..}, _)` and `Ptr(Dynamic{..}, _)`, the
fat-pointer's **`data` slot has its mutability discriminant
erased**: the IR drops the source `Provenance` / `Mutability`
distinction at the dyn boundary uniformly across all source
borrow flavors.

Reason: trait-object dispatch erases the concrete `Self`
type. The vtable shim's receiver is uniformly `*mut ()`
(§13.2), because shared `&self` methods on impls whose
`Self` contains `UnsafeCell` may legitimately mutate through
the shared reference — preserving a source-side
immutability discriminant through type-erased dispatch
would be unsound for those impls. The
`contains_unsafe_cell` predicate (above) cannot be evaluated
on `Dynamic{..}` (the concrete `Self` is not in the type),
so the only sound rule is to drop the mutability
discriminant uniformly at the trait-object boundary.

The Rust borrow flavor remains visible at the IR's
type-system level — the source `Ref(Dynamic{..},
SharedBorrow)` is still distinguishable from
`Ref(Dynamic{..}, UniqueBorrow)` in the `Ty` term — but the
**`data` slot's mutability bit is dropped** in the
fat-pointer aggregate (§17.1 wide-pointee table). The
vtable slot carries an immutable vtable handle uniformly
across all source borrow flavors; vtable instances are
immutable statics, and this immutability is a property of
vtable identity, not a per-borrow discriminant.

Per-target rendering of the erased `data` slot is
backend-codegen content; see `generic-c-backend.md` §3.3 for
the generic-C target.

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
distinct types remain distinct in the IR (see `discussions.md`
D15 for the rationale). This contract drove `FnAbi` storage
on `FnSig`, the `sig_c_abi_lowerings` side table (so
`extern "C" fn(Handle)` and `extern "C" fn(u32)` remain
distinct), and the `PrincipalTraitKey` shape (so
`dyn Register<u32>` and `dyn Register<u64>` remain distinct).

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

## 18. Pretty-printer

The pretty-printer specification lives in top-level
`pretty-printer.md`. That file is the canonical source
for rendering style, the `PrettyPrinter` API surface,
sigil conventions, the per-method ownership table,
per-renderer specifications, and worked-example
specimens. `spec.md` and `pretty-printer.md` are
co-equal merger artifacts: `spec.md` covers the IR's
structural definitions (producer-side); `pretty-printer.md`
covers the rendering surface (consumer-side). For
rationale, see top-level `discussions.md`.

### 18.1 Sigil conventions

See `pretty-printer.md` §3.3.

### 18.2 Renderer ownership table

See `pretty-printer.md` §8.

---

## 19. Place and projections

The `Place` and `PlaceElem` vocabulary is the lvalue surface
of MIR -- the typed expression of "which memory location."
The bridge fills it from rustc's `mir::Place<'tcx>` /
`mir::PlaceElem<'tcx>`; downstream consumers (stages 6-11
body content, codegen) read it as the `Place` slot inside
operand / rvalue / statement / terminator payloads.

### 19.1 `Place` and `PlaceElem` definitions

```rust
/// A typed lvalue: head local plus a chain of projections.
///
/// `Place` is body-local content: it appears inside a body's
/// statements / terminators, not in any top-level interner
/// table. Equality is structural over `(local, projection)`.
pub struct Place {
    /// The head local. Must resolve to a `LocalDecl` entry in
    /// the enclosing body's `local_decls` (stage-11 vocabulary).
    pub local: LocalId,

    /// The projection chain, applied left-to-right. Empty for
    /// a bare local (e.g., `Place::from(_3)`). Owned slab
    /// (no interning); see §19.2.
    pub projection: Box<[PlaceElem]>,
}

/// Closed projection vocabulary (6 variants in source-
/// declaration order). The bridge raises a diagnostic if a
/// rustc `PlaceElem<'tcx>` outside this set appears (see
/// §15.1 rejections for `OpaqueCast` and `UnwrapUnsafeBinder`).
pub enum PlaceElem {
    /// Pointer load: `*head` / `*projected_value`. Subject to
    /// the I-Deref-first invariant (§19.3).
    Deref,

    /// Field projection. The `FieldIdx` indexes into the
    /// projected type's field list, dispatched by predecessor
    /// `Ty` (per §11 layout vocabulary):
    /// - `Ty::Struct{..}`: the `LayoutKind::Struct.fields`
    ///   vector.
    /// - `Ty::Closure(item, args)`: an identity view whose
    ///   layout lives on a parallel `Ty::Struct(item, args, _)`
    ///   per stage-3 D6.3. Resolve via
    ///   `find_nominal(NominalKind::Struct, item, args)` and
    ///   use that `Ty::Struct`'s `LayoutKind::Struct.fields`.
    ///   (`Ty::Closure` itself does not carry an inline
    ///   `Layout`.)
    /// - `Ty::Union{..}`: the `LayoutKind::Union.fields`
    ///   vector.
    /// - `Ty::Tuple{..}`: the tuple's `fields` vector.
    /// - `Ty::Enum{..}` after a prior `Downcast`: the active
    ///   variant's `VariantLayout.fields` (carried under
    ///   `LayoutKind::Enum.shape`'s
    ///   `EnumShape::Multiple.variants` or
    ///   `EnumShape::Single.variant` per §11.1).
    /// The inline `TyId` is the post-narrow field type,
    /// stored redundantly with what the layout would yield.
    /// Cheap O(1) narrowing during the projection walk.
    Field(FieldIdx, TyId),

    /// Array / slice index by a runtime-computed value held
    /// in a separate local. The `LocalId` payload is the
    /// head of a separate `Place::from(local)` that produces
    /// the index value. **Note:** `Index` carries a `LocalId`,
    /// not an `Operand` -- so `Place` has no circular
    /// dependency on stage-6 `Operand`.
    Index(LocalId),

    /// Constant offset into a fixed-length array or slice,
    /// generated by slice/array patterns. The doc-level
    /// invariant from rustc (`stage-5/prior-art/rustc.md` §3.4) is
    /// that `from_end` is always `false` when indexing an
    /// array (the MIR builder folds the array length into
    /// the offset rather than emitting a from-end form).
    /// Three combinations appear in our input:
    /// - `ConstantIndex { from_end: false }` over an array:
    ///   `array[offset]`.
    /// - `ConstantIndex { from_end: false }` over a slice:
    ///   `slice[offset]`.
    /// - `ConstantIndex { from_end: true }` over a slice:
    ///   `slice[len - offset]` where `len` is the slice's
    ///   runtime length.
    /// The cross combination (`from_end: true` over an
    /// array) does not appear in our input and is rejected
    /// by Place validation (§19.7).
    ///
    /// Worked examples (slice-pattern shapes from rustc
    /// `syntax.rs:1232-1238`; show the per-pattern
    /// `ConstantIndex` payload, not constraining the base
    /// type beyond what the doc-level invariant fixes):
    ///
    /// ```text
    /// [X, _, .., _, _] => offset 0, min_length 4, from_end false
    /// [_, X, .., _, _] => offset 1, min_length 4, from_end false
    /// [_, _, .., X, _] => offset 2, min_length 4, from_end true (slice base only)
    /// [_, _, .., _, X] => offset 1, min_length 4, from_end true (slice base only)
    /// ```
    ConstantIndex { offset: u64, min_length: u64, from_end: bool },

    /// Sub-slice projection. The doc-level invariant from
    /// rustc (`stage-5/prior-art/rustc.md` §3.5) constrains
    /// `from_end == true` iff the base is a slice;
    /// `false` iff the base is an array. Two combinations
    /// appear in our input:
    /// - `Subslice { from, to, from_end: false }` over an
    ///   array: yields `array[from..to]`.
    /// - `Subslice { from, to, from_end: true }` over a
    ///   slice: yields `slice[from..slice.len() - to]`.
    /// The cross combinations (`from_end: true` over an
    /// array; `from_end: false` over a slice) do not appear
    /// in our input and are rejected by Place validation
    /// (§19.7).
    Subslice { from: u64, to: u64, from_end: bool },

    /// Downcast to a specific enum variant (selecting the
    /// active variant for the next `Field` projection).
    /// Coroutine variants are unreachable because the
    /// coroutine family is rejected at type translation
    /// (§15.1). Subject to the I-Downcast-followed-by-Field
    /// invariant (§19.3). The `VariantIdx` is the source-
    /// order variant index (per §2.4 / §11.0; stage-3 D11
    /// pinned the layout-side variant table); rustc's
    /// `Option<Symbol>` variant-name slot is dropped (per
    /// D-5-1.3 -- printing-only in rustc, recovered render-
    /// side from the enum layout's `VariantLayout.name`
    /// per §11.1 / §19.8).
    Downcast(VariantIdx),
}
```

The six variants are exhaustive: a rustc post-Cleanup
`mir::PlaceElem<'tcx>` reaching the bridge must canonicalise
into exactly one variant, or the bridge rejects with a
diagnostic. There is no "Other" / "Opaque" variant. Of rustc's
8 `ProjectionElem` variants, `OpaqueCast` is absent in our
input under both solver paths and `UnwrapUnsafeBinder` is
gated and rejected -- see §15.1 for the rejection rows.

### 19.2 Identity and storage

`Place` is body-local content; it does not participate in any
top-level interning. There is no `PlaceId`. Equality and
hashing are structural over `(local, projection)`.

The projection chain is stored as `Box<[PlaceElem]>`. This
matches rustc's `&'tcx List<PlaceElem<'tcx>>` slab semantics
modulo `'tcx`-arena interning (which Q2 forbids inheriting).
Eight bytes leaner than `Vec<PlaceElem>` (no capacity slot)
and conveys the precise immutable-after-construction
contract. (If end-of-phase-1 profiling confirms a typical
projection-length pattern that motivates a small-vec
optimisation, the swap is a mechanical impl change but
*not* invisible to the public field type; it would land via
a follow-up spec revision.)

`Place` does not participate in the cross-build identity
discipline (F4 / F4-2 in §16). Place equality is per-build,
structural; `LocalId`s are body-local. This is a deliberate
scope reduction vs. `Ty` (cross-build via `TyId` interning)
and `Const` (cross-build via `ConstIdentityKey`); body-local
identity is sufficient for Place's role.

### 19.3 Structural invariants

Two structural invariants -- both strictly tighter than the
rustc `ProjectionElem` enum's static type, with distinct
grounds: I-Deref-first comes from rustc's PostCleanup phase
(a guaranteed invariant of our `MirPhase::Runtime(Optimized)`
input); I-Downcast-followed-by-Field comes from
`PlaceTy::projection_ty`'s successor assertion plus the H4
exhaustive cross-check across all rustc `SetDiscriminant`
construction sites (no site produces `Downcast`-as-last).

**I-Deref-first.** If `place.projection[i]` is `Deref`, then
`i == 0`. Compound chains like `Field.Deref` cannot occur in
our input (rustc's PostCleanup phase forbids them; box-deref
elaboration introduces a fresh raw-pointer temp rather than
a non-first `Deref` projection -- the `Deref` is preserved as
the first projection of the new place over the temp).

**I-Downcast-followed-by-Field.** If `place.projection[i]` is
`Downcast(_)`, then `i < projection.len() - 1` AND
`projection[i+1]` is `Field(_, _)`. `Downcast` is the variant
selector; the only legal next step is the field projection
that reads the variant's field. `Downcast`-as-last is rejected
(no rustc construction site produces it -- enumerated
exhaustively across `SetDiscriminant`-style sites; see
`discussions.md` D-5-1.6 for the audit).

Both invariants are checked in §3.3 Phase B place validation
(rule 5; see §19.7).

### 19.4 Type-narrowing rules

Each `PlaceElem` step derives a successor "place type" --
the pair `(Ty, active_variant)` -- from the predecessor.
The chain begins at the head local's `LocalDecl.ty`, with
no active variant. The rules per `PlaceElem` variant:

| Variant | Predecessor `Ty` requirement | Successor `Ty` | Active variant after step |
|---|---|---|---|
| `Deref` | `Ty::Ref(inner, _)` or `Ty::Ptr(inner, _)` (raw pointer) | the resolved `inner` | `None` (I-Deref-first means no prior `Downcast`) |
| `Field(idx, fty)` | when active variant is `None`: `Ty::Struct{..}`, `Ty::Union{..}`, `Ty::Tuple{..}`, or `Ty::Closure(..)` (closure-as-struct shape; see `discussions.md` D6.3 for rationale); when active variant is `Some(v)`: `Ty::Enum{..}` (projecting into variant `v`'s fields) | `fty` (must equal the field type at `idx` in the appropriate variant's field table) | `None` (cleared) |
| `Index(_)` | `Ty::Array(elem, _)` or `Ty::Slice(elem)` | the resolved `elem` | unchanged |
| `ConstantIndex { from_end: false, ... }` | `Ty::Array(elem, _)` or `Ty::Slice(elem)` | the resolved `elem` | unchanged |
| `ConstantIndex { from_end: true, ... }` | `Ty::Slice(elem)` | the resolved `elem` | unchanged |
| `Subslice { from, to, from_end: false }` | `Ty::Array(elem, _)` | `Ty::Array(elem, to - from)` | unchanged |
| `Subslice { from, to, from_end: true }` | `Ty::Slice(elem)` | `Ty::Slice(elem)` (unchanged) | unchanged |
| `Downcast(idx)` | `Ty::Enum{..}` | unchanged | `Some(idx)` |

`Ty::Coroutine` does not appear in this table because the
coroutine family is rejected at type translation (§15.1);
rustc's `Field`-on-coroutine and `Downcast`-on-coroutine
narrowing arms therefore have no analog in our input.

The active-variant interaction with `Field`: when the
predecessor's active variant is `Some(v)`, the `Field(idx,
fty)` step reads the `idx`-th field of variant `v`'s
`VariantLayout.fields` (rather than the enum's general layout,
which doesn't have a flat field list). The `fty` payload must
equal that field's type. After consumption, the active
variant is cleared (the post-`Field` place type has active
variant `None`).

The `Subslice` doc-level invariant from
`stage-5/prior-art/rustc.md` §3.5 is that for `PlaceElem`s
`from_end == true` iff the base is a slice; `false` iff the
base is an array. Only the two combinations shown in the
table above appear in our `optimized_mir` input. The cross
combinations (`Subslice { from_end: true }` over an array;
`Subslice { from_end: false }` over a slice) are rejected
by Place validation (§19.7 rule 1) as bridge bugs. Rustc's
narrowing function
(`rustc_middle/src/mir/statement.rs:224-234`) handles both
array arms (`from_end: true` and `from_end: false`) as a
general utility -- the function is called in non-`PlaceElem`
contexts too -- but the `PlaceElem` doc fixes the input
shape per the invariant, so the array-with-`from_end: true`
arm is non-normative for our spec and surfaces only as a
diagnostic if a bridge bug emits the wrong combination.

### 19.5 Bridge translation

Per-variant translation from rustc `mir::PlaceElem<'tcx>` to
Regseq `PlaceElem`:

| Rustc variant | Regseq variant |
|---|---|
| `ProjectionElem::Deref` | `PlaceElem::Deref` |
| `ProjectionElem::Field(FieldIdx, Ty<'tcx>)` | `PlaceElem::Field(FieldIdx, TyId)` (the `TyId` is the post-`'tcx`-erasure interner key for the field type) |
| `ProjectionElem::Index(Local)` | `PlaceElem::Index(LocalId)` |
| `ProjectionElem::ConstantIndex { offset, min_length, from_end }` | `PlaceElem::ConstantIndex { offset, min_length, from_end }` (verbatim) |
| `ProjectionElem::Subslice { from, to, from_end }` | `PlaceElem::Subslice { from, to, from_end }` (verbatim) |
| `ProjectionElem::Downcast(Option<Symbol>, VariantIdx)` | `PlaceElem::Downcast(VariantIdx)` (the `Option<Symbol>` is dropped; see `discussions.md` D-5-1.3 for rationale) |
| `ProjectionElem::OpaqueCast(_)` | bridge-rejection (see §15.1) |
| `ProjectionElem::UnwrapUnsafeBinder(_)` | bridge-rejection (see §15.1) |

The rustc `Place::projection: &'tcx List<PlaceElem<'tcx>>` is
materialised as the owned `Box<[PlaceElem]>` per §19.2; the
bridge constructs the slab once per Place and the result is
immutable.

### 19.6 C lowering

Per-`PlaceElem` C lvalue forms are backend-codegen content;
see `generic-c-backend.md` §4 for the generic-C target's
table. The lowering is deterministic from the projection-chain
content with no special-case flags (composition rule per the
§2 compositional-behavior principle).

### 19.7 Place validation

§3.3 Phase B place-validation rules. The five rule categories:

1. **Type-coherence walk.** Each `PlaceElem` step's
   predecessor-type requirement and successor-type derivation
   is satisfied per §19.4. The walk begins at the head
   local's `LocalDecl.ty` with active variant `None` and
   applies the rules in projection order.

2. **Head-local existence.** The head `local: LocalId`
   resolves to a valid entry in the enclosing body's
   `local_decls` table (stage-11 data).

3. **`FieldIdx` range.** For each `Field(idx, _)`, `idx` is
   in range for the predecessor type's field list,
   dispatched by predecessor (per the §19.1 doc and §11
   layout vocabulary):
   `LayoutKind::Struct.fields.len()` for `Ty::Struct{..}`;
   for `Ty::Closure(item, args)`, look up the parallel
   `Ty::Struct(item, args, _)` via
   `find_nominal(NominalKind::Struct, item, args)` and use
   that struct's `LayoutKind::Struct.fields.len()` (closure-
   as-struct identity-vs-layout split; see `discussions.md`
   D6.3 for rationale);
   `LayoutKind::Union.fields.len()` for `Ty::Union{..}`; the
   tuple's `fields.len()` for `Ty::Tuple{..}`; the active
   variant's `VariantLayout.fields.len()` for `Ty::Enum{..}`
   after `Downcast`.

4. **`VariantIdx` range.** For each `Downcast(idx)`, let
   `shape` be the `EnumShape` carried in the predecessor
   `Ty::Enum{..}`'s `layout.kind` (per §11.1, the layout's
   inner `LayoutKind::Enum.shape`). Then `idx` is in
   `0..variant_count(shape)`, where `variant_count`
   dispatches on `EnumShape` (§11.1): `Empty -> 0`,
   `Single { .. } -> 1`,
   `Multiple { variants, .. } -> variants.len()`.

5. **Structural invariants.** I-Deref-first (§19.3) and
   I-Downcast-followed-by-Field (§19.3).

**Walk implementation: deferred to stage 11.** Places live
inside body content (operand / rvalue / statement /
terminator payloads). Stage-1's `RegseqModel` carries empty
bodies, so no Places are present until stage 11 materialises
`Body` / `LocalDecls`. The §3.3 Phase B walk applying these
rules is added as part of stage 11's body-validation step;
once the walk lands, all 5 rules are applied in a single
traversal of body content with no rule-specific
infrastructure. (See `discussions.md` D-5-2 for the audit
shape and the deferral rationale.)

### 19.8 Rendering

See `pretty-printer.md` §7.1.

---


## 20. Operand vocabulary

The `Operand` vocabulary is the RHS-atom surface of MIR --
the typed expression of "which value." The bridge fills it
from rustc's `mir::Operand<'tcx>`; downstream consumers
(stages 7-9 body content, codegen) read it as the `Operand`
slot inside Rvalue / Statement / Terminator payloads.

### 20.1 `Operand` enum definition

```rust
/// A typed RHS atom: either a load from a place, or a
/// constant value. Body-local content; appears inside a
/// body's rvalues / statements / terminators, not in any
/// top-level interner table. Equality is structural.
pub enum Operand {
    /// Load a value from the place. Marks the place as
    /// participating in a copy-style read; semantically
    /// equivalent to `Move` after drop-elaboration in
    /// optimized_mir but distinguishable for backend
    /// lowering (e.g., debug-info annotation, copy-vs-memcpy
    /// dispatch).
    Copy(Place),

    /// Load a value from the place. Marks the place as
    /// participating in a move-style consumption. At
    /// rustc's `Call` argument site this enables in-place
    /// passing semantics; elsewhere in optimized_mir this
    /// is operationally equivalent to `Copy`.
    Move(Place),

    /// A constant value, referenced by its `ConstId` in the
    /// model's `ConstInterner` (§9.7). Drops rustc's
    /// `Box<ConstOperand>` wrapper -- `user_ty` is
    /// NLL/borrowck-only and dead in optimized_mir;
    /// operand-level `span` is dropped (for Statement-level
    /// uses the use-site span is naturally carried by the
    /// containing Statement's `SourceInfo` at stage 8; for
    /// Call / TailCall arguments where rustc preserves a
    /// finer-grained per-argument span via
    /// `Spanned<Operand>` at the Terminator level, the span
    /// is captured at the Terminator's argument-list slot
    /// in stage 9, not embedded in the Operand itself).
    Constant(ConstId),
}
```

The three variants are exhaustive: a rustc `mir::Operand<'tcx>`
reaching the bridge canonicalises into exactly one variant
(or, in the case of `RuntimeChecks`, is rewritten before
becoming an `Operand` -- see §20.3.1). There is no `Other` /
`Opaque` arm.

Sizing:
- `Place` ~20 bytes (`LocalId` 4 + `Box<[PlaceElem]>` 16).
- `ConstId` 4 bytes (`NonMaxU32`).
- Worst-case enum width ~24 bytes including discriminant.

No `Box` indirection on `Constant` because `ConstId` is
small; contrast rustc's `Box<ConstOperand>` indirection,
which exists because `ConstOperand` carries `Span` +
`UserTy` + `Const` inline. RegseqModel's stripped wrapper
plus interned-`Const` storage removes the size pressure.

### 20.2 Identity and storage

`Operand` is body-local content; it does not participate in
any top-level interning. There is no `OperandId`. Equality
and hashing are structural over the variant payload.

Operands are constructed inline at use sites in stages 7-9
payloads (`Rvalue`, `Statement`, `Terminator`). The owning
struct stores the `Operand` by value (no indirection)
unless the parent variant chooses to box for size reasons --
a stage-7+ choice, not stage 6's concern.

Operand identity is per-build, structural, and does not
participate in the cross-build identity discipline (F4 /
F4-2, §16). Same scope reduction as `Place` (§19.2).

### 20.3 Bridge translation

Per-rustc-variant translation:

| rustc form                                | RegseqModel form                                                       |
| ----------------------------------------- | ---------------------------------------------------------------------- |
| `mir::Operand::Copy(p)`                   | `Operand::Copy(translate_place(p))`                                    |
| `mir::Operand::Move(p)`                   | `Operand::Move(translate_place(p))`                                    |
| `mir::Operand::Constant(box co)`          | `Operand::Constant(intern_const(translate_const(co.const_)))`          |
| `mir::Operand::RuntimeChecks(checks)`     | `Operand::Constant(intern_const(Const::Bool(checks.value(tcx.sess))))` -- see §20.3.1 |

`Copy` and `Move` translate directly, threading
`translate_place` from §19.5.

`Constant` strips rustc's `ConstOperand` wrapper. `co.const_`
flows through `translate_const` (the §9.7 bridge
construction dispatch for `mir::Const<'tcx>` -> RegseqModel
`Const`) and is interned via `ConstInterner::intern` (§9.7). The wrapper's
operand-level `span` is dropped from the Operand itself.
For Statement-level uses (Assign, etc.), the use-site
source location is naturally carried by the containing
Statement's `span` field at stage 8 (per §22.1.1's
`Statement { kind, span }` wrapper). For Call
arguments, rustc preserves a finer-grained per-argument
span via `Spanned<Operand>` at the Terminator level
(`rustc_middle::mir::syntax.rs:811-820`); stage 9
**drops** these per-argument spans entirely (per §23.2.5
+ §23.7's `args` translation rule -- `args.iter().map(|sp|
translate_operand(&sp.node)).collect()` strips the
`Spanned` wrapper). Per-argument diagnostic granularity
falls back to the Terminator-level `span`. (TailCall is
not in the RegseqModel surface per §23.6.1 row 7.) The wrapper's
`user_ty` is dropped (NLL/borrowck-only metadata, dead in
`optimized_mir`).

#### 20.3.1 `RuntimeChecks` lowering rule

When the bridge encounters
`mir::Operand::RuntimeChecks(checks)`, it does **not**
preserve the variant in RegseqModel. The bridge:

1. Resolves the boolean value against the bridge-host's
   rustc session:
   ```text
   b = checks.value(tcx.sess)
   ```
   Per Q4 (`rustc_driver::Callbacks` integration), the
   bridge runs inside a single rustc invocation per build
   with one `Session` in scope. All `RuntimeChecks`
   operands -- whether sourced from MIR constructed in the
   primary spec crate or in a regseq-aware dependency
   compiled separately under `RegseqAwareCallbacks` (Q6)
   and consumed via `tcx.optimized_mir(def_id)` for
   cross-crate `DefId`s -- resolve against the bridge-host
   crate's session. This mirrors rustc's documented codegen
   behavior: `RuntimeChecks` values are determined by the
   *consumer's* session, not the producer's; for
   RegseqModel the consumer is the bridge. Within one
   build the lowering is deterministic. Different builds
   may produce different lowered consts (e.g., a build with
   `ub_checks=on` produces `Const::Bool(true)` where a
   build with `ub_checks=off` produces `Const::Bool(false)`).
   The build-equivalence quad in §12.4 / §16 (equal source,
   equal pinned toolchain, equal `RegseqModel.target`,
   equal bridge options) does not currently list
   `ub_checks` / `contract_checks` / `overflow_checks` among
   its equality dimensions, so two builds that differ only
   on those flags satisfy the quad's equivalence but produce
   different lowered consts at `RuntimeChecks`-derived
   operand sites. If cross-build alignment of those
   operand sites becomes a requirement, the appropriate
   fix is to widen the quad in §12.4 (out of stage 6's
   scope).

   Resolution table (per `rustc_middle::mir::syntax.rs:1585-1592`):

   | `RuntimeChecks` arm | session method            |
   | ------------------- | ------------------------- |
   | `UbChecks`          | `sess.ub_checks()`        |
   | `ContractChecks`    | `sess.contract_checks()`  |
   | `OverflowChecks`    | `sess.overflow_checks()`  |

2. Constructs `Const::Bool(b)` (the `Bool` variant of the
   §9.7 `Const` enum).

3. Interns the `Const::Bool(b)` via `ConstInterner::intern`,
   yielding a `ConstId`. The const-uniqueness invariant
   (§9.7 D-4-2 byte-form schema) ensures the same boolean
   value at multiple sites resolves to the same `ConstId`.

4. Emits `Operand::Constant(ConstId)`.

The IR carries no `RuntimeChecks` variant. Driver source
that legitimately uses `cfg!(ub_checks)`,
`core::intrinsics::contract_checks()`, or analogous
constructs is accepted: the operand-level evidence becomes
an `Operand::Constant(ConstId)` referencing a
`Const::Bool(b)` whose `b` is build-pinned via the session.

This is gentler than reject-on-encounter while preserving
the closed-vocabulary discipline at the IR level. See
`discussions.md` D-6-1.2 for the reject-vs-lower rationale,
the option-(a) / option-(b) / option-(c) trade-off, and the
Charon precedent for option (c).

### 20.4 C lowering

Per-`Operand`-variant C lowerings are backend-codegen content;
see `generic-c-backend.md` §5 for the generic-C target. The
IR-side commitment at this section is the
`Operand::Constant(ConstId) -> Const-resolution` boundary --
the operand carries an interner key, the backend resolves it
through `model.consts` per the closed `Const` vocabulary at
§9.7. Because the IR has no `RuntimeChecks` variant (§20.3.1),
no backend ever sees a session-flag query at an operand site;
operands referencing build-pinned booleans appear as
`Constant(ConstId)` referencing a `Const::Bool(b)` exactly as
if the source had written the literal `true` / `false`.

### 20.5 Operand-site invariants

Stage 6 specifies one operand-site invariant, discharged by
inheritance:

**I-Operand-Place-Validity.** For `Operand::Copy(p)` and
`Operand::Move(p)`, the place `p` must satisfy the five
Place-validation rules in §19.7 (head-local existence,
`FieldIdx` range, `VariantIdx` range, structural invariants
I-Deref-first and I-Downcast-followed-by-Field, and
type-coherence walk). Discharged by **inheritance**:
enforcing Place validity transitively covers all
`Operand::Copy` / `Operand::Move` slots; no operand-side
walk is needed beyond what stage 5 + stage 11 already
provide.

**No type-`Copy` enforcement on `Operand::Copy(p)`.** Rustc
documents the type-`Copy` invariant on `Operand::Copy` as
required pre-drop-elaboration only and explicitly gone
post-drop-elaboration (`rustc_middle/src/mir/syntax.rs:1312-1313`).
RegseqModel reads `optimized_mir`, which is
post-drop-elaboration; the rustc-side invariant has been
discharged before the IR reaches us. The bridge does not
re-enforce it. See `discussions.md` D-6-3 for rationale.

**No type-coherence enforcement on `Operand::Constant(id)`.**
`ConstId` carries canonical value identity through the
`ConstInterner` (§9.7), but some `Const` variants
(notably `ZST` and `EnumValue`) get their type from
consumer context rather than from the `Const` payload
itself. Operand-context type coherence -- matching the
`Const`'s effective type at the use site against the
operand-context-expected type -- is owned by stages 7-9.
Stage 6 does not duplicate that check.

The Place-validation walk over operand slots is non-vacuous
only once stages 7-9 materialise operand use sites; it
piggybacks on the §3.3 Phase B body-validation traversal
that stage 11 owns (per D-5-2 deferral).

### 20.6 Rendering

See `pretty-printer.md` §7.2.

---

## 21. Rvalue vocabulary

The `Rvalue` vocabulary is the RHS-expression surface of MIR
-- the typed expression of "how a value is computed." The
bridge fills it from rustc's `mir::Rvalue<'tcx>`; downstream
consumers (stage 8 statements, stage 9 terminators --
transitively through `StatementKind::Assign(Place, Rvalue)`;
codegen) read it as the `Rvalue` slot inside
`Statement::Assign` payloads.

### 21.1 `Rvalue` enum definition

```rust
/// A typed RHS expression: a value-producing computation.
/// Body-local content; appears inside a body's statements,
/// not in any top-level interner table. Equality is
/// structural.
pub enum Rvalue {
    /// Identity: yields the operand's value unchanged.
    Use(Operand),

    /// Array constructor: `[x; N]`. The `u64` count is the
    /// post-monomorphisation array length, mirroring stage-3
    /// `Ty::Array(elem_ty, n)` canonical form (§10.0).
    Repeat(Operand, u64),

    /// Borrow construction: `&p` / `&mut p`. The `Provenance`
    /// discriminator reuses the §10.1 type-system enum
    /// (`SharedBorrow` / `UniqueBorrow`); rustc's
    /// `Region<'tcx>` slot is dropped per the region-erasure
    /// rule of §17.1.
    Ref(Provenance, Place),

    /// Raw pointer construction: `&raw const p` / `&raw mut p`.
    /// The `Mutability` discriminator reuses the §10.1
    /// type-system enum (`Not` / `Mut`).
    RawPtr(Mutability, Place),

    /// `as`-style cast (or compiler-inserted coercion). The
    /// destination `TyId` resolves through the §12 type
    /// interner.
    Cast(CastKind, Operand, TyId),

    /// Binary arithmetic / comparison / pointer offset.
    /// Operands inline (no `Box<(Operand, Operand)>`
    /// indirection); see §21.2.6 for the size-budget
    /// rationale.
    BinaryOp(BinOp, Operand, Operand),

    /// Unary operation: `!x`, `-x`, or pointer-metadata
    /// extraction.
    UnaryOp(UnOp, Operand),

    /// Read the discriminant of an enum at the place. For
    /// non-enum predecessor types yields zero (rustc's
    /// documented semantics, `rustc_middle/src/mir/syntax.rs:1439-1449`).
    Discriminant(Place),

    /// Aggregate value construction (tuple / struct / enum
    /// variant / union / closure environment / fixed-length
    /// array literal / fat raw pointer). Operand storage is
    /// `Box<[Operand]>` (boxed slice; matches §19.2's
    /// `Box<[PlaceElem]>` projection-storage convention).
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

The nine variants are exhaustive: a rustc `mir::Rvalue<'tcx>`
reaching the bridge canonicalises into exactly one variant,
or the bridge rejects with a diagnostic (§21.5.1). There is
no `Other` / `Opaque` arm.

Sizing (reference):
- `Operand` ~24 bytes (§20.1).
- `Place` ~20 bytes (§19.2).
- `BinaryOp` flat width ~56 bytes; dominates the enum.
- Worst-case `Rvalue` width ~64 bytes including discriminator.

#### 21.1.1 Variants dropped vs rustc

Of rustc's 13-variant `mir::Rvalue<'tcx>`
(`rustc_middle/src/mir/syntax.rs:1369-1482`), four variants
are dropped from the RegseqModel surface; each is rejected on
encounter (§21.5.1):

- `CopyForDeref(Place)` -- eliminated by the required
  `EraseDerefTemps` runtime-lowering pass
  (`rustc_mir_transform/src/lib.rs:658`); rustc codegen
  `bug!()`s the variant
  (`rustc_codegen_ssa/src/mir/rvalue.rs:712`). Cannot reach
  optimized_mir.
- `ShallowInitBox(Operand, Ty)` -- eliminated by the
  required `ElaborateBoxDerefs` runtime-lowering pass
  (`rustc_mir_transform/src/lib.rs:659`); rustc codegen
  `bug!()`s the variant
  (`rustc_codegen_ssa/src/mir/rvalue.rs:713`). Cannot reach
  optimized_mir.
- `ThreadLocalRef(DefId)` -- DSL-surface rejection: the v1
  driver DSL targets Linux kernel modules and Zephyr drivers,
  neither of which exposes `#[thread_local]` runtime
  infrastructure to driver code. Source code that uses
  `#[thread_local]` cannot produce a working driver on the
  documented target OSes.
- `WrapUnsafeBinder(Operand, Ty)` -- §15.1 (Type-level
  rejection table at line 4583) rejects `Ty::UnsafeBinder`
  at type translation; the rvalue-level rejection cascades
  by composition (the binder type the variant wraps cannot
  enter the IR). Additional grounds: gated behind the
  unstable `unsafe_binders` feature; the binder type has no
  C-shape; no v1 driver-corpus precedent (Charon and mrustc
  both lack the analog).

Closely related: rustc's `Operand::RuntimeChecks` is
rewritten at the operand surface (§20.3.1) before reaching
any `Rvalue` slot, so RegseqModel `Rvalue` operand positions
never see session-flag queries.

### 21.2 Per-variant payload shapes

#### 21.2.1 `Use(Operand)`

Yields the operand's value unchanged. Direct embed of
stage-6 `Operand`. C-semantics: a value-load (or constant
materialisation) of the operand at the use site.

#### 21.2.2 `Repeat(Operand, u64)`

Constructs an array `[x; N]` whose every element equals the
value of `Operand`. The count `N` is a
post-monomorphisation `u64`, mirroring stage-3's canonical
`Ty::Array(elem_ty, n)` length (§10.0). Rustc's MIR carries
the count as a `ty::Const<'tcx>` (the *type-system* constant
carrier, distinct from the *value-system* `mir::Const`); the
post-monomorphisation value is a concrete non-negative
integer fitting in `u64`. The bridge resolves the type-const
and stores the plain `u64`; no routing through the
value-system `ConstInterner` (§9.7) is needed -- the
type-system carrier already has its canonical form.

#### 21.2.3 `Ref(Provenance, Place)`

Constructs an immutable or mutable reference to the place.
The `Provenance` discriminator reuses the §10.1 enum
(`SharedBorrow` / `UniqueBorrow`), keeping the
borrow-flavour discriminator coherent across `Ty::Ref` and
`Rvalue::Ref`. Rustc's `Region<'tcx>` slot is dropped:
regions are fully erased in `Ty::Ref` (§17.1), and codegen
itself ignores the rvalue's region slot
(`rustc_codegen_ssa/src/mir/rvalue.rs:506-511` patterns
`(_, bk, place)` and synthesises with
`tcx.lifetimes.re_erased`).

The `BorrowKind` collapse (mapping rustc's 3-variant
`BorrowKind` plus 3-variant `MutBorrowKind` sub-discriminator
to `Provenance`'s 2 variants, including reject-on-encounter
of `BorrowKind::Fake`) is described in §21.6.1.

#### 21.2.4 `RawPtr(Mutability, Place)`

Constructs a raw pointer to the place. The `Mutability`
discriminator reuses the §10.1 enum (`Not` / `Mut`), keeping
the const-vs-mut split coherent across `Ty::Ptr` and
`Rvalue::RawPtr`. Mapping: rustc `RawPtrKind::Const` ->
`Mutability::Not`; rustc `RawPtrKind::Mut` ->
`Mutability::Mut`.

The bridge-side collapse of `RawPtrKind::FakeForPtrMetadata`
to `Mutability::Not` is described in §21.6.2.

#### 21.2.5 `Cast(CastKind, Operand, TyId)`

Performs an `as`-style cast (or a compiler-inserted
coercion) from the source operand to the destination type.
The closed `CastKind` is given in §21.3.3; the destination
`TyId` resolves through the §12 type interner.

The `CoercionSource` slot inside
`mir::CastKind::PointerCoercion(_, CoercionSource)` (per
`rustc_middle/src/mir/syntax.rs:1531-1536`) is dropped: the
diagnostic-only `AsCast` vs `Implicit` distinction is not
load-bearing for codegen and does not affect the C lowering.
Source-location information is recoverable via the
containing `Statement.source_info` at stage 8.

The `hir::Safety` slot inside
`PointerCoercion::ReifyFnPointer(Safety)` and
`ClosureFnPointer(Safety)` is dropped: the result safety is
already encoded in the destination `Ty::FnPtr(FnSigId)`
through `FnSig.safety` (§14.3); carrying it again at the
cast site is structural duplication.

#### 21.2.6 `BinaryOp(BinOp, Operand, Operand)`

Computes a binary operation on two operands. Storage is
flat (operands inline, not boxed) -- a deliberate divergence
from rustc's `Box<(Operand, Operand)>` packaging. Rustc
boxes to keep its `Rvalue<'_>` enum at the 40-byte size
assert (`rustc_middle/src/mir/syntax.rs:1752`); RegseqModel
does not assert a comparable size budget at the rvalue
surface. The flat shape is congruent with the §20 (Operand)
constant-payload pattern of inlining when possible (which
dropped the `Box<ConstOperand>` wrapper for similar size-
budget reasons; see `discussions.md` D-7-1.7 for the
size-budget analysis).

The closed `BinOp` enum is given in §21.3.4; all 26 rustc
variants are preserved verbatim.

#### 21.2.7 `UnaryOp(UnOp, Operand)`

Computes a unary operation on one operand. The closed `UnOp`
enum is given in §21.3.5; all 3 rustc variants are preserved
verbatim. `UnOp::PtrMetadata` is documented as
Runtime-MIR-only (`rustc_middle/src/mir/syntax.rs:1609`);
since `optimized_mir` is `MirPhase::Runtime(PostCleanup)`
(`rustc_mir_transform/src/lib.rs:612`), the variant is
reachable.

#### 21.2.8 `Discriminant(Place)`

Reads the discriminant of an enum at the place, returning an
integer of the enum's discriminant type. For non-enum
predecessor types the variant is well-formed and yields zero
(rustc's documented semantics,
`rustc_middle/src/mir/syntax.rs:1439-1449`). The bridge does
not constrain the predecessor type beyond rustc's own rules.

#### 21.2.9 `Aggregate(AggregateKind, Box<[Operand]>)`

Constructs an aggregate value (tuple, struct, enum variant,
union, closure environment, fixed-length array literal, or
fat raw pointer). The closed `AggregateKind` is given in
§21.3.6. Operand storage is `Box<[Operand]>` -- matching
stage-5's `Box<[PlaceElem]>` projection storage convention
(§19.2): boxed slice for owned, immutable-after-construction
content. The operand vector's per-position semantics are
dispatched by the `AggregateKind` variant; see §21.5.2 rule
**I-Rvalue-Aggregate-Operand-Count** for the per-variant
operand-count rules.

### 21.3 Sub-enums

#### 21.3.1 `Provenance` (reuse of §10.1)

`Provenance { SharedBorrow, UniqueBorrow }` is the §10.1
enum defined for the `Ty::Ref` discriminator. The same
two-variant enum is used at `Rvalue::Ref`'s discriminator
slot. No parallel enum is introduced at stage 7.

#### 21.3.2 `Mutability` (reuse of §10.1)

`Mutability { Not, Mut }` is the §10.1 enum defined for the
`Ty::Ptr` discriminator. The same two-variant enum is used
at `Rvalue::RawPtr`'s discriminator slot and at
`AggregateKind::RawPtr`'s mutability slot. No parallel enum
is introduced at stage 7.

#### 21.3.3 `CastKind` and `PointerCoercion`

```rust
pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
    Subtype,
}

pub enum PointerCoercion {
    ReifyFnPointer,           // hir::Safety dropped (§21.2.5)
    UnsafeFnPointer,
    ClosureFnPointer,         // hir::Safety dropped (§21.2.5)
    Unsize,
}
```

11 top-level `CastKind` variants; 4-variant `PointerCoercion`
sub-enum. Drops vs rustc:

- `PointerCoercion::ArrayToPointer` and
  `PointerCoercion::MutToConstPointer` -- both rewritten to
  `PtrToPtr` by the required `CleanupPostBorrowck` MIR pass
  (`rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`,
  `is_required(): true`); cannot reach optimized_mir.
  Rustc codegen also rejects them with a `bug!()`
  (`rustc_codegen_ssa/src/mir/rvalue.rs:448-452` -- "is for
  borrowck, and should never appear in codegen"). Two
  independent grounds.
- `CoercionSource` slot inside `PointerCoercion(_, _)` (per
  `rustc_middle/src/mir/syntax.rs:1531-1536`) -- diagnostic-
  only; dropped (§21.2.5).
- `hir::Safety` slot inside `ReifyFnPointer(_)` /
  `ClosureFnPointer(_)` -- structurally duplicated with the
  destination `Ty::FnPtr(FnSigId)`'s `FnSig.safety`;
  dropped (§21.2.5).

`CastKind::Subtype` is *added* by the runtime-lowering
`Subtyper` pass (`rustc_mir_transform/src/lib.rs:644`) and
survives into optimized_mir; it is preserved.

`CastKind::Transmute` is the lowering of the `mem::transmute`
intrinsic and survives into optimized_mir; preserved.

#### 21.3.4 `BinOp`

```rust
pub enum BinOp {
    // Wrapping arithmetic
    Add, Sub, Mul, Div, Rem,

    // UB-on-overflow arithmetic (from `unchecked_*` intrinsics)
    AddUnchecked, SubUnchecked, MulUnchecked,

    // Overflow-detecting arithmetic (returns `(T, bool)`)
    AddWithOverflow, SubWithOverflow, MulWithOverflow,

    // Bitwise
    BitXor, BitAnd, BitOr,

    // Shifts (wrapping and UB-on-overflow flavours)
    Shl, ShlUnchecked,
    Shr, ShrUnchecked,

    // Comparisons (return `bool`)
    Eq, Lt, Le, Ne, Ge, Gt,

    // Three-way comparison (returns `core::cmp::Ordering`)
    Cmp,

    // Pointer offset (`<*const T>::offset` semantics)
    Offset,
}
```

26 variants, preserved verbatim from
`rustc_middle/src/mir/syntax.rs:1615-1703`. No curation. Each
maps to a distinct C lowering (or runtime-helper call) at the
codegen stage; the spec commits only to the variant set, not
to per-variant C lowerings.

The `XxxWithOverflow` family
(`AddWithOverflow` / `SubWithOverflow` / `MulWithOverflow`)
returns `(T, bool)` tuples where the bool flags overflow;
these are emitted by overflow-check lowering and survive
into optimized_mir
(`rustc_middle/src/mir/syntax.rs:1426-1428`).

The `XxxUnchecked` family
(`AddUnchecked` / `SubUnchecked` / `MulUnchecked` /
`ShlUnchecked` / `ShrUnchecked`) is the UB-on-overflow
flavour, emitted by `unchecked_*` intrinsics; survives into
optimized_mir.

`Cmp` returns `core::cmp::Ordering` encoded as `i8`
(`rustc_middle/src/mir/syntax.rs:1691-1700`). `Offset`
mirrors `<*const T>::offset` semantics
(`rustc_middle/src/mir/syntax.rs:1701-1702`).

#### 21.3.5 `UnOp`

```rust
pub enum UnOp {
    Not,           // logical inversion (`!`)
    Neg,           // arithmetic negation (`-`)
    PtrMetadata,   // metadata extraction from a fat pointer
}
```

3 variants, preserved verbatim from
`rustc_middle/src/mir/syntax.rs:1597-1611`. `PtrMetadata` is
documented as Runtime-MIR-only
(`rustc_middle/src/mir/syntax.rs:1609`); since `optimized_mir`
is `MirPhase::Runtime(PostCleanup)`
(`rustc_mir_transform/src/lib.rs:612`), the variant is
reachable.

#### 21.3.6 `AggregateKind`

```rust
pub enum AggregateKind {
    /// Fixed-length array literal: `[a, b, c]`.
    /// `TyId` resolves to `Ty::Array(elem_ty, n)`.
    Array(TyId),

    /// Tuple constructor: `(a, b, c)`.
    /// `TyId` resolves to `Ty::Tuple { fields, layout }`.
    Tuple(TyId),

    /// Struct constructor: `MyStruct { field: a, ... }`.
    /// `TyId` resolves to `Ty::Struct { item, args, layout }`.
    Struct(TyId),

    /// Enum variant constructor: `MyEnum::Variant(a)`.
    /// `TyId` resolves to `Ty::Enum { item, args, layout }`;
    /// `VariantIdx` selects the active variant.
    Enum(TyId, VariantIdx),

    /// Union constructor: `MyUnion { active_field: a }`.
    /// `TyId` resolves to `Ty::Union { item, args, layout }`;
    /// `FieldIdx` selects the active field. Unions construct
    /// exactly one active field.
    Union(TyId, FieldIdx),

    /// Closure environment construction: `|...| { ... }`.
    /// `TyId` resolves to `Ty::Closure(item, args)` -- the
    /// closure-type identity view (§13.1). The captures
    /// struct's layout view is reachable via the parallel
    /// `find_nominal(NominalKind::Struct, item, args)`
    /// lookup.
    Closure(TyId),

    /// Fat raw pointer construction from `(data_ptr, metadata)`.
    /// `TyId` is the *pointee* type (mirroring rustc's
    /// `RawPtr(Ty<'tcx>, Mutability)` shape); the constructed
    /// rvalue is a raw pointer to that pointee with the
    /// indicated mutability. Both operand positions are
    /// required even for thin pointers (where metadata is
    /// the unit value).
    RawPtr(TyId, Mutability),
}
```

Seven variants. Drops vs rustc's
`rustc_middle/src/mir/syntax.rs:1538-1570`:

- `Coroutine(DefId, GenericArgsRef)` and
  `CoroutineClosure(DefId, GenericArgsRef)` -- both
  unreachable by composition with stage-3's §15.1 rejection
  of `Ty::Coroutine` / `Ty::CoroutineClosure` /
  `Ty::CoroutineWitness` at type translation. With no
  destination type available, the construction cannot be
  expressed. Additionally, the runtime-lowering
  `coroutine::StateTransform` pass
  (`rustc_mir_transform/src/lib.rs:660`) rewrites coroutine
  constructions into state-machine struct Adt constructions,
  so the variants are also pass-discharged (per the
  `rustc_middle/src/mir/syntax.rs:1457-1458` comment "After
  coroutine lowering, `Coroutine` aggregate kinds are
  disallowed too"). Two independent rejection grounds.
- `Option<UserTypeAnnotationIndex>` slot in `Adt` --
  NLL/borrowck-only carrier. The required `CleanupPostBorrowck`
  pass clears the body-level annotation table
  (`rustc_mir_transform/src/cleanup_post_borrowck.rs:81-85`,
  `body.user_type_annotations.raw.clear()`); rustc codegen
  ignores the slot
  (`rustc_codegen_ssa/src/mir/rvalue.rs:670-672`). Same drop
  pattern as §20's drop of `user_ty` from `ConstOperand`.
- `(DefId, GenericArgsRef)` pair in `Adt` / `Closure` -- the
  `(item, args)` pair is already carried inside the
  destination `Ty` (per §10.0:
  `Ty::Struct { item, args, layout }` and analogous
  `Ty::Enum`, `Ty::Union`, `Ty::Closure`); a `TyId` resolving
  to one of those Ty variants reaches the same information
  through stage-3's type interner. Collapsing the pair into a
  single `TyId` avoids structural duplication.
- "Three-states-in-five-slots" Adt encoding -- rustc's
  `Adt(DefId, VariantIdx, GenericArgsRef, Option<UserTypeAnnotationIndex>, Option<FieldIdx>)`
  is split into three RegseqModel variants: `Struct(TyId)`,
  `Enum(TyId, VariantIdx)`, and `Union(TyId, FieldIdx)`. The
  type system enforces "active_field is Union-only" and
  "VariantIdx is Enum-only" without a runtime invariant; no
  representable state encodes a meaningless slot
  combination. This split aligns with the §10.0 / §12
  `NominalKind` taxonomy (`Struct`, `Enum`, `Union`, `FnDef`,
  `Closure`); `FnDef` has no construction operation and
  therefore no `AggregateKind` variant.

Closure semantics: `Closure(TyId)` references the closure's
*identity view* (`Ty::Closure(item, args)`); the captures
struct's *layout view* is reachable via the parallel
`find_nominal(NominalKind::Struct, item, args)` lookup
(per §13.1; same pattern §19.7 rule 3 uses for closure-place
field validation).

`RawPtr(TyId, Mutability)`: the `TyId` is the *pointee* type
(mirroring rustc's `RawPtr(Ty<'tcx>, Mutability)` shape per
`rustc_middle/src/mir/syntax.rs:1556-1569`). Operand layout:
position 0 is a thin raw pointer to the data; position 1 is
the metadata value. Both positions are required even for
thin pointers (where metadata is the unit value), per
`rustc_middle/src/mir/syntax.rs:1561-1566`.

### 21.4 Identity and storage

`Rvalue` is body-local content; it does not participate in
any top-level interning. There is no `RvalueId`. Equality
and hashing are structural over the variant payload.

Rvalues are constructed inline at use sites in
`Statement::Assign(Place, Rvalue)` payloads (defined at
the Statement vocabulary level). The owning struct stores
the `Rvalue` by value (no indirection); any boxing at the
parent body-content level is a parent-variant decision
made by the owning vocabulary, not a property of the
`Rvalue` surface itself. The `Rvalue` surface introduces
no top-level `Box` indirection beyond what individual
variants need internally (e.g., `Aggregate`'s
`Box<[Operand]>`).

Rvalue identity is per-build, structural, and does not
participate in the cross-build identity discipline (F4 /
F4-2, §16). Same scope reduction as `Place` (§19.2) and
`Operand` (§20.2).

### 21.5 Rvalue-site invariants

Two parts: rejection rules (rejection-on-encounter) and
positive invariants (well-formedness checks).

#### 21.5.1 Rejection rules

Constructs that cannot reach a well-formed `optimized_mir`
input are rejected on encounter, with a diagnostic naming
the discharging mechanism so the driver author sees a useful
error message:

| Construct | Rejection ground |
|---|---|
| `mir::Rvalue::CopyForDeref(_)` | Eliminated by required `EraseDerefTemps` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:658`; `is_required(): true` at `rustc_mir_transform/src/erase_deref_temps.rs:38`); rustc codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:712`). Not in optimized_mir. |
| `mir::Rvalue::ShallowInitBox(_, _)` | Eliminated by required `ElaborateBoxDerefs` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:659`; `is_required(): true` at `rustc_mir_transform/src/elaborate_box_derefs.rs:224`); rustc codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:713`). Not in optimized_mir. |
| `mir::Rvalue::ThreadLocalRef(_)` | DSL-surface rejection: `#[thread_local]` cannot produce a working driver on Linux/Zephyr targets (no TLS infrastructure exposed to driver code). |
| `mir::Rvalue::WrapUnsafeBinder(_, _)` | §15.1 (Type-level rejection table at line 4583) rejects `Ty::UnsafeBinder` at type translation; the rvalue-level rejection cascades by composition (the binder type the variant wraps cannot enter the IR). Additional grounds: gated behind unstable `unsafe_binders` feature; binder type has no C-shape; no v1 driver-corpus precedent (Charon and mrustc both lack the analog). |
| `mir::Rvalue::Ref(_, BorrowKind::Fake(_), _)` | NOPed by required `CleanupPostBorrowck` pass (NOP code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`). Fake assignments are statement-level Nops in optimized_mir. |
| `mir::Rvalue::Cast(CastKind::PointerCoercion(MutToConstPointer \| ArrayToPointer, _), _, _)` | Rewritten to `CastKind::PtrToPtr` by required `CleanupPostBorrowck` (rewrite code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`); rustc codegen also `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:448-452`). Not in optimized_mir. |
| `mir::Rvalue::Aggregate(box AggregateKind::Coroutine(_, _) \| CoroutineClosure(_, _), _)` | Type-level rejection in §15.1 (Coroutine / CoroutineClosure / CoroutineWitness rejected at type translation) plus runtime-lowering `coroutine::StateTransform` discharge (`rustc_mir_transform/src/lib.rs:660`). Two independent grounds. |

These rvalue-translation-phase rejections compose with the
§15.1 type-translation-phase rejections; the bridge applies
both classes during MIR construction.

#### 21.5.2 Positive invariants

Six rules, enforced at body-level walk time (deferred to
stage 11; see §21.5.3).

**I-Rvalue-Operand-Wellformed.** Every embedded `Operand`
satisfies the §20.5 invariants (I-Operand-Place-Validity,
inherited from §19.7); every embedded `Place` satisfies the
§19.7 place-validation rules (head-local existence, field /
variant index range, structural invariants, type-coherence
walk).

**I-Rvalue-TyId-Interned.** Every `TyId` slot resolves
through the §12 type interner: in `Cast(_, _, ty_id)`'s
destination type slot; in every `AggregateKind::*` variant's
TyId slot.

**I-Rvalue-Aggregate-Operand-Count.** The number of operands
in `Aggregate(kind, ops)` matches the destination type's
structural field-count, dispatched per `AggregateKind`
variant:

| `AggregateKind` variant | Required operand count |
|---|---|
| `Array(ty)` | `n` where the destination type resolves to `Ty::Array(_, n)` |
| `Tuple(ty)` | `elem_tys.len()` where the destination type resolves to `Ty::Tuple { fields: elem_tys, .. }` |
| `Struct(ty)` | the struct's field count: `Ty::Struct { layout: l, .. }` then `l.kind` is `LayoutKind::Struct` and the count is `l.kind.fields.len()` |
| `Enum(ty, vidx)` | the named variant's field count: dispatched via `Ty::Enum`'s `LayoutKind::Enum.shape`'s active variant's `VariantLayout.fields.len()` (per §11.1) |
| `Union(ty, _)` | exactly 1 (unions construct exactly the active field) |
| `Closure(ty)` | the captures-struct field count: dispatched via the parallel `Ty::Struct(item, args, _)` reached by `find_nominal(NominalKind::Struct, item, args)` (per §13.1 / §19.7 rule 3) |
| `RawPtr(ty, _)` | exactly 2 (`(data_ptr, metadata)`; both required even for thin pointers per `rustc_middle/src/mir/syntax.rs:1561-1566`) |

**I-Rvalue-Repeat-Count-Wellformed.** `Repeat(_, count)`'s
`count: u64` is a non-negative integer (trivially true; `u64`
is unsigned).

**I-Rvalue-VariantIdx-Wellformed.** For
`Aggregate(AggregateKind::Enum(ty_id, vidx), _)`, the
`VariantIdx vidx` is in range
`[0, variant_count(ty_id))`, where `variant_count`
dispatches on the `EnumShape` carried by the resolved
`Ty::Enum`'s layout (per §19.7 rule 4: `Empty -> 0`,
`Single { .. } -> 1`,
`Multiple { variants, .. } -> variants.len()`).

**I-Rvalue-FieldIdx-Wellformed.** For
`Aggregate(AggregateKind::Union(ty_id, fidx), _)`, the
`FieldIdx fidx` is in range
`[0, union_field_count(ty_id))`, where
`union_field_count` reads `LayoutKind::Union.fields.len()`
(per §11).

#### 21.5.3 Walk-implementation: deferred to stage 11

Rules enumerated at this stage; the walk implementation
applying them is deferred to stage 11's body-validation
step. Stage 11 owns the body-level walk that visits every
`Statement::Assign(Place, Rvalue)` and applies the rules
above in a single traversal of body content. Same split as
§19.7's `Place` validation and §20.5's operand-site
invariants, both of which inherit the walk via §3.3 Phase B
body validation.

#### 21.5.4 F4 / F4-2 cross-build identity: out of scope

`Rvalue` is body-local content; it does not participate in
the cross-build identity discipline (F4 / F4-2, §16). Same
scope reduction as `Place` (§19.2) and `Operand` (§20.2).
The F4-2 instance ordering canonicalises `(InstanceKey)`,
not `(BodyContents)`.

### 21.6 Bridge translation rules

Per-rustc-variant translation. The 9 RegseqModel `Rvalue`
variants plus the 7 reject-on-encounter rows (§21.5.1) cover
the full 13-variant rustc surface (`mir::Rvalue<'tcx>`) and
its sub-enum variants.

#### 21.6.1 Top-level `Rvalue` variants

| rustc form | RegseqModel form |
|---|---|
| `mir::Rvalue::Use(op)` | `Rvalue::Use(translate_operand(op))` |
| `mir::Rvalue::Repeat(op, ty_const)` | `Rvalue::Repeat(translate_operand(op), n)` where `n: u64` is the post-mono evaluated count of `ty_const` |
| `mir::Rvalue::Ref(_region, BorrowKind::Shared, p)` | `Rvalue::Ref(Provenance::SharedBorrow, translate_place(p))` |
| `mir::Rvalue::Ref(_region, BorrowKind::Mut { kind: _ }, p)` | `Rvalue::Ref(Provenance::UniqueBorrow, translate_place(p))` -- the `MutBorrowKind` sub-discriminator is collapsed (codegen also collapses it via `bk.to_mutbl_lossy()`, `rustc_codegen_ssa/src/mir/rvalue.rs:506-511`) |
| `mir::Rvalue::Ref(_region, BorrowKind::Fake(_), _)` | reject (§21.5.1) |
| `mir::Rvalue::ThreadLocalRef(_)` | reject (§21.5.1) |
| `mir::Rvalue::RawPtr(RawPtrKind::Const, p)` | `Rvalue::RawPtr(Mutability::Not, translate_place(p))` |
| `mir::Rvalue::RawPtr(RawPtrKind::Mut, p)` | `Rvalue::RawPtr(Mutability::Mut, translate_place(p))` |
| `mir::Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, p)` | `Rvalue::RawPtr(Mutability::Not, translate_place(p))` -- bridge collapse (see §21.6.2) |
| `mir::Rvalue::Cast(kind, op, ty)` | `Rvalue::Cast(translate_cast_kind(kind), translate_operand(op), translate(ty))` (see §21.6.3) |
| `mir::Rvalue::BinaryOp(op, box (lhs, rhs))` | `Rvalue::BinaryOp(op, translate_operand(lhs), translate_operand(rhs))` -- `op: BinOp` translates verbatim; the `Box<(Operand, Operand)>` indirection is unwrapped at translation time |
| `mir::Rvalue::UnaryOp(op, x)` | `Rvalue::UnaryOp(op, translate_operand(x))` -- `op: UnOp` translates verbatim |
| `mir::Rvalue::Discriminant(p)` | `Rvalue::Discriminant(translate_place(p))` |
| `mir::Rvalue::Aggregate(box kind, ops)` | `Rvalue::Aggregate(translate_aggregate_kind(kind), translate_operands(ops))` (see §21.6.4) |
| `mir::Rvalue::CopyForDeref(_)` | reject (§21.5.1) |
| `mir::Rvalue::ShallowInitBox(_, _)` | reject (§21.5.1) |
| `mir::Rvalue::WrapUnsafeBinder(_, _)` | reject (§21.5.1) |

#### 21.6.2 `RawPtrKind::FakeForPtrMetadata` collapse

When the bridge encounters
`mir::Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, p)`, it
does **not** preserve the variant. The collapse target is
`Mutability::Not` (i.e., `*const`), matching rustc codegen's
own choice: `RawPtrKind::to_mutbl_lossy()` at
`rustc_middle/src/mir/statement.rs:994-1003` maps
`FakeForPtrMetadata` to `Mutability::Not` ("we have no type
corresponding to a fake borrow, so use `*const` as an
approximation," per the inline comment at
`rustc_middle/src/mir/statement.rs:999-1000`); rustc codegen
applies this mapping at `rustc_codegen_ssa/src/mir/rvalue.rs:513-515`
when synthesising the result type via `Ty::new_ptr(tcx, ty,
kind.to_mutbl_lossy())`. The syntax.rs:189-197 doc comment
"operationally equivalent to `RawPtrKind::Mut`, but will never
incur a retag" describes Miri retag semantics (a borrow-stack
abstract-machine concept); rustc's actual codegen uses the
`to_mutbl_lossy()` mapping to `Not`. RegseqModel adopts the
codegen target.

This is also Charon's choice (Charon's `FakeForPtrMetadata`
translates to `Shared` / read-only), peer-validating that
read-only is the correct collapse target
when the C/output-language has no fake-borrow representation.
Drivers that read slice length or vtable metadata via
`<*const [T]>::len()` / `<*const dyn Trait>::metadata()`
generate `FakeForPtrMetadata` rvalues; the bridge collapses
to `Mutability::Not` and produces a `*const`-typed C result,
which is what rustc codegen would also produce. See
`discussions.md` D-7-1.5 for the reject-vs-collapse trade-off
and the peer comparison.

#### 21.6.3 `CastKind` translation

| rustc form | RegseqModel form |
|---|---|
| `CastKind::PointerExposeProvenance` | `CastKind::PointerExposeProvenance` |
| `CastKind::PointerWithExposedProvenance` | `CastKind::PointerWithExposedProvenance` |
| `CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer(_safety), _coerce_src)` | `CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer)` -- safety dropped (§21.2.5); CoercionSource dropped (§21.2.5) |
| `CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer, _coerce_src)` | `CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer)` |
| `CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer(_safety), _coerce_src)` | `CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer)` -- safety dropped (§21.2.5) |
| `CastKind::PointerCoercion(PointerCoercion::Unsize, _coerce_src)` | `CastKind::PointerCoercion(PointerCoercion::Unsize)` |
| `CastKind::PointerCoercion(PointerCoercion::MutToConstPointer, _)` | reject (§21.5.1) |
| `CastKind::PointerCoercion(PointerCoercion::ArrayToPointer, _)` | reject (§21.5.1) |
| `CastKind::IntToInt` | `CastKind::IntToInt` |
| `CastKind::FloatToInt` | `CastKind::FloatToInt` |
| `CastKind::FloatToFloat` | `CastKind::FloatToFloat` |
| `CastKind::IntToFloat` | `CastKind::IntToFloat` |
| `CastKind::PtrToPtr` | `CastKind::PtrToPtr` |
| `CastKind::FnPtrToPtr` | `CastKind::FnPtrToPtr` |
| `CastKind::Transmute` | `CastKind::Transmute` |
| `CastKind::Subtype` | `CastKind::Subtype` |

The `CoercionSource` slot inside every `PointerCoercion(_, _)`
is discarded at translation (diagnostic-only; codegen ignores
it; §21.2.5).

The `hir::Safety` slot inside `ReifyFnPointer(_)` /
`ClosureFnPointer(_)` is discarded at translation (the
result safety is encoded in the destination `Ty::FnPtr`'s
`FnSig.safety` per §14.3; §21.2.5).

#### 21.6.4 `AggregateKind` translation

| rustc form | RegseqModel form |
|---|---|
| `AggregateKind::Array(elem_ty)` | `AggregateKind::Array(intern_or_lookup(Ty::Array(translate(elem_ty), operand_count)))` -- rustc carries the *element* type (`rustc_middle/src/mir/syntax.rs:1541`); the bridge constructs the destination `Ty::Array(elem, n)` where `n` is the rvalue's operand count (rustc's `Aggregate(Array(_), ops)` carries one operand per element) and interns via §12 |
| `AggregateKind::Tuple` | `AggregateKind::Tuple(translate(rvalue.ty(body, tcx)))` -- rustc's `Aggregate(Tuple, ops)` does not carry a type slot; the bridge obtains the destination tuple type via `rvalue.ty(body, tcx)` (rustc's MIR Rvalue type-of helper). Translation routes through stage-3 `Ty::Tuple { fields, layout }` interning where `fields = elem_tys` is the translated tuple element list and `layout` is sourced from `tcx.layout_of` per §15 (parallel to stage-3's tuple translation rule at `spec.md:4530`) |
| `AggregateKind::Adt(did, _vidx, args, _user_ty, _afi)` with `adt_def.adt_kind() == Struct` | `AggregateKind::Struct(intern(Ty::Struct { item: translate(did), args: translate(args), layout }))` |
| `AggregateKind::Adt(did, vidx, args, _user_ty, _afi)` with `adt_def.adt_kind() == Enum` | `AggregateKind::Enum(intern(Ty::Enum { item, args, layout }), vidx)` |
| `AggregateKind::Adt(did, _vidx, args, _user_ty, Some(field_idx))` with `adt_def.adt_kind() == Union` | `AggregateKind::Union(intern(Ty::Union { item, args, layout }), field_idx)` |
| `AggregateKind::Closure(did, args)` | `AggregateKind::Closure(intern(Ty::Closure(translate(did), translate(args))))` -- identity view (§13.1) |
| `AggregateKind::Coroutine(_, _)` | reject (§21.5.1) |
| `AggregateKind::CoroutineClosure(_, _)` | reject (§21.5.1) |
| `AggregateKind::RawPtr(pointee_ty, mutbl)` | `AggregateKind::RawPtr(translate(pointee_ty), mutbl)` |

The `_user_ty: Option<UserTypeAnnotationIndex>` slot is
discarded (NLL/borrowck-only carrier; §21.2.9). The
`(did, args)` pair collapses through stage-3 type interning
to a `TyId` referencing the appropriate `Ty::Struct` /
`Ty::Enum` / `Ty::Union` / `Ty::Closure` variant per the
§10.0 / §15 nominal translation rules. For `Adt` the rustc
five-slot encoding is split at translation time into the
three RegseqModel variants `Struct` / `Enum` / `Union`
based on `adt_def.adt_kind()`; structural slots that are
meaningless for a given Adt kind (e.g., a non-default
`VariantIdx` on a struct) are discarded.

### 21.7 Worked examples

Five representative examples illustrating the most-used
variants and the principal translation collapses.

#### 21.7.1 Binary arithmetic

**Rust source:**
```rust
let s = a + b;       // a, b: u32
```

**rustc MIR (post-cleanup, illustrative):**
```text
_3 = Add(copy _1, copy _2)
```

**RegseqModel form:**
```text
_3 = BinaryOp(Add, Copy(_1), Copy(_2))
```

`BinaryOp` operands stored flat (no `Box<(Operand, Operand)>`
indirection); operands are stage-6 `Operand::Copy(Place)`
referencing stage-5 `Place { local: LocalId, projection: [] }`.

#### 21.7.2 Shared borrow with field access

**Rust source:**
```rust
let r: &u32 = &x.field;       // x: MyStruct, MyStruct.field: u32
```

**rustc MIR (post-cleanup, illustrative):**
```text
_3 = &_1.field
```

**RegseqModel form:**
```text
_3 = Ref(SharedBorrow, _1.field)
```

The `Place` payload uses stage-5's projection chain (head
local `_1`, projection `[Field(field_idx, t_u32)]`). Rustc's
`Region<'tcx>` slot from `Ref(Region, BorrowKind, Place)` is
dropped (§21.2.3); the `BorrowKind::Shared` arm collapses to
`Provenance::SharedBorrow`.

#### 21.7.3 Struct construction

**Rust source:**
```rust
let s = MyStruct { a: x, b: y };       // a, b: u32
```

**rustc MIR (post-cleanup, illustrative):**
```text
_3 = Aggregate(Adt(MyStruct_did, 0, args, None, None),
               [move _1, move _2])
```

**RegseqModel form:**
```text
_3 = Aggregate(Struct(t_MyStruct), [Move(_1), Move(_2)])
```

`AggregateKind::Adt(_, 0, _, None, None)` with
`adt_kind() == Struct` becomes
`AggregateKind::Struct(t_MyStruct)`, where `t_MyStruct` is
the interned `TyId` resolving to
`Ty::Struct { item, args, layout }` (per §10.0). Operand
count matches the struct's field count (rule
**I-Rvalue-Aggregate-Operand-Count**, §21.5.2).

#### 21.7.4 Enum variant construction

**Rust source:**
```rust
let r: Option<u32> = Some(x);
```

**rustc MIR (post-cleanup, illustrative):**
```text
_2 = Aggregate(Adt(Option_did, 1 /* Some */, args, None, None),
               [move _1])
```

**RegseqModel form:**
```text
_2 = Aggregate(Enum(t_Option_u32, vidx_Some), [Move(_1)])
```

`AggregateKind::Adt(_, vidx, _, None, None)` with
`adt_kind() == Enum` becomes
`AggregateKind::Enum(t_Option_u32, vidx_Some)`. The
`VariantIdx` is preserved (rule
**I-Rvalue-VariantIdx-Wellformed**, §21.5.2).

#### 21.7.5 Cast: integer truncation

**Rust source:**
```rust
let x: u8 = (n as u8);       // n: u32
```

**rustc MIR (post-cleanup, illustrative):**
```text
_2 = (copy _1) as u8 (IntToInt)
```

**RegseqModel form:**
```text
_2 = Cast(IntToInt, Copy(_1), t_u8)
```

`Cast(CastKind::IntToInt, _, _)` carries no
`CoercionSource` or `hir::Safety` slots (those drops apply
to `PointerCoercion` variants); the destination `TyId` is
the interned `Ty::Uint(U8)`.

### 21.8 Rendering

See `pretty-printer.md` §7.3.

---


## 22. Statement vocabulary

The `Statement` vocabulary is the sub-terminator
instruction surface of MIR -- the typed expression of "an
in-block side-effect that is not a control-flow transfer."
The bridge fills it from rustc's `mir::StatementKind<'tcx>`;
downstream consumers (stage 10 basic blocks --
`BasicBlock.statements: Box<[Statement]>` per §24.1.2;
stage 11 bodies; stage 14 backends) read it as the
per-block straight-line sequence preceding the
terminator.

### 22.1 `Statement` struct and `StatementKind` enum

#### 22.1.1 `Statement` wrapper struct

```rust
/// A sub-terminator instruction in a basic block, with
/// the source location it derives from. Body-local
/// content; lives inline inside basic-block storage,
/// not in any top-level interner table. Equality is
/// structural.
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
```

The wrapper carries a single `Span` field (12 bytes per
§5.3, no `SpanId` interning since stage 1 rejected span-
deduplication). Rustc's `SourceInfo { span, scope }` is
collapsed to span-only: the `scope` field is debuginfo-
only (it indexes into `body.source_scopes` for DWARF
lexical-scope display), with no C-shape meaning, and is
dropped at the bridge translation site.

Rustc's `Statement.debuginfos: StmtDebugInfos<'tcx>` field
(per-statement DWARF instrumentation hints like
`StmtDebugInfo::AssignRef(Local, Place)` for tracking
reference assignments to ssa-locals, `InvalidAssign(Local)`
for poison markers; defined at
`rustc_middle/src/mir/statement.rs:21, 1015-1079`) is
dropped entirely. RegseqModel does not emit DWARF; the C
backend produces C source which gets compiled by
clang/gcc with their own debug-info pipelines. (The
distinct body-level `var_debug_info: Vec<VarDebugInfo>`
field at `rustc_middle/src/mir/mod.rs:264, 1235-1257`,
which carries user-source variable name mappings, is a
*stage-11* `Body` design concern -- the named-local
preservation requirement is filed as an outbound
expectation to stage 11.)

Charon's novel `comments_before: Vec<String>` field
(Aeneas source-fidelity for formal-verification
artefacts) has no analog in RegseqModel: the C-emission
pipeline does not preserve source-comment fidelity for
proof artefacts.

Field order is `kind, span` (matches rustc_public's
analog at `rustc_public/src/mir/body.rs:470-474`, the
closest existing `'tcx`-free owned-IR mirror for our
design space).

#### 22.1.2 `StatementKind` enum definition

```rust
/// A typed sub-terminator instruction. Body-local
/// content; appears inside `Statement.kind`.
pub enum StatementKind {
    /// In-place value assignment: `place := rvalue`.
    /// Does NOT call drop on the previous LHS value
    /// (drop is the explicit `Drop` terminator's job
    /// at stage 9). The LHS place type and the RHS
    /// rvalue's resulting type must be equal modulo
    /// lifetime erasure (§22.5.2 I-Statement-Assign-
    /// Type-Equality).
    Assign(Place, Rvalue),

    /// In-place enum tag-write: writes the discriminant
    /// for the named variant to the place, without
    /// touching variant payload fields. Matches rustc's
    /// `SetDiscriminant`: tag-only mutation; payload is
    /// independently set by separate `Assign` statements.
    /// The target place must have type
    /// `Ty::Enum { item, args, layout }` per §10
    /// (§22.5.2 I-Statement-SetDiscriminant-Target);
    /// the variant index must be in-range
    /// (§22.5.2 I-Statement-SetDiscriminant-VariantIdx).
    SetDiscriminant(Place, VariantIdx),

    /// Non-diverging rustc-internal hint primitive: an
    /// `Assume` (boolean hint to the optimizer that the
    /// operand is true) or a `CopyNonOverlapping`
    /// (inline non-overlapping byte copy without a
    /// terminator boundary). Distinct from regseq DSL
    /// semantic intrinsics, which flow through Calls
    /// (per Q7) -- this variant carries only rustc's
    /// own non-diverging primitives. See §22.3.1 for
    /// the closed sub-enum.
    Intrinsic(NonDivergingIntrinsic),
}
```

The three variants are exhaustive: a rustc
`mir::StatementKind<'tcx>` reaching the bridge after the
required + whitelisted cleanup-pass discharge (§22.5.1)
canonicalises into exactly one variant, or the bridge
rejects with a diagnostic (§22.5.1 reject-on-encounter
table). There is no `Other` / `Opaque` arm.

Sizing (reference):
- `Place` ~20 bytes (§19.2).
- `Rvalue` worst-case ~64 bytes (§21.1, dominated by
  `BinaryOp`).
- `Operand` ~24 bytes (§20.1).
- `Assign(Place, Rvalue)` flat ~84 bytes; dominates the
  enum.
- `Statement` worst-case ~96 bytes including span and
  discriminator.

#### 22.1.3 Variants dropped vs rustc

Of rustc's 13-variant `mir::StatementKind<'tcx>`
(`rustc_middle/src/mir/syntax.rs:311-471`), 10 distinct
source variants do not appear in RegseqModel; each is
rejected on encounter (§22.5.1) with a diagnostic naming
the discharging mechanism. The dropped variants split
into three groups by discharge mechanism:

**Group 1 -- `CleanupPostBorrowck` NOPed (then swept by
`RemovePlaceMention` or `RemoveStorageMarkers`):**

- `FakeRead(_, _)` -- NOPed at
  `rustc_mir_transform/src/cleanup_post_borrowck.rs:40`
  (`is_required(): true` at line 89). Documented as
  "Disallowed after drop elaboration"
  (`rustc_middle/src/mir/syntax.rs:357`). Pure NLL
  borrowck artefact.
- `AscribeUserType(_, _)` -- NOPed at
  `cleanup_post_borrowck.rs:33`. Documented as
  "Disallowed after drop elaboration"
  (`rustc_middle/src/mir/syntax.rs:426`). Pure user-type-
  ascription tracking; cleared alongside
  `body.user_type_annotations` at
  `cleanup_post_borrowck.rs:81-85` and
  `LocalDecl.user_ty` at `cleanup_post_borrowck.rs:83-85`.
- `BackwardIncompatibleDropHint { .. }` -- NOPed at
  `cleanup_post_borrowck.rs:41`. Lint-only marker
  ("semantically equivalent to `Nop`,"
  `rustc_middle/src/mir/syntax.rs:462-463`); rustc_public
  drops the variant entirely
  (`rustc_public/src/mir/body.rs:476-490` only carries
  12 variants).
- `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))` --
  whole `Assign` NOPed at
  `cleanup_post_borrowck.rs:34`. Compositional with
  stage-7 `Rvalue::Ref` rejection of `BorrowKind::Fake`
  (§21.5.1 row 5).
- `Coverage(BlockMarker | SpanMarker)` -- NOPed at
  `cleanup_post_borrowck.rs:35-39`. Mid-pipeline coverage
  markers used during MIR-building, not needed after
  `InstrumentCoverage`.

**Group 2 -- always-running cleanup-pass strip:**

- `PlaceMention(_)` -- removed by required `RemovePlaceMention`
  (`rustc_mir_transform/src/remove_place_mention.rs:18`;
  `is_required(): true` at line 25; `is_enabled` returns
  `!sess.opts.unstable_opts.mir_preserve_ub` which is
  off by default so the pass runs). Pure `let _ =
  PLACE;` desugar artefact.
- `Nop` -- swept by both `RemovePlaceMention`
  (`remove_place_mention.rs:18`) and `RemoveStorageMarkers`
  (`remove_storage_markers.rs:20`). Only present
  transiently after a `make_nop` call; not in
  optimized_mir.
- `StorageLive(_)` -- removed by `RemoveStorageMarkers`
  (`rustc_mir_transform/src/remove_storage_markers.rs:18`).
  The pass's `is_enabled = mir_opt_level > 0 && !sess.emit_lifetime_markers()`
  (`remove_storage_markers.rs:10-12`) would gate it off
  at our default `mir_opt_level=0`, BUT the bridge
  driver flips it on via `mir_enable_passes`
  (`crates/regseq-driver/src/callbacks.rs:27` registers
  `("RemoveStorageMarkers", true)`). The pass-manager
  override mechanism at
  `rustc_mir_transform/src/pass_manager.rs:227-238`
  consults `mir_enable_passes` first; the named-pass
  polarity (`true` here) is respected, bypassing the
  `is_enabled` gate. Therefore `RemoveStorageMarkers`
  runs in our pipeline; `StorageLive` / `StorageDead`
  do not reach optimized_mir.
- `StorageDead(_)` -- same as `StorageLive(_)`.

**Group 3 -- out-of-pipeline (build-flag-gated; never
injected for our compilation mode):**

- `Retag(_, _)` -- only emitted by `AddRetag`
  (`rustc_mir_transform/src/add_retag.rs:85, 118, 177`)
  which `is_enabled` requires `-Z mir-emit-retag`. The
  bridge driver does not enable that flag; the variant
  cannot appear.
- `ConstEvalCounter` -- only injected by `CtfeLimit`
  (`rustc_mir_transform/src/ctfe_limit.rs:36`) for
  const-eval-bound bodies (`mir_for_ctfe`); driver
  bodies are run-time bodies (`optimized_mir`) and do
  not receive the injection.
- `Coverage(non-marker variants)` -- only emitted by
  `InstrumentCoverage` under `-C instrument-coverage`;
  driver builds do not enable coverage instrumentation.
  After the `BlockMarker | SpanMarker` arms are NOPed
  at `CleanupPostBorrowck` (Group 1 above), the
  remaining `Coverage(...)` content cannot appear
  unless the user explicitly asks for instrumentation,
  which is not part of our compilation contract.

These statement-translation-phase rejections compose
with the §15.1 type-translation-phase rejections and
the §21.5.1 rvalue-translation-phase rejections; the
bridge applies all classes during MIR construction.

(See `discussions.md` D-8-1 / D-8-1.1 for the cluster
audit including the cleanup-pass-discharge trace and
the closed surface options considered.)

### 22.2 Per-variant payload shapes

#### 22.2.1 `Assign(Place, Rvalue)`

In-place value assignment. Storage is flat (place and
rvalue inline, not boxed) -- a deliberate divergence
from rustc's `Box<(Place, Rvalue)>` packaging
(`rustc_middle/src/mir/syntax.rs:341`). Rustc boxes to
keep its `StatementKind<'_>` enum at the 16-byte size
assert (`rustc_middle/src/mir/syntax.rs:1753`);
RegseqModel does not assert a comparable size budget
at the statement surface. The flat shape is congruent
with rustc_public's `Assign(Place, Rvalue)`
(`rustc_public/src/mir/body.rs:478`), Charon's
`Assign(Place, Rvalue)` (per
`stage-8/prior-art/charon.md`), mrustc's flat
`Assign { dst, src }` (per `stage-8/prior-art/mrustc.md`),
and stage-7's flat-`BinaryOp(BinOp, Operand, Operand)`
pattern (flat `BinaryOp(BinOp, Operand, Operand)`).

`Assign` does NOT call drop on the previous LHS value:
"Assign statements roughly correspond to an assignment
in Rust proper (`x = ...`) except without the
possibility of dropping the previous value (that must
be done separately, if at all)"
(`rustc_middle/src/mir/syntax.rs:312-317`). Drop is the
explicit `Drop` terminator's job at stage 9 (when stage
9 lands; the terminator vocabulary for v1 will carry
explicit drop-glue calls per the bridge translation
discipline). No drop-suppression flag on the variant;
compositional with stage-9 drop terminator placement,
not a parallel side-channel.

The well-formedness rule "Assignments in which the
types of the place and rvalue differ are not
well-formed" (`rustc_middle/src/mir/syntax.rs:324`)
is enforced as positive invariant
I-Statement-Assign-Type-Equality (§22.5.2); rustc's
own validator pins it
(`rustc_mir_transform/src/validate.rs:1467-1582`);
walk-enforcement deferred to stage 11.

C-semantics: `<lhs> = <rhs>;` for value assignments;
or compound-literal initialisation when the rvalue is
an `Aggregate`; or runtime-helper call for unsizing /
transmute casts (per §21.2.5's `Cast` lowering). Backends select
the C form per the rvalue's variant and types; the
bridge does not pre-select.

(See `discussions.md` D-8-1.2 for the audit.)

#### 22.2.2 `SetDiscriminant(Place, VariantIdx)`

In-place enum tag-write. The variant writes the
discriminant for the named variant to the target
place's tag location, without touching variant payload
fields. Matches rustc's documented semantics: "Write
the discriminant for a variant to the enum Place...
This does not necessarily write to the entire place;
instead, it writes to the minimum set of bytes as
required by the layout for the type"
(`rustc_middle/src/mir/syntax.rs:362-364`).

Storage is flat tuple (no struct-with-named-fields, no
Box on `Place`) -- mirrors stage-7's flat-tuple pattern
for small 2-3-field payloads (§21.1's
`Ref(Provenance, Place)`, `RawPtr(Mutability, Place)`,
and `Aggregate(AggregateKind, Box<[Operand]>)` are
all flat tuples). Charon's `SetDiscriminant(Place,
VariantId)` (per `stage-8/prior-art/charon.md`) is the
direct precedent. rustc_public's
`SetDiscriminant { place, variant_index }`
(`rustc_public/src/mir/body.rs:480`) uses struct-with-
flat-place; the choice is stylistic-only and the
RegseqModel project leans tuple per stage-7 precedent.

Target type: the `Place` argument must have an enum
type. Rustc's validator at
`rustc_mir_transform/src/validate.rs:1551-1567` allows
`place_ty(p).kind() ∈ { ty::Adt(..), ty::Coroutine(..),
ty::Alias(ty::Opaque, ..) }` but RegseqModel's
post-translation surface narrows this further:

- `ty::Coroutine(..)` is rejected at stage 3 §15.1
  (the type-translation-phase rejection table); the
  variant cannot reach optimized_mir with a
  coroutine-typed target.
- `ty::Alias(ty::Opaque, ..)` is normalized away by
  the required `PostAnalysisNormalize` pass
  (`rustc_mir_transform/src/lib.rs:642`); opaque
  types resolve to their underlying type before
  optimized_mir.
- `ty::Adt(..)` covers RegseqModel's three direct
  Adt-shaped Ty variants `Ty::Struct{..}`,
  `Ty::Enum{..}`, `Ty::Union{..}` per §10. Only
  `Ty::Enum` has a discriminant to write;
  `Ty::Struct` and `Ty::Union` have no tag location.

The bridge rule: `place_ty(p) = Ty::Enum { item, args,
layout }` per §10. Other shapes (`Ty::Struct{..}`,
`Ty::Union{..}`, `Ty::Tuple{..}`, primitive types,
references, raw pointers, function types, etc.) reject
at the bridge per the I-Statement-SetDiscriminant-
Target invariant (§22.5.2). Compositional with stage-3
type taxonomy, not a parallel `target_kind: Enum`
flag.

The variant carries no redundant Item / Layout slot;
the target type is fully recoverable from `place_ty(p)`
via stage-3 / stage-5 / §10 vocabulary at render /
codegen time (matches stage-7 Aggregate's TyId-lookup-
via-§12-type-interner pattern).

C-semantics: tag-write at the enum's C representation.
The discriminant field location depends on the chosen
Rust ADT layout per stage-3 §13; the tag bytes written
come from `place_ty(p)`'s `LayoutKind::Enum.shape`'s
`VariantLayout.tag_value: Option<u128>` for the
named variant (per §11.1's `tag_value` table). When
`tag_value` is `Some(v)`, codegen emits the
`VariantLayout.tag_value` bytes at the
`TagScalar.offset` location in the enum's C layout;
when `tag_value` is `None` (e.g., niche-encoded
untagged variants where the payload IS the tag, or
single-variant enums without a tag scalar),
`SetDiscriminant` is a codegen no-op -- the tag is
either implicit in the data being written by
companion `Assign` statements or absent entirely.
This is distinct from rustc's semantic-level
`VariantLayout.discriminant` (the value
`Discriminant(place)` returns at stage-7 Rvalue
level). Distinct also from full enum construction
via `Rvalue::Aggregate(AggregateKind::Enum(...))`
which writes both the tag and all payload fields in
one atomic place-write; `SetDiscriminant` writes the
tag in place over an already-constructed (or
to-be-constructed) payload.

(See `discussions.md` D-8-1.3 for the audit including
the struct-vs-tuple precedent reasoning.)

#### 22.2.3 `Intrinsic(NonDivergingIntrinsic)`

Non-diverging rustc-internal hint primitive. The
nested sub-enum `NonDivergingIntrinsic` (§22.3.1)
carries the two arms `Assume(Operand)` (boolean hint
to the optimizer) and `CopyNonOverlapping(...)` (inline
non-overlapping byte copy). Rustc's design rationale
for keeping these as Statement-level rather than as
Calls: "this avoids adding a new block and a terminator
for simple intrinsics"
(`rustc_middle/src/mir/syntax.rs:448-451`).

Storage is flat (no `Box` on the `NonDivergingIntrinsic`
arm) -- divergence from rustc's `Intrinsic(Box<NonDivergingIntrinsic>)`
(`rustc_middle/src/mir/syntax.rs:450`). Same flat-vs-
Box reasoning as §22.2.1's `Assign`: rustc's Box
serves the 16-byte size budget which RegseqModel does
not inherit. rustc_public's `Intrinsic(NonDivergingIntrinsic)`
(`rustc_public/src/mir/body.rs:487`) is the direct
precedent.

The nested-vs-fanout choice: keep the
`NonDivergingIntrinsic` sub-enum nested (rustc /
rustc_public precedent), rather than fan out the two
arms into top-level Statement variants `Assume(...)` /
`CopyNonOverlapping(...)` (a hypothetical Charon-style
approach -- though Charon actually transforms the
arms: `Assume` lifts to `Assert{on_failure:
UndefinedBehavior}`; `CopyNonOverlapping` becomes its
own dedicated boxed Statement variant -- per
`stage-8/prior-art/charon.md`).

Reasoning for nested:
- Preserves rustc's "non-diverging hint primitive"
  semantic grouping: both sub-variants share the
  property of being rustc-internal hints that don't
  return a value to a place (Assume) or do a fixed-
  shape memory op without divergence
  (CopyNonOverlapping). The grouping has meaning.
- Keeps the closed Statement surface at 3 variants;
  fanning out would expand to 4 without semantic gain.
- Future-proofs against new `NonDivergingIntrinsic`
  sub-variants. Rustc may add more non-diverging hint
  intrinsics; with nested, they land as sub-enum
  extensions; with fanout, they would land as
  Statement-enum changes (a larger blast radius for
  the spec).

This Intrinsic variant is **distinct from regseq DSL
semantic intrinsics**: regseq DSL surface intrinsics
(MMIO read/write, register-bank access, etc.) flow
through `TerminatorKind::Call` per Q7 (intrinsic
recognition by `#[rseq::intrinsic("NAME")]` attribute);
they do NOT appear in `Statement::Intrinsic`. The
Statement-level `Intrinsic` variant carries only
rustc's own `NonDivergingIntrinsic` primitives.

C-emission of `Statement::Intrinsic` variants
(`NonDivergingIntrinsic::Assume` -- backend-specific assume
builtin; `NonDivergingIntrinsic::CopyNonOverlapping{src, dst,
count}` -- `memcpy`-family emission with the IR-side `src` /
`dst` field naming preserved so backends consume by name) is
backend-codegen content; see `generic-c-backend.md` §6 for
the generic-C target.

(See `discussions.md` D-8-1.4 for the audit including
the nested-vs-fanout reasoning and the lift-to-Call
rejection.)

### 22.3 Sub-enums and sub-structs

#### 22.3.1 `NonDivergingIntrinsic` enum

```rust
/// The non-diverging rustc-internal hint primitives
/// that survive into RegseqModel as Statement-level
/// content. Closed 2-variant set; the bridge rejects
/// any rustc `mir::NonDivergingIntrinsic<'tcx>` arm
/// outside this set (current rustc has only the two
/// arms; future additions land as new sub-enum
/// variants).
pub enum NonDivergingIntrinsic {
    /// Boolean hint: "the operand is true." Backends
    /// may use this to backtrack the boolean's
    /// derivation and elide subsequent bound checks.
    /// If the operand is false, semantics matches
    /// `TerminatorKind::Unreachable`.
    Assume(Operand),

    /// Inline non-overlapping byte copy:
    /// `*dst[i] := *src[i]` for `i ∈ [0, count)`.
    /// `src` and `dst` are pointers (or references);
    /// `count` is `usize`. The src and dst regions
    /// must not overlap (caller obligation; not
    /// checked by the operation).
    CopyNonOverlapping(CopyNonOverlapping),
}
```

The 2 variants mirror rustc's
`mir::NonDivergingIntrinsic<'tcx>`
(`rustc_middle/src/mir/syntax.rs:485-508`) verbatim,
modulo the lifetime parameter erasure.

#### 22.3.2 `CopyNonOverlapping` struct

```rust
/// Payload for `NonDivergingIntrinsic::CopyNonOverlapping`.
/// Named-fields struct (NOT a tuple) -- src/dst/count
/// are all `Operand` and order-confusable in a tuple
/// shape; named fields prevent silent wrong-direction
/// copy bugs at the bridge translation site and the
/// stage-14 codegen site. This is the one place where
/// named-fields-vs-tuple is correctness-essential
/// (compare §22.2.2's `SetDiscriminant(Place,
/// VariantIdx)` flat tuple where the two payload
/// elements have distinct types and are not order-
/// confusable).
pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}
```

Mirrors rustc's
`mir::CopyNonOverlapping<'tcx> { src, dst, count }`
(`rustc_middle/src/mir/syntax.rs:633`) and
rustc_public's `CopyNonOverlapping`
(`rustc_public/src/mir/body.rs:457-462`).

The per-variant operand-type invariants (Assume's
`Operand: Ty::Bool`, CopyNonOverlapping's pointer-
pointer-usize rules) are pinned in §22.5.2's
I-Statement-Intrinsic-* invariants.

### 22.4 Identity and storage

`Statement` is body-local content; it does not
participate in any top-level interning. There is no
`StatementId`. Equality and hashing are structural over
the wrapper struct's `(kind, span)` pair.

Statements live inline in stage-10's basic-block
storage (`BasicBlock.statements: Box<[Statement]>`
per §24.1.2 stage-10 D-10-1.2 lock; finalized
single-fill payload aligning with the stage-7
`Aggregate(Box<[Operand]>)` and stage-9
`SwitchTargets.cases: Box<[(u128, BlockId)]>`
internal pattern). The owning struct stores
statements by value (no indirection); any boxing at
the parent body-content level is a parent-variant
decision made by the owning vocabulary, not a
property of the `Statement` surface itself. The
`Statement` surface introduces no top-level `Box`
indirection.

Statement identity is per-build, structural, and does
not participate in the cross-build identity discipline
(F4 / F4-2, §16). Same scope reduction as `Place`
(§19.2), `Operand` (§20.2), and `Rvalue` (§21.4). The
F4-2 instance ordering canonicalises `(InstanceKey)`,
not `(BodyContents)`.

(See `discussions.md` D-8-2 for the audit.)

### 22.5 Statement-site invariants

Two parts: rejection rules (rejection-on-encounter)
and positive invariants (well-formedness checks).

#### 22.5.1 Rejection rules

Constructs that cannot reach a well-formed
`optimized_mir` input under our compilation contract
are rejected on encounter, with a diagnostic naming
the discharging mechanism so the driver author sees
a useful error message:

| Construct | Rejection ground |
|---|---|
| `mir::StatementKind::FakeRead(_, _)` | NOPed by required `CleanupPostBorrowck` (NOP code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:40`; `is_required(): true` at line 89). The resulting `Nop` is then swept by required `RemovePlaceMention` (`rustc_mir_transform/src/remove_place_mention.rs:18-20`) and/or whitelisted `RemoveStorageMarkers` (`remove_storage_markers.rs:18-20`). Pure NLL borrowck artefact; "Disallowed after drop elaboration" per `rustc_middle/src/mir/syntax.rs:357`. |
| `mir::StatementKind::AscribeUserType(_, _)` | NOPed at `cleanup_post_borrowck.rs:33`. Pure user-type-ascription tracking; "Disallowed after drop elaboration" per `rustc_middle/src/mir/syntax.rs:426`. Cleared alongside `body.user_type_annotations` and `LocalDecl.user_ty` at `cleanup_post_borrowck.rs:81-85`. |
| `mir::StatementKind::BackwardIncompatibleDropHint { .. }` | NOPed at `cleanup_post_borrowck.rs:41`. Lint-only marker semantically equivalent to `Nop` per `rustc_middle/src/mir/syntax.rs:462-463`; rustc_public drops the variant entirely (`rustc_public/src/mir/body.rs:476-490` carries 12 variants without it). |
| `mir::StatementKind::Coverage(BlockMarker | SpanMarker)` | NOPed at `cleanup_post_borrowck.rs:35-39`. Mid-pipeline coverage markers used during MIR-building; not needed after `InstrumentCoverage`. |
| `mir::StatementKind::Assign(_, mir::Rvalue::Ref(_, BorrowKind::Fake(_), _))` | Whole `Assign` NOPed at `cleanup_post_borrowck.rs:34`. Compositional with §21.5.1 row 5 (rvalue-side `BorrowKind::Fake` rejection). |
| `mir::StatementKind::PlaceMention(_)` | Removed by required `RemovePlaceMention` (`rustc_mir_transform/src/remove_place_mention.rs:18`; `is_required(): true` at line 25; `is_enabled` returns `!sess.opts.unstable_opts.mir_preserve_ub` which is off by default). Pure `let _ = PLACE;` desugar artefact. |
| `mir::StatementKind::Nop` | Swept by required `RemovePlaceMention` (`remove_place_mention.rs:18`), whitelisted `RemoveStorageMarkers` (`remove_storage_markers.rs:20`), and whitelisted `SimplifyLocals-final` (`simplify.rs:580`; the SimplifyLocals retain filter excludes `StatementKind::Nop`). Whitelisted `RemoveZsts` (`crates/regseq-driver/src/callbacks.rs:28`) ALSO produces fresh `Nop`s by calling `statement.make_nop(true)` at `rustc_mir_transform/src/remove_zsts.rs:143`; those are then swept by `SimplifyLocals-final` later in the pipeline (`RemoveZsts` is registered in the optimization pass list at `rustc_mir_transform/src/lib.rs:728-730`; `SimplifyLocals::Final` runs further down the same pass list at `rustc_mir_transform/src/lib.rs:775`, after the post-`RemoveZsts` Nops have been emitted). Only transient inside the pipeline; not in the optimized_mir we consume. |
| `mir::StatementKind::StorageLive(_)` | Removed by `RemoveStorageMarkers` (`rustc_mir_transform/src/remove_storage_markers.rs:18`). The pass is force-on via the bridge driver's `mir_enable_passes` whitelist (`crates/regseq-driver/src/callbacks.rs:27` registers `("RemoveStorageMarkers", true)`); the pass-manager override at `rustc_mir_transform/src/pass_manager.rs:227-238` consults `mir_enable_passes` ahead of `is_enabled`, so the `mir_opt_level > 0` gate at `remove_storage_markers.rs:10-12` is bypassed. |
| `mir::StatementKind::StorageDead(_)` | Same as `StorageLive(_)`. |
| `mir::StatementKind::Retag(_, _)` | Only emitted by `AddRetag` (`rustc_mir_transform/src/add_retag.rs:85, 118, 177`) which `is_enabled` requires `-Z mir-emit-retag`; the bridge driver does not enable that flag. Out-of-pipeline. |
| `mir::StatementKind::ConstEvalCounter` | Only injected by `CtfeLimit` (`rustc_mir_transform/src/ctfe_limit.rs:36`) for const-eval-bound bodies (`mir_for_ctfe`). Driver bodies are run-time bodies (`optimized_mir`) and do not receive the injection. |
| `mir::StatementKind::Coverage(non-marker variants)` | Only emitted by `InstrumentCoverage` under `-C instrument-coverage`. Driver builds do not enable coverage instrumentation. |

These statement-translation-phase rejections compose
with the §15.1 type-translation-phase rejections and
the §21.5.1 rvalue-translation-phase rejections; the
bridge applies all classes during MIR construction.

#### 22.5.2 Positive invariants

Eight rules. The first three (B-0a / B-0b / B-0c) are
inherited explicitly from prior stages; the remaining
five (B-1 -- B-5) are stage-8-introduced. All eight
are enforced at body-level walk time (deferred to
stage 11; see §22.5.3).

**I-Statement-Place-Wellformed (B-0a).** Every embedded
`Place` reachable through `Statement` payloads
satisfies the §19.7 place-validation rules: head-local
existence, field / variant-index range, structural
invariants (I-Deref-first, I-Downcast-followed-by-
Field), type-coherence walk. Inherited from §19.7
(Place validation rules).

**I-Statement-Rvalue-Wellformed (B-0b).** Every
embedded `Rvalue` reachable through `Assign` payloads
satisfies the §21.5.2 rvalue-site invariants:
I-Rvalue-Operand-Wellformed, I-Rvalue-TyId-Interned,
I-Rvalue-Aggregate-Operand-Count, I-Rvalue-Repeat-
Count-Wellformed, I-Rvalue-VariantIdx-Wellformed,
I-Rvalue-FieldIdx-Wellformed. Inherited from §21.5
(Rvalue-site invariants).
The §21.5.1 7-row rejection table also applies
transitively (an `Assign(_, rejected_rvalue)` is
rejected at the rvalue level).

**I-Statement-Operand-Wellformed (B-0c).** Every
embedded `Operand` reachable through Statement
payloads outside an `Assign`'s `Rvalue`
(specifically: the operand positions inside
`Intrinsic(NonDivergingIntrinsic::Assume(op))` and
inside `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping{src, dst, count})`)
satisfies the §20.5 operand-site invariant
`I-Operand-Place-Validity` (the single rule §20.5
defines, which transitively inherits §19.7
place-validation rules for the `Copy` / `Move`
arms). B-0c is distinct from B-0b: B-0b applies to
operands embedded in rvalues (covered by §21.5.2's
I-Rvalue-Operand-Wellformed); B-0c applies to
operands embedded directly in Statement payloads
without an enclosing Rvalue. The `Constant` arm of
`Operand` carries a `ConstId` whose value-level
identity flows through the §9.7 ConstInterner;
operand-context type coherence for the `Constant`
arm is owned by stages 7-9 (per §20.5's explicit
note that "Stage 6 does not duplicate that check"),
so B-0c does not duplicate it either.

**I-Statement-Assign-Type-Equality (B-1).** For
`Assign(p, rv)`, `place_ty(p) == rvalue_ty(rv)` modulo
lifetime erasure. Defined by rustc's well-formedness
rule "Assignments in which the types of the place and
rvalue differ are not well-formed"
(`rustc_middle/src/mir/syntax.rs:324`); pinned by
rustc's validator
(`rustc_mir_transform/src/validate.rs:1467-1582`).
Type recovery uses §19's `place_ty` (the type-coherence
walk over the head local's `LocalDecl.ty` plus the
projection chain) and §21's `rvalue_ty` (the per-
variant type-recovery rules).

**I-Statement-SetDiscriminant-VariantIdx (B-2).** For
`SetDiscriminant(p, vi)`, `vi < variant_count(place_ty(p))`
where `variant_count` reads `place_ty(p)`'s
`LayoutKind::Enum.shape` (§11.1) per the
`EnumShape` cases:

- `EnumShape::Empty -> 0`,
- `EnumShape::Single { .. } -> 1`,
- `EnumShape::Multiple { variants, .. } -> variants.len()`.

Same bound-check shape as §19.7 rule 4 (Place
validation for Downcast on enum types) and
§21.5.2 I-Rvalue-VariantIdx-Wellformed for
`Aggregate(AggregateKind::Enum(_, _), _)`.

**I-Statement-SetDiscriminant-Target (B-3).** For
`SetDiscriminant(p, _)`, `place_ty(p) = Ty::Enum {
item, args, layout }` per §10. Other shapes
(`Ty::Struct{..}`, `Ty::Union{..}`, `Ty::Tuple{..}`,
primitive types, references, raw pointers, function
types, etc.) reject at the bridge with a diagnostic
citing the §10 `Ty` variant taxonomy. Coroutines
were already rejected in stage 3 §15.1; opaque-alias
types were normalized away by the required
`PostAnalysisNormalize` pass per
`rustc_mir_transform/src/lib.rs:642`. Compositional
with stage-3 `Ty` taxonomy, not a parallel
`target_kind: Enum` flag.

**I-Statement-Intrinsic-Assume-Bool (B-4).** For
`Intrinsic(NonDivergingIntrinsic::Assume(op))`,
`operand_ty(op) == Ty::Bool`. Pinned by rustc's
validator
(`rustc_mir_transform/src/validate.rs:1467-1582`); the
documentation at `rustc_middle/src/mir/syntax.rs:486-492`
states "The operand must be a boolean."

**I-Statement-Intrinsic-CopyNonOverlapping-Types
(B-5).** For
`Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping { src, dst, count }))`:
- `operand_ty(src)` and `operand_ty(dst)` are each
  one of the RegseqModel deref-able pointer-like
  types: raw pointer (`Ty::Ptr(pointee_ty, _)`) or
  reference (`Ty::Ref(pointee_ty, _)`);
- their resolved pointee types are identical
  (`pointee(operand_ty(src)) ==
  pointee(operand_ty(dst))`);
- `operand_ty(count) == Ty::Uint(UintKind::Usize)`.

Pinned by rustc's validator at
`rustc_mir_transform/src/validate.rs:1517-1549` --
specifically the `builtin_deref(true)` check at
`validate.rs:1521, 1531` plus the pointee-equality
check at `validate.rs:1542` and the `usize` check at
`validate.rs:1547`. The documentation at
`rustc_middle/src/mir/syntax.rs:494-507` states "src
and dest must each be a reference, pointer, or
`Box` pointing to the same type T. count must
evaluate to a usize."

**Narrowing vs rustc's validator.** Rustc's
`builtin_deref(true)` predicate also returns `Some`
for `Box<T>`, but RegseqModel does NOT admit Box at
the operand level here -- stage-3 §15.1 rejects
`Box<T>` at type translation (`alloc` construct
outside the no-alloc driver DSL; see `spec.md:4546`).
A Box-typed operand cannot reach a stage-8
`CopyNonOverlapping` site because its enclosing type
would have been rejected upstream. RegseqModel's
post-translation B-5 thus accepts the strict subset
(`Ty::Ptr` and `Ty::Ref`) that survives the §15.1
rejection cascade; in practice, driver code at
`copy_nonoverlapping` call sites overwhelmingly
passes raw pointers (the intrinsic signature takes
`*const T` / `*mut T`), with references admitted
when rustc emits them through deref coercions.

#### 22.5.3 Walk-implementation: deferred to stage 11

Rules enumerated at this stage; the walk implementation
applying them is deferred to stage 11's body-validation
step. Stage 11 owns the body-level walk that visits
every `BasicBlock.statements[i]` and applies §22.5.1
rejection rules + §22.5.2 positive invariants in a
single traversal of body content. Compositional with
the §19.7 Place walk and §21.5.3 Rvalue walk
inheritance: a stage-11 visitor that walks Statement
content visits Place / Rvalue invariants in the same
pass (B-0a / B-0b implicit in walk dispatch).

Same split as §19.7's `Place` validation, §20.5's
operand-site invariants, and §21.5.3's rvalue-site
invariants -- all of which inherit the walk via §3.3
Phase B body validation.

#### 22.5.4 F4 / F4-2 cross-build identity: out of scope

`Statement` is body-local content; it does not
participate in the cross-build identity discipline
(F4 / F4-2, §16). Same scope reduction as `Place`
(§19.2), `Operand` (§20.2), and `Rvalue` (§21.4). The
F4-2 instance ordering canonicalises `(InstanceKey)`,
not `(BodyContents)`.

### 22.6 Bridge translation rules

Per-rustc-variant translation. The 3 RegseqModel
`StatementKind` variants plus the 12-row reject-on-
encounter table (§22.5.1) cover the full 13-variant
rustc surface (`mir::StatementKind<'tcx>`) and its
sub-enum variants.

#### 22.6.1 Top-level `StatementKind` variants

| rustc form | RegseqModel form |
|---|---|
| `mir::StatementKind::Assign(box (p, rv))` | `StatementKind::Assign(translate_place(p), translate_rvalue(rv))` -- the `Box<(Place, Rvalue)>` indirection is unwrapped at translation time |
| `mir::StatementKind::Assign(_, mir::Rvalue::Ref(_, BorrowKind::Fake(_), _))` | reject (§22.5.1) -- whole Assign cleanup-discharged |
| `mir::StatementKind::SetDiscriminant { box place, variant_index }` | `StatementKind::SetDiscriminant(translate_place(*place), VariantIdx::from(variant_index))` -- the `Box<Place>` is unwrapped |
| `mir::StatementKind::Intrinsic(box ndi)` | `StatementKind::Intrinsic(translate_non_diverging_intrinsic(*ndi))` (see §22.6.2) -- the `Box<NonDivergingIntrinsic>` is unwrapped |
| `mir::StatementKind::FakeRead(_, _)` | reject (§22.5.1) |
| `mir::StatementKind::AscribeUserType(_, _)` | reject (§22.5.1) |
| `mir::StatementKind::BackwardIncompatibleDropHint { .. }` | reject (§22.5.1) |
| `mir::StatementKind::Coverage(_)` | reject (§22.5.1; both marker and non-marker arms) |
| `mir::StatementKind::PlaceMention(_)` | reject (§22.5.1) |
| `mir::StatementKind::Nop` | reject (§22.5.1) |
| `mir::StatementKind::StorageLive(_)` | reject (§22.5.1) |
| `mir::StatementKind::StorageDead(_)` | reject (§22.5.1) |
| `mir::StatementKind::Retag(_, _)` | reject (§22.5.1) |
| `mir::StatementKind::ConstEvalCounter` | reject (§22.5.1) |

#### 22.6.2 `NonDivergingIntrinsic` translation

| rustc form | RegseqModel form |
|---|---|
| `mir::NonDivergingIntrinsic::Assume(op)` | `NonDivergingIntrinsic::Assume(translate_operand(op))` |
| `mir::NonDivergingIntrinsic::CopyNonOverlapping(mir::CopyNonOverlapping { src, dst, count })` | `NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping { src: translate_operand(src), dst: translate_operand(dst), count: translate_operand(count) })` |

#### 22.6.3 `Statement` wrapper translation

```text
translate_statement(mir::Statement<'tcx> { source_info, kind, debuginfos })
  -> Statement {
       kind: translate_statement_kind(kind),
       span: translate_span(source_info.span),
     }
```

The `source_info.scope` slot is dropped (debuginfo-only;
§22.1.1). The `debuginfos` field is dropped (DWARF
emission infrastructure has no C-shape meaning;
§22.1.1).

### 22.7 Worked examples

Three examples illustrating the three retained
variants. Bodies are post-monomorphisation,
post-cleanup-pass-discharge.

#### 22.7.1 `Assign` (the central case)

```rust
fn add_one(x: u32) -> u32 {
    let y = x + 1;
    y
}
```

Body (sketch):

```text
bb0: {
    _2 = const 1_u32;                     // Assign(_2, Use(Constant(c1)))
    _3 = Add(move _1, move _2);           // Assign(_3, BinaryOp(Add, Move(_1), Move(_2)))
    _0 = move _3;                          // Assign(_0, Use(Move(_3)))
    return;                                // (terminator -- stage 9)
}
```

Three `Statement::Assign(Place, Rvalue)` instances,
each lowering through stage-7's `Rvalue` vocabulary
to a different shape: `Use` for the constant load and
return-place forward; `BinaryOp` for the arithmetic.
None of the dropped variants
(`FakeRead`/`AscribeUserType`/`StorageLive`/etc.)
appears.

#### 22.7.2 `SetDiscriminant`

```rust
enum State { Idle, Configured, Running }
fn advance(s: &mut State) {
    *s = State::Configured;
}
```

Body (sketch, post-cleanup):

```text
bb0: {
    discriminant((*_1)) = 1;              // SetDiscriminant((*_1), VariantIdx(1))
    return;
}
```

A single `Statement::SetDiscriminant(place, vidx)`
where `place` is `(*_1)` (Deref of the `&mut State`
parameter) and `vidx` is `VariantIdx(1)` (the
`Configured` variant). The target type is recoverable
from `place_ty((*_1)) = Ty::Enum { item: <ItemId of
State>, args: [], layout: <Layout of State> }`. `State`
has 3 fieldless variants, so the variant_index is
in-range; the bound check reads
`Ty::Enum`'s `LayoutKind::Enum.shape ->
EnumShape::Multiple { variants, .. } -> variants.len()
== 3`, and `vidx == 1 < 3` holds.

A reader reading the optimized_mir for this body
verifies B-2 (`vidx < 3`), B-3 (`place_ty` is enum-
Adt), and the §22.5.1 rejection rows do not fire
(none of the dropped variants appears).

#### 22.7.3 `Intrinsic` (both arms)

```rust
fn assume_and_copy(p: *mut u32, q: *const u32, n: usize) {
    unsafe {
        core::intrinsics::assume(!p.is_null());
        core::ptr::copy_nonoverlapping(q, p, n);
    }
}
```

Body (sketch, post-cleanup, post-`LowerIntrinsics`):

```text
bb0: {
    _4 = Eq(move _1, const 0_usize as *mut u32);  // computes !p.is_null() inverted
    _5 = Not(move _4);
    assume(move _5);                                // Intrinsic(Assume(Move(_5)))
    copy_nonoverlapping(src: move _2, dst: move _1, count: move _3);
                                                    // Intrinsic(CopyNonOverlapping{...})
    return;
}
```

Two `Statement::Intrinsic` instances:
- `Intrinsic(NonDivergingIntrinsic::Assume(Move(_5)))`
  where `operand_ty(Move(_5)) == Ty::Bool` (B-4
  satisfied).
- `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping { src: Move(_2), dst: Move(_1), count: Move(_3) }))`
  where:
  - `operand_ty(Move(_2)) == Ty::Ptr(u32_ty_id, Mutability::Not)` (the `*const u32`),
  - `operand_ty(Move(_1)) == Ty::Ptr(u32_ty_id, Mutability::Mut)` (the `*mut u32`),
  - `pointee(_2) == pointee(_1) == u32_ty_id` (B-5
    pointee-equality satisfied),
  - `operand_ty(Move(_3)) == Ty::Uint(UintKind::Usize)`
    (B-5 count-type satisfied).

C-emission shapes for the generic-C target are backend-codegen
content; see `generic-c-backend.md` §6 for the generic-C
target's `Assume` and `CopyNonOverlapping` lowerings,
including the argument-order swap between rustc's
`copy_nonoverlapping(src, dst, count)` and C's `memcpy(dst,
src, count * sizeof T)`. The named `src` / `dst` fields on
the `CopyNonOverlapping` payload struct (IR-side) preserve
rustc's argument naming so backends consume by name; the
positional swap between Rust and C signatures cannot
silently confuse construction or consumption sites.

### 22.8 Rendering

See `pretty-printer.md` §7.4.

---


## 23. Terminator vocabulary

The `Terminator` vocabulary is the **control-flow-edge
surface** of MIR -- the typed expression of "how a basic
block exits and where execution continues." The bridge
fills it from rustc's `mir::TerminatorKind<'tcx>`;
downstream consumers (stage 10 basic blocks --
`BasicBlock.terminator: Terminator`; stage 11 bodies;
stage 14 backends) read it as the per-block control-flow
transfer ending each basic block.

Stage 9 sits **fifth in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands ->
rvalues -> statements -> **terminators** -> basic blocks
-> bodies). Stage-9 content embeds stage-1 / stage-3 /
stage-5 / stage-6 vocabulary (no Rvalue or Statement
embeddings at the Terminator surface).

### 23.1 `Terminator` struct and `TerminatorKind` enum

#### 23.1.1 `Terminator` wrapper struct

```rust
/// A control-flow transfer ending a basic block, with
/// the source location it derives from. Body-local
/// content; lives inline inside basic-block storage,
/// not in any top-level interner table. Equality is
/// structural.
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}
```

The wrapper carries exactly one diagnostic field
(`span`) beyond the variant payload. **Dropped from
rustc:**
- `source_info.scope: SourceScope` -- debuginfo-only
  DWARF-emission index. RegseqModel does not emit
  DWARF (per stage-1 spec rejection of debuginfo
  storage on grounds of C-shape preservation). Same
  drop as §22.1.1 for Statement.

The `span: Span` field uses the stage-1 `Span` form
inline (per top-level `spec.md` §5.3, which carries
`Span` as a plain inline struct, not via a `SpanId`
interner -- spans are not deduplicated). No new
machinery introduced by stage 9.

#### 23.1.2 `TerminatorKind` enum

```rust
/// Closed 6-variant set covering every basic-block exit
/// in well-formed driver MIR post-pipeline. Any rustc
/// `TerminatorKind<'tcx>` variant outside this set is
/// either pipeline-discharged (FalseEdge, FalseUnwind,
/// Yield, CoroutineDrop, UnwindResume, UnwindTerminate)
/// or DSL-rejected (TailCall, InlineAsm); see §23.6.1.
/// Drop is collapsed at the bridge to a synthetic Call
/// per §23.4.
pub enum TerminatorKind {
    /// Unconditional branch. Single successor.
    Goto(BlockId),

    /// Multi-target conditional branch on an integer-
    /// typed discriminant. The closed-form `cases` list
    /// plus an `otherwise` block; see §23.3.2.
    SwitchInt {
        discr:   Operand,
        targets: SwitchTargets,
    },

    /// Function return. The return value lives at
    /// `LocalId::RETURN_PLACE` (`_0`); its type is
    /// `FnSig.output()` (stage 11). No payload.
    Return,

    /// Terminating UB marker. Executing this terminator
    /// is undefined behaviour. Emitted naturally from
    /// `loop {}` patterns and as the output of cleanup-
    /// pass simplifications under panic=abort.
    Unreachable,

    /// Function call. `callee` distinguishes
    /// compile-time-resolved Direct calls from
    /// runtime-resolved Indirect calls (function-
    /// pointer calls and post-rewrite virtual dispatch);
    /// see §23.3.1. `target = None` indicates a
    /// divergent call (callee does not return; e.g.,
    /// `panic!()`, `-> !` callee).
    Call {
        callee:      CallCallee,
        args:        Box<[Operand]>,
        destination: Place,
        target:      Option<BlockId>,
    },

    /// Guarded conditional branch. If `cond != expected`,
    /// the runtime invokes the panic helper with `msg`;
    /// otherwise execution continues at `target`. Under
    /// panic=abort, "invokes the panic helper" means
    /// "calls a chosen abort routine and follows
    /// `__builtin_unreachable()`."
    Assert {
        cond:     Operand,
        expected: bool,
        msg:      AssertKind,
        target:   BlockId,
    },
}
```

**Cardinality 6.** Smallest closed surface that covers
well-formed driver MIR post-pipeline. See
`discussions.md` D-9-1.1 for the cardinality decision
and prior-art comparison.

#### 23.1.3 What's dropped from rustc's `TerminatorKind`

Eight rustc variants do not appear in the RegseqModel
surface; one is collapsed at the bridge.

**Pipeline-discharged (6 rustc variants):**
- `FalseEdge { real_target, imaginary_target }` --
  `CleanupPostBorrowck` rewrites to
  `Goto(real_target)` at
  `rustc_mir_transform/src/cleanup_post_borrowck.rs:67-74`;
  pass is `is_required: true`.
- `FalseUnwind { real_target, unwind }` -- same pass,
  same site at `cleanup_post_borrowck.rs:69`.
- `Yield { ... }` -- `coroutine::StateTransform`
  discharges; stage-3 §15.1 type-rejects coroutines
  (double-discharged).
- `CoroutineDrop` -- same pass + same type-rejection.
- `UnwindResume` -- under panic=abort,
  `AbortUnwindingCalls` rewrites unwind edges to
  `UnwindAction::Unreachable` at
  `rustc_mir_transform/src/abort_unwinding_calls.rs:115-121`
  (the conditional rewrite when
  `!call_can_unwind || !target_supports_unwinding`);
  dead-block removal at
  `rustc_mir_transform/src/abort_unwinding_calls.rs:137-138`.
  Cleanup blocks become unreachable (Resume only lives
  in cleanup blocks per
  `rustc_middle/src/mir/syntax.rs:725-727`); SimplifyCfg
  prunes them.
- `UnwindTerminate(reason)` -- same panic=abort pruning
  path.

**DSL-rejected (2 rustc variants):**
- `TailCall { func, args, fn_span }` -- rustc supports
  under `#![feature(explicit_tail_calls)]` (unstable);
  driver-source DSL does not include the construct.
- `InlineAsm { ... }` -- rustc supports unconditionally;
  no pipeline pass discharges it. DSL has explicit
  domain-specific intrinsics (MMIO, synchronization,
  reset, clock, etc.) for low-level operations.

**Bridge-collapsed (1 rustc variant):**
- `Drop { place, target, unwind, replace, drop, async_fut }`
  -- bridge converts unconditionally to a synthetic
  `Call` to drop-glue (§23.4). The variant's extras
  (`replace` / `drop` / `async_fut`) are diagnostic-only
  or async-coroutine-only; both subsumed (no
  RegseqModel-side carry).

The full registry with diagnostic shape and discharge
mechanism per row lives at §23.6.1.

#### 23.1.4 `Terminator.successors()` -- per-variant successor enumeration

`Terminator::successors(&self) -> impl Iterator<Item = BlockId>`
is the canonical iterator over the BlockIds reachable
from a given Terminator's control-flow exit. The order
is per-variant fixed and stable across builds:

| Variant | Successors (in iteration order) |
|---|---|
| `Goto(target)` | `[target]` |
| `SwitchInt { targets: SwitchTargets { cases, otherwise } }` | the `BlockId` of each `(_, target)` in `cases` (storage order), followed by `[otherwise]` |
| `Return` | `[]` |
| `Unreachable` | `[]` |
| `Call { target: Some(t), .. }` | `[t]` |
| `Call { target: None, .. }` | `[]` (divergent call; no successor) |
| `Assert { target, .. }` | `[target]` |

Mirrors rustc's `TerminatorKind::successors()` at
`rustc_middle/src/mir/terminator.rs:527-568` for the
retained variants. The order is deterministic
per-build (no monomorphization-induced reordering).
B-Successors-Valid (§24.6.2) ensures every yielded
`BlockId` is in `0..body.basic_blocks.len()` and is
walk-pre-checked at §24.4.3 step 2 before any DFS
traversal indexes into `body.basic_blocks`.

Consumers include §24.4.3 stage-11 walk-impl
(DFS-RPO from `BlockId(0)`), §24.6.2's
B-Successors-Valid pre-check, and `regseq-analysis`
CFG analyses (predecessors, dominators).

### 23.2 Per-variant payload shapes

The per-variant payload structure of each `TerminatorKind`
variant. All fields are flat (no `Box<...>` indirection
on payloads except `args: Box<[Operand]>` for
slim-pointer storage of variable-length arg lists).

#### 23.2.1 `Goto(BlockId)`

Single-field flat tuple. The `BlockId` resolves to a
basic block in the enclosing body's `basic_blocks`
table (stage-10 vocabulary). Universal across all
prior-art projects; no payload alternatives worth
surveying.

#### 23.2.2 `SwitchInt { discr, targets }`

Two named fields:
- `discr: Operand` -- the value being tested. Per
  §23.6.2 B-1, `discr`'s type ∈ {`Ty::Bool`, `Ty::Char`,
  `Ty::Int(_)`, `Ty::Uint(_)`}. Enum-discriminant
  switches reach SwitchInt only via a preceding
  `Rvalue::Discriminant` projection (stage 7) that
  produces an integer-typed Operand; SwitchInt itself
  never sees an enum-typed discr.
- `targets: SwitchTargets` -- the (value -> block)
  mapping plus the otherwise destination. Storage shape
  per §23.3.2.

#### 23.2.3 `Return`

Unit variant; no payload. The return value lives
implicitly at `LocalId::RETURN_PLACE` (`_0` per
stage-1's body-local ID convention). Its type is reachable
two ways: directly as `Body.local_decls[RETURN_PLACE].ty`
(the body-local positional convention); or via the owning
Function Instance's signature
`model.sigs[inst.kind.as_function().fn_sig].output()`
per the §23.6.2 `callee_fn_sig` helper applied to the
returning Body's owning Instance (Option B; supersedes
stage-11 D-11-3's Item-side framing). The Terminator
carries no operand because the return value is at `_0`
post-`optimized_mir`.

#### 23.2.4 `Unreachable`

Unit variant; no payload. Terminating UB marker.
Universal across all four prior-art projects as a unit
variant.

#### 23.2.5 `Call { callee, args, destination, target }`

Four named fields:
- `callee: CallCallee` -- per §23.3.1. `Direct(InstanceId)`
  for compile-time-resolved calls; `Indirect(Operand)`
  for function-pointer calls and post-stage-11-rewrite
  virtual-dispatch calls.
- `args: Box<[Operand]>` -- variable-length argument
  list. `Box<[T]>` matches §21's `Aggregate` storage
  shape (slim-pointer + length, no `Vec` overhead).
  Per-arg spans dropped per §23.1.1's wrapper-only span
  policy; per-arg diagnostic granularity falls
  back to the Terminator-level `span`.
- `destination: Place` -- where the return value is
  written. Always present in `optimized_mir` (rustc
  pre-allocates the destination Place post-cleanup; the
  source-MIR `Option<Place>` shape is gone).
- `target: Option<BlockId>` -- `Some(bb)` indicates
  successor block on normal return; `None` indicates a
  divergent call (callee does not return -- e.g.,
  `panic!()`, `-> !` callee, `core::intrinsics::unreachable()`).

**Dropped from rustc Call payload:**
- `unwind: UnwindAction` -- under panic=abort, only
  `Continue` and `Unreachable` survive at our consumption
  point and collapse semantically (no cleanup edges).
  Preserving the field would store a 1-bit flag with no
  consumer.
- `call_source: CallSource` -- 5-variant diagnostic enum
  (`OverloadedOperator` / `MatchCmp` / `Misc` / `Use` /
  `Normal`); rustc itself uses it only for "better error
  messages." No C-codegen-relevant information.
- `fn_span: Span` -- function-position span without the
  receiver; subsumed by Terminator-level `span`.

#### 23.2.6 `Assert { cond, expected, msg, target }`

Four named fields:
- `cond: Operand` -- the condition operand. Per §23.6.2
  B-2, `cond`'s type = `Ty::Bool`.
- `expected: bool` -- the expected value of `cond`. If
  `cond != expected`, the runtime invokes the panic
  helper.
- `msg: AssertKind` -- the panic-message data per
  §23.3.3. 8-variant subset of rustc's 11-variant
  `AssertKind` (drops 3 coroutine-only variants).
- `target: BlockId` -- success-path successor block.

**Assert is classified as a Terminator** (not a
Statement, per the Charon-style alternative).
Compiler-theory framing: Assert is a 2-target
conditional branch (success vs panic), made implicit
under panic=abort because the panic-side is structurally
unreachable post-`AbortUnwindingCalls`. CFG analyses see
it uniformly with `SwitchInt` as a guarded branch
instruction.

**Dropped from rustc Assert payload:**
- `unwind: UnwindAction` -- same rationale as Call's
  unwind drop above.

### 23.3 Sub-enums and sub-structs

Three new types introduced at stage 9: `CallCallee`,
`SwitchTargets`, `AssertKind`. None are interned (same
inline-storage rule as §23.5).

#### 23.3.1 `CallCallee`

```rust
/// Closed 2-arm classification of `Call.callee`.
/// `Direct` carries a compile-time-resolved
/// `InstanceId`; `Indirect` carries a runtime function-
/// pointer Operand. Universal IR distinction across
/// LLVM (`call` vs SSA-value), Cranelift (`call` vs
/// `call_indirect`), Charon
/// (`FnOperand::{Regular(FnPtr), Move(Place)}`), and
/// mrustc (`CallTarget::{Path, Value, Intrinsic}`).
pub enum CallCallee {
    /// Direct call: callee resolves at compile time to
    /// a specific InstanceKind (Item, Intrinsic, shim,
    /// drop-glue, etc. per stage-1 InstanceKind closure).
    /// Subject to the closure-completeness invariant
    /// per §23.6.2 B-7.
    Direct(InstanceId),

    /// Indirect call: callee is a runtime function-
    /// pointer value. Used for:
    /// (a) function-pointer calls --
    ///     `let f: fn(i32) -> i32 = some_fn; f(x)`.
    /// (b) post-stage-11-rewrite virtual dispatch --
    ///     vtable-loaded function pointer through the
    ///     receiver's fat-pointer metadata field.
    /// Subject to the operand-type-is-FnPtr invariant
    /// per §23.6.2 B-5.
    Indirect(Operand),
}
```

The `Direct` / `Indirect` split serves three call shapes:
direct calls to monomorphized functions (`Direct`),
function-pointer calls (`Indirect(Operand)` with
`Operand: Ty::FnPtr`), and post-stage-11-rewrite virtual
dispatch (`Indirect(Operand)` with `Operand` carrying
the loaded vtable function pointer). See
`discussions.md` D-9-3 for the load-bearing rationale
and the foreclosed `InstanceKind::Virtual` alternative.

#### 23.3.2 `SwitchTargets`

```rust
/// Closed-form switch target table: explicit
/// (case_value -> block) pairs plus a structurally
/// separate otherwise destination. No positional
/// "last entry is otherwise" convention; the type
/// system enforces the otherwise field is present.
pub struct SwitchTargets {
    /// (case value, branch target) pairs. Empty when
    /// the switch has no cases; the otherwise field
    /// then carries the entire dispatch.
    pub cases: Box<[(u128, BlockId)]>,

    /// Catch-all destination when `discr` matches no
    /// case value.
    pub otherwise: BlockId,
}
```

The case-value width is `u128`. Stage-1's `Operand` /
`Const` machinery preserves integer values up to 128
bits. The rustc `SwitchTargets.values` field uses
`Pu128` (a packed u128 wrapper for HashStable);
RegseqModel uses bare `u128` since we do not implement
HashStable. See `discussions.md` D-9-6 for the
explicit-split-vs-rustc-mirror rationale.

#### 23.3.3 `AssertKind`

```rust
/// Closed 8-variant set of panic-message kinds for
/// `Assert.msg`. Subset of rustc's 11-variant
/// `AssertKind<O>` (drops 3 coroutine-only variants per
/// D-9-5: `ResumedAfterReturn`, `ResumedAfterPanic`,
/// `ResumedAfterDrop`). Coroutines are stage-3 §15.1
/// type-rejected, so the dropped variants are
/// structurally unreachable in well-formed
/// RegseqModel bodies.
pub enum AssertKind {
    /// Index-out-of-bounds: `index >= len`. Operand
    /// types: `len: Ty::Uint(_)` (typically usize),
    /// `index: Ty::Uint(_)` (typically usize).
    BoundsCheck { len: Operand, index: Operand },

    /// Arithmetic overflow on a checked binary op.
    /// `BinOp` reuses stage-7's BinOp vocabulary
    /// (Add, Sub, Mul, Div, Rem, Shl, Shr, ...).
    Overflow(BinOp, Operand, Operand),

    /// Negation overflow: `-i128::MIN` etc.
    OverflowNeg(Operand),

    /// `x / 0`. Operand is the divisor.
    DivisionByZero(Operand),

    /// `x % 0`. Operand is the divisor.
    RemainderByZero(Operand),

    /// `*p` where p has alignment requirement R but
    /// `(p as usize) % R != 0`. `required` is R as a
    /// usize-typed operand; `found` is `p` as a usize.
    MisalignedPointerDereference {
        required: Operand,
        found:    Operand,
    },

    /// `*p` where `p` is null. No operand carried (the
    /// failing pointer was a null literal).
    NullPointerDereference,

    /// Construction of an enum value with a discriminant
    /// outside the valid set. Operand is the
    /// out-of-range discriminant value.
    InvalidEnumConstruction(Operand),
}
```

See `discussions.md` D-9-5 for the foreclosed
alternatives (rustc_public's preserve-all-11; Charon's
3-variant `AbortKind` collapse) and the type-
rejection-cascade rationale for dropping the 3
coroutine variants.

### 23.4 Drop-as-Call collapse at the bridge

The bridge converts each MIR `TerminatorKind::Drop`
into a synthetic `TerminatorKind::Call` to the
drop-glue `Instance` for `place_ty`. Stage-9 surface
has **no separate `Drop` variant**. (See
`discussions.md` D-9-2 for the C-semantics +
closure-correctness + no-information-loss rationale.)

**Bridge translation rule (5 steps):**

1. **Resolve drop-glue Instance.** Compute
   `place_ty = place.ty(local_decls, tcx)`; call
   `Instance::resolve_drop_in_place(tcx, place_ty)` to
   obtain the drop-glue `Instance<'tcx>`; bridge-intern
   as a RegseqModel `InstanceId`. The resulting Regseq
   `Instance.kind = InstanceKind::Function { flavor:
   <F>, .. }` where `F` depends on `place_ty`:
   - **Concrete `place_ty`** (struct, enum, tuple,
     etc.): rustc resolves to
     `ty::InstanceKind::DropGlue(def_id, Some(ty))`;
     the Regseq Instance has
     `flavor = FunctionInstanceFlavor::DropGlue` and
     keys with `flavor = FunctionInstanceFlavor::DropGlue`
     and `fn_shim_disambig = 10` (per §9.3
     `rustc_instance_kind_sort_rank`) on the F4-2
     RegseqInstanceKey.
   - **`place_ty = Ty::Dynamic { ... }`** (`dyn Trait`):
     rustc resolves to
     `ty::InstanceKind::Virtual(drop_fn.def_id(), 0)`
     per `rustc_codegen_ssa/src/mir/block.rs:640-641`;
     the Regseq Instance has
     `flavor = FunctionInstanceFlavor::Virtual` and
     `virtual_idx = Some(0)` (the
     `COMMON_VTABLE_ENTRIES_DROPINPLACE = 0` convention
     per `rustc_middle/src/ty/vtable.rs:45-49`). The
     vtable index is **stored on the Regseq Instance's
     `virtual_idx` field** (per §9.4 Function variant)
     and
     consumed by the stage-11 vtable-indirect rewrite,
     which handles index-0 / drop-slot virtual calls
     uniformly with index-N / method-slot virtual
     calls -- same `flavor=Virtual` predicate, same
     `virtual_idx` lookup, same projection synthesis
     and args rewrite.

2. **No ZST elision at the bridge.** Always emit the
   Call to the no-op drop-glue when
   `!place_ty.needs_drop(...)`; let stage-14 codegen
   peephole `Call(no_op_drop_glue)` -> `<no emission>`.

3. **Synthesize the Call.**
   ```rust
   TerminatorKind::Call {
       callee:      CallCallee::Direct(drop_glue_instance_id),
       args:        Box::new([Operand::Move(<&raw mut place> temp)]),
       destination: <fresh unit-typed temp>,
       target:      Some(target),  // copied from rustc Drop
   }
   ```

4. **Synthesize the `&raw mut place` operand.** The
   bridge **appends** the Statement
   `Statement::Assign(<fresh_local>, Rvalue::RawPtr(Mutability::Mut, place))`
   as the **last entry** of the current basic block's
   construction-time `Vec<Statement>` buffer
   (finalized as `Box<[Statement]>` per §24.1.2
   stage-10 D-10-1.2 at block-construction completion)
   -- positioned immediately before the synthetic Call
   terminator (and **after** all earlier statements
   that may have mutated `place`). The Call argument
   is `Operand::Move(<fresh_local>)`. Stage-6
   `Operand` does NOT admit an `&raw mut place` form
   directly (per D-6-1's 3-variant surface), so this
   terminator-adjacent insertion is the only path. The
   fresh `LocalId` allocation is outbound to stage 11.

   *Why append, not prepend.* Earlier statements in the
   basic block may write to `place`. The synthesised
   `&raw mut place` must observe the latest write at
   the moment Drop runs, which is the end of the basic
   block (rustc's Drop is the basic-block terminator;
   no statements follow it). Appending the synthetic
   Assign after all earlier statements gives the
   correct pointer value.

5. **Synthesize the unit return slot.** Drop-glue
   returns `()`. Allocate a fresh `LocalId` of type
   `Ty::Tuple([])` for the destination. Outbound to
   stage 11.

**Implementer's references:**
- **rustc-internal precedent (with two intentional
  divergences).** rustc's codegen-SSA at
  `rustc_codegen_ssa/src/mir/block.rs:599-693`
  (`codegen_drop_terminator`) handles three cases:
  1. **ZST drop elision** (`block.rs:606-613`): when
     the resolved drop-glue is `DropGlue(_, None)`,
     rustc branches directly to the target without
     emitting a call. RegseqModel does NOT do this at
     the bridge; ZST elision is deferred to stage-14
     codegen peephole.
  2. **Dynamic-drop vtable path** (`block.rs:624-657`):
     for `ty::Dynamic` (`dyn Trait`) drops, rustc
     loads the drop_in_place fn-pointer from the
     vtable and dispatches indirectly via
     `ty::InstanceKind::Virtual(drop_fn.def_id(), 0)`.
     RegseqModel mirrors this resolution at the
     bridge: when `place_ty = Ty::Dynamic { ... }`,
     step 1 above produces a Regseq Instance with
     `flavor = FunctionInstanceFlavor::Virtual` (vtable
     index 0). The stage-11 vtable-indirect rewrite
     then handles index-0 drop-slot virtual calls
     uniformly with index-N method-slot virtual
     calls -- same `flavor=Virtual` predicate, same
     `PtrMetadata -> Deref -> Field(idx)` projection
     synthesis, same args-rewrite (extract data
     pointer from fat pointer). This keeps the
     rewrite mechanism uniform across method virtual
     calls and drop virtual calls; no separate
     dyn-Trait-drop special case at either bridge
     time or rewrite time.
  3. **Concrete dispatch** (`block.rs:680-692`):
     `helper.do_call(...)` resolves drop-glue via
     `Instance::resolve_drop_in_place(tcx, ty)` and
     dispatches as if the source was an explicit
     `drop_in_place::<T>(p)` call. RegseqModel's
     bridge mirrors this case for non-`dyn Trait`
     types.

  RegseqModel's collapse-at-bridge thus differs from
  rustc's collapse-at-codegen on (i) ZST elision
  policy (deferred vs done) and (ii) `dyn Trait` drop
  handling (uniform with virtual method rewrite vs
  special-case). Both divergences are deliberate
  layering choices, not rustc-precedent contradictions.
- **Charon precedent.** Charon's optional
  `--desugar-drops` pass at
  `charon-lib/src/transform/normalize/desugar_drops.rs:23-72`
  is structurally identical: rewrites
  `Drop { place, target, ... }` into a `Call` with
  `&raw mut place` arg, fresh unit-typed return slot,
  copied `target` / `on_unwind` block ids. RegseqModel
  applies the same shape unconditionally at the bridge
  instead of as an opt-in normalize pass.
- **Charon's belt-and-braces ZST elision.** Charon's
  `filter_trivial_drops` pass collapses no-op-destruct
  Drops directly to `Goto`. RegseqModel defers this
  optimization to stage-14 codegen peephole instead.

### 23.5 Identity and storage

Terminators are **body-local content**. Each
`Terminator` lives inline inside the enclosing basic
block; there is **no top-level `terminators` interner
table** and **no `TerminatorId` index type**.

The enclosing basic-block container (`BasicBlock`) is
formally pinned at §24:

```rust
pub struct BasicBlock {
    pub statements: Box<[Statement]>,
    pub terminator: Terminator,
}
```

(See §24.1 for the full struct shape, §24.1.2 for the
`Box<[Statement]>` rationale, and §24.5 for identity
and storage at the basic-block level.)

**Equality and hashing** are `derive(PartialEq, Hash)`
on `BasicBlock` walking through to `Terminator`'s
structural derive, then through to embedded
`Operand` / `Place` / `InstanceId` (per stages 5 / 6
/ 1).

See `discussions.md` D-9-7 for the
no-interning rationale (universal precedent + no
deduplication win + pattern-match consistency with
stages 5-8 inline-storage rule).

**F4 / F4-2 cross-build identity is OUT of scope.**
Terminator content is per-build structural and need
not be canonicalized across build environments. Same
scope reduction as stages 5-8.

### 23.6 Terminator-site invariants

4-part registry: rejection table (Part A; cross-ref
to §23.6.1), positive invariants (Part B; §23.6.2),
walk-implementation deferral (Part C; §23.6.3), F4 /
F4-2 OUT-of-scope note (Part D; per §23.5).

#### 23.6.1 Rejection table (9 rows)

Two-category framing:
**pipeline-discharged** (rows 1-6) vs **DSL-only**
(rows 7-8); plus row 9 for the bridge-collapsed Drop.

| # | rustc variant | Category | Discharge mechanism | Diagnostic message |
|---|---------------|----------|---------------------|--------------------|
| 1 | `FalseEdge { real_target, imaginary_target }` | Pipeline-discharged | `CleanupPostBorrowck` rewrites to `Goto(real_target)` at `cleanup_post_borrowck.rs:67-74`; pass is `is_required: true`. | "internal error: borrowck FalseEdge survived CleanupPostBorrowck -- pipeline misconfiguration" |
| 2 | `FalseUnwind { real_target, unwind }` | Pipeline-discharged | Same pass, same site at `cleanup_post_borrowck.rs:69`. | "internal error: borrowck FalseUnwind survived CleanupPostBorrowck -- pipeline misconfiguration" |
| 3 | `Yield { value, resume, resume_arg, drop }` | Pipeline-discharged (double-discharged) | `coroutine::StateTransform` at `rustc_mir_transform/src/lib.rs:660` + stage-3 §15.1 type-rejection. | "internal error: coroutine Yield survived StateTransform; coroutine type-rejection at stage-3 §15.1 should have prevented reach" |
| 4 | `CoroutineDrop` | Pipeline-discharged (double-discharged) | Same pass + same type-rejection. | "internal error: CoroutineDrop survived StateTransform; coroutine type-rejection at stage-3 §15.1 should have prevented reach" |
| 5 | `UnwindResume` | Pipeline-discharged (panic=abort) | `AbortUnwindingCalls` rewrites unwind edges to `UnwindAction::Unreachable` at `abort_unwinding_calls.rs:115-121`; dead cleanup-block removal at `abort_unwinding_calls.rs:137-138`; cleanup blocks become unreachable; Resume only lives in cleanup blocks per `rustc_middle/src/mir/syntax.rs:725-727`; `SimplifyCfg::PreOptimizations` (in our cleanup-pass whitelist) prunes them. | "internal error: UnwindResume survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy" |
| 6 | `UnwindTerminate(reason)` | Pipeline-discharged (panic=abort) | Same panic=abort pruning path. | "internal error: UnwindTerminate survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy" |
| 7 | `TailCall { func, args, fn_span }` | DSL-only | rustc supports under `#![feature(explicit_tail_calls)]` (unstable). | "tail calls are not supported in Regseq driver source; the `become` expression and `#![feature(explicit_tail_calls)]` are not part of the DSL surface" |
| 8 | `InlineAsm { ... }` | DSL-only | rustc supports unconditionally; no pipeline pass discharges it. | "inline assembly is not supported in Regseq driver source; use the domain-specific intrinsics (MMIO, synchronization, reset, clock, ...) for low-level operations" |
| 9 | `Drop { place, target, unwind, replace, drop, async_fut }` | **Bridge-collapsed (NOT rejected)** | Per §23.4, the bridge converts Drop into a synthetic Call. This row is in the table for completeness so the implementer doesn't list Drop as "supported as-is." | n/a -- collapsed per §23.4 |

Rows 1-8 emit an `unsupported_construct!`-style
diagnostic at the bridge translation match site.
Row 9 emits no diagnostic; it's a bridge translation
rewrite producing a synthetic Call.

**Diagnostic discipline.** rustc_public's bridge
translator
(`rustc_public_bridge/src/unstable/convert/stable/mir.rs:704-790`)
surfaces the discipline distinction between
`unreachable!()` (correctness precondition violated:
rows 1-6 here) and `todo!()` (incomplete
implementation: not applicable to rows 7-8 here, which
are deliberate DSL-source rejections). RegseqModel
bridge follows the same `unsupported_construct!`-style
pattern stages 1, 3, 5, 6, 7, 8 use.

#### 23.6.2 Positive invariants

2 inherited + 9 stage-9-introduced (11 total).

**Callee-signature lookup helper.** Several invariants
below reference the callee's `FnSig`. Per stage-12
D-12-1.4 (Option B), `FnSig` lives on
`InstanceKind::Function` (per-mono substituted signature).
Define:

```text
callee_fn_sig(callee_id: InstanceId) -> &FnSig :=
    let inst = &model.instances[callee_id];
    let fn_sig_id = inst.kind.as_function().fn_sig;     // §9.4 InstanceKind::Function {fn_sig, ...}
    &model.sigs[fn_sig_id]                              // §14.4 FnSigInterner
```

Two-level indirection (Instance -> FnSigId -> FnSig).
The Item indirection that stage-11's framing required is
removed under Option B -- per-mono signature is directly
on Instance, not via parent Item. See `discussions.md`
D-12-1.4 for the Option-B reframe rationale (the
contradiction between pre-mono Item model and Item-side
post-mono FnSig storage). The B-3 / B-4 / B-6 / B-9
invariants below use `callee_fn_sig(...)` as the
resolution shorthand.

**Inherited (2):** stage-9 Terminator embeds Place
(Call.destination) and Operand (multiple sites);
Statement-level and Rvalue-level invariants do not
propagate (those constructs do not appear in
Terminator content).

- **B-0a** -- All embedded Place content satisfies
  Place invariants per stage-5 §19.7.
- **B-0c** -- All embedded Operand content satisfies
  Operand invariants per stage-6 §20.5.

**Stage-9-introduced (9):**

- **B-1** -- `SwitchInt.discr` Operand has type
  `∈ {Ty::Bool, Ty::Char, Ty::Int(_), Ty::Uint(_)}`.
  Stage-3 type-taxonomy match. Enum-discriminant
  switches reach SwitchInt only via a preceding
  `Rvalue::Discriminant` (stage-7) projection that
  produces an integer-typed Operand; SwitchInt itself
  never sees an enum-typed discr.

- **B-2** -- `Assert.cond` Operand has type `Ty::Bool`.
  Stage-3 type-taxonomy match.

- **B-3** -- Call destination type matches callee
  return type **when `target: Some(_)`**. Divergent
  calls (`target: None`, callee return type is `!`)
  have an unread destination; B-3 does not apply.
  **Post-rewrite invariant**: holds after the
  stage-11 vtable-indirect rewrite has run; pre-
  rewrite, virtual calls have receiver-type mismatch
  by design (see B-6).
  - (Direct, non-Virtual flavor) `destination_ty ==
    callee_fn_sig(callee_id).output()`.
  - (Direct, Virtual flavor pre-rewrite) `destination_ty
    == callee_fn_sig(callee_id).output()`
    where the fn_sig is rustc's "object-safe" virtual
    signature (erased receiver). Rewrite preserves
    return type.
  - (Indirect post-rewrite) `destination_ty ==
    operand_ty(callee_operand).output()` where
    `operand_ty(callee_operand) = Ty::FnPtr(...)` per
    B-5.

- **B-4** -- Call args length matches callee signature
  arity. Holds in all forms (Direct non-Virtual,
  Direct Virtual pre-rewrite, Indirect post-rewrite);
  the rewrite preserves arity (it changes the receiver
  TYPE but not the arg count).
  - (Direct) `args.len() ==
    callee_fn_sig(callee_id).inputs().len()`.
  - (Indirect) `args.len() == operand_ty(
    callee_operand).inputs().len()`.

  **Exception: diagnostic-logging intrinsic callees
  (I-DiagLog-CallArity).** When `callee =
  CallCallee::Direct(id)` and
  `model.instance_intrinsic(id) ==
  Some(IntrinsicKind::Log(_))` (per the §3.5.2
  pull-through helper), the post-DiagLogPeel Call shape
  is `[fmt_literal, expr_1, ..., expr_N]` (1 fmt
  literal + N peeled argument operands per §26.6), and
  the declared callee signature is `fn(&'static str,
  &[LogArg<'_>])` (2 inputs). The arity-match rule
  reframes to: `args.len() == 1 + N` where N is the
  number of `{}` placeholders in the format template at
  the call site (D-14-2.7 D1 enforces template/arg
  arity parity). The declared 2-arg callee signature
  does not apply; the desugared shape is the invariant.
  Indirect callees
  (`CallCallee::Indirect(Operand)`) cannot satisfy the
  log-intrinsic predicate: `IntrinsicKind` is keyed by
  Instance identity, and indirect callees do not pin
  an InstanceId. See §26.6 for the canonical statement.

- **B-5** -- Call callee is callable:
  - (Direct) `InstanceId` resolves to an
    `InstanceKind::Function` per top-level `spec.md`
    §9.4. Always callable by stage-1 instance-closure
    construction (closure verification per B-7).
    Rustc-Virtual callees produce `Direct(<id>)` with
    `FunctionInstanceFlavor::Virtual`; the per-arg type
    match B-6 holds **post-rewrite** (the receiver type
    changes from `&dyn Trait` to the erased pointer
    after the stage-11 vtable rewrite).
  - (Indirect) `operand_ty(callee_operand) ∈
    {Ty::FnPtr(_)}`. NOT `Ty::FnDef` (that resolves to
    Direct).
  - **Closures are not direct callees in `Call.callee`
    position.** Closure invocation `c(args)` is
    desugared at HIR/MIR construction to a Call to the
    closure's call method (`call_once` / `call_mut` /
    `call`) -- a synthesised Function Item per §13.1
    (closure family). The closure value `c` becomes
    the first argument (receiver) of that synthesised
    method, not the callee. Direct(InstanceId)
    referring to a closure call method has
    `flavor = FunctionInstanceFlavor::Item` per §9.4's
    bridge-synthesised-Function convention.

- **B-6** -- Per-arg type match. For each
  `i ∈ 0..args.len()`, `operand_ty(args[i]) ==
  callee_inputs[i]` where `callee_inputs` comes from
  the same source as B-4. **Post-rewrite invariant**:
  holds after the stage-11 vtable-indirect rewrite has
  run. Pre-rewrite virtual calls have args[0] with
  type `&dyn Trait` or `&mut dyn Trait` (the fat pointer
  at the call site) while `callee_fn_sig(callee_id).
  inputs()[0]` is `*mut ()` -- per §13.2 + §17.2.3, the
  vtable's erased receiver type is uniformly
  `Ty::Ptr(unit, Mutability::Mut)` regardless of the
  source borrow flavor (the const-qualified `*const ()`
  variant is sound only when the impl's Self contains no
  UnsafeCell, which is not knowable at the trait-object
  boundary; the spec pins the mutable-erased-receiver
  rule uniformly per §13.2 vtable layout). The rewrite
  resolves the mismatch by extracting `args[0].data`
  and passing the data pointer in the rewritten call.
  Stage-11 sequencing: rewrite first, validate after.
  Catches stage-11 rewrite-pass bugs in any
  post-rewrite shape.

  **Exception: diagnostic-logging intrinsic callees
  (I-DiagLog-CallArity).** When `callee =
  CallCallee::Direct(id)` and
  `model.instance_intrinsic(id) ==
  Some(IntrinsicKind::Log(_))` (per the §3.5.2
  pull-through helper), per-arg type match against the
  declared callee signature does not apply (declared
  signature is `fn(&'static str, &[LogArg<'_>])` -- 2
  inputs -- while the post-DiagLogPeel Call carries
  1 + N args per the B-4 exception). The peeled
  shape's type invariants are:
  - `args[0]` has type `Ty::Ref(Ty::Str,
    Provenance::SharedBorrow)` -- the `&'static str`
    format template. **Literal-ness** of `args[0]` is
    validated at DiagLogPeel synthesis time (§25.7.3
    step 1) against the rustc MIR Constant shape **before**
    Regseq const lowering -- post-lowering, the `Const`
    shape is `Const::Aggregate([Reloc(<static>),
    Uint(Usize, len)])` per §9.7 reference-typed
    const handling, and this byte-form does not by
    itself distinguish a direct literal from any other
    `&'static str` Constant value. The D6 diagnostic
    (§26.8) fires from the DiagLogPeel-time check,
    not from a post-RegseqModel re-derivation.
  - `args[i]` for `i ∈ 1..args.len()` has type `T_i`
    where `T_i` is one of the **value-typed** admissible
    operand shapes: an integer-width `Ty::Uint(UintTy::U8..U64)`
    or `Ty::Int(IntTy::I8..I64)`, `Ty::Bool`, or
    `Ty::Ref(Ty::Str, _)`. **Reference-companion source
    types** (`&u32`, `&mut u32`, `&bool`, etc., where the
    inner type is value-typed) are admissible source-level
    inputs per §26.4's `From<&T>` / `From<&mut T>` impls,
    but DiagLogPeel synthesises a load before the flat Call
    (§25.7.3 step 2), so the IR Call's args carry the loaded
    value-typed Operand -- never the reference-companion
    Operand. The `usize` / `isize` platform-width aliases
    route through their `#[cfg(target_pointer_width = ...)]`
    impls to `Ty::Uint(UintTy::U64)` / `Ty::Uint(UintTy::U32)`
    / `Ty::Int(IntTy::I64)` / `Ty::Int(IntTy::I32)` per
    target pointer width; the IR Call carries the matching
    fixed-width integer `Ty`, not `Ty::Uint(UintTy::Usize)`
    / `Ty::Int(IntTy::Isize)`. Reject types not admissible
    per the `From` matrix per D-14-2.7 D4.
  - Per-position template/operand compatibility
    (e.g., hex modifier requires integer-width
    operand) per D-14-2.7 D2; the placeholder modifier
    parsed from `args[0]`'s template content selects
    the per-position constraint.

  Indirect callees (`CallCallee::Indirect(Operand)`)
  cannot satisfy the log-intrinsic predicate per the
  B-4 exception's same rationale.

  See §26.6 for the canonical statement and §26.8 for
  the validation diagnostics.

- **B-7** -- **Closes construction invariant 9.**
  Every `Direct(InstanceId)` referenced from any
  Terminator::Call resolves in `model.instances`.
  `Indirect(Operand)` callees do not pin an InstanceId
  and do not contribute to the closure check.
  Load-bearing closure-correctness invariant the stage-1
  inbound expectation pinned for stage 9.

- **B-8** (Terminator-site projection of the §9.4
  global biconditional) -- For every
  `Call.callee = Direct(InstanceId)` in any Terminator
  whose resolved Regseq Instance has
  `flavor == FunctionInstanceFlavor::Virtual`, the
  Instance must satisfy `virtual_idx.is_some()`. The
  global biconditional `virtual_idx.is_some() iff
  flavor == Virtual` (covering all Function instances
  including those not appearing in any Call) lives at
  §9.4 + §3.3 step 8 model validation. B-8 here is
  the per-Terminator-site projection ensuring the
  stage-11 vtable-indirect rewrite's prerequisite
  holds at every call site whose callee will be
  rewritten.

- **B-9** (target/callee-output coherence) -- For
  every `Call`, `target.is_none()` (the call is
  divergent) iff the callee's return type is
  `Ty::Never`. Equivalently:
  - `target == Some(_)` requires callee output `!=
    Ty::Never`.
  - `target == None` requires callee output ==
    `Ty::Never`.
  Catches bridge-translation bugs that emit a
  diverging-target Call with a non-`!`-returning
  callee or vice versa. Same callee-output source as
  B-3: `callee_fn_sig(callee_id).output()`
  for Direct, `operand_ty(callee_operand).output()`
  for Indirect.

#### 23.6.3 Walk-implementation deferral

The walk-pass that applies B-0a/c + B-1-B-9 to every
Terminator in every basic block is **deferred to
stage 11** (same pattern as §19.7 / §20.5 / §21.5.3 /
§22.5.3). Stage 11 owns:

- `Body::validate(...)` walk over all Terminators in
  all basic blocks.
- Visitor-trait extensions surfacing `Terminator`,
  `CallCallee`, `SwitchTargets`, `AssertKind` at
  body-level walks.
- Actual `assert!` / `assert_eq!` checks for
  B-0a/c + B-1-B-9.
- Bridge-rejection diagnostic emission for §23.6.1 row
  encounters (most rejection happens at the bridge
  translation site, not at body-level walk; the walk
  catches stage-11-rewrite-pass-induced violations of
  the closed surface).

The deferral is filed in `stage-11/expectations.md`
"From stage 9" alongside `render_terminator` deferral
and the body-level vtable-indirect rewrite pass.

### 23.7 Bridge translation rules

Per-rustc-variant rules covering the 6 retained
variants + the 1 collapsed variant + the 8 rejected
variants.

**Goto.** Direct passthrough.
```
mir::TerminatorKind::Goto { target } ->
  TerminatorKind::Goto(<bridge_block_id>)
```

**SwitchInt.** Translate `discr` via stage-6 Operand
machinery; `targets` per the §23.3.2 explicit-split
form.
```
let n = mir_targets.targets.len();
let cases = (0..n-1).map(|i| (
    mir_targets.values[i].into(),     // Pu128 -> u128
    bridge_block_id(mir_targets.targets[i]),
)).collect::<Box<[_]>>();
let otherwise = bridge_block_id(mir_targets.targets[n-1]);
TerminatorKind::SwitchInt {
    discr:   translate_operand(&mir_discr),
    targets: SwitchTargets { cases, otherwise },
}
```

**Return / Unreachable.** Trivial passthrough.

**Call.** Four-case translation rule on `Call.func`:

| Rustc `Call.func` | RegseqModel `callee` |
|--------------------|----------------------|
| `Operand::Constant(c)` where `c.const_.ty() = Ty::FnDef(def_id, args)` resolves to a non-Virtual Instance | `Direct(<bridge-interned InstanceId>)` -- the resolved Regseq Instance has `kind = InstanceKind::Function { flavor: <non-Virtual variant>, .. }`. |
| `Operand::Constant(c)` where the resolved rustc Instance has `InstanceKind::Virtual(_, idx)` | `Direct(<bridge-interned InstanceId>)` -- the resolved Regseq Instance has `kind = InstanceKind::Function { flavor: FunctionInstanceFlavor::Virtual, virtual_idx: Some(idx), .. }` (per §9.4). The `virtual_idx` field stores rustc's vtable-slot index from `ty::InstanceKind::Virtual(_, idx)`. The §9.4 construction invariant `virtual_idx.is_some() iff flavor == Virtual` (projected at Direct virtual call sites by §23.6.2 B-8) ensures this field is always populated for Virtual-flavor Instances. **This is the pre-rewrite shape**; stage-11's body-level rewrite walks every `Direct(InstanceId)` and rewrites those whose resolved Instance carries `FunctionInstanceFlavor::Virtual` into `Indirect(Operand)` form via vtable load synthesis at `virtual_idx.unwrap()`. The bridge does NOT reject; it preserves rustc's resolved Virtual Instance verbatim and lets stage 11 do the rewrite. |
| `Operand::Move(p)` / `Operand::Copy(p)` where `p.ty() = Ty::FnPtr(...)` | `Indirect(Operand::Move(p))` / `Indirect(Operand::Copy(p))` |
| Anything else | bridge-rejection (mistyped callee shape) |

The remainder of the Call payload:
- `args`: `mir_args.iter().map(|sp|
  translate_operand(&sp.node)).collect()` -- strips
  the `Spanned` wrapper; drops `sp.span`
  (rustc_public-aligned at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`).
  See `discussions.md` D-9-4 for the consumer-
  landscape rationale and the supersession of
  stage-6 D-6-1.3's preservation expectation.
- `destination`: stage-5 Place machinery.
- `target`: bridge `BlockId` interner.
- Drop `unwind` / `call_source` / `fn_span`.

**Assert.** Translate per-field:
- `cond`: stage-6 Operand machinery.
- `expected`: passthrough.
- `msg`: `AssertKind` 8-variant translation (8 rustc
  variants map one-to-one; 3 coroutine-variants emit
  bridge-rejection per row 3/4-style discharge).
- `target`: bridge BlockId interner.
- Drop `unwind`.

**Drop.** Per §23.4, 5-step bridge translation rule
into a synthetic `TerminatorKind::Call`.

**Rejected variants** (FalseEdge, FalseUnwind, Yield,
CoroutineDrop, UnwindResume, UnwindTerminate, TailCall,
InlineAsm). Bridge emits the per-row diagnostic from
§23.6.1.

### 23.8 Worked examples

#### 23.8.1 Direct call with multiple args

**Source:**
```rust
let sum = my_func(a, b);
```

**rustc `optimized_mir`:**
```
bb3: {
  _5 = my_func(move _3, copy _4)
       -> [success: bb4, unwind unreachable];
}
```

**RegseqModel:**
```
bb3: {
  // ... pre-statements
  ----- terminator -----
  Terminator {
    span: src.rs:42:8,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Direct(<my_func InstanceId>),
      args:        [Operand::Move(_3), Operand::Copy(_4)],
      destination: _5,
      target:      Some(bb4),
    }
  }
}
```

#### 23.8.2 Indirect call through function pointer

**Source:**
```rust
let f: fn(i32) -> i32 = some_fn;
let x = f(5);
```

**rustc `optimized_mir`:**
```
bb3: {
  _5 = (move _f)(const 5_i32)
       -> [success: bb4, unwind unreachable];
}
```

**RegseqModel:**
```
bb3: {
  // ... pre-statements
  ----- terminator -----
  Terminator {
    span: src.rs:42:8,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Indirect(Operand::Move(_f)),
      args:        [Operand::Constant(<5_i32 ConstId>)],
      destination: _5,
      target:      Some(bb4),
    }
  }
}
```

#### 23.8.3 Drop collapsed to Call

**Source:**
```rust
{
    let s = SomeStruct::new();
    // ... use s
}  // drop(s) at end of scope
```

**rustc `optimized_mir`:**
```
bb3: {
  // ... earlier statements
  drop(_4) -> [return: bb4, unwind unreachable];
}
```
where `_4: SomeStruct` and `SomeStruct: Drop`.

**Bridge output (per §23.4):**
```
bb3: {
  // ... earlier statements
  _tmp_drop_arg = &raw mut _4;             // pre-statement Assign(Rvalue::RawPtr(Mut, _4))
  ----- terminator -----
  Terminator {
    span: src.rs:42:9,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Direct(<drop_glue_for_SomeStruct>),
      args:        [Operand::Move(_tmp_drop_arg)],
      destination: _tmp_drop_ret,            // fresh, ty = ()
      target:      Some(bb4),
    }
  }
}
bb4: {
  // ... continuation
}
```

#### 23.8.4 SwitchInt with otherwise branch

**Source:**
```rust
match x {
    0 => bb_zero(),
    1 | 2 => bb_one_or_two(),
    _ => bb_default(),
}
```

**rustc `optimized_mir`:**
```
bb0: {
  switchInt(_x) -> [0: bb_zero, 1: bb_one_or_two, 2: bb_one_or_two, otherwise: bb_default];
}
```

**RegseqModel:**
```
bb0: {
  ----- terminator -----
  Terminator {
    span: src.rs:42:5,
    kind: TerminatorKind::SwitchInt {
      discr:   Operand::Copy(_x),
      targets: SwitchTargets {
        cases: [
          (0, bb_zero),
          (1, bb_one_or_two),
          (2, bb_one_or_two),
        ],
        otherwise: bb_default,
      }
    }
  }
}
```

#### 23.8.5 Bounds-check Assert

**Source:**
```rust
let v = arr[i];   // bounds-checked indexing
```

**rustc `optimized_mir`:**
```
bb3: {
  _3 = Len(_1);
  _4 = Lt(copy _2, copy _3);
  assertEq(move _4, true, "index out of bounds: ...", move _3, copy _2)
       -> [success: bb4, unwind unreachable];
}
```

**RegseqModel:**
```
bb3: {
  _3 = Len(_1);
  _4 = Lt(_2, _3);
  ----- terminator -----
  Terminator {
    span: src.rs:42:13,
    kind: TerminatorKind::Assert {
      cond:     Operand::Move(_4),
      expected: true,
      msg:      AssertKind::BoundsCheck {
        len:   Operand::Move(_3),
        index: Operand::Copy(_2),
      },
      target:   bb4,
    }
  }
}
```

#### 23.8.6 Virtual dispatch (post-stage-11 rewrite shape)

**Source:**
```rust
fn dispatch(obj: &dyn Driver) {
    obj.start();
}
```

**rustc `optimized_mir`:**
```
bb0: {
  _0 = <dyn Driver as Driver>::start(move _obj)
       -> [success: bb1, unwind unreachable];
}
```
where rustc resolves `<dyn Driver as Driver>::start` to
an Instance with `InstanceKind::Virtual(start_def_id,
method_idx)`.

**RegseqModel pre-rewrite (immediately after bridge
translation; before stage-11 rewrite):**
```
bb0: {
  ----- terminator -----
  Terminator {
    span: src.rs:42:5,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Direct(<id_for_virtual>),
                   // resolved Instance has kind =
                   //   InstanceKind::Function {
                   //     flavor:      FunctionInstanceFlavor::Virtual,
                   //     virtual_idx: Some(method_idx),
                   //     ..
                   //   }
      args:        [Operand::Move(_obj)],   // type: &dyn Driver (fat pointer)
      destination: _0,
      target:      Some(bb1),
    }
  }
}
```
Note that pre-rewrite, args[0] type is `&dyn Driver`
while `callee_fn_sig(callee_id).inputs()[0]` is
`*mut ()` (per §13.2 + §17.2.3, the vtable's erased
receiver type is uniformly `Ty::Ptr(unit,
Mutability::Mut)` regardless of source borrow flavor;
resolved per the §23.6.2 helper through
`inst.kind.as_function().fn_sig` under stage-12
D-12-1.4 / Option B). B-6 (per-arg type match) is
**violated** in this pre-rewrite shape by design;
stage-11 sequencing runs the rewrite before the
validate walk.

**RegseqModel post-rewrite (after stage-11 vtable-
indirect rewrite pass):**
```
bb0: {
  _t1 = move _obj;                                    // fat pointer
  _t2 = PtrMetadata(move _t1);                        // *const VtableType
  _t3 = (*_t2).start_field;                           // fn(*mut ()) -> ()
  ----- terminator -----
  Terminator {
    span: src.rs:42:5,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Indirect(Operand::Move(_t3)),
      args:        [Operand::Move(_obj_data)],   // *mut () extracted from fat pointer
      destination: _0,
      target:      Some(bb1),
    }
  }
}
```
Post-rewrite, B-6 holds: args[0] type matches the
indirect callee operand's `Ty::FnPtr(sig)` resolved-
signature inputs()[0] (both `*mut ()` per §13.2 +
§17.2.3's uniform-mutable-erased-receiver rule).

Charon's `transform_dyn_trait_calls.rs:31-122` is the
direct template for this rewrite; the body-level pass
implementation is filed in `stage-11/expectations.md`.

#### 23.8.7 Divergent call (`-> !` callee)

**Source:**
```rust
panic!("driver failure");
```

**rustc `optimized_mir`:**
```
bb3: {
  panic_str("driver failure")
       -> unwind unreachable;
}
```
(Note: divergent call has no success-path target; rustc
encodes this as `target: None`.)

**RegseqModel:**
```
bb3: {
  ----- terminator -----
  Terminator {
    span: src.rs:42:5,
    kind: TerminatorKind::Call {
      callee:      CallCallee::Direct(<panic_str InstanceId>),
      args:        [Operand::Constant(<"driver failure" ConstId>)],
      destination: _<unused>,                // any LocalId; never read
      target:      None,                     // divergent
    }
  }
}
```

Codegen pattern: `panic_str(...); __builtin_unreachable();`.

### 23.9 Rendering

See `pretty-printer.md` §7.5 (top-level `render_terminator`),
§7.6 (`render_terminator_kind`), §7.7
(`render_call_callee`), §7.8 (`render_switch_targets`),
§7.9 (`render_assert_kind`).


---


## 24. Basic-block vocabulary

The `BasicBlock` vocabulary is the **per-block container
surface** of MIR -- the structural unit binding stage-8
`Statement` and stage-9 `Terminator` content into the
unit of control flow. The bridge fills it from rustc's
`mir::BasicBlockData<'tcx>` and stores the body-level
table as `IndexVec<BlockId, BasicBlock>`; downstream
consumers (stage 11 bodies, stage 14 backends) read it
as the per-body collection of straight-line statement
sequences each ending in exactly one control-flow
transfer.

Stage 10 sits **sixth in the MIR pipeline** (stages
5-11 form a strict bottom-up chain: places -> operands
-> rvalues -> statements -> terminators -> **basic
blocks** -> bodies). Stage-10 content embeds stage-8
Statement (per `BasicBlock.statements`) and stage-9
Terminator (per `BasicBlock.terminator`) vocabulary;
stage-10 introduces the `BlockId` body-local typed-index
that stage-11 `Body` and stage-14 backends consume.

### 24.1 `BasicBlock` struct

#### 24.1.1 Struct definition

```rust
/// A basic block: zero or more straight-line statements
/// followed by exactly one control-flow terminator.
/// Body-local content; lives inline inside the body's
/// basic-block table at `Body.basic_blocks` (per §24.2).
/// Equality is structural.
pub struct BasicBlock {
    /// The straight-line instruction sequence preceding
    /// the terminator. Finalized single-fill payload
    /// per §24.1.2; consumers index by `usize` or
    /// iterate via `&[Statement]` deref.
    pub statements: Box<[Statement]>,

    /// The control-flow transfer ending this block.
    /// Exactly one terminator per block (no Option;
    /// see §24.1.3).
    pub terminator: Terminator,
}
```

The struct carries exactly two fields. **Dropped from
rustc's `BasicBlockData<'tcx>`:**

- `is_cleanup: bool` -- the MSVC-unwind cleanup-block
  marker. Pipeline-discharged under panic=abort by
  stage-9's D-9-9 rows 5-6 (`AbortUnwindingCalls` +
  `SimplifyCfg::PreOptimizations` cleanup-block
  pruning). See §24.1.4.
- `after_last_stmt_debuginfos: StmtDebugInfos<'tcx>` --
  trailing-debuginfo container. Stage-8 D-8-3
  debuginfo discharge applies symmetrically: no DWARF
  emission target, so no debuginfo carry. See §24.1.5.

**Collapsed from rustc:**
- `terminator: Option<Terminator<'tcx>>` -- rustc's
  Option is construction-only (per `mod.rs:1318-1324`
  docstring); RegseqModel collapses to non-Optional at
  the bridge boundary, encoding the post-construction
  invariant at the type level. See §24.1.3.

**No block-level metadata** (no span, no scope; spans
live on Statement and Terminator individually). See
§24.1.6.

#### 24.1.2 `statements: Box<[Statement]>` storage

The `statements` field uses `Box<[Statement]>`
(finalized single-fill payload), not `Vec<Statement>`
(mutable, peer-aligned).

Rationale: RegseqModel's existing pattern for finalized
payloads uses `Box<[T]>`:
- Stage-7 `Rvalue::Aggregate(_, Box<[Operand]>)` per §21.
- Stage-9 `SwitchTargets.cases: Box<[(u128, BlockId)]>`
  per §23.3.2.

Basic-block statements have the same lifecycle: the
bridge fills once, consumers read; mutation is not a use
case. `Box<[Statement]>` saves 8 bytes per block (no
spare-capacity field) at no information loss.
Bridge-side construction uses a working `Vec<Statement>`
buffer, finalized via `into_boxed_slice()` at
block-construction completion (per §24.7).

Consumer ergonomics are unchanged: `Box<[T]>` derefs to
`&[T]`, so `block.statements.iter()`,
`block.statements.len()`, and `block.statements[i]` work
identically to a `Vec<Statement>` field.

#### 24.1.3 `terminator: Terminator` non-Optional

The `terminator` field is non-Optional. rustc's
`BasicBlockData.terminator: Option<Terminator<'tcx>>` is
documented as None-only-during-construction
(`rustc_middle/src/mir/mod.rs:1318-1324`); the
`BasicBlockData::terminator()` accessor at `mod.rs:1357`
panics on None.

RegseqModel consumes post-construction MIR (the
`optimized_mir` query result), so the construction-time
None state is structurally inaccessible at the
consumption boundary. RegseqModel mirrors the structural
guarantee at the type level rather than carrying a
runtime invariant, eliminating per-walk-site `unwrap`s.

Three of four peers agree: rustc_public's `BasicBlock`
at `body.rs:136`, Charon's `BlockData` at
`ullbc_ast.rs:128`, and mrustc's `BasicBlock` at
`mir.hpp:729` all store non-Optional terminators.

This becomes positive invariant **B-Terminator-Some**
(structural / type-level) in §24.6.2.

#### 24.1.4 Dropped: `is_cleanup`

Per stage-9 D-9-9 rows 5-6, the panic=abort discharge
chain (`AbortUnwindingCalls` at
`rustc_mir_transform/src/abort_unwinding_calls.rs:115-121,137-138`
+ cleanup-block pruning under panic=abort) eliminates
all `is_cleanup=true` blocks before MIR reaches the
bridge. The field carries no driver-relevant
information at our consumption point; carrying it would
be a parallel "is panic=abort" marker that violates the
README §2 compositional rule.

Defensive bridge-side rejection if encountered is row 1
of the §24.6.1 rejection table.

#### 24.1.5 Dropped: `after_last_stmt_debuginfos`

Per stage-8 D-8-3 debuginfo discharge, RegseqModel emits
no DWARF; the per-statement `debuginfos: StmtDebugInfos`
field was dropped at stage 8. The basic-block-level
`after_last_stmt_debuginfos: StmtDebugInfos<'tcx>` is
the symmetric basic-block-level analog and shares the
same disposition: silent drop at the bridge.

This is **not** a defensive rejection (unlike
`is_cleanup`); the field is silently absent in
RegseqModel's `BasicBlock`. Listed in §24.6.1 row 3 for
table completeness only.

#### 24.1.6 No block-level metadata

`BasicBlock` carries no block-level span, scope, or
other metadata fields. rustc's `BasicBlockData` carries
none either; spans propagate through the per-statement
`Statement.span` (§22.1.1, stage-8 D-8-3) and
per-terminator `Terminator.span` (§23.1.1, stage-9
D-9-8). All four peers (rustc, rustc_public, Charon,
mrustc) match this convention.

### 24.2 Body-level basic-block container

#### 24.2.1 Storage shape

The body-level basic-block table is stored as a typed
`IndexVec`:

```rust
// (Belongs to stage-11 Body struct; stage 10 specifies
// the field's type and invariants. See §24.4 for
// ordering invariants and §24.6 for the per-stage
// invariants registry.)
pub basic_blocks: IndexVec<BlockId, BasicBlock>,
```

Aligned with stage-1 §2.6's `IndexVec<I, T>` canonical
typed-index dense storage convention (every existing
crate-level interner table -- `model.types`,
`model.consts`, `model.items`, `model.instances`,
`model.sigs`, `model.strings`, `model.source_files`,
`model.crates` -- uses `IndexVec<I, T>`). Per stage-12
D-12-1.4 / Option B (supersedes stage-11 D-11-5), bodies
live structurally inside Function Instances on
`InstanceKind::Function { body: Option<Body>, ... }`
per §9.4; no separate `model.bodies` table.

#### 24.2.2 No wrapper struct

The field is the bare type `IndexVec<BlockId, BasicBlock>`,
not a wrapper struct (à la rustc's `BasicBlocks<'tcx>`).
The only reason to wrap would be to attach methods; the
methods rustc attaches are CFG-related (predecessors,
reverse_postorder, dominators). Per Q1 (architectural
crate split), those belong in `regseq-analysis`, not on
the model type.

3-of-4 peer alignment: rustc_public's `Body.blocks:
Vec<BasicBlock>`, Charon's `Body.body: Vector<BlockId,
BlockData>`, and mrustc's `Function.blocks:
std::vector<BasicBlock>` all use bare storage.

#### 24.2.3 No CFG cache; principled model-vs-analysis rule

RegseqModel does NOT cache CFG analyses (predecessors,
dominators, post-dominators, loop structure, liveness)
on the body-level container. CFG analyses live in
`regseq-analysis`, computed on demand.

The principled rule:

> **The model stores facts the bridge owns or rustc has
> resolved (types, layouts, const values, instance
> identities). The model does not store analyses
> derivable from those facts via standard CFG/SSA
> algorithms.**
>
> The single exception -- RPO ordering of
> `basic_blocks` storage (§24.4.1) -- is justified by
> being the layout itself, not a separate field; and by
> being the order rustc's `ReorderBasicBlocks` already
> produces.

Rationale stack:
1. By the time the bridge consumes the body, rustc's
   own CFG cache (`Arc<Cache>` at
   `rustc_middle/src/mir/basic_blocks.rs:32-37`) is
   empty. `ReorderBasicBlocks` calls
   `reverse_postorder()` once during the reorder, then
   `as_mut()` immediately invalidates the cache at
   `rustc_mir_transform/src/prettify.rs:36`. So
   "preserve what rustc gave us" doesn't apply --
   there is nothing free to preserve.
2. Populating a `dominators` field would require the
   bridge to *invoke* the computation; the work
   doesn't disappear, only the boundary moves.
3. Predecessors / dominators / loop structure /
   liveness all derivable from primitive content;
   storing them is shadow data that must be kept in
   sync with the underlying CFG.
4. Serde wire format hygiene (Q5): derived analyses
   on the wire muddy the contract that primitive
   content alone round-trips.
5. F4/F4-2 cross-build identity surface: derived
   analyses shouldn't participate in cross-build
   identity; carrying them as `pub` fields would need
   either canonicalization or carve-out.
6. 3-of-4 peer alignment: rustc_public, Charon, mrustc
   all decline to cache. rustc_middle's `Arc<Cache>` is
   motivated by being inside the compiler (long-lived,
   reused across passes); RegseqModel is consumed once
   per session per body.

#### 24.2.4 No type alias

The field declaration uses the bare type
`IndexVec<BlockId, BasicBlock>` directly (no `pub type
BasicBlocks = IndexVec<BlockId, BasicBlock>` alias).
The field appears in only a few places (the stage-11
`Body` struct, analysis helper signatures); a semantic
alias adds nothing. If a future stage / phase-2
device-pressure surfaces real ergonomic friction,
introduce the alias then. Per the README "Don't add
features beyond what the task requires" rule.

### 24.3 `BlockId` typed-index

#### 24.3.1 `newtype_index!` application

`BlockId` is introduced as a `newtype_index!` invocation
in `crates/regseq-model/src/idx.rs`:

```rust
newtype_index! {
    pub struct BlockId {
        const START_BLOCK = 0;
    }
}
```

The macro generates u32-backed niche-encoded storage
(via stage-1's private `NonMaxU32`), the `Idx` trait
impl, manual `Ord`/`PartialOrd` over the logical index,
and a `Debug` impl rendering as `BlockId(N)` (per
stage-1 conventions). The `START_BLOCK` const is
emitted as `pub const START_BLOCK: BlockId =
BlockId::from_u32(0);` inside the impl, semantically
aliasing the auto-generated `BlockId::ZERO` with
explicit entry-block-by-IR-convention naming.

This rejects rustc_public's bare `BasicBlockIdx = usize`
alias (`rustc_public/src/mir/body.rs:40`) on stage-1
ID-discipline grounds: no niche optimisation, no sigil
hook, no type-safety against `LocalId` confusion.

#### 24.3.2 Macro extension: const-body arm

The `newtype_index!` macro at
`crates/regseq-model/src/idx.rs:75-160` is extended with
a const-body arm so the form above is accepted. Grammar:

```
newtype_index! {
    pub struct $Name {
        const NAME_1 = INT_LITERAL_1;
        const NAME_2 = INT_LITERAL_2;
        ...
    }
}
```

Each `const NAME_K = INT_LITERAL_K;` line generates
`pub const NAME_K: $Name = $Name::from_u32(INT_LITERAL_K);`
inside the impl. Multi-const support is included
(zero-extra-cost) for IDs that may declare multiple
named consts. Integer-literal-only (not general const
expressions) -- the use cases (RETURN_PLACE = 0,
START_BLOCK = 0) only need integer literals; per the
README "Don't add features beyond what the task
requires" rule.

The existing empty-body arm (`pub struct $Name {}`)
continues to work unchanged.

`LocalId::RETURN_PLACE` migration to this arm is
deferred to stage 11 (LocalId formally lands at
stage 11 alongside `Body.local_decls`); see outbound
expectation in stage-10 `discussions.md` §2 item 5.

Direct grammar match to rustc's
`pub struct BasicBlock { const START_BLOCK = 0; }`
at `rustc_middle/src/mir/mod.rs:1289-1291`.

#### 24.3.3 Size and niche assertions

The following static assertions append to
`crates/regseq-model/src/lib.rs`, matching the existing
crate-level-ID pattern at lines 40-57:

```rust
const _: () = assert!(core::mem::size_of::<BlockId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<BlockId>>() == 4);
```

Both assertions are non-vacuous: `BlockId == 4` checks
the u32 backing; `Option<BlockId> == 4` catches a
regression where the `NonMaxU32` niche encoding silently
breaks.

#### 24.3.4 Sigil convention `bbN`

The textual sigil convention for `BlockId(N)` is `bbN`
(e.g., `BlockId(0)` -> `bb0`, `BlockId(3)` -> `bb3`).
Universal across all four peers: rustc's
`#[debug_format = "bb{}"]` at
`rustc_middle/src/mir/mod.rs:1288`, rustc_public's
manual `bb{index}` at `pretty.rs:38-88`, Charon's
`bb{}` block-label render at
`src/pretty/fmt_with_ctx.rs:748` (within
`:736-756`; `:206-214` covers block-content rendering,
not the label), mrustc's `bbN` envelope at
`src/mir/dump.cpp:26-48`.

The macro's auto-generated `Debug` impl is unchanged
(renders `BlockId(N)`, uniform with all other typed
IDs per stage-1 convention). The `bbN` sigil belongs
to RegseqModel's *custom textual format* (Q5 primary
serialization), not to Rust's `Debug`. The
sigil-rendering helper that emits `bbN` is
renderer-injected at the BasicBlock-rendering envelope
in `pretty.rs`, introduced at stage 11 alongside its
first consumer (§24.9 / D-10-K.2).

### 24.4 Block-ordering invariants

Two positive invariants the bridge guarantees, encoded
structurally in the IR's storage shape and entry-block
const:

#### 24.4.1 B-RPO -- storage in reverse-postorder

> `Body.basic_blocks: IndexVec<BlockId, BasicBlock>` is
> stored in reverse-postorder of the CFG rooted at
> `BlockId::START_BLOCK`. Iterating the storage
> (`basic_blocks.iter_enumerated()`) yields blocks in
> RPO; consumers may exploit this without re-deriving.

Inheritance: rustc's `ReorderBasicBlocks` cleanup pass
at `rustc_mir_transform/src/prettify.rs:14-42` reorders
the IndexVec storage into reverse-postorder. The pass
has `is_required: false` in rustc's pipeline but is
force-enabled via our cleanup whitelist:
`crates/regseq-driver/src/callbacks.rs:30` sets
`("ReorderBasicBlocks".to_string(), true)` in
`regseq_mir_passes()`.

The invariant carries no back-edge carve-out: RPO is a
graph-structural property; whether a given edge is
forward (`j > i`) or back (`j <= i`) falls out of the
topology. Consumers detecting back-edges use standard
CFG algorithms in `regseq-analysis`.

This is the single carve-out to the §24.2.3
"model-vs-analysis line" rule: RPO is encoded *as the
storage layout itself* (zero extra bytes; the IndexVec
needs *some* order, and RPO is the natural one given
`ReorderBasicBlocks` already produces it).

Walk-checked at stage 11 (per §24.4.3).

#### 24.4.2 B-Entry -- `START_BLOCK = BlockId(0)`

> `BlockId::START_BLOCK == BlockId(0)` is the entry
> block of every `Body`.

Inheritance: rustc's `START_BLOCK = 0` const at
`rustc_middle/src/mir/mod.rs:1290`, preserved by
`ReorderBasicBlocks` via the explicit fixed-point at
`rustc_mir_transform/src/prettify.rs:33`
(`debug_assert_eq!(updater.map[START_BLOCK], START_BLOCK)`).

The named const introduced at §24.3.1 carries the
semantic meaning: `BlockId::START_BLOCK` is "the entry
block by IR convention," not just "the const value 0".
Pretty-printer renders `BlockId(0)` as `bb0` (see
`pretty-printer.md` §3.3); the entry-ness is
conventional, not a separate sigil.

#### 24.4.3 Walk-implementation deferred to stage 11

The walk that jointly enforces B-RPO, B-Reachable,
B-Entry (per §24.6.2) is deferred to stage 11's
body-walk validator alongside the other deferred walks
(per D-5-2 Part C / D-7-3 Part C / D-8-4 Part C /
D-9-10 Part C). Stage 11 owns the body-level walk that
proceeds in a fixed order:

1. **Pre-check B-NonEmpty.** Assert
   `!body.basic_blocks.is_empty()`. A zero-length
   body is rejected here before any traversal; this
   guarantees `BlockId(0)` is a valid index for the
   subsequent steps.
2. **Pre-check B-Successors-Valid.** For each block
   in `body.basic_blocks.iter_enumerated()`, for each
   `BlockId` in `block.terminator.successors()` (per
   §23.1.4), assert
   `successor.index() < body.basic_blocks.len()`.
   Performing this check **before** the DFS prevents
   an invalid successor from causing an `IndexVec`
   indexing panic during step 3 (rustc's traversal
   indexes `self.basic_blocks[bb]` during visit per
   `rustc_middle/src/mir/traversal.rs:124-134`,
   so an out-of-range successor would otherwise
   surface as a panic rather than a clean
   B-Successors-Valid diagnostic).
3. **Compute RPO from BlockId(0).** Perform a
   depth-first traversal from `BlockId::START_BLOCK ==
   BlockId(0)` using `Terminator.successors()` (per
   §23.1.4 per-variant successor enumeration).
   Record each block in post-order (visit children
   before parent); reverse the post-order list to
   obtain reverse-postorder. This mirrors rustc's
   `BasicBlocks::reverse_postorder()` at
   `rustc_middle/src/mir/basic_blocks.rs:72-78`.
   Step 2's pre-check guarantees every traversed
   `BlockId` is a valid index.
4. **Assert B-Reachable.** Assert the computed RPO
   list has `len == body.basic_blocks.len()`. If
   shorter, some block is unreachable from
   `BlockId(0)` -- B-Reachable violation.
5. **Assert B-RPO.** For each `i` in
   `0..body.basic_blocks.len()`, assert that the
   computed RPO list at position `i` equals
   `BlockId::from_u32(i as u32)`. If any position
   differs, the storage order does not match the
   CFG's RPO -- B-RPO violation.
6. **Assert B-Entry structurally.** Step 1 has
   already established `body.basic_blocks.len() >= 1`;
   step 5 places `BlockId(0)` at position 0 by RPO
   construction (since DFS started at `BlockId(0)`,
   it is the last node post-order-visited, hence the
   first node in the reversed list). This mirrors
   rustc's pass-time fixed-point at
   `rustc_mir_transform/src/prettify.rs:33`.

Step 4 covers B-Reachable; step 5 covers B-RPO; step 6
covers B-Entry. Steps 1-2 are pre-checks that prevent
the DFS from indexing-panicking; the post-DFS
assertions in steps 4-6 are walk-checks rather than
vacuous claims.

#### 24.4.4 No defensive fallback for non-RPO inputs

The cleanup-pass whitelist is part of the
pinned-toolchain contract (per `architecture.md` Q6 +
`crates/regseq-driver/src/callbacks.rs:25-36`).
Mis-configuration is a setup bug, not a runtime
concern. Stage-11's walk validator may raise a
diagnostic if it observes RPO violation
post-construction, but stage-10 does not carve out a
non-RPO handling path in the spec surface (per the
README "no defensive fallbacks" methodology rule).

### 24.5 Identity and storage

Basic blocks are **body-local content**. Each
`BasicBlock` lives inline inside the body's basic-block
table at `Body.basic_blocks: IndexVec<BlockId,
BasicBlock>` (per §24.2); there is **no top-level
`basic_blocks` interner table** and **no `BasicBlockId`
crate-level index type**.

`BlockId` is the body-local index *into* a single
body's basic-block table; it is NOT a project-level
interner key. `BlockId(N)` references in stage-9
Terminator content (`Goto.target`,
`SwitchInt.targets.cases[i].1`,
`SwitchInt.targets.otherwise`, `Call.target`,
`Assert.target`) are interpreted relative to the
enclosing body's basic-block table.

**Equality and hashing** are `derive(PartialEq, Hash)`
on `BasicBlock`, walking through to stage-8 Statement
and stage-9 Terminator structural derives, then through
to embedded Operand / Place / InstanceId / TyId /
ConstId per stages 5-8.

**F4 / F4-2 cross-build identity is OUT of scope.**
`BasicBlock` is body-local content; it does not
participate in cross-build identity (the F4-2 instance
ordering canonicalises `(InstanceKey)`, not
`(BodyContents)`). Block storage and `BlockId`
numbering are per-build, structural, and need not be
canonicalised across build environments. Same scope
reduction as stages 5-9.

### 24.6 Basic-block-site invariants

5-part registry: rejection table (Part A; §24.6.1),
positive invariants (Part B; §24.6.2), walk-implementation
deferral (Part C; §24.6.3), F4/F4-2 OUT-of-scope note
(Part D; §24.6.4), forward-looking considerations
(Part E; §24.6.5).

#### 24.6.1 Rejection table (3 rows)

Two rejection categories: pipeline-discharged (defensive
against pipeline misconfiguration) and silent-drop
(NOT rejected; for table completeness).

| # | rustc construct | Category | Discharge mechanism | Diagnostic message |
|---|---|---|---|---|
| 1 | `BasicBlockData.is_cleanup = true` | Pipeline-discharged (panic=abort) | Stage-9 D-9-9 rows 5-6: `AbortUnwindingCalls` rewrites unwind edges to `UnwindAction::Unreachable` at `rustc_mir_transform/src/abort_unwinding_calls.rs:115-121` + dead cleanup-block removal at `:137-138`; cleanup blocks become unreachable; `SimplifyCfg::PreOptimizations` is scheduled in rustc runtime cleanup (`rustc_mir_transform/src/lib.rs:666-680`; `is_required()` returns `false` at `simplify.rs:101-103`) and prunes them in practice. The bridge still rejects any survivor at construction. | "internal error: is_cleanup=true block survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy" |
| 2 | `BasicBlockData.terminator = None` post-construction | Defensive (rustc invariant) | Per `rustc_middle/src/mir/mod.rs:1318-1324` docstring: terminator is None *only during construction*; `BasicBlockData::terminator()` accessor at `mod.rs:1357` panics on None. RegseqModel's bridge consumes post-construction MIR. | "internal error: terminator: None survived MIR construction -- pipeline misconfiguration or post-construction state corruption" |
| 3 | `BasicBlockData.after_last_stmt_debuginfos` non-empty | **Silent drop (NOT rejected)** | Stage-8 D-8-3 debuginfo discharge (no DWARF emission target). The field is silently dropped at the bridge. Listed for table completeness so the implementer doesn't list it as "carried." | n/a -- silent drop, no diagnostic |

Rows 1-2 emit an `unsupported_construct!`-style
diagnostic at the bridge translation match site;
row 3 emits no diagnostic (it's a bridge translation
silent drop).

**Diagnostic discipline.** Same `unsupported_construct!`-style
pattern used at stages 1, 3, 5, 6, 7, 8, 9. The
post-construction None case (row 2) is structurally
preempted by the §24.1.3 type-level non-Optional
encoding -- the bridge's match site translating
`BasicBlockData::terminator: Option<...>` to
RegseqModel's non-Optional `terminator: Terminator`
emits the diagnostic on the None arm.

#### 24.6.2 Positive invariants (8)

2 inherited + 6 stage-10-introduced (8 total).

**Inherited (2):** stage-10 BasicBlock embeds Statement
(via `statements: Box<[Statement]>`) and Terminator
(via `terminator: Terminator`); stage-8 Statement-level
and stage-9 Terminator-level invariants propagate
transitively.

- **B-0a** -- All embedded Statement content satisfies
  Statement invariants per stage-8 §22.5.2.
- **B-0b** -- All embedded Terminator content
  satisfies Terminator invariants per stage-9 §23.6.2.

**Stage-10-introduced (6):**

- **B-RPO** -- `Body.basic_blocks` storage is in
  reverse-postorder of the body's CFG rooted at
  `BlockId::START_BLOCK`. Inherited from
  `ReorderBasicBlocks` in the cleanup whitelist
  (§24.4.1). Walk-checked at stage 11 (per §24.4.3).
- **B-Reachable** -- All blocks in `Body.basic_blocks`
  are reachable from `BlockId::START_BLOCK` via
  `Terminator.successors()` traversal. **Verified by
  the stage-11 DFS-RPO walk procedure** (§24.4.3
  step 4, after the step-3 DFS-RPO computation: a
  DFS from `BlockId(0)` that yields fewer blocks
  than `body.basic_blocks.len()` flags a B-Reachable
  violation). The spec asserts B-Reachable
  at the bridge boundary; rustc's pipeline produces
  reachable-only bodies in practice (the standard
  scheduling at `rustc_mir_transform/src/lib.rs:666-680`
  includes `SimplifyCfg::PreOptimizations` and
  `remove_dead_blocks` at
  `rustc_mir_transform/src/simplify.rs:79-85,349-383`
  which prune unreachable blocks), but the spec does
  not lean on this scheduling for soundness -- the
  stage-11 walk is the authoritative check. Per the
  README "no defensive fallbacks" methodology rule:
  the walk verifies; we do not duplicate
  rustc-pipeline assumptions in spec text.
- **B-Entry** -- `BlockId::START_BLOCK == BlockId(0)`
  is the entry block of every `Body` (§24.4.2).
- **B-Terminator-Some** -- Every `BasicBlock.terminator`
  is well-formed (no `Option::None`). Type-level
  guarantee per §24.1.3 (non-Optional field type);
  no runtime check needed.
- **B-Successors-Valid** -- Every `BlockId` referenced
  as a successor in any Terminator is `<
  body.basic_blocks.len()`. Body-level invariant
  (BlockId range only meaningful relative to the
  containing body); applies to:
  - `Terminator.kind = Goto(target)` -> `target <
    len()`.
  - `Terminator.kind = SwitchInt { targets: SwitchTargets
    { cases, otherwise } }` -> every `cases[i].1 <
    len()` and `otherwise < len()`.
  - `Terminator.kind = Call { target: Some(t), ... }`
    -> `t < len()`.
  - `Terminator.kind = Assert { target, ... }` ->
    `target < len()`.
- **B-NonEmpty** -- Every body has at least one
  `BasicBlock` (the entry block). Falls out of B-Entry
  (an entry-at-BlockId(0) requires `len() >= 1`); stated
  explicitly for walk-clarity.

#### 24.6.3 Walk-implementation deferred to stage 11

Rules enumerated at this stage; the walk implementation
applying them is deferred to stage 11's body-validation
step. Mirrors D-5-2 Part C / D-7-3 Part C / D-8-4 Part C /
D-9-10 Part C walk-impl deferrals.

**Walk ownership split.** Part A rejection rules
(§24.6.1 rows 1-2) are **bridge-enforced at
construction**, not stage-11-walk-enforced: rows 1-2
inspect rustc-side fields (`is_cleanup`,
`Option<Terminator>`) that are gone after the bridge
finalises `BasicBlock` per §24.7. The bridge raises
`unsupported_construct!`-style diagnostics at the match
sites in §24.7 step 2a (`is_cleanup=true`) and 2b
(`terminator: None`). Row 3 is silent-drop with no
diagnostic (per §24.7 step 2c). Stage-11 walk
**inherits the post-construction guarantee** that no
BasicBlock can exhibit a Part-A condition --
RegseqModel's `BasicBlock` shape (per §24.1) makes
those conditions structurally inexpressible.

Stage 11 owns the body-level walk that applies the
**RegseqModel-visible Part B positive invariants** in
the fixed step order detailed in §24.4.3 to ensure
clean diagnostics:
- **Step 1 -- B-NonEmpty pre-check**:
  `!body.basic_blocks.is_empty()` (must precede any
  traversal so subsequent indexing is well-defined).
- **Step 2 -- B-Successors-Valid pre-check**:
  per-Terminator successor-range check
  (`successor.index() < body.basic_blocks.len()`)
  iterated over `body.basic_blocks.iter_enumerated()`
  before the DFS. Performing this check **before**
  the DFS-RPO traversal prevents an invalid successor
  from triggering an `IndexVec`-indexing panic during
  the visit (the panic would obscure a clean
  B-Successors-Valid diagnostic; cf. rustc's traversal
  indexing pattern at
  `rustc_middle/src/mir/traversal.rs:124-134`).
- **Steps 3-6 -- B-RPO + B-Reachable + B-Entry**:
  jointly checked via the DFS-RPO procedure
  (single DFS from `BlockId(0)` computes RPO; length
  comparison enforces B-Reachable; positional
  comparison enforces B-RPO; starting-from-BlockId(0)
  with B-NonEmpty pre-cleared makes B-Entry
  structural).
- **B-0a / B-0b** inherited from per-Statement /
  per-Terminator walks already running at stage 11
  (compositional with §22.5.3 and §23.6.3).
- **B-Terminator-Some** structurally encoded at the
  type level (§24.1.3); no runtime check needed.

Compositional with the §22.5.3 Statement walk and
§23.6.3 Terminator walk: a stage-11 visitor that walks
basic-block content visits Statement and Terminator
invariants in the same pass.

#### 24.6.4 F4 / F4-2 cross-build identity OUT of scope

`BasicBlock` is body-local content; doesn't participate
in cross-build identity (the F4-2 instance ordering
canonicalises `(InstanceKey)`, not `(BodyContents)`).
Block storage and `BlockId` numbering are per-build,
structural, and need not be canonicalised across build
environments. Same scope reduction as stages 5-9.

#### 24.6.5 Future considerations (not stage-10 invariants)

Stage 10 enforces reachability (B-Reachable, §24.6.2)
and inherits density from `IndexVec<I, T>`'s stage-1
§2.6 canonical-storage construction (no gaps possible).
This subsection records forward-looking concerns that
are NOT stage-10 invariants but may become spec
concerns at later stages:

- **Per-instance const-folding of type-dependent
  branches.** rustc's codegen-side
  `mono_reachable_reverse_postorder`
  (`rustc_codegen_ssa/src/mir/mod.rs:250`) filters
  blocks made unreachable by per-instance const-eval
  of branches like `if T::IS_FOO { ... }`. RegseqModel's
  bridge does NOT apply this filter; the per-instance
  Body's `basic_blocks` reflects the generic-body
  CFG (post-`SimplifyCfg`, post-`ReorderBasicBlocks`),
  with per-instance const-folding deferred to stage-14
  codegen or to a hypothetical stage-11 simplification
  pass. If a future stage adds bridge-side per-instance
  const-folding, that stage owns the policy (prune at
  transform-time vs. carry with per-instance
  reachability filtering at consumption); stage 10
  does not pre-empt it.

This is a forward-looking note, not a stage-10 spec
weakening. B-Reachable holds at the bridge output as
established in §24.6.2.

### 24.7 Bridge translation rules

The bridge translates rustc's
`mir::BasicBlocks<'tcx>` and per-block
`mir::BasicBlockData<'tcx>` into RegseqModel's
`IndexVec<BlockId, BasicBlock>`. Per-body procedure:

1. **Allocate `IndexVec<BlockId, BasicBlock>`**
   with `body.basic_blocks.len()` capacity (rustc's
   `body.basic_blocks` is post-`ReorderBasicBlocks`
   so already in RPO).
2. **For each rustc `(BasicBlock, BasicBlockData)` pair
   in `body.basic_blocks.iter_enumerated()`:**
   a. **Reject if `data.is_cleanup == true`** (§24.6.1
      row 1; pipeline-misconfiguration diagnostic).
   b. **Reject if `data.terminator == None`** (§24.6.1
      row 2; post-construction-corruption diagnostic).
      *Implementation note:* the match destructuring
      `Option<Terminator>` already separates the Some
      and None arms; row 2's diagnostic emits on the
      None arm.
   c. **Silently drop `data.after_last_stmt_debuginfos`**
      (§24.6.1 row 3; no diagnostic).
   d. **Translate `data.statements: Vec<Statement<'tcx>>`**
      to a working `Vec<RegseqStatement>` via stage-8
      §22.6 bridge translation rules (per-Statement),
      then `.into_boxed_slice()` to materialize
      `Box<[Statement]>` (§24.1.2).
   e. **Translate `data.terminator: Terminator<'tcx>`**
      (already known to be `Some(_)` after step b) to
      `RegseqTerminator` via stage-9 §23.7 bridge
      translation rules (including stage-9 D-9-2
      Drop-as-Call collapse; D-9-9 rejection table
      enforcement).
   f. **Construct `BasicBlock { statements, terminator }`**
      (§24.1) and `IndexVec::push` it. The pushed
      `BlockId` matches rustc's source `BasicBlock`
      index because both IndexVecs are post-RPO.
3. **Post-loop assertions** (debug-only; per §24.4.4
   no-defensive-fallback rule):
   a. `result.len() == body.basic_blocks.len()`.
   b. `result[BlockId::START_BLOCK]` corresponds to
      rustc's `START_BLOCK = BasicBlock::from_u32(0)`
      (per `prettify.rs:33` fixed-point).

The bridge produces no separate predecessors /
dominators / RPO cache (per §24.2.3); analyses live in
`regseq-analysis`.

**Compositional with stage-9 D-9-2 Drop-as-Call
collapse.** The §23.4 Drop-as-Call collapse procedure
(steps 1-7) inserts a synthetic `&raw mut place`
Statement into the construction-time `Vec<Statement>`
buffer (per stage-9 §23.4 step 4); the working buffer
is then finalized to `Box<[Statement]>` in step 2d
above. The collapse procedure operates on the working
buffer, not on the finalized `Box<[Statement]>`.

### 24.8 Worked example

A trivial 2-block well-formed body illustrating the
stage-10 surface (statement and terminator content
abbreviated; see stages 8 / 9 for full per-construct
detail):

```
Body for `fn check(x: u32) -> bool { x > 0 }` after
`ReorderBasicBlocks` and standard cleanup-pipeline
pruning:

basic_blocks: IndexVec<BlockId, BasicBlock> = [
    // BlockId(0) = START_BLOCK
    BasicBlock {
        statements: Box::from([
            Statement { kind: Assign(_2, BinaryOp(Gt, _1, Constant(c_0))), span: _ },
        ]),
        terminator: Terminator { kind: Goto(BlockId(1)), span: _ },
    },
    // BlockId(1)
    BasicBlock {
        statements: Box::from([
            Statement { kind: Assign(_0, Use(Move(_2))), span: _ },
        ]),
        terminator: Terminator { kind: Return, span: _ },
    },
]
```

A would-be "panic=abort overflow path" block that
might appear in pre-cleanup MIR is **absent** from
this body: standard rustc pipeline scheduling
(`SimplifyCfg::PreOptimizations` +
`remove_dead_blocks`) prunes blocks unreachable from
`BlockId(0)` before the bridge consumes the body.
Per B-Reachable (§24.6.2), every block in the
finalized `basic_blocks` is reachable from
`BlockId::START_BLOCK`.

Note the storage order is RPO (BlockId(0) -> BlockId(1)
along the success path); B-Entry holds (BlockId(0) is
START_BLOCK); B-Terminator-Some holds structurally
(every basic block has a `Terminator` not
`Option<Terminator>`); B-Successors-Valid holds
(every successor BlockId is `< 2`); B-Reachable
holds (both blocks reachable from BlockId(0));
B-NonEmpty holds (`len() == 2`).

The pretty-printer rendering (per `pretty-printer.md`
§7.10) would emit:

```
bb0: {
    _2 = Gt(_1, const 0u32);
    goto -> bb1;
}

bb1: {
    _0 = move _2;
    return;
}
```

### 24.9 Rendering

See `pretty-printer.md` §7.10.

---


## 25. Body-level vocabulary

The function-level container that binds the stage-10
basic-block table and per-local declarations into a unit
of MIR. The function signature is **not** carried on Body;
per stage-12 D-12-1.4 (Option B; supersedes stage-11
D-11-3), the signature lives **Instance-side** on
`InstanceKind::Function { fn_sig, ... }` per §9.4.
Argument types are reachable through
`local_decls[1..=arg_count]` and return type via
`local_decls[RETURN_PLACE]` per the §25.1 layout
convention. Stage 11
sits seventh in the MIR pipeline (places -> operands ->
rvalues -> statements -> terminators -> basic blocks ->
**bodies**) and is the convergence point of the body-level
walks deferred from stages 5/8/9/10 plus the body-level
transformation passes deferred from stages 3/9.

Stage 11 is the **body-level integration stage**: it pins
how all earlier stages' content composes inside one
function-level unit, the bridge-side construction +
post-construction transformation passes that produce the
canonical body, and the joint visitor that validates it.
Body-level rendering lives in `pretty-printer.md` §3 +
§7 (see §25.11).

### 25.1 `Body` struct

```rust
pub struct Body {
    pub basic_blocks: IndexVec<BlockId, BasicBlock>,
    pub local_decls: IndexVec<LocalId, LocalDecl>,
    pub arg_count: usize,
    pub span: Span,
}
```

Cardinality: 4 fields. `basic_blocks` is inherited from
§24 (stage-10 D-10-2: bare `IndexVec<BlockId, BasicBlock>`
storage in reverse-postorder with `START_BLOCK = BlockId(0)`).
`local_decls`, `arg_count`, and `span` are stage-11 contributions.

**Layout convention** for `local_decls`:
```
local_decls[0]                  -> RETURN_PLACE (the return slot)
local_decls[1..=arg_count]      -> argument locals (positional)
local_decls[arg_count+1..]      -> internal temporaries (compiler-introduced
                                   + bridge-introduced + rewrite-pass-introduced)
```

Mirrors rustc + rustc_public + Charon. Helper methods
(implementation-side, not part of the spec interface):
- `Body::return_local() -> LocalId` returns `LocalId::RETURN_PLACE`.
- `Body::arg_locals() -> impl Iterator<Item = LocalId>` yields `1..=arg_count`.
- `Body::inner_locals() -> impl Iterator<Item = LocalId>` yields `arg_count+1..local_decls.len()`.

`span: Span` is a whole-body diagnostic location used by
body-level diagnostics (e.g., "body of fn foo at file:line
has unreachable basic_blocks"). Per-construct diagnostics
(per-LocalDecl, per-Statement, per-Terminator) carry their
own spans.

**No `Locals` substructure.** rustc + rustc_public use
flat `local_decls` + `arg_count`; Charon collapses into a
`Locals { arg_count, locals }` substructure. RegseqModel
takes the flat form: simpler shape; substructure-encapsulation
benefits accrue from methods on Body directly without an
additional struct layer; consistent with rustc + rustc_public
discipline.

**No body-level `fn_sig` field.** Per stage-12 D-12-1.4
(Option B; supersedes stage-11 D-11-3), function signatures
live **Instance-side** on `InstanceKind::Function { fn_sig,
... }`. See §25.4 + `discussions.md` D-12-1.4.

**No body-level `item: ItemId` back-pointer.** Per stage-12
D-12-1.4 (Option B; supersedes stage-11 D-11-5), bodies
live structurally on `InstanceKind::Function { body:
Option<Body>, ... }`; walkers reach Body via the parent
Instance. §25.5 records the Instance-side linkage shape.

**No `var_debug_info` body-level vector.** Consumed at bridge
construction time to populate per-`LocalDecl.debug_name`
(§25.2). Symmetric with `rustc_codegen_ssa`'s own
`per_local_var_debug_info` partition for codegen consumption.

**Rejection table (rustc Body fields not carried into RegseqModel):**

| rustc field                                         | Drop rationale                                                                                                                                                                  |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `phase: MirPhase`                                   | post-mono consumption; phase is fixed as `optimized_mir`; not informational.                                                                                                    |
| `pass_count: usize`                                 | rustc-pipeline bookkeeping; opaque post-bridge.                                                                                                                                 |
| `source: MirSource`                                 | Body-vs-Instance linkage held Instance-side per §25.5 (Option B); no body-side back-pointer.                                                                                    |
| `source_scopes: IndexVec<SourceScope, ...>`         | DWARF-emission only; aligned with stage-1 debuginfo rejection + stage-8 D-8-3 (no DWARF emission target).                                                                       |
| `coroutine: Option<Box<CoroutineInfo>>`             | coroutines DSL-rejected (§15.1; stage-7 D-7-1.7).                                                                                                                                |
| `user_type_annotations: ...`                        | borrowck artifact; pre-`optimized_mir` consumption; not visible at our stage.                                                                                                   |
| `spread_arg: Option<Local>`                         | rust-call ABI marker; D15 locks abi to `{Rust, C}`; closures lower with explicit arg lists per §13.1; no spread.                                                                |
| `var_debug_info: Vec<VarDebugInfo>`                 | consumed at construction time; lifted into per-`LocalDecl.debug_name` per §25.2 + stage-8 D-8-3.                                                                                  |
| `required_consts: Option<Vec<ConstOperand>>`        | collector-discharged; rustc itself notes "the MirUsedCollector already did that" at codegen consumption.                                                                       |
| `mentioned_items: Option<Vec<Spanned<...>>>`        | collector-only metadata.                                                                                                                                                         |
| `is_polymorphic: bool`                              | post-mono is by definition fully concrete.                                                                                                                                       |
| `injection_phase: Option<MirPhase>`                 | `rustc_custom_mir` only; not driver-relevant.                                                                                                                                    |
| `tainted_by_errors: Option<ErrorGuaranteed>`        | driver-level error gating; we reject upstream rather than carry tainted state.                                                                                                  |
| `coverage_info_hi`, `function_coverage_info`        | `-Cinstrument-coverage` only; not driver-relevant.                                                                                                                              |

The 17->4 collapse is mechanical from RegseqModel's binding
constraints (post-mono, no DWARF, no borrowck, no coroutines,
no instrumentation, no collector residue, C-shape codegen).

### 25.2 `LocalDecl` struct

```rust
pub struct LocalDecl {
    pub ty: TyId,
    pub mutability: Mutability,
    pub span: Span,
    pub debug_name: Option<StringId>,
}
```

Cardinality: 4 fields.

| Field         | Type                  | Carries                                                                                                                       |
| ------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `ty`          | `TyId`                | the local's type (interned via §12).                                                                                           |
| `mutability`  | `Mutability`          | `Not` / `Mut` per stage-3 D11; preserves rustc's mutability marker for IR-dump readability + optional codegen `const` qualifier. |
| `span`        | `Span`                | per-LocalDecl declaration location (`Span::dummy()` for synthetic locals introduced by bridge collapse / post-construction passes). |
| `debug_name`  | `Option<StringId>`    | source-level variable name (`Some("temp")`) or `None` for compiler-synthesized rustc temps + bridge-introduced synthetic locals. Lifted from rustc's body-level `var_debug_info` at bridge construction time. |

**Named-local lift.** Per stage-8 D-8-3, the bridge translates
each `VarDebugInfo` entry whose `value = VarDebugInfoContents::Place(p)`
with empty projection into a per-`LocalDecl.debug_name` for `p.local`
(via stage-1's `StringInterner`). Composite / non-Place /
non-empty-projection entries are bridge-rejected (they encode
debugger expressions RegseqModel doesn't use).

Charon's `Local.name` is direct positive precedent. rustc_public
keeps `var_debug_info` body-level (no per-LocalDecl name); mrustc
explicitly rejected the lift (commented-out `local_names`).
RegseqModel's lift is a deliberate divergence justified by C-shape
ergonomics: per-local lookup at render time + codegen
identifier-emission time without secondary scan.

**Synthetic locals.** Bridge-introduced (Drop-as-Call
`_t_drop_arg`/`_t_drop_ret` per §25.7.1) and rewrite-pass-introduced
(vtable-rewrite `_t_recv`/`_t_meta`/`_t_fn`/`_t_data`/`_t_raw` per
§25.7.2) all carry `debug_name = None`. Their identity in the
IR is purely positional (LocalId).

**Rejection table (rustc LocalDecl fields not carried):**

| rustc field                                            | Drop rationale                                                                                                                                                                                  |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `local_info: ClearCrossCrate<Box<LocalInfo>>`          | 9-variant `LocalInfo` enum is almost entirely diagnostic / borrowck (`User(BindingForm)`, `StaticRef`, `ConstRef`, `AggregateTemp`, `BlockTailTemp`, `IfThenRescopeTemp`, `DerefTemp`, `FakeBorrow`, `Boring`); the consumer-relevant `is-user-variable` distinction is discharged at construction (`debug_name=Some` <-> user variable; `debug_name=None` <-> compiler/synthetic). |
| `user_ty: Option<Box<UserTypeProjections>>`            | borrowck artifact; pre-`optimized_mir` consumption.                                                                                                                                              |
| `source_info.scope: SourceScope`                       | DWARF-emission only; aligned with stage-8 D-8-3 debuginfo rejection (`StmtDebugInfos` removed).                                                                                                 |

Note: rustc's `source_info: SourceInfo { span, scope }`
flattens to RegseqModel's `span: Span` field (the `scope`
half is dropped per the rejection table).

### 25.3 `LocalId` typed-index

```rust
newtype_index! {
    pub struct LocalId {
        const RETURN_PLACE = 0;
    }
}
```

Located in `crates/regseq-model/src/idx.rs` alongside the other
domain ID newtypes per stage-1 ID conventions. Symmetric with
stage-10 `BlockId::START_BLOCK = 0` (D-10-3.2's const-body
macro-arm extension).

**Sigil convention.** Textual rendering: `LocalId(N) -> _N`
(e.g., `_0` for `RETURN_PLACE`, `_1` for first argument, ...,
`_<arg_count+1..>` for temporaries). Rendering site **deferred
to stage 13 per D-11-K**.

**Layout convention.** Reach back to §25.1 (`local_decls[0]` is
RETURN_PLACE; `local_decls[1..=arg_count]` are args; rest are
temps).

**Size + niche assertions** (appended to
`crates/regseq-model/src/lib.rs` alongside stage-10's `BlockId`
assertions; spec-side pin only at stage 11; impl lands at
end-of-phase-1 sweep):

```rust
const _: () = assert!(size_of::<LocalId>() == 4);
const _: () = assert!(size_of::<Option<LocalId>>() == 4);
```

**Why typed (not bare alias).** rustc + Charon use proper typed
newtype indexes; rustc_public's `pub type Local = usize` and
mrustc's `unsigned int` typedef are weaker discipline (no niche;
no sigil; no type-safety against `BlockId` confusion). RegseqModel
inherits the rustc + Charon discipline per stage-1 ID conventions.

### 25.4 `FnSig` location

**Body has NO `fn_sig` field.** Per stage-12 D-12-1.4
(Option B), `FnSig` lives **Instance-side** on
`InstanceKind::Function { fn_sig: FnSigId, body:
Option<Body>, ... }` per §9.4. Per-monomorphization
function instantiations each carry their substituted
(post-mono) signature directly on the InstanceKind variant.

**Stage-11 D-11-3 supersession.** Stage 11 originally
placed FnSig as a structural field of `Item.kind =
Function`, citing 4-peer convergence (rustc-internal,
rustc_public, Charon, mrustc). Stage 12 D-12-1.4 reopens
this decision: the cited 4-peer evidence actually supports
**per-Instance** body / signature placement (rustc_public's
`Instance::body() -> Option<Body>` is the cited pattern;
Charon's `FunDecl` is post-mono under `--monomorphize`).
The Item-side framing was incompatible with the rest of
the spec's pre-mono Item model (§16.1 F4 sort + §13.4
source DefPathHash + §9.3 RegseqInstanceKey all imply
Items are 1:1 with rustc DefIds / pre-mono; stage-3 §15.1
rejects `Param/Bound/Placeholder/Infer/Alias` TyKind
variants -- so a stored FnSig must be already-substituted
post-mono). Stage 12 restores symmetry by moving FnSig +
Body to InstanceKind. Stage-11 D-11-3 + D-11-5 flip to
`[REOPENED]` in `decisions.md` at stage-12 merger contract
close per D-12-1.7. See `discussions.md` D-12-1.4 for full
rationale.

**Signature data reachable through `Body.local_decls`.** Argument
types: `Body.local_decls[1..=arg_count].ty` (positional convention).
Return type: `Body.local_decls[RETURN_PLACE].ty` (= `local_decls[0].ty`).
Mirrors rustc's `Body::return_ty()` reading
`local_decls[RETURN_PLACE].ty` directly + rustc_public's positional
helpers.

**Full FnSig** (with `abi` + `c_variadic` + `safety` per §14's
canonical schema) is reachable via the parent Instance:
`inst.kind = Function { fn_sig: FnSigId, ... }` resolved
through `model.sigs[fn_sig]` per §14.4 FnSigInterner identity
rule. Foreign declarations (`extern "C" { fn foo(); }`) still
have signatures via the same `InstanceKind::Function.fn_sig`
field; rustc's `tcx.fn_sig(did)` is callable for foreign fns.

§14's canonical FnSig schema (5 fields: `inputs`, `output`,
`abi`, `c_variadic`, `safety`) is independent of placement; the
schema definition stands. Stage-12 D-12-1.4's contribution is
the placement reframe (Instance-side per Option B; not
Item-side as stage-11 framed) and the per-Instance
substitution semantics for generic functions.

Stage-1's "`sigs` side-table" framing (`HashMap<InstanceId, FnSig>`)
remains rejected: side-tables fail Q2 (eager translation,
fully-owned IR; no side-channel lookups). The Instance-side
structural field on `InstanceKind::Function` IS the eager
owned representation.

### 25.5 Body-vs-Item linkage

**No `BodyId` allocation.** RegseqModel does not introduce a
separate ID type for bodies. Per stage-12 D-12-1.4 (Option B),
bodies live as a structural field on `InstanceKind::Function
{ ..., body: Option<Body>, ... }` per §9.4 -- one body per
Instance, not per Item.

**Stage-11 D-11-5 supersession.** Stage 11 originally placed
bodies as a structural field on `Item.body: Option<Body>`
under the Item-side framing. Stage 12 D-12-1.4 reopens: bodies
are per-monomorphization (each Instance has its own
substituted body), so they belong on InstanceKind, not Item.
The stage-11 D-11-5 framing's "one body per Item" assumption
was incompatible with generic functions (which produce
multiple Instances per Item, each needing its own substituted
body). Stage-11 D-11-5 flips to `[REOPENED]` in
`decisions.md` at stage-12 merger contract close.

**Walk direction.** Walkers reach `Body` via Instance. The
joint validator (§25.6) iterates over `model.instances`; for
each Instance whose kind is `Function { body: Some(b), ... }`,
runs the body visitor. No `BodyId -> Instance` lookup table
needed.

**`Option<Body>` accommodates absence (per D-12-2.1):**
- Foreign declarations (`extern "C" fn` with `tcx.is_foreign_item(did)`
  returning true) -> `body = None`. The `fn_sig` field is
  populated regardless (rustc's `tcx.fn_sig(did)` is callable
  for foreign fns).
- Non-Function InstanceKinds (Static, Const, Struct, Enum,
  Union) -> the variant doesn't have a `body` field at all
  (variant-specific shape per §9.4).

**Multi-peer evidence under Option B.** Two peers
**directly establish** the per-Instance body pattern that
RegseqModel adopts under Option B:
- **rustc-internal** fetches bodies via
  `tcx.instance_mir(InstanceKind)` per `rustc_middle/src/
  ty/mod.rs:1671`. Each Instance dispatches to its own
  monomorphized body (post-mono substitution applied).
  This is per-Instance, not per-DefId: a generic
  `fn foo<T>` with two monomorphizations
  (`Instance::Item(foo, [u32])` and
  `Instance::Item(foo, [u64])`) fetches **two distinct
  bodies** through `instance_mir`. Per
  `prior-art/rustc.md:11-24` (rustc_hir's source-side
  `ItemKind::Fn { body: BodyId, ... }` is the HIR-layer
  binding from DefId to a single source-level BodyId;
  the post-mono dispatch happens at the Instance layer
  via `instance_mir`).
- **rustc_public** provides per-Instance body via
  `Instance::body() -> Option<Body>` at primary source
  `rustc_public/src/mir/mono.rs:53-55`, structurally
  separate from `CrateItem`'s methods. This is the cited
  4-peer-convergence pattern stage-11 D-11-5 referenced:
  `Instance::body()` (per-Instance), NOT
  `CrateItem::body()` (per-DefId). Per
  `prior-art/rustc_public.md` "Per-Instance body access"
  paragraph (the dedicated note added during stage-12
  R2-correction pass to record this evidence directly).

**Two peers don't take a position:**
- **Charon** with `--monomorphize` produces one
  `FunDecl` per (DefId, GenericArgs) -- after Charon's
  monomorphization pass, there's no separation between
  "Item" and "Instance"; FunDecls ARE per-mono. Charon's
  `FunDecl.body: Result<Body, Opaque>` at
  `prior-art/charon.md:23-25` is therefore per-FunDecl
  (which equals per-Instance under monomorphize mode).
  This supports per-mono body placement but does not
  directly distinguish Item-side vs Instance-side
  because Charon doesn't carry the same Item/Instance
  split RegseqModel has.
- **mrustc** carries Function structures with linkage
  metadata at `prior-art/mrustc.md:20-30`; primary
  source for body-storage placement (per-Function vs
  per-mono) was not directly verifiable from the
  prior-art file. mrustc is primarily Rust-to-C++; its
  monomorphization-vs-storage discipline isn't a direct
  RegseqModel analog.

**Conclusion.** rustc-internal's `instance_mir` and
rustc_public's `Instance::body()` are the load-bearing
peer evidence; Charon's `--monomorphize` mode is
supportive; mrustc is informational only. The Option B
reframe rests on the rustc-internal + rustc_public
pattern, which directly establishes per-Instance body
placement.

Composes with Q3 (single-level IR; bodies are content of
Instances) and Q2 (eager translation; `Option<Body>` is owned
content, not a query).

### 25.6 Joint validation walk

The body-level validation walk is implemented as a visitor
pattern with override hooks. Validation is a model concern (the
IR knows how to validate itself); CFG analyses (predecessors,
dominators, RPO iteration) live in `regseq-analysis` per the
stage-10 model-vs-analysis line (§24.2.3).

#### 25.6.1 Visitor trait

Located at `crates/regseq-model/src/visit.rs`. 10 `visit_X` /
`super_X` hook pairs:

```rust
pub trait Visitor {
    fn visit_body(&mut self, body: &Body) { self.super_body(body) }
    fn super_body(&mut self, body: &Body) { /* default recursion */ }

    fn visit_basic_block(&mut self, bb: &BasicBlock) { self.super_basic_block(bb) }
    fn super_basic_block(&mut self, bb: &BasicBlock) { /* default recursion */ }

    fn visit_local_decl(&mut self, decl: &LocalDecl) { self.super_local_decl(decl) }
    fn super_local_decl(&mut self, decl: &LocalDecl) { /* default recursion */ }

    fn visit_statement(&mut self, stmt: &Statement) { self.super_statement(stmt) }
    fn super_statement(&mut self, stmt: &Statement) { /* default recursion */ }

    fn visit_terminator(&mut self, term: &Terminator) { self.super_terminator(term) }
    fn super_terminator(&mut self, term: &Terminator) { /* default recursion */ }

    fn visit_place(&mut self, place: &Place) { self.super_place(place) }
    fn super_place(&mut self, place: &Place) { /* default recursion */ }

    fn visit_rvalue(&mut self, rvalue: &Rvalue) { self.super_rvalue(rvalue) }
    fn super_rvalue(&mut self, rvalue: &Rvalue) { /* default recursion */ }

    fn visit_operand(&mut self, op: &Operand) { self.super_operand(op) }
    fn super_operand(&mut self, op: &Operand) { /* default recursion */ }

    fn visit_ty(&mut self, ty: TyId) { self.super_ty(ty) }
    fn super_ty(&mut self, ty: TyId) { /* default recursion */ }

    fn visit_call_callee(&mut self, callee: &CallCallee) { self.super_call_callee(callee) }
    fn super_call_callee(&mut self, callee: &CallCallee) { /* default recursion */ }
}
```

**Discipline.** Mirrors rustc_public's `make_mir_visitor!` macro
template:
- Exhaustive destructuring in `super_X` defaults: no `..`, no
  catch-all `_` arms. Adding a variant becomes a compile-time
  TODO; new variants are forced into explicit hook coverage.
- `super_X` recurses into child constructs, calling `visit_Y`
  for each child.
- Implementors override only the hooks they need; defaults
  handle traversal.

**Mutability discipline.** Read-only `Visitor` trait at stage 11
(`&mut self` for the visitor accumulator + `&Body` for the data).
A `MutVisitor` variant for transformation passes (`&mut self` +
`&mut Body`) is a phase-1 implementation extension, not specified
in detail at stage 11 -- the body-level transformation passes
per §25.7 do direct mutation rather than a generic mutation
visitor (their rewrite logic is too specific to benefit from a
generic trait; mrustc's `MIR_Cleanup_Virtualize` does direct
mutation).

#### 25.6.2 Body-level walk procedure (§24.4.3 absorption)

The `visit_body` hook implements the §24.4.3 6-step DFS-RPO
procedure deferred from stage-10 D-10-5 Part C:

```text
visit_body(body):
    // Step 1 (B-NonEmpty pre-check).
    if body.basic_blocks.is_empty():
        emit_error(B_NonEmpty); return

    // Step 2 (B-Successors-Valid pre-check; guards subsequent indexing).
    for bb in body.basic_blocks:
        for succ in bb.terminator.successors():       // §23.1.4 helper
            if succ.as_u32() >= body.basic_blocks.len():
                emit_error(B_Successors_Valid); return

    // Steps 3-5 (DFS-RPO traversal + B-Reachable + B-RPO).
    let rpo = dfs_rpo_from(BlockId::START_BLOCK, body)  // §24.4.3 helper
    if rpo.len() != body.basic_blocks.len():
        emit_error(B_Reachable)
    for (i, bb_id) in rpo.iter().enumerate():
        if bb_id != BlockId::from_u32(i as u32):
            emit_error(B_RPO); break

    // Step 6 (B-Entry implicit from step-3 starting point + step-1 pre-check).

    // Per-construct walks.
    super_body(body)  // recurses into local_decls + each basic_block
```

Pre-checks (steps 1-2) GUARD the indexing in the DFS-RPO
traversal: an empty body or out-of-range successor would panic
otherwise.

#### 25.6.3 Construction invariants 1 + 9 fold-in

Stage-1's deferred construction invariants 1 (TyId closure for
body-side IDs) and 9 (`Direct(InstanceId)` closure for callees;
stage-9 D-9-10 B-7) **fold into the joint visitor** rather than
running as separate pre-passes:

- **Invariant 1.** `visit_local_decl` checks `LocalDecl.ty in
  model.types`. `visit_ty` (recursing into structural TyId
  references in `Place::Field(idx, TyId)`,
  `Rvalue::Aggregate(AggregateKind::Struct(TyId), ...)`, etc.)
  checks each TyId.
- **Invariant 9.** `visit_terminator`'s Call arm checks
  `CallCallee::Direct(InstanceId)` membership in `model.instances`.

Reasoning: closure invariants are simple membership tests. A
separate pre-pass would re-walk the body for one tiny check per
local/ty/Call. mrustc's `TU_MATCH` inline-fold is supporting
precedent (one walk applies multiple checks per match arm).
rustc's `non_ssa_locals` pre-walk is precedent for "pre-walk
classifies; main walk consumes" but rustc's case populates a
sidecar map; our case is check-and-error with no sidecar needed.

#### 25.6.4 Walk-implementation absorption summary

The 5 deferred walk-implementations all close at stage-11's
joint visitor:

| Source                                | Stage-11 hook                            | Invariants applied          |
| ------------------------------------- | ---------------------------------------- | --------------------------- |
| D-5-2 Place validation walk           | `visit_place`                            | §19.7's 5 rule categories   |
| §20.5 Operand-site invariants         | `visit_operand`                          | §20.5 invariants            |
| D-7-3 Rvalue walk                     | `visit_rvalue`                           | §21.5 invariants            |
| D-8-4 Part C Statement walk           | `visit_statement`                        | §22.5 B-1..B-5              |
| D-9-10 Part C Terminator walk         | `visit_terminator`                       | §23.6 B-1..B-9              |
| D-10-5 Part C Body walk (§24.4.3)     | `visit_body` (pre-checks per §25.6.2)    | B-NonEmpty + B-Successors-Valid + B-Reachable + B-RPO + B-Entry |

Plus construction invariants 1 + 9 fold-in per §25.6.3.

### 25.7 Body-level transformation passes

Three passes operate on Body content with **hybrid timing**,
all landing in Phase 5 of the §3.4 5-phase pipeline (after
Phase 4 ItemId assignment):
- Two run at **bridge body-construction time** (Phase 5
  body-materialisation sub-step; per-rustc-terminator
  translation envelope; forced by the locked vocabulary):
  Drop-as-Call collapse (§25.7.1) and DiagLogPeel
  (§25.7.3).
- One runs as a **post-body-construction pass** over the
  fully-materialised Body (Phase 5 intra-body-pass sub-step;
  sequenced after body materialisation, before joint
  validation): the vtable-indirect rewrite (§25.7.2).

The §3.4 Phase 5 "Per-phase body work" subsection is the
authoritative pass-scheduling reference; this section spells
out each pass's mechanics.

#### 25.7.1 Drop-as-Call collapse (Phase 5 body construction)

**Forced timing.** Stage-9 D-9-1.1 rejected the `Drop` variant
from `TerminatorKind`. The bridge MUST translate
`mir::TerminatorKind::Drop` at body-construction time (Phase 5
body-materialisation sub-step per §3.4) -- there is no Drop
variant in RegseqModel's vocabulary to defer. The drop-glue
Function Item that the resulting Call terminator targets was
synthesised in Phase 2 (per §3.4 Phase 2 trigger table) and
got its final ItemId in Phase 4; Phase 5 just inserts the
Call terminator referencing the already-final ItemId.

**Procedure** (per stage-9 §23.4 step 4):

For each `mir::TerminatorKind::Drop { place, target, .. }`:

1. Allocate fresh `_t_drop_arg: LocalDecl { ty: Ty::Ptr(place_ty,
   Mutability::Mut), mutability: Mutability::Mut, span: Span::dummy(),
   debug_name: None }` via `Body::push_synthetic_local(ptr_to_place_ty)`.
2. Allocate fresh `_t_drop_ret: LocalDecl { ty: Ty::Tuple([]), ... }`
   via `Body::push_synthetic_local(unit_ty)`.
3. Append `Statement::Assign(Place::from(_t_drop_arg),
   Rvalue::RawPtr(Mutability::Mut, place))` to the current
   basic block's statements list (immediately before the
   synthesized Call terminator -- after all earlier statements
   that may have mutated `place`).
4. Emit `TerminatorKind::Call { callee: CallCallee::Direct(
   drop_glue_instance_id), args: [Operand::Move(Place::from(_t_drop_arg))],
   destination: Place::from(_t_drop_ret), target: Some(target) }`.

The drop-glue Instance has `InstanceKind::Function { fn_sig,
body, flavor: FunctionInstanceFlavor::DropGlue, virtual_idx:
None }` (concrete drops) or `InstanceKind::Function { fn_sig,
body, flavor: FunctionInstanceFlavor::Virtual, virtual_idx:
Some(0) }` (dyn-Trait drops; `FunctionInstanceFlavor::Virtual`
is the unit-variant flavor tag, `virtual_idx` is the separate
`InstanceKind::Function` field; `0` is the
COMMON_VTABLE_ENTRIES_DROPINPLACE slot per rustc's
`ty/vtable.rs:45-49`). The `fn_sig` and `body` fields are
populated per §9.4 / Option B (D-12-1.4): `fn_sig` is the
substituted drop-glue signature (typically `fn(*mut SelfTy)
-> ()` for concrete drops, `fn(*mut ()) -> ()` for the
erased-receiver Virtual case); `body` is the synthesized
drop-in-place body (or fetched via rustc's
`tcx.instance_mir(InstanceKind::DropGlue(...))` for concrete
drops).

**Invariant timing.** Drop-as-Call collapse runs inside the
bridge's atomic per-rustc-terminator translation envelope. The
constructed Body has no `Drop` terminators; only the synthesized
`Call` terminators with drop-glue callees. Stage-9's §23.6
invariants (B-3 Call destination type matches callee return
type, etc.) hold post-collapse.

#### 25.7.2 Vtable-indirect rewrite (post-construction)

**Indicated timing.** A body-level pass running over the
fully-constructed Bodies of Function Instances. Per §3.4's
5-phase pipeline, Items (including stage-12 vtable type Items)
are F4-sorted and assigned final ItemIds at Phase 4; Function
Instances and their Bodies are constructed at Phase 5. The
vtable-indirect rewrite is therefore an **intra-Phase-5** pass
that runs over Bodies *after* they are materialized
(Phase 5 sub-step) but *before* the joint validation walk
(§25.6 sub-step). At rewrite time, the Item table is fully
populated (Phase 4 complete) and the rewriter resolves
virtual-call targets to vtable function-pointer slots whose
underlying Items already have final ItemIds.

A bridge-time (Phase 2) rewrite would require Item synthesis
to be coupled with body translation -- contradicting §3.4's
Phase 2/4/5 separation (Items synthesized + cached during
Phase 2 via Phase2Handles; ItemIds finalized at Phase 4;
Bodies materialized at Phase 5). Post-construction
intra-Phase-5 timing matches Q2 (eager translation produces
fully-owned IR; passes mutate fully-owned IR) cleanly.
Charon's `transform_dyn_trait_calls` (`transform_dyn_trait_calls.rs:31-122`,
registered in `INITIAL_CLEANUP_PASSES` at `mod.rs:104, 111`) and
mrustc's `MIR_Cleanup_Virtualize` (`cleanup.cpp:548-684`) both
run post-body-construction. rustc's codegen-time direct dispatch
(via `Instance::Virtual` + `tcx.vtable_entries`) is structurally
inadmissible for C-emission under Q2 (no `'tcx` post-bridge).

**Procedure** (per stage-9 expectations.md §23.3):

For each `BasicBlock.terminator` of kind `TerminatorKind::Call`
where `callee = CallCallee::Direct(instance_id)` and the resolved
Instance has `kind: InstanceKind::Function { flavor:
FunctionInstanceFlavor::Virtual, virtual_idx: Some(idx), .. }`:

1. **Receiver materialization.** Match `args[0]` and produce
   `_t_recv: LocalId` typed same as `args[0]`:
   - If `args[0] = Operand::Copy(p)` or `Operand::Move(p)`:
     insert `Statement::Assign(_t_recv, Rvalue::Use(args[0].clone()))`.
   - If `args[0] = Operand::Constant(_)`: synthesize via
     `Statement::Assign(_t_recv, Rvalue::Use(args[0]))` (rare;
     well-formed only via stage-12 vtable instance synthesis).
   `_t_recv` has type `Ty::Ref(dyn_ty, _)` or `Ty::Ptr(dyn_ty, _)`.

2. **Source normalization** (Ref receivers only). For
   `_t_recv: Ty::Ref(dyn_ty, _)`, synthesize a raw-pointer
   intermediate via `Statement::Assign(_t_raw, Rvalue::RawPtr(<mut>,
   Place { local: _t_recv, projection: [Deref] }))` -- the `&raw
   const *_t_recv` (or `&raw mut *_t_recv` for `&mut dyn Trait`)
   form. `_t_raw` has type `Ty::Ptr(dyn_ty, Mutability::Not)` (or
   `Mut`). For `_t_recv: Ty::Ptr(dyn_ty, _)` (raw-pointer receivers,
   e.g. dyn-Trait drops per §23.4 where args[0] is the synthetic
   `&raw mut place`), this step is skipped.

3. **Metadata extraction.** Synthesize `Statement::Assign(_t_meta,
   Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(Place::from(_t_recv))))`.
   `_t_meta` has type `Ty::Ptr(<vtable_struct_TyId>, Mutability::Not)`
   where `<vtable_struct_TyId>` is `dyn_ty.vtable_ty` (from
   stage-3 D5).

4. **Function pointer load.** Synthesize `Statement::Assign(_t_fn,
   Rvalue::Use(Operand::Copy(Place { local: _t_meta, projection:
   [Deref, Field(virtual_idx, fn_ptr_ty)] })))`. The `Field`
   projection at `virtual_idx` reads the vtable's
   function-pointer-typed slot. `_t_fn` has function-pointer type
   matching the loaded slot.

5. **Data pointer extraction.** Synthesize `Statement::Assign(_t_data,
   Rvalue::Cast(CastKind::PtrToPtr, <_t_raw_or_recv_operand>,
   Ty::Ptr(unit_ty, <mut>)))`. The source operand is
   `Operand::Move(Place::from(_t_raw))` for the reference-receiver
   case (after step 2 normalization) or `Operand::Copy(Place::from(_t_recv))`
   for the raw-pointer-receiver case. The destination type is
   `Ty::Ptr(Ty::Tuple([]), <mut>)`.

6. **Call rewrite.** Replace `Call.callee` from
   `CallCallee::Direct(virtual_instance_id)` to
   `CallCallee::Indirect(Operand::Move(Place::from(_t_fn)))`.
   Replace `Call.args[0]` with `Operand::Move(Place::from(_t_data))`
   (matches the erased-receiver fn_sig at the loaded vtable slot).
   Other args (`args[1..]`), destination, target unchanged.

The 4-5 fresh locals (4 for raw-pointer receivers; 5 for
reference receivers including source-normalization) all allocate
via `Body::push_synthetic_local`.

**Cross-stage friction with stage-5 D-5-1.** Charon's compact
in-place projection-chain rewrite at line 119 of
`transform_dyn_trait_calls.rs`:
```
call.func = FnOperand::Move(method_field_place)
```
with `method_field_place = (*receiver.ptr_metadata).method`
uses a `PtrMetadata` PlaceElem variant in Charon's projection
vocabulary. **Stage-5 D-5-1 locked a 6-variant `PlaceElem` set
that does NOT include `PtrMetadata`**: Deref / Field / Index /
ConstantIndex / Subslice / Downcast.

Without `PtrMetadata` as a PlaceElem, RegseqModel cannot inline
the metadata extraction into a Place projection chain. The
metadata MUST be extracted as a separate `Rvalue::UnaryOp(UnOp::PtrMetadata,
...)` Statement. The chain expands to 4-5 fresh locals (Step 3
+ Step 4 + Step 5 + Step 1 + optional Step 2) vs Charon's
0-fresh-locals form.

The verbosity is **forced by stage-5's locked vocabulary**, not
a stage-11 design choice. Reopening D-5-1 to add `PtrMetadata`
as a PlaceElem variant is out of stage-11 scope (would cascade
through stage-5's projection-typing rules + walk + invariants
+ the §19.4 type-narrowing table). `UnOp::PtrMetadata` is in
stage-7 D-7-1.6's locked `UnaryOp` enum -- that's how the
metadata extraction happens via Rvalue rather than via Place.

**Invariant timing.** Stage-9 §23.6.2 invariants B-3 (Call
destination type = callee return type), B-4 (Call args length =
callee arity), B-6 (per-arg type match) hold POST-rewrite. Pre-rewrite,
`args[0]` type does not match `callee_fn_sig(callee_id).inputs()[0]`
for Virtual-flavor Calls (args[0] is `&dyn Trait` or `*mut dyn Trait`;
`callee_fn_sig(...).inputs()[0]` is `*mut ()` per §13.2 +
§17.2.3's uniform-mutable-erased-receiver rule, resolved per
the §23.6.2 helper through `inst.kind.as_function().fn_sig`
under stage-12 D-12-1.4 / Option B). The rewrite resolves the
mismatch by extracting the data pointer.

**Pass scheduling.** Vtable-rewrite runs **before** the joint
validation walk (§25.6) -- §23.6.2 B-3/B-4/B-6 invariants assume
post-rewrite shape. Running validation pre-rewrite would emit
spurious B-3/B-4/B-6 errors at every Virtual-flavor Call site.

#### 25.7.3 DiagLogPeel (Phase 5 body construction)

**Forced timing.** Diagnostic-logging calls in source-level
Rust expand (through the `regseq::log::<sev>!` macro family
per §26.5) to MIR Call sites of canonical shape:

```text
Call {
  callee: Direct(<InstanceId of regseq::log::intrinsics::log_<sev>>),
  args: [Operand::Constant(<&'static str literal>),
         Operand::Move(<slice-aggregate of LogArg::from(<expr_i>) calls>)],
  destination: <unit place>,
  target: Some(<next bb>),
  unwind: <unchanged>,
}
```

The bridge MUST peel this shape at body-construction time and
synthesise the desugared flat-arg Call form -- the IR carries
the peeled shape, not the slice-aggregate. The peel discharges
the `I-DiagLog-CallArity` invariant captured at §23.6.2 B-4
and B-6 exceptions; it composes with §3.4 Phase 5's
per-rustc-terminator translation envelope (same pattern as
§25.7.1 Drop-as-Call collapse).

**Trigger.** For each `mir::TerminatorKind::Call { func, args,
destination, target, unwind, .. }` being translated, after
resolving `func` to a RegseqModel InstanceId and looking up the
callee's `domain_attrs.item_attrs` entry: if the entry contains
`AttrData::Intrinsic(IntrinsicKind::Log(level))`, run
DiagLogPeel; otherwise translate as a regular Call.

**Procedure.** For a Call recognised as a Log intrinsic:

1. **Canonical-shape assertion** (performed against the
   **rustc MIR** shape before Regseq const lowering, so that
   D6 literal identity is checkable at this point -- after
   lowering, an `&'static str` literal and an `&'static str`
   const-reference value both serialise to
   `Const::Aggregate([Reloc(<static>), Uint(Usize, len)])`
   per §9.7 and are no longer distinguishable). Assert the
   MIR Call has exactly two args. Assert `args[0]` is a
   Constant whose type is `&'static str` and whose value is a
   **string literal** (rustc carries this at the MIR level as
   a `Const::Slice` over interned string bytes per the const
   machinery; the literal-ness check rejects MIR Constants
   sourced from `const`-bindings, `static` references, or
   other non-literal `&'static str` values that satisfy the
   type but not the form). Failing the literal-ness check
   emits **D6** (§26.8) as an author-facing diagnostic with
   the original Rust call-site span. Assert `args[1]`
   resolves to an `Rvalue::Aggregate(AggregateKind::Array(_),
   inner)` whose element type is `LogArg<'_>` and whose
   element count is `N` (zero or more). Each inner element
   resolves to a `Call` to `<LogArg<'_> as
   From<T_i>>::from(<expr_i>)` where `T_i` is one of the
   admissible source types per §26.4. Shape mismatches other
   than D6 (e.g., wrong arity, non-aggregate second arg,
   wrong inner-element type) are **bridge-internal
   invariant violations**: panic with a `bug!`-style
   diagnostic citing the offending MIR shape. These are not
   author-facing -- the only path to reach a Log-intrinsic
   callee is through the macro per §26.5, and the macro
   pins the shape. The literal-ness check (D6) is the one
   author-facing diagnostic at this step because authors can
   structurally produce a non-literal `&'static str` even
   through the macro (e.g., a future macro revision that
   passes a `const`-bound expression; the current
   `$fmt:literal` matcher rejects this at the macro level
   as defense in depth per §26.5, but the bridge-side check
   defends against the structural escape).

2. **Peel each `LogArg::from(<expr_i>)`, normalising
   reference-companion impls to value loads.** The
   recognised inner-Call signature is `From::from(<expr_i>)
   -> LogArg<'_>`. The peel reads `<expr_i>` directly off
   the inner Call's args and recovers its RegseqModel `Ty`.
   For the **by-value** `From<T>` impls (where `T` is one of
   the value-typed scalars `u8..u64` / `i8..i64` / `bool` /
   `usize` / `isize` per §26.4) and the **by-value
   `From<&'a str>`** impl, the recovered Operand and `Ty`
   are used directly as the flat-Call argument; the variant
   tag is implicit in `T_i`'s shape and is not stored
   (per the transient-fingerprint rule -- §26.6).

   For the **reference-companion** impls `From<&T>` /
   `From<&mut T>` (where `T` is a value-typed scalar per
   §26.4), the bridge synthesises a load before the flat
   Call.

   **Source-Place extraction.** The recovered `<expr_i>` is
   one of:
   - `Operand::Copy(p_ref)` where `p_ref: Place` has type
     `Ty::Ref(T_value_id, _)`. Take `p_ref` as the source
     reference Place.
   - `Operand::Move(p_ref)` where `p_ref: Place` has type
     `Ty::Ref(T_value_id, _)`. Take `p_ref` as the source
     reference Place.
   - `Operand::Constant(_)` -- bridge-internal invariant
     violation (references are not Constants in normal
     post-cleanup MIR; the macro expansion path that
     produces a reference-companion `From::from(&x)` call
     always materialises the reference as a local via
     `Statement::Assign(_t_ref, Rvalue::Ref(...))` before
     the inner Call). Panic with a `bug!`-style diagnostic.

   **Load synthesis.** Construct the deref source Place by
   extending the projection chain by one `PlaceElem::Deref`:

   ```text
   deref_source = Place {
     local:      p_ref.local,
     projection: [p_ref.projection..., PlaceElem::Deref],
   }
   ```

   `deref_source` has type `T_value` (the pointee of
   `Ty::Ref(T_value_id, _)`) and is well-formed under
   stage-5 §19.7 Place validation (the trailing `Deref`
   composes with stage-5 D-5-1's projection-typing rule
   for reference-typed bases; see stage-5
   `spec-snapshot.md` §19.4 type-narrowing table).

   Allocate a fresh value-typed local `_tmp_i` via
   `Body::push_synthetic_local(T_value_id)` per §25.8 (the
   helper defaults to `Mutability::Mut`, `Span::dummy()`,
   `debug_name: None`). Append the load Statement to the
   current basic block's statements list (immediately before
   the synthesised flat Call):

   ```text
   Statement::Assign(
     Place::from(_tmp_i),
     Rvalue::Use(Operand::Copy(deref_source)),
   )
   ```

   `Operand::Copy` is correct for all admissible
   reference-companion source types: the pointee `T` is one
   of `u8..u64` / `i8..i64` / `bool`, all of which are
   `Copy` Rust-side. (`Operand::Move` would also be sound
   but produces unnecessary move-out semantics on a
   `Copy`-typed scalar; `Operand::Copy` matches the bridge's
   convention for scalar loads.)

   Finally, substitute `Operand::Move(Place::from(_tmp_i))`
   of type `T_value` (NOT `&T` / `&mut T`) as the flat
   Call's `args[i+1]`.

   The `&str` companions (`From<&'a str>`) are NOT
   reference-companions in the normalisation sense -- they
   are the by-value impl for the `Ty::Ref(Ty::Str, _)`
   admissible type; no load is synthesised because the
   value-typed admissible variant for `&str` IS the
   reference shape (the carrier's `Str(&'a str)` variant
   stores the reference directly).

   The IR Call's `args[1..args.len()]` therefore carry
   uniformly value-typed Operands: integer-width or `bool`
   values for the numeric / boolean carrier variants, and
   `Ty::Ref(Ty::Str, _)` Operands for the `Str` carrier.
   No reference-companion-typed Operand appears in the
   peeled Call. This single-peel normalisation discipline
   ensures the §26.9 backend specifier table covers every
   admissible operand shape without per-target
   reference-deref logic.

3. **Drop the slice aggregate.** The `Rvalue::Aggregate` that
   built the `[LogArg<'_>; N]` array (or the `&[LogArg<'_>]`
   slice reference) does NOT survive into RegseqModel. Any
   `LocalId`s that the original MIR introduced solely to
   materialise the slice (intermediate temporaries holding
   `LogArg<'_>` values, the array itself, the slice
   reference) are not translated; their definitions are
   skipped.

4. **Synthesise the flat-arg Call.** Emit a single
   `TerminatorKind::Call`:

   ```text
   Call {
     callee: Direct(<InstanceId of log_<sev>>),
     args: [Operand::Constant(<&'static str literal>),
            <expr_1>, ..., <expr_N>],
     destination: <unit place from original>,
     target: <unchanged>,
     unwind: <unchanged>,
   }
   ```

   `args.len() == 1 + N`. The `LogArg<'_>` type and the
   `&[LogArg<'_>]` slice type do not appear anywhere in the
   resulting Body.

5. **Produce the call-site fingerprint** for §26.8
   validation: `{ severity: level, fmt_template: <&'static
   str content of args[0]>, arg_tys: [T_1, ..., T_N],
   span: <original Call's span> }`. The fingerprint is
   **transient** -- consumed by validation immediately and
   not stored on `RegseqModel`. Both `severity` (re-derivable
   via `domain_attrs.item_attrs[callee_item_id]`) and
   `arg_tys` (re-derivable via the flat-arg Call's
   `operand_ty(args[i])`) are reachable from the IR without
   a side table. See §26.6 for the canonical statement of
   transient-fingerprint discipline.

**Per-position validation.** §26.8 (D-14-2.7) defines the
bridge-side diagnostics D1-D4 + D6 + D7 + D8 that run on the
fingerprint produced at step 5 (D5 is deferred to phase-2
stage 17; see §26.10). Validation runs immediately after
DiagLogPeel synthesis, still within Phase 5 body
materialisation. Author-facing diagnostics use the preserved
original Rust call-site span. D1-D4 + D6 + D7 are
target-independent (template/arg arity, modifier set,
admissible argument types, literal-ness, trailing newline).
**D8 is target-aware**: it evaluates the post-DiagLogPeel
`&str` operand count in args[1..] against the selected
binding target's declared `max_str_args_per_call`. The bound
is declared per-backend (see `generic-c-backend.md` §8.4 for
the generic-C target; phase-2 targets per their respective
`<target>-backend.md` artifacts). The bridge validation pass
receives the binding-target identity as part of its
operational context (via the build's target-selection flag
or equivalent) and evaluates D8 against the target's
declared bound.

**Invariant timing.** §23.6.2 B-4 and B-6 exceptions (the
`I-DiagLog-CallArity` carve-out) apply to the peeled Call.
The joint validation walk (§25.6) at Phase 5 final sub-step
treats Log-intrinsic Calls per the exception clauses.

**Worked example (by-value args).**

```rust
// Author source (speed: u32, retries: u32, both passed by value):
regseq::log::warn!("link up at {} Mbps after {} retries", speed, retries);

// Post-macro-expansion MIR shape (pre-DiagLogPeel):
Call {
  callee: Direct(<log_warn>),
  args: [
    Constant("link up at {} Mbps after {} retries"),
    Move(_t_slice_ref),   // &[LogArg<'_>; 2] produced by:
                          //   _t_arr = [LogArg::from(speed), LogArg::from(retries)]
                          //   _t_slice_ref = &_t_arr
  ],
  destination: _t_unit, target: Some(bb_next),
}

// Post-DiagLogPeel RegseqModel Call:
Call {
  callee: Direct(<log_warn>),
  args: [
    Operand::Constant("link up at {} Mbps after {} retries"),
    <expr for `speed`>,    // Ty::Uint(U32); From<u32> impl, no load synthesised
    <expr for `retries`>,  // Ty::Uint(U32); From<u32> impl, no load synthesised
  ],
  destination: _t_unit, target: Some(bb_next),
}
```

The intermediate locals (`_t_arr`, `_t_slice_ref`, and any
per-arg `LogArg::from` temporaries) are not present in the
resulting Body.

**Worked example (reference-companion args; demonstrates
step-2 load synthesis).**

```rust
// Author source (head: u32, tail: u32; author passes references):
let head: u32 = self.rx_ring.head();
let tail: u32 = self.rx_ring.tail();
regseq::log::debug!("rx ring head={} tail={}", &head, &tail);

// Post-macro-expansion MIR shape (pre-DiagLogPeel) -- the macro
// expansion materialises each reference as a local before passing
// it into LogArg::from:
//   _t_ref_head = &head                                // Ty::Ref(u32, _)
//   _t_ref_tail = &tail                                // Ty::Ref(u32, _)
//   _t_arr = [LogArg::from(Move(_t_ref_head)),
//             LogArg::from(Move(_t_ref_tail))]
//   _t_slice_ref = &_t_arr
//   <Call to log_debug with the slice>

// Step-2 recovered Operand for each inner LogArg::from call:
//   <expr_1> = Operand::Move(Place { local: _t_ref_head,
//                                     projection: [] })   // Ty::Ref(u32, _)
//   <expr_2> = Operand::Move(Place { local: _t_ref_tail,
//                                     projection: [] })   // Ty::Ref(u32, _)
//
// Step-2 source-Place extraction (per the Operand::Move arm of the
// source-Place extraction rule above) gives:
//   p_ref_1 = Place { local: _t_ref_head, projection: [] }
//   p_ref_2 = Place { local: _t_ref_tail, projection: [] }
//
// Step-2 deref-source construction (extending the projection chain
// by one PlaceElem::Deref):
//   deref_source_1 = Place { local: _t_ref_head,
//                            projection: [PlaceElem::Deref] }  // Ty::Uint(U32)
//   deref_source_2 = Place { local: _t_ref_tail,
//                            projection: [PlaceElem::Deref] }  // Ty::Uint(U32)
//
// Step-2 load synthesis (one fresh value-typed local per
// reference-companion arg, allocated via
// Body::push_synthetic_local(u32_ty)):

Statement::Assign(Place::from(_t_head_val),
  Rvalue::Use(Operand::Copy(deref_source_1)))   // _t_head_val: Ty::Uint(U32)
Statement::Assign(Place::from(_t_tail_val),
  Rvalue::Use(Operand::Copy(deref_source_2)))   // _t_tail_val: Ty::Uint(U32)

// Flat-arg Call carries the value-typed loads, NOT the references:
Call {
  callee: Direct(<log_debug>),
  args: [
    Operand::Constant("rx ring head={} tail={}"),
    Operand::Move(Place::from(_t_head_val)),   // Ty::Uint(U32)
    Operand::Move(Place::from(_t_tail_val)),   // Ty::Uint(U32)
  ],
  destination: _t_unit, target: Some(bb_next),
}
```

`_t_head_val` and `_t_tail_val` are fresh synthetic
LocalDecls allocated via `Body::push_synthetic_local(u32_ty)`.
The IR carries value-typed `Ty::Uint(U32)` Operands at
args[1..3]; the `&u32` reference Operands the macro
expansion originally produced do not appear. The §26.9
backend specifier table dispatches on `Ty::Uint(U32)` ->
`%u` uniformly for both this case and the by-value example
above.

#### 25.7.4 Pass scheduling

The body-validation pipeline within §3.4 Phase 5
(after Phase 4 ItemId assignment is complete):

```text
[1] Phase 5 body materialisation / bridge body construction
    (after Phase 4 ItemId assignment)
    - rustc Body -> RegseqModel Body (atomic per-rustc-terminator translation)
    - Drop-as-Call collapse INSIDE this step (§25.7.1)
    - DiagLogPeel INSIDE this step (§25.7.3) for Calls whose
      callee carries IntrinsicKind::Log(_) in domain_attrs
    - Calls to dyn-Trait methods land as Direct(InstanceId)
      where Instance has flavor=Virtual + virtual_idx=Some(idx)
   |
   v
[2] Vtable-indirect rewrite pass (§25.7.2)
    - Phase 5 intra-body pass over the materialised Body
    - Walks every BasicBlock.terminator
    - For each Direct(virtual)-callee Call: 6-step rewrite
      (5 synthesis steps + Call rewrite at step 6)
    - Produces Calls with CallCallee::Indirect(Operand)
   |
   v
[3] Joint validation walk (§25.6 visitor)
    - Phase 5 final sub-step
    - Pre-checks + DFS-RPO traversal (§25.6.2)
    - Per-construct invariants (§25.6.4) -- §23.6.2 B-4 and
      B-6 apply the I-DiagLog-CallArity exception for Calls
      whose callee carries IntrinsicKind::Log(_)
    - Construction invariants 1 + 9 fold-in (§25.6.3)
```

All three steps run in Phase 5 (per §3.4 "Per-phase body work
in Phase 5"). Step 2 must precede step 3. Step 1's two
body-construction-time passes (Drop-as-Call collapse and
DiagLogPeel) are both sequenced WITH step 1 (atomic
per-rustc-terminator). They are independent: a single
rustc Terminator is at most one of `Drop` (handled by
Drop-as-Call) or `Call` (potentially handled by DiagLogPeel
if the callee carries `IntrinsicKind::Log(_)`); no rustc
Terminator is both.

### 25.8 LocalDecls allocation API

```rust
impl Body {
    /// Append `decl` to local_decls; return the freshly-allocated LocalId.
    /// Allocation is positional: LocalId equals the previous local_decls.len().
    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId;

    /// Convenience helper for synthetic locals (bridge-introduced
    /// + rewrite-pass-introduced). Defaults to `Mutability::Mut`,
    /// `Span::dummy()`, `debug_name: None`. Returns the freshly-
    /// allocated LocalId.
    pub fn push_synthetic_local(&mut self, ty: TyId) -> LocalId;
}
```

**Single API; no Builder pattern.** Body is mutable directly
during both bridge construction and post-construction passes
(§25.7.2 vtable-rewrite). A `BodyBuilder` intermediate would not
simplify -- post-construction passes mutate Body anyway; either
Body is mutable always (Builder is redundant) or Body is
immutable post-construction (passes can't mutate, blocking
§25.7.2). The Charon pattern (`Locals::new_var` + a
`BodyTransformCtx` trait) and the mrustc pattern
(`MirBuilder::new_temporary` / `MirMutator::new_temporary`
split) both add a layer that doesn't pay off given RegseqModel's
locked LocalDecl shape (4 fields per §25.2; no rich helpers
needed).

**No buffering pattern.** mrustc's `MirMutator` pending-statements
buffer exists because their rewrite walks DURING basic-block
iteration (mid-iteration mutation safety). RegseqModel's §25.7.2
rewrite walks `basic_blocks_iter()` and rewrites each block as
a unit (extends `block.statements` then replaces `block.terminator`
atomically per block); no mid-iteration mutation. Drop-as-Call
at bridge time is similarly atomic per rustc terminator.

**No auto-`StorageLive`.** Charon's `Locals::new_var` auto-emits
a `StorageLive` statement upon allocation. RegseqModel D-8-1
dropped `StorageLive` from `StatementKind` (storage markers
were `RemoveStorageMarkers`-discharged); the auto-emit pattern
does not transfer.

**Consumers:**

| Site                                   | Allocation pattern                                                                                                  |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Bridge per-rustc-local population      | `body.push_local(LocalDecl { ty, mutability, span, debug_name })` with full LocalDecl from rustc's body data         |
| Bridge Drop-as-Call collapse (§25.7.1) | `body.push_synthetic_local(ptr_to_place_ty)` for `_t_drop_arg`; `body.push_synthetic_local(unit_ty)` for `_t_drop_ret` |
| Vtable-rewrite pass (§25.7.2)          | 4-5 calls to `body.push_synthetic_local(ty)` per virtual-call site (`_t_recv`, `_t_meta`, `_t_fn`, `_t_data`, optionally `_t_raw`) |
| Bridge DiagLogPeel (§25.7.3)           | One `body.push_synthetic_local(T_value_id)` per reference-companion log argument (`From<&T>` / `From<&mut T>` impls; one fresh value-typed local per argument that needs load synthesis). By-value `From<T>` and `From<&'a str>` impls allocate no synthetic locals. |

### 25.9 Per-stage invariants registry

#### 25.9.1 Part A: Rejection table (already covered)

Body-side rejection table is in §25.1 (~14 rustc Body fields
dropped). LocalDecl-side rejection table is in §25.2 (3 rustc
LocalDecl fields dropped). All rejections are bridge-side at
construction; the constructed Body never carries the dropped
fields.

#### 25.9.2 Part B: Positive invariants

| Tag         | Statement                                                                                          | Walk-enforcement site                          |
| ----------- | -------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| B-Body-1    | `local_decls.len() > arg_count` (RETURN_PLACE always present, args follow, temps after)           | `visit_body` pre-check                         |
| B-Body-2    | `arg_count` matches the function signature's input count from `InstanceKind::Function { fn_sig }` (per Option B; stage-12 D-12-1.4) | cross-Instance walk (stage-12 owns; phase-1 impl) |
| B-Body-3    | All `LocalDecl.ty` resolve in `model.types` (construction invariant 1, body-side)                  | `visit_local_decl` (§25.6.3)                   |
| B-Body-4    | All `Direct(InstanceId)` callees resolve in `model.instances` (construction invariant 9, B-7 close) | `visit_terminator::Call` (§25.6.3)             |
| B-Body-5    | basic_blocks ordering inherits §24.4 (B-RPO + B-Reachable + B-Entry per §25.6.2 procedure)          | `visit_body` DFS-RPO traversal                 |
| B-Body-6    | Inherited: §19.7 Place validation (B-0a)                                                           | `visit_place`                                  |
| B-Body-7    | Inherited: §20.5 Operand-site invariants                                                           | `visit_operand`                                |
| B-Body-8    | Inherited: §21.5 Rvalue-site invariants                                                            | `visit_rvalue`                                 |
| B-Body-9    | Inherited: §22.5 Statement-site invariants (B-1..B-5)                                              | `visit_statement`                              |
| B-Body-10   | Inherited: §23.6 Terminator-site invariants (B-1..B-9)                                             | `visit_terminator` (B-7 also covered by B-Body-4) |

**Inheritance discipline.** B-Body-6 through B-Body-10 are
per-construct invariants pinned by earlier stages; the joint
visitor enforces them at the appropriate hooks but the rule
statements live in their owning stages' sections (§19.7, §20.5,
§21.5, §22.5, §23.6). Stage-11 spec does not duplicate the
rules.

#### 25.9.3 Part C: Walk-implementation absorption (already covered)

The 5 deferred walk-implementations (D-5-2 / D-7-3 / D-8-4 /
D-9-10 / D-10-5) close at stage-11's joint visitor per §25.6.4.
Construction invariants 1 + 9 fold-in per §25.6.3.

#### 25.9.4 Part D: F4 / F4-2 cross-build identity scope

Body content is per-build. `LocalId` and `BlockId` numberings
are body-local and need not canonicalise across builds. The
Instance-side `RegseqInstanceKey` (§9.3 + stage 12 D-12-1.4)
is what canonicalises across builds; stage-11 body content
composes downstream. **F4 / F4-2 OUT of stage-11 scope** (same
as stages 5-10).

#### 25.9.5 Part E: Forward-looking notes

Walk-implementation invariants beyond §25.6 (e.g., type-coherence
of post-rewrite Calls per §23.6.2 holding only post-rewrite per
§25.7.2 pass scheduling) are pinned. Phase-1 implementation
sequences the rewrite pass before validation per §25.7.4.

### 25.10 C lowering

Body-to-C function lowering (signature template from
`InstanceKind::Function.fn_sig`, local-decl emission policy,
label-per-basic-block envelope, identifier emission for named
vs synthetic locals) is backend-codegen content; see
`generic-c-backend.md` §7 for the generic-C target. The
IR-side commitment at this section is the data shape the
backend consumes: signature from `InstanceKind::Function.fn_sig`
(per §25.4), parameter locals at `local_decls[1..=arg_count]`,
internal locals at `local_decls[arg_count+1..]`, basic blocks
ordered per `body.basic_blocks` storage order (RPO).

### 25.11 Cross-references

**Inbound** (closed by stage 11):

- Stage 1 `LocalId` definition (§25.3) + `Body` placeholder
  expansion (§25.1) + `FnSig` placement (§25.4; lives
  Instance-side per stage-12 D-12-1.4 Option B; supersedes
  stage-11 D-11-3 Item-side framing) + niche optimization
  (§25.3) + construction invariant 1 extension (§25.6.3).
- Stage 1 `Option<LocalId>` size assertions (§25.3).
- Stage 3 D5 body-level dyn-Trait dispatch rewriting pass
  (§25.7.2).
- Stage 5 D-5-2 Place validation walk (§25.6 absorption).
- Stage 7 D-7-3 Rvalue walk (§25.6 absorption).
- Stage 8 D-8-3 named-local preservation (§25.2 `debug_name`)
  + D-8-4 Part C Statement walk (§25.6 absorption).
- Stage 9 D-9-2 Drop-as-Call bridge concerns + fresh LocalIds
  API (§25.7.1 + §25.8) + D-9-3 (b) post-rewrite Call shape +
  D-9-10 Part C Terminator walk (§25.6 absorption) + B-7
  closure (§25.6.3 invariant 9).
- Stage 10 D-10-5 Part C Body walk (§25.6.2 absorption) +
  body-level basic-block table integration (§25.1
  `basic_blocks` field) + `LocalId::RETURN_PLACE` migration
  (§25.3 const-body declaration).

**Outbound** (filed at stage-11 merger contract close;
stage-11 D-11-3 + D-11-5's Item-side framing was reopened
at stage-12 D-12-1.4 / Option B; the stage-12 spec carries
the post-Option-B Instance-side placement):

- **Stage 12 expectations (post-Option-B).** FnSig and Body
  live on `InstanceKind::Function` per stage-12 D-12-1.4
  (supersedes stage-11 D-11-3's Item-side framing for
  fn_sig and stage-11 D-11-5's `Item.body` framing).
  Stage-11 spec edits land at stage-12 merger close: §25.4
  (FnSig location reframe), §25.5 (Body-vs-Instance linkage
  reframe), §23.6.2 (callee_fn_sig drops Item indirection),
  §9.4 (InstanceKind::Function gains fn_sig + body), §3.1
  invariant 10 (Body presence reword). Joins the existing
  stage-3 D5 + stage-9 D-9-3 (b) outbound bundle for
  vtable Item synthesis machinery.
- **Stage 13 expectations.** Categorical rendering bundle per
  D-11-K (§18.2 row retargeting in place at merger -- rows
  remain in §18.2 with new stage-13 ownership; rendering for
  body-level content all retargeted from per-stage K-tags to
  stage-13). 12-item bundle: 9 public renderers
  (`render_place`, `render_operand`, `render_rvalue`,
  `render_statement`, `render_terminator`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind`,
  `render_basic_block`), plus 1 internal helper
  (`render_terminator_kind`), plus 2 sigil rendering wirings
  (`_N` for `LocalId`, `bbN` for `BlockId`). Predecessor
  K-tag attribution preserved.

**Renderer reference.** All body-level rendering for §25
content lives in `pretty-printer.md` §3 (Body envelope) +
§7 (per-renderer specs). §25 contains no rendering
specification; renderer signatures, sigil rendering wiring,
and per-method ownership rows are stage-13-owned and
absorbed into `pretty-printer.md`.

(See `discussions.md` for the per-D-tag audit shape and the
stage-11 outbound bundles.)

---


## 26. Diagnostic logging (intrinsic family)

Diagnostic logging is the **sole phase-1 domain extension**
per `../README.md` §1.3 (the foundation-observability
carve-out). All other domain families (MMIO, interrupt,
atomic, clock, reset, ...) move to phase-2 stages 15+.
Stage 14 owns the full diagnostic-logging surface end-to-end:
author macros, post-expansion intrinsic contract, bridge
translation rules, validation, and the per-target backend
lowering record.

This section is the canonical home for that surface. §3.6.3
defines the `IntrinsicKind::Log(LogLevel)` storage variant
and the dash-form NAME mapping; §25.7.3 defines the
DiagLogPeel body-level pass; §23.6.2 B-4 and B-6 exceptions
state the `I-DiagLog-CallArity` invariant. This section
pulls those references together and adds the
diagnostic-logging-specific content (author macros, `LogArg`
carrier, format-template grammar, validation diagnostics,
backend lowering record, deferrals).

### 26.1 Overview

Authors emit operator-attention diagnostics via four
severity-named declarative macros at `regseq::log::<sev>!`:

```rust
regseq::log::error!("device {} init failed at offset 0x{:08x}", name, off);
regseq::log::warn!( "frame counter wrapped after {} ticks", count);
regseq::log::info!( "link up at {} Mbps", speed_mbps);
regseq::log::debug!("rx ring head={} tail={}", head, tail);
```

The compiler captures each call as a structured logging
intent (severity + format template + typed argument carrier)
and lowers the call directly into the target backend's native
severity-tagged emitter via Pipeline A (post-DiagLogPeel
flat-arg Call per §26.6; backends consume the peeled operands
element-by-element). Per-target emitter shapes (e.g., the
generic-C target's `printf` with leading severity tag) are
backend-codegen content; see `generic-c-backend.md` §8 for
the generic-C target's binding and the phase-2
`<target>-backend.md` artifacts for each subsequent target.
Emission suppression in the deployed artifact is delegated
to the target's native filter; the DSL provides no gating
axis of its own (per the construct contract; see §26.10).

**Pipeline A guarantee.** The `LogArg` argument carrier,
the slice that the macro constructs to pass it, and the
`regseq::log::intrinsics::log_<sev>` function itself do
NOT appear in the generated target code. The bridge peels
the carrier at body-construction time (§25.7.3
DiagLogPeel); the backend emits a direct call to the
target's native printf-family emitter. This guarantee is
load-bearing for the zero-runtime-cost-on-emit contract.
The DiagLogPeel pass exists for this guarantee.

**Construct posture.** The construct is intrinsic-heavy.
There is no trait, no callback surface, and no typed Rust
resource on the author surface. The only Rust types
introduced (`LogArg`) are macro-internal and not
author-named.

### 26.2 Severity vocabulary + intrinsic NAME mapping

The `LogLevel` enum is the closed 4-tuple:

```rust
pub enum LogLevel { Debug, Info, Warn, Error }
```

No `Trace`. No `Critical`. Per-level intrinsics (four
distinct callees), not a single intrinsic parameterised by
a level operand: per-level matches both the source taxonomy
(refined-construct.md P1 outcome) and the natural Backend
lowering pattern (each target binds severity at compile
time, not via a runtime branch).

Intrinsic NAME mapping (the `&str` that appears inside
`#[rseq::intrinsic("...")]` on the facade-side stubs;
dash form per the NAME-form rule in §3.6.3):

| `LogLevel` variant | Macro path             | Function name (Rust) | Intrinsic NAME |
|--------------------|------------------------|----------------------|----------------|
| `Debug`            | `regseq::log::debug!`  | `log_debug`          | `log-debug`    |
| `Info`             | `regseq::log::info!`   | `log_info`           | `log-info`     |
| `Warn`             | `regseq::log::warn!`   | `log_warn`           | `log-warn`     |
| `Error`            | `regseq::log::error!`  | `log_error`          | `log-error`    |

The `IntrinsicKind::from_name` lookup at §3.6.3 is
authoritative for the NAME -> `IntrinsicKind::Log(LogLevel)`
mapping. Unknown NAMEs are rejected at Phase-2 recognition
per the Q7 closed-vocabulary rule.

### 26.3 Intrinsic function shape

The four facade-side stub functions are the bridge's
recognition targets. Identical shape across severities;
shown for `error`:

```rust
#[inline(never)]
#[rseq::intrinsic("log-error")]
pub fn log_error(fmt: &'static str, args: &[LogArg<'_>]) {
    unimplemented!()
}
```

**Required attributes** (load-bearing for the recognition
contract):

- `#[inline(never)]` -- blocks rustc's MIR inliner and
  LLVM's later inliner from collapsing the call site
  before the bridge sees it. The MIR inliner is the
  primary concern: it runs as a MIR optimisation pass
  before the bridge invokes its translation; without
  `#[inline(never)]` rustc can inline the
  `unimplemented!()`-bodied function and erase the call
  site DiagLogPeel needs to pattern-match against.
- `#[rseq::intrinsic("log-<sev>")]` -- declares the stable
  intrinsic NAME the bridge keys on per Q7 + D-14-1. The
  attribute's NAME-payload validation chain is recorded
  at §3.6.4 (Phase 2 recognition; the parse-then-lookup
  helper `IntrinsicKind::from_name` is authoritative).

**Signature.** Exactly two parameters: a `&'static str`
format template + a `&[LogArg<'_>]` slice. Return type
is `()`.

**Body.** `unimplemented!()` by design. The bridge replaces
the call before the body executes. A runtime panic from
`unimplemented!()` reaching execution signals that the
Regseq pass did not run on this build -- a build-pipeline
misconfiguration, not a soundness bug.

**Location.** `regseq::log::intrinsics::log_<sev>` in a
`#[doc(hidden)] pub mod intrinsics` submodule of
`regseq::log`. The submodule (and the `LogArg` type per
§26.4) is hidden from rustdoc and from the author surface;
only the macro expansion references the path.

**Item-level metadata in RegseqModel.** After bridge
recognition, each `log_<sev>` Instance carries
`AttrData::Intrinsic(IntrinsicKind::Log(<level>))` in
`model.domain_attrs.item_attrs[item_id]` per §3.6.6 + the
two-phase population at §3.6.4. Consumer-side access via
`model.instance_intrinsic(instance_id)` per §3.5.2.

### 26.4 `LogArg` argument carrier

The closed Regseq-owned argument carrier. Variant tags pin
the admissible argument-type set:

```rust
pub enum LogArg<'a> {
    U8(u8),   U16(u16), U32(u32), U64(u64),
    I8(i8),   I16(i16), I32(i32), I64(i64),
    Bool(bool),
    Str(&'a str),
}
```

Ten variants. Width-explicit (one variant per native
width). The lifetime parameter `'a` is the borrow lifetime
of `&str` arguments; for value-typed variants the lifetime
is decoupled from the carrier.

**`From` impl set (closed).** Each admissible source type
has a dedicated `From` impl (no marker-trait coverage; per
admissible type so the unimplemented-trait error message
points at the author argument expression). For each `T` in
`{u8, u16, u32, u64, i8, i16, i32, i64, bool}`:

```rust
impl<'a> From<T>      for LogArg<'a> { /* lifts to LogArg::<Variant>(v)  */ }
impl<'a> From<&T>     for LogArg<'a> { /* lifts to LogArg::<Variant>(*v) */ }
impl<'a> From<&mut T> for LogArg<'a> { /* lifts to LogArg::<Variant>(*v) */ }
```

For `&'a str`:

```rust
impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }
```

Platform-width aliases route via `#[cfg(target_pointer_width
= ...)]`:

```rust
#[cfg(target_pointer_width = "64")]
impl<'a> From<usize>      for LogArg<'a> { /* -> LogArg::U64(v as u64) */ }
#[cfg(target_pointer_width = "64")]
impl<'a> From<&usize>     for LogArg<'a> { /* -> LogArg::U64(*v as u64) */ }
#[cfg(target_pointer_width = "64")]
impl<'a> From<&mut usize> for LogArg<'a> { /* -> LogArg::U64(*v as u64) */ }
// equivalent triplet under target_pointer_width = "32" routing to LogArg::U32.
// equivalent triplets for isize routing to LogArg::I64 / LogArg::I32.
```

**One level of indirection only.** `&&T`, `&mut &T`, and
higher-arity indirections have no impls; authors
dereference explicitly.

**Reference-companion impls are macro-side ergonomics
only.** `From<&T>` and `From<&mut T>` exist so authors can
write `info!("{}", &x)` (or `info!("{}", &mut y)`) without
explicit `*x` derefs. The reference companions do NOT
introduce a reference-typed `LogArg` carrier variant. The
`From<&T>` / `From<&mut T>` impl bodies dereference the
input (`LogArg::<Variant>(*v)`); semantically the reference
is consumed to produce a value-typed `LogArg` variant. The
**bridge mirrors this discipline at the IR level**:
DiagLogPeel (§25.7.3 step 2) synthesises a load before the
flat Call when it recognises a reference-companion `From`
impl. The IR Call's args therefore carry value-typed
Operands uniformly; reference-companion Operands never
survive into RegseqModel. The §26.9 backend specifier table
consequently covers only value-typed admissible shapes
(`Ty::Uint(U8..U64)`, `Ty::Int(I8..I64)`, `Ty::Bool`,
`Ty::Ref(Ty::Str, _)`); no `Ty::Ref(<value type>, _)`
specifier row is needed because no such operand reaches
the backend.

The `From<&'a str>` impl is NOT a reference-companion in
this sense -- it is the **by-value** impl for the
`Ty::Ref(Ty::Str, _)` admissible type (the `Str` carrier
variant stores the reference directly because `&str` IS
the value-shape for the carrier; there is no inner type
to dereference into). DiagLogPeel does not synthesise a
load for `From<&'a str>`.

**Deliberately rejected source types** (each surfaces D4
per §26.8 at the macro expansion site). The list below
names commonly-tried specific types; the **catch-all rule**
follows the named list and closes the set.

- `u128`, `i128` -- not portably supported across target
  printf-family emitters.
- `f32`, `f64` -- floats restricted in kernel context on
  multiple targets; outside the carrier scope.
- `*const T`, `*mut T` -- the DSL does not expose raw
  pointers; address formatting goes through hex-formatting
  an integer-typed address.
- `c_char` as a distinct type -- aliases to `i8` or `u8`
  per platform; authors lift through the integer impls.
- `&T` and `&mut T` for `T` outside the supported
  value-typed set -- no auto-coercion of struct references
  to an address variant.
- `Option<T>`, `Result<T, E>` -- authors unwrap or extract
  before passing.
- `&[T]`, `Vec<T>`, `String`, `CString` -- out of v1
  scope; per-element or null-terminated formatting needs a
  separate policy.
- `Ty::FnDef(_)`, `Ty::FnPtr(_)`, `Ty::Closure(_)` --
  function items, function pointers, and closure values
  are not loggable. Pointer-formatted function addresses
  go through hex-formatting an integer-typed address (the
  same rule as `*const T`).
- Tuple values (`(T1, T2, ...)`), array values
  (`[T; N]`), and slice values (`[T]`) -- aggregates have
  no native printf-family representation; authors
  destructure and pass elements individually.
- Interior-mutability wrappers and other ADT wrappers
  whose concrete `Ty` after monomorphisation is not in
  the admissible value-typed set: `core::cell::Cell<T>`,
  `core::cell::RefCell<T>`, `core::sync::atomic::AtomicU*` /
  `AtomicI*`, `core::mem::MaybeUninit<T>`, and similar.
  Authors `load` / `get` / unwrap the inner value before
  passing.
- Opaque `impl Trait` whose concrete monomorphised type
  is not in the admissible set.
- Custom structs and enums -- out of v1 scope; would
  re-introduce trait-driven formatting (`Display` /
  `Debug`) which the construct contract excludes.

**Catch-all closure rule.** The admissible set is closed
**by the `From` impl set** (the impls enumerated above).
Any source-Rust value whose monomorphised `Ty` lacks a
matching `From<...> for LogArg<'a>` impl is rejected by
D4 -- whether or not it appears on the named-rejected
list above. The named list above is a documentation aid,
not an exhaustive enumeration; the `From` matrix is the
authoritative closure. For an arbitrary
post-monomorphisation `Ty`, admissibility is decided by
membership in the matching value or one-level-reference
shape (`Ty::Uint(UintTy::U8..U64)`, `Ty::Int(IntTy::I8..I64)`,
`Ty::Bool`, `Ty::Ref(Ty::Str, _)`, and the `&T` /
`&mut T` companions per the impl matrix above; plus
`usize` / `isize` via the cfg-routed impls); all other
`Ty` shapes fall outside the matrix and surface D4.

**Carrier survival.** `LogArg` does NOT survive into
RegseqModel after the bridge runs. The DiagLogPeel pass
(§25.7.3) peels every `LogArg::from(<expr_i>)` wrapper at
body-construction time, drops the slice aggregate, and
synthesises the flat-arg Call shape. The
`I-DiagLog-CallArity` invariant (§23.6.2 B-4 and B-6
exceptions) describes the post-peel shape.

**Visibility.** `LogArg` lives at
`regseq::log::intrinsics::LogArg` and is `#[doc(hidden)]`
(via the parent `intrinsics` module). Author code never
names it; only the macro expansion references it. D4
diagnostics (§26.8) report against the original author
argument expression, not the `LogArg::from(...)` wrapper.

### 26.5 Author macros + post-expansion shape

The four severity-named macros are declarative
(`macro_rules!`; no proc-macro crate). Each is marked
`#[macro_export]` at definition; re-exported at
`regseq::log::<sev>!` for the canonical author path.
Identical shape across severities; shown for `error`:

```rust
#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::intrinsics::log_error(
            $fmt,
            &[ $( $crate::log::intrinsics::LogArg::from($arg) ),* ],
        )
    };
}
```

The expansion contract pins three properties the bridge
keys on:

1. **Callee is one of the four `log_<sev>` Instances.** No
   other callee can match the bridge's
   `IntrinsicKind::Log(_)` recognition.
2. **First arg is a `&'static str` literal.** The
   `$fmt:literal` matcher enforces this at the macro level
   (rejecting `const TPL: &str = "..."; warn!(TPL, x)`
   ergonomically; D-14-2.7 D6 captures the structural
   invariant for the bridge side).
3. **Second arg is a slice literal of `LogArg::from(<expr>)`
   calls.** Exactly one `From::from` wrapper per author
   argument; no nested expressions. The carrier slice is
   the surface DiagLogPeel peels.

**Argument-evaluation contract.** Arguments are evaluated
at the call site unconditionally. The macro expands to a
direct call; there is no lazy-evaluation gate behind a
level check. Authors must avoid placing expensive
computations or side-effecting expressions inside log calls
on hot paths. Target-aware codegen-level elision (preserving
each target's native laziness mechanism, e.g., Linux's
`pr_debug` dynamic-debug branch wrapping args in `if`) is
deferred to a future revision per §26.10; the current
phase-1 contract carries no laziness.

**Future-macro discipline.** A future revision that breaks
the expansion shape MUST update this section's contract and
DiagLogPeel's canonical-shape assertion (§25.7.3 step 1).
The shape is part of the contract, not just an
implementation detail.

### 26.6 Bridge translation contract

The bridge translates source-level diagnostic-logging calls
into a desugared flat-arg Call shape that the backend
lowers directly to a target-native printf-family emit. The
mechanism is the DiagLogPeel pass at §25.7.3; this section
states the contract and the named invariant.

**Translation rule (Option A).** For any MIR `Call` whose
callee resolves to a RegseqModel Instance whose
`domain_attrs.item_attrs` entry contains
`AttrData::Intrinsic(IntrinsicKind::Log(level))`, the
bridge peels the canonical `&'static str` + slice shape
into a flat-arg Call shape. The `LogArg::from(...)`
wrappers, the slice aggregate, and the `LogArg` type do
NOT appear in RegseqModel. Procedure: §25.7.3.

**Resulting Call shape (`I-DiagLog-CallArity`).** Named
invariant for cross-stage citation:

> A Call whose callee `cc = CallCallee::Direct(id)`
> satisfies `model.instance_intrinsic(id) ==
> Some(IntrinsicKind::Log(_))` has args
> `[Operand::Constant(<&'static str literal>),
> <value_operand_1>, ..., <value_operand_N>]` where
> `N >= 0` is the number of `{}` placeholders in the
> literal. `args.len() == 1 + N`. Each
> `<value_operand_i>` has a **value-typed** RegseqModel
> `Ty` per the admissible set: `Ty::Uint(UintTy::U8..U64)`,
> `Ty::Int(IntTy::I8..I64)`, `Ty::Bool`, or
> `Ty::Ref(Ty::Str, _)`. Reference-companion source-Rust
> types (`&u32`, `&mut bool`, etc.) are normalised by
> DiagLogPeel (§25.7.3 step 2) before reaching the flat
> Call; they do not survive into RegseqModel. The
> declared callee signature `fn(&'static str,
> &[LogArg<'_>])` (two inputs) does not constrain
> `args.len()` at the call site; the desugared shape is
> the load-bearing invariant. Indirect callees
> (`CallCallee::Indirect(Operand)`) cannot satisfy the
> predicate (IntrinsicKind is keyed by Instance identity).

§23.6.2 B-4 and B-6 exception clauses cite
`I-DiagLog-CallArity` as the operative invariant for
Log-intrinsic callees.

**Transient fingerprint discipline.** DiagLogPeel produces
a call-site fingerprint at step 5 (severity + fmt template
+ per-arg `Ty` + span) consumed immediately by validation
(§26.8) and discarded. The fingerprint is NOT stored on
`RegseqModel`. Backend lowering re-derives the same
information from the IR:
- Severity: `model.instance_intrinsic(callee_instance_id)`
  per §3.5.2 -> `IntrinsicKind::Log(level)`.
- Per-arg `Ty`: `operand_ty(args[i])` for
  `i >= 1` per the standard operand-type rules.

A side table keyed by call-site identity (e.g., a new
sibling on `DomainAttrTables`) would duplicate information
the regular IR carries -- per the composition-over-flags
rule, no side table is introduced. (Were a future revision
to need information the IR cannot carry -- e.g., a
modifier-set selection per call site beyond what the
template literal carries -- the question reopens.)

**Composition with concurrent passes.** DiagLogPeel runs
inside Phase 5 body materialisation per §25.7.4, alongside
Drop-as-Call collapse. The two are mutually exclusive
per-rustc-Terminator: a rustc Terminator is at most one of
`Drop` (handled by Drop-as-Call) or `Call` to a
Log-intrinsic callee (handled by DiagLogPeel); no
Terminator is both.

### 26.7 Format template grammar

The Regseq format template is a restricted printf-style
subset, owned by Regseq (not `core::fmt`). Stage-14 surface:

**Template form.** `&'static str` literal at the macro call
site. Structurally enforced by the macro's `$fmt:literal`
matcher (rejects const-reference indirection like
`const TPL: &str = "..."; info!(TPL, x)`); D-14-2.7 D6 also
asserts this at the bridge to defend against future
macro-mediated escape hatches.

**Placeholders.** `{}` only -- no named arguments, no
positional indexes (`{0}`, `{1}`, ...), no dynamic
precision (`{:.*}` with a precision-arg), no
`Display`/`Debug` formatter identification (`{:?}` etc.).

**Closed modifier subset (stage-14 v1).** Each `{}` may
carry one of the following modifiers; the leading set is:

| Modifier  | Meaning                              | Compatible LogArg variants |
|-----------|--------------------------------------|----------------------------|
| (none)    | default `{}`                         | any                        |
| `:x`      | lowercase hex                        | integer-width (U8..U64, I8..I64) |
| `:X`      | uppercase hex                        | integer-width              |
| `:#x`     | lowercase hex with `0x` prefix       | integer-width              |
| `:#X`     | uppercase hex with `0x` prefix       | integer-width              |
| `:08x`    | zero-padded 8-wide lowercase hex     | integer-width              |
| `:#010x`  | zero-padded 10-wide lowercase hex with `0x` prefix | integer-width |

The `:p` modifier (raw-pointer formatting in stock Rust)
is NOT in the subset -- the DSL exposes no raw pointers;
address formatting goes through hex-formatting an
integer-typed address.

The subset is **closed at the leading set above for v1**.
Wider modifier coverage (other widths, decimal padding,
alignment specifiers, ...) belongs in the experiments track
at `docs/discussions/logging-and-printing/` and may extend
the subset in a future revision; no automatic
backwards-compatibility commitment binds future extensions
to the v1 set.

**Escape sequences.** `{{` and `}}` are literal `{` and `}`
in the rendered output. They are NOT placeholders: the
compiler treats them as literal text (no entry in
`placeholder_kinds`, no contribution to `placeholder_count`,
no per-position validation pair).

**Trailing newline forbidden.** Templates MUST NOT end with
`\n`, `\r`, or `\r\n`. Backend lowering appends the
target-appropriate terminator (per the backend's emitter
contract); a trailing newline in the template would produce
a double terminator on every target. Embedded newlines mid-template are permitted and
render as visible line breaks in the output; multi-line
emissions from a single call are a supported pattern.
D-14-2.7 D7 enforces.

**Empty template legal.** `regseq::log::info!("connected")`
is well-formed: 0 placeholders + 0 arguments.

### 26.8 Bridge-side validation diagnostics

The bridge validates each call-site fingerprint produced by
DiagLogPeel (§25.7.3 step 5) against the format-template
grammar (§26.7) and the `LogArg`-admissible argument-type
set (§26.4). A `RegseqError`-shaped diagnostic emits per
failure. All diagnostics use the original Rust call-site
span; D-14-2.7's diagnostic-quality requirement (below)
binds the author-facing error to the offending
author-source expression, not to bridge-internal machinery.

**Bridge-emitted diagnostics:**

| #  | Rule                                                                                       | Trigger                                  |
|----|--------------------------------------------------------------------------------------------|------------------------------------------|
| D1 | `placeholder_count == argument_count` (template-arg arity parity)                          | post-DiagLogPeel fingerprint validation  |
| D2 | Per-position modifier compatible with operand `Ty` (e.g., hex modifier requires integer-width operand) per §26.7 modifier table | post-DiagLogPeel fingerprint validation |
| D3 | Every modifier in template is a member of the closed Regseq subset (§26.7)                | post-DiagLogPeel fingerprint validation  |
| D4 | Every operand's source-Rust `Ty` is admissible per §26.4's `From`-closed set; rejection list per §26.4 | macro expansion + bridge fingerprint validation (defense in depth) |
| D6 | `args[0]` resolves to a string literal at the **MIR shape** (pre-Regseq-const-lowering); not an indirect const reference, not a runtime expression, not a non-string literal. Checked at DiagLogPeel step 1 (§25.7.3) because post-Regseq-const-lowering both literal and const-reference forms serialise to `Const::Aggregate([Reloc, Uint])` (§9.7) and are no longer distinguishable. | DiagLogPeel step 1 (pre-lowering) |
| D7 | Template does NOT end in `\n`, `\r`, or `\r\n`                                              | post-DiagLogPeel fingerprint validation  |
| D8 | `&str` operand count in args[1..] <= **selected binding target's** declared `max_str_args_per_call` (the bound is backend-declared per-target -- see `generic-c-backend.md` §8.4 for the generic-C target; phase-2 targets declare in their respective `<target>-backend.md` artifacts; counts ALL `Ty::Ref(Ty::Str, _)` operands without provenance distinction; args[0] format template excluded). Bounds per-call stack use against the per-`%s`-site temporary-buffer allocation. Evaluated once per (call site, target) pair: a single build is one target; cross-target validation (when a future stage adds multi-target build) repeats D8 per target. | post-DiagLogPeel fingerprint validation against the build's selected target |

**D5 deferred.** The atomic-context analysis ("Call site is
reachable from an atomic context for which the target offers
no compatible emitter at the chosen severity") depends on
the call-site context fact owned by the interrupt construct.
**Deferred to phase-2 stage 17 (interrupt / ISR family).**
Filed as an inbound expectation on
`stage-17/expectations.md`. Phase-1 implementations of the
diagnostic-logging pipeline operate as if every call site is
atomic-safe (the generic-C target has no atomic context to
worry about; the Linux backend at phase-2 stage 15 emits
without atomic-context guarding until stage 17 lands).

**Diagnostic-message quality requirement (binding on impl).**
D4's author-facing message MUST report against the original
author argument expression (e.g., "value of type `MyStruct`
cannot be passed to `regseq::log::info!`; only the primitive
carrier types are accepted") rather than against the
bridge-synthesised `LogArg::from(...)` wrapper (which is
gone after DiagLogPeel anyway). Implementations may use
`#[diagnostic::on_unimplemented]` on the `From` impls,
custom proc-macro diagnostics, or equivalent
Rust-toolchain affordances to preserve the author-side
span; the exact error-message text is not part of the
construct contract, but the span-preservation requirement
IS part of the contract.

### 26.9 Backend lowering contract

Per-target lowering tables (per-`LogLevel` C call shape,
per-arg C format specifier, `&str` NUL-termination strategy,
`max_str_args_per_call` declaration) are backend-codegen
content and live in the per-target backend merger artifact
(see README §5.9 rule 7). Stage 14 specs the **generic-C
binding** -- the phase-1 implementation territory: hosted
userspace, libc, `printf` -- at `generic-c-backend.md` §8.

Forward-looking content for phase-2 targets (Linux kernel
`pr_*` / `dev_*` family, Linux `max_str_args_per_call`
declaration, Zephyr / FreeBSD / Windows / macOS bindings) is
not embedded here per the "no forward-looking placeholders"
rule (README §5.9 rule 4). Each phase-2 backend stage owns
its own target's contract, re-derived against the pinned
toolchain and target evidence available at that stage.
Linux-binding inbound expectation filed on
`stage-15/expectations.md`.

**Pipeline A uniformity.** All targets apply the same
DiagLogPeel-output IR shape (the flat-arg Call per §26.6).
The per-target lowering reads the peeled operands and emits
the target's printf-family call; no target reconstructs the
`LogArg` slice. Because DiagLogPeel removes `LogArg` and
the carrier slice from RegseqModel entirely (§25.7.3), a
backend that observed a `LogArg`-variant-tag runtime
dispatch shape would have had to **reconstruct the
pre-DiagLogPeel carrier** -- this is explicitly forbidden:
backends MUST NOT reconstruct a `LogArg` slice and MUST
NOT lower any pre-DiagLogPeel carrier shape. The
zero-runtime-cost-on-emit guarantee rests on this rule.
This uniformity rule is IR-side and binding on every
backend artifact (current and future).

### 26.10 Deferrals and scope boundaries

**Deferred to phase-2 stage 15 (regseq-trait scaffolding +
Lifecycle + driver-struct discovery + RegseqError):**

- **`register_log_module`** -- compiler-emitted glue at
  driver-module entry that registers the driver's log
  module with the target's logging facility. Per-target
  binding: `pr_fmt` setup on Linux, `LOG_MODULE_REGISTER`
  on Zephyr, device-class log binding on FreeBSD,
  `WPP_INIT_TRACING` on Windows, `os_log` subsystem /
  category binding on macOS. Stage-15 owns because the
  driver-identity / lifecycle hook this composes with is
  stage-15 territory. Not part of the author surface.
- **Device-context joining** -- compiler-injected device
  handle that surfaces in the lowered call (e.g., `dev_err(dev,
  ...)` on Linux instead of `pr_err(...)`). Required for
  Linux's `dev_*` family; irrelevant for the generic-C
  target (hosted userspace, no device handle). The
  bridge-side mechanism that joins device-identity at
  call-site lowering is stage-15 territory (driver-struct
  discovery surfaces the device handle).

**Deferred to phase-2 stage 17 (interrupt / ISR family):**

- **Atomic-context analysis (§26.8 D5)** -- the call-site
  context fact "this `emit_diagnostic` call is reachable
  from an atomic context" depends on the interrupt
  construct's call-site context analysis, which is
  stage-17 territory.

**Deferred to a future revision (not phase-2-scheduled):**

- **Target-aware codegen-level elision for `debug!`** --
  preserving each target's native laziness mechanism
  (Linux `pr_debug` dynamic-debug branch, Zephyr
  compile-time `LOG_LEVEL` elision, Windows WPP flag
  filtering, macOS unified-logging level filtering). The
  phase-1 contract evaluates arguments at the call site
  unconditionally. A proper resolution would let the
  per-target lowering emit each target's native lazy form;
  deferred so the design can account for the per-target
  mechanism uniformly rather than papering over with a
  DSL-level toggle.
- **Wider format-template modifier coverage** -- the
  closed leading subset at §26.7 is v1; extensions belong
  in the experiments track at
  `docs/discussions/logging-and-printing/`.
- **Per-target `&str` null-termination strategy choice**
  -- each binding backend pins its own materialisation
  strategy at implementation-sweep time. **Generic-C is
  pinned in `generic-c-backend.md` §8.3** (static-literal
  direct emission + temporary stack-buffer fallback for
  non-static `&str`). Linux backend (phase-2 stage 15) pins
  null-termination at its implementation sweep in
  `linux-backend.md`; future targets pin theirs when their
  backend artifacts open.
  The deferred axis is the **inter-target** comparison
  / common-helper extraction; not the per-target binding
  obligation itself.

**Explicitly out of scope:**

- **DSL-level emission gating.** No `debug_logging` toggle.
  No DSL flag for runtime / compile-time emission
  suppression. Emission suppression is delegated to the
  target's native filter (per the construct contract).
- **`emit_structured_event`** (Track-B extension from the
  taxonomy work) -- not represented in phase-1.
- **Function-style stable logging API** -- preserve macro
  form; no `regseq::log::error_fn(...)` callable surface.
- **Driver-passed device handle on author surface.**
  Device context is joined implicitly per stage-15 (see
  above); authors never pass a device handle to
  `regseq::log::*!` macros.

### 26.11 Cross-references

- §3.6 -- domain attribute tables (where
  `IntrinsicKind::Log(LogLevel)` is stored).
- §3.6.3 -- `IntrinsicKind` enum + dash-form NAME mapping.
- §3.6.4 -- two-phase recognition + materialisation.
- §3.6.5 -- per-family validation invariants
  (`I-DomainAttr-Intrinsic-Singleton` constrains
  `IntrinsicKind::Log(_)`).
- §3.6.6 -- attribute-namespace storage rule
  (`#[rseq::intrinsic("log-<sev>")]` -> DomainAttr).
- §3.5.2 -- `model.instance_intrinsic(instance_id)`
  pull-through helper.
- §23.6.2 B-4 + B-6 -- `I-DiagLog-CallArity` exception
  clauses.
- §25.7.3 -- DiagLogPeel pass mechanics.
- §25.7.4 -- pass scheduling diagram.

**Rendering.** Diagnostic-logging calls render as standard
`Call` terminators per the body-level rendering rules in
`pretty-printer.md`. The post-DiagLogPeel flat-arg shape
renders naturally: the callee path identifies the
log-intrinsic Instance, and the args render
element-by-element. No special-case rendering rule.

---


## Appendix A: Bridge-rejection examples

C-lowering worked examples (reference + pointer types,
fat-pointer types, niche-encoded enums, `UnsafeCell` aliasing,
packed structs, `&dyn Trait` lowering, closures, `repr(u32)`
enum, niche-encoded `Option<&u32>`) live in
`generic-c-backend.md` §9 per README §5.9 rule 7. Linux-target
worked examples (MMIO read intrinsic, atomic acquire-load)
re-derive in phase-2 stages 16 / 17 against the Linux backend
spec; the dropped examples are filed as inbound expectations
on `stage-16/expectations.md` and `stage-17/expectations.md`.

This appendix retains only IR-side bridge-rejection examples
that illustrate spec-level rejection rules, not C codegen.

### A.1 Rejected `*mut c_void` usage

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

The opaque-struct pattern is structurally an `Adt` (an
ordinary `Ty::Struct`). Backend C-lowering for the rewrite is
backend-codegen content; the generic-C target's rendering of
opaque-handle struct pointers lives in `generic-c-backend.md`
(per-`Ty` table at §2).
