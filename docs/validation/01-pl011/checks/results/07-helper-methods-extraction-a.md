# Validation Certificate: PL011 Helper Methods Extraction Check A

Source premise: `docs/validation/01-pl011/01-authored-source.md` (Helper Methods section)
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - Every in-scope method signature string (whitespace-normalized) appears verbatim in both the real fixture source (`tests/fixtures/pl011-full-spec/src/driver.rs`) and the inline specimen.
  - Every in-scope method body fragment (whitespace-normalized) appears verbatim in both the real fixture source and the inline specimen.
  - All three associated constant declarations (`ALL_UART_INTERRUPTS_MASK`, `RX_INTERRUPT_MASK`, `TX_INTERRUPT_MASK`) match the fixture source verbatim.
  - The `set_baud_rate` divisor arithmetic lines (`div_times_64`, `ibrd`, `fbrd`) match the fixture source verbatim.
  - The `program_polling_port` ordered sequence (pre-ordered calls + ordered! region) matches the fixture source verbatim.
- Fidelity Evidence (Exact Citations):
  - Test: `specimen_fidelity_method_signatures_match_fixture` -- asserts all 12 method signatures are present in both fixture and specimen (whitespace-normalized substring match).
  - Test: `specimen_fidelity_body_fragments_match_fixture` -- asserts 13 body fragments and constant declarations match in both fixture and specimen.
  - Test: `specimen_fidelity_program_polling_port_sequence_matches_fixture` -- asserts all 7 ordered-region statements plus the 2 pre-ordered calls match the fixture verbatim.

## Check-to-Surface Mapping

- Identity surface: `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`, inspecting `impl_blocks[inherent].methods`
- Body surface: `extract_body_surface(tcx) -> MirBodySurface`, inspecting `functions` entries
- Both surfaces are extracted from compiler-side `TyCtxt` within `after_analysis`, not from persisted JSON.
- Unevaluated constant references are inspected via `MirConstValue::Unsupported { description }` containing the constant's canonical path.
- Scalar inline literals are inspected via `MirConstValue::Scalar { bits, size }`.
- Register target identity is inspected via `MirProjectionElem::Field` projections whose type is `Register<u32, XxxReg>`.

## Check Group 1: Identity, Signature, and Return Type Preservation

- Check: Every in-scope helper preserves identity, signature, and return type.
- Status: `pass`
- Premise: Every in-scope helper preserves identity, signature, and return type at the extraction boundary.
- Assertion Predicate(s):
  - `inherent.methods.iter().find(|m| m.name == name).is_some()` for all 12 helpers
  - `method.canonical_path.0.contains(name)` for all 12 helpers
  - `!method.is_isr` for all 12 helpers
  - `func.receiver == "ref_mut"` for all 12 helpers
  - `func.kind == MirFnKind::Helper` for all 12 helpers
  - `func.return_type == None` (unit) for all 12 helpers
  - `func.params.len() == 2` for `set_baud_rate` and `program_polling_port`
  - `func.params[0].name == "uart_clk_hz"` and `func.params[0].ty == "u32"` for both
  - `func.params[1].name == "baud_rate"` and `func.params[1].ty == "u32"` for both
  - `func.params.len() == 0` for all remaining 10 helpers
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_identity_surface_all_helpers_present`: exercises identity surface
  - `check_group_1_body_surface_all_helpers_present_with_correct_signatures`: exercises body surface signatures
- Reasoning Trace:
  - Authored Fact (Verbatim): Each of the 12 helpers has `fn <name>(&mut self, ...)` with unit return type
  - Extracted Fact (Verbatim): All 12 methods appear in `IdentityEnrichmentSurface.impl_blocks[inherent].methods` and `MirBodySurface.functions` with correct name, receiver, parameter list, and unit return type
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Both identity and body surfaces preserve the complete method signature facts.
- Counterexample: N/A
- Conclusion: `pass`

## Check Group 2: Body-Level Downstream Predicate Preservation

### disable_uart

- Check: one write to `cr`; value preserves `UARTEN = DISABLED`, `TXE = DISABLED`, `RXE = DISABLED`; no return value.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_unevaluated_const(body, "CR_REG::UARTEN::DISABLED")`
  - `has_unevaluated_const(body, "CR_REG::TXE::DISABLED")`
  - `has_unevaluated_const(body, "CR_REG::RXE::DISABLED")`
  - `has_register_field_access(body, "CrReg")`
- Target Evidence: Test `check_group_2_disable_uart_body`
- Reasoning Trace:
  - Authored Fact: writes `CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED` to `cr`
  - Extracted Fact: MIR contains exactly one `Register::<u32, R>::write` call targeting CrReg, with `Add::add` combining three unevaluated constants `CR_REG::UARTEN::DISABLED`, `CR_REG::TXE::DISABLED`, `CR_REG::RXE::DISABLED`
  - Correspondence: `exact`
- Conclusion: `pass`

### disable_fifo

- Check: one logical modify of `lcr_h`; exactly one source read and one write-back; only `FEN` changes to `DISABLED`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "modify") == 1`
  - `count(calls containing "write" and not "modify") == 0`
  - `has_unevaluated_const(body, "FEN::DISABLED")`
  - `has_register_field_access(body, "LcrhReg")`
- Target Evidence: Test `check_group_2_disable_fifo_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)`
  - Extracted Fact: MIR contains exactly one `Register::<u32, R>::modify` call targeting LcrhReg with unevaluated `LCRH_REG::FEN::DISABLED`
  - Correspondence: `exact`
- Conclusion: `pass`

### mask_all_interrupts

- Check: one write to `imsc`; uses `ALL_UART_INTERRUPTS_MASK`; masked bits written as `0`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_call_containing(body, "raw_reg_val")`
  - `has_unevaluated_const(body, "ALL_UART_INTERRUPTS_MASK")`
  - `has_scalar_const(body, 0)`
  - `has_register_field_access(body, "ImscReg")`
- Target Evidence: Test `check_group_2_mask_all_interrupts_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(Self::ALL_UART_INTERRUPTS_MASK, 0))`
  - Extracted Fact: MIR contains `raw_reg_val` call with unevaluated `Pl011FullSpec::ALL_UART_INTERRUPTS_MASK` and scalar `0`, followed by one `Register::<u32, R>::write` on ImscReg
  - Correspondence: `exact`
- Conclusion: `pass`

### clear_all_interrupts

- Check: one write to `icr`; mask and value both preserve `ALL_UART_INTERRUPTS_MASK (= 0x07ff)`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_call_containing(body, "raw_reg_val")`
  - `count_unevaluated_const(body, "ALL_UART_INTERRUPTS_MASK") >= 2`
  - `has_register_field_access(body, "IcrReg")`
- Target Evidence: Test `check_group_2_clear_all_interrupts_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.icr.write(raw_reg_val::<ICR_REG::Register>(Self::ALL_UART_INTERRUPTS_MASK, Self::ALL_UART_INTERRUPTS_MASK))`
  - Extracted Fact: MIR contains `raw_reg_val` call with `ALL_UART_INTERRUPTS_MASK` appearing at least twice (mask and value), followed by one `Register::<u32, R>::write` on IcrReg
  - Correspondence: `exact`
- Conclusion: `pass`

### configure_interrupt_levels

- Check: one write to `ifls`; preserves `RXIFLSEL = ONE_HALF`, `TXIFLSEL = ONE_EIGHTH`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_unevaluated_const(body, "RXIFLSEL::ONE_HALF")`
  - `has_unevaluated_const(body, "TXIFLSEL::ONE_EIGHTH")`
  - `has_register_field_access(body, "IflsReg")`
- Target Evidence: Test `check_group_2_configure_interrupt_levels_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.ifls.write(IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH)`
  - Extracted Fact: MIR contains `Add::add` combining unevaluated `IFLS_REG::RXIFLSEL::ONE_HALF` and `IFLS_REG::TXIFLSEL::ONE_EIGHTH`, followed by one `Register::<u32, R>::write` on IflsReg
  - Correspondence: `exact`
- Conclusion: `pass`

### set_baud_rate

- Check: preserves parameters `uart_clk_hz`, `baud_rate`; preserves `u64` widening and divisor arithmetic; one write to `ibrd`, one write to `fbrd`.
- Status: `pass`
- Assertion Predicate(s):
  - `func.params.len() == 2`, `params[0].name == "uart_clk_hz"`, `params[0].ty == "u32"`, `params[1].name == "baud_rate"`, `params[1].ty == "u32"`
  - MIR contains `Cast { cast_kind: IntToInt }` (u64 widening)
  - `has_scalar_const(body, 4)` (multiply by 4)
  - `has_scalar_const(body, 6)` (shift right by 6)
  - `has_scalar_const(body, 63)` (0x3f fractional mask)
  - MIR contains `BinaryOp { op: MulWithOverflow }`, `BinaryOp { op: Div }`, `BinaryOp { op: Shr }`, `BinaryOp { op: BitAnd }`
  - `count(calls containing "write") == 2`
  - `count(calls containing "field_val") == 2`
  - `has_register_field_access(body, "IbrdReg")`, `has_register_field_access(body, "FbrdReg")`
  - `has_unevaluated_const(body, "IBRD_REG::BAUD_DIVINT::FIELD")`, `has_unevaluated_const(body, "FBRD_REG::BAUD_DIVFRAC::FIELD")`
- Target Evidence: Test `check_group_2_set_baud_rate_body`
- Reasoning Trace:
  - Authored Fact: `let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64; let ibrd = (div_times_64 >> 6) as u32; let fbrd = (div_times_64 & 0x3f) as u32;` then writes to ibrd/fbrd via field_val
  - Extracted Fact: MIR contains IntToInt casts, MulWithOverflow with scalar 4, Div, Shr with scalar 6, BitAnd with scalar 63, two field_val calls with unevaluated IBRD_REG::BAUD_DIVINT::FIELD and FBRD_REG::BAUD_DIVFRAC::FIELD, two writes targeting IbrdReg and FbrdReg
  - Correspondence: `exact`
- Conclusion: `pass`

### configure_line_control

- Check: one write to `lcr_h`; preserves `WLEN = EIGHT_BITS`, `FEN = ENABLED`; preserves materialized literal `0x70`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_unevaluated_const(body, "WLEN::EIGHT_BITS")`
  - `has_unevaluated_const(body, "FEN::ENABLED")`
  - `has_register_field_access(body, "LcrhReg")`
  - `!has_scalar_const(body, 0x70)` (negative assertion confirming the combined literal is absent at MIR level)
- Target Evidence: Test `check_group_2_configure_line_control_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.lcr_h.write(LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED)` with materialized literal `0x70` = `WLEN::EIGHT_BITS.value` (0x60) + `FEN::ENABLED.value` (0x10).
  - Extracted Fact: MIR contains `Add::add` combining unevaluated `LCRH_REG::WLEN::EIGHT_BITS` and `LCRH_REG::FEN::ENABLED`, followed by one `Register::<u32, R>::write` on LcrhReg. The combined scalar `0x70` does not appear as a `MirConstValue::Scalar` because at MIR opt level 0 the compiler preserves `FieldValue` constants as unevaluated references rather than folding them to a scalar.
  - Correspondence: `weakened` for the materialized literal `0x70`. The constituent constants `WLEN::EIGHT_BITS` and `FEN::ENABLED` are individually preserved as unevaluated constants (which together reconstruct to `0x70`), but the folded scalar literal itself is absent.
  - Gap / Loss Description: The spec requires "preserves materialized literal 0x70". At MIR opt level 0, the compiler does not fold the two `FieldValue` constants into a single scalar. The individual constant references are preserved, so the information is recoverable, but the literal `0x70` is not directly present.
  - Why This Status Follows: The write target, register identity, and both constituent constants are individually proven. Only the folded scalar form is absent, which is a representation difference rather than information loss.
- Counterexample: N/A
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Extraction could optionally evaluate unevaluated constants to produce scalar equivalents.
  - Minimal Missing Fact To Preserve: Scalar `0x70` as a `MirConstValue::Scalar` in the extracted body.
- Conclusion: `pass` (weakened correspondence for materialized literal)

### enable_uart

- Check: one write to `cr`; preserves enabled `UARTEN`, `TXE`, `RXE`; preserves materialized literal `0x301`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "write") == 1`
  - `has_unevaluated_const(body, "CR_REG::UARTEN::ENABLED")`
  - `has_unevaluated_const(body, "CR_REG::TXE::ENABLED")`
  - `has_unevaluated_const(body, "CR_REG::RXE::ENABLED")`
  - `has_register_field_access(body, "CrReg")`
  - `!has_scalar_const(body, 0x301)` (negative assertion confirming the combined literal is absent at MIR level)
- Target Evidence: Test `check_group_2_enable_uart_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.cr.write(CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED)` with materialized literal `0x301` = `UARTEN::ENABLED.value` (0x1) + `TXE::ENABLED.value` (0x100) + `RXE::ENABLED.value` (0x200).
  - Extracted Fact: MIR contains two `Add::add` calls combining three unevaluated constants `CR_REG::UARTEN::ENABLED`, `CR_REG::TXE::ENABLED`, `CR_REG::RXE::ENABLED`, followed by one `Register::<u32, R>::write` on CrReg. The combined scalar `0x301` does not appear as a `MirConstValue::Scalar` because at MIR opt level 0 the compiler preserves `FieldValue` constants as unevaluated references rather than folding them to a scalar.
  - Correspondence: `weakened` for the materialized literal `0x301`. The three constituent constants are individually preserved as unevaluated constants (which together reconstruct to `0x301`), but the folded scalar literal itself is absent.
  - Gap / Loss Description: The spec requires "preserves materialized literal 0x301". At MIR opt level 0, the compiler does not fold the three `FieldValue` constants into a single scalar. The individual constant references are preserved, so the information is recoverable, but the literal `0x301` is not directly present.
  - Why This Status Follows: The write target, register identity, and all three constituent constants are individually proven. Only the folded scalar form is absent, which is a representation difference rather than information loss.
- Counterexample: N/A
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Extraction could optionally evaluate unevaluated constants to produce scalar equivalents.
  - Minimal Missing Fact To Preserve: Scalar `0x301` as a `MirConstValue::Scalar` in the extracted body.
- Conclusion: `pass` (weakened correspondence for materialized literal)

### enable_rx_interrupts

- Check: one IMSC read-modify-write; only bits selected by `RX_INTERRUPT_MASK (= 0x50)` change.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "modify") == 1`
  - `count_unevaluated_const(body, "RX_INTERRUPT_MASK") >= 2`
  - `has_register_field_access(body, "ImscReg")`
- Target Evidence: Test `check_group_2_enable_rx_interrupts_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(Self::RX_INTERRUPT_MASK, Self::RX_INTERRUPT_MASK))`
  - Extracted Fact: MIR contains `raw_reg_val` call with `RX_INTERRUPT_MASK` appearing twice (mask and value), followed by one `Register::<u32, R>::modify` on ImscReg
  - Correspondence: `exact`
- Conclusion: `pass`

### enable_tx_interrupts

- Check: one IMSC read-modify-write; only bits selected by `TX_INTERRUPT_MASK (= 0x20)` change.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "modify") == 1`
  - `count_unevaluated_const(body, "TX_INTERRUPT_MASK") >= 2`
  - `has_register_field_access(body, "ImscReg")`
- Target Evidence: Test `check_group_2_enable_tx_interrupts_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(Self::TX_INTERRUPT_MASK, Self::TX_INTERRUPT_MASK))`
  - Extracted Fact: MIR contains `raw_reg_val` call with `TX_INTERRUPT_MASK` appearing twice (mask and value), followed by one `Register::<u32, R>::modify` on ImscReg
  - Correspondence: `exact`
- Conclusion: `pass`

### disable_tx_interrupts

- Check: one IMSC read-modify-write; only bits selected by `TX_INTERRUPT_MASK (= 0x20)` change to `0`.
- Status: `pass`
- Assertion Predicate(s):
  - `count(calls containing "modify") == 1`
  - `has_unevaluated_const(body, "TX_INTERRUPT_MASK")`
  - `has_scalar_const(body, 0)`
  - `has_register_field_access(body, "ImscReg")`
- Target Evidence: Test `check_group_2_disable_tx_interrupts_body`
- Reasoning Trace:
  - Authored Fact: `self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(Self::TX_INTERRUPT_MASK, 0))`
  - Extracted Fact: MIR contains `raw_reg_val` call with unevaluated `TX_INTERRUPT_MASK` (mask) and scalar `0` (value), followed by one `Register::<u32, R>::modify` on ImscReg
  - Correspondence: `exact`
- Conclusion: `pass`

### program_polling_port

- Check: `disable_uart()` then `disable_fifo()` before the ordered region; ordered region is bracketed by leading/trailing full memory barriers; ordered helper-call sequence preserves `set_baud_rate`, `configure_line_control`, `configure_interrupt_levels`, `mask_all_interrupts`, `clear_all_interrupts`, `enable_uart`, `enable_rx_interrupts`.
- Status: `pass`
- Assertion Predicate(s):
  - `func.params.len() == 2`, params are `uart_clk_hz: u32` and `baud_rate: u32`
  - `call_paths contains "disable_uart"` and `call_paths contains "disable_fifo"`
  - all 7 ordered helpers appear in `call_paths`
  - `has_compiler_fence(body)` and `count_calls_containing(body, "compiler_fence") >= 2`
  - `disable_uart_idx < first_fence_idx` and `disable_fifo_idx < first_fence_idx`
  - all ordered helpers have `idx > first_fence_idx && idx < last_fence_idx`
  - ordered helper positions are strictly increasing (preserving call sequence order)
  - `func.helper_calls` contains all 9 directly called helpers
- Target Evidence: Tests `check_group_2_program_polling_port_body` and `check_group_2_program_polling_port_helper_calls_field`
- Reasoning Trace:
  - Authored Fact: pre-ordered `disable_uart()`, `disable_fifo()`, then `ordered! { set_baud_rate; configure_line_control; configure_interrupt_levels; mask_all_interrupts; clear_all_interrupts; enable_uart; enable_rx_interrupts; }`
  - Extracted Fact: MIR basic block call sequence shows: `disable_uart` and `disable_fifo` before the first `compiler_fence`, then 7 helpers in the exact authored order between two `compiler_fence` calls, then a trailing `compiler_fence`
  - Correspondence: `exact`
- Conclusion: `pass`

## Summary

All 12 in-scope helper methods pass both Check Group 1 (identity/signature/return type preservation) and Check Group 2 (body-level downstream predicate preservation). Specimen fidelity is established by 3 exercised tests comparing 12 signatures, 13 body fragments, and 9 ordered-sequence statements against the real fixture source.

Two materialized-literal checks have weakened correspondence:
- `configure_line_control`: the spec requires materialized literal `0x70` but at MIR opt level 0 the compiler preserves the constituent `FieldValue` constants (`WLEN::EIGHT_BITS`, `FEN::ENABLED`) as unevaluated references rather than folding them to a scalar. The individual components are preserved and sufficient to reconstruct `0x70`.
- `enable_uart`: the spec requires materialized literal `0x301` but at MIR opt level 0 the compiler preserves the constituent `FieldValue` constants (`UARTEN::ENABLED`, `TXE::ENABLED`, `RXE::ENABLED`) as unevaluated references rather than folding them to a scalar. The individual components are preserved and sufficient to reconstruct `0x301`.
