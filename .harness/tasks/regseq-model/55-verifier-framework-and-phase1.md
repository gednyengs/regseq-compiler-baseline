# regseq-model-55: Verifier Framework + Post-Phase-1 Verifier

## Description

Introduce the `regseq-model` verifier infrastructure and implement
`verify_phase_1(&RegseqModel) -> Result<(), VerifierError>`. This
task establishes the error-type vocabulary, the verifier level
taxonomy (module / driver-struct / body / cross-item per spec.md
Section 13), and the module-level + cross-item-level rule buckets.
Task 56 extends the framework with post-Phase-2 checks, and task 57
routes mutation calls through it.

The verifier must not be callable as an afterthought -- its signature
and error model are shared with downstream tasks 56, 57, 58, 59. Get
the shape right here so the rest of Phase 6 is additive.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Section 13
  (Phase legality verification -- post-Phase-1 block on lines
  1398-1402, verification-after-every-transformation on
  1418-1435, level taxonomy on 1424-1432).
- `.harness/spec-batches/regseqmodel-phase-6.md` (Implementation
  Notes sections `Post-Phase-1 verifier`, `Verify after every
  transformation`, `Verifier levels`, and the negative test
  "Phase-1 output mutated to leave a binding populated").
- `crates/regseq-model/src/model/regseq_model.rs` (the root
  struct under verification).
- `crates/regseq-model/src/model/module.rs`,
  `crates/regseq-model/src/model/driver.rs`,
  `crates/regseq-model/src/model/type_def.rs`,
  `crates/regseq-model/src/model/register.rs` (module-tree and
  cross-reference shapes the verifier scans).
- `crates/regseq-model/src/identity/decl_id.rs` (the typed
  `DeclId` vocabulary the cross-item bucket uses).
- `.harness/tasks/regseq-model/54-pl011-end-to-end.md` (Phase-1
  invariants are already enforced structurally for the PL011
  fixture by task 54's end-to-end test -- read that task for the
  enumerated invariants the verifier must replicate. Note: task 54
  only committed per-function Body goldens, NOT a model-level
  golden; the first model-level golden lands in task 60. The
  happy-path test in this task therefore builds a minimal valid
  model programmatically; do not load a serialized PL011 model).

## Implementation Notes

### Module layout

Create `crates/regseq-model/src/verify/` with:
- `mod.rs` (public surface: `VerifierError`, `VerifierErrorKind`,
  `VerifierLevel`, `verify_phase_1`).
- `phase_1.rs` (Phase-1 entry function + the two rule buckets
  this task owns).
- `rules/module.rs` (module-level rules -- owned by task 55).
- `rules/cross_item.rs` (cross-item-level rules -- owned by
  task 55).

Task 56 will add `phase_2.rs`, `rules/driver_struct.rs`, and
`rules/body.rs`. Task 57 will add `guard.rs`. Leave those stubs
unimplemented here.

### Error model

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerifierError {
    pub level: VerifierLevel,
    pub kind: VerifierErrorKind,
    pub provenance: ErrorProvenance, // reuse regseq-driver's type
    pub details: String,             // human-readable specifics
}

pub enum VerifierLevel { Module, DriverStruct, Body, CrossItem }

pub enum VerifierErrorKind {
    // ----- Module level (task 55) -----
    MissingDriverModule,
    MultipleDriverModules { count: usize },
    DuplicateItemName { module: String, name: String },
    // ----- Cross-item level (task 55) -----
    UnresolvedTypeDefId { referencing: DeclId, target: TypeDefId },
    UnresolvedFunctionId { referencing: DeclId, target: FunctionId },
    UnresolvedRegisterBlockId { referencing: DeclId, target: RegisterBlockId },
    MissingSourceLocation { decl_id: DeclId },
    // ----- Phase-1 boundary leakage (task 55) -----
    UnexpectedBindingPopulated { resource: DeclId, slot: &'static str },
    UnexpectedPropertyValuePopulated { property: DeclId },
    DeclarationFieldUnpopulated { decl_id: DeclId, field: &'static str },
    // ----- Driver-struct + Body + Phase-2 kinds added by tasks 56/57 -----
}
```

`ErrorProvenance` is currently defined in `regseq-driver`; this task
either re-exports it from `regseq-model` (preferred -- the verifier
lives there) or introduces a minimal `VerifierProvenance` struct with
the same shape (`item_path: String`, `span_or_decl: ...`). Decide at
implementation and document in the module-level rustdoc.

### Module-level rules (task 55 owns)

Per spec.md Section 13 line 1425 and lines 1398-1402:

- **Exactly one Driver module**: count modules with
  `kind == ModuleKind::Driver`; raise `MissingDriverModule` /
  `MultipleDriverModules` as appropriate.
- **No duplicate item names per module**: for each module, assert
  all `items` entries have distinct names; raise
  `DuplicateItemName`.
- **Declaration-side completeness**: every TypeDef has a
  classification; every Resource has its taxonomic properties
  (`ClockResource.clock_id`, `ResetResource.reset_id`, etc.
  populated); every Function has a non-placeholder `Body` (every
  cross-crate function required by Phase 5 task 47 has a body).
  Raise `DeclarationFieldUnpopulated` citing the decl id + field
  name.

### Cross-item-level rules (task 55 owns)

Per spec.md Section 13 lines 1431-1432:

- **Cross-references resolve**: every `DeclId` appearing in a
  body, a resource binding slot's type, or a RegisterBlock's
  field list must map to a declared entity in the model. Raise
  the appropriate `Unresolved*Id` variant.
- **Source locations present**: for every `DeclId` assigned
  during Phase 5 Pass 1, `RegseqModel.source_locations` carries
  an entry. No `CompilerGenerated` slot for user-sourced
  entities. Raise `MissingSourceLocation`.

### Phase-1 boundary-leakage rules (task 55 owns)

Per spec.md Section 13 lines 1400-1401:

- **All binding slots `None`**: every clock/reset/pinctrl/
  interrupt/mmio resource has `binding == None`. Raise
  `UnexpectedBindingPopulated` with the slot name.
- **All property bound values `None`**: every property's
  `bound_value == None`. Raise
  `UnexpectedPropertyValuePopulated`.

### Entry function

```rust
pub fn verify_phase_1(model: &RegseqModel) -> Result<(), VerifierError> {
    rules::module::run(model)?;
    rules::cross_item::run(model)?;
    // Task 57 may route additional transformation-level checks here.
    Ok(())
}
```

`verify_phase_1` is fail-fast: return the first violation. A
batching/collect-all API can land later if downstream consumers need
it; do not speculate now.

### Determinism of error output

Error rendering is deterministic: when walking collections to detect
violations, iterate in `DeclId` order (or the canonical BTreeMap order
each field already uses). Tests assert that two runs on the same model
produce the same first error.

## Non-Goals

- No Phase-2 verification (task 56).
- No mutation-API wiring (task 57).
- No `normalize()` invocation (task 58).
- No body-level or driver-struct-level rules (task 56).
- No backend-specific legality checks (Phase 8).
- Do not add verification code that depends on execution context (ISR
  vs non-ISR is body-level scope owned by task 56).
- **Do not modify `crates/regseq-driver/src/construction.rs` or any
  file in `crates/regseq-driver/src/construction/`.** The verifier
  reads a finished `RegseqModel`; it does not reshape construction.
  Per Phase-6 brief lines 18-21.

## Test Plan

Unit tests under `crates/regseq-model/tests/verify_phase_1.rs`:

- **Happy path**: construct a minimal valid Phase-1 `RegseqModel`
  programmatically in a test builder helper (`build_minimal_ok()`
  under `tests/common/mod.rs` or inline): one Driver module
  named `"drv"`, one DriverStruct with zero resources and zero
  properties, empty `user_data`, one registered Function with a
  non-placeholder Body, populated `source_locations` for every
  `DeclId` used, all binding slots `None`. Assert
  `verify_phase_1(&model).is_ok()`. Do NOT depend on any golden
  produced by another task -- task 54 produces per-function Body
  goldens, not a model-level golden, and task 60 is the one that
  will commit the first model-level golden.
- **Negative -- missing driver module**: build a minimal model
  with zero modules; assert `MissingDriverModule`.
- **Negative -- multiple driver modules**: build a model with
  two `ModuleKind::Driver` modules; assert
  `MultipleDriverModules { count: 2 }`.
- **Negative -- duplicate item name**: build a model whose driver
  module has two items named the same; assert
  `DuplicateItemName`.
- **Negative -- Phase-1 leakage (THIS IS THE BRIEF'S REQUIRED
  NEGATIVE TEST)**: take a valid Phase-1 model, mutate one
  clock resource's `binding` to `Some(...)`, assert
  `UnexpectedBindingPopulated { slot: "clock" }`. This test is
  the proof that the post-Phase-1 invariant is enforced rather
  than convention.
- **Negative -- unresolved TypeDefId**: build a model whose
  Function param references a `TypeDefId` not in
  `model.type_defs`; assert `UnresolvedTypeDefId`.
- **Negative -- missing SourceLocation**: build a model with a
  `DeclId` that has no entry in `source_locations`; assert
  `MissingSourceLocation`.
- **Determinism**: run `verify_phase_1` twice on the same
  malformed model; assert identical `VerifierError` returned.

## Review

After tests pass, use `codex:rescue` (via
`scripts/python/codex_runner.py`) to verify: (1) every module-level
and cross-item-level rule listed in spec.md Section 13 lines 1400-1402
and 1425/1431-1432 has a concrete implementation here; (2) the
VerifierError shape is rich enough for tasks 56/57 to extend without
breaking callers; (3) the brief's required negative test for
Phase-1 leakage is implemented exactly, not approximated. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-model --test verify_phase_1
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/verify/` module structure landed with
  `mod.rs`, `phase_1.rs`, `rules/module.rs`,
  `rules/cross_item.rs` (and empty `phase_2.rs` /
  `rules/driver_struct.rs` / `rules/body.rs` stubs awaiting
  tasks 56).
- `verify_phase_1` rejects every negative case listed above and
  accepts the programmatically-constructed minimal valid model
  from the happy-path test (there is no Phase-5 model-level
  golden to consume).
- `VerifierError` / `VerifierLevel` / `VerifierErrorKind` are
  documented public types; their shape is stable for tasks 56-59.
- All unit tests pass; determinism test proves fail-fast output
  is stable across runs.
- Codex review approved.
