# Task: regseq-model-05 - Declaration-model schema and boundaries

## Description

Define the first normalized declaration-model types in `regseq-model` for the
supported PL011 frontend.

This task establishes the schema only. It should not implement Layer 2
binding normalization, join logic, backend-oriented reshaping, or hidden raw
extraction expansion.

The goal is to create stable rustc-independent declaration types that preserve
construct ownership, typed distinctions, and per-entity provenance.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-01-declaration-model-plan.md`
- `crates/regseq-driver/src/raw_artifact.rs`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/compiler-constraints.md`
- `.harness/spec-batches/phase-c-01.md`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Create whatever modules are needed for the declaration-model type family, but
keep declaration-model types clearly separated from the existing Layer 2
platform-file types in this crate.

## Implementation Notes

The declaration model should follow these fixed decisions:

- it is declaration IR only
- it coexists with later binding/join/lowering-oriented IR families
- it starts from the current `RawFrontendArtifact` shape as-is
- it preserves construct ownership rather than flattening all facts into one
  generic bag
- provenance must be attached per normalized entity, not only in top-level
  metadata

The schema should be able to represent normalized forms for:

- driver identity and device class
- build-context metadata kept separate from declaration identity
- callback-family impl declarations and callback contexts
- resource declarations:
  - mmio
  - interrupts
  - clock
  - reset
  - pinctrl
  - synchronization
- firmware-property declarations and property-side lints
- register blocks and register descriptor facts
- construct-local operation evidence
- synchronization sub-families kept structurally distinct:
  - sync field declarations
  - lock operations
  - completion operations
  - guard scopes
- timed-delay declaration-side facts already present in the raw artifact,
  including call-site and duration-constructor detail
- logging declaration-side facts already present in the raw artifact,
  including level/template/argument-shape detail
- PM gate declaration-side facts already present in the raw artifact

Current boundary decisions that the schema must encode explicitly:

- callback normalization is impl-level for now, not method-level
- `target_os` and similar build settings belong in build-context metadata, not
  declaration identity
- interrupt-carried Layer 2 facts are not normalized as declaration facts; they
  should have an explicit sidecar/binding-hint home if preserved
- ordered regions are not yet in `RawFrontendArtifact`; do not force ordered
  region support into this task's schema unless the raw-artifact prerequisite is
  added explicitly in a separate task

The top-level model may include a supplementary traversal/index structure, but
operation evidence must remain primarily construct-owned.

Document the current extraction limitation for register-wrapper call sites:

- the raw artifact preserves wrapper method and `containing_fn`
- it does not preserve the concrete target register for each wrapper call site
- the declaration model should normalize this honestly rather than pretending
  the target-register fact is available

Do not include:

- Layer 2 platform/binding normalization
- join results
- backend/lowering-focused nodes
- DMA-specific expansion
- hidden prerequisites on raw extraction changes

## Test Plan

Add schema-focused tests to verify:

1. the declaration-model types construct and import cleanly
2. construct-owned sub-families remain structurally distinct
3. per-entity provenance fields are present where expected
4. build-context metadata is separate from declaration identity
5. interrupt binding-hint / sidecar placement is distinct from declaration facts
6. property-side lints have an explicit home in the schema
7. synchronization, timed-delay, logging, and PM gate each retain distinct
   typed sub-families rather than collapsing into one opaque bucket

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- declaration-model types exist in `regseq-model`
- declaration-model modules are separated from Layer 2 platform-file modules
- the schema preserves construct ownership and per-entity provenance
- the schema does not assume ordered-region or method-level callback facts that
  the current raw artifact does not provide
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
