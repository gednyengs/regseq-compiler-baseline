# regseq-backends-27: Generic-C entry-aware emit API + return-shape classifier (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (tasks **T07+T01**),
> decisions **D08/D09**, contract **C04**, requirements **R03/R05/R09**,
> invariant **I02**. (Resolves Codex round-1 blocking #1.)

## Description

Make the backend the **single source of truth** for the entry shim shape.
Introduce an entry-aware emit API (e.g. `emit_generic_c_for_entry(model,
entry_suffix)`) that resolves the named entry, classifies its return type into
`{ UnitVoid, ErrResult(err_enum_ty), Reject(reason) }`, and **returns the shim
shape (+ adapter symbol name)** alongside the TU. Thread that shape to **both**
consumers (the codegen-coverage runner and `cargo-regseq --emit bin`) so neither
re-derives it. This task also pins the **byte-identical unit-void baseline**
(folded T01): the existing void path must not change.

The adapter *body* and the actual int-shim *emission* land in regseq-backends-28;
this task is the API + classifier + call-site threading (UnitVoid path only, so
everything compiles and existing goldens are unchanged).

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/mod.rs` — `emit_generic_c(model)` (~:151):
  add the entry-aware variant. Resolve the entry by iterating `model.instances`,
  matching the `Function` whose `emission_name` ends with the suffix (the
  `entry_c_symbol` logic), then `model.sigs.resolve(data.fn_sig).output`.
- `crates/regseq-backends/src/generic_c/naming.rs` — `entry_c_symbol` (~:45),
  `generate_main_c` (~:75, currently `(entry: &str)` → void shim). Its signature
  changes to carry the shim shape; **both** call sites move in lock-step.
- `crates/regseq-backends/src/generic_c/mod.rs` — `is_diag_log_instance` (~:1029):
  the pattern for reading a recorded `AttrData` from
  `model.domain_attrs.item_attrs` — reuse it to confirm the `Err` enum is
  `FrameworkType`-marked (from regseq-driver-89).
- `crates/codegen-coverage-runner/src/pipeline.rs` (~:134/:158) — the runner
  resolves `manifest.entry` **after** TU emit; this is *why* the shim shape must be
  returned from the backend rather than re-derived by the harness.
- `crates/codegen-coverage-runner/src/toolchain.rs` (~:62/:81) and
  `crates/regseq-driver/src/emit.rs` (~:88) — the two `generate_main_c` call sites
  to update (UnitVoid path here; error path completed in regseq-driver-90).

## Implementation Notes

- `ErrResult` requires `Result<(), E>` with a `()` Ok payload and `E` an enum
  carrying the `FrameworkType` attr. Classify the `Result` by its **niche
  two-variant layout + `()` Ok payload + enum Err payload**, then the attr check —
  not by name.
- **Reject (fail loud, no panic)**: non-unit `Ok` (`Result<T,_>`, `T != ()`), an
  `Err` enum **not** marked, an entry taking arguments, an ambiguous/absent
  `regseq_main`, and a **manifest-custom entry combined with an error-returning
  signature** (deferred this phase, D09).
- For Phase 2 both consumers pass the default `regseq_main` suffix on the error
  path.

## Non-Goals

- Not the adapter body / symbolic switch / `<errno.h>` (regseq-backends-28).
- Not the `expected.exit` oracle (regseq-backends-29).
- No support for a manifest-custom error-returning entry (rejected, D09).

## Test Plan

1. Classifier unit tests over hand-built models (`mod_tests.rs` pattern): each of
   `UnitVoid` / `ErrResult` / every `Reject` shape, asserting the reject diagnostic
   names the shape.
2. Unit-void returns the void shim shape **unchanged**; both call sites compile.
3. Golden corpus + `scalar_function_emits_prototype_and_body`-style regressions:
   every existing golden byte-identical (**I02/R05**).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`,
`codegen-coverage-runner`, `regseq-driver`); `RUST_TEST_THREADS=1 cargo test
-p regseq-backends -p codegen-coverage-runner -p regseq-driver -- --test-threads=1`;
ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. Entry-aware API resolves + classifies the entry and **returns** the shim shape;
   both consumers consume it (no re-derivation) (**C04**, **R03**).
2. Every unsupported shape — incl. custom-entry+error-return — **fails loud at
   emit** with a shape-naming diagnostic (**D08/D09/R09**).
3. Unit-void emission is **byte-identical**; existing goldens unchanged
   (**I02/R05**).
4. fmt + scoped clippy clean; scoped tests green; ledger byte-stable.
