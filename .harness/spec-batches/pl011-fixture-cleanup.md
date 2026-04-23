# Spec Batch: pl011-fixture-cleanup -- PL011 fixture drift + four-issue cleanup

## Description

This batch authors an executable harness task that resolves two
orthogonal-but-entangled cleanup workstreams on the PL011
Stage-1 fixture:

1. **Test-level drift cleanup**: the Stage-1 Codex audit
   (2026-04-21) renamed / restructured several fixture methods
   (`program_polling_port` -> `bring_up_port` +
   `reprogram_line_format`; `configure_line_control` ->
   `line_control_value` + `write_line_control`; `pending_tx:
   Option<u8>` field removed). Hardcoded method-name lists and
   specimen-fidelity assertions in the test suite still expect
   the pre-audit shape. Fixing these requires updating the
   affected test files' string-matching and `GOLDENS` lists.

2. **Four fixture-level issues discovered 2026-04-23**: the
   compiler's construction pass skips four method bodies in the
   fixture due to constructs the pipeline does not yet
   represent. Detailed in
   `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
   under "Known fixture issues (discovered 2026-04-23)":
   - `line_control_value` -- tuple destructuring at
     `driver.rs:190`
   - `uart_clock_hz` -- method-reference ZST at `driver.rs:334`
   - `rx_char` -- bitflags constant missing from `ConstantIndex`
     at `driver.rs:529`
   - `set_baud_rate` -- rustc `Assert` terminator for division
     at `driver.rs:148`

Both workstreams block the same downstream goal: a clean
golden-regeneration pass (`raw-frontend.json`,
`regseq-model.json`, IR goldens) that in turn unblocks the
hand-authored Linux reference C driver authoring. They are
bundled in one batch because: (a) both touch the fixture +
test surface at roughly the same sites; (b) golden regeneration
must happen once, covering both the rename drift and the
post-fix fixture; (c) splitting would force two golden
regenerations and invite merge churn.

This batch authors ONE task spec. The implementation happens
when a worker picks up the resulting harness task.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/plans/asplos-implementation-plan.md` (workflow line
  107 requires elaboration sessions to read the plan; no
  specific section of the plan governs this Stage-1 retrofit,
  but the plan is listed here per the workflow contract)
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
  (authoritative enumeration of the four fixture issues;
  "Known fixture issues (discovered 2026-04-23)" section)
- `docs/discussions/pl011-full-spec/audit-history/README.md`
  (summarizes what the Stage-1 audit changed and why; context
  for the rename drift)
- `docs/discussions/models/regseq-model/user-data-initialization.md`
  ("Stage-1 Rule-1 whitelist narrowing (tuples excluded)"
  section explains why issue #1's tuple-destructuring pattern
  is unsupported)
- `docs/discussions/models/regseq-model/contex-handoff.md`
  Session 4 entry (2026-04-23) for the handoff context
- `tests/fixtures/pl011-full-spec/src/driver.rs` -- the
  fixture under cleanup; lines 148 (issue #4), 174-200 (issue
  #1), 334 (issue #2), 519-540 (issue #3)
- `crates/regseq-driver/tests/pl011_construction.rs:61-80`
  (`GOLDENS` list with stale method names) and `:719,726`
  (per-function assertions with stale names)
- `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs:460,500`
  (specimen-fidelity assertion expecting removed
  `pending_tx: Option<u8>` field)
- `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`
  (specimen-fidelity assertions including
  `specimen_fidelity_program_polling_port_sequence_matches_fixture`
  that needs renaming to reflect the `bring_up_port` split)
- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  (specimen-fidelity body-fragment assertion; drifted)
- `crates/regseq-driver/tests/source_to_extraction_09_lifecycle_callbacks.rs`
  (drifted)
- `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs`
  (drifted)
- `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs`
  and `source_to_extraction_12_isr.rs` (drifted)
- `crates/regseq-driver/src/callbacks.rs` -- confirm the
  release-profile overflow-checks setting relevant to issue #4;
  `regseq-driver-25` (Phase-3c) was supposed to set
  `overflow-checks=false` + `debug-assertions=false` to
  suppress rustc's div-by-zero assert
- `crates/regseq-driver/src/construction/pass2.rs:981`
  (the site that rejects `Assert` terminators)

Implementation anchors (test patterns to mirror):

- `crates/regseq-driver/tests/pl011_construction.rs:1568-1580`
  (`UPDATE_GOLDENS=1` regeneration path)
- `crates/regseq-model/tests/pl011_pipeline_phase6.rs:126-134`
  (`REGEN_GOLDEN=1` regeneration path)

## Output Specification

Create:

- `.harness/tasks/regseq-driver/NN-pl011-fixture-cleanup.md`
  (NN to be chosen as the next available number in the
  `regseq-driver` task group) -- the executable task spec.

Update when done:

- `.harness/tasks.json` -- add the new task entry under the
  `regseq-driver` group. `regseq-backends-02` does NOT gain
  an additional dependency because the cleanup task should
  land before emission-contract work regardless (the cleanup
  task is itself blocked on nothing and unblocks golden-
  consuming workflows).
- `.harness/task-spec-status.yaml` -- change
  `pl011-fixture-cleanup.status` from `ready_to_elaborate` to
  `tasked`; populate `generated_task_ids`.

Optionally create:

- `docs/discussions/models/regseq-model/pl011-fixture-cleanup-codex-review/`
  directory for preserving Codex review transcripts if the
  batch uses adversarial-review rounds.

The output is one executable harness task spec plus
associated tracker and graph updates.

## Implementation Notes

This batch authors ONE task spec. Do not split into multiple
task specs.

### Task-spec authoring constraints

The task spec must:

- Title the task "PL011 fixture cleanup: rename drift +
  four-issue fix + golden regeneration".
- State that it spans two crates (`regseq-driver` for the
  fixture + the test suite; `regseq-model` for the Phase-6
  golden) as an intentional override of the single-crate
  norm per the rationale in this brief's Description.
- Include an **Implementation Notes** section with a
  TODO-style checklist covering:

  **Fixture-level fixes** (in order; each fix is verified
  individually before moving to the next):
  1. Fix `line_control_value` tuple destructuring at
     `tests/fixtures/pl011-full-spec/src/driver.rs:190` --
     split into two separate `match` expressions for `pen`
     and `eps`. Verify `pl011_construction` no longer skips
     the body.
  2. Fix `uart_clock_hz` method-reference at
     `tests/fixtures/pl011-full-spec/src/driver.rs:334` --
     replace `Clock::get_rate` with a closure
     `|c| c.get_rate()`. Verify body no longer skipped.
  3. Investigate `rx_char` bitflags constant issue at
     `tests/fixtures/pl011-full-spec/src/driver.rs:529`:
     classify as (a) compiler-extraction gap for framework
     bitflags constants, or (b) fixture misuse requiring
     inline numeric values. Apply the appropriate fix.
  4. Investigate `set_baud_rate` `Assert` terminator issue
     at `tests/fixtures/pl011-full-spec/src/driver.rs:148`:
     confirm whether
     `crates/regseq-driver/src/callbacks.rs` sets
     release-profile MIR semantics (`overflow-checks=false`
     + `debug-assertions=false`) and whether those reach
     the fixture crate. If yes and the assert is still
     present, diagnose further. If no, restore the
     release-profile settings. If the assert cannot be
     suppressed without a compiler change, document the
     decision and leave `set_baud_rate`'s body skipped.

  **Test-level drift fixes** (after the fixture-level fixes
  pass):
  5. Update `crates/regseq-driver/tests/pl011_construction.rs:61-80`
     `GOLDENS` list to the new method names. Mapping:
     `program_polling_port` -> split into `bring_up_port` +
     `reprogram_line_format`; `configure_line_control` ->
     split into `line_control_value` + `write_line_control`.
     Update per-function assertions at `:719,726` likewise.
  6. Update specimen-fidelity tests at
     `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs:460,500`
     to remove `pending_tx: Option<u8>` expectations (field
     was removed per REQ-PERF-BATCH-01 in Stage-1 audit).
  7. Update
     `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`:
     rename `specimen_fidelity_program_polling_port_sequence_matches_fixture`
     and update assertions to match the
     `bring_up_port`/`reprogram_line_format` split.
  8. Update
     `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
     specimen-fidelity body-fragment assertion.
  9. Update `source_to_extraction_09_lifecycle_callbacks.rs`,
     `source_to_extraction_10_serial_callbacks_a.rs`,
     `source_to_extraction_06_driver_constants.rs`,
     `source_to_extraction_12_isr.rs` to the current fixture
     shape.

  **Golden regeneration** (after the fixture and test fixes
  compile cleanly):
  10. Regenerate
      `tests/fixtures/pl011-full-spec/expected/raw-frontend.json`
      via the production `cargo-regseq` path (see task
      `regseq-model-64` for the concrete command). Commit.
  11. Regenerate
      `tests/fixtures/pl011-full-spec/expected/regseq-model.json`
      via `REGEN_GOLDEN=1 cargo test -p regseq-model --test pl011_pipeline_phase6`.
      Commit.
  12. Regenerate
      `tests/fixtures/pl011-full-spec/expected/ir/*.json`
      via `UPDATE_GOLDENS=1 cargo test -p regseq-driver --test pl011_construction`.
      Commit.

- Include a **Test Plan** section asserting:
  - `pl011_construction` passes (the updated `GOLDENS` list +
    regenerated IR goldens pass byte-comparison).
  - Each `source_to_extraction_*` test passes.
  - `pl011_pipeline_phase6` still passes.
  - All 13 downstream tests from
    `regseq-model-64`'s acceptance criteria still pass against
    the regenerated `raw-frontend.json`.
  - `cargo test --workspace` passes.

- Include a **Review** section that enumerates what a verifier
  must confirm:
  - Each of the 4 fixture issues either has a concrete fix
    committed (issues 1-3 should be clean fixes) or a
    documented deferral (issue 4 if the `Assert` suppression
    cannot be achieved).
  - Every `skipped build_body` message that the verifier sees
    when running `pl011_construction` is from framework code
    (`regseq::...`, `core::...`), NOT from
    `driver::Pl011FullSpec::*` methods.
  - No hardcoded method names in any test file still reference
    `program_polling_port`, `configure_line_control`, or
    `pending_tx`. Verify via
    `rg 'program_polling_port|configure_line_control|pending_tx: Option<u8>' crates/regseq-driver/tests/`.
  - All regenerated goldens commit cleanly; no byte drift
    between two consecutive regeneration runs (determinism).

- Include **Non-Goals**:
  - No changes to the user-data initialization contract
    (`regseq-model-64` is done; this task does not re-open it).
  - No shadow-register feature work (deferred per
    `docs/discussions/models/regseq-model/shadow-registers.md`).
  - No widening of the Rule-1 whitelist or Rule-2 support
    beyond what `regseq-model-64` landed.
  - No Phase-7 Linux-emission-contract work (that is
    `regseq-backends-02`, which this cleanup unblocks but does
    not execute).
  - No changes to `emit_semantic.rs:216`,
    `emit_module.rs:703`, or
    `Mmio::modify`'s `todo!()` gap at `mmio.rs:72-86` --
    all explicitly scoped out by `regseq-model-64`.
  - If fixture issue #4 (`set_baud_rate` `Assert`) requires a
    compiler change beyond restoring the release-profile MIR
    settings, that compiler change is NOT in scope for this
    task. Document the decision and leave `set_baud_rate`'s
    body skipped; a future task addresses it.

- Include a **Quality Gate** section running:
  ```
  cargo test -p regseq-driver
  cargo test -p regseq-model
  cargo test -p regseq-backends
  cargo test --workspace
  cargo fmt --all -- --check
  cargo clippy --workspace -- -D warnings
  ```

- Include **Acceptance Criteria** with each bullet anchored to
  a mechanically verifiable artifact.

### Cross-crate scope override

The task spans two crates plus the fixture directory as an
intentional override of `task-spec-workflow.md:120-121`'s
single-crate guidance. Authority: this brief's Description +
the pl011-full-spec-status "Known fixture issues" section.
Rationale: the fixture fixes and the test-drift fixes and the
golden regeneration must land together or the workspace is
uncompilable mid-sequence.

### Task ID and group

- Task ID: next available in the `regseq-driver` group (check
  `tasks.json` at authoring time; likely `regseq-driver-36`).
- Group: `regseq-driver` (the fixture and most affected test
  files live under `crates/regseq-driver/tests/`).

### Dependency graph

- The new cleanup task depends on `regseq-model-64` (done).
- `regseq-backends-02` currently depends on `regseq-model-64`;
  after this cleanup task is authored, `regseq-backends-02`
  should ALSO gain a dependency on the cleanup task (so the
  emission contract work does not start against a fixture
  that still has the 4 issues). Update the `regseq-backends-02`
  `depends_on` array in `tasks.json` to include the new
  cleanup-task id.

### Codex review gates (during this batch's execution)

Two Codex review rounds scoped to IMPLEMENTATION GUIDANCE ONLY:

1. After the task spec is drafted: Codex reviews it for
   implementation-guidance completeness (all 12 items covered,
   test plan exhaustive, verification sufficient).

Codex prompts must say: "Scope the review to the
implementation guidance. Do not re-debate strategy or the
user-data initialization contract."

## Non-Goals

- This batch does not implement the cleanup. It only authors
  the task spec.
- No changes to tracker items for other phases.
- No changes to the Path A design doc, the shadow-registers
  design doc, or their Codex review transcripts.
- No changes to `regseq-model-64`'s status (done).

## Acceptance Criteria

- `.harness/tasks/regseq-driver/NN-pl011-fixture-cleanup.md`
  created with all required sections and the 12-item TODO
  checklist embedded in Implementation Notes.
- `.harness/tasks.json` updated: new task entry under the
  `regseq-driver` group with `depends_on: ["regseq-model-64"]`;
  `regseq-backends-02` entry gains the new cleanup-task id in
  its `depends_on` list.
- `.harness/task-spec-status.yaml` updated:
  `pl011-fixture-cleanup.status` moves from `ready_to_elaborate`
  to `tasked`; `generated_task_ids` populated with the new
  task id.
- Codex review round (implementation-guidance scope) returns
  `VERDICT: APPROVED`. Any intermediate `REQUIRES FIXES`
  verdicts had the fixes folded before the final round.

## Update Instructions

After the task spec is drafted and committed:

1. In `.harness/task-spec-status.yaml`:
   - Change `pl011-fixture-cleanup.status` from
     `ready_to_elaborate` to `tasked`.
   - Populate `generated_task_ids` with the new task id
     (e.g., `["regseq-driver-36"]`).

2. In `.harness/tasks.json`:
   - Add the new task under the `regseq-driver` group with
     `depends_on: ["regseq-model-64"]`.
   - Modify the existing `regseq-backends-02` entry to add
     the new cleanup-task id to its `depends_on` array
     (preserving existing entries). Current dependencies
     include `regseq-model-60`, `regseq-model-64`,
     `regseq-linux-backend-06`, `regseq-linux-backend-07`,
     `regseq-linux-backend-08`, `regseq-lowering-12`.

3. Do not alter any other tracker items or tasks.
