# regseq-driver-63: implementation evidence

Implementation deliverable for `regseq-driver-63`, the F0 typed
`BridgeOutcome` helper described at
`.harness/tasks/regseq-driver/63-typed-bridge-outcome-helper.md`
and the bridge-coverage-runner strategy at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 6.

Parent context: none. This task is F0 of the bridge-coverage-runner
strategy; no prior implementation gap is closed by it. F1+ runner
work consumes the typed surface this task lands.

## Scope

Helper-only edits confined to `crates/test-ir-fixtures/src/lib.rs`
plus a new integration test file
`crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`. No bridge
change, no model change, no spec change, no fixture change.

## Before-fix evidence

- Pre-fix `crates/test-ir-fixtures/src/lib.rs:90` exposed only
  `bridge_fixture(name: &str) -> RegseqModel`, which panicked on
  every non-OK outcome.
- Pre-fix `crates/test-ir-fixtures/src/lib.rs:228-231` converted
  `BridgeError -> String` BEFORE surfacing through the Mutex:
  `Err(format!("{err}"))`. Typed-rejection information was lost.
- Pre-fix `crates/test-ir-fixtures/src/lib.rs:247-263` panicked
  on three distinct non-OK shapes (`rustc never reached
  after_analysis` / `bridge returned Err` / `validate returned
  Err`) inside the private `compile_and_bridge` workhorse.

## After-fix evidence

### Change A: typed callback result type lift

- `crates/test-ir-fixtures/src/lib.rs:325` and
  `crates/test-ir-fixtures/src/lib.rs:328` now both type the
  Mutex as `Arc<Mutex<Option<Result<RegseqModel, BridgeError>>>>`
  (the local `result` binding and the `Callbacks { result }`
  field). Downstream `result.lock()` / `result.clone()` /
  `result.lock().take()` sites at
  `crates/test-ir-fixtures/src/lib.rs:354`, `:360`, and `:364`
  consume the lifted type unchanged.
- `crates/test-ir-fixtures/src/lib.rs:353` now stores the raw
  `run_bridge(input)` Result directly (`let outcome =
  run_bridge(input);`) without a `format!` conversion; the typed
  `BridgeError` survives the Mutex.
- `BridgeError` is imported from `regseq_driver::bridge` at
  `crates/test-ir-fixtures/src/lib.rs:29` alongside the existing
  `run_bridge` import. No new crate dependency (test-ir-fixtures
  already depends on `regseq-driver`).

### Change B: typed entry points + `sanitize_crate_name`

- `crates/test-ir-fixtures/src/lib.rs:32-72` defines
  `pub enum BridgeOutcome` with the four variants
  `Ok(RegseqModel)`, `ValidatorFailed { model, violation }`,
  `Rejected(BridgeError)`, `RustcRejected { stderr: String }`.
  Shape matches strategy section 6.1 byte-for-byte.
- `crates/test-ir-fixtures/src/lib.rs:156-172` defines
  `pub fn try_bridge_fixture(name: &str) -> BridgeOutcome`. It
  panics on caller-bug input (unknown fixture name / non-file
  path) with the same wording the pre-fix helper used; bridge
  outcomes surface as `BridgeOutcome` variants.
- `crates/test-ir-fixtures/src/lib.rs:183-190` defines
  `pub fn try_bridge_fixture_path(path: &Path) -> BridgeOutcome`.
  It derives a diagnostic name from the file stem and a
  rustc-valid synthetic crate name from the same stem via
  `sanitize_crate_name`.
- `crates/test-ir-fixtures/src/lib.rs:197-215` defines
  `pub(crate) fn sanitize_crate_name(stem: &str) -> String` per
  spec Change B and Non-Goals. F0 ships the helper as
  `pub(crate)`; its three coverage assertions (spec Change D
  item 4) live in the lib's `#[cfg(test)] mod tests` block at
  `crates/test-ir-fixtures/src/lib.rs:467-489` where they have
  visibility into the `pub(crate)` surface. F1 may lift to `pub`
  when the runner asks.
- `InvariantViolation` is imported from `regseq_model` at
  `crates/test-ir-fixtures/src/lib.rs:30`. Re-export confirmed at
  `crates/regseq-model/src/lib.rs:87`.

### Change C: `bridge_fixture` rewired

- `crates/test-ir-fixtures/src/lib.rs:132-149` rewrites
  `bridge_fixture(name) -> RegseqModel` as a thin wrapper over
  `try_bridge_fixture(name)` that panics on every non-Ok variant.
- The four panic shapes preserve the exact wording from the
  pre-fix helper (pre-fix anchors are against
  `git stash`-tested HEAD prior to this commit):
  - `ValidatorFailed` -> "bridge produced invalid RegseqModel for
    fixture {name:?}: {violation:?}" (was at pre-fix
    `lib.rs:263`; now at `lib.rs:136`).
  - `Rejected` -> "bridge failed on fixture {name:?}: {err}" (was
    at pre-fix `lib.rs:257`; now at `lib.rs:139`; equivalent to
    the pre-fix `format!("{err}")` rendering since `{err}` uses
    Display).
  - `RustcRejected` -> "bridge produced no result for fixture
    {name:?}: rustc invocation completed without reaching
    after_analysis" (was at pre-fix `lib.rs:248-249`; now at
    `lib.rs:143-144`).
  - Unknown fixture name -> "fixture {name:?} is not shipped by
    test-ir-fixtures (expected at {path}): {err}" (was at
    pre-fix `lib.rs:93-96`; now at `lib.rs:158-163`, inside
    `try_bridge_fixture` which `bridge_fixture` defers to).

### Change D: typed-outcome smoke test (new file)

- New file `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`
  carries six `#[test]` functions covering spec Change D items
  1, 2, 3, 5, 6, 7 (the items that exercise the public
  `BridgeOutcome` / `try_bridge_fixture` / `try_bridge_fixture_path`
  / `bridge_fixture` surface). Item 4's three
  `sanitize_crate_name` assertions live in the lib's unit-test
  module instead (see below) because that helper is
  `pub(crate)` per spec Non-Goals. Coverage:
  1. `try_bridge_fixture_returns_ok_on_no_op` (item 1)
  2. `try_bridge_fixture_path_returns_ok_on_no_op_path` (item 2)
  3. `try_bridge_fixture_path_lifts_hyphenated_stem` -- writes a
     tempfile `TY-001.rs` with the no_op source, asserts the
     `BridgeOutcome::Ok(_)` shape (item 3, load-bearing for F1's
     snippet naming).
  4. `try_bridge_fixture_panics_on_unknown_fixture_name` -- asserts
     the panic message names the missing fixture AND contains the
     "is not shipped" wording (item 5).
  5. `bridge_fixture_still_returns_model_on_no_op` (item 6).
  6. `bridge_fixture_panics_on_unknown_fixture_with_descriptive_message`
     -- mirrors test 4 against the rewired
     `bridge_fixture` (item 7).
- `sanitize_crate_name` coverage (item 4) lives in
  `crates/test-ir-fixtures/src/lib.rs:467-489` as three
  `#[cfg(test)]` unit tests:
  `sanitize_crate_name_rewrites_hyphen` (TY-001 -> TY_001),
  `..._prepends_s_on_digit_leading` (123foo -> s_123foo), and
  `..._idempotent_on_valid_name` (no_op -> no_op). The unit-test
  module has visibility into the `pub(crate)` helper without
  forcing it onto the public surface.
- All six integration tests and three additional unit tests pass
  under `cargo test -p test-ir-fixtures -- --test-threads=1`.

### Test evidence

- `cargo test --workspace -- --test-threads=1` is green. Specific
  result lines (excerpted):
  - `test-ir-fixtures` unit tests: 5 passed (2 pre-existing
    plus the 3 new `sanitize_crate_name` unit tests).
  - `test-ir-fixtures::no_op_smoke`: 1 passed (B1 unchanged).
  - `test-ir-fixtures::typed_outcome_smoke`: 6 passed (B2).
  - All existing e2e tests under `regseq-model/tests/` and
    `regseq-driver/tests/` that call `bridge_fixture` (e.g.,
    `e2e_enum_round_trip`, `e2e_synthesis`,
    `bridge_closure_apply_indirection`,
    `bridge_drop_glue_sig_sync`) pass unchanged.
- The hyphenated-stem rustc compile emits a `non_snake_case`
  warning on the `TY_001` crate name; this is rustc's lint about
  the compiled fixture source, not a clippy / fmt failure on our
  code. Test still passes.

## Caller audit (B3)

`grep -rn 'bridge_fixture\b' crates/ tests/ --include='*.rs' | wc -l`:

- Pre-rewire (against `git stash`): 42 occurrences.
- Post-rewire (current HEAD + working tree): 46 occurrences.

Delta = +4 occurrences, all introduced by this task:

- 2 doc comment references to `[bridge_fixture]` in the new
  `try_bridge_fixture` / `BridgeOutcome` rustdoc at
  `crates/test-ir-fixtures/src/lib.rs:43` and
  `crates/test-ir-fixtures/src/lib.rs:152`.
- 2 new `bridge_fixture(...)` call sites in
  `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs:110`
  and `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs:117`
  (the backwards-compat Change-C-property tests).

No existing caller was removed or rewired to fallible semantics.
Every pre-existing caller (e.g.,
`crates/regseq-model/tests/e2e_enum_round_trip.rs:34`,
`crates/test-ir-fixtures/tests/no_op_smoke.rs:15`,
`crates/regseq-driver/tests/bridge_drop_glue_sig_sync.rs:73`)
continues to expect `RegseqModel` or a panic, and continues to
get exactly that under the rewire.

## Panic-message parity (B4)

Verified by source-level comparison between pre-fix
`crates/test-ir-fixtures/src/lib.rs:92-103` + `:247-263` (pre-fix
HEAD, available via `git stash`) and post-fix
`crates/test-ir-fixtures/src/lib.rs:132-149` + `:156-172`:

- Unknown fixture name -> "fixture {name:?} is not shipped by
  test-ir-fixtures (expected at {path}): {err}". Identical
  format string and arg shape.
- Non-file path -> "fixture {name:?} entry at {path} is not a
  regular file". Identical format string and arg shape.
- Rustc-no-after-analysis -> "bridge produced no result for
  fixture {name:?}: rustc invocation completed without reaching
  after_analysis". Identical format string.
- Bridge-error -> "bridge failed on fixture {name:?}: {err}".
  Pre-fix used `format!("{err}")` to convert `BridgeError -> String`
  inside the callback, then panic with "...: {message}"; post-fix
  uses `{err}` directly on the `BridgeError`. Both render via
  `BridgeError`'s `thiserror`-derived `Display` (no `format!`
  wrapper changes the output). Byte-identical.
- Validator-failure -> "bridge produced invalid RegseqModel for
  fixture {name:?}: {violation:?}". Identical format string and
  Debug-rendering of `InvariantViolation`.

The two reachable-from-tests panic shapes (unknown-fixture-name)
are also exercised at runtime by:
- `crates/test-ir-fixtures/src/lib.rs:445-465` (the pre-existing
  unit test `bridge_fixture_unknown_name_panics_with_descriptive_message`,
  unchanged after rewire).
- `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs:88-104`
  (`try_bridge_fixture` panic).
- `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs:114-130`
  (`bridge_fixture` panic, backwards-compat).

The three internal panic shapes (rustc-no-after-analysis,
bridge-error, validator-failure) are not reachable from any
committed fixture, per the task spec's note that negative-path
coverage is an F1 runner concern. B4 parity for those shapes is
verified by source-level review only, as the spec permits.

## Out of scope (per task Non-Goals)

- No `crates/bridge-coverage-runner/` scaffolding (F1).
- No subprocess-based rustc stderr capture for the
  `RustcRejected.stderr` field (F1 runner concern per strategy
  section 6.3); the helper ships `String::new()`.
- No `catch_unwind` around the bridge invocation (F1 runner
  concern per strategy section 6.2).
- No fixture changes under `crates/test-ir-fixtures/fixtures/`.
- No spec.md / pretty-printer.md / generic-c-backend.md changes.
- No `bridge_fixture` signature change (preserved
  byte-identically).
- No negative-path smoke tests for `BridgeOutcome::Rejected` /
  `ValidatorFailed` / `RustcRejected` (F1 + matrix snippets).
- No richer public library surface beyond the two typed entry
  points; `sanitize_crate_name` remains `pub(crate)` per spec
  (strategy section 3 defers a richer public API until F5+).
  Its three coverage assertions live in the lib's
  `#[cfg(test)] mod tests` block at
  `crates/test-ir-fixtures/src/lib.rs:467-489`, where unit-test
  visibility into the `pub(crate)` helper is preserved without
  widening the public surface.

## F1 readiness

The typed surface introduced by this task is operationally
sufficient for the bridge-coverage runner's `oracle.rs` module
(strategy section 3 architecture):

- `BridgeOutcome::Ok(model)` -> classify as expectation-pass and
  optionally feed `model` to inspection predicates (strategy
  section 7).
- `BridgeOutcome::Rejected(err)` -> classify by `BridgeError`
  variant + cluster by row-id-aware fingerprint (strategy
  section 8).
- `BridgeOutcome::ValidatorFailed { violation, .. }` -> classify
  by `InvariantViolation` variant + cluster the same way.
- `BridgeOutcome::RustcRejected { stderr }` -> classify against
  the rustc taxonomy (strategy section 6.3). F1 fills the
  `stderr` field via the subprocess pre-flight; this helper
  ships `String::new()`.

The path-based `try_bridge_fixture_path` accepts the runner's
hyphenated-row-id snippet stems (`TY-001.rs`, `RV-006-A.rs`,
`OP-003-G-1.rs`) via `sanitize_crate_name`; the load-bearing
hyphenated-stem assertion in the smoke test locks this contract.

No additional `test-ir-fixtures` change is required for F1 to
begin runner-crate scaffolding.
