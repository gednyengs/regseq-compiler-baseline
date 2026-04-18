# Spec Batch: regseqmodel-phase-9 - Old module cleanup and final tests

## Description

This batch authors executable harness task specs for the final
cleanup of the RegseqModel refactor. Three deliverables:

1. **Delete the old model modules** (`declaration/`, `binding/`,
   `joined/`, `semantic/`, `item/`) and the temporary `compat.rs`
   adapter from Phase 7.
2. **Regenerate golden files** with the new IR/JSON serialization
   format (adjacently-tagged enums, omitted-when-None options,
   `format_version`, `Symbol`/`FileId` rendered as resolved strings)
   per ir-reference.md Section 7.
3. **Final test cleanup**: remove old-model assertions, add
   `RegseqModel`-shaped assertions where coverage is missing.

This is the last phase. Phases 7 and 8 must already be `tasked` and
their tasks complete -- compat removal can only happen after every
consumer is confirmed off it, and old-module deletion can only
happen after every reader is gone.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 9)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 7
  -- the JSON serialization contract goldens must match)
- `crates/regseq-model/src/lib.rs` (module declarations to remove)
- `crates/regseq-model/src/declaration/` (entire directory --
  deletion target)
- `crates/regseq-model/src/binding/`
- `crates/regseq-model/src/joined/`
- `crates/regseq-model/src/semantic/`
- `crates/regseq-model/src/item/`
- `crates/regseq-model/src/compat.rs` (Phase 7 adapter --
  deletion target after consumer migration is complete)
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` (the
  primary frontend golden -- regenerate against new format if
  format_version changed; otherwise leave intact)
- `tests/fixtures/` (sweep for any stale golden assertions
  referencing old-model JSON shape)
- `tests/e2e/tests/` (every file -- sweep for old-model assertions)
- `crates/regseq-driver/tests/` (33+ unit-test specimens already
  updated in Phase 4; sweep for any remaining old-model
  references)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task specs (compat removal,
  per-old-module deletion, lib.rs cleanup)
- `.harness/tasks/track-a-validation/` task specs (golden
  regeneration, E2E and unit-test assertion cleanup)

Update when done:

- `.harness/task-spec-status.yaml` (mark `regseqmodel-phase-9`
  tasked, populate `generated_task_ids`; all tracker items should
  now be `tasked`)
- `.harness/tasks.json`

## Implementation Notes

- **Deletion gates** (in order, each gate must be cleared before
  the next deletion):
  1. **Confirm zero consumers of `compat.rs`.** Grep across
     `crates/` (excluding `regseq-model/src/compat.rs` itself)
     for any `use regseq_model::compat::` or equivalent
     paths. Result must be empty before compat is deleted.
  2. **Delete `compat.rs`.** Update
     `crates/regseq-model/src/lib.rs` to remove the `pub mod
     compat;` line. `cargo check --workspace` must pass.
  3. **Confirm zero consumers of each old module
     (`declaration`, `binding`, `joined`, `semantic`, `item`).**
     Grep for `use regseq_model::declaration::`, etc. Result
     must be empty per module before that module is deleted.
  4. **Delete one old module at a time**, removing its `pub mod`
     declaration from `lib.rs` and running `cargo check --workspace`
     between each. If the build breaks, restore and identify the
     remaining caller (likely a missed migration in Phase 7).
- **Golden regeneration policy:**
  - The frontend artifact (`tests/fixtures/pl011-full-spec/
    expected/raw-frontend.json`) is produced by `regseq-driver`'s
    extraction stage and was last regenerated in Phase 4
    (regseq-driver-34). Re-regenerate ONLY if Phase 9 deletions
    or final cleanup change its content. If unchanged, leave
    intact and document.
  - The new RegseqModel/IR JSON goldens (Phase 5/6 outputs) are
    locked under the regseq-model crate's tests. These are the
    canonical IR-shape golden artifacts. Regenerate any whose
    format drifted between when they were authored and final
    deletion (e.g., if old modules' presence affected
    serialization order).
  - The serialization contract is the source of truth for golden
    shape: adjacently-tagged enums (`{"kind": ..., "data": ...}`),
    optional fields omitted (not `null`), `Symbol` and `FileId`
    rendered as resolved strings, deterministic collection
    ordering. Goldens that violate this contract are wrong and
    must be regenerated, not pinned.
- **E2E and unit-test cleanup:**
  - Remove every assertion still keyed on `JoinedArtifact`,
    `DeclarationModel`, `BindingModel`, `SemanticModel`, or
    `Item`. Replace with the equivalent `RegseqModel` assertion
    (binding population, module tree shape, IR body structure,
    intrinsic metadata).
  - Delete tests whose entire purpose was to validate the old
    model shape and have no `RegseqModel` analog.
  - Where `RegseqModel`-side coverage is genuinely missing
    (e.g., a behavior the old tests asserted but no new test
    exercises), add a focused new test rather than leaving the
    gap.
- **Phase boundary.** Phase 9 ends with: zero references to old
  model types in any production crate, zero
  `compat`/`declaration`/`binding`/`joined`/`semantic`/`item`
  files under `crates/regseq-model/src/`, all goldens regenerated
  and matching the IR JSON contract, and `cargo test --workspace`
  passing. There is no Phase 10.

## Non-Goals

- No new features.
- No new model types or IR constructs.
- No Zephyr or E1000 work.
- No structured-control-flow recovery in C emission (separate
  follow-up).
- No re-architecting of the verifier or pipeline (Phase 6 owns
  that).

## Acceptance Criteria

- After all Phase 9 tasks land:
  - `find crates/regseq-model/src -type d -name declaration -o
    -name binding -o -name joined -o -name semantic -o -name item`
    returns empty.
  - `find crates/regseq-model/src -name compat.rs` returns empty.
  - `crates/regseq-model/src/lib.rs` has no `pub mod` lines for
    the deleted modules or compat.
  - Workspace-wide grep for old-model imports
    (`use regseq_model::{declaration,binding,joined,semantic,item,compat}::`)
    returns zero hits.
  - `cargo build --workspace` and `cargo test --workspace` both
    pass.
  - `cargo fmt --check` and `cargo clippy --workspace` both pass.
- Per-module deletion is its own task with the corresponding
  pre-deletion grep evidence captured in the task notes.
- The compat-removal task explicitly demonstrates (via grep) that
  no consumer remains; the task does not run if any old import
  exists.
- Goldens that were regenerated produce byte-identical output on
  a second `--regenerate` invocation (determinism check).
- Goldens that were NOT regenerated produce byte-identical output
  to the committed version when re-serialized through the
  current code (drift check).
- All previously-skipped or `#[ignore]`d tests added during
  migration are either re-enabled and passing, or explicitly
  deleted with rationale in the commit message.
- Tracker updated: `regseqmodel-phase-9` is `tasked`; every other
  tracker item is also `tasked`.

## Update Instructions

1. Change `regseqmodel-phase-9` status to `tasked`.
2. Fill `generated_task_ids`.
3. Confirm all tracker items are now `tasked` (no `not_ready`,
   `ready_to_elaborate`, or `in_elaboration` items remain).
