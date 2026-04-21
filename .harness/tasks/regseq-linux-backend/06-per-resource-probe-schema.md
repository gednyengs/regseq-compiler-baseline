# regseq-linux-backend-06: Per-Resource Linux Probe/Emission Schema

**Depends on**: `regseq-linux-backend-03`, `regseq-lowering-10`.

## Description

Post-Phase-7B device-generality fix. Replaces PL011-shaped
single-resource assumptions in `LinuxProbeStep`, `LinuxBackendPlan`,
and `emit_module.rs` probe / register-accessor emission with a
per-resource schema that carries the authored field name for every
MMIO region, interrupt, and clock, and supports per-clock
optionality.

Source of the findings this task addresses:
`.harness/spec-batches/device-generality-cleanup.md` Findings 2,
3, 4, 5, 6. Specifically:

- `emit_module.rs:258-274` register accessors hardcode `d->regs`
  (Finding 2).
- `emit_module.rs:686-697` `MapMmio` hardcodes `d->regs` and
  platform resource index 0 (Finding 3).
- `emit_module.rs:657-672` `GetClock` hardcodes `d->clock` and
  passes `NULL` as the consumer-name hint; `clock_id` is
  discarded in the rendered C (Finding 4).
- `emit_module.rs:719-732` `RequestIrq` hardcodes `d->irq` and
  IRQ index 0 (Finding 5).
- `emit_module.rs:745-758` + `plan.rs::has_optional_clock` encode
  "any clock is optional" as a single bit, which cannot express
  mixed-optionality multi-clock drivers (Finding 6).

The task also populates `MmioObligation.register_block_type` in
the model-era portable planner
(`from_model.rs:201-208` currently hardcodes `None`) so the
register-accessor helpers can identify which MMIO owns each
register block in multi-MMIO drivers.

This task does **not** fix the ReadProperty probe step (Finding
13, owned by `regseq-linux-backend-07`), the `uart_driver.nr` or
register-prefix issues (Findings 7 and 8, owned by
`regseq-linux-backend-05`), the Option pointer detection issue
(Finding 9, owned by `regseq-linux-backend-04`), or the AMBA mask
issue (Finding 1, owned by `regseq-lowering-12`).

## Input Context

- `.harness/spec-batches/device-generality-cleanup.md` -- the
  brief this task implements, Findings 2-6 and the Task A
  scope section.
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  -- the prior boundary-cleanup task that introduced
  `has_optional_clock`; this task deletes that field.
- `.harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md`
  -- the model-era portable planner; this task extends
  `from_model.rs` to populate `register_block_type`.
- `crates/regseq-backends/src/linux/plan.rs` -- `LinuxProbeStep`,
  `LinuxRegisterBlock`, `LinuxBackendPlan.has_optional_clock` (to
  delete).
- `crates/regseq-backends/src/linux/planner.rs` lines 43-69 (plan
  construction), 207-259 (`build_probe`), 382-432
  (`build_register_layout`) -- the planner code this task
  updates.
- `crates/regseq-backends/src/linux/emit_module.rs` lines 258-274,
  657-758 -- the emission sites that consume the new schema.
- `crates/regseq-backends/src/lowering/plan.rs` lines 127-163 --
  `MmioObligation`, `InterruptObligation`, `ClockObligation`;
  confirm `field_name` is already populated on every variant
  (no portable-plan type change required).
- `crates/regseq-backends/src/lowering/from_model.rs` lines
  195-230 -- where the model-era planner currently emits
  `register_block_type: None`.
- `crates/regseq-model/src/model/resources.rs` lines 220-228 --
  `MmioDecl { mmio_id, register_block: RegisterBlockId }`. The
  register block is referenced by typed ID, not by type-name
  string; the model-era planner must resolve the ID to a block
  type name via the module's register-block table (see
  `Module::register_blocks`).

## Implementation Notes

### Plan type changes

In `crates/regseq-backends/src/linux/plan.rs`:

1. Delete `LinuxBackendPlan.has_optional_clock: bool`. The field
   is replaced by per-clock optionality on each `GetClock` step
   (see below).

2. Extend `LinuxProbeStep` variants so every resource-acquisition
   step carries the authored driver-struct field name:

```rust
pub enum LinuxProbeStep {
    MapMmio {
        mmio_id: String,
        /// Driver-struct field name for the MMIO pointer (e.g. "regs").
        field_name: String,
        /// Resource index passed to `platform_get_resource`.
        /// Always `0` today; explicit on the plan for
        /// multi-MMIO drivers.
        index: u32,
    },
    RequestIrq {
        interrupt_id: String,
        /// Driver-struct field name for the IRQ number.
        field_name: String,
        /// Resource index passed to `platform_get_irq`.
        index: u32,
    },
    GetClock {
        clock_id: String,
        /// Driver-struct field name for the `struct clk *`.
        field_name: String,
        /// Whether this clock is optional.
        optional: bool,
        /// Consumer-name hint for `devm_clk_get{,_optional}`.
        /// `None` when the driver does not declare one; the
        /// emitter renders `NULL` in that case.
        name_hint: Option<String>,
    },
    DeassertReset { /* unchanged */ },
    SelectPinCtrl { /* unchanged */ },
    ReadProperty { /* unchanged -- task 07 owns this step's
        shape */ },
    RegisterUartPort,
}
```

3. Extend `LinuxRegisterBlock` with:

```rust
pub struct LinuxRegisterBlock {
    pub block_name: String,
    pub reg_prefix: String,
    /// Driver-struct field name for the MMIO pointer that owns
    /// this register block (e.g. "regs"). The emitter renders
    /// `d->{owning_mmio_field}` in register-accessor helper
    /// bodies.
    pub owning_mmio_field: String,
    pub fields: Vec<LinuxRegisterFieldEntry>,
}
```

### Planner changes

In `crates/regseq-backends/src/linux/planner.rs`:

1. `build_probe` (currently lines 207-259) reads
   `plan.resources.mmio[i].field_name`,
   `plan.resources.interrupts[i].field_name`,
   `plan.resources.clocks[i].field_name` and emits the
   `field_name` on each `LinuxProbeStep` variant. The `index`
   passed to `platform_get_resource` / `platform_get_irq` is the
   **per-family ordinal** -- the position of this resource among
   the portable plan's `resources.mmio` (or `resources.interrupts`)
   vector, matched by `resource_id`. It is NOT the
   `AcquisitionStep.order` field, which is global across all
   families (clocks, resets, pinctrl, MMIO, properties,
   interrupts) and would not correspond to the Linux
   `platform_get_resource` index space. Concretely: for each
   `ResourceKind::Mmio` or `ResourceKind::Interrupt` entry in
   `plan.acquisition_order`, look up the resource by its
   `resource_id` in the corresponding per-family vector and use
   that vector's index as `LinuxProbeStep.index`. For clocks,
   `name_hint` comes from the portable `ClockObligation.clock_id`
   when the authored declaration carries a consumer name; `None`
   otherwise. The initial implementation MAY use the `clock_id`
   string as the hint for single-clock drivers; authors opting
   into multi-clock are expected to declare distinct consumer
   names in a follow-up authoring-surface task (out of scope
   here -- today's portable plan does not yet carry `name_hint`
   independently from `clock_id`).

2. `build_register_layout` (currently lines 382-432) resolves each
   register block's owning MMIO by reading
   `MmioObligation.register_block_type` on the portable plan and
   matching it to the block's `name`. When zero or multiple MMIO
   entries have a `register_block_type` that matches a given
   block, `build_linux_backend_plan` fails with:

```rust
LinuxPlanError::AmbiguousRegisterBlockOwnership {
    block_name: String,
    candidates: Vec<String>,  // MMIO field_names that matched
}
```

3. Delete the `has_optional_clock = plan.resources.clocks.iter().any(|c| !c.required)`
   computation (currently in `build_linux_backend_plan`).

### Portable planner (model-era) changes

In `crates/regseq-backends/src/lowering/from_model.rs`:

1. Lines 195-210 (`mmio` collection builder) currently emit
   `register_block_type: None`. Replace with a lookup that:
   (a) reads `m.decl.register_block: RegisterBlockId` on each
   `MmioResource`, (b) resolves the ID to the corresponding
   `RegisterBlock` via the driver module's register-block table
   (walk `Module::register_blocks` for the driver's containing
   module, match by ID), and (c) extracts the block's authored
   type-name symbol and interns it as a `String`. Set
   `register_block_type: Some(type_name_string)` when the lookup
   succeeds. If the ID does not resolve (malformed model;
   verifier should have caught it), fail
   `LoweringError::InvariantViolated`. The portable plan field is
   already `Option<String>`; this task populates it, it does not
   change the type.

2. No other portable-plan field types change in this task.

### Emitter changes

In `crates/regseq-backends/src/linux/emit_module.rs`:

1. `emit_register_accessors` (currently lines 258-297) receives
   the `owning_mmio_field` per-block from
   `plan.register_layout.blocks`. When a driver has a single
   register block (PL011 today), the helpers still emit
   `d->{field}` with `field` resolved from the plan -- for PL011
   that resolves to `"regs"`, preserving byte parity. When a
   driver has multiple blocks, one set of accessor helpers is
   emitted per block, each dereferencing its `owning_mmio_field`.
   The helper names stay `{snake}_{block_prefix}_reg_read/write/modify`
   (task 05's amendment handles the multi-block naming; this
   task emits one `{owning_mmio_field}` per helper).

2. `emit_probe` (currently lines 654-830) renders
   `d->{field_name}` for every `MapMmio`, `RequestIrq`, `GetClock`
   step. For `GetClock`, the `name_hint` renders as a C string
   literal when present, `NULL` when absent.

3. The probe's `uartclk` ternary at lines 745-758 (today guarded
   by `plan.has_optional_clock`) becomes per-clock: the probe
   iterates `plan.probe.steps` and, for each `GetClock` step with
   `optional == true`, emits the ternary using that clock's
   `field_name`. A driver with one required and one optional
   clock gets two distinct initializer lines, one per clock.

### Error type extension

In `crates/regseq-backends/src/linux/planner.rs::LinuxPlanError`:
add `AmbiguousRegisterBlockOwnership` variant as shown above.

### Grep gates

After this task:

```
rg -n 'd->regs|d->clock|d->irq' crates/regseq-backends/src/linux/emit_module.rs
# must return zero matches except inside string-building calls
# that substitute `{field_name}` placeholders (verify by eye)

rg -n 'has_optional_clock' crates/regseq-backends/src/
# must return zero matches (field and all consumers deleted)

rg -n 'register_block_type: None' crates/regseq-backends/src/lowering/
# must return zero matches (all call sites populate it)
```

### What this task does NOT do

- Does not change `DriverLoweringPlan` struct-field types.
  `field_name`, `register_block_type` already exist in the
  portable plan; this task only populates
  `register_block_type` in the model-era path.
- Does not modify `emit_semantic.rs` or the semantic body
  walker (owned by task 04).
- Does not delete `emit_behavior.rs` (owned by task 05).
- Does not fix the ReadProperty probe step (owned by task 07).
- Does not fix `uart_driver.nr = 1` or register-prefix-first
  (owned by task 05's amendment).
- Does not add any new authoring-surface attribute. Authors
  declaring multi-clock / multi-MMIO drivers use the existing
  `#[rseq::...]` attributes.

## Non-Goals

- No Zephyr backend work.
- No authoring surface changes.
- No changes to PL011 fixture inputs. PL011 continues to exercise
  the single-resource code path; the new multi-resource paths are
  exercised by the test fixture added below.
- No performance or optimization work.
- No deletion of the legacy lowering path (owned by
  `regseq-lowering-11`).

## Test Plan

### Unit tests

1. `LinuxPlanError::AmbiguousRegisterBlockOwnership` is raised
   when two MMIO obligations carry the same
   `register_block_type`. New test in
   `crates/regseq-backends/tests/linux_emission_scaffold.rs` or
   a new `crates/regseq-backends/tests/multi_resource_plan.rs`
   (author's choice; keep file size under the 500-line
   convention).

2. `build_probe` emits `field_name` on every `MapMmio` /
   `RequestIrq` / `GetClock` step for a hand-built portable plan
   with non-default field names (`mmio_field = "memory"`,
   `interrupt_field = "hw_irq"`, `clock_field = "baud_clk"`).

3. `build_register_layout` sets `owning_mmio_field` to the
   field name of the MMIO whose `register_block_type` matches
   the block's `name`. Exercise with a two-MMIO, two-block
   portable plan where each MMIO owns one block.

### Output-parity smoke (PL011)

4. Re-run the PL011 emission scaffold
   (`crates/regseq-backends/tests/linux_emission_scaffold.rs`)
   with the capture/compare pattern used in task 03: emit PL011
   before and after, diff byte-for-byte, assert equal. The PL011
   fixture has single MMIO, single IRQ, single optional clock;
   after the migration the plan still resolves those fields to
   `"regs"`, `"irq"`, `"clock"`, producing identical C.

### Multi-resource emission test

5. Hand-build a two-MMIO, two-clock, two-IRQ portable plan in
   a new test file. Invoke `build_linux_backend_plan` +
   `emit_linux_artifacts`; assert the generated
   `regseq_generic_uart_module.c` contains
   `d->{mmio_a_field}`, `d->{mmio_b_field}`, both IRQ fields,
   both clock fields. Assert no `d->regs` / `d->clock` /
   `d->irq` collision unless an authored field happens to be
   spelled that way.

### Clippy / format / regression

6. `cargo test --workspace` green.
7. `cargo fmt --all -- --check` clean.
8. `cargo clippy -p regseq-backends -- -D warnings` clean.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-06 against
.harness/tasks/regseq-linux-backend/06-per-resource-probe-schema.md.

Files to audit:
- crates/regseq-backends/src/linux/plan.rs (LinuxProbeStep +
  LinuxRegisterBlock + removed has_optional_clock)
- crates/regseq-backends/src/linux/planner.rs (build_probe,
  build_register_layout, AmbiguousRegisterBlockOwnership)
- crates/regseq-backends/src/linux/emit_module.rs (probe /
  register-accessor emission reads field_name from plan)
- crates/regseq-backends/src/lowering/from_model.rs
  (register_block_type populated)
- crates/regseq-backends/tests/ (new multi-resource test +
  PL011 byte-parity smoke)

Focus on:
- No emit_* reads literal `d->regs` / `d->clock` / `d->irq` (grep
  gate zero).
- has_optional_clock field and all consumers deleted; per-clock
  optionality flows through each GetClock step.
- register_block_type populated in the model-era planner; zero
  `register_block_type: None` writes remain.
- PL011 byte parity preserved.
- Multi-resource portable plans produce correct C without
  field collisions.
- AmbiguousRegisterBlockOwnership error fires for conflicting
  block-to-MMIO mappings.

Do NOT flag anything in the Non-Goals section -- those belong
to tasks 04, 05, 07, or lowering-11.
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate about code
you did not read. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<= 120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a
verdict, stop.
</default_follow_through_policy>
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

- `LinuxProbeStep::{MapMmio, RequestIrq, GetClock}` each carry a
  `field_name: String`; `GetClock` carries `optional: bool` and
  `name_hint: Option<String>`; `MapMmio` and `RequestIrq` carry
  `index: u32`.
- `LinuxRegisterBlock` carries `owning_mmio_field: String`.
- `LinuxBackendPlan.has_optional_clock` deleted; all consumers
  migrated.
- `LinuxPlanError::AmbiguousRegisterBlockOwnership` defined and
  raised on conflicting block-to-MMIO mappings.
- `from_model.rs` populates `MmioObligation.register_block_type`
  from the model; no `register_block_type: None` writes remain
  in the model-era path.
- `emit_module.rs` renders `d->{field_name}` for every resource
  probe step and register accessor; grep gate zero for
  hardcoded `d->regs` / `d->clock` / `d->irq`.
- PL011 emission byte-identical to baseline (capture/compare per
  task 03 pattern).
- Two-MMIO-two-clock-two-IRQ portable-plan test produces
  non-colliding generated C with every authored field name
  present.
- Codex review APPROVED.
