# Validation Certificate: PL011 Driver Struct Extraction Check

Source premise: `docs/validation/01-pl011/checks/03-driver-struct-extraction.md`
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs`

## Specimen Fidelity

Type: Inline reduced specimen
Test file: `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs`

Fidelity Assertion Predicate(s):
- `precondition_fixture_source_contains_authored_driver_struct`: asserts the fixture source at `tests/fixtures/pl011-full-spec/src/driver.rs` contains every authored fact fragment (driver attribute, device class attribute, struct declaration, all resource/property/user field declarations with their rseq attributes).
- `precondition_specimen_fidelity_against_fixture`: asserts every authored fact fragment appears in both the specimen and the fixture source.

Fidelity Evidence (Exact Citations):
- 17 authored fact fragments verified against both specimen and fixture source via `assert!(source.contains(...))` and `assert!(SPECIMEN.contains(...))`.
- Fragments include: driver attribute, device class attribute, struct declaration, mmio/interrupt/clock/reset/pinctrl attributes and field declarations, property attributes and field declarations, and pending_tx declaration.

## Check-to-Surface Mapping

| Check Group | Extraction Surface(s) |
|---|---|
| 1: driver identity | `extract_driver_identities` -> `DriverIdentity` |
| 2: device class | `extract_driver_identities` -> `DriverIdentity.device_class` |
| 3: resources | `extract_register_access_surface` -> `mmio_fields`, `extract_interrupt_surface` -> `resources`, `extract_clock_surface` -> `fields`, `extract_reset_surface` -> `fields`, `extract_pinctrl_surface` -> `fields`, `extract_identity_enrichment` -> `structs[].fields[].rseq_kind` |
| 4: properties | `extract_property_surface` -> `fields`, `extract_identity_enrichment` -> `structs[].fields[].rseq_kind` |
| 5: user field | `extract_identity_enrichment` -> `structs[].fields[].rseq_kind`, closed-world absence across all resource/property surfaces |

---

## Check Group 1: Driver identity preserves name, compatible, and bus

- Check: driver identity preserves `name`, `compatible`, and `bus`
- Status: `pass`
- Premise: driver identity preserves `name = "pl011-full-spec"`, `compatible = ["arm,pl011"]`, `bus = "amba"`
- Assertion Predicate(s):
  - `assert_eq!(id.driver_name, "pl011-full-spec")`
  - `assert_eq!(id.compatible, vec!["arm,pl011".to_string()])`
  - `assert_eq!(id.bus.as_deref(), Some("amba"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_1_driver_identity_preserves_name_compatible_bus` in `source_to_extraction_03_driver_struct.rs` extracts `DriverIdentity` via `extract_driver_identities(tcx)` and asserts all three fields match authored values.
- Reasoning Trace:
  - Authored Fact (Verbatim): `name = "pl011-full-spec"`, `compatible = ["arm,pl011"]`, `bus = "amba"`
  - Extracted Fact (Verbatim): `DriverIdentity { driver_name: "pl011-full-spec", compatible: ["arm,pl011"], bus: Some("amba"), .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all three identity fields are preserved exactly in the extraction surface
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

## Check Group 2: Device class remains serial

- Check: device class remains `serial`
- Status: `pass`
- Premise: device class is `serial`
- Assertion Predicate(s):
  - `assert_eq!(id.device_class, "serial")`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_2_device_class_remains_serial` in `source_to_extraction_03_driver_struct.rs` extracts `DriverIdentity` and asserts `device_class == "serial"`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::device_class(serial)]`
  - Extracted Fact (Verbatim): `DriverIdentity { device_class: "serial", .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: device class identity is preserved exactly
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

## Check Group 3: Resource field preservation

### 3a: regs (MMIO resource)

- Check: `regs` resource preserves field name, kind, Rust type, requiredness, resource properties, join-key semantics
- Status: `pass`
- Premise: `regs`: field name `regs`, kind `mmio`, type `Mmio<Pl011Registers>`, required, `id = "uart_regs"`, join key `Properties.id`
- Assertion Predicate(s):
  - `assert_eq!(mmio.field_name, "regs")`
  - `assert_eq!(mmio.mmio_id.as_deref(), Some("uart_regs"))`
  - `assert_eq!(mmio.register_block_type.as_deref(), Some("Pl011Registers"))`
  - `assert_eq!(AUTHORED_REGS_TYPE, format!("Mmio<{}>", mmio.register_block_type...))` (Rust type reconstruction)
  - `assert!(!AUTHORED_REGS_TYPE.starts_with("Option<"))` (requiredness via authored type)
  - `assert!(matches!(regs_field.rseq_kind, Some(RseqFieldKind::Mmio { id }) if id == "uart_regs"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_regs_mmio_resource` asserts field name, mmio_id, register_block_type, Rust type reconstruction from `Mmio<register_block_type>`, and requiredness (non-Optional type) via `extract_register_access_surface`, and rseq_kind via `extract_identity_enrichment`.
- Supporting Evidence:
  - `MmioFieldDecl` captures the inner generic type as `register_block_type`, which together with the `Mmio<>` wrapper reconstructs the full Rust type `Mmio<Pl011Registers>`. Requiredness is proven via the authored type constant not being `Option<>` wrapped. The `MmioFieldDecl` surface does not have an explicit `required` field.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub regs: Mmio<Pl011Registers>`, `#[rseq::mmio(id = "uart_regs")]`
  - Extracted Fact (Verbatim): `MmioFieldDecl { field_name: "regs", mmio_id: Some("uart_regs"), register_block_type: Some("Pl011Registers") }`, `RseqFieldKind::Mmio { id: "uart_regs" }`
  - Correspondence: `exact`
  - Gap / Loss Description: full Rust type not stored as a single string, but exactly reconstructable from `Mmio<register_block_type>`
  - Why This Status Follows: all authored resource facts are preserved or exactly reconstructable
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 3b: irq (interrupt resource)

- Check: `irq` resource preserves field name, kind, Rust type, requiredness, resource properties, join-key semantics
- Status: `pass`
- Premise: `irq`: field name `irq`, kind `interrupt`, type `Interrupt`, required, `id = "uart_irq"`, join key `Properties.id`
- Assertion Predicate(s):
  - `assert_eq!(irq.field_name, "irq")`
  - `assert_eq!(irq.interrupt_id, "uart_irq")`
  - `assert_eq!(irq.shape, InterruptFieldShape::Single)`
  - `assert_eq!(irq.field_type, "Interrupt")` (exact Rust type fidelity)
  - `assert!(matches!(irq_field.rseq_kind, Some(RseqFieldKind::Interrupt { id }) if id == "uart_irq"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_irq_interrupt_resource` in `source_to_extraction_03_driver_struct.rs` asserts field name, interrupt_id, shape, exact Rust type via `field_type`, and interrupt-field classification.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub irq: Interrupt`, `#[rseq::interrupt(id = "uart_irq")]`
  - Extracted Fact (Verbatim): `InterruptResourceDecl { field_name: "irq", interrupt_id: "uart_irq", shape: Single, field_type: "Interrupt", .. }`, `RseqFieldKind::Interrupt { id: "uart_irq" }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all authored resource facts including exact Rust type are preserved explicitly in the extraction surface via `field_type`
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 3c: clock resource

- Check: `clock` resource preserves field name, kind, Rust type, requiredness, resource properties, join-key semantics
- Status: `pass`
- Premise: `clock`: field name `clock`, kind `clock`, type `Option<Clock>`, optional, `id = "uart_clk"`, join key `Properties.id`
- Assertion Predicate(s):
  - `assert_eq!(clk.field_name, "clock")`
  - `assert_eq!(clk.clock_id, "uart_clk")`
  - `assert!(!clk.required)`
  - `assert_eq!(clk.field_type, "Option<Clock>")` (exact Rust type fidelity)
  - `assert!(matches!(clock_field.rseq_kind, Some(RseqFieldKind::Clock { id }) if id == "uart_clk"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_clock_resource` in `source_to_extraction_03_driver_struct.rs` asserts field name, clock id, requiredness, exact Rust type via `field_type`, and clock-field classification.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub clock: Option<Clock>`, `#[rseq::clock(id = "uart_clk")]`
  - Extracted Fact (Verbatim): `ClockField { field_name: "clock", clock_id: "uart_clk", required: false, field_type: "Option<Clock>" }`, `RseqFieldKind::Clock { id: "uart_clk" }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all authored resource facts including exact Rust type are preserved explicitly in the extraction surface via `field_type`
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 3d: reset resource

- Check: `reset` resource preserves field name, kind, Rust type, requiredness, join-key semantics
- Status: `pass`
- Premise: `reset`: field name `reset`, kind `reset`, type `Option<ResetLine>`, optional, join key `Field name`
- Assertion Predicate(s):
  - `assert_eq!(rst.field_name, "reset")`
  - `assert!(!rst.required)`
  - `assert_eq!(rst.field_type, "Option<ResetLine>")` (exact Rust type fidelity)
  - `assert!(reset_field.rseq_kind.is_none())` (no rseq attribute; discovered by type)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_reset_resource` in `source_to_extraction_03_driver_struct.rs` asserts field name, requiredness, exact Rust type via `field_type`, and absence of any `#[rseq::*]` field classification.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub reset: Option<ResetLine>` (no rseq attribute)
  - Extracted Fact (Verbatim): `ResetField { field_name: "reset", required: false, field_type: "Option<ResetLine>" }`, `FieldFact { rseq_kind: None }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all authored resource facts including exact Rust type are preserved explicitly in the extraction surface via `field_type`
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 3e: pinctrl resource

- Check: `pinctrl` resource preserves field name, kind, Rust type, requiredness, resource properties, join-key semantics
- Status: `pass`
- Premise: `pinctrl`: field name `pinctrl`, kind `pinctrl`, type `Option<PinCtrl>`, optional, `id = "uart_pins"`, join key `Properties.id`
- Assertion Predicate(s):
  - `assert_eq!(pin.field_name, "pinctrl")`
  - `assert_eq!(pin.pinctrl_id.as_deref(), Some("uart_pins"))`
  - `assert!(!pin.required)`
  - `assert_eq!(pin.field_type, "Option<PinCtrl>")` (exact Rust type fidelity)
  - `assert!(matches!(pinctrl_field.rseq_kind, Some(RseqFieldKind::PinCtrl { id }) if id == "uart_pins"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_pinctrl_resource` in `source_to_extraction_03_driver_struct.rs` asserts field name, pinctrl id, requiredness, exact Rust type via `field_type`, and pinctrl-field classification.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub pinctrl: Option<PinCtrl>`, `#[rseq::pinctrl(id = "uart_pins")]`
  - Extracted Fact (Verbatim): `PinCtrlField { field_name: "pinctrl", required: false, pinctrl_id: Some("uart_pins"), field_type: "Option<PinCtrl>" }`, `RseqFieldKind::PinCtrl { id: "uart_pins" }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all authored resource facts including exact Rust type are preserved explicitly in the extraction surface via `field_type`
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 3f: canonical paths for driver struct and resource fields

- Check: canonical path of driver struct and all resource/property/user fields
- Status: `pass`
- Premise: canonical path `pl011_full_spec::driver::Pl011FullSpec` for struct; `pl011_full_spec::driver::Pl011FullSpec::<field>` for each field
- Assertion Predicate(s):
  - `assert_eq!(full_canonical_path, "pl011_full_spec::driver::Pl011FullSpec")` for struct (reconstructed from crate root + def_path_str)
  - `assert_eq!(full_field_path, "pl011_full_spec::driver::Pl011FullSpec::<field>")` for each of 8 fields
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_3_canonical_paths` reconstructs the full canonical path by prepending the crate root module name (from enrichment modules) to the `def_path_str`-derived path, and asserts exact equality against the authored canonical path for the struct and all 8 fields.
- Supporting Evidence:
  - The specimen uses `--crate-name pl011_full_spec` and wraps the struct in `mod driver`, so `def_path_str` produces `driver::Pl011FullSpec` and the crate root is `pl011_full_spec`. Reconstruction produces the exact authored path.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::driver::Pl011FullSpec`
  - Extracted Fact (Verbatim): `driver::Pl011FullSpec` (def_path_str), crate root `pl011_full_spec` (enrichment modules)
  - Correspondence: `exact` (reconstructed as `pl011_full_spec::driver::Pl011FullSpec`)
  - Gap / Loss Description: none -- full path proven exactly via reconstruction from enrichment surface facts
  - Why This Status Follows: canonical path is exactly reconstructed and asserted with `assert_eq!` against the full authored path
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

## Check Group 4: Property field preservation

### 4a: current_speed property

- Check: `current_speed` preserves field name, type, id, default, requiredness
- Status: `pass`
- Premise: `current_speed`: field name `current_speed`, type `u32`, id `current-speed`, default `115200`, required
- Assertion Predicate(s):
  - `assert_eq!(speed.field_name, "current_speed")`
  - `assert_eq!(speed.property_id, "current-speed")`
  - `assert_eq!(speed.field_type, "u32")`
  - `assert_eq!(speed.default_literal.as_deref(), Some("115200"))`
  - `assert!(speed.required)`
  - `assert!(matches!(speed_field.rseq_kind, Some(RseqFieldKind::Property { id }) if id == "current-speed"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_4_current_speed_property` asserts all five property facts via `extract_property_surface` and rseq_kind via `extract_identity_enrichment`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::property(id = "current-speed", default = 115200)]`, `pub current_speed: u32`
  - Extracted Fact (Verbatim): `PropertyField { field_name: "current_speed", property_id: "current-speed", field_type: "u32", default_literal: Some("115200"), required: true, .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all five property facts preserved exactly in the extraction surface
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

### 4b: fifo_size property

- Check: `fifo_size` preserves field name, type, id, default, requiredness
- Status: `pass`
- Premise: `fifo_size`: field name `fifo_size`, type `u32`, id `fifo-size`, default `32`, required
- Assertion Predicate(s):
  - `assert_eq!(fifo.field_name, "fifo_size")`
  - `assert_eq!(fifo.property_id, "fifo-size")`
  - `assert_eq!(fifo.field_type, "u32")`
  - `assert_eq!(fifo.default_literal.as_deref(), Some("32"))`
  - `assert!(fifo.required)`
  - `assert!(matches!(fifo_field.rseq_kind, Some(RseqFieldKind::Property { id }) if id == "fifo-size"))`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_4_fifo_size_property` asserts all five property facts via `extract_property_surface` and rseq_kind via `extract_identity_enrichment`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::property(id = "fifo-size", default = 32)]`, `pub fifo_size: u32`
  - Extracted Fact (Verbatim): `PropertyField { field_name: "fifo_size", property_id: "fifo-size", field_type: "u32", default_literal: Some("32"), required: true, .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all five property facts preserved exactly
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

## Check Group 5: pending_tx user field not reclassified

- Check: `pending_tx` remains an ordinary user field and is not reclassified as a Regseq resource or property
- Status: `pass`
- Premise: `pending_tx`: type `Option<u8>`, ordinary user state, not a Regseq resource or property
- Assertion Predicate(s):
  - `assert!(SPECIMEN.contains("pending_tx: Option<u8>,"))` (Rust type verified in specimen)
  - `assert!(ptx_field.rseq_kind.is_none())` (identity enrichment: no rseq classification)
  - `assert!(!mmio_fields.iter().any(|f| f.field_name == "pending_tx"))` (closed-world: not in mmio)
  - `assert!(!int_surface.resources.iter().any(|r| r.field_name == "pending_tx"))` (closed-world: not in interrupt)
  - `assert!(!clk_surface.fields.iter().any(|f| f.field_name == "pending_tx"))` (closed-world: not in clock)
  - `assert!(!rst_surface.fields.iter().any(|f| f.field_name == "pending_tx"))` (closed-world: not in reset)
  - `assert!(!pin_surface.fields.iter().any(|f| f.field_name == "pending_tx"))` (closed-world: not in pinctrl)
  - `assert!(!prop_surface.fields.iter().any(|f| f.field_name == "pending_tx"))` (closed-world: not in property)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check_group_5_pending_tx_not_reclassified` asserts `rseq_kind == None` in enrichment and absence from every resource/property extraction surface (mmio, interrupt, clock, reset, pinctrl, property).
- Reasoning Trace:
  - Authored Fact (Verbatim): `pending_tx: Option<u8>` (no `#[rseq::*]` attribute)
  - Extracted Fact (Verbatim): `FieldFact { name: "pending_tx", rseq_kind: None, .. }`, absent from all six resource/property surfaces
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: closed-world evidence confirms pending_tx has no rseq classification and does not appear in any resource or property extraction surface
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass

---

## Summary

| Check | Status |
|---|---|
| Group 1: driver identity (name, compatible, bus) | pass |
| Group 2: device class (serial) | pass |
| Group 3a: regs MMIO resource | pass |
| Group 3b: irq interrupt resource | pass |
| Group 3c: clock resource | pass |
| Group 3d: reset resource | pass |
| Group 3e: pinctrl resource | pass |
| Group 3f: canonical paths | pass |
| Group 4a: current_speed property | pass |
| Group 4b: fifo_size property | pass |
| Group 5: pending_tx user field not reclassified | pass |

All checks pass. The extraction boundary preserves driver identity, device class, canonical paths, exact Rust type fidelity for all resources (including `irq`, `clock`, `reset`, `pinctrl` via the `field_type` field added to each resource declaration struct), property fields, and user-field non-reclassification.
