# Task: regseq-lowering-02 - PL011 supported-slice lowering into plans

## Description

Implement the first real lowering path from the stabilized PL011 lowering input
into `DriverLoweringPlan` and `LinuxBackendPlan`.

This task should lower the accepted `tests/fixtures/pl011-full-spec` slice into
typed portable and Linux-specific plans that later backend-emission work can
consume without re-deriving core driver semantics from raw frontend models.

## Input Context

Read before starting:

- `.harness/tasks/regseq-lowering/01-lowering-module-scaffold-and-typed-plan-boundary.md`
- `docs/discussions/linux-codegen/pl011-linux-first-slice.md`
- `docs/discussions/linux-codegen/linux-driver-layout-strategy.md`
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `archive/guide/lowering-contract.md`
- `tests/fixtures/pl011-full-spec/`
- `tests/e2e/tests/pl011_lowering_input_stability.rs`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`
- `tests/e2e/tests/` only if a reviewer-facing lowering snapshot/proof is
  needed

Add the first supported PL011 lowering implementation that turns the stable
analysis-side PL011 input into typed portable and Linux backend plans.

## Implementation Notes

This task should treat `tests/fixtures/pl011-full-spec` as the required target.
It should not reduce the target to a smaller toy slice.

The portable plan for the supported PL011 slice should explicitly cover:

- driver identity and provenance
- callback inventory and structured callback-body actions
- serial device-class role obligations
- MMIO-backed register-block realization intent
- interrupt registration and ISR intent
- firmware-property reads with fallback-to-authored-default behavior
- optional clock participation
- optional reset participation
- optional pinctrl participation
- synchronization and ordering obligations
- diagnostic logging obligations
- timed polling and delay obligations already justified by the accepted slice
- bus identity as `amba`, not as Linux-flavored `platform`

The Linux-specific plan should derive Linux realization choices for the same
slice, including:

- `amba_driver`-shaped registration intent for PL011
- serial-core realization intent (`uart_driver`, `uart_ops`, relevant
  per-port/probe structures)
- Linux resource-acquisition and teardown sequences
- Linux callback realization structure for lifecycle, ISR, and serial methods
- Linux-specific file partitioning intent for emitted output

The task should align with the newer PL011 bus decision:

- PL011 remains `compatible = ["arm,pl011"]`
- PL011 portable bus identity is `bus = "amba"`
- Linux planning should reflect AMBA-aware realization rather than a generic
  `platform_driver` interpretation

Do not turn this task into:

- emitted C source generation
- build-system packaging
- guest/QEMU validation
- PM, DMA, modem-control, RS-485, loopback, IrDA, or console-takeover work
- a fully general multi-device lowering framework

## Test Plan

Add tests to verify:

1. the concrete PL011 lowering input lowers successfully into
   `DriverLoweringPlan`
2. the resulting portable plan carries the required resource, callback, and
   ordering obligations
3. Linux planning derived from the portable plan preserves AMBA-specific bus
   identity for PL011
4. optional resource participation and fallback/default policies remain visible
5. the resulting plans are deterministic and reviewer-friendly

Test commands:

- `cargo test -p regseq-backends`
- `cargo test -p regseq-e2e -- pl011_lowering_input_stability`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- the PL011 full-spec fixture lowers into a typed portable plan and a typed
  Linux backend plan
- the portable plan carries real compiler-owned semantics, not just summaries
- the Linux plan captures AMBA-shaped PL011 realization intent
- plan output is deterministic enough for later emission and review
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
