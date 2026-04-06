# Task: regseq-lowering-05 - Semantic procedural IR types and boundary

## Description

Introduce the semantic procedural IR types and the backend-facing semantic model
boundary described in the approved plan.

This task is the type-boundary step only. It defines the new semantic IR and
its typed interface, but it does not yet perform MIR-to-semantic lowering.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/backend-boundary-validation.md`
- `docs/discussions/semantic-procedural-ir/invariant-validation.md`
- `.harness/tasks/regseq-model/14-item-hierarchy-schema-and-canonical-cross-link-model.md`
- `.harness/tasks/regseq-lowering/01-lowering-module-scaffold-and-typed-plan-boundary.md`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`

Add the approved semantic procedural IR type boundary, including:

- `SemanticBody`
- `Block`
- `Stmt`
- `Expr`
- `SemanticLocal`
- `LocalId`
- `LocalOrigin`
- `SemanticType`
- body-level semantic refs:
  - function refs
  - driver-field refs
  - register/register-field refs
  - interrupt-class refs
  - subsystem-action refs
- `SemanticModel`

These types must be introduced with the following crate-boundary rule:

- shared semantic/item-core types that both `regseq-model` and
  `regseq-backends` need must live in `regseq-model`
- `regseq-backends` may define lowering/planner wrappers around those types,
  but must not become the sole owner of types the model layer also needs

This pins the architectural resolution and avoids a circular dependency between
`regseq-model` and `regseq-backends`.

## Implementation Notes

### Phase Boundary

This task defines the types only. It must not:

- walk MIR
- populate semantic bodies from real driver functions
- migrate `DriverLoweringPlan.behavior`
- rewrite Linux emitter behavior

### Required Design Properties

The defined types must reflect the approved plan, including:

- statement/expression separation
- explicit local environment
- explicit type universe
- structured refs instead of stringly lowering-critical references
- explicit option-carrier forms
- explicit `FallibleOp.success_local`
- explicit `OptionUnwrap` validity contract in docs/comments
- backend-facing `SemanticModel` container

### Type-Quality Requirements

- public types and fields must be documented
- the type boundary must be readable and reviewable
- do not introduce ad hoc escape hatches that violate the approved plan
- do not collapse semantic nodes back into generic “call + tag” placeholders
- do not add Linux-, Zephyr-, UART-, or PL011-specific IR variants

### Relationship to Model Layer

These types are the first concrete lowering-side semantic boundary. They must
be compatible with the item hierarchy and identity model introduced earlier,
not a parallel ad hoc identity system.

In particular:

- if `FunctionDef` or related item-core types are shared with the model layer,
  define them in the shared/model-owned location now
- do not leave the implementer to invent an ad hoc crate split or placeholder
  ownership strategy during implementation

### Non-Goals / Negative Constraints

Do not in this task:

- implement semantic lowering
- change current backend emission to consume these types
- encode backend helper names or OS APIs into the IR
- invent a secondary C-oriented lower IR

## Test Plan

Add tests to verify:

1. the new semantic IR module compiles and is importable
2. the public type surface matches the required shape from the approved plan
3. body-level refs use structured identity rather than bare strings
4. `FallibleOp` supports both unit-success and value-success shapes
5. option-carrier forms exist as explicit IR constructs
6. no Linux-specific realization types leak into the semantic IR
7. the resulting type boundary is compatible with the lowering module scaffold
8. shared-type ownership between `regseq-model` and `regseq-backends` is
   acyclic and explicit

Test commands:

- `cargo test -p regseq-model`
- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-model -- -D warnings`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- the semantic procedural IR types exist in-tree
- they match the approved design closely enough that later phases do not need
  to redesign the boundary
- the IR is backend-agnostic, device-agnostic, and not Rust-shaped after
  construction
- structured refs replace stringly lowering-critical identity
- no semantic lowering or Linux emission migration is hidden inside this task
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
