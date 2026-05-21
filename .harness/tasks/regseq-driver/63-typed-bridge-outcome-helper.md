# regseq-driver-63: Typed `BridgeOutcome` helper in `test-ir-fixtures` (F0 of bridge-coverage-runner)

## Description

Introduce a typed-outcome path through the `test-ir-fixtures`
fixture helper so that downstream tooling (specifically the
bridge-coverage-runner per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`,
phase F0) can classify a fixture compile + bridge + validate sweep
into one of four named outcome classes WITHOUT parsing panic
messages or error strings.

Today the helper at
`crates/test-ir-fixtures/src/lib.rs:90-99` exposes only
`bridge_fixture(name: &str) -> RegseqModel` which panics on every
non-OK outcome. The rustc-driver `Callbacks::after_analysis`
implementation at
`crates/test-ir-fixtures/src/lib.rs:228-231` converts `BridgeError`
to `String` BEFORE surfacing through the Mutex, so the typed
rejection information is lost. Subsequent code at
`crates/test-ir-fixtures/src/lib.rs:247-263` panics on:

- rustc never reaching `after_analysis`
- bridge returning `Err(BridgeError)`
- `model.validate()` returning `Err(InvariantViolation)`

This task adds a typed `BridgeOutcome` enum + two typed entry-point
helpers (`try_bridge_fixture(name)` and `try_bridge_fixture_path(path)`)
that distinguish those four outcome classes, plus an internal
callback result-type lift so the typed information survives the
Mutex. The existing `bridge_fixture(name) -> RegseqModel` API is
rewired to call the typed path and panic on every non-OK variant,
preserving backwards compatibility for the ~12 existing callers
that expect either `RegseqModel` or a panic.

Scope is intentionally narrow: this is the F0 prerequisite for
the bridge-coverage-runner strategy. F0 does NOT scaffold any
runner crate, does NOT add snippet authoring, does NOT add
fingerprint clustering, does NOT add reporting. Those are F1+
deliverables.

The task is a focused production-code change confined to
`crates/test-ir-fixtures/src/lib.rs` plus its smoke test surface.
No spec amendment, no schema change, no validator change, no
bridge change.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  §6 "Typed-outcome oracle" (Codex-approved v3 at HEAD). The
  strategy doc spells out the `BridgeOutcome` enum shape, the
  two-layered Change A (internal callback lift) + Change B (public
  sibling helpers) design, the rustc-ICE / no-after-analysis
  taxonomy (§6.3), and the panic-capture boundary (§6.2: the
  helper does NOT catch panics; that's the runner's job via
  `catch_unwind`).

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §3 (`RegseqModel`,
  `spec.md:204`) -- the model type that `BridgeOutcome::Ok` and
  `BridgeOutcome::ValidatorFailed` carry.
- `docs/discussions/regseq-model/spec.md` §12 step 5 validation
  invariants (`spec.md:742-758`) -- the validator surface that
  `BridgeOutcome::ValidatorFailed` exposes through
  `InvariantViolation`.

### Bridge code anchors (this task's edit surface)

- `crates/test-ir-fixtures/src/lib.rs:75-105` --
  `bridge_fixture(name) -> RegseqModel`. Rewired in Change C to
  call `try_bridge_fixture(name)` and panic on every non-OK
  variant with descriptive messages preserving the current panic
  wording.
- `crates/test-ir-fixtures/src/lib.rs:161-266` --
  `compile_and_bridge(name, src) -> RegseqModel`. The internal
  workhorse. Change A lifts its callback Result type +
  `compile_and_bridge` itself becomes the typed
  `try_compile_and_bridge(name, src) -> BridgeOutcome`. The
  existing `compile_and_bridge` becomes a thin wrapper that
  panics on non-OK if it is still needed (or is removed entirely
  if `bridge_fixture` is the only caller -- verify via grep).
- `crates/test-ir-fixtures/src/lib.rs:228-231` -- the rustc
  callback's `BridgeError -> String` conversion at line 230. Change
  A replaces `Err(format!("{err}"))` with `Err(err)`. The Mutex
  becomes `Option<Result<RegseqModel, BridgeError>>`.
- `crates/test-ir-fixtures/src/lib.rs:242-250` -- the
  "rustc invocation completed without reaching after_analysis"
  panic. Change B converts this to
  `BridgeOutcome::RustcRejected { stderr: String::new() }`
  (F0 ships with an EMPTY stderr; subprocess-based stderr capture
  is an F1 runner concern per strategy §6.3 and is OUT OF SCOPE
  for F0).

### Existing-caller audit

Before rewiring `bridge_fixture`, confirm every existing caller
expects panic semantics on non-OK. Grep across the workspace:

```bash
grep -rn 'bridge_fixture\b' crates/ tests/ --include='*.rs' | head
```

Codex round-1 of the strategy confirmed the audit landed clean
(all observed callers expect `RegseqModel` or panic; no caller
treats `bridge_fixture` as fallible -- e.g.,
`crates/test-ir-fixtures/tests/no_op_smoke.rs:15`). The
implementer re-confirms against HEAD before merging.

### Test surface

- `crates/test-ir-fixtures/tests/no_op_smoke.rs` -- existing smoke
  test. Stays green; `bridge_fixture` behavior is unchanged on
  the OK path.
- New smoke test for the typed path (B2 below).

### Prior tasks

- regseq-driver-62 (closed at `3748f47`): the most recent close;
  chronological dependency only. No code-conflict dependency.
- Bridge-coverage-runner strategy (committed at `4012635`):
  the F0-F5 plan this task implements F0 of.
- Bridge-coverage-matrix (committed at `4012635`): the matrix the
  runner (F1+) will eventually drive. Not directly relevant to
  F0 but cited for traceability.

## Implementation Notes

### Change A: lift callback's internal result type to typed `BridgeError`

**Anchor**: `crates/test-ir-fixtures/src/lib.rs:228-231`.

Current code:

```rust
fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
) -> rustc_driver::Compilation {
    let input = DriverBridgeInput::new(tcx);
    let outcome = match run_bridge(input) {
        Ok(model) => Ok(model),
        Err(err) => Err(format!("{err}")),
    };
    *self.result.lock().expect("result mutex poisoned") = Some(outcome);
    rustc_driver::Compilation::Stop
}
```

Replace `Err(format!("{err}"))` with `Err(err)` so the typed
`BridgeError` survives the Mutex. The `result` field's type
changes from
`Arc<Mutex<Option<Result<RegseqModel, String>>>>` to
`Arc<Mutex<Option<Result<RegseqModel, BridgeError>>>>`.

`BridgeError` is `pub` at
`crates/regseq-driver/src/bridge/mod.rs:376`; the
`test-ir-fixtures` crate already depends on `regseq-driver` per
`crates/test-ir-fixtures/Cargo.toml`. No new dependency.

### Change B: introduce `BridgeOutcome` + typed entry-point helpers

**Anchor**: new code in `crates/test-ir-fixtures/src/lib.rs` (place
adjacent to `bridge_fixture` at line 90, with the enum exported at
the top alongside `pub use ...` statements).

```rust
/// Typed result of a fixture compile + bridge + validate sweep.
///
/// `Ok` means BOTH bridge AND `model.validate()` succeeded.
/// Distinct rejection / validator-failure / rustc-rejected
/// variants are exposed so downstream tooling can classify
/// outcomes without parsing error strings.
///
/// This is the typed surface introduced by regseq-driver-63 (F0
/// of the bridge-coverage-runner strategy). Existing callers
/// continue to use the legacy panicking `bridge_fixture` helper.
#[derive(Debug)]
pub enum BridgeOutcome {
    /// Bridge succeeded AND `model.validate()` succeeded.
    Ok(RegseqModel),
    /// Bridge succeeded but `model.validate()` failed. The model
    /// is returned alongside the violation so the caller can
    /// pretty-print or inspect.
    ValidatorFailed {
        model: RegseqModel,
        violation: InvariantViolation,
    },
    /// Bridge returned a typed `BridgeError` (e.g.,
    /// `DslRejection`, `NonRigidTy`, `Phase4`, ...).
    Rejected(BridgeError),
    /// Rustc exited before `after_analysis` was reached -- either
    /// rustc rejected the input or the compile produced no result
    /// for an unknown reason. `stderr` is the rustc subprocess
    /// stderr, captured by the bridge-coverage runner. In this
    /// F0 helper, `stderr` is always the empty string; the
    /// runner adds subprocess-based stderr capture in F1
    /// (per strategy doc §6.3).
    RustcRejected { stderr: String },
}

/// Typed entry point for fixtures already shipped under
/// `crates/test-ir-fixtures/fixtures/<name>.rs`.
///
/// Mirrors `bridge_fixture(name)` but returns a `BridgeOutcome`
/// instead of panicking on non-OK paths. Panics ONLY when the
/// fixture name is unknown or the fixture file is unreadable
/// (those are caller-bugs, not bridge outcomes).
pub fn try_bridge_fixture(name: &str) -> BridgeOutcome {
    let path = fixture_path(name);
    // ... fixture-existence checks (same shape as bridge_fixture
    // at lines 91-103) panic on missing fixture or non-file path.
    try_compile_and_bridge(name, &path)
}

/// Typed entry point for arbitrary source paths. Used by the
/// bridge-coverage runner to drive snippets that do NOT live in
/// the `test-ir-fixtures/fixtures/` directory.
pub fn try_bridge_fixture_path(path: &std::path::Path) -> BridgeOutcome {
    // Derive a diagnostic name from the file stem, and a
    // rustc-valid SYNTHETIC CRATE NAME by sanitizing the stem.
    //
    // The runner's snippet naming convention uses hyphenated
    // row IDs (e.g., `TY-001.rs`, `RV-006-A.rs`,
    // `OP-003-G-1.rs` per the runner-strategy doc §3 and the
    // matrix's row-ID format). rustc's `--crate-name` rejects
    // hyphens (see `rustc_session::config::validate_crate_name`),
    // so the helper MUST NOT pass the raw stem through to
    // `--crate-name` (which the current `compile_and_bridge`
    // does at `crates/test-ir-fixtures/src/lib.rs:181-182`).
    let diagnostic_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("<anonymous>");
    let crate_name = sanitize_crate_name(diagnostic_name);
    try_compile_and_bridge_with_crate_name(diagnostic_name, &crate_name, path)
}

/// Map a free-form fixture stem to a rustc-valid crate name by
/// replacing every non-(ascii-alphanumeric-or-underscore) byte
/// with `_`, and prepending `s_` if the result starts with a
/// digit. Idempotent on already-valid names.
fn sanitize_crate_name(stem: &str) -> String {
    let mut out: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if out.is_empty() {
        return "snippet".to_string();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "s_");
    }
    out
}
```

`try_bridge_fixture(name)` continues to pass the existing
fixture name directly to `--crate-name` (Change B preserves the
current behavior for the fixture-name path -- existing fixtures
under `crates/test-ir-fixtures/fixtures/` use rustc-valid stems
like `no_op`, `enum_round_trip`, etc., so no sanitization is
needed for that path).

`try_compile_and_bridge(name, src) -> BridgeOutcome` is the
internal workhorse adapted from the existing
`compile_and_bridge(name, src) -> RegseqModel`, with a
crate-name parameter split out so the path-based entry point can
pass a sanitized crate name without losing the diagnostic name.
For `try_bridge_fixture(name)` the diagnostic name and the
crate name are the same (existing fixtures use rustc-valid stems).
For `try_bridge_fixture_path(path)` the crate name is the
sanitized stem and the diagnostic name is the raw stem. Outcome
classification:

```rust
fn try_compile_and_bridge(name: &str, src: &std::path::Path) -> BridgeOutcome {
    // Convenience wrapper for the fixture path -- the existing
    // `compile_and_bridge` callers pass the same string for both.
    try_compile_and_bridge_with_crate_name(name, name, src)
}

fn try_compile_and_bridge_with_crate_name(
    diagnostic_name: &str,
    crate_name: &str,
    src: &std::path::Path,
) -> BridgeOutcome {
    // ... rustc setup identical to existing compile_and_bridge
    //     at crates/test-ir-fixtures/src/lib.rs:161-240,
    //     except (a) the Mutex type is
    //     `Option<Result<RegseqModel, BridgeError>>` per
    //     Change A, AND (b) the `--crate-name` argument at
    //     `crates/test-ir-fixtures/src/lib.rs:181-182` is sourced
    //     from `crate_name`, while existing diagnostic panic
    //     messages continue to use `diagnostic_name`.

    let outcome = result.lock().expect("result mutex poisoned").take();
    let _ = std::fs::remove_dir_all(&out_dir);

    let mut model = match outcome {
        None => {
            // rustc completed without reaching after_analysis.
            return BridgeOutcome::RustcRejected {
                stderr: String::new(),
            };
        }
        Some(Err(err)) => {
            // bridge returned a typed BridgeError.
            return BridgeOutcome::Rejected(err);
        }
        Some(Ok(model)) => model,
    };

    let bridge_input = regseq_model::BridgeInput::matching(&model);
    match model.validate(&bridge_input) {
        Ok(()) => BridgeOutcome::Ok(model),
        Err(violation) => BridgeOutcome::ValidatorFailed { model, violation },
    }
}
```

`InvariantViolation` is `pub` at
`crates/regseq-model/src/model.rs:163`; the import is added
alongside the existing `regseq_model::` imports.

### Change C: rewire `bridge_fixture` to call the typed path

**Anchor**: `crates/test-ir-fixtures/src/lib.rs:90-105`.

Replace the body of `bridge_fixture(name)` with:

```rust
pub fn bridge_fixture(name: &str) -> RegseqModel {
    match try_bridge_fixture(name) {
        BridgeOutcome::Ok(model) => model,
        BridgeOutcome::ValidatorFailed { violation, .. } => {
            panic!("bridge produced invalid RegseqModel for fixture {name:?}: {violation:?}");
        }
        BridgeOutcome::Rejected(err) => {
            panic!("bridge failed on fixture {name:?}: {err}");
        }
        BridgeOutcome::RustcRejected { .. } => {
            panic!(
                "bridge produced no result for fixture {name:?}: rustc invocation completed without reaching after_analysis"
            );
        }
    }
}
```

The four panic messages preserve the exact phrasing the current
helper uses at `crates/test-ir-fixtures/src/lib.rs:247-263`. This
is the load-bearing backwards-compatibility property: existing
integration tests that match on panic messages (none observed in
the audit, but conservatively preserved) see byte-identical
output.

The `bridge_fixture` no-args fixture-existence checks (the two
panics at `crates/test-ir-fixtures/src/lib.rs:92-103` for unknown
fixture / non-file path) move into `try_bridge_fixture` per
Change B's contract -- both helpers panic on those caller-bugs
(NOT bridge outcomes).

### Change D: smoke test for the typed surface

**New file**:
`crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`

Smoke tests:

1. `try_bridge_fixture("no_op")` returns `BridgeOutcome::Ok(model)`
   (the no_op fixture is trivially-bridged; existing smoke
   coverage at `crates/test-ir-fixtures/tests/no_op_smoke.rs`).
2. `try_bridge_fixture_path(<path to no_op.rs>)` returns
   `BridgeOutcome::Ok(model)` (path-based entry point,
   rustc-valid stem).
3. `try_bridge_fixture_path(<path to a tempfile named with a
   hyphenated stem, e.g. `TY-001.rs`, containing the no_op
   source>)` returns `BridgeOutcome::Ok(model)`. Asserts the
   sanitization path lifts the hyphenated stem to a rustc-valid
   crate name. This is the LOAD-BEARING ASSERTION for the F1
   runner's snippet naming convention.
4. `sanitize_crate_name("TY-001") == "TY_001"` and
   `sanitize_crate_name("123foo") == "s_123foo"` and
   `sanitize_crate_name("no_op") == "no_op"` (idempotent on
   already-valid names). The function is `pub(crate)` (or `pub`
   if the runner needs it; F0 ships as `pub(crate)` and lifts
   if F1 asks).
5. `try_bridge_fixture(<intentionally-broken-fixture-name>)`
   panics with the existing "fixture <name> is not shipped"
   wording (preserved caller-bug panic).
6. `bridge_fixture("no_op")` STILL returns `RegseqModel` (Change C
   backwards-compat property).
7. `bridge_fixture(<intentionally-broken-name>)` panics with the
   same descriptive wording as today (Change C backwards-compat).

Negative-path testing for `BridgeOutcome::Rejected` /
`ValidatorFailed` / `RustcRejected` is OUT OF SCOPE for F0. Those
paths surface during runner work in F1 against actual
matrix-uncovered rows; F0 ships with positive-path coverage only.

### Change E: harness expectations entry

**Anchor**: `.harness/expectations/regseq-driver-63.md` (new file).

Record:

- Before-fix evidence: the current helper's `BridgeError -> String`
  conversion at `lib.rs:230` + the panic-on-non-OK behavior at
  `lib.rs:247-263`.
- After-fix evidence: the new `BridgeOutcome` enum + typed entry
  points + the rewired `bridge_fixture` preserving panic-message
  parity.
- Caller audit result: the grep output (or its summary) for
  `bridge_fixture` callers across the workspace, confirming none
  treat the helper as fallible.
- Out-of-scope: subprocess stderr capture for
  `BridgeOutcome::RustcRejected.stderr` (F1 runner concern);
  panic-capture for bridge panics (F1 runner concern via
  `catch_unwind`); negative-path test coverage for `Rejected` /
  `ValidatorFailed` / `RustcRejected` (F1 + matrix snippets);
  scaffolding for the runner crate itself (F1).

## Non-Goals

- Do NOT scaffold `crates/bridge-coverage-runner/`. That is F1 of
  the runner strategy; this task is F0 only.
- Do NOT add subprocess-based rustc stderr capture for the
  `RustcRejected.stderr` field. F0 ships with an empty stderr
  string; the F1 runner adds the subprocess pre-flight per
  strategy §6.3.
- Do NOT add `catch_unwind` around the bridge invocation in this
  helper. Panic capture is the runner's job per strategy §6.2.
- Do NOT modify any existing fixture under
  `crates/test-ir-fixtures/fixtures/`. F0 is helper-only.
- Do NOT modify `spec.md`, `pretty-printer.md`,
  `generic-c-backend.md`, or any model-validator. F0 is
  test-helper-only.
- Do NOT change `bridge_fixture`'s public signature. The rewire
  preserves `pub fn bridge_fixture(name: &str) -> RegseqModel`
  byte-identically. Callers expecting `RegseqModel` continue to
  work; callers passing wrong fixture names continue to panic
  with the same wording.
- Do NOT add negative-path smoke tests for
  `BridgeOutcome::Rejected` / `ValidatorFailed`. Those surface
  organically during F1's snippet authoring; F0 ships with
  positive-path coverage only.
- Do NOT introduce a public library surface for in-process
  agent consumption beyond the two typed entry points
  (`try_bridge_fixture` / `try_bridge_fixture_path`). Strategy
  §3 explicitly defers a richer public API until F5+.

## Test Plan

### B1: existing tests stay green

`cargo test --workspace -- --test-threads=1` green. Specifically:

- `crates/test-ir-fixtures/tests/no_op_smoke.rs` passes
  unchanged (the `bridge_fixture("no_op")` call returns
  `RegseqModel` exactly as before).
- All existing e2e tests under `crates/regseq-model/tests/` and
  `crates/regseq-driver/tests/` that call `bridge_fixture(...)`
  pass unchanged.

### B2: typed-outcome smoke (new file)

**New file**:
`crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`

Asserts per Change D. All seven assertions land in this file. The
assertions exercise:

- `BridgeOutcome::Ok(_)` positive path via `try_bridge_fixture`
  AND `try_bridge_fixture_path` (rustc-valid stem).
- `BridgeOutcome::Ok(_)` positive path via `try_bridge_fixture_path`
  on a tempfile with a HYPHENATED stem (`TY-001.rs`) -- the
  load-bearing sanitization assertion for F1's snippet naming.
- `sanitize_crate_name` unit tests for hyphen, digit-prefix, and
  already-valid inputs.
- The fixture-name caller-bug panic path via
  `try_bridge_fixture("__nonexistent")`.
- The backwards-compat `bridge_fixture` panic on the same
  fixture-name caller bug.
- The backwards-compat `bridge_fixture` -> `RegseqModel` happy
  path.

### B3: caller-audit invariant

A simple script-level check (executed manually by the implementer
during work, not a committed test):

```bash
grep -rn 'bridge_fixture\b' crates/ tests/ --include='*.rs' | wc -l
```

Run before and after the rewire. Count is identical (no caller
introduced or removed by this task). The implementer records the
count in the harness expectations file.

### B4: backwards-compat panic-message parity

For each existing call site that panics today (none in committed
code, per the round-1 audit; but conservatively): the rewired
`bridge_fixture` panic wording matches the existing wording
character-for-character. The implementer manually verifies by
constructing a deliberately-broken fixture and comparing pre-fix
vs post-fix panic output. Recorded in the expectations file.

### B5: `cargo clippy` + `cargo fmt` clean

`cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets -- -D warnings` both pass.
The new code is small enough to be clippy-clean by construction;
any warnings get addressed (or, if non-actionable like
"useless_format", suppressed with a targeted `#[allow(...)]` and a
one-line justifying comment).

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/63-typed-bridge-outcome-helper.md`).
- The runner strategy doc (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  as the design source-of-truth.

### Reviewer focus

- **API contract**: does the proposed `BridgeOutcome` enum
  exactly match the strategy doc's §6.1 enum shape (Ok /
  ValidatorFailed / Rejected / RustcRejected with the named
  fields)? Cross-reference
  `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  §6.1 against this task's Change B.
- **Backwards-compat audit**: is the caller-audit claim correct
  against HEAD? Re-run the grep and confirm no caller treats
  `bridge_fixture` as fallible.
- **Panic-message parity**: does Change C preserve the exact panic
  wording at
  `crates/test-ir-fixtures/src/lib.rs:247-263`? Word-for-word
  comparison.
- **Mutex type lift**: does Change A correctly update the
  `result` field's type AND every place that locks/take()s it?
  Walk
  `crates/test-ir-fixtures/src/lib.rs:200-265` for all uses.
- **Out-of-scope discipline**: Non-Goals forbids stderr capture,
  catch_unwind, runner crate scaffolding, fixture changes,
  negative-path tests, public library API. Confirm Implementation
  Notes do not creep into any of those.
- **Citation drift**: every `crates/test-ir-fixtures/src/lib.rs:NNN`
  citation must anchor against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/63-typed-bridge-outcome-helper.md.
Identify:
- API contract drift vs strategy §6.1.
- Backwards-compat-audit correctness against HEAD.
- Panic-message parity between Change C and lib.rs:247-263.
- Mutex-type-lift completeness in Change A.
- Out-of-scope creep against Non-Goals.
- Citation drift.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
API-CONTRACT CHECK: PASS / FAIL with citation
BACKWARDS-COMPAT-AUDIT CHECK: PASS / FAIL with citation
PANIC-MESSAGE PARITY CHECK: PASS / FAIL with citation
MUTEX-TYPE-LIFT COMPLETENESS CHECK: PASS / FAIL with citation
OUT-OF-SCOPE DISCIPLINE: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - All five B2 assertions in
    `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs` pass.
  - No regressions in any existing workspace test.
- `git diff --stat HEAD..` shows changes confined to:
  - `crates/test-ir-fixtures/src/lib.rs` (Changes A + B + C)
  - new file
    `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs` (B2)
  - new file `.harness/expectations/regseq-driver-63.md` (Change E)
- No bridge / model / spec / pretty-printer / generic-c-backend
  changes.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.

## Acceptance Criteria

1. **`BridgeOutcome` enum lands**: `crates/test-ir-fixtures/src/lib.rs`
   defines a `pub enum BridgeOutcome` with the four variants
   `Ok(RegseqModel)`, `ValidatorFailed { model, violation }`,
   `Rejected(BridgeError)`, `RustcRejected { stderr: String }`,
   matching strategy §6.1 byte-for-byte.

2. **`try_bridge_fixture[_path]` typed entry points exist**:
   `crates/test-ir-fixtures/src/lib.rs` exposes
   `pub fn try_bridge_fixture(name: &str) -> BridgeOutcome` and
   `pub fn try_bridge_fixture_path(path: &Path) -> BridgeOutcome`.

3. **Callback `BridgeError -> String` conversion removed**:
   `crates/test-ir-fixtures/src/lib.rs:228-231` (current HEAD)
   replaces `Err(format!("{err}"))` with `Err(err)`; the Mutex
   type is updated to `Option<Result<RegseqModel, BridgeError>>`.

4. **`bridge_fixture` preserves panic-message parity**: the four
   panic-shape outcomes (rustc-no-result, bridge-error,
   validator-failure, fixture-not-found) panic with byte-identical
   wording to the current HEAD's `crates/test-ir-fixtures/src/lib.rs:247-263`
   panics.

5. **All existing tests pass**: B1 returns green.

6. **B2 typed-outcome smoke test lands**: new file at
   `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs` carries
   the seven assertions from Change D (including the
   load-bearing hyphenated-stem `try_bridge_fixture_path` test
   and the `sanitize_crate_name` unit tests) and passes.

7. **B3 caller-audit recorded**: the grep count for
   `bridge_fixture\b` before vs after the rewire is identical;
   recorded in `.harness/expectations/regseq-driver-63.md`.

8. **B4 panic-parity verified**: the implementer's manual
   broken-fixture test produces identical panic output pre- and
   post-fix; recorded in the expectations file.

9. **No scope creep**: `git diff main` touches ONLY
   `crates/test-ir-fixtures/src/lib.rs`,
   `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`,
   `.harness/expectations/regseq-driver-63.md`, and the harness
   `.harness/tasks.json` registration. No bridge / model / spec /
   pretty-printer / generic-c-backend / fixture changes.

10. **Expectations file lands**:
    `.harness/expectations/regseq-driver-63.md` records before /
    after evidence + caller-audit grep counts + B4 panic-parity
    verification.

11. **F1 readiness signal**: the implementer's expectations file
    closes with a "F1 readiness" note stating whether the typed
    surface is operationally sufficient for the runner crate's
    `oracle.rs` module (per strategy §3 architecture). This is
    the bridge between F0 and F1.
