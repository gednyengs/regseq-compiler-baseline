# regseq-model-65: Root `RegseqModel` + Foundation Conformance

## Description

Refresh the existing root `RegseqModel` struct at
`crates/regseq-model/src/model.rs` to the locked phase-1 spec §3 / §3.1
shape, add the `TargetInfo` / `BuildInfo` / `Endian` / `ScalarLayouts`
foundation types from spec §3, introduce the `BridgeInput` struct
(§3.5.1) and Instance-side pull-through helpers (§3.5.2), and pin the
20 construction invariants in a registry the validator tasks
(`P1a-VAL-01`/`02`/`03`) read. This task also closes foundation
conformance for the spec's §2 (ID system) / §4 (StringInterner) /
§5 (Source files and spans) / §6 (CrateInfo) / §7 (DefPath) tables --
the existing Stage-1-grandfathered foundation predates the locked
phase-1 spec and must be brought up to spec-mandated shape before
later phase-1a tasks land.

Per stage-12 D-12-1.4 (Option B; spec.md:315-321), this task drops the
legacy `bodies: IndexVec<BodyId, Body>` table from the root struct
(bodies live structurally inside `InstanceKind::Function { body:
Option<Body>, ... }` per spec §9.4). The remaining root-struct fields
are amended to add `target` / `build` / `item_synthetic_kinds` /
`sig_c_abi_lowerings` / `vtables` (`#[serde(skip)]`) / `domain_attrs`,
and the existing `validate(&self)` signature is extended to
`validate(&self, bridge_input: &BridgeInput)` per stage-12 D-12-4.3.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (the brief; this task
  is the foundation row of Axis 1 -- P1a-DS-01)
- `docs/discussions/regseq-model/spec.md` §2, §3, §4, §5, §6, §7
  (foundation alignment + the new §3 root-struct shape)
- `crates/regseq-model/src/model.rs` (existing root struct;
  amend in place per the spec)
- `crates/regseq-model/src/ids.rs`,
  `crates/regseq-model/src/idx.rs`,
  `crates/regseq-model/src/index_vec.rs` (§2 ID system; verify
  conformance and extend with any missing surface)
- `crates/regseq-model/src/string_interner.rs` (§4 StringInterner;
  verify conformance)
- `crates/regseq-model/src/source.rs` (§5 source files / spans;
  verify conformance)
- `crates/regseq-model/src/crate_info.rs` (§6 CrateInfo;
  verify conformance)
- `crates/regseq-model/src/def_path.rs` (§7 DefPath; verify
  conformance)
- `crates/regseq-model/src/placeholders.rs` (placeholder `Ty` /
  `Const` / `Item` / `FnSig` / `Body` -- referenced as types here;
  expansion lands in P1a-DS-02..DS-14b)
- `crates/regseq-model/src/instance.rs` (existing Instance shell
  -- pull-through helpers attach to `RegseqModel`, but consume
  `Instance` fields)

## Implementation Notes

Amend `crates/regseq-model/src/model.rs` in place. Add the new
foundation types in a sibling module (`target.rs` / `build.rs` /
`bridge_input.rs` / `domain_attr.rs`) re-exported from `lib.rs`;
the registry of construction invariants lives next to the
`InvariantViolation` enum.

### Public API (verbatim from spec)

#### `TargetInfo` (spec.md:215-233)

```rust
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
```

`SizeAlign`, `INT_KIND_COUNT`, `UINT_KIND_COUNT`,
`FLOAT_KIND_COUNT` land at P1a-DS-03 (Layout) / P1a-DS-02 (Ty);
this task introduces them as forward-declared placeholder
re-exports if needed (or accepts `pub struct SizeAlign { pub size:
u64, pub align: u64 }` here so DS-02 / DS-03 amend rather than
re-define). The constants are `usize` per spec.md:3700-3702;
provide them with values matching the spec (`6` / `6` / `4`).

#### `BuildInfo` (spec.md:235-247)

```rust
pub struct BuildInfo {
    pub toolchain:           StringId,
    pub bridge_options_hash: u128,
}
```

#### `RegseqModel` root (spec.md:259-348)

```rust
pub struct RegseqModel {
    // Crate inventory.
    pub crates:        IndexVec<CrateId, CrateInfo>,
    pub primary_crate: CrateId,

    // Target context.
    pub target: TargetInfo,

    // Build provenance.
    pub build: BuildInfo,

    // Interning tables. `source_files[0]` is the synthesised-source sentinel.
    pub strings:      StringInterner,
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    // Content-keyed interners.
    pub types:  TyInterner,        // P1a-DS-04 lands the real type
    pub consts: ConstInterner,     // P1a-DS-05 lands the real type
    pub sigs:   FnSigInterner,     // P1a-DS-06 lands the real type

    // Source-level item alphabet.
    pub items:                IndexVec<ItemId, Item>,
    pub item_def_paths:       IndexVec<ItemId, DefPathHash>,
    pub item_synthetic_kinds: IndexVec<ItemId, SyntheticItemKind>,

    // C ABI lowering side table, parallel to `sigs`.
    pub sig_c_abi_lowerings: IndexVec<FnSigId, Option<CAbiLowering>>,

    // No separate BodyId. Per stage-12 D-12-1.4 (Option B), bodies
    // live as a structural field on `InstanceKind::Function {
    // body: Option<Body>, ... }` per §9.4.

    // Primary monomorphised table.
    pub instances: IndexVec<InstanceId, Instance>,

    // Bridge-side vtable dedup table. `#[serde(skip)]`; rebuilt on
    // deserialise from `items` by scanning `item_synthetic_kinds`
    // for `SyntheticItemKind::VTableType { principal }` entries.
    #[serde(skip)]
    pub vtables: HashMap<PrincipalTraitKey, ItemId>,

    // Domain attribute sibling tables per §3.6.
    pub domain_attrs: DomainAttrTables,
}
```

Forward-declared types whose concrete bodies land in subsequent
phase-1a tasks may be introduced here as **stub types** (empty
struct / single-discriminant enum) so the root struct compiles --
P1a-DS-04 (TyInterner), P1a-DS-05 (ConstInterner), P1a-DS-06
(FnSigInterner), P1a-DS-07 (Item), P1a-DS-08b (`SyntheticItemKind` +
`PrincipalTraitKey`), P1a-DS-09 (`DomainAttrTables`) replace the
stubs. **Mark each stub with a `// FORWARD: P1a-DS-NN` comment**
so later tasks have an unambiguous insertion point.

#### `BridgeInput` (spec.md:1314-1335)

```rust
pub struct BridgeInput {
    pub spec_crate_stable_id: StableCrateId,
}

impl BridgeInput {
    pub fn matching(model: &RegseqModel) -> Self { /* ... */ }
}
```

`RegseqModel::validate` signature changes from
`validate(&self) -> Result<...>` to
`validate(&self, bridge_input: &BridgeInput) -> Result<...>` per
D-12-4.3 (spec.md:1337-1340). The pre-existing validator coverage
(invariants 1, 2, 4-partial, 5, 8, 10) is preserved and extended;
new invariants (3, 6, 11, etc.) remain "deferred to P1a-VAL-02 /
P1a-VAL-03" until those validator tasks land. This task only
**reshapes** the validator entry point and pins the construction-
invariant registry; full enforcement is the validator-axis concern.

#### Instance-side pull-through helpers (spec.md:1349-1395)

```rust
impl RegseqModel {
    pub fn instance_def_path(&self, id: InstanceId) -> &DefPath;
    pub fn instance_span(&self, id: InstanceId) -> Span;
    pub fn instance_linkage(&self, id: InstanceId) -> Linkage;
    pub fn instance_link_section(&self, id: InstanceId) -> Option<StringId>;
    pub fn instance_inline_hint(&self, id: InstanceId) -> InlineHint;
    pub fn instance_is_unsafe(&self, id: InstanceId) -> bool;
    pub fn instance_is_phantom_data(&self, id: InstanceId) -> bool;
    pub fn instance_is_unsafe_cell(&self, id: InstanceId) -> bool;
    pub fn instance_is_manually_drop(&self, id: InstanceId) -> bool;
    pub fn instance_opaque_handle_eligible(&self, id: InstanceId) -> bool;
    pub fn instance_fn_sig(&self, id: InstanceId) -> Option<&FnSig>;
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body>;
    pub fn instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind>;
}
```

`Linkage` / `InlineHint` ship with P1a-DS-07; `IntrinsicKind` ships
with P1a-DS-09. Helper bodies stub via `todo!()` against forward-
declared types until those tasks land; the helper signatures and
unit tests stand here per the §3.5.2 spec ownership.

### Construction-invariant registry (foundation / pinning)

Pin the 20 invariants from spec.md §3.1:1-20 in a registry the
validator tasks read. Format: a public `enum InvariantId`
(numbered 1-20, doc-commented per spec.md:422-605), plus a public
`pub fn invariant_descriptor(id: InvariantId) -> &'static str`
returning the spec wording. This is the binding contract between
DS-01 and the VAL-* tasks; VAL-01/02/03 add per-invariant
enforcement against this registry.

### Foundation alignment for §2 / §4 / §5 / §6 / §7

Read the existing files (`ids.rs`, `idx.rs`, `index_vec.rs`,
`string_interner.rs`, `source.rs`, `crate_info.rs`,
`def_path.rs`) and confirm conformance against the cited spec
sections. Any spec-mandated field, constant, or method missing
from the existing foundation MUST be added in this task. Known
checkpoints:

- **§2.1 `Idx` trait** -- `Copy + Eq + Hash + Ord + Debug +
  'static` with `new(usize) -> Self` and `index(self) -> usize`.
- **§2.3 `newtype_index!` ZERO + `from_u32` const fn**.
- **§2.4 ID inventory** -- 11 IDs (`CrateId`, `StringId`,
  `SourceFileId`, `TyId`, `ConstId`, `ItemId`, `FnSigId`,
  `InstanceId`, `LocalId`, `BlockId`, `VariantIdx`).
- **§2.5 `LocalId::RETURN_PLACE` + `BlockId::START_BLOCK`** const
  values.
- **§4 StringInterner** -- `new`, `intern`, `resolve`, `len`,
  `iter`; sentinel `StringId(0) == ""`; custom serde shape (flat
  JSON array, content uniqueness on deserialise).
- **§5 SourceFile / Position / Span** -- spec.md:1909-1989; confirm
  `Span::slice` / `start` / `end` with the `&IndexVec<SourceFileId,
  SourceFile>` parameter and `SourceFileId::SYNTHESIZED` documentary
  alias.
- **§6 CrateInfo** -- `name: StringId` + `stable_crate_id:
  StableCrateId` (definition lives in `model.rs` per spec.md:1995-1999).
- **§7 DefPath** -- `DefPathData` (19 closed variants per spec.md:2026-2051),
  `DefPathSegment { data, disambiguator }`, `DefPath { crate_id,
  segments, rendered }`, `DefPathHash { stable_crate_id, local_hash }`
  per spec.md:2092-2097. The existing `render_def_path` helper is
  retained.

If any of these is already conformant, the task simply documents
the verification result in a code comment; if any drift is found,
the task brings the foundation into conformance.

### Conventions

- **Serde shape**: `RegseqModel` derives `Serialize` /
  `Deserialize`; per the brief's cross-cutting note, types
  containing `Symbol` / `StringId` defer custom serde until the
  resolution-time pass lands. The `vtables` field carries
  `#[serde(skip)]` per spec.md:328-333. Stub types this task
  introduces use plain derives.
- **Doc comments on every public type / function** per
  `SESSION.md` Coding Conventions.
- **No unicode in comments / docs**.
- **Stub types carry `// FORWARD: P1a-DS-NN` comments** so later
  tasks have an unambiguous insertion point.

## Non-Goals

- **No `Ty` enum body** -- P1a-DS-02 owns the 19-variant Ty.
- **No `Layout` machinery** -- P1a-DS-03 owns Layout / EnumShape /
  TagScalar / `UNIT_LAYOUT`.
- **No `TyInterner` body** -- P1a-DS-04 owns interning + ArgsKey.
- **No `Const` body / ConstInterner** -- P1a-DS-05 owns the 11-variant
  Const + Byte + InstanceRef + ConstInterner.
- **No `FnSig` / `FnSigInterner` / `CAbiLowering` body** -- P1a-DS-06.
- **No `Item` schema** -- P1a-DS-07 owns Item / ItemKind / Linkage /
  InlineHint.
- **No closure / vtable synthesised Items** -- P1a-DS-08a / DS-08b.
- **No `DomainAttrTables` / `IntrinsicKind` body** -- P1a-DS-09.
- **No `Instance` body** beyond the existing skeleton -- P1a-DS-10
  expands InstanceKind to the Option-B Function shape.
- **No validator enforcement of new invariants** -- P1a-VAL-01 / VAL-02 /
  VAL-03 own per-invariant checks; this task pins the registry only.
- **No bridge code** (`crates/regseq-driver/`) -- bridge axis owns its own
  task batch.
- **No pretty-printer changes** -- pretty-printer axis (P1a-PP-*).

## Test Plan

Per `SESSION.md` Testing Requirements, every public type / function gets
at least one test. All tests live in
`crates/regseq-model/src/{module}.rs` `#[cfg(test)] mod tests` blocks
(`unit-in-src` discipline per the brief).

### Foundation conformance tests (one per §)

- `ids.rs::tests`: every newtype ID compiles, exposes `ZERO`, the
  `from_u32` const fn round-trips through `index()`, satisfies `Idx`,
  serialises as a bare `u32` (`#[serde(into / try_from)]` round-trip
  with niche-violating input rejected).
- `string_interner.rs::tests`: `new()` pre-interns `""` at
  `StringId(0)`; `intern("foo")` returns the same id on second call;
  `resolve` round-trips; `iter` yields `(StringId, &str)` pairs;
  custom serde round-trips (the brief defers `Symbol`-bearing serde,
  but `StringInterner` itself is the resolution surface and ships
  serde per §4.2).
- `source.rs::tests`: `SourceFile` + `Position` + `Span` construct;
  `SourceFile::position(byte)` returns 1-based line/col; `line_text`
  returns the slice; `Span::start` / `end` / `slice` against an
  `IndexVec<SourceFileId, SourceFile>` round-trip; sentinel `Span` (file
  = `SourceFileId::SYNTHESIZED`, lo = 0, hi = 0) resolves `Span::slice`
  to `None`.
- `crate_info.rs::tests`: `CrateInfo { name, stable_crate_id }`
  constructs; `StableCrateId` mirrors as `pub struct
  StableCrateId(pub u64)`.
- `def_path.rs::tests`: 19 `DefPathData` variants construct; each
  `DefPathSegment` accepts a `disambiguator`; `DefPathHash` stores
  `(StableCrateId, u64)`; if a `render_def_path` helper exists, its
  verbose-rendering invariant continues to hold (`disambiguator != 0`
  appends `#N`).

### Root-struct shape tests (`model.rs::tests`)

- **Construction**: `RegseqModel::with_primary("foo",
  StableCrateId(1))` returns a model whose `crates[primary_crate]`
  matches; `target` and `build` are populated with caller-supplied
  defaults; the synthesised source-file sentinel is at index 0; all
  empty interners / IndexVecs are present.
- **Field presence**: assert every spec-mandated field exists by
  destructuring (`let RegseqModel { crates, primary_crate, target,
  build, strings, source_files, types, consts, sigs, items,
  item_def_paths, item_synthetic_kinds, sig_c_abi_lowerings,
  instances, vtables, domain_attrs } = ...;`). The destructure must
  bind every field with no leftover fields and no missing fields --
  this catches schema drift at compile time.
- **Endian / ScalarLayouts**: construct a `TargetInfo` with each
  `Endian` variant, verify `pointer_width in {16, 32, 64}` is
  representable, verify `ScalarLayouts` indexable by every
  `IntKind` / `UintKind` / `FloatKind` discriminant via `as usize`.
- **`SizeAlign` constructor + getters**: `SizeAlign { size, align }`
  builds; size & align are `u64`.

### Serde round-trip tests

- A constructed `RegseqModel` round-trips through `serde_json` and
  preserves every public field's content. Stub fields whose body
  types ship later (e.g., `TyInterner` / `ConstInterner`) round-trip
  through their stub serde derives; replacement bodies in DS-04 /
  DS-05 retain the serde contract.
- The `vtables` field is **dropped** on serialise (`#[serde(skip)]`)
  and reconstructed as an empty `HashMap` on deserialise (Phase C
  rebuild lands in P1a-VAL-02; this task only wires the
  serialisation skip).

### `BridgeInput` tests

- `BridgeInput::matching(&model)` returns a value whose
  `spec_crate_stable_id == model.crates[model.primary_crate].stable_crate_id`.
- `RegseqModel::validate(&self, bridge_input: &BridgeInput)` accepts
  a freshly-constructed `with_primary` model paired with
  `BridgeInput::matching(&model)` and returns `Ok(())` (the new
  signature must not regress the legacy stage-1 validator coverage:
  invariants 1, 2, 4 (index-bound half), 5, 8, 10 still pass).

### Pull-through helper tests

For the 13 helpers listed in §3.5.2, write at least one
construction + retrieval test each, against a fixture model that
seeds at least one `Instance` whose underlying `Item` exposes the
relevant field. Where a helper depends on a stub type (e.g.,
`instance_intrinsic` needs `IntrinsicKind` from DS-09), the helper
body is `todo!()`-stubbed and the test is `#[ignore]`-marked with a
`// IGNORE-UNTIL: P1a-DS-NN` comment; the signature still compiles
and the placeholder is in place. Active helpers (anything
resolvable against existing stage-1 surface) ship green tests.

### Construction-invariant registry tests

- `InvariantId` enumerates 20 distinct variants (1..=20).
- `invariant_descriptor(id)` returns a non-empty `&'static str`
  for every variant.
- The registry is closed: a `match` over `InvariantId` is exhaustive
  (no wildcard arm).

### Validator regression tests

The existing stage-1 validator tests at the bottom of
`crates/regseq-model/src/model.rs` continue to pass after the
signature flip from `validate(&self)` to `validate(&self,
bridge_input: &BridgeInput)`. Every existing failure mode
(`SynthesizedSentinelInvalid`, `PrimaryCrateOutOfRange`,
`DanglingId`, `DuplicateStableCrateId`,
`ItemDefPathsLengthMismatch`) is exercised against the new
signature with `BridgeInput::matching(&model)` supplied.

Run `cargo test -p regseq-model` after each module's tests land;
the workspace gate runs at task close.

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-65 against its task spec at
.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/model.rs
- crates/regseq-model/src/ids.rs
- crates/regseq-model/src/idx.rs
- crates/regseq-model/src/index_vec.rs
- crates/regseq-model/src/string_interner.rs
- crates/regseq-model/src/source.rs
- crates/regseq-model/src/crate_info.rs
- crates/regseq-model/src/def_path.rs
- crates/regseq-model/src/lib.rs
- any new sibling modules introduced by this task

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures and the §2 / §4 /
  §5 / §6 / §7 foundation-conformance checklist).
- Construction-invariant registry: 20 entries, closed enum, descriptors.
- Forward-stub discipline: every stub type carries a
  `// FORWARD: P1a-DS-NN` comment naming its expansion task.
- Serde shape: `vtables` carries `#[serde(skip)]`; the rest derives
  serde; round-trip preserves every field per the cross-cutting note.
- `BridgeInput` and the 13 pull-through helpers: signatures match
  spec.md:1349-1395; helpers depending on later-task types are
  `todo!()`-stubbed with `// IGNORE-UNTIL: P1a-DS-NN` test markers.
- Test coverage vs the task's Test Plan (every public function /
  type has at least one test; foundation conformance, root-struct
  shape, serde round-trip, BridgeInput, helpers, registry).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later P1a-DS-* / P1a-VAL-* / P1a-BR-* / P1a-PP-* tasks).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/model.rs` carries the spec.md §3
  `RegseqModel` field set verbatim (including new fields `target`,
  `build`, `item_synthetic_kinds`, `sig_c_abi_lowerings`,
  `vtables` (`#[serde(skip)]`), `domain_attrs`).
- The legacy `bodies: IndexVec<BodyId, Body>` table is removed per
  Option B (spec.md:315-321).
- `TargetInfo`, `Endian`, `ScalarLayouts`, `SizeAlign`,
  `BuildInfo`, `BridgeInput` are public types in the crate; the
  `INT_KIND_COUNT`, `UINT_KIND_COUNT`, `FLOAT_KIND_COUNT` constants
  match spec.md:3700-3702.
- `RegseqModel::validate` takes `&BridgeInput` (D-12-4.3); legacy
  validator coverage continues to pass.
- The 13 Instance-side pull-through helpers (`instance_def_path`
  through `instance_intrinsic`) compile against the existing
  Instance skeleton and are unit-tested (helpers depending on
  later-task types are `todo!()`-stubbed with `IGNORE-UNTIL`
  markers).
- Construction-invariant registry: `InvariantId` enumerates 1..=20
  with `invariant_descriptor` per spec wording.
- Foundation conformance verified for §2 / §4 / §5 / §6 / §7;
  any drift fixed in this task.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
