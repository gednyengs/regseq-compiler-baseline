# Spec Batch: regseqmodel-phase-9 - Old module cleanup and final tests

## Description

This batch authors executable harness task specs for removing the old
model modules (declaration/, binding/, joined/, semantic/, item/,
compat.rs), final golden file regeneration with the new JSON format,
and final test cleanup.

This is the last phase of the RegseqModel refactor.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 9)
- `crates/regseq-model/src/lib.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task specs
- `.harness/tasks/track-a-validation/` task specs

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- Delete: declaration/ (16 files), binding/ (7 files), joined/ (3 files),
  semantic/ (8 files), item/ (12 files), compat.rs.
- Update lib.rs to remove old module declarations.
- Regenerate golden files with new format (adjacently-tagged enums,
  format_version, Symbol-as-string).
- Remove old-model assertions from E2E tests. Add RegseqModel structure
  assertions.

## Non-Goals

- No new features.
- No Zephyr or E1000 work.

## Acceptance Criteria

- Old modules deleted. All tests pass. Golden files regenerated.
- No references to old model types remain in the codebase.
- Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-9` to `tasked`.
2. Fill `generated_task_ids`.
3. All tracker items should now be `tasked`.
