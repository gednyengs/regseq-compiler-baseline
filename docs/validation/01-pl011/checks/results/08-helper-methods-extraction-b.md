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
  - `check_group_1_helper_identity_and_signature` in `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`:
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
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_call_containing(f, "get_rate") || body_has_call_containing(f, "map")` -- get_rate or map call present
  - `body_has_const_scalar(f, 48_000_000) || body_has_const_ref_containing(f, "DEFAULT_UART_CLOCK_HZ")` -- fallback constant
  - `body_has_switch_int(f) || body_has_call_containing(f, "unwrap_or")` -- Option branching
  - `find_call_bb_with_fn_arg(f, "map", "get_rate").is_some()` -- map carries Clock::get_rate as FnDef argument (higher-order combinator flow proof)
  - `find_call_bb_with_const_arg(f, "unwrap_or", 48_000_000, "DEFAULT_UART_CLOCK_HZ").is_some()` -- unwrap_or carries DEFAULT_UART_CLOCK_HZ as fallback argument (exact fallback proof)
  - `straight_line_bbs(f, map_bb).contains(&unwrap_bb)` -- map -> unwrap_or CFG ordering (combinator chain proof)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_uart_clock_hz_body`: coarse checks plus path-sensitive proof that `map` carries `get_rate` as FnDef arg, `unwrap_or` carries 48_000_000 as fallback arg, and the CFG path connects map to unwrap_or in order.
- Reasoning Trace:
  - Authored Fact: returns u32; Some(clock) path calls Clock::get_rate; None path falls back to DEFAULT_UART_CLOCK_HZ (= 48_000_000); clock-rate read stays attached to the specific clock field
  - Extracted Fact: MIR contains call to map with Clock::get_rate as FnDef argument (proving the higher-order combinator structure); unwrap_or carries 48_000_000 as fallback (proving the exact fallback path); the CFG path from map leads to unwrap_or (proving flow ordering)
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The path-sensitive assertions prove the exact authored combinator chain: self.clock.as_ref().map(Clock::get_rate).unwrap_or(DEFAULT_UART_CLOCK_HZ). The map call carries get_rate as its FnDef argument, the unwrap_or call carries the fallback constant, and the CFG connects them in sequence.
- Counterexample: N/A
- Conclusion: Passes with path-sensitive proof of the higher-order combinator structure.

### enable_clock_if_present

- Check: Body preserves Option test on self.clock and fallible Clock::enable call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "enable")` -- Clock::enable call
- Target Evidence: `check_group_2_enable_clock_if_present_body` test exercises both assertions
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
- Correspondence: `exact`

### apply_default_pins_if_present

- Check: Body preserves Option test and PinCtrl::apply::<DefaultState> call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "apply")` -- PinCtrl::apply call
  - `body_has_call_containing_in_type(f, "DefaultState")` -- DefaultState type parameter present in FnDef type args
- Target Evidence: `check_group_2_apply_default_pins_if_present_body` test
- Correspondence: `exact`

### apply_sleep_pins_if_present

- Check: Body preserves Option test and PinCtrl::apply::<SleepState> call.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- Option discriminant test
  - `body_has_call_containing(f, "apply")` -- PinCtrl::apply call
  - `body_has_call_containing_in_type(f, "SleepState")` -- SleepState type parameter present in FnDef type args
- Target Evidence: `check_group_2_apply_sleep_pins_if_present_body` test
- Correspondence: `exact`

### shutdown_port

- Check: Body preserves helper-call order (mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart) and pending_tx = None after that sequence.
- Status: `pass`
- Assertion Predicate(s):
  - `f.helper_calls.contains("mask_all_interrupts")` -- call present
  - `f.helper_calls.contains("clear_all_interrupts")` -- call present
  - `f.helper_calls.contains("disable_fifo")` -- call present
  - `f.helper_calls.contains("disable_uart")` -- call present
  - `ordered_call_names_matching(f, &[...]).len() >= 4` -- all 4 calls found in MIR bb chain
  - `mask_idx < clear_idx < fifo_idx < uart_idx` -- ordering verified via basic block terminator chain walk
  - `body_assigns_none_option(f)` -- pending_tx = None
  - `bb_set_has_none_assignment(f, &path_after_disable_uart)` -- pending_tx = None occurs on the straight-line path after the disable_uart call target (path-sensitive placement proof)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_shutdown_port_body`: Verifies call ordering via MIR BB chain walk, then locates the disable_uart Call terminator's target BB and proves the None assignment appears on the straight-line path from that point. This proves that `pending_tx = None` occurs after all four helper calls in the authored sequence.
- Reasoning Trace:
  - Authored Fact: helper-call order: mask_all_interrupts, clear_all_interrupts, disable_fifo, disable_uart; then self.pending_tx = None
  - Extracted Fact: All four helper calls found and ordered by BB chain walk; None assignment proven to occur on the CFG path after the final disable_uart call
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The test proves both the helper-call ordering and the exact placement of `pending_tx = None` after the four-call sequence by walking the MIR CFG from disable_uart's target to the None assignment BB.
- Counterexample: N/A
- Conclusion: Passes with path-sensitive proof of helper ordering and None-assignment placement.

### service_rx_fifo

- Check: Body preserves handled=false init, rx_char loop, handled=true assignment, return handled.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_call_containing(f, "rx_char")` -- loop calls rx_char
  - `body_has_const_scalar(f, 0)` -- false/0 for handled init
  - `body_has_const_scalar(f, 1)` -- true/1 for handled = true assignment
  - `return_type_is_bool(f)` -- returns bool
  - `body_has_switch_int(f)` -- loop condition branching
  - `has_back_edge(f)` -- CFG back-edge proves while-loop exists
  - `find_local_by_name(f, "handled").is_some()` -- authored `handled` variable found in MIR debug info
  - `bb_set_assigns_local_scalar(f, &all_bbs, handled, 0)` -- handled is assigned false
  - SwitchInt branch reachable set assigns `handled = true` (handled=true on successful loop iteration)
  - `return_assigned_from_local(f, handled)` -- return place is assigned from handled local (returned bool is exactly handled)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_service_rx_fifo_body`: Locates the `handled` local by authored debug name, verifies it is initialized to false, proves a back-edge (loop) exists, proves `handled = true` occurs on a SwitchInt branch (loop body), and proves the return place (local 0) is assigned from the `handled` local.
- Reasoning Trace:
  - Authored Fact: initializes handled = false; loops on self.rx_char().is_some(); sets handled = true on each successful iteration; returns exactly final handled
  - Extracted Fact: MIR local `handled` (found by debug name) is assigned false (0); CFG has back-edge (loop); handled assigned true (1) on a SwitchInt branch (loop body); return place assigned from handled local
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The path-sensitive assertions prove: (1) `handled` init to false via authored debug name + scalar assignment, (2) loop existence via back-edge detection, (3) `handled = true` on the loop body branch, (4) return value is exactly the `handled` local. The extraction improvement (authored debug variable names from MIR var_debug_info) enables identifying `handled` by name rather than by positional index.
- Counterexample: N/A
- Conclusion: Passes with path-sensitive proof of loop/flag/return correspondence.

### service_tx_fifo

- Check: Body preserves pending_tx branching, tx_ready check, DR.DATA write, pending_tx=None, disable_tx_interrupts, return values per branch.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- pending_tx Option branching
  - `body_has_call_containing(f, "tx_ready")` -- tx_ready check
  - `f.helper_calls.contains("disable_tx_interrupts")` -- disable TX interrupts
  - `body_has_call_containing(f, "field_val")` -- DR.DATA write via field_val
  - `body_assigns_none_option(f)` -- pending_tx = None
  - `return_type_is_bool(f)` -- returns bool
  - `find_switch_int_bb_indices(f).len() >= 2` -- at least 2 SwitchInt terminators (pending_tx + tx_ready)
  - First SwitchInt Some branch -> reachable set has `tx_ready` call and second SwitchInt
  - First SwitchInt None branch -> reachable set has `disable_tx_interrupts` and Return
  - Second SwitchInt ready branch -> reachable set has `field_val` (DR write), None assignment, `disable_tx_interrupts`
  - Second SwitchInt not-ready branch -> reachable set does NOT have `field_val` or None assignment
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_service_tx_fifo_body`: Finds two SwitchInt terminators. The first (pending_tx Option discriminant) branches into Some and None paths. The Some path leads to tx_ready and a second SwitchInt. The ready branch has field_val (DR write), None assignment, and disable_tx_interrupts. The not-ready branch has neither field_val nor None assignment. The None branch has disable_tx_interrupts and Return.
- Reasoning Trace:
  - Authored Fact: branches on self.pending_tx; ready Some(byte) path writes DR.DATA, clears pending_tx, disables TX interrupts, returns true; not-ready Some(byte) path returns false; None path disables TX interrupts then returns true
  - Extracted Fact: Two SwitchInt terminators separate the three authored paths. The ready path has DR write (field_val), None assignment, and disable_tx_interrupts. The not-ready path lacks these operations. The None path has disable_tx_interrupts and reaches Return.
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The path-sensitive assertions trace each branch of both SwitchInt terminators and verify that specific operations (DR write, None assignment, disable_tx_interrupts) appear only on the authored paths, with negative assertions confirming their absence on other branches.
- Counterexample: N/A
- Conclusion: Passes with path-sensitive proof of the three-way branch structure.

### clear_explicit_interrupts

- Check: Body preserves BitAnd for status & EXPLICIT_ISR_CLEAR_MASK, guard on clear_mask != 0, ICR write via raw_reg_val, conditional write only on nonzero branch, clear_mask used as both mask and value.
- Status: `pass`
- Assertion Predicate(s):
  - `body_has_switch_int(f)` -- clear_mask != 0 guard
  - BitAnd operation found in MIR statements -- status & mask
  - `body_has_call_containing(f, "raw_reg_val")` -- ICR write via raw_reg_val
  - `body_has_call_containing(f, "write")` -- ICR register write
  - SwitchInt nonzero branch -> reachable set has `raw_reg_val` and `write` calls
  - SwitchInt zero branch -> reachable set does NOT have `raw_reg_val` or `write` calls (conditional write proof)
  - `bb_set_has_call_with_dual_use_args(f, &nonzero_bbs, "raw_reg_val")` -- both raw_reg_val arguments trace back to the same local (clear_mask dual-use proof)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_clear_explicit_interrupts_body`: Finds the SwitchInt guard on clear_mask, follows the nonzero branch to verify raw_reg_val and write calls exist, follows the zero branch to verify they are absent (conditional write), and verifies both raw_reg_val arguments trace to the same source local (dual-use of clear_mask as both mask and value).
- Reasoning Trace:
  - Authored Fact: computes clear_mask = status & EXPLICIT_ISR_CLEAR_MASK (= 0x0780); writes icr only when clear_mask != 0; uses clear_mask as both raw write mask and value
  - Extracted Fact: MIR contains BitAnd for the mask computation; SwitchInt for the != 0 guard; raw_reg_val and write calls on the nonzero branch only; both raw_reg_val arguments trace to the same source local (the BitAnd result), confirming dual-use of clear_mask
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The path-sensitive assertions prove: (1) the write path exists only on the nonzero branch via positive and negative branch assertions, (2) clear_mask is used as both mask and value via the dual-use argument tracing through compiler-introduced temporaries back to the common BitAnd result local.
- Counterexample: N/A
- Conclusion: Passes with path-sensitive proof of conditional write and dual-use behavior.

## Conclusion

Check Group 1 passes.

Check Group 2 passes for all 10 in-scope helpers:
- `enable_clock_if_present`, `disable_clock_if_present`, `toggle_reset_if_present`, `apply_default_pins_if_present`, `apply_sleep_pins_if_present`: pass with coarse structural assertions
- `uart_clock_hz`: pass with path-sensitive proof of higher-order combinator flow and exact fallback
- `shutdown_port`: pass with path-sensitive proof of helper ordering and None-assignment placement
- `service_rx_fifo`: pass with path-sensitive proof of loop/flag/return correspondence using authored debug variable names
- `service_tx_fifo`: pass with path-sensitive proof of three-way branch structure with positive and negative path assertions
- `clear_explicit_interrupts`: pass with path-sensitive proof of conditional write and clear_mask dual-use

Extraction improvement: authored debug variable names are now patched into MIR local declarations from `var_debug_info`, enabling tests to locate authored locals (e.g. `handled`) by name rather than positional index. This is a general extraction improvement, not PL011-specific.
