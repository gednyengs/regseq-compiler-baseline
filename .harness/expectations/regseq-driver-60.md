# regseq-driver-60: implementation evidence

Implementation deliverable for `regseq-driver-60`, the body-materializer
args-substitution fix described at
`.harness/tasks/regseq-driver/60-body-materializer-args-substitution.md`.

Parent context: `.harness/expectations/regseq-driver-59.md` (which
shipped the failing B9 test that surfaces the bug).

## Scope

Focused production-code fix to the Phase-5 body materializer at
`crates/regseq-driver/src/bridge/body/materialise.rs`, plus the two
necessary corollary changes in
`crates/regseq-driver/src/bridge/mod.rs` (eager HIR enumeration
classifier + `construct_function_instances` skip rule) required to
unblock the regseq-driver-59 B9 test per its acceptance contract.
Adds one new regseq-driver-60 test (B4 generic-fn body
materialization) and the trait-default-method snapshot that the B9
test now reaches. No spec amendment, no schema change, no
pretty-printer change, no four-bucket validator change.

## Before-fix evidence

- Regseq-driver-59 commit `0a4502f` fail reason: B9 test
  `bridged_trait_default_method_body_surfaces_at_trait_def_path`
  at `crates/regseq-model/tests/e2e_trait_default_method.rs:24`
  panics at the `compile_with_bridge` unwrap site
  (`crates/test-ir-fixtures/src/lib.rs:257`) because `run_bridge`
  returns `Err(BridgeError::NonRigidTy { kind: Param, ty_repr:
  "Self" })`.
- Root cause walk: the Phase-5 body materializer at
  `crates/regseq-driver/src/bridge/body/materialise.rs:153`
  (pre-fix) constructed a rustc `Instance` via
  `Instance::new_raw(local_def_id.to_def_id(), translated_args).def`
  and kept ONLY the `.def` field, dropping the `.args` field on
  the floor. The subsequent
  `cx.input.tcx.instance_mir(rustc_instance_kind)` call at
  pre-fix line 243 returned the polymorphic MIR body for trait
  default method `Greeter::greet`, whose `local_decls[0].ty` is
  `TyKind::Ref(_, TyKind::Param(0), _)`. The local-decl loop at
  pre-fix lines 252-260 then fed that polymorphic Ty into
  `translate_ty`, which rejects `TyKind::Param` at
  `crates/regseq-driver/src/bridge/ty.rs:275` as
  `BridgeError::NonRigidTy`.

## After-fix evidence

### Change A: body materializer substitutes `Instance.generic_args`

- `crates/regseq-driver/src/bridge/body/materialise.rs` now threads
  the full `Instance<'tcx>` (def + args) through each flavor arm
  (Item, DropGlue, VTableShim, ClosureCall, etc.) and applies
  `instance.instantiate_mir_and_normalize_erasing_regions(tcx,
  TypingEnv::fully_monomorphized(),
  EarlyBinder::bind(rustc_body_poly.clone()))` to substitute the
  args into the polymorphic MIR before walking
  `local_decls`. The substituted body is allocated in the tcx
  arena so the downstream loops that consume
  `&'tcx mir::LocalDecls<'tcx>` via
  `BodyCtxt::new_with_model_body` keep their existing borrow
  contract.
- Pinned-nightly API confirmation (rustc nightly-2026-02-01):
  `Instance::instantiate_mir_and_normalize_erasing_regions` at
  `rustc_middle/src/ty/instance.rs:885` calls
  `tcx.instantiate_and_normalize_erasing_regions(args, env,
  binder)` when `has_polymorphic_mir_body()` is true and falls
  back to `tcx.normalize_erasing_regions(env,
  v.instantiate_identity())` when false (e.g.,
  `DropGlue(_, Some(_))`), so identity-args substitution is a
  no-op for non-generic Instances per Change C of the task spec.

### Change B + C: bucket-1 classification carries trait default methods

- `crates/regseq-driver/src/bridge/mod.rs::eager_hir_enumeration`
  now classifies trait default method Items
  (`tcx.def_kind(def_id) == AssocFn && tcx.trait_of_assoc(def_id).is_some()`)
  by their `own_params` rather than the parent-inheriting
  `count()`. This realizes the regseq-driver-60 task Non-Goal
  rule "Trait default bodies are still bucket 1 (non-generic +
  sig-rigid + non-foreign) at the trait DefPath" in code.
- `construct_function_instances` skips the bucket-1 push for
  trait default method Items: their identity_args include the
  trait's `Self` and cannot be resolved via
  `Instance::try_resolve` with identity args. The concrete
  Function Instance for the resolved receiver is created
  reactively by Phase 5's `lookup_or_create_direct_instance` at
  the call site per the regseq-driver-56 canonicalization, and
  the validator's bucket-1 count check (exactly 1 Instance) still
  applies to that reactive Instance.

### Test evidence

- Regseq-driver-59 B9 test passes unmodified:
  `bridged_trait_default_method_body_surfaces_at_trait_def_path`
  at `crates/regseq-model/tests/e2e_trait_default_method.rs:24`
  passes end-to-end. The trait-default-method render snapshot
  lands at
  `crates/regseq-model/tests/snapshots/e2e_trait_default_method__render.snap`
  with the `42_u32` default-body literal and the block-body
  rendering shape.
- Regseq-driver-59 B8 test stays green:
  `crates/regseq-model/tests/e2e_foreign_rendering.rs`.
- Regseq-driver-58 B1-B5 tests stay green:
  `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs`.
- New regseq-driver-60 B4 test:
  `crates/regseq-driver/tests/bridge_generic_fn_body_materialization.rs`
  bridges `pub fn id<T: Copy>(x: T) -> T { x }` called as
  `id::<u32>(7)` and asserts:
  - `run_bridge` returns `Ok(_)` (no `BridgeError::NonRigidTy`).
  - A Function Instance is keyed on the `id` Item with
    `generic_args == [Type(u32)]` and flavor `Item`.
  - The Instance's `body == Some(_)`.
  - At least one `LocalDecl.ty` in the materialized body
    resolves to `Ty::Uint(UintKind::U32)`, proving the
    substitution applied.
- `cargo test --workspace -- --test-threads=1` is green.

## Out of scope (per task Non-Goals)

- No modification to the regseq-driver-58 Change H spec amendment
  at `docs/discussions/regseq-model/spec.md:2351-2358`. The
  amendment is correct as authored; the bug was in the bridge
  implementation, not the spec.
- No modification to the regseq-driver-58 four-bucket validator
  at `crates/regseq-model/src/model.rs:3528-3633`. The bucket
  policy is unchanged.
- No modification to the regseq-driver-59 test surface
  (`e2e_trait_default_method.rs`, `e2e_foreign_rendering.rs`,
  the two fixtures, the regseq-driver-59 expectations file).
  The previously-`.new` trait-default-method snapshot is
  promoted to its committed location as a corollary of the B9
  test now reaching the snapshot assertion.
- No modification to `crates/regseq-driver/src/bridge/ty.rs:275`.
  The `NonRigidTy::Param` rejection remains correct for
  genuinely-polymorphic inputs; the fix simply stops feeding
  polymorphic Tys into `translate_ty`.
- No change to pretty-printer.md. Rendering rules are unchanged.
- No retrofit of args-substitution into Phase-2 / Phase-5 passes
  other than the body materializer. Other passes already
  substitute via `instantiate(tcx, args)` on rustc binders per
  the existing pattern at
  `crates/regseq-driver/src/bridge/body/terminator.rs:491-501`.
- No args-substitution for the closure-call shim's synthesised
  body path (`crates/regseq-driver/src/bridge/synth/closure.rs`).
  Closure bodies are synthesised, not fetched via
  `tcx.instance_mir`, and follow a different code path.

## Spec-author misdiagnosis note

The task spec at
`.harness/tasks/regseq-driver/60-body-materializer-args-substitution.md`
predicted that Change A alone would make the regseq-driver-59 B9
test pass. In practice Change A alone unblocks the materializer
but the test then fails at structural assertion (d) (bucket-1
flags) because the eager HIR classifier was setting
`is_generic = true` for trait default methods (Self is in the
parent generics chain). Change B + C realize the
"Trait default bodies are still bucket 1 at the trait DefPath"
non-goal rule explicitly in code, since acceptance criterion 4
requires the B9 test to pass unmodified.
