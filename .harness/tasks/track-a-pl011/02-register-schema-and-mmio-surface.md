# Task: track-a-pl011-02 - Register schema and MMIO surface for the polling slice

## Description

Extend the PL011 spec-slice fixture with the concrete register schema,
 register-block type, and MMIO resource declaration needed by the first
 polling-first bring-up slice.

This task should establish the PL011 register-facing portion of the slice,
 but should stop short of the full driver callback implementation.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/discussions/pl011-spec-slice/pl011-spec-slice-discussion.md
- docs/discussions/register-access/register-access-discussion.md
- .harness/tasks/regseq-macros/02-register-bitfields-helper-macro.md
- .harness/tasks/regseq-facade/03-mmio-resource-surface.md
- .harness/tasks/regseq-facade/04-register-support-types-and-access-wrappers.md
- .harness/tasks/regseq-driver/06-register-mmio-frontend-capture-scaffolding.md
- tests/fixtures/pl011-spec-slice/Cargo.toml
- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/e2e/tests/pl011_spec_slice.rs

## Output Specification

Create or modify:

- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/fixtures/pl011-spec-slice/src/registers.rs
- tests/e2e/tests/pl011_spec_slice.rs

## Implementation Notes

The register portion of the first slice should use the accepted polling-first
 bring-up subset from the discussion note.

Use the real PL011 offsets for this subset so the register block layout is
credible rather than arbitrary. The expected offsets are:

- `DR` at `0x000`
- `FR` at `0x018`
- `IBRD` at `0x024`
- `FBRD` at `0x028`
- `LCR_H` at `0x02C`
- `CR` at `0x030`

Model the intervening gaps with `Reserved<u32>` fields/arrays as needed.

Required register modules in `register_bitfields!`:

- `DR_REG`
  - `DATA`
- `FR_REG`
  - `TXFF`
  - `RXFE`
- `IBRD_REG`
  - integer baud divisor field
- `FBRD_REG`
  - fractional baud divisor field
- `LCRH_REG`
  - `WLEN` with `WLEN_8`
  - `FEN` with `CLEAR` / `SET`
- `CR_REG`
  - `UARTEN` with `CLEAR` / `SET`
  - `TXE` with `CLEAR` / `SET`
  - `RXE` with `CLEAR` / `SET`

Do not include `IMSC` / `ICR` in the first slice unless you find that the
slice cannot stay coherent without explicit interrupt-mask writes. The default
expectation for this task is that they remain out of scope.

Do not expand this task to the full PL011 register file.

The fixture should define a real register block type, for example
 `Pl011Registers`, using:

- `#[repr(C)]`
- `#[rseq::register_block(endian = "little", access_width = 32)]`
- explicit wrapper field types like:
  - `ReadWrite<u32, DR_REG::Register>`
  - `ReadOnly<u32, FR_REG::Register>`
- reserved gaps with `Reserved<u32>` where needed to keep the layout plausible

The fixture's driver type should now declare:

- `#[rseq::mmio(id = "uart_regs")]`
- `Mmio<Pl011Registers>`

Keep this task register-surface focused:

- do not implement the full callback surface yet
- do not add interrupts
- do not add DMA
- do not add PM

The e2e compile-path test should be extended so that the wrapper compile output
 demonstrates real register-surface evidence, such as:

- register access extraction succeeds
- the register block name is visible
- the MMIO field binding is visible
- representative register descriptor facts are visible

String-based assertions over the current debug extraction output are acceptable
 for this task.

The e2e assertions should check for concrete evidence patterns, such as:

- the driver wrapper reaches `after_analysis`
- the register block name `Pl011Registers` appears in register-access output
- the MMIO field `regs` appears with `Pl011Registers`
- representative register descriptor evidence appears for:
  - `DR_REG`
  - `FR_REG`
  - `LCRH_REG`

## Test Plan

Tests/commands to support:

1. Wrapper compile of `tests/fixtures/pl011-spec-slice` still succeeds
2. E2E assertions now confirm register/MMIO extraction evidence from the real
   fixture
3. The fixture remains within the current Phase B frontend boundary

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_spec_slice -- --nocapture`
- `cargo test -p regseq-driver --test register_access`
- `cargo test --workspace`

## Acceptance Criteria

- the PL011 fixture defines the required polling-slice register modules
- the fixture defines a real `Pl011Registers` block with the accepted wrapper
  surface
- the fixture driver declares `Mmio<Pl011Registers>` with `#[rseq::mmio(...)]`
- the wrapper compile path remains green
- the e2e test shows register/MMIO extraction evidence from the real fixture
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
