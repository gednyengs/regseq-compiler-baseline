# R3 plan: zero-argument closure call ICE (`fn_sig()` on a closure)

**Status**: PLAN (pre-Codex). Drafted 2026-05-24 after a HEAD-grounded
feasibility probe on pinned `nightly-2026-02-01` (post-R2 HEAD `666ca26`).
This plan **corrects the B1-audit / HANDOFF framing of R3**, which was
doubly mis-scoped (the 5th mis-scope this session after C1/C2/B1 and the
two-layer R2 correction). Probe-first caught it again.

---

## 0. The B1-audit framing was doubly wrong

The HANDOFF / B1-retirement audit (`2026-05-23-b1-cloneshim-retirement.md:101-108`)
described R3 as:

> Used closure `.clone()` → rustc ICE. A cloned closure whose result is
> *used* (`let d = c.clone(); d()`) triggers `... to get the signature of
> a closure, use args.as_closure().sig() not fn_sig()` ... the bridge
> calls `fn_sig()` on a closure type somewhere in its **CloneShim
> handling**.

Both claims are false (verified §1):

1. **Not clone-specific.** `let c = move || x; c()` — no clone anywhere —
   ICEs identically. The clone in the repro was incidental: the cloned
   closure was simply *called with zero arguments*.
2. **Not CloneShim handling.** The ICE is in the **closure-call** path
   (`rewrite_closure_call_shim` → fall-through → `callsite_fn_sig`), not
   anything to do with `Clone`.

(Bonus: the B1 audit's *unused* clone case, reported as `ItemMissingInstance`,
now bridges `Ok` — **R2/driver-74's orphan pass fixed it**.)

## 1. Problem statement (verified at HEAD on pinned nightly-2026-02-01)

Minimal repro (plain stable `#![no_std]` Rust, no clone):

```rust
pub fn f() -> u32 {
    let x = 5u32;
    let c = move || x;   // zero-arg closure
    c()
}
```

Bridge outcome at HEAD: rustc **ICE** during the in-process bridge run:
`internal compiler error: rustc_hir_analysis/src/collect.rs:1028: to get
the signature of a closure, use args.as_closure().sig() not fn_sig()`.

**Root cause (fully traced via instrumentation):**

1. `c()` lowers to `_0 = <{closure} as Fn<()>>::call(move _self, move _args)`.
   In the **optimized** MIR that `tcx.instance_mir` returns, the zero-arg
   args-tuple `()` is folded to a **constant `const ()`** operand (the
   `_args = (); move _args` of unoptimized MIR collapses to `const ()`).
2. `translate_call` calls `rewrite_closure_call_shim` first
   (`terminator.rs:156-157`). The rewrite recognises the `Fn::call` on a
   `TyKind::Closure` receiver and begins untupling the args operand. Its
   untupling match only handles `Operand::Move(place) | Operand::Copy(place)`
   (`terminator.rs:268-289`); a constant tuple hits the
   `_ => return Ok(None)` arm (`terminator.rs:290`) and the rewrite
   **bails**.
   (Confirmed by instrumentation: `bail tuple operand not Move/Copy: const ()`.)
3. With the rewrite returning `None`, `translate_call` falls through to the
   generic `translate_call_callee` (`terminator.rs:167`).
   `canonicalize_call_site_callee` resolves the `Fn::call` via
   `Instance::try_resolve` to `InstanceKind::Item(closure_def_id)` (the
   `Item` arm, `mod.rs:2475-2480`), returning `callee_def_id = closure_def_id`.
4. `lookup_or_create_direct_instance`'s sig closure calls
   `callsite_fn_sig(cx, closure_def_id, args)` (`terminator.rs:654`), which
   calls `tcx.fn_sig(closure_def_id)` (`terminator.rs:721-725`) → **ICE**:
   closures have no `fn_sig` query result; their signature must come from
   `args.as_closure().sig()`.

**One-line root cause:** the closure-call rewrite bails on a **constant
args-tuple operand**, and the generic fall-through then calls `tcx.fn_sig()`
on the closure's DefId, which ICEs.

**Breadth (corrected per Codex round 1 — broader than just zero-arg):**
The trigger is **any closure call whose args-tuple operand is an
`Operand::Constant`**. Codex's MIR probe on the pinned toolchain shows this
class is:
- **Zero-argument** closure calls (`c()`): args-tuple is `const ()`.
  (My own probe: capturing/non-capturing/FnMut/cloned-closure all ICE.)
- **Non-empty all-ZST argument** closure calls: `c(())`, `c(Z)` for a ZST
  `Z`, two unit args, `FnOnce<((),)>` — all lower to a `const (...)` tuple
  operand and hit the **same** bail→ICE path. (Codex MIR probe.)

What does **not** ICE (args-tuple is a `Move`/`Copy` **place**, handled by
the existing rewrite): closure calls with at least one **non-ZST** arg —
`c(1)` (const), `c(a)` (dynamic). Every passing corpus closure row (IX-004
`f(x,y)`, SK-003 `f(x)`, BD-007 `f(1)`, IX-014 `f(0)`) passes a non-ZST
arg via a tuple local, which is why this gap was never surfaced.

So the precise condition is **"closure `Fn*::call` whose args-tuple arrives
as a constant operand"** (empty tuple, or all-ZST fields), not "zero-arg
only" — the plan's first draft was too narrow here.

## 2. Code-surface grounding

- `crates/regseq-driver/src/bridge/body/terminator.rs:140-176` —
  `translate_call`: rewrite-first, else `translate_call_callee`.
- `:184-292` — `rewrite_closure_call_shim`. Receiver handling `:226-251`;
  **args-tuple untupling `:256-291`**, with the bail at `:290`
  (`_ => return Ok(None)`). The `Move|Copy` arm `:268-289` projects tuple
  fields off the place; the `tuple_ty` Tuple check `:265-267`.
- `:577-657` — `translate_call_callee` (the fall-through ICE path);
  sig closure at `:648-655`.
- `:716-727` — `callsite_fn_sig` → `tcx.fn_sig(def_id).instantiate(...)`.
- `crates/regseq-driver/src/bridge/mod.rs:2460-2501` —
  `canonicalize_call_site_callee`; the `Item` arm `:2475-2480` returns the
  closure DefId for a closure `Fn::call`.
- rustc ICE site: `rustc_hir_analysis/src/collect.rs:1028` (`fn_sig` on a
  closure). Correct accessor: `args.as_closure().sig()`.
- Closure-call shim arity: the synth shim for `Fn<()>` takes the receiver
  only (no positional args); see `construct_closure_call_instances`
  (`bridge/mod.rs` ~`:2278-2289`) and the synth in `bridge/synth/closure.rs`.

## 3. Fix directions evaluated

### Option A — handle the constant args-tuple in the rewrite (RECOMMENDED)

Extend `rewrite_closure_call_shim`'s untupling (`terminator.rs:256-291`) to
accept an `Operand::Constant` args-tuple, so the rewrite keeps ownership of
the call and **never** falls through to the closure-ICEing path. Two
sub-cases (per the corrected breadth):

- **Empty tuple `const ()`** (zero fields): push **zero** positional args
  (`normalised` stays `[receiver]`). Matches the `Fn<()>` shim arity
  (receiver only). **Empirically confirmed viable** (temporary-patch probe,
  below).
- **Non-empty constant tuple** (all-ZST fields, e.g. `c(())`): the existing
  `Move`/`Copy` arm projects each field off a *place*; a constant has no
  place. Materialise the constant args-tuple into a **synthetic local**
  via `Statement::Assign(local, Rvalue::Use(<translated const>))` and then
  project its fields exactly like the `Move`/`Copy` arm — the same
  synthetic-local-then-project pattern `cast_receiver_to_captures_view`
  already uses for the receiver (`push_synthetic_local` + `Statement::Assign`,
  `terminator.rs:527`). (The shim expects one positional input per
  closure-input-tuple field, `mod.rs:2474-2482`, so the projected ZST fields
  match per-field; the bridge already supports value-level ZST constants via
  `Const::ZST`, `const_value.rs:32` / `const_.rs:340`.) **Decision (Codex
  round 2): materialise-and-project is the chosen approach** — feasibility
  confirmed and ZST closure args are not exotic for `#![no_std]`. A typed
  `DslRejectionKind::Terminator` is reserved only for an *untranslatable*
  constant or a *truly unexpected* operand shape, never the silent bail→ICE.
- **Replace the catch-all bail.** Change the args-tuple
  `_ => return Ok(None)` (`terminator.rs:286`) into a typed
  `DslRejectionKind::Terminator` carrying the unexpected operand, so **no**
  closure-call args shape can ever again silently fall through to
  `tcx.fn_sig(closure)`. (Codex round 1: the ICE is fully closed only if
  the bail is replaced, not merely if the empty-tuple arm is added.)
- **Blast radius:** confined to the closure-call rewrite's args-untupling.
  Non-ZST-arg closure calls are unchanged (still place-projected).
- **Viability — empty case empirically confirmed (temporary patch probe):**
  adding just the `Operand::Constant if tuple_fields.is_empty() => push
  nothing` arm makes the zero-arg ICE cases bridge `Ok` + validate —
  capturing `move || x` (5 items/5 instances), non-capturing `|| 7`, an
  `FnMut` zero-arg (5 items/4 instances), **and** the original B1
  cloned-closure repro `let d = c.clone(); d()` (7 items/7 instances). The
  rewrite completes through `synthesise_closure_for_def_id` +
  `pick_closure_call_handle` + the shim-instance lookup; the model
  validates with `normalised = [receiver]`. **The non-empty-ZST
  materialise-and-project sub-case is NOT yet probed** — the implementer
  must verify it (test `c(())`).

### Option B — make `callsite_fn_sig`/canonicalize closure-aware — REJECTED

Teach `callsite_fn_sig` (or `canonicalize_call_site_callee`) to use
`args.as_closure().sig()` when `def_id` is a closure, instead of
`tcx.fn_sig()`. This silences the ICE but **keeps the closure call on the
generic direct-FnDef path**, producing a model where the closure is called
as a plain `Item` rather than through the `ClosureCall` synth shim that all
other closure calls use — an **inconsistent / wrong model** (and likely a
downstream validator or captures-view mismatch). It treats the symptom, not
the cause. Reject.

### Option C — defense-in-depth guard only — REJECTED as primary

A standalone closure-guard in `callsite_fn_sig` without Option A is dead
code once the rewrite fully covers closure calls (violates CLAUDE.md §1.2
no-speculative-code). The "clean rejection instead of bail→ICE" element is
folded into Option A instead (it is reachable and useful, not speculative).

## 4. Why Option A is the principled fix

Closure calls are modelled via the `ClosureCall` synth shim by design
(spec §13.1). The bug is that one operand shape (`const ()`) slips past the
rewrite into a generic path that was never meant to see a closure DefId.
Option A keeps **all** closure calls on the intended rewrite path, matching
how the corpus already models non-empty closure calls; B/C diverge from
that model or add dead code.

## 5. Proposed scope (Option A)

- **Change A** — `rewrite_closure_call_shim` untupling
  (`terminator.rs:256-291`): add an `Operand::Constant` arm for the
  args-tuple. Empty tuple → push no positional args. Non-empty constant
  tuple (all-ZST) → materialise into a synthetic local + project fields
  (reuse the `Move`/`Copy` projection), per §3. Replace the catch-all
  `_ => return Ok(None)` for the args-tuple operand with a typed
  `DslRejectionKind::Terminator`, so no closure-call args shape can silently
  fall through to the ICE path again.
- **Change B** — coverage: new matrix row `FF-017` (next free id; verify)
  + sidecar row + snippet for a zero-arg closure call; integration tests
  including a non-empty-ZST-arg case (`c(())`).

## 6. Acceptance criteria (preview)

1. `let c = move || x; c()` bridges to `BridgeOutcome::Ok` and validates,
   routed through the `ClosureCall` shim (not a direct closure-Item call).
2. The non-capturing `let c = || 7u32; c()` also bridges `Ok`.
3. A non-empty all-ZST-arg closure call (`c(())`) bridges `Ok` + validates
   (the materialise-and-project sub-case), or — if the fallback is chosen —
   produces a typed `DslRejection`, never an ICE.
4. Non-ZST-arg closure calls (`c(1)`, `c(a)`) are unchanged and passing.
5. The used cloned-closure case `let d = c.clone(); d()` now bridges `Ok`
   (it was the original R3 repro; the clone path was already fine post-R2,
   only the zero-arg call ICEd).
6. No closure-call args operand can reach `tcx.fn_sig(closure)` — verify no
   ICE for any probed closure-call shape; unexpected shapes get a typed
   rejection, not an ICE.
7. Mode A: `Total == Pass`, `Fail == 0`, with the new `FF-017` row; all
   prior rows unchanged.

## 7. Risks

- **R-1 (closure-call modelling invariant):** Change A alters how the
  closure-call rewrite normalises args. The closure-call shim arity /
  captures-view pairing is validated by `CallArgTypeMismatch`
  (`model.rs:3041-3048`); a zero-positional-arg call must match the shim's
  `FnSig.inputs = [captures]`. The assessment must confirm the shim sig and
  the normalised arg list agree for the zero-arg case. (Per
  [[assessment-soundness-pass]], a focused Codex review of the rewrite
  change is warranted since it touches call-terminator construction.)
- **R-2 (which Fn-trait):** the probe saw `Fn<()>::call`; `FnMut`/`FnOnce`
  zero-arg variants (`call_mut`/`call_once`) must be covered too (the
  rewrite handles all three call kinds — confirm the const-tuple arm
  applies regardless of `ClosureCallKind`).
- **R-3 (non-empty constant tuple — now known to occur):** non-empty
  all-ZST arg tuples (`c(())`) DO produce a constant operand (Codex MIR
  probe), so this sub-case is real, not hypothetical. If the chosen
  treatment is "reject," `c(())`-style calls become unbridgeable (a clean
  rejection, not a regression — they ICE today). If "handle," the
  materialise-and-project path must be verified (not yet probed). Either
  way, Mode A over the full corpus must confirm no currently-passing row
  changes — but note the corpus has no such shape today (all closure rows
  pass non-ZST args), so the risk of regressing an existing row is nil; the
  decision only affects *new* coverage.

## 8. Open questions for Codex

1. Is Option A's "handle the constant args-tuple; convert the bail to a
   typed rejection" the right shape, vs a narrower "empty-tuple only" patch?
2. For a non-empty **constant** args-tuple (confirmed to occur for all-ZST
   args, e.g. `c(())`): materialise-to-local-and-project (full support), or
   clean-reject (`DslRejectionKind::Terminator`)? Is ZST-arg closure-call
   support worth the extra code, or is a typed rejection acceptable for the
   `#![no_std]` driver surface?
3. Does pushing zero positional args for `Fn<()>` exactly match the synth
   shim's expected `FnSig.inputs` (receiver-only), so `CallArgTypeMismatch`
   stays satisfied?
4. Any other operand/shape (besides the args-tuple) by which a closure
   `Fn::call` could still reach `translate_call_callee` →
   `tcx.fn_sig(closure)`? (i.e., is the rewrite the *only* guard, and does
   Change A make it complete?)
