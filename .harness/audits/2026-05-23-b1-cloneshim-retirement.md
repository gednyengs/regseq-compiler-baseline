# B1 (FF-012, CloneShim) — RETIRED by reclassification; FF-012 authored

**Status**: RESOLVED (2026-05-23). The F68 audit's B1 proposal is
**retired as mis-scoped** (no bridge code change). Its target row,
**FF-012**, is authored as a passing sidecar row with **zero bridge
changes** via a fn-pointer `.clone()` snippet. Verified at HEAD on the
pinned `nightly-2026-02-01` toolchain. Mode A: Total=142, Pass=142,
Fail=0. Codex-reviewed (APPROVED_WITH_MINOR_FIXES; one closure-example
correction applied below).

This is the **third consecutive F68 proposal retired for mis-scoping**
(after C1/OP-003-I and C2/FF-013). The recurring lesson: const- and
shim-flavor audit proposals' fix-hooks are frequently wrong; verify the
*mechanism* against HEAD + the bridge model + an empirical bridge run
before drafting.

---

## 1. What B1 proposed

Per `.harness/audits/2026-05-21-post-f4-triage.md:141-198`, B1
("CloneShim materialization for arrays/tuples") proposed to **fix the
bridge's handling of the array-length const `4_usize`** so that
`<[u32; 4] as Clone>::clone`'s CloneShim materializes, hypothesizing the
fix at `bridge/const_.rs:42-49`. A Codex footnote (lines 172-188)
correctly suspected the real blocker is `translate_generic_args`
rejecting `GenericArgKind::Const`, and mandated the spec-drafting
session empirically pin the location.

Origin: the F4 escalation
(`.harness/expectations/regseq-driver-67.md:320-335`) tried three
shapes — `#[derive(Clone)] struct` (no CloneShim), `(u32, u32).clone()`
(Phase-5 `ItemMissingInstance`), `[u32; 4].clone()` (`UnsupportedConst`
on `4_usize`) — and judged the array shape "the simpler unblock."

## 2. Why B1 is mis-scoped (empirical, HEAD)

`InstanceKind::CloneShim` is synthesized by rustc **only** for self
types `FnDef | FnPtr | Coroutine | CoroutineWitness | Closure |
CoroutineClosure | Tuple` (`rustc_ty_utils/src/instance.rs:256-275`).
**Arrays are excluded** — `[T; N]` falls into the `_ => return
Ok(None)` arm and instead uses the const-generic library impl
`impl<T: Clone, const N: usize> Clone for [T; N]`.

So `[u32; 4].clone()` never produces a CloneShim. It is rejected at
`bridge/ty.rs:744-748`, where `translate_generic_args` rejects the
`N = 4_usize` const generic arg of the library impl with
`UnsupportedConst { const_repr: format!("{konst:?}") }` — the **same
by-design const-generic rejection that retired C1** (coordinated gap
driver-37, declared out of scope for `#![no_std]` driver specs). This
is NOT an array-length problem: array *types* translate fine via
`len.try_to_target_usize` at `bridge/ty.rs:212-219`. B1 thus targeted
both the wrong construct (arrays don't make CloneShims) and an
out-of-scope fix (const-generic args).

Empirical bridge runs (in-process, bridge-matching config) over the
CloneShim-eligible shapes:

| Snippet | Bridge outcome |
| ------- | -------------- |
| `[u32; 4].clone()` (array) | Rejected: `UnsupportedConst { const_repr: "4_usize" }` (= C1/driver-37 const-generic rejection, `ty.rs:744`) |
| `(u32, u32).clone()` (tuple) | Rejected: Phase-5 `ItemMissingInstance` |
| `f.clone()` where `f: fn()` | **Ok**, flavors `[CloneShim, Item, Item]` |
| `let d = c.clone(); d()` (used closure clone) | rustc ICE: `to get the signature of a closure, use args.as_closure().sig() not fn_sig()` |

## 3. How FF-012 is covered (zero bridge changes)

`crates/bridge-coverage-runner/snippets/FF-012.rs`:

```rust
#![no_std]
pub fn g() {}
pub fn f() -> fn() {
    let p: fn() = g;
    p.clone()
}
```

`fn()` is a `FnPtr` self type, so `<fn() as Clone>::clone` resolves to
`InstanceKind::CloneShim` (`instance.rs:263`). The bridge already
handles the flavor body-absent via the deferred arm at
`crates/regseq-driver/src/bridge/body/materialise.rs:246-269` (same
treatment as FnPtrAddrShim/the other non-`Item` shims); the model
validates OK. The runner's `FunctionInstanceFlavor=CloneShim` predicate
(`crates/bridge-coverage-runner/src/inspect.rs:629-642`) requires only
flavor presence, so the row PASSES. No feature gate is needed (stable
`.clone()` on a fn pointer).

The matrix's "derived `Clone` impl" description was a red herring: a
`#[derive(Clone)]` produces a *user* impl body (flavor `Item`), not a
shim.

## 4. Two real but SEPARATE gaps (neither blocks FF-012)

1. **Tuple `.clone()` → Phase-5 `ItemMissingInstance`.** The tuple
   CloneShim Item is interned by the callee scan but left instance-less
   — the same family as FF-007, which regseq-driver-72 (sub-task B)
   closed via lazy instance allocation in
   `bridge/body/terminator.rs::lookup_or_create_direct_instance`. A
   future task could apply that pattern to tuple CloneShims.
2. **Used closure `.clone()` → rustc ICE.** A cloned closure whose
   result is *used* (`let d = c.clone(); d()`) triggers
   `internal compiler error: ... to get the signature of a closure, use
   args.as_closure().sig() not fn_sig()` during the bridge run — the
   bridge calls `fn_sig()` on a closure type somewhere in its CloneShim
   handling. (A *bare* `c.clone();` whose result is unused instead hits
   `ItemMissingInstance`, per Codex review.) This is an ICE-class bridge
   bug worth its own task.

## 5. Disposition

- **B1 RETIRED** — removed from the active escalation queue; no bridge
  change filed. (FF-012 was the only escalation B1 covered.)
- **FF-012 `uncovered` → `covered`** in the matrix
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md:405`) and as
  a committed sidecar row (`matrix_metadata/rows.toml`,
  `snippets/FF-012.rs`). Sidecar 141 → 142.
- Tuple-clone `ItemMissingInstance` and used-closure-clone ICE logged as
  separate deferred gaps.
