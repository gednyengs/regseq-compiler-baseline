# regseq-driver-50: Vtable-Indirect End-to-End Bringup (Erased-Receiver Mutability + Canonical Vacant CAbiLowering + Fixture Extension)

## Description

Close the three interlocking defects that today prevent the
phase-1a vtable-indirect dispatch path from completing
end-to-end against a real `&dyn Trait` fixture. Surfaced by an
agent's attempt at `regseq-model-95` (P1a-E2E-03) on
`fixes/audit-2026-05-15`, independently runtime-verified, and
independently diagnosed by Codex from the spec citations. After
this task lands, the original two-test scope of
`regseq-model-95` becomes achievable; until then, its test (a)
(`vtable_synthesis_and_indirect_rewrite`) cannot evaluate.

The defects are on the SAME dispatch path and must be fixed
together because each blocks observation of the next:

- **Defect A -- Fixture has no `Cast::Unsize` site.**
  `crates/test-ir-fixtures/fixtures/trait_object.rs` ships
  in abstract-only form:
  ```rust
  #![no_std]
  pub trait Reader { fn read_byte(&self) -> u8; }
  pub fn drive(r: &dyn Reader) -> u8 { r.read_byte() }
  ```
  No concrete `impl Reader`, no `&Concrete -> &dyn Reader`
  coercion site. Per `spec.md:4745` VTableInstance synthesis
  fires per `(PrincipalTraitKey, ConcreteImpl)` reachable via
  `Cast::Unsize` rvalues at coercion sites
  (`spec.md:1151`). Without a cast site in this crate, no
  VTableInstance is synthesised and the Phase-5
  vtable-indirect rewrite at
  `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:355-359`
  hard-errors with "matching VTableInstance item is absent
  for virtual call".

- **Defect B -- `canonical_vacant_fn_ptr_ty` skips
  `sig_c_abi_lowerings` population.**
  `crates/regseq-driver/src/bridge/synth/vtable.rs:526-542`
  interns the canonical vacant `extern "C" fn(*mut ()) -> ()`
  FnSig per `spec.md:4633-4641` but never updates the
  parallel `sig_c_abi_lowerings` side table. The validator at
  `crates/regseq-model/src/layout_validator.rs:310-314` then
  trips `CAbiLoweringMalformed { sig: FnSigId(0), reason:
  "FnAbi::C signature requires Some(CAbiLowering)" }` per the
  spec.md §3.3 step 5 invariant that every `FnAbi::C` FnSig
  carry a `Some(CAbiLowering)`. Surfaced once Defect A is
  fixed and synthesis actually fires.

- **Defect C -- Vtable-indirect rewrite preserves source
  borrow mutability instead of unifying to `Mut`.**
  `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:264-272`'s
  `receiver_dynamic_ty` maps the source `&dyn Reader`
  (shared borrow) receiver to `Ty::Ptr(_, Mutability::Not)`,
  and that flavor propagates into `_t_data` at
  `vtable_indirect.rs:72-73` plus the `args[0]` replacement
  at `vtable_indirect.rs:124-125`. spec.md §13.2's erased-
  receiver rule (`spec.md:4607-4618`) mandates the uniform
  erased receiver is `Ty::Ptr(ErasedPointee,
  Mutability::Mut)` **regardless of the source borrow
  flavor** -- the soundness argument cites
  `UnsafeCell`-bearing impls that legitimately mutate
  through `&self`. The mismatch surfaces at the B-6 per-arg
  type-match check in `validate_call` (`crates/regseq-model/src/model.rs:2924-2935`)
  as `CallArgTypeMismatch { idx: 0, expected_ty: <C-ABI
  shim's input[0] = Ty::Ptr(unit, Mut)>, actual_ty:
  <_t_data's Ty::Ptr(unit, Not)> }`.

After all three land, the trait_object fixture bridges +
validates end-to-end, the synthesised
`(VTableType, VTableInstance, VTableShim*)` Items are present in
`model.items` / `model.item_synthetic_kinds`, `model.vtables`
maps the `PrincipalTraitKey` to the instance ItemId, and the
rewritten `drive(...)` body's Call carries `CallCallee::Indirect(
fnptr_local)` with the 4-5 fresh-local chain per
`spec.md:11673-11748`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §13.2 Vtable family
  (`spec.md:4589-4750`), in particular:
  - The erased-receiver uniform mutability rule
    (`spec.md:4607-4618`) -- the load-bearing wording for
    Defect C.
  - The Cast::Unsize-triggered VTableInstance synthesis
    rule (`spec.md:4743-4750`) -- the load-bearing wording
    for Defect A.
  - The canonical vacant signature spec
    (`spec.md:4633-4641`) -- the load-bearing wording for
    Defect B.
- `docs/discussions/regseq-model/spec.md` §25.7.2
  Vtable-indirect rewrite (`spec.md:11642-11750`), in
  particular:
  - The 6-step procedure (`spec.md:11680-11722`) -- the rule
    the current rewrite implements; the bug is in step 5's
    `Ty::Ptr(unit_ty, <mut>)` destination type, where
    `<mut>` MUST be `Mutability::Mut` per §13.2 (§25.7.2
    inherits §13.2's erased-receiver invariant).
  - The "4-5 fresh locals" rule (`spec.md:11725-11748`).
- `docs/discussions/regseq-model/spec.md` §3.3 step 5
  (`sig_c_abi_lowerings` invariant; cited via
  `crates/regseq-model/src/layout_validator.rs:298-305`).
- `docs/discussions/regseq-model/spec.md` §23.6.2 B-6
  per-arg type-match rule (`spec.md:9695-9713`) -- the
  validator's enforcement of Defect C's mismatch.
- `docs/discussions/regseq-model/spec.md` table at §13
  synthesis triggers (`spec.md:1148-1152`) -- the rule that
  `Cast::Unsize` rvalues trigger VTableInstance +
  VTableShim Items.
- `crates/regseq-driver/src/bridge/synth/vtable.rs:526-542`
  -- `canonical_vacant_fn_ptr_ty` (the Defect B fix site).
- `crates/regseq-driver/src/bridge/fnsig.rs:65-74` --
  `compute_cabi_lowering` (the canonical helper Defect B's
  fix calls into).
- `crates/regseq-driver/src/bridge/fnsig.rs:51-59` -- the
  reference pattern for intern-then-populate.
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:264-272`
  -- `receiver_dynamic_ty` (the Defect C fix site).
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:60-130`
  -- the step-1..step-6 rewrite body where `_t_data`'s
  mutability flavor propagates.
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:340-360`
  -- the "matching VTableInstance item is absent" hard-error
  Defect A's fixture extension unblocks.
- `crates/regseq-model/src/layout_validator.rs:298-330` --
  `validate_c_abi_lowerings` (the validator Defect B's fix
  satisfies).
- `crates/regseq-model/src/model.rs:2785-2950` --
  `validate_call`'s B-6 per-arg type-match (the validator
  Defect C's fix satisfies).
- `crates/test-ir-fixtures/fixtures/trait_object.rs` -- the
  fixture file Defect A's fix extends.
- `crates/test-ir-fixtures/src/lib.rs:40-43` -- the
  `fixture_names::TRAIT_OBJECT` constant (unchanged; the
  extension keeps the same name).
- `.harness/tasks/regseq-model/95-e2e-synthesis-vtable-and-closure.md`
  -- the downstream task this fix unblocks (its test (a)
  `vtable_synthesis_and_indirect_rewrite` becomes evaluable
  after this lands).
- `.harness/tasks/regseq-driver/40-closure-vtable-drop-glue-synthesis.md`
  -- regseq-driver-40 (P1a-BR-04); Defect B's fix is a
  small-surface follow-up.
- `.harness/tasks/regseq-driver/43-body-materialisation-and-vtable-indirect.md`
  -- regseq-driver-43 (P1a-BR-07); Defect C's fix is a
  single-line correction in this task's deliverable.

## Implementation Notes

Three changes, in dependency order so each is testable
in isolation. The order matches the runtime cascade: A
unblocks B's observability, B unblocks C's observability,
C closes the path.

### Change A: Extend `trait_object.rs` with concrete impl + Cast::Unsize site

The fixture must include:

1. The existing `pub trait Reader { fn read_byte(&self) -> u8; }`.
2. The existing `pub fn drive(r: &dyn Reader) -> u8 { r.read_byte() }`.
3. A concrete `pub struct StaticByte(pub u8);` (single-field
   tuple struct; no-alloc; the byte field is `pub` so it's
   reachable for read-back).
4. `impl Reader for StaticByte { fn read_byte(&self) -> u8 {
   self.0 } }`.
5. A `pub fn make_and_drive() -> u8 { let r = StaticByte(0x42);
   drive(&r) }` whose `drive(&r)` callsite produces a
   `&StaticByte -> &dyn Reader` coercion -- the
   `Cast::Unsize` rvalue rustc emits at that site (per
   `spec.md:1151`) triggers VTableInstance synthesis for
   `(PrincipalTraitKey { trait_def: Reader, ... }, ArgsKey
   { StaticByte's type })`.

Keep the `#![no_std]` attribute. No other changes.

### Change B: Populate `sig_c_abi_lowerings` for the canonical vacant FnSig

In `crates/regseq-driver/src/bridge/synth/vtable.rs`'s
`canonical_vacant_fn_ptr_ty` (around line 526-542), after
`cx.model.sigs.intern(sig)`, call
`crate::bridge::fnsig::compute_cabi_lowering(cx, sig_id)`
exactly once. The helper at `bridge/fnsig.rs:65-74`:

- Computes the lowering via `CAbiChecker::new(cx).sig_lowering(sig_id)`.
- Calls `ensure_cabi_lowering_slots(cx)` so the side table
  matches `sigs.len()`.
- Writes `Some(lowering)` into `sig_c_abi_lowerings[sig_id]`.

The vacant signature `extern "C" fn(*mut ()) -> ()` is
trivially C-ABI admissible (the input is a pointer, the
output is unit; both are leaf-admissible per §14.5). The
helper's internal `expect("translated FnSig must be C ABI
admissible")` cannot fire on this shape.

Match the existing pattern at `bridge/fnsig.rs:51-58`:

```rust
let lowering = if abi == FnAbi::C {
    Some(CAbiChecker::new(cx).sig_lowering_for_sig(&sig)?)
} else {
    None
};
let sig_id = cx.model.sigs.intern(sig);
ensure_cabi_lowering_slots(cx);
cx.model.sig_c_abi_lowerings[sig_id] = lowering;
```

Use `compute_cabi_lowering` (the pub fn) rather than
reimplementing the CAbiChecker call inline; it's the single
canonical helper for this case.

### Change C: Vtable-indirect rewrite must produce `Mutability::Mut` for `_t_data`

`crates/regseq-driver/src/bridge/body/vtable_indirect.rs:264-272`'s
`receiver_dynamic_ty` (or equivalent helper that determines
the erased pointee mutability for `_t_data` at step 5) must
ALWAYS return `Mutability::Mut` for the erased receiver,
regardless of the source borrow flavor of `args[0]`.

Per `spec.md:4607-4618`:

> Erased receiver type is uniformly `Ty::Ptr(ErasedPointee,
> Mutability::Mut)` in the IR, regardless of the source
> borrow flavor [...] a shared-receiver erasure
> (`Ty::Ptr(_, Mutability::Not)`) would not be sound across
> all admissible impls.

The change is a single-site fix: the helper / inline
computation that today branches on `Mutability::Not` vs
`Mutability::Mut` from `args[0]`'s source `Ref(t, _)` flavor
must collapse to a constant `Mutability::Mut`. The source
borrow flavor remains visible on the original `args[0]`'s
type (preserved at the type-system level per
`spec.md:4615-4617`), but the rewrite's step-5 `Rvalue::Cast`
destination type uses `Mut` uniformly.

The downstream propagation through `_t_data` at
`vtable_indirect.rs:72-73` and the `args[0]` replacement at
`vtable_indirect.rs:124-125` (`Operand::Move(Place::from(_t_data))`)
then aligns with the C-ABI shim's input[0] type
(`Ty::Ptr(unit, Mutability::Mut)`, per the canonical vacant
signature + the method-shim ABI uniformity rule at
`spec.md:4647-4660`).

The B-6 per-arg type-match check at
`crates/regseq-model/src/model.rs:2924-2935` then passes:
`operand_ty(args[0]) = Ty::Ptr(unit, Mut)` matches
`inputs[0] = Ty::Ptr(unit, Mut)`.

### Conventions

- Match the existing closure / vtable synth file style for
  doc comments (matching the synth/vtable.rs:524-525 comment
  density for new explanatory blocks).
- Keep the fixture diff surgical: extend only with the new
  struct + impl + function; do NOT touch the existing
  `Reader` trait + `drive` function.
- The fixture's `make_and_drive` returns `u8` and is
  monomorphic; do NOT add generic parameters.

## Non-Goals

- The closure-side `source_item` DefPath rendering fix is
  already landed (audit-fix commit on
  `fixes/audit-2026-05-15`); this task does NOT re-land or
  touch `crates/regseq-driver/src/bridge/synth/closure.rs`.
- VTableShim Item *contents* beyond what the existing
  `synthesise_vtable_with_rustc_entries` path already
  produces are out of scope; this task asserts the
  end-to-end path completes, not the shim-body shape.
  Shim-body shape unit tests live in `regseq-driver-40`.
- The `regseq-model-95` test file itself is out of scope;
  this task makes it ABLE to pass, the test file lands when
  `regseq-model-95` is reset + executed.
- Other `Cast::Unsize` flavors (`Box<dyn Trait>`,
  `Rc<dyn Trait>`, `Arc<dyn Trait>`) are out of scope;
  phase-1a exercises `&dyn Trait` only via this fixture.
  Box/Rc/Arc variants are phase-1b / future-fixture
  territory.
- Trait-object dispatch for trait objects with associated
  types / projection bindings is out of scope; the fixture's
  `Reader` trait has no associated types.
- Multi-method trait objects, supertrait vtables, and
  trait-upcasting are out of scope; the fixture's `Reader`
  has exactly one method and no supertraits.
- The post-bridge `regseq-model-95` test (a) assertions are
  out of scope (they are the downstream consumer).

## Test Plan

**Test organization**: a single integration test file at
`crates/regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs`
that drives the bridge against the extended trait_object
fixture and asserts the post-bridge shape, with `model.validate`
as the central gate. Quality-gate unit-test coverage for the
two bridge-side fixes lives alongside in
`crates/regseq-driver/src/bridge/synth/vtable.rs`'s existing
unit tests + a new test in
`crates/regseq-driver/src/bridge/body/vtable_indirect.rs`'s
existing test module.

### A. Defect A -- fixture compiles standalone

1. `rustc --edition 2021 --crate-type rlib
   crates/test-ir-fixtures/fixtures/trait_object.rs -o
   /tmp/trait_object.rlib` succeeds. (Smoke-checks the
   no_std extension didn't introduce any unresolvable
   reference.)

### B. Defect B -- canonical vacant FnSig carries CAbiLowering

2. New unit test in
   `crates/regseq-driver/src/bridge/synth/vtable.rs` (or the
   `#[cfg(test)] mod tests` block alongside, matching the
   file's existing test layout): construct a `BridgeCtxt`
   stub, call `canonical_vacant_fn_ptr_ty(&mut cx)`, assert
   that for the returned `Ty::FnPtr(sig_id)`:
   - `cx.model.sigs.resolve(sig_id).abi == FnAbi::C`.
   - `cx.model.sig_c_abi_lowerings[sig_id].is_some()`.
   - The lowering's `inputs.len() == 1` and `output` matches
     unit per §14.5.

### C. Defect C -- rewrite produces `Mutability::Mut` for `_t_data`

3. New unit test in
   `crates/regseq-driver/src/bridge/body/vtable_indirect.rs`
   (or its existing test module): construct a synthetic
   pre-rewrite Body whose `args[0]` is `Operand::Move(_1)`
   with `_1: Ty::Ref(dyn_reader_ty, Provenance::SharedBorrow)`
   (the shared-borrow source flavor that previously triggered
   the bug). Apply the rewrite. Assert:
   - The `_t_data` LocalDecl's `ty` resolves to
     `Ty::Ptr(unit_ty, Mutability::Mut)` (NOT
     `Mutability::Not`).
   - The rewritten Call's `args[0]` is
     `Operand::Move(Place::from(_t_data))`, and recovering
     its operand type via `operand_ty` returns
     `Ty::Ptr(unit_ty, Mutability::Mut)`.

### D. End-to-end on trait_object fixture

4. New integration test
   `bridge_vtable_indirect_end_to_end.rs` in
   `crates/regseq-driver/tests/`. Uses the same in-process
   rustc invocation pattern as `test-ir-fixtures::bridge_fixture`
   (compile the fixture, invoke `run_bridge`, validate the
   model). Assertions:
   - `run_bridge(...)` returns `Ok(model)` (no
     "matching VTableInstance item is absent" error,
     no `CallArgTypeMismatch`).
   - `model.validate(&BridgeInput::matching(&model))`
     returns `Ok(())`.
   - `model.vtables` contains exactly one entry whose key
     is the `PrincipalTraitKey` for `Reader` (extracted by
     trait DefPath terminal segment match -- the
     `<regseq_synth>` reserved prefix isn't on
     `PrincipalTraitKey.trait_def` directly; pattern-match
     via the `Item` referenced by `trait_def`).
   - At least one `Item` in `model.item_synthetic_kinds`
     has value
     `Some(SyntheticItemKind::VTableInstance { principal:
     <Reader's key>, self_ty: <StaticByte's ArgsKey> })`.
   - The `drive` function's `Body` contains exactly one
     `TerminatorKind::Call`, and that Call's `callee` is
     `CallCallee::Indirect(_)` (NOT `Direct(_)`).
   - That `drive` body's `local_decls.len() >= 6` (2
     original + at least 4 fresh per
     `spec.md:11725-11748`).
   - The `make_and_drive` function's `Body` contains the
     `Cast::Unsize` rvalue chain (one `Rvalue::Cast` with
     `kind: CastKind::PointerCoercion(Unsize)` or
     equivalent ABI-locked variant).

### E. Workspace regression-clean

5. `cargo test --workspace` -- all existing tests still
   pass.
6. `cargo fmt --all -- --check` -- clean.
7. `cargo clippy --workspace --tests -- -D warnings` -- no
   new lint sites beyond the pre-existing baseline (35 sites
   per the current `fixes/audit-2026-05-15` baseline; this
   task must NOT introduce any new lint sites in its
   changed files).

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash tool
(per `SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-50 against its task
spec at
.harness/tasks/regseq-driver/50-vtable-indirect-end-to-end-bringup.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/test-ir-fixtures/fixtures/trait_object.rs (Defect A extension)
- crates/regseq-driver/src/bridge/synth/vtable.rs (Defect B fix)
- crates/regseq-driver/src/bridge/body/vtable_indirect.rs (Defect C fix)
- crates/regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs (new integration test)
- any new unit tests inside the bridge crate

Focus on:
- Defect A: fixture extension MUST include a concrete impl +
  a Cast::Unsize site (a `&Concrete -> &dyn Trait` coercion).
- Defect B: the canonical_vacant_fn_ptr_ty fix MUST call
  through fnsig::compute_cabi_lowering (the canonical helper),
  not reimplement the lowering computation.
- Defect C: the erased-receiver mutability MUST be
  `Mutability::Mut` UNIFORMLY, regardless of source borrow
  flavor (spec.md:4607-4618). Verify the fix is not just
  "Mut for shared, Not for mutable" or vice versa.
- All Test Plan items A1..E7 must be present.
- The integration test must call BOTH `run_bridge(...)` AND
  `model.validate(...)`, not just one.

Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT:
APPROVED.

## Quality Gate

Workspace-wide because this task touches three crates
(`test-ir-fixtures` fixture content + `regseq-driver` synth +
body passes):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/test-ir-fixtures/fixtures/trait_object.rs` extended
  with `StaticByte` struct + `impl Reader for StaticByte` +
  `pub fn make_and_drive() -> u8` per Change A.
- `crates/regseq-driver/src/bridge/synth/vtable.rs`
  `canonical_vacant_fn_ptr_ty` calls
  `fnsig::compute_cabi_lowering(cx, sig_id)` post-intern per
  Change B.
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs`
  `_t_data` destination mutability is `Mutability::Mut`
  uniformly per Change C.
- `crates/regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs`
  created with the integration test covering Test Plan D4.
- Unit tests covering Test Plan B2 + C3.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
