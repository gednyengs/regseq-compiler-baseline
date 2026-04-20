# regseq-linux-backend-03: `LinuxBackendPlan` Self-Sufficiency

**Depends on**: `regseq-lowering-10`.

## Description

Phase-7B Linux plan cleanup. Fixes the leaky `LinuxBackendPlan`
boundary identified in migration.md Q7. Today, `emit_linux_artifacts`
takes BOTH `&LinuxBackendPlan` AND `&DriverLoweringPlan`
(`linux/emitter.rs:63-76`), and `emit_module_integration` reads
semantic behavior + resource facts out of `portable.behavior`
(`linux/emit_module.rs:33-37`). This means `LinuxBackendPlan` is
not a complete emission contract; it's a Linux-policy overlay on
top of the portable plan.

This task makes `LinuxBackendPlan` **self-sufficient for emission**.
`emit_linux_artifacts` takes only `&LinuxBackendPlan` + a body
lookup (either a `&BTreeMap<FunctionId, &Body>` or a `&RegseqModel`
for on-demand resolution). Linux-specific decisions all flow through
the plan; no re-derivation from portable-plan internals.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q7
  (recommended Option A -- materialize refs/handles the emitter
  reads, not full body copies; Linux plan becomes a real target
  IR).
- `.harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md`
  -- the upstream `DriverLoweringPlan` trim this task assumes.
- `crates/regseq-backends/src/linux/emitter.rs` lines 63-105 --
  the current dual-plan signature.
- `crates/regseq-backends/src/linux/emit_module.rs` lines 33-115,
  200-647 -- the current reads from `portable.*`.
- `crates/regseq-backends/src/linux/plan.rs` lines 23-525 -- the
  struct that becomes self-sufficient.
- `crates/regseq-backends/src/linux/planner.rs` lines 37-65 --
  the planner that populates it.
- `crates/regseq-backends/tests/{pl011_full_spec_lowering,pl011_lowering_completeness_proof,lowering_boundary}.rs`
  -- tests that assert on emitter signatures; this task OWNS
  their update per Codex round-2 MANDATORY fix.

## Implementation Notes

### Signature changes

```rust
// Before (current):
pub fn emit_linux_artifacts(
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    out_dir: &Path,
) -> Result<EmittedArtifactSet, LinuxEmitError> { ... }

// After:
pub fn emit_linux_artifacts(
    plan: &LinuxBackendPlan,
    bodies: &FunctionBodyIndex<'_>,
    out_dir: &Path,
) -> Result<EmittedArtifactSet, LinuxEmitError> { ... }

// Where:
pub struct FunctionBodyIndex<'a> {
    // Borrowed from RegseqModel; backend never copies bodies.
    inner: BTreeMap<FunctionId, &'a ir::Body>,
}

impl<'a> FunctionBodyIndex<'a> {
    pub fn from_regseq_model(model: &'a RegseqModel) -> Self;
    pub fn get(&self, id: FunctionId) -> Option<&'a ir::Body>;
    // (Plus whatever minimal accessor shape emit_semantic / emit_module need.)
}
```

All data the emitter reads from `portable` today either:

1. Migrates INTO `LinuxBackendPlan` as a Linux-planner-computed
   field (Linux-policy decisions go here).
2. Is accessed via `FunctionBodyIndex` (raw CFG bodies go here,
   borrowed from `RegseqModel`).
3. Derives inline in the emitter from (1) + (2) without touching
   the portable plan.

### `LinuxBackendPlan` additions

For every portable-plan read in `emit_module.rs`, audit whether the
datum is:

- **Pure Linux policy** (e.g., callback naming, probe-step
  sequencing already mapped to Linux APIs): should already live on
  `LinuxBackendPlan`. If not, promote it.
- **Raw CFG data** (e.g., function body contents): access via
  `FunctionBodyIndex`.
- **Portable obligation** (e.g., resource field names, interrupt
  IDs): should be on `LinuxBackendPlan` as a Linux-adapted form
  (e.g., the Linux-side interrupt identifier, not the portable ID).

Representative migrations (incomplete; audit the full call graph):

- Reading `portable.resources.mmio[i].field_name` in
  `emit_module.rs:212` -> promote to
  `LinuxBackendPlan.registration` or `serial_core` as already-adapted
  Linux fields.
- Reading `portable.behavior.functions[i].body` in
  `emit_module.rs:317-318` -> replace with
  `bodies.get(function_id)` where `function_id` is carried on
  `LinuxBackendPlan.behavior_mapping`.
- Reading `portable.callbacks.serial[i].context` in planner
  (`linux/planner.rs:585`) -> already happens at planner level;
  the planner copies the useful bit into `LinuxBackendPlan`.

### Planner update

`build_linux_backend_plan(&DriverLoweringPlan, &LinuxPlannerConfig)`
keeps its input signature (takes the portable plan). Output:
`LinuxBackendPlan` that carries every Linux decision the emitter
needs, no references back into the portable plan.

### Test rewrites (explicit ownership per Codex round-2)

- `crates/regseq-backends/tests/pl011_full_spec_lowering.rs`
  tests that pass both plans to the emitter: update to pass
  `(LinuxBackendPlan, FunctionBodyIndex)`.
- `crates/regseq-backends/tests/pl011_lowering_completeness_proof.rs`
  same pattern.
- `crates/regseq-backends/tests/lowering_boundary.rs` same
  pattern.

### Grep gate

After this task:

```
rg -n 'emit_linux_artifacts\(.*portable\s*:' crates/regseq-backends/
# must return zero matches (old dual-plan signature gone)
rg -n '&DriverLoweringPlan' crates/regseq-backends/src/linux/
# limited to linux/planner.rs (its input); zero in linux/emit*
```

### What this task does NOT do

- Does not yet migrate emitters to walk CFG bodies (tasks 9/10).
  Emitters still string-format bodies, but now they receive bodies
  via `FunctionBodyIndex` instead of by reaching into
  `portable.behavior`.
- Does not delete `lowering/behavior_planner.rs` etc. (task 14).
- **Does not modify `crates/regseq-driver/src/construction*`.**

## Non-Goals

- No CFG emitter work (tasks 9/10).
- No Linux-policy semantic changes -- emitters continue to
  produce the same C they did pre-task (minus the boundary
  cleanup).
- No Zephyr work.

## Test Plan

- Emitter signature change compiles; existing tests updated.
- **Output-parity smoke**: run the existing
  `pl011_linux_module_host_build_proof.rs` before and after the
  task; verify the generated `.c` files are byte-identical. This
  confirms zero semantic drift from the boundary cleanup.
- `cargo test -p regseq-backends` green.
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-03 against .harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md.

Files to audit:
- crates/regseq-backends/src/linux/emitter.rs (signature)
- crates/regseq-backends/src/linux/emit_module.rs (no more portable reads)
- crates/regseq-backends/src/linux/plan.rs (self-sufficient)
- crates/regseq-backends/src/linux/planner.rs (populates new fields)
- Tests under crates/regseq-backends/tests/ that exercise the emitter

Focus on:
- emit_linux_artifacts signature takes only (LinuxBackendPlan,
  FunctionBodyIndex, out_dir).
- No emit_* function reads from portable; grep confirms zero.
- Output-parity smoke passes: existing host-build-proof PL011
  output is byte-identical to pre-task baseline.
- FunctionBodyIndex borrows from RegseqModel, no copies.

Do NOT flag:
- Emitters still string-formatting bodies (tasks 9/10).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `emit_linux_artifacts` signature: `(plan: &LinuxBackendPlan,
  bodies: &FunctionBodyIndex<'_>, out_dir: &Path) -> ...`.
- `FunctionBodyIndex` public type defined with `from_regseq_model`
  + `get`.
- No `emit_*` function reads fields from `&DriverLoweringPlan`
  (grep verified).
- `LinuxBackendPlan` carries every Linux-side decision the emitter
  consumes.
- Existing PL011 host-build-proof produces byte-identical output
  pre/post task.
- Tests updated in the same commit.
- Codex review APPROVED.
