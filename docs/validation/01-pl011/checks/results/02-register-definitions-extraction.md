# Validation Certificate: PL011 Register Definitions Extraction Check

Source premise: `docs/validation/01-pl011/checks/02-register-definitions-extraction.md`
Target boundary evidence: `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`

## Specimen Fidelity

- Specimen type: inline (generated from authored facts, mirroring `register_bitfields!` macro expansion pattern)
- Fidelity Assertion Predicate(s):
  - `precondition_specimen_fidelity_against_fixture` reads the real fixture at `tests/fixtures/pl011-full-spec/src/registers.rs` and asserts that every authored register name, field declaration (`FIELD_NAME OFFSET(n) NUMBITS(m)`), and named value (`NAME = value`) appears verbatim in the fixture source text.
  - Asserts exactly 22 in-scope registers.
  - For each register: verifies the register declaration pattern exists in the fixture.
  - For each field: verifies the exact `OFFSET(n) NUMBITS(m)` pattern exists.
  - For each named value: verifies the `NAME = value` pattern exists (checking decimal, hex, and binary representations).
- Fidelity Evidence (Exact Citations):
  - Test: `precondition_specimen_fidelity_against_fixture` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Fixture: `tests/fixtures/pl011-full-spec/src/registers.rs`
  - All 22 registers, all fields, and all named values verified against fixture source.

## Check-to-Surface Mapping

| Check Group | Extraction Surface | Test Function |
|---|---|---|
| 1 - Register identity | `RegisterAccessSurface.register_descriptors[].register_module` | `check_group_1_register_identity_preservation` |
| 2 - Field inventory and order | `RegisterAccessSurface.register_descriptors[].fields` (name list and count) | `check_group_2_field_inventory_completeness_and_order` |
| 3 - Field name, offset, width | `RegisterAccessSurface.register_descriptors[].fields[].{field_name, offset, width}` | `check_group_3_field_name_offset_width_preservation` |
| 4 - Named value symbolic name and literal | `RegisterAccessSurface.register_descriptors[].fields[].named_values[].{name, value}` | `check_group_4_named_value_preservation` |
| 5 - No insertions/removals/renames/reorders | All of the above with closed-world exact-count and exact-set assertions | `check_group_5_no_insertions_removals_renames_reorders` |

## Check Group 1: Register Identity Preservation

- Check: every in-scope register remains identifiable by its authored register name
- Status: `pass`
- Premise: every in-scope register remains identifiable by its authored register name
- Assertion Predicate(s):
  - For each of the 22 in-scope registers: `surface.register_descriptors.iter().any(|d| d.register_module == reg_name)` asserts true
  - `surface.register_descriptors.len() == 22` asserts no extra descriptors
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_register_identity_preservation` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Asserts each of: DR_REG, RSR_ECR_REG, FR_REG, ILPR_REG, IBRD_REG, FBRD_REG, LCRH_REG, CR_REG, IFLS_REG, IMSC_REG, RIS_REG, MIS_REG, ICR_REG, DMACR_REG, PERIPHID0_REG, PERIPHID1_REG, PERIPHID2_REG, PERIPHID3_REG, PCELLID0_REG, PCELLID1_REG, PCELLID2_REG, PCELLID3_REG
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 register names as listed in check spec "Registers in scope"
  - Extracted Fact (Verbatim): `register_module` field on each `RegisterDescriptorFact` matches the authored name exactly
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the extraction surface preserves register identity as the module name, which matches the authored register name exactly
- Counterexample: n/a
- Conclusion: pass

## Check Group 2: Field Inventory Completeness and Order

- Check: every in-scope register preserves its complete field inventory in authored order
- Status: `pass`
- Premise: every in-scope register preserves its complete field inventory in authored order
- Assertion Predicate(s):
  - For each register: `descriptor.fields.len() == authored_fields.len()` (exact count)
  - For each register: `extracted_names == authored_names` where names are collected in order (exact order preservation)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_field_inventory_completeness_and_order` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Exercises all 22 registers with their complete field lists
- Reasoning Trace:
  - Authored Fact (Verbatim): field inventories from check spec (e.g. DR_REG: OE, BE, PE, FE, DATA)
  - Extracted Fact (Verbatim): field names extracted from `RegisterDescriptorFieldFact.field_name` in order
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: field count and ordered name list match exactly for every register
- Counterexample: n/a
- Conclusion: pass

## Check Group 3: Field Name, Bit Offset, and Bit Width Preservation

- Check: every field preserves field name, bit offset, and bit width
- Status: `pass`
- Premise: every field in every in-scope register preserves field name, bit offset, and bit width
- Assertion Predicate(s):
  - For each field: `extracted.offset == authored.offset`
  - For each field: `extracted.width == authored.width`
  - Field name match is implicit via the `find(|f| f.field_name == authored.name)` lookup
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_3_field_name_offset_width_preservation` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Exercises every field in all 22 registers
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g. DR_REG.OE: offset=11, width=1; LCRH_REG.WLEN: offset=5, width=2
  - Extracted Fact (Verbatim): `RegisterDescriptorFieldFact.offset` and `.width` match
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all three per-field properties (name, offset, width) are proven by separate assertions for every field
- Counterexample: n/a
- Conclusion: pass

## Check Group 4: Named Value Symbolic Name and Literal Value Preservation

- Check: every authored named field value preserves symbolic name and literal value
- Status: `pass`
- Premise: every authored named field value preserves symbolic name and literal value
- Assertion Predicate(s):
  - For each field with named values: `extracted.named_values.len() == authored.named_values.len()` (closed-world count)
  - For each named value: `found.name == nv_name` (via find lookup)
  - For each named value: `found.value == nv_value`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_4_named_value_preservation` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Exercises all named values across LCRH_REG (7 fields, 16 values), CR_REG (12 fields, 24 values), IFLS_REG (2 fields, 10 values), DMACR_REG (3 fields, 6 values), PERIPHID0_REG through PCELLID3_REG identification values
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g. LCRH_REG.WLEN: FIVE_BITS=0, SIX_BITS=1, SEVEN_BITS=2, EIGHT_BITS=3
  - Extracted Fact (Verbatim): `RegisterNamedValueFact.name` and `.value` match
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: both symbolic name and literal value are proven by assertion for every authored named value
- Counterexample: n/a
- Conclusion: pass

## Check Group 5: No Insertions, Removals, Renames, Reorders, or Changes

- Check: no fields or named values are inserted, removed, renamed, reordered, or changed
- Status: `pass`
- Premise: the inspected boundary preserves exact authored membership and ordering
- Assertion Predicate(s):
  - Register level: `surface.register_descriptors.len() == facts.len()` (exact count, no insertions/removals)
  - Register level: sorted extracted register names == sorted authored register names (exact set equality)
  - Per register: `extracted_field_names == authored_field_names` (exact ordered list, catches insertions, removals, renames, reorders)
  - Per field: `extracted.named_values.len() == authored.named_values.len()` (exact count)
  - Per field: bidirectional set membership check (every authored name exists in extracted; every extracted name exists in authored)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_5_no_insertions_removals_renames_reorders` in `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
  - Uses closed-world evidence: exact counts, exact ordered lists, and bidirectional membership checks
- Reasoning Trace:
  - Authored Fact (Verbatim): exact inventories from check spec
  - Extracted Fact (Verbatim): inventories from extraction match exactly
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: closed-world assertions prove no insertions, removals, renames, or reorders at register, field, and named-value levels
- Counterexample: n/a
- Conclusion: pass

## Summary

All 5 check groups pass. The PL011 register definitions are faithfully preserved at the compiler-side extraction boundary (`RegisterAccessSurface.register_descriptors`). Specimen fidelity was established by programmatic comparison against the real fixture source before any extraction checks were evaluated.
