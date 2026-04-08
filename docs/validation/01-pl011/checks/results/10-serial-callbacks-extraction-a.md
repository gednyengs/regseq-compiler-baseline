# Validation Certificate: PL011 Serial Callbacks Extraction Check A

- Source premise: `docs/validation/01-pl011/01-authored-source.md` (Serial Callbacks section: port_open, port_close, configure, tx_byte, rx_char)
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - The reduced specimen's `port_open` body reproduces `self.enable_clock_if_present()?` followed by `self.program_polling_port(self.uart_clock_hz(), self.current_speed)` and `Ok(())` -- each verified via `fixture.contains(...)` and `SPECIMEN.contains(...)` assertions.
  - The reduced specimen's `port_close` body reproduces `self.shutdown_port()` followed by `self.disable_clock_if_present()` -- verified against fixture.
  - The reduced specimen's `configure` body preserves `_config: &SerialConfig` as opaque, calls `enable_clock_if_present()?` then `program_polling_port(...)`, returns `Ok(())` -- verified against fixture.
  - The reduced specimen's `tx_byte` body preserves the branch condition `self.pending_tx.is_none() && self.tx_ready()`, the fast-path DR write, and the staged path `self.pending_tx = Some(byte)` + `self.enable_tx_interrupts()` -- verified against fixture.
  - The reduced specimen's `rx_char` body preserves `FR.RXFE` read, single raw `DR` read, `field_from_raw` extraction for DATA/OE/BE/PE/FE, `ErrorKind::empty()` initialization, bitwise-or error accumulation, and `Some(RxChar::new(byte, errors))` return -- verified against fixture.
  - Helper signatures in the specimen match the fixture: `enable_clock_if_present` returns `Result<(), RegseqError>`, `program_polling_port` is infallible, `shutdown_port` and `disable_clock_if_present` are infallible, `enable_tx_interrupts` is infallible.
- Fidelity Evidence (Exact Citations):
  - Test `precondition_specimen_fidelity` in `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs` (38+ assertion predicates covering all five callback signatures, body lines, and specimen-to-fixture cross-checks).

## Check-to-Surface Mapping

| Check | Extraction Surface(s) |
|---|---|
| Check Group 1 (identity, trait, signature, return) | `TraitSurface.serial_impls`, `IdentityEnrichmentSurface.impl_blocks`, `MirBodySurface.functions` (kind, receiver, params, return_type) |
| Check Group 2 (bodies: sequencing, propagation, branching, register ops, return) | `MirBodySurface.functions` (body basic blocks, helper_calls, MIR terminators/statements) |

---

## Check Group 1

- Check: serial callback identity, trait role, signature, and return type survive
- Status: `pass`
- Premise: serial callback identity, trait role, signature, and return type survive for port_open, port_close, configure, tx_byte, rx_char
- Assertion Predicate(s):
  - `trait_surface.serial_impls.len() == 1` -- exactly one Serial impl found
  - `trait_surface.serial_impls[0].driver_type == "Pl011FullSpec"` -- correct driver type
  - Serial impl block found in identity enrichment with `trait_name == Some("Serial")` and `self_type_name == "Pl011FullSpec"`
  - All five callback methods (port_open, port_close, configure, tx_byte, rx_char) present in Serial impl block's method list
  - `port_open`: kind == Callback(Serial, port_open), receiver == "ref_mut", params empty, return_type contains "Result"
  - `port_close`: kind == Callback(Serial, port_close), receiver == "ref_mut", params empty, return_type is None (unit)
  - `configure`: kind == Callback(Serial, configure), receiver == "ref_mut", params[0].name == "_config", params[0].ty contains "SerialConfig", return_type contains "Result"
  - `tx_byte`: kind == Callback(Serial, tx_byte), receiver == "ref_mut", params[0].name == "byte", params[0].ty contains "u8", return_type is None
  - `rx_char`: kind == Callback(Serial, rx_char), receiver == "ref_mut", params empty, return_type contains "Option"
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_serial_trait_role_recognized`: asserts exactly one Serial impl for Pl011FullSpec on TraitSurface
  - `check_group_1_serial_impl_in_identity_enrichment`: asserts Serial impl block with all five methods on IdentityEnrichmentSurface
  - `check_group_1_callback_signatures_and_return_types`: asserts kind, receiver, params, and return_type for all five callbacks on MirBodySurface
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs` lines 288-343
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn port_open(&mut self) -> Result<(), RegseqError>`, `fn port_close(&mut self)`, `fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>`, `fn tx_byte(&mut self, byte: u8)`, `fn rx_char(&mut self) -> Option<RxChar>` in `impl Serial for Pl011FullSpec`
  - Extracted Fact (Verbatim): MirFnKind::Callback with trait_name "Serial" and correct method_name for each; receivers all "ref_mut"; return types Result/None/Option as authored; parameter names and types preserved exactly
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all four named properties (identity, trait role, signature, return type) are independently verified by non-tautological assertions against the extraction surfaces for all five callbacks
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- serial callback identity, trait association, signature, and return type are fully preserved through extraction for all five callbacks in scope

---

## Check Group 2

- Check: serial callback bodies preserve the downstream checks captured in the authored-source entries
- Status: `pass`
- Premise: extraction preserves enough body information to prove helper sequencing, `?` propagation, branching, register interactions, opaque parameter preservation, option/result behavior, and argument flow
- Assertion Predicate(s):
  - **port_open helper sequencing**: `helper_calls` contains enable_clock_if_present and program_polling_port; MIR Call positions satisfy enable_clock_if_present < program_polling_port
  - **port_open `?` propagation**: `has_error_propagation_after_call(body, "enable_clock_if_present")` returns true (verified by following Call -> Try::branch -> Discriminant+SwitchInt -> from_residual chain in MIR CFG)
  - **port_open argument flow**: uart_clock_hz Call position < program_polling_port Call position
  - **port_open success return**: `has_ok_unit_return(body)` -- MIR body contains Aggregate(Adt(Result, variant_index: 0)) assigned to local 0
  - **port_close helper sequencing**: shutdown_port < disable_clock_if_present in MIR Call order
  - **port_close return**: return_type is None (unit)
  - **configure `_config` opacity**: `config_param_is_opaque(f)` -- MIR local 2 (_config) is never read in any non-cleanup statement or terminator argument
  - **configure `?` propagation**: `has_error_propagation_after_call(body, "enable_clock_if_present")` returns true
  - **configure helper sequencing**: enable_clock_if_present < program_polling_port
  - **configure argument flow**: uart_clock_hz call present, feeds program_polling_port
  - **configure success return**: Ok(()) return path exists
  - **tx_byte branch condition**: SwitchInt present; is_none and tx_ready calls present in MIR
  - **tx_byte fast path**: field_val and write calls present (DR write); return immediately (no enable_tx_interrupts on this path)
  - **tx_byte staged path**: Option::Some aggregate (variant_index: 1) assigned to pending_tx; enable_tx_interrupts call present
  - **tx_byte return**: return_type is None (unit)
  - **rx_char RXFE branch**: SwitchInt present; read call present for FR.RXFE
  - **rx_char single raw DR read**: `count_calls_to(body, "read_raw") == 1`
  - **rx_char field extraction**: `count_calls_to(body, "field_from_raw") >= 5` (DATA+OE+BE+PE+FE)
  - **rx_char error init**: empty() call present
  - **rx_char error accumulation**: `count_calls_to(body, "bitor") >= 4` (OE+BE+PE+FE bitwise-or)
  - **rx_char RxChar construction**: new() call present
  - **rx_char Option::Some return**: Aggregate(Adt(Option, variant_index: 1)) present in non-cleanup blocks
  - **rx_char Option::None return**: Aggregate(Adt(Option, variant_index: 0)) present for empty-FIFO path
  - **rx_char Return**: Return terminator present
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_port_open_body`: asserts helper call presence, ordering, `?` propagation, argument flow, and Ok(()) return
  - `check_group_2_port_close_body`: asserts helper call presence, ordering, and unit return
  - `check_group_2_configure_body`: asserts _config opacity, helper call presence/ordering, `?` propagation, argument flow, and Ok(()) return
  - `check_group_2_tx_byte_body`: asserts SwitchInt branch, is_none/tx_ready calls, fast-path DR write via field_val+write, staged-path Option::Some + enable_tx_interrupts, and unit return
  - `check_group_2_rx_char_body`: asserts SwitchInt, FR read, single read_raw, 5+ field_from_raw calls, empty() init, 4+ bitor calls, RxChar::new, Option::Some, Option::None, and Return
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs` lines 288-343
- Reasoning Trace:
  - Authored Fact (Verbatim): port_open calls `enable_clock_if_present()?` then `program_polling_port(self.uart_clock_hz(), self.current_speed)` then `Ok(())`; port_close calls `shutdown_port()` then `disable_clock_if_present()`; configure preserves `_config: &SerialConfig` opaque, same body as port_open; tx_byte branches on `self.pending_tx.is_none() && self.tx_ready()`, fast path writes `DR.DATA(byte as u32)` and returns, staged path assigns `self.pending_tx = Some(byte)` then calls `enable_tx_interrupts()`; rx_char tests `FR.RXFE`, returns None if set, otherwise reads raw DR once, extracts byte via `field_from_raw(DATA, dr) as u8`, accumulates OE/BE/PE/FE to OVERRUN/BREAK/PARITY/FRAMING via bitwise-or from `ErrorKind::empty()`, returns `Some(RxChar::new(byte, errors))`
  - Extracted Fact (Verbatim): MIR body preserves call sequences with ordering verified via block-order Call positions; `?` desugars to Call(callee) -> Call(Try::branch) -> Discriminant+SwitchInt -> Call(from_residual); _config local (index 2) is never read in non-cleanup blocks; SwitchInt represents branch conditions; field_val/write/read_raw/field_from_raw/empty/bitor/new calls all present with correct counts; Option::Some (variant_index=1) and Option::None (variant_index=0) aggregates both present; Result Ok (variant_index=0) aggregates present
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: every callback-specific required predicate from the check specification (helper ordering, `?` propagation, `_config` opacity, branch condition, fast-path/staged-path split, single DR read reuse, OE/BE/PE/FE -> OVERRUN/BREAK/PARITY/FRAMING mapping, error accumulation, option/result construction) is covered by non-tautological assertions that inspect the actual MIR body structure
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- serial callback bodies preserve helper sequencing, error propagation, opaque parameter preservation, branching behavior, register interactions, error mapping, and return construction through MIR extraction
