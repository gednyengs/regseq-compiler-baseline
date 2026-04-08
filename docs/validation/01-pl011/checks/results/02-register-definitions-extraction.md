# Validation Certificate: PL011 Register Definitions Extraction Check

Source premise: `docs/validation/01-pl011/checks/02-register-definitions-extraction.md`
Target boundary evidence: `crates/regseq-driver/src/extract/register_access.rs` (extraction surface), `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs` (exercised test)

## Specimen Fidelity

An inline specimen is used. The specimen is generated programmatically from
the `AUTHORED_REGISTERS` table, which mirrors the `register_bitfields!` macro
expansion for all 22 PL011 registers defined in
`tests/fixtures/pl011-full-spec/src/registers.rs`.

Fidelity Assertion Predicate(s):
- Every authored register name appears in the fixture source as a top-level `register_bitfields!` entry (`fixture_src.contains(&format!("\n    {} [", reg.name))`).
- The total register count in the fixture (lines matching `_REG [`) equals `AUTHORED_REGISTERS.len()` (22).
- Every authored field appears with exact `OFFSET(n) NUMBITS(m)` in the fixture.
- Every authored named value appears in its register's section with the exact numeric value (parsed from decimal, binary, or hex literal).

Fidelity Evidence (Exact Citations):
- Test `precondition_specimen_fidelity_against_fixture_source` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs` exercises all four predicates. The test reads the real fixture file at runtime via `CARGO_MANIFEST_DIR` and asserts every register, field, offset, width, and named value.

## Check-to-Surface Mapping

| Check Group | Extraction Surface | Test Function |
|---|---|---|
| 1 (register identity) | `RegisterAccessSurface.register_descriptors[].register_module` | `check_group_1_register_identity_preserved` |
| 2 (field inventory and order) | `RegisterAccessSurface.register_descriptors[].fields[]` | `check_group_2_field_inventory_complete_and_ordered` |
| 3 (field name, offset, width) | `RegisterDescriptorFieldFact.{field_name, offset, width}` | `check_group_3_field_name_offset_width_preserved` |
| 4 (named values) | `RegisterDescriptorFieldFact.named_values[]` | `check_group_4_named_values_preserved` |
| 5 (closed-world membership) | All of the above with exact-count and exact-order assertions | `check_group_5_exact_membership_and_ordering` |

## Check Group 1

- Check: Every in-scope register remains identifiable by its authored register name
- Status: pass
- Premise: Every in-scope register remains identifiable by its authored register name
- Assertion Predicate(s):
  - `surface.register_descriptors.iter().find(|d| d.register_module == reg.name).is_some()` for each of the 22 registers
  - `surface.register_descriptors.len() == AUTHORED_REGISTERS.len()` (exact total count = 22, no filtering)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_register_identity_preserved` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs`
  - Asserts presence of all 22 register modules by name and exact count
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 register names (DR_REG through PCELLID3_REG)
  - Extracted Fact (Verbatim): `register_module` field in each `RegisterDescriptorFact` matches the authored name
  - Correspondence: exact
  - Gap / Loss Description: none
  - Why This Status Follows: The extraction surface preserves the register module name exactly as authored. Each register is identified by `register_module` matching the authored name.
- Counterexample: N/A
- Conclusion: pass

## Check Group 2

- Check: Every in-scope register preserves its complete field inventory in authored order
- Status: pass
- Premise: Every in-scope register preserves its complete field inventory in authored order
- Assertion Predicate(s):
  - `descriptor.fields.len() == reg.fields.len()` for each register
  - `extracted_names == authored_names` (Vec<&str> equality preserving order) for each register
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_field_inventory_complete_and_ordered` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs`
  - Asserts exact field count and ordered name list for all 22 registers
- Reasoning Trace:
  - Authored Fact (Verbatim): Per-register field lists (e.g., DR_REG: OE, BE, PE, FE, DATA)
  - Extracted Fact (Verbatim): `RegisterDescriptorFieldFact.field_name` values in order match authored
  - Correspondence: exact
  - Gap / Loss Description: none
  - Why This Status Follows: Field sub-modules in the extraction appear in the same order as authored and with exact count matching.
- Counterexample: N/A
- Conclusion: pass

## Check Group 3

- Check: Every field preserves field name, bit offset, and bit width
- Status: pass
- Premise: Every field in every in-scope register preserves field name, bit offset, and bit width
- Assertion Predicate(s):
  - `extracted_field.offset == authored_field.offset` for each field
  - `extracted_field.width == authored_field.width` for each field
  - Field found by `descriptor.fields.iter().find(|f| f.field_name == authored_field.name)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_3_field_name_offset_width_preserved` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs`
  - Asserts all three properties for every field across all 22 registers
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g., CR_REG.CTSEN OFFSET(15) NUMBITS(1)
  - Extracted Fact (Verbatim): `RegisterDescriptorFieldFact { field_name: "CTSEN", offset: 15, width: 1, .. }`
  - Correspondence: exact
  - Gap / Loss Description: none
  - Why This Status Follows: The extraction recovers offset from the shift parameter and width from the mask bit count in `parse_field_const`. These match the authored OFFSET/NUMBITS exactly.
- Counterexample: N/A
- Conclusion: pass

## Check Group 4

- Check: Every authored named field value preserves symbolic name and literal value
- Status: pass
- Premise: Every authored named field value preserves symbolic name and literal value
- Assertion Predicate(s):
  - `extracted_field.named_values.len() == authored_field.named_values.len()` (exact count)
  - `extracted_nv.name == nv_name && extracted_nv.value == nv_value` for each named value
  - `extracted_nv_names == authored_nv_names` (order preservation)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_4_named_values_preserved` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs`
  - Asserts name, value, count, and order for all named values across all registers with named values (LCRH_REG, CR_REG, IFLS_REG, DMACR_REG, PERIPHID0-3_REG, PCELLID0-3_REG)
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g., LCRH_REG.WLEN: FIVE_BITS=0b00, SIX_BITS=0b01, SEVEN_BITS=0b10, EIGHT_BITS=0b11
  - Extracted Fact (Verbatim): `RegisterNamedValueFact { name: "FIVE_BITS", value: 0 }`, etc.
  - Correspondence: exact
  - Gap / Loss Description: none
  - Why This Status Follows: `parse_field_value_const` recovers the un-shifted value by dividing out the field offset. The extraction preserves the constant name from the HIR item ident.
- Counterexample: N/A
- Conclusion: pass

## Check Group 5

- Check: No fields or named values are inserted, removed, renamed, reordered, or changed
- Status: pass
- Premise: The inspected boundary preserves exact authored membership and ordering
- Assertion Predicate(s):
  - `surface.register_descriptors.len() == AUTHORED_REGISTERS.len()` (exact total register count, no filtering, closed-world)
  - `descriptor.fields.len() == reg.fields.len()` (exact field count per register)
  - `extracted_field_names == authored_field_names` (exact field membership and order)
  - `extracted_pairs == authored_pairs` where pairs are `(name, value)` tuples (exact named-value membership, values, and order)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_5_exact_membership_and_ordering` in `crates/regseq-driver/tests/source_to_extraction_02_register_definitions.rs`
  - Asserts closed-world evidence: exact counts, exact ordered lists, exact name-value pairs for all 22 registers
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 registers with exact field and named-value inventories
  - Extracted Fact (Verbatim): Extraction produces exactly matching inventories
  - Correspondence: exact
  - Gap / Loss Description: none
  - Why This Status Follows: Exact-length checks combined with ordered equality assertions constitute closed-world evidence. Any insertion, removal, rename, reorder, or value change would cause assertion failure.
- Counterexample: N/A
- Conclusion: pass
