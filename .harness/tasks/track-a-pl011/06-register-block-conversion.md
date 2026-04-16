# track-a-pl011-06: PL011 register block conversion to register_block!

## Description

Convert the PL011 fixture's register definitions from the old
`register_bitfields!` + manual `#[repr(C)]` struct pattern to a single
`register_block!` macro invocation producing a scoped `pl011` module.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.7)
- `tests/fixtures/pl011-full-spec/src/registers.rs` (current file)
- `tests/fixtures/pl011-full-spec/src/lib.rs`
- `crates/regseq-macros/src/register_block.rs` (register_block! macro from regseq-macros-04)
- `crates/regseq/src/registers.rs` (RegisterOf, Field, FieldValue, RegisterId types)

## Implementation Notes

### Replace registers.rs

Replace the entire contents of `tests/fixtures/pl011-full-spec/src/registers.rs`
with a single `register_block!` invocation covering all 22 PL011 registers.

The current file has:
- `register_bitfields!` defining: DR_REG, RSR_ECR_REG, FR_REG, ILPR_REG,
  IBRD_REG, FBRD_REG, LCRH_REG, CR_REG, IFLS_REG, IMSC_REG, RIS_REG,
  MIS_REG, ICR_REG, DMACR_REG
- `#[repr(C)] struct Pl011Registers` with 33 fields including reserved gaps
- 8 peripheral ID registers (PERIPHID0-3, PCELLID0-3)

The new file should use:
```rust
use regseq::register_block;

register_block! {
    #[endian = "little", access_width = 32]
    pub pl011 {
        dr: ReadWrite {
            DATA OFFSET(0) NUMBITS(8) [],
            FE OFFSET(8) NUMBITS(1) [],
            PE OFFSET(9) NUMBITS(1) [],
            BE OFFSET(10) NUMBITS(1) [],
            OE OFFSET(11) NUMBITS(1) [],
        },
        rsr_ecr: ReadWrite {
            FE OFFSET(0) NUMBITS(1) [],
            PE OFFSET(1) NUMBITS(1) [],
            BE OFFSET(2) NUMBITS(1) [],
            OE OFFSET(3) NUMBITS(1) [],
        },
        _reserved0: Reserved<u32> [4],
        fr: ReadOnly {
            CTS OFFSET(0) NUMBITS(1) [],
            DSR OFFSET(1) NUMBITS(1) [],
            DCD OFFSET(2) NUMBITS(1) [],
            BUSY OFFSET(3) NUMBITS(1) [],
            RXFE OFFSET(4) NUMBITS(1) [],
            TXFF OFFSET(5) NUMBITS(1) [],
            RXFF OFFSET(6) NUMBITS(1) [],
            TXFE OFFSET(7) NUMBITS(1) [],
            RI OFFSET(8) NUMBITS(1) [],
        },
        // ... remaining registers following the same pattern
    }
}
```

Carry over ALL registers from the current file: DR, RSR_ECR, FR, ILPR,
IBRD, FBRD, LCRH (with named values like WLEN, FEN, BRK, etc.), CR
(with named values like UARTEN, TXE, RXE, etc.), IFLS (with named
values), IMSC, RIS, MIS, ICR, DMACR, and the peripheral/cell ID
registers. Preserve all named values (ENABLED/DISABLED, EIGHT_BITS,
ONE_HALF, ONE_EIGHTH, etc.).

Preserve reserved gaps between registers with the correct counts to
maintain byte offset alignment matching the PL011 hardware specification.

### Update lib.rs

Update `tests/fixtures/pl011-full-spec/src/lib.rs` to export the new
module path if needed. The old `pub mod registers;` should still work
since the file still exists.

### Key transformation

| Old | New |
|-----|-----|
| `Pl011Registers` (top-level struct) | `pl011::Regs` (scoped in module) |
| `DR_REG::DATA::FIELD` | `pl011::dr::DATA` (or `pl011::dr::DATA::FIELD` if named values exist) |
| `CR_REG::UARTEN::ENABLED` | `pl011::cr::UARTEN::ENABLED` |
| `IMSC_REG::Register` | `pl011::imsc::Register` |

## Non-Goals

- No driver.rs changes (track-a-pl011-07).
- No extraction pipeline changes.
- No golden file regeneration.

## Test Plan

- `cargo build -p pl011-full-spec` compiles (registers.rs only -- driver.rs
  will NOT compile until track-a-pl011-07 updates it).
  Actually, driver.rs imports from `crate::registers::*` so compilation
  will fail. Use `cargo check -p regseq-macros` and manual inspection to
  verify the macro expansion is correct. Or temporarily comment out
  driver.rs imports.
- Verify the generated module has: `pl011::Regs`, `pl011::dr::Register`,
  `pl011::dr::REG`, `pl011::dr::OFFSET`, `pl011::dr::DATA`,
  `pl011::cr::UARTEN::ENABLED`, etc.
- Verify byte offsets match the PL011 hardware spec (DR at 0x000,
  FR at 0x018, CR at 0x030, etc.).

## Review

After implementation, use `codex:rescue` to review. Verify all 22
registers are present, all named values are preserved, and byte offsets
are correct. Iterate until approved.

## Quality Gate

```
cargo check -p regseq-macros
cargo fmt --all -- --check
```

Note: Full workspace build will fail until driver.rs is updated in
track-a-pl011-07.

## Acceptance Criteria

- `registers.rs` rewritten with single `register_block!` invocation.
- All 22 registers present with correct fields and named values.
- Byte offsets match PL011 hardware specification.
- Codex review approved.
