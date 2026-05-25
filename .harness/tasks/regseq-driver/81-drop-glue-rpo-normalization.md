# regseq-driver-81 — DFS-RPO normalization of materialised bodies (FT-003: multi-Drop drop-glue B-RPO reject)

## 1. Description

A struct with **two or more `Drop` fields** (or a Drop type holding a Drop type)
fails to bridge:

```rust
#![no_std]
struct Inner; impl Drop for Inner { fn drop(&mut self) {} }
struct Outer { a: Inner, b: Inner }
pub fn f() { let _o = Outer { a: Inner, b: Inner }; }
```

At HEAD: `BridgeOutcome::Rejected(phase 5 body pass failed: joint validation body
pre-check failed for instance 2: [BRpo])`. A **single** Drop field bridges `Ok`.

Root cause (verified; full chain + Codex review in
`.harness/audits/2026-05-25-ft003-drop-glue-rpo-plan.md`): the model requires
`basic_blocks` to be stored in **DFS reverse-postorder** (`BRpo` check,
`regseq-model/src/visit.rs:74-78`), but the bridge copies rustc's block order 1:1
(`bridge/body/materialise.rs:319-329`) and rustc's **shim** MIR pipeline
(`build_drop_shim`, `rustc_mir_transform/src/shim.rs:164` + the shim `run_passes`
sets at `:149,173,194,232`) does **not** run `ReorderBasicBlocks`. So a multi-block
drop-glue shim arrives non-RPO and the bridge forwards it unchanged.

Fix: the bridge normalises every materialised body to DFS-RPO (the pass rustc's
shim pipeline skips) — reorder reachable blocks into RPO, drop unreachable, remap
every terminator `BlockId`. Idempotent on already-RPO bodies (every `optimized_mir`
body), so it cannot regress them. The model `BRpo`/`BReachable` invariants are
UNCHANGED — the bridge is made to satisfy them.

## 2. Input Context

### Design source

- `.harness/audits/2026-05-25-ft003-drop-glue-rpo-plan.md` — Codex-APPROVED plan.
  Decisive soundness finding: the model has **no cleanup/unwind edges** (`is_cleanup`
  dropped at `regseq-model/src/basic_block.rs:27`; the bridge rejects any surviving
  cleanup block at `bridge/body/basic_block.rs:23`, panic=abort enforcement), so
  `Terminator::successors()` is the COMPLETE `BlockId` reference set and
  drop-unreachable is safe.

### Model anchors (modified)

- `crates/regseq-model/src/visit.rs:54-82` — `visit_body` pre-checks (`BNonEmpty`,
  `BSuccessorsValid`, `BReachable`, `BRpo`). `:390` `fn dfs_rpo_from(start, body)
  -> Vec<BlockId>` — **private**; the reorder reuses it (bump to `pub(crate)` or
  call from the same module).
- `crates/regseq-model/src/terminator.rs:37-62` `Terminator::successors()` (the
  complete BlockId set); `:63-176` `TerminatorKind` (Goto / SwitchInt / Return /
  Unreachable / Call / Assert); `:127-133` `SwitchTargets { cases, otherwise }`.
- `crates/regseq-model/src/body.rs` — `Body { basic_blocks: IndexVec<BlockId,
  BasicBlock>, .. }`; `:111` local decls carry no BlockId.
- `crates/regseq-model/src/ids.rs` — `BlockId` (`START_BLOCK`, `from_u32`, `index`).

### Bridge anchors (modified — the call site)

- `crates/regseq-driver/src/bridge/body/materialise.rs:296-329` body construction
  (1:1 copy of rustc block order); `:364` region `run_phase5_body_passes` runs
  `rewrite_vtable_indirect` then `diag_log_peel_pass` then `joint_validation_walk`
  (`:404` `joint_validation_walk`). **Call site = immediately before
  `joint_validation_walk`, after the two Phase-5 passes** (Codex: `diag_log_peel_pass`
  rewrites `Call`→`Goto(target)` at `bridge/diag_log/peel.rs:448`, so normalising
  earlier would let a later peel change the CFG).

### Prior tasks

- driver-80 triage — FT-003 is the HIGH-priority Bucket-B item (blocks any
  nested/multi-Drop fixture); root cause + repro recorded there.
- driver-69 — `BuildOverrides` surface (unrelated here; FT-003 needs no overrides).

### Test surface

- New model unit tests in `regseq-model` (reorder correctness, direct against
  `visit_body`).
- New `crates/regseq-driver/tests/bridge_drop_glue_rpo.rs` (end-to-end).
- New `crates/bridge-coverage-runner/snippets/FT-003.rs` + `rows.toml` row + matrix
  `FT-003` status; Mode A over the corpus.

## 3. Implementation Notes

### Change A — model-side DFS-RPO reorder (`regseq-model`)

Add a `Body` normalization (e.g. `pub fn reorder_blocks_to_rpo(&mut self)`,
co-located with `dfs_rpo_from` in `visit.rs` or on `Body` in `body.rs`):

1. `let rpo = dfs_rpo_from(BlockId::START_BLOCK, self);` (reuse the existing fn;
   `rpo[0]` is `START_BLOCK`, so START stays block 0).
2. If `rpo.len() == basic_blocks.len()` AND already in order (`rpo[i] == BlockId(i)`
   ∀i), return early (no-op fast path — optional; the general path is also
   idempotent).
3. Build `old→new`: `new_index[rpo[i].index()] = i`; old ids not in `rpo` are
   unreachable → no mapping (dropped).
4. Reorder: `new_blocks[i] = take(old_blocks[rpo[i]])`, dropping unreachable.
5. Remap every `BlockId` in every terminator via `old→new`, using an **exhaustive
   `match` on `TerminatorKind`** (+ `SwitchTargets`): `Goto(b)`, `SwitchInt {
   targets: { cases: [(_, b)], otherwise } }`, `Call { target: Option<b> }`,
   `Assert { target: b }`; `Return`/`Unreachable` unchanged. (A reachable
   terminator's successors are reachable, so every remapped id has a mapping.)
6. Replace `self.basic_blocks` with `new_blocks`.

The exhaustive match is mandatory (a future BlockId-bearing variant must force this
to update — mirrors the Visitor's exhaustive-destructuring contract). Prefer a
`SwitchTargets`/`TerminatorKind` remap helper so the logic is one exhaustive site.

### Change B — call it in the bridge, strictly last before validation

In `run_phase5_body_passes` (`bridge/body/materialise.rs:364` region), after
`rewrite_vtable_indirect` and `diag_log_peel_pass`, immediately before
`joint_validation_walk(...)`, call `body.reorder_blocks_to_rpo()` on the
materialised body. No other change to the materialiser.

### Change C — coverage row + tests

- New `snippets/FT-003.rs` (the 2-Drop-field witness); `rows.toml` `FT-003` row
  (`expectation = "positive"`, predicate inspecting the drop-glue / a `Call` to the
  drop shim — choose a stable predicate, e.g. an Item/Instance for the
  `ClosureDropGlue`/drop-shim, or simply that it bridges Ok; coordinate with the
  inspect surface). Matrix `FT-003` `uncovered`→`covered`.
- `bridge_drop_glue_rpo.rs` integration test (see Test Plan).

## 4. Non-Goals

- **No relaxation of `BRpo`/`BReachable`** — the fix makes the bridge satisfy them.
- **No rustc pass-config change** (`callbacks.rs`) — we consume post-rustc MIR and
  cannot inject a pass into rustc's shim pipeline; the normalization is bridge/model-
  side.
- **No other triage rows** — FT-004/IX-020/etc. are separate F-b3 snippets (they may
  incidentally start passing; not in scope here). Not the RJ-013 fix.
- No change to `block_id` identity mapping for the *initial* copy; the reorder is a
  distinct final step.

## 5. Test Plan

### Model unit tests (`regseq-model`, direct against `visit_body`)

- **B1 reorder correctness**: construct a synthetic `Body` whose `basic_blocks` are
  NOT in DFS-RPO (e.g. START→2→1→Return with blocks laid out [START,1,2] where DFS
  visits START,2,1); after `reorder_blocks_to_rpo`, `visit_body` emits NO body error
  (BRpo/BReachable/BSuccessorsValid all pass).
- **B2 CFG-preservation**: the post-reorder CFG edge set equals the pre-reorder edge
  set mapped through the permutation (enumerate `(src, succ)` pairs via
  `successors()` before/after; assert equality under `old→new`). This is the
  soundness anchor for the remap. **Use a fully-reachable fixture (no unreachable
  blocks) for this comparison**, since Change A intentionally drops unreachable
  blocks (the unreachable-drop case is covered separately by B3).
- **B3 drop-unreachable**: a body with an unreachable block → after reorder, the
  unreachable block is gone, `basic_blocks.len()` shrinks, `visit_body` passes.
- **B4 idempotence**: `reorder` of an already-RPO body is a no-op (the body is
  unchanged — equal blocks + terminators); `reorder∘reorder == reorder`.
- **B5 START fixed**: `START_BLOCK` maps to index 0 after reorder.

### Bridge integration tests (`bridge_drop_glue_rpo.rs`)

- **B6 multi-Drop**: the 2-Drop-field witness bridges `Ok` + validates; the drop-glue
  instance body is RPO (`bb_id == idx` ∀ blocks).
- **B7 nested Drop**: a Drop type holding a Drop field (`Outer{a:Inner}` with both
  `impl Drop`) bridges `Ok`.
- **B8 single-Drop control**: still `Ok` (unchanged from HEAD).

### Mode A

- **B9**: full runner; `Total == Pass`, `Fail == 0`, exit 0, with the new `FT-003`
  row; every pre-existing row unchanged (146 → 147).

## 6. Review

### Files to send to Codex

- This spec + the plan + the implemented diff (Change A/B/C).

### Reviewer focus

1. **Remap exhaustiveness/soundness**: does the remap cover every `BlockId`
   (exhaustive match; matches `successors()`); is the reorder a semantics-preserving
   relabel (CFG edge set preserved under the permutation)?
2. **Drop-unreachable safety**: confirmed no unwind/cleanup edges (so no live block
   dropped); a reachable terminator never references a dropped block.
3. **Call-site ordering**: reorder runs after `rewrite_vtable_indirect` +
   `diag_log_peel_pass`, immediately before `joint_validation_walk`.
4. **Idempotence + no regression**: already-RPO bodies unchanged; Mode A 147/147/0.
5. Invariants `BRpo`/`BReachable` unchanged.

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-81 (cwd = repo root): a DFS-RPO body
normalization fixing the FT-003 multi-Drop [BRpo] reject, per the APPROVED plan
.harness/audits/2026-05-25-ft003-drop-glue-rpo-plan.md. A regseq-model Body method reorders
reachable blocks into DFS-RPO, drops unreachable, and remaps every terminator BlockId via an
exhaustive match; the bridge calls it immediately before joint_validation_walk. Verify the
remap is exhaustive + semantics-preserving (CFG edge set preserved under the permutation),
drop-unreachable is safe (no unwind/cleanup edges in the model), the call site is strictly last,
the reorder is idempotent on already-RPO bodies, and BRpo/BReachable are unchanged. Build, run
cargo test + Mode A (147/147/0), and probe the multi-Drop + single-Drop witnesses.
</task>
<grounding_rules>
Cite file:line. Run the tests + Mode A + probes. Check the remap match is exhaustive against
TerminatorKind. This is a load-bearing body rewrite — be adversarial about a missed BlockId ref.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
REMAP_EXHAUSTIVE_AND_CFG_PRESERVED: PASS|FAIL + cite
DROP_UNREACHABLE_SAFE: PASS|FAIL + cite
CALLSITE_STRICTLY_LAST: PASS|FAIL + cite
IDEMPOTENT_NO_REGRESSION: PASS|FAIL + cite (Mode A)
INVARIANTS_UNCHANGED: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-model --lib -- -D warnings` + `cargo clippy -p
  regseq-driver --lib -- -D warnings` + the new test targets clean. **Do NOT gate on
  `--all-targets`** (pre-existing test-target lint drift; [[clippy-all-targets-test-drift]]).
- `cargo test -p regseq-model -- --test-threads=1` green (incl. the new reorder unit
  tests); `cargo test -p regseq-driver -- --test-threads=1` green (incl.
  `bridge_drop_glue_rpo.rs`).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `FT-003` (146 → 147).
- **A focused Codex soundness pass is REQUIRED at assessment** ([[assessment-soundness-pass]]):
  this rewrites body structure for ALL bridged bodies; the load-bearing risk is a
  missed BlockId in the remap (silent mis-target). The B2 CFG-preservation test is
  the primary guard.
- Diff scope confined to: the `regseq-model` reorder (+ `dfs_rpo_from` visibility) +
  its unit tests; the one bridge call site; `FT-003` snippet + row + matrix status;
  the new bridge integration test. No invariant relaxation, no `callbacks.rs`, no
  other triage rows.

## 8. Acceptance Criteria

1. The multi-Drop witness (2-Drop-field struct) bridges `BridgeOutcome::Ok` and
   validates; its drop-glue body is DFS-RPO (`bb_id == idx`). Nested Drop also `Ok`.
2. Single-Drop and every pre-existing body are unchanged (reorder idempotent on
   already-RPO bodies).
3. `regseq-model` gains a `Body` DFS-RPO reorder that: reorders reachable blocks into
   RPO, drops unreachable, remaps every terminator `BlockId` via an exhaustive match,
   keeps `START_BLOCK` at index 0; covered by unit tests incl. the CFG-preservation
   (B2) and idempotence (B4) tests.
4. The bridge calls the reorder immediately before `joint_validation_walk`, after the
   Phase-5 vtable/diag-log-peel passes.
5. `BRpo`/`BReachable` model invariants are unchanged; no `callbacks.rs` change.
6. Mode A `147/147/0` exit 0 (`FT-003` covered; all prior rows unchanged); build /
   fmt / clippy (lib + new tests) / `cargo test -p regseq-model` / `cargo test -p
   regseq-driver` green.
7. Diff confined per the Quality Gate.
