# regseq-driver-42: Terminator + Drop-as-Call Collapse + Basic-Block Bridge

## Description

Translate rustc Terminators into the `regseq-model` 6-variant
TerminatorKind per `spec.md` §23.7, with the rustc `Drop` variant
collapsed into a `Call` against the drop-glue Function Item synthesised
by regseq-driver-40 per spec.md §23.4 and §25.7.1. Also translates
basic blocks per spec.md §24.7.

The Drop-as-Call collapse runs **inside** the atomic per-rustc-terminator
translation envelope (spec.md §3.4 Phase 5 body-construction): for a
rustc `Drop` Terminator, the bridge consults the
`drop_glue_synth_cache` (regseq-driver-40), resolves the drop-glue
ItemHandle, and emits a `Call` Terminator targeting that Item. **No
`Drop` variant survives in RegseqModel** post-collapse.

## Input Context

- `docs/discussions/regseq-model/spec.md` §23 (Terminator +
  TerminatorKind; §23.4 Drop-as-Call collapse; §23.7 translation rules)
- `docs/discussions/regseq-model/spec.md` §24 (BasicBlock; §24.7
  translation rules)
- `docs/discussions/regseq-model/spec.md` §25.7.1 (Drop-as-Call
  body-level pass)
- `docs/discussions/regseq-model/spec.md` §3.4 (Phase 5 body-construction
  ordering)
- `crates/regseq-model/src/terminator.rs` (Terminator + TerminatorKind +
  CallCallee + SwitchTargets + AssertKind from regseq-model-80)
- `crates/regseq-model/src/basic_block.rs` (BasicBlock + BlockId from
  regseq-model-81)
- `crates/regseq-driver/src/bridge/body/` (per-construct translators
  from regseq-driver-41)
- `crates/regseq-driver/src/bridge/synth/drop_glue.rs`
  (synthesise_drop_glue + drop_glue_synth_cache from regseq-driver-40)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/body/terminator.rs` and
`crates/regseq-driver/src/bridge/body/basic_block.rs`. Wire both from
`crates/regseq-driver/src/bridge/body/mod.rs`.

### Public API

```rust
// bridge/body/terminator.rs

/// Translate a rustc `mir::Terminator` into a
/// `regseq_model::Terminator`. Per spec.md §23.7. Rustc `Drop`
/// collapses into `Call` per spec.md §23.4 / §25.7.1 (no `Drop`
/// variant exists in regseq_model).
pub fn translate_terminator<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    term: rustc_middle::mir::Terminator<'tcx>,
) -> Result<regseq_model::Terminator, BridgeError>;
```

```rust
// bridge/body/basic_block.rs

/// Translate a rustc `mir::BasicBlockData` into a
/// `regseq_model::BasicBlock`. Per spec.md §24.7. Iterates statements
/// (regseq-driver-41) then runs the terminator translator above.
pub fn translate_basic_block<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    bb: &rustc_middle::mir::BasicBlockData<'tcx>,
) -> Result<regseq_model::BasicBlock, BridgeError>;
```

### Per-TerminatorKind translation table (spec.md §23.7)

The translation runs as a `match` over `term.kind`. Per spec.md §23.7
each rustc `TerminatorKind` row maps to one of the 6 closed
TerminatorKind variants:

- Goto -> Goto
- SwitchInt -> SwitchInt with the SwitchTargets payload per spec.md
  §23.3
- Return -> Return
- Unreachable -> Unreachable
- Call -> Call with CallCallee payload (Direct / Indirect; the
  Indirect variant is populated later by regseq-driver-43's
  vtable-indirect rewrite)
- Assert -> Assert with AssertKind payload per spec.md §23.3
- **Drop -> Call against the drop-glue Item** per spec.md §23.4 (see
  below)
- The remaining rustc TerminatorKind variants (e.g. Yield,
  GeneratorDrop, FalseEdge, FalseUnwind, InlineAsm) either reject per
  spec.md §23 or elide per spec.md §23.7.

### Drop-as-Call collapse (spec.md §23.4 / §25.7.1)

Per spec.md §23.4 the rustc `Drop` Terminator collapses inside the
atomic per-rustc-terminator translation envelope:

1. Translate the drop target Place via `translate_place` (regseq-driver-41).
2. Resolve the Ty being dropped from the body's local-decl table.
3. Look up the drop-glue ItemHandle from
   `BridgeCtxt.drop_glue_synth_cache[ty_id]`. If absent, eagerly call
   `bridge::synth::synthesise_drop_glue(ty_id)` (regseq-driver-40).
4. Emit a `TerminatorKind::Call` whose callee is `CallCallee::Direct`
   targeting the drop-glue ItemHandle, with the drop target Place as
   the single argument.
5. Preserve the rustc successor edge as the Call's normal-return target.

This collapse runs INSIDE the per-rustc-terminator translation envelope
(per spec.md §3.4 Phase 5 atomicity), not as a separate post-pass.

## Non-Goals

- Body materialisation + LocalDecls allocation API (regseq-driver-43).
- Vtable-indirect rewrite (regseq-driver-43, spec.md §25.7.2 -- runs
  AFTER Drop-as-Call collapse).
- Joint validation walk wiring (regseq-driver-43).
- Diagnostic-logging recognition (regseq-driver-44).
- DiagLogPeel + format-template parsing (regseq-driver-45).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/body/terminator.rs`, `basic_block.rs` `#[cfg(test)] mod tests`)

- For each of the 6 closed TerminatorKind variants per
  regseq-model-80, hand-construct a rustc Terminator that exercises
  the variant; assert the translated TerminatorKind matches.
- BasicBlock translation: hand-construct a basic block with N
  statements + a terminator; assert the translated BasicBlock contains
  N statements and the translated terminator.

### Integration tests (`crates/regseq-driver/tests/bridge_terminator.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per non-Drop §23.7 row):

- Goto -- a function with simple sequential control flow whose first
  block ends in Goto.
- SwitchInt -- a function with an `if` or `match` on an integer.
- Return -- every function trivially exercises Return.
- Unreachable -- a function with `unreachable!()`.
- Call (Direct) -- `pub fn f() { g(); } pub fn g() {}`.
- Assert -- a function with arithmetic that triggers a Rust overflow
  assertion in debug builds.

Drop-as-Call collapse coverage:

- Non-Copy drop fixture -- `struct S(u32); impl Drop for S { fn drop(&mut self) {} } pub fn f() { let _s = S(0); }`
  -- assert (a) **no rustc Drop Terminator survives** in the
  RegseqModel; (b) the drop site is a `TerminatorKind::Call`; (c) the
  Call's callee is `CallCallee::Direct` against the drop-glue Item id
  for `S`; (d) the drop-glue Item is registered in
  `RegseqModel.item_synthetic_kinds` per regseq-driver-40.
- Two consecutive non-Copy drops in the same body -- assert two Call
  Terminators against the same drop-glue Item id (cache hit per
  regseq-driver-40's drop-glue dedup).

Negative coverage (one fixture per spec.md §23.7 rejection row):

- A fixture using a TerminatorKind form that §23.7 lists as rejected
  (e.g., InlineAsm if rejected per §23) -- assert
  `BridgeError::DslRejection`.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-42 against its task spec at
.harness/tasks/regseq-driver/42-terminator-and-drop-as-call-bridge.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/body/terminator.rs
- crates/regseq-driver/src/bridge/body/basic_block.rs
- crates/regseq-driver/src/bridge/body/mod.rs
- crates/regseq-driver/tests/bridge_terminator.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §23.4 / §23.7 / §24.7 / §25.7.1.
- Per-TerminatorKind row coverage.
- Drop-as-Call collapse runs INSIDE the atomic per-terminator
  translation envelope (not a separate post-pass).
- No `Drop` variant survives in RegseqModel post-collapse.
- Drop-glue cache lookup falls back to eager synthesis via
  regseq-driver-40's `synthesise_drop_glue`.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Vtable-indirect rewrite is regseq-driver-43.
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

- `crates/regseq-driver/src/bridge/body/{terminator,basic_block}.rs`
  created and wired from `bridge/body/mod.rs`.
- Per-TerminatorKind translation per spec.md §23.7.
- Drop-as-Call collapse per spec.md §23.4 -- no `Drop` variant in
  RegseqModel post-bridge.
- Each translation rule covered by a positive test.
- Drop-as-Call covered by a non-Copy drop fixture.
- All tests pass; Codex Task Review verdict APPROVED.
