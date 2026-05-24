# B2 (FF-004 ReifyShim + RJ-024 UnsupportedEnumLayout) — probe outcome: DEFER both

**Status**: RESOLVED (2026-05-23). B2 was the F68 audit's LOW-priority
*probe-or-defer* proposal (deliberately NOT a bridge-fix task). The
HEAD-grounded probe found **no zero-change source shape** for either
row, so both are reclassified `uncovered` → blocked, and B2 is removed
from the active escalation queue. Verified at HEAD on the pinned
`nightly-2026-02-01` toolchain.

Unlike C2/B1 (mis-scoped proposals where the target row was actually
coverable today), B2 was correctly scoped, and the honest outcome is a
**deferral** — but the probe produced real value by pinning FF-004's
unblock path and linking it to the C2-logged const-provenance gap.

---

## 1. FF-004 (`ReifyShim`) → `blocked-open-bridge-bug`

`InstanceKind::ReifyShim`'s source-reachable trigger is reifying a
`#[track_caller]` fn to a fn pointer (`resolve_for_fn_ptr` at
`rustc_middle/src/ty/instance.rs:606-609`: an `Item` requiring caller
location → `ReifyShim`; also the `Virtual` and KCFI paths, not
source-authorable in `#![no_std]`). Both reachable paths are blocked at
HEAD:

| Shape | Bridge outcome |
| ----- | -------------- |
| `let fp: fn() = tc_fn` / `fn e() -> fn() { tc_fn }` (runtime reify) | Ok, flavors `[Item, Item]` — **no ReifyShim** |
| `static FP: fn() = tc_fn` (const reify) | **Rejected**: `unsupported const: pointer scalar over Function { instance: ReifyShim(tc_fn) }` |

1. **Runtime reify** lowers to a `Cast(PointerCoercion(ReifyFnPointer))`
   rvalue (confirmed via MIR dump). The bridge resolves the cast's
   fn-item operand to a plain `Item` — it does **not** route through
   `resolve_for_fn_ptr`, so it never applies the track_caller→ReifyShim
   adjustment. (This is also a latent correctness gap: the generated
   instance models a direct call to the target, dropping the
   caller-location shim semantics — but track_caller is an edge case in
   `#![no_std]` driver code.)
2. **Const/static reify** is where rustc *does* emit the ReifyShim — as
   `GlobalAlloc::Function(ReifyShim)` provenance in the const value —
   but the bridge rejects it at `crates/regseq-driver/src/bridge/const_.rs:429`
   (`format!("pointer scalar over {alloc:?}")`). This is the
   **C2-family const-fn-ptr-provenance gap** (the sibling site to the
   allocation-bytes rejection at `const_.rs:540`).

**Unblock path** (deferred to the real-fixes phase): accept
`GlobalAlloc::Function` provenance in const translation
(`const_.rs:429` + `:540`), emitting a `Reloc`/`InstanceRef` to the
function instance. That fix would cover FF-004 (the const reify would
surface the ReifyShim instance) **and** close the C2-logged gap
(fn pointers embedded in consts/statics). The gap now has **two
motivations** — FF-004 + the C2 const-fn-ptr shapes — upgrading it from
"future candidate" to a well-motivated real task.

Classification rationale: `blocked-open-bridge-bug` per the matrix
legend (a realisable shape that currently rejects the bridge; a fixture
would be filed as a regseq-driver-NN task) — NOT
`blocked-bridge-rejects-by-design` (we intend to fix it, it is not a
deliberate rejection).

## 2. RJ-024 (`UnsupportedEnumLayout`) → `blocked-rustc-infeasible`

RJ-024 is a *rejection*-coverage row: it wants the bridge to reject with
`UnsupportedEnumLayout`. The three rejection paths are:

- `Scalar::Union` tag — `bridge/ty.rs:1319-1322`
- `Float` tag — `bridge/ty.rs:1368`
- niche-variant out-of-range — `bridge/ty.rs:1445-1448`

These are defensive bridge invariants for enum layouts rustc's layout
solver does not emit from valid `#![no_std]` source: enum discriminants
are integral (so a tag is never a `Float` or `Scalar::Union` primitive),
and niche ranges always contain their variant indices (rustc's niche
construction guarantees it). Probed `enum E { A(bool), B, C }` (niche),
`#[repr(u128)]` with a maximal discriminant, and a `NonZeroU8` niche
squeeze — none reached `UnsupportedEnumLayout` (the type-level layout
translated fine; unrelated body/normalization rejections were observed
on richer bodies, masking nothing about the layout path).

Classification rationale: `blocked-rustc-infeasible` per the matrix
legend (rustc cannot produce the shape from valid `#![no_std]` Rust).

## 3. Disposition

- **B2 RETIRED** from the active escalation queue (LOW probe-or-defer,
  outcome = defer both, no bridge fix, per the proposal's own scope).
- **FF-004 `uncovered` → `blocked-open-bridge-bug`** with the
  const-provenance unblock path recorded.
- **RJ-024 `uncovered` → `blocked-rustc-infeasible`**.
- The **const-fn-ptr-provenance fix** (`const_.rs:429`+`:540`) is added
  to the real-fixes backlog, now motivating both FF-004 and the
  C2-logged gap.
