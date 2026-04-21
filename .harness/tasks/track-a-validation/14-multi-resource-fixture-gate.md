# track-a-validation-14: Multi-Resource Fixture Gate

**Depends on**: `regseq-linux-backend-06`,
`regseq-linux-backend-07`, `regseq-lowering-12`,
`regseq-lowering-11`.

## Description

Anchors the device-generality invariants that tasks
`regseq-linux-backend-06`, `-07`, and `regseq-lowering-12`
establish with an executable test that exercises a synthetic
multi-resource driver through the full Regseq pipeline.

Today's PL011 fixture has one MMIO region, one interrupt, one
optional clock, and only `u32`/`bool`-compatible properties. It
exercises the single-resource code paths and hides the shape
assumptions described in
`.harness/spec-batches/device-generality-cleanup.md`. After tasks
06 / 07 / 12 land, no fixture in-tree proves the multi-resource
paths work. PL011 alone cannot regression-test them.

This task adds a synthetic "generic-uart" test fixture with two
MMIO regions (each owning one register block), two interrupts,
two clocks (one required, one optional), and a `bool` property.
The fixture hand-builds a minimal `RegseqModel` and drives the
model-era path directly:

```
hand-built RegseqModel
  -> build_driver_lowering_plan_from_model(&model, &config)
  -> build_linux_backend_plan(&portable, behavior)
  -> emit_linux_artifacts(&linux, &body_index, output_dir)
```

The fixture does NOT go through the upstream `normalize ->
populate_bindings` pipeline (doing so would require authoring
Rust source for the fixture and running the extraction driver,
out of scope). The fixture asserts:

1. Every authored driver-struct field name appears in the
   generated driver-state struct and in the resource-acquisition
   probe body.
2. The `device_property_read_bool` call for the bool property
   resolves to the authored field name.
3. The AMBA mask default diagnostic fires correctly when the
   fixture declares AMBA with no `amba_mask`.
4. No production code under
   `crates/regseq-backends/src/{lowering,linux}/` reads
   `.first()` on `resource_fields` or `mmio_fields`, and no
   literal `"regs".into()` string fallback remains -- the "no
   first-resource fallback" invariant for lowering-path
   resolution, anchored as a three-pattern ripgrep-style
   assertion. Register-layout `.first()` and multi-block
   accessor naming are OUT OF SCOPE (owned by the task-05
   amendment described in the brief).

The fixture lives under `crates/regseq-backends/tests/`, not
`tests/fixtures/`, because it is synthetic and exists solely to
assert structural invariants on generated C; it is never
host-built as a kernel module.

After `regseq-lowering-11` deletes the legacy lowering path, the
multi-resource invariants this fixture tests depend only on the
model-era `from_model.rs` path.

Source: `.harness/spec-batches/device-generality-cleanup.md`
Task D; backs Findings 2-8, 10-13.

## Input Context

- `.harness/spec-batches/device-generality-cleanup.md` Task D
  scope section.
- `.harness/tasks/regseq-linux-backend/06-per-resource-probe-schema.md`
  -- defines the per-resource schema this fixture exercises.
- `.harness/tasks/regseq-linux-backend/07-property-realization-honoring.md`
  -- defines the property-realization enum this fixture's
  `bool` property exercises.
- `.harness/tasks/regseq-lowering/12-amba-mask-binding-override.md`
  -- defines the binding schema this fixture omits to trigger
  the default-applied diagnostic.
- `.harness/tasks/regseq-lowering/11-delete-dead-lowering-submodules.md`
  -- deletes the legacy lowering fallback paths
  (`mir_interpreter/place_resolver.rs`,
  `semantic_lowering/semantic_walker.rs` first-resource logic);
  this task's ripgrep gate asserts the post-deletion state.
- `crates/regseq-backends/tests/linux_emission_scaffold.rs`
  lines 63-870 -- the hand-built PL011 pattern this fixture
  follows for constructing a portable plan from typed inputs.
  Copy the shape; substitute multi-resource values.
- `tests/fixtures/pl011-full-spec/src/` -- PL011 authored
  driver source, for reference only (the new fixture is
  hand-built plan data; no authored Rust source).

## Implementation Notes

### Test file layout

Create
`crates/regseq-backends/tests/multi_resource_fixture_gate.rs`
(new file, under the 500-line convention).

The fixture does NOT author a driver in Rust or run the
extraction pipeline end-to-end from source. It hand-builds a
minimal `RegseqModel` and then drives the model-era portable
planner + Linux backend:

```
RegseqModel (hand-built)
  -> build_driver_lowering_plan_from_model(&model, &config)
  -> build_linux_backend_plan(&portable, behavior)
  -> emit_linux_artifacts(&linux, &body_index, output_dir)
```

The task depends on `regseq-lowering-11`, which deletes the
legacy `build_driver_lowering_plan(&AnalysisInput)` entry. After
that deletion, the model-era path is the ONLY path; there is no
legacy fallback this fixture needs to cover. The fixture MUST go
through the model-era planner specifically -- hand-constructing a
`DriverLoweringPlan` struct directly is explicitly forbidden
because it would skip the exact code path tasks 06 and 12
modify (`from_model.rs`'s MMIO block-type resolution and
AMBA-mask population).

### Fixture shape

Driver identity:
- `driver_name = "generic-uart"`
- `compatible = ["example,generic-uart"]`
- `device_class = DeviceClass::SerialUart`
- `bus_type = BusType::Amba`
- `amba_id = Some(0x00051234)`
- `amba_mask = None` (triggers the default-applied diagnostic
  from task C)

MMIO:
- Region 1: `field_name = "core_regs"`, `mmio_id =
  "uart_core"`, `register_block_type = Some("CoreRegisters")`.
- Region 2: `field_name = "fifo_regs"`, `mmio_id =
  "uart_fifo"`, `register_block_type = Some("FifoRegisters")`.

Interrupts:
- `field_name = "tx_irq"`, `interrupt_id = "tx"`, index 0.
- `field_name = "rx_irq"`, `interrupt_id = "rx"`, index 1.

Clocks:
- `field_name = "bus_clock"`, `clock_id = "apb_pclk"`,
  required.
- `field_name = "baud_clock"`, `clock_id = "uart_baud"`,
  optional.

Property:
- `property_id = "wakeup-source"`, `field_name = "wakeup"`,
  `field_type = "bool"`, required = false, default_literal = None.

Register layout:
- Block `"CoreRegisters"`: one field `dr` at offset 0x00,
  width 32; bitfield `DATA` (offset 0, width 8).
- Block `"FifoRegisters"`: one field `status` at offset 0x00,
  width 32; bitfield `TX_FULL` (offset 0, width 1).

### Assertions

After invoking `build_linux_backend_plan` +
`emit_linux_artifacts` into a tempdir:

1. Generated shared header (`regseq_generic_uart_shared.h`)
   contains fields `core_regs`, `fifo_regs`, `tx_irq`,
   `rx_irq`, `bus_clock`, `baud_clock`, `wakeup`, `port`.
2. Generated module source (`regseq_generic_uart_module.c`)
   contains:
   - `d->core_regs = devm_ioremap_resource(...)`
   - `d->fifo_regs = devm_ioremap_resource(...)`
   - `d->tx_irq = platform_get_irq(pdev, 0)` (or
     `adev->irq[0]` on AMBA -- the fixture is AMBA, so
     `adev->irq[0]` is expected)
   - `d->rx_irq = adev->irq[1]`
   - `d->bus_clock = devm_clk_get(...)` (required path)
   - `d->baud_clock = devm_clk_get_optional(...)` (optional
     path)
   - `d->wakeup = device_property_read_bool(dev, "wakeup-source");`
     (the `AssignReturn` shape per task 07; no `ret` variable,
     no `&d->wakeup` out-pointer, no `if (ret)` branch)
3. No literal `d->regs`, `d->clock`, `d->irq` unless a field
   happens to be named exactly that (the fixture uses
   none-of-the-above names; those literals must not appear).
4. Register accessor helpers are emitted once per register
   block with the correct `d->{owning_mmio_field}`:
   - `static inline u32 generic_uart_core_reg_read(struct
     generic_uart_data *d, unsigned int off) { return
     readl(d->core_regs + off); }`
   - `static inline u32 generic_uart_fifo_reg_read(struct
     generic_uart_data *d, unsigned int off) { return
     readl(d->fifo_regs + off); }`
   (The multi-block accessor-naming policy is defined by task
   05; this task's assertions use whatever name task 05
   settles on, treating the block prefix as part of the
   accessor identifier.)
5. The AMBA match table contains
   `{ .id = 0x00051234, .mask = 0x000f_ffff }` (default mask
   applied because the fixture supplies none).
6. `plan.diagnostics.amba_mask_default_applied` is `true` on
   the portable plan (the fixture declares AMBA without an
   `amba_mask`, triggering the PrimeCell default per task C).

### Ripgrep "no first-resource fallback" gate

A separate assertion in the same test file runs ripgrep
programmatically (via `std::process::Command::new("rg")`)
against the production source tree to assert that no
`first-of-collection` fallback remains in resource resolution.
Specifically:

```rust
#[test]
fn no_first_resource_fallback_in_production() {
    // Patterns owned by tasks 06 + 11 (resource-field /
    // mmio-field single-cardinality fallbacks in the portable
    // lowering path). Register-layout .first() and multi-block
    // accessor naming are OUT OF SCOPE for task 14 (owned by
    // task 05's amendment); those are NOT asserted here.
    let patterns = &[
        r"resource_fields\s*\.\s*first\b",
        r"mmio_fields\s*\.\s*first\b",
        // The literal "regs" fallback string in MMIO resolution
        // (place_resolver.rs's `find_mmio_field_name` -- deleted
        // by regseq-lowering-11).
        r#""regs"\.into\(\)"#,
    ];
    let paths = &[
        "crates/regseq-backends/src/linux/",
        "crates/regseq-backends/src/lowering/",
    ];

    for pat in patterns {
        for path in paths {
            let out = std::process::Command::new("rg")
                .args(["-n", pat, path])
                .output()
                .expect("ripgrep");
            assert!(
                !out.status.success(),
                "pattern `{pat}` matched in `{path}`:\n{}",
                String::from_utf8_lossy(&out.stdout)
            );
        }
    }
}
```

(`rg` exits nonzero when no matches; this test passes when
every pattern-path pair has zero matches.)

Notes on the ripgrep test:

- The test is skipped on systems without ripgrep installed via
  a `which rg` probe. Returning `Ok(())` on a missing tool is
  the correct behavior for a developer-local tool gate that is
  not always installed; the CI configuration that exercises
  this task must ensure ripgrep is present.

- The patterns are intentionally narrow. Do not expand to
  generic `\.first\(\)` patterns or to
  `register_layout.blocks.first` -- multi-block accessor
  emission is explicitly owned by task 05 (the
  `regseq-linux-backend-05` amendment described in the brief
  at `.harness/spec-batches/device-generality-cleanup.md`).
  Task 14 scopes strictly to the lowering-path fallbacks that
  tasks 06/11 eliminate.

### Grep gate (this task's own delivery)

```
rg -n "generic_uart_data\|generic_uart_core_reg_read\|generic_uart_fifo_reg_read" \
    crates/regseq-backends/tests/multi_resource_fixture_gate.rs
# must return at least three matches (one per expected symbol)
```

### What this task does NOT do

- Does not host-build the generic-uart fixture as a kernel
  module. The fixture's purpose is structural assertion on
  emitted C, not kernel-side correctness.
- Does not add an authored driver fixture under
  `tests/fixtures/`. The fixture is hand-built plan data,
  scoped to
  `crates/regseq-backends/tests/multi_resource_fixture_gate.rs`.
- Does not modify the PL011 fixture.
- Does not exercise Zephyr.
- Does not define a second lead fixture. PL011 remains the lead
  reference.

## Non-Goals

- No kernel compilation of the generic-uart artifacts.
- No new authoring surface.
- No changes to PL011.
- No binding-schema or portable-plan type changes beyond what
  tasks 06 / 07 / 12 delivered.
- No changes to `regseq-macros` or any authoring crate.

## Test Plan

The task IS a test. There are no separate unit tests to write
beyond the fixture itself.

Verify:

1. The fixture test in
   `crates/regseq-backends/tests/multi_resource_fixture_gate.rs`
   passes and asserts all six generated-C properties listed
   above.
2. The ripgrep gate test passes (zero matches of the flagged
   patterns in the production source tree, or the test is
   skipped on ripgrep-less systems).
3. PL011 emission-scaffold tests continue to pass (existing
   `crates/regseq-backends/tests/linux_emission_scaffold.rs`).
4. `cargo test -p regseq-backends` green.
5. `cargo test --workspace` green.
6. `cargo fmt --all -- --check` clean.
7. `cargo clippy -p regseq-backends -- -D warnings` clean.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review track-a-validation-14 against
.harness/tasks/track-a-validation/14-multi-resource-fixture-gate.md.

Files to audit:
- crates/regseq-backends/tests/multi_resource_fixture_gate.rs
  (new test file)
- crates/regseq-backends/src/linux/ (production tree; targets
  of the ripgrep gate)
- crates/regseq-backends/src/lowering/ (production tree; same)

Focus on:
- Fixture carries two MMIO, two interrupts, two clocks (mixed
  optionality), one bool property, AMBA with no amba_mask.
- All six generated-C assertions fire correctly.
- plan.diagnostics.amba_mask_default_applied == true on the
  portable plan.
- Ripgrep gate asserts zero matches for exactly three flagged
  shape-assumption patterns (resource_fields.first,
  mmio_fields.first, "regs".into()). Register-layout .first
  patterns are NOT part of task 14.
- Fixture hand-builds a minimal RegseqModel and calls
  build_driver_lowering_plan_from_model; does not hand-
  construct a DriverLoweringPlan directly; does not run
  extraction / normalization.
- PL011 tests remain green.

Do NOT flag absence of kernel build, absence of Zephyr fixture,
or absence of authored Rust source -- all explicitly out of
scope.
</task>

<grounding_rules>
Cite file:line for every claim. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<= 120 words).
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Form a verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends --test multi_resource_fixture_gate
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-backends/tests/multi_resource_fixture_gate.rs`
  exists, contains the multi-resource fixture, and exercises
  all six generated-C assertions.
- The ripgrep gate test passes (or skips gracefully on
  ripgrep-less systems).
- PL011 emission-scaffold tests remain green.
- `plan.diagnostics.amba_mask_default_applied == true` is
  asserted on the portable plan.
- Codex review APPROVED.
