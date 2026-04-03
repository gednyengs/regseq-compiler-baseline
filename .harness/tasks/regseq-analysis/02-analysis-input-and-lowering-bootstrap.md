# Task: regseq-analysis-02 - Analysis input and lowering bootstrap

## Description

Add the first real analysis-facing input layer over the normalized declaration
model and joined artifact.

This task should define the minimum stable, rustc-independent contract that
later lowering work can read without making `regseq-analysis` redesign the
models or encode backend-specific policy.

## Input Context

Read before starting:

- `docs/discussions/regseq-analysis/phase-c-03-analysis-bootstrap-plan.md`
- `.harness/spec-batches/phase-c-03.md`
- `crates/regseq-model/src/declaration/`
- `crates/regseq-model/src/joined/`
- `.harness/tasks/regseq-model/07-declaration-model-reviewability-and-traversal-baseline.md`
- `.harness/tasks/regseq-model/10-declaration-binding-join-and-joined-artifact-baseline.md`
- `docs/plans/asplos-implementation-plan.md`

## Output Specification

Modify:

- `crates/regseq-analysis/src/`
- `crates/regseq-analysis/tests/`

Add the smallest read-only analysis/bootstrap layer needed to consume the
current declaration model and joined artifact together.

## Implementation Notes

This task should follow these fixed decisions:

- `regseq-analysis` consumes existing normalized model types; it does not
  redefine them
- the analysis/bootstrap input must stay rustc-independent
- the bootstrap contract should preserve family ownership instead of flattening
  all facts into one untyped list
- the bootstrap contract may pair declaration-side and joined-side views, but
  it must not mutate or reinterpret the underlying models

The output should make it easy for later lowering work to answer:

- which callback families are present
- which resource/property families are joined, incomplete, declaration-only, or
  binding-only
- which caution-side facts remain visible for later validation and lowering

The concrete value of this bootstrap layer over direct model consumption is:

- providing a single entry point that pairs a declaration model with its
  corresponding joined artifact, so later consumers do not need to manage the
  two independently
- exposing summary/query helpers that span both inputs, for example retrieving
  a declaration-side resource, its join outcome, and the construct-local
  evidence/provenance needed to review it together
- stabilizing the API surface that Phase D will depend on, so model internals
  can evolve without forcing lowering consumers to track every internal type
  directly

If the existing model types already provide adequate traversal for a given
query, the bootstrap layer should delegate rather than re-implement.

The bootstrap contract must also expose the full declaration model's
operation-evidence families that have no binding-side join path, including:

- register blocks, descriptors, wrapper call sites, and poll call sites
- synchronization field declarations, lock operations, completion operations,
  and guard scopes
- timed-delay call sites and duration constructors
- logging call sites
- PM gate facts

These families are consumed directly by Phase D lowering and must be
accessible through the analysis layer without requiring Phase D to bypass the
bootstrap contract.

The bootstrap contract should explicitly preserve:

- impl-level callback-family presence
- joined resource/property outcomes
- property lint visibility
- both-side provenance where available
- interrupt authoritative binding values plus diagnostic-only declaration hints

Do not:

- introduce backend-specific lowering types
- normalize method-level callback identity beyond the current declaration-model
  boundary
- redesign the declaration or joined schemas
- collapse unmatched or incomplete facts into one success flag

## Test Plan

Add tests to verify:

1. a representative declaration model and joined artifact can be wrapped by the
   bootstrap contract
2. family ownership remains explicit in traversal helpers
3. callback-family presence is queryable without backend assumptions
4. partial join outcomes and unmatched facts remain queryable
5. provenance remains visible through the bootstrap layer
6. declaration-only operation-evidence families (registers, synchronization,
   timed-delay, logging, PM gate) remain accessible through the bootstrap
   contract
7. bootstrap output is deterministic where serialized/debug-printed forms are
   exposed

Test commands:

- `cargo test -p regseq-analysis`
- `cargo clippy -p regseq-analysis -- -D warnings`

## Acceptance Criteria

- `regseq-analysis` exposes a stable read-only input/bootstrap layer over the
  current declaration and joined models
- family ownership and caution facts remain explicit
- no backend-specific policy is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
