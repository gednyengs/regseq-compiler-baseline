# Task: regseq-driver-24 - Semantic IR Phase 0 extraction enrichment

## Description

Enrich the frontend extraction pipeline so the compiler carries the canonical
identity and ownership facts required by the semantic procedural IR before any
new lowering work begins.

This task is Phase 0 from
`docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`.
It exists because the approved semantic-IR plan requires extraction-time facts
that are not fully present in the current raw frontend path:

- canonical item identity (`CanonicalPath`)
- enough HIR ownership data to reconstruct logical item containment
- associated-constant ownership
- field-level identity for cross-linking
- ISR marker capture for later semantic-role assignment

This task is about upstream truth only. It must not silently defer these facts
to later lowering or backend phases.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/cross-link-validation.md`
- `docs/discussions/semantic-procedural-ir/paper-migration-validation.md`
- `.harness/tasks/regseq-driver/21-driver-bus-attribute-extraction.md`
- `.harness/tasks/regseq-driver/22-authored-callback-and-helper-body-extraction.md`
- `.harness/tasks/regseq-model/13-authored-body-schema-and-normalization.md`
- `crates/regseq-driver/src/extract/`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/hir.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/def.rs`

## Output Specification

Modify:

- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/`
- `crates/regseq-model/src/declaration/raw_input.rs`
- raw frontend fixture expectations only where deterministic review artifacts
  need to change

Add extraction support for the identity and ownership facts required by the
semantic IR and item-hierarchy work.

The resulting raw frontend path must carry, for the supported PL011 slice:

- canonical paths for:
  - functions
  - impls
  - constants
  - structs
  - enums relevant to interrupt-class and related declaration identity
  - relevant fields
- module and impl ownership facts sufficient for later `Module` / `ItemStore`
  construction
- associated-constant ownership facts
- ISR marker capture
- resource-field and register-block-field identity needed for canonical
  cross-linking

### Output Contract

This task must make the enriched extraction contract explicit in the raw input
path. It is not enough to merely “have the data somewhere in extraction code.”
The real raw/frontend handoff consumed downstream must expose the enriched
facts through typed fields or typed companion structures in the raw artifact /
raw input model.

At minimum, the enriched contract must make it explicit:

- where canonical item identity lives
- where field-level canonical identity lives
- where impl/self-type/trait ownership lives
- where associated-constant ownership lives
- where ISR-marker capture lives

Later tasks must not have to inspect extraction internals or infer these facts
from incidental fields.

## Implementation Notes

### Phase Boundary

This task is strictly Phase 0. It must not:

- introduce the semantic procedural IR itself
- introduce backend-facing semantic body nodes
- move Linux-specific concepts into extraction
- ask later lowering to recover missing ownership or identity facts by name

### CanonicalPath Requirements

Define and thread a canonical item-identity string derived from rustc `DefId`
for all supported item families needed by the semantic plan:

- driver struct
- register-block struct
- callback/helper function bodies
- impl blocks
- associated constants
- relevant struct fields

The rendering must be deterministic and must be suitable for later
cross-reference matching. It must not be:

- backend-specific
- based on display-only pretty names
- dependent on file layout assumptions

If a shared helper for canonical-path rendering is needed, create one here and
test it directly.

### Ownership and Containment Requirements

Extraction must expose enough information for later phases to build the logical
item hierarchy without guessing:

- which items belong to which module
- which methods belong to which impl
- which impl belongs to which self type
- whether an impl is trait-backed or inherent
- which constants are associated with which owner

Do not require later phases to infer these relationships from method names,
short constant names, or string heuristics.

### Field Identity Requirements

Relevant fields need deterministic identity for later cross-link resolution:

- driver resource fields (clock, reset, pinctrl, mmio)
- register-block fields where later lowering or backend planning will need
  canonical field references

Field identity must not be limited to a short field name. The enriched raw path
must let later phases distinguish:

- same short field names on different structs
- same field names across modules

### ISR Marker Capture

Capture the HIR/source-level fact that a method is marked as an ISR root
(`#[rseq::isr]`) so later semantic-role assignment does not have to rediscover
it indirectly from body shape.

### Non-Goals / Negative Constraints

Do not in this task:

- build `Module`, `ItemStore`, or `SemanticModel`
- define `Stmt` / `Expr` / `SemanticBody`
- reinterpret MIR into semantic nodes
- change the MIR query away from `mir_drops_elaborated_and_const_checked`
- introduce Linux, Zephyr, UART, or PL011-specific semantic payloads
- silently keep short-name-only extraction as a fallback for required identity

### Reviewability

This task must leave behind an inspectable artifact or deterministic test
surface proving that the new identity/ownership data is really present in the
raw frontend path.

## Test Plan

Add tests to verify:

1. canonical paths are present for extracted functions, impls, constants,
   structs, and relevant fields
2. canonical path rendering is deterministic for the supported PL011 fixture
3. callback/helper bodies retain their function-level canonical identity
4. associated constants carry ownership information rather than only short names
5. resource/register fields carry deterministic identity sufficient for later
   cross-linking
6. ISR marker capture is preserved in extracted data
7. extraction does not regress the existing MIR body capture path
8. overlapping short names remain disambiguated by the enriched identity path

Test commands:

- `cargo test -p regseq-driver`
- `cargo clippy -p regseq-driver -- -D warnings`

## Acceptance Criteria

- the raw frontend path now carries canonical identity and ownership facts
  required by the semantic-IR plan
- canonical identity is deterministic and not based on short-name heuristics
- later phases would be able to build module/impl/associated-const structure
  without guessing relationships by name
- field-level identity is present for the supported slice's resource/register
  needs
- ISR root markers are captured explicitly
- no semantic-body IR or backend-specific behavior is introduced in this task
- reviewer-facing evidence proves the enriched extraction data exists in the
  real extraction path
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes

## Reopen Note

This task was reopened after review of the first implementation.

The extraction module and its direct tests are directionally correct, but the
implementation does not yet satisfy the Phase 0 contract:

- required enrichment failures are still silently dropped from the real raw
  frontend path because the raw artifact is built with
  `identity_enrichment: ex.enrichment.as_ref().ok().cloned()` rather than a
  failure-preserving contract
- driver detection is still top-level-only before the recursive module walk, so
  a valid nested `#[rseq::driver]` struct would incorrectly fail extraction
- the new tests prove the extractor in isolation, but they do not yet prove
  that the enriched surface survives the real raw artifact / raw input handoff;
  the existing raw artifact tests still only cover `identity_enrichment: None`

The revised implementation must correct those three issues before this task can
be accepted.
