# regseq-model-59: `populate_bindings()` Function + JoinError + Phase-2 Determinism

## Description

Implement `populate_bindings(model: &mut RegseqModel,
platform: &PlatformFile) -> Result<(), JoinError>`: in-place
platform join that ID-matches platform sections to resource
declarations, binds `[properties]` values into property slots,
records platform/bus metadata on the driver, and calls
`verify_phase_2` internally on commit. Mutation goes through task
57's `MutationGuard` so every transformation step auto-verifies.

**Owns Phase-2 determinism** (Codex fix): sort-before-serialize
rules for `Vec<PropertyEntry>`, `Vec<ConstructSection>`, and platform
metadata so that the same `(RegseqModel, PlatformFile)` pair
produces a byte-identical serialization. A two-run determinism test
at this granularity (not just task 60's end-to-end level) isolates
join regressions.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Section 11
  (entire, lines 1301-1335: platform metadata, resource binding,
  property binding, join rules) and Section 12 Phase 2 block
  (lines 1366-1388).
- `.harness/spec-batches/regseqmodel-phase-6.md` Implementation
  Notes: "populate_bindings() semantics" (lines 88-105), and
  the `JoinError` negative-test cases in Acceptance Criteria
  (lines 159-162).
- `crates/regseq-model/src/platform.rs` -- parsed
  `PlatformFile` shape (lines 20+).
- `crates/regseq-model/src/model/driver.rs`,
  `crates/regseq-model/src/model/resource.rs` -- the binding
  slots being populated.
- `.harness/tasks/regseq-model/56-post-phase2-verifier.md`
  (`verify_phase_2` called internally).
- `.harness/tasks/regseq-model/57-mutation-api-auto-verify.md`
  (`MutationGuard` + `MutationPhase::Phase2`).
- `.harness/tasks/regseq-model/58-normalize.md` (input model
  comes from `normalize()`; task 59 consumes its output).
- `crates/regseq-model/src/joined/join.rs` -- **legacy**
  join logic for reference; being replaced. Do not re-export.
- `tests/fixtures/pl011-full-spec/` -- platform file fixture
  used in tests.

## Implementation Notes

### Module layout

Create `crates/regseq-model/src/join/` with:
- `mod.rs` (public surface: `populate_bindings`, `JoinError`,
  `JoinErrorKind`, `JoinProvenance`).
- `sections.rs` (per-section-kind join logic:
  `join_clock_sections`, `join_reset_sections`,
  `join_pinctrl_sections`, `join_interrupt_sections`,
  `join_mmio_sections`, `join_properties`, `join_platform_bus`).
- `determinism.rs` (sort-before-serialize helpers + internal
  canonicalization order policy).

### Error model

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JoinError {
    pub kind: JoinErrorKind,
    pub provenance: JoinProvenance,
}

pub enum JoinErrorKind {
    // ----- From spec.md Section 11 lines 1330-1335 -----
    UnknownPlatformSection { kind: String, id: String },
    RequiredResourceNotBound { kind: String, id: String },
    // ----- From brief line 104-105 -----
    PropertyTypeMismatch {
        property_id: String,
        declared: String,
        provided: String,
    },
    // ----- From Phase-2 post-condition -----
    Phase2VerificationFailed { inner: VerifierError },
}

pub struct JoinProvenance {
    /// The platform-file section that caused the error, e.g.,
    /// "[clock.uart_clk]".
    pub platform_section: String,
    /// The declared resource id referenced, if applicable.
    pub resource_id: Option<String>,
}
```

### In-place semantics

Per brief lines 89-91: "In-place mutation on `&mut RegseqModel`.
No new model is constructed; existing IDs and bodies stay put."
Implementation:

```rust
pub fn populate_bindings(
    model: &mut RegseqModel,
    platform: &PlatformFile,
) -> Result<(), JoinError> {
    let mut guard = model.begin_mutation(MutationPhase::Phase2);

    sections::join_platform_bus(&mut guard, &platform.header, &platform.bus)?;
    sections::join_clock_sections(&mut guard, &platform.sections)?;
    sections::join_reset_sections(&mut guard, &platform.sections)?;
    sections::join_pinctrl_sections(&mut guard, &platform.sections)?;
    sections::join_interrupt_sections(&mut guard, &platform.sections)?;
    sections::join_mmio_sections(&mut guard, &platform.sections)?;
    sections::join_properties(&mut guard, &platform.properties)?;

    // Required-binding and required-property-value checks fold
    // into verify_phase_2 (task 56). Unmatched platform ids are
    // NOT in that fold -- they are rejected at the section-level
    // joiners above with full platform-section provenance (see
    // "ID-matched join (per kind)" below). The guard's commit
    // runs verify_phase_2 and maps VerifierError -> JoinError at
    // this layer.
    guard.commit()
        .map_err(|inner| JoinError {
            kind: JoinErrorKind::Phase2VerificationFailed { inner },
            provenance: JoinProvenance {
                platform_section: String::from("<post-join verify>"),
                resource_id: None,
            },
        })
}
```

Section-level joiners emit `JoinError` directly for local errors
(unknown platform section, property type mismatch). Global
required-fields-populated post-conditions are owned by
`verify_phase_2`. The "every platform id matches a declaration"
rule is enforced at the join step itself, not by `verify_phase_2`
-- see the explicit note in the "ID-matched join (per kind)"
section below.

### ID-matched join (per kind)

For each resource kind `K` with platform sections `[K.<id>]`:
1. Build an `Iterator<Item = (&str, &ConstructSection)>` yielding
   `(id, section)` pairs sorted by `id` (determinism).
2. For each, look up the resource by `id` in
   `model.driver.resources[K]`. Missing declaration → emit
   `UnknownPlatformSection { kind: K, id }`.
3. Populate the resource's `binding` slot using the section's
   `key_values`. The shape of `Binding` is per-kind and already
   defined by `regseq-model`; do not re-invent the struct.

The "every platform id matches a declaration" rule from spec.md
Section 13 line 1408 is enforced HERE, at the point of section
iteration (step 2 above). Raising `UnknownPlatformSection` with
proper platform-section provenance is strictly better than
collecting matched ids into a side-record and deferring the
check to the verifier. Do not introduce a
`platform_ids_matched` side-record.

After all kinds processed, walk the declared resources: any
**required** resource with `binding == None` at this point stays
`None` and is caught by `verify_phase_2`'s
`RequiredBindingMissing` on commit. Optional resources
(`Option<T>`) with no section stay `None` and pass.

### Property binding

`[properties]` is flat (`property_id -> value`). For each entry:
- Look up property by `property_id`. Missing → `UnknownPlatformSection
  { kind: "property", id: property_id }`.
- Check value type matches declared property `field_type`. Use
  `PlatformValue`'s discriminator vs `RegseqType`'s scalar
  shape. On mismatch → `PropertyTypeMismatch`.
- Set `bound_value = Some(platform_value)`.

### Phase-2 determinism (this task's explicit ownership)

Sort-before-serialize rules:
- `platform.properties: Vec<PropertyEntry>` iterated in
  `property_id` lexicographic order when assigning
  `bound_value`s.
- `platform.sections: Vec<ConstructSection>` iterated in
  `(kind, id)` lexicographic order when assigning bindings.
- `platform.bus.amba_id` / `revision` recorded in the driver
  metadata canonically (string order).
- Any collection types introduced in the model by this task
  (e.g., per-binding auxiliary records) MUST be `BTreeMap`
  / `BTreeSet`, never `HashMap` / `HashSet`.

The determinism test in this task is granular: serialize
`model_post` from `populate_bindings` run 1, then run 2 (on a
cloned fresh Phase-1 model + same platform), assert byte-identical.
Task 60 re-asserts at the end-to-end level; the scoped test here
localizes Phase-2 regressions.

### Unknown sections

Per brief lines 102-103: a platform section referencing an `id`
with no matching declaration is a hard error. The implementation
must check this explicitly in each section-kind joiner -- do not
rely only on `verify_phase_2`.

## Non-Goals

- No Phase-1 construction work (task 58).
- **Do not modify `crates/regseq-driver/src/construction.rs` or any
  file in `crates/regseq-driver/src/construction/`.** The platform
  join mutates the `RegseqModel` produced by Phase 5 / task 58; it
  does not reshape construction. Per Phase-6 brief lines 18-21.
- No consumer migration (Phase 7).
- No backend-specific binding checks (e.g., Linux
  `trigger_type`; Phase 8 owns that per spec.md line 1410-1413).
- No platform-file **parsing** (already handled by
  `crates/regseq-model/src/platform_parser.rs`).
- No new `Binding` type design -- reuse existing per-kind
  binding structs from `regseq-model`.

## Test Plan

Unit tests under
`crates/regseq-model/tests/populate_bindings.rs`:

- **Happy path**: `normalize()` the PL011 fixture into a Phase-1
  model, then `populate_bindings()` with the PL011 platform
  file. Assert `Ok(())`; assert every required resource has
  `binding == Some(_)`; assert every property has `bound_value
  == Some(_)`; assert `verify_phase_2(&model).is_ok()`.
- **Phase-2 determinism**: run `populate_bindings` twice on
  two cloned fresh Phase-1 models with the same platform file;
  serialize both; assert byte-identical. This assertion lives
  in this task even though task 60 has an end-to-end
  determinism test.
- **Optional-absent is OK**: PL011 platform without the optional
  reset section. Run, assert `Ok(())` and reset resource's
  `binding == None`.
- **Negative -- unknown platform section id**: add
  `[clock.bogus_clk]` to the platform file; run, assert
  `JoinErrorKind::UnknownPlatformSection { kind: "clock",
  id: "bogus_clk" }`.
- **Negative -- required resource without section**: remove the
  required MMIO section from the platform file; run, assert
  `Phase2VerificationFailed { inner: VerifierError { kind:
  RequiredBindingMissing { slot: "mmio" }, .. } }` (via the
  guard's commit mapping).
- **Negative -- property type mismatch**: platform `[properties]`
  sets a string for a property declared as `u32`; run, assert
  `PropertyTypeMismatch { declared: "u32", provided: "string",
  .. }`.
- **Provenance check**: each negative case populates
  `JoinProvenance.platform_section` non-empty and, where
  applicable, `resource_id`.
- **Mutation API integration**: on a rejected join, assert the
  model is byte-equal to its pre-`populate_bindings` state
  (rollback via `MutationGuard` from task 57).

## Review

After tests pass, use `codex:rescue` to verify: (1) every join
rule from spec.md Section 11 lines 1330-1335 has a test; (2) the
Phase-2 determinism test is a true byte-equality check; (3) the
implementation goes through `MutationGuard::commit` (task 57),
not a direct `verify_phase_2` call -- the auto-verify contract
must be honored; (4) the `JoinProvenance` is always populated;
(5) no legacy `crates/regseq-model/src/joined/join.rs` symbol is
used or re-exported. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model --test populate_bindings
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `populate_bindings(&mut RegseqModel, &PlatformFile) ->
  Result<(), JoinError>` implemented in
  `crates/regseq-model/src/join/` and re-exported.
- All join-rule negative tests pass with the correct
  `JoinErrorKind` and populated `JoinProvenance`.
- Optional-absent case is accepted (no spurious errors).
- Phase-2 determinism test asserts byte-identical JSON.
- Implementation goes through `MutationGuard::commit`; a
  rejected join leaves the model byte-equal to pre-join state.
- Unknown-platform-section errors are raised at the join step
  (not deferred to a verifier side-record).
- All unit tests pass.
- Codex review approved.
