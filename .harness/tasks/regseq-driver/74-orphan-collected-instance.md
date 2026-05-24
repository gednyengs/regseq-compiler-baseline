# regseq-driver-74 — Instantiate orphaned mono-collected source functions (tuple `.clone()` Phase-5 `ItemMissingInstance`)

## 1. Description

`(u32, u32).clone()` (plain stable `#![no_std]` Rust) fails to bridge at
HEAD with `Phase5: ItemMissingInstance { item, Function }`. The orphan is
**not** the tuple's CloneShim (which is correctly instantiated) — it is
the field-type's library Clone impl, `<u32 as Clone>::clone`
(`core::clone::impls::{impl#15}::clone`), which rustc's mono-collector
collects as a real emitted mono-item but which the bridge interns without
ever instantiating.

Root cause: **the bridge interns every collected mono-item but only
instantiates the ones a translated body reaches.** `<u32 as Clone>::clone`
is collected because the *pre-optimization* tuple CloneShim MIR mentions
it (`collect_and_partition_mono_items` collects mentioned items), but its
sole call edge is inside the CloneShim body, which (a) the bridge leaves
absent and (b) rustc has anyway **optimized to a plain field copy** (the
runtime-optimized `(u32,u32)` CloneShim is `(_0.0: u32) = copy (*_2)`,
with no clone call). So no call site instantiates it → orphan.

Fix: add a surgical post-construction pass that creates the missing
`Item`-flavored Function instance (`body: None`, valid for cross-crate)
for any **source bucket-1 Function item left with zero instances**, using
a resolution stashed during the Phase-2 `MonoItem::Fn` walk (where the
resolved rustc `Instance` is already in hand). The instance is created
**identically to the reactive direct-call path** so it is indistinguishable
from the instance a real caller would have produced.

This is **~zero-regression by construction**: a zero-instance bucket-1
source item *today* trips `ItemMissingInstance` and fails the build, so
the pass is a no-op on every currently-passing run; it can only fire where
the build currently fails.

Full analysis + Codex-reviewed plan (2 rounds): `.harness/audits/2026-05-24-r2-tuple-clone-plan.md`.

## 2. Input Context

### Design source

- `.harness/audits/2026-05-24-r2-tuple-clone-plan.md` — R2 plan, Option
  B-ii (recommended), Codex round 1 (NOT_READY, killed the
  materialise-the-shim approach) + round 2 (APPROVED_WITH_MINOR_FIXES).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md` — `FF-007`
  (`:410`) + `FF-012` (`:415`) prose names tuple `.clone()` as a known
  "FF-007 family" gap with no dedicated row.

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §3.4 (five-phase pipeline,
  Phase 2 mono-item walk, Phase 5 completeness) + §16.2/16.3 (instance
  construction + F4-2 order) + §9.4 (cross-crate / body-absent Item
  instances).

### Bridge code anchors (consumed/modified)

- `crates/regseq-driver/src/bridge/discovery.rs:12` —
  `tcx.collect_and_partition_mono_items(())` (mentioned-items collection).
- `crates/regseq-driver/src/bridge/mod.rs:699-708` — Phase-2
  `MonoItem::Fn` walk. The `instance` binding (`:701`) gives
  `instance.def = InstanceKind::Item(def_id)` (destructured `:702`) and
  `instance.args`; today only `intern_item` is called and the resolution
  is discarded. **(Change A capture site.)**
- `crates/regseq-driver/src/bridge/mod.rs:1290-1295` —
  `def_path_hash(tcx, def_id)`, the bridge's converter to the **model**
  `regseq_model::DefPathHash` that `model.item_def_paths` stores
  (`regseq-model/src/model.rs:81`). **(Stash key + orphan lookup must use
  this, not raw `tcx.def_path_hash`.)**
- `crates/regseq-driver/src/bridge/mod.rs:747-760`, `:804-817` — eager
  HIR enumeration interns source-defined fns regardless of mono
  reachability (so not every interned source fn comes through the
  `MonoItem::Fn` walk — see Change B stash-miss handling).
- `crates/regseq-driver/src/bridge/mod.rs:617-660` — `run_bridge`.
  `construct_instances_and_resolve_cross_references` `:630` (contains
  `run_phase5_body_passes` body materialisation internally), then
  `sort_instances_by_f4_2` `:641`, then `validate_bridge_completeness`
  `:652`. **(Change B insertion point: between `:630` and `:641`.)**
- `crates/regseq-driver/src/bridge/mod.rs:1524-1531` — eager pass skips
  cross-crate items (`source_item_def_id` local-only → `None` →
  `continue`); `:1600-1606` — trait-assoc-fn carve-out. (Why the orphan
  is never eagerly instantiated.)
- `crates/regseq-driver/src/bridge/body/terminator.rs:648` — reactive
  direct-call instance creation; `:716` `callsite_fn_sig`
  (`tcx.fn_sig(def_id).instantiate(generic_args)` → `translate_fn_sig`);
  `:729` `lookup_or_create_direct_instance`; `:761` `(item, args_key,
  flavor)` keying via `args_key_for_generic_args_with_pending`; `:770`
  pushes `body: None` Item instance. **(The exact behaviour Change B must
  replicate.)**
- `crates/regseq-driver/src/bridge/ty.rs:258` — `TyKind::FnDef`
  translation path (`Ty::new_fn_def(tcx, def_id, args)` → `translate_ty`
  → `Ty::FnDef(item, model_args)`); the canonical way to get model args
  identical to the reactive path.
- `crates/regseq-driver/src/bridge/body/materialise.rs:632`
  `translate_model_generic_args`; `:171-173` cross-crate ⇒ `body: None`.
- `crates/regseq-model/src/model.rs:3631-3706` — `validate_bridge_completeness`
  bucketing: bucket-2 foreign `:3645`, bucket-3 generic `:3685`, bucket-4
  sig-non-rigid `:3689`, bucket-1 `ItemMissingInstance` `:3699-3706`.
  Change B's targeting MUST mirror this exactly. `:3731` (≈)
  `validate_unique_instance_keys` / `DuplicateInstanceKey` (R1 backstop).

### Prior tasks

- regseq-driver-72 (F72) — `lookup_or_create_direct_instance` lazy
  allocation for `ClosureOnceShim` (FF-007); precedent for late instance
  creation. NOTE: that pattern does **not** apply here (the orphan is a
  real-body library `Item`, not a body-absent shim).
- regseq-driver-73 (R1) — added `validate_unique_instance_keys`. Change B
  must not introduce a duplicate key.

### Test surface

- New: `crates/regseq-driver/tests/bridge_orphan_collected_instance.rs`.
- New matrix row `FF-016` + sidecar row + `snippets/FF-016.rs`.
- Mode A runner over the full corpus.

## 3. Implementation Notes

### Change A — capture the collection-time resolution

In the Phase-2 `MonoItem::Fn` walk (`mod.rs:699-708`), for each collected
`InstanceKind::Item(def_id)` source fn (the existing `is_source_derived_function`
gate is unchanged), record `(def_id, instance.args)` into a new
`BridgeCtxt` side-table keyed by the **model** `DefPathHash` produced by
the bridge's converter `def_path_hash(tcx, def_id)` (`mod.rs:1290-1295`),
so the key type matches `model.item_def_paths[id]`:

```text
collected_item_resolutions: HashMap<regseq_model::DefPathHash, (DefId, ty::GenericArgsRef<'tcx>)>
```

The table holds the **rustc** `GenericArgsRef` (not model-translated —
translation needs the post-Phase-4 model). `instance.args` is the correct
rustc args (the reactive canonicalization uses the same `instance.def.def_id()`
+ `instance.args` for Item callees at `mod.rs:2475-2478`). Only the
capture is new. Generic items (bucket-3) may write multiple
monomorphisations to the same key; that is harmless because Change B only
reads the table for **non-generic** orphans (unique resolution).

### Change B — orphan-instantiation post-pass

Add `instantiate_orphaned_collected_functions(ctxt)` and call it from
`run_bridge` **after** `construct_instances_and_resolve_cross_references`
(`:630`) and **before** `sort_instances_by_f4_2` (`:641`). At this point
all reactive instances (created during `run_phase5_body_passes`) exist and
item IDs are final.

For each model `ItemId` whose item is a **source bucket-1 Function** —
i.e. mirroring the validator's classification exactly:
- `ItemKind::Function`,
- `item_synthetic_kinds[id] == SyntheticItemKind::None`,
- `linkage != Linkage::Foreign` (exclude bucket-2),
- `!item_bridge_flags[id].is_generic` (exclude bucket-3),
- `!(item_bridge_flags[id].signature_non_rigid)` (exclude bucket-4),
- and **zero** instances currently reference it —

create its instance by **calling the reactive helper itself** (do NOT
re-implement the push — the helper at `terminator.rs:729` already does
the `ArgsKey` keying (`:761`), the `ensure_cabi_lowering_slots` call
(`:766-767`), and the `body: None` Item push (`:770-782`); re-implementing
risks omitting a step, e.g. the cabi-slot setup):

1. Look up `collected_item_resolutions[model.item_def_paths[id]]`.
2. Translate the FnDef ty to model args via the canonical path:
   `Ty::new_fn_def(tcx, def_id, rustc_args)` → `translate_ty` →
   destructure `Ty::FnDef(callee_item, model_args)` (matches `ty.rs:258`
   and the reactive `terminator.rs:620-621`). `callee_item == id`.
3. Call
   `lookup_or_create_direct_instance(cx, callee_item, model_args,
   FunctionInstanceFlavor::Item, None, |cx| callsite_fn_sig(cx, def_id,
   rustc_args))` — the same helper, flavor, and sig closure
   (`callsite_fn_sig`, `terminator.rs:716`) the reactive direct-call path
   uses. (`lookup_or_create_direct_instance` is already `pub(crate)`
   (`terminator.rs:729`); `callsite_fn_sig` is currently **private**
   (`terminator.rs:716`) and must be made `pub(crate)` — or an equivalent
   `pub(crate)` sig helper added — so the new pass in `mod.rs` can reach
   it. This visibility bump is the only API change.)

This guarantees the instance is identical to what a real caller would
have produced and that the R1 `validate_unique_instance_keys` backstop
sees no spurious duplicate (the `(id, args_key, Item)` key is unique
because the item had zero instances; `pending_def_path_hashes` is empty
this late per `mod.rs:267-280`, so the `ArgsKey` is stable).

**Stash-miss handling (Codex round-1 spec fix #2).** The assumption "every
interned source fn came through the `MonoItem::Fn` walk" is **false**: the
eager HIR enumeration (`mod.rs:747-760`, `:804-817`) interns source fns
regardless of mono reachability. So a bucket-1 zero-instance orphan could
be a **local fn that was eagerly interned but never mono-collected** (and
thus not stashed) — e.g. an uncalled local trait-impl/default method that
slips the eager `construct_function_instances` trait-assoc-fn carve-out
(`:1600-1606`). Such an item **already fails the build today** (bucket-1,
zero instances ⇒ `ItemMissingInstance`), so it is a **separate
pre-existing orphan class, out of R2 scope**. On a stash miss, emit a
**clear hard `BridgeError::Phase5`** naming the unresolved item (per
no-defensive-fallbacks — surfaced explicitly, never silently skipped).
This is never a regression: the build was already failing for that item.
R2's target orphan (cross-crate field-clone, which is **always**
mono-collected because that is the only reason it is interned) is always
stashed and always resolves.

### Change C — coverage row

- New matrix row `FF-016` (next free id; verify at implementation time)
  in `bridge-coverage-matrix.md`: positive, covered, the tuple-`.clone()`
  CloneShim-with-collected-field-clone shape, citing this task.
- New sidecar row `FF-016` in `matrix_metadata/rows.toml` + snippet
  `snippets/FF-016.rs` = the §1 tuple-clone snippet. The runner's
  inspection predicates are **coarse** (`FunctionInstanceFlavor` matches
  any instance of that flavor — `inspect.rs:629-642`; `BodyAbsent` matches
  any absent body — `:971-982`) and the diff scope **forbids runner
  changes**, so the row uses a coarse predicate: positive, expects
  `BridgeOutcome::Ok` with `FunctionInstanceFlavor=CloneShim` present
  (the same predicate FF-012 uses). The discriminating signal is that the
  row reaches `Ok` **at all** — at HEAD this snippet fails Phase-5
  `ItemMissingInstance`. The **exact** field-clone-instance proof
  (def-path `::clone`, flavor `Item`, `body: None`) lives in the
  integration tests (§5 B1), not the sidecar predicate.

## 4. Non-Goals

- **Do NOT materialise CloneShim (or any shim) bodies.** The CloneShim
  instance stays body-absent exactly as today; FF-012 (fnptr clone) must
  be unchanged. (Codex round 1 proved shim-body materialisation is
  insufficient — the optimized tuple shim is copy-only.)
- **Do NOT broaden instantiation to all collected Item fns** (the rejected
  B-i). Only zero-instance bucket-1 source items are touched.
- **Do NOT change the eager `construct_function_instances` carve-outs**
  (`mod.rs:1524-1531`, `:1600-1606`). The new pass is additive and runs
  later.
- **Do NOT touch `validate_bridge_completeness` bucketing** — Change B
  consumes the same classification, it does not alter it.
- No DFS-RPO block reordering (no shim bodies are materialised, so the
  `BRpo` invariant is not engaged).
- No handling of the separate used-closure-`.clone()` rustc ICE (that is
  R3).

## 5. Test Plan

Integration tests in `bridge_orphan_collected_instance.rs` (helper
pattern from `bridge_const_fnptr_provenance.rs` — temp-file snippet →
`try_bridge_fixture_path`):

- **B1 (target, positive)**: `(u32,u32).clone()` returns `BridgeOutcome::Ok`
  and validates. Assert the field-clone item (`def_path` ends `::clone`,
  not the `Clone::clone` CloneShim owner) carries exactly one
  `Function/Item` instance with `body == None`.
- **B2 (CloneShim unchanged)**: in the same model, assert the
  `Clone::clone` item still carries its `Function/CloneShim` instance
  (the fix added the field instance, did not disturb the shim).
- **B3 (nested)**: `(u32,(u16,u8)).clone()` returns `Ok` + validates
  (multiple field-clone orphans instantiated in one sweep; pass
  terminates).
- **B4 (regression — fnptr)**: the FF-012 snippet
  (`fn() .clone()`) still returns `Ok`; CloneShim instance unchanged,
  no new `Item` instance spuriously added.
- **B5 (clean crate)**: a crate with no orphans (e.g.
  `pub fn f(x: u32) -> u32 { x + 1 }`) bridges `Ok` and validates (the
  pass is inert when no bucket-1 item is orphaned). (Global no-regression
  evidence comes from B6's full-corpus comparison, which is the recorded
  baseline — there is no per-test HEAD baseline to diff against.)
- **B6 (Mode A — the regression baseline)**: full runner over the
  committed corpus + the new `FF-016` row reports `Total == Pass`,
  `Fail == 0`, exit 0; **every pre-existing row's outcome is unchanged**
  vs HEAD (re-run Mode A at HEAD and after the fix; diff the per-row
  results — this is the concrete no-regression check).

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/74-orphan-collected-instance.md` (this spec)
- `.harness/audits/2026-05-24-r2-tuple-clone-plan.md`
- The diff (Changes A/B/C) once implemented; for spec review, the anchors
  in §2 suffice (Codex probes the repo).

### Reviewer focus

1. Does Change B's targeting predicate mirror `validate_bridge_completeness`'s
   bucket-1 classification **exactly** (model.rs:3631-3706), so it can
   never instantiate a bucket-2/3/4 item nor miss a real bucket-1 orphan?
2. Are the stashed-args translation + `ArgsKey` + fn_sig **identical** to
   the reactive path (`terminator.rs:716/761`, `ty.rs:258`), so no
   `DuplicateInstanceKey` and no divergent sig?
3. Is the insertion point (after body materialisation inside
   `construct_instances...`, before sort + completeness) correct, so the
   `body: None` instance is never re-materialised and never double-created
   by a later reactive path?
4. Is the "~zero regression by construction" property airtight for the
   full corpus (every prior row's instance count unchanged)?
5. Any orphan class that is NOT a clone-family method (e.g. some other
   trait-impl method or free fn collected-but-call-edge-optimized-away)
   that this pass would now newly instantiate — and is that correct?

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-74 (cwd = repo root). The
fix adds (A) a collection-time resolution stash at the MonoItem::Fn walk
and (B) a post-construction pass that instantiates source bucket-1
Function items left with zero instances, identically to the reactive
direct-call path. Verify correctness against the anchors in the spec
§2 and the plan at .harness/audits/2026-05-24-r2-tuple-clone-plan.md.
Out of scope: redesigning the approach (plan is Codex-approved); judge the
implementation.
</task>
<grounding_rules>
Cite file:line for every claim. Read the diff + cited code; do not
speculate. You may run cargo/the Mode A runner.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
BUCKET_TARGETING_EXACT: PASS|FAIL + cite
KEY_AND_SIG_IDENTICAL_TO_REACTIVE: PASS|FAIL + cite
ZERO_REGRESSION_VERIFIED: PASS|FAIL + cite (Mode A result)
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
- `cargo clippy -p regseq-driver -p bridge-coverage-runner --all-targets -- -D warnings` clean.
  (regseq-model test-target clippy has 23 PRE-EXISTING failures unrelated
  to this task — see R1 assessment; do not be blocked by them, but do not
  add new ones.)
- `cargo test -p regseq-driver -- --test-threads=1` green (incl. the new
  `bridge_orphan_collected_instance.rs`).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with the new `FF-016` row.
- Diff scope confined to: `bridge/mod.rs` (Change A capture + Change B
  pass + `run_bridge` wiring), `bridge/body/terminator.rs` (the single
  `callsite_fn_sig` visibility bump to `pub(crate)`), the new test file,
  the matrix row, the sidecar row + snippet. No edits to
  `validate_bridge_completeness`, the eager-pass carve-outs, or any shim
  materialisation path.

## 8. Acceptance Criteria

1. `(u32,u32).clone()` bridges to `BridgeOutcome::Ok` and `validate()`
   succeeds.
2. The field-clone library impl (`<u32 as Clone>::clone`) carries exactly
   one `Function/Item` instance with `body: None`.
3. The `Clone::clone` CloneShim instance is unchanged (still present,
   still body-absent).
4. Nested tuple `(u32,(u16,u8)).clone()` bridges `Ok` + validates.
5. FF-012 (fnptr `.clone()`) is unchanged and passing — no new instance
   added to its model.
6. A clean crate with no orphans bridges `Ok` + validates (pass inert);
   global no-regression is evidenced by AC9's per-row Mode A diff.
7. Change B's targeting mirrors the validator's bucket-1 classification;
   bucket-2/3/4 items are never instantiated by it.
8. The new instance's `(item, args_key, Item)` key matches what the
   reactive path would produce; no `DuplicateInstanceKey`.
9. Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `FF-016` covered;
   every pre-existing row's outcome unchanged.
10. `cargo build` / `fmt` / `clippy` (regseq-driver + bridge-coverage-runner)
    / `cargo test -p regseq-driver` all green.
