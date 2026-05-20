# regseq-driver-60: Body materializer substitutes `Instance.generic_args` into MIR

## Description

Fix the bridge body-materializer bug surfaced by the regseq-driver-59
implementation's B9 test
(`crates/regseq-model/tests/e2e_trait_default_method.rs`): the
materializer walks the polymorphic MIR returned by
`tcx.instance_mir(...)` without applying the Function Instance's
`generic_args`, so any local_decl whose `ty` is a `TyKind::Param`
(e.g., `&Self` in a trait default body) causes
`translate_ty` to reject as `BridgeError::NonRigidTy { kind: Param,
ty_repr: "Self" }`.

The failure trace from the regseq-driver-59 commit
(`crates/regseq-driver/src/bridge/body/materialise.rs` analysis):

1. Source: `pub trait Greeter { fn greet(&self) -> u32 { 42 } } pub
   struct S; impl Greeter for S {} pub fn entry(s: &S) -> u32 {
   s.greet() }` at
   `crates/test-ir-fixtures/fixtures/trait_default_method.rs`.
2. `entry`'s body translation resolves `s.greet()` via
   `lookup_or_create_direct_instance` to
   `InstanceKind::Item(greet_def_id, [Type(S)])`. The Function Instance
   gets a substituted `FnSig` via `callsite_fn_sig`
   (`crates/regseq-driver/src/bridge/body/terminator.rs:491-501`) --
   `Self -> S` substitution succeeds for the signature.
3. Later, `run_phase5_body_passes` calls
   `materialise_function_body_inner` at
   `crates/regseq-driver/src/bridge/body/materialise.rs:243`. The
   materializer fetches the polymorphic MIR via
   `cx.input.tcx.instance_mir(rustc_instance_kind)` -- `instance_mir`
   for `InstanceKind::Item(def_id)` returns
   `tcx.optimized_mir(def_id)`, which is the polymorphic body with
   `Param` types in `local_decls`.
4. Materializer at
   `crates/regseq-driver/src/bridge/body/materialise.rs:252-260`
   walks `rustc_body.local_decls` and calls `translate_ty(cx,
   local.ty)?` directly. `translate_ty` at
   `crates/regseq-driver/src/bridge/ty.rs:275` rejects
   `TyKind::Param` as `NonRigidTy::Param`.
5. Result: bridge fails on the entire crate even though
   `entry`'s call site successfully resolved to a concrete Instance.

Root cause: the materializer constructs an `Instance` at line 153
(`Instance::new_raw(local_def_id.to_def_id(), translated_args).def`)
but then KEEPS ONLY the `.def` field (the `InstanceKind`). The
`.args` field (the `GenericArgsRef` carrying the
substitution) is dropped on the floor. From that point on, the
materializer has no way to substitute `Self -> S` (or any other
generic param -> concrete) into the polymorphic body.

The fix is the standard rustc monomorphization API: replace the bare
`tcx.instance_mir(...)` call with a substitution-applying variant.
The two canonical APIs are:

- `instance.instantiate_mir_and_normalize_erasing_regions(tcx, param_env, ty::EarlyBinder::bind(body))`
  -- the all-in-one substitution + region erasure.
- `tcx.instantiate_and_normalize_erasing_regions(instance.args, param_env, ty::EarlyBinder::bind(body))`
  -- the type-context variant; equivalent for the bridge's use.

Either applies `instance.args` to substitute `Param(0) -> [S]`,
producing a fully monomorphized body whose `local_decls[i].ty` are
concrete (no `Param` left).

After this task lands:

- The body materializer at
  `crates/regseq-driver/src/bridge/body/materialise.rs:243-260` walks
  the SUBSTITUTED body, not the polymorphic one. `local_decls[i].ty`
  are concrete; `translate_ty` succeeds without raising
  `BridgeError::NonRigidTy`.
- The regseq-driver-59 B9 test
  (`crates/regseq-model/tests/e2e_trait_default_method.rs`) passes
  end-to-end without modification. Its existing
  `bridged_trait_default_method_body_surfaces_at_trait_def_path`
  test was authored against the regseq-driver-58 Change H contract
  and is the regression guard for this fix.
- `cargo test --workspace -- --test-threads=1` returns to green.
- Any source-level Rust input that monomorphizes a generic function
  (or a trait default body invoked on a concrete receiver) now
  materializes its Function Instance body correctly. Specifically:
  generic fns reached through `MonoItem::Fn` walks with concrete
  args, trait default bodies invoked via concrete-receiver method
  calls, and any other case where the polymorphic MIR's `local_decls`
  contain `Param` that the Function Instance's `args` substitute.

This is a focused production-code fix to the body materializer.
No spec amendment, no schema change, no validator change.

## Input Context

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §8.1 default-trait-method
  body amendment from regseq-driver-58 (`spec.md:2351-2358`) -- the
  normative contract being unblocked.
- `docs/discussions/regseq-model/spec.md` §9.4 Function variant
  (§9.4 starts at `spec.md:2624`; body-semantics block extends to
  `spec.md:2654`) -- `body: Some(b)` is mandatory for
  source-defined fns; `None` is reserved for foreign decls. Trait
  default bodies are source-defined per Change H, so their
  Instances MUST carry `Some(b)`.

### Bridge code anchors (this task's edit surface)

- `crates/regseq-driver/src/bridge/body/materialise.rs:153` -- the
  `Instance::new_raw(local_def_id.to_def_id(), translated_args).def`
  call that constructs an Instance and discards its `.args` field.
  The fix keeps the full Instance and uses its `.args` for the
  substitution.
- `crates/regseq-driver/src/bridge/body/materialise.rs:243` -- the
  `cx.input.tcx.instance_mir(rustc_instance_kind)` call. Replace
  with a substitution-applying variant; details in Change A.
- `crates/regseq-driver/src/bridge/body/materialise.rs:252-260` --
  the local_decl loop calling `translate_ty(cx, local.ty)?`.
  Behavior unchanged after Change A; the input ty is now
  substituted.
- `crates/regseq-driver/src/bridge/body/materialise.rs:268-279` --
  the basic-block translation loop that passes
  `&rustc_body.local_decls` to `BodyCtxt::new_with_model_body`. The
  reference is to the SAME body object, so Change A's substituted
  body propagates here automatically.
- `crates/regseq-driver/src/bridge/ty.rs:275` -- the
  `NonRigidTy::Param` rejection. UNCHANGED. The rejection is
  correct for genuinely-polymorphic Tys; after Change A the
  materializer simply never feeds polymorphic Tys into
  `translate_ty`.
- `crates/regseq-driver/src/bridge/body/terminator.rs:491-501`
  (`callsite_fn_sig`) -- the existing call-site signature
  substitution. Confirms the bridge already has the right pattern
  for substituting via `.instantiate(tcx, args)` on rustc binders;
  Change A applies the same pattern to MIR bodies.

### Test surface (the regression guard)

- `crates/regseq-model/tests/e2e_trait_default_method.rs` -- the
  regseq-driver-59 B9 test that currently fails. Authored against
  the regseq-driver-58 Change H contract; passes once Change A lands.
- `crates/test-ir-fixtures/fixtures/trait_default_method.rs` --
  the regseq-driver-59 B9 fixture. Unchanged.
- `crates/regseq-model/tests/e2e_foreign_rendering.rs` and its
  fixture -- the regseq-driver-59 B8 test that currently passes.
  Should remain passing (foreign Function Instances have empty
  `args`, so the substitution is a no-op for them).
- `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs` --
  the regseq-driver-58 B1-B5 tests. The materializer change applies
  to ALL Function Instances; verify the eager-bucket-1 fixtures'
  bodies still materialize correctly post-fix (they should, because
  identity-substitution on a non-generic body is a no-op).

### Prior tasks

- regseq-driver-58 (closed at `e7796d7`): eager HIR enumeration. The
  Change H amendment is what introduced the default-trait-method
  Function Item that this task now materializes correctly.
- regseq-driver-59 (failed at `0a4502f`): the test that surfaced the
  bug. The test file + fixture + expectations file are already
  committed; this task does NOT re-author them.

## Implementation Notes

### Change A: substitute `Instance.generic_args` into MIR before walking

**Anchor**: `crates/regseq-driver/src/bridge/body/materialise.rs:243`.

Replace the bare polymorphic-MIR fetch with a substitution-applying
variant.

**Current code** (~lines 145-243):

```rust
FunctionInstanceFlavor::Item => {
    let Some(local_def_id) = local_def_id_for_item(cx.input.tcx, &cx.model, item)
    else {
        return Ok(Err(NoBodyReason::UnresolvableSource));
    };
    let translated_args =
        translate_model_generic_args(cx, local_def_id.to_def_id(), &generic_args)?;
    Instance::new_raw(local_def_id.to_def_id(), translated_args).def
}
// ...
let rustc_body = cx.input.tcx.instance_mir(rustc_instance_kind);
```

Two structural problems compounded: (a) the Instance is constructed
and its `.def` is kept, but its `.args` is dropped, so the
materializer has no substitution context; (b) `tcx.instance_mir(...)`
returns the polymorphic body unmodified.

**Target shape**:

1. Keep the full `Instance<'tcx>` alongside the existing
   `InstanceKind<'tcx>` value -- either pass both through the
   match arms or thread a single `Instance` variable. For the
   `Item` arm, the existing
   `Instance::new_raw(local_def_id.to_def_id(), translated_args)` call
   produces the right `Instance`; capture it before extracting
   `.def`.
2. For non-`Item` arms (`DropGlue`, `VTableShim`, etc.), construct
   the corresponding `Instance<'tcx>` by combining the
   already-constructed `InstanceKind` with the arm's args.
   Important: do NOT use `Instance::new_raw(...)` for these arms --
   `Instance::new_raw` always sets `def = InstanceKind::Item(_)`
   per `rustc_middle/src/ty/instance.rs:457-462`. Use the struct
   literal form
   `rustc_middle::ty::Instance { def: <existing InstanceKind>, args: <arm args> }`
   directly (or wrap the existing `InstanceKind::DropGlue(...)` /
   `VTableShim(...)` construction with the appropriate args
   passed alongside). Each arm currently has the data needed to
   build the full Instance; the fix wires it through. For
   `DropGlue`, the args are typically `tcx.mk_args(&[GenericArg::from(impl_ty)])`
   to match the `Some(impl_ty)` carried in the `InstanceKind::DropGlue`
   variant. For `VTableShim`, args are typically empty
   (`tcx.mk_args(&[])`) since the shim is keyed by DefId.
   Implementer confirms the exact args at impl time.
3. Replace `let rustc_body = cx.input.tcx.instance_mir(...);` with
   a substitution-applying variant:

   ```rust
   let rustc_body_poly = cx.input.tcx.instance_mir(rustc_instance_kind);
   let rustc_body = cx.input.tcx.instantiate_and_normalize_erasing_regions(
       rustc_instance.args,
       ty::TypingEnv::fully_monomorphized(),
       ty::EarlyBinder::bind(rustc_body_poly.clone()),
   );
   ```

   Or equivalently, using the `Instance` method:

   ```rust
   let rustc_body = rustc_instance.instantiate_mir_and_normalize_erasing_regions(
       cx.input.tcx,
       ty::TypingEnv::fully_monomorphized(),
       ty::EarlyBinder::bind(cx.input.tcx.instance_mir(rustc_instance_kind).clone()),
   );
   ```

   Implementer confirms the exact pinned-nightly API at impl time.
   The `TypingEnv::fully_monomorphized()` matches the existing
   bridge convention -- see
   `crates/regseq-driver/src/bridge/mod.rs:1064`,
   `crates/regseq-driver/src/bridge/mod.rs:1124`,
   `crates/regseq-driver/src/bridge/mod.rs:1563`, and the layout-query
   call sites in `crates/regseq-driver/src/bridge/const_.rs:44, 291,
   487` and `crates/regseq-driver/src/bridge/ty.rs:897, 943, 998,
   1364`.

4. After substitution, the existing loops at
   `materialise.rs:252-260` and `:268-279` see a substituted body:
   `local_decls[i].ty` are concrete Tys (no `Param`), basic-block
   statements/terminators reference concrete Tys, and the
   `BodyCtxt::new_with_model_body(&rustc_body.local_decls, &mut body)`
   call sees the substituted local_decls.

### Change B: explicit `EarlyBinder` clone semantics

**Anchor**: same.

The polymorphic body returned by `tcx.instance_mir(...)` is a
`&Body<'tcx>` (immutable reference into the rustc query cache).
`instantiate_mir_and_normalize_erasing_regions` (or the equivalent)
requires an owned `EarlyBinder<Body<'tcx>>` to substitute into. The
implementer MUST clone the body before binding:

```rust
let rustc_body = cx.input.tcx.instance_mir(rustc_instance_kind).clone();
let rustc_body = rustc_instance.instantiate_mir_and_normalize_erasing_regions(
    cx.input.tcx,
    param_env,
    ty::EarlyBinder::bind(rustc_body),
);
```

The clone is unavoidable -- rustc's MIR is reference-cached and
cannot be substituted in place. The clone cost is one body per
Function Instance materialization, which is comparable to the
existing per-Instance overhead.

### Change C: no-op for non-generic Instances (regression-guard)

**Anchor**: same.

For a Function Instance whose `generic_args == identity_args ==
[Param(0), Param(1), ...]` (e.g., a non-generic fn or a bucket-1
eager-enumerated trait default body at the eager-enumeration site
itself), substitution with identity args returns the polymorphic
body essentially unchanged -- `Param(i)` maps to `Param(i)`, no
concrete Ty appears.

This is a no-op for the happy-path bucket-1 case (non-generic source
fn called as itself: `entry` -> `entry`). The substitution applies
only to Instances whose `args` differ from identity.

**Note for the implementer**: do NOT branch on "args is identity"
to skip substitution -- the substitution itself is a no-op in that
case, and the unconditional call is cleaner. (Per the
`feedback_no_defensive_fallbacks` rule, only branch when the
spec/semantics demand it; performance micro-optimisation is not a
sufficient reason.)

### Change D: harness expectations entry

**Anchor**: `.harness/expectations/regseq-driver-60.md` (new file).

Record:

- Before-fix evidence: cite the regseq-driver-59 commit `0a4502f`'s
  fail_reason and the failing test
  `crates/regseq-model/tests/e2e_trait_default_method.rs:24` +
  the `materialise.rs:243-260` walk that's the failure point.
- After-fix evidence: the regseq-driver-59 B9 test passes
  unmodified; `cargo test --workspace` returns to green.
- Out-of-scope: the regseq-driver-58 Change H spec amendment is
  unchanged; the validator's four-bucket policy is unchanged; no
  pretty-printer change.

## Non-Goals

- Do not modify the regseq-driver-58 Change H spec amendment at
  `docs/discussions/regseq-model/spec.md:2351-2358`. The amendment
  is correct; the bug is in the bridge implementation, not the
  spec.
- Do not modify the regseq-driver-58 four-bucket validator at
  `crates/regseq-model/src/model.rs:3528-3633`. Trait default bodies
  are still bucket 1 (non-generic + sig-rigid + non-foreign) at the
  trait DefPath; their Instances must materialize, which is what
  Change A enables.
- Do not modify the regseq-driver-59 test surface
  (`e2e_trait_default_method.rs`, `e2e_foreign_rendering.rs`, the
  two fixtures, the expectations file). They are correct as
  authored; the failing test starts passing automatically once
  Change A lands.
- Do not modify `crates/regseq-driver/src/bridge/ty.rs:275`. The
  `NonRigidTy::Param` rejection is correct for genuinely-polymorphic
  inputs; the fix simply stops feeding polymorphic Tys into
  `translate_ty`.
- Do not change pretty-printer.md. Rendering rules are unchanged.
- Do not modify `tests/fixtures/playground/`. Playground is the
  user's experimentation space.
- Do not retrofit substitution into Phase-2 / Phase-5 passes other
  than the body materializer. The bug is specifically at
  `materialise.rs:243`; other Phase-2/5 passes already substitute
  via `instantiate(tcx, args)` per the bridge's existing pattern.
- Do not add args-substitution for the closure-call shim path
  (`crates/regseq-driver/src/bridge/synth/closure.rs`). Closure
  bodies are synthesised, not fetched via `tcx.instance_mir`; they
  follow a different code path.

## Test Plan

### B1: regseq-driver-59 B9 test passes unmodified

**Anchor**: existing test
`crates/regseq-model/tests/e2e_trait_default_method.rs`
(`bridged_trait_default_method_body_surfaces_at_trait_def_path`).

Before this task lands: the test panics at
`crates/test-ir-fixtures/src/lib.rs:257` (the `compile_with_bridge`
helper unwrap site) because `run_bridge` returns
`Err(BridgeError::NonRigidTy)`.

After Change A lands: the test passes end-to-end. Its existing
structural assertions (exactly one Function Item with terminal
`ValueNs("greet")`, no `DefPathData::Impl` ancestor, bucket-1
`BridgeItemFlags` shape, body `Some(_)`) all hold against the
substituted body.

### B2: regseq-driver-59 B8 test stays green

**Anchor**:
`crates/regseq-model/tests/e2e_foreign_rendering.rs`
(`bridged_foreign_rendering_emits_foreign_attribute`).

Foreign Function Instances have empty `args`, so the new
substitution is an identity no-op. Asserts the existing snapshot
still pins.

### B3: regseq-driver-58 B1-B5 tests stay green

**Anchor**:
`crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs` (six
tests covering all four buckets).

Bucket-1 fixtures contain non-generic fns whose identity-args
substitution is a no-op; bucket-2 foreign Instances have empty
args; bucket-3 generic Items have no Function Instances (no
materialization happens); bucket-4 RPIT Items have no Function
Instances either. All six tests should stay green.

### B4: generic-fn-with-concrete-args body materializes

**New file**: `crates/regseq-driver/tests/bridge_generic_fn_body_materialization.rs`
OR extend
`crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs`.

Bridge a fixture:

```rust
#![no_std]
pub fn id<T: Copy>(x: T) -> T { x }
pub fn entry() -> u32 { id::<u32>(7) }
```

Assert:

- A Function Instance exists keyed on
  `(id_item, [Type(u32)], FunctionInstanceFlavor::Item)`.
- That Instance's `body == Some(_)`.
- The body's local_decls resolve to the concrete `Ty::Uint(UintKind::U32)`
  row (proves substitution applied). NOTE: the Regseq `Ty` enum has
  no `Param` variant -- `translate_ty` rejects `TyKind::Param` BEFORE
  interning (`crates/regseq-driver/src/bridge/ty.rs:275`), so the
  negative form is "no `BridgeError::NonRigidTy { kind: Param, .. }`
  raised during `run_bridge`" rather than "no `Ty::Param` Ty in the
  model".
- `run_bridge` returns `Ok(_)` (no `BridgeError::NonRigidTy`).

Before this task lands: this test fails for the same root cause as
B1 (the body materializer hits `Param(0) -> &Self` rejection... or
more precisely, `Param(0) -> T = u32` requires substitution, which
the current materializer doesn't do). Need to confirm whether
generic-fn-with-args ALREADY fails today; if it does, this test is
also a regression guard that was implicitly absent in the
regseq-driver-58 coverage.

Hypothesis: it does fail. The implementer confirms by running the
fixture against the current bridge before applying Change A.

### B5: workspace test suite returns to green

`cargo test --workspace -- --test-threads=1` green (no failing
tests, no panics, no `BridgeError::NonRigidTy` from any e2e
fixture).

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/60-body-materializer-args-substitution.md`).

(No spec amendment in this task.)

### Reviewer focus

- **Root-cause framing**: does the Description correctly identify
  `materialise.rs:153` (Instance constructed-then-defangled) and
  `materialise.rs:243` (polymorphic body fetched without
  substitution) as the joined bug surface? Cite both lines.
- **API correctness**: is the proposed
  `instantiate_mir_and_normalize_erasing_regions` (or
  `instantiate_and_normalize_erasing_regions`) the correct rustc
  API for the pinned nightly (2026-02-01)? Confirm against
  `rustc_middle::ty::Instance` and
  `rustc_middle::ty::TyCtxt` method tables. If the exact name has
  drifted in the pinned nightly, recommend the verified API name.
- **Identity-args correctness**: does substitution with identity
  args truly behave as a no-op for non-generic Instances? Confirm
  against the rustc folding semantics. If identity substitution
  has a non-trivial cost (e.g., always clones), flag it.
- **B4 viability**: does the proposed generic-fn-with-concrete-args
  fixture actually exercise the body-materializer substitution
  path? Or does rustc's MonoItem walk pre-substitute somewhere
  earlier? Confirm by tracing
  `crates/regseq-driver/src/bridge/mod.rs:1411-1623`
  (`construct_function_instances`) -- does it ever call
  `instance_mir(...)` for non-identity-args Instances pre-fix?
- **Citation drift**: every file:line must anchor against HEAD.
- **Scope creep**: per CLAUDE.md. The task is a single-spot
  production fix + harness expectations. Flag any unrequested
  scaffolding.
- **No defensive fallbacks**: per
  `feedback_no_defensive_fallbacks`. Change C explicitly forbids
  branching on "args is identity" to skip substitution. Confirm
  the implementer doesn't introduce such a branch.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/60-body-materializer-args-substitution.md.
Identify:
- Citation drift.
- API correctness against the pinned-nightly rustc.
- B4 viability (does the proposed test actually exercise the bug).
- Scope creep.
- Defensive-fallback smells.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C, D bullets with file:line
API-CORRECTNESS CHECK: PASS / FAIL with citation
B4-VIABILITY CHECK: PASS / FAIL with rustc-behavior reasoning
CITATION DRIFT: list or "none"
SCOPE-CREEP CHECK: pass/fail with citation
NO-DEFENSIVE-FALLBACK AUDIT: pass/fail
TEST PLAN VERIFICATION: B1-B5 bullets
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - The regseq-driver-59 B9 test
    (`bridged_trait_default_method_body_surfaces_at_trait_def_path`)
    passes unmodified.
  - The new B4 generic-fn-body test passes.
  - No new test failures introduced.
- `git diff --stat HEAD..` shows changes ONLY under
  `crates/regseq-driver/src/bridge/body/materialise.rs` (Change A
  + B + C) and the new
  `.harness/expectations/regseq-driver-60.md` (Change D) and the
  new B4 test file. No spec.md, no pretty-printer.md, no
  validator, no fixture, no playground modifications.
- Manual verification on `tests/fixtures/playground/`: optional --
  the playground fixture is non-generic so the substitution is a
  no-op for it; verifying that the playground model.txt is
  byte-identical post-fix confirms the no-op property.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.

## Acceptance Criteria

1. **Body materializer substitutes Instance args**:
   `crates/regseq-driver/src/bridge/body/materialise.rs:243` (or
   immediately after the existing `tcx.instance_mir(...)` call)
   applies `instance.args` to the body via
   `instantiate_mir_and_normalize_erasing_regions` (or the
   equivalent `tcx.instantiate_and_normalize_erasing_regions`).

2. **Instance kept as full `Instance<'tcx>` through the match arms**:
   each arm at `materialise.rs:145-240` constructs the full
   `Instance<'tcx>` (not just its `.def`), threading the `.args`
   field through to the substitution call. The `Item`/`DropGlue`/
   `VTableShim` arms all provide concrete `args`.

3. **No `NonRigidTy::Param` regression**:
   `crates/regseq-driver/src/bridge/ty.rs:275` rejection is
   unchanged. The materializer simply never feeds polymorphic Tys
   into `translate_ty` after Change A.

4. **regseq-driver-59 B9 test passes unmodified**:
   `crates/regseq-model/tests/e2e_trait_default_method.rs::bridged_trait_default_method_body_surfaces_at_trait_def_path`
   passes without changes to the test file or its fixture.

5. **regseq-driver-59 B8 test stays green**: foreign-decl rendering
   snapshot unchanged.

6. **regseq-driver-58 B1-B5 tests stay green**: bucket-coverage
   tests unchanged.

7. **B4 generic-fn-body materialization test lands**: a new test
   exercises `pub fn id<T: Copy>(x: T) -> T` called as
   `id::<u32>(7)` and asserts the resulting Function Instance's
   body contains concrete `Ty::Uint(UintKind::U32)` and `run_bridge`
   returns `Ok(_)` without raising `BridgeError::NonRigidTy` (the
   model has no `Ty::Param` variant; the negative form is the
   absence of the bridge error) in
   local_decls.

8. **No defensive fallback for identity-args**: Change A does NOT
   branch on "args is identity" to skip the substitution.
   Identity substitution is applied unconditionally; the rustc
   folder makes it a no-op naturally.

9. **No spec / pretty-printer / validator changes**: `git diff
   main -- docs/ crates/regseq-model/src/model.rs` returns zero.

10. **Workspace test suite passes**: `cargo test --workspace --
    --test-threads=1` green.

11. **Expectations file lands**:
    `.harness/expectations/regseq-driver-60.md` records before/after
    evidence with file:line citations.
