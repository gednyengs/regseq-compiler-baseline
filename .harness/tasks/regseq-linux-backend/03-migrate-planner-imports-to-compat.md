# regseq-linux-backend-03: Migrate Linux Planner + Plan Imports to `regseq_model::compat`

## Description

Phase-7 consumer migration task 3 of 4. Rewrites every
`regseq_model::{declaration, item, semantic}` import in
`crates/regseq-backends/src/linux/planner.rs` and
`crates/regseq-backends/src/linux/plan.rs` to
`regseq_model::compat`. No behavior changes; workspace stays green.

**Scope is exactly two files**: `linux/planner.rs` and
`linux/plan.rs`. `plan.rs` is in scope because it carries a
`FunctionRole` field on `LinuxUartOpsBody` that the planner reads
(per Codex review of Phase-7 decomposition). Emitter / `emit_*.rs`
/ `c_ast.rs` files are explicitly deferred to Phase 8.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` lines 12-19
  ("Linux backend planner -- model inputs only"); lines 104-109
  (Linux backend boundary block); lines 145-147 (Linux planner
  grep gate, with scope corrected to the two files per Codex
  review).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  and Phase 8 sections (Phase 8 owns emitter + emit_*.rs +
  c_ast.rs rewrite).
- `.harness/tasks/regseq-lowering/10-migrate-planner-imports-to-compat.md`
  -- upstream task; this task's template mirrors its shape.
- `.harness/tasks/regseq-model/61-compat-scaffold.md` -- the
  re-exports and view surface this task consumes.
- `crates/regseq-backends/src/linux/planner.rs` lines 6-8 --
  current import block; other lines use the imported types for
  matching / keying.
- `crates/regseq-backends/src/linux/plan.rs` lines 426-435 --
  `LinuxUartOpsBody` stores `FunctionRole`; the type reference
  is the reason this file is in scope.

## Implementation Notes

### Import-path-only migration

In `linux/planner.rs`:

- `use regseq_model::declaration::driver::DeviceClass;` → `use
  regseq_model::compat::DeviceClass;`.
- `use regseq_model::item::function::{DeviceCallbackRole,
  DeviceClassKind, FunctionRole};` → `use
  regseq_model::compat::{DeviceCallbackRole, DeviceClassKind,
  FunctionRole};`.
- Any other `regseq_model::{declaration, item, semantic}::*`
  import in this file gets the same treatment.

In `linux/plan.rs`:

- Swap `regseq_model::item::function::FunctionRole` (and any
  other model-input type that appears in the file) to
  `regseq_model::compat::*`.
- Struct/enum definitions in `plan.rs` keep their shape; only
  import paths change. `LinuxUartOpsBody { role: FunctionRole,
  ... }` continues to compile because `FunctionRole` still
  resolves to the same underlying enum via the compat re-export.

### Siblings explicitly deferred to Phase 8

Do **not** touch these files:

- `crates/regseq-backends/src/linux/emitter.rs`
- `crates/regseq-backends/src/linux/emit_behavior.rs`
- `crates/regseq-backends/src/linux/emit_module.rs`
- `crates/regseq-backends/src/linux/emit_regs.rs`
- `crates/regseq-backends/src/linux/emit_semantic.rs`
- `crates/regseq-backends/src/linux/c_ast.rs`
- Any other `linux/` file beyond `planner.rs` and `plan.rs`.

These still import `regseq_model::{declaration, semantic, item}`
and will migrate in Phase 8 as part of the emission rewrite.
Their continued presence is expected and does not violate the
Phase-7 acceptance.

### Grep acceptance (scoped to two files)

```
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' \
   crates/regseq-backends/src/linux/planner.rs \
   crates/regseq-backends/src/linux/plan.rs
```

must return zero matches.

The broader `crates/regseq-backends/src/linux/` directory will
still show matches from the deferred emitter files; that is
expected.

### What this task does NOT touch

- No changes to any file outside the two in scope.
- No signature changes on `build_linux_driver_plan` or any
  `plan.rs` public struct.
- No changes to `AnalysisInput` or upstream crates.
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Non-Goals

- No Phase-8 work (emitter / emit_*.rs / c_ast.rs rewrite).
- No changes to sibling linux files.
- No backend semantic changes -- the emitted C remains bitwise
  identical.
- No Zephyr backend work.
- No fixture / golden file changes.

## Test Plan

- `cargo test -p regseq-backends` passes unchanged.
- `cargo test --workspace` passes.
- Grep gate:

  ```
  rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' \
      crates/regseq-backends/src/linux/planner.rs \
      crates/regseq-backends/src/linux/plan.rs && echo FAIL || echo OK
  rg -n 'regseq_model::compat' \
      crates/regseq-backends/src/linux/planner.rs \
      crates/regseq-backends/src/linux/plan.rs
  ```

- Output-parity check: run an existing Linux E2E test that emits
  C (e.g., `pl011_driver_emission.rs` under `tests/e2e/tests/`)
  before and after the task lands; both runs produce byte-identical
  generated C. This confirms zero behavioral drift.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-03 against
.harness/tasks/regseq-linux-backend/03-migrate-planner-imports-to-compat.md.

Files to audit:
- crates/regseq-backends/src/linux/planner.rs
- crates/regseq-backends/src/linux/plan.rs

Focus on:
- Grep gate: zero `regseq_model::(declaration|binding|joined|semantic|item)`
  matches in either file.
- Emitter and emit_*.rs files untouched.
- Linux backend continues to emit byte-identical C on the PL011
  fixture.
- No edits to crates/regseq-driver/src/construction*.

Do NOT flag:
- emitter.rs / emit_*.rs / c_ast.rs still importing old types
  (Phase 8).
- Missing `from_regseq_model` ctor on compat views (Phase-7 task
  regseq-driver-35).
</task>
... structured_output_contract per SESSION.md ...
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

- `crates/regseq-backends/src/linux/planner.rs` and
  `crates/regseq-backends/src/linux/plan.rs` contain zero
  `regseq_model::{declaration, binding, joined, semantic, item}`
  imports.
- Each file imports at least one `regseq_model::compat::*`
  symbol.
- `build_linux_driver_plan` and all public structs in `plan.rs`
  retain their pre-task signatures / field types (nominally the
  same; the underlying type resolves through compat).
- `cargo test --workspace` passes.
- Linux backend emits byte-identical generated C vs pre-task
  baseline on PL011.
- `git diff --name-only main..HEAD` touches only the two files
  and (possibly) `Cargo.lock`.
- Codex review approved.
