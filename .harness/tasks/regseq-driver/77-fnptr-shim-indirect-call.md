# regseq-driver-77 — fn pointer called through an `Fn`/`FnMut`/`FnOnce` bound (FnPtrShim → indirect call)

## 1. Description

A bare `fn` item coerced to a function pointer and passed to a generic
`F: Fn/FnMut/FnOnce` bound, then invoked, is rejected by the bridge:

```rust
#![no_std]
fn g() -> u32 { 7 }
fn call_it<F: Fn() -> u32>(f: F) -> u32 { f() }
pub fn f() -> u32 { call_it(g as fn() -> u32) }
```

At HEAD this returns `BridgeOutcome::Rejected(UnsupportedAbi { abi_repr: "RustCall" })`.

Root cause: `f()` inside `call_it` (monomorphised with `F = fn() -> u32`) lowers to
`<fn()->u32 as Fn<()>>::call(move _recv, const ())`, which resolves to
`InstanceKind::FnPtrShim`. `canonicalize_call_site_callee` keeps `callee_def_id =
Fn::call`, so `translate_call_callee` takes the Direct path → `callsite_fn_sig` →
`translate_fn_sig` on the `extern "rust-call"` `Fn::call` signature →
`translate_abi(RustCall)` → `UnsupportedAbi` (`bridge/fnsig.rs:107-115`).

Fix (Option A — codegen-faithful): lower the fn-pointer `Fn*::call` to an
**indirect call** through the (deref'd) fn-pointer value — a fn-pointer call IS
an indirect call — reusing the existing `CallCallee::Indirect` path (direct
fn-pointer calls already bridge `Ok` end-to-end). The `FnPtrShim` flavor is never
materialised, exactly as the closure `Fn::call` family is covered by the
`ClosureCall` rewrite rather than a raw closure Item.

Plan (probe- + spike-grounded, Codex-APPROVED across 2 rounds):
`.harness/audits/2026-05-24-ff005-fnptrshim-rustcall-plan.md`. A reverted
end-to-end spike confirmed all repros bridge `Ok`, Mode A 145/145/0, no
regression. Covers **FF-005** (`FunctionInstanceFlavor::FnPtrShim`),
reclassified covered-by-rewrite (predicate `CallCallee = Indirect`).

## 2. Input Context

### Design source

- `.harness/audits/2026-05-24-ff005-fnptrshim-rustcall-plan.md` — Option A,
  3-site scope, architect-confirmed fork + reclassification. Codex round 1
  (NOT_READY: pre-scan orphan, no-op drop, deref-first) → round 2 (APPROVED:
  3-site scope complete, Goto no-op-drop safe, deref callee validates — Codex
  added + passed a temp unit test with the exact callee shape).

### Call-site MIR shapes (rustc `-Zunpretty=mir`, verified)

```
Fn::call:        _2 = &_1;      <F as Fn<()>>::call(move _2, const ())          ; receiver &F
FnMut::call_mut: _2 = &mut _1;  <F as FnMut<()>>::call_mut(move _2, const ())   ; receiver &mut F
FnOnce::call_once:              <F as FnOnce<()>>::call_once(copy _1, const ()) ; receiver F by value
```

`Fn`/`FnMut` bodies also emit `drop(_1)` of the `F = fn()` local after the call
(a no-op for the Copy fn pointer).

### Bridge code anchors (consumed/modified)

- `crates/regseq-driver/src/bridge/body/terminator.rs:140-176` — `translate_call`;
  `rewrite_closure_call_shim` (`:157`) returns `Option<(CallCallee, Vec<Operand>)>`.
- `:184-353` — `rewrite_closure_call_shim`; receiver gate `:204-207`
  (`receiver_ty = method_args.type_at(0)`); call_kind `:209-214`; args-untupling
  `:252-352` (incl. the driver-75 `Operand::Constant` arm). **Change A site.**
- `:642-740` — `translate_call_callee`; the Direct path → `callsite_fn_sig`
  (`:719`, the reject). `CallCallee::Indirect` exists `:724-737`.
- `:856-981` — `translate_drop_as_call`; `place_ty` `:867`, unconditional
  drop-glue synthesis `:891`/`:927`, **no `needs_drop` guard**. **Change C site.**
- `crates/regseq-driver/src/bridge/mod.rs:1213-1290` —
  `scan_terminator_for_synthesis` (Phase-2 pre-scan): `:1237` canonicalize,
  `:1246` `translate_ty(callee_ty)` interns the callee Item; arg scans `:1271-1273`;
  Drop `needs_drop` skip `:1279`. **Change B site.**
- `crates/regseq-driver/src/bridge/mod.rs:2639-2680` —
  `canonicalize_call_site_callee`; `FnPtrShim` → `other` arm →
  `function_instance_flavor_for_kind` (`:2660-2664`) → the concrete
  `RK::FnPtrShim(_, _) => FunctionInstanceFlavor::FnPtrShim` mapping at
  `mod.rs:2694`; `callee_def_id = Fn::call`. `CanonicalCallSiteCallee.flavor:
  FunctionInstanceFlavor` (`:2608-2622`).
- `crates/regseq-model/src/place.rs:58-60` + `place_validator.rs:326-329` —
  `PlaceElem::Deref` MUST be the first projection element.
- `crates/regseq-model/src/place_validator.rs:300` + `model.rs:2986` — Indirect
  callee validation: operand validated; recovered callee type must be `Ty::FnPtr`.
- `crates/regseq-model/src/body_ty.rs:80` — `Deref` narrows `Ref(inner)`/`Ptr(inner)`
  to `inner`.
- `crates/bridge-coverage-runner/src/inspect.rs:90-91,417-422,948-960` —
  `InspectionPredicate::CallCallee(String)`, tag `"Indirect"`.

### Prior tasks

- regseq-driver-70 (A1/F70) — escalated FF-005 (RustCall ABI), now this task.
- regseq-driver-37/62/71/72/75 — the closure-call rewrite + args-untupling
  (incl. driver-75's `Operand::Constant` arm) this change extends.

### Test surface

- New: `crates/regseq-driver/tests/bridge_fnptr_shim_indirect_call.rs`.
- New sidecar row `FF-005` + `snippets/FF-005.rs`; matrix `FF-005:408` flip.
- Mode A over the full corpus.

## 3. Implementation Notes

### Change A — fn-pointer branch in the call rewrite (`terminator.rs`)

Move the `call_kind` computation (`:209-214`) above the closure-only receiver
gate (`:204-207`). When `receiver_ty.kind()` is `ty::TyKind::FnPtr(..)`, delegate
to a new sibling `rewrite_fn_ptr_call_shim` (recommended over renaming
`rewrite_closure_call_shim`, for a smaller diff) returning
`Some((CallCallee::Indirect(callee), untupled_args))`:

- **Callee value (the indirect target):**
  - `ClosureCallKind::CallOnce` (by-value receiver): the translated `args[0]`
    operand directly.
  - `ClosureCallKind::Call` / `CallMut` (`&F`/`&mut F` receiver): deref. Extract
    the receiver place from the translated `args[0]` (`Operand::Move|Copy(place)`).
    Because `PlaceElem::Deref` must be first (`place.rs:58`), build
    `Operand::Copy(Place { local: place.local, projection: Box::new([PlaceElem::Deref]) })`
    **only when `place.projection.is_empty()`** (the observed receiver is a fresh
    temp); otherwise reject with a typed `DslRejectionKind::Terminator` (the
    spike confirmed the temp always has an empty projection; a non-empty
    projection is unexpected, not a silent fall-through).
- **Args:** untuple `args[1]` into positional operands with **no receiver
  prefix** (the receiver became the callee), reusing the `Move`/`Copy` field
  projection and the empty-`const ()` zero-arg case from the closure path. The
  match over `&tuple_arg.node` MUST be exhaustive — this nightly's `mir::Operand`
  has a 4th `RuntimeChecks` variant (driver-75 finding); any non-place /
  non-empty-const operand → typed `DslRejectionKind::Terminator`, never
  `Ok(None)` (which would fall through to the RustCall reject).

The body rewrite owning this shape means no `FnPtrShim` Instance is created and
`translate_fn_sig` is never reached for the `Fn::call` RustCall sig.

### Change B — Phase-2 pre-scan skip for the FnPtrShim callee (`mod.rs`)

In `scan_terminator_for_synthesis`, **wrap** the callee-intern block (the
`translate_ty(canonical.callee_ty)` at `:1246` + the diag-log recognition block)
in `if !matches!(canonical.flavor, FunctionInstanceFlavor::FnPtrShim) { ... }`.
Do **NOT** early-`return Ok(())` — the arg scans (`:1271-1273`,
`scan_call_ty_for_vtable` / `scan_operand_move_for_drop_glue`) must still run for
fn-ptr calls whose args carry vtable/drop-glue needs. Rationale: under Change A
the call is Indirect (no callee Item), so pre-interning `core::ops::Fn::call`
would orphan a bucket-1 Function → completeness rejection (`model.rs:3599-3607`).

### Change C — `needs_drop` guard in `translate_drop_as_call` (`terminator.rs`)

Immediately after computing `place_ty` (`:867`), add:

```rust
if !place_ty.needs_drop(cx.input.tcx, ty::TypingEnv::fully_monomorphized()) {
    return Ok(TerminatorKind::Goto(block_id(target)));
}
```

Mirrors the pre-scan no-op-drop skip (`mod.rs:1279`). Without it, the `Fn`/`FnMut`
bodies' no-op `drop(_1: fn())` — unmasked once the call no longer rejects first —
tries to synthesise drop glue for a fn pointer and ICEs/rejects. `Goto` is the
correct lowering (Regseq has no Drop terminator; same single-successor shape;
`Call` would add an artificial callee/return local). Codex confirmed no HEAD path
relies on the drop-glue path for `needs_drop == false`.

### Change D — coverage row + matrix

- New sidecar row `FF-005` in `matrix_metadata/rows.toml` + `snippets/FF-005.rs`
  (the §1 coerced-fn-pointer-via-`Fn` snippet). Predicate
  `inspect = { kind = "CallCallee", arg = "Indirect" }`. The snippet must be
  FF-005-specific (the predicate matches any indirect call); the discriminating
  signal is reaching `Ok` AT ALL (HEAD rejects).
- Matrix `FF-005:408` `uncovered` → `covered`, noting the reclassification:
  fn-pointer `Fn*::call` is lowered to an indirect call (the `FnPtrShim` flavor
  is intentionally never materialised, mirroring the `ClosureCall` rewrite).

### Change E — integration test `bridge_fnptr_shim_indirect_call.rs`

Helper pattern from `bridge_zero_arg_closure_call.rs` (`catch_unwind` guard).
See Test Plan.

## 4. Non-Goals

- **bare-FnDef-via-`Fn`** (`call_it(g)` without `as fn()`) is OUT — a separate
  gap: `<{g} as FnOnce>::Output` over a *FnDef* self type (A1 only normalises
  *FnPtr* self types, `ty.rs:99-101,177-179`). It rejects with
  `NonRigidTy{Alias}` before the ABI path. A test MAY assert it still rejects.
- **Non-empty all-ZST constant args tuples** for fn-ptr calls (driver-75-style
  materialise) — exotic; defer unless trivially free.
- No change to `translate_abi`'s bare-RustCall rejection, the `ty.rs:265-327`
  FnPtr-type untupling, the closure path's `Move`/`Copy` arm, or any
  model/validator.
- No async/coroutine-closure work.

## 5. Test Plan

Integration tests in `bridge_fnptr_shim_indirect_call.rs` (each `catch_unwind`-
wrapped; assert `BridgeOutcome::Ok` + validates, and the call lowered to
`CallCallee::Indirect` — walk instance bodies for a `TerminatorKind::Call` whose
`callee` is `CallCallee::Indirect`, NOT a `FnPtrShim`-flavored Direct callee):

- **B1 (Fn, zero-arg)**: `fn call_it<F: Fn() -> u32>(f: F) -> u32 { f() }` invoked
  as `call_it(g as fn() -> u32)` → `Ok`, indirect call.
- **B2 (FnMut, zero-arg)**: `fn call_it<F: FnMut() -> u32>(mut f: F) -> u32 { f() }`,
  `call_it(g as fn() -> u32)` → `Ok`, indirect.
- **B3 (FnOnce, zero-arg)**: `fn call_it<F: FnOnce() -> u32>(f: F) -> u32 { f() }`,
  `call_it(g as fn() -> u32)` → `Ok`, indirect.
- **B4 (Fn, non-ZST arg)**: `fn call_it<F: Fn(u32) -> u32>(f: F) -> u32 { f(41) }`,
  `call_it(g as fn(u32) -> u32)` → `Ok`, indirect, one positional arg.
- **B5 (control: direct fn-ptr call)**: `fn f(g: fn()->u32) -> u32 { g() }` → `Ok`,
  unchanged (already indirect at HEAD).
- **B6 (control: closure via `Fn`)**: `call_it(move || x)` → `Ok`, routes through
  a `ClosureCall` synth (unchanged).
- **B7 (boundary, optional)**: bare-FnDef-via-`Fn` (`call_it(g)`) still `Rejected`.
- **B8 (Mode A)**: full runner + the new `FF-005` row → `Total == Pass`,
  `Fail == 0`, exit 0; every pre-existing row unchanged (re-run at HEAD and after).

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/77-fnptr-shim-indirect-call.md` (this spec)
- `.harness/audits/2026-05-24-ff005-fnptrshim-rustcall-plan.md`
- The implemented diff (Changes A–E).

### Reviewer focus

1. Does Change A keep the call off `translate_call_callee`/`callsite_fn_sig` for
   all three call kinds, and is the deref built only on an empty receiver
   projection (Deref-first)?
2. Is Change B a WRAP (arg scans preserved), not an early return?
3. Change C: `Goto` no-op-drop lowering correct + no regression to real-drop rows?
4. The call validates as `CallCallee::Indirect` (recovered callee type `Ty::FnPtr`)
   for `Fn`/`FnMut` (deref) and `FnOnce` (by-value)?
5. Diff scope confined to the 3 sites + the new row/snippet/matrix + the test?

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-77 (cwd = repo root). The fix lowers a
fn-pointer Fn*::call to a CallCallee::Indirect through the deref'd (Fn/FnMut) or by-value
(FnOnce) receiver, untupling args, in a new rewrite_fn_ptr_call_shim; skips pre-interning
the FnPtrShim callee in scan_terminator_for_synthesis (WRAP, not early-return); and adds a
needs_drop guard in translate_drop_as_call (-> Goto). Verify against the spec anchors +
plan. You may build and run the Mode A runner and bridge probe snippets. Out of scope:
bare-FnDef-via-Fn (separate gap); Option B.
</task>
<grounding_rules>
Cite file:line for every claim. Read the diff + cited code; you may build/probe.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
NO_FNPTRSHIM_REACHES_FNSIG: PASS|FAIL + cite
INDIRECT_CALL_VALIDATES (Fn/FnMut/FnOnce): PASS|FAIL + cite
DROP_GUARD_NO_REGRESSION: PASS|FAIL + cite
DIFF_SCOPE_CONFINED: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-driver --lib -- -D warnings` clean AND the new test
  target clean: `cargo clippy -p regseq-driver --test bridge_fnptr_shim_indirect_call
  -- -D warnings`. **NOTE:** do NOT gate on `--all-targets` — pre-existing
  test-target lint drift (driver-74/-75/-76 assessments). The bar: the new lib
  code + new test add no new lints.
- `cargo test -p regseq-driver -- --test-threads=1` green (incl. the new test).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `FF-005` covered.
- Diff scope confined to: `terminator.rs` (the fn-ptr branch + helper + the drop
  guard), `mod.rs` (the pre-scan wrap), `snippets/FF-005.rs`, the `FF-005` matrix
  + sidecar rows, and the new test file. No edits to `translate_abi`, the
  closure `Move`/`Copy` arm, `callsite_fn_sig`, `canonicalize_*`, or any
  model/validator.

## 8. Acceptance Criteria

1. A coerced fn pointer called through `Fn`/`FnMut`/`FnOnce` bridges
   `BridgeOutcome::Ok` and validates (ZST and non-ZST args).
2. The call lowers to `CallCallee::Indirect` through the (deref'd-per-kind)
   fn-pointer value; no `FnPtrShim`-flavored Instance is created.
3. The direct-fn-ptr-call control and closure-via-`Fn` control are unchanged and
   passing.
4. bare-FnDef-via-`Fn` stays rejected (out of scope).
5. A no-op drop (`needs_drop == false`, e.g. a Copy fn pointer) lowers to `Goto`;
   no regression to real-drop rows.
6. Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `FF-005` covered; every
   pre-existing row unchanged.
7. `cargo build` / `fmt` / `clippy -p regseq-driver --lib` / `clippy -p
   regseq-driver --test bridge_fnptr_shim_indirect_call` / `cargo test -p
   regseq-driver` all green.
