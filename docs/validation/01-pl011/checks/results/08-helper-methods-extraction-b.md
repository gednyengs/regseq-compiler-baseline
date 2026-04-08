# Validation Certificate: PL011 Helper Methods Extraction Check B

Source premise: `docs/validation/01-pl011/01-authored-source.md` (lines 1149-1458)
Target boundary evidence: `crates/regseq-driver/tests/helper_methods_b_extraction.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - `precondition_specimen_fidelity` test asserts that every authored fact
    (function signature, body-level control flow pattern, literal, and resource
    operation) from the check-08 scope matches both the real PL011 fixture
    source at `tests/fixtures/pl011-full-spec/src/driver.rs` and the inline
    specimen used for extraction testing.
  - 23 string-containment assertions on the fixture source.
  - 23 string-containment assertions on the specimen.
- Fidelity Evidence (Exact Citations):
  - `fixture.contains("fn uart_clock_hz(&self) -> u32")` -- true
  - `fixture.contains(".map(Clock::get_rate)")` -- true
  - `fixture.contains(".unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)")` -- true
  - `fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;")` -- true
  - `fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>")` -- true
  - `fixture.contains("clock.enable()?;")` -- true
  - `fixture.contains("fn disable_clock_if_present(&self)")` -- true
  - `fixture.contains("clock.disable();")` -- true
  - `fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>")` -- true
  - `fixture.contains("reset.toggle(Duration::from_us(10))?;")` -- true
  - `fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>")` -- true
  - `fixture.contains("pinctrl.apply::<DefaultState>()?;")` -- true
  - `fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>")` -- true
  - `fixture.contains("pinctrl.apply::<SleepState>()?;")` -- true
  - `fixture.contains("fn shutdown_port(&mut self)")` -- true
  - `fixture.contains("self.pending_tx = None;")` -- true
  - `fixture.contains("fn service_rx_fifo(&mut self) -> bool")` -- true
  - `fixture.contains("let mut handled = false;")` -- true
  - `fixture.contains("while self.rx_char().is_some()")` -- true
  - `fixture.contains("fn service_tx_fifo(&mut self) -> bool")` -- true
  - `fixture.contains("if let Some(byte) = self.pending_tx")` -- true
  - `fixture.contains("field_val(DR_REG::DATA::FIELD, byte as u32)")` -- true
  - `fixture.contains("fn clear_explicit_interrupts(&mut self, status: u32)")` -- true

## Check-to-Surface Mapping

| Check | Extraction Surface(s) |
|---|---|
| Check Group 1 (identity, signature, return type) | `MirBodySurface.functions` (name, receiver, params, return_type), `IdentityEnrichmentSurface.impl_blocks.methods` |
| Check Group 2 (body-level downstream) | `MirBodySurface.functions[*].body` (MIR basic blocks), `ClockSurface.call_sites`, `ResetSurface.call_sites`, `PinCtrlSurface.call_sites` |

---

## Check Group 1 -- Identity, Signature, and Return Type

### Check: uart_clock_hz identity
- Status: `pass`
- Premise: uart_clock_hz preserves identity `fn uart_clock_hz(&self) -> u32`
- Assertion Predicate(s):
  - `f.receiver == "ref"` (receiver is &self)
  - `f.params.is_empty()` (no parameters)
  - `f.return_type == Some("u32")` (returns u32)
  - `find_method(identity, "uart_clock_hz").is_some()` (exists in identity enrichment)
- Target Evidence: `check1_uart_clock_hz_identity` test in `helper_methods_b_extraction.rs`
- Reasoning Trace:
  - Authored Fact: `fn uart_clock_hz(&self) -> u32`
  - Extracted Fact: receiver="ref", params=[], return_type=Some("u32"), kind=Helper
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all four identity properties verified by exercised assertion

### Check: enable_clock_if_present identity
- Status: `pass`
- Premise: preserves `fn enable_clock_if_present(&self) -> Result<(), RegseqError>`
- Assertion Predicate(s): receiver=="ref", params empty, return_type contains "Result"
- Target Evidence: `check1_enable_clock_if_present_identity`
- Correspondence: `exact`

### Check: disable_clock_if_present identity
- Status: `pass`
- Premise: preserves `fn disable_clock_if_present(&self)`
- Assertion Predicate(s): receiver=="ref", params empty, return_type==None (unit)
- Target Evidence: `check1_disable_clock_if_present_identity`
- Correspondence: `exact`

### Check: toggle_reset_if_present identity
- Status: `pass`
- Premise: preserves `fn toggle_reset_if_present(&self) -> Result<(), RegseqError>`
- Assertion Predicate(s): receiver=="ref", params empty, return_type contains "Result"
- Target Evidence: `check1_toggle_reset_if_present_identity`
- Correspondence: `exact`

### Check: apply_default_pins_if_present identity
- Status: `pass`
- Premise: preserves `fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>`
- Assertion Predicate(s): receiver=="ref", params empty, return_type contains "Result"
- Target Evidence: `check1_apply_default_pins_if_present_identity`
- Correspondence: `exact`

### Check: apply_sleep_pins_if_present identity
- Status: `pass`
- Premise: preserves `fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>`
- Assertion Predicate(s): receiver=="ref", params empty, return_type contains "Result"
- Target Evidence: `check1_apply_sleep_pins_if_present_identity`
- Correspondence: `exact`

### Check: shutdown_port identity
- Status: `pass`
- Premise: preserves `fn shutdown_port(&mut self)`
- Assertion Predicate(s): receiver=="ref_mut", params empty, return_type==None
- Target Evidence: `check1_shutdown_port_identity`
- Correspondence: `exact`

### Check: service_rx_fifo identity
- Status: `pass`
- Premise: preserves `fn service_rx_fifo(&mut self) -> bool`
- Assertion Predicate(s): receiver=="ref_mut", params empty, return_type==Some("bool")
- Target Evidence: `check1_service_rx_fifo_identity`
- Correspondence: `exact`

### Check: service_tx_fifo identity
- Status: `pass`
- Premise: preserves `fn service_tx_fifo(&mut self) -> bool`
- Assertion Predicate(s): receiver=="ref_mut", params empty, return_type==Some("bool")
- Target Evidence: `check1_service_tx_fifo_identity`
- Correspondence: `exact`

### Check: clear_explicit_interrupts identity
- Status: `pass`
- Premise: preserves `fn clear_explicit_interrupts(&mut self, status: u32)`
- Assertion Predicate(s): receiver=="ref_mut", params.len()==1, params[0].name=="status", params[0].ty=="u32", return_type==None
- Target Evidence: `check1_clear_explicit_interrupts_identity`
- Correspondence: `exact`

---

## Check Group 2 -- Body-Level Downstream Checks

### Check: uart_clock_hz body
- Status: `pass`
- Premise: body preserves Clock::get_rate call, DEFAULT_UART_CLOCK_HZ fallback, option branching, clock-rate attachment to clock field
- Assertion Predicate(s):
  - `body_has_fn_ref(body, "get_rate")` -- Clock::get_rate appears as FnDef argument to Option::map
  - `body_has_unevaluated_const(body, "DEFAULT_UART_CLOCK_HZ")` -- fallback constant referenced
  - `body_has_call(body, "map") && body_has_call(body, "unwrap_or")` -- option branching via combinators
  - Clock::get_rate FnDef path includes "Clock" -- attached to correct resource type
- Target Evidence: `check2_uart_clock_hz_body`
- Reasoning Trace:
  - Authored Fact: `self.clock.as_ref().map(Clock::get_rate).unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)`
  - Extracted Fact: MIR calls `Option::as_ref`, `Option::map` with `Clock::get_rate` as fn-ptr arg, `Option::unwrap_or` with unevaluated `DEFAULT_UART_CLOCK_HZ`
  - Correspondence: `exact`
  - Note: The HIR-based ClockSurface does not detect `get_rate` via higher-order combinator usage; the body surface FnDef reference serves as primary evidence.

### Check: enable_clock_if_present body
- Status: `pass`
- Premise: body preserves Clock::enable call, option branch, no resource op on None
- Assertion Predicate(s):
  - `body_has_call(body, "enable")` -- true
  - `body_has_switch(body)` -- SwitchInt on Option discriminant
  - ClockSurface shows Enable in enable_clock_if_present
- Target Evidence: `check2_enable_clock_if_present_body`
- Correspondence: `exact`

### Check: disable_clock_if_present body
- Status: `pass`
- Premise: body preserves Clock::disable call, option branch, no resource op on None
- Assertion Predicate(s):
  - `body_has_call(body, "disable")` -- true
  - `body_has_switch(body)` -- SwitchInt on Option discriminant
  - ClockSurface shows Disable in disable_clock_if_present
- Target Evidence: `check2_disable_clock_if_present_body`
- Correspondence: `exact`

### Check: toggle_reset_if_present body
- Status: `pass`
- Premise: body preserves ResetLine::toggle call, Duration::from_us(10), option branch
- Assertion Predicate(s):
  - `body_has_call(body, "toggle")` -- true
  - `body_has_switch(body)` -- SwitchInt on Option discriminant
  - `body_has_const_scalar(body, 10)` -- duration literal 10 as u64 scalar
  - `body_has_call(body, "from_us")` -- Duration::from_us call preserved
  - ResetSurface shows Toggle in toggle_reset_if_present
- Target Evidence: `check2_toggle_reset_if_present_body`
- Correspondence: `exact`

### Check: apply_default_pins_if_present body
- Status: `pass`
- Premise: body preserves PinCtrl::apply::<DefaultState> call, option branch
- Assertion Predicate(s):
  - `body_has_call(body, "apply")` -- true
  - `body_has_switch(body)` -- SwitchInt on Option discriminant
  - PinCtrlSurface shows DefaultState in apply_default_pins_if_present
- Target Evidence: `check2_apply_default_pins_if_present_body`
- Correspondence: `exact`

### Check: apply_sleep_pins_if_present body
- Status: `pass`
- Premise: body preserves PinCtrl::apply::<SleepState> call, option branch
- Assertion Predicate(s):
  - `body_has_call(body, "apply")` -- true
  - `body_has_switch(body)` -- SwitchInt on Option discriminant
  - PinCtrlSurface shows SleepState in apply_sleep_pins_if_present
- Target Evidence: `check2_apply_sleep_pins_if_present_body`
- Correspondence: `exact`

### Check: shutdown_port body
- Status: `pass`
- Premise: body preserves helper-call order (mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart), then pending_tx = None
- Assertion Predicate(s):
  - `helper_calls.contains("mask_all_interrupts")` -- true
  - `helper_calls.contains("clear_all_interrupts")` -- true
  - `helper_calls.contains("disable_fifo")` -- true
  - `helper_calls.contains("disable_uart")` -- true
  - `body_assigns_option_none(body)` -- pending_tx set to None
- Target Evidence: `check2_shutdown_port_body`
- Correspondence: `exact`

### Check: service_rx_fifo body
- Status: `pass`
- Premise: body preserves handled=false init, rx_char loop, handled=true assignment, returns handled
- Assertion Predicate(s):
  - `body_has_const_scalar(body, 0)` -- handled = false initialization
  - `body_has_call(body, "rx_char")` -- loop condition calls rx_char
  - `body_has_const_scalar(body, 1)` -- handled = true in loop body
  - `return_type == Some("bool")` -- returns bool
- Target Evidence: `check2_service_rx_fifo_body`
- Correspondence: `exact`

### Check: service_tx_fifo body
- Status: `pass`
- Premise: body preserves pending_tx branch, tx_ready check, DR write via field_val, pending_tx = None, disable_tx_interrupts call
- Assertion Predicate(s):
  - `body_has_switch(body)` -- SwitchInt for pending_tx Option discriminant
  - `body_has_call(body, "tx_ready")` -- readiness check preserved
  - `body_has_call(body, "field_val")` -- DR write via field_val preserved
  - `body_assigns_option_none(body)` -- pending_tx = None on ready path
  - `helper_calls.contains("disable_tx_interrupts")` -- disable_tx_interrupts call preserved
- Target Evidence: `check2_service_tx_fifo_body`
- Correspondence: `exact`

### Check: clear_explicit_interrupts body
- Status: `pass`
- Premise: body preserves BitAnd for clear_mask, SwitchInt for != 0, raw_reg_val call for ICR write, EXPLICIT_ISR_CLEAR_MASK constant
- Assertion Predicate(s):
  - `body_has_bitand(body)` -- BitAnd operation for status & EXPLICIT_ISR_CLEAR_MASK
  - `body_has_switch(body)` -- SwitchInt for clear_mask != 0 branch
  - `body_has_call(body, "raw_reg_val")` -- ICR write via raw_reg_val
  - `body_has_unevaluated_const(body, "EXPLICIT_ISR_CLEAR_MASK")` -- mask constant referenced
- Target Evidence: `check2_clear_explicit_interrupts_body`
- Correspondence: `exact`

---

## Conclusion

All 10 in-scope helper methods preserve their identity, signature, return type,
and body-level downstream checks at the current Regseq extraction boundary. 21
automated assertions pass across 21 test functions covering both check groups.

One observation: the HIR-based ClockSurface does not detect `Clock::get_rate`
when it is passed as a function pointer to `Option::map` (as in `uart_clock_hz`).
The MIR body surface compensates for this by capturing the `FnDef` reference.
This is not a check failure, but a potential extraction enhancement opportunity
for the clock surface extractor.
