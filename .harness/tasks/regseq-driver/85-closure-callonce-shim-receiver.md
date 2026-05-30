# regseq-driver-85: Closure `call_once` shim — by-value receiver vs `Deref` mismatch

## Description

Containing **any** closure in a fixture produces invalid C. The bridge synthesizes
the closure's `Fn` family shims; the `call_once` shim declares its receiver **by
value** (the closure struct), but the synthesized/materialised shim body projects it
through a `PlaceElem::Deref` — so the backend (which faithfully trusts the body's
projections + local types) emits `_3 = (*_1)._0`, dereferencing a by-value struct
local `_1`. `cc` rejects:
`invalid type argument of unary '*' (have struct input_regseq_main__closure_0...)`.
This fires at compile time **even when the closure is never called** (all three `Fn`
impls are reachable from the closure type), so a closure cannot be contained at all.

Surfaced by the §06 push (b3). Blocks **AG-6** (closure aggregate / captures struct)
and **TY-15** (Closure type).

> Re-filed from `regseq-backends-21` (archived): the Codex spec review established
> the defect is the **bridge `call_once` shim synthesis / materialised body**, not a
> backend-only lvalue special-case — the backend correctly emits whatever the body's
> `Deref` projection + local types say.

## Input Context

### Code anchors (confirm at HEAD — per Codex review)

- `crates/regseq-driver/src/bridge/mod.rs:~2624-2635` — declares the `CallOnce`
  receiver **by value**.
- `crates/regseq-driver/src/bridge/body/materialise.rs:~99-111`, `~314-321` —
  materialises the raw closure shim MIR / local decls (where the receiver projection
  + local types are produced; locate the `Deref` on the by-value receiver).
- `crates/regseq-backends/src/generic_c/body_emit.rs:~955-990` — the backend trusts
  `PlaceElem::Deref` + body local types (NOT the bug site; emits faithfully).
- `crates/regseq-driver/src/bridge/synth/closure.rs:~133` — Fn-family shim synthesis
  (all three impls reachable from the closure type).

## Implementation Notes

Reconcile the by-value `call_once` receiver with its body projection: either the
synthesized body must access the by-value receiver **without** a `Deref` (`_1._0`,
matching the by-value `self` declaration), or the receiver local must be declared as
a reference consistently with a `Deref` body. Whichever the rustc-faithful shape is,
the receiver's declared type and the body's projection must AGREE so the emitted C
type-checks. Confirm `call` / `call_mut` (which take `&self` / `&mut self`) remain
correct (their `Deref` is genuine).

## Non-Goals

- No backend change (it emits the body faithfully; fixing the body/decl agreement in
  the bridge is sufficient). If, after the bridge fix, a backend issue remains,
  split it out.
- Not the `ClosureFnPointer` coercion (PC-3 — vtable task).

## Test Plan

1. Build + workspace green; bridge/backend test suites.
2. A `kind=rust` codegen-coverage fixture containing a closure that captures a value
   and is called (e.g. `let cap = 7; let c = |x| x + cap; c(2)`), logging the result,
   compiles-and-runs → AG-6 / TY-15 green; re-run `cargo run -p codegen-coverage-runner`,
   ledger byte-stable.
3. Bridge/backend unit test: the `call_once`-by-value shim body projects the receiver
   consistently with its declared type (the emitted C compiles).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-backends`,
`codegen-coverage-runner`); single-threaded scoped tests; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture containing (and calling) a capturing closure
   compiles-and-runs; AG-6 + TY-15 green in the ledger.
2. The `call_once` receiver decl + body projection agree; `call` / `call_mut`
   unaffected.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
