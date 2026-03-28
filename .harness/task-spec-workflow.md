# Harness Task-Spec Workflow

This document defines how to use the multi-phase harness when executable
tasks do not yet exist for the implementation plan.

## Purpose

The harness now has two operational layers:

- executable task execution via `.harness/tasks.json` and `.harness/tasks/`
- task-spec authoring progress via `.harness/task-spec-status.yaml`

Executable tasks always take priority. The task-spec tracker exists only
to guide authoring of the next harness task batch when no executable
tasks are available.

## Decision Order

Every cold worker session should follow this order:

1. Read `.harness/config.md`.
2. Read `.harness/SESSION.md`.
3. Run `uv run python scripts/python/task.py doctor`.
4. Run `uv run python scripts/python/task.py next-step`.
5. If the next action is an executable task, do that task and ignore this document.
6. If the next action is `TASK_SPEC_BATCH`, read `.harness/task-spec-status.yaml`.
7. Find the item whose `TRACKER_ID` matches the `next-step` output.
8. Read the brief file printed by `next-step`.
9. Use that brief to author the next harness task spec or task batch.
10. If the next action is `BRIEF_AUTHORING_BATCH`, create the missing brief first using `.harness/spec-batches/BRIEF_TEMPLATE.md`, then continue with the normal task-spec flow in a later fresh session or after updating the tracker.

If there is no item with status `ready_to_elaborate`, stop and report the
state rather than guessing the next batch.

## What the Tracker Means

`.harness/task-spec-status.yaml` does not track implementation execution.
It tracks authoring progress for harness task specs only.

Valid statuses are:

- `not_ready`: dependencies have not yet been elaborated into harness tasks
- `ready_to_elaborate`: this is the next batch that should be turned into harness tasks
- `in_elaboration`: a worker is currently drafting or revising the batch
- `tasked`: the harness task specs for this batch already exist
- `blocked`: the batch cannot yet be authored due to an external decision or missing input

## How to Use a Tracker Item

When a tracker item is `ready_to_elaborate`, use its fields as follows:

- `phase`: the plan phase the batch belongs to
- `title`: the batch identity for discussion and review
- `depends_on`: spec-authoring dependencies that must already be `tasked`
- `target_harness_groups`: the groups that should receive the new harness tasks
- `source_refs`: the documents that must be read before writing the task specs
- `summary`: the intended outcome of the batch
- `brief`: the self-contained markdown brief that should be read first
- `spec_authoring_notes`: constraints that should shape the resulting task specs
- `generated_task_ids`: fill this after the real tasks are created

If the `brief` path exists, the brief is the primary bounded context
artifact for the batch.

If the `brief` path is missing, the next required work is to author that
brief before authoring executable harness tasks.

## Brief Authoring Rules

When `next-step` returns `BRIEF_AUTHORING_BATCH`:

1. Open `.harness/spec-batches/BRIEF_TEMPLATE.md`.
2. Open the tracker item referenced by `TRACKER_ID`.
3. Read the files listed in the item's `source_refs`.
4. Read the relevant sections of `docs/plans/asplos-implementation-plan.md`.
5. Create the missing brief at the path shown in `EXPECTED_BRIEF`.

Every brief must contain exactly these sections:

- `Description`
- `Input Context`
- `Output Specification`
- `Implementation Notes`
- `Non-Goals`
- `Acceptance Criteria`
- `Update Instructions`

Every brief must:

- stay bounded to one tracker item
- list the exact files a cold session should read
- list the exact files a cold session should create or modify
- state what is out of scope
- state how the tracker must be updated when the batch is done

## Brief Update Rules

After creating a missing brief:

1. If the brief is complete and landed, leave the current tracker item at `ready_to_elaborate`.
2. If the brief is only partially drafted, change the tracker item to `in_elaboration` and record the note needed for the next session.
3. Do not change the item to `tasked`; brief creation alone does not mean executable harness task specs exist.
4. After a complete brief is created, rerun `uv run python scripts/python/task.py next-step`. The same tracker item should now appear as `TASK_SPEC_BATCH`.

## Authoring Rules

When elaborating a tracker batch into harness tasks:

1. Read the batch brief referenced by `brief`.
2. Read the referenced sections of `docs/plans/asplos-implementation-plan.md`.
3. Read the cited `docs/guide/` source files.
4. Write narrow, testable task specs.
5. Keep each task within one crate or one tightly related crate pair.
6. Include implementation scope, non-goals, test plan, and quality gate in every task.
7. Carry forward caution notes for any accepted-with-caution constructs.
8. Do not invent DSL primitives or backend behavior not justified by the guide.

The batch brief is the primary bounded context artifact for task-spec
authoring. A cold session should not need to infer the work from the YAML
tracker alone.

## Update Rules

After authoring harness task specs for a tracker batch:

1. Change that item's status from `ready_to_elaborate` or `in_elaboration` to `tasked`.
2. Populate `generated_task_ids` with the new task IDs.
3. Move the next dependency-satisfied item to `ready_to_elaborate`.
4. Do not change tracker items that are unrelated to the authored batch.

If a batch is partially drafted but not ready to land:

- mark it `in_elaboration`
- add any needed note directly in the YAML item before ending the session

If a batch cannot proceed:

- mark it `blocked`
- record the reason in the YAML item before ending the session

## Non-Rules

Do not use `.harness/task-spec-status.yaml` to:

- mark code implementation progress
- track test failures during task execution
- replace `.harness/tasks.json`
- decide what to implement when executable tasks already exist

## Current Intent

The current tracker is designed so that:

- executable harness tasks remain the primary workflow
- task-spec authoring is only a fallback mode
- early task-spec work follows the implementation plan in dependency order
- PL011-first sequencing remains enforced at the spec-authoring layer
