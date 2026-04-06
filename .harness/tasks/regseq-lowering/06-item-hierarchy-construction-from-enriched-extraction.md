# Task: regseq-lowering-06 - Item hierarchy construction from enriched extraction

## Description

Construct the populated Layer 2 item hierarchy from the enriched extraction and
model schema before full semantic-body lowering begins.

This task is Phase 2A from
`docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`.
It exists to keep the approved phase boundary real:

- Phase 2A: build inspectable item hierarchy and ownership graph
- Phase 2B: attach semantic bodies and lower executable semantics

The hierarchy must be reviewable before semantic-body lowering proceeds.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/paper-migration-validation.md`
- `docs/discussions/semantic-procedural-ir/ownership-and-identity-stress-test.md`
- `.harness/tasks/regseq-driver/24-semantic-ir-phase0-extraction-enrichment.md`
- `.harness/tasks/regseq-model/14-item-hierarchy-schema-and-canonical-cross-link-model.md`
- `.harness/tasks/regseq-lowering/05-semantic-procedural-ir-types-and-boundary.md`
- `crates/regseq-model/src/`
- `crates/regseq-backends/src/lowering/`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`
- `crates/regseq-model/src/` only if narrow compatibility glue is required by
  the approved shared-type boundary

Add the first construction pass that turns enriched extraction/model inputs into
the populated Layer 2 hierarchy required by later semantic-body lowering.

At minimum, this task must populate:

- `Module`
- canonical `ItemStore`
- `FunctionDef` headers
- `ImplDef`
- `ConstantDef`
- minimal `StructDef`
- `EnumDef` and/or canonical `InterruptClassDef` population required by the
  supported slice's interrupt-class model
- `CrossLinkTable`

for the supported PL011 representative set and the transitive item set required
by that slice.

## Implementation Notes

### Phase Boundary

This task owns hierarchy construction only. It must not:

- populate `SemanticBody` with MIR-derived content (`Stmt` / `Expr` trees,
  locals, or return types beyond `Void`). `FunctionDef.body` uses
  `SemanticBody::empty()` per the accepted regseq-lowering-05 type boundary
  as the canonical "no body content yet" representation.
- walk MIR into `Stmt` / `Expr`
- migrate `DriverLoweringPlan.behavior`
- rewrite Linux emission

### Crate-Boundary Resolution

The approved resolution for the `FunctionDef.body: SemanticBody` circularity is:

- shared semantic/item-core types live in `regseq-model`
- lowering/backends consume those shared types from `regseq-model`
- `regseq-backends` must not become the sole owner of types that
  `regseq-model` also needs to reference

If the current code layout needs transitional glue, keep it explicit and
temporary. Do not leave the final ownership of shared semantic/item-core types
ambiguous.

### Construction Requirements

Build the hierarchy from canonical identities and enriched ownership facts, not
from short-name heuristics. This includes:

- module membership
- impl grouping
- trait-vs-inherent impl distinction
- `FunctionRole` assignment for each `FunctionDef`, using enriched extraction
  facts such as ISR markers, trait/callback classification, and helper default
  classification
- associated-constant ownership
- field-level cross-link targets

Constant-value continuity requirement:

- where statically evaluable constant values are already available through the
  existing frontend/model pipeline, preserve and thread them into populated
  `ConstantDef.value`
- do not let interrupt-mask or other supported-slice constant values silently
  degrade to absent data if the current upstream path already knows them

### Reviewability Requirement

This task must leave behind deterministic hierarchy evidence, such as:

- hierarchy dumps
- ownership snapshots
- cross-link resolution snapshots

The result must be inspectable before any MIR-derived semantic body content is
populated (Phase 2B). Functions carry `SemanticBody::empty()` at this boundary.

### Non-Goals / Negative Constraints

Do not in this task:

- build `Stmt` / `Expr` trees
- populate `SemanticBody` with MIR-derived content (use `SemanticBody::empty()`
  per the accepted lowering-05 boundary)
- infer semantics from Linux-facing needs
- collapse the hierarchy into backend-only planning structures

## Test Plan

Add tests to verify:

1. the populated item hierarchy can be constructed deterministically from the
   enriched extraction inputs
2. `Module.items`, `ItemStore`, and `ItemId` ownership are coherent
3. `ImplDef.methods` and `ImplDef.constants` resolve correctly
4. associated constants and field-level identities are preserved
5. `CrossLinkTable` resolves both item-level and field-level canonical paths
6. overlapping short names remain unambiguous in the populated hierarchy
7. the result is inspectable without semantic-body attachment

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- the Layer 2 item hierarchy is populated from real enriched extraction input
- the hierarchy is inspectable and reviewable before semantic-body lowering
- ownership/grouping/cross-link facts are real and deterministic
- no semantic-body lowering is hidden inside this task
- no short-name recovery is required to populate hierarchy relationships
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
