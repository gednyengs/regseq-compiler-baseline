# regseq-driver-46: Top-level FnDef Item Interning + Canonical DefPath Rendering (Phase-2 Baseline Translation)

## Description

Close the spec.md §3.4 Phase-2 baseline translation gap that blocks
`test-ir-fixtures-01`'s no-op smoke fixture. Two coupled bridge changes:

1. **FnDef Item interning per discovered MonoItem.** Per spec.md §3.4
   Phase 2 (`spec.md:1142`): "Bridge walks the discovered mono-item
   set. For each MonoItem, translates rustc state into RegseqModel
   content." The current bridge in
   `crates/regseq-driver/src/bridge/mod.rs::translate_with_interleaved_synthesis`
   (`mod.rs:603-631`) iterates the discovered MonoItems but only
   scans each body for **synthesis triggers** (closures, `dyn`,
   refs, arrays, tuples). It never interns the function's own
   `FnDef` as a source-derived `ItemKind::Function` Item, so
   `construct_function_instances` (`mod.rs:991-1079`, which walks
   `model.items` of `ItemKind::Function`) produces zero Function
   Instances for a leaf function with no in-crate callers.

2. **Canonical `Item.def_path.rendered` + full multi-segment
   `Item.def_path.segments`.** `crates/regseq-driver/src/bridge/ty.rs::intern_source_item_handle`
   (`ty.rs:386-438`) currently stores `tcx.def_path_str(def_id)` as
   `Item.def_path.rendered` (e.g. `"no_op"`) and a leaf-only
   `segments` vec. `crates/regseq-model/src/model.rs::validate_items`
   (`model.rs:1442-1450`) requires
   `rendered == render_def_path(&path, &crates, &strings)` (e.g.
   `"no_op::no_op"`), so any Item that runs validation triggers
   `InvariantViolation::DefPathRenderMismatch`. Per spec.md §3.4
   Phase 3 (`spec.md:1198-1202`) and §7.2 (`spec.md:2079-2088`),
   source-derived Items must carry rustc's DefPath segments
   verbatim and `rendered` must be the eagerly-computed canonical
   `crate::module::Item` string from the spec-pinned
   `render_def_path` helper.

Both gaps are explicitly out of scope for `test-ir-fixtures-01`
per that task's Non-Goals; they belong in the bridge. This task
closes them so `test-ir-fixtures-01` can be reset and run as
written.

This task is **Functions only**. Cross-crate handling is enabled
by removing the local-only gate on the new pass (per spec.md §3.4
Phase 1 cross-crate prose at `spec.md:1133-1139`), but
Static/Const top-level MonoItem interning is not in scope.

## Input Context

- `docs/discussions/regseq-model/spec.md` §3.4 (bridge
  construction-pipeline; especially Phase 2 prose at
  `spec.md:1142-1146` and Phase 1 cross-crate prose at
  `spec.md:1133-1139`)
- `docs/discussions/regseq-model/spec.md` §7.1-§7.2 (DefPathData
  enum + DefPath struct + canonical-rendering invariant;
  `spec.md:2020-2088`)
- `docs/discussions/regseq-model/spec.md` §9.2 + §9.3 (one
  Instance per `(Item, GenericArgs, flavor)` tuple;
  FunctionInstanceFlavor enum)
- `docs/discussions/regseq-model/spec.md` §13.1 (closure
  synthesis: ClosureCaptures + ClosureCall + ClosureDropGlue;
  `spec.md:4511-4554`) -- why `DefKind::Closure` must be excluded
  from the new pass's filter
- `docs/discussions/regseq-model/spec.md` §16.1-§16.2 (F4 sort
  comparator; the F4 key includes full segments)
- `crates/regseq-model/src/def_path.rs` (`render_def_path`,
  `DefPathData` enum, `anon_namespace_name`,
  `def_path_data_sort_rank`)
- `crates/regseq-model/src/model.rs` (`validate_items` at
  `model.rs:1424-1453`)
- `crates/regseq-driver/src/bridge/mod.rs`
  (`translate_with_interleaved_synthesis`,
  `construct_function_instances`, `is_rustc_function`,
  `function_instance_flavor_for_kind`)
- `crates/regseq-driver/src/bridge/ty.rs` (`intern_item`,
  `intern_source_item_handle`, `crate_id`, `TyKind::FnDef`
  translation at `ty.rs:126-132`)
- `crates/regseq-driver/src/bridge/synth/closure.rs`
  (existing `translate_def_path_data` at `synth/closure.rs:201-242`
  -- the 19-variant rustc -> RegseqModel `DefPathData` mapper
  that this task lifts to a shared bridge helper)
- `crates/regseq-driver/src/bridge/discovery.rs`
  (`collect_crate_mono_items`)
- `crates/regseq-driver/tests/bridge_pipeline.rs` (the
  `no_op_crate_phase_1_discovers_mono_items` test at line 110
  whose `instances.is_empty()` assertion locks in the buggy
  contract and must be updated)
- `.harness/tasks/test-ir-fixtures/01-scaffold-and-no-op-smoke.md`
  (the failed task this bridge fix unblocks)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  this task is a phase-1a bridge follow-up)

## Implementation Notes

Three coupled changes plus one cleanup. Land them in one task
because the `def_path.rendered` fix (Change B) is a prerequisite
for the FnDef interning pass (Change A): without Change B, the
first Function Item Change A interns would fail `validate_items`
on the smoke fixture.

### Change A: Phase-2 FnDef Item interning pass

#### A.1: Add `is_source_derived_function` helper

In `crates/regseq-driver/src/bridge/mod.rs`, alongside the
existing `is_rustc_function` (`mod.rs:1121-1128`), add a new
helper that filters specifically for top-level source-derived
function definitions (NOT closures):

```rust
/// True iff `def_id` names a top-level source-derived function
/// definition that this task's Phase-2 baseline pass should
/// intern as an `ItemKind::Function` Item. Excludes
/// `DefKind::Closure`: closures are handled by the §13.1
/// synthesis path (ClosureCaptures + ClosureCall +
/// ClosureDropGlue) via `synth::closure::synthesise_closure_for_def_id`
/// and would otherwise be interned by both paths.
///
/// This is intentionally narrower than `is_rustc_function`,
/// which includes `DefKind::Closure` for Instance resolution in
/// `construct_function_instances`.
fn is_source_derived_function(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    matches!(
        tcx.def_kind(def_id),
        rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
    )
}
```

Keep `is_rustc_function` untouched; it serves a different
filtering purpose in `construct_function_instances`.

#### A.2: Add the interning pass

Flip `crates/regseq-driver/src/bridge/ty.rs::intern_item`
(`ty.rs:368`) from `fn intern_item` to `pub(crate) fn intern_item`
so it can be called from `bridge/mod.rs`.

In `crates/regseq-driver/src/bridge/mod.rs::translate_with_interleaved_synthesis`
(`mod.rs:603-631`), **before** the existing body-scan loop, add
a new pass that iterates the cloned `discovered_mono_items` set
and interns each source-derived FnDef:

```rust
for mono_item in &discovered_mono_items {
    let MonoItem::Fn(instance) = mono_item else {
        continue;
    };
    // Only source-derived `Item` flavor; shim flavors
    // (DropGlue, VTableShim, ClosureOnceShim, etc.) are
    // handled by the existing synthesis triggers per spec.md
    // §3.4 Phase-2 trigger table at spec.md:1148-1158.
    let rustc_middle::ty::InstanceKind::Item(def_id) = instance.def else {
        continue;
    };
    if !is_source_derived_function(ctxt.input.tcx, def_id) {
        continue;
    }
    // Intern as ItemKind::Function. The existing
    // `intern_source_item_handle` dedup cache
    // (`source_item_handles`) makes this idempotent if the
    // body-scan loop later re-discovers the same FnDef via
    // a callee constant.
    let (_item_id, _handle) =
        ty::intern_item(ctxt, def_id, ItemKind::Function, SpecialAdt::None);
}
```

**No `as_local()` gate.** Spec.md §3.4 Phase 1 (`spec.md:1133-1139`)
says the discovered closure covers "primary crate + all
regseq-aware dependency crates per Q6." The existing body-scan
loop's `as_local()` gate at `mod.rs:614` is retained because
`optimized_mir` requires a `LocalDefId`; the new Item-interning
pass does NOT need MIR access, only the `DefId`. `crate_id` at
`ty.rs:451-464` already handles non-local DefIds (searches
`model.crates`, pushes new entry if absent).

**Caveat (not in scope to fix):** `construct_function_instances::source_item_def_id`
(`mod.rs:1109-1118`) only walks local `hir_body_owners`, so
non-local FnDef Items registered by this pass won't get **eager**
Instances. They'll be lazy-bound at call sites via
`body/terminator.rs`'s `lookup_or_create_direct_instance` path.
For the no_op fixture this is moot (the only function is local);
for future cross-crate fixtures it may become a separate task.

### Change B: Full multi-segment `DefPath.segments` + canonical `rendered`

#### B.1: Lift `translate_def_path_data` to a shared bridge helper

The full 19-variant rustc `DefPathData` -> RegseqModel
`DefPathData` mapper already exists at
`crates/regseq-driver/src/bridge/synth/closure.rs:201-242`
(`translate_def_path_data`). Lift it to a new shared module
`crates/regseq-driver/src/bridge/def_path.rs` with the same
public signature:

```rust
//! Shared rustc -> RegseqModel `DefPathData` mapping.
//!
//! Lifted from `synth/closure.rs:201-242` so both closure
//! synthesis and the Phase-2 baseline FnDef-interning path
//! (regseq-driver-46) share one mapper.

use rustc_hir::definitions::DefPathData as RustcDefPathData;

use regseq_model::{DefPathData, StringInterner};

/// Map a rustc `DefPathData` to the RegseqModel variant. Maps
/// all 19 variants per spec.md §7.1 (`spec.md:2020-2056`) and
/// the pinned-toolchain rustc `rustc_hir/src/definitions.rs:274-320`.
pub(crate) fn translate_def_path_data(
    strings: &mut StringInterner,
    data: RustcDefPathData,
) -> DefPathData {
    // body identical to synth/closure.rs:205-241
}
```

Wire from `crates/regseq-driver/src/bridge/mod.rs` (add `pub mod
def_path;` near the other module declarations). Update
`crates/regseq-driver/src/bridge/synth/closure.rs` to import from
the new shared location and delete the local copy.

The lifted helper takes `&mut StringInterner` (not
`&mut BridgeCtxt`) so callers can use it without holding a
mutable borrow on the entire context. The signature change
from `&mut BridgeCtxt` to `&mut StringInterner` is the only
delta from the closure-synth copy; update the one closure
caller accordingly.

#### B.2: Rewrite `intern_source_item_handle` to populate full segments + canonical rendered

Replace the leaf-only segment construction at
`crates/regseq-driver/src/bridge/ty.rs:401-417` with:

```rust
let crate_id = crate_id(cx, def_id);

// Build full multi-segment DefPath.segments per spec.md §3.4
// Phase 3 (spec.md:1198-1202): "source-derived Items use
// rustc's DefPath segments verbatim". `tcx.def_path(def_id).data`
// yields a Vec<DisambiguatedDefPathData> per the pinned
// toolchain.
let rustc_def_path = cx.input.tcx.def_path(def_id);
let segments: Vec<DefPathSegment> = rustc_def_path
    .data
    .iter()
    .map(|disambiguated| DefPathSegment {
        data: def_path::translate_def_path_data(
            &mut cx.model.strings,
            disambiguated.data,
        ),
        disambiguator: disambiguated.disambiguator,
    })
    .collect();

// Validate the bridge's `kind` agreement with the segment-leaf
// rustc DefPathData. The existing match on `kind` chose
// ValueNs vs TypeNs based on `ItemKind`; the leaf segment from
// rustc's def_path already encodes this, so the rendered
// agreement check (validate_items at model.rs:1442) will
// confirm consistency.
//
// Note: `tcx.def_path(def_id).data` does NOT include a
// leading CrateRoot segment for non-root DefIds; the
// `render_def_path` helper at def_path.rs:175 emits the crate
// name first regardless. No leading CrateRoot needs to be
// prepended.

let intermediate_path = DefPath {
    crate_id,
    segments,
    rendered: regseq_model::StringId::from_u32(0), // placeholder
};

// Compute canonical rendered string per spec.md §7.2
// (spec.md:2079-2088): the always-verbose
// crate::module::Item form.
let rendered = render_def_path(
    &intermediate_path,
    &cx.model.crates,
    &cx.model.strings,
);
let rendered_id = cx.model.strings.intern(&rendered);

let item = Item {
    def_path: DefPath {
        crate_id,
        segments: intermediate_path.segments,
        rendered: rendered_id,
    },
    kind,
    span: Span::dummy(),
    linkage: Linkage::Default,
    link_section: None,
    inline_hint: InlineHint::None,
    is_unsafe: false,
    is_phantom_data: matches!(special, SpecialAdt::PhantomData),
    is_unsafe_cell: matches!(special, SpecialAdt::UnsafeCell),
    is_manually_drop: matches!(special, SpecialAdt::ManuallyDrop),
    opaque_handle_eligible: false,
};
```

The `kind` parameter (`ItemKind`) is no longer used to pick
between `ValueNs(leaf_id)` and `TypeNs(leaf_id)` because the
full rustc segments already encode that distinction at the leaf.
The function signature stays the same; remove the now-dead
`segment_data` match block at `ty.rs:405-410`.

Add `use regseq_model::{DefPathSegment, render_def_path};` to
`bridge/ty.rs` imports.

**Callers affected:** `intern_source_item_handle` is called by
`intern_item` (`ty.rs:374`), which is called for `TyKind::FnDef`
(`ty.rs:127`) and `TyKind::Adt` via `translate_adt` (`ty.rs:121`).
All callers are source-derived; the synthesis paths
(closures via `synth/closure.rs::source_item`, vtable / drop
glue / ConstMemoryStatic via `push_pending_synthetic_item` /
`synth/vtable.rs::544-571` / `const_.rs:136-193`) bypass
`intern_source_item_handle` entirely, so Change B does NOT
affect synthesis-derived Items.

### Change C: Update locked-in zero-Instances test contract

Update `crates/regseq-driver/tests/bridge_pipeline.rs::no_op_crate_phase_1_discovers_mono_items`
at lines 110-121 (with the `#[no_mangle] pub extern "C" fn
no_op() {}` source) to:

```rust
assert_eq!(model.instances.len(), 1);
```

Replacing the current `assert!(model.instances.is_empty())` at
line 120. This now matches the spec.md §3.4 Phase 5 contract
"one Instance per (Item, GenericArgs, flavor) tuple" at
`spec.md:1226` for the single discovered `no_op` FnDef.

Keep the `empty_crate_returns_empty_instances` test at
`bridge_pipeline.rs:138-145` unchanged: an empty crate has no
discovered MonoItems, so `instances.is_empty()` is still
correct there.

The other determinism test at `bridge_pipeline.rs:123-136`
asserts that `model.items.len()` is stable across runs; it does
not lock in a specific count and needs no update.

## Non-Goals

- **Static/Const top-level MonoItem interning.** This task is
  scoped to `MonoItem::Fn` with `InstanceKind::Item(def_id)`
  + `DefKind::Fn | DefKind::AssocFn`. `MonoItem::Static` and
  `MonoItem::GlobalAsm` (per `rustc_middle/src/mir/mono.rs:53-58`)
  are out of scope; file as a separate task if a phase-1a
  fixture surfaces a need.
- **Cross-crate eager Instance construction.** Change A enables
  cross-crate FnDef Item interning, but
  `construct_function_instances` continues to walk local
  `hir_body_owners` only. Cross-crate Instances remain
  lazy-bound at call sites; out of scope to change here.
- **Pretty-printer changes.** Already implemented by
  `regseq-model-86` through `regseq-model-93`. This task
  exercises the existing printer via the smoke fixture.
- **Body materialisation / vtable-indirect / DiagLogPeel.**
  Covered by `regseq-driver-43`, `regseq-driver-44`,
  `regseq-driver-45`. No changes here.
- **Synthesis paths.** No changes to
  `synth/{closure,vtable,drop_glue,const_}.rs` beyond updating
  `closure.rs` to import `translate_def_path_data` from the
  new shared module.
- **The `test-ir-fixtures-01` task itself.** Its source files,
  test body, and `instances.len() == 1` assertion stay
  exactly as written. After this task completes, that task is
  reset to `pending` and re-run unchanged.
- **The unrelated `crate_id` duplication** between
  `bridge/ty.rs:451-464` and `synth/closure.rs:244-262`. Worth
  noting but out of scope; file as cleanup if/when it matters.

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/def_path.rs` + adjacent)

1. **`translate_def_path_data` coverage of all 19 variants.**
   In the new `crates/regseq-driver/src/bridge/def_path.rs`
   `#[cfg(test)] mod tests`, construct one
   `rustc_hir::definitions::DefPathData` per variant and assert
   the helper returns the matching RegseqModel
   `DefPathData`. Covers all 19 per spec.md §7.1
   (`spec.md:2026-2056`). Name-bearing variants assert the
   interned `StringId` resolves to the source `Symbol`'s
   string.

2. **`is_source_derived_function` filter table.** In
   `crates/regseq-driver/src/bridge/mod.rs` `#[cfg(test)] mod
   tests` (or co-located with the helper), construct a fixture
   with `DefKind::Fn`, `DefKind::AssocFn`, and
   `DefKind::Closure` def-ids and assert the helper returns
   `true` for the first two and `false` for the closure. If
   the test harness can't easily fabricate raw `DefKind`s,
   exercise via a compiled fixture (one regular fn, one assoc
   fn, one closure) and a `tcx`-borrowing helper.

### Integration tests (`crates/regseq-driver/tests/bridge_fndef_interning.rs`)

3. **`pub fn no_op() {}` produces exactly one Function Item +
   one Function Instance.** New fixture mirroring the existing
   `compile_with_bridge` harness in `bridge_pipeline.rs`. Assert:
   - `model.items.iter().filter(|it| matches!(it.kind,
     ItemKind::Function)).count() == 1`.
   - `model.instances.len() == 1`.
   - The lone Function Instance's `item` ItemId resolves to the
     `no_op` Item.

4. **`Item.def_path` is canonical for a top-level fn.** Same
   fixture as above. Assert:
   - The Function Item's `def_path.segments` is
     `[DefPathSegment { data: ValueNs(s), disambiguator: 0 }]`
     where the resolved `s` is `"no_op"`. (`tcx.def_path` for
     a crate-root function emits a single name-bearing
     segment; no leading CrateRoot.)
   - The Function Item's `def_path.rendered` resolves to
     `"no_op::no_op"` (matches
     `render_def_path(&path, &crates, &strings)`).

5. **`model.validate(&bridge_input)` passes on the no-op
   fixture.** Same fixture. After bridging, call
   `RegseqModel::validate(&BridgeInput::matching(&model))` and
   assert `Ok(())`. This is the assertion `test-ir-fixtures-01`'s
   smoke depends on; the new integration test exercises it
   inside the bridge crate so the bridge's contract is locked
   independently of `test-ir-fixtures`.

6. **Multi-segment `def_path` for an `impl` method.** New
   fixture: `pub struct S; impl S { pub fn m() {} }`. Assert
   the `m` Function Item's `def_path.segments` carries the
   rustc-verbatim shape (Impl segment with disambiguator + leaf
   ValueNs(m)) and `rendered` matches
   `render_def_path(...)` (e.g. `"<crate>::{impl#0}::m"`).
   This covers the multi-segment path that the previous
   leaf-only implementation truncated; cite spec.md §3.4 Phase 3
   (`spec.md:1201`).

7. **Generic function instantiated at two argument sets.**
   New fixture: `pub fn id<T>(x: T) -> T { x }` plus a call
   site exercising two distinct `T`s. Per audit
   regseq-driver-42-4 (`spec.md` §16.2), the bridge produces
   two distinct Function Instances keyed by `(ItemId, ArgsKey,
   FunctionInstanceFlavor)`. Assert the `id` Function Item
   appears exactly once in `model.items`, and that the count
   of Function Instances whose `item == id_item_id` matches
   the number of monomorphisations rustc emits for the fixture
   (typically 2).

### Updated existing test

8. **`bridge_pipeline.rs::no_op_crate_phase_1_discovers_mono_items`**
   at lines 110-121: change `assert!(model.instances.is_empty())`
   at line 120 to `assert_eq!(model.instances.len(), 1)`. The
   `discovered > 0` assertion at line 119 stays. Cite spec.md
   §3.4 Phase 5 contract at `spec.md:1226`.

9. **`bridge_pipeline.rs::empty_crate_returns_empty_instances`**
   at lines 138-145: unchanged. An empty `#![no_std]` crate
   discovers no MonoItems; `instances.is_empty()` remains
   correct.

10. **`bridge_pipeline.rs::no_op_crate_phase_4_is_deterministic_across_runs`**
    at lines 123-136: unchanged. The assertion shape (item
    counts equal across runs) is not affected.

### Conventions

- Doc comments on every new public item (per `SESSION.md`
  Coding Conventions).
- No unicode symbols in source / docs.
- The new integration test file follows the same
  `compile_with_bridge` harness pattern as
  `bridge_pipeline.rs:1-107`. Extract a shared helper if the
  duplication exceeds two fixture invocations.
- All assertions cite the `spec.md` section they verify in a
  one-line comment so the Codex review can cross-reference.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-46 against its task spec at
.harness/tasks/regseq-driver/46-top-level-fndef-interning-and-canonical-def-path.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs (the new is_source_derived_function
  helper + the new Phase-2 FnDef-interning pass in
  translate_with_interleaved_synthesis)
- crates/regseq-driver/src/bridge/ty.rs (intern_item visibility change +
  rewritten intern_source_item_handle producing full multi-segment
  DefPath.segments + canonical rendered)
- crates/regseq-driver/src/bridge/def_path.rs (new shared
  translate_def_path_data helper lifted from synth/closure.rs)
- crates/regseq-driver/src/bridge/synth/closure.rs (closure.rs imports
  translate_def_path_data from the new shared location; local copy
  deleted)
- crates/regseq-driver/tests/bridge_pipeline.rs (line 120 updated to
  assert instances.len() == 1)
- crates/regseq-driver/tests/bridge_fndef_interning.rs (new integration
  tests per Test Plan items 3-7)

Focus on:
- Spec deviations vs spec.md sections 3.4, 7.1-7.2, 9.2-9.3, 13.1, 16.
- Phase-2 baseline translation contract (spec.md:1142): every
  source-derived FnDef MonoItem yields exactly one ItemKind::Function
  Item.
- Canonical DefPath rendering contract (spec.md:2079-2088 + model.rs
  validate_items): Item.def_path.rendered must byte-equal
  render_def_path() output; segments must mirror tcx.def_path verbatim.
- Closure exclusion: DefKind::Closure MUST NOT be interned by the new
  pass (synth/closure.rs handles it via spec.md section 13.1).
- Cross-crate readiness: no as_local() gate on the new pass; crate_id
  in bridge/ty.rs handles non-local DefIds.
- No regressions on synthesis-derived Items: closure / vtable / drop
  glue / ConstMemoryStatic Items continue to enter pending_items via
  their existing synth/* paths, NOT via intern_source_item_handle.
- Test Plan items 1-10 implemented and passing.
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
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/def_path.rs` created with
  `translate_def_path_data` lifted from `synth/closure.rs`;
  covered by Test Plan item 1.
- `crates/regseq-driver/src/bridge/synth/closure.rs` updated to
  import `translate_def_path_data` from the new shared
  location; local copy deleted.
- `crates/regseq-driver/src/bridge/ty.rs::intern_item`
  visibility flipped to `pub(crate)`.
- `crates/regseq-driver/src/bridge/ty.rs::intern_source_item_handle`
  rewritten to populate full multi-segment `DefPath.segments`
  via the shared helper and derive `rendered` via
  `render_def_path(...)` at intern time.
- `crates/regseq-driver/src/bridge/mod.rs::is_source_derived_function`
  helper added (excludes `DefKind::Closure`).
- `crates/regseq-driver/src/bridge/mod.rs::translate_with_interleaved_synthesis`
  gains a pre-body-scan pass that interns each discovered
  `MonoItem::Fn(instance)` with `InstanceKind::Item(def_id)`
  and `is_source_derived_function(tcx, def_id)`.
- `crates/regseq-driver/tests/bridge_pipeline.rs:120` updated
  to `assert_eq!(model.instances.len(), 1)`; lines 138-145
  unchanged.
- `crates/regseq-driver/tests/bridge_fndef_interning.rs`
  created with Test Plan items 3-7.
- `cargo test -p regseq-driver` passes (covers Test Plan items
  1-7 + 8).
- `cargo fmt --all -- --check` and `cargo clippy --workspace --
  -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
- After this task completes: `task.py reset test-ir-fixtures-01
  --reason "regseq-driver-46 unblocks Test Plan §6"` and re-run
  that task unchanged.
