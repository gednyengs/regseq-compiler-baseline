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
- Status: `fail`
- Premise: serial callback bodies preserve the downstream checks captured in the authored-source entries
- Assertion Predicate(s):
  - **tx_ready**: `has_call_to(&f.body, "read")` -- calls register read; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `has_return(&f.body)` -- has Return; `f.return_type contains "bool"` -- returns bool
  - **tx_empty**: `has_call_to(&f.body, "read")` -- calls register read; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `has_return(&f.body)` -- has Return; `f.return_type contains "bool"` -- returns bool
  - **flush_rx_fifo**: `has_call_to(&f.body, "read")` -- calls read for loop condition; `call_has_type_arg(&f.body, "read", "FR_REG")` -- read targets FR register module; `has_call_to(&f.body, "read_raw")` -- calls read_raw for DR drain; `has_switch_int(&f.body)` -- has SwitchInt for while condition; `has_goto(&f.body)` -- has Goto for loop back-edge; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit
  - **flush_tx_fifo**: `count_calls_to(&f.body, "modify") == 2` -- exactly two modify calls; `collect_calls_with_type_args` verifies both modify calls instantiated with `LCRH_REG::Register`; `modify_indices[0] < modify_indices[1]` -- first modify before second; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit
  - **send_break**: `has_switch_int(&f.body)` -- has SwitchInt for `if on` branch; `has_call_to(&f.body, "modify")` -- calls modify for LCR_H; `count_calls_to(&f.body, "modify") == 2` -- exactly two modify calls (one per branch); `collect_calls_with_type_args` verifies both modify calls instantiated with `LCRH_REG::Register`; `has_return(&f.body)` -- has Return; `f.return_type.is_none()` -- returns unit; `f.params[0].name == "on"` -- param named on; `f.params[0].ty contains "bool"` -- param type bool
- Target Evidence (Exercised by Test; Exact Citations):
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L859)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L896)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L932)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L979)
  - [source_to_extraction_11_serial_callbacks_b.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs#L1046)
- Supporting Evidence (Optional; Exact Citations): N/A
- Reasoning Trace:
  - Authored Fact (Verbatim):
    - tx_ready returns exactly whether TX FIFO is not full using `FR.TXFF`
    - tx_empty returns exactly whether the transmitter is not busy using `FR.BUSY`
    - flush_rx_fifo loops while `FR.RXFE == 0` and drains `DR` via `read_raw()` on each iteration
    - flush_tx_fifo performs exactly two sequential `lcr_h.modify(...)` calls: first `LCRH_REG::FEN::DISABLED`, then `LCRH_REG::FEN::ENABLED`
    - send_break branches on `on: bool`, modifies `LCR_H`, preserves `on == true -> BRK::ENABLED` and `on == false -> BRK::DISABLED`
  - Extracted Fact (Verbatim): All five callback bodies contain the expected MIR patterns: register read calls targeting FR_REG, loop structures (SwitchInt + Goto for flush_rx_fifo), sequential modify calls targeting LCRH_REG (flush_tx_fifo), branching modify calls targeting LCRH_REG (send_break)
  - Correspondence: `weakened`
  - Gap / Loss Description:
    - **tx_ready / tx_empty field identity**: MIR exposes that `read` is called with generic `R = FR_REG::Register`, proving FR register access. However, the specific Field constant (TXFF vs BUSY) passed as the argument is not distinguishable in MIR because both fields share the same `FR_REG::Register` type parameter. The authored fact that tx_ready tests FR.TXFF specifically (and tx_empty tests FR.BUSY specifically) cannot be proven at the field level.
    - **flush_tx_fifo value ordering**: MIR proves two ordered modify calls both targeting `LCRH_REG::Register`. However, the specific FieldValue constants (`FEN::DISABLED` and `FEN::ENABLED`) are inlined as scalar values in MIR and their symbolic names are lost. The authored fact that the first call applies `FEN::DISABLED` and the second `FEN::ENABLED` cannot be proven.
    - **send_break branch-to-value mapping**: MIR proves SwitchInt branching on the `on: bool` parameter and two modify calls both targeting `LCRH_REG::Register`. However, which branch maps to which FieldValue constant (`BRK::ENABLED` vs `BRK::DISABLED`) cannot be proven because the symbolic constant names are inlined in MIR.
  - Why This Status Follows: Each callback's structural body semantics (register module identity, call patterns, control flow shape) are proven by non-tautological MIR assertions. However, three specific authored facts (field-level identity within a register module, constant value ordering, and branch-to-constant mapping) are not provable from the current MIR representation because Field/FieldValue constants are inlined and lose their symbolic names. The correspondence is `weakened` rather than `exact` because these gaps exist.
- Counterexample:
  - No exercised MIR evidence distinguishes `FR_REG::TXFF::FIELD` from `FR_REG::BUSY::FIELD`, distinguishes `LCRH_REG::FEN::DISABLED` from `LCRH_REG::FEN::ENABLED` by symbolic identity, or maps the `send_break(on)` branches to `BRK::ENABLED` versus `BRK::DISABLED`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/mir_extract.rs` -- MIR extraction could be extended to preserve symbolic constant paths in Call arguments, enabling field-level and value-level identity proofs.
  - Minimal Missing Fact To Preserve: Symbolic path of FieldValue/Field constants passed as call arguments (e.g., `FR_REG::TXFF::FIELD`, `LCRH_REG::FEN::DISABLED`).
- Conclusion: fail -- serial callback body structure is strongly preserved, but exact field-level identity, constant value ordering, and branch-to-value mappings required by the authored predicates are not directly provable at the current extraction boundary.
