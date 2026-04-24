# PL011 Layer 1: Authored Source

This file captures the authored PL011 Regseq source facts that must survive all
downstream compiler boundaries.

Primary source fixture:

- [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)

## Status

- `completed` for authored-source ledgering

## Current Review State

- This document is the upstream authored-facts ledger for the PL011 fixture.
- New validation work should use this ledger to check the live
  HIR/MIR-to-RegseqModel boundary.
- Later boundary reviews in
  [02-hir-mir.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/02-hir-mir.md),
  [03-regseq-model.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/03-regseq-model.md),
  [04-linux-backend-plan.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/04-linux-backend-plan.md),
  and [05-generated-c.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/05-generated-c.md)
  remain separate work.

## Review Template

### Required semantic facts

- callbacks
- helpers
- helper parameters and returns
- lifecycle sequencing
- serial callback semantics
- ISR semantics
- option carriers
- external item usage
- enum variants
- resource usage
- register operations
- fallible paths

### Evidence

- file references
- authored snippets

### Findings

- link ids from [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md)

## Register Definitions

### Register Block: Pl011Registers

- Source File:
  - [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
- Source Local Path:
  - `registers::Pl011Registers`
- Canonical Path:
  - `pl011_full_spec::registers::Pl011Registers`
- Properties:
  - `endian`: `little`
  - `access_width`: `32`
- Reserved Regions:
  - `_reserved0`: `0x008 -> 0x014`
  - `_reserved1`: `0x01C -> 0x01C`
  - `_reserved2`: `0x04C -> 0xFDC`
- Declared Registers:
  - `dr`: `DR_REG @ 0x000 [RW]`
  - `rsr_ecr`: `RSR_ECR_REG @ 0x004 [RW]`
  - `fr`: `FR_REG @ 0x018 [R]`
  - `ilpr`: `ILPR_REG @ 0x020 [RW]`
  - `ibrd`: `IBRD_REG @ 0x024 [RW]`
  - `fbrd`: `FBRD_REG @ 0x028 [RW]`
  - `lcr_h`: `LCRH_REG @ 0x02C [RW]`
  - `cr`: `CR_REG @ 0x030 [RW]`
  - `ifls`: `IFLS_REG @ 0x034 [RW]`
  - `imsc`: `IMSC_REG @ 0x038 [RW]`
  - `ris`: `RIS_REG @ 0x03C [R]`
  - `mis`: `MIS_REG @ 0x040 [R]`
  - `icr`: `ICR_REG @ 0x044 [W]`
  - `dmacr`: `DMACR_REG @ 0x048 [RW]`
  - `periph_id0`: `PERIPHID0_REG @ 0xFE0 [R]`
  - `periph_id1`: `PERIPHID1_REG @ 0xFE4 [R]`
  - `periph_id2`: `PERIPHID2_REG @ 0xFE8 [R]`
  - `periph_id3`: `PERIPHID3_REG @ 0xFEC [R]`
  - `pcell_id0`: `PCELLID0_REG @ 0xFF0 [R]`
  - `pcell_id1`: `PCELLID1_REG @ 0xFF4 [R]`
  - `pcell_id2`: `PCELLID2_REG @ 0xFF8 [R]`
  - `pcell_id3`: `PCELLID3_REG @ 0xFFC [R]`
- Downstream Checks:
  - check that the register block remains identifiable as `Pl011Registers`
  - check that the canonical path remains `pl011_full_spec::registers::Pl011Registers`
  - check that the register-block properties preserve:
    - `endian = little`
    - `access_width = 32`
  - check that the reserved regions preserve their authored offsets:
    - `_reserved0: 0x008 -> 0x014`
    - `_reserved1: 0x01C -> 0x01C`
    - `_reserved2: 0x04C -> 0xFDC`
  - check that the declared register inventory remains complete and in authored offset order
  - check that each declared register preserves:
    - struct field name
    - register name
    - byte offset
    - access kind
  - check that no registers are inserted, removed, renamed, or reordered
  - check that no register-block properties are invented or dropped

### Register: DR_REG

- `OE`: offset = `11`, width = `1`
- `BE`: offset = `10`, width = `1`
- `PE`: offset = `9`, width = `1`
- `FE`: offset = `8`, width = `1`
- `DATA`: offset = `0`, width = `8`
- Downstream Checks:
  - check that the register remains identifiable as `DR_REG`
  - check that the field inventory remains complete and in authored order: `OE`, `BE`, `PE`, `FE`, `DATA`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: RSR_ECR_REG

- `OE`: offset = `3`, width = `1`
- `BE`: offset = `2`, width = `1`
- `PE`: offset = `1`, width = `1`
- `FE`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `RSR_ECR_REG`
  - check that the field inventory remains complete and in authored order: `OE`, `BE`, `PE`, `FE`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: FR_REG

- `RI`: offset = `8`, width = `1`
- `TXFE`: offset = `7`, width = `1`
- `RXFF`: offset = `6`, width = `1`
- `TXFF`: offset = `5`, width = `1`
- `RXFE`: offset = `4`, width = `1`
- `BUSY`: offset = `3`, width = `1`
- `DCD`: offset = `2`, width = `1`
- `DSR`: offset = `1`, width = `1`
- `CTS`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `FR_REG`
  - check that the field inventory remains complete and in authored order: `RI`, `TXFE`, `RXFF`, `TXFF`, `RXFE`, `BUSY`, `DCD`, `DSR`, `CTS`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: ILPR_REG

- `ILPDVSR`: offset = `0`, width = `8`
- Downstream Checks:
  - check that the register remains identifiable as `ILPR_REG`
  - check that the field inventory remains complete and in authored order: `ILPDVSR`
  - check that the field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: IBRD_REG

- `BAUD_DIVINT`: offset = `0`, width = `16`
- Downstream Checks:
  - check that the register remains identifiable as `IBRD_REG`
  - check that the field inventory remains complete and in authored order: `BAUD_DIVINT`
  - check that the field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: FBRD_REG

- `BAUD_DIVFRAC`: offset = `0`, width = `6`
- Downstream Checks:
  - check that the register remains identifiable as `FBRD_REG`
  - check that the field inventory remains complete and in authored order: `BAUD_DIVFRAC`
  - check that the field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: LCRH_REG

- `SPS`: offset = `7`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `WLEN`: offset = `5`, width = `2`
  - Named Field Values:
    - `FIVE_BITS`: `0b00`
    - `SIX_BITS`: `0b01`
    - `SEVEN_BITS`: `0b10`
    - `EIGHT_BITS`: `0b11`
- `FEN`: offset = `4`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `STP2`: offset = `3`, width = `1`
  - Named Field Values:
    - `ONE_STOP_BIT`: `0`
    - `TWO_STOP_BITS`: `1`
- `EPS`: offset = `2`, width = `1`
  - Named Field Values:
    - `ODD`: `0`
    - `EVEN`: `1`
- `PEN`: offset = `1`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `BRK`: offset = `0`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- Downstream Checks:
  - check that the register remains identifiable as `LCRH_REG`
  - check that the field inventory remains complete and in authored order: `SPS`, `WLEN`, `FEN`, `STP2`, `EPS`, `PEN`, `BRK`
  - check that each field preserves its authored offset and width
  - check that named field values preserve their symbolic names and literal values:
    - `SPS.{DISABLED = 0, ENABLED = 1}`
    - `WLEN.{FIVE_BITS = 0b00, SIX_BITS = 0b01, SEVEN_BITS = 0b10, EIGHT_BITS = 0b11}`
    - `FEN.{DISABLED = 0, ENABLED = 1}`
    - `STP2.{ONE_STOP_BIT = 0, TWO_STOP_BITS = 1}`
    - `EPS.{ODD = 0, EVEN = 1}`
    - `PEN.{DISABLED = 0, ENABLED = 1}`
    - `BRK.{DISABLED = 0, ENABLED = 1}`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: CR_REG

- `CTSEN`: offset = `15`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `RTSEN`: offset = `14`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `OUT2`: offset = `13`, width = `1`
  - Named Field Values:
    - `INACTIVE`: `0`
    - `ACTIVE`: `1`
- `OUT1`: offset = `12`, width = `1`
  - Named Field Values:
    - `INACTIVE`: `0`
    - `ACTIVE`: `1`
- `RTS`: offset = `11`, width = `1`
  - Named Field Values:
    - `INACTIVE`: `0`
    - `ACTIVE`: `1`
- `DTR`: offset = `10`, width = `1`
  - Named Field Values:
    - `INACTIVE`: `0`
    - `ACTIVE`: `1`
- `RXE`: offset = `9`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `TXE`: offset = `8`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `LBE`: offset = `7`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `SIRLP`: offset = `2`, width = `1`
  - Named Field Values:
    - `NORMAL`: `0`
    - `LOW_POWER`: `1`
- `SIREN`: offset = `1`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `UARTEN`: offset = `0`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- Downstream Checks:
  - check that the register remains identifiable as `CR_REG`
  - check that the field inventory remains complete and in authored order: `CTSEN`, `RTSEN`, `OUT2`, `OUT1`, `RTS`, `DTR`, `RXE`, `TXE`, `LBE`, `SIRLP`, `SIREN`, `UARTEN`
  - check that each field preserves its authored offset and width
  - check that named field values preserve their symbolic names and literal values for every field in `CR_REG`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: IFLS_REG

- `RXIFLSEL`: offset = `3`, width = `3`
  - Named Field Values:
    - `ONE_EIGHTH`: `0b000`
    - `ONE_QUARTER`: `0b001`
    - `ONE_HALF`: `0b010`
    - `THREE_QUARTERS`: `0b011`
    - `SEVEN_EIGHTHS`: `0b100`
- `TXIFLSEL`: offset = `0`, width = `3`
  - Named Field Values:
    - `ONE_EIGHTH`: `0b000`
    - `ONE_QUARTER`: `0b001`
    - `ONE_HALF`: `0b010`
    - `THREE_QUARTERS`: `0b011`
    - `SEVEN_EIGHTHS`: `0b100`
- Downstream Checks:
  - check that the register remains identifiable as `IFLS_REG`
  - check that the field inventory remains complete and in authored order: `RXIFLSEL`, `TXIFLSEL`
  - check that each field preserves its authored offset and width
  - check that named field values preserve their symbolic names and literal values for both `RXIFLSEL` and `TXIFLSEL`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: IMSC_REG

- `OEIM`: offset = `10`, width = `1`
- `BEIM`: offset = `9`, width = `1`
- `PEIM`: offset = `8`, width = `1`
- `FEIM`: offset = `7`, width = `1`
- `RTIM`: offset = `6`, width = `1`
- `TXIM`: offset = `5`, width = `1`
- `RXIM`: offset = `4`, width = `1`
- `DSRMIM`: offset = `3`, width = `1`
- `DCDMIM`: offset = `2`, width = `1`
- `CTSMIM`: offset = `1`, width = `1`
- `RIMIM`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `IMSC_REG`
  - check that the field inventory remains complete and in authored order: `OEIM`, `BEIM`, `PEIM`, `FEIM`, `RTIM`, `TXIM`, `RXIM`, `DSRMIM`, `DCDMIM`, `CTSMIM`, `RIMIM`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: RIS_REG

- `OERIS`: offset = `10`, width = `1`
- `BERIS`: offset = `9`, width = `1`
- `PERIS`: offset = `8`, width = `1`
- `FERIS`: offset = `7`, width = `1`
- `RTRIS`: offset = `6`, width = `1`
- `TXRIS`: offset = `5`, width = `1`
- `RXRIS`: offset = `4`, width = `1`
- `DSRRMIS`: offset = `3`, width = `1`
- `DCDRMIS`: offset = `2`, width = `1`
- `CTSRMIS`: offset = `1`, width = `1`
- `RIRMIS`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `RIS_REG`
  - check that the field inventory remains complete and in authored order: `OERIS`, `BERIS`, `PERIS`, `FERIS`, `RTRIS`, `TXRIS`, `RXRIS`, `DSRRMIS`, `DCDRMIS`, `CTSRMIS`, `RIRMIS`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: MIS_REG

- `OEMIS`: offset = `10`, width = `1`
- `BEMIS`: offset = `9`, width = `1`
- `PEMIS`: offset = `8`, width = `1`
- `FEMIS`: offset = `7`, width = `1`
- `RTMIS`: offset = `6`, width = `1`
- `TXMIS`: offset = `5`, width = `1`
- `RXMIS`: offset = `4`, width = `1`
- `DSRMMIS`: offset = `3`, width = `1`
- `DCDMMIS`: offset = `2`, width = `1`
- `CTSMMIS`: offset = `1`, width = `1`
- `RIMMIS`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `MIS_REG`
  - check that the field inventory remains complete and in authored order: `OEMIS`, `BEMIS`, `PEMIS`, `FEMIS`, `RTMIS`, `TXMIS`, `RXMIS`, `DSRMMIS`, `DCDMMIS`, `CTSMMIS`, `RIMMIS`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: ICR_REG

- `OEIC`: offset = `10`, width = `1`
- `BEIC`: offset = `9`, width = `1`
- `PEIC`: offset = `8`, width = `1`
- `FEIC`: offset = `7`, width = `1`
- `RTIC`: offset = `6`, width = `1`
- `TXIC`: offset = `5`, width = `1`
- `RXIC`: offset = `4`, width = `1`
- `DSRMIC`: offset = `3`, width = `1`
- `DCDMIC`: offset = `2`, width = `1`
- `CTSMIC`: offset = `1`, width = `1`
- `RIMIC`: offset = `0`, width = `1`
- Downstream Checks:
  - check that the register remains identifiable as `ICR_REG`
  - check that the field inventory remains complete and in authored order: `OEIC`, `BEIC`, `PEIC`, `FEIC`, `RTIC`, `TXIC`, `RXIC`, `DSRMIC`, `DCDMIC`, `CTSMIC`, `RIMIC`
  - check that each field preserves its authored offset and width
  - check that no fields are inserted, removed, renamed, or reordered

### Register: DMACR_REG

- `DMAONERR`: offset = `2`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `TXDMAE`: offset = `1`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- `RXDMAE`: offset = `0`, width = `1`
  - Named Field Values:
    - `DISABLED`: `0`
    - `ENABLED`: `1`
- Downstream Checks:
  - check that the register remains identifiable as `DMACR_REG`
  - check that the field inventory remains complete and in authored order: `DMAONERR`, `TXDMAE`, `RXDMAE`
  - check that each field preserves its authored offset and width
  - check that named field values preserve their symbolic names and literal values for all three fields
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PERIPHID0_REG

- `PARTNUMBER0`: offset = `0`, width = `8`
  - Named Field Values:
    - `PL011`: `0x11`
- Downstream Checks:
  - check that the register remains identifiable as `PERIPHID0_REG`
  - check that the field inventory remains complete and in authored order: `PARTNUMBER0`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `PARTNUMBER0.PL011 = 0x11`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PERIPHID1_REG

- `DESIGNER0`: offset = `4`, width = `4`
  - Named Field Values:
    - `ARM_LOW`: `0x1`
- `PARTNUMBER1`: offset = `0`, width = `4`
  - Named Field Values:
    - `PL011`: `0x0`
- Downstream Checks:
  - check that the register remains identifiable as `PERIPHID1_REG`
  - check that the field inventory remains complete and in authored order: `DESIGNER0`, `PARTNUMBER1`
  - check that each field preserves its authored offset and width
  - check that the named field values preserve:
    - `DESIGNER0.ARM_LOW = 0x1`
    - `PARTNUMBER1.PL011 = 0x0`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PERIPHID2_REG

- `REVISION`: offset = `4`, width = `4`
  - Named Field Values:
    - `R1P0`: `0x0`
    - `R1P1`: `0x1`
    - `R1P3`: `0x2`
    - `R1P4`: `0x2`
    - `R1P5`: `0x3`
- `DESIGNER1`: offset = `0`, width = `4`
  - Named Field Values:
    - `ARM_HIGH`: `0x4`
- Downstream Checks:
  - check that the register remains identifiable as `PERIPHID2_REG`
  - check that the field inventory remains complete and in authored order: `REVISION`, `DESIGNER1`
  - check that each field preserves its authored offset and width
  - check that the named field values preserve:
    - `REVISION.{R1P0 = 0x0, R1P1 = 0x1, R1P3 = 0x2, R1P4 = 0x2, R1P5 = 0x3}`
    - `DESIGNER1.ARM_HIGH = 0x4`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PERIPHID3_REG

- `CONFIGURATION`: offset = `0`, width = `8`
  - Named Field Values:
    - `DEFAULT`: `0x00`
- Downstream Checks:
  - check that the register remains identifiable as `PERIPHID3_REG`
  - check that the field inventory remains complete and in authored order: `CONFIGURATION`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `CONFIGURATION.DEFAULT = 0x00`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PCELLID0_REG

- `VALUE`: offset = `0`, width = `8`
  - Named Field Values:
    - `DEFAULT`: `0x0D`
- Downstream Checks:
  - check that the register remains identifiable as `PCELLID0_REG`
  - check that the field inventory remains complete and in authored order: `VALUE`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `VALUE.DEFAULT = 0x0D`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PCELLID1_REG

- `VALUE`: offset = `0`, width = `8`
  - Named Field Values:
    - `DEFAULT`: `0xF0`
- Downstream Checks:
  - check that the register remains identifiable as `PCELLID1_REG`
  - check that the field inventory remains complete and in authored order: `VALUE`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `VALUE.DEFAULT = 0xF0`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PCELLID2_REG

- `VALUE`: offset = `0`, width = `8`
  - Named Field Values:
    - `DEFAULT`: `0x05`
- Downstream Checks:
  - check that the register remains identifiable as `PCELLID2_REG`
  - check that the field inventory remains complete and in authored order: `VALUE`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `VALUE.DEFAULT = 0x05`
  - check that no fields or named values are inserted, removed, renamed, or reordered

### Register: PCELLID3_REG

- `VALUE`: offset = `0`, width = `8`
  - Named Field Values:
    - `DEFAULT`: `0xB1`
- Downstream Checks:
  - check that the register remains identifiable as `PCELLID3_REG`
  - check that the field inventory remains complete and in authored order: `VALUE`
  - check that the field preserves its authored offset and width
  - check that the named field value preserves `VALUE.DEFAULT = 0xB1`
  - check that no fields or named values are inserted, removed, renamed, or reordered

## Driver Struct

### Driver Identity

- `name`: `pl011-full-spec`
- `compatible`: `["arm,pl011"]`
- `bus`: `amba`
- Downstream Checks:
  - check that the driver name remains `pl011-full-spec`
  - check that the compatible list preserves `["arm,pl011"]`
  - check that the bus remains `amba`
  - check that no driver-identity fields are inserted, removed, renamed, or changed

### Device Class

- `device_class`: `serial`
- Downstream Checks:
  - check that the device class remains `serial`
  - check that no alternate device class is introduced

### Resource: regs

- Kind: `mmio`
- Rust Type: `Mmio<Pl011Registers>`
- Source Local Path: `driver::Pl011FullSpec::regs`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::regs`
- Required: `yes`
- Properties:
  - `id`: `uart_regs`
- Join Key: `Properties.id`
- Notes:
  - Declared as the driver's MMIO-backed register block field.
  - The join key for Layer 2 properties is the `id` property.
- Downstream Checks:
  - check that the resource remains identifiable as field `regs`
  - check that the resource kind remains `mmio`
  - check that the Rust type remains `Mmio<Pl011Registers>`
  - check that the source-local path remains `driver::Pl011FullSpec::regs`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::regs`
  - check that the required/optional status remains `required`
  - check that resource property `id = "uart_regs"` is preserved
  - check that the join key remains `Properties.id`
  - check that the associated register block type remains `Pl011Registers`

### Resource: irq

- Kind: `interrupt`
- Rust Type: `Interrupt`
- Source Local Path: `driver::Pl011FullSpec::irq`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::irq`
- Required: `yes`
- Properties:
  - `id`: `uart_irq`
- Join Key: `Properties.id`
- Notes:
  - Declared as the driver's interrupt resource field.
- Downstream Checks:
  - check that the resource remains identifiable as field `irq`
  - check that the resource kind remains `interrupt`
  - check that the Rust type remains `Interrupt`
  - check that the source-local path remains `driver::Pl011FullSpec::irq`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::irq`
  - check that the required/optional status remains `required`
  - check that resource property `id = "uart_irq"` is preserved
  - check that the join key remains `Properties.id`

### Resource: clock

- Kind: `clock`
- Rust Type: `Option<Clock>`
- Source Local Path: `driver::Pl011FullSpec::clock`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::clock`
- Required: `no`
- Properties:
  - `id`: `uart_clk`
- Join Key: `Properties.id`
- Notes:
  - Optional clock resource modeled explicitly in the driver state.
- Downstream Checks:
  - check that the resource remains identifiable as field `clock`
  - check that the resource kind remains `clock`
  - check that the Rust type remains `Option<Clock>`
  - check that the source-local path remains `driver::Pl011FullSpec::clock`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::clock`
  - check that the required/optional status remains `optional`
  - check that resource property `id = "uart_clk"` is preserved
  - check that the join key remains `Properties.id`

### Resource: reset

- Kind: `reset`
- Rust Type: `Option<ResetLine>`
- Source Local Path: `driver::Pl011FullSpec::reset`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::reset`
- Required: `no`
- Properties:
- Join Key: `Field name`
- Notes:
  - Optional reset resource with no explicit `rseq::reset(...)` attribute in the authored source.
- Downstream Checks:
  - check that the resource remains identifiable as field `reset`
  - check that the resource kind remains `reset`
  - check that the Rust type remains `Option<ResetLine>`
  - check that the source-local path remains `driver::Pl011FullSpec::reset`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::reset`
  - check that the required/optional status remains `optional`
  - check that the join key remains `Field name`
  - check that no explicit resource `id` is invented downstream

### Resource: pinctrl

- Kind: `pinctrl`
- Rust Type: `Option<PinCtrl>`
- Source Local Path: `driver::Pl011FullSpec::pinctrl`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::pinctrl`
- Required: `no`
- Properties:
  - `id`: `uart_pins`
- Join Key: `Properties.id`
- Notes:
  - Optional pinctrl resource modeled explicitly in the driver state.
- Downstream Checks:
  - check that the resource remains identifiable as field `pinctrl`
  - check that the resource kind remains `pinctrl`
  - check that the Rust type remains `Option<PinCtrl>`
  - check that the source-local path remains `driver::Pl011FullSpec::pinctrl`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::pinctrl`
  - check that the required/optional status remains `optional`
  - check that resource property `id = "uart_pins"` is preserved
  - check that the join key remains `Properties.id`

### Property: current_speed

- Rust Type: `u32`
- id: `current-speed`
- default: `115200`
- Required: `yes`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::current_speed`
- Downstream Checks:
  - check that the property remains identifiable as field `current_speed`
  - check that the Rust type remains `u32`
  - check that the property id remains `current-speed`
  - check that the default remains `115200`
  - check that the required/optional status remains `required`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::current_speed`

### Property: fifo_size

- Rust Type: `u32`
- id: `fifo-size`
- default: `32`
- Required: `yes`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::fifo_size`
- Downstream Checks:
  - check that the property remains identifiable as field `fifo_size`
  - check that the Rust type remains `u32`
  - check that the property id remains `fifo-size`
  - check that the default remains `32`
  - check that the required/optional status remains `required`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::fifo_size`

### User Field: pending_tx

- Rust Type: `Option<u8>`
- Source Local Path: `driver::Pl011FullSpec::pending_tx`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::pending_tx`
- Visibility: `private`
- Notes:
  - Ordinary authored driver state; not declared as a Regseq resource or firmware property.
- Downstream Checks:
  - check that the user field remains identifiable as `pending_tx`
  - check that the Rust type remains `Option<u8>`
  - check that the source-local path remains `driver::Pl011FullSpec::pending_tx`
  - check that the canonical path remains `pl011_full_spec::driver::Pl011FullSpec::pending_tx`
  - check that the field remains ordinary user state and is not reclassified as a Regseq resource or firmware property
  - check that no resource/property join metadata is invented for this field

## Local Utility Functions

### Utility Function: field_val

- Rust Signature: `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>`
- Source Local Path: `driver::field_val`
- Canonical Path: `pl011_full_spec::driver::field_val`
- Return Type: `FieldValue<u32, R>`
- Parameters:
  - `field`: `Field<u32, R>`
  - `raw`: `u32`
- Summary:
  - constructs a `FieldValue<u32, R>` from the provided field descriptor and raw value
  - returns `FieldValue::new(field.mask, raw << field.shift)`
  - provides a local typed wrapper for register-field write construction
- Notes:
  - authored as a thin wrapper over `FieldValue::new(field.mask, raw << field.shift)`
  - generic over register marker type `R`
- Downstream Checks:
  - check that the utility preserves both authored parameters
  - check that the returned value is exactly `FieldValue::new(field.mask, raw << field.shift)`
  - check that no additional arithmetic, masking, or side effects are introduced

### Utility Function: raw_reg_val

- Rust Signature: `fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>`
- Source Local Path: `driver::raw_reg_val`
- Canonical Path: `pl011_full_spec::driver::raw_reg_val`
- Return Type: `FieldValue<u32, R>`
- Parameters:
  - `mask`: `u32`
  - `value`: `u32`
- Summary:
  - constructs a `FieldValue<u32, R>` from a raw mask/value pair
  - returns `FieldValue::new(mask, value)`
  - provides a local typed wrapper for raw register modify/write payload construction
- Notes:
  - authored as a thin wrapper over `FieldValue::new(mask, value)`
  - generic over register marker type `R`
- Downstream Checks:
  - check that the utility preserves both authored parameters
  - check that the returned value is exactly `FieldValue::new(mask, value)`
  - check that both payload components `mask` and `value` are preserved
  - check that no additional arithmetic, masking, or side effects are introduced

### Utility Function: field_from_raw

- Rust Signature: `fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32`
- Source Local Path: `driver::field_from_raw`
- Canonical Path: `pl011_full_spec::driver::field_from_raw`
- Return Type: `u32`
- Parameters:
  - `field`: `Field<u32, R>`
  - `raw`: `u32`
- Summary:
  - extracts a field value from a raw register word
  - returns `(raw & field.mask) >> field.shift`
  - provides a local typed wrapper for raw register-field extraction
- Notes:
  - authored as a thin wrapper over `(raw & field.mask) >> field.shift`
  - generic over register marker type `R`
- Downstream Checks:
  - check that the utility preserves both authored parameters
  - check that the returned value is exactly `(raw & field.mask) >> field.shift`
  - check that no additional arithmetic, masking, or side effects are introduced

## Pin-State Marker Types

### Pin-State Marker: DefaultState

- Rust Declaration: `pub struct DefaultState;`
- Source Local Path: `driver::DefaultState`
- Canonical Path: `pl011_full_spec::driver::DefaultState`
- Implemented Traits:
  - `PinState`
- Summary:
  - marks the authored default/active pinctrl state used by `pinctrl.apply::<DefaultState>()`
- Notes:
  - authored as a zero-sized marker type
  - currently participates in pinctrl state selection by type argument
- Downstream Checks:
  - check that `DefaultState` remains distinguishable from `SleepState`
  - check that `pinctrl.apply::<DefaultState>()` preserves the `DefaultState` marker identity through lowering

### Pin-State Marker: SleepState

- Rust Declaration: `pub struct SleepState;`
- Source Local Path: `driver::SleepState`
- Canonical Path: `pl011_full_spec::driver::SleepState`
- Implemented Traits:
  - `PinState`
- Summary:
  - marks the authored sleep pinctrl state used by `pinctrl.apply::<SleepState>()`
- Notes:
  - authored as a zero-sized marker type
  - currently participates in pinctrl state selection by type argument
- Downstream Checks:
  - check that `SleepState` remains distinguishable from `DefaultState`
  - check that `pinctrl.apply::<SleepState>()` preserves the `SleepState` marker identity through lowering

## Driver Constants

### Constant: ALL_UART_INTERRUPTS_MASK

- Rust Type: `u32`
- Value: `0x07ff`
- Source Local Path: `driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK`
- Summary:
  - mask covering all architected PL011 interrupt bits used by this specimen

### Constant: RX_INTERRUPT_MASK

- Rust Type: `u32`
- Value: `(1 << 4) | (1 << 6)`
- Source Local Path: `driver::Pl011FullSpec::RX_INTERRUPT_MASK`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK`
- Summary:
  - mask selecting RX and receive-timeout interrupt sources

### Constant: TX_INTERRUPT_MASK

- Rust Type: `u32`
- Value: `1 << 5`
- Source Local Path: `driver::Pl011FullSpec::TX_INTERRUPT_MASK`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK`
- Summary:
  - mask selecting the TX interrupt source

### Constant: ERROR_INTERRUPT_MASK

- Rust Type: `u32`
- Value: `(1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)`
- Source Local Path: `driver::Pl011FullSpec::ERROR_INTERRUPT_MASK`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK`
- Summary:
  - mask selecting PL011 error interrupt sources

### Constant: EXPLICIT_ISR_CLEAR_MASK

- Rust Type: `u32`
- Value: `Self::ERROR_INTERRUPT_MASK`
- Source Local Path: `driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK`
- Summary:
  - subset of interrupt status bits that must be explicitly acknowledged in the ISR

### Constant: DEFAULT_UART_CLOCK_HZ

- Rust Type: `u32`
- Value: `48_000_000`
- Source Local Path: `driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ`
- Summary:
  - fallback UART input clock used when no modeled clock resource is present

## Driver Helper Methods

### Helper Method: disable_uart

- Rust Signature: `fn disable_uart(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::disable_uart`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::disable_uart`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `CR` register directly
  - sets `CR.UARTEN = DISABLED`
  - sets `CR.TXE = DISABLED`
  - sets `CR.RXE = DISABLED`
  - disables UART operation before divisor or line-control reprogramming
- Notes:
  - authored as a direct full-register write, not a field-by-field modify
- Downstream Checks:
  - check that exactly one write targets `cr`
  - check that the written value preserves `CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED`
  - check that the method does not return a value

### Helper Method: disable_fifo

- Rust Signature: `fn disable_fifo(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::disable_fifo`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::disable_fifo`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - modifies the `LCR_H` register
  - sets `LCRH_REG::FEN = DISABLED`
  - disables FIFO mode to begin from a clean bring-up state
- Notes:
  - authored as a field-level register modify, not a full-register write
- Downstream Checks:
  - check that `LCR_H` is read exactly once as the source value for the modification
  - check that only the `FEN` field is changed
  - check that `FEN` is set to `LCRH_REG::FEN::DISABLED`
  - check that the disabled literal materializes as `0`
  - check that the modified `LCR_H` value is written back exactly once
  - check that the method does not return a value

### Helper Method: mask_all_interrupts

- Rust Signature: `fn mask_all_interrupts(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::mask_all_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::mask_all_interrupts`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `IMSC` register directly
  - uses `Self::ALL_UART_INTERRUPTS_MASK` as the write mask
  - writes `0` for all masked interrupt bits
  - masks all PL011 interrupt sources during polling-oriented bring-up
- Notes:
  - authored as a raw full-register write through `raw_reg_val::<IMSC_REG::Register>(Self::ALL_UART_INTERRUPTS_MASK, 0)`
- Downstream Checks:
  - check that exactly one write targets `imsc`
  - check that the write uses `Self::ALL_UART_INTERRUPTS_MASK`
  - check that the written value clears all bits selected by that mask
  - check that the literal written for the masked bits materializes as `0`
  - check that the method does not return a value

### Helper Method: clear_all_interrupts

- Rust Signature: `fn clear_all_interrupts(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::clear_all_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::clear_all_interrupts`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `ICR` register directly
  - uses `Self::ALL_UART_INTERRUPTS_MASK (= 0x07ff)` as the write mask
  - writes `Self::ALL_UART_INTERRUPTS_MASK (= 0x07ff)` as the value
  - clears all pending PL011 interrupt sources
- Notes:
  - authored as a raw full-register write through `raw_reg_val::<ICR_REG::Register>(Self::ALL_UART_INTERRUPTS_MASK, Self::ALL_UART_INTERRUPTS_MASK)`
- Downstream Checks:
  - check that exactly one write targets `icr`
  - check that the write uses `Self::ALL_UART_INTERRUPTS_MASK (= 0x07ff)`
  - check that the written value preserves `Self::ALL_UART_INTERRUPTS_MASK (= 0x07ff)`
  - check that the method does not return a value

### Helper Method: configure_interrupt_levels

- Rust Signature: `fn configure_interrupt_levels(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::configure_interrupt_levels`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::configure_interrupt_levels`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `IFLS` register directly
  - sets `IFLS_REG::RXIFLSEL = ONE_HALF (= 0b010)`
  - sets `IFLS_REG::TXIFLSEL = ONE_EIGHTH (= 0b000)`
  - programs FIFO trigger levels for interrupt-driven service
- Notes:
  - authored as a direct full-register write using the sum of two named field values
- Downstream Checks:
  - check that exactly one write targets `ifls`
  - check that the written value preserves `IFLS_REG::RXIFLSEL::ONE_HALF (= 0b010)`
  - check that the written value preserves `IFLS_REG::TXIFLSEL::ONE_EIGHTH (= 0b000)`
  - check that no other `IFLS` fields are introduced or modified
  - check that the method does not return a value

### Helper Method: set_baud_rate

- Rust Signature: `fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::set_baud_rate`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::set_baud_rate`
- Return Type: `()`
- Parameters:
  - `uart_clk_hz`: `u32`
  - `baud_rate`: `u32`
- Summary:
  - computes `div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64`
  - derives `ibrd = (div_times_64 >> 6) as u32`
  - derives `fbrd = (div_times_64 & 0x3f) as u32`
  - writes `IBRD.BAUD_DIVINT` from `ibrd`
  - writes `FBRD.BAUD_DIVFRAC` from `fbrd`
  - programs the PL011 integer and fractional baud divisors
- Notes:
  - authored arithmetic widens to `u64` before multiplication/division
  - authored write pattern is one write to `ibrd` and one write to `fbrd`
  - the doc comment explicitly states the divisor formula: `(uart_clk_hz * 4) / baud_rate`, with integer/fractional split at 6 bits
- Downstream Checks:
  - check that the function preserves both authored parameters: `uart_clk_hz: u32` and `baud_rate: u32`
  - check that the arithmetic widens to `u64` before multiplication/division
  - check that the divisor computation preserves `(uart_clk_hz * 4) / baud_rate`
  - check that the integer divisor preserves `(div_times_64 >> 6)`
  - check that the fractional divisor preserves `(div_times_64 & 0x3f)`
  - check that exactly one write targets `ibrd`
  - check that the `ibrd` write preserves `IBRD_REG::BAUD_DIVINT::FIELD`
  - check that exactly one write targets `fbrd`
  - check that the `fbrd` write preserves `FBRD_REG::BAUD_DIVFRAC::FIELD`
  - check that the method does not return a value

### Helper Method: configure_line_control

- Rust Signature: `fn configure_line_control(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::configure_line_control`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::configure_line_control`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `LCR_H` register directly
  - sets `LCRH_REG::WLEN = EIGHT_BITS (= 0b11)`
  - sets `LCRH_REG::FEN = ENABLED (= 1)`
  - programs 8N1 with FIFOs enabled
- Notes:
  - authored as a direct full-register write using the sum of two named field values
  - the doc comment explicitly states that `LCR_H` must be written after `IBRD` and `FBRD` so the divisor update commits
- Downstream Checks:
  - check that exactly one write targets `lcr_h`
  - check that the written value preserves `LCRH_REG::WLEN::EIGHT_BITS (= 0b11)`
  - check that the written value preserves `LCRH_REG::FEN::ENABLED (= 1)`
  - check that `LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED` materializes as `0x70`
  - check that no other `LCR_H` fields are introduced or modified
  - check that the method does not return a value

### Helper Method: enable_uart

- Rust Signature: `fn enable_uart(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::enable_uart`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::enable_uart`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - writes the `CR` register directly
  - sets `CR_REG::UARTEN = ENABLED (= 1)`
  - sets `CR_REG::TXE = ENABLED (= 1)`
  - sets `CR_REG::RXE = ENABLED (= 1)`
  - enables UART operation with both RX and TX active
- Notes:
  - authored as a direct full-register write, not a field-by-field modify
- Downstream Checks:
  - check that exactly one write targets `cr`
  - check that the written value preserves `CR_REG::UARTEN::ENABLED (= 1)`
  - check that the written value preserves `CR_REG::TXE::ENABLED (= 1)`
  - check that the written value preserves `CR_REG::RXE::ENABLED (= 1)`
  - check that `CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED` materializes as `0x301`
  - check that no other `CR` fields are introduced or modified
  - check that the method does not return a value

### Helper Method: enable_rx_interrupts

- Rust Signature: `fn enable_rx_interrupts(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::enable_rx_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::enable_rx_interrupts`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - modifies the `IMSC` register
  - uses `Self::RX_INTERRUPT_MASK (= (1 << 4) | (1 << 6))` as both mask and value
  - enables RX and receive-timeout interrupts for the interrupt-driven path
- Notes:
  - authored as a raw register modify through `raw_reg_val::<IMSC_REG::Register>(Self::RX_INTERRUPT_MASK, Self::RX_INTERRUPT_MASK)`
- Downstream Checks:
  - check that `IMSC` is read exactly once as the source value for the modification
  - check that only bits selected by `Self::RX_INTERRUPT_MASK (= 0x50)` are changed
  - check that the written value for the modified bits preserves `Self::RX_INTERRUPT_MASK (= 0x50)`
  - check that the modified `IMSC` value is written back exactly once
  - check that the method does not return a value

### Helper Method: enable_tx_interrupts

- Rust Signature: `fn enable_tx_interrupts(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::enable_tx_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::enable_tx_interrupts`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - modifies the `IMSC` register
  - uses `Self::TX_INTERRUPT_MASK (= 1 << 5)` as both mask and value
  - enables TX interrupts when transmit work is pending
- Notes:
  - authored as a raw register modify through `raw_reg_val::<IMSC_REG::Register>(Self::TX_INTERRUPT_MASK, Self::TX_INTERRUPT_MASK)`
- Downstream Checks:
  - check that `IMSC` is read exactly once as the source value for the modification
  - check that only bits selected by `Self::TX_INTERRUPT_MASK (= 0x20)` are changed
  - check that the written value for the modified bits preserves `Self::TX_INTERRUPT_MASK (= 0x20)`
  - check that the modified `IMSC` value is written back exactly once
  - check that the method does not return a value

### Helper Method: disable_tx_interrupts

- Rust Signature: `fn disable_tx_interrupts(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::disable_tx_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::disable_tx_interrupts`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - modifies the `IMSC` register
  - uses `Self::TX_INTERRUPT_MASK (= 1 << 5)` as the modification mask
  - writes `0` for the masked bits
  - disables TX interrupts when no more transmit work remains
- Notes:
  - authored as a raw register modify through `raw_reg_val::<IMSC_REG::Register>(Self::TX_INTERRUPT_MASK, 0)`
- Downstream Checks:
  - check that `IMSC` is read exactly once as the source value for the modification
  - check that only bits selected by `Self::TX_INTERRUPT_MASK (= 0x20)` are changed
  - check that the written value for the modified bits materializes as `0`
  - check that the modified `IMSC` value is written back exactly once
  - check that the method does not return a value

### Helper Method: program_polling_port

- Rust Signature: `fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::program_polling_port`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::program_polling_port`
- Return Type: `()`
- Parameters:
  - `uart_clk_hz`: `u32`
  - `baud_rate`: `u32`
- Summary:
  - calls `disable_uart()`
  - calls `disable_fifo()`
  - enters an `ordered!` region
  - `ordered!` expands to a leading `::regseq::intrinsics::sync::memory_barrier(::regseq::intrinsics::sync::BarrierKind::Full)`
  - within the ordered region, calls `set_baud_rate(uart_clk_hz, baud_rate)`
  - then calls `configure_line_control()`
  - then calls `configure_interrupt_levels()`
  - then calls `mask_all_interrupts()`
  - then calls `clear_all_interrupts()`
  - then calls `enable_uart()`
  - then calls `enable_rx_interrupts()`
  - `ordered!` expands to a trailing `::regseq::intrinsics::sync::memory_barrier(::regseq::intrinsics::sync::BarrierKind::Full)`
  - implements the shared polling-oriented bring-up sequence
- Notes:
  - the first two helper calls occur before the explicit ordered region
  - the remaining seven helper calls are authored inside the `ordered!` region
  - parameter flow into `set_baud_rate` is direct: `uart_clk_hz`, `baud_rate`
  - `ordered!` preserves the block result via a temporary `__regseq_ordered_result`, but this method itself returns `()`
- Downstream Checks:
  - check that the function preserves both authored parameters: `uart_clk_hz: u32` and `baud_rate: u32`
  - check that `disable_uart()` occurs before `disable_fifo()`
  - check that both of those calls occur before the ordered region begins
  - check that the ordered region is bracketed by:
    - leading `::regseq::intrinsics::sync::memory_barrier(::regseq::intrinsics::sync::BarrierKind::Full)`
    - trailing `::regseq::intrinsics::sync::memory_barrier(::regseq::intrinsics::sync::BarrierKind::Full)`
  - check that the ordered region preserves this exact helper-call order:
    - `set_baud_rate(uart_clk_hz, baud_rate)`
    - `configure_line_control()`
    - `configure_interrupt_levels()`
    - `mask_all_interrupts()`
    - `clear_all_interrupts()`
    - `enable_uart()`
    - `enable_rx_interrupts()`
  - check that `set_baud_rate` preserves direct parameter flow from the authored parameters
  - check that no enclosed operation is moved outside the ordered-region barrier pair
  - check that the method does not return a value

### Helper Method: uart_clock_hz

- Rust Signature: `fn uart_clock_hz(&self) -> u32`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::uart_clock_hz`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::uart_clock_hz`
- Return Type: `u32`
- Parameters:
  - none
- Summary:
  - reads `self.clock`
  - converts `Option<Clock>` to `Option<&Clock>` via `as_ref()`
  - if `self.clock` is `Some(clock)`, calls `Clock::get_rate(clock)` and returns that value
  - if `self.clock` is `None`, returns `Self::DEFAULT_UART_CLOCK_HZ (= 48_000_000)`
  - resolves the UART input clock frequency used for divisor programming
- Notes:
  - authored as `self.clock.as_ref().map(Clock::get_rate).unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)`
  - authored control flow is option-based selection between an external framework call and a constant fallback
- Downstream Checks:
  - check that the method returns `u32`
  - check that `self.clock` is tested exactly once for presence/absence
  - check that the `Some` path preserves a call to `Clock::get_rate`
  - check that the clock-rate read remains attached to the specific `clock` resource field in focus, not merely to the generic `Clock` resource kind
  - check that the `None` path preserves fallback to `Self::DEFAULT_UART_CLOCK_HZ (= 48_000_000)`
  - check that no other fallback value is introduced
  - check that the method returns exactly one of:
    - the `Clock::get_rate` result from the present `clock` resource
    - `Self::DEFAULT_UART_CLOCK_HZ (= 48_000_000)`

### Helper Method: enable_clock_if_present

- Rust Signature: `fn enable_clock_if_present(&self) -> Result<(), RegseqError>`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::enable_clock_if_present`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::enable_clock_if_present`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - reads `self.clock`
  - if `self.clock` is `Some(clock)`, calls `clock.enable()?`
  - if the `clock.enable()` call returns an error, returns that `RegseqError` immediately
  - if `self.clock` is `None`, performs no resource operation
  - returns `Ok(())` when no error occurs
  - conditionally enables the modeled UART clock resource
- Notes:
  - authored as an `if let Some(clock) = &self.clock { ... }` conditional over an optional clock resource
  - authored effect is fallible only on the `Some` path via `Clock::enable`
- Downstream Checks:
  - check that the method returns `Result<(), RegseqError>`
  - check that `self.clock` is tested exactly once for presence/absence
  - check that only the specific `clock` resource field in focus may be enabled
  - check that the `Some` path preserves a fallible `Clock::enable` resource operation
  - check that the `None` path performs no resource operation
  - check that an error from `Clock::enable` is propagated as `RegseqError`
  - check that the success path returns `Ok(())`
  - check that no additional side effects are introduced on either branch

### Helper Method: disable_clock_if_present

- Rust Signature: `fn disable_clock_if_present(&self)`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::disable_clock_if_present`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::disable_clock_if_present`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - reads `self.clock`
  - if `self.clock` is `Some(clock)`, calls `clock.disable()`
  - if `self.clock` is `None`, performs no resource operation
  - conditionally disables the modeled UART clock resource
- Notes:
  - authored as an `if let Some(clock) = &self.clock { ... }` conditional over an optional clock resource
  - authored effect is infallible on both paths
- Downstream Checks:
  - check that the method returns `()`
  - check that `self.clock` is tested exactly once for presence/absence
  - check that only the specific `clock` resource field in focus may be disabled
  - check that the `Some` path preserves a `Clock::disable` resource operation
  - check that the `None` path performs no resource operation
  - check that no error result or fallback value is introduced
  - check that no additional side effects are introduced on either branch

### Helper Method: toggle_reset_if_present

- Rust Signature: `fn toggle_reset_if_present(&self) -> Result<(), RegseqError>`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::toggle_reset_if_present`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::toggle_reset_if_present`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - reads `self.reset`
  - if `self.reset` is `Some(reset)`, calls `reset.toggle(Duration::from_us(10))?`
  - the reset toggle duration is exactly `10` microseconds
  - if the `reset.toggle(...)` call returns an error, returns that `RegseqError` immediately
  - if `self.reset` is `None`, performs no resource operation
  - returns `Ok(())` when no error occurs
  - conditionally toggles the modeled reset resource before first hardware bring-up
- Notes:
  - authored as an `if let Some(reset) = &self.reset { ... }` conditional over an optional reset resource
  - authored effect is fallible only on the `Some` path via `ResetLine::toggle`
  - the duration argument is authored as `Duration::from_us(10)`
- Downstream Checks:
  - check that the method returns `Result<(), RegseqError>`
  - check that `self.reset` is tested exactly once for presence/absence
  - check that only the specific `reset` resource field in focus may be toggled
  - check that the `Some` path preserves a fallible `ResetLine::toggle` resource operation
  - check that the toggle duration preserves `Duration::from_us(10)`
  - check that the duration literal `10` microseconds is preserved
  - check that the `None` path performs no resource operation
  - check that an error from `ResetLine::toggle` is propagated as `RegseqError`
  - check that the success path returns `Ok(())`
  - check that no additional side effects are introduced on either branch

### Helper Method: apply_default_pins_if_present

- Rust Signature: `fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::apply_default_pins_if_present`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::apply_default_pins_if_present`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - reads `self.pinctrl`
  - if `self.pinctrl` is `Some(pinctrl)`, calls `pinctrl.apply::<DefaultState>()?`
  - applies the `DefaultState` pinctrl state
  - if the `pinctrl.apply::<DefaultState>()` call returns an error, returns that `RegseqError` immediately
  - if `self.pinctrl` is `None`, performs no resource operation
  - returns `Ok(())` when no error occurs
  - conditionally applies the modeled default/active pin muxing state
- Notes:
  - authored as an `if let Some(pinctrl) = &self.pinctrl { ... }` conditional over an optional pinctrl resource
  - authored effect is fallible only on the `Some` path via `PinCtrl::apply::<DefaultState>`
- Downstream Checks:
  - check that the method returns `Result<(), RegseqError>`
  - check that `self.pinctrl` is tested exactly once for presence/absence
  - check that only the specific `pinctrl` resource field in focus may be used
  - check that the `Some` path preserves a fallible `PinCtrl::apply` resource operation
  - check that the applied pinctrl state preserves `DefaultState`
  - check that the `None` path performs no resource operation
  - check that an error from `PinCtrl::apply::<DefaultState>` is propagated as `RegseqError`
  - check that the success path returns `Ok(())`
  - check that no additional side effects are introduced on either branch

### Helper Method: apply_sleep_pins_if_present

- Rust Signature: `fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>`
- Receiver: `&self`
- Source Local Path: `driver::Pl011FullSpec::apply_sleep_pins_if_present`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::apply_sleep_pins_if_present`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - reads `self.pinctrl`
  - if `self.pinctrl` is `Some(pinctrl)`, calls `pinctrl.apply::<SleepState>()?`
  - applies the `SleepState` pinctrl state
  - if the `pinctrl.apply::<SleepState>()` call returns an error, returns that `RegseqError` immediately
  - if `self.pinctrl` is `None`, performs no resource operation
  - returns `Ok(())` when no error occurs
  - conditionally applies the modeled sleep pin muxing state during teardown
- Notes:
  - authored as an `if let Some(pinctrl) = &self.pinctrl { ... }` conditional over an optional pinctrl resource
  - authored effect is fallible only on the `Some` path via `PinCtrl::apply::<SleepState>`
- Downstream Checks:
  - check that the method returns `Result<(), RegseqError>`
  - check that `self.pinctrl` is tested exactly once for presence/absence
  - check that only the specific `pinctrl` resource field in focus may be used
  - check that the `Some` path preserves a fallible `PinCtrl::apply` resource operation
  - check that the applied pinctrl state preserves `SleepState`
  - check that the `None` path performs no resource operation
  - check that an error from `PinCtrl::apply::<SleepState>` is propagated as `RegseqError`
  - check that the success path returns `Ok(())`
  - check that no additional side effects are introduced on either branch

### Helper Method: shutdown_port

- Rust Signature: `fn shutdown_port(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::shutdown_port`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::shutdown_port`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - calls `mask_all_interrupts()`
  - then calls `clear_all_interrupts()`
  - then calls `disable_fifo()`
  - then calls `disable_uart()`
  - then assigns `self.pending_tx = None`
  - performs the shared shutdown sequence for the polling-oriented slice
- Notes:
  - authored as a straight-line helper-call sequence followed by a user-field assignment
  - the final assignment clears any staged transmit byte from ordinary driver state
- Downstream Checks:
  - check that the helper-call order is preserved exactly:
    - `mask_all_interrupts()`
    - `clear_all_interrupts()`
    - `disable_fifo()`
    - `disable_uart()`
  - check that `self.pending_tx` is assigned exactly once after those helper calls
  - check that the assigned value preserves `Option::None`
  - check that no additional state writes are introduced
  - check that the method does not return a value

### Helper Method: service_rx_fifo

- Rust Signature: `fn service_rx_fifo(&mut self) -> bool`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::service_rx_fifo`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::service_rx_fifo`
- Return Type: `bool`
- Parameters:
  - none
- Summary:
  - initializes local `handled = false`
  - enters a `while` loop with condition `self.rx_char().is_some()`
  - on each loop iteration, assigns `handled = true`
  - continues looping until `self.rx_char().is_some()` becomes `false`
  - returns `handled`
  - reports whether at least one received character was drained from the FIFO
- Notes:
  - authored loop body contains only `handled = true`
  - the draining effect comes from repeated evaluation of `self.rx_char()`, not from additional body statements
- Downstream Checks:
  - check that the method returns `bool`
  - check that local `handled` is initialized to `false`
  - check that the loop condition preserves `self.rx_char().is_some()`
  - check that `rx_char()` remains the operation that drives loop progress
  - check that `handled` is assigned `true` on each successful loop iteration
  - check that the returned `bool` is exactly the final value of `handled`
  - check that the method returns `false` when the loop executes zero times
  - check that the method returns `true` when the loop executes one or more times
  - check that no additional side effects are introduced in the loop body

### Helper Method: service_tx_fifo

- Rust Signature: `fn service_tx_fifo(&mut self) -> bool`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::service_tx_fifo`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::service_tx_fifo`
- Return Type: `bool`
- Parameters:
  - none
- Summary:
  - reads `self.pending_tx`
  - if `self.pending_tx` is `Some(byte)`, enters the staged-transmit path
  - within the staged-transmit path, calls `self.tx_ready()`
  - if `self.tx_ready()` is `true`:
    - writes `byte as u32` into `DR.DATA`
    - assigns `self.pending_tx = None`
    - calls `disable_tx_interrupts()`
    - returns `true`
  - if `self.tx_ready()` is `false`, returns `false`
  - if `self.pending_tx` is `None`:
    - calls `disable_tx_interrupts()`
    - returns `true`
  - services at most one staged transmit byte and reports whether transmit work is complete for this interrupt
- Notes:
  - authored control flow has three terminal outcomes:
    - `Some(byte)` and ready -> `true`
    - `Some(byte)` and not ready -> `false`
    - `None` -> `true`
  - authored MMIO write uses `field_val(DR_REG::DATA::FIELD, byte as u32)`
- Downstream Checks:
  - check that the method returns `bool`
  - check that `self.pending_tx` is tested exactly once for presence/absence
  - check that the `Some(byte)` path preserves the staged byte value as `byte`
  - check that the nested readiness test preserves a call to `self.tx_ready()`
  - check that when `tx_ready()` is `true`, exactly one write targets `dr`
  - check that the `dr` write preserves `DR_REG::DATA::FIELD`
  - check that the written value preserves `byte as u32`
  - check that `self.pending_tx` is assigned `Option::None` exactly once on the ready path
  - check that `disable_tx_interrupts()` is called on the ready path after the `dr` write and `pending_tx = None`
  - check that the ready path returns `true`
  - check that the not-ready path returns `false`
  - check that the `None` path calls `disable_tx_interrupts()` exactly once before returning `true`
  - check that no extra register writes or state writes are introduced on any path

### Helper Method: clear_explicit_interrupts

- Rust Signature: `fn clear_explicit_interrupts(&mut self, status: u32)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::clear_explicit_interrupts`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::clear_explicit_interrupts`
- Return Type: `()`
- Parameters:
  - `status`: `u32`
- Summary:
  - computes local `clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK`
  - `Self::EXPLICIT_ISR_CLEAR_MASK = 0x0780`
  - if `clear_mask != 0`, writes `raw_reg_val::<ICR_REG::Register>(clear_mask, clear_mask)` to `ICR`
  - if `clear_mask == 0`, performs no register write
  - clears only explicitly acknowledged interrupt sources from the provided interrupt status word
- Notes:
  - authored write is conditional on `clear_mask` being nonzero
  - the same `clear_mask` value is used as both the raw write mask and the raw write value
- Downstream Checks:
  - check that the function preserves the authored parameter `status: u32`
  - check that local `clear_mask` preserves `status & Self::EXPLICIT_ISR_CLEAR_MASK (= 0x0780)`
  - check that the branch condition preserves `clear_mask != 0`
  - check that when `clear_mask != 0`, exactly one write targets `icr`
  - check that the `icr` write uses `clear_mask` as both the write mask and the write value
  - check that when `clear_mask == 0`, no register write occurs
  - check that the method does not return a value

## Lifecycle Callbacks

### Lifecycle Callback: init

- Rust Signature: `fn init(&mut self) -> Result<(), RegseqError>`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::init`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::init`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - calls `toggle_reset_if_present()?`
  - then calls `apply_default_pins_if_present()?`
  - then calls `enable_clock_if_present()?`
  - then calls `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - returns `Ok(())`
  - performs the initial bring-up sequence using the declared current-speed property and the resolved UART input clock
- Notes:
  - the first three helper calls are fallible and use `?` for early error propagation
  - `program_polling_port(...)` is infallible and executes only after all three fallible steps succeed
  - the second argument to `program_polling_port` is the authored property field `self.current_speed`
  - the first argument to `program_polling_port` is produced by `self.uart_clock_hz()`
- Downstream Checks:
  - check that the callback returns `Result<(), RegseqError>`
  - check that the helper-call order is preserved exactly:
    - `toggle_reset_if_present()?`
    - `apply_default_pins_if_present()?`
    - `enable_clock_if_present()?`
    - `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - check that each of the first three helper calls preserves early `RegseqError` propagation
  - check that `program_polling_port(...)` executes only after the first three helper calls succeed
  - check that the first argument to `program_polling_port` preserves the result of `self.uart_clock_hz()`
  - check that the second argument to `program_polling_port` preserves `self.current_speed`
  - check that the success path returns `Ok(())`
  - check that no additional side effects or reordered steps are introduced

### Lifecycle Callback: deinit

- Rust Signature: `fn deinit(&mut self) -> Result<(), RegseqError>`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::deinit`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::deinit`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - calls `shutdown_port()`
  - then calls `disable_clock_if_present()`
  - then calls `apply_sleep_pins_if_present()?`
  - returns `Ok(())`
  - performs the teardown sequence, including sleep-state pin muxing when modeled
- Notes:
  - the first two helper calls are infallible
  - the final helper call is fallible and uses `?` for early error propagation
  - `apply_sleep_pins_if_present()?` executes only after shutdown and clock-disable steps complete
- Downstream Checks:
  - check that the callback returns `Result<(), RegseqError>`
  - check that the helper-call order is preserved exactly:
    - `shutdown_port()`
    - `disable_clock_if_present()`
    - `apply_sleep_pins_if_present()?`
  - check that `apply_sleep_pins_if_present()?` preserves early `RegseqError` propagation
  - check that the success path returns `Ok(())`
  - check that no additional side effects or reordered steps are introduced

## Serial Callbacks

### Serial Callback: port_open

- Rust Signature: `fn port_open(&mut self) -> Result<(), RegseqError>`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::port_open`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::port_open`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - none
- Summary:
  - calls `enable_clock_if_present()?`
  - then calls `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - returns `Ok(())`
  - performs the open-time bring-up sequence using the resolved UART clock and the declared current-speed property
- Notes:
  - `enable_clock_if_present()` is fallible and uses `?` for early error propagation
  - `program_polling_port(...)` is infallible and executes only after clock-enable succeeds
  - the first argument to `program_polling_port` is produced by `self.uart_clock_hz()`
  - the second argument to `program_polling_port` is the authored property field `self.current_speed`
- Downstream Checks:
  - check that the callback returns `Result<(), RegseqError>`
  - check that the helper-call order is preserved exactly:
    - `enable_clock_if_present()?`
    - `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - check that `enable_clock_if_present()?` preserves early `RegseqError` propagation
  - check that `program_polling_port(...)` executes only after clock-enable succeeds
  - check that the first argument to `program_polling_port` preserves the result of `self.uart_clock_hz()`
  - check that the second argument to `program_polling_port` preserves `self.current_speed`
  - check that the success path returns `Ok(())`
  - check that no additional side effects or reordered steps are introduced

### Serial Callback: port_close

- Rust Signature: `fn port_close(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::port_close`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::port_close`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - calls `shutdown_port()`
  - then calls `disable_clock_if_present()`
  - performs the close-time teardown sequence
- Notes:
  - both helper calls are infallible
- Downstream Checks:
  - check that the helper-call order is preserved exactly:
    - `shutdown_port()`
    - `disable_clock_if_present()`
  - check that the method does not return a value
  - check that no additional side effects or reordered steps are introduced

### Serial Callback: configure

- Rust Signature: `fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::configure`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::configure`
- Return Type: `Result<(), RegseqError>`
- Parameters:
  - `_config`: `&SerialConfig`
- Summary:
  - accepts an opaque serial configuration parameter `_config`
  - does not inspect or destructure `_config`
  - calls `enable_clock_if_present()?`
  - then calls `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - returns `Ok(())`
  - preserves only the validated current-speed intent while keeping `SerialConfig` opaque
- Notes:
  - the source comment explicitly states that `SerialConfig` remains intentionally opaque in the current boundary work
  - `_config` is present in the signature but unused in the authored body
  - `enable_clock_if_present()` is fallible and uses `?` for early error propagation
  - `program_polling_port(...)` is infallible and executes only after clock-enable succeeds
- Downstream Checks:
  - check that the callback returns `Result<(), RegseqError>`
  - check that the `_config: &SerialConfig` parameter is preserved in the callback surface
  - check that `_config` remains opaque and is not inspected, destructured, or translated into additional behavior in the current slice
  - check that the helper-call order is preserved exactly:
    - `enable_clock_if_present()?`
    - `program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - check that `enable_clock_if_present()?` preserves early `RegseqError` propagation
  - check that `program_polling_port(...)` executes only after clock-enable succeeds
  - check that the first argument to `program_polling_port` preserves the result of `self.uart_clock_hz()`
  - check that the second argument to `program_polling_port` preserves `self.current_speed`
  - check that the success path returns `Ok(())`
  - check that no additional side effects or reordered steps are introduced

### Serial Callback: tx_byte

- Rust Signature: `fn tx_byte(&mut self, byte: u8)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::tx_byte`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::tx_byte`
- Return Type: `()`
- Parameters:
  - `byte`: `u8`
- Summary:
  - checks `self.pending_tx.is_none() && self.tx_ready()`
  - if that condition is `true`:
    - writes `byte as u32` into `DR.DATA`
    - returns immediately
  - otherwise:
    - assigns `self.pending_tx = Some(byte)`
    - calls `enable_tx_interrupts()`
  - sends the byte immediately when the transmit path is free and ready, otherwise stages it for interrupt-driven transmission
- Notes:
  - authored fast path is the conjunction of:
    - no staged transmit byte already pending
    - transmitter ready according to `tx_ready()`
  - authored MMIO write uses `field_val(DR_REG::DATA::FIELD, byte as u32)`
  - the staged path does not return an explicit value because the callback return type is `()`
- Downstream Checks:
  - check that the callback preserves the authored parameter `byte: u8`
  - check that the branch condition preserves `self.pending_tx.is_none() && self.tx_ready()`
  - check that the `pending_tx.is_none()` test and the `tx_ready()` test are both preserved
  - check that when the fast-path condition is `true`, exactly one write targets `dr`
  - check that the `dr` write preserves `DR_REG::DATA::FIELD`
  - check that the written value preserves `byte as u32`
  - check that the fast path returns immediately after the `dr` write
  - check that the fast path does not call `enable_tx_interrupts()`
  - check that the staged path assigns `self.pending_tx = Some(byte)` exactly once
  - check that the staged path preserves `Option::Some(byte)`
  - check that the staged path calls `enable_tx_interrupts()` after staging `pending_tx`
  - check that the staged path does not write `dr`
  - check that no extra register writes or state writes are introduced on either path
  - check that the callback does not return a value

### Serial Callback: rx_char

- Rust Signature: `fn rx_char(&mut self) -> Option<RxChar>`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::rx_char`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::rx_char`
- Return Type: `Option<RxChar>`
- Parameters:
  - none
- Summary:
  - reads `FR.RXFE`
  - if `FR.RXFE != 0`, returns `None`
  - otherwise reads raw `DR` once into local `dr`
  - extracts `byte = field_from_raw(DR_REG::DATA::FIELD, dr) as u8`
  - initializes `errors = ErrorKind::empty()`
  - if `field_from_raw(DR_REG::OE::FIELD, dr) != 0`, updates `errors = errors | ErrorKind::OVERRUN`
  - if `field_from_raw(DR_REG::BE::FIELD, dr) != 0`, updates `errors = errors | ErrorKind::BREAK`
  - if `field_from_raw(DR_REG::PE::FIELD, dr) != 0`, updates `errors = errors | ErrorKind::PARITY`
  - if `field_from_raw(DR_REG::FE::FIELD, dr) != 0`, updates `errors = errors | ErrorKind::FRAMING`
  - returns `Some(RxChar::new(byte, errors))`
  - reads one received character and accumulates all PL011 receive error flags into the returned `RxChar`
- Notes:
  - authored control flow has an early `None` return for empty RX FIFO
  - authored data path uses one raw `DR` read and then reuses that local raw value for byte/error extraction
  - authored error accumulation starts from `ErrorKind::empty()` and uses bitwise-or updates
  - authored returned value is constructed through `RxChar::new(byte, errors)` and then wrapped in `Some(...)`
- Downstream Checks:
  - check that the callback returns `Option<RxChar>`
  - check that `FR.RXFE` is read exactly once for the empty-FIFO test
  - check that the empty-FIFO branch condition preserves `FR.RXFE != 0`
  - check that the empty-FIFO path returns `Option::None`
  - check that the non-empty path performs exactly one raw `dr` register read from `DR`
  - check that all byte/error extraction on the non-empty path reuses that same local raw `dr` value
  - check that `byte` preserves `field_from_raw(DR_REG::DATA::FIELD, dr) as u8`
  - check that `errors` is initialized from `ErrorKind::empty()`
  - check that each error-bit test is preserved against the same raw `dr` value:
    - `DR_REG::OE::FIELD` -> `ErrorKind::OVERRUN`
    - `DR_REG::BE::FIELD` -> `ErrorKind::BREAK`
    - `DR_REG::PE::FIELD` -> `ErrorKind::PARITY`
    - `DR_REG::FE::FIELD` -> `ErrorKind::FRAMING`
  - check that each matching error condition updates `errors` via bitwise-or with the corresponding `ErrorKind` constant
  - check that the non-empty path returns `Option::Some(...)`
  - check that the inner value of `Some(...)` preserves `RxChar::new(byte, errors)`
  - check that the `RxChar` construction preserves both payload components:
    - `byte`
    - `errors`
  - check that no extra register reads, state writes, or dropped error bits are introduced

### Serial Callback: tx_ready

- Rust Signature: `fn tx_ready(&mut self) -> bool`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::tx_ready`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::tx_ready`
- Return Type: `bool`
- Parameters:
  - none
- Summary:
  - reads `FR.TXFF`
  - compares the read value to `0`
  - returns `true` when `FR.TXFF == 0`
  - returns `false` when `FR.TXFF != 0`
  - reports whether the transmit FIFO can accept another byte
- Notes:
  - authored as `self.regs.fr.read(FR_REG::TXFF::FIELD) == 0`
- Downstream Checks:
  - check that the callback returns `bool`
  - check that `FR.TXFF` is read exactly once
  - check that the comparison preserves `== 0`
  - check that the returned `bool` is exactly the result of `FR.TXFF == 0`
  - check that no additional side effects are introduced

### Serial Callback: tx_empty

- Rust Signature: `fn tx_empty(&mut self) -> bool`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::tx_empty`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::tx_empty`
- Return Type: `bool`
- Parameters:
  - none
- Summary:
  - reads `FR.BUSY`
  - compares the read value to `0`
  - returns `true` when `FR.BUSY == 0`
  - returns `false` when `FR.BUSY != 0`
  - reports whether the transmitter is idle
- Notes:
  - authored as `self.regs.fr.read(FR_REG::BUSY::FIELD) == 0`
- Downstream Checks:
  - check that the callback returns `bool`
  - check that `FR.BUSY` is read exactly once
  - check that the comparison preserves `== 0`
  - check that the returned `bool` is exactly the result of `FR.BUSY == 0`
  - check that no additional side effects are introduced

### Serial Callback: flush_rx_fifo

- Rust Signature: `fn flush_rx_fifo(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::flush_rx_fifo`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::flush_rx_fifo`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - enters a `while` loop with condition `self.regs.fr.read(FR_REG::RXFE::FIELD) == 0`
  - on each loop iteration, performs one raw `DR` read and discards the result
  - continues looping until `FR.RXFE != 0`
  - drains all currently buffered receive characters from the FIFO
- Notes:
  - authored loop body contains only `let _ = self.regs.dr.read_raw();`
  - the loop termination condition is the receive-FIFO-empty flag `FR.RXFE`
- Downstream Checks:
  - check that `FR.RXFE` remains the loop condition
  - check that the loop condition preserves `FR.RXFE == 0`
  - check that each loop iteration performs exactly one raw `DR` read
  - check that the raw `DR` read result is discarded
  - check that the loop continues until `FR.RXFE != 0`
  - check that no additional state writes or register writes are introduced
  - check that the callback does not return a value

### Serial Callback: flush_tx_fifo

- Rust Signature: `fn flush_tx_fifo(&mut self)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::flush_tx_fifo`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::flush_tx_fifo`
- Return Type: `()`
- Parameters:
  - none
- Summary:
  - modifies `LCR_H` once to set `LCRH_REG::FEN = DISABLED (= 0)`
  - then modifies `LCR_H` once to set `LCRH_REG::FEN = ENABLED (= 1)`
  - flushes the transmit FIFO by toggling FIFO enable off and back on
- Notes:
  - authored as two consecutive `lcr_h.modify(...)` operations
  - both operations target only the `FEN` field
- Downstream Checks:
  - check that the first `LCR_H` modification sets `LCRH_REG::FEN = DISABLED (= 0)`
  - check that the second `LCR_H` modification sets `LCRH_REG::FEN = ENABLED (= 1)`
  - check that the two `LCR_H` modifications occur in exactly that order
  - check that only the `FEN` field is changed by either modification
  - check that no additional register or state operations are introduced
  - check that the callback does not return a value

### Serial Callback: send_break

- Rust Signature: `fn send_break(&mut self, on: bool)`
- Receiver: `&mut self`
- Source Local Path: `driver::Pl011FullSpec::send_break`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::send_break`
- Return Type: `()`
- Parameters:
  - `on`: `bool`
- Summary:
  - branches on the authored parameter `on`
  - if `on` is `true`, modifies `LCR_H` to set `LCRH_REG::BRK = ENABLED (= 1)`
  - if `on` is `false`, modifies `LCR_H` to set `LCRH_REG::BRK = DISABLED (= 0)`
  - controls break transmission by toggling the `BRK` field
- Notes:
  - each branch performs exactly one `lcr_h.modify(...)`
  - both branches target only the `BRK` field
- Downstream Checks:
  - check that the callback preserves the authored parameter `on: bool`
  - check that the branch condition preserves the value of `on`
  - check that the `on == true` path modifies `LCR_H` to set `LCRH_REG::BRK = ENABLED (= 1)`
  - check that the `on == false` path modifies `LCR_H` to set `LCRH_REG::BRK = DISABLED (= 0)`
  - check that only the `BRK` field is changed on either path
  - check that no additional register or state operations are introduced
  - check that the callback does not return a value

## Interrupt Service Routine

### ISR: handle_irq

- Rust Signature: `pub fn handle_irq(&mut self) -> IrqResult`
- Receiver: `&mut self`
- ISR Annotation: `#[rseq::isr]`
- Source Local Path: `driver::Pl011FullSpec::handle_irq`
- Canonical Path: `pl011_full_spec::driver::Pl011FullSpec::handle_irq`
- Return Type: `IrqResult`
- Parameters:
  - none
- Summary:
  - reads raw `MIS` once into local `status`
  - if `status == 0`, returns `IrqResult::NotHandled`
  - initializes local `handled = false`
  - if `status & Self::RX_INTERRUPT_MASK != 0`, updates `handled |= self.service_rx_fifo()`
  - if `status & Self::TX_INTERRUPT_MASK != 0`, updates `handled |= self.service_tx_fifo()`
  - calls `self.clear_explicit_interrupts(status)`
  - if `handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0)`, returns `IrqResult::Handled`
  - otherwise returns `IrqResult::NotHandled`
  - services RX/TX interrupt causes, clears explicit interrupt sources, and reports handled/not-handled status
- Notes:
  - authored interrupt status source is raw `MIS`
  - authored ISR has an early `NotHandled` return for `status == 0`
  - authored `handled` flag accumulates helper-return booleans via `|=`
  - explicit interrupt-clear work is performed regardless of whether RX/TX service changed `handled`
  - the final handled decision depends on either:
    - `handled == true`
    - or `status & Self::EXPLICIT_ISR_CLEAR_MASK != 0`
- Downstream Checks:
  - check that the ISR return type remains `IrqResult`
  - check that raw `MIS` is read exactly once into local `status`
  - check that the early branch condition preserves `status == 0`
  - check that the early-zero-status path returns `IrqResult::NotHandled`
  - check that local `handled` is initialized to `false`
  - check that the RX service branch condition preserves `status & Self::RX_INTERRUPT_MASK (= 0x50) != 0`
  - check that the TX service branch condition preserves `status & Self::TX_INTERRUPT_MASK (= 0x20) != 0`
  - check that the RX service branch updates `handled |= self.service_rx_fifo()`
  - check that the TX service branch updates `handled |= self.service_tx_fifo()`
  - check that `clear_explicit_interrupts(status)` executes after the RX/TX service branches
  - check that the final handled condition preserves `handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK (= 0x0780) != 0)`
  - check that the final true branch returns `IrqResult::Handled`
  - check that the final false branch returns `IrqResult::NotHandled`
  - check that `IrqResult::{Handled, NotHandled}` remain nominally distinguishable and do not collapse to anonymous integer residue
  - check that no extra interrupt-status reads, register writes, or state writes are introduced
