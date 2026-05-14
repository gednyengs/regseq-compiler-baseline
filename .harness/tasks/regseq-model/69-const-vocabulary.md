# regseq-model-69: `Const` Vocabulary + `Byte` + `InstanceRef` + `ConstInterner` (Data Shape Only)

## Description

Land the const-value vocabulary from spec.md §9.7 (spec.md:2826-3576):
the closed 11-variant `Const` enum (spec.md:2883-2967), the
three-state `Byte` enum (spec.md:2977-2995), the `InstanceRef`
struct (spec.md:3024-3061), the `ConstInterner` BiMap
(spec.md:2837-2853), the `ConstIdentityKey` content key (private,
parallel to `TyIdentityKey`), the per-`Const`-variant byte-encoding
tags (spec.md:3134-3185), the `Byte` sub-encoding (spec.md:3179-3185),
and the `InstanceRef` 34-byte canonical encoding (spec.md:3190-3203).

This task is **data shape only**. The `ConstMemoryStatic` Item
synthesis (spec.md:3328-3521 -- the bridge-side reference-typed
const path that synthesises `ItemKind::Static` Items per unique
content-blob) is bridge-axis territory and lives in P1a-BR-03a per
the brief's post-draft Codex coverage review (Codex finding ERROR-2:
synthesis fires during bridge Phase 2/4/5, not at data-shape time).
This task ships the `Const::Aggregate([Reloc, Uint])` /
`Const::Reloc(InstanceRef)` data shape that the bridge composes
into when consuming `mir::ConstValue::Slice` /
`mir::ConstValue::Indirect` arms; the synthesis machinery itself
is bridge-side.

The `ConstInterner` is content-keyed (spec construction invariant 11);
no two distinct `ConstId`s map to canonically-equal values under the
§9.7 byte-encoding. The validation phase split (type-level / value-
level) is documented here as informational; enforcement lives in
P1a-VAL-02 (type-level) and the bridge / VAL-03 (value-level).

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-05)
- `docs/discussions/regseq-model/spec.md` §9.7 (spec.md:2826-3576);
  §3.1 invariant 11 (content-keyed interners); §9.3 (Instance
  interning -- the rank tables `regseq_instance_kind_rank`)
- `crates/regseq-model/src/ty.rs` (P1a-DS-02 -- `IntKind` /
  `UintKind` / `FloatKind` discriminants for byte-encoding tags)
- `crates/regseq-model/src/ty_interner.rs` (P1a-DS-04 --
  `TyInterner`, `ArgsKey`; `Const` payloads carry `TyId` for
  `NullPtr`)
- `crates/regseq-model/src/args_key.rs` (P1a-DS-04 -- `ArgsKey`,
  the SHA-256 truncation pattern)
- `crates/regseq-model/src/def_path.rs` (`DefPathHash` -- 16 byte
  encoding for `InstanceRef.def_path_hash`)
- `crates/regseq-model/src/model.rs` (P1a-DS-01 -- `RegseqModel`
  hosts the `consts` interner; this task replaces the
  `ConstInterner` stub)
- `crates/regseq-model/src/ids.rs` (`ConstId`, `VariantIdx`,
  `TyId`)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
- `.harness/tasks/regseq-model/68-ty-interner-and-args-key.md`

## Implementation Notes

Create new modules:

- `crates/regseq-model/src/const_value.rs` -- the `Const` enum,
  `Byte` enum.
- `crates/regseq-model/src/instance_ref.rs` -- `InstanceRef` and
  its 34-byte canonical encoding.
- `crates/regseq-model/src/const_interner.rs` -- `ConstInterner`,
  the private `ConstIdentityKey`, custom serde for the interner.

Re-export from `lib.rs`. Replace the `ConstInterner` stub
introduced by P1a-DS-01 in `model.rs`.

### Public API (verbatim from spec)

#### The `Const` enum (spec.md:2886-2967)

```rust
pub enum Const {
    /// Boolean scalar.
    Bool(bool),

    /// Unicode scalar value as 32-bit code point.
    Char(u32),

    /// Signed integer scalar. Smaller widths sign-extend to u128.
    Int(IntKind, u128),

    /// Unsigned integer scalar. Smaller widths zero-extend.
    Uint(UintKind, u128),

    /// IEEE-754 bit-pattern.
    Float(FloatKind, u128),

    /// Zero-sized value. Single canonical representative for all ZSTs.
    ZST,

    /// Byte-level value: a sequence of bytes mixing initialised payload,
    /// uninit bytes, and pointer-bearing relocations.
    Bytes(Vec<Byte>),

    /// Structured aggregate.
    Aggregate(Vec<ConstId>),

    /// Enum value: explicit variant index plus payload children.
    EnumValue {
        variant_idx: VariantIdx,
        payload: Vec<ConstId>,
    },

    /// Typed null pointer.
    NullPtr(TyId),

    /// Whole-pointer relocation.
    Reloc(InstanceRef),
}
```

The 11 variants are exhaustive (spec.md:2970-2974). No `Other` /
`Opaque` escape variant; non-conforming inputs are bridge-rejected.

#### `Byte` enum (spec.md:2978-2995)

```rust
pub enum Byte {
    Uninit,
    Value(u8),
    Reloc(InstanceRef),
}
```

#### `InstanceRef` (spec.md:3024-3061)

```rust
pub struct InstanceRef {
    pub regseq_kind_rank: u8,
    pub def_path_hash:    DefPathHash,
    pub args_key:         ArgsKey,
    pub fn_shim_disambig: u8,
}
```

#### `ConstInterner` (spec.md:2837-2853)

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

`ConstId` is `u32`-backed via `NonMaxU32` (spec.md:2855-2856);
`size_of::<Option<ConstId>>() == 4`. (The `ids.rs` newtype already
satisfies this from the foundation; verify in P1a-DS-01's
foundation pass and assert here.)

The `ConstIdentityKey` is a **private** content key paralleling
`TyIdentityKey`. It encodes the canonical byte form of a `Const`
(per the per-variant tags below) and serves as the `lookup`'s
`HashMap` key. It is `pub(crate)` at most; not re-exported.

### Per-`Const`-variant byte-encoding tags (spec.md:3134-3155)

```text
const_variant_tag = u8
  0  = Bool                followed by u8 (0x00 | 0x01)
  1  = Char                followed by u32 LE Unicode scalar value
  2  = Int(IntKind, u128)  followed by u8 IntKind discriminant + u128 LE value
                           (small widths sign-extended into u128)
  3  = Uint(UintKind, u128) followed by u8 UintKind discriminant + u128 LE value
                           (small widths zero-extended)
  4  = Float(FloatKind, u128) followed by u8 FloatKind discriminant + u128 LE
                           bit-pattern
  5  = ZST                 (no payload)
  6  = Bytes(Vec<Byte>)    followed by u32 LE byte_count + byte_count * Byte
                           encoding
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
`#[repr(u8)]` numeric assignments pinned in §10 (the single source
of truth -- shipped in P1a-DS-02).

### `Byte` sub-encoding (spec.md:3179-3185)

```text
byte_sub_tag = u8
  0  = Uninit             (no payload)
  1  = Value(u8)          followed by 1 byte (the initialised payload byte)
  2  = Reloc(InstanceRef) followed by InstanceRef bytes (34 B; see below)
```

### `InstanceRef` byte encoding (spec.md:3190-3203)

```text
InstanceRef encoding (34 bytes total):
  u8       regseq_kind_rank
  16 bytes def_path_hash       (StableCrateId u64 LE + local_hash u64 LE
                                per §12.3 termination rules)
  16 bytes args_key            (emitted as `ArgsKey.0.to_be_bytes()`)
  u8       fn_shim_disambig
```

`args_key` is **big-endian** per spec.md:3205-3219 (load-bearing:
the `ArgsKey.0` field is the BE-loaded leading SHA-256 bytes;
`to_be_bytes()` reproduces the original digest bytes). All other
multi-byte integers in this encoding are LE.

### `canonical_const(ConstId)` recursion (spec.md:3221-3231)

When `Aggregate.0` and `EnumValue.payload` reference nested
`ConstId`s, the canonicaliser:

1. Resolves the `ConstId` against `model.consts.resolve(...)`.
2. Recursively encodes the resolved `Const` per the per-variant
   rules.
3. Inlines the resulting bytes into the parent's stream.

Local `ConstId` integers are **never** emitted (build-local
indices defeat cross-build stability). The recursion is bounded by
const-graph acyclicity (well-typed Rust + const-eval termination
guarantee no transitive self-containment); P1a-VAL-03 enforces
acyclicity on tampered serialised models via three-state DFS.

### Worked examples (spec.md:3245-3283)

Document the 8 worked examples (Bool, Int, Float, ZST, Aggregate,
EnumValue, NullPtr, Reloc) as test fixtures. See Test Plan below.

### Type-level vs value-level partition (spec.md:3082-3116)

Document on the `Const` enum:

- **Type-level admissible** (`GenericArg::Const(_)` positions):
  `Bool`, `Char`, `Int(_, _)`, `Uint(_, _)`, `Float(_, _)`.
- **Value-level admissible** (`InstanceKind`, `Bytes::Reloc`,
  `Aggregate` / `EnumValue` payload children): all 11 variants.

Provide a public predicate:

```rust
impl Const {
    pub fn is_type_level_admissible(&self) -> bool;
}
```

This returns `true` for the 5 scalar variants and `false` for the
6 value-level-only variants. Used by P1a-VAL-02 step 4 (type-level
const phase validation).

### Conventions

- **`Const` derives**: `Clone, Debug, PartialEq, Eq, Hash,
  Serialize, Deserialize`. The `Eq`/`Hash` is full-value equality;
  the `ConstIdentityKey` (private) is the canonical-content key
  used by the interner's `lookup` HashMap.
- **`Byte` derives**: `Clone, Copy (where possible), Debug,
  PartialEq, Eq, Hash, Serialize, Deserialize`. (`Copy` requires
  no heap payload; `Byte::Reloc(InstanceRef)` carries a struct,
  but `InstanceRef` is `Copy` because it is a 34-byte plain-old-data
  value -- the `Vec<Byte>` parent owns the heap; individual `Byte`
  values are `Copy`.)
- **`InstanceRef` derives**: `Clone, Copy, Debug, PartialEq, Eq,
  Hash, Serialize, Deserialize, PartialOrd, Ord` (Ord needed for
  deterministic sort orders downstream).
- **`ConstInterner` derives**: same as `TyInterner` (P1a-DS-04) --
  custom serde, on-wire shape is `storage` only.
- **`ConstIdentityKey` is crate-private**: `pub(crate)` at most.
- **Doc comments on every public type / variant / function**.
- **No unicode in comments / docs**.
- **Closed-enum discipline**: `Const`, `Byte` are closed; no
  `#[non_exhaustive]`, no wildcard arms.

### Forward-stub for type-level / value-level partition

This task ships the `Const` data shape, the `ConstInterner`, and the
canonical byte form. The bridge-side dispatch table (spec.md:3291-3318
-- the rustc `mir::ConstValue` arm -> RegseqModel `Const` variant
mapping) is **bridge-axis territory** (P1a-BR-03a). The
type-erase-to-bytes invariants for `ConstMemoryStatic` (spec.md:3325-
3454) live in P1a-BR-03a / P1a-DS-08b synthesis machinery; this task
ships only the `Const::Aggregate([Reloc, Uint])` and
`Const::Reloc(InstanceRef)` data shape that the bridge composes into.

## Non-Goals

- **No `ConstMemoryStatic` Item synthesis** -- P1a-BR-03a owns it
  per Codex finding ERROR-2.
- **No bridge construction** (`mir::ConstValue` arm dispatch
  table) -- P1a-BR-03a.
- **No `SyntheticItemKind::ConstMemoryStatic`** -- P1a-DS-08b owns
  the variant.
- **No `ContentHash`** -- P1a-DS-08b owns the sibling hash family
  (parallel to `ArgsKey`).
- **No const-graph acyclicity validator** -- P1a-VAL-02 / VAL-03.
- **No type-level / value-level partition validator** --
  P1a-VAL-02 step 4 enforces type-level subset; P1a-VAL-03 enforces
  value-level. This task ships the `is_type_level_admissible()`
  predicate as the validator-input.
- **No `InstanceKind` body shape** -- P1a-DS-10 owns the
  `Static.init: Option<ConstId>` / `Const.value: ConstId` slots.
- **No bridge interning invariant for fat-pointer auxiliary types**
  (spec.md:3422-3439) -- P1a-BR-03a owns the bridge contract; the
  data-shape side admits the `Aggregate([Reloc, Uint])` shape.

## Test Plan

`unit-in-src` discipline. Tests live in
`crates/regseq-model/src/const_value.rs::tests`,
`crates/regseq-model/src/instance_ref.rs::tests`,
`crates/regseq-model/src/const_interner.rs::tests`. Per `SESSION.md`
Testing Requirements, every public type / function gets at least one
test.

### `Const` constructor tests (one per variant)

11 tests, one per `Const` variant:

- `Const::Bool(true)` and `Const::Bool(false)` construct.
- `Const::Char(0x61)` constructs (`'a'`); a Unicode-out-of-range u32
  is admissible at the data-shape level (rejection at the bridge).
- `Const::Int(IntKind::I32, 42_u128)` constructs; sign-extension
  example: `Const::Int(IntKind::I8, u128::MAX)` for `-1_i8`.
- `Const::Uint(UintKind::U64, 0xDEADBEEF_u128)` constructs.
- `Const::Float(FloatKind::F32, 0x3f800000_u128)` constructs (1.0f32
  bit-pattern).
- `Const::ZST` constructs.
- `Const::Bytes(vec![Byte::Value(0x12), Byte::Uninit, Byte::Value(0x34)])`
  constructs.
- `Const::Aggregate(vec![child1, child2])` constructs.
- `Const::EnumValue { variant_idx: VariantIdx::from_u32(0), payload:
  vec![] }` constructs (unit variant); same with non-empty payload.
- `Const::NullPtr(some_ty_id)` constructs.
- `Const::Reloc(some_instance_ref)` constructs.

### `Byte` constructor tests

- `Byte::Uninit` constructs.
- `Byte::Value(0)`, `Byte::Value(255)` construct.
- `Byte::Reloc(some_instance_ref)` constructs.
- `Byte` is `Copy` (assert via `let b: Byte = ...; let _c = b; let
  _d = b;`).

### `InstanceRef` constructor + invariants

- Construct an `InstanceRef` per each `regseq_kind_rank` value
  (`0..=5` for `Function`, `Static`, `Const`, `Struct`, `Enum`,
  `Union`).
- `InstanceRef` is `Copy`.
- Equality and hashing: two `InstanceRef`s with identical fields are
  equal and hash equal.
- `InstanceRef` round-trips through serde.
- Ordering: `InstanceRef`s sort lexicographically by
  `(regseq_kind_rank, def_path_hash, args_key, fn_shim_disambig)`
  (verify with a hand-constructed sort).

### `ConstInterner` smoke tests

- `ConstInterner::new()` returns an empty interner.
- `intern(Const::Bool(true))` returns `ConstId(0)`; second call
  returns the same `ConstId` (content-keyed dedup).
- `intern(Const::Bool(false))` returns a distinct `ConstId(1)`.
- `resolve(ConstId)` returns the matching `&Const`.
- `find(&key)` returns `Some(ConstId)` after interning; `None` if
  the key is not present.
- `iter()` yields `(ConstId, &Const)` pairs in insertion order.
- `len()` reflects the number of interned values.

### Per-variant identity dedup

For each of the 11 `Const` variants, intern two structurally-equal
instances and assert they collapse to the same `ConstId`:

- Two `Const::Bool(true)` -> one `ConstId`.
- Two `Const::Int(IntKind::I32, 42)` -> one `ConstId`.
- `Const::Int(IntKind::I32, 42)` and `Const::Int(IntKind::I64, 42)`
  -> distinct `ConstId`s (the IntKind discriminant is part of
  identity).
- Two `Const::Aggregate(vec![c1, c2])` -> one `ConstId`.
- Two `Const::EnumValue { variant_idx: V0, payload: vec![c1] }` ->
  one `ConstId`.
- Two `Const::NullPtr(t1)` -> one `ConstId`; `Const::NullPtr(t1)`
  and `Const::NullPtr(t2)` (with t1 != t2) -> distinct.
- Two `Const::Reloc(ref1)` (identical InstanceRef) -> one `ConstId`.
- Two `Const::Bytes(vec![Byte::Value(1), Byte::Value(2)])` ->
  one `ConstId`.

### `is_type_level_admissible` predicate

Per spec.md:3082-3116 type-level / value-level partition table:

- `Const::Bool(_)`, `Const::Char(_)`, `Const::Int(_, _)`,
  `Const::Uint(_, _)`, `Const::Float(_, _)` return `true`.
- `Const::ZST`, `Const::Bytes(_)`, `Const::Aggregate(_)`,
  `Const::EnumValue { .. }`, `Const::NullPtr(_)`, `Const::Reloc(_)`
  return `false`.

### Per-variant canonical byte-encoding tests (the worked examples)

Match each spec.md:3245-3283 worked example exactly:

- `canonical_const(Const::Bool(true))` -> `[0x00, 0x01]`.
- `canonical_const(Const::Bool(false))` -> `[0x00, 0x00]`.
- `canonical_const(Const::Int(IntKind::I32, (-7_i32 as i128) as u128))`
  -> `[0x02, 0x02] ++ (-7_i64).to_le_bytes()` (sign-extended into u128).
- `canonical_const(Const::Float(FloatKind::F32, 0x3f800000_u128))`
  -> `[0x04, 0x01] ++ 0x3f800000_u128.to_le_bytes()` (low 4 bytes
  carry IEEE bits, high 12 zero).
- `canonical_const(Const::ZST)` -> `[0x05]`.
- `canonical_const(Const::Bytes(vec![Byte::Value(0xAB)]))` ->
  `[0x06, 0x01, 0x00, 0x00, 0x00, 0x01, 0xAB]` (tag=6, byte_count=1
  LE, sub_tag=1 Value, payload byte).
- `canonical_const(Const::Bytes(vec![Byte::Uninit]))` ->
  `[0x06, 0x01, 0x00, 0x00, 0x00, 0x00]` (sub_tag=0 Uninit, no
  payload).
- `canonical_const(Const::Aggregate(vec![c1, c2]))` ->
  `[0x07, 0x02, 0x00, 0x00, 0x00] ++ <canonical(c1)> ++
  <canonical(c2)>`.
- `canonical_const(Const::EnumValue { variant_idx: V1, payload:
  vec![c1] })` -> `[0x08, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
  0x00] ++ <canonical(c1)>`.
- `canonical_const(Const::NullPtr(t_i32))` -> `[0x09] ++
  <canonical_ty(t_i32)>` (the canonical_ty recursion lands in
  P1a-DS-04).
- `canonical_const(Const::Reloc(ref))` -> `[0x0A] ++ <34-byte
  InstanceRef encoding>`.

### `InstanceRef` 34-byte encoding tests

- `InstanceRef { regseq_kind_rank: 0, def_path_hash:
  DefPathHash { stable_crate_id: StableCrateId(0xAABB),
  local_hash: 0xCCDD }, args_key: ArgsKey(0xEEFF), fn_shim_disambig: 5 }`
  encodes to:
  - 1 byte: 0x00.
  - 16 bytes: 8 bytes LE 0xAABB ++ 8 bytes LE 0xCCDD.
  - 16 bytes: ArgsKey(0xEEFF).0.to_be_bytes() (big-endian!).
  - 1 byte: 0x05.
- Total: 34 bytes (assert `encoding.len() == 34`).
- `args_key` field's BE encoding is verified explicitly: construct
  an `ArgsKey(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210_u128)` and
  assert the encoded slice matches `[0x01, 0x23, 0x45, 0x67, 0x89,
  0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10]`.

### `Byte` sub-encoding tests

- `byte_encoding(Byte::Uninit)` -> `[0x00]`.
- `byte_encoding(Byte::Value(0xAB))` -> `[0x01, 0xAB]`.
- `byte_encoding(Byte::Reloc(ref))` -> `[0x02] ++ <34-byte
  InstanceRef encoding>`.
- `Byte::Reloc` total length is 35 bytes.

### `canonical_const` recursion test

- Build a fixture `ConstInterner` with `c1 = Const::Bool(true)`,
  `c2 = Const::Int(IntKind::I32, 42)`, `c3 =
  Const::Aggregate(vec![c1, c2])`. Compute
  `canonical_const(c3)`. Verify the bytes inline `c1`'s and `c2`'s
  encodings (no `c1` / `c2` integer indices appear in the output).
- Self-referential reservation: hand-build a tampered model where a
  `Const::Aggregate(vec![own_id])` references itself; assert the
  canonical computation reaches a documented bound (e.g.,
  recursion depth limit) or panics. (The proper cycle rejection
  lives at P1a-VAL-03 step 9 const-graph acyclicity; this task's
  `canonical_const` is allowed to assume acyclicity per spec.md:
  3232-3241.)

### Custom serde for `ConstInterner`

- Round-trip a populated `ConstInterner` (5 distinct values) through
  `serde_json`. Assert `len()` preserved, `resolve(id)` matches,
  `iter()` yields original order.
- Inject duplicate canonical content into the serialised form
  (hand-build a JSON array with two identical `Const::Bool(true)`
  entries) and assert deserialise rejects with a content-uniqueness
  error.
- Inject a dangling `ConstId` reference (a `Const::Aggregate(vec![
  ConstId(99)])` in a 3-element storage) and assert deserialise
  rejects with an out-of-bounds error.

### Closed-enum exhaustiveness scaffolds

`Const` and `Byte` each get a hidden `_exhaustiveness_check`
function with an exhaustive `match` (no wildcard arm).

### `ConstId` `NonMaxU32` invariant test

Verify `size_of::<Option<ConstId>>() == 4` via an `assert_eq!` in a
const context (or `static_assertions::assert_eq_size!`).

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-69 against its task spec at
.harness/tasks/regseq-model/69-const-vocabulary.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/const_value.rs
- crates/regseq-model/src/instance_ref.rs
- crates/regseq-model/src/const_interner.rs
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/model.rs (consts field replacement)

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures: 11-variant `Const`,
  3-variant `Byte`, `InstanceRef`, `ConstInterner` with 6 spec
  methods).
- Per-`Const`-variant byte-encoding tags 0..=10 match
  spec.md:3134-3155 verbatim.
- `Byte` sub-encoding tags 0..=2 match spec.md:3179-3185.
- `InstanceRef` 34-byte encoding matches spec.md:3190-3203 verbatim,
  including `args_key` BIG-ENDIAN exception (`to_be_bytes`),
  not LE.
- `is_type_level_admissible` predicate matches the spec.md:3086-3098
  type-level / value-level partition table.
- `ConstIdentityKey` is crate-private; `Const` and `Byte` are
  closed enums; no Other / Opaque escape variants.
- Custom serde rejects content-duplicate and dangling-ConstId
  payloads at deserialise.
- `ConstId` `NonMaxU32` invariant verified.
- Test coverage vs the task's Test Plan (per-variant constructors,
  identity dedup, type-level predicate, every worked-example byte
  encoding, InstanceRef 34-byte encoding with BE args_key,
  Byte sub-encoding, recursion test, custom serde failure modes,
  exhaustiveness scaffolds).
Do NOT flag anything explicitly marked out of scope (Non-Goals:
P1a-BR-03a owns ConstMemoryStatic synthesis; P1a-DS-08b owns
SyntheticItemKind and ContentHash; P1a-VAL-02 / VAL-03 own const
validators; P1a-DS-10 owns InstanceKind body).
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

- The 11-variant closed `Const` enum lands per spec.md:2886-2967.
- The 3-variant closed `Byte` enum lands per spec.md:2978-2995.
- `InstanceRef` lands per spec.md:3024-3061 with `Copy`,
  `Eq`, `Hash`, `Ord`, serde derives.
- `ConstInterner` lands with all 6 spec methods (`new`, `intern`,
  `resolve`, `find`, `len`, `iter`); private `ConstIdentityKey`.
- Per-`Const`-variant byte-encoding tags 0..=10 match
  spec.md:3134-3155 verbatim.
- `Byte` sub-encoding tags 0..=2 match spec.md:3179-3185.
- `InstanceRef` 34-byte canonical encoding matches spec.md:3190-3203
  including the BIG-ENDIAN `args_key` exception.
- `is_type_level_admissible(&self) -> bool` predicate matches the
  spec.md:3086-3098 table.
- Custom serde for `ConstInterner` rejects duplicates and dangling
  references at deserialise.
- `ConstId` is `NonMaxU32`-backed (`size_of::<Option<ConstId>>() == 4`).
- Closed-enum exhaustiveness scaffolds present for `Const` and `Byte`.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
