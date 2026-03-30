# Compiler Collateral

Compiler-facing package derived from the accepted primitive-mapping and Pass 1
closure work.

This package is not a new normative methodology. It is a compiler-consumer
projection of the already accepted sources of truth:

- `regseq-primitive-mapping-framework.md`
- `primitive-mapping-tracker.md`
- `construct-elaborations/`
- `pass1-notes/`
- `full-device-audit/`

## Purpose

The goal of this package is to make compiler work mechanically actionable
without requiring implementers to read the entire research tree.

It provides three artifact layers:

1. `compiler-baseline-overview.md`
   - the cross-construct implementation baseline
2. `compiler-constraints.md`
   - shared implementation constraints and audit carry-forwards
3. `constructs/`
   - one compiler-facing document per construct

## Relationship to Existing Handoff Files

This package does not replace:

- `construct-catalog.md`
- `lowering-contract.md`
- `track-A-implementation-guide.md`

Instead:

- `construct-catalog.md` remains the readable inventory
- `lowering-contract.md` remains the earlier construct-by-construct lowering
  reference
- `track-A-implementation-guide.md` remains implementation-planning guidance
- this package is the compact, compiler-consumer baseline derived after
  primitive mapping, Pass 1 closure, and the PL011 full-device audit

## Scope

This package currently targets:

- the full accepted PL011 baseline
- the accepted E1000-only construct extensions:
  - Network Interface Management
  - Parameter Registration
  - Deferred work item scheduling

## Status

- package schema: established
- overview: drafted and validated
- shared constraints: drafted and validated
- per-construct compiler docs: complete and validated
- package-level review: accepted with small global patch
