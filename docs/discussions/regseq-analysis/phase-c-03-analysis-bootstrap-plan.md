# Phase C-03 Analysis Bootstrap And Lowering-Input Plan

## Purpose

`phase-c-03` should be the last model-side gate before Phase D lowering work.

Its job is not to redesign declaration IR, binding IR, or joined IR. Its job
is to prove that the current normalized PL011 artifact can be consumed through
stable read-only analysis inputs and reviewer-facing validation outputs without
smuggling backend structure into Phase C.

## Boundary

The staged IR family remains:

1. declaration IR
2. binding IR
3. joined IR
4. lowering-oriented inputs
5. backend-specific lowering forms

`phase-c-03` should introduce only the fourth item, and only as a stable,
read-only contract over the existing normalized models.

That means:

- no declaration-schema redesign
- no binding-schema redesign
- no join-mechanism redesign
- no backend-specific data structures
- no code generation behavior

## Analysis Goal

The minimum analysis/bootstrap layer should answer:

- what callbacks and operation classes are present for the current device class
- what resource and property joins are complete, incomplete, declaration-only,
  or binding-only
- what caution facts remain visible to a later lowering phase
- whether the current PL011 artifact is stable enough to act as the input
  contract for Phase D

The analysis layer should be reviewer-facing and deterministic.

## Lowering-Input Contract

For this phase, the lowering-input contract should be a read-only,
rustc-independent view over:

- the normalized declaration model
- the normalized joined artifact

This contract should preserve:

- family ownership
- callback identities at the currently normalized impl-level boundary
- property-lint visibility
- partial-join outcomes
- both-side provenance
- interrupt-hint-vs-binding authority boundaries

It should not:

- flatten all families into one bag
- erase unmatched or incomplete facts
- assume backend-specific resolution policy

## Caution Coverage

The final Phase C bootstrap should explicitly keep visible the caution points
already accepted in the current supported PL011 frontend:

- required-with-default property lint state
- partial join outcomes for supported resource families
- optional-resource presence versus required-resource absence
- interrupt binding hints remaining diagnostic-only
- serial v1 callback-family presence for the supported PL011 artifact

## Recommended Task Split

1. analysis-facing lowering-input bootstrap over declaration + joined IR
2. lowering-input readiness and consistency checks
3. reviewer-facing PL011 lowering-input proof and snapshot coverage
