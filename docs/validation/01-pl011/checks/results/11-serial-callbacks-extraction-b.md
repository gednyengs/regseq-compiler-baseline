# Validation Certificate: PL011 Serial Callbacks Extraction Check B

Source premise: `docs/validation/01-pl011/checks/11-serial-callbacks-extraction-b.md`
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs`

## Specimen Fidelity

- Specimen type: reduced inline specimen
- Fidelity Assertion Predicate(s):
  - `fixture.contains("fn tx_ready(&mut self) -> bool")` -- fixture has tx_ready signature
  - `fixture.contains("self.regs.fr.read(FR_REG::TXFF::FIELD) == 0")` -- fixture has tx_ready body
  - `fixture.contains("fn tx_empty(&mut self) -> bool")` -- fixture has tx_empty signature
  - `fixture.contains("self.regs.fr.read(FR_REG::BUSY::FIELD) == 0")` -- fixture has tx_empty body
  - `fixture.contains("fn flush_rx_fifo(&mut self)")` -- fixture has flush_rx_fifo signature
  - `fixture.contains("while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0")` -- fixture has flush_rx_fifo loop
  - `fixture.contains("let _ = self.regs.dr.read_raw()")` -- fixture has flush_rx_fifo drain
  - `fixture.contains("fn flush_tx_fifo(&mut self)")` -- fixture has flush_tx_fifo signature
  - `fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)")` -- fixture has FEN disable
  - `fixture.contains("self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED)")` -- fixture has FEN enable
  - `fixture.contains("fn send_break(&mut self, on: bool)")` -- fixture has send_break signature
  - `fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED)")` -- fixture has BRK enable
  - `fixture.contains("self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED)")` -- fixture has BRK disable
  - `fixture.contains("impl Serial for Pl011FullSpec")` -- fixture has Serial impl
  - Corresponding SPECIMEN.contains assertions verify specimen reproduces each fact
- Fidelity Evidence (Exact Citations):
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L525)
  - [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L345)

## Check-to-Surface Mapping

| Callback | TraitSurface | IdentityEnrichmentSurface | MirBodySurface |
|---|---|---|---|
| tx_ready | serial_impls[0].driver_type == "Pl011FullSpec" | impl_blocks[Serial].methods contains "tx_ready" | functions["tx_ready"].kind, receiver, params, return_type, body |
| tx_empty | serial_impls[0].driver_type == "Pl011FullSpec" | impl_blocks[Serial].methods contains "tx_empty" | functions["tx_empty"].kind, receiver, params, return_type, body |
| flush_rx_fifo | serial_impls[0].driver_type == "Pl011FullSpec" | impl_blocks[Serial].methods contains "flush_rx_fifo" | functions["flush_rx_fifo"].kind, receiver, params, return_type, body |
| flush_tx_fifo | serial_impls[0].driver_type == "Pl011FullSpec" | impl_blocks[Serial].methods contains "flush_tx_fifo" | functions["flush_tx_fifo"].kind, receiver, params, return_type, body |
| send_break | serial_impls[0].driver_type == "Pl011FullSpec" | impl_blocks[Serial].methods contains "send_break" | functions["send_break"].kind, receiver, params, return_type, body |

---

## Check Group 1

- Check: Serial callback identity, trait role, signature, and return type survive extraction
- Status: `pass`
- Premise: serial callback identity, trait role, signature, and return type survive
- Assertion Predicate(s):
  - `ext.trait_surface.serial_impls.len() == 1` -- exactly one Serial impl
  - `ext.trait_surface.serial_impls[0].driver_type == "Pl011FullSpec"` -- impl is for correct driver type
  - `serial_impl.trait_name == Some("Serial")` -- identity enrichment has Serial trait
  - `serial_impl.self_type_name == "Pl011FullSpec"` -- correct self type
  - `method_names.contains("tx_ready")` (and tx_empty, flush_rx_fifo, flush_tx_fifo, send_break) -- all five methods present
  - `tx_ready.receiver == "ref_mut"` -- correct receiver
  - `tx_ready.params.is_empty()` -- no params
  - `tx_ready.return_type contains "bool"` -- returns bool
  - `tx_ready.kind == Callback { trait_name: "Serial", method_name: "tx_ready" }` -- correct classification
  - (analogous for tx_empty: receiver ref_mut, no params, returns bool, Callback Serial::tx_empty)
  - `flush_rx_fifo.receiver == "ref_mut"`, `.params.is_empty()`, `.return_type == None` -- unit return
  - `flush_rx_fifo.kind == Callback { Serial, flush_rx_fifo }`
  - (analogous for flush_tx_fifo: ref_mut, no params, unit return, Callback Serial::flush_tx_fifo)
  - `send_break.receiver == "ref_mut"`, `.params.len() == 1`, `.params[0].name == "on"`, `.params[0].ty contains "bool"`, `.return_type == None`
  - `send_break.kind == Callback { Serial, send_break }`
- Target Evidence (Exercised by Test; Exact Citations):
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L660)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L676)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L714)
- Supporting Evidence (Optional; Exact Citations): N/A
- Reasoning Trace:
  - Authored Fact (Verbatim): tx_ready: `fn tx_ready(&mut self) -> bool`, tx_empty: `fn tx_empty(&mut self) -> bool`, flush_rx_fifo: `fn flush_rx_fifo(&mut self)`, flush_tx_fifo: `fn flush_tx_fifo(&mut self)`, send_break: `fn send_break(&mut self, on: bool)`
  - Extracted Fact (Verbatim): All five callbacks extracted with correct trait role (Serial), receiver (ref_mut), parameter signatures, and return types
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: All identity, trait-role, signature, and return-type properties are asserted via non-tautological predicates against extraction surfaces
- Counterexample: N/A
- Suggested Fix Direction / Implementation Hints: N/A
- Conclusion: All five serial callbacks in group B have their identity, trait role, signatures, and return types preserved through extraction.

---

## Check Group 2

- Check: Serial callback bodies preserve the downstream checks captured in the authored-source entries
- Status: `pass`
- Premise: serial callback bodies preserve the downstream checks captured in the authored-source entries
- Assertion Predicate(s):
  - **tx_ready**: `has_call_to(&f.body, "read")` -- calls register read; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `collect_call_arg_const_paths(&f.body, "read")` contains `FR_REG::TXFF::FIELD` -- exact field identity; `has_return(&f.body)` -- has Return; `f.return_type contains "bool"` -- returns bool
  - **tx_empty**: `has_call_to(&f.body, "read")` -- calls register read; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `collect_call_arg_const_paths(&f.body, "read")` contains `FR_REG::BUSY::FIELD` -- exact field identity; `has_return(&f.body)` -- has Return; `f.return_type contains "bool"` -- returns bool
  - **flush_rx_fifo**: `has_call_to(&f.body, "read")` -- calls read for loop condition; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `has_call_to(&f.body, "read_raw")` -- calls read_raw for DR drain; `has_switch_int(&f.body)` -- has SwitchInt for while condition; `has_goto(&f.body)` -- has Goto for loop back-edge; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit
  - **flush_tx_fifo**: `count_calls_to(&f.body, "modify") == 2` -- exactly two modify calls; `collect_calls_with_type_args` verifies both modify calls instantiated with `LCRH_REG::Register`; `modify_indices[0] < modify_indices[1]` -- first modify before second; `collect_call_arg_const_paths(&f.body, "modify")[0]` contains `FEN::DISABLED` -- first modify passes FEN::DISABLED; `collect_call_arg_const_paths(&f.body, "modify")[1]` contains `FEN::ENABLED` -- second modify passes FEN::ENABLED; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit
  - **send_break**: `has_switch_int(&f.body)` -- has SwitchInt for `if on` branch; `has_call_to(&f.body, "modify")` -- calls modify for LCR_H; `count_calls_to(&f.body, "modify") == 2` -- exactly two modify calls (one per branch); `collect_calls_with_type_args` verifies both modify calls instantiated with `LCRH_REG::Register`; `collect_branch_call_arg_paths` true branch contains `BRK::ENABLED` -- on==true maps to BRK::ENABLED; `collect_branch_call_arg_paths` false branch contains `BRK::DISABLED` -- on==false maps to BRK::DISABLED; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit; `f.params[0].name == "on"` -- param named on; `f.params[0].ty contains "bool"` -- param type bool
- Target Evidence (Exercised by Test; Exact Citations):
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L930) -- check_group_2_tx_ready_body
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L971) -- check_group_2_tx_empty_body
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L1009) -- check_group_2_flush_rx_fifo_body
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L1054) -- check_group_2_flush_tx_fifo_body
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L1132) -- check_group_2_send_break_body
- Supporting Evidence (Optional; Exact Citations): N/A
- Reasoning Trace:
  - Authored Fact (Verbatim):
    - tx_ready returns exactly whether TX FIFO is not full using `FR.TXFF`
    - tx_empty returns exactly whether the transmitter is not busy using `FR.BUSY`
    - flush_rx_fifo loops while `FR.RXFE == 0` and drains `DR` via `read_raw()` on each iteration
    - flush_tx_fifo performs exactly two sequential `lcr_h.modify(...)` calls: first `LCRH_REG::FEN::DISABLED`, then `LCRH_REG::FEN::ENABLED`
    - send_break branches on `on: bool`, modifies `LCR_H`, preserves `on == true -> BRK::ENABLED` and `on == false -> BRK::DISABLED`
  - Extracted Fact (Verbatim): All five callback bodies contain the expected MIR patterns with exact symbolic constant identity: tx_ready call arg contains `FR_REG::TXFF::FIELD`, tx_empty call arg contains `FR_REG::BUSY::FIELD`, flush_tx_fifo first modify arg contains `FEN::DISABLED` and second contains `FEN::ENABLED`, send_break true-branch modify arg contains `BRK::ENABLED` and false-branch contains `BRK::DISABLED`. flush_rx_fifo preserves loop structure (SwitchInt + Goto), read targeting FR_REG, and read_raw for DR drain.
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: The MIR extraction preserves `Const::Unevaluated` references in call arguments, which carry the full symbolic def-path of Field and FieldValue constants (e.g., `regs::FR_REG::TXFF::FIELD[0=0x20,1=0x5]`). This path distinguishes `TXFF::FIELD` from `BUSY::FIELD`, `FEN::DISABLED` from `FEN::ENABLED`, and `BRK::ENABLED` from `BRK::DISABLED`. The SwitchInt bool encoding (value 0 = false target, otherwise = true target) combined with per-branch call arg constant paths proves the exact branch-to-value mapping for send_break. All required authored facts are proven by non-tautological exercised assertions.
- Counterexample: N/A
- Suggested Fix Direction / Implementation Hints: N/A
- Conclusion: pass -- serial callback body structure and exact field-level identity, constant value ordering, and branch-to-value mappings are all directly provable at the current extraction boundary via symbolic constant paths preserved in MIR call arguments.
