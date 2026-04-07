# PL011 Layer 1: Authored Source

This file captures the authored PL011 Regseq source facts that must survive all
downstream compiler boundaries.

Primary source fixture:

- [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)

## Status

- `not started`

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
  - `repr`: `C`
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
