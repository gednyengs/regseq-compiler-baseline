# track-a-validation-11: PL011 E2E Rewrites + Phase-7 Tracker Gate

## Description

Phase-7 gate task. Two responsibilities:

1. **Rewrite the E2E tests that still bootstrap legacy pipelines
   directly** (`tests/e2e/tests/pl011_bus_identity_proof.rs` and
   `tests/e2e/tests/pl011_lowering_input_stability.rs`) to build
   their inputs through the Phase-6 pipeline
   (`normalize()` + `populate_bindings()`) and to assert against
   `RegseqModel` / `compat` views instead of legacy
   `JoinedArtifact` / `SemanticModel` / `BindingModel` surfaces.
   Remove residual `JoinedArtifact` / `SemanticModel` assertions
   from those two files.
2. **Flip the tracker** at the end so `regseqmodel-phase-7` moves
   from `ready_to_elaborate` / `in_elaboration` to `tasked` with
   `generated_task_ids` populated, and `regseqmodel-phase-8`
   promotes from `not_ready` to `ready_to_elaborate`.

The other seven E2E files
(`driver_e2e.rs`, `pl011_driver_emission.rs`,
`pl011_frontend_qc.rs`, `pl011_layer_boundary.rs`,
`pl011_linux_module_host_build_proof.rs`, `pl011_spec_slice.rs`,
`platform_file_boundary.rs`) do **not** bootstrap legacy pipelines
directly -- they exercise the binary via `RUSTC_WRAPPER` and
assert against the end-to-end pipeline output, which already
reaches `RegseqModel` as of task `regseq-driver-35`. Enumerate
each of the seven in the Implementation Notes and confirm it
passes unchanged. If one fails, stop and fix the cutover
upstream; do not rebase E2E assertions.

Golden inputs (`tests/fixtures/pl011-full-spec/expected/*.json`)
stay frozen. Phase 9 owns the golden refresh.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` lines 110-114
  (E2E migration rules; "Keep golden inputs unchanged in Phase 7;
  golden refresh is Phase 9"); lines 151-154 (E2E grep gate with
  scope clarification per Codex review); lines 158-159 (tracker
  update -- promotes phase-8 to `ready_to_elaborate`).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  and Phase 8 sections.
- `.harness/tasks/regseq-model/60-pl011-pipeline-end-to-end.md`
  -- the gate task shape this task mirrors for tracker flipping.
- `.harness/tasks/regseq-driver/35-emit-driver-pipeline-cutover.md`
  -- the upstream cutover this task tests against.
- `.harness/tasks/regseq-model/61-compat-scaffold.md` -- the
  view types this task's rewrites consume.
- `tests/e2e/tests/pl011_bus_identity_proof.rs` -- legacy
  bootstrap at lines 17-19 and 172-276; rewrite target.
- `tests/e2e/tests/pl011_lowering_input_stability.rs` -- legacy
  bootstrap at lines 13-16 and 27-460; rewrite target.
- The other seven E2E files under `tests/e2e/tests/` (read each
  to verify they do not construct `DeclarationModel`,
  `JoinedArtifact`, `BindingModel`, or `SemanticModel`
  directly).
- `.harness/task-spec-status.yaml` -- tracker to update.
- `.harness/tasks.json` -- tasks 61 / regseq-analysis-06 /
  regseq-lowering-10 / regseq-linux-backend-03 /
  regseq-driver-35 / track-a-validation-11 must all be present
  and done at the end.

## Implementation Notes

### Part 1: Rewrite the two legacy-bootstrap E2Es

For each of the two files:

1. **Replace the legacy pipeline construction** with the Phase-6
   pipeline:

   ```rust
   // Before:
   let decl = declaration::normalize_from_json(&json)?;
   let binding_model = binding::normalize(&pf, ...)?;
   let joined = perform_join(&decl, &binding_model)?;

   // After:
   let raw = load_raw_artifact(
       "tests/fixtures/pl011-full-spec/expected/raw-frontend.json");
   let platform = load_platform_file(
       "tests/fixtures/pl011-full-spec/platform.toml");
   let mut model = regseq_model::normalize(&raw)?;
   regseq_model::populate_bindings(&mut model, &platform)?;
   ```

2. **Replace legacy-shape assertions** with `RegseqModel`-level
   or `compat`-view-level assertions that capture the same
   invariant:

   - `pl011_bus_identity_proof.rs`: the invariant is "platform
     bus identity survives into downstream consumer input". The
     new assertion reads the bus field on `RegseqModel.root_module
     .driver_module().driver_struct.bus` (or the equivalent
     resolved path) and the bus binding via
     `compat::BindingView::new_from_regseq_model(&model).bus_binding()`.
     Assert round-trip equality with the platform file value.
     Do **not** go through `AnalysisInput` in this file -- the
     test's purpose is the bus-identity invariant in isolation.
   - `pl011_lowering_input_stability.rs`: the invariant is "the
     `AnalysisInput` that the lowering planner consumes is
     stable across pipeline runs on the same inputs." The new
     assertion builds `AnalysisInput::from_regseq_model(&model)`
     twice on two cloned models, serializes each accessor's
     output, and asserts byte-equality. Old per-accessor
     assertions that compared against legacy-shape expectations
     should be converted to compare new-vs-new
     (the legacy-reference values stay valid because
     `AnalysisInput::from_regseq_model` was proven equivalent to
     the legacy ctor in task `regseq-driver-35`'s equivalence
     test).

3. **Remove dead imports**: grep each file for
   `regseq_model::{declaration, binding, joined, semantic,
   item}` and remove any that are no longer reachable. Replace
   with `regseq_model::compat::*` where still needed.

4. **Do not delete whole tests** -- each invariant tested today
   is still a real invariant. Keep the test function names and
   docstrings; only the body implementation changes.

### Part 2: Verify the other seven E2Es pass unchanged

Enumerate each file and its expected status:

| File | Touches legacy bootstrap? | Expected status after task 35 |
|------|--------------------------|-------------------------------|
| `driver_e2e.rs` | No -- runs the binary via RUSTC_WRAPPER | Pass unchanged |
| `pl011_driver_emission.rs` | No -- asserts generated C artifacts | Pass unchanged |
| `pl011_frontend_qc.rs` | No -- asserts raw artifact structure | Pass unchanged |
| `pl011_layer_boundary.rs` | No -- asserts Layer 1/2 boundary | Pass unchanged |
| `pl011_linux_module_host_build_proof.rs` | No -- host-build the emitted module | Pass unchanged |
| `pl011_spec_slice.rs` | No -- spec-slice fixture via RUSTC_WRAPPER | Pass unchanged |
| `platform_file_boundary.rs` | No -- platform parser + property-id vocabulary | Pass unchanged |

Read each file before declaring "no legacy bootstrap." If any of
them does construct a legacy model directly, add it to the Part-1
rewrite set and note the expansion in this task's fix-up commit.

If one of the seven *fails* after task 35 landed, stop and fix
task 35 upstream -- do not reshape the E2E assertion to match
a drifted behavior.

### Grep gate

```
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' tests/e2e/tests/
```

must return zero matches after this task lands. (Some tests may
still import `regseq_model::compat::*` or type-alias symbols --
that is intentional.)

```
rg -n 'JoinedArtifact|SemanticModel|BindingModel' tests/e2e/tests/
```

must return zero matches -- these legacy struct names no longer
appear in the E2E assertion surface, only `RegseqModel` and
`compat::*` views. (Exception: references inside string literals
or comments are fine; only use-as-type should be absent.)

### Part 3: Tracker flip (this task's explicit ownership)

On green tests:

1. **`.harness/task-spec-status.yaml`**:
   - `regseqmodel-phase-7.status`: `ready_to_elaborate` /
     `in_elaboration` → `tasked`.
   - `regseqmodel-phase-7.generated_task_ids`: populate with
     `[regseq-model-61, regseq-analysis-06, regseq-lowering-10,
       regseq-linux-backend-03, regseq-driver-35,
       track-a-validation-11]`.
   - `regseqmodel-phase-8.status`: `not_ready` →
     `ready_to_elaborate`.

2. **`.harness/tasks.json`**: confirm the six tasks above are
   present with correct `depends_on` edges:
   - `regseq-model-61` → depends on `regseq-model-60`.
   - `regseq-analysis-06` → depends on `regseq-model-61`.
   - `regseq-lowering-10` → depends on `regseq-analysis-06`.
   - `regseq-linux-backend-03` → depends on `regseq-lowering-10`.
   - `regseq-driver-35` → depends on `regseq-linux-backend-03`.
   - `track-a-validation-11` → depends on `regseq-driver-35`.

   The harness marks each task done via `task.py done` in the
   normal flow; this task's role is to confirm the final tracker
   state.

Both tracker updates must land in the same commit as the E2E
rewrites.

### What this task does NOT touch

- Golden JSON files under `tests/fixtures/pl011-full-spec/expected/`
  (Phase 9 refreshes).
- Any E2E test beyond the two listed in Part 1 and the
  enumeration in Part 2.
- `emit_driver.rs` (task 35 landed).
- `compat.rs` implementations (task 35 filled those in).
- **Do not modify `crates/regseq-driver/src/construction*`.**
- No Phase-8 work.

## Non-Goals

- No Linux emission semantic changes (Phase 8).
- No old-module deletion (Phase 9).
- No new E2E fixtures.
- No golden refresh.
- No backend-specific legality checks.
- No cross-fixture regression suite expansion -- PL011 remains
  the single fixture through Phase 7.

## Test Plan

- `cargo test --workspace` passes with all 9 E2E tests green.
- `cargo test -p <e2e crate>` passes specifically on the two
  rewritten files.
- Two-run determinism on the rewrite:

  ```rust
  #[test]
  fn pl011_lowering_input_stability_two_runs() {
      // Build AnalysisInput::from_regseq_model twice on cloned
      // models; assert every accessor outputs the same bytes.
      // This is the Phase-7-local analog of task 60's
      // end-to-end determinism.
  }
  ```

- Grep gates (both must pass as part of CI-equivalent local
  check before committing):

  ```
  rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' tests/e2e/tests/ && echo FAIL || echo OK
  rg -n 'JoinedArtifact|SemanticModel|BindingModel' tests/e2e/tests/ | grep -v '//.*' | grep -v '".*"' && echo FAIL || echo OK
  ```

- Tracker invariants:

  ```
  grep 'regseqmodel-phase-7' .harness/task-spec-status.yaml  # status: tasked
  grep 'regseqmodel-phase-8' .harness/task-spec-status.yaml  # status: ready_to_elaborate
  ```

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review track-a-validation-11 against
.harness/tasks/track-a-validation/11-pl011-e2e-regseq-model-rewrites.md.

Files to audit:
- tests/e2e/tests/pl011_bus_identity_proof.rs
- tests/e2e/tests/pl011_lowering_input_stability.rs
- tests/e2e/tests/driver_e2e.rs
- tests/e2e/tests/pl011_driver_emission.rs
- tests/e2e/tests/pl011_frontend_qc.rs
- tests/e2e/tests/pl011_layer_boundary.rs
- tests/e2e/tests/pl011_linux_module_host_build_proof.rs
- tests/e2e/tests/pl011_spec_slice.rs
- tests/e2e/tests/platform_file_boundary.rs
- .harness/task-spec-status.yaml
- .harness/tasks.json

Focus on:
- Every Phase-7 Acceptance Criteria item in the brief (lines
  131-160) has a concrete assertion here.
- The two rewritten E2Es no longer import
  regseq_model::{declaration,binding,joined,semantic,item}; they
  build inputs via normalize() + populate_bindings().
- The other 7 E2Es are unchanged (diff only the two rewrite
  targets).
- Golden JSON files are unchanged (diff tests/fixtures/).
- Tracker: phase-7 is `tasked` with correct generated_task_ids,
  phase-8 is `ready_to_elaborate`.
- tasks.json has the six tasks with correct depends_on edges.
- No edits to crates/regseq-driver/src/construction*.

Do NOT flag:
- emit_driver.rs state (task 35).
- compat.rs state (task 35).
- Any Phase-8 concern (emission rewrite).
</task>
... structured_output_contract per SESSION.md ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' tests/e2e/tests/
```

## Acceptance Criteria

- `tests/e2e/tests/pl011_bus_identity_proof.rs` and
  `tests/e2e/tests/pl011_lowering_input_stability.rs` build their
  inputs via `normalize()` + `populate_bindings()` and assert
  against `RegseqModel` / `compat` views.
- The other 7 E2E files are byte-unchanged vs pre-task state
  (verified by `git diff --name-only`).
- Golden JSON files under `tests/fixtures/pl011-full-spec/expected/`
  are byte-unchanged.
- Grep gates pass (no `regseq_model::{declaration, binding,
  joined, semantic, item}` imports and no `JoinedArtifact` /
  `SemanticModel` / `BindingModel` use-as-type in E2E files).
- All 9 E2E tests pass; `cargo test --workspace` green.
- `.harness/task-spec-status.yaml` shows `regseqmodel-phase-7`
  as `tasked` with `generated_task_ids: [regseq-model-61,
  regseq-analysis-06, regseq-lowering-10, regseq-linux-backend-03,
  regseq-driver-35, track-a-validation-11]`; `regseqmodel-phase-8`
  as `ready_to_elaborate`.
- `.harness/tasks.json` contains the six Phase-7 tasks with the
  dependency chain described in Implementation Notes Part 3.
- Codex review approved.
- After this task: Phase 7 is operationally complete; Phase 8
  work can begin.
