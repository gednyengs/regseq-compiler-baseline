# regseq-driver-47: F4 Ordering Across Phase-5 Late Items + Global `ItemId` Remap (Phase 3/4 Contract Fix)

## Description

Close the spec.md §3.4 Phase 3/4 + §16 F4 contract gap surfaced by the
regseq-driver-45 audit-fix sequence. The bridge's
`assign_late_pending_items`
(`crates/regseq-driver/src/bridge/assign.rs:19-21`) drains the
late-Phase-5 `pending_items` produced by body-materialisation
synthesis (per the comment at
`crates/regseq-driver/src/bridge/mod.rs:1003-1011`) and `push`es them
onto the end of `model.items` via the shared
`assign_pending_items` (`assign.rs:23-35`). The late batch is
pre-sorted only against itself (`mod.rs:1009`:
`sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)`), not merged
into the already-assigned items' F4 order. Any late item whose F4
sort key
(`crates/regseq-model/src/model.rs::compare_items_f4`
at `model.rs:913-931`) sorts *before* any already-assigned item's
breaks `InvariantId::ItemsSortedAtConstruction`
(`model.rs:762-768`) and `model.validate()` trips
`InvariantViolation::F4OutOfOrder` (`model.rs:234-240`).

For the `diag_logging` fixture, end-to-end probing (during the
regseq-driver-45 audit) localised the violation to:

- `ItemId(10) = diag_logging::emit_logs`, terminal segment `ValueNs`
  (sort rank 6 per spec.md §16.1).
- `ItemId(11) = diag_logging::<regseq_synth>::const_memory_static…`,
  terminal segment `TypeNs` (sort rank 5).

Both items resolve to `Item.def_path.crate_id == cx.model.primary_crate`
because `synthesise_const_memory_static`
(`crates/regseq-driver/src/bridge/const_.rs:158-170`) builds the
DefPath under the primary crate. The "synthetic crate" tag lives on
`item_def_paths[ItemId]` via `synthetic_def_path_hash`
(`synthetic_def_path.rs:130-132`); F4 sort does not consult it.
`TypeNs(5) < ValueNs(6)` per `def_path_data_sort_rank` (spec.md
§16.1 / `crates/regseq-model/src/model.rs:1495-1505`), so the late
`<regseq_synth>` segment should sort before `emit_logs` but is
appended after it → F4 violation.

This task lands a `sort_items_by_f4(&mut self)` method on
`RegseqModel` that re-sorts `model.items` and remaps every
`ItemId` reference throughout the model so the terminator /
type / instance / side-table graph survives the permutation;
wires it into the bridge pipeline after
`assign_late_pending_items`; and adds regression coverage from
a fixture that reproduces the late-item-sorts-early shape.

Mirrors the pattern of the existing
`sort_instances_by_f4_2` (`model.rs:933-1044`), extended to the
wider Item reference surface (every `Ty::Enum/Struct/Union/FnDef/
Closure`, every `Instance.item`, every `HandleTable` binding,
every `IndexVec<ItemId, _>` side table, every
`HashMap<_, ItemId>` value, every `HashMap<ItemId, _>` key).

This task is **F4-1 items-side only**. F4-2 instances-side
ordering is already handled by `sort_instances_by_f4_2`; this
task does not touch it.

## Input Context

- `docs/discussions/regseq-model/spec.md` §3.4 (bridge
  construction-pipeline; especially Phase 3 F4 SORT at
  `spec.md:1198-1206` and Phase 4 ItemId ASSIGNMENT at
  `spec.md:1208-1213`)
- `docs/discussions/regseq-model/spec.md` §16.1 (F4 sort
  comparator + `def_path_data_sort_rank` table)
- `docs/discussions/regseq-model/spec.md` §16.2 (F4-2 instance
  sort, for the analogous pattern this task extends to items)
- `crates/regseq-model/src/model.rs` (`compare_items_f4` at
  `model.rs:913-931`; `sort_instances_by_f4_2` at
  `model.rs:933-1044`; `InvariantViolation::F4OutOfOrder` at
  `model.rs:234-240`; `validate_items` F4 monotonicity check at
  the §16-cited block)
- `crates/regseq-model/src/index_vec.rs` (IndexVec
  reorder-by-permutation helper at `index_vec.rs:77+`)
- `crates/regseq-driver/src/bridge/mod.rs`
  (`run_bridge` pipeline at `mod.rs:574-602`;
  `construct_instances_and_resolve_cross_references` at
  `mod.rs:990-1013`)
- `crates/regseq-driver/src/bridge/assign.rs`
  (`assign_late_pending_items` at `assign.rs:19-21`;
  `assign_pending_items` at `assign.rs:23-35`)
- `crates/regseq-driver/src/bridge/sort.rs` (`f4_sort` at
  `sort.rs:1-15`)
- `crates/regseq-driver/src/bridge/const_.rs`
  (`synthesise_const_memory_static` at `const_.rs:158-193` --
  the main late-item producer this task's fixture exercises)
- `crates/regseq-driver/src/bridge/ty.rs` (`Ty::Enum/Struct/
  Union/FnDef/Closure` variants that carry `ItemId` and need
  remapping; type-interner table at `ty.rs:255-306`)
- `crates/regseq-driver/src/bridge/handles.rs` -- HandleTable
  module (`handles.rs:22-55`), re-exported from `bridge/mod.rs:29-30`;
  handle-to-ItemId bindings established by
  `HandleTable::bind_final_item_id` (consumed at `assign.rs:33`)
- `crates/regseq-model/src/instance.rs` (`Instance.item` at
  `instance.rs:136`)
- `crates/regseq-model/src/model.rs` for `RegseqModel.vtables`
  (`HashMap<PrincipalTraitKey, ItemId>`) and
  `closure_captures_struct_ty`
  (`HashMap<TyId, ItemId>` per the model struct comment)
- `.harness/tasks/regseq-driver/45-diag-log-peel-and-format-grammar.md`
  (the audit-fix sequence that surfaced this gap)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent
  brief; this task is a phase-1a bridge follow-up addition
  beyond the original BR-01..BR-08b table)

## Implementation Notes

Three coupled changes. The sort + remap landing on the model
side (Change A) is the load-bearing change; the bridge
pipeline wiring (Change B) and the test fixture (Change C) are
the surrounding contract.

### Change A: `RegseqModel::sort_items_by_f4` + global `ItemId` remap

This change lands two coupled methods on `RegseqModel`: a
private shared `ItemId`-reference walker (consumed by both
this task and `regseq-driver-48`) and the public sort method
that uses it.

#### A.1: Shared `pub(crate) fn for_each_item_id_mut`

Add a `pub(crate)` reference walker on `RegseqModel`
(`crates/regseq-model/src/model.rs`) that takes a
`FnMut(&mut ItemId)` closure and applies it to every
location in the model that holds an `ItemId`. Single source
of truth for the reference surface so any future Item
mutation (this task's sort, `regseq-driver-48`'s removal,
any future Item-permuting pass) re-uses one walker:

```rust
/// Apply `f` to every `ItemId` reference in the model. Visits:
/// - Type-interner nominal entries: `Ty::Enum/Struct/Union/
///   FnDef/Closure { item, .. }` and any transitively
///   reached `Ty` inside `GenericArgs`.
/// - `Instance.item` for every instance.
/// - Side-table keys: `domain_attrs.field_attrs`'s
///   `FieldPath.item`.
/// - Side-table values: `vtables`'s `HashMap<_, ItemId>`
///   values.
///
/// Does NOT visit `IndexVec<ItemId, _>` side tables --
/// those are positional and must be permuted (not remapped)
/// by callers via the shared permutation table.
///
/// Does NOT visit bridge-side `HandleTable` --
/// `HandleTable` lives on the driver side
/// (`crates/regseq-driver/src/bridge/handles.rs:22-55`,
/// re-exported at `crates/regseq-driver/src/bridge/mod.rs:29-30`)
/// and is not part of `RegseqModel`. Bridge-side callers
/// apply the same remap separately via
/// `HandleTable::remap_item_ids`.
pub(crate) fn for_each_item_id_mut<F>(&mut self, mut f: F)
where
    F: FnMut(&mut ItemId),
{ /* ... */ }
```

The walker implementation enumerates the same surface that
Implementation Notes A.2 step 5-8 enumerate; lifting it to
this single helper is what makes the surface a one-source-
of-truth contract.

#### A.2: Public `sort_items_by_f4`

Add a new method on `RegseqModel`
(`crates/regseq-model/src/model.rs`, alongside
`sort_instances_by_f4_2` at `model.rs:933-1044`):

```rust
/// Sort `model.items` by `compare_items_f4` per spec.md §16.1,
/// remapping every `ItemId` reference everywhere it appears in
/// the model so the terminator / type / instance / side-table
/// graph survives the permutation. Idempotent: a model already
/// in F4 order stays unchanged.
///
/// Mirrors `sort_instances_by_f4_2` (§16.2 / `model.rs:933+`)
/// for the items-side ordering. The instance-side method exists
/// because Phase-5 sub-passes push Function and Static instances
/// in distinct orders; this items-side method exists because
/// `assign_late_pending_items`
/// (`crates/regseq-driver/src/bridge/assign.rs:19+`) appends
/// late synthesis items past the original F4-sorted batch,
/// which can interleave them out of order with the existing
/// items per spec.md §16.1 ranking.
///
/// Audit-finding regseq-driver-45-extension (see
/// `.harness/tasks/regseq-driver/47-...md`).
///
/// Returns the old-to-new ItemId remap table
/// (`Vec<ItemId>` indexed by old ItemId). Callers MUST apply
/// the same remap to any bridge-side state that holds
/// ItemIds outside the model (e.g.,
/// `HandleTable.handle_to_item_id` per Change B step 10).
pub fn sort_items_by_f4(&mut self) -> Vec<ItemId> { /* ... */ }
```

The implementation:

1. **Compute the new permutation.** Build
   `(old_index, new_index)` pairs by sorting a `Vec<ItemId>` by
   `compare_items_f4(&items[a], &items[b])`. Stable sort so
   ties on `DefPath` retain insertion order (spec.md §16.1
   ties-admitted clause).

2. **Permute `model.items` in place.** Apply the permutation
   via the IndexVec reorder helper at `index_vec.rs:77+`.

3. **Permute every `IndexVec<ItemId, _>` side table in lock-step:**
   - `model.item_def_paths`
   - `model.item_synthetic_kinds`
   - `model.domain_attrs.item_attrs`
   - Any other `IndexVec<ItemId, _>` discovered during scan.

4. **Build the old→new ItemId remap table** (`Vec<ItemId>`
   indexed by old ItemId).

5. **Walk the type interner and remap every embedded
   `ItemId`:**
   - `Ty::Enum { item, .. }`
   - `Ty::Struct { item, .. }`
   - `Ty::Union { item, .. }`
   - `Ty::FnDef { item, .. }` (or whichever Ty variant carries
     FnDef's ItemId per the existing interner shape; verify
     against `ty.rs:255-306` and the model's Ty enum at
     `crates/regseq-model/src/ty.rs:220+`)
   - `Ty::Closure { item, .. }` if present
   - Any nested ItemId references reached transitively through
     `GenericArgs` (the type interner re-interns content-keyed,
     so this likely requires building a fresh interner from the
     remapped Ty values).

6. **Walk `model.instances` and remap `Instance.item`:**
   - `for instance in model.instances.iter_mut() { instance.item =
     remap[instance.item]; }`.

7. **Walk `model.vtables`** (the
   `HashMap<PrincipalTraitKey, ItemId>` per the §3.5.7 struct
   comment): remap every value via the same table.

8. **Walk `model.domain_attrs.field_attrs`**
   (`HashMap<FieldPath, Vec<AttrData>>` per
   `crates/regseq-model/src/domain_attr.rs:27`). `FieldPath`
   embeds an `ItemId` at its `.item` field
   (`domain_attr.rs:39-41`); rebuild the HashMap with remapped
   keys (HashMap key mutation requires a re-insert path:
   drain, remap each key's `.item`, re-insert).

   NOTE: `model.closure_captures_struct_ty` is
   `HashMap<TyId, TyId>` (`crates/regseq-model/src/model.rs:103`)
   -- TyIds only, no ItemId. Not part of this remap.

9. **Body-level terminator + statement walk: NOT needed for
   ItemId remap.** `CallCallee::Direct` carries `InstanceId`
   (`crates/regseq-model/src/terminator.rs:113-115`), not
   `ItemId`; `AggregateKind` and `CastKind` carry `TyId`s
   (`crates/regseq-model/src/rvalue.rs:45-72,183-215`), not
   `ItemId` directly. Body-level ItemId references reach the
   Item table only through the type interner, which is
   already remapped in step 5. (Contrast `sort_instances_by_f4_2`
   at `model.rs:946-1044`, which DOES walk terminators
   because it remaps `InstanceId` -- a different concern.)

10. **Remap `HandleTable.handle_to_item_id` in lock-step.**
    Bridge-side handles bind to ItemIds at Phase 4 (`assign.rs:33`).
    `construct_const_memory_static_instances`
    (`crates/regseq-driver/src/bridge/const_.rs::203-211`) is the
    only Phase-5 step that consults `handles.resolve` *after*
    this task's sort fires. Without a HandleTable remap, that
    resolution would return stale pre-sort ItemIds and corrupt
    the ConstMemoryStatic Instance wiring. Add a
    `HandleTable::remap_item_ids(&mut self, remap: &[ItemId])`
    method (the `HandleTable` lives in
    `crates/regseq-driver/src/bridge/handles.rs:22-55`,
    re-exported at `crates/regseq-driver/src/bridge/mod.rs:29-30`)
    that walks
    `handle_to_item_id` and rewrites every bound entry through
    the remap table. Call it from `sort_items_by_f4`'s caller
    in the bridge wiring (Change B); the model-side
    `sort_items_by_f4` itself does not touch the HandleTable
    (it lives on the driver side), so the caller composes the
    two updates.

11. **Re-verify F4 monotonicity in debug builds.** After the
    permutation, optionally walk adjacent items and assert
    `compare_items_f4(&items[i], &items[i+1]) != Greater` so a
    bug in the remap surfaces immediately rather than at the
    validator.

### Change B: Bridge pipeline wiring

In `crates/regseq-driver/src/bridge/mod.rs::construct_instances_and_resolve_cross_references`
(`mod.rs:990-1013`), insert the sort + HandleTable remap pair
immediately after the late-pending drain at `mod.rs:1008-1011`
and BEFORE the call to
`const_::construct_const_memory_static_instances`
(`mod.rs:1012`). Pseudocode:

```rust
if !ctxt.pending_items.is_empty() {
    sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)?;
    assign::assign_late_pending_items(ctxt);
}
let item_remap = ctxt.model.sort_items_by_f4();
ctxt.handles.remap_item_ids(&item_remap);
const_::construct_const_memory_static_instances(ctxt)?;
```

The unconditional sort call (rather than gating on the
late-pending drain firing) is intentional: idempotency makes
the cost negligible for fixtures with no late items, and the
unconditional call provides forward safety if a future bridge
addition introduces another late-item path. The
`HandleTable::remap_item_ids` call directly after the model
sort closes the stale-handle window that
`construct_const_memory_static_instances`
(`crates/regseq-driver/src/bridge/const_.rs:203-211`) would
otherwise hit when it consults `handles.resolve` for the
ConstMemoryStatic Item handles assigned in the late-pending
drain.

Add the new method
`HandleTable::remap_item_ids(&mut self, remap: &[ItemId])` in
`crates/regseq-driver/src/bridge/handles.rs:22-55`
alongside `HandleTable::bind_final_item_id` (consumed at
`assign.rs:33`). It walks
the internal `handle_to_item_id` table and rewrites each bound
entry through the remap. Per the model-side method's contract,
`remap[old_idx]` is the new `ItemId` for the item formerly at
`old_idx`.

The existing `ctxt.model.sort_instances_by_f4_2()` at
`mod.rs:599` runs *after*
`construct_instances_and_resolve_cross_references` returns
and is unaffected by this task (instance ordering is keyed by
ItemId among other components, so the items sort must run
first -- the current placement preserves that ordering).

### Change C: Regression test fixture

New integration test at
`crates/regseq-driver/tests/bridge_f4_late_item_reorder.rs`.
The fixture must satisfy: at least one Phase-2 source-derived
Item with a `ValueNs`-leaf segment whose F4 key would naturally
sort *after* a Phase-5 late-synthesised ConstMemoryStatic Item
in the same crate (the `diag_logging` shape).

Minimal reproducer (per the diag_logging probe):

```rust
#![no_std]
// Inline shim to keep the fixture standalone-compilable; mirror
// the pattern in crates/test-ir-fixtures/fixtures/diag_logging.rs
// for the regseq::log facade, but trim to ONE severity to keep
// the test focused on the F4 surface.
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> { U32(u32), Str(&'a str) }
            impl<'a> From<u32> for LogArg<'a> {
                fn from(v: u32) -> Self { LogArg::U32(v) }
            }

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {
                unimplemented!()
            }
        }
        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[ $( $crate::regseq::log::intrinsics::LogArg::from($arg) ),* ],
                )
            };
        }
        pub(crate) use info;
    }
}

pub fn emit(x: u32) {
    regseq::log::info!("x={}", x);
}
```

This fixture forces Phase-5 to synthesise a `ConstMemoryStatic`
Item for the `"x={}"` literal under `cx.model.primary_crate`
with a `<regseq_synth>` `TypeNs` segment that sorts before
`emit`'s `ValueNs` leaf. Without this task's fix, the bridge
produces a model whose `validate()` returns
`InvariantViolation::F4OutOfOrder`.

The test must:

1. Bridge the fixture via `run_bridge` (NOT through the
   `test-ir-fixtures` helper, which invokes `validate()` and
   panics on the current code path; the test instead calls
   `model.validate()` directly so the pre-fix failure mode is
   diagnostic rather than a panic).
2. Pre-fix: assert `model.validate()` returns
   `Err(InvariantViolation::F4OutOfOrder { .. })`. This locks
   in the bug for one CI cycle so future regressions surface.
   Once the fix lands, flip to `assert!(model.validate().is_ok())`
   and remove the pre-fix assertion.
3. Post-fix: assert `model.validate()` returns `Ok(())`. Walk
   `model.items` and assert adjacent F4 monotonicity holds via
   `compare_items_f4`.

### Conventions

- Doc comments on every new public item (per `SESSION.md`
  Coding Conventions).
- No unicode symbols in source / docs.
- The new integration test follows the same `compile_with_bridge`
  harness pattern as `crates/regseq-driver/tests/bridge_pipeline.rs`.
- All assertions cite the `spec.md` section they verify in a
  one-line comment so the Codex review can cross-reference.

## Non-Goals

- **F4-2 instance reordering.** Already handled by
  `sort_instances_by_f4_2` (`model.rs:933+`). This task does
  not modify that surface.
- **Dead-Item elimination.** Owned by `regseq-driver-48`
  (P1a-BR-11). After F4-1 lands, `validate()` succeeds but the
  From-impl Items still survive; that's the next task's
  territory.
- **Generalised HandleTable mid-pipeline ItemId remapping.**
  This task DOES remap `HandleTable.handle_to_item_id` after
  the sort (Change A step 10 + Change B's
  `HandleTable::remap_item_ids` call) -- that is load-bearing
  because `construct_const_memory_static_instances`
  (`crates/regseq-driver/src/bridge/const_.rs:203-211`)
  consults `handles.resolve` AFTER the sort fires. What is
  out of scope: generalising HandleTable to support
  arbitrary mid-pipeline ItemId mutation by other future
  passes. The new `remap_item_ids` method is specifically
  scoped to this task's post-F4-sort window and
  `regseq-driver-48`'s post-DCE window (where it composes
  the same way).
- **Alternative Phase 4 restructuring (Option B from the
  audit).** Deferring Phase 4 until all pending_items
  accumulate (including Phase-5 late additions) would also
  fix the gap but restructures the pipeline order at
  `mod.rs:585-601` more invasively. Chosen approach (Option
  A: re-sort + remap) is the targeted surgical change.
- **The regseq-model-94 E2E test.** Once this task lands,
  `validate()` succeeds on the diag_logging fixture. The
  remaining `regseq-model-94` blocker (LogArg surviving in
  From-impl bodies) is owned by `regseq-driver-48`.

## Test Plan

### Unit tests (`crates/regseq-model/src/model.rs::mod tests` or a sibling `model_sort.rs`)

1. **Idempotency.** Construct a model with two items already
   in F4 order. Call `sort_items_by_f4`. Assert `model.items`
   and every remapped side table are byte-identical to
   pre-sort.

2. **Two-item swap.** Construct a model with two items in
   reverse F4 order. Call `sort_items_by_f4`. Assert
   `model.items` are now in F4 order and that an `Instance`
   referencing each item by `ItemId` has its `Instance.item`
   correctly remapped to the new index.

3. **`Ty::Enum`/`Struct`/`Union`/`FnDef` ItemId remap.**
   Construct a model with two items + a Ty::Enum referencing
   item A and a Ty::FnDef referencing item B, in reverse F4
   order. Call `sort_items_by_f4`. Assert the type interner's
   resolution of the Ty handles now points at the new ItemId
   positions for A and B.

4. **`model.vtables` ItemId-value remap.** Construct a
   `vtables: HashMap<PrincipalTraitKey, ItemId>` entry and
   sort with the keyed item in reverse position. Assert the
   HashMap's value points at the remapped ItemId.

5. **`IndexVec<ItemId, _>` side-table permutation.** Construct
   a model with three items and three distinct
   `item_synthetic_kinds` entries. Assert post-sort that
   `item_synthetic_kinds[new_idx_of_old_X] ==
   original_synthetic_kind_for_X` for each X.

6. **`domain_attrs.field_attrs` HashMap key remap.** Construct
   a model with two items + a `FieldPath { item: ItemId(1),
   variant: VariantIdx::from_u32(0), field: FieldIdx::from_u32(0) }`
   key (matching the actual `FieldPath` shape at
   `crates/regseq-model/src/domain_attr.rs:39-46`) in
   `domain_attrs.field_attrs` mapped to a non-empty
   `Vec<AttrData>`. Sort items into a new order such that
   `ItemId(1)` permutes to `ItemId(0)`. Assert the
   `field_attrs` HashMap now contains the same value under
   `FieldPath { item: ItemId(0), variant: VariantIdx::from_u32(0),
   field: FieldIdx::from_u32(0) }` and the old key is gone.

7. **`HandleTable::remap_item_ids` lock-step.** Bridge-side
   unit test in `crates/regseq-driver/src/bridge/mod.rs` (or
   the existing `bridge_pipeline.rs` test harness): bind two
   handles to two ItemIds via `HandleTable::bind_final_item_id`;
   construct the matching ItemId remap table (swap them); call
   `HandleTable::remap_item_ids(&remap)`. Assert `handles.resolve`
   for each handle returns the new ItemId.

8. **Debug-build F4-monotonicity post-sort assertion.** Invoke
   the post-sort debug check (per Change A step 11) and
   assert it does not panic on a correctly-sorted model.

### Integration test (`crates/regseq-driver/tests/bridge_f4_late_item_reorder.rs`)

9. **`diag_logging`-shape fixture validates post-sort.** Per
   Change C: bridge a fixture that forces a Phase-5 late
   ConstMemoryStatic Item with a `<regseq_synth>` `TypeNs`
   segment whose F4 key sorts before a `ValueNs`-leaf
   `emit`-style fn Item. Assert `model.validate()` returns
   `Ok(())`.

10. **F4 monotonicity over `model.items` post-bridge.** Same
    fixture as 9. Walk adjacent items in `model.items` and
    assert
    `compare_items_f4(&items[i], &items[i+1]) != Ordering::Greater`
    for every adjacent pair.

11. **No-op fixture (regression-clean).** Run the existing
    `bridge_pipeline.rs` no-op fixture through the new sort.
    Assert no change to `model.items` order (the no-op
    fixture has no late items, so the sort is a no-op).

### Updated existing tests

12. **`bridge_pipeline.rs` instance-determinism tests** at
    lines 123-145 stay unchanged. The sort runs deterministically
    and does not affect those count assertions.

13. **`bridge_diag_log_recognition.rs` + `bridge_diag_log_peel.rs`**
    test suites stay green. The empty-`&[]` shims those tests
    use do not trigger late-pending ConstMemoryStatic
    synthesis, so the sort is a no-op for them.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-47 against its task spec at
.harness/tasks/regseq-driver/47-f4-late-item-reordering-and-id-remap.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/model.rs (new sort_items_by_f4 method +
  ItemId-remap surface)
- crates/regseq-driver/src/bridge/mod.rs (pipeline wiring -- the
  unconditional sort_items_by_f4 call inside
  construct_instances_and_resolve_cross_references after
  assign_late_pending_items)
- crates/regseq-driver/tests/bridge_f4_late_item_reorder.rs (new
  integration test per Test Plan items 9-11)
- Any new unit-test module added for Test Plan items 1-8

Focus on:
- Spec deviations vs spec.md sections 3.4 (Phase 3/4) and 16.1.
- ItemId remap coverage via the shared `for_each_item_id_mut`
  walker: every Ty::Enum/Struct/Union/FnDef/Closure in the type
  interner, every Instance.item, every IndexVec<ItemId, _> side
  table (permuted, not remapped), every HashMap<*, ItemId> value,
  every HashMap key that embeds ItemId (in particular
  `domain_attrs.field_attrs` -> `FieldPath.item`). Confirm
  body-level terminator / Statement walks are NOT in the surface
  (CallCallee::Direct carries InstanceId per
  terminator.rs:113-115; Aggregate / Cast carry TyId per
  rvalue.rs:45-72, 183-223).
- Sort stability (ties on DefPath retain insertion order per spec
  16.1).
- Idempotency (a model already in F4 order is unchanged).
- Pipeline placement: sort runs after assign_late_pending_items
  and before construct_const_memory_static_instances inside
  construct_instances_and_resolve_cross_references (so the
  stale-handle window before const-memory-static synthesis is
  closed).
- HandleTable IS remapped (via the new
  `HandleTable::remap_item_ids`); the caller composes the
  model-side `sort_items_by_f4`'s returned remap with the
  bridge-side handle update.
- Test coverage covers each side-table surface explicitly.
Do NOT flag anything explicitly marked out of scope (Non-Goals).
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

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-model/src/model.rs::sort_items_by_f4` method
  added per Change A, covering every ItemId reference surface
  enumerated in Implementation Notes steps 5-9.
- `crates/regseq-driver/src/bridge/mod.rs` wires
  `ctxt.model.sort_items_by_f4()` unconditionally after the
  late-pending drain in
  `construct_instances_and_resolve_cross_references`
  (`mod.rs:1008-1013`).
- `crates/regseq-driver/tests/bridge_f4_late_item_reorder.rs`
  created with Test Plan items 9-11.
- Unit tests for Test Plan items 1-8 added (in
  `crates/regseq-model/src/model.rs` `#[cfg(test)] mod tests`
  or a sibling module).
- `cargo test --workspace` passes; in particular, the diag_logging
  fixture's `model.validate()` (via the existing
  `test-ir-fixtures` helper or the new dedicated test) returns
  `Ok(())`.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
