# regseq-linux-backend-05: Retarget `emit_module` to CFG; Delete `emit_behavior`

**Depends on**: `regseq-backends-02`, `regseq-linux-backend-03`,
`regseq-linux-backend-04`, `regseq-linux-backend-08`.

## Description

Phase-7C emitter rewrite (part 2). Retargets `linux::emit_module.rs`
from walking `SemanticModel` / using `BehaviorPlan` fallbacks to
walking CFG `Body` via `FunctionBodyIndex` + `backends::c` AST.
Deletes `linux::emit_behavior.rs` in the same commit -- no consumer
remains after this task.

This task owns the **callback-wrapper / probe / remove / module-init
exemplars and their tests**. Per Codex round-3 review, these are
Linux module-integration policy (not CFG primitives) and therefore
live here, not in task 1 or task 9.

Task 9's transitional adapter (if any) between old
`emit_module` and new `emit_semantic` is retired here -- after
this task, no string-formatted body emission remains.

## Input Context

- `.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md`
  (the contract rules for callback-return adaptation, probe/remove
  orchestration, module_init/module_exit).
- `.harness/tasks/regseq-linux-backend/04-emit-semantic-cfg-rewrite.md`
  (upstream CFG body emitter + adapter discussion).
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  (`FunctionBodyIndex` input the new emit_module consumes).
- `crates/regseq-backends/src/linux/emit_module.rs` lines 1-1116
  -- the module being rewritten.
- `crates/regseq-backends/src/linux/emit_behavior.rs` lines 1-789
  -- the legacy BehaviorPlan emitter, DELETED in this task.
- `crates/regseq-backends/src/linux/plan.rs` (behavior_mapping +
  uart_ops_realization fields populated by task 8) -- how Linux
  decisions flow into this emitter.

## Implementation Notes

### Callback-wrapper exemplars (owned by this task)

Add to `tests/goldens/pl011/linux/constructs/`:

- `callback_wrapper_lifecycle_init.c` -- lifecycle init/remove
  wrapper translating Linux `struct device *` + `ioremap` +
  `request_irq` into the authored `Lifecycle::init` signature.
- `callback_wrapper_serial_configure.c` -- serial configure
  wrapper adapting Linux `struct uart_port *` + `ktermios *` into
  the authored `Serial::configure` signature.
- `callback_wrapper_return_adaptation.c` -- authored
  `Result<(), RegseqError>` return lowered to Linux `int` errno
  per `REGSEQ_ERROR_LINUX_ERRNO`.
- `callback_wrapper_isr.c` -- ISR handler wrapper (`irqreturn_t`
  vs authored `IrqResult`).
- `probe_orchestration.c` -- module_init that calls
  `platform_driver_register` / `amba_driver_register` per plan;
  the generated probe body that consumes `LinuxProbeStep`s and
  emits devm_* sequences.
- `remove_orchestration.c` -- corresponding remove body.
- `module_exit_boilerplate.c` -- module_exit stub.

Each exemplar is small (~30-80 lines); each cites the contract
rule it exemplifies.

### Construct tests for module integration (owned here)

Under `crates/regseq-backends/tests/module_integration_tests.rs`:

For each wrapper / probe / remove exemplar above, a test that:

1. Constructs a minimal `LinuxBackendPlan` + `FunctionBodyIndex`
   + `DriverLoweringPlan` (trimmed, per task 7) matching the
   exemplar's scenario.
2. Invokes `emit_module_integration` (new signature).
3. Asserts normalized output equals the exemplar.

### `emit_module.rs` rewrite

The new `emit_module_integration`:

```rust
pub fn emit_module_integration(
    plan: &LinuxBackendPlan,
    bodies: &FunctionBodyIndex<'_>,
) -> Result<CTranslationUnit, LinuxEmitError>;
```

No `&DriverLoweringPlan` argument (task 8 made the Linux plan
self-sufficient). No string-formatted bodies (`emit_semantic`
produces `CStmt`s). The returned `CTranslationUnit` is assembled
from `CTopLevelItem::FuncDef { signature, body: CStmt::Compound,
comment }` using the expanded AST variant (task 2).

Components:

- **Includes and banner**: via `linux::conventions::generated_file_banner`
  + a `linux::conventions` helper that returns the fixed include
  set (per contract).
- **Driver struct** (`struct <driver>_drvdata`): synthesized from
  `plan.registration` / resource fields.
- **Match table**: synthesized from `plan.registration.compatible_entries`
  or `amba_id` + `amba_mask`.
- **Probe function**: synthesized from `plan.probe.steps` using
  per-step templates from `emission_tables.rs`.
- **Remove function**: synthesized from `plan.remove.steps`.
- **ISR handlers**: each authored ISR emitted via `emit_semantic`
  on the corresponding `FunctionId`; wrapped in an `irqreturn_t`
  adapter.
- **uart_ops table**: generated from `plan.uart_ops_realization`.
- **Serial operations**: each authored callback emitted via
  `emit_semantic` + wrapped per `CALLBACK_RETURN_ADAPTATION`.
- **Helper functions**: every helper in the call graph emitted
  via `emit_semantic`.
- **`module_init` / `module_exit`**: `MODULE_DEVICE_TABLE` +
  `MODULE_LICENSE` + `MODULE_AUTHOR` + `module_*_driver(...)`.

### Callback-return adaptation (single strategy)

For every authored callback returning `Result<(), RegseqError>`,
emit an **inner helper + outer wrapper** pair:

1. **Inner helper**: the authored body, emitted by `emit_semantic`
   unchanged. Return type is the authored `int`-equivalent
   (0 for Ok, negative errno for Err). `emit_semantic` lowers
   `return Ok(())` as `return 0;` and `return Err(e)` as `return
   -EINVAL;` etc., per `REGSEQ_ERROR_LINUX_ERRNO`.
2. **Outer wrapper**: the Linux-signature entry point that
   callers invoke (e.g., the `uart_ops.startup` slot). It calls
   the inner helper, and in the case of callbacks that need an
   adapted return type (e.g., `irqreturn_t` for ISRs, `void` for
   remove), translates the inner `int` return into the outer
   return type.

Rationale for inner+outer vs direct: the authored function may be
reused across multiple callback slots or shared with helper call
sites; the wrapper localizes Linux-specific signature adaptation
without forcing the authored body to know about Linux return
types. This is the single canonical strategy -- do not emit
bodies that directly bake Linux return adaptation into the
authored code's lowering.

### `emit_behavior.rs` deletion

In the same commit:

- `git rm crates/regseq-backends/src/linux/emit_behavior.rs`.
- Remove `pub mod emit_behavior;` from `linux/mod.rs`.
- Remove all imports of `emit_behavior::*` from other modules
  (should be zero callers after `emit_module` rewrite).

### Transitional adapter retirement

If task 9 introduced a shim between old `emit_module` and new
`emit_semantic`, delete it here. After this task, `emit_module`
calls `emit_semantic` directly with the CFG-walk shape.

### Final workspace check

After this task:

- `rg -n 'emit_behavior' crates/` returns zero matches.
- `rg -n 'Vec<String>' crates/regseq-backends/src/linux/` returns
  only the legacy-adjacent `Raw(String)` fallbacks (documented).
- `rg -n 'SemanticBody\|SemanticModel\|Stmt::\|Expr::' crates/regseq-backends/src/linux/`
  returns zero matches.

### What this task does NOT do

- Does not delete `lowering/{behavior_planner, mir_interpreter,
  semantic_lowering, hierarchy, projection, body_interpreter}`
  (task 14).
- Does not delete old model modules (task 15).
- Does not cut over `emit_driver` (task 12).
- **`crates/regseq-driver/src/construction*`** strict non-goal.

### Device-generality constraints (amendment)

Per the post-Phase-7B audit captured in
`.harness/spec-batches/device-generality-cleanup.md`, the CFG
rewrite of `emit_module.rs` must **not** re-introduce the
PL011-shape assumptions flagged as Findings 2, 3, 4, 5, 6, 7,
8, and 13. Specifically:

- **Findings 2, 3 (MMIO)**: register-accessor helpers
  (`{snake}_reg_read/write/modify`) must render
  `d->{owning_mmio_field}` from
  `LinuxRegisterBlock.owning_mmio_field` (added by task 06),
  not a hardcoded `d->regs`. The `MapMmio` probe step consumes
  the per-step `field_name` and `index` the plan carries.
- **Findings 4, 5 (clock, IRQ)**: `GetClock` and `RequestIrq`
  probe emission render the authored `field_name` from each
  `LinuxProbeStep` variant (added by task 06). No hardcoded
  `d->clock` or `d->irq` literals.
- **Finding 6 (per-clock optionality)**: the probe's uartclk
  initializer iterates `plan.probe.steps` and emits one
  ternary per optional clock; there is no plan-wide
  `has_optional_clock` bool (deleted by task 06).
- **Finding 7 (`uart_driver.nr`)**: the current `.nr = 1`
  literal in `emit_uart_driver_def` becomes a plan-driven
  value. This task adds `LinuxSerialCoreRealization.port_count:
  u32` (default 1 for single-port drivers; PL011 preserves byte
  parity).
- **Finding 8 (register prefix)**: the current
  `plan.register_layout.blocks.first().map(...)` idiom at
  `emit_module.rs:295-300` and `emit_regs.rs:51-57` becomes
  per-descriptor resolution -- each emitted bitfield-mask
  define uses the prefix of the block that declares its
  parent register, not the first block.
- **Finding 13 (property realization)**: `ReadProperty` probe
  emission consumes `LinuxPropertyRealization.field_name` and
  `scalar.read_api()` / `scalar.call_shape()` (added by task
  07). If task 07 has already landed when this task runs, the
  emitter uses the typed-enum shape and makes no further
  changes to `ReadProperty`; if this task lands first in some
  alternative landing order, the same code must be authored
  here and task 07 picks it up unchanged.

PL011 byte parity stays mandatory. The brief's guarantee is
that PL011's single-MMIO single-IRQ single-clock shape resolves
to the same emitted strings through the new plan-driven paths.

#### Findings from regseq-linux-backend-08 (RegisterUartPort generality)

Surfaced by a two-source post-cleanup audit (Claude + Codex)
after tasks 06/07/12 landed; addressed by task 08. The CFG
rewrite MUST preserve these four invariants:

- `d->port.membase` is rendered from
  `LinuxSerialCoreRealization.primary_mmio_field`, not a
  first-in-probe-order helper (`primary_mmio_field` / `primary_irq_field`
  functions are deleted by task 08 and must not be re-introduced).
- `d->port.irq` is rendered from
  `LinuxSerialCoreRealization.primary_irq_field`.
- Exactly ONE `d->port.uartclk` assignment line is emitted,
  using `LinuxSerialCoreRealization.uartclk_field` and
  `uartclk_optional`. No iteration over all `GetClock` probe
  steps for uartclk emission.
- `d->port.fifosize` is wired iff a `LinuxPropertyRealization`
  carries `serial_role == Some(SerialPortRole::FifoSize)`; the
  clock-rate default lookup in `emit_uartclk_initializer` uses
  `serial_role == Some(SerialPortRole::ClockRateDefault)`. No
  `property_id` / `field_name` string literals (`"current-speed"`,
  `"fifo-size"`, `"fifo_size"`) in `emit_module.rs`.

See `.harness/tasks/regseq-linux-backend/08-register-uart-port-resource-generality.md`
for the full scope, error variants, and grep-gate spec.

## Non-Goals

- No Phase-8 dominator-cleanup work.
- No structured-recovery emission (Phase-8+ follow-up; Phase-7C
  is goto-based only).
- No Zephyr / other-OS work.

## Test Plan

- **Module integration tests**: every wrapper/probe/remove
  exemplar has a passing test.
- **PL011 end-to-end smoke**: full pipeline on PL011 produces a
  complete translation unit that, after normalization, byte-equals
  `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  (the hand-authored reference from task 1). Any diff localizes
  to an emit_module construct this task owns (since task 9's
  emit_semantic output was already verified in its scope).
- **Host-build proof**: `pl011_linux_module_host_build_proof.rs`
  continues to pass.
- **emit_behavior deletion**: grep confirms the module is gone.
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-05 against
.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md.

Files:
- crates/regseq-backends/src/linux/emit_module.rs (rewritten)
- crates/regseq-backends/src/linux/emit_behavior.rs (expected DELETED)
- crates/regseq-backends/src/linux/mod.rs (module decl removed)
- tests/goldens/pl011/linux/constructs/callback_wrapper_*.c + probe_*.c + remove_*.c + module_exit_*.c (new)
- crates/regseq-backends/tests/module_integration_tests.rs (new)

Focus on:
- emit_module has no Stmt/Expr tree walks, no Vec<String> bodies,
  no imports from removed modules.
- Every wrapper/probe/remove exemplar tested.
- Every authored callback wraps via CALLBACK_RETURN_ADAPTATION.
- emit_behavior.rs fully gone; no consumer remains.
- Host-build proof unchanged (output parity against the blessed
  reference when task 11 runs; for now, parity against pre-task
  output is acceptable IF the pre-task baseline was already
  emit_semantic-based post-task-9).

Do NOT flag:
- Legacy lowering submodules still existing (task 14).
- Old model modules still existing (task 15).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends --test module_integration_tests
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `linux::emit_module.rs` rewritten; consumes
  `(LinuxBackendPlan, FunctionBodyIndex)`, emits via
  `backends::c` AST.
- `linux::emit_behavior.rs` deleted.
- Module integration exemplars + tests landed.
- Callback-wrapper return adaptation emits correct errno mapping.
- `cargo test --workspace` green.
- Host-build proof still passes.
- Codex review APPROVED.
