# Task: regseq-lowering-09 - External referenced item materialization and enum-variant preservation

## Description

Generalize the populated item hierarchy and semantic lowering so that
referenced items from supported external crates become first-class modeled
items, and enum variant identity is preserved through semantic lowering without
collapsing variants to raw integers.

This task closes the remaining identity/preservation gap exposed by
`regseq::RxChar::new(...)` and `regseq::IrqResult::{Handled, NotHandled}`.

This is not a Linux-backend task. It is a prerequisite portable-model /
portable-lowering task needed so backends can consume the semantic model
truthfully instead of reconstructing lost semantics.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/acceptance-example-set.md`
- `docs/discussions/semantic-procedural-ir/backend-boundary-validation.md`
- `docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md`
- `.harness/tasks/regseq-lowering/06-item-hierarchy-construction-from-enriched-extraction.md`
- `.harness/tasks/regseq-lowering/07-supported-regseq-semantic-body-lowering-validated-on-pl011-representative-set.md`
- `.harness/tasks/regseq-lowering/08-semanticmodel-projection-and-driverloweringplan-behavior-migration.md`
- `.harness/tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md`
- `crates/regseq-model/src/item/`
- `crates/regseq-model/src/semantic/`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`

Modify only if required to surface already-modeled identity/metadata cleanly:

- `crates/regseq-model/src/item/`
- `crates/regseq-driver/src/extract/`

Expand external-item materialization and semantic preservation so that
referenced supported external items can flow through ordinary modeled identity
rather than ad hoc transparent skips, fake refs, intrinsics, or backend-side
semantic reconstruction.

## Implementation Notes

### Core Rule

If an item is semantically referenced by the supported analyzed slice and comes
from a supported external crate, it must be eligible to become a first-class
modeled item with:

- module ownership
- real `ItemId`
- canonical path
- cross-link registration
- ordinary lowering/reference behavior

Externality alone must not make an item second-class in the model.

### Supported External Universe

For this task, the supported external universe is the currently accepted one:

- `regseq::*`

Do not generalize to arbitrary third-party crates in this slice.

Discovery must be:

- reference-driven
- analysis-driven
- not hardcoded inventory-driven

The compiler should materialize what the analyzed supported slice actually
references, not a predeclared dump of all `regseq` items.

### Item Kinds In Scope

This task expands external materialization beyond nominal types.

Support referenced external items of the kinds needed by the supported slice,
including at minimum:

- `StructDef`
- `EnumDef`
- `FunctionDef`
- `ConstantDef`
- `ImplDef` if required to model external methods coherently

This is not limited to constructors. The rule is general.

### Enum Representation Rule

Enum variants must remain nested inside `EnumDef`.

Do not introduce first-class variant items in this task.

Use the existing shape:

- `EnumDef`
- `EnumVariantDef { name, discriminant }`

with:

- MIR `variant_index` interpreted by position in `EnumDef.variants`

That ordering contract must be treated as semantically significant.

If additional extraction/materialization work is needed so external enums carry
truthful variant names and discriminants, that work is in scope.

### Semantic-Lowering Preservation Rule

Semantic lowering must preserve external referenced items through ordinary
modeled identity rather than degrading them.

For the supported PL011 slice, this specifically includes:

- `regseq::RxChar::new(byte, errors)`
- `regseq::IrqResult::{Handled, NotHandled}`

Required outcome examples:

- `RxChar::new(...)` is preserved as an ordinary modeled call with a real
  referenced external function item, not skipped as transparent and not
  degraded to the first operand.
- `IrqResult` unit variants preserve nominal variant identity and do not
  collapse to raw integer literals with lost meaning.

### Boundary Rules

Do not solve this task by:

- adding Linux-specific or device-specific semantic nodes
- adding PL011-specific constructor/variant handling
- inventing fake `FunctionRef` / `ItemId`
- turning ordinary framework constructors into compiler intrinsics
- teaching the backend to reconstruct semantics that portable lowering dropped

The fix must live in the portable item/lowering boundary.

### Non-Goals / Negative Constraints

Do not in this task:

- rewrite Linux emission
- add backend-owned PL011 shortcuts
- treat enum variants as first-class top-level items
- widen the supported external universe beyond `regseq`
- introduce intrinsics merely to compensate for missing portable identity

## Test Plan

Add tests to verify:

1. referenced external functions/methods are materialized as real modeled items
   with real identity in the populated hierarchy
2. referenced external constants/items required by the supported slice are
   materialized with ordinary cross-linkable identity
3. external item discovery is reference-driven rather than a hardcoded full
   inventory dump
4. `RxChar::new(byte, errors)` is preserved through semantic lowering as an
   ordinary modeled call with both arguments intact
5. `IrqResult::{Handled, NotHandled}` preserve nominal variant identity through
   semantic lowering and do not collapse to indistinguishable integer residue
6. enum variant reconstruction from MIR `variant_index` is stable and based on
   the resolved `EnumDef.variants` ordering
7. the resulting hierarchy / semantic bodies remain OS-neutral and
   device-neutral
8. no fake item identity or `Opaque` residue is introduced as a workaround

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- referenced supported external items are first-class modeled items in the
  populated hierarchy regardless of being external
- this includes the callable external items needed by the supported slice
- `RxChar::new(...)` no longer loses payload information in semantic lowering
- `IrqResult` variant identity is preserved through lowering
- enum variants remain nested under `EnumDef` with stable name/discriminant
  metadata; no first-class variant items are required
- no backend phase is required to reconstruct semantics lost by portable
  lowering
- no Linux-specific, Zephyr-specific, or PL011-specific logic is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
