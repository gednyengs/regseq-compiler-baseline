# regseq-driver-62: implementation evidence

Implementation deliverable for `regseq-driver-62`, the synthesis
pre-scan args-substitution fix described at
`.harness/tasks/regseq-driver/62-prescan-args-substitution.md`.

Parent context: `.harness/expectations/regseq-driver-60.md` (the
parallel body-materializer fix; this task closes the same root-cause
shape in a different pass).

## Scope

Focused production-code fix to the Phase-2 synthesis pre-scan loop
at `crates/regseq-driver/src/bridge/mod.rs:762-806` (Change A per
task spec). Plus two minimum-necessary corollary changes in
`crates/regseq-driver/src/bridge/body/terminator.rs` that the B2
closure-apply fixture requires to bridge end-to-end (see
"Corollary closure-call shim fixes" below). Adds the spec-mandated
fixture and one new bridge integration test. No spec amendment, no
schema change, no pretty-printer change, no four-bucket validator
change.

## Before-fix evidence

- Pre-fix the synthesis pre-scan at
  `crates/regseq-driver/src/bridge/mod.rs:769` (pre-fix) fetched the
  polymorphic MIR via `ctxt.input.tcx.optimized_mir(local_def_id)`
  for every monomorphized `MonoItem::Fn` Instance and dropped
  `instance.args` on the floor. For the canonical closure-apply
  indirection shape (`apply<F: Fn(i32, i32) -> i32>(...)` calling
  `f(x, y)`, monomorphized to `apply::<{closure}>`), the
  polymorphic body's call terminator was `<F as Fn>::call(&f, (x, y))`
  with `F == TyKind::Param(0)`. The inner
  `scan_terminator_for_synthesis` loop reached
  `canonicalize_call_site_callee` at
  `crates/regseq-driver/src/bridge/mod.rs:1039-1047` with `Self ==
  Param`, and `rustc_middle::ty::Instance::try_resolve` returned
  `Ok(None)` because trait-method resolution requires a rigid
  `Self`. The bridge then surfaced this as
  `BridgeError::Phase4("call-site Instance::try_resolve failed for
  DefId(... Fn::call) with args [impl Fn(i32, i32) -> i32/#0,
  (i32, i32)]: Instance::try_resolve returned Ok(None)")` at
  `crates/regseq-driver/src/bridge/mod.rs:1042-1046`.
- This is the IDENTICAL root-cause shape as regseq-driver-60 (closed
  at `a1ac768`): process polymorphic MIR for a monomorphized
  Instance without applying `instance.args`. regseq-driver-60 fixed
  the Phase-5 body materializer at
  `crates/regseq-driver/src/bridge/body/materialise.rs:287-293`;
  this task closes the parallel gap in the Phase-2 pre-scan.

## After-fix evidence

### Change A: synthesis pre-scan substitutes `instance.args` into MIR

- `crates/regseq-driver/src/bridge/mod.rs:782-790` now fetches the
  polymorphic body via `ctxt.input.tcx.instance_mir(instance.def)`
  (not `optimized_mir(local_def_id)`) so non-`Item` flavors
  (DropGlue, VTableShim, etc.) receive the right body view per the
  pinned-nightly rustc API contract, then applies `instance.args`
  via `instance.instantiate_mir_and_normalize_erasing_regions(tcx,
  TypingEnv::fully_monomorphized(),
  EarlyBinder::bind(polymorphic_body.clone()))`, matching the
  regseq-driver-60 Change A pattern at
  `crates/regseq-driver/src/bridge/body/materialise.rs:287-293`.
- The substituted body is consumed UNCHANGED by the existing
  `scan_ty_for_synthesis` / `scan_statement_for_synthesis` /
  `scan_terminator_for_synthesis` loops (lines 791-803). For
  non-generic Instances `instance.args` is the identity
  substitution and the substitution call is a no-op naturally, per
  the no-defensive-fallback rule (no `if args.is_identity { skip }`
  branch).
- Pinned-nightly API confirmation (rustc nightly-2026-02-01):
  `Instance::instantiate_mir_and_normalize_erasing_regions` at
  `rustc_middle/src/ty/instance.rs` calls
  `tcx.instantiate_and_normalize_erasing_regions(args, env, binder)`
  when `has_polymorphic_mir_body()` is true and falls back to
  `tcx.normalize_erasing_regions(env, v.instantiate_identity())`
  when false (e.g., `DropGlue(_, Some(_))`).

### Corollary closure-call shim fixes (required by spec acceptance criterion 3)

Change A on its own unblocks the Phase-2 pre-scan for the B2
closure-apply fixture, but the resulting monomorphized
`apply::<{closure}>` body cannot complete end-to-end without two
narrow corrections in
`crates/regseq-driver/src/bridge/body/terminator.rs` that surface
pre-existing structural gaps in the closure-call shim pipeline
(the gaps were latent because no committed fixture invoked a
closure directly via `<closure as Fn>::call(...)` -- the existing
`closure_call_sig_sync.rs` fixture header at
`crates/test-ir-fixtures/fixtures/closure_call_sig_sync.rs:9-19`
explicitly notes this latent surface). Spec acceptance criterion
3 mandates that the B2 fixture "compiles cleanly via both the
`bridge_fixture` test helper and the production `cargo regseq`
wrapper", so this task closes the two gaps.

- **`pick_closure_call_handle`
  (`crates/regseq-driver/src/bridge/body/terminator.rs:364-391`)
  now resolves bound handles to their final `ItemId` instead of
  the pre-Phase-4 placeholder.** Pre-fix the helper always used
  `placeholder_item_id_for_handle(*handle)` for the
  `lookup_item_synthetic_kind` query, which only succeeds while
  `pending_items` still carries the synth entry. After
  `assign_item_ids_and_populate_sidetables` drains `pending_items`,
  the placeholder no longer resolves and the helper returned
  `None`, causing `rewrite_closure_call_shim` to fall through to
  `translate_call_callee` -> `callsite_fn_sig` ->
  `tcx.fn_sig(closure_def_id)`, which trips the rustc ICE at
  `compiler/rustc_hir_analysis/src/collect.rs:1028` ("to get the
  signature of a closure, use `args.as_closure().sig()` not
  `fn_sig()`"). Since `rewrite_closure_call_shim` is invoked
  during Phase-5 body materialisation (post-Phase-4 by
  construction), the bound-ItemId path is the load-bearing one.

- **`cast_receiver_to_captures_view`
  (`crates/regseq-driver/src/bridge/body/terminator.rs:336-389`):
  a new helper that emits a `CastKind::Transmute` Statement
  re-typing the closure-call receiver from the `Ty::Closure(item,
  args)` view to the parallel `Ty::Struct { item, args, layout }`
  view per spec.md section 13.1.** The closure-call shim's
  `FnSig.inputs[0]` is constructed against the captures-struct
  view per spec.md:4590-4594 at
  `crates/regseq-driver/src/bridge/mod.rs:2278-2289`, but the
  MIR call site's receiver Place carries the closure-type-identity
  view per spec.md:4619-4634 ("`Ty::Closure(item, args)` -- the
  closure-type-identity view (used in fn signatures and Place
  types)"). The two views share one `Item` and identical layout,
  so the transmute is bit-equivalent. Without the cast the
  validator's `CallArgTypeMismatch` check at
  `crates/regseq-model/src/model.rs:3041-3048` rejects the
  pairing. The helper is no-op when the receiver is already in
  the captures-struct view (so existing fixtures that never
  invoke a closure directly stay green). The
  `cast_receiver_to_captures_view` synthetic local lives in the
  MODEL body's `local_decls` only (not the rustc local-decls
  view), so the per-operand arg-Ty recovery downstream at
  `terminator.rs:301-326` is routed through the model-side
  `regseq_model::operand_ty` helper instead of the rustc-side
  index lookup that pre-fix would panic on the synthetic-local
  index.

### Test evidence

- New fixture:
  `crates/test-ir-fixtures/fixtures/closure_apply_indirection.rs`
  is byte-identical to the spec's B2 source: `pub fn apply(x:
  i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 { f(x, y) }`
  + `pub fn entry(x: i32, y: i32) -> i32 { apply(x, y, |x, y| x
  + y) }`.
- New test:
  `crates/regseq-driver/tests/bridge_closure_apply_indirection.rs`
  bridges the fixture and asserts the spec's B2 assertion set:
  - `bridge_fixture("closure_apply_indirection")` returns
    `Ok(_)` without panicking (the success path is "did not
    panic" per `crates/test-ir-fixtures/src/lib.rs:90-99` +
    `:254-258`).
  - A Function Instance keyed on the source `apply` Item exists
    with `flavor == FunctionInstanceFlavor::Item` and
    `body.is_some()` (so Phase-5 body materialization completed
    against the substituted body).
  - The resulting model has at least one
    `SyntheticItemKind::ClosureCall { call_kind:
    ClosureCallKind::Call, .. }` Item (per spec.md section 13.1
    closure-family synthesis at `bridge/mod.rs:2278+`).
- `cargo test --workspace -- --test-threads=1` is green; no
  regressions in regseq-driver-58 / -59 / -60 / -61 tests or any
  existing workspace test.

### B4: end-to-end smoke via `cargo regseq`

Manually verified. Set up the throwaway crate per the spec's
template (`regseq_driver_62_smoke`, `panic = "abort"`, pinned
toolchain `nightly-2026-02-01`, byte-identical `src/lib.rs`),
then drove the production wrapper from the throwaway-crate root:

```
PATH="$WS/target/debug:$PATH" cargo regseq \
  --target-os linux \
  --platform-file $WS/tests/fixtures/playground/platform.toml \
  --emit model
```

Output: `regseq-out/model.txt` (92 lines, pretty-printed model)
containing the `apply` Function with `local_decls[3]:
regseq_driver_62_smoke::entry::{closure#0}<...>`, the `entry`
Function with the closure construction and the apply call, all
three closure-call shims (`call`, `call_mut`, `call_once`) for
the closure, the closure drop-glue Function, and zero
`BridgeError::Phase4` surfaces. The production-wrapper code
path matches the in-process `bridge_fixture` test-helper path.
Manual verification recorded at commit (to be assigned post-
`task.py done`).

## Out of scope (per task Non-Goals)

- No modification to the regseq-driver-60 body materializer fix at
  `crates/regseq-driver/src/bridge/body/materialise.rs:287-293`. The
  Phase-5 substitution is correct; this task closes the parallel
  gap in Phase-2.
- No modification to `canonicalize_call_site_callee` at
  `crates/regseq-driver/src/bridge/mod.rs:2460-2501`. The resolver
  is correct; the bug was that the pre-scan was feeding it
  polymorphic inputs.
- No modification to the `Instance::try_resolve` -> `BridgeError::Phase4`
  conversion at `crates/regseq-driver/src/bridge/mod.rs:1039-1047`.
  The conversion is the correct shape for legitimately-unresolvable
  call sites; after this fix, the pre-scan stops feeding it
  polymorphic inputs.
- No spec.md / pretty-printer.md / generic-c-backend.md changes.
  The bug was in the bridge implementation, not the spec.
- No `tests/fixtures/playground/` changes (playground is the user's
  experimentation space per `playground-is-experimentation-space`).
- No defensive `if args.is_identity { skip_substitute }` branch
  (per `feedback_no_defensive_fallbacks`). Identity substitution is
  applied unconditionally; the rustc folder makes it a no-op
  naturally.
- No `construct_closure_call_instances` changes at
  `bridge/mod.rs:2278-2289`. The eager-shim FnSig still expects the
  captures-struct view per spec.md:4590-4594; the receiver cast on
  the call site is the correct bridge between the call-site
  closure-type-identity view and the shim sig's captures-struct
  view. The pre-existing regseq-driver-51 / regseq-driver-37 tests
  at `crates/regseq-driver/tests/bridge_synth.rs:1005-1088` that
  assert the captures-struct receiver shape stay green.

## Spec-author scoping note

The task spec's Quality Gate listed `crates/regseq-driver/src/bridge/mod.rs`
as the sole production-code edit surface. In practice the closure-
apply fixture mandated by Test Plan B2 requires the two corollary
terminator.rs fixes documented above; without them the bridge fails
with `pick_closure_call_handle` returning `None` (rustc ICE in
`callsite_fn_sig`) and the validator's `CallArgTypeMismatch` check.
Both are pre-existing latent structural gaps unrelated to the
Phase-2 root cause but pinned-blocking on B2's acceptance contract
("the B2 fixture compiles cleanly"). The corollary fixes are kept
strictly minimal: one query-target swap in `pick_closure_call_handle`
plus one new helper that no-ops when the receiver already uses the
captures-struct view. Per the task spec's
`default_follow_through_policy`, this is the most spec-faithful
interpretation -- it preserves the spec's literal B2 fixture and
assertion set while addressing the corollary issues that block the
spec acceptance contract.
