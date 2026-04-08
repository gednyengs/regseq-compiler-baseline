# Validation Certificate: PL011 Serial Callbacks Extraction Check A

Source premise: `docs/validation/01-pl011/01-authored-source.md` (Serial Callbacks section)
Target boundary evidence: `crates/regseq-driver/tests/serial_callbacks_extraction_a.rs`

## Specimen Fidelity

- **Approach**: Reduced inline specimen containing a `regseq` module with
  `Serial` trait, `RegseqError`, `SerialConfig`, `ErrorKind`, `RxChar`,
  `Clock`, register types, and the five serial callbacks (port_open,
  port_close, configure, tx_byte, rx_char) with their reachable helpers.
- **Fidelity Assertion Predicate(s)**:
  - `fixture.contains("fn port_open(&mut self) -> Result<(), RegseqError>")`
  - `fixture.contains("self.enable_clock_if_present()?;")`
  - `fixture.contains("self.program_polling_port(self.uart_clock_hz(), self.current_speed)")`
  - `fixture.contains("fn port_close(&mut self)")`
  - `fixture.contains("self.shutdown_port();")`
  - `fixture.contains("self.disable_clock_if_present();")`
  - `fixture.contains("fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>")`
  - `fixture.contains("fn tx_byte(&mut self, byte: u8)")`
  - `fixture.contains("if self.pending_tx.is_none() && self.tx_ready()")`
  - `fixture.contains("self.pending_tx = Some(byte)")`
  - `fixture.contains("self.enable_tx_interrupts()")`
  - `fixture.contains("fn rx_char(&mut self) -> Option<RxChar>")`
  - `fixture.contains("self.regs.fr.read(FR_REG::RXFE::FIELD) != 0")`
  - `fixture.contains("let dr = self.regs.dr.read_raw()")`
  - `fixture.contains("ErrorKind::OVERRUN")`
  - `fixture.contains("ErrorKind::BREAK")`
  - `fixture.contains("ErrorKind::PARITY")`
  - `fixture.contains("ErrorKind::FRAMING")`
  - `fixture.contains("Some(RxChar::new(byte, errors))")`
  - Corresponding specimen assertions verify the same substrings exist in the specimen.
- **Fidelity Evidence (Exact Citations)**: Test `precondition_specimen_fidelity` in
  `crates/regseq-driver/tests/serial_callbacks_extraction_a.rs`, lines exercising
  both fixture and specimen substring matches.

## Check-to-Surface Mapping

| Check | Extraction Surface | Inspector |
|---|---|---|
| Trait role | `TraitSurface.serial_impls` | `check_group_1_serial_trait_role_recognized` |
| Callback identity in impl block | `IdentityEnrichmentSurface.impl_blocks` | `check_group_1_serial_impl_in_identity_enrichment` |
| Signatures and return types | `MirBodySurface.functions` (MirExtractedFn fields) | `check_group_1_callback_signatures_and_return_types` |
| port_open body | `MirBodySurface.functions` (MirExtractedFn.body) | `check_group_2_port_open_body` |
| port_close body | `MirBodySurface.functions` (MirExtractedFn.body) | `check_group_2_port_close_body` |
| configure body | `MirBodySurface.functions` (MirExtractedFn.body) | `check_group_2_configure_body` |
| tx_byte body | `MirBodySurface.functions` (MirExtractedFn.body) | `check_group_2_tx_byte_body` |
| rx_char body | `MirBodySurface.functions` (MirExtractedFn.body) | `check_group_2_rx_char_body` |

---

## Check Group 1

- Check: Serial callback identity, trait role, signature, and return type survive
- Status: `pass`
- Premise: serial callback identity, trait role, signature, and return type survive
- Assertion Predicate(s):
  - `ext.trait_surface.serial_impls.len() == 1`
  - `ext.trait_surface.serial_impls[0].driver_type == "Pl011FullSpec"`
  - Serial impl block in `identity.impl_blocks` has `trait_name == Some("Serial")`
    and `self_type_name == "Pl011FullSpec"`, with methods containing all five
    callback names
  - For each callback: `kind` matches `MirFnKind::Callback { trait_name: "Serial", method_name }`,
    `receiver == "ref_mut"`, params and return_type match authored signatures
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_serial_trait_role_recognized`: asserts `serial_impls.len() == 1`
    and `driver_type == "Pl011FullSpec"`
  - `check_group_1_serial_impl_in_identity_enrichment`: asserts trait impl block
    presence, self type, and method membership for all five callbacks
  - `check_group_1_callback_signatures_and_return_types`: per-callback assertions on
    receiver, params, return_type, and MirFnKind
- Supporting Evidence: n/a
- Reasoning Trace:
  - Authored Fact (Verbatim): `impl Serial for Pl011FullSpec` with `port_open(&mut self) -> Result<(), RegseqError>`, `port_close(&mut self)`, `configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError>`, `tx_byte(&mut self, byte: u8)`, `rx_char(&mut self) -> Option<RxChar>`
  - Extracted Fact (Verbatim): TraitSurface contains 1 serial_impl for Pl011FullSpec; IdentityEnrichmentSurface contains Serial impl block with all five methods; MirBodySurface contains each callback with matching kind, receiver, params, return_type
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: All four named properties (identity, trait role, signature, return type) are exercised by non-tautological assertions against the three extraction surfaces
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

---

## Check Group 2

- Check: Serial callback bodies preserve the downstream checks captured in the authored-source entries
- Status: `pass`
- Premise: serial callback bodies preserve the downstream checks captured in the authored-source entries
- Assertion Predicate(s):

  **port_open**:
  - `f.helper_calls.contains("enable_clock_if_present")`
  - `f.helper_calls.contains("program_polling_port")`
  - `has_call_to(&f.body, "enable_clock_if_present")`
  - `has_call_to(&f.body, "program_polling_port")`
  - `has_call_to(&f.body, "uart_clock_hz")`
  - Call ordering: `clock_idx < poll_idx` in `collect_call_names`
  - `has_return(&f.body)`

  **port_close**:
  - `f.helper_calls.contains("shutdown_port")`
  - `f.helper_calls.contains("disable_clock_if_present")`
  - `has_call_to(&f.body, "shutdown_port")`
  - `has_call_to(&f.body, "disable_clock_if_present")`
  - Call ordering: `shutdown_idx < disable_idx` in `collect_call_names`
  - `f.return_type.is_none()` (unit return)

  **configure**:
  - `f.params.len() == 1` with `name == "_config"` and `ty.contains("SerialConfig")`
  - `f.helper_calls.contains("enable_clock_if_present")`
  - `f.helper_calls.contains("program_polling_port")`
  - Call ordering: `clock_idx < poll_idx`
  - `f.return_type contains "Result"`
  - `has_return(&f.body)`

  **tx_byte**:
  - `has_switch_int(&f.body)` (branch condition)
  - `has_call_to(&f.body, "is_none")` (pending_tx.is_none())
  - `has_call_to(&f.body, "tx_ready")`
  - `has_call_to(&f.body, "field_val")` (DR write value)
  - `has_call_to(&f.body, "write")` (register write)
  - `f.helper_calls.contains("enable_tx_interrupts")`
  - `has_call_to(&f.body, "enable_tx_interrupts")`
  - Option::Some aggregate with `variant_index == 1` present (pending_tx = Some(byte))
  - `f.return_type.is_none()` (unit return)

  **rx_char**:
  - `f.return_type contains "Option"`
  - `has_switch_int(&f.body)` (RXFE branch)
  - `has_call_to(&f.body, "read")` (FR.RXFE)
  - `has_call_to(&f.body, "read_raw")` (DR read)
  - `count_calls_to(&f.body, "field_from_raw") >= 5` (DATA + OE + BE + PE + FE)
  - `has_call_to(&f.body, "empty")` (ErrorKind::empty)
  - `has_call_to(&f.body, "bitor")` (error accumulation)
  - `has_call_to(&f.body, "new")` (RxChar::new)
  - Option::Some aggregate with `variant_index == 1` present
  - Option::None aggregate with `variant_index == 0` present
  - `has_return(&f.body)`

- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_port_open_body`: helper call presence, MIR call presence, call ordering, return
  - `check_group_2_port_close_body`: helper call presence, MIR call presence, call ordering, unit return
  - `check_group_2_configure_body`: opaque param, helper calls, ordering, Result return
  - `check_group_2_tx_byte_body`: SwitchInt, is_none, tx_ready, field_val, write, enable_tx_interrupts, Option::Some aggregate, unit return
  - `check_group_2_rx_char_body`: Option return, SwitchInt, read, read_raw, 5x field_from_raw, empty, bitor, new, Option::Some, Option::None, Return
- Supporting Evidence: n/a
- Reasoning Trace:
  - Authored Fact (Verbatim): per-callback helper sequencing, branching, register interactions, opaque parameter preservation, option/result behavior as documented in `01-authored-source.md` Serial Callbacks section
  - Extracted Fact (Verbatim): MIR body surfaces preserve helper call names, call ordering, branch structure (SwitchInt), register operation calls (read/write/read_raw/field_val/field_from_raw), error accumulation (bitor), Option construction (variant 0 = None, variant 1 = Some), and return terminators
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Every downstream check from the authored-source entries is exercised by a non-tautological assertion over the extracted MIR body surface. Helper sequencing is proven by call-order indexing. Branching is proven by SwitchInt presence. Register interactions are proven by call presence to read/write/read_raw/field_val/field_from_raw. Opaque parameter preservation is proven by param name and type assertions. Option/Result return behavior is proven by aggregate variant construction and Return terminator assertions.
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass
