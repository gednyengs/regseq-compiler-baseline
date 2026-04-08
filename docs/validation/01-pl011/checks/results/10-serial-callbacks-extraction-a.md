# Validation Certificate: PL011 Serial Callbacks Extraction Check A

- Source premise: `docs/validation/01-pl011/checks/10-serial-callbacks-extraction-a.md`
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
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1093)

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
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1288)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1304)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1336)
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
  - **tx_byte fast path**: field_val and write calls present (DR write); structurally proven via `reachable_from` that the fast-path SwitchInt branch reaches field_val/write but NOT enable_tx_interrupts
  - **tx_byte staged path**: Option::Some aggregate (variant_index: 1) assigned to pending_tx; enable_tx_interrupts call present; structurally proven via `reachable_from` that the staged-path SwitchInt branch reaches enable_tx_interrupts but NOT write
  - **tx_byte byte parameter flow**: `call_arg_receives_cast_of_local(body, "field_val", 2)` returns true -- byte (local 2) flows into field_val via `byte as u32` cast
  - **tx_byte return**: return_type is None (unit)
  - **rx_char RXFE branch**: SwitchInt present; `has_call_to_exact(body, "read")` verifies FR.RXFE read using exact suffix match `::read` to avoid matching `read_raw`
  - **rx_char single raw DR read**: `count_calls_to(body, "read_raw") == 1`
  - **rx_char field extraction**: `count_calls_to(body, "field_from_raw") >= 5` (DATA+OE+BE+PE+FE)
  - **rx_char error init**: empty() call present
  - **rx_char error accumulation**: `count_calls_to(body, "bitor") >= 4` (OE+BE+PE+FE bitwise-or)
  - **rx_char exact error mappings**: `extract_error_flag_mappings(body)` returns exactly 4 pairs:
    - `OE -> OVERRUN`: field_from_raw arg contains "DR_REG::OE::FIELD", bitor arg path contains "ErrorKind::OVERRUN"
    - `BE -> BREAK`: field_from_raw arg contains "DR_REG::BE::FIELD", bitor arg path contains "ErrorKind::BREAK"
    - `PE -> PARITY`: field_from_raw arg contains "DR_REG::PE::FIELD", bitor arg path contains "ErrorKind::PARITY"
    - `FE -> FRAMING`: field_from_raw arg contains "DR_REG::FE::FIELD", bitor arg path contains "ErrorKind::FRAMING"
  - **rx_char error SwitchInt count**: at least 5 SwitchInt blocks (1 RXFE + 4 error-flag != 0 checks)
  - **rx_char RxChar construction**: new() call present
  - **rx_char Option::Some return**: Aggregate(Adt(Option, variant_index: 1)) present in non-cleanup blocks
  - **rx_char Option::None return**: Aggregate(Adt(Option, variant_index: 0)) present for empty-FIFO path
  - **rx_char Return**: Return terminator present
- Target Evidence (Exercised by Test; Exact Citations):
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1459)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1532)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1579)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1656)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1800)
  - [source_to_extraction_10_serial_callbacks_a.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs#L1911)
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs` lines 288-343
- Reasoning Trace:
  - Authored Fact (Verbatim): port_open calls `enable_clock_if_present()?` then `program_polling_port(self.uart_clock_hz(), self.current_speed)` then `Ok(())`; port_close calls `shutdown_port()` then `disable_clock_if_present()`; configure preserves `_config: &SerialConfig` opaque, same body as port_open; tx_byte branches on `self.pending_tx.is_none() && self.tx_ready()`, fast path writes `DR.DATA(byte as u32)` and returns, staged path assigns `self.pending_tx = Some(byte)` then calls `enable_tx_interrupts()`; rx_char tests `FR.RXFE`, returns None if set, otherwise reads raw DR once, extracts byte via `field_from_raw(DATA, dr) as u8`, accumulates OE/BE/PE/FE to OVERRUN/BREAK/PARITY/FRAMING via bitwise-or from `ErrorKind::empty()`, returns `Some(RxChar::new(byte, errors))`
  - Extracted Fact (Verbatim): MIR body preserves call sequences with ordering verified via block-order Call positions; `?` desugars to Call(callee) -> Call(Try::branch) -> Discriminant+SwitchInt -> Call(from_residual); _config local (index 2) is never read in non-cleanup blocks; SwitchInt represents branch conditions; field_val/write/read_raw/field_from_raw/empty/bitor/new calls all present with correct counts; Option::Some (variant_index=1) and Option::None (variant_index=0) aggregates both present; Result Ok (variant_index=0) aggregates present; ErrorKind constants preserved as `Unevaluated` operands with symbolic def-paths (e.g. `regseq::ErrorKind::OVERRUN`), and the CFG chain from field_from_raw(DR_REG::XX::FIELD) through SwitchInt to bitor(errors, ErrorKind::YY) proves all four exact pairings
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: `port_open`, `port_close`, `configure`, and `tx_byte` are strongly covered by non-tautological MIR assertions for helper ordering, `?` propagation, parameter opacity, branch separation, and return construction. `rx_char` is now fully covered: the extraction preserves ErrorKind constants as `Unevaluated` operands with their authored def-paths, and the test follows the MIR CFG chain (field_from_raw -> SwitchInt -> bitor) to prove each individual error-flag-to-ErrorKind pairing structurally.
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- all serial callback body predicates are preserved through extraction, including exact `rx_char` error-kind mapping proved via CFG-following of symbolic Unevaluated constant paths
