# Spec Batch: regseqmodel-phase-6 - Normalization, join, and verification

## Description

This batch authors executable harness task specs for **pipeline
integration** of the Phase 5 IR construction work. Three deliverables:

1. `normalize(input: &RawArtifactInput) -> Result<RegseqModel, NormalizeError>`
   that produces a Phase-1 `RegseqModel` (declaration-side fields
   populated, every binding slot `None`).
2. `populate_bindings(model: &mut RegseqModel, platform: &PlatformFile)
   -> Result<(), JoinError>` that mutates the model in place from a
   parsed Layer-2 platform file.
3. **Phase legality verification** (post-Phase-1 and post-Phase-2
   checks) plus the "verify after every transformation" invariant
   (spec.md Section 13).

This phase **does not** re-define IR construction. `normalize()` is a
thin wrapper around the construction pass authored in Phase 5 plus
provenance/error handling. Phase 6 task specs must not modify
`construction.rs`.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 6,
  including the consumer-migration cascade risk row that downstream
  Phase 7 inherits from this phase's API surface)
- `docs/discussions/models/regseq-model/spec.md` (Sections 11
  Platform Binding, 12 Normalization Contract, 13 Verification)
- `crates/regseq-model/src/ir/construction.rs` (Phase 5 output -- the
  function `normalize()` wraps; do not modify)
- `crates/regseq-model/src/model/regseq_model.rs` (Phase 2 root struct
  that `normalize()` returns)
- `crates/regseq-model/src/platform_parser.rs` (parsed Layer-2 input
  consumed by `populate_bindings()`)
- `crates/regseq-driver/src/raw_artifact.rs` (`RawFrontendArtifact`
  schema -- the input shape `normalize()` consumes)
- `crates/regseq-driver/src/emit_driver.rs` (current pipeline
  entrypoint that will eventually invoke `normalize()`)
- `crates/regseq-model/src/declaration/normalize.rs` (old
  normalization, for reference only -- being replaced)
- `crates/regseq-model/src/joined/join.rs` (old join logic, for
  reference only -- being replaced)
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` (input
  fixture for the end-to-end happy-path test)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files (one per deliverable
  -- `normalize()`, `populate_bindings()`, post-Phase-1 verifier,
  post-Phase-2 verifier, and the always-on transformation verifier;
  may be combined where dependencies allow)

Update when done:

- `.harness/task-spec-status.yaml` (mark `regseqmodel-phase-6`
  tasked, populate `generated_task_ids`, advance
  `regseqmodel-phase-7` to `ready_to_elaborate`)
- `.harness/tasks.json`

## Implementation Notes

- **Determinism contract** (spec.md Section 12): the same
  `RawFrontendArtifact` MUST produce a byte-identical `RegseqModel`
  serialization across runs. Sort-before-serialize on every
  collection. Tests must assert this.
- **Locally-scoped normalization** (spec.md Section 12): each module
  and item normalizes independently. Cross-references resolve only
  via typed `DeclId`s already assigned by Phase 5 Pass 1. No
  string-path lookup at normalization time.
- **Provenance** (spec.md Section 12): every normalized entity must
  be traceable to its originating `RawFrontendArtifact` surface and
  containing item. Errors must cite the surface and item.
- **`normalize()` error conditions** (spec.md Section 12):
  unrecognized enum tag in raw input, missing required attribute
  (e.g., `clock` without `id`), structural violation (e.g., driver
  struct in multiple modules), type-constraint violation (e.g., user
  data field not `Option<T: Copy>`). Each MUST produce an explicit
  `NormalizeError` variant -- never a silent default.
- **`populate_bindings()` semantics** (spec.md Section 11):
  - In-place mutation on `&mut RegseqModel`. No new model is
    constructed; existing IDs and bodies stay put.
  - **ID-matched join**: resources matched by declared `id` against
    `[clock.<id>]`, `[interrupt.<id>]`, `[mmio.<id>]`,
    `[reset.<id>]`, `[pinctrl.<id>]` sections.
  - **Property binding**: `bound_value` slots populated from
    `[properties]` keyed by `property_id`.
  - **Platform metadata**: `[platform]` and `[bus]` sections stored
    alongside driver metadata (spec.md Section 11).
  - **Required vs optional**: a required (`T`) resource declaration
    with no platform section is a hard error; an optional
    (`Option<T>`) declaration with no platform section leaves the
    binding slot `None` and is valid.
  - **Unknown sections**: a platform section referencing an `id`
    with no matching declaration is a hard error.
  - **Type mismatch**: a property's bound value whose type does not
    match the declared `field_type` is a hard error.
- **Post-Phase-1 verifier** (spec.md Section 13):
  - Every declaration-side field populated.
  - Every binding slot `None` (a populated slot indicates Phase
    boundary leakage).
  - Module-tree invariants: exactly one `Driver` module, no
    duplicate item names within a module, all cross-references
    resolve.
- **Post-Phase-2 verifier** (spec.md Section 13):
  - Every required resource binding populated.
  - Every required property `bound_value` populated.
  - Every platform-file `id` matched a declaration.
- **"Verify after every transformation"** (spec.md Section 13): the
  verifier is built into the pipeline infrastructure, not left to
  callers. Any function that mutates a `RegseqModel` runs the
  appropriate level of verification (module / driver-struct /
  body / cross-item) before returning Ok. Document this contract
  in `RegseqModel`'s mutation API.
- Verifier levels per spec.md Section 13: module, driver struct,
  body, cross-item. Body-level checks include "no `sleep()` in ISR
  context" and "no writes to read-only registers" -- defer those
  specific checks if Phase 5 did not surface the metadata they
  require, but the verifier scaffolding must be in place.
- **Phase boundary.** Phase 6 ends with a callable
  `normalize()` -> verifier -> `populate_bindings()` -> verifier
  pipeline that produces a fully-populated `RegseqModel` for PL011.
  **Do not** migrate consumers (Phase 7) or change emission logic
  (Phase 8) in Phase 6 tasks.

## Non-Goals

- No new IR construction logic (Phase 5 owns that).
- No consumer migration (Phase 7).
- No backend changes (Phase 8).
- No backend-specific legality checks (e.g., Linux interrupt
  trigger_type) beyond the verifier hook -- those land with the
  affected backend.

## Acceptance Criteria

- Full PL011 happy path: `RawFrontendArtifact` JSON fixture →
  `normalize()` → post-Phase-1 verify → `populate_bindings()` with
  PL011 platform file → post-Phase-2 verify → success, with the
  resulting `RegseqModel` matching a serialized JSON golden.
- Determinism test: running `normalize()` twice on the same input
  produces byte-identical serializations.
- **Negative tests** (one assertion per case):
  - Unrecognized enum tag in raw input → `NormalizeError`.
  - Missing required attribute (e.g., `clock` without `id`) →
    `NormalizeError`.
  - User data field with non-`Option<T>` type → `NormalizeError`.
  - Two driver structs in the spec → `NormalizeError`.
  - Phase-1 output mutated to leave a binding populated → verifier
    rejects (proves the post-Phase-1 invariant is enforced).
  - Platform section with an `id` that has no declaration →
    `JoinError`.
  - Required resource with no platform section → `JoinError`.
  - Property type mismatch → `JoinError`.
- The mutation API for `RegseqModel` triggers verification on
  every code path; a unit test confirms a deliberately invalid
  in-place mutation surfaces a verifier error rather than silent
  corruption.
- Tracker updated; `regseqmodel-phase-7` advances to
  `ready_to_elaborate`.

## Update Instructions

1. Change `regseqmodel-phase-6` status to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-7` to `ready_to_elaborate`.
