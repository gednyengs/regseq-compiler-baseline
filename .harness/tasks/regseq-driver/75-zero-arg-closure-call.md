# regseq-driver-75 — Closure calls with a constant args-tuple operand (zero-arg / all-ZST-arg closure call ICE)

## 1. Description

Calling a closure whose monomorphised `Fn*::call` args-tuple arrives as an
`Operand::Constant` causes a **rustc ICE** during the in-process bridge
run: `internal compiler error: rustc_hir_analysis/src/collect.rs:1028: to
get the signature of a closure, use args.as_closure().sig() not fn_sig()`.

The smallest repro is a **zero-argument** closure call (plain stable
`#![no_std]`):

```rust
pub fn f() -> u32 {
    let x = 5u32;
    let c = move || x;
    c()
}
```

Root cause: `c()` lowers to `<{closure} as Fn<()>>::call(move _self, ARGS)`
where — in the optimized MIR `tcx.instance_mir` returns — the args-tuple
is a **constant** operand (`const ()` for zero args; a `const (...)` for
non-empty **all-ZST** args). The closure-call rewrite
`rewrite_closure_call_shim` only untuples `Move`/`Copy` *place* operands and
otherwise bails (`terminator.rs:286 _ => return Ok(None)`). `translate_call`
then falls through to the generic `translate_call_callee`, where
`canonicalize_call_site_callee` resolves the `Fn::call` to
`InstanceKind::Item(closure_def_id)` and `callsite_fn_sig` calls
`tcx.fn_sig(closure_def_id)` — which ICEs (closures have no `fn_sig` query).

Fix: handle the constant args-tuple inside the rewrite so closure calls
never fall through to the closure-ICEing path, and replace the silent bail
with a typed rejection.

NOTE: the project's B1 audit mis-scoped this as a "used cloned closure /
CloneShim" ICE. A probe disproved both: a plain `let c = move || x; c()`
(no clone) ICEs identically, and the path is the closure-**call** rewrite,
not anything `Clone`. Full analysis + Codex-reviewed plan (2 rounds):
`.harness/audits/2026-05-24-r3-zero-arg-closure-call-plan.md`.

## 2. Input Context

### Design source

- `.harness/audits/2026-05-24-r3-zero-arg-closure-call-plan.md` — R3 plan,
  Option A (recommended). Codex round 1 (NOT_READY — breadth too narrow,
  missed all-ZST constant tuples) → round 2 (APPROVED).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md` — closure-call
  rows IX-004/SK-003/BD-007/IX-014 all pass **non-ZST** args via a generic
  `impl Fn` fn, which is why this gap was never surfaced.

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §13.1 (closure-call shim rewrite:
  `Fn::call`/`FnMut::call_mut`/`FnOnce::call_once` against a closure receiver
  lower to a `ClosureCall` synthetic Function Item with the args untupled to
  positional inputs).

### Bridge code anchors (consumed/modified)

- `crates/regseq-driver/src/bridge/body/terminator.rs:140-176` —
  `translate_call`: tries `rewrite_closure_call_shim` first (`:156-157`),
  else `translate_call_callee` (`:167`, the fall-through ICE path).
- `:184-385` — `rewrite_closure_call_shim`. **Args-tuple untupling
  `:252-288`**: `tuple_ty` Tuple check `:261-263`; `Move`/`Copy` projection
  arm `:264-285`; **the catch-all bail `:286` (`_ => return Ok(None)`) — the
  modification site.**
- `:483-546` — `cast_receiver_to_captures_view`: the canonical synthetic-
  local pattern — `body_cx.model_body_mut()` (`:527`) →
  `model_body.push_synthetic_local(ty_id)` (`:533`) → push
  `Statement { kind: StatementKind::Assign(Place::from(local), <rvalue>), span }`
  (`:534-544`) → use `Operand::Move(Place::from(local))` (`:545`).
- `:577-657` — `translate_call_callee` (fall-through); sig closure `:648-655`.
- `:716-731` — `callsite_fn_sig`; the `tcx.fn_sig(def_id).instantiate(...)`
  call (the ICE site) is at `:729-730`.
- `crates/regseq-driver/src/bridge/mod.rs:2455-2485` — closure-call shim
  `FnSig` construction: `inputs = [receiver] ++ (one per closure-input-tuple
  field)` (receiver pushed `:2482-2483`, fields appended `:2484-2485`). For a
  zero-arg closure the tuple is empty ⇒ `inputs = [receiver]`.
- `crates/regseq-model/src/model.rs:3053-3075` — `CallArgTypeMismatch`
  validator (`:3070-3075`; exact arity + per-arg type equality). The
  normalised arg list must match the shim `FnSig.inputs`.
- `crates/regseq-model/src/const_value.rs:34` (`Const::ZST`) +
  `crates/regseq-driver/src/bridge/const_.rs:342` (`ConstValue::ZeroSized` →
  `Const::ZST`) — the bridge already represents value-level ZSTs.
- `crates/regseq-driver/src/bridge/body/operand.rs:24-25` —
  `translate_operand` maps a MIR constant to `Operand::Constant(translate_const(...))`
  (the way to obtain the translated const operand for `Rvalue::Use`).
- `crates/regseq-model/src/body_ty.rs:88-114` — tuple field projection
  recovers the projected field type at `:114` (so projecting fields off the
  materialised local is type-valid).

### Prior tasks

- regseq-driver-37/62/71/72 — closure-call rewrite + `ClosureCall` synth +
  the existing `Move`/`Copy` untupling this change extends.
- regseq-driver-74 (R2) — its orphan pass already fixed the *unused* cloned
  closure case; R3 is the orthogonal closure-**call** ICE.

### Test surface

- New: `crates/regseq-driver/tests/bridge_zero_arg_closure_call.rs`.
- New matrix row `FF-017` + sidecar row + `snippets/FF-017.rs`.
- Mode A runner over the full corpus.

## 3. Implementation Notes

### Change A — handle the constant args-tuple in `rewrite_closure_call_shim`

In the args-tuple untupling block (`terminator.rs:252-288`), add handling
for an `Operand::Constant` args-tuple and replace the catch-all bail:

- **Empty tuple** (`tuple_fields.is_empty()`, e.g. `const ()`): push **no**
  positional args (`normalised` stays `[receiver]`). Matches the `Fn<()>`
  shim arity (`mod.rs:2455-2485`). (Empirically proven viable — see plan §3.)
- **Non-empty constant tuple** (all-ZST fields, e.g. `c(())`): materialise
  the constant args-tuple into a synthetic local and project its fields,
  reusing the `Move`/`Copy` projection. Follow the
  `cast_receiver_to_captures_view` pattern exactly:
  `model_body.push_synthetic_local(<model tuple ty>)`, push
  `StatementKind::Assign(Place::from(synth_local), Rvalue::Use(<translated
  const operand>))` where the const operand is
  `translate_operand(cx, body_cx, tuple_arg.node.clone())` (maps the MIR
  constant to `Operand::Constant`, `operand.rs:24-25`) — **not** a
  `Transmute` cast (the receiver-cast uses `Transmute` only because it
  changes the closure view; this is a plain value move). Then project
  `PlaceElem::Field(FieldIdx, <field model ty via translate_ty>)` off
  `Place::from(synth_local)` for each field (same loop as `:268-284`,
  carrying each field's model type as that arm does at `:268-274`), and push
  each as **`Operand::Move(projected)`** (a materialised constant has no
  source move/copy discriminator; `Move` is the uniform choice). The bridge
  already represents value-level ZSTs (`Const::ZST`), so the translated
  constant operand is well-defined.
- **Replace the catch-all `_ => return Ok(None)` (`:286`)** with a typed
  `BridgeError::DslRejection { kind: DslRejectionKind::Terminator, ty_repr:
  <unexpected operand repr> }`. This guarantees no closure-call args shape
  can silently fall through to `translate_call_callee` →
  `tcx.fn_sig(closure)` again. (A typed rejection is reserved for a truly
  unexpected operand or an untranslatable constant; the ZST cases above are
  handled, not rejected.)

The receiver handling (`:226-251`) and the shim-instance resolution
(`:294-...`) are unchanged: the call stays on the `ClosureCall` rewrite path.

### Change B — coverage row + tests

- New matrix row `FF-017` (next free id; verify) in
  `bridge-coverage-matrix.md`: positive, covered, the zero-arg closure-call
  shape, citing this task.
- New sidecar row `FF-017` in `matrix_metadata/rows.toml` + snippet
  `snippets/FF-017.rs` = the §1 zero-arg closure-call snippet. Predicate:
  positive, expects `BridgeOutcome::Ok` with `SyntheticItemKind = ClosureCall`
  (the predicate the existing closure-call rows use, `rows.toml:1379-1467`;
  the runner supports it via `inspect.rs:43-67`). The discriminating signal
  is reaching `Ok` at all — at HEAD this snippet ICEs.

## 4. Non-Goals

- **Do NOT change the fall-through `translate_call_callee` / `callsite_fn_sig`
  / `canonicalize_call_site_callee` to special-case closures** (the rejected
  Option B). Making the rewrite own all closure-call shapes is the fix; a
  closure-guard in `callsite_fn_sig` would either be dead code or produce a
  model that bypasses the `ClosureCall` synth path (inconsistent with every
  other closure call).
- **Do NOT alter the `Move`/`Copy` untupling arm** — non-ZST-arg closure
  calls already work and must stay byte-identical.
- **Do NOT touch the closure synthesis (`synth/closure.rs`) or the shim
  `FnSig` construction (`mod.rs:2455-2485`)** — the fix is purely in the
  call-site args normalisation.
- No DropGlue/Coroutine/async-closure work.

## 5. Test Plan

Integration tests in `bridge_zero_arg_closure_call.rs` (helper pattern from
`bridge_const_fnptr_provenance.rs`; wrap the bridge call in
`catch_unwind` so a regression ICE is reported as a failure, not an abort):

- **B1 (zero-arg, capturing)**: `let x = 5u32; let c = move || x; c()`
  → `BridgeOutcome::Ok` + validates. Assert the call routed through a
  `ClosureCall` synth instance (a `SyntheticItemKind::ClosureCall` item
  exists), not a direct closure-`Item` call.
- **B2 (zero-arg, non-capturing)**: `let c = || 7u32; c()` → `Ok` + validates.
- **B3 (FnMut zero-arg)**: `let mut n = 0u32; let mut c = move || { n += 1; n }; c()`
  → `Ok` + validates (covers the `CallMut` call kind / receiver shape).
- **B4 (all-ZST arg)**: `let c = |_u: ()| 1u32; c(())` → `Ok` + validates
  (the materialise-and-project sub-case). Materialise is the decided
  treatment (plan §3, Codex round 2), so this MUST bridge `Ok`, not reject.
- **B5 (used cloned closure — original B1-audit repro)**:
  `let x = 5u32; let c = move || x; let d = c.clone(); d()` → `Ok` + validates.
- **B6 (regression — non-ZST arg)**: `let c = |n: u32| n + 1; c(41)` → `Ok`,
  unchanged from HEAD (still place-projected).
- **B7 (Mode A)**: full runner over the corpus + the new `FF-017` row reports
  `Total == Pass`, `Fail == 0`, exit 0; every pre-existing row unchanged
  (re-run Mode A at HEAD and after the fix; diff per-row).

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/75-zero-arg-closure-call.md` (this spec)
- `.harness/audits/2026-05-24-r3-zero-arg-closure-call-plan.md`
- The implemented diff (Change A/B); for spec review the §2 anchors suffice.

### Reviewer focus

1. Does Change A keep **all** closure-call shapes on the rewrite path, so
   none reaches `tcx.fn_sig(closure)`? Is the bail truly replaced (not just
   the empty arm added)?
2. For the zero-arg case, does `normalised = [receiver]` exactly match the
   shim `FnSig.inputs` (receiver-only) so `CallArgTypeMismatch`
   (`model.rs:3041-3066`) stays satisfied — for `Fn`, `FnMut`, AND `FnOnce`
   receiver shapes?
3. Non-empty all-ZST: does the materialise-into-synthetic-local + project
   produce arg types matching the shim's per-field inputs?
4. Diff scope confined to the rewrite's args-untupling + the new row/tests?

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-75 (cwd = repo root). The fix
handles a constant args-tuple operand in rewrite_closure_call_shim (empty →
zero positional args; non-empty all-ZST → materialise+project; replace the
`_ => Ok(None)` bail with a typed DslRejection) so closure calls never fall
through to tcx.fn_sig(closure) and ICE. Verify against the spec §2 anchors
and the plan. Out of scope: redesigning the approach (plan is Codex-approved).
</task>
<grounding_rules>
Cite file:line for every claim. Read the diff + cited code; you may build/run
the Mode A runner and probe closure-call snippets (incl. an ICE check).
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
NO_CLOSURE_REACHES_FN_SIG: PASS|FAIL + cite
ARITY_MATCHES_SHIM: PASS|FAIL + cite
NO_ICE_ANY_SHAPE: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-driver --lib -- -D warnings` clean AND the new
  test file clippy-clean. **NOTE:** do NOT gate on
  `cargo clippy -p regseq-driver --all-targets` — it has MANY PRE-EXISTING
  test-target lint failures (clippy-version drift in untouched test files;
  documented in the driver-74 assessment). The bar is: the implementer's
  lib change + the new test file add no new lints.
- `cargo test -p regseq-driver -- --test-threads=1` green (incl. the new
  `bridge_zero_arg_closure_call.rs`).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with the new `FF-017` row.
- Diff scope confined to: `bridge/body/terminator.rs` (the args-untupling
  block only), the new test file, the matrix row, the sidecar row + snippet.
  No edits to `translate_call_callee`/`callsite_fn_sig`/`canonicalize_*`, the
  closure synth, or the shim `FnSig` construction.

## 8. Acceptance Criteria

1. `let c = move || x; c()` bridges to `BridgeOutcome::Ok` and validates,
   routed through a `ClosureCall` synth instance (not a direct closure-`Item`
   call).
2. Non-capturing `let c = || 7u32; c()` bridges `Ok` + validates.
3. `FnMut` zero-arg closure call bridges `Ok` + validates.
4. An all-ZST-arg closure call (`c(())`) bridges `Ok` + validates (the
   materialise-and-project path), **never an ICE**. (Typed
   `DslRejectionKind::Terminator` is reserved only for a truly unexpected
   args-tuple operand shape, not all-ZST constant tuples.)
5. The used cloned-closure repro `let d = c.clone(); d()` bridges `Ok` +
   validates.
6. Non-ZST-arg closure calls (`c(41)`) are unchanged and passing.
7. No closure-call args operand can reach `tcx.fn_sig(closure)`: the
   `_ => Ok(None)` args-tuple bail is replaced by a typed rejection.
8. Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `FF-017` covered;
   every pre-existing row unchanged.
9. `cargo build` / `fmt` / `clippy -p regseq-driver --lib` / `cargo test -p
   regseq-driver` all green.
