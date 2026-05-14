# regseq-driver-36: Bridge Construction-Pipeline Scaffold

## Description

Land the 5-phase bridge pipeline skeleton in `regseq-driver` that
translates a rustc compilation into a `RegseqModel`, without any
per-construct translation. This task implements the pipeline ordering
mandated by `spec.md` §3.4: Phase 1 DISCOVERY (`collect_crate_mono_items`)
plus Phase 3 F4 SORT plus Phase 4 ItemId ASSIGNMENT plus side-table
population (`item_def_paths`, `item_synthetic_kinds`,
`domain_attrs.item_attrs` init), with the Phase-2-handle-to-final-ItemId
remap machinery (D-12-5.3). Per-construct translation, synthesis, and
body materialisation are intentionally deferred to subsequent bridge
tasks (regseq-driver-37 through regseq-driver-45).

This is the first net-new module set under `crates/regseq-driver/src/bridge/`.
The scaffold pins the public entry point, the per-phase types, the
working-state container, and the ItemHandle-to-ItemId remap so every
later task slots into a fixed pipeline rather than restructuring the
caller surface.

## Input Context

- `docs/discussions/regseq-model/spec.md` §3 (Top-level RegseqModel),
  especially §3.4 5-phase pipeline ordering and the D-12-5.2 / D-12-5.3
  decisions
- `docs/discussions/regseq-model/spec.md` §16 (F4 / F4-2 sort comparators)
- `crates/regseq-model/src/lib.rs` and `crates/regseq-model/src/model.rs`
  (root `RegseqModel` shape post P1a-DS-01)
- `crates/regseq-model/src/identity/` (Symbol, ItemId, DefPath)
- `crates/regseq-driver/src/lib.rs` and `crates/regseq-driver/src/callbacks.rs`
  (existing rustc-callback host)

## Implementation Notes

Create a new `src/bridge/` subtree under `crates/regseq-driver/`. Wire
`pub mod bridge;` from `crates/regseq-driver/src/lib.rs`. The scaffold
introduces these modules:

- `crates/regseq-driver/src/bridge/mod.rs` -- public entry point, the
  `BridgeCtxt` working-state container, the 5-phase orchestrator
- `crates/regseq-driver/src/bridge/discovery.rs` -- Phase 1
  (`collect_crate_mono_items`)
- `crates/regseq-driver/src/bridge/sort.rs` -- Phase 3 F4 SORT
- `crates/regseq-driver/src/bridge/assign.rs` -- Phase 4 ItemId
  ASSIGNMENT + side-table population
- `crates/regseq-driver/src/bridge/handles.rs` -- Phase-2 ItemHandle
  vocabulary + Phase-4 handle-to-final-ItemId remap (D-12-5.3)

### Public API

```rust
// bridge/mod.rs

/// Inputs the bridge consumes from the rustc host (`spec.md` §3.5.1
/// `BridgeInput`).
pub struct BridgeInput<'tcx> { /* tcx + the discovered mono-item set */ }

/// Mutable working state threaded through every phase.
pub struct BridgeCtxt<'tcx> {
    pub input: BridgeInput<'tcx>,
    pub model: regseq_model::RegseqModel,
    pub handles: handles::HandleTable,
    /* per-phase caches populated by later bridge tasks */
}

/// Orchestrates the 5-phase pipeline. Returns the populated
/// `RegseqModel`. Per `spec.md` §3.4, this entry point is the only
/// public way to drive the bridge.
pub fn run_bridge<'tcx>(input: BridgeInput<'tcx>)
    -> Result<regseq_model::RegseqModel, BridgeError>;

/// Errors returnable from the bridge. Variants are extended by later
/// bridge tasks; this scaffold defines the enum + the
/// `GenericOpaqueHandle` / `Phase1` / `Phase3` / `Phase4` baseline
/// variants per the §3.4 phase boundary.
pub enum BridgeError { /* baseline variants */ }
```

### Per-phase ordering (`spec.md` §3.4)

Per spec.md §3.4, `run_bridge` MUST execute exactly:

1. Phase 1 DISCOVERY (`discovery::collect_crate_mono_items`)
2. Phase 2 TRANSLATION-with-interleaved-synthesis (later tasks; this
   task wires the empty hook)
3. Phase 3 F4 SORT (`sort::f4_sort`)
4. Phase 4 ItemId ASSIGNMENT + side-table population
   (`assign::assign_item_ids_and_populate_sidetables`)
5. Phase 5 INSTANCE construction + cross-reference resolution + body
   rewrites (later tasks; this task wires the empty hook)

The Phase-2 / Phase-5 hooks are `fn(&mut BridgeCtxt) -> Result<(), BridgeError>`
no-op stubs that later tasks replace.

### Phase-2-handle-to-final-ItemId remap (D-12-5.3)

`bridge/handles.rs` defines an `ItemHandle` opaque newtype produced
during Phase 2 translation. Phase 4's `assign_item_ids_and_populate_sidetables`
walks the F4-sorted item set, assigns dense `ItemId` values, and builds
the `HandleTable::handle_to_item_id: HashMap<ItemHandle, ItemId>` map.
Side-tables (`item_def_paths`, `item_synthetic_kinds`,
`domain_attrs.item_attrs` init) are populated in the same Phase-4 walk
in lock-step with the ItemId assignment.

### F4 sort entry point

`sort::f4_sort` calls into the spec-pinned F4 comparator surface owned
by `regseq-model` (per the dependency on regseq-model-84 / P1a-VAL-02);
the bridge does not re-derive the comparator. The sort is over the
Phase-2-populated handle set.

## Non-Goals

- Per-`TyKind` translation (regseq-driver-37).
- Const / FnSig translation (regseq-driver-38, regseq-driver-39).
- Synthesised-Item production (regseq-driver-40).
- Place / Operand / Rvalue / Statement translation (regseq-driver-41).
- Terminator / Drop-as-Call / basic-block translation (regseq-driver-42).
- Body materialisation + vtable-indirect rewrite + joint validation
  (regseq-driver-43).
- Diagnostic-logging recognition / DiagLogPeel (regseq-driver-44, regseq-driver-45).

## Test Plan

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures driving the bridge end-to-end.

### Unit tests (`crates/regseq-driver/src/bridge/*` `#[cfg(test)] mod tests`)

- `handles.rs`: construct an empty `HandleTable`, insert N handles,
  assert `handle_to_item_id` resolves each handle back to a unique
  dense `ItemId`.
- `sort.rs`: feed a deterministic in-memory handle set into `f4_sort`
  and assert the output ordering matches the F4 comparator's contract
  (regseq-model-84 owns the sort check; this asserts the bridge calls
  it correctly).
- `assign.rs`: assert that `assign_item_ids_and_populate_sidetables`
  produces the same ItemId for the same input handle order across
  10 repeated runs (deterministic ItemId assignment).

### Integration tests (`crates/regseq-driver/tests/bridge_pipeline.rs`)

- Drive `run_bridge` against a no-op `#![no_std]` crate fixture; assert
  Phase 1 closure (the discovered mono-item set is non-empty when the
  fixture defines `pub fn no_op() {}`).
- Assert Phase 4 produces a deterministic ItemId for the no-op function
  across 5 repeated runs of the same fixture.
- Negative: drive `run_bridge` against an empty crate (no mono-items)
  and assert the resulting `RegseqModel.instances` is empty (Phase 1
  returns the empty closure cleanly; no panic).

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-36 against its task spec at
.harness/tasks/regseq-driver/36-bridge-pipeline-scaffold.md. ALL CHECKS
MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/discovery.rs
- crates/regseq-driver/src/bridge/sort.rs
- crates/regseq-driver/src/bridge/assign.rs
- crates/regseq-driver/src/bridge/handles.rs
- crates/regseq-driver/tests/bridge_pipeline.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  especially the §3.4 5-phase ordering and D-12-5.3 handle remap).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
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

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/{mod,discovery,sort,assign,handles}.rs`
  created and wired from `crates/regseq-driver/src/lib.rs`.
- `run_bridge` orchestrates the 5 phases in spec.md §3.4 order with
  Phase-2 / Phase-5 no-op stubs.
- `HandleTable` provides the Phase-2-handle-to-final-ItemId remap per
  D-12-5.3.
- All unit and integration tests pass.
- Codex Task Review verdict APPROVED.
