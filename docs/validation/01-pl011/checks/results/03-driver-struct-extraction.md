# Validation Certificate: PL011 Driver Struct Extraction Check

- Source premise: `docs/validation/01-pl011/checks/03-driver-struct-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/pl011_driver_struct_boundary.rs`

## Specimen Fidelity

- Specimen type: reduced inline specimen
- Specimen constant: `PL011_DRIVER_STRUCT_SPECIMEN` in test file
- Fidelity Assertion Predicate(s):
  - The fixture file `tests/fixtures/pl011-full-spec/src/driver.rs` is read at test
    time and parsed to extract the struct declaration block.
  - 14 fidelity assertions verify every authored fact named in the check contract
    against the real fixture source, including:
    - driver attribute (`name`, `compatible`, `bus`) exact string match
    - device_class attribute exact string match
    - struct name `Pl011FullSpec` presence
    - each field's `#[rseq::*]` attribute and type signature exact string match
    - absence of `#[rseq::*]` on `pending_tx` and `reset` (line-before check)
    - all 8 field names present in specimen
- Fidelity Evidence (Exact Citations):
  - Test function `specimen_fidelity` in `pl011_driver_struct_boundary.rs`
  - Assertions 1-14 each produce a test failure if the fixture source does not
    contain the exact authored fact string.

## Check-to-Surface Mapping

| Check Group | Extraction Surface(s) |
|---|---|
| 1 (driver identity) | `extract_driver_identities` -> `DriverIdentity` |
| 2 (device class) | `extract_driver_identities` -> `DriverIdentity.device_class` |
| 3 (resources) | `extract_identity_enrichment` -> `StructFact.fields[].rseq_kind`, `extract_register_access_surface` -> `mmio_fields`, `extract_interrupt_surface` -> `resources`, `extract_clock_surface` -> `fields`, `extract_reset_surface` -> `fields`, `extract_pinctrl_surface` -> `fields` |
| 4 (properties) | `extract_property_surface` -> `PropertyField`, `extract_identity_enrichment` -> `StructFact.fields[].rseq_kind` |
| 5 (user field) | `extract_identity_enrichment` -> `FieldFact.rseq_kind`, closed-world absence check across all 6 resource/property surfaces |

## Check Group 1: Driver Identity

- Check: driver identity preserves `name`, `compatible`, and `bus`
- Status: `pass`
- Premise: driver identity preserves `name`, `compatible`, and `bus`
- Assertion Predicate(s):
  - `identities.len() == 1`
  - `identities[0].driver_name == "pl011-full-spec"`
  - `identities[0].compatible == vec!["arm,pl011"]`
  - `identities[0].bus == Some("amba".to_string())`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_1_driver_identity` in `pl011_driver_struct_boundary.rs`
  - `extract_driver_identities(tcx)` returns exactly one `DriverIdentity`
  - Each of the three identity fields is asserted to equal its authored value
- Reasoning Trace:
  - Authored Fact (Verbatim): `name = "pl011-full-spec"`, `compatible = ["arm,pl011"]`, `bus = "amba"`
  - Extracted Fact (Verbatim): `driver_name = "pl011-full-spec"`, `compatible = ["arm,pl011"]`, `bus = Some("amba")`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all three identity fields match exactly
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: driver identity is fully preserved at the extraction boundary.

## Check Group 2: Device Class

- Check: device class remains `serial`
- Status: `pass`
- Premise: device class remains `serial`
- Assertion Predicate(s):
  - `identities[0].device_class == "serial"`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_2_device_class` in `pl011_driver_struct_boundary.rs`
  - The `device_class` field of the extracted `DriverIdentity` equals `"serial"`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::device_class(serial)]`
  - Extracted Fact (Verbatim): `device_class = "serial"`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the device class string matches the authored attribute
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: device class identity is preserved exactly.

## Check Group 3: Resources

- Check: each resource preserves field name, kind, Rust type, requiredness,
  resource properties, and join-key semantics
- Status: `pass`
- Premise: each resource preserves field name, kind, Rust type, requiredness,
  resource properties, and join-key semantics

### Resource: regs (mmio)

- Assertion Predicate(s):
  - `regs_field.name == "regs"`
  - `regs_field.canonical_path.0 == "driver::Pl011FullSpec::regs"`
  - `regs_field.rseq_kind == Some(RseqFieldKind::Mmio { id: "uart_regs" })`
  - `mmio_regs.mmio_id == Some("uart_regs")`
  - `mmio_regs.register_block_type == Some("Pl011Registers")`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_resources`: enrichment `FieldFact` for `regs` carries
    `RseqFieldKind::Mmio { id: "uart_regs" }`; `RegisterAccessSurface.mmio_fields`
    entry for `regs` carries `mmio_id = Some("uart_regs")` and
    `register_block_type = Some("Pl011Registers")`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::mmio(id = "uart_regs")] pub regs: Mmio<Pl011Registers>`, required, kind `mmio`
  - Extracted Fact (Verbatim): `RseqFieldKind::Mmio { id: "uart_regs" }`, `mmio_id = Some("uart_regs")`, `register_block_type = Some("Pl011Registers")`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: field name, kind, id, type, and required status all match

### Resource: irq (interrupt)

- Assertion Predicate(s):
  - `irq_field.name == "irq"`
  - `irq_field.rseq_kind == Some(RseqFieldKind::Interrupt { id: "uart_irq" })`
  - `irq_res.interrupt_id == "uart_irq"`
  - `irq_res.shape == InterruptFieldShape::Single` (required)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_resources`: enrichment classifies `irq` as
    `RseqFieldKind::Interrupt { id: "uart_irq" }`; `InterruptSurface.resources`
    confirms `interrupt_id = "uart_irq"` and `shape = Single`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::interrupt(id = "uart_irq")] pub irq: Interrupt`, required
  - Extracted Fact (Verbatim): `interrupt_id = "uart_irq"`, `shape = Single`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: kind, id, requiredness all match

### Resource: clock

- Assertion Predicate(s):
  - `clock_field.name == "clock"`
  - `clock_field.rseq_kind == Some(RseqFieldKind::Clock { id: "uart_clk" })`
  - `clock_res.clock_id == "uart_clk"`
  - `clock_res.required == false`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_resources`: enrichment and `ClockSurface` both confirm
    clock kind, id, and optional status
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::clock(id = "uart_clk")] pub clock: Option<Clock>`, optional
  - Extracted Fact (Verbatim): `clock_id = "uart_clk"`, `required = false`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: kind, id, optionality all match

### Resource: reset

- Assertion Predicate(s):
  - `reset_field.name == "reset"`
  - `reset_field.rseq_kind == None` (no rseq attribute; join key is Field name)
  - `reset_res.required == false`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_resources`: enrichment shows `rseq_kind = None` for
    `reset`; `ResetSurface.fields` confirms `field_name = "reset"` and
    `required = false`
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub reset: Option<ResetLine>`, optional, join key `Field name`
  - Extracted Fact (Verbatim): `rseq_kind = None`, `required = false`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: reset has no explicit rseq attribute (join key is field name, not Properties.id), and optionality is correctly extracted

### Resource: pinctrl

- Assertion Predicate(s):
  - `pinctrl_field.name == "pinctrl"`
  - `pinctrl_field.rseq_kind == Some(RseqFieldKind::PinCtrl { id: "uart_pins" })`
  - `pinctrl_res.pinctrl_id == Some("uart_pins")`
  - `pinctrl_res.required == false`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_resources`: enrichment and `PinCtrlSurface` both
    confirm kind, id, and optional status
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::pinctrl(id = "uart_pins")] pub pinctrl: Option<PinCtrl>`, optional
  - Extracted Fact (Verbatim): `pinctrl_id = Some("uart_pins")`, `required = false`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: kind, id, optionality all match

- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: all five resources preserve field name, kind, id/join-key, and
  requiredness at the extraction boundary.

## Check Group 4: Properties

- Check: each property preserves field name, type, id, default, and requiredness
- Status: `pass`
- Premise: each property preserves field name, type, id, default, and requiredness

### Property: current_speed

- Assertion Predicate(s):
  - `cs.field_name == "current_speed"`
  - `cs.property_id == "current-speed"`
  - `cs.field_type == "u32"`
  - `cs.default_literal == Some("115200")`
  - `cs.required == true`
  - `cs_field.rseq_kind == Some(RseqFieldKind::Property { id: "current-speed" })`
  - `cs_field.canonical_path.0 == "driver::Pl011FullSpec::current_speed"`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_4_properties`: `PropertySurface.fields` entry for
    `current_speed` confirms id, type, default, and requiredness;
    `IdentityEnrichmentSurface` cross-check confirms `RseqFieldKind::Property`
    classification and canonical path
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::property(id = "current-speed", default = 115200)] pub current_speed: u32`, required
  - Extracted Fact (Verbatim): `property_id = "current-speed"`, `field_type = "u32"`, `default_literal = Some("115200")`, `required = true`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all five property facets match

### Property: fifo_size

- Assertion Predicate(s):
  - `fs.field_name == "fifo_size"`
  - `fs.property_id == "fifo-size"`
  - `fs.field_type == "u32"`
  - `fs.default_literal == Some("32")`
  - `fs.required == true`
  - `fs_field.rseq_kind == Some(RseqFieldKind::Property { id: "fifo-size" })`
  - `fs_field.canonical_path.0 == "driver::Pl011FullSpec::fifo_size"`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_4_properties`: same structure as `current_speed` above
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::property(id = "fifo-size", default = 32)] pub fifo_size: u32`, required
  - Extracted Fact (Verbatim): `property_id = "fifo-size"`, `field_type = "u32"`, `default_literal = Some("32")`, `required = true`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all five property facets match

- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: both properties preserve all authored facets at the extraction boundary.

## Check Group 5: User Field (pending_tx)

- Check: `pending_tx` remains an ordinary user field and is not reclassified as a
  Regseq field
- Status: `pass`
- Premise: `pending_tx` remains an ordinary user field and is not reclassified as
  a Regseq resource or property
- Assertion Predicate(s):
  - `pending_tx.rseq_kind == None` (no rseq classification in enrichment)
  - `pending_tx.canonical_path.0 == "driver::Pl011FullSpec::pending_tx"`
  - `pending_tx` does not appear in `RegisterAccessSurface.mmio_fields`
  - `pending_tx` does not appear in `InterruptSurface.resources`
  - `pending_tx` does not appear in `ClockSurface.fields`
  - `pending_tx` does not appear in `ResetSurface.fields`
  - `pending_tx` does not appear in `PinCtrlSurface.fields`
  - `pending_tx` does not appear in `PropertySurface.fields`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_5_user_field_not_reclassified`: the enrichment surface
    confirms `rseq_kind = None`, and six closed-world absence assertions
    verify that `pending_tx` does not appear in any resource or property
    extraction surface
- Reasoning Trace:
  - Authored Fact (Verbatim): `pending_tx: Option<u8>`, ordinary user state, no `#[rseq::*]` attribute
  - Extracted Fact (Verbatim): `rseq_kind = None`, absent from all 6 resource/property surfaces
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the field carries no rseq classification and is
    absent from every resource/property extraction surface (closed-world evidence)
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: `pending_tx` is correctly preserved as an ordinary user field with
  no invented resource or property metadata.

## Summary

All 5 check groups pass. The PL011 driver-struct declaration facts (driver
identity, device class, resources, properties, and user field classification)
are fully preserved at the compiler-side extraction boundary.
