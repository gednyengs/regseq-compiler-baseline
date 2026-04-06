# Task: regseq-lowering-07 - Supported Regseq semantic-body lowering validated on the PL011 representative set

## Description

Implement semantic-body lowering for the supported Regseq construct slice using
the new semantic procedural IR, and validate it against the representative
PL011 full-spec function set.

This task is **not** about inventing PL011-specific IR. The lowering must be
general Regseq lowering for the supported construct families, with PL011 used
only as the first evidence and acceptance fixture.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md`
- `docs/discussions/semantic-procedural-ir/hir-mir-coverage-matrix.md`
- `docs/discussions/semantic-procedural-ir/acceptance-example-set.md`
- `docs/discussions/semantic-procedural-ir/negative-validation.md`
- `.harness/tasks/regseq-lowering/06-item-hierarchy-construction-from-enriched-extraction.md`
- `.harness/tasks/track-a-validation/10-pl011-full-spec-lowering-completeness-and-backend-independence-proof.md`
- `crates/regseq-backends/src/lowering/`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`
- reviewer-facing lowering snapshots only if they are narrow, deterministic,
  and directly tied to semantic-body inspection

Add semantic lowering support that maps the supported MIR/HIR construct slice
into the new semantic IR for the representative PL011 function set and any
additional helper bodies transitively required by that set.

## Implementation Notes

### Scope

This task owns lowering for the supported construct families, including:

- local environment construction
- assignment / arithmetic / casts
- structured control flow
- option carriers
- fallible `?` lowering
- register read/write/modify
- resource operations and property reads
- ordered regions
- interrupt-class tests
- helper-call lowering with structured refs

### Representative Acceptance Set

The lowering must be validated against the representative functions already
identified in the semantic-IR discussion:

- `uart_clock_hz`
- `set_baud_rate`
- `init`
- `service_tx_fifo`
- `service_rx_fifo`
- `handle_irq`
- required resource helpers

If additional helper bodies are transitively required to lower these correctly,
they are in scope too.

### Required Internal Review Gate

This task must be implemented and reviewed in two explicit slices:

1. narrow subset first:
   - `uart_clock_hz`
   - `set_baud_rate`
   - `init`
2. full representative set second

The first subset must produce reviewable semantic-body evidence before the full
set is considered complete. Do not treat this as an informal recommendation.

### Generalization Rule

The lowering must not introduce:

- PL011-specific semantic nodes
- UART-only IR variants
- Linux-specific semantics in the IR
- handwritten device behavior templates

PL011 is the first acceptance fixture, not the shape of the IR.

### Validation Passes

This task must add and exercise semantic validation over the produced bodies,
including at minimum:

- no unresolved/Opaque backend-facing types in supported bodies
- `OptionUnwrap` domination validation
- no stringly lowering-critical refs remain in produced semantic bodies
- all `SemanticLocal.name` values are valid C identifiers
- no raw MIR local spellings such as `_3`, `_14`, or similar residue in
  produced semantic-local names
- unsupported MIR constructs fail explicitly rather than degrade silently

### Reviewability Requirement

This task must leave behind reviewer-friendly semantic-body evidence, such as:

- deterministic textual dumps
- structured snapshot tests
- narrow inspection helpers in tests

The acceptance examples must be inspectable as semantic IR, not inferred from
backend output.

### Non-Goals / Negative Constraints

Do not in this task:

- migrate `DriverLoweringPlan.behavior`
- rewrite Linux emitter code
- encode backend callback mappings
- patch around missing lowering by letting backends reconstruct semantics

## Test Plan

Add tests to verify:

1. the narrow representative subset lowers into semantic bodies successfully
2. the full representative function set lowers into semantic bodies successfully
3. `uart_clock_hz` lowers with explicit resource/default semantics
4. `set_baud_rate` lowers with typed locals and register writes/modifies
5. `init` lowers with explicit fallible sequencing
6. `service_tx_fifo` and `service_rx_fifo` lower with explicit option-carrier
   semantics rather than implicit truthiness
7. `handle_irq` lowers with explicit interrupt tests and accumulated result flow
8. semantic validation rejects unmatched `OptionUnwrap`, unresolved backend-
   facing residue, and raw MIR local-name leakage
9. the produced bodies remain device-neutral and OS-neutral in representation

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- the supported Regseq construct slice lowers into semantic bodies using the
  new IR
- the narrow subset and the full representative set both have deterministic,
  inspectable semantic-body evidence
- no PL011-specific or Linux-specific semantic nodes are introduced
- option carriers, fallible flow, register/resource ops, and interrupt tests
  are all represented explicitly in the semantic IR
- semantic validation passes catch the known historical failure modes,
  including raw MIR local-name leakage
- no later backend phase is required to reconstruct Rust/MIR meaning from the
  semantic bodies
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
