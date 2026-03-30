# Phase A-01 Review Draft

These are review-only harness task specs for `phase-a-01`.

They are intentionally not registered in `.harness/tasks.json` and do not
update `.harness/task-spec-status.yaml`.

## Proposed Task Set

1. `regseq-macros-01` - activate the proc-macro crate
2. `regseq-model-01` - add and activate the rustc-independent model crate
3. `regseq-diagnostics-01` - activate the diagnostics support crate
4. `regseq-facade-01` - activate the public `regseq` facade crate
5. `regseq-analysis-01` - activate the analysis crate with topology-aligned dependencies
6. `regseq-backends-01` - activate the unified backend crate with Linux and Zephyr module boundaries

## Proposed Ordering

- `regseq-macros-01`: no dependencies
- `regseq-model-01`: no dependencies
- `regseq-diagnostics-01`: no dependencies
- `regseq-facade-01`: depends on `regseq-macros-01`
- `regseq-analysis-01`: depends on `regseq-model-01`, `regseq-diagnostics-01`, `regseq-config-02`
- `regseq-backends-01`: depends on `regseq-model-01`, `regseq-diagnostics-01`, `regseq-config-02`

## Review Notes

- The draft keeps Phase A bounded to crate activation and crate-topology decisions.
- No task introduces real DSL behavior, extraction logic, or lowering logic.
- `regseq-model` is introduced here because the implementation plan requires it
  before later extractor and analysis task writing can proceed.
- `regseq-backends` remains one crate, but its activation task requires separate
  `linux` and `zephyr` module stubs so the topology decision is encoded in the
  initial crate skeleton.

## Draft Artifacts

- [proposed-tasks.json](./proposed-tasks.json)
- [regseq-macros/01-crate-init.md](./tasks/regseq-macros/01-crate-init.md)
- [regseq-model/01-crate-init.md](./tasks/regseq-model/01-crate-init.md)
- [regseq-diagnostics/01-crate-init.md](./tasks/regseq-diagnostics/01-crate-init.md)
- [regseq-facade/01-crate-init.md](./tasks/regseq-facade/01-crate-init.md)
- [regseq-analysis/01-crate-init.md](./tasks/regseq-analysis/01-crate-init.md)
- [regseq-backends/01-crate-init.md](./tasks/regseq-backends/01-crate-init.md)

## Capture Plan

If this draft is approved, the next step is to:

1. copy these task specs into `.harness/tasks/<group>/`
2. merge the group and task entries from `proposed-tasks.json` into `.harness/tasks.json`
3. update `phase-a-01` in `.harness/task-spec-status.yaml` to `tasked`
4. populate `generated_task_ids` with the six approved task IDs
