# regseq-model-68: `TyInterner` + Identity Rules + `ArgsKey` Deep Canonicaliser

## Description

Land the type-interning machinery from spec.md §12 (spec.md:4116-4499):
the public `TyInterner` BiMap (`new` / `intern` / `resolve` / `find`
/ `find_nominal` / `len` / `iter`), the **private**
`TyIdentityKey` interner key encoding the §12.1 nominal /
structural identity rules, the `NominalKind` enum (5 variants:
`Struct`, `Enum`, `Union`, `FnDef`, `Closure`), the public
`ArgsKey` newtype (`pub struct ArgsKey(pub u128)`) plus
`ArgsKey::EMPTY`, the `GenericArg` / `GenericArgs` shape from §9.1
(if not already landed), and the deep canonical byte-encoding for
`Ty` and `GenericArgs` per §12.3 (spec.md:4288-4399).

The interner is content-keyed (spec construction invariant 11): no
two distinct `TyId`s map to canonically-equal values under the
§12.1 buckets. The `ArgsKey` deep canonicaliser is the
SHA-256-truncated content fingerprint used by F4-2 instance
ordering (§16.3), synthetic-item identity (§13.3), and
cross-build determinism (§12.4). This task delivers both the
local interner identity (private `TyIdentityKey`) and the
cross-build deep identity (`ArgsKey`).

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-04)
- `docs/discussions/regseq-model/spec.md` §12 (spec.md:4116-4499);
  §9.1 (`GenericArg` / `GenericArgs` shape, spec.md:2402-2415);
  §12.4 (cross-build property, spec.md:4424-4445); §12.5
  (serialisation, spec.md:4447-4499)
- `crates/regseq-model/src/ty.rs` (P1a-DS-02 -- `Ty` enum and
  auxiliary closed enums)
- `crates/regseq-model/src/layout.rs` (P1a-DS-03 -- `Layout` is
  excluded from nominal / tuple identity per §12.1)
- `crates/regseq-model/src/model.rs` (P1a-DS-01 -- `RegseqModel`
  hosts the interner; this task replaces the placeholder
  `TyInterner` stub introduced by DS-01)
- `crates/regseq-model/src/def_path.rs` (`DefPathHash` -- terminal
  in the canonical byte stream)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
- `.harness/tasks/regseq-model/66-ty-enum-and-helpers.md`
- `.harness/tasks/regseq-model/67-layout.md`

## Implementation Notes

Create new modules:

- `crates/regseq-model/src/ty_interner.rs` -- the `TyInterner`
  BiMap, the private `TyIdentityKey`, the `NominalKind` enum,
  custom serde for the interner.
- `crates/regseq-model/src/args_key.rs` -- `ArgsKey`,
  `ArgsKey::EMPTY`, `GenericArg`, `GenericArgs`, the deep
  canonical byte encoding, the SHA-256 truncation hash.
- `crates/regseq-model/src/canonical.rs` (optional) -- the
  shared canonicaliser walk if the canonical-byte logic grows
  large enough to warrant a sibling module.

Re-export from `lib.rs`. Replace the `TyInterner` stub in
`model.rs` (introduced by DS-01) with a re-export from
`ty_interner.rs`.

### Public API (verbatim from spec)

#### `TyInterner` (spec.md:4155-4184)

```rust
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup:  HashMap<TyIdentityKey, TyId>,    // private key (not part of public API)
}

// Private to the regseq-model crate.
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

#### `GenericArg` / `GenericArgs` (spec.md:2404-2412)

```rust
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
    // Lifetimes erased post-monomorphisation; not represented.
}

pub type GenericArgs = Vec<GenericArg>;
```

(If P1a-DS-01 / DS-02 already declared a forward stub, this task
finalises the body. ConstId comes from existing `ids.rs`.)

#### `ArgsKey` (spec.md:4244-4247, 4374-4398)

```rust
pub struct ArgsKey(pub u128);    // 128-bit content hash

impl ArgsKey {
    /// Canonical `ArgsKey` for an empty `GenericArgs` value.
    /// Computed once: leading 16 bytes of SHA-256(0x00 0x00 0x00 0x00)
    /// loaded as a big-endian u128.
    pub const EMPTY: ArgsKey = ArgsKey(<canonical SHA-256-truncated value>);
}
```

The `EMPTY` constant value MUST be computed at build time from the
canonical empty-`GenericArgs` byte form (`u32 LE 0x00_00_00_00`,
followed by zero entries). Because `ArgsKey::EMPTY` is `const`, the
SHA-256 must be either pre-computed and committed into the source
(with a `// Computed: SHA-256(\x00\x00\x00\x00)[..16] big-endian`
comment), or computed in a `const fn` if the `sha2` crate exposes
const operations. Default approach: pre-compute the value, commit
it, and add a `#[test]` that re-computes the SHA-256 at runtime and
asserts equality with `ArgsKey::EMPTY.0`. This makes a future
implementation drift detectable.

### §12.1 identity rules (spec.md:4200-4218)

The private `TyIdentityKey` encodes per-variant identity per the
buckets:

| Bucket                                      | Variants                                                             | `TyIdentityKey` shape                 | Layout role                                                          |
| ------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------- |
| **Nominal**                                 | `Struct`, `Enum`, `Union`, `FnDef`, `Closure`                        | `(variant_kind_tag, item, args)`      | inline `Layout` excluded (functionally determined by `(item, args)`) |
| **Tuple** (structural-with-layout-excluded) | `Tuple`                                                              | `(variant_kind_tag, fields)`          | inline `Layout` excluded (functionally determined by `fields`)       |
| **Other structural**                        | Bool/Char/Int/Uint/Float/Str/Array/Slice/Ptr/Ref/FnPtr/Dynamic/Never | full structural recursion over fields | no inline layout                                                     |

The `variant_kind_tag` is load-bearing: it distinguishes
`Ty::Closure(item, args)` and `Ty::Struct { item, args, layout }`
sharing one Item (closure's identity-view + layout-bearing-view).

### §12.2 self-referential ADT reservation

Spec.md:4220-4234 commits the TyId-reservation pattern: parent's
`TyId` is reserved before its layout is computed; recursive
references hit the reservation via the nominal hash. The
implementation provides `intern` semantics that admit this
pattern. Concretely:

1. `intern(ty)` first computes the `TyIdentityKey` from `ty`'s
   per-variant rule (layout excluded for nominal / tuple).
2. If the key is already present in `lookup`, return the existing
   `TyId`.
3. Otherwise, push `ty` into `storage`, get the new `TyId`, insert
   `(key -> TyId)` into `lookup`, return the new `TyId`.

The bridge handles the reservation pattern by interning a "stub"
nominal `Ty` first (e.g., `Ty::Struct { item, args, layout:
PENDING }`), then patching the stored `Ty` once layout is computed.
Patching is via direct mutation of `storage[id]`; this task
provides a `pub fn patch_layout(&mut self, id: TyId, layout:
Layout)` method (or equivalent) that updates the stored `Ty`'s
inline `Layout` without re-interning. Document the sound usage:
the patched `Ty`'s `TyIdentityKey` MUST be unchanged after the
patch (i.e., layout is the only field allowed to change).

### §12.3 deep canonical byte-encoding

The `ArgsKey` canonicaliser walks `Ty` content per the table at
spec.md:4297-4307. Each node emits a 1-byte variant tag followed by
tag-specific payload bytes; all multi-byte integers are little-
endian; lengths emit as little-endian `u32` prefixes.

Per-`Ty`-variant byte-encoding tags (spec.md:4313-4334):

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
  9  = Ref                 followed by canonical_args(pointee) + u8 Provenance discriminant
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

`GenericArgs` container encoding (spec.md:4347-4364):

```text
GenericArgs encoding:
  u32 LE arg_count
  followed by arg_count * GenericArg entry

GenericArg entry:
  u8 disc     // 0 = Type(TyId), 1 = Const(ConstId)
  followed by disc-specific payload:
    disc = 0: canonical Ty bytes per the per-`Ty` rules above
    disc = 1: canonical Const bytes per the per-`Const`-variant
              byte-encoding rules in §9.7
```

For tag `12 = FnPtr`, the canonical FnSig content lives in §14.3
(P1a-DS-06). For this task, the FnSig canonical encoding is a
`todo!()` stub keyed on a `// FORWARD: P1a-DS-06` comment; tests
that exercise `FnPtr` are `#[ignore]`-marked with
`// IGNORE-UNTIL: P1a-DS-06`.

For `disc = 1` (Const), the canonical Const encoding lives in §9.7
(P1a-DS-05). Same `todo!()` + `IGNORE-UNTIL` pattern.

`DefPathHash` bytes per spec.md:4307: terminal, emit `(StableCrateId
u64 LE + local_hash u64 LE)` -- 16 bytes total.

`ItemId` field per spec.md:4303: replace with `DefPathHash` from
`model.item_def_paths` (cross-build terminator). The
canonicaliser therefore takes a `&RegseqModel` argument (or at
minimum a `&IndexVec<ItemId, DefPathHash>`).

`StringId` per spec.md:4305: resolve to content (UTF-8 bytes,
length-prefixed). `StringId` does not appear inside `Ty` directly
in the variants this task ships, but the encoding spec lists the
rule for completeness; document it.

### Hash function

SHA-256 truncated to leading 16 bytes (big-endian) loaded as
`u128` big-endian (spec.md:4252-4260). Use the `sha2` crate;
add to `regseq-model/Cargo.toml` as a dependency if not already
present.

The `ArgsKey` constructor:

```rust
fn args_key_from_canonical_bytes(bytes: &[u8]) -> ArgsKey {
    use sha2::{Sha256, Digest};
    let digest = Sha256::digest(bytes);
    let mut head = [0u8; 16];
    head.copy_from_slice(&digest[..16]);
    ArgsKey(u128::from_be_bytes(head))
}
```

### §12.4 cross-build property

Spec.md:4424-4445 commits the cross-build invariants. The bridge
side enforces these through the build-equivalence quad; this task's
contribution is correctness of the canonicaliser (deterministic on
canonically-equal `Ty` trees, regardless of local TyId / FnSigId /
ConstId / StringId integers, because the canonicaliser resolves
nested IDs to content before encoding).

### §12.5 serialisation

Spec.md:4447-4499 commits the on-wire shape: `storage` only
(flat array of `Ty`); `lookup` reconstructed at deserialise by
direct insertion preserving original `TyId` values. Implement
custom `Serialize` / `Deserialize` (do NOT field-derive: the
private `TyIdentityKey` is not part of the wire form).

Deserialise validation order per spec.md:4456-4498 (load full
`storage`, then validate references, then validate content
uniqueness under the §12.1 rule, then validate layout sanity).
This task ships steps 1, 2 (bound-check `TyId`s referenced inside
loaded `Ty`s), 3 (content-uniqueness via `TyIdentityKey`); steps 4
(layout sanity) and 5 (rebuild `lookup` -- already covered by step
3 if implemented as deserialise-time interning) are validator-axis
(P1a-VAL-02 / VAL-03) for the bridge-side construction; the
deserialise rebuild is local to this interner and lands here.

### Conventions

- **`TyIdentityKey` is crate-private**: `pub(crate)` at most. Not
  re-exported.
- **`NominalKind` is public**: needed by `find_nominal` callers
  (P1a-DS-08a's closure identity-view lookup, P1a-DS-08b's vtable
  type-Item lookup).
- **`ArgsKey` derives** `Copy, Clone, Debug, PartialEq, Eq, Hash,
  Serialize, Deserialize` plus `PartialOrd, Ord` so it can sort in
  the F4-2 comparator output check (P1a-VAL-02).
- **`GenericArg` derives** `Clone, Debug, PartialEq, Eq, Hash,
  Serialize, Deserialize`.
- **Doc comments** on every public type / variant / function.
- **No unicode in comments / docs**.
- **Closed-enum discipline** on `NominalKind`, `GenericArg`.

## Non-Goals

- **No `FnSigInterner`** -- P1a-DS-06 ships it. The `FnPtr` tag-12
  canonical encoding here is a `todo!()` stub.
- **No `ConstInterner`** -- P1a-DS-05 ships it. The `disc = 1`
  GenericArg::Const canonical encoding here is a `todo!()` stub.
- **No `Item` schema** -- P1a-DS-07 ships it; `find_nominal` only
  requires `ItemId` (a `u32` index) and `args`, not the Item body.
- **No `SyntheticItemKind` / `PrincipalTraitKey`** -- P1a-DS-08b.
- **No bridge construction** -- P1a-BR-02 owns the bridge-side
  TyKind translation pipeline + the rustc-dep `Ty<'tcx>`
  translation cache.
- **No validator** -- P1a-VAL-02 enforces interner uniqueness on
  deserialised models; this task ships the local `intern`-time
  uniqueness only.
- **No `ProjectionBindingsKey` / `ContentHash`** -- P1a-DS-08b owns
  these (sibling hash families to `ArgsKey`).
- **No `EMPTY` constant on `ConstIdentityKey`** -- P1a-DS-05 owns
  the const-side canonical encoding.

## Test Plan

`unit-in-src` discipline. Tests live in
`crates/regseq-model/src/ty_interner.rs::tests` and
`crates/regseq-model/src/args_key.rs::tests`. Per `SESSION.md`
Testing Requirements, every public type / function / constant gets
at least one test.

### `TyInterner` constructor + smoke tests

- `TyInterner::new()` returns an empty interner (`len() == 0`,
  `iter()` empty).
- `intern(Ty::Bool)` returns a `TyId(0)`; second call returns the
  same `TyId(0)` (content-keyed dedup).
- `intern(Ty::Bool)` then `intern(Ty::Char)` produces distinct
  `TyId`s with `len() == 2`.
- `resolve(TyId)` returns the matching `&Ty` for each interned
  value.
- `find(&Ty::Bool)` returns `Some(TyId(0))` after interning;
  `find(&Ty::Char)` returns `None` if not yet interned.
- `iter()` yields `(TyId, &Ty)` pairs in insertion order.

### Per-variant identity rules

For each of the 19 `Ty` variants, intern two structurally-equal
instances and assert they collapse to the same `TyId`:

- **Nominal bucket** (`Struct`, `Enum`, `Union`, `FnDef`, `Closure`):
  two `Ty::Struct { item: I0, args: vec![], layout: L0 }` and
  `Ty::Struct { item: I0, args: vec![], layout: L1 }` (different
  layouts, same `(item, args)`) collapse to one `TyId`. Patch the
  layout afterwards; assert the stored `Ty` has the patched
  layout. Repeat for `Enum`, `Union`, `FnDef`, `Closure`.
- **Tuple bucket**: two `Ty::Tuple { fields: vec![ty1, ty2],
  layout: L0 }` and `Ty::Tuple { fields: vec![ty1, ty2], layout:
  L1 }` collapse to one `TyId`.
- **Other structural bucket**: every variant in the bucket
  (`Bool`, `Char`, `Int(_)`, `Uint(_)`, `Float(_)`, `Str`,
  `Array(_, _)`, `Slice(_)`, `Ref(_, _)`, `Ptr(_, _)`,
  `FnPtr(_)`, `Dynamic { .. }`, `Never`) deduplicates by full
  structural equality.
- **`variant_kind_tag` discrimination**: `Ty::Closure(item, args)`
  and `Ty::Struct { item, args, layout }` for the same `(item,
  args)` get **distinct** `TyId`s -- the `variant_kind_tag` is
  load-bearing per spec.md:4214-4218.

### `find_nominal` tests

- `find_nominal(NominalKind::Struct, item, &args)` returns the
  `TyId` of the previously-interned `Ty::Struct { item, args, .. }`
  regardless of the layout used at intern time.
- Same for the other 4 NominalKind variants (`Enum`, `Union`,
  `FnDef`, `Closure`).
- `find_nominal` for a non-existent `(kind, item, args)` returns
  `None`.
- Different `args` against the same `(kind, item)` produce distinct
  `TyId` results from `find_nominal`.

### Layout patch test

- Intern a `Ty::Struct` with placeholder `Layout`. Verify
  `resolve(id)` returns the placeholder. Call
  `patch_layout(id, real_layout)`. Verify `resolve(id)` returns
  a `Ty::Struct` whose layout is `real_layout`.
- Verify `find_nominal(Struct, item, &args)` returns the same
  `TyId` after the patch (identity unchanged).

### Self-referential ADT pattern test

Walk the spec.md:4225-4234 example:

1. Reserve `Ty::Struct { item: NodeItem, args: vec![], layout:
   PENDING }` at `TyId(K)` via `intern`.
2. Construct `Ty::Ptr(TyId(K), Mutability::Mut)` and intern at
   `TyId(K+1)`.
3. Patch `storage[K].layout` with a layout whose
   `FieldLayout.ty == TyId(K+1)`.
4. Verify `resolve(TyId(K))` shows the patched layout.
5. Verify `resolve(TyId(K+1))` shows `Ty::Ptr(TyId(K),
   Mutability::Mut)`.
6. Verify `intern(Ty::Struct { item: NodeItem, args: vec![],
   layout: REAL_LAYOUT })` returns `TyId(K)` (content-keyed dedup
   ignores layout under nominal bucket).

### `ArgsKey` tests

- `ArgsKey(0)` constructs.
- `ArgsKey::EMPTY.0` matches the runtime SHA-256 of the canonical
  empty-`GenericArgs` byte form (`b"\x00\x00\x00\x00"`); test
  computes SHA-256 at runtime and asserts equality.
- `ArgsKey` round-trips through serde (`u128` shape).
- `ArgsKey` derives `Copy`, `Eq`, `Hash`, `Ord`.

### Per-`Ty`-variant canonical byte-encoding

For each of the 19 `Ty` variants, hand-construct a small fixture
model + `Ty` instance, compute the canonical bytes, and compare
against a hand-checked golden byte sequence. Sample tests:

- `canonical_ty(Ty::Bool)` produces `[0x00]`.
- `canonical_ty(Ty::Int(IntKind::I32))` produces `[0x02, 0x02]`
  (tag=2, IntKind disc=2).
- `canonical_ty(Ty::Char)` produces `[0x01]`.
- `canonical_ty(Ty::Float(FloatKind::F64))` produces `[0x04, 0x02]`.
- `canonical_ty(Ty::Str)` produces `[0x05]`.
- `canonical_ty(Ty::Never)` produces `[0x06]`.
- `canonical_ty(Ty::Array(elem_ty, 7))` produces `[0x07] ++
  <canonical(elem_ty)> ++ 7_u64.to_le_bytes()`.
- `canonical_ty(Ty::Slice(elem_ty))` produces `[0x08] ++
  <canonical(elem_ty)>`.
- `canonical_ty(Ty::Ref(pointee, Provenance::SharedBorrow))`
  produces `[0x09] ++ <canonical(pointee)> ++ [0x00]`.
- `canonical_ty(Ty::Ptr(pointee, Mutability::Mut))` produces
  `[0x0A] ++ <canonical(pointee)> ++ [0x01]`.
- `canonical_ty(Ty::FnDef(item, args))` produces `[0x0B] ++
  <DefPathHash bytes for item> ++ <canonical_args(args)>`.
- `canonical_ty(Ty::FnPtr(sig))`: `#[ignore]` marked
  `// IGNORE-UNTIL: P1a-DS-06`.
- `canonical_ty(Ty::Dynamic { auto_traits: vec![Send, Sync],
  vtable_ty })` produces `[0x0D] ++ [2, 0, 0, 0] ++ [0, 1] ++
  <canonical(vtable_ty)>` (sorted u8s by AutoTrait discriminant).
- `canonical_ty(Ty::Closure(item, args))` produces `[0x0E] ++
  <DefPathHash> ++ <canonical_args(args)>`.
- `canonical_ty(Ty::Tuple { fields: vec![t1, t2], layout })`
  produces `[0x0F] ++ [2, 0, 0, 0] ++ <canonical(t1)> ++
  <canonical(t2)>`.
- `canonical_ty(Ty::Struct { item, args, layout })` produces
  `[0x10] ++ <DefPathHash(item)> ++ <canonical_args(args)>`.
- Same for `Enum` (tag 0x11) and `Union` (tag 0x12).

### `GenericArgs` container encoding

- Empty `GenericArgs`: `canonical_args(&vec![])` produces
  `[0, 0, 0, 0]` (u32 LE 0).
- `GenericArgs::from(vec![GenericArg::Type(ty1)])` produces
  `[1, 0, 0, 0] ++ [0x00] ++ <canonical(ty1)>`.
- Mixed: `vec![GenericArg::Type(ty1), GenericArg::Const(c1)]`
  produces `[2, 0, 0, 0] ++ [0x00] ++ <canonical(ty1)> ++ [0x01]
  ++ <canonical(c1)>` -- the Const tail is `#[ignore]`-marked
  `// IGNORE-UNTIL: P1a-DS-05`.

### Cross-build determinism (deep canonicaliser correctness)

- Build two fixture models with deliberately-different `TyId`
  assignment order (e.g., construct `Ty::Bool` first in model A,
  third in model B) but identical underlying `Ty` content. Compute
  `args_key` for an equivalent `GenericArgs` value in each model.
  Assert the two `ArgsKey` values are equal.
- Repeat with deliberately-different `StringId` assignment order
  (where the canonical encoding resolves through `model.strings`).

### Custom serde for `TyInterner`

- Round-trip `TyInterner` populated with 5 distinct `Ty` values
  through `serde_json`. Assert `len()` is preserved, `resolve(id)`
  matches the original `Ty` for every original `id`, `iter()`
  yields entries in the original order, and `lookup` is
  reconstructed (verifiable by `find(&ty)` returning the right
  `TyId` post-deserialise).
- Inject duplicate canonical content into the serialised form
  (hand-build a JSON array with two `Ty::Bool` entries) and assert
  deserialise rejects with a content-uniqueness error.
- Inject a dangling `TyId` reference (a `Ty::Array(TyId(99), 5)`
  in a 3-element storage) and assert deserialise rejects with an
  out-of-bounds error.

### `NominalKind` exhaustiveness scaffold

```rust
#[allow(dead_code)]
fn _exhaustiveness_check(k: &NominalKind) {
    match k {
        NominalKind::Struct
        | NominalKind::Enum
        | NominalKind::Union
        | NominalKind::FnDef
        | NominalKind::Closure => (),
    }
}
```

### `GenericArg` exhaustiveness scaffold

```rust
#[allow(dead_code)]
fn _generic_arg_exhaustiveness(a: &GenericArg) {
    match a {
        GenericArg::Type(_) | GenericArg::Const(_) => (),
    }
}
```

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-68 against its task spec at
.harness/tasks/regseq-model/68-ty-interner-and-args-key.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/ty_interner.rs
- crates/regseq-model/src/args_key.rs
- crates/regseq-model/src/canonical.rs (if introduced)
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/model.rs (root-struct types field
  replacement)
- crates/regseq-model/Cargo.toml (sha2 dep)

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures, the §12.1
  identity bucket table, the §12.3 per-`Ty`-variant byte-encoding
  table, the §12.5 serialisation rules).
- `TyIdentityKey` is crate-private; `NominalKind` is public; the
  `variant_kind_tag` distinguishes `Closure` and `Struct` for the
  same Item per spec.md:4214-4218.
- `find_nominal` is layout-agnostic per spec.md:4193-4198.
- `ArgsKey::EMPTY` matches runtime SHA-256 of the canonical
  empty-`GenericArgs` bytes.
- Canonical byte encoding: per-variant tags 0..=18 match spec.md:4313-4334
  exactly; multi-byte integers little-endian; lengths LE u32 prefixes;
  nested IDs resolved to content via the spec's termination rules.
- Self-referential ADT pattern: `intern` + `patch_layout` semantics
  preserve nominal identity.
- Custom serde: storage-only on-wire shape; deserialise rejects
  content-duplicate and dangling-TyId payloads.
- `FnPtr` (tag 12) and `GenericArg::Const` (disc 1) canonical
  encoding stubs are explicitly `todo!()` with
  `// FORWARD: P1a-DS-06` / `// FORWARD: P1a-DS-05`; the
  corresponding tests carry `// IGNORE-UNTIL: P1a-DS-NN`.
- Test coverage vs the task's Test Plan (per-variant identity,
  patching, self-referential pattern, ArgsKey EMPTY constant,
  per-variant canonical bytes, GenericArgs container, cross-build
  determinism, custom serde failure modes, exhaustiveness scaffolds).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
P1a-DS-06 for FnSig canonical bytes, P1a-DS-05 for Const canonical
bytes, P1a-VAL-02 for deserialise-time validator beyond the local
interner rules).
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

- `TyInterner` lands with all 7 spec methods (`new`, `intern`,
  `resolve`, `find`, `find_nominal`, `len`, `iter`); the private
  `TyIdentityKey` encodes the §12.1 nominal / tuple / structural
  buckets including the load-bearing `variant_kind_tag`.
- `NominalKind` is the public 5-variant enum.
- `GenericArg` (2 closed variants) and `GenericArgs = Vec<GenericArg>`
  land per spec.md:2404-2412.
- `ArgsKey(pub u128)` lands; `ArgsKey::EMPTY` matches the runtime
  SHA-256-truncated value of the empty-`GenericArgs` canonical
  bytes (verified by a runtime test).
- The deep canonical byte-encoding for `Ty` and `GenericArgs`
  matches spec.md:4313-4334 + spec.md:4347-4364 verbatim, with
  `FnPtr` (tag 12) and `GenericArg::Const` (disc 1) explicitly
  `todo!()`-stubbed and forward-pointed.
- Self-referential ADT reservation pattern is supported via
  `patch_layout`.
- Custom serde for `TyInterner` lands with deserialise validation
  per spec.md:4456-4498 (load full storage; bound-check IDs;
  content-uniqueness via `TyIdentityKey`).
- `sha2` crate added to `regseq-model/Cargo.toml`.
- Closed-enum exhaustiveness scaffolds present for `NominalKind`
  and `GenericArg`.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
