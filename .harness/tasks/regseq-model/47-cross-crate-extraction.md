# regseq-model-47: Rule 3 — Cross-Crate Function Extraction

## Description

Spec code is `#![no_std]` -- only `core` is permitted -- and per
spec.md Section 1, "monomorphized `core` functions (e.g.,
`Option::map`, `Option::unwrap_or`) are available in MIR at the
extraction stage and are extracted as regular functions." The
construction pipeline must traverse the call graph from authored
callbacks/ISRs/helpers into referenced `core` and facade
functions, extract their monomorphized bodies via
`tcx.optimized_mir(instance)`, and register them in the
constructed `RegseqModel` as regular `Function`s with bodies.

This task lives entirely inside
`crates/regseq-driver/src/construction.rs` (construction-side
work, same rustc invocation). No `MirBodySurface` expansion is
needed -- construction walks rustc MIR directly.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 1 spec
  environment -- `#![no_std]`, only `core` available; Section 8
  type system; Section 9 body ownership)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 8 recognition rule 3 + cross-crate function
  extraction)
- `docs/discussions/models/regseq-model/execution-plan.md`
  (Phase 5.6 cross-crate function extraction)
- `crates/regseq-driver/src/construction.rs` (from tasks 41-46)
- `crates/regseq-model/src/model/function.rs`

## Implementation Notes

### Call-graph walk during Pass 1

Extend Pass 1 (task 41) to walk every `mir::TerminatorKind::Call`
in already-registered function bodies (authored callbacks, ISRs,
helpers) and enqueue referenced **cross-crate instances** for
registration:

1. For each Call, resolve the `Instance` via
   `Instance::resolve(tcx, param_env, callee_def_id,
   generic_args)`.
2. Filter: keep only instances whose `DefId`'s defining crate is
   `core` or the facade crate (`regseq`). Maintain a small
   `ALLOWED_CROSS_CRATES: &[Symbol]` list covering these two
   (extensible later).
3. For each allowed cross-crate instance, compute a stable
   identity (`DefId` + monomorphized substitution); if not
   already registered, assign a `FunctionId` and place the
   Function under a synthetic module path derived from
   `tcx.def_path_str(def_id)` (e.g.,
   `core::option::Option::<Duration>::map` lives under
   synthetic `core::option::Option` ↓ `core::option` ↓ `core`
   chain). Reuse existing synthetic module entries by interned
   name path.
4. Synthetic modules have `kind: Normal` and
   `SourceOrigin::CompilerGenerated` source (the synthetic
   module itself has no user source; the function's span comes
   from `tcx.def_span(def_id)`).
5. Walk transitively: each newly-enqueued function's MIR may
   reference further cross-crate instances. Bound depth at 16;
   raise `ConstructionError::CrossCrateRecursionLimit` if
   exceeded (defensive).

### Disallow std

If a referenced instance's defining crate is `std` (or any
crate not in the allowlist), raise
`ConstructionError::StdFunctionNotPermitted` citing the
`def_path_str` and the authored caller that triggered the
reference.

### Pass 2 body construction (cross-crate)

For each registered cross-crate function, call
`build_body(tcx, def_id, function_id, ...)` (task 43) with
the monomorphized `def_id`. No special-casing in Pass 2. All
recognition rules from tasks 44-52 apply uniformly.

Fetch the monomorphized MIR via
`tcx.instance_mir(instance.def)` (or
`tcx.optimized_mir(def_id)` for simple-polymorphism-free
instances), matching the rustc MIR stage pinned by
`regseq-driver-25`.

### Required cross-crate functions for PL011

After this task lands, these must all be present as `Function`s
with non-empty bodies (exact set depends on PL011's MIR call
graph; test plan validates a baseline):
- `core::option::Option::<T>::map`
- `core::option::Option::<T>::unwrap_or`
- `core::option::Option::<T>::is_some` (if referenced)
- `<FieldValue as core::ops::Add>::add` (facade crate)
- `Mmio::<Block>::modify` (facade crate, monomorphized for
  PL011's block type)

## Non-Goals

- No special handling for closures (deferred per IR design).
- No inlining at construction time -- bodies remain as
  separate `Function`s; backend decides whether to inline.
- No changes to `MirBodySurface` (legacy, Phase 9 deletes).
- No `?`-pattern folding code change (task 46 already covers
  it; it applies to extracted bodies the same as user
  bodies).
- No allowlist beyond `core` and `regseq`.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Compile the PL011 fixture and assert after construction:
  - `Option::<T>::map`, `Option::<T>::unwrap_or`,
    `<FieldValue as Add>::add`, and `Mmio::modify` exist as
    `Function`s with non-empty `Body`s.
  - Each is reachable through synthetic module paths rooted
    at `core` or `regseq`.
  - Each has its `source_locations` entry populated from
    `tcx.def_span`.
- Fixture that references `std::vec::Vec::<u8>::push` in
  authored code; assert
  `ConstructionError::StdFunctionNotPermitted`.
- Fixture generating 17 levels of cross-crate call chain
  (synthetic helpers that call into each other through
  `core`); assert
  `ConstructionError::CrossCrateRecursionLimit`.
- Assert the PL011 `RegseqModel` golden (at
  `tests/fixtures/pl011-full-spec/expected/ir/`) carries the
  expected cross-crate Function entries.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify:
- The call-graph walk uses `Instance::resolve` and
  `tcx.instance_mir` (not string def-path matching).
- Allowlist is restricted to `core` and `regseq`.
- Synthetic module placement is deterministic and reuses
  existing entries by interned name.
- Pass 2 body construction is uniform with authored bodies.
- Both std guard and recursion-limit guard fire.
- Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- All required PL011 cross-crate functions exist as `Function`s
  with non-empty bodies in the constructed model.
- Synthetic module placement is deterministic.
- `std`-crate guard fires.
- Recursion-limit guard fires.
- Pass 2 uses the same `build_body()` pipeline as authored
  bodies.
- Codex review approved.
