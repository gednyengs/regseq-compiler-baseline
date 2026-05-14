# regseq-model-66: `Ty` Enum + Auxiliary Closed Enums + §17.2 IR-Visible Helpers

## Description

Land the closed 19-variant `Ty` enum verbatim from spec.md §10
(spec.md:3643-3663), the auxiliary closed enums on `Ty`
(`Provenance` / `Mutability` / `AutoTrait` / `FnAbi` / `Safety`,
plus the primitive scalar discriminants `IntKind` / `UintKind` /
`FloatKind` and their `_KIND_COUNT` constants from spec.md:3672-3736),
and the §17.2 IR-visible helpers that operate on `Ty` without rustc
dependencies (notably the recursive `contains_unsafe_cell(ty,
model) -> bool` predicate at spec.md:6336+). This task replaces the
placeholder `Ty` in `crates/regseq-model/src/placeholders.rs` with
the spec-mandated body.

The 19-variant `Ty` enum is **closed** -- every variant is a *rigid*
post-monomorphisation type per `TyKind::is_known_rigid()`. Layout is
inline on `Tuple` / `Struct` / `Enum` / `Union`; the `Layout` type
ships in P1a-DS-03 (this task uses a forward-declared placeholder if
DS-03 has not landed, but the spec ordering says DS-03 follows DS-02
so the dependency is downstream).

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-02)
- `docs/discussions/regseq-model/spec.md` §10 (spec.md:3578-3796) +
  §17.1 (region erasure) + §17.2 IR-helper subset (spec.md:6298-6461)
- `crates/regseq-model/src/placeholders.rs` (existing placeholder
  `Ty` to replace)
- `crates/regseq-model/src/model.rs` (root struct -- consumed by
  `contains_unsafe_cell`)
- `crates/regseq-model/src/ids.rs` (`TyId`, `ItemId`, `FnSigId`,
  `VariantIdx`)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
  (previous task; the foundation provides the IDs and the
  `INT_KIND_COUNT` / `UINT_KIND_COUNT` / `FLOAT_KIND_COUNT`
  constants if DS-01 introduced them; otherwise this task does)

## Implementation Notes

Create a new module `crates/regseq-model/src/ty.rs` (or `ty/mod.rs`
with submodules `ty/kinds.rs`, `ty/aux.rs`, `ty/helpers.rs` if
splitting reads better). Re-export from `lib.rs`. Replace the
`Ty` placeholder in `placeholders.rs` with a re-export from the
new module (or remove the placeholder entirely if no other module
references it).

### Public API (verbatim from spec)

#### The `Ty` enum (spec.md:3643-3663)

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

`GenericArgs` and `Layout` come from sibling tasks
(`GenericArgs = Vec<GenericArg>` lands in P1a-DS-04 / DS-10;
`Layout` lands in P1a-DS-03). For this task they may be referenced
as forward types (the dependencies row in the brief sequences DS-02
before DS-03 / DS-04, but the type names are spec-pinned and may
exist as stubs from P1a-DS-01).

#### Primitive scalar enums (spec.md:3671-3702)

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
```

If P1a-DS-01 already introduced these constants for `ScalarLayouts`,
this task confirms the values match and does not redefine.

#### Safety (spec.md:3704-3709)

```rust
#[repr(u8)]
pub enum Safety {
    Safe   = 0,
    Unsafe = 1,
}
```

#### Auxiliary closed enums (spec.md:3717-3729)

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

#### §17.2 IR-visible helper (spec.md:6326-6356)

```rust
impl RegseqModel {
    pub fn contains_unsafe_cell(&self, ty: TyId) -> bool;
}
```

Recursion table (spec.md:6341-6351):

| `Ty` variant                         | Rule                                                                                                                          |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `Struct { item, .. }`                | `true` if `model.items[item].is_unsafe_cell`; else recurse into every `FieldLayout.ty` of the inline `Layout`.                |
| `Enum { item, .. }`                  | recurse into every `FieldLayout.ty` of every `VariantLayout`.                                                                  |
| `Union { item, .. }`                 | recurse into every `FieldLayout.ty`.                                                                                          |
| `Tuple { fields, .. }`               | recurse into each element (the inline `Layout`'s field types).                                                                |
| `Array(elem, _)`                     | recurse into `elem`.                                                                                                          |
| `Closure(item, args)`                | recurse via the parallel `Ty::Struct` (captures-struct) found via `find_nominal(NominalKind::Struct, item, args)`.            |
| `Ref(_, _)`, `Ptr(_, _)`, `FnPtr(_)` | `false`.                                                                                                                      |
| `FnDef(_, _)`                        | `false`.                                                                                                                      |
| `Bool`, `Char`, `Int`, `Uint`, `Float`, `Str`, `Slice`, `Dynamic`, `Never` | `false`.                                                                                |

Memoise via a visited `HashSet<TyId>` for cost (spec.md:6355).
The helper depends on `find_nominal` (P1a-DS-04 -- TyInterner)
and `Item.is_unsafe_cell` (P1a-DS-07); if those are not yet
landed, the helper body is `todo!()` with the recursion structure
in scaffolding and the unit tests are `#[ignore]`-marked with
`// IGNORE-UNTIL: P1a-DS-04 / P1a-DS-07`. The signature and the
public-API attestation MUST land here.

### Variant-level invariants (documentation, enforcement deferred)

Spec.md:3611-3635 commits five variant-level invariants. Each
appears as a doc-comment on the relevant variant:

- `FnDef` is a ZST.
- `Closure` carries identity-view semantics (parallel
  `Ty::Struct` for layout).
- `Ty::Tuple { fields: [] }` is Rust unit `()`; its layout is
  `UNIT_LAYOUT` (P1a-DS-03 owns the constant).
- `Str` / `Slice(_)` / `Dynamic{..}` are unsized markers; they
  appear only as `Ref`/`Ptr` pointee (invariant 12; enforced in
  P1a-VAL-03).
- `Never` is accepted as a `Ref`/`Ptr` pointee (codegen MAY reject).

Spec.md:3770-3791 commits the **wide-scalar admission**
(`Int(I128)` / `Uint(U128)` / `Float(F16)` / `Float(F128)` are
admitted unconditionally by the bridge; per-target rejection is at
codegen). This task documents the rule on the relevant `IntKind` /
`UintKind` / `FloatKind` variants and ships unit tests confirming
the variants construct and round-trip without conditional gating.

### Identity buckets (documentation, enforcement deferred)

Per spec.md:3585-3605 (the variant table) and spec.md:4200-4218 (the
identity-bucket table), each variant doc-comment names its identity
bucket (Nominal / Tuple / Other structural). Enforcement of the
private `TyIdentityKey` lives in P1a-DS-04.

### Conventions

- **Closed enum**: every variant is enumerated; no `#[non_exhaustive]`,
  no wildcard `Other(...)` arm. Future spec revisions extend by
  appending; this is a pinned spec contract.
- **Discriminant stability**: `IntKind` / `UintKind` / `FloatKind` /
  `Safety` / `Provenance` / `Mutability` / `AutoTrait` / `FnAbi`
  carry `#[repr(u8)]` with explicit numeric assignments per spec
  (load-bearing for canonical byte form in P1a-DS-04 ArgsKey).
- **Doc comments on every variant + every helper**.
- **No unicode in comments / docs**.
- **Serde shape**: `Ty` derives `Serialize` / `Deserialize` (no
  Symbol/StringId payloads inside `Ty` itself, so the brief's
  cross-cutting deferral does not apply); auxiliary enums derive
  serde with the natural shape; `IntKind` / `UintKind` / `FloatKind`
  serialise as their u8 discriminant (`#[serde(into / try_from)]` to
  enforce closed-vocabulary on deserialise; reject unknown
  discriminants).
- **Derive set**: `Clone, Debug, PartialEq, Eq, Hash` on `Ty`
  (full-value equality including `Layout` per spec.md:4127-4137);
  also `Serialize, Deserialize`. Auxiliary enums add `Copy, Ord,
  PartialOrd` where it makes sense (`AutoTrait` needs `Ord` for
  the sorted-discriminant invariant).

## Non-Goals

- **No `TyInterner` body** -- P1a-DS-04 owns the BiMap, the private
  `TyIdentityKey`, and the canonical-encoding ArgsKey.
- **No `Layout` machinery** -- P1a-DS-03 owns Layout / EnumShape /
  TagScalar / `UNIT_LAYOUT` constant / `size_align_of` query.
- **No fat-pointer mutability rules** -- spec.md:6377-6427 (§17.2.2 +
  §17.2.3) is **phase-1b** territory (codegen-side). This task only
  ships the `contains_unsafe_cell` helper, NOT the wide-pointee /
  trait-object dispatch carve-outs.
- **No `Item` schema** -- P1a-DS-07 owns `Item.is_unsafe_cell`. The
  `contains_unsafe_cell` helper depends on it; ship the helper as a
  `todo!()` stub with `IGNORE-UNTIL` test markers if DS-07 has not
  landed at execution time.
- **No bridge translation rules** -- spec §15 is the bridge axis
  (P1a-BR-02).
- **No `find_nominal` body** -- P1a-DS-04 owns it.
- **No invariant 12 (unsized-marker placement) enforcement** --
  P1a-VAL-03 owns it; this task documents the rule on variant
  doc-comments.
- **No pretty-printer** -- pretty-printer axis (P1a-PP-02 owns
  `render_ty`).

## Test Plan

`unit-in-src` discipline. All tests live in
`crates/regseq-model/src/ty.rs::tests` (or a sibling test module per
the new file structure). Per `SESSION.md` Testing Requirements,
every public type / function gets at least one test.

### Variant constructors (one per variant)

19 tests, one per `Ty` variant:

- `Ty::Bool`, `Ty::Char`, `Ty::Str`, `Ty::Never` -- niladic.
- `Ty::Int(IntKind::*)` -- six variants × tested per discriminant.
- `Ty::Uint(UintKind::*)` -- six variants.
- `Ty::Float(FloatKind::*)` -- four variants.
- `Ty::Array(TyId, u64)`, `Ty::Slice(TyId)` -- payload roundtrip.
- `Ty::Ref(TyId, Provenance)` × both Provenance variants.
- `Ty::Ptr(TyId, Mutability)` × both Mutability variants.
- `Ty::FnDef(ItemId, GenericArgs)`, `Ty::FnPtr(FnSigId)`.
- `Ty::Dynamic { auto_traits, vtable_ty }` -- empty + sorted set.
- `Ty::Closure(ItemId, GenericArgs)`.
- `Ty::Tuple { fields: vec![], layout }` -- unit form construct.
- `Ty::Struct { item, args, layout }`, `Ty::Enum { ... }`, `Ty::Union { ... }`.

### Auxiliary enums (one round-trip test per enum)

- `IntKind` discriminants 0..=5 round-trip through `as u8` and back via
  `try_from(u8)`; unknown discriminant `6` is rejected.
- Same for `UintKind`, `FloatKind`, `Safety`, `Provenance`,
  `Mutability`, `AutoTrait`, `FnAbi`.

### Constants

- `INT_KIND_COUNT == 6`, `UINT_KIND_COUNT == 6`, `FLOAT_KIND_COUNT == 4`.
- Each constant matches the actual variant count of its enum
  (`std::mem::variant_count` is unstable; assert by hand-counting
  with a `match` arm that returns a per-variant `0` count summing
  to the constant).

### Equality and hash

- Two `Ty::Bool` values are equal and hash to the same bucket.
- Two `Ty::Struct { item, args, layout }` values with the same
  `(item, args)` but **different** layout are NOT publicly equal
  per spec.md:4127-4137 (full-value equality including `Layout`).
- Hashing two `Ty::Tuple` values with the same fields and the same
  layout produces the same hash.

### Serde round-trip

- Each `Ty` variant round-trips through `serde_json` losslessly
  (single-variant test suffices per variant).
- Each auxiliary enum round-trips; serialised form is the u8
  discriminant for `#[repr(u8)]` enums (verify via `serde_json::to_string`
  output containing only the integer).

### Wide-scalar admission

- `Ty::Int(IntKind::I128)`, `Ty::Uint(UintKind::U128)`,
  `Ty::Float(FloatKind::F16)`, `Ty::Float(FloatKind::F128)`
  construct without panic and round-trip through serde
  (per spec.md:3770-3791 unconditional bridge admission;
  rejection lives at codegen).

### `AutoTrait` ordering

- `[AutoTrait::Send, AutoTrait::Sync, AutoTrait::Unpin]` is sorted
  ascending by discriminant.
- `Ty::Dynamic { auto_traits: vec![AutoTrait::Send, AutoTrait::Sync],
  vtable_ty: ... }` constructs; out-of-order auto_traits is NOT
  rejected here (validation lives in P1a-VAL-02 / VAL-03).

### `contains_unsafe_cell` helper

If P1a-DS-04 (`find_nominal`) and P1a-DS-07 (`Item.is_unsafe_cell`)
are landed at execution time, ship green tests:

- `contains_unsafe_cell(model, bool_ty)` returns `false`.
- `contains_unsafe_cell(model, ref_ty)` returns `false` regardless of
  pointee.
- `contains_unsafe_cell(model, fnptr_ty)` returns `false`.
- A `Ty::Struct` whose Item is flagged `is_unsafe_cell == true`
  returns `true`.
- A `Ty::Struct` containing a `Ty::Struct` flagged `is_unsafe_cell`
  via a non-pointer field returns `true` (transitivity through
  structural fields).
- A `Ty::Array(elem, n)` recurses into `elem`.
- A `Ty::Tuple { fields, .. }` recurses into each element.
- A `Ty::Closure(item, args)` recurses via the parallel `Ty::Struct`.
- Memoisation: feeding the same `TyId` through the helper twice
  produces the same result without panicking on cycles (well-formed
  Rust + spec §12.2 self-referential reservation breaks cycles
  through `Ref`/`Ptr`, on which the helper terminates -- but the
  test asserts the helper doesn't infinite-recurse on a
  pathologically-constructed cycle, e.g., a hand-built `Ty::Struct`
  whose layout field references itself, by asserting the visited
  HashSet limits recursion).

If those dependencies have not landed, mark the helper tests
`#[ignore]` with `// IGNORE-UNTIL: P1a-DS-04 + P1a-DS-07` and
ship the helper signature only.

### Closed-enum exhaustiveness (compile-time)

Add a hidden test function

```rust
#[allow(dead_code)]
fn _exhaustiveness_check(t: &Ty) {
    match t {
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_)
        | Ty::Str | Ty::Never | Ty::Array(_, _) | Ty::Slice(_)
        | Ty::Ref(_, _) | Ty::Ptr(_, _) | Ty::FnDef(_, _) | Ty::FnPtr(_)
        | Ty::Dynamic { .. } | Ty::Closure(_, _) | Ty::Tuple { .. }
        | Ty::Struct { .. } | Ty::Enum { .. } | Ty::Union { .. } => (),
    }
}
```

Adding a 20th variant in a future spec revision (without removing
this scaffold) breaks the build, surfacing the omission.

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-66 against its task spec at
.harness/tasks/regseq-model/66-ty-enum-and-helpers.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/ty.rs (or ty/*.rs)
- crates/regseq-model/src/placeholders.rs (verify Ty stub removed
  or replaced)
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/model.rs (the `contains_unsafe_cell`
  impl and any forward-declaration cleanup)

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures for the 19-variant
  Ty, the 8 auxiliary closed enums, the `_KIND_COUNT` constants,
  and `contains_unsafe_cell`).
- Closed-enum discipline: no `#[non_exhaustive]`, no wildcard
  `Other(...)` arm, exhaustiveness-check scaffold present.
- `#[repr(u8)]` discriminants match spec verbatim per spec.md:3672-3729.
- Wide-scalar admission documented on `IntKind::I128` /
  `UintKind::U128` / `FloatKind::F16` / `FloatKind::F128`.
- `contains_unsafe_cell`: signature matches spec.md:6336;
  recursion table arm-by-arm matches spec.md:6341-6351; memoisation
  via visited HashSet in scaffolding (or `todo!()` with explicit
  IGNORE-UNTIL test markers if dependencies absent).
- Test coverage vs the task's Test Plan (per-variant constructors,
  per-enum round-trips, equality / hash, serde round-trip, wide
  scalars, AutoTrait ordering, contains_unsafe_cell rules,
  exhaustiveness scaffold).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
P1a-DS-03 for Layout, P1a-DS-04 for TyInterner, fat-pointer
mutability for phase-1b, etc.).
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

- The 19-variant closed `Ty` enum lands verbatim per spec.md:3643-3663.
- Auxiliary closed enums `Provenance`, `Mutability`, `AutoTrait`,
  `FnAbi`, `Safety`, `IntKind`, `UintKind`, `FloatKind` land with
  `#[repr(u8)]` discriminants matching the spec.
- `INT_KIND_COUNT == 6`, `UINT_KIND_COUNT == 6`, `FLOAT_KIND_COUNT == 4`.
- The `Ty` placeholder in `crates/regseq-model/src/placeholders.rs`
  is replaced (or removed entirely if no other module references it).
- `contains_unsafe_cell(&self, ty: TyId) -> bool` lands on
  `RegseqModel`; recursion table per spec.md:6341-6351 implemented
  (or `todo!()`-stubbed with `IGNORE-UNTIL` test markers).
- All variants and enums have doc comments; variant-level invariants
  per spec.md:3611-3635 are documented.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
