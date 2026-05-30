# regseq-backends-21: Generic-C closure `call_once` by-value shim deref (Phase-1)

## Description

Containing **any** closure in a fixture fails to emit valid C. The bridge
synthesizes the closure's `Fn` family shims (call / call_mut / call_once); the
backend emits the `call_once`-by-value shim body as `_3 = (*_1)._0`, dereferencing
the by-value closure receiver `_1` — which is a `struct ...closure_0...`, NOT a
pointer. `cc` rejects:
`invalid type argument of unary '*' (have struct input_regseq_main__closure_0...)`.
This fires at emit/compile time **even when the closure is never called** (all
three `Fn` impls are reachable from the closure type), so a closure cannot be
contained at all.

Surfaced by the §06 push (b3). Blocks **AG-6** (closure aggregate / captures
struct) and **TY-15** (Closure type).

## Input Context

### Code anchors (confirm at HEAD — per workflow diagnosis)

- `crates/regseq-driver/src/bridge/synth/closure.rs:~133` — synthesizes the
  call/call_mut/call_once shims for an `Fn` closure (so all three are reachable
  from the closure type itself).
- The backend `call_once`-by-value shim body emission — the `(*_1)._0` deref of the
  by-value receiver. Find where the shim receiver is treated as a pointer; for a
  **by-value** `self` the receiver is the struct directly (`_1._0`, no deref). Grep
  the closure-shim / `ClosureOnceShim` body synthesis in
  `crates/regseq-backends/src/generic_c/`.

## Implementation Notes

The `call_once` shim takes `self` **by value** (the closure struct), not by
reference — so the field access must be `_1._0`, not `(*_1)._0`. Fix the shim-body
emission to project the by-value receiver without a deref (distinguish by-value vs
by-ref receiver per the shim flavor / signature). Confirm `call`/`call_mut` (which
DO take `&self`/`&mut self`) still deref correctly.

## Non-Goals

- Not the `ClosureFnPointer` coercion (PC-3 — vtable task).
- Not closure capture-by-move semantics beyond what makes the shim body valid C.

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture containing a closure that captures a
   value and is called (e.g. `let c = |x| x + cap; c(2)`), logging the result,
   compiles-and-runs → AG-6 / TY-15 green; re-run the runner, ledger byte-stable.
3. Backend unit test: a `call_once`-by-value shim body emits `_1._0` (no deref).

## Quality Gate

`cargo fmt --all -- --check`; `cargo clippy -p regseq-backends -p codegen-coverage-runner
--all-targets -- -D warnings` (scoped); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture containing (and calling) a capturing closure
   compiles-and-runs; AG-6 + TY-15 green in the ledger.
2. `call`/`call_mut`/`call_once` shim receivers each project correctly (by-ref
   deref vs by-value direct).
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
