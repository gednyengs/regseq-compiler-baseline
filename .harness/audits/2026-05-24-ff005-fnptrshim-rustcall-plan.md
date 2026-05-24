# FF-005 plan: fn pointer called through an `Fn`/`FnMut`/`FnOnce` bound (FnPtrShim / RustCall ABI)

R5 of the real-fix backlog; the **last** F70 residual. Plan-first, probe- AND
spike-grounded at HEAD on pinned nightly-2026-02-01. Codex plan round 1 returned
NOT_READY with 3 required fixes (pre-scan orphan, no-op drop path, deref-first
constraint); this revision folds all three into the scope and proves the result
with a reverted end-to-end spike.

## 0. Decision (architect-confirmed)

- **Fork = Option A**: lower a fn-pointer `Fn*::call` to an **indirect call**
  through the (deref'd) fn-pointer value. Codegen-faithful (a fn-pointer call IS
  an indirect call); reuses the proven `CallCallee::Indirect` path. Option B
  (materialise a body-absent `FnPtrShim` Direct callee) was rejected: it gives
  downstream C codegen nothing to lower for a call shim. Codex concurred.
- **FF-005 reclassified** `uncovered` → `covered`-by-rewrite: the `FnPtrShim`
  flavor is intentionally never materialised (lowered past), exactly as the
  closure `Fn::call` family is covered via the `ClosureCall` rewrite rather than
  a raw closure Item. Predicate becomes `CallCallee = Indirect`.

## 1. Problem statement (verified at HEAD)

A bare `fn` item coerced to a function pointer, passed to a generic
`F: Fn/FnMut/FnOnce` bound, then invoked, resolves to `InstanceKind::FnPtrShim`
whose `Fn*::call` sig is `extern "rust-call"` — rejected at `translate_abi`.

Probe outcomes (throwaway `try_bridge_fixture_path`, since deleted):

| Snippet | HEAD |
| --- | --- |
| coerced fn-ptr via `Fn` / `FnMut` / `FnOnce`, `f()` | `Rejected(UnsupportedAbi "RustCall")` |
| coerced fn-ptr via `Fn`, non-ZST arg `f(41)` | `Rejected(UnsupportedAbi "RustCall")` |
| **bare FnDef** `call_it(g)` (no `as fn()`) | `Rejected(NonRigidTy{Alias, "<...{g} as FnOnce<()>>::Output"})` |
| direct fn-ptr call `f()` on `f: fn()` (control) | `Ok` |
| closure via generic `Fn` (control) | `Ok` |

Call-site MIR shapes (rustc `-Zunpretty=mir`):

```
call_fn:    _2 = &_1;      <F as Fn<()>>::call(move _2, const ())        ; receiver &F
call_fnmut: _2 = &mut _1;  <F as FnMut<()>>::call_mut(move _2, const ()) ; receiver &mut F
call_fnonce:               <F as FnOnce<()>>::call_once(copy _1, const ()) ; receiver F by value
```

The receiver operand is a reference for `Fn`/`FnMut` (deref to recover the
fn-pointer value) and by-value for `FnOnce`. The args tuple is `const ()` (zero
args) or a `Move`/`Copy` place (non-ZST args). `Fn`/`FnMut` bodies also emit a
`drop(_1)` of the `F = fn()` local after the call (a **no-op** for the Copy fn
pointer).

**Scope corrections (probe):**
- FF-005 = a **coerced fn pointer** through a generic `Fn*` bound (ZST + non-ZST args).
- **bare-FnDef-via-`Fn` is a SEPARATE gap** (`<{g} as FnOnce>::Output` over a
  *FnDef* self type; A1 only normalises *FnPtr* self types — `ty.rs:99-101,177-179`).
  **OUT of scope**; log as an A1-residue candidate (FnDef-self `::Output`).
- Clean `Rejected`, not an ICE.

## 2. Code-surface grounding (HEAD)

- `bridge/fnsig.rs:107-115` — `translate_abi`; RustCall → `UnsupportedAbi` (`:43`).
- `bridge/body/terminator.rs:140-176` — `translate_call`; `rewrite_closure_call_shim`
  (`:157`) returns `Option<(CallCallee, Vec<Operand>)>`.
- `:184-353` — `rewrite_closure_call_shim`; receiver gate `:204-207`
  (`receiver_ty = method_args.type_at(0)`); call_kind `:209-214`; args-untupling
  block `:252-352`.
- `:642-740` — `translate_call_callee`; `canonicalize_call_site_callee` (`:671`)
  → Direct → `callsite_fn_sig` (`:719`, the reject). `CallCallee::Indirect` path
  `:724-737` (fires only when `func_ty` itself is `TyKind::FnPtr`).
- `bridge/mod.rs:2639-2680` — `canonicalize_call_site_callee`; `FnPtrShim` lands in
  the `other =>` arm → `flavor = FnPtrShim`, `callee_def_id = Fn::call`. Struct
  `CanonicalCallSiteCallee` has a `flavor: FunctionInstanceFlavor` field (`:2608-2622`).
- `bridge/mod.rs:1213-1290` — `scan_terminator_for_synthesis` (Phase-2 pre-scan):
  `:1237` canonicalizes, `:1246` `translate_ty(callee_ty)` **interns the callee
  Item**. For FnPtrShim this interns `core::ops::Fn::call`. `:1277-1288` Drop
  arm with a `needs_drop` skip (`:1279`).
- `bridge/body/terminator.rs:856-981` — `translate_drop_as_call`; computes
  `place_ty` (`:867`) and **unconditionally synthesises drop glue** (`:891`,
  `lookup_or_create_direct_instance` `:927`) with **no `needs_drop` guard**.
- `model.rs:3599-3607,3701-3705` — bucket-1 completeness rejects zero-instance
  Function Items; orphan pass hard-errors (`mod.rs:720-725,782-800`).
- `bridge/body/materialise.rs:246-269` — FnPtrShim is in the body-absent bucket.
- `crates/regseq-model/src/place.rs:58-60` + `place_validator.rs:326-329` —
  `PlaceElem::Deref` must be the FIRST projection element.
- `crates/bridge-coverage-runner/src/inspect.rs:90-91,417-422,948-960` —
  `InspectionPredicate::CallCallee(String)`, tag `"Indirect"`, dispatched over
  call terminators.

## 3. The fix — Option A, three coupled sites (spike-proven)

A reverted end-to-end spike (rewrite branch + pre-scan skip + drop guard) made
ALL repros bridge `Ok` — `Fn`/`FnMut`/`FnOnce` zero-arg, `Fn` non-ZST arg — with
the closure and direct-fn-ptr controls unregressed, the full `regseq-driver`
suite green, and **Mode A 145/145/0** (every drop-exercising row — IX-012/013/019,
FT-002/005, FF-011 DropGlue — still PASS, confirming the drop-guard is regression-free).

### Change A — fn-pointer branch in the call rewrite (`terminator.rs`)

Compute `call_kind` before the closure-only receiver gate, and when
`receiver_ty.kind()` is `TyKind::FnPtr`, delegate to a new
`rewrite_fn_ptr_call_shim` that returns `(CallCallee::Indirect(callee), untupled_args)`:
- **Callee value:** `FnOnce` (by-value receiver) → the translated `args[0]`
  operand directly; `Fn`/`FnMut` (`&F`/`&mut F` receiver) → deref. Because
  `PlaceElem::Deref` must be first (`place.rs:58`), build
  `Operand::Copy(Place { local: receiver_place.local, projection: [Deref] })`
  **only when the receiver place projection is empty** (the observed MIR receiver
  is a fresh temp — empty projection); otherwise fall back / materialise a temp.
- **Args:** untuple `args[1]` into positional operands (NO receiver prefix —
  the receiver became the callee), reusing the existing `Move`/`Copy` field
  projection and the empty-`const ()` zero-arg case. Match must be exhaustive
  over `mir::Operand` (this nightly has a 4th `RuntimeChecks` variant — see the
  driver-75 finding); a non-place/non-empty-const args operand → typed
  `DslRejectionKind::Terminator` (mirror driver-75), never `Ok(None)` fall-through.

Decision for the spec: rename `rewrite_closure_call_shim` to a fn-trait-call
rewrite, or keep it as the dispatcher with a `rewrite_fn_ptr_call_shim` sibling
(the spike used a sibling; recommend the sibling for a smaller diff).

### Change B — Phase-2 pre-scan skip for the FnPtrShim callee (`mod.rs`)

In `scan_terminator_for_synthesis`, when `canonical.flavor` is
`FunctionInstanceFlavor::FnPtrShim`, **do not pre-intern the callee Item**
(`translate_ty(callee_ty)` + the diag-log block). Otherwise `core::ops::Fn::call`
is interned as a bucket-1 Function with no instance (the rewrite makes the call
Indirect) → completeness rejects. **Skip only the callee-interning, NOT the arg
scans** (`scan_call_ty_for_vtable` / `scan_operand_move_for_drop_glue` on `args`
must still run for fn-ptr calls whose args carry vtable/drop-glue needs) — i.e.
wrap the callee block in `if !FnPtrShim { ... }`, do **not** early-`return Ok(())`
(the spike used an early return for expedience; the spec must preserve arg scans).

### Change C — `needs_drop` guard in `translate_drop_as_call` (`terminator.rs`)

After computing `place_ty` (`:867`), if
`!place_ty.needs_drop(tcx, TypingEnv::fully_monomorphized())`, return
`TerminatorKind::Goto(block_id(target))` (a no-op drop) instead of synthesising
drop glue. Mirrors the pre-scan skip (`mod.rs:1279`). Without it the `Fn`/`FnMut`
bodies' no-op `drop(_1: fn())` (unmasked once the call no longer rejects first)
tries to synth drop glue for a fn pointer and ICEs/rejects. Mode A confirms no
regression to real-drop rows.

## 4. Coverage + scope

- **Change D — coverage:** FF-005 sidecar row + `snippets/FF-005.rs` (a coerced
  fn pointer called through `Fn`); matrix `FF-005:408` `uncovered`→`covered` with
  the reclassification note. Predicate `inspect = { kind = "CallCallee", arg = "Indirect" }`
  (exists, `inspect.rs:90`); the snippet must be FF-005-specific since the
  predicate is broad (any indirect call matches).
- **Change E — integration test** `bridge_fnptr_shim_indirect_call.rs`: Fn (B1),
  FnMut (B2), FnOnce (B3), non-ZST arg (B4) — each `Ok` + validates + asserts the
  call lowered to `CallCallee::Indirect`; controls: direct-fn-ptr call and
  closure-via-`Fn` unchanged (B5/B6).

Non-goals: bare-FnDef-via-`Fn` (separate `::Output`-over-FnDef gap); non-empty
all-ZST constant args tuples (driver-75-style materialise — exotic for fn-ptr,
defer unless trivially free); async/coroutine closures; changing `translate_abi`'s
bare-RustCall rejection; the `ty.rs:265-327` FnPtr-type untupling.

## 5. Acceptance criteria (preview)

1. A coerced fn pointer through `Fn`/`FnMut`/`FnOnce` bridges `Ok` + validates
   (ZST + non-ZST args).
2. The call lowers to `CallCallee::Indirect` through the (deref'd-per-kind)
   fn-pointer value; no `FnPtrShim` instance is created.
3. `direct_fnptr_call` and closure-via-`Fn` unchanged; Mode A `Total == Pass`,
   `Fail == 0` with FF-005 covered; every prior row unchanged.
4. bare-FnDef-via-`Fn` stays rejected (out of scope; documents the boundary).
5. No-op drops of Copy types compile to `Goto` (Change C) with no regression to
   real-drop rows.
6. build / fmt / clippy(lib + new test) / driver suite green.

## 6. Risks

- **Change C touches ALL drops** (not just fn-ptr). Mitigated: Mode A 145/145/0
  with every drop row passing in the spike; the guard only converts
  `needs_drop == false` drops (already semantically no-ops) to `Goto`.
- **Change B early-return vs wrap**: the spec MUST wrap (preserve arg scans), not
  early-return as the spike did.
- **Deref-first**: only valid on an empty receiver projection; assert/handle the
  non-empty case explicitly rather than producing an invalid place.
- Predicate breadth: FF-005's `CallCallee=Indirect` matches any indirect call;
  the snippet must isolate the fn-ptr-via-`Fn` shape.

## 7. Open questions for Codex (round 2)

1. Is the three-site scope (rewrite branch + pre-scan skip + drop guard)
   complete, or does any other path (e.g. `scan_operand_move_for_drop_glue`,
   the orphan pass) still need a FnPtrShim carve-out?
2. Change C: is `TerminatorKind::Goto(block_id(target))` the correct no-op-drop
   lowering, and is there any existing row whose drop is `needs_drop == false`
   that currently relies on the drop-glue path (i.e. a hidden behavioural change)?
3. Change A deref: confirm `Operand::Copy(Place{local,[Deref]})` validates as the
   indirect callee (place_ty over the Deref yields the fn-ptr type; the indirect
   call's arg/sig typing).
4. Should Change B wrap (preserve arg scans) — confirm no fn-ptr-call arg shape
   needs the vtable/drop-glue pre-scan that an early return would drop.
5. Sibling `rewrite_fn_ptr_call_shim` vs rename — cleaner given the shared
   untupling? (Spike used a sibling.)
6. FF-005 predicate `CallCallee=Indirect` + an FF-005-specific snippet — right, or
   is a tighter predicate available/needed?
