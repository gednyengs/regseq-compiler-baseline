# track-a-pl011-07: PL011 driver.rs conversion to Mmio methods and pin_state attrs

## Description

Convert the PL011 fixture's driver.rs to use the new register access
surface (Mmio<T> methods instead of wrapper methods), scoped module
imports from register_block!, and add missing `#[rseq::]` attributes
on marker types and resource fields.

This is the fixture-facing proof that all Phase 3 facade, macro, and
register changes compose correctly in a realistic driver.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.7)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (current file)
- `tests/fixtures/pl011-full-spec/src/registers.rs` (post track-a-pl011-06)
- `crates/regseq/src/mmio.rs` (Mmio methods from regseq-facade-23)
- `crates/regseq/src/registers.rs` (RegisterOf, RegisterId, Field, FieldValue)

## Implementation Notes

### Import changes

Replace:
```rust
use crate::registers::{
    Pl011Registers, CR_REG, DR_REG, FBRD_REG, FR_REG, IBRD_REG, ICR_REG,
    IFLS_REG, IMSC_REG, LCRH_REG,
};
```

With scoped imports from the register_block! output:
```rust
use crate::registers::pl011::{
    self, cr, dr, fbrd, fr, ibrd, icr, ifls, imsc, lcr_h, mis,
};
```

### Struct type change

Change:
```rust
pub regs: Mmio<Pl011Registers>,
```
To:
```rust
pub regs: Mmio<pl011::Regs>,
```

### Register access conversions

Every register access changes from wrapper methods to Mmio methods:

| Old pattern | New pattern |
|-------------|-------------|
| `self.regs.cr.write(CR_REG::UARTEN::DISABLED + ...)` | `self.regs.write_field(cr::UARTEN::DISABLED + ...)` |
| `self.regs.fr.read(FR_REG::RXFE::FIELD)` | `self.regs.read_field(fr::RXFE)` |
| `self.regs.dr.read_raw()` | `self.regs.read(dr::REG)` |
| `self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)` | `self.regs.modify(lcr_h::REG, lcr_h::FEN::DISABLED)` |
| `self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(...))` | `self.regs.write_field(raw_reg_val::<imsc::Register>(...))` |
| `self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(...))` | `self.regs.modify(imsc::REG, raw_reg_val::<imsc::Register>(...))` |
| `self.regs.mis.read_raw()` | `self.regs.read(mis::REG)` |

### Helper function generic parameter changes

Update `field_val`, `raw_reg_val`, `field_from_raw` -- their generic
parameter `R` is unchanged in shape but call sites change from
`DR_REG::Register` to `dr::Register`, `IMSC_REG::Register` to
`imsc::Register`, etc.

### Missing attribute additions

1. Add `#[rseq::pin_state(id = "default")]` above `pub struct DefaultState;`
2. Add `#[rseq::pin_state(id = "sleep")]` above `pub struct SleepState;`
3. Add `#[rseq::reset(id = "uart_rst")]` to the reset field:
   ```rust
   #[rseq::reset(id = "uart_rst")]
   pub reset: Option<ResetLine>,
   ```

### Unused import cleanup

Remove `Field` and `FieldValue` from the top-level regseq import if
they are no longer used directly (they may still be needed for helper
functions). Remove `RegisterId` if not used directly. Adjust as needed
to make the compiler happy.

## Non-Goals

- No extraction pipeline changes (Phase 4).
- No golden file regeneration (Phase 4).
- No changes to platform.toml.
- No new driver functionality -- this is a mechanical conversion.

## Test Plan

- `cargo build -p pl011-full-spec` compiles successfully.
- `cargo test --workspace` passes (no regressions).
- The driver's behavioral logic is unchanged -- only access patterns
  and imports differ.
- Grep confirms no remaining references to old register module paths
  (`Pl011Registers`, `CR_REG::`, `DR_REG::`, `FR_REG::`, etc.) in
  driver.rs.
- `#[rseq::pin_state]` attributes present on both marker types.
- `#[rseq::reset(id = "uart_rst")]` present on reset field.

## Review

After tests pass, use `codex:rescue` to review. Verify:
1. Every register access uses Mmio methods (no wrapper method calls remain).
2. All imports use scoped module paths.
3. `#[rseq::pin_state]` and `#[rseq::reset]` attributes are present.
4. Behavioral logic is unchanged.
Iterate until approved.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- driver.rs fully converted to Mmio methods and scoped imports.
- `#[rseq::pin_state]` on DefaultState and SleepState.
- `#[rseq::reset(id = "uart_rst")]` on reset field.
- `Mmio<pl011::Regs>` as the regs type.
- Workspace compiles and all tests pass. Codex review approved.
