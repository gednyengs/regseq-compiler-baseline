# Phase C-02 Binding And Join Plan

## Purpose

`phase-c-02` should add the second internal-model layer:

- a normalized Layer 2 binding model derived from the accepted platform-file
  contract
- a real declaration-plus-binding join path
- a reviewable joined artifact for the supported PL011 frontend

This phase is where the compiler first combines authored Layer 1 declarations
with deployment-side Layer 2 data.

## IR Boundary

The IR families remain staged and coexisting:

1. declaration IR
2. binding IR
3. joined IR
4. lowering-oriented IR

`phase-c-02` should only introduce the second and third families.

It must not:

- redesign declaration IR
- widen the accepted Layer 2 file contract
- pull backend/lowering structure into the joined model
- treat joined facts as if they were authored declaration facts

## Binding Model Scope

The normalized binding model should be informed by the accepted parser output
and existing Phase B Layer 2 contract only.

For the current supported subset, it should cover:

- platform header facts:
  - version
  - os
  - arch
  - board
- `[properties]` values with accepted scalar/aggregate typing
- supported construct-side Layer 2 resource sections already established in
  Phase B and exercised by the current PL011 frontend:
  - interrupts
  - clocks
  - resets
  - pinctrl

The binding model should preserve:

- typed values
- exact join-relevant identifiers
- resource optionality and sharing hints
- enough provenance to trace back to parsed Layer 2 sections/keys

It should not:

- invent new Layer 2 constructs
- perform declaration-aware inference during binding normalization
- collapse all sections into one untyped key-value bag

## Join Model Scope

The join should combine:

- normalized declaration IR
- normalized binding IR

into a reviewable joined artifact.

For `phase-c-02`, the join should focus on the accepted join mechanisms:

- explicit attribute/id join
- field-name join

The joined artifact should make the following relationships explicit:

- property declaration -> Layer 2 property value presence/absence
- interrupt resource declaration -> Layer 2 interrupt binding
- clock field declaration -> Layer 2 clock binding
- reset field declaration -> Layer 2 reset binding
- pinctrl field declaration -> Layer 2 pin-state inventory

The joined artifact should preserve enough traceability to answer:

- what declaration joined with what binding entry
- what remained unjoined
- what came from Layer 1 only vs Layer 2 only

## Reviewability Rules

The joined artifact must be reviewer-facing and stable.

That means:

- deterministic serialization/snapshot output
- explicit joined vs unjoined structure
- explicit provenance back to declaration and binding inputs
- no hidden backend-oriented lowering data

## Known Cautions

1. `phase-c-02` should not silently reinterpret interrupt-side Layer 2 hints
   that were preserved as opaque sidecar data in declaration IR. The actual
   binding model must remain the authoritative Layer 2 source.
2. Property completeness/default reasoning should remain bounded to the accepted
   join contract and should not drift into broader lowering policy.
3. The join should preserve unjoined facts honestly rather than forcing all
   declarations/resources to appear fully resolved.

## Recommended Task Split

1. binding-model schema and boundaries
2. parsed Layer 2 artifact to binding-model normalization
3. declaration-plus-binding join and joined-artifact serialization
4. focused validation over the joined PL011 artifact
