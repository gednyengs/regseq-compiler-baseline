# User-Data Initialization Contract

## Status

Settled 2026-04-22 via three-round Codex adversarial debate
(transcripts preserved under
`user-data-initialization-codex-review/`). Stage-1 implementation
scope: **Path A** -- backend-realizable subset only. Wider scope
deferred until a fixture motivates it.

## Motivation

The Regseq compiler generates driver-struct creation code in the
target OS (e.g., `devm_kzalloc` on Linux). The driver author never
writes a constructor; their first opportunity to run code is inside
lifecycle methods like `init()`, which execute AFTER the struct is
fully allocated. Every user-data field on the driver struct must
therefore have a well-defined initial value the compiler knows how
to emit without author input.

The current rule at
`crates/regseq-driver/src/construction/driver.rs:216-253` requires
user-data to be `Option<T: Copy>`. Rationale: `None` has a known
zero-tag layout for most `T`, so `devm_kzalloc` naturally leaves the
field at `None`. Authors acknowledge "unseeded" as a distinct state
via pattern matching or `unwrap_or`.

This rule is too narrow for always-seeded driver state. Concrete
blocker: PL011 Stage 1 introduced `imsc_shadow: u32` (software
mirror of the IMSC interrupt-mask register, per datasheet 3.3.8 and
REQ-CORR-MASK-SHADOW). This field is always seeded at `init()` and
has no meaningful "unseeded" state. The existing rule rejects it
with `InvalidUserDataShape` at `driver.rs:249`, breaking the
compiler's construction pass on the Stage-1-complete fixture.

## Design contract

User-data field types must satisfy **Rule 1** OR **Rule 2**.

### Rule 1 -- zero-init whitelist (compiler-controlled)

A type `T` is zero-init-accepted iff:

1. `T: Copy` (outer check; preserved so interior-mutability types
   like `UnsafeCell<u32>` cannot sneak through recursive descent),
   AND ONE OF:
2a. `T` is on the compiler's exact primitive whitelist (see
    "Stage-1 whitelist" and "Full target whitelist" below).
2b. `T` is `[U; N]` where `U` is zero-init-accepted.
2c. `T` is a tuple `(U1, U2, ...)` where every `Ui` is
    zero-init-accepted.
2d. `T` is a struct (`DefKind::Struct`, not enum, not union) where
    every monomorphized field type satisfies the predicate. Field
    types obtained via `field.ty(tcx, substs)`. Recursion uses a
    visited-set keyed by **monomorphized type identity**
    `(DefId, GenericArgsRef)` or `Ty<'tcx>`, NOT by `DefId` alone
    (see "Soundness: cycle guard" below).
3. `T` contains no unresolved type parameters (`TyKind::Param`) or
   projections (`<T as Trait>::Assoc`). Generic driver-struct
   user-data types are accepted only when fully monomorphized;
   associated types must be normalized via the appropriate typing
   environment before applying the whitelist, or rejected outright.

**Initialization mechanism**: the all-zero bit pattern produced by
`devm_kzalloc` on Linux (and equivalent zero-allocation on other
backends) is a valid and semantically-meaningful initial value. No
post-allocation store is emitted.

### Rule 2 -- Option for unseeded state

Field type is `Option<T: Copy>`. Unchanged from the current rule.

**Initialization mechanism**: `devm_kzalloc` today. A future
hardening pass will emit compiler-controlled `None`-init bytes for
niche-optimized Options where `kzalloc` gives `Some(T::zero())`
rather than `None` (see "Deferred: Rule-2 niche-init hardening"
below).

### Explicitly rejected types and unsoundness demonstrations

- **Enums** (even zero-discriminant): `#[repr(u8)] enum E { A = 1 }`
  -- `zeroed::<E>()` produces discriminant `0`, which is invalid
  per the Rust Reference's enum-invalid-values rule.
- **Unions**: `union U { nz: NonZeroU32, raw: u32 }` -- zeroing
  then reading `u.nz` produces an invalid `NonZeroU32`. Union
  validity is also not fully decided by the Rust Reference.
- **`NonZero*` and `NonNull<T>`**: zero violates the valid-range
  invariant enforced by the standard library.
- **Function pointers**: `zeroed::<fn()>()` is a null function
  pointer; null `fn` pointers are invalid.
- **`Box`, `Rc`, `Arc`, references**: zero is null/dangling;
  violates ownership invariants.
- **`Cell`, `UnsafeCell`, `RefCell`**: not `Copy`; blocked by the
  outer `Copy` check. `UnsafeCell<NonZeroU32>` would also have
  invalid inner state.
- **Atomics (`AtomicU32`, etc.)**: not `Copy`; synchronization
  semantics are not a plain field-init contract.
- **Raw pointers (`*mut T`, `*const T`)**: zero is null;
  dereferencing would break. Semantic-policy rejection (the
  zero-bit pattern is technically valid, but pointer-typed state
  is not a driver-private-state shape Regseq aims to support).
- **`char`**: zero is `'\0'` which IS a valid `char`, but we
  exclude it from the whitelist because driver state rarely needs
  it and the backend has no mapping for it.
- **`u128`, `i128`**: excluded from the Stage-1 and Full-target
  whitelists because the Linux C backend has no mapping for these
  widths in device-driver contexts.
- **`!` (never type)**: no valid inhabitants; no valid init.

### Soundness: cycle guard keyed by monomorphized type

A global visited-set on `DefId` is insufficient. Counterexample:

```rust
#[derive(Copy, Clone)]
struct Boxed<T>(T);

#[derive(Copy, Clone)]
struct Outer {
    good: Boxed<u32>,
    bad:  Boxed<core::num::NonZeroU32>,
}
```

If `Boxed` is memoized by `DefId`, visiting `Boxed<u32>` first
causes `Boxed<NonZeroU32>` to be skipped on the second visit. The
`NonZeroU32` case never gets rejected. The visited-set must be keyed
by monomorphized type identity: `(DefId, GenericArgsRef)` or
`Ty<'tcx>`.

## Stage-1 whitelist (Path A -- backend-realizable today)

The compiler's Rule-1 predicate accepts the following primitive
types in Stage 1:

- `u8`, `u16`, `u32`, `u64`
- `bool`

Plus arrays and `Copy` structs recursively composed of these.

### Stage-1 Rule-1 whitelist narrowing (tuples excluded)

Tuples are part of the design target (Rule 1 2c) but are
**excluded from the Stage-1 implementation** of Path A. The
model's `RegseqType` enum at
`crates/regseq-model/src/ir/types.rs:18-29` has `Scalar`, `Bool`,
`Array`, `Defined` (user-type reference), and others, but no
`Tuple` variant. Admitting tuples at construction would require
extending `RegseqType` with a `Tuple(Vec<RegseqType>)` variant
(or equivalent), which cascades into every consumer (IR
construction, normalize, verify, lowering plan, backend). That
extension is out of scope for Path A.

When a future fixture actually needs a tuple as user-data, a
follow-up task adds the `RegseqType::Tuple` variant plus the
corresponding construction-predicate arm, verifier rules, and
backend C-mapping extension (anonymous struct emission). Until
then: tuples are rejected at construction with a clear
"not yet supported" error.

Rationale: the Linux backend's scalar-to-C type mapper at
`crates/regseq-backends/src/linux/plan.rs:400` currently emits C
types only for this set. Accepting types the backend cannot emit
would produce a compiler-accept-but-emitter-panic failure mode,
which is worse than a conservative reject.

PL011's `imsc_shadow: u32` falls within this subset.

### Stage-1 backend-tested scope (explicit narrowing)

The whitelist accepts scalars, arrays, tuples, and `Copy` structs
of these (the Rule-1 four arms). **Stage-1 Linux emission is
backend-tested for SCALAR user-data fields only**. Arrays, tuples,
and user-defined structs are accepted at the model + lowering-plan
levels in Stage 1 but are NOT guaranteed to reach the Linux
emitter without error. This is a deliberate scope constraint, not
an omission.

Constraint statement:

- **Model level**: the Rule-1 predicate accepts the whitelist
  (scalar + array + `Copy` struct of accepted; tuples excluded
  per the narrowing subsection below).
- **Lowering plan level**: user-data obligations carry the
  whitelist shapes. The RegseqModel-based lowering pipeline
  (`from_model.rs`) populates them from `DriverStruct.user_data`;
  the legacy lowering pipeline (`planner.rs`) does NOT populate
  user-data in Stage-1 and is deprecated (see touch-point list
  below for the Stage-1 asymmetry rationale).
- **Linux plan level (construction-time rejection)**: only scalar
  user-data fields are guaranteed to produce a valid Linux plan.
  Non-scalar user-data fields (array, `Copy` struct) are rejected
  at Linux plan construction with a new
  `LinuxPlanError::UnsupportedUserDataShape` variant on
  `crates/regseq-backends/src/linux/planner.rs`. The rejection
  happens BEFORE the field would be inserted into
  `shared_struct_fields`, so the C-AST renderer at
  `crates/regseq-backends/src/c/mod.rs:232-238` (which produces
  `<type_name> <field_name>;` output from a
  `LinuxSharedStructField.c_type` string via
  `crates/regseq-backends/src/linux/emitter.rs:152-160`) never
  sees an unrealizable type.
- **Infrastructure for non-scalar emission is NOT landed in
  Stage 1**: a `user_data_aggregate_to_c_type` helper for arrays
  and structs requires a correct C declarator strategy
  (specifically, arrays cannot be rendered as `u32[4] field;`
  under the current renderer; the correct form is
  `u32 field[4];`, which requires a richer declarator AST).
  Adding such a declarator AST is out of scope for Path A. A
  future task adds both the aggregate helper AND the declarator
  AST together, triggered by the first fixture that uses a
  non-scalar user-data field.

Non-scalar user-data in a Stage-1 fixture is thus a plan-time
compile error, not a silent miscompile. The PL011 Stage-1 fixture
only uses `imsc_shadow: u32` (scalar), so this error path is not
reached in normal Stage-1 execution. This approach satisfies
round-3 Codex Q5's backend-realizability requirement via the "constrain
accepted backend-tested shapes" option, not the "implement
emission for every shape" option.

## Full target whitelist (deferred)

The complete design target admits these additional primitives once
the backend C-type mapping is extended:

- Signed integers: `i8`, `i16`, `i32`, `i64`
- Platform-width: `usize`, `isize`
- Floats: `f32`, `f64`

Extension is pull-based: when a real fixture needs one of these
types as user-data, extend the backend mapping FIRST, then add the
type to the Rule-1 whitelist. Do not widen the whitelist
speculatively.

## Rule-2 niche-init hardening (deferred)

`devm_kzalloc` gives `Some(T::zero())` rather than `None` for
niche-optimized Options where the zero byte pattern encodes a
different variant than `None`:

- `Option<bool>`: zero byte = `Some(false)`, not `None`.
- `Option<char>`: zero byte = `Some('\0')`, not `None`.
- Other enum/niche-optimized cases that rustc's layout picks
  non-zero-niche-values for.

This is a latent soundness bug in the current Rule-2 contract. No
current fixture exercises it. Hardening plan when a fixture does:
the compiler emits explicit `None`-init bytes after `kzalloc`,
computed via `tcx.layout_of(Option<T>)` plus const-evaluation of
`Option::<T>::None`. Init bytes are encoded in the frontend
artifact and carried to the backend (which has no `TyCtxt`
access).

## Alternatives considered

| Option | Why rejected |
|---|---|
| Author `Option<T: Copy>` workaround with `.unwrap()` / `.expect()` | Codex confirmed `Option::unwrap`/`expect` lower to no-ops in `semantic_walker`; `core::panic` isn't classified; rejecting always-seeded state to force Option is semantic tax. |
| `T: Default + Copy` with compile-time `T::default()` evaluation | Pinned toolchain 2026-02-01 requires unstable `const_default`/`const_trait_impl` features; no existing machinery evaluates trait-method calls at compile time. Toolchain-blocked. |
| `regseq::ZeroInit` safe trait with compiler structural verification + `unsafe impl` escape hatch | Illegal Rust: rustc rejects `unsafe impl` of a safe trait (E0199) and safe `impl` of an unsafe trait (E0200). |
| `regseq::ZeroInit` unsafe trait + `#[derive(ZeroInit)]` generating `unsafe impl` | Works legally but introduces trait + derive + structural verification machinery without a motivating fixture. Deferred; revisit when author opt-in becomes necessary. |
| `#[rseq::zero_init]` attribute on type definitions | Alternative to the trait approach with equivalent power; same deferral rationale. |
| Compiler-emitted definite-assignment analysis | Most permissive, most complex. Large analysis problem, not a small init rule. Out of scope. |
| Author-implemented `Default` on the driver struct | Conflicts with compiler-owned resource fields (MMIO, IRQ, Clock). Rejected prima facie. |

## Implementation touch-points

Per Codex's exhaustive scan, Stage-1 Path A requires changes at the
following sites (more than the initial 5 the design summary listed):

### Construction

- `crates/regseq-driver/src/construction/driver.rs:216-253` --
  replace the strict `Option<T>` check with a branch accepting
  either Rule 1 or Rule 2; preserve the outer `Copy` check.
- `crates/regseq-driver/src/construction/driver.rs:227,562` --
  outer `Copy` check continues to use
  `type_is_copy_modulo_regions` with
  `TypingEnv::fully_monomorphized()`; reject types with
  unresolved `TyKind::Param` before applying the whitelist.
- `crates/regseq-driver/src/construction.rs:146,467` -- update
  error docs and display text that currently say
  "user-data must be `Option<T>`".

### Model

- `crates/regseq-model/src/model/resources.rs:306` -- extend
  `UserDataField` to carry the Rule-1-vs-Rule-2 classification
  (enum variant or a classification field; pick the shape that
  preserves existing consumers).
- `crates/regseq-model/src/verify/mod.rs:205` -- rename/redocument
  the `UserDataFieldNotOptionCopy` error kind.
- `crates/regseq-model/src/verify/rules/driver_struct.rs:212,239` --
  update verifier logic and the "spec requires Option<T>"
  diagnostic text.
- `crates/regseq-model/src/verify/rules/cross_item.rs:348` --
  cross-item type resolution must follow the new classified
  stored type.
- `crates/regseq-model/src/normalize/surface.rs:1044,1081,1102` --
  raw/legacy normalization currently enforces `Option<T: Copy>`
  and stores every user-data field as `Scalar(U32)`. The primitive
  Copy table at line 1102 also includes `u128`, `i128`, and
  `char`, which diverges from the Stage-1 whitelist and must be
  tightened.

### Lowering

- `crates/regseq-backends/src/lowering/plan.rs:109` -- add
  user-data to `ResourceObligations` (currently only
  MMIO/IRQ/property/clock/reset/pinctrl).
- `crates/regseq-backends/src/lowering/from_model.rs:190` -- the
  RegseqModel-based lowering pipeline; populates user-data from
  `DriverStruct.user_data`.
- `crates/regseq-backends/src/lowering/planner.rs:311` -- the
  legacy lowering pipeline (deprecated; being retired per
  Phase-7 migration). **Stage-1 Path A asymmetry**: the legacy
  pipeline does NOT populate user-data. Extending it would
  require cascading `DeclarationModel` + raw-input + legacy
  normalization changes that are out of Path A scope. The
  `ResourceObligations.user_data` field is initialized to an
  empty `Vec` in the legacy path; only the RegseqModel pipeline
  populates real entries. This asymmetry is acceptable because
  PL011 (the Stage-1 fixture) runs through the RegseqModel
  pipeline, not the legacy one, and the legacy path has no
  downstream consumer of user-data.

### Linux backend

- `crates/regseq-backends/src/linux/planner.rs:146` -- include
  user-data in `shared_struct_fields` (currently built from
  resources/properties/serial-port only).
- `crates/regseq-backends/src/linux/plan.rs:400` -- the scalar
  C-type mapper family. `LinuxPropertyScalar::c_type` already
  supports `u8, u16, u32, u64, bool` for property scalars.
  Stage-1 adds a narrow parallel user-data-scoped mapping:
  `LinuxUserDataScalar` enum (covers only the Stage-1 scalar
  whitelist). No aggregate helper lands in Stage 1.
- `crates/regseq-backends/src/linux/planner.rs` -- add a new
  `LinuxPlanError::UnsupportedUserDataShape` variant. Linux plan
  construction iterates the driver's user-data fields and either
  emits a `LinuxSharedStructField` via `LinuxUserDataScalar` (for
  scalar user-data) or returns `UnsupportedUserDataShape` (for
  non-scalar user-data). This is the intentional Stage-1 error
  signal; see the "Stage-1 backend-tested scope (explicit
  narrowing)" section.
- `crates/regseq-backends/src/linux/emit_semantic.rs:216` --
  **NOT MODIFIED in Stage 1**. The existing unknown-struct panic
  path is unrelated to the shared-header user-data emission path
  (which renders via `emitter.rs:152-160` and `c/mod.rs:232-238`).
  A future task that wires non-scalar user-data emission will
  need to extend both the C declarator AST and the Linux planner
  path; `emit_semantic.rs` remains unchanged in both Stage 1 and
  the follow-up wiring task.

### Backend kzalloc path

- `crates/regseq-backends/src/linux/emit_module.rs:703` --
  `devm_kzalloc` already zero-initializes the whole struct; no
  additional post-allocation store is required for any Stage-1
  Rule-1 field. No change.

## Paper framing

> Regseq initializes driver-private state implicitly only for a
> compiler-proven structural zero-safe subset of types; all other
> unseeded state must be explicit as `Option<T>`. No user
> assertion is trusted for initialization validity. The accepted
> subset is a closed, auditable whitelist; widening is pull-based
> and backend-gated.

This framing avoids trust in author-supplied unsafety markers and
keeps the accepted-type set fully mechanically checkable. Tier-2
author opt-in (via `ZeroInit` trait or `#[rseq::zero_init]`
attribute) is deferred future work; its absence strengthens the
paper's safety story because initialization is never trusted on
author signature alone.

## Codex review provenance

Three adversarial debate rounds preserved under
`user-data-initialization-codex-review/`:

- `round-1-initial-debate.md` -- evaluated six initialization-rule
  options against Regseq goals. Verdict: Option III
  (`Option<T: Copy>` OR `T: Zeroable + Copy`) recommended over
  Option II (`Default + Copy`). Discovered Option II is
  toolchain-blocked (unstable `const_default` features).
- `round-2-revised-tier2.md` -- reviewed the revised `ZeroInit`
  safe-trait-with-unsafe-escape design. Verdict:
  `revise-and-re-review`. Flagged that the design is illegal Rust:
  rustc rejects `unsafe impl` of a safe trait (E0199) and safe
  `impl` of an unsafe trait (E0200). Identified 10-12 compiler
  touch-points beyond the initial 5 and deferred Tier-2/Tier-3
  implementation until motivating fixtures surface.
- `round-3-final-design.md` -- reviewed the collapsed two-rule
  whitelist+Option design. Verdict: `adopt-with-amendments`;
  `CONVERGENCE: converged`. Confirmed struct recursion via
  `field.ty(tcx, args)` is sound, foreign-struct acceptance
  requires no namespace restriction, outer `Copy` check must be
  preserved, cycle guard must key by monomorphized type.
  Identified backend-realizability gate leading to Path A.

## Open follow-ups for implementation phase

- Decide the exact shape of the `UserDataField` model extension
  (enum variant vs classification flag) during implementation.
- Decide the precise normalization strategy for associated-type
  projections in user-data field types (normalize or reject).
- Author at least one negative test per rejected category (enum,
  union, `NonZero*`, `UnsafeCell`, raw pointer, generic param).
- Author at least one positive test per Rule-1 arm (scalar, array,
  tuple, `Copy` struct of scalars, nested struct of structs).
