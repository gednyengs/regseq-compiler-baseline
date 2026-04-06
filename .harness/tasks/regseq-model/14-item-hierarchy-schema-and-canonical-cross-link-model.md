# Task: regseq-model-14 - Item hierarchy schema and canonical cross-link model

## Description

Add the rustc-independent schema support for the semantic-IR item hierarchy and
its canonical cross-link model.

This task creates the model boundary for:

- logical item containment and identity
- canonical `ItemId` / `ItemStore` ownership
- canonical item-level and field-level cross-link resolution

It does **not** yet perform semantic body lowering. That remains a later
lowering task.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/ownership-and-identity-stress-test.md`
- `docs/discussions/semantic-procedural-ir/cross-link-validation.md`
- `docs/discussions/semantic-procedural-ir/model-layer-coherence-validation.md`
- `.harness/tasks/regseq-driver/24-semantic-ir-phase0-extraction-enrichment.md`
- `.harness/tasks/regseq-model/07-declaration-model-reviewability-and-traversal-baseline.md`
- `.harness/tasks/regseq-model/13-authored-body-schema-and-normalization.md`
- `crates/regseq-model/src/`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Add the model-side schema/types for the Layer 2 item hierarchy and canonical
cross-link support needed by the approved semantic-IR plan.

At minimum, this task must provide typed homes for:

- `CanonicalPath`
- `ItemId`
- `ItemStore`
- `Module`
- `Item`
- `StructDef`
- `EnumDef`
- `StructFieldDef`
- `ImplDef`
- `ConstantDef`
- `InterruptClassDef` or an explicitly named equivalent type that can act as
  the canonical top-level interrupt-class definition carried into
  `SemanticModel`
- `TypeRef`
- `CrossLinkTable`

For executable items, this task must define the **structural** function/item
metadata needed before semantic bodies are attached:

- canonical identity
- provenance
- structural ownership (`FunctionKind`)
- semantic role slot (`FunctionRole`) or equivalent forward-compatible home

Crate-boundary resolution note:

- the shared semantic/item-core types referenced by both `regseq-model` and
  `regseq-backends` must live in `regseq-model`
- do not leave `FunctionDef.body: SemanticBody` or equivalent shared-type
  ownership unresolved between crates
- if body attachment needs to remain absent at this task boundary, use an
  explicitly named body-less/header form with a documented migration path
  rather than a vague placeholder

## Implementation Notes

### Phase Boundary

This task owns model schema and cross-link structure only. It must not:

- define `Stmt`, `Expr`, or `SemanticBody`
- walk MIR
- produce backend plans
- introduce Linux- or PL011-specific semantic nodes

### Item Ownership Model

The schema must reflect the approved ownership model:

- `Module.items` resolves through `Vec<ItemId>`
- `ItemStore` is the canonical payload store
- impl methods/constants are referenced by `ItemId`; they are not duplicate
  embedded payloads

This task must make the storage model explicit enough that later lowering and
review tooling can resolve any `ItemId` deterministically.

### Canonical Identity Model

The schema must support:

- item-level canonical identity
- field-level canonical identity
- deterministic cross-link lookup without short-name matching

If the model uses separate lookup tables for items and fields, that split must
be explicit and tested.

### Relationship to Existing DeclarationModel

This task must preserve the architectural rule that the item hierarchy and
`DeclarationModel` coexist as complementary views. Do not:

- collapse one into the other
- make the item hierarchy depend on backend constructs
- make the cross-link model depend on short-name recovery

### Non-Goals / Negative Constraints

Do not in this task:

- define the semantic procedural IR proper
- add MIR CFG lowering
- define Linux backend nodes
- encode PL011-specific names, roles, or register assumptions in model schema
- treat `CanonicalPath` display/stable names as the same thing

### Reviewability

The resulting schema must be easy to inspect in tests. Reviewer-facing tests
should demonstrate:

- item ownership
- impl grouping
- associated constant ownership
- field-level identity
- canonical cross-link lookup

## Test Plan

Add tests to verify:

1. `ItemStore` is the canonical owner of item payloads
2. `Module.items` references items through `ItemId`
3. `ImplDef.methods` and `ImplDef.constants` resolve through `ItemId`
4. item-level `CanonicalPath` lookup works deterministically
5. field-level `CanonicalPath` lookup works deterministically
6. overlapping short names are disambiguated through canonical identity
7. associated constants can be resolved without short-name matching
8. the schema remains device-neutral and OS-neutral
9. no model type names or required identities are Linux-, Zephyr-, UART-, or
   PL011-specific beyond reviewer-facing fixture/examples

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- the model layer now has an explicit schema for the Layer 2 item hierarchy
- storage/ownership through `ItemStore` and `ItemId` is explicit and coherent
- item-level and field-level canonical cross-link support exist and are tested
- the schema is compatible with the approved semantic-IR plan and does not
  force name-based recovery in later phases
- semantic-body IR has **not** been prematurely introduced here
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes

## Reopen Note

This task was reopened after review of the first implementation.

The overall schema direction is correct, but the implementation does not yet
satisfy the required ownership/coherence boundary:

- `ItemStore` is documented as the canonical owner of item payloads and
  `ItemStore::push()` assigns a fresh `ItemId`, but the stored payload is
  inserted unchanged even though the payload types still carry their own `id`
  field
- as implemented, later code can retrieve an item through one `ItemId` while
  observing a different embedded `id` inside the payload, which violates the
  task's required "`ItemStore` / `ItemId` ownership is explicit and coherent"
  contract
- the current test suite does not assert this invariant; it constructs payloads
  with placeholder ids and assumes the store overwrites them, but it never
  proves that a retrieved payload's internal `id` matches the store-assigned
  `ItemId`

The revised implementation must:

- make `ItemStore` ownership coherent by ensuring stored item payload identity
  cannot diverge from the `ItemId` used to retrieve it
- add reviewer-facing tests that explicitly prove the retrieved payload id
  matches the store-assigned id for all relevant item families

Do not address this by weakening the ownership claim in the schema docs. Fix
the model so the ownership invariant is actually true.
