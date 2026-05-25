# Plan — FT-003-fix: DFS-RPO normalization of materialised bodies (multi-Drop drop-glue B-RPO reject)

Plan-first design doc (pre-spec). Grounded at HEAD (`9286210`). First real-fix of
the feature-complete F-b arc; the HIGH-priority Bucket-B item from the driver-80
triage (`.harness/audits/2026-05-25-needs-validation-tail-triage.md`). Probe-first
per the R-series discipline.

## 0. Goal

A struct with **multiple `Drop` fields** (or two Drop-typed fields) fails to bridge:

```rust
#![no_std]
struct Inner; impl Drop for Inner { fn drop(&mut self) {} }
struct Outer { a: Inner, b: Inner }
pub fn f() { let _o = Outer { a: Inner, b: Inner }; }
```

→ `BridgeOutcome::Rejected(phase 5 body pass failed: joint validation body
pre-check failed for instance 2: [BRpo])` (reproduced at HEAD). A **single** Drop
field bridges `Ok` (reproduced). Goal: multi-Drop bridges `Ok` + validates, with no
regression to any existing row.

## 1. HEAD-grounded root cause (4-link chain, all verified)

1. **Model invariant — bodies must be stored in DFS reverse-postorder.**
   `Visitor::visit_body` (`regseq-model/src/visit.rs:54-82`) runs the spec §25.6.2
   pre-checks; the `BRpo` check (`:74-78`) requires `rpo[idx] == BlockId(idx)` for
   all idx — i.e. `basic_blocks` must already be laid out in DFS-RPO. (`BReachable`
   at `:70-72` additionally requires every block reachable from START.)
2. **The bridge copies rustc's block order 1:1 with NO normalization.**
   `materialise_function_body_inner` (`bridge/body/materialise.rs:319-329`) does
   `rustc_body.basic_blocks.iter().cloned()` → `body.basic_blocks.push(translated)`;
   `block_id` is a trivial index identity (`bridge/body/terminator.rs:125`). No
   `dfs_rpo`/reorder/remap exists anywhere in `bridge/body/` (grep-confirmed).
3. **rustc's shim MIR pipeline does NOT run `ReorderBasicBlocks`.** `build_drop_shim`
   (`rustc_mir_transform/src/shim.rs:164`) elaborates the drop glue, and the shim
   `run_passes` sets (`shim.rs:149,173,194,232`) run `AbortUnwindingCalls`,
   `CriticalCallEdges`, `SimplifyCfg::MakeShim` — but **never `ReorderBasicBlocks`**
   (the pass `optimized_mir` runs for ordinary fns). So a multi-block drop-glue
   shim keeps the elaborator's block order, which is not DFS-RPO.
4. **Result:** multi-Drop drop glue → non-RPO `basic_blocks` → `BRpo` Phase-5
   reject. Single-Drop glue is RPO-trivial (linear, few blocks) → passes. This is a
   latent gap for ANY non-RPO shim body (drop glue is just the first realisable
   witness); the bridge has been relying on rustc delivering RPO, which holds for
   `optimized_mir` but not for shims.

## 2. The fix — bridge runs its own RPO normalization (the pass rustc's shims skip)

Normalise every materialised body to the model's RPO invariant, by replicating what
rustc's `ReorderBasicBlocks` does: keep the blocks reachable from START in DFS-RPO
order, drop unreachable blocks, and remap every `BlockId` reference. Idempotent on
already-RPO bodies (the overwhelming majority — `optimized_mir` output), so it is a
no-op for them and cannot regress them.

**Mechanism:**
1. Compute `rpo = dfs_rpo` from `BlockId::START_BLOCK` over the materialised body.
2. Build `old→new` map: `new_id[rpo[i]] = BlockId(i)`; blocks not in `rpo` are
   unreachable → dropped (matches `ReorderBasicBlocks`; satisfies `BReachable`).
3. Reorder `basic_blocks` into `rpo` order.
4. Remap **every** `BlockId` in **every** terminator via `old→new`: `Goto(b)`,
   `SwitchInt { targets: { cases: [(_, b)], otherwise } }`, `Call { target:
   Option<b> }`, `Assert { target: b }` — exactly the set `Terminator::successors()`
   enumerates (`terminator.rs:37-62`). `Return`/`Unreachable` carry none.

**Home (recommended): a model-side `Body` method** (e.g.
`Body::reorder_blocks_to_rpo(&mut self)`) in `regseq-model`, called by the bridge
at the end of body materialisation. Rationale: the model owns the RPO invariant
(`visit.rs`), the `Body`/`Terminator` types, `dfs_rpo_from` (`visit.rs:390`,
currently private), and `successors()` — co-locating the reorder there keeps the
remap exhaustive-by-construction (same exhaustive-match discipline as the Visitor)
and lets it be unit-tested directly against the `BRpo`/`BReachable` checks in
`regseq-model`'s suite. The bridge alternative (replicate `dfs_rpo` + a remap in
`bridge/body/`) duplicates logic and splits the invariant from its enforcer.

**Call site (bridge): in `run_phase5_body_passes`, AFTER `rewrite_vtable_indirect`
and `diag_log_peel_pass`, immediately before `joint_validation_walk`**
(`materialise.rs:364` region). This is the strictly-last CFG-mutating point —
Codex confirmed `diag_log_peel_pass` rewrites a `Call` terminator into
`Goto(target)` (`bridge/diag_log/peel.rs:448`), so normalising at the end of
`materialise_function_body_inner` (before these Phase-5 passes) would let a later
peel re-introduce a non-RPO/changed CFG. Normalising right before the validation
walk guarantees the body is RPO at the moment of the check.

## 3. Design forks (recommendation; Codex to pressure-test)

- **A. Reorder home — model method vs bridge fn.** *Recommend model method*
  (`Body::reorder_blocks_to_rpo`) for cohesion + testability + exhaustive remap;
  exposes no fragile internals (it's a method on the owned type). Alternative:
  `pub` the existing `dfs_rpo_from` + a bridge-side remap. Decide.
- **B. Unreachable blocks — drop vs preserve.** *Recommend drop* (keep only
  reachable, in RPO) — matches `ReorderBasicBlocks`, satisfies `BReachable`, and is
  semantics-preserving (unreachable = no effect). Confirm drop-glue shims don't rely
  on an unreachable block being retained (they shouldn't post-SimplifyCfg).
- **C. Apply universally vs only-when-not-RPO.** *Recommend universally* (idempotent
  on RPO bodies; a single normalization path is simpler + more robust than a
  conditional). The reorder of an already-RPO body is the identity permutation.
- **D. Remap exhaustiveness mechanism.** *Recommend* a `TerminatorKind` (or
  `SwitchTargets`) remap via exhaustive `match` so a future BlockId-bearing variant
  forces an update — mirrors the Visitor's exhaustive-destructuring contract.

## 4. Soundness considerations (this is load-bearing → Codex soundness pass at assessment)

The reorder is a **semantics-preserving relabel**: correctness hinges on the remap
covering EVERY `BlockId` reference. A missed reference would silently retarget a
terminator to the wrong block (a miscompile the validator might not catch, since the
result is still structurally valid). Mitigations the spec must require:
- Exhaustive-match remap (fork D); cross-check against `Terminator::successors()`.
- A test asserting the reordered body PASSES `visit_body` (BRpo + BReachable +
  BSuccessorsValid) AND that successor *structure* is preserved (the post-remap CFG
  edge set equals the pre-remap edge set under the permutation).
- Confirm no double-application hazard and idempotence (reorder∘reorder == reorder).
- [[assessment-soundness-pass]]: a focused Codex soundness pass at assessment, since
  this rewrites body structure for ALL bodies.

## 5. Test plan sketch (for the spec)

- **Repro/positive:** the 2-Drop-field struct (and a Drop-type-holding-Drop nested
  struct) bridges `Ok` + validates; the materialised body is RPO
  (`bb_id == idx`). This is the FT-003 row.
- **Control:** single-Drop-field still `Ok` (unchanged).
- **Structure:** a synthetic non-RPO body → after reorder, passes `visit_body`; CFG
  edge set preserved under the permutation; unreachable block dropped. (model unit
  test, direct.)
- **Idempotence:** reorder of an already-RPO body is a no-op (byte-identical).
- **Regression:** Mode A `146/146/0` unchanged (every existing row already RPO →
  reorder is identity for them).
- **Coverage row:** FT-003 `uncovered`→`covered` + a committed runner snippet
  (the multi-Drop witness) + an integration test asserting Ok + RPO.

## 6. Scope / non-goals

- Fix is the RPO normalization + its call site + the FT-003 row/test. No other
  triage rows (FT-004/IX-020 etc. are separate F-b3 snippets; they may incidentally
  start passing but are out of THIS task's scope).
- No relaxation of the `BRpo`/`BReachable` model invariants (the fix makes the
  bridge SATISFY them, not weaken them).
- No change to rustc pass config (`callbacks.rs`) — the fix is bridge/model-side, not
  "turn on ReorderBasicBlocks for shims" (we consume post-rustc MIR; can't inject a
  shim pass).
- Not the RJ-013 fix (the other Bucket-B item; separate task).

## 7. Risks / open questions for Codex

1. **Remap completeness** — RESOLVED (Codex): `successors()` (`terminator.rs:37-62`)
   IS the complete BlockId set. `TerminatorKind` = Goto/SwitchInt/Return/Unreachable/
   Call/Assert only; `SwitchTargets` block refs are just `cases`+`otherwise`
   (`terminator.rs:127`); statements (`statement.rs:33`) and local decls
   (`body.rs:111`) carry NO `BlockId`. The exhaustive-match remap (fork D) covers all.
2. **Call-site ordering** — RESOLVED (Codex): `run_phase5_body_passes` runs
   `rewrite_vtable_indirect` + `diag_log_peel_pass` after materialisation, and
   `diag_log_peel_pass` rewrites `Call`→`Goto(target)` (`peel.rs:448`). The reorder
   is therefore placed immediately before `joint_validation_walk`, after those
   passes (see §2 Call site).
3. **Unreachable-drop safety** — RESOLVED (Codex, decisive): the model has **NO
   cleanup/unwind edges at all** — `is_cleanup` is explicitly dropped
   (`regseq-model/src/basic_block.rs:27`) and the bridge REJECTS any surviving
   cleanup block before model construction (`bridge/body/basic_block.rs:23`,
   panic=abort enforcement). So there are no unwind landing pads that could be
   wrongly dropped; every CFG ref is a terminator successor. Drop-unreachable is SAFE.
4. **Home decision** (fork A) — is a new `regseq-model` `Body` method acceptable, or
   is a bridge-local helper preferred to keep the model surface minimal?
5. Does any downstream consumer assume the bridge preserves rustc's exact block
   numbering (so a reorder would surprise it)? (The model already mandates RPO, so
   no — but confirm no test pins specific block ids from a shim.)
