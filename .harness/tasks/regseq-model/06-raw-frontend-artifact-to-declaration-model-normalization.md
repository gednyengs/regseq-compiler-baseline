# Task: regseq-model-06 - Raw frontend artifact to declaration-model normalization

## Description

Implement normalization from `regseq-driver`'s `RawFrontendArtifact` into the
new declaration-model types.

This task is the bridge from raw extraction into the first internal IR layer.
It should normalize the current raw artifact deterministically and locally,
without silently introducing new extraction families or join logic.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-01-declaration-model-plan.md`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/`
- `.harness/tasks/regseq-model/05-declaration-model-schema-and-boundaries.md`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Add the normalization entry points and any conversion helpers needed to map
`RawFrontendArtifact` into the declaration model.

## Implementation Notes

`regseq-model` must remain rustc-independent.

That means this task must not solve normalization by adding a dependency from
`regseq-model` to `regseq-driver`.

Use a rustc-independent boundary for raw-artifact input. Prefer:

- local serde/deserialization-only mirror input types in `regseq-model`, fed
  from raw-artifact JSON

Do not choose an approach that couples `regseq-model` to `regseq-driver`'s
nightly rustc dependency graph inside this task.

Normalize from the current raw artifact as it exists.

That means:

- convert stringly tags into typed enums where the raw artifact already carries
  finite method/kind strings
- keep open-ended strings as strings when they are not finite vocabularies from
  the raw artifact
- preserve construct ownership while normalizing
- preserve `containing_fn`-style provenance on operation evidence
- keep callback-family normalization at impl level for now
- keep interrupt-carried Layer 2 details as opaque sidecar/binding-hint data if
  they are preserved at all
- normalize property-side lints rather than silently dropping them
- keep synchronization sub-families distinct rather than merging them into one
  generic synchronization bucket
- preserve logging and timed-delay call-site detail rather than collapsing them
  to presence-only markers

Finite raw vocabularies that should become typed enums include method/kind
strings such as wrapper methods, clock methods, reset methods, completion
methods, synchronization field kinds, delay intrinsics, delay path kinds,
duration constructors, log levels, log argument kinds, interrupt field shapes,
ISR bind targets, and callback contexts.

Open-ended strings that should remain strings include things like
`register_block_type`, `state_type`, and property `field_type`.

Do not:

- add hidden raw-artifact prerequisites inside this task
- infer ordered regions that are not present in `RawFrontendArtifact`
- infer method-level callback data that the raw artifact does not carry
- collapse all construct-local operation evidence into one unowned global bag
- perform Layer 1 / Layer 2 joins

This task should also define the normalization stance for raw-artifact gaps:

- if a raw family is absent, normalize its absence honestly
- do not synthesize placeholder declaration facts to make the model look more
  complete than the raw artifact actually is

## Test Plan

Add normalization tests to verify:

1. a representative raw frontend artifact normalizes successfully
2. all currently present raw families map into the declaration model
3. stringly raw kinds become typed declaration-model enums
4. construct-local call-site evidence retains `containing_fn` provenance
5. interrupt Layer 2 leakage is kept separate from declaration facts
6. absent families remain absent rather than being inferred
7. property-side lints survive normalization
8. synchronization sub-families stay structurally distinct
9. logging/timed-delay detail survives normalization

Do not rely only on `tests/fixtures/pl011-spec-slice/expected/raw-frontend.json`
for coverage. Add at least one richer normalization test input with populated
surfaces for families that are empty in the spec-slice artifact, such as
interrupts, synchronization, timed-delay, logging, and PM gate.

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- `RawFrontendArtifact` can be normalized into the declaration model
- the normalization boundary keeps `regseq-model` rustc-independent
- normalization is deterministic and local
- construct ownership and per-entity provenance survive normalization
- no hidden join logic or raw-artifact expansion is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
