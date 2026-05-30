# regseq-backends-19: Generic-C `BinOp::Cmp` → `Ordering` destination type fix (Phase-1)

## Description

`BinOp::Cmp` (three-way compare, matrix row **BO-25**) emits a bare `int8_t`
expression but its destination place is typed `core::cmp::Ordering` — a fieldless
enum the backend lowers to `struct core_cmp_Ordering`. The C assignment therefore
fails to compile:

```
error: incompatible types when assigning to type 'struct core_cmp_Ordering'
       from type 'signed char'
  _72 = ((int8_t)(((7) > (3)) - ((7) < (3))));
```

Found by the §06 codegen-coverage B1 pilot (`b1_int_scalars`), which greens the
other 39 B1 rows and defers BO-25 pending this fix. The construct lowers fine
through the bridge (`core::intrinsics::three_way_compare` → `BinOp::Cmp` via
`LowerIntrinsics`); only the C **emission** is wrong: the scalar comparison result
is not materialized into the `Ordering` enum's C representation.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/body_emit.rs:1543` — the offending
  `BinOp::Cmp => Ok(format!("((int8_t)((({x}) > ({y})) - (({x}) < ({y}))))"))`
  nested-rvalue spelling. It produces `int8_t`, valid only when the destination
  is `int8_t` — but `Cmp`'s MIR result type is always `core::cmp::Ordering`.
- `crates/regseq-backends/src/generic_c/body_emit.rs:585`–`612` /
  `assign_with_overflow` — the precedent: `*WithOverflow` is **not** a nested
  rvalue; it is materialized at the **assign site** into its `(T, bool)`
  result type. `Cmp` needs the analogous treatment for its `Ordering` result.
- The fieldless-enum emission + tag/discriminant write path (enum_emit.rs +
  the `SetDiscriminant` / `AggregateKind::Enum` / `Const::EnumValue` handlers in
  body_emit.rs) — the source of truth for how an `Ordering` value is spelled in
  C (its `struct core_cmp_Ordering` shape and how a discriminant `-1/0/1` is
  written into it). Reuse it; do not invent a second Ordering spelling.

### Prior context

- `Ordering` is `#[repr(i8)]` with `Less = -1`, `Equal = 0`, `Greater = 1`.
- The §06 plan records this as `§4b-bis` (backend-bug-blocked). Memory:
  [[codegen-fixture-no-core-methods]].

## Implementation Notes

`BinOp::Cmp` must materialize its three-way result **into the `Ordering` C
representation** at the assignment site, not as a bare `int8_t` nested rvalue —
mirroring `assign_with_overflow`. The scalar `(x>y) - (x<y)` (an `int8_t` in
`{-1,0,1}`) is the discriminant value; write it into the destination
`Ordering` place using the same construction the backend already uses for a
fieldless-enum discriminant write (so the emitted form matches
`enum_emit`'s `struct core_cmp_Ordering`). Keep the nested-rvalue arm
(`body_emit.rs:1543`) only if some context still needs an `int8_t` Cmp; if Cmp
is always assign-site (like `*WithOverflow`), make the nested arm `Unsupported`
(defensive) and route Cmp through the assign-site path.

## Non-Goals

- No change to `BinOp::Cmp` recognition in the bridge (`rvalue.rs` already maps
  it correctly).
- No change to any other BinOp or to the `Ordering`/fieldless-enum C shape
  itself — only how a `Cmp` result is stored into an `Ordering` place.
- Not the `*WithOverflow` path (already correct).

## Test Plan

1. **Build + workspace green** (scoped: `regseq-backends`,
   `codegen-coverage-runner`).
2. **End-to-end compile-AND-run** via a `kind=rust` codegen-coverage fixture
   that drives `BinOp::Cmp` and logs the result: `core::intrinsics::three_way_compare(a,b)`
   for `a>b`, `a==b`, `a<b`, observed as `as i8` → stdout `1`, `0`, `-1`. This is
   the BO-25 green fixture (extend `b1_int_scalars` with the deferred `cmp=` line
   + add `BO-25` to its `covers`, or a small dedicated fixture). Re-run
   `cargo run -p codegen-coverage-runner` → BO-25 flips green, ledger byte-stable.
3. **Backend unit test** (`mod_tests.rs` hand-built model, mirroring the existing
   `AddWithOverflow` test at `mod_tests.rs:2269`): a `Cmp` assigned to an
   `Ordering` place emits valid C that compiles.

## Review

Files to send to Codex: the `body_emit.rs` Cmp/assign-site diff + the new/edited
fixture + the unit test. Reviewer focus: the emitted `Ordering` form matches
`enum_emit`'s `struct core_cmp_Ordering` exactly (no second spelling); the
`{-1,0,1}` discriminant mapping is correct; no regression to `*WithOverflow` or
the comparison BinOps.

## Quality Gate

`cargo fmt --all -- --check`; `cargo clippy -p regseq-backends -p codegen-coverage-runner
--all-targets -- -D warnings` (scoped — [[clippy-all-targets-test-drift]]);
`RUST_TEST_THREADS=1 cargo test -p regseq-backends -p codegen-coverage-runner --
--test-threads=1`; `cargo run -p codegen-coverage-runner` byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture driving `BinOp::Cmp` compiles-and-runs and logs the
   correct `Ordering` (`1`/`0`/`-1`) — BO-25 green in the ledger.
2. The emitted `Ordering` value uses the backend's existing fieldless-enum C
   representation (no second spelling).
3. Backend unit test covering `Cmp`-into-`Ordering` emission passes.
4. fmt + clippy (scoped) clean; full scoped test suite green; ledger byte-stable.
