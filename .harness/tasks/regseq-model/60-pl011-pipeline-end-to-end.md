# regseq-model-60: PL011 Phase-6 End-to-End Pipeline + Golden + Tracker Updates

## Description

Phase-6 gate: prove the full pipeline
`RawFrontendArtifact` → `normalize()` → `verify_phase_1` →
`populate_bindings()` → `verify_phase_2` produces a fully-populated
`RegseqModel` for PL011, and freeze the resulting serialized shape
as a JSON golden. After this task, Phase-6 is operationally complete
and Phase-7 consumer migration can start against a stable API.

This task is test-only and configuration-only. It does not introduce
new pipeline logic; tasks 55-59 own the implementation.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Sections 11-13
  (the pipeline contract this test validates).
- `.harness/spec-batches/regseqmodel-phase-6.md` Acceptance
  Criteria (lines 143-168) -- the full checklist this task
  closes. Note: this task owns the
  "Tracker updated; regseqmodel-phase-7 advances to
  ready_to_elaborate" final bullet.
- `.harness/tasks/regseq-model/58-normalize.md` -- `normalize()`.
- `.harness/tasks/regseq-model/59-populate-bindings.md` --
  `populate_bindings()`.
- `.harness/tasks/regseq-model/54-pl011-end-to-end.md` -- shape
  template; the Phase-5 end-to-end test that this task mirrors
  at the Phase-6 level.
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` --
  input fixture.
- `tests/fixtures/pl011-full-spec/platform.toml` (or whichever
  concrete PL011 platform file exists) -- the platform-file
  input.
- `.harness/tasks.json` -- will be updated to add tasks 55-60
  when the batch is authored; this task's completion requires
  verifying the `generated_task_ids` list on `regseqmodel-phase-6`
  is complete.
- `.harness/task-spec-status.yaml` -- tracker to update.

## Implementation Notes

### Integration test

Add `crates/regseq-model/tests/pl011_pipeline_phase6.rs`. Structure:

```rust
#[test]
fn pl011_phase6_pipeline_end_to_end() {
    let raw = load_raw_artifact(
        "tests/fixtures/pl011-full-spec/expected/raw-frontend.json"
    );
    let platform = load_platform_file(
        "tests/fixtures/pl011-full-spec/platform.toml"
    );

    // Phase 1.
    let mut model = normalize(&raw).expect("normalize must succeed on PL011");
    verify_phase_1(&model).expect("PL011 normalize must satisfy Phase-1 invariants");

    // Phase 2.
    populate_bindings(&mut model, &platform)
        .expect("populate_bindings must succeed on PL011 platform");
    verify_phase_2(&model).expect("PL011 bound model must satisfy Phase-2 invariants");

    // Golden byte-equality.
    let serialized = serialize_regseq_model(&model);
    assert_golden_bytes_equal(
        &serialized,
        "tests/fixtures/pl011-full-spec/expected/regseq-model.json",
    );
}

#[test]
fn pl011_phase6_pipeline_is_deterministic() {
    // Run the pipeline twice on the same inputs; assert byte-equal.
    let out_a = run_pipeline();
    let out_b = run_pipeline();
    assert_eq!(out_a, out_b, "pipeline must be deterministic");
}
```

### Golden file

Create `tests/fixtures/pl011-full-spec/expected/regseq-model.json`.
This is the full `RegseqModel` produced by running the pipeline on
the PL011 fixture. Generation procedure:
1. Implement the test; run once; capture serialized output.
2. Manually eyeball the output for obvious defects (e.g., missing
   `binding.clock`, empty `abstractions_used`).
3. Commit the JSON as the golden.
4. The test asserts byte-equality on every future run.

### Model-level serialization

Phase-5 task 54 landed Body-level serialization. Phase-6 needs full
`RegseqModel`-level serialization. This task must verify that
`RegseqModel` and every member type has serde wiring consistent with
the ir-reference.md Section 7 contract:
- Adjacently-tagged enums.
- Omit-when-None options.
- `Symbol`/`FileId` rendered as interned strings via the model's
  `SymbolInterner`.
- `BTreeMap`/`BTreeSet` natural ordering for determinism.
- A `serialize_regseq_model(&RegseqModel) -> String` helper that
  pretty-prints with consistent formatting.

If any `regseq-model` type is missing serde wiring, add it here. This
is the Phase-6 analogue of task 54's Body-serialization ownership.

### Non-goal: re-implementing pipeline logic

If the PL011 golden produces a surprising shape, fix the underlying
task (55/56/57/58/59). Do NOT massage the golden to work around
defects. Do not modify `crates/regseq-driver/src/construction*`
under any circumstance in this task.

### Tracker updates (this task's explicit ownership)

When the test passes and the golden is committed:

1. **`.harness/task-spec-status.yaml`**:
   - Change `regseqmodel-phase-6.status` from `ready_to_elaborate`
     (or `in_elaboration`) to `tasked`.
   - Populate
     `regseqmodel-phase-6.generated_task_ids` with
     `[regseq-model-55, regseq-model-56, regseq-model-57,
       regseq-model-58, regseq-model-59, regseq-model-60]`.
   - Change `regseqmodel-phase-7.status` from `not_ready` to
     `ready_to_elaborate`.

2. **`.harness/tasks.json`**:
   - Ensure tasks 55-60 are present with correct
     `depends_on` edges (55: regseq-model-54; 56: 55;
     57: 55, 56; 58: 55; 59: 56, 57, 58; 60: 58, 59).
   - Mark all six as `status: done` on completion (normal
     harness flow handles this per task; this task confirms
     the final state).

Both updates must land in the same commit as the test passing.

## Non-Goals

- **No modifications to `crates/regseq-driver/src/construction*`.**
  This is the strictest non-goal of Phase 6 (brief line 20-21).
- No new normalization, join, or verification logic.
- No consumer migration (Phase 7).
- No backend changes (Phase 8).
- No backend-specific legality checks (those land with their
  affected backend per spec.md line 1410-1413).
- No `emit_driver` migration (Phase 7 scope).

## Test Plan

- Integration test `pl011_pipeline_phase6.rs` asserts:
  - Pipeline runs end-to-end without errors.
  - `verify_phase_1` passes between `normalize` and
    `populate_bindings`.
  - `verify_phase_2` passes after `populate_bindings`.
  - Serialized output byte-equals the committed JSON golden.
  - Two consecutive pipeline runs produce byte-identical
    serialization (determinism).
- Negative integration tests (one `#[test]` each):
  - Malformed raw artifact → `normalize` returns
    `NormalizeError`.
  - Platform file with unknown section id → `populate_bindings`
    returns `JoinError::UnknownPlatformSection`.
  - Platform file missing a required section →
    `populate_bindings` returns
    `JoinError::Phase2VerificationFailed`.
- `cargo test -p regseq-model` passes.
- `cargo test --workspace` passes (full-repo green after the
  Phase-6 pipeline lands).

## Review

After tests pass and the golden is committed, use `codex:rescue` to
verify: (1) every Phase-6 brief Acceptance Criteria item (brief
lines 143-168) has a concrete assertion in this test; (2) the golden
JSON covers the complete PL011 model (every resource bound, every
property populated, every cross-crate function has a body); (3) the
determinism test is a true byte-equality assertion across two
distinct `run_pipeline()` calls; (4) no Phase-7 or Phase-8 code
leaked into the diff; (5) `.harness/task-spec-status.yaml` moves
phase-6 to `tasked` and phase-7 to `ready_to_elaborate`; (6)
`.harness/tasks.json` contains tasks 55-60 with correct
dependencies. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model --test pl011_pipeline_phase6
cargo test -p regseq-model
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `pl011_pipeline_phase6.rs` integration test passes with the
  full set of assertions listed in the Test Plan.
- `tests/fixtures/pl011-full-spec/expected/regseq-model.json`
  committed as the Phase-6 golden.
- Determinism test passes.
- Negative integration tests pass with the expected error types.
- `cargo test --workspace` green.
- `git diff --name-only main..HEAD` shows zero changes to
  `crates/regseq-driver/src/construction*`.
- `.harness/task-spec-status.yaml` updated: `regseqmodel-phase-6`
  `tasked` with `generated_task_ids: [regseq-model-55, ...,
  regseq-model-60]`; `regseqmodel-phase-7` promoted to
  `ready_to_elaborate`.
- `.harness/tasks.json` contains tasks 55-60 with correct
  dependency edges.
- Codex review approved.
- After this task: `regseqmodel-phase-6` is operationally
  complete and Phase 7 work can begin.
