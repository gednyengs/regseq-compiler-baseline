# regseq-driver-56: Direct-call callee resolution for static-dispatch trait methods

## Description

Close the Phase-4/Phase-5 callee-canonicalization gap surfaced by manual
inspection of pretty-printed RegseqModel output for a fixture exercising
a trait + concrete `impl` + a top-level fn that invokes the trait method
on a concrete receiver, and independently confirmed by an adversarial
Codex review against `spec.md` and `pretty-printer.md`:
`crates/regseq-driver/src/bridge/body/terminator.rs::translate_call_callee`
(`terminator.rs:352-405`) interns a direct-call callee's `Item` from the
**raw** `mir::Operand::Constant`'s `TyKind::FnDef(def_id, generic_args)`
without consulting `ty::Instance::try_resolve(..)`. When the MIR
operand's `def_id` is a trait-method declaration's DefId (`tcx.trait_of_item(def_id) ==
Some(trait_def_id)`) and the resolved Instance is `InstanceKind::Item(impl_method_def_id)`,
the bridge produces a **second** Function Item and a body-less Function
Instance keyed on the trait-method DefId -- distinct from the
already-existing impl-method Function Item and Function Instance produced
by `construct_function_instances`
(`crates/regseq-driver/src/bridge/mod.rs:1236-1324`), the Phase-5 eager
pass that iterates the existing `ItemKind::Function` items previously
interned by the upstream Phase-2 `MonoItem::Fn` walk at
`crates/regseq-driver/src/bridge/mod.rs:657-703`.

Per `spec.md` §8.1 (`spec.md:2341-2350`):

> Trait impl methods lift the same way; the trait association
> survives only via DefPath ... Trait declarations are erased;
> static dispatch resolves directly to the implementing
> function's Instance, dynamic dispatch goes through a vtable.

Per `spec.md` §9.4 (`spec.md:2595-2606`) `Function.body` is `None` ONLY
for foreign declarations (`tcx.is_foreign_item(did) && !tcx.is_mir_available(did)`);
source-defined fns and bridge-synthesized shims always have `Some(b)`. Per
`pretty-printer.md` §7.6-§7.7 (`pretty-printer.md:1006-1027`) the rendered
direct-call callee comes from the **resolved Instance's** `item` +
`generic_args`, not the MIR operand's raw `TyKind::FnDef`.

The observed symptom on the playground fixture
(`tests/fixtures/playground/src/lib.rs:5-29`) is twofold:

1. `tests/fixtures/playground/regseq-out/model.txt:38` shows a phantom
   body-less Function Instance `fn playground::GAdd::add(_1, _2) -> Point;`
   -- a trait DECLARATION DefId interned as `ItemKind::Function`. This
   violates §8.1 (trait declarations are erased) and §9.4 (`body: None`
   reserved for foreign declarations) simultaneously.

2. `tests/fixtures/playground/regseq-out/model.txt:51` shows
   `playground::GAdd::add<playground::Point>(...)` at the call site
   inside `pub fn add`, instead of the resolved
   `playground::{impl#1}::add(...)`. The legitimate impl-method
   Function Instance at `model.txt:14-36` exists with full body but is
   unreachable in the model's call graph.

After this task lands:

- Every direct call whose resolved `ty::InstanceKind` is
  `Item(resolved_def_id)` keys its callee `Item` + `Instance` on
  `(resolved_def_id, instance.args)`, not on the MIR operand's
  pre-resolution `(def_id, generic_args)`.
- Direct calls whose resolved `ty::InstanceKind` is `Virtual(trait_def_id, idx)`
  continue to key on the trait method DefId so the stage-11 vtable-indirect
  rewrite (`spec.md` §23.6.2 B-8) can look up the vtable slot.
- Direct calls whose resolved `ty::InstanceKind` is any shim variant
  (`ReifyShim`, `FnPtrShim`, `ClosureOnceShim`, etc.) continue to key
  on the ORIGINAL MIR operand's `(def_id, generic_args)` -- i.e., the
  bridge's current pre-fix behavior is preserved for shims. The
  flavor field is set per `function_instance_flavor_for_kind`
  (`mod.rs:1925-1948`) so the resolved `InstanceKind` survives as the
  Function Instance's stable identity tag per §9.4, without rekeying
  the Item.
- The playground fixture (and the new in-tree fixture mirroring its
  shape) round-trips through `cargo regseq` with exactly ONE
  Function Item + ONE Function Instance per concrete trait-impl method,
  and the call site renders as the resolved impl DefPath.

Foreign declarations and RPIT-uncalled-fn coverage are explicitly out
of scope: this task is a callee-canonicalization fix at one call-site
boundary, not a validator surface change.

## Input Context

- `docs/discussions/regseq-model/spec.md` §8.1 impl-lifting rules
  (`spec.md:2341-2361`) -- trait declarations erased, static dispatch
  resolves directly to the implementing function's Instance, six-variant
  `ItemKind` alphabet with no `Trait` variant.
- `docs/discussions/regseq-model/spec.md` §9.4 `Function` variant
  (`spec.md:2595-2618`) -- `body: None` reserved for foreign declarations;
  source-defined fns and shims always have `Some(b)`. `flavor` records
  the rustc `InstanceKind` variant; `virtual_idx = Some(idx)` exactly
  when `flavor == Virtual`.
- `docs/discussions/regseq-model/spec.md` §3.4 Phase 5
  (`spec.md:1226-1229`) -- "one Instance per (Item, GenericArgs, rustc
  shim flavor) tuple". The current bug violates this by producing two
  Instances (one keyed on trait DefId, one keyed on impl DefId) for the
  same monomorphized function.
- `docs/discussions/regseq-model/pretty-printer.md` §7.6-§7.7
  (`pretty-printer.md:1006-1027`) -- Direct-call rendering reflects the
  resolved Instance's `item` + `generic_args`.
- `crates/regseq-driver/src/bridge/body/terminator.rs::translate_call_callee`
  (`terminator.rs:352-405`) -- current implementation. Line 368 calls
  `translate_ty(constant.const_.ty())` on the raw operand Ty; line 369
  extracts `(callee_item, model_args)` from the resulting `Ty::FnDef`;
  line 394 calls `resolve_callsite_flavor` but uses its return only
  for `(flavor, virtual_idx)`, never to rewrite `callee_item`/`model_args`.
- `crates/regseq-driver/src/bridge/body/terminator.rs::resolve_callsite_flavor`
  (`terminator.rs:465-479`) -- already computes
  `ty::Instance::try_resolve(tcx, fully_monomorphized, def_id, generic_args)`.
  Discriminates `Virtual(_, idx)` separately. For `Item` and other
  shim flavors, returns `function_instance_flavor_for_kind(instance.def)`
  but DROPS the resolved `(instance.def.def_id(), instance.args)` tuple.
- `crates/regseq-driver/src/bridge/ty.rs::translate_ty` `TyKind::FnDef`
  arm (`ty.rs:128-133`) -- bare `TyKind::FnDef` interning, called from
  `translate_call_callee` line 368 and from local-decl Ty translation.
  This arm is the **upstream interning site** that creates the phantom
  trait-method `Item`. Per the fix design (see Implementation Notes),
  this arm stays unchanged; the fix is localized to the call-site path.
- `crates/regseq-driver/src/bridge/mod.rs::function_instance_flavor_for_kind`
  (`mod.rs:1925-1948`) -- the 15-variant flavor mapping over rustc's
  `InstanceKind` (`Item`, `Intrinsic`, `VTableShim`, `ReifyShim`,
  `FnPtrShim`, `Virtual`, `ClosureOnceShim`,
  `ConstructCoroutineInClosureShim`, `ThreadLocalShim`,
  `FutureDropPollShim`, `DropGlue`, `CloneShim`, `FnPtrAddrShim`,
  `AsyncDropGlueCtorShim`, `AsyncDropGlue`).
- `crates/regseq-driver/src/bridge/mod.rs::construct_function_instances`
  (`mod.rs:1236-1324`) -- Phase-5 eager pass that creates the
  impl-method Function Instance for each pre-existing
  `ItemKind::Function` Item. The upstream Phase-2 `MonoItem::Fn` walk
  at `mod.rs:657-703` is what interns the impl-method Item itself
  (resolved by rustc to
  `MonoItem::Fn(Instance(Item(impl_method_def_id), [Point]))` in the
  playground case). After the fix, the call-site path will dedupe
  against this pre-existing Function Instance via the
  `function_instance_for_item` table key
  `(impl_item_id, args_key, FunctionInstanceFlavor::Item)`.
- `tests/fixtures/playground/src/lib.rs` -- the manual reproduction case
  (not modified by this task; playground is the user's experimentation
  space).
- `tests/fixtures/playground/regseq-out/model.txt` -- the current
  observed output exhibiting the bug. After the fix, slot 3
  (`fn playground::GAdd::add(...);`) disappears and slot 4's call site
  renders `playground::{impl#1}::add(...)` instead of
  `playground::GAdd::add<playground::Point>(...)`.

## Implementation Notes

### Change A: callee-resolution gate in `translate_call_callee`

**Anchor:** `crates/regseq-driver/src/bridge/body/terminator.rs:352-405`.

Replace the body of `translate_call_callee`'s `Operand::Constant` +
`TyKind::FnDef` arm so that:

1. `reject_call_site_intrinsics(cx, *def_id)?;` runs FIRST (unchanged --
   spec §15.1 rejection must precede any Item interning per
   `terminator.rs:362-366` comment).

2. `ty::Instance::try_resolve(cx.input.tcx, ty::TypingEnv::fully_monomorphized(), *def_id, generic_args)`
   runs SECOND, BEFORE any `translate_ty(constant.const_.ty())` call on
   the callee. This canonicalizes the callee identity.

3. Branch on the resolved Instance:
   - `Ok(Some(instance))` with `instance.def == ty::InstanceKind::Virtual(_, idx)`:
     intern Item from the ORIGINAL `*def_id` (the trait method DefId)
     and translate `generic_args` ORIGINAL. Flavor =
     `FunctionInstanceFlavor::Virtual`, `virtual_idx = Some(idx as u32)`.
     This preserves the §23.6.2 B-8 vtable-indirect contract.
   - `Ok(Some(instance))` with `instance.def == ty::InstanceKind::Item(_)`:
     intern Item from `instance.def.def_id()` and translate
     `instance.args` (NOT the original `generic_args`). Flavor =
     `FunctionInstanceFlavor::Item`, `virtual_idx = None`. This is the
     case that fixes the bug.
   - `Ok(Some(instance))` with any other `instance.def` (shim variants
     `Intrinsic` / `VTableShim` / `ReifyShim` / `FnPtrShim` /
     `ClosureOnceShim` / `ConstructCoroutineInClosureShim` /
     `ThreadLocalShim` / `FutureDropPollShim` / `DropGlue` /
     `CloneShim` / `FnPtrAddrShim` / `AsyncDropGlueCtorShim` /
     `AsyncDropGlue`): intern Item from the ORIGINAL `*def_id` and
     translate `generic_args` ORIGINAL. Flavor =
     `function_instance_flavor_for_kind(instance.def)`,
     `virtual_idx = None`. The existing (pre-fix) behavior already
     handles these correctly via the pre-resolution `(def_id, args)`
     because rustc's MIR keeps the shim's user-facing DefId at the
     call site; the shim flavor is recovered for stable identity per
     §9.4. This task does NOT change the shim path's normative shape;
     it only adds the `Item`-variant resolution as a new normative
     contract. Test coverage for shim paths is OUT OF SCOPE for this
     task (see Non-Goals).
   - `Ok(None)` or `Err(_)`: return
     `BridgeError::Phase4(format!("call-site Instance::try_resolve
     failed for {def_id:?} with args {generic_args:?}"))`. The bridge
     operates exclusively in `TypingEnv::fully_monomorphized` per
     `terminator.rs:470`; in that context `Instance::try_resolve` never
     legitimately fails for a `TyKind::FnDef` operand of a successfully
     monomorphized body, so this is a hard error, not a defensive
     pass-through. The toolchain is pinned (rule
     `feedback_no_defensive_fallbacks`); a `None`/`Err` here is a
     rustc-invariant violation worth surfacing.

4. After Item interning + GenericArgs translation, synthesize the
   `Ty::FnDef(callee_item, model_args)` view via
   `cx.model.types.intern(...)` so the rest of `translate_call_callee`
   sees a `Ty::FnDef` whose `Item` is the canonical (resolved) Item.
   This replaces the current `translate_ty(constant.const_.ty())` call
   for the callee Ty.

5. `lookup_item_def_path(callee_item)` (used at `terminator.rs:376-379`
   for `diag_log::recognise_log_intrinsic_def_id`) is fed the
   resolved-or-original Item. The diag-log recognition logic compares
   against intrinsic-shaped DefPaths and is unaffected by the swap --
   none of the intrinsic DefPaths is a trait method.

6. `lookup_or_create_direct_instance(cx, callee_item, model_args, flavor,
   virtual_idx, fn_sig_fn)` (currently at `terminator.rs:397-404`) is
   called with the canonical inputs. The eager
   `construct_function_instances` (`mod.rs:1236-1324`) has already
   created the impl-method Function Instance keyed on the resolved
   `(impl_item, args_key, FunctionInstanceFlavor::Item)`; the call site
   path now finds and reuses that Instance instead of synthesizing a
   second one keyed on the trait method DefId.

7. `callsite_fn_sig` (`terminator.rs:482-493`) takes `*def_id` and
   `*generic_args` from a captured closure. After the fix it must take
   the RESOLVED `(def_id, generic_args)` so the FnSig matches the
   resolved Instance. Update the captured copies.

**Out of scope for Change A:** the upstream `translate_ty` `TyKind::FnDef`
arm at `ty.rs:128-133`. That arm is reached not only from
`translate_call_callee` line 368 (which Change A removes) but also from
local-decl Ty translation and rvalue translation when a `FnDef` Ty
appears as a value (e.g., `let f = <Point as GAdd>::add;`). Those
non-call sites can continue to intern the original DefId; they produce
a `Ty::FnDef` value whose Item happens to be the trait method DefId, but
they do NOT produce a Function Instance and they do NOT create a phantom
body-less rendering -- because the rendered output iterates Instances,
not Items (`pretty-printer.md` §10.3). A future task can canonicalize
the bare-Ty arm if a use-case emerges (see Non-Goals).

### Change B: spec amendment recording the call-site resolution contract

**Anchor:** `docs/discussions/regseq-model/spec.md` §8.1
(`spec.md:2348-2350`).

Add a normative paragraph (or footnote / cross-reference depending on
spec author preference) immediately after `spec.md:2350` clarifying:

> Implementation note (call-site canonicalization): at a direct call
> site whose MIR operand is `Operand::Constant(FnDef(def_id, generic_args))`,
> the bridge resolves `ty::Instance::try_resolve(.., fully_monomorphized,
> def_id, generic_args)` BEFORE interning the callee Item, and uses the
> resolved Instance's identity for the `InstanceKind::Item(_)` and
> `InstanceKind::Virtual(_, _)` arms exclusively:
>
> - `InstanceKind::Item(resolved_def_id)`: callee Item is keyed on
>   `(resolved_def_id, instance.args)`. This is the static-dispatch
>   canonicalization that erases trait declaration DefIds per §8.1.
> - `InstanceKind::Virtual(trait_def_id, idx)`: callee Item stays keyed
>   on the trait method DefId + original `generic_args`; the resulting
>   Function Instance carries `flavor = Virtual` and `virtual_idx =
>   Some(idx)` so the stage-11 vtable-indirect rewrite (§23.6.2 B-8)
>   can look up the vtable slot.
>
> All other rustc `InstanceKind` shim variants continue to key on the
> MIR operand's pre-resolution `(def_id, generic_args)` per the current
> bridge behavior; that shim path is not part of this normative
> contract (`function_instance_flavor_for_kind` records flavor for
> stable identity per §9.4).

The amendment is normative for the Item + Virtual arms because those
two arms together determine whether trait-declaration DefIds leak into
`model.items` and `model.instances` as phantom Function Instances.
Without it, two bridge implementations could produce inequivalent
Instance graphs for the same static-dispatch source.

### Change C: harness expectations entry

**Anchor:** `.harness/expectations/regseq-driver-56.md` (new file).

Record:

- Before-fix evidence: cite `tests/fixtures/playground/regseq-out/model.txt:38`
  (phantom body-less Function Instance) and
  `tests/fixtures/playground/regseq-out/model.txt:51` (call site
  rendering trait DefPath). Make explicit that the playground fixture
  is the user's experimentation space and the expectation file
  references but does NOT modify it.
- After-fix evidence: list the exact slot enumeration expected from
  the playground fixture (3 Function Instances + 1 Struct Instance --
  the phantom body-less `fn playground::GAdd::add(...);` disappears,
  leaving `{impl#0}::new`, `{impl#1}::add`, and `playground::add`; the
  call site inside `playground::add` renders
  `playground::{impl#1}::add(...)`).
- Test-fixture evidence: the new `test-ir-fixtures` fixture mirrors the
  playground's trait + impl + caller shape and is the normative
  regression guard.

## Non-Goals

- Do not modify the `TyKind::FnDef` arm of `translate_ty`
  (`ty.rs:128-133`). The bare-Ty interning of `FnDef` values can be
  canonicalized in a follow-up task if a non-call use-site emerges.
- Do not modify `tests/fixtures/playground/`. Playground is the user's
  experimentation space; verifying the fix against it is a manual
  step in the Quality Gate, not an automated assertion.
- Do not re-tighten `validate_bridge_completeness`'s `Function` carve-out
  (`crates/regseq-model/src/model.rs:3442-3486`). Re-introducing
  blanket Function coverage would resurrect the
  regseq-driver-55-v2 RPIT-uncalled-fn blocker (the RPIT fixture
  `pub fn closure_fixture() -> impl Fn(u32) -> u32` defined at
  `crates/regseq-driver/tests/bridge_synth.rs:399` has no Function
  Instance for a legitimate spec reason).
- Do not address foreign declarations (`extern "C"`). The surface
  remains SPEC-AMBIGUOUS per the regseq-driver-55 Description.
- Do not address dyn-Trait call sites. `resolve_callsite_flavor`
  (`terminator.rs:471-475`) already returns
  `(FunctionInstanceFlavor::Virtual, Some(idx as u32))` for
  `InstanceKind::Virtual(_, idx)`, and the resulting trait DefId
  keying happens at `terminator.rs:397-404` +
  `terminator.rs:526-529`; the fix preserves that path unchanged.
- Do not address closure-call shim direct callees
  (`terminator.rs:320`). That call site already passes
  `FunctionInstanceFlavor::Item` with a canonical synth Item; it does
  not exhibit the bug.

## Test Plan

All tests in `crates/regseq-driver/tests/` or `crates/test-ir-fixtures/`.
Before-fix expectation: B1 and B2 fail; B3 passes (Virtual path
unaffected); B4 baseline passes; B5 fails (golden mismatch on the
call-site rendering). After-fix expectation: B1, B2, B3, B4, and B5
all pass.

### B1: bridge produces exactly ONE Function Item + ONE Function Instance per static-dispatch trait-method call

**New file:** `crates/regseq-driver/tests/bridge_trait_method_resolution.rs`.

Build a fixture defining a trait with one method, a concrete impl, and
a top-level fn that invokes the trait method on the concrete type:

```rust
trait T { fn m(&self) -> u32; }
struct S;
impl T for S { fn m(&self) -> u32 { 42 } }
pub fn caller(s: &S) -> u32 { s.m() }
```

Run the bridge end-to-end and assert:

- Exactly TWO source-derived Function Items: one for the impl method
  `<S as T>::m` (DefPath segment `{impl#0}::m`), one for `caller`.
  No Function Item is a trait DECLARATION: for every source-derived
  Function Item in the model, if `tcx.trait_of_item(item.def_id)` is
  `Some(_)` (meaning the item has a trait association) then
  `tcx.impl_of_method(item.def_id)` must also be `Some(_)` (meaning
  the item lives in an `impl Trait for Type` block, not in the trait
  declaration). Per `spec.md:2341-2344` the trait association survives
  via DefPath after impl-lifting, so `trait_of_item == Some(_)` is
  expected for the impl method; the distinguishing rejection predicate
  is `trait_of_item == Some(_) && impl_of_method == None` (trait
  declaration).
- Exactly TWO Function Instances (one per Function Item). Zero
  Function Instances with `body == None` for source-defined fns.
- The `caller` Instance's body contains a `Terminator::Call` whose
  callee is `CallCallee::Direct(id)` where `model.instances[id].item`
  resolves to the impl-method Item (NOT the trait method DefId).

### B2: pretty-printer renders the resolved DefPath at the call site

**Same file as B1.**

After running the bridge on the B1 fixture, format the model via
`PrettyPrinter::render` and assert:

- The rendered `caller` body contains the substring
  `{impl#0}::m(` (or whatever DefPath `tcx.def_path_str` produces for the
  impl method).
- The rendered output contains NO body-less Function declarations of
  the form `fn <crate>::T::m(...);` (i.e., no semicolon-terminated
  function whose DefPath traces to the trait declaration).

### B3: dyn-Trait call site still keys on trait DefId + virtual_idx (regression guard)

**Anchor:** the existing dyn-Trait Virtual assertion at
`crates/regseq-driver/tests/bridge_body_passes.rs:607-650` already
verifies that the pre-rewrite Direct callee carries
`flavor == FunctionInstanceFlavor::Virtual` and
`virtual_idx == Some(_)` keyed on the trait method's `DefPathHash`. B3
is satisfied by confirming this test continues to pass after Change A
lands; if Change A inadvertently rewrites the Virtual path to use the
resolved impl DefId, that test will fail (the `DefPathHash` comparison
at `bridge_body_passes.rs:644-645` looks up the trait method's hash
specifically). This guards the §23.6.2 B-8 contract preserved by
Change A point 3 first sub-bullet. No new test file is required.

### B4: existing bridge-pipeline + Phase-5 coverage tests stay green

Run the full workspace test suite. Specifically watch:

- `crates/regseq-driver/tests/bridge_pipeline.rs` -- assert no
  Function-Instance count regressions (the trait-method-call fixture
  in `bridge_pipeline.rs` if any, plus `no_op_fixture`).
- `crates/regseq-model/tests/validate_item_instance_coverage.rs` --
  the regseq-driver-55 validator tests stay green (Change A does not
  touch the validator).
- The RPIT-uncalled-fn carve-out: the RPIT fixture defined at
  `crates/regseq-driver/tests/bridge_synth.rs:399` (`pub fn
  closure_fixture() -> impl Fn(u32) -> u32`) must continue to compile
  without triggering `validate_bridge_completeness` (Change A does not
  touch the Function carve-out in
  `crates/regseq-model/src/model.rs:3442-3486`).
- `crates/test-ir-fixtures/fixtures/trait_object.rs`-driven tests --
  the dyn-Trait Virtual path stays intact.

### B5: end-to-end via `cargo regseq` on a `test-ir-fixtures` source

Add a new fixture `crates/test-ir-fixtures/fixtures/trait_method_resolution.rs`
mirroring the B1 shape and a matching e2e test that runs the bridge
through the cargo subcommand end-to-end (parallel to the
regseq-driver-55 e2e test at the same crate). Assert the rendered
`model.txt` matches a checked-in golden.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/56-direct-call-callee-static-dispatch-resolution.md`).
- `docs/discussions/regseq-model/spec.md` §8.1 +new Change B paragraph.

NOTE: `.harness/expectations/regseq-driver-56.md` is an implementation
deliverable produced by the implementer alongside the code change (see
Change C); it is NOT a pre-review input. Subsequent reviews of the
implementation PR will include it.

### Reviewer focus

- **Root cause framing.** Does the Description correctly anchor the
  bug at `translate_call_callee` (`terminator.rs:352-405`) and identify
  that `resolve_callsite_flavor` (`terminator.rs:465-479`) already
  computes the resolved Instance but drops the resolved DefId/args?
  CITATION CHECK: every claim in Description and Implementation Notes
  must trace to a real file:line.
- **Spec contract precision.** Does Change B's amendment accurately
  capture the normative split (`Item` -> resolved `(instance.def.def_id(),
  instance.args)`; `Virtual` -> original trait DefId + original
  `generic_args`)? Cross-check against `function_instance_flavor_for_kind`'s
  15-variant mapping (`mod.rs:1925-1948`); verify that all other shim
  variants are explicitly OUT of the normative contract and continue
  to use the bridge's current pre-fix behavior (key on the MIR
  operand's original `(def_id, generic_args)`, flavor via
  `function_instance_flavor_for_kind`).
- **Test Plan coverage.** Does B1+B2 actually catch the
  symptom on `model.txt:38` + `model.txt:51`? Does B3 actually exercise
  the Virtual path with a real `dyn T` receiver (not just a fixture
  declaration)? Does B5's e2e golden capture the rendered call-site
  shape (not just the Function-Instance count)?
- **Regression surface.** Does B4 enumerate every test that exercises
  trait-method direct calls or dyn-Trait virtual calls? Flag missing
  fixtures.
- **Non-Goal completeness.** Does the Non-Goals section explicitly
  block: (a) the bare-Ty `TyKind::FnDef` arm at `ty.rs:128-133`, (b)
  re-tightening the `Function` carve-out in
  `validate_bridge_completeness`, (c) foreign-decl scope, (d)
  closure-call shim direct callees? Each must be a separate bullet so
  scope creep is unambiguous.
- **Scope-creep check.** Per root `CLAUDE.md` "No features beyond what
  was asked" (`CLAUDE.md:19-23`) and "Touch only what you must"
  (`CLAUDE.md:30-44`): does Change A introduce any unrequested
  abstraction? In particular, does it avoid adding new methods to
  `BridgeCtxt` or new InstanceKind variants?
- **No defensive fallbacks.** Per memory rule `feedback_no_defensive_fallbacks`:
  is the `Ok(None) | Err(_)` branch at Change A point 3 fourth
  sub-bullet a hard `BridgeError::Phase4` (correct per the
  no-defensive-fallbacks rule) or a silent pass-through (escalate)?
  AC5 binds the hard-error outcome.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/56-direct-call-callee-static-dispatch-resolution.md
plus the proposed spec.md §8.1 amendment (Change B). Identify:
- Citation drift (any file:line that does not anchor the claim).
- Missing test coverage for the `Item` / `Virtual` call-site
  resolution contract (shim variants are explicitly out of the
  normative contract; current pre-fix behavior preserved).
- Implementation composability against `lookup_or_create_direct_instance`.
- Scope creep beyond the minimal call-site canonicalization fix.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C bullets with file:line
CITATION DRIFT: list or "none"
SCOPE-CREEP CHECK: pass/fail with citation
TEST PLAN VERIFICATION: B1, B2, B3, B4, B5 bullets
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
- `cargo test --workspace` green, including:
  - B1+B2 new tests in `crates/regseq-driver/tests/bridge_trait_method_resolution.rs`.
  - B3 dyn-Trait regression assertion.
  - B5 e2e golden via `test-ir-fixtures`.
- `crates/regseq-driver` (call-site resolution gate + diagnostic preservation)
  + `regseq-model` (no changes expected) + `test-ir-fixtures` (new fixture +
  matching driver-side e2e test) all build clean.
- Manual verification on `tests/fixtures/playground/`: rebuild
  `cargo build -p regseq-driver -p cargo-regseq`, recompile the
  playground via `cargo regseq --target-os linux --platform-file
  platform.toml --emit model`, and inspect `regseq-out/model.txt`:
  - Slot 3 (`fn playground::GAdd::add(...);`) disappears.
  - Slot 4's call site renders
    `playground::{impl#1}::add(move _3, move _4)` (or the equivalent
    canonical DefPath produced by `tcx.def_path_str`).
  - Total Function Instance count drops by 1 (3 instead of 4).
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.
  Pass-rate to merge: zero unaddressed BLOCKERs across review passes.

## Acceptance Criteria

1. `translate_call_callee` (`crates/regseq-driver/src/bridge/body/terminator.rs:352-405`)
   resolves the MIR operand's `(def_id, generic_args)` via
   `ty::Instance::try_resolve(.., fully_monomorphized, .., ..)` BEFORE
   interning the callee Item, and uses the resolved
   `(instance.def.def_id(), instance.args)` to key the Item for the
   `InstanceKind::Item(_)` case, while preserving the original trait
   DefId for `InstanceKind::Virtual(_, _)`.
2. `callsite_fn_sig` (`crates/regseq-driver/src/bridge/body/terminator.rs:482-493`)
   takes the RESOLVED `(def_id, generic_args)` so its `tcx.fn_sig(def_id)
   .instantiate(tcx, generic_args)` matches the resolved Instance.
3. No new Function Item is created for any DefId where
   `tcx.trait_of_item(def_id) == Some(trait_def_id)` AND
   `tcx.impl_of_method(def_id) == None` (i.e., trait DECLARATION DefIds
   are not interned as `ItemKind::Function`) in the static-dispatch
   path. The Virtual-dispatch path may continue to intern trait DefIds.
4. No `Function` Instance with `body == None` exists for a source-defined
   fn after the fix (i.e., the `body: None` carve-out is exclusively
   reached via the foreign-decl Phase-2 path, which remains
   SPEC-AMBIGUOUS and out of scope).
5. `translate_call_callee` returns
   `Err(BridgeError::Phase4(_))` (not a silent pass-through) when
   `ty::Instance::try_resolve(.., fully_monomorphized, def_id, generic_args)`
   returns `Ok(None)` or `Err(_)`. Per Change A point 3 fourth
   sub-bullet, this branch is not expected to fire for valid bridge
   input but must surface as a hard error per the
   no-defensive-fallbacks rule.
6. Shim-flavor direct callees (any `InstanceKind` other than `Item`
   or `Virtual`) continue to key the callee Item on the ORIGINAL MIR
   operand's `(def_id, generic_args)` (current pre-fix behavior
   preserved); flavor is set via
   `function_instance_flavor_for_kind(instance.def)` per `mod.rs:1925-1948`.
7. New B1+B2 tests in `crates/regseq-driver/tests/bridge_trait_method_resolution.rs`
   exercise the static-dispatch fixture; B3 exercises the Virtual
   regression guard; B5 e2e exercises the rendered call-site shape
   via a `test-ir-fixtures` fixture and matching driver-side integration
   test.
8. The full workspace test suite (`cargo test --workspace`) passes; no
   regression in `bridge_pipeline.rs`, `validate_item_instance_coverage.rs`,
   `bridge_synth.rs::closure_fixture`, or the dyn-Trait coverage in
   `bridge_synth.rs` / `bridge_dyn_trait_indirect.rs` (whichever owns
   it -- the implementer confirms the exact file during execution).
9. Manual playground verification (Quality Gate fourth bullet) passes:
   the phantom `fn playground::GAdd::add(...);` slot is gone and the
   `pub fn add` body's call site renders the resolved impl DefPath.
10. Spec amendment (Change B) lands in
   `docs/discussions/regseq-model/spec.md` §8.1 capturing the
   `Item` / `Virtual` call-site resolution contract as the normative
   pair, with all other rustc `InstanceKind` shim variants
   explicitly out of the new normative contract (current bridge
   behavior preserved: original MIR `(def_id, generic_args)` keying,
   flavor recorded via `function_instance_flavor_for_kind`).
11. Harness expectations file `.harness/expectations/regseq-driver-56.md`
   (Change C) records before/after evidence with file:line citations.
