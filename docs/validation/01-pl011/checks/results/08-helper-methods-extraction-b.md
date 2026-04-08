# Validation Certificate: PL011 Helper Methods Extraction Check B

Source premise: `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - The test `specimen_fidelity_against_fixture` reads the real PL011 fixture
    source at `tests/fixtures/pl011-full-spec/src/driver.rs` and verifies that
    every authored fact for the 10 in-scope helper methods matches verbatim.
  - Assertions cover: method signatures, return types, body structure (call
    targets, branching patterns, constant literals, call ordering), and constant
    definitions.
- Fidelity Evidence (Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:847` exercises the fixture/source fidelity checks for all 10 in-scope helpers.
  - `fixture.contains("fn uart_clock_hz(&self) -> u32")` -- signature fidelity
  - `fixture.contains(".map(Clock::get_rate)")` -- body fidelity
  - `fixture.contains(".unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)")` -- body fidelity
  - `fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;")` -- constant fidelity
  - `fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>")` -- signature
  - `fixture.contains("clock.enable()?;")` -- body call
  - `fixture.contains("fn disable_clock_if_present(&self)")` -- signature
  - `fixture.contains("clock.disable();")` -- body call
  - `fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>")` -- signature
  - `fixture.contains("reset.toggle(Duration::from_us(10))?;")` -- body call
  - `fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>")` -- signature
  - `fixture.contains("pinctrl.apply::<DefaultState>()?;")` -- body call
  - `fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>")` -- signature
  - `fixture.contains("pinctrl.apply::<SleepState>()?;")` -- body call
  - `fixture.contains("fn shutdown_port(&mut self)")` -- signature
  - Call order verification: mask_all < clear_all < disable_fifo < disable_uart < pending_tx=None
  - `fixture.contains("fn service_rx_fifo(&mut self) -> bool")` -- signature
  - `fixture.contains("let mut handled = false;")` -- body init
  - `fixture.contains("while self.rx_char().is_some()")` -- loop pattern
  - `fixture.contains("fn service_tx_fifo(&mut self) -> bool")` -- signature
  - `fixture.contains("if let Some(byte) = self.pending_tx")` -- branching
  - `fixture.contains("DR_REG::DATA::FIELD, byte as u32")` -- register write
  - `fixture.contains("fn clear_explicit_interrupts(&mut self, status: u32)")` -- signature
  - `fixture.contains("let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;")` -- computation
  - `fixture.contains("if clear_mask != 0")` -- guard
  - `fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;")` -- constant
  - `fixture.contains("const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);")` -- constant

## Check-to-Surface Mapping

| Check Group | Surface | Extraction Function |
|---|---|---|
| CG1 (identity/signature) | IdentityEnrichmentSurface.impl_blocks, MirBodySurface.functions | extract_identity_enrichment, extract_body_surface |
| CG2 (body-level) | MirBodySurface.functions (MIR body inspection) | extract_body_surface |

## Check Group 1

- Check: Every in-scope helper preserves identity, signature, and return type.
- Status: `pass`
- Premise: 10 helper methods (uart_clock_hz, enable_clock_if_present, disable_clock_if_present, toggle_reset_if_present, apply_default_pins_if_present, apply_sleep_pins_if_present, shutdown_port, service_rx_fifo, service_tx_fifo, clear_explicit_interrupts) preserve identity, signature (receiver, params), and return type.
- Assertion Predicate(s):
  - All 10 helpers exist in `MirBodySurface.functions`
  - All 10 helpers appear in the inherent impl block methods on `Pl011FullSpec`
  - `uart_clock_hz`: receiver=ref, params=[], return_type=Some("u32"), MIR local 0 is u32
  - `enable_clock_if_present`: receiver=ref, params=[], return_type contains "Result", MIR local 0 is Result ADT
  - `disable_clock_if_present`: receiver=ref, params=[], return_type=None, MIR local 0 is ()
  - `toggle_reset_if_present`: receiver=ref, params=[], return_type contains "Result"
  - `apply_default_pins_if_present`: receiver=ref, return_type contains "Result"
  - `apply_sleep_pins_if_present`: receiver=ref, return_type contains "Result"
  - `shutdown_port`: receiver=ref_mut, params=[], return_type=None
  - `service_rx_fifo`: receiver=ref_mut, params=[], return_type=Some("bool"), MIR local 0 is bool
  - `service_tx_fifo`: receiver=ref_mut, params=[], return_type=Some("bool"), MIR local 0 is bool
  - `clear_explicit_interrupts`: receiver=ref_mut, params=[("status", "u32")], return_type=None
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_helper_identity_and_signature` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1010`:
    - `find_fn(&surface.bodies, name).is_some()` for each of 10 helpers
    - `inherent_impl.methods.iter().any(|m| m.name == *name)` for each helper
    - `f.receiver == "ref"` / `"ref_mut"` per method
    - `f.return_type.as_deref() == Some("u32")` for uart_clock_hz
    - `return_type_is_u32(f)` confirms MIR return local
    - `f.return_type.as_ref().map(|r| r.contains("Result"))` for fallible helpers
    - `f.params.len() == 1` and `f.params[0].name == "status"` for clear_explicit_interrupts
- Reasoning Trace:
  - Authored Fact (Verbatim): "method identity, signature, parameters, and return type survive"
  - Extracted Fact (Verbatim): All 10 helpers found in MirBodySurface with matching receiver, params, return_type; all found in IdentityEnrichmentSurface inherent impl block
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: Every named property (identity, receiver, params, return type) is individually asserted for each in-scope method
- Counterexample: N/A
- Conclusion: Check Group 1 passes. All 10 helper methods preserve identity, signature, and return type at the extraction boundary.

## Check Group 2

### uart_clock_hz

- Check: Body preserves Clock::get_rate call via Some path, DEFAULT_UART_CLOCK_HZ fallback via None path, Option branching.
- Status: `fail`
- Assertion Predicate(s):
  - `body_has_call_containing(f, "get_rate") || body_has_call_containing(f, "map")` -- get_rate is passed as fn pointer to Option::map
  - `body_has_const_scalar(f, 48_000_000) || body_has_const_ref_containing(f, "DEFAULT_UART_CLOCK_HZ")` -- fallback constant
  - `body_has_switch_int(f) || body_has_call_containing(f, "unwrap_or")` -- Option branching
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_uart_clock_hz_body` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1201`: calls_map or calls_get_rate asserted, const ref or scalar asserted, branch or unwrap_or asserted
- Reasoning Trace:
  - Authored Fact: returns u32; Some(clock) path calls Clock::get_rate; None path falls back to DEFAULT_UART_CLOCK_HZ (= 48_000_000); clock-rate read stays attached to the specific clock field
  - Extracted Fact: MIR contains call to map (carrying get_rate as FnDef), DEFAULT_UART_CLOCK_HZ constant reference, and Option discriminant branching or unwrap_or call
  - Correspondence: `weakened`
  - Gap / Loss Description: The test proves the presence of a map/get_rate-style call, fallback constant evidence, and Option branching, but it does not prove exact higher-order flow from `self.clock.as_ref().map(Clock::get_rate)` or that the clock-rate read remains attached to the specific `self.clock` field.
  - Why This Status Follows: The check contract requires path-sensitive proof of the higher-order `Clock::get_rate` combinator usage and exact fallback flow. The current assertions are coarse proxies, not exact path-preserving proofs.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  - Minimal Missing Fact To Preserve: Path-sensitive assertions over the specific `self.clock` access, the `Clock::get_rate` FnDef/type argument, and the `DEFAULT_UART_CLOCK_HZ` fallback path.

### enable_clock_if_present

- Check: Body preserves Option test on self.clock and fallible Clock::enable call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "enable")` -- Clock::enable call
- Target Evidence: `check_group_2_enable_clock_if_present_body` test exercises both assertions
  - Exact Citation: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1237`
- Reasoning Trace:
  - Authored Fact: tests self.clock once; Some path performs fallible Clock::enable; None path performs no resource operation
  - Extracted Fact: SwitchInt present for Option discriminant; enable call found in MIR
  - Correspondence: `exact`

### disable_clock_if_present

- Check: Body preserves Option test and Clock::disable call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "disable")` -- Clock::disable call
- Target Evidence: `check_group_2_disable_clock_if_present_body` test
  - Exact Citation: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1255`
- Correspondence: `exact`

### toggle_reset_if_present

- Check: Body preserves Option test, ResetLine::toggle call with Duration::from_us(10).
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "toggle")` -- ResetLine::toggle call
  - `body_has_call_containing(f, "from_us")` -- Duration::from_us call
  - `body_has_const_scalar(f, 10)` -- literal 10 for Duration::from_us(10)
- Target Evidence: `check_group_2_toggle_reset_if_present_body` test
  - Exact Citation: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1273`
- Correspondence: `exact`

### apply_default_pins_if_present

- Check: Body preserves Option test and PinCtrl::apply::<DefaultState> call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "apply")` -- PinCtrl::apply call
  - `body_has_call_containing_in_type(f, "DefaultState")` -- DefaultState type parameter present in FnDef type args
- Target Evidence: `check_group_2_apply_default_pins_if_present_body` test
  - Exact Citation: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1301`
- Correspondence: `exact`

### apply_sleep_pins_if_present

- Check: Body preserves Option test and PinCtrl::apply::<SleepState> call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "apply")` -- PinCtrl::apply call
  - `body_has_call_containing_in_type(f, "SleepState")` -- SleepState type parameter present in FnDef type args
- Target Evidence: `check_group_2_apply_sleep_pins_if_present_body` test
  - Exact Citation: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1327`
- Correspondence: `exact`

### shutdown_port

- Check: Body preserves helper-call order (mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart) and pending_tx = None.
- Status: `fail`
- Assertion Predicate(s):
  - `f.helper_calls.contains("mask_all_interrupts")` -- call present
  - `f.helper_calls.contains("clear_all_interrupts")` -- call present
  - `f.helper_calls.contains("disable_fifo")` -- call present
  - `f.helper_calls.contains("disable_uart")` -- call present
  - `ordered_call_names_matching(f, &[...]).len() >= 4` -- all 4 calls found in MIR bb chain
  - `mask_idx < clear_idx < fifo_idx < uart_idx` -- ordering verified via basic block terminator chain walk
  - `body_assigns_none_option(f)` -- pending_tx = None
- Target Evidence: `check_group_2_shutdown_port_body` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1353`
- Reasoning Trace:
  - Authored Fact: helper-call order: mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart; then self.pending_tx = None
  - Extracted Fact: All four helper calls found in helper_calls; call ordering proven by walking MIR basic block terminator chain (bb0 -> bb1 -> ...); None option assignment found via SetDiscriminant or Aggregate in MIR
  - Correspondence: `weakened`
  - Gap / Loss Description: The test proves helper-call order and the presence of a `pending_tx = None` assignment, but it does not prove that the assignment occurs after the four helper calls.
  - Why This Status Follows: The check contract requires exact placement of `pending_tx = None` after the authored helper sequence. The current assertions do not establish that placement.
  - Note: The helper_calls field uses BTreeSet (alphabetical order), so ordering is verified separately via `ordered_call_names_matching` which walks the basic block terminator chain from bb0 following Call.target and Goto.target edges.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  - Minimal Missing Fact To Preserve: A path-sensitive assertion that the None assignment occurs after the final `disable_uart` call in the MIR control-flow chain.

### service_rx_fifo

- Check: Body preserves handled=false init, rx_char loop, handled=true assignment, return handled.
- Status: `fail`
- Assertion Predicate(s):
  - `body_has_call_containing(f, "rx_char")` -- loop calls rx_char
  - `body_has_const_scalar(f, 0)` -- false/0 for handled init
  - `body_has_const_scalar(f, 1)` -- true/1 for handled = true assignment
  - `return_type_is_bool(f)` -- returns bool
  - `body_has_switch_int(f)` -- loop condition branching
- Target Evidence: `check_group_2_service_rx_fifo_body` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1416`
  - Correspondence: `weakened`
  - Gap / Loss Description: The test proves the presence of `rx_char`, bool literals `false` and `true`, a bool return type, and branching, but it does not prove that the loop condition is exactly `rx_char().is_some()`, that `handled = true` is set on each successful iteration, or that the returned bool is exactly the `handled` local.
  - Why This Status Follows: The check contract requires exact loop/flag/return correspondence. The current assertions only prove a coarse structural proxy.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  - Minimal Missing Fact To Preserve: Path-sensitive assertions over the loop condition, the `handled` local assignments, and the final returned local.

### service_tx_fifo

- Check: Body preserves pending_tx branching, tx_ready check, DR.DATA write, pending_tx=None, disable_tx_interrupts, return values.
- Status: `fail`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- pending_tx Option branching
  - `body_has_call_containing(f, "tx_ready")` -- tx_ready check
  - `f.helper_calls.contains("disable_tx_interrupts")` -- disable TX interrupts
  - `body_has_call_containing(f, "field_val")` -- DR.DATA write via field_val
  - `body_assigns_none_option(f)` -- pending_tx = None
  - `return_type_is_bool(f)` -- returns bool
- Target Evidence: `check_group_2_service_tx_fifo_body` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1450`
  - Correspondence: `weakened`
  - Gap / Loss Description: The test proves pending_tx branching, a `tx_ready` call, a DR write helper, a None assignment, and a `disable_tx_interrupts` helper call, but it does not prove the exact Some/None and ready/not-ready path split or the exact returned bool on each path.
  - Why This Status Follows: The check contract requires exact path-sensitive behavior for the four branches of `service_tx_fifo`. The current assertions are not strong enough for that conclusion.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  - Minimal Missing Fact To Preserve: Path-sensitive assertions proving the write/clear/disable/true path, the Some+not-ready false path, and the None+disable+true path separately.

### clear_explicit_interrupts

- Check: Body preserves BitAnd for status & EXPLICIT_ISR_CLEAR_MASK, guard on clear_mask != 0, ICR write via raw_reg_val.
- Status: `fail`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- clear_mask != 0 guard
  - BitAnd operation found in MIR statements -- status & mask
  - `body_has_call_containing(f, "raw_reg_val")` -- ICR write via raw_reg_val
  - `body_has_call_containing(f, "write")` -- ICR register write
- Target Evidence: `check_group_2_clear_explicit_interrupts_body` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs:1490`
- Reasoning Trace:
  - Authored Fact: computes clear_mask = status & EXPLICIT_ISR_CLEAR_MASK (= 0x0780); writes icr only when clear_mask != 0; uses clear_mask as both raw write mask and value
  - Extracted Fact: MIR contains BinaryOp::BitAnd for the mask computation; SwitchInt for the != 0 guard; raw_reg_val and write calls for ICR register access
  - Correspondence: `weakened`
  - Gap / Loss Description: The test proves the presence of a BitAnd, a guard, and the write path helpers, but it does not prove that the write occurs only on the nonzero branch or that `clear_mask` is used as both the raw write mask and the raw write value.
  - Why This Status Follows: The check contract requires exact conditional-write behavior. The current assertions are not path-sensitive enough to discharge that premise.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
  - Minimal Missing Fact To Preserve: Path-sensitive assertions tying the `clear_mask != 0` branch to the raw_reg_val/write path and proving `clear_mask` is passed in both argument positions.

## Conclusion

Check Group 1 passes.

Check Group 2 is mixed:
- `pass`: `enable_clock_if_present`, `disable_clock_if_present`, `toggle_reset_if_present`, `apply_default_pins_if_present`, `apply_sleep_pins_if_present`
- `fail`: `uart_clock_hz`, `shutdown_port`, `service_rx_fifo`, `service_tx_fifo`, `clear_explicit_interrupts`

The five failures are not fixture mismatches. They are real proof-strength gaps at the current extraction boundary review:
- `uart_clock_hz`: higher-order `Clock::get_rate` and fallback flow are only proven by coarse proxies
- `shutdown_port`: helper order is proven, but `pending_tx = None` placement is not
- `service_rx_fifo`: loop/flag/return correspondence is not path-sensitively proven
- `service_tx_fifo`: branch-specific behavior and returned bools are not path-sensitively proven
- `clear_explicit_interrupts`: conditional write and dual use of `clear_mask` are not path-sensitively proven
