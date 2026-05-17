# regseq-driver-51: Eager Closure-Call Function Instance Synthesis

## Description

Close the bridge synthesis gap surfaced by `regseq-model-95`
(P1a-E2E-03) and independently diagnosed by an adversarial Codex
review on `fixes/audit-2026-05-15`: synthesised `ClosureCall`
Function Items (per `spec.md` §13.1; `spec.md:4531-4566`) do
NOT receive a corresponding `Function` `Instance` entry during
Phase-2 / Phase-4 synthesis, leaving them as orphans in
`model.items` without a parallel entry in `model.instances`.

Two spec invariants are violated by the current state:

- **spec.md:4531-4546** -- lines 4531-4541 introduce the rule
  ("For each call-method Function Item, the corresponding
  Function `Instance`'s `InstanceKind::Function { fn_sig, ... }`
  ... carries the substituted signature derived from the
  closure's `closure_sig_as_fn_ptr_ty`, with a per-call-kind
  receiver shape"); lines 4542-4546 carry the actual mappings:
  - `Call`     -> receiver `Ref(captures_struct, SharedBorrow)`
                   (i.e., `&Captures`)
  - `CallMut`  -> receiver `Ref(captures_struct, UniqueBorrow)`
                   (i.e., `&mut Captures`)
  - `CallOnce` -> receiver `captures_struct` by value

- **spec.md:778-781** -- "`body.is_some()` for source-defined
  functions, bridge-synthesized shims (drop glue, vtable shims,
  **closure call methods**), and any other Item with `linkage
  != Linkage::Foreign`."

The bridge today eagerly constructs Instances only for Items
whose `source_item_def_id` resolves to a rustc DefId in this
compilation. `crates/regseq-driver/src/bridge/mod.rs:1114-1121`
explicitly skips items where `source_item_def_id` returns
`None`, deferring construction to call-site lazy allocation
via `lookup_or_create_direct_instance`
(`crates/regseq-driver/src/bridge/body/terminator.rs:303,384,
609`). For `ClosureCall` Function Items the synthetic Item has
no rustc DefId (it's bridge-only), so it is unconditionally
skipped by the Phase-4 walker.

The materialisation machinery to construct a body for these
Instances already exists at
`crates/regseq-driver/src/bridge/body/materialise.rs:87-100`:
it resolves the closure's MIR via `tcx.hir_body_owners().find()`
against the closure's `DefPathHash` and constructs an
`Instance::new_raw(closure_def_id, args).def`. What is missing
is the eager Instance entry creation that feeds this materialiser.

After this task lands:

- Every synthesised `ClosureCall` Function Item carries a
  corresponding `Function` `Instance` entry in `model.instances`.
- Each Instance's `fn_sig` resolves to a `FnSig` whose receiver
  shape (`inputs[0]`) matches the per-`ClosureCallKind` rule.
- Each Instance's `body` is `Some(Body)` populated by the
  existing `materialise.rs:87-100` resolution path.
- `regseq-model-95`'s closure snapshot at
  `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`
  is re-baselined to capture the ClosureCaptures struct + at
  least one ClosureCall Function Instance rendering.

ClosureDropGlue Function Items (also listed under spec.md:778-781's
"bridge-synthesized shims" enumeration) are NOT in scope; the
`|x| x + 1` fixture exercises only the zero-capture path, so no
ClosureDropGlue is synthesised. The drop-glue Instance gap (if
any) will be surfaced by a future capture-bearing closure
fixture and scoped as its own task.

## Input Context

- `docs/discussions/regseq-model/spec.md` §13.1 Closure family
  (`spec.md:4503-4567`), in particular:
  - The ClosureCall Function Item shape + per-call-kind
    receiver rule (`spec.md:4531-4546`) -- the load-bearing
    wording for the FnSig receiver shape this task synthesises.
    Lines 4531-4541 introduce the rule; lines 4542-4546 carry
    the actual per-`ClosureCallKind` mappings (`Call` shared
    ref, `CallMut` unique ref, `CallOnce` by value).
  - The `ClosureKind` -> synthesised-arms mapping
    (`spec.md:4531-4535`) -- `Fn` -> `{Call, CallMut, CallOnce}`,
    `FnMut` -> `{CallMut, CallOnce}`, `FnOnce` -> `{CallOnce}`.
  - The §3.1 invariant 2 carve-out (`spec.md:5200-5203`) --
    ClosureCaptures retains rustc's verbatim DefPath while
    ClosureCall arms use the synthetic `<regseq_synth>::...`
    DefPath (`spec.md:5067-5077`).
- `docs/discussions/regseq-model/spec.md` §9.4 Instance body
  presence invariant (`spec.md:768-799`) -- the `body.is_some()`
  rule for non-Foreign Items including closure call methods.
- `docs/discussions/regseq-model/spec.md` §3.5.2 `instance_intrinsic`
  + Instance pull-through pattern (already referenced by
  regseq-driver-49) -- the architectural pattern this task
  preserves.
- `crates/regseq-driver/src/bridge/synth/closure.rs:42-91`
  -- `synthesise_closure_tail` (where ClosureCall Items get
  pushed via `push_pending_synthetic_item`); the natural site to
  ALSO record the data needed to construct the Instance later.
- `crates/regseq-driver/src/bridge/synth/closure.rs:129-141`
  -- `closure_call_kinds` (the per-ClosureKind arm enumeration
  this task's FnSig synthesis branches off).
- `crates/regseq-driver/src/bridge/synth/vtable.rs:150-193`
  -- `synthesise_vtable_method_shim_signature` (the reference
  pattern for synthesising an erased-receiver C-ABI FnSig +
  populating `sig_c_abi_lowerings`; ClosureCall FnSigs are
  `FnAbi::Rust`, so the lowering branch is `None`, but the
  intern-then-record pattern is the same).
- `crates/regseq-driver/src/bridge/mod.rs:1088-1175`
  -- `construct_function_instances` (the Phase-4 walker; this
  task extends it OR adds a parallel walk over synthesised
  ClosureCall Items).
- `crates/regseq-driver/src/bridge/mod.rs:1114-1121` -- the
  current skip-when-no-rustc-DefId site this task changes.
- `crates/regseq-driver/src/bridge/body/materialise.rs:87-100`
  -- the closure-body resolution machinery this task feeds.
- `crates/regseq-driver/src/bridge/body/terminator.rs:303,384,609`
  -- the lazy `lookup_or_create_direct_instance` paths that
  currently allocate ClosureCall Instances on demand; this task
  does NOT remove those paths (call-site lazy allocation
  remains valid for late discovery), but the eager path means
  the lazy paths are no-ops for already-constructed Instances.
- `crates/regseq-model/src/instance.rs` (or wherever
  `Instance::new_function` / equivalent constructor lives) --
  the model-side Instance constructor this task calls.
- `crates/regseq-model/tests/e2e_synthesis.rs:444-485` -- the
  current closure-snapshot comment block that explicitly cites
  this task as the unblocker; after this task lands the
  comment can be tightened further and the snapshot re-baselined.
- `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`
  -- the snapshot file that gets re-baselined as part of this
  task's Test Plan F.
- `.harness/tasks/regseq-model/95-e2e-synthesis-vtable-and-closure.md`
  -- the downstream task whose closure-side coverage gets
  promoted from item-level to Instance-level by this task.
- `.harness/tasks/regseq-driver/40-closure-vtable-drop-glue-synthesis.md`
  -- regseq-driver-40; the original synthesis task whose closure
  family this audit-finding extends.

## Implementation Notes

Two changes, in order:

### Change A: Construct ClosureCall Function Instances after Phase-4 assignment, before Phase-5 body passes

Phase placement is load-bearing. `Instance.item` is an `ItemId`
(`crates/regseq-model/src/instance.rs:134-144`), but during
Phase-2 the bridge has only `ItemHandle`s -- `ItemId`s are
assigned by `bridge/assign.rs:28-33` during Phase-4. Construction
therefore MUST happen AFTER Phase-4 ItemId assignment and
BEFORE `run_phase5_body_passes` at
`crates/regseq-driver/src/bridge/mod.rs:1015-1027`.

Phase-2 records the synthetic ClosureCall Items as today
(`push_pending_synthetic_item` at `synth/closure.rs:48-58`,
unchanged). The bridge MUST then walk those Items after Phase-4
assignment and construct one `Function` `Instance` per arm, such
that:

- `model.instances[<id>].item == <ClosureCall Item's ItemId>`
- `model.instances[<id>].kind == InstanceKind::Function { ... }`
- The Instance's `fn_sig` resolves to a `FnSig` with:
  - `inputs[0]` typed per the per-`ClosureCallKind` receiver
    rule at `spec.md:4542-4546`:
    - `Call`     -> `Ty::Ref(captures_struct_ty,
                              Provenance::SharedBorrow)`
    - `CallMut`  -> `Ty::Ref(captures_struct_ty,
                              Provenance::UniqueBorrow)`
    - `CallOnce` -> `captures_struct_ty` by value (no `Ref`
                    wrapper)
  - `inputs[1..]` is the closure's argument types (per
    `closure_sig_as_fn_ptr_ty.inputs[0]`, which is the tupled
    argument struct -- the bridge must untuple this into flat
    inputs, OR follow the rustc-side `tupled_upvars` shape; see
    the existing untupling logic at
    `crates/regseq-driver/src/bridge/body/terminator.rs:219`).
  - `output` is the closure's return type (per
    `closure_sig_as_fn_ptr_ty.output`).
  - `abi: FnAbi::Rust`, `c_variadic: false`,
    `safety: Safety::Safe`.
- The Instance's `body` field is `None` at construction time.
  The Phase-5 body materialisation pass at
  `bridge/body/materialise.rs:300-338` then fills it in via the
  existing closure-body resolution machinery at
  `materialise.rs:87-100` (which keys off the closure's
  `DefPathHash` to recover the closure DefId and call into
  `tcx.instance_mir`). Do NOT call `materialise_function_body`
  inline from the construction site: it requires an `InstanceId`
  and reads `cx.model.instances[instance_handle]` /
  `instance.item` / `item_synthetic_kinds[item]`
  (`bridge/body/materialise.rs:44-77`); the body-pass loop at
  `bridge/body/materialise.rs:303-338` is the canonical fill-in
  path that Phase-5 invokes.

The eager construction is keyed off the `closure_def_id` and the
`ClosureCallKind` for each pushed arm. The closure's tupled-args
+ return type are reachable via
`closure_def.closure_sig_as_fn_ptr_ty()` (rustc), or via the
already-translated parallel `Ty::Closure` view interned in the
type interner (per stage-3 D6.3).

`FnSig` interning + `sig_c_abi_lowerings` population uses the
canonical pattern from `bridge/fnsig.rs:51-58` (lowering is `None`
because ClosureCall FnSigs are `FnAbi::Rust`, but
`ensure_cabi_lowering_slots` MUST be called to keep the side
table dense per spec.md §3.3 step 5).

**Idempotency.** Two layers of idempotency are required:

- *Item-level*, already handled by the existing
  `closure_synth_cache` at `synth/closure.rs:29-35,80-81` --
  repeated synthesis for the same `closure_def_id` returns the
  cached `ClosureSynthHandles`, so the Items are not duplicated.
- *Instance-level*, NOT handled today. Construction MUST guard
  against creating duplicate Function Instances for the same
  `(ItemId, ArgsKey, FunctionInstanceFlavor)` triple. The
  canonical guard pattern is the lookup table consulted by
  `lookup_or_create_direct_instance` at
  `bridge/body/terminator.rs:513-534`; the new construction site
  MUST use the same `(item, args_key, fn_shim_disambig)` lookup
  before pushing a fresh Instance row. After Phase-4 assignment
  the table is keyed by `ItemId`, so the guard naturally
  collapses with any lazy-call-site Instance that
  `lookup_or_create_direct_instance` would otherwise create.

### Change B: Post-assignment ClosureCall Instance pass

Add a new synthetic-walk helper that runs after Phase-4
assignment (`mod.rs:1015-1027`) and either parallels
`construct_function_instances` or extends it with a synthetic
ClosureCall branch. The existing `source_item_def_id` filter at
`mod.rs:1114-1121` MUST remain intact -- it correctly skips
items with no rustc DefId; the new synthetic branch handles the
ClosureCall case explicitly. Two implementation shapes are
acceptable:

- **B1 (parallel walk).** A new helper next to
  `construct_function_instances` that iterates
  `model.item_synthetic_kinds` for `SyntheticItemKind::ClosureCall
  { closure, call_kind }` entries and constructs one Instance
  per entry using the Change-A FnSig + receiver shape rules.
  Invoked from `mod.rs:1026` (after `construct_function_instances`
  returns, before `run_phase5_body_passes`).
- **B2 (extended walker).** Extend
  `construct_function_instances` itself with a second walk
  pass over synthetic ClosureCall entries after the existing
  source-DefId pass. The two-pass shape avoids interleaving
  source and synthetic logic.

B1 is the preferred shape because it keeps the existing walker
shape unchanged and the synthetic case isolated; B2 is acceptable
if the agent finds it more natural.

**No change to `lookup_or_create_direct_instance`.** Its
existing lazy-allocation path at `terminator.rs:303,384,609`
becomes a no-op for already-constructed ClosureCall Instances
(the duplicate guard at `terminator.rs:513-534` finds the eager
entry). The lazy path remains valid for any late discovery of
ClosureCall sites the eager pass missed; this task does not
remove it.

### Change C: Re-baseline `regseq-model-95`'s closure snapshot

After Change A, `crates/regseq-model/tests/e2e_synthesis.rs`'s
`closure_synthesis_and_struct_layout_view` test will fail
because the snapshot at
`crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`
expects only `make_inc` + `core::mem::drop` (the current thin
form). Re-baseline via:

```
cargo insta review
```

Accept the new snapshot. The accepted snapshot MUST include:

- A `<regseq_synth>::...::call::...` Function Instance rendering
  per `pretty-printer.md` §6.4 / §10.2.1.
- The `ClosureCaptures` ADT-definition Instance rendered as an
  `ItemKind::Struct` (zero-field, ZST) per
  `pretty-printer.md` §10.2.4.
- The original `make_inc` + `core::mem::drop` Instance
  renderings (preserved).
- At least one inherited arm (`CallMut` or `CallOnce`).

Also tighten the comment at `tests/e2e_synthesis.rs:444-485` to
remove the "bridge gap" framing now that the gap is closed; the
comment can shrink to a 2-3-line note that the snapshot
captures the full ClosureCall arms + ClosureCaptures struct.

### Conventions

- The new post-Phase-4 helper lives next to
  `construct_function_instances` at
  `crates/regseq-driver/src/bridge/mod.rs:1088+`, NOT inside
  `synth/closure.rs` (which only registers Items in Phase-2;
  `synthesise_closure_tail` itself is unchanged). The helper
  reads `model.item_synthetic_kinds` for
  `SyntheticItemKind::ClosureCall { closure, call_kind }`
  entries and emits one Instance per entry.
- Doc comments on any new `pub` / `pub(crate)` function citing
  the relevant `spec.md` line range.
- Idempotency MUST cover both layers per the Change-A
  Idempotency block: Item-level via the existing
  `closure_synth_cache` (unchanged) AND Instance-level via the
  `(ItemId, ArgsKey, FunctionInstanceFlavor)` lookup pattern at
  `bridge/body/terminator.rs:513-534`. Cache-only guarding is
  insufficient because Instance construction now runs post-
  Phase-4, after the cache's Phase-2 / Phase-3 effect.

## Non-Goals

- ClosureDropGlue Function Items: out of scope. The current
  fixture (`|x| x + 1`) has zero captures, so no DropGlue is
  synthesised. A future capture-bearing closure fixture would
  surface the analogous gap for ClosureDropGlue; that becomes
  its own task.
- VTable shim Function Instances: out of scope. The vtable end-
  to-end test (regseq-driver-50) confirms vtable shim Instances
  ARE constructed today (via the Phase-5 vtable-indirect rewrite
  pulling in shim Items); no gap to close there.
- Source-side closure call sites (e.g., a function that
  invokes the closure via `Fn::call`): out of scope. The bridge
  rejects `<closure as FnOnce<(u32,)>>::Output` (`TyKind::Alias`
  non-rigid) at `crates/regseq-driver/src/bridge/ty.rs:278`, so
  the fixture cannot legally invoke the closure via the `Fn`
  trait. The eager-construction path this task lands gives the
  Instance directly from synthesis, sidestepping the missing
  call-site path. Fixing the projection rejection (allowing the
  closure to be invoked) is a separate concern and outside this
  task.
- The `regseq-model-95` test FILE: this task re-baselines its
  snapshot (Change C) but does NOT change the test's structural
  assertions. If new Test Plan items are desired for the
  ClosureCall Instance-level coverage gap Codex called out, that
  is a regseq-model-95 spec amendment (separate task or
  in-task amendment by the user).
- Phase-1b backend C-emission of ClosureCall bodies: out of
  scope. This task lands the Instance + body in the model only.
- Coroutine / CoroutineClosure paths: explicitly rejected by
  the bridge per `spec.md:4568-4569`; no fixture covers them
  in phase-1a.

## Test Plan

**Test organization**: extend the existing bridge-side unit
tests in `crates/regseq-driver/tests/bridge_synth.rs` and the
end-to-end test in `crates/regseq-model/tests/e2e_synthesis.rs`.

### A. Eager Instance construction unit test

1. New unit test in `crates/regseq-driver/tests/bridge_synth.rs`
   that drives the bridge against the `closure` fixture (via
   the existing `with_bridge` helper or equivalent) and asserts:
   - For each `SyntheticItemKind::ClosureCall` entry in
     `model.item_synthetic_kinds`, there exists exactly one
     `Instance` in `model.instances` whose `item` field equals
     the corresponding `ItemId`.
   - The Instance's `kind` is `InstanceKind::Function { fn_sig, .. }`.
   - The Instance's `body.is_some()` (per spec.md:778-781).

### B. Per-`ClosureCallKind` receiver shape + closure inputs/output

2. For each constructed ClosureCall Instance (test A1's
   collected set), assert the FnSig's `inputs[0]` Ty matches
   the spec.md:4542-4546 rule:
   - `Call`     -> `Ty::Ref(<captures_ty>,
                             Provenance::SharedBorrow)`
   - `CallMut`  -> `Ty::Ref(<captures_ty>,
                             Provenance::UniqueBorrow)`
   - `CallOnce` -> `<captures_ty>` (no Ref wrapper)

   where `<captures_ty>` is the `Ty::Struct(ClosureCaptures
   Item, args, UNIT_LAYOUT)` for the zero-capture fixture's
   closure.

2b. For each constructed Instance, assert the FnSig's
    `inputs[1..]` matches the closure's argument types per
    `closure_sig_as_fn_ptr_ty.inputs[0]` (the tupled arg
    struct, untupled into flat inputs). For the `|x: u32|
    x + 1` fixture this is exactly `[Ty::Uint(UintKind::U32)]`.

2c. For each constructed Instance, assert the FnSig's `output`
    matches the closure's return type per
    `closure_sig_as_fn_ptr_ty.output`. For the fixture this
    is `Ty::Uint(UintKind::U32)`.

### C. FnSig invariants

3. Each ClosureCall Instance's FnSig has:
   - `abi: FnAbi::Rust`
   - `c_variadic: false`
   - `safety: Safety::Safe`
4. The `sig_c_abi_lowerings` side table entry for each
   ClosureCall FnSig is `None` (per the spec.md §3.3 step 5
   invariant: FnAbi::Rust signatures carry None lowering).

### D. Body materialisation

5. Each ClosureCall Instance's `body` is `Some(Body)`.
6. The materialised body's `basic_blocks` is non-empty (at
   least one block).
7. The materialised body's `local_decls[LocalId::ZERO]` is the
   return-place LocalDecl with type matching the FnSig's
   `output`.

### D'. Idempotency / no-duplicate Instances

7b. Reconstruct the bridge for the same closure fixture a
    second time (or trigger the synthesis path twice in one
    bridge run, e.g. by injecting a duplicate
    `closure_synth_cache` hit) and assert
    `model.instances` carries EXACTLY ONE Instance per
    `(ItemId, args_key, fn_shim_disambig)` triple for each
    ClosureCall Item. The lookup-table guard from
    `bridge/body/terminator.rs:513-534` MUST catch the
    duplicate request and return the existing Instance.

7c. The Instance count for any ClosureCall Item MUST remain
    stable across any subsequent bridge code path that could
    request a Direct Instance for the same Item. Verifiable
    indirectly: after the eager pass completes, assert
    `model.instances.iter().filter(|i| i.item == <closure_call_item>).count() == 1`
    for each ClosureCall Item; this collapses the eager
    Instance with any same-key Instance a Phase-5 body-pass
    consumer (e.g. a Call terminator referencing the closure
    call) would otherwise materialise via
    `lookup_or_create_direct_instance`. Direct call to that
    private helper is NOT required; the public-path
    observation through `model.instances` is sufficient.

### E. End-to-end on the closure fixture (regseq-model-95
re-baseline)

8. `cargo test -p regseq-model --test e2e_synthesis` --
   `closure_synthesis_and_struct_layout_view` still passes;
   the existing item-level assertions (Test Plan B17-E26 of
   regseq-model-95) all hold.
9. `cargo insta review` accepts the new snapshot at
   `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`.
10. The accepted snapshot includes (verifiable by inspection):
    - A `<regseq_synth>::...::call::...` Function Instance
      rendering.
    - The `ClosureCaptures` ADT-definition Instance rendering.
    - At least one inherited arm (`CallMut` or `CallOnce`).
    - The original `make_inc` + `core::mem::drop` renderings
      (preserved).
11. The comment block in
    `crates/regseq-model/tests/e2e_synthesis.rs` at the
    F-section is tightened to remove the "bridge gap" framing;
    the comment shrinks to a 2-3-line note that the snapshot
    captures the full ClosureCall + ClosureCaptures rendering.

### F. Workspace regression-clean

12. `cargo test --workspace` -- all existing tests still pass
    (in particular, no regression in the regseq-driver-50
    vtable end-to-end test, the regseq-model-94 diag_logging
    test, or the regseq-driver-40 closure unit tests).
13. `cargo fmt --all -- --check` -- clean.
14. `cargo clippy --workspace --tests -- -D warnings` -- no
    new lint sites beyond the pre-existing baseline (35 sites
    on `fixes/audit-2026-05-15` at the time of this task's
    registration).

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash tool
(per `SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-51 against its task
spec at
.harness/tasks/regseq-driver/51-eager-closure-call-instance-synthesis.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/synth/closure.rs
  (Phase-2 ClosureCall Item registration; unchanged by this
  task)
- crates/regseq-driver/src/bridge/mod.rs (the post-Phase-4 +
  pre-Phase-5 Instance construction call site -- a new helper
  invoked between construct_function_instances and
  run_phase5_body_passes per Change B1, OR the extended walker
  per Change B2)
- crates/regseq-driver/tests/bridge_synth.rs (new unit tests)
- crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap
  (re-baselined snapshot)
- crates/regseq-model/tests/e2e_synthesis.rs (tightened F-section
  comment)

Focus on:
- Phase placement: Instance construction MUST happen after
  Phase-4 ItemId assignment (mod.rs:1015-1027) and before
  run_phase5_body_passes. Phase-2 inline construction is
  architecturally wrong because Instance.item requires ItemId.
- The eager Instance construction MUST cover all
  ClosureCallKind arms produced by closure_call_kinds() per
  the per-ClosureKind enumeration at spec.md:4531-4535.
- Per-ClosureCallKind receiver shape MUST match spec.md:4542-4546
  exactly (Ref/SharedBorrow for Call, Ref/UniqueBorrow for
  CallMut, by-value for CallOnce).
- FnSig inputs[1..] MUST be the closure's untupled argument
  types per closure_sig_as_fn_ptr_ty; FnSig.output MUST be the
  closure's return type.
- FnSig MUST be FnAbi::Rust with c_variadic=false, safety=Safe.
- body is None at construction time; the existing Phase-5
  materialiser at materialise.rs:300-338 fills it in. Do NOT
  call materialise_function_body inline from construction.
- body.is_some() MUST hold after Phase-5 per spec.md:778-781.
- The fnsig::compute_cabi_lowering helper is NOT called for
  ClosureCall FnSigs (FnAbi::Rust), but the
  sig_c_abi_lowerings side table MUST stay dense (None entry
  per ClosureCall sig).
- Idempotency: the construction site MUST use the
  (ItemId, ArgsKey, FunctionInstanceFlavor) lookup guard from
  bridge/body/terminator.rs:513-534 so reruns and any
  subsequent lookup_or_create_direct_instance calls return the
  same Instance.
- Test Plan items A1, B2, B2b, B2c, C3, C4, D5, D6, D7, D'.7b,
  D'.7c, E8-E11, F12-F14 must all be present.
- The lazy-allocation path at lookup_or_create_direct_instance
  remains valid (call-site lazy allocation is not removed by
  this task; it becomes a no-op for already-constructed
  Instances).

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

Workspace-wide because this task spans `regseq-driver` (synth +
unit tests) + `regseq-model` (snapshot re-baseline + comment
tightening):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- A new helper invoked between `construct_function_instances`
  and `run_phase5_body_passes` at `bridge/mod.rs:1015-1027`
  (per Change B1) constructs one `Function` `Instance` per
  synthesised `ClosureCall` Item with the per-`ClosureCallKind`
  receiver shape from spec.md:4542-4546, FnSig inputs[1..] /
  output from the closure's `closure_sig_as_fn_ptr_ty`, and
  `body: None` for the existing Phase-5 materialiser
  (`materialise.rs:300-338`) to fill in.
- The Instance-level idempotency guard uses the lookup pattern
  at `bridge/body/terminator.rs:513-534` so reruns and any
  subsequent `lookup_or_create_direct_instance` calls return
  the same Instance.
- `crates/regseq-driver/tests/bridge_synth.rs` carries unit
  tests covering Test Plan A1, B2, B2b, B2c, C3, C4, D5, D6,
  D7, D'.7b, D'.7c.
- `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`
  re-baselined per Change C to include ClosureCall + ClosureCaptures
  renderings.
- `crates/regseq-model/tests/e2e_synthesis.rs`'s F-section
  comment block tightened (per Change C step 11).
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
