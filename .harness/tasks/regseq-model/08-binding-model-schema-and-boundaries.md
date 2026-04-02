# Task: regseq-model-08 - Binding-model schema and boundaries

## Description

Define the first normalized binding-model types in `regseq-model` for the
accepted Layer 2 platform-file contract.

This task establishes schema only. It should not perform joins, declaration
redesign, backend shaping, or parser-side contract expansion.

The goal is to create stable rustc-independent binding types that preserve
typed Layer 2 values, join-relevant identifiers, section-local structure, and
per-entity provenance.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-02-binding-and-join-plan.md`
- `crates/regseq-model/src/platform.rs`
- `crates/regseq-model/src/platform_parser.rs`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/compiler-constraints.md`
- `.harness/spec-batches/phase-c-02.md`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Create whatever modules are needed for the binding-model type family, but keep
binding-model types clearly separated from both:

- declaration-model types under `crates/regseq-model/src/declaration/`
- the parser-facing `platform.rs` / `platform_parser.rs` representation

## Implementation Notes

The binding model should follow these fixed decisions:

- it is normalized Layer 2 binding IR only
- it coexists with declaration, joined, and lowering-oriented IR families
- it starts from the accepted parser output shape as-is
- it preserves section ownership rather than flattening all Layer 2 data into
  one generic property map
- provenance must be attached per normalized binding entity, not only at the
  top level

The schema should be able to represent normalized forms for:

- platform header facts:
  - version
  - os
  - arch
  - board
- property binding entries and typed values
- mmio binding entries
- interrupt binding entries
- clock binding entries
- reset binding entries
- pinctrl binding entries and available states

For construct sections, the binding schema should use typed per-family entries
rather than a generic `BTreeMap<String, PlatformValue>` passthrough.

Binding-entry fields should be individually optional where needed so
matched-but-incomplete join outcomes can be represented structurally later,
rather than being compressed into one summary completeness flag.

The accepted binding-side field contracts for this batch are:

- `[mmio.{id}]`
  - `base`
  - `size`
- `[interrupt.{id}]`
  - `trigger_type`
  - `trigger_polarity`
  - `priority`
  - `shared`
- `[clock.{id}]`
  - `default_frequency`
- `[reset.{field_name}]`
  - `reset_line_id`
  - `reset_shared`
- `[pinctrl.{id_or_field_name}]`
  - `pin_states`

Current boundary decisions that the schema must encode explicitly:

- the binding model should not depend on declaration IR to be well-formed
- explicit-id join keys and field-name join keys should have an explicit home
- mmio is included in binding normalization for this batch, but reviewer-facing
  validation may still focus on the resource families most exercised by the
  current PL011 frontend
- unrecognized future sections preserved by the parser are not normalized in
  this task unless the accepted Layer 2 contract already gives them meaning
- build/deployment metadata from Layer 2 stays in the binding model; it does
  not become declaration identity

Do not include:

- declaration-side facts
- joined artifact types
- parser redesign
- new Layer 2 sections or widened value kinds
- lowering-oriented backend decisions
- DMA-specific expansion

## Test Plan

Add schema-focused tests to verify:

1. binding-model types construct and import cleanly
2. section-owned sub-families remain structurally distinct
3. per-entity provenance fields are present where expected
4. header/build metadata is separate from resource/property binding entries
5. explicit-id and field-name join-relevant identifiers have an explicit home
6. accepted construct-section fields have a typed home per family rather than
   remaining generic key-value bags

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- binding-model types exist in `regseq-model`
- binding-model modules are separated from declaration-model and parser-facing
  modules
- the schema preserves section ownership and per-entity provenance
- the schema does not assume declaration-aware inference or joined facts
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
