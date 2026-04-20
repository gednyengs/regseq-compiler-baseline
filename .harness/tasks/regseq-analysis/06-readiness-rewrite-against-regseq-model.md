# regseq-analysis-06: Rewrite `readiness` Against `RegseqModel`; Delete `body_readiness` + `input`

**Depends on**: `regseq-model-61`, `regseq-model-62`.

## Description

Phase-7A consumer migration. Rewrites `regseq-analysis::readiness`
over `&RegseqModel` (consuming `query::*` helpers from task 4 and
`LintFinding`s from task 5's `verify_phase_1`), and **deletes**
`body_readiness.rs` and `input.rs` outright.

Per migration.md Q3: `AnalysisInput` is DELETED (its only job was to
pair `DeclarationModel` + `JoinedArtifact`; with `RegseqModel` that
pairing is unnecessary). `body_readiness.rs` is DELETED (its checks
are now structural invariants enforced by `normalize()` success in
Phase 5/6, or have moved into `verify_phase_1`/`verify_phase_2`).
`readiness.rs` SHRINKS to reviewer-facing diagnostic reporting only
-- phase-legality checks moved to Phase-6 verifiers; lint findings
consumed from task 5's `VerifyReport`.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q3 + Q4
  (`regseq-analysis::input` DELETED, `body_readiness` DELETED,
  `readiness` SHRINKS).
- `.harness/tasks/regseq-model/61-query-module.md` -- upstream
  helpers.
- `.harness/tasks/regseq-model/62-lint-finding-api.md` -- upstream
  `VerifyReport.lints`.
- `crates/regseq-analysis/src/input.rs` (to be deleted).
- `crates/regseq-analysis/src/body_readiness.rs` (to be deleted).
- `crates/regseq-analysis/src/readiness.rs` lines 1-401 (to be
  rewritten).
- `crates/regseq-analysis/src/lib.rs` (public exports change).
- Every caller of `regseq_analysis::AnalysisInput::*`, grep:
  `rg -n 'regseq_analysis::AnalysisInput' crates/ tests/`.

## Implementation Notes

### New `readiness` surface

Replace `input.rs` and `body_readiness.rs` with direct-on-RegseqModel
reporters. New `readiness.rs` shape:

```rust
//! Reviewer-facing driver-readiness diagnostics computed directly
//! from `&RegseqModel` + the `VerifyReport` emitted by
//! `verify_phase_1`.
//!
//! Phase-legality checks (did every required binding populate? did
//! every Phase-1 field fill in?) live in Phase-6 verifiers. This
//! module does NOT re-implement those; it only surfaces
//! reviewer-oriented observations: callback inventories, property
//! lints, resource-family summaries, device-class expectations.

use regseq_model::RegseqModel;
use regseq_model::query::{
    callback_families_present, callback_impls, resource_field_names,
};
use regseq_model::verify::{LintFinding, VerifyReport};

/// The complete readiness report emitted for one driver.
#[derive(Debug, Clone)]
pub struct ReadinessReport {
    pub callback_families_present: BTreeSet<CallbackFamily>,
    pub callback_impls: Vec<CallbackImplSummary>,
    pub resource_field_names: Vec<(ResourceKind, String)>,
    pub property_lints: Vec<LintFinding>,
    // ... every reviewer-facing output currently produced by
    // readiness.rs that is NOT a phase-legality check.
}

pub fn check_readiness(
    model: &RegseqModel,
    verify_report: &VerifyReport,
) -> ReadinessReport { ... }
```

### Deletions

Delete outright:

- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-analysis/src/body_readiness.rs`

Remove corresponding `pub mod ...;` declarations and `pub use ...;`
re-exports from `crates/regseq-analysis/src/lib.rs`.

### Callers that survive (not updated by this task)

After this task, `AnalysisInput` is GONE as a type. Several callers
currently import it; each must either migrate, stub, or point at a
downstream task that handles the migration. Concrete inventory
(verify by grep before starting):

- `crates/regseq-backends/src/lowering/planner.rs:14` (current:
  `use regseq_analysis::AnalysisInput;`) -- updated by task 7
  (`regseq-lowering-10`), which rewrites the planner against
  `&RegseqModel`. During the transitional window (while task 6
  lands but tasks 7/8/12 have not), this import breaks. Resolution:
  **this task preserves a thin `AnalysisInput` shim** under a new
  `regseq-analysis/src/legacy_input.rs` that exposes the old
  struct backed by direct field storage (not by `DeclarationModel`
  / `JoinedArtifact`). The shim is deleted by task 7 once the
  planner migrates off it. Document the shim's retirement plan
  (target: `regseq-lowering-10`) inline.
- `crates/regseq-driver/src/emit_driver.rs` -- cutover is task 12.
  Uses `AnalysisInput::new(...)` today; continues to via the shim.
- `tests/e2e/tests/pl011_bus_identity_proof.rs:16`,
  `pl011_lowering_input_stability.rs:13` -- E2E tests that import
  `regseq_analysis::AnalysisInput`. Task 13 rewrites/retires
  these. Until then they consume the shim.
- Any other grep-discovered caller: either delete (if reachable
  from the tests this task migrates) or route through the shim
  with a documented target task.

**This task's hard requirement**: after it lands,
`cargo test --workspace` is green. The shim keeps the workspace
compiling through the transitional window; task 7 deletes it.

Shim shape:

```rust
//! Transitional backward-compat shim for `AnalysisInput`.
//!
//! This is a forward-compatible skeleton that keeps the workspace
//! green between regseq-analysis-06 (which deletes the old
//! AnalysisInput + body_readiness + input.rs) and
//! regseq-lowering-10 (which rewrites the lowering planner to
//! take `&RegseqModel` directly).
//!
//! DELETE THIS FILE as part of regseq-lowering-10. No production
//! code should link against `legacy_input` after that.

#[deprecated(note = "Migrate to &RegseqModel via regseq-lowering-10")]
pub struct AnalysisInput { /* minimal storage for legacy callers */ }

impl AnalysisInput {
    #[deprecated]
    pub fn new(decl: ..., joined: ..., bus_binding: ...) -> Self { ... }
    // Only the accessor subset the remaining callers read. Grep
    // the tree and enumerate before authoring.
}
```

### Grep gate

After this task:

```
rg -n 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/
# must return zero matches
rg -n 'AnalysisInput' crates/regseq-analysis/
# must return zero matches (type deleted)
rg -n 'body_readiness' crates/regseq-analysis/
# must return zero matches
```

### Interaction with MutationGuard

`verify_phase_1` now returns `Result<VerifyReport, VerifierError>`
(per task 5). `readiness::check_readiness` takes the `VerifyReport`
by reference to consume its `lints` field. It does NOT re-run
`verify_phase_1` internally (the caller is responsible for verifier
invocation; Phase-6 pipeline auto-runs verifiers via
`MutationGuard::commit`).

### What this task does NOT touch

- `crates/regseq-backends/**` (tasks 7-10).
- `crates/regseq-driver/**` (task 12).
- `crates/regseq-model/**` (tasks 4, 5).
- `tests/e2e/**` (task 13).
- `experiments/pl011-eval-env/**`.
- **`crates/regseq-driver/src/construction*`** (strict non-goal).

## Non-Goals

- No rewrites of downstream consumers (tasks 7, 12).
- No Phase-7F cleanup (legacy modules stay alive until tasks 14/15).
- No new lint rules -- those come through `verify_phase_1` (task 5).
- No changes to `query::*` helpers -- those come from task 4.
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Test Plan

- **Module deletions**: `rg -n 'body_readiness|AnalysisInput'
  crates/regseq-analysis/src/` returns empty.
- **Rewritten readiness tests**: `crates/regseq-analysis/tests/readiness.rs`
  replaces the old tests that consumed `AnalysisInput`. Each new
  test constructs a minimal model, calls `verify_phase_1`, passes
  the report + model to `check_readiness`, asserts expected
  findings.
- **PL011 smoke**: load Phase-6 golden, run `verify_phase_1` +
  `check_readiness`, assert `ReadinessReport` fields populate as
  expected (callback families present matching the PL011 spec,
  resource field names matching `resource_field_names` output,
  property_lints empty or matching a documented baseline).
- **Grep assertions** (documented in Acceptance Criteria; run
  locally before commit).
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-analysis-06 against .harness/tasks/regseq-analysis/06-readiness-rewrite-against-regseq-model.md.

Files to audit:
- crates/regseq-analysis/src/readiness.rs (rewritten)
- crates/regseq-analysis/src/lib.rs (exports updated)
- crates/regseq-analysis/src/input.rs (expected DELETED)
- crates/regseq-analysis/src/body_readiness.rs (expected DELETED)
- crates/regseq-analysis/src/legacy_input.rs (NEW transitional shim;
  retirement target: regseq-lowering-10)
- crates/regseq-analysis/tests/readiness.rs (rewritten tests)

Focus on:
- Zero references to old models (regseq_model::{declaration,
  binding, joined, semantic, item}) in crates/regseq-analysis/src/.
- body_readiness module entirely gone; input.rs entirely gone.
- check_readiness consumes VerifyReport + RegseqModel directly.
- legacy_input shim:
    (a) exposes the minimum AnalysisInput surface the surviving
        callers (lowering planner, emit_driver, two E2E tests)
        require;
    (b) is marked #[deprecated] and documents its retirement
        target (regseq-lowering-10);
    (c) is NOT backed by DeclarationModel / JoinedArtifact --
        it's a standalone type with direct field storage, so it
        does not re-introduce the old-model dependency;
    (d) is the ONLY place AnalysisInput still exists; every
        downstream import points at legacy_input.
- PL011 smoke assertion matches the fixture's actual shape.
- Workspace green: surviving callers compile via the shim;
  tests/E2E that will be rewritten later are unchanged here.

Do NOT flag:
- Consumers not yet migrated (tasks 7, 12, 13).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-analysis
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-analysis -- -D warnings
rg -n 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/
# must print nothing
rg -n 'AnalysisInput|body_readiness' crates/regseq-analysis/src/
# must print nothing
```

## Acceptance Criteria

- `crates/regseq-analysis/src/input.rs` deleted.
- `crates/regseq-analysis/src/body_readiness.rs` deleted.
- `crates/regseq-analysis/src/readiness.rs` rewritten; only
  imports from `regseq_model::{query, verify, RegseqModel}`.
- `crates/regseq-analysis/src/lib.rs` updated: no `mod input`,
  no `mod body_readiness`, no re-export of `AnalysisInput`.
- Grep gate passes (commands above).
- New `readiness` tests pass; PL011 smoke passes.
- `cargo test --workspace` green (intermediate callers of
  AnalysisInput are either migrated or appropriately stubbed
  with task-pointer ignores).
- Codex review APPROVED.
