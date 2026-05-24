# R2 plan: tuple `.clone()` Phase-5 `ItemMissingInstance`

**Status**: PLAN (rev 2, post-Codex-round-1). Drafted 2026-05-24 after a
HEAD-grounded feasibility probe on pinned `nightly-2026-02-01`, then
**substantially revised** after Codex round 1 found a hidden blocker that
invalidated the rev-1 recommendation (Option A). This plan corrects the
HANDOFF's R2 framing (the 4th mis-scope this session after C1/C2/B1) AND
the rev-1 plan's own mis-diagnosis.

---

## 0. Two corrections this plan makes

**(a) HANDOFF mis-scope.** The HANDOFF / B1-retirement audit
(`2026-05-23-b1-cloneshim-retirement.md:95-100`) claimed the orphan is
the *tuple CloneShim*, fixable via F72's lazy-allocation pattern. **False
about the mechanism** (§1): the CloneShim has its instance; the orphan is
the field-type's library Clone impl.

**(b) Rev-1 plan mis-diagnosis.** Rev-1 recommended "Option A: materialise
the CloneShim body so its callees get instances." Codex round 1 + a
direct MIR dump proved this **insufficient**: for `(u32,u32)` the
*optimized* CloneShim MIR `tcx.instance_mir` returns is **copy-only** (no
clone call), so materialising it creates no call edge to the field clone.
Option A is dropped (§3).

## 1. Problem statement (verified at HEAD on pinned nightly-2026-02-01)

Snippet (plain stable `#![no_std]` Rust):

```rust
pub fn f() -> (u32, u32) {
    let t = (1u32, 2u32);
    t.clone()
}
```

Bridge outcome at HEAD (`a62b900`):
`Phase5("bridge-completeness validation failed: ItemMissingInstance { item: ItemId(1), item_kind: Function }")`.
(Unused-result `let _ = t.clone();` fails identically.)

Instrumented dump of the failing model (temporary `validate_bridge_completeness`
probe — Phase-5 rejection returns no model):

| Item | Instances at HEAD |
| ---- | ----------------- |
| `core::clone::Clone::clone` | **1** — `Function/CloneShim` ✅ |
| `core::clone::impls::{impl#15}::clone` (= `<u32 as Clone>::clone`) | **0** ← orphan |
| `<crate>::f` | 1 — `Function/Item` ✅ |

(`{impl#15}` = `<u32 as Clone>::clone` confirmed by a direct
`fn f(x: &u32) -> u32 { x.clone() }` probe, which renders the identical
impl path.)

**Root cause (fully traced + verified):**

1. `f`'s MIR is `_0 = <(u32, u32) as Clone>::clone(move _2)`
   (`--emit=mir`). The bridge canonicalises via `Instance::try_resolve` →
   `InstanceKind::CloneShim(Clone::clone, (u32,u32))` (`mod.rs:2481-2487`,
   the `other` arm) and reactively creates a `CloneShim`-flavored instance
   on the `Clone::clone` item (`terminator.rs:648`). ✓ (`inst[0]`).
2. rustc's mono-collector (`tcx.collect_and_partition_mono_items`,
   `discovery.rs:12`) collects **mentioned items** from pre-optimization
   MIR. The early tuple CloneShim MIR (`MentionedItems` pass) *does* call
   `<u32 as Clone>::clone` (verified: `core.clone-Clone-clone.1-1-000.MentionedItems.after.mir:22,31`),
   so the field clone is a **genuinely collected & emitted mono-item**.
   The bridge interns it as a source `ItemKind::Function` via the
   `MonoItem::Fn` walk (`mod.rs:699-708`).
3. The eager `construct_function_instances` pass cannot instantiate it:
   it is cross-crate, so `source_item_def_id` (local `hir_body_owners`
   scan) returns `None` → `continue` (`mod.rs:1524-1531`); independently
   it is a trait-assoc-fn (`mod.rs:1600-1606`). Either way: no eager
   instance.
4. Its **only** call edge is inside the CloneShim body, which (i) the
   bridge leaves absent (`materialise.rs:246-269`), and (ii) rustc has
   anyway **optimized to a plain field copy** — the runtime-optimized
   shim MIR is `(_0.0: u32) = copy (*_2)` with **no clone call**
   (verified: `core.clone-Clone-clone.3-3-000.runtime-optimized.after.mir`).
   So no reactive call site ever instantiates it → orphan → Phase-5
   `ItemMissingInstance`.

**One-line root cause:** the bridge **interns every collected mono-item
but only instantiates the ones a translated body reaches**. Items whose
sole call edge is optimized away (or lives in an un-materialised shim
body) are interned-but-orphaned. FF-012 (fnptr `.clone()`) works only
because a fnptr CloneShim mentions no sub-items.

**Load-bearing fact for the fix:** the orphan needs only an *instance*,
not a body. A direct `<u32 as Clone>::clone` call probe shows it gets a
`Function/Item` instance with **`body = None`** (cross-crate ⇒ no local
MIR to materialise) and the model **validates OK**. So the fix is "create
the missing instance," not "materialise a body."

## 2. Code-surface grounding

- Orphan check: `regseq-model/src/model.rs:3699-3706` (bucket-1, count==0).
- Collection (mentioned items): `bridge/discovery.rs:12`
  (`collect_and_partition_mono_items`).
- Mono-item interning: `bridge/mod.rs:699-708` (`MonoItem::Fn` walk; the
  resolved rustc `Instance` is in hand at `:701` but the **instance is
  discarded** — only the item is interned).
- Eager instance pass: cross-crate skip `mod.rs:1524-1531`;
  trait-assoc-fn carve-out `mod.rs:1600-1606`.
- Reactive direct-call instance creation: `terminator.rs:648` →
  `lookup_or_create_direct_instance` (`:729`); cross-crate ⇒ body stays
  None at `materialise.rs:171-173` (`local_def_id_for_item` local-only).
- Phase ordering: `run_bridge` `mod.rs:617-660` — instance construction
  (`:630`) → F4-2 sort (`:641`) → completeness check (`:652-658`).

## 3. Fix directions evaluated (with empirical evidence)

### Option A — materialise CloneShim bodies — DROPPED

Rev-1's recommendation. **Insufficient AND unnecessary:**
- *Insufficient*: the optimized tuple CloneShim is copy-only (evidence
  §1.4), so materialising it adds no call edge to the field clone — the
  orphan survives.
- *Unnecessary*: for non-trivially-clonable fields (where the optimized
  shim *would* call the field clone), Option B (below) already
  instantiates the orphan, with no shim handling.
- *Extra cost it would have added*: a DFS-RPO block-reorder, since the
  shim MIR block order is `bb0→bb2→bb1` (non-RPO) and the bridge maps
  block ids 1:1 with no reorder (`terminator.rs:124`) vs the `BRpo`
  invariant (`visit.rs:70`). Moot now.

(Codex confirmed `LangItem::CloneFn` and the CloneShim self-type family
list `instance.rs:256` were correct — but they were only needed for the
dropped Option A.)

### Option B — instantiate the orphaned mono-collected impl method — RECOMMENDED

Give the orphan an `Item`-flavored instance (fn_sig via
`tcx.fn_sig(def_id).instantiate(args)` → `translate_fn_sig`, body `None`
for cross-crate). This is exactly the instance the reactive path already
produces when there *is* a direct caller (§1, load-bearing fact).

Codex round 1 also **retracted** rev-1's objection to this direction:
"instance present but uncalled" is **already a property of the model**
(uncalled `ClosureOnceShim` body:None per `expectations/regseq-driver-72.md:81`;
eager source fns can be uncalled per `mod.rs:1499`). So Option B is *not*
incoherent — reject the *broad* form only for blast radius.

The design question is **where/how broadly** to create the instance:

- **B-i (broad eager)**: instantiate every collected `InstanceKind::Item`
  fn at the `MonoItem::Fn` walk. *Rejected*: huge blast radius (every
  core/std fn the crate touches), model bloat, and may surface new
  body/sig translation gaps across the corpus.
- **B-ii (surgical post-pass) — RECOMMENDED**: stash the collection-time
  resolution `(item, def_id, args)` for collected Item-fns during the
  `MonoItem::Fn` walk (where the resolved `Instance` is already in hand,
  `mod.rs:701`); then, in a new pass after instance construction but
  before the F4-2 sort + completeness check, for each **source bucket-1
  Function item with zero instances**, create its `Item` instance from
  the stashed resolution.
  - **~Zero regression by construction**: a zero-instance bucket-1 source
    item *today* already trips `ItemMissingInstance` and fails the build.
    So on every currently-passing run there are no such orphans → the
    pass is a no-op on the 143-row corpus. It can only fire where the
    build currently fails.
  - Handles trivially- and non-trivially-clonable field tuples uniformly,
    and any other "collected-but-call-edge-optimized-away" orphan, with
    one mechanism. No CloneShim/shim handling, no block-reorder.

### Option C — don't intern shim-only-reachable items — REJECTED

The `MonoItem::Fn` walk has no per-item reachability provenance, and
worse, the orphan is a *genuinely emitted* mono-item (§1.2) — not interning
it would make the model *less* faithful than rustc, not more.

## 4. Proposed scope (Option B-ii)

- **Change A** — `mod.rs` `MonoItem::Fn` walk (`:701-708`): stash the
  resolved `(ItemHandle/def_id, rustc args)` for each collected
  `InstanceKind::Item` source fn into a new `BridgeCtxt` side-table.
  (The resolution is already computed here; only the capture is new.)
  The table preserves the **rustc** `GenericArgsRef` (not yet
  model-translated), since translation needs the post-Phase-4 model.
- **Change B** — new post-construction pass (called from `run_bridge`
  between `construct_instances_and_resolve_cross_references` `:630` and
  `sort_instances_by_f4_2` `:641`): for each source bucket-1
  `ItemKind::Function` item with zero instances, look up its stashed
  resolution and create the instance **identically to the reactive
  path** (Codex round 2 required fix): translate the stashed rustc args
  with the same logic as the `TyKind::FnDef` path (`bridge/ty.rs:258`),
  compute the same `ArgsKey` used by `lookup_or_create_direct_instance`
  (`terminator.rs:761`), build the fn_sig via
  `tcx.fn_sig(def_id).instantiate(args)` → `translate_fn_sig`
  (mirroring `callsite_fn_sig`, `terminator.rs:716`), and push an
  `Item`-flavored Function instance with `body: None`, registering it in
  `function_instance_for_item` under that key. Reusing the exact
  args-translation + keying is what guarantees (a) the orphan's instance
  is identical to the reactive one and (b) the R1
  `validate_unique_instance_keys` backstop sees no spurious duplicate.
  (For the `(u32,u32)` target the field-clone args are empty, but the
  mechanism must be correct for non-empty stashed args too.)
- **Change C** — coverage: author the tuple-clone sidecar row (the R2
  target); confirm FF-012 (fnptr) and the full corpus stay green.

## 5. Acceptance criteria (preview)

1. `(u32,u32).clone()` bridges to `BridgeOutcome::Ok` and validates.
2. The field clone `<u32 as Clone>::clone` carries an `Item` instance
   (`body: None`); the CloneShim instance is unchanged (still body-absent).
3. FF-012 (fnptr `.clone()`) unchanged and passing.
4. Mode A: Total = Pass, Fail = 0, with the new tuple-clone row.
5. The new pass is a no-op on all currently-passing rows (no instance
   count changes outside the orphan being fixed).
6. No change outside the `MonoItem::Fn` capture + the new post-pass + the
   new sidecar row.

## 6. Bounded gate

- **Termination / nesting**: nested tuples (`(u32,(u16,u8))`) collect
  multiple field-clone impls; the post-pass instantiates each orphan
  independently (no recursion). Probe showed the same single failure
  class, no infinite loop. Spec must confirm the pass terminates (finite
  item set) and instantiates *all* orphans in one sweep.
- **Which orphans qualify**: the pass keys off "source bucket-1 Function,
  zero instances." Spec must confirm this set is exactly the
  collected-but-uninstantiated class and excludes Foreign (bucket 2),
  generic (bucket 3), and sig-non-rigid (bucket 4) items, matching the
  validator's own bucketing (`model.rs:3631-3706`).
- **Stashed-resolution lookup misses**: if an orphan has no stashed
  resolution (shouldn't happen — every interned source fn comes from the
  walk), that is a hard error per no-defensive-fallbacks, not a silent
  skip.

## 7. Risks

- **R-1 (load-bearing invariant — Phase-5 / instance table)**: the new
  pass creates instances and registers `function_instance_for_item`
  entries; it runs before the F4-2 sort + the bucket-1 completeness check
  and the R1 `validate_unique_instance_keys` backstop. Per
  [[assessment-soundness-pass]] + the R1 lesson, the assessment **must**
  include a focused Codex soundness pass: confirm the new instances get a
  unique `(item, args_key, Item)` key (no duplicate-key violation), are
  ordered correctly by the F4-2 sort, and don't double-create when a
  later reactive path would also resolve the same key.
- **R-2 (body=None Item instance)**: relies on cross-crate Item instances
  validly carrying `body: None` (verified for u32 clone). Spec must
  confirm this holds for the orphan class generally and that no
  downstream pass assumes Item ⇒ body present.
- **R-3 (blast radius — claimed ~zero)**: the "no-op on passing runs"
  argument must be validated empirically by a full Mode A run showing
  every prior row's instance count unchanged.

## 8. Open questions for Codex (round 2)

1. Is B-ii (surgical post-pass on zero-instance bucket-1 items) the right
   shape, or should the instance be created inline at the `MonoItem::Fn`
   walk (gated so it only adds the orphan class)? Is there a cleaner
   existing hook than a brand-new pass?
2. Is the "~zero regression by construction" argument airtight — can a
   currently-passing run ever contain a zero-instance bucket-1 source
   item that is *intended* to stay instance-less (and would this pass
   wrongly instantiate it)? (Bucket-4 sig-non-rigid is the candidate to
   rule out.)
3. fn_sig for the orphan: is `tcx.fn_sig(def_id).instantiate(stashed_args)
   → translate_fn_sig` the correct/identical sig to what the reactive
   path (`callsite_fn_sig`, `terminator.rs:716`) produces? Any HRTB /
   RustCall ABI hazard for clone-family sigs?
4. Does creating the orphan's instance with `body: None` interact badly
   with anything that later tries to materialise bodies for all Item
   instances (Phase-5 body pass)?
