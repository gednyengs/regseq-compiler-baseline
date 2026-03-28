# Spec Batch: phase-a-01 - Workspace activation and crate-topology decisions

## Description

Author the first harness task-spec batch for the ASPLOS implementation stream.
This batch should turn the implementation plan's Phase A workspace-activation
requirements into concrete harness tasks.

This is not code implementation work. It is task-spec authoring work.

The goal is to create the initial executable harness tasks that:

- activate the missing implementation crates in the workspace
- record the crate-topology decisions already fixed by the implementation plan
- prepare the workspace for later API-first DSL and extraction tasks

## Input Context

Read before drafting task specs:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `README.md`
- `Cargo.toml`
- `docs/plans/asplos-implementation-plan.md`
- `docs/guide/implementation-guide.md`

## Output Specification

Create or modify:

- `.harness/tasks.json`
- `.harness/tasks/regseq-facade/...`
- `.harness/tasks/regseq-macros/...`
- `.harness/tasks/regseq-model/...`
- `.harness/tasks/regseq-analysis/...`
- `.harness/tasks/regseq-backends/...`
- `.harness/tasks/regseq-diagnostics/...`

Update when done:

- `.harness/task-spec-status.yaml`

The result of this batch should be real executable harness tasks, not just a
discussion or a plan note.

## Implementation Notes

The implementation plan already fixes these decisions:

- `regseq-model` is a new rustc-independent crate
- extraction stays in `regseq-driver`
- analysis passes live in `regseq-analysis`
- `regseq-backends` stays unified initially, even if Linux and Zephyr later get separate task groups

The resulting harness tasks should reflect those decisions directly. They
should not reopen them.

Recommended decomposition for this batch:

1. Add a task for activating `regseq` as a real workspace crate.
2. Add a task for activating `regseq-macros` as a real workspace crate.
3. Add a task for adding and activating `regseq-model`.
4. Add a task for activating `regseq-analysis`.
5. Add a task for activating `regseq-backends`.
6. Add a task for activating `regseq-diagnostics`.
7. If needed, add one task to record or enforce crate-level README/module
   boundary expectations.

Keep tasks narrow. Do not bundle all crate activation work into one large task.

## Non-Goals

Do not include:

- DSL surface implementation
- macro parsing behavior
- extraction logic
- platform-binding format work
- PL011 spec work
- lowering or backend code generation behavior

Those belong to later spec batches.

## Acceptance Criteria

This spec batch is complete when:

- the first Phase A harness tasks exist under `.harness/tasks/`
- `.harness/tasks.json` includes those tasks in the correct groups
- the tasks are narrow, testable, and aligned with the implementation plan
- the batch tracker entry `phase-a-01` is updated to `tasked`
- `generated_task_ids` for `phase-a-01` lists the new harness task IDs

## Update Instructions

After creating the harness tasks:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-a-01` to `tasked`
   - populate `generated_task_ids`
2. Move the next dependency-satisfied item to `ready_to_elaborate`
   if appropriate.
3. Do not change unrelated tracker items.
