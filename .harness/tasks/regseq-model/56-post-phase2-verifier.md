# regseq-model-56: Post-Phase-2 Verifier + Driver-Struct & Body Rules

## Description

Extend the verifier framework (task 55) with
`verify_phase_2(&RegseqModel) -> Result<(), VerifierError>` plus the
two remaining verifier-level buckets from spec.md Section 13:
**driver-struct level** (resource attribute completeness, no duplicate
IDs, user-data field constraints) and **body level** (referenced IDs
resolve, operand types match, context-specific legality hooks). The
body-level rules that depend on metadata Phase 5 did not surface are
scaffolded as deferred hook points with TODO markers, not silently
dropped.

After this task, the verifier covers all four levels from spec.md
Section 13 line 1424-1432. Tasks 57 and 59 consume `verify_phase_2`.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Section 13
  (post-Phase-2 block lines 1404-1408; driver-struct level rules
  line 1426-1427; body level rules lines 1428-1430).
- `.harness/spec-batches/regseqmodel-phase-6.md` (Implementation
  Notes: `Post-Phase-2 verifier`, `Verifier levels per spec.md
  Section 13: ... Body-level checks include "no sleep() in ISR
  context" and "no writes to read-only registers" -- defer those
  specific checks if Phase 5 did not surface the metadata`).
- `.harness/tasks/regseq-model/55-verifier-framework-and-phase1.md`
  (the framework this task extends).
- `crates/regseq-model/src/verify/` (the module surface created by
  task 55, with stubs this task fills).
- `crates/regseq-model/src/model/driver.rs` (DriverStruct field
  shape -- resources, properties, user_data).
- `crates/regseq-model/src/model/register.rs` (RegisterDescriptor
  access_kind the RO-write check will consult).
- `crates/regseq-model/src/ir/body.rs`,
  `crates/regseq-model/src/ir/terminator.rs` (body shapes the
  body-level bucket walks).

## Implementation Notes

### What this task owns

Fill the stubs left by task 55:
- `crates/regseq-model/src/verify/phase_2.rs` (entry function).
- `crates/regseq-model/src/verify/rules/driver_struct.rs`
  (driver-struct-level rules).
- `crates/regseq-model/src/verify/rules/body.rs`
  (body-level rules + deferred hook scaffolding).

Extend `VerifierErrorKind` (public enum from task 55) with the
following variants; document in rustdoc which Section 13 line each
maps to:

```rust
// ----- Driver-struct level (task 56) -----
ResourceAttributeIncomplete { resource: DeclId, attribute: &'static str },
DuplicateResourceId { id: String, kind: &'static str },
UserDataFieldNotOptionCopy { field: DeclId, offending_ty: String },
// ----- Body level (task 56) -----
UnresolvedBodyReference { body: FunctionId, decl_id: DeclId },
OperandTypeMismatch { body: FunctionId, location: String, expected: String, got: String },
IsrContextViolation { body: FunctionId, kind: IsrViolationKind },
ReadOnlyRegisterWrite { body: FunctionId, register: RegisterDescriptorId },
// ----- Phase-2 boundary (task 56) -----
RequiredBindingMissing { resource: DeclId, slot: &'static str },
RequiredPropertyValueMissing { property: DeclId },
// ----- Phase-2 cross-item (task 56) -----
UnknownIsrTarget { interrupt: DeclId, target: FunctionId },
NonIsrFunctionBoundAsIsr { interrupt: DeclId, target: FunctionId },
```

`IsrViolationKind` is a small enum (`Sleep`, future additions);
document its extension policy (append-only, never renumber).

### Driver-struct rules (task 56 owns)

Per spec.md Section 13 line 1426-1427:

- **Resource attribute completeness**: every `ClockResource`,
  `ResetResource`, `PinCtrlResource`, `InterruptResource`,
  `MmioResource`, and `SyncResource` has its declaration-side
  attributes populated (not binding-side; this is the
  declaration field set from Section 4.2). Raise
  `ResourceAttributeIncomplete` citing the missing field name.
- **No duplicate IDs**: within a single resource kind, the
  declared `id` strings are unique. Raise
  `DuplicateResourceId { id, kind }`.
- **User-data fields are `Option<T>` where `T: Copy`**: every
  `DriverStruct.user_data` field's `RegseqType` must be
  `Option<T>` where `T` is scalar or structurally `Copy`. Raise
  `UserDataFieldNotOptionCopy` for violations. The check reads
  `TypeDef.classification` for the inner type; if inner type is
  a user-defined struct, walk its fields recursively.

### Body rules (task 56 owns -- scaffolded)

Per spec.md Section 13 lines 1428-1430:

- **Referenced IDs resolve**: every `FunctionId`, `TypeDefId`,
  `ConstantId` appearing in any `Body.basic_blocks` terminator
  or statement must be present in the model. Raise
  `UnresolvedBodyReference`.
- **Operand types match**: for each
  `TerminatorKind::Call`/`IntrinsicCall`, the callee's parameter
  types match the argument operand types (read from
  `LocalDecl.ty`). This check is simple at Phase 6 since Pass 2
  preserved `ty` at every local. Raise `OperandTypeMismatch`.
- **Context-specific legality (deferred)**: add public hook
  points for:
  - `no_sleep_in_isr`: currently emits an
    `UnimplementedContextCheck` warning log line (not an error).
    The metadata path that would identify "this body runs in ISR
    context" (ISR binding → `FunctionId`) is present, but the
    set of "sleep" intrinsics is not enumerated in Phase 5. The
    scaffold checks that any `IntrinsicCall` whose intrinsic is
    in a configured `ISR_FORBIDDEN_INTRINSICS: &[RegseqIntrinsic]`
    (empty set at Phase 6) triggers `IsrContextViolation`. Tests
    cover the hook with a synthetic non-empty set to prove the
    plumbing works.
  - `no_writes_to_read_only_registers`: for every
    `IntrinsicCall` with `IntrinsicMetadata::Register` whose
    intrinsic is `RegisterWrite` or `RegisterWriteField`, the
    referenced `RegisterDescriptor.access_kind` must not be
    `ReadOnly`. Raise `ReadOnlyRegisterWrite`. This check is
    fully implementable at Phase 6 because the metadata is
    present from Phase 5 task 51.

Document clearly in rustdoc which body-level checks are
fully implemented (`ReadOnlyRegisterWrite`) versus hook-scaffolded
(`IsrContextViolation`). The brief accepts this split explicitly
(spec-batches/regseqmodel-phase-6.md lines 123-127).

### Phase-2 boundary rules (task 56 owns)

Per spec.md Section 13 lines 1404-1408:

- **All required bindings populated**: every resource whose
  declared shape is `T` (not `Option<T>`) has
  `binding == Some(...)`. Raise `RequiredBindingMissing`.
- **All required property bound_values populated**: every
  property whose declared shape is required has `bound_value ==
  Some(...)`. Raise `RequiredPropertyValueMissing`.

Note on "platform file IDs must match declarations" (spec.md line
1408): this is already enforced at join time by task 59's
section-kind joiners, each of which raises
`JoinError::UnknownPlatformSection` when a platform section
references an id with no declaration. There is no verifier-side
duplicate check here -- detecting the error earlier, with proper
platform-section provenance, is strictly better than re-detecting
it from a side-record. Do not reintroduce such a side-record.

### Phase-2 cross-item rules (task 56 owns)

Per spec.md Section 13 line 1432 ("Interrupt ISR bindings match
declared interrupt resources"):

- **ISR binding target resolves**: for each bound
  `InterruptResource.binding` whose shape includes an ISR handler
  `FunctionId`, that `FunctionId` MUST be present in
  `model.functions`. Raise `UnknownIsrTarget`.
- **ISR binding target is an ISR-marked function**: the referenced
  `Function` must carry the ISR classification that Phase 5
  extraction records for `#[rseq::isr]`-annotated functions. A
  non-ISR function bound as an ISR target is an error. Raise
  `NonIsrFunctionBoundAsIsr`.

This rule only runs at Phase 2 because the binding (and therefore
the ISR target) is populated post-join. The Phase-1 cross-item
bucket (owned by task 55) skips ISR checks for that reason.

### Entry function

```rust
pub fn verify_phase_2(model: &RegseqModel) -> Result<(), VerifierError> {
    // Post-Phase-2 implies Phase-1 invariants held before the join.
    // Re-check the subset that still applies (module invariants still
    // hold; cross-item still holds; binding "None" invariants no
    // longer apply).
    rules::module::run(model)?;
    rules::cross_item::run(model)?;
    rules::driver_struct::run(model)?;
    rules::body::run(model)?;
    rules::phase_2_boundary::run(model)?;
    rules::phase_2_cross_item::run(model)?;
    Ok(())
}
```

`rules::phase_2_boundary::run` is a small new module owning the
required-fields-populated checks. `rules::phase_2_cross_item::run`
owns the ISR-binding-target checks.

## Non-Goals

- No Phase-1 checks re-authored (task 55 owns those).
- No mutation-API wiring (task 57).
- No `populate_bindings()` implementation (task 59).
- No backend-specific legality checks like Linux `trigger_type`
  (Phase 8, spec.md lines 1410-1413).
- Do not enumerate a canonical set of "sleep" intrinsics --
  plumbing only; concrete ISR-forbidden set lands when Phase 7
  or Phase 8 adds the intrinsics.
- **Do not modify `crates/regseq-driver/src/construction.rs` or any
  file in `crates/regseq-driver/src/construction/`.** The Phase-2
  verifier reads a finished `RegseqModel`; it does not reshape
  construction. Per Phase-6 brief lines 18-21.

## Test Plan

Unit tests under `crates/regseq-model/tests/verify_phase_2.rs`:

- **Happy path**: hand-build a fully-bound PL011-shaped model
  (one clock bound, one MMIO bound, one property bound). Assert
  `verify_phase_2(&model).is_ok()`.
- **Driver-struct -- attribute incomplete**: remove a resource
  attribute; assert `ResourceAttributeIncomplete`.
- **Driver-struct -- duplicate IDs**: two clocks with same `id`;
  assert `DuplicateResourceId { kind: "clock" }`.
- **Driver-struct -- user data not Option<Copy>**: user-data
  field of type `String`; assert `UserDataFieldNotOptionCopy`.
- **Body -- unresolved reference**: body references a
  `FunctionId` not in the model; assert
  `UnresolvedBodyReference`.
- **Body -- RO register write**: synthesize an
  `IntrinsicCall { intrinsic: RegisterWrite, metadata: Register
  { register_descriptor: ro_descriptor }}` where
  `ro_descriptor.access_kind == ReadOnly`; assert
  `ReadOnlyRegisterWrite`.
- **Body -- ISR hook plumbing**: inject a test-only
  `ISR_FORBIDDEN_INTRINSICS = &[BusyWait]`; build an ISR body
  containing a `BusyWait` intrinsic; assert
  `IsrContextViolation { kind: Sleep }` (the scaffolded hook
  fires correctly).
- **Phase-2 boundary -- missing required binding**: required
  clock has `binding == None`; assert
  `RequiredBindingMissing { slot: "clock" }`.
- **Phase-2 boundary -- optional binding absent OK**: optional
  reset with `binding == None` passes verification.
- **Phase-2 cross-item -- unknown ISR target**: an interrupt
  resource's bound ISR `FunctionId` does NOT appear in
  `model.functions`; assert `UnknownIsrTarget`.
- **Phase-2 cross-item -- non-ISR function bound as ISR**: the
  bound ISR `FunctionId` resolves to a regular function (not
  ISR-classified); assert `NonIsrFunctionBoundAsIsr`.
- **Phase-2 cross-item -- valid ISR binding accepted**: the
  bound ISR resolves to an ISR-classified function; assert
  verification passes.

## Review

After tests pass, use `codex:rescue` to verify: (1) every
spec.md Section 13 driver-struct-level and body-level rule has
either a concrete implementation or an explicitly-documented
deferral with a plumbed hook; (2) the Phase-2 boundary checks
exactly mirror spec.md lines 1404-1408; (3) the rustdoc on the
deferred checks makes clear what the hook will fire on when its
policy set is populated; (4) no Phase-7 or Phase-8 work leaked
in. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model --test verify_phase_2
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `verify_phase_2` implemented; four rule buckets resolved
  (module + cross-item + driver-struct + body) plus Phase-2
  boundary checks.
- All tests in the Test Plan pass.
- `ReadOnlyRegisterWrite` check is fully functional, proven
  against a synthetic RO register body.
- `IsrContextViolation` scaffolding fires against an injected
  test-only forbidden-intrinsic set.
- New `VerifierErrorKind` variants documented with Section 13
  line references in rustdoc.
- Codex review approved.
