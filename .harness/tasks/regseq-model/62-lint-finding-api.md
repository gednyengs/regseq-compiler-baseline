# regseq-model-62: `LintFinding` API + Fold Property Lints into `verify_phase_1`

**Depends on**: `regseq-model-60`.
**Parallel with**: `regseq-model-61`, `regseq-backends-02`,
`regseq-backends-03`, `track-a-validation-11`.

## Description

Phase-7A foundation. Introduces a **non-fail-fast lint channel**
alongside the existing `verify_phase_1` / `verify_phase_2` fail-fast
error channel. Folds the current legacy property-lint rules (today
carried as `DeclarationModel::property_lints` via `PropertyLintDecl`
at extraction time) into `verify_phase_1` as structured
`LintFinding`s surfaced to callers without aborting the commit.

Per migration.md Q4 + decision 1: property lints are reviewer-facing
diagnostics, not phase-legality failures. Folding them into
`verify_phase_1` keeps them paired with the rule that produced
them, eliminates the separate extraction-time computation, and
removes the need for `PropertyLintDecl` to exist at all (it's
deleted as part of Phase-7F when old modules disappear).

Per Codex round-3 review (MANDATORY fix 7): **this task explicitly
owns the interface between `LintFinding` and `MutationGuard`**.
Today `MutationGuard::commit` calls `verify_phase_1` / `verify_phase_2`
and rolls back on any `VerifierError`. Lints must flow through
without triggering rollback; the guard's return type needs a
corresponding shape change.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q4 (property
  lints recommendation: fold into `verify_phase_1::LintFinding`),
  "Open decisions" item 1, Codex round-3 review MANDATORY fix 7
  (lint-channel interface with MutationGuard).
- `crates/regseq-model/src/verify/phase_1.rs` lines 1-46 -- current
  fail-fast `verify_phase_1` signature.
- `crates/regseq-model/src/verify/phase_2.rs` -- same pattern for
  Phase-2 verifier.
- `crates/regseq-model/src/model/mutation.rs` lines 18-27, 259-289
  -- `MutationGuard::commit` fail-fast + rollback contract.
- `crates/regseq-model/src/declaration/mod.rs` (the soon-to-be-deleted
  `PropertyLintDecl` + its extraction origin). Read the rules that
  populate `property_lints` today; those rules re-home here.
- `.harness/tasks/regseq-model/55-verifier-framework-and-phase1.md`
  + `56-post-phase2-verifier.md` + `57-mutation-api-auto-verify.md`
  -- the verify + mutate contracts this task extends.

## Implementation Notes

### `LintFinding` vocabulary

Define under `crates/regseq-model/src/verify/lints.rs`:

```rust
use crate::identity::{DeclId, SourceOrigin};
use crate::verify::ErrorProvenance;

/// A reviewer-facing finding surfaced by `verify_phase_1` /
/// `verify_phase_2`. NOT a fail-fast error; does NOT trigger
/// `MutationGuard` rollback.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LintFinding {
    pub level: LintLevel,
    pub kind: LintKind,
    pub provenance: ErrorProvenance,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum LintLevel {
    /// Reviewer attention recommended but not required.
    Advisory,
    /// Reviewer attention strongly recommended; likely a driver bug.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum LintKind {
    // ----- From legacy PropertyLintDecl rules -----
    PropertyMissingDefault { property: DeclId },
    PropertyTypeLooseEnum { property: DeclId, concrete: String },
    OptionalPropertyWithoutNullHandling { property: DeclId },
    // Enumerate every rule PropertyLintDecl currently produces;
    // read crates/regseq-model/src/declaration/mod.rs + its
    // callers to ensure completeness.
    // Additional non-property lint kinds may land here in future
    // tasks; this enum is extensible.
}
```

### Verifier signature changes (single canonical shape)

`verify_phase_1` and `verify_phase_2` return:

```rust
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct VerifyReport {
    /// Reviewer-facing findings. Non-fatal. Empty if no rules triggered.
    pub lints: Vec<LintFinding>,
}

pub fn verify_phase_1(
    model: &RegseqModel,
) -> Result<VerifyReport, VerifierError>;

pub fn verify_phase_2(
    model: &RegseqModel,
) -> Result<VerifyReport, VerifierError>;
```

This shape is **canonical**: downstream task `regseq-analysis-06`
imports and destructures `VerifyReport` directly, so the return
type cannot be left open.

The critical property: **lints flow out via `Ok(VerifyReport)` arm;
only `VerifierError` triggers `MutationGuard` rollback.**

### `MutationGuard::commit` contract change

```rust
impl<'m> MutationGuard<'m> {
    /// Commits the mutation. Runs the phase-level verifier.
    /// Returns `Ok(VerifyReport)` with any lint findings if the
    /// model is well-formed, or `Err(VerifierError)` with
    /// rollback-applied on a structural failure.
    pub fn commit(self) -> Result<VerifyReport, VerifierError> {
        match self.phase {
            MutationPhase::Phase1 => crate::verify::verify_phase_1(&self.model_after),
            MutationPhase::Phase2 => crate::verify::verify_phase_2(&self.model_after),
        }
        // ... existing rollback-on-error logic unchanged;
        // VerifyReport with lints passes straight through.
    }
}
```

This is a **public-API-breaking change**: `commit()` return type
changes from `Result<(), VerifierError>` to
`Result<VerifyReport, VerifierError>`. Every existing caller
(`normalize()`, `populate_bindings()`, task 57's tests) must be
updated. Since lints are non-fatal, existing callers that
previously treated `commit()` as pass/fail simply ignore the
`VerifyReport.lints` for now; they gain the ability to surface
them by destructuring.

Update call sites:

- `crates/regseq-model/src/normalize/mod.rs` -- lines around
  the `commit()` call.
- `crates/regseq-model/src/join/mod.rs` -- ditto.
- Any test under `crates/regseq-model/tests/` that destructures
  `commit()`'s return.

### Lint rule implementation

Under `crates/regseq-model/src/verify/lints/rules/`, one rule per
legacy `PropertyLintDecl` kind:

```
rules/
    property_missing_default.rs
    property_type_loose_enum.rs
    optional_property_without_null_handling.rs
    (one file per rule)
    mod.rs  -- runs all rules; returns Vec<LintFinding>.
```

Each rule walks the model's driver-module property set, emits
findings. `verify_phase_1` calls `lints::rules::run(model)` and
packages the output into the `VerifyReport`.

### Deletion of legacy carry-over

`declaration::PropertyLintDecl` and its extraction-time computation
are removed in Phase-7F (task 14 or 15). For now, this task leaves
them alive (deletion is out of scope); it just stops *using* them
from the verifier side. After task 6 migrates regseq-analysis to
consume `verify_phase_1`'s lint output, nothing reads
`PropertyLintDecl` anymore.

### Determinism

Lint iteration order: `LintFinding`s sorted by
`(LintLevel, DeclId, LintKind-variant-discriminant)`. A
determinism test asserts two runs produce byte-equal serialized
output.

## Non-Goals

- No consumer migration (task 6 consumes this).
- No new non-property lint rules beyond folding existing
  `PropertyLintDecl` rules.
- No deletion of `declaration::PropertyLintDecl` (Phase-7F).
- No Phase-6 pipeline semantic changes (Phase-1/2 commit behavior
  unchanged; only the return shape adds lints).
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Test Plan

Unit tests under `crates/regseq-model/tests/lint_findings.rs`:

- **Happy path**: minimal valid Phase-1 model with no lints
  triggered; `verify_phase_1` returns
  `Ok(VerifyReport { lints: [] })`.
- **One lint per rule**: for each `LintKind` variant, a minimal
  model that triggers exactly that rule; verifier returns
  `Ok(VerifyReport { lints: [expected] })`.
- **Determinism**: two runs on a model with multiple lints produce
  byte-equal `VerifyReport`.
- **Mutation guard rollback contract**: a mutation that introduces
  a Phase-1 structural violation returns `Err(VerifierError)` and
  rolls back (existing task-57 test; extend to confirm return type
  still compiles).
- **Mutation guard lint pass-through**: a mutation that introduces
  a model triggering a lint (e.g., a property with no default)
  returns `Ok(VerifyReport { lints: [_] })` and does NOT roll
  back.
- **PL011 smoke**: load Phase-6 golden, run `verify_phase_1`,
  assert `Ok(_)` and lint list matches expected baseline (may be
  empty for a well-authored fixture).

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-model-62 against .harness/tasks/regseq-model/62-lint-finding-api.md.

Files to audit:
- crates/regseq-model/src/verify/lints.rs + lints/rules/
- crates/regseq-model/src/verify/{mod,phase_1,phase_2}.rs (signature changes)
- crates/regseq-model/src/model/mutation.rs (MutationGuard::commit signature)
- crates/regseq-model/src/normalize/mod.rs + join/mod.rs (call site updates)
- crates/regseq-model/tests/lint_findings.rs + mutation_api_auto_verify.rs

Focus on:
- Does the `MutationGuard::commit` change preserve the original
  contract: rollback on VerifierError, pass-through on lint-only Ok?
- Is every `PropertyLintDecl` rule from the legacy extraction
  accounted for by a LintKind variant?
- Does `LintFinding` sort deterministically?
- Does the shape match `Result<VerifyReport, VerifierError>`
  exactly (no `VerifyOutcome` alternative)?
- Are lints separated from `VerifierError` cleanly -- could a
  rule author accidentally turn a lint into a hard error, or
  vice versa?

Do NOT flag:
- Consumer-side lint consumption (task 6).
- Deletion of declaration::PropertyLintDecl (Phase-7F).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-model --test lint_findings
cargo test -p regseq-model --test mutation_api_auto_verify
cargo test -p regseq-model
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/verify/lints.rs` with `LintFinding`,
  `LintLevel`, `LintKind` types.
- `crates/regseq-model/src/verify/lints/rules/` with one rule per
  legacy `PropertyLintDecl` kind.
- `verify_phase_1` + `verify_phase_2` return
  `Result<VerifyReport, VerifierError>` (single canonical shape;
  no `VerifyOutcome` alternative).
- `MutationGuard::commit` returns `Result<VerifyReport, VerifierError>`;
  rollback still fires only on `Err`; `Ok(report)` with lints does
  NOT trigger rollback.
- `normalize()` + `populate_bindings()` call sites updated; their
  public API either surfaces or explicitly drops the
  `VerifyReport`.
- Determinism test passes.
- PL011 smoke passes.
- `cargo test --workspace` green.
- Codex review APPROVED.
