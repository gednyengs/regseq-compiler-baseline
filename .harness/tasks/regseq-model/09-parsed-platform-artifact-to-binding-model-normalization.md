# Task: regseq-model-09 - Parsed platform artifact to binding-model normalization

## Description

Implement normalization from the accepted parser-facing Layer 2 platform-file
representation into the new binding-model types.

This task is the bridge from parsed Layer 2 input into the second internal IR
layer. It should normalize the accepted parser output deterministically and
locally, without silently widening the Layer 2 contract or introducing joins.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-02-binding-and-join-plan.md`
- `crates/regseq-model/src/platform.rs`
- `crates/regseq-model/src/platform_parser.rs`
- `.harness/tasks/regseq-model/08-binding-model-schema-and-boundaries.md`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Add the normalization entry points and any conversion helpers needed to map the
accepted parsed Layer 2 representation into the binding model.

## Implementation Notes

Normalize from the current accepted parser output as it exists.

That means:

- preserve typed property values from the accepted Layer 2 subset
- preserve section ownership while normalizing
- preserve section/key-level provenance on binding entities
- preserve join-relevant identifiers explicitly
- treat construct-section dispatch and typed extraction as the core
  normalization responsibility for this task
- keep unknown preserved parser sections out of the binding model unless they
  already belong to the accepted Layer 2 contract

Accepted construct-section kinds and field contracts for normalization are:

- `mmio`
  - `base`
  - `size`
- `interrupt`
  - `trigger_type`
  - `trigger_polarity`
  - `priority`
  - `shared`
- `clock`
  - `default_frequency`
- `reset`
  - `reset_line_id`
  - `reset_shared`
- `pinctrl`
  - `pin_states`

Do not:

- add hidden parser prerequisites inside this task
- widen accepted Layer 2 value kinds or section meanings
- infer declaration-side defaults or completeness
- perform Layer 1 / Layer 2 joins
- collapse all resource/property bindings into one unowned map

This task should also define the normalization stance for absence:

- if a parsed section is absent, normalize its absence honestly
- do not synthesize binding entries to make the model look more complete than
  the parser output actually is

## Test Plan

Add normalization tests to verify:

1. a representative parsed platform artifact normalizes successfully
2. all currently accepted Layer 2 sections map into the binding model
3. typed property values survive normalization
4. join-relevant identifiers survive normalization
5. section/key provenance survives normalization
6. absent sections remain absent rather than being inferred

Use both:

- the existing parser-oriented platform fixtures
- at least one richer Layer 2 input that exercises resource sections beyond
  `[properties]`

If existing checked-in PL011 platform fixtures are too narrow for this, it is
acceptable for this task to use synthetic parsed inputs with concrete construct
sections matching the accepted field contracts above.

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- parsed Layer 2 artifacts can be normalized into the binding model
- normalization is deterministic and local
- section ownership and per-entity provenance survive normalization
- no hidden join logic or Layer 2 contract expansion is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
