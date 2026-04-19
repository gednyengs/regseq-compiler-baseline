# regseq-model-57: RegseqModel Mutation API + Always-On Verification

## Description

Implement the `RegseqModel` mutation API that routes **every**
in-place transformation through the appropriate verifier level
(module / driver struct / body / cross-item, per spec.md Section 13
line 1424-1432). The verifier is built into the pipeline
infrastructure, not left to callers to remember to invoke (spec.md
line 1434-1435). After this task, the only way to get an invalid
`RegseqModel` into downstream code is through a deliberately-unsafe
escape hatch; accidental corruption at the API surface is impossible.

Task 59 (`populate_bindings()`) consumes this API to perform the
platform join; its type signature works only after this scaffolding
lands.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Section 13
  "Verification after every transformation" (lines 1418-1435,
  the level taxonomy on 1424-1432).
- `.harness/spec-batches/regseqmodel-phase-6.md` Implementation
  Notes "Verify after every transformation" block
  (lines 117-127), and the Acceptance Criteria item that
  explicitly calls out this task:
  "The mutation API for `RegseqModel` triggers verification on
  every code path; a unit test confirms a deliberately invalid
  in-place mutation surfaces a verifier error rather than silent
  corruption." (lines 163-166).
- `.harness/tasks/regseq-model/55-verifier-framework-and-phase1.md`
  (VerifierError + Phase-1 checks).
- `.harness/tasks/regseq-model/56-post-phase2-verifier.md`
  (Phase-2 checks + driver-struct + body buckets).
- `crates/regseq-model/src/model/regseq_model.rs` (root struct --
  currently a plain data container; this task adds the guarded
  mutation surface).

## Implementation Notes

### Design shape

Introduce a transaction-style mutation API. Callers do not poke
`model.modules[i].items` directly; they open a scoped mutator that
enforces verification on drop.

```rust
impl RegseqModel {
    /// Open a mutation transaction at `level`. The returned guard
    /// grants typed access to the fields at that level and runs
    /// `verify_*` on commit. Dropping without commit rolls back
    /// (the model is cloned on `begin_mutation`).
    pub fn begin_mutation(&mut self, level: VerifierLevel)
        -> MutationGuard<'_>;
}

pub struct MutationGuard<'m> { /* holds &mut RegseqModel + snapshot */ }

impl MutationGuard<'_> {
    pub fn modify_module(&mut self, id: ModuleId, f: impl FnOnce(&mut Module));
    pub fn modify_driver_struct(&mut self, id: DriverStructId, f: impl FnOnce(&mut DriverStruct));
    pub fn modify_body(&mut self, id: FunctionId, f: impl FnOnce(&mut Body));
    pub fn insert_binding(&mut self, resource: DeclId, binding: Binding);
    pub fn insert_property_value(&mut self, property: DeclId, value: PlatformValue);
    /// Runs verify_phase_1 or verify_phase_2 (caller-specified on
    /// `begin_mutation`) and consumes the guard. On error, rolls
    /// back to the pre-mutation snapshot.
    pub fn commit(self) -> Result<(), VerifierError>;
}
```

The caller's choice is **which phase-level verifier** runs on
commit, not **whether** verification runs. `begin_mutation` takes an
enum:

```rust
pub enum MutationPhase {
    /// Post-mutation model must still satisfy verify_phase_1.
    Phase1,
    /// Post-mutation model must satisfy verify_phase_2.
    Phase2,
}
```

(Repurpose `VerifierLevel` here only if it matches this semantic
cleanly; otherwise keep these as separate enums to avoid conflating
"what level of rule bucket" with "which phase invariants hold post-
mutation".)

Rolling back on failure is achieved via a `RegseqModel` clone taken
on `begin_mutation`. The clone cost is acceptable at Phase 6: joins
happen once per driver, not in hot loops. Document the trade-off in
the module-level rustdoc.

### Level-specific commit routing

On `commit`:
- If `MutationPhase::Phase1`: call `verify_phase_1(&model)`.
- If `MutationPhase::Phase2`: call `verify_phase_2(&model)`.

The guard's typed `modify_*` methods mean we don't need to guess
which *level* changed -- we commit the full phase verifier regardless.
This is correct because the phase-level verifier includes all four
rule buckets. An optimization pass to run only the affected-level
bucket is future work and must not land here.

### Unsafe escape hatch (justified)

For internal infrastructure that reconstructs models from
deserialization (e.g., loading a golden JSON in tests), expose an
`unchecked` constructor:

```rust
impl RegseqModel {
    /// Construct without verification. The caller must invoke
    /// `verify_phase_1` or `verify_phase_2` separately before passing
    /// the result to any non-test consumer.
    #[doc(hidden)]
    pub fn from_verified_components_unchecked(/* ... */) -> Self;
}
```

Rustdoc this as "not a public API" and ensure `cargo doc` doesn't
surface it by default (`#[doc(hidden)]`). Serde's `Deserialize` may
use it, but then must run the appropriate verifier downstream.

### Cross-item-level guarantee

Include a unit test specifically for cross-item-level checks through
the mutation API: start with a valid PL011 model, open a
`MutationPhase::Phase1` transaction, insert a new TypeDef that
references a nonexistent RegisterBlockId, call `commit`, and assert
the commit returns `VerifierError { level: CrossItem, kind:
UnresolvedRegisterBlockId, .. }` AND that the model is rolled back
to the pre-mutation state. This proves the cross-item bucket is
routed (not just module-level).

### Determinism note

Verifier rollback preserves byte-for-byte equality of the model
(same interner, same BTreeMap ordering). A unit test serializes
model-pre, opens+aborts a mutation, serializes model-post, and
asserts byte-equal JSON.

## Non-Goals

- No Phase-1 or Phase-2 verifier rule authorship (tasks 55, 56).
- No `normalize()` call sites (task 58 does not use the mutation
  API -- it constructs a fresh model).
- No `populate_bindings()` implementation (task 59 consumes this
  API).
- No opt-out performance mode (no `commit_unchecked`). If callers
  bypass verification, it must be through the `unchecked`
  construction escape, not through the mutation API.
- **Do not modify `crates/regseq-driver/src/construction.rs` or any
  file in `crates/regseq-driver/src/construction/`.** The mutation
  API operates on an already-constructed `RegseqModel`; it does not
  touch the Phase-5 construction pipeline. Per Phase-6 brief
  lines 18-21.

## Test Plan

Unit tests under
`crates/regseq-model/tests/mutation_api_auto_verify.rs`:

- **Commit success path**: open Phase1 mutation on a valid PL011
  model, make a benign change (add a new typedef + its source
  location + required cross-refs), commit, assert success.
- **Commit rejection path -- module level**: open Phase1, add a
  second driver module, commit, assert
  `VerifierError { kind: MultipleDriverModules, .. }` and the
  model reverted.
- **Commit rejection path -- driver struct level**: open Phase2,
  corrupt a resource attribute, commit, assert
  `ResourceAttributeIncomplete` and rollback.
- **Commit rejection path -- body level**: open Phase2, mutate a
  body to reference a nonexistent FunctionId, commit, assert
  `UnresolvedBodyReference` and rollback.
- **Commit rejection path -- cross-item level** (the spec's
  explicit test): open Phase1, insert TypeDef referencing
  nonexistent RegisterBlockId, commit, assert
  `UnresolvedRegisterBlockId` AND model-post bytes equal
  model-pre bytes (rollback was complete).
- **Abort without commit**: open mutation, mutate, drop guard
  without commit, assert the model was NOT mutated (because
  mutation happens on the snapshot; or document alternative
  semantics clearly if the design mutates in place and only
  rolls back on commit-error).
- **Determinism**: serialize model-pre, open+abort, serialize
  model-post, assert byte-identical.

## Review

After tests pass, use `codex:rescue` to verify: (1) every
transformation method on the mutation API routes through
`verify_phase_1` or `verify_phase_2` -- no escape path exists
through the guarded surface; (2) the cross-item-level test proves
routing (not just Module/Driver-struct); (3) the `unchecked`
constructor is narrowly scoped and doc-hidden; (4) the rollback
behavior is specified in rustdoc (which semantics -- snapshot-on-
begin or diff-on-commit). Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model --test mutation_api_auto_verify
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `RegseqModel::begin_mutation` + `MutationGuard` public surface
  exists with typed `modify_*` methods for module, driver
  struct, body, binding slot, and property value.
- `commit()` routes to `verify_phase_1` or `verify_phase_2` per
  `MutationPhase`, and rolls back on error.
- `MutationGuard` has NO method that bypasses verification.
- The cross-item-level rejection test (brief's explicit
  requirement) passes and proves rollback byte-equality.
- All tests listed in Test Plan pass.
- Rustdoc on the mutation API documents the verification
  contract and the `unchecked` escape hatch.
- Codex review approved.
