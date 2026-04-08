# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence:
  - `crates/regseq-driver/src/extract/register_access.rs` (RegisterAccessSurface)
  - `crates/regseq-driver/src/extract/identity_enrichment.rs` (IdentityEnrichmentSurface)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs` (test file)

## Specimen Fidelity

- Specimen type: inline reduced specimen
- Fidelity Assertion Predicate(s):
  - `fixture_src.contains("pub struct Pl011Registers")` -- struct name matches
  - `fixture_src.contains(r#"#[rseq::register_block(endian = "little", access_width = 32)]"#)` -- attribute matches
  - For every authored field declaration in `AUTHORED_FIELD_DECLS`, `fixture_src.contains(field_decl)` and `SPECIMEN_SOURCE.contains(field_decl)` -- exact declaration equality across all 25 fields
  - Positional scan over `AUTHORED_FIELD_DECLS` succeeds in `fixture_src` -- fixture field ordering matches authored order
  - Positional scan over `AUTHORED_FIELD_DECLS` succeeds in `SPECIMEN_SOURCE` -- specimen field ordering matches authored order
  - `AUTHORED_FIELD_DECLS.len() == AUTHORED_FIELD_NAMES.len()` -- every authored field has an exact declaration entry
- Fidelity Evidence (Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:349`
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:371`
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:384`
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:393`
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:410`
- Justification: The inline specimen proves fidelity exhaustively for every in-scope register-block field declaration, not by representative wrapper/access samples. Facts not modeled (register_bitfields! expansion, driver struct methods, trait impls) are not needed for the register-block extraction checks.

## Check-to-Surface Mapping

| Check | Surface(s) | Test Function |
|-------|-----------|---------------|
| 1 | RegisterAccessSurface.register_blocks | check_1_register_block_identity |
| 2 | IdentityEnrichmentSurface.structs, .modules | check_2_canonical_path |
| 3 | RegisterAccessSurface.register_blocks[].properties | check_3_register_block_properties |
| 4 | RegisterAccessSurface.register_blocks[].properties, .fields[].byte_offset, .array_count | check_4_reserved_regions |
| 5 | RegisterAccessSurface.register_blocks[].fields | check_5_register_inventory_and_order |
| 6 | RegisterAccessSurface.register_blocks[].fields[].register_type_name, .byte_offset, .access_kind | check_6_per_register_facts |
| 7 | RegisterAccessSurface.register_blocks, .register_blocks[].fields | check_7_closed_world_inventory |
| 8 | RegisterAccessSurface.register_blocks[].properties; per-field absence checks | check_8_property_set_integrity |

## Per-Check Results

### Check 1

- Check: Register block remains identifiable as Pl011Registers
- Status: `pass`
- Premise: The register block remains identifiable as `Pl011Registers`.
- Assertion Predicate(s):
  - `surface.register_blocks.iter().filter(|b| b.name == "Pl011Registers").count() == 1`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:422`
  - `RegisterAccessSurface.register_blocks` contains exactly one entry with `name == "Pl011Registers"`
- Reasoning Trace:
  - Authored Fact (Verbatim): `Pl011Registers`
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", ... }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The extraction surface explicitly preserves the struct name via HIR `ident.name.as_str()` in `extract_register_block`.
- Counterexample: none found
- Conclusion: pass

### Check 2

- Check: Canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Status: `pass`
- Premise: The canonical path remains `pl011_full_spec::registers::Pl011Registers`.
- Assertion Predicate(s):
  - `identity.structs.iter().any(|s| s.canonical_path.0 == "registers::Pl011Registers" && s.is_register_block)`
  - `identity.modules.iter().any(|m| m.name == "pl011_full_spec" && m.canonical_path.0 == "pl011_full_spec")`
  - crate root module has `registers` module as child
  - struct's `module_path.0 == "registers"`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:445`
  - `IdentityEnrichmentSurface.structs` contains struct with `canonical_path.0 == "registers::Pl011Registers"`, `is_register_block == true`, `module_path.0 == "registers"`
  - `IdentityEnrichmentSurface.modules` contains crate root `pl011_full_spec` with `registers` as child
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::registers::Pl011Registers`
  - Extracted Fact (Verbatim): source-local path `registers::Pl011Registers` + crate root module `pl011_full_spec` + module containment `registers` is child of crate root
  - Correspondence: `exact` (reconstructible from four exercised boundary facts)
  - Gap / Loss Description: The source-local path does not include the crate name prefix, but the crate root module name and containment hierarchy provide exactly enough evidence to reconstruct the full canonical path.
  - Why This Status Follows: Combining (1) crate root name `pl011_full_spec`, (2) `registers` module is a child of crate root, (3) struct canonical_path is `registers::Pl011Registers`, and (4) struct module_path is `registers` yields the exact canonical path `pl011_full_spec::registers::Pl011Registers`. No heuristic is needed.
- Counterexample: none found
- Conclusion: pass

### Check 3

- Check: Register-block properties preserve `endian = little` and `access_width = 32`
- Status: `pass`
- Premise: The register-block properties preserve `endian = little` and `access_width = 32`.
- Assertion Predicate(s):
  - `block.properties.iter().find(|(k, _)| k == "endian").unwrap().1 == BlockPropertyValue::Str("little")`
  - `block.properties.iter().find(|(k, _)| k == "access_width").unwrap().1 == BlockPropertyValue::Int(32)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:524` (endian property lookup and assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:536` (access_width property lookup and assertion)
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::register_block(endian = "little", access_width = 32)]`
  - Extracted Fact (Verbatim): `RegisterBlockDecl { properties: [("endian", Str("little")), ("access_width", Int(32))], ... }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: `parse_block_properties` parses all key=value pairs from the `#[rseq::register_block(...)]` attribute into a generic property bag (`Vec<(String, BlockPropertyValue)>`), preserving every authored property regardless of whether it is a known key. The typed `BlockPropertyValue` enum distinguishes string and integer literals.
- Counterexample: none found
- Conclusion: pass

### Check 4

- Check: Reserved regions preserve authored offsets
- Status: `pass`
- Premise: The reserved regions preserve their authored offsets: `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`.
- Assertion Predicate(s):
  - `_reserved0.byte_offset == Some(0x008)`, `_reserved0.array_count == Some(4)`, computed end `0x008 + (4-1)*4 == 0x014`
  - `_reserved1.byte_offset == Some(0x01C)`, `_reserved1.array_count == None`, computed end `0x01C + (1-1)*4 == 0x01C`
  - `_reserved2.byte_offset == Some(0x04C)`, `_reserved2.array_count == Some(997)`, computed end `0x04C + (997-1)*4 == 0xFDC`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:581` (_reserved0 start offset)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:583` (_reserved0 array count)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:585` (_reserved0 end offset)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:593` (_reserved1 start offset)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:594` (_reserved1 array count is None)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:596` (_reserved1 end offset)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:604` (_reserved2 start offset)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:606` (_reserved2 array count)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:608` (_reserved2 end offset)
- Reasoning Trace:
  - Authored Fact (Verbatim): `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
  - Extracted Fact (Verbatim): `_reserved0: byte_offset=0x008, array_count=4`; `_reserved1: byte_offset=0x01C, array_count=None`; `_reserved2: byte_offset=0x04C, array_count=997`
  - Correspondence: `exact` (start offsets explicit, end offsets exactly reconstructable from `byte_offset + (array_count.unwrap_or(1) - 1) * element_size` where `element_size = access_width / 8`)
  - Gap / Loss Description: none
  - Why This Status Follows: `extract_register_block` computes byte offsets cumulatively from the block-level `access_width` property (extracted from the generic property bag) and field array counts. Array counts are extracted via `tcx.type_of(field.def_id)` for array-typed fields. The test reconstructs the exact authored end offsets from these boundary facts without heuristic guesswork.
- Counterexample: none found
- Conclusion: pass

### Check 5

- Check: Declared register inventory complete and in authored offset order
- Status: `pass`
- Premise: The declared register inventory remains complete and in authored offset order.
- Assertion Predicate(s):
  - `extracted_names == AUTHORED_FIELD_NAMES` (exact 25-element sequence equality)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:630`
  - All 25 fields extracted in authored order from `RegisterAccessSurface.register_blocks[0].fields`
- Supporting Evidence: The check spec states "a complete extracted field sequence in authored order is sufficient for the inventory/order checks even if per-field offset and access metadata is not preserved."
- Reasoning Trace:
  - Authored Fact (Verbatim): 25 fields in order: dr, rsr_ecr, _reserved0, fr, _reserved1, ilpr, ibrd, fbrd, lcr_h, cr, ifls, imsc, ris, mis, icr, dmacr, _reserved2, periph_id0, periph_id1, periph_id2, periph_id3, pcell_id0, pcell_id1, pcell_id2, pcell_id3
  - Extracted Fact (Verbatim): exact same 25-element field name sequence from `block.fields`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: HIR struct field iteration preserves source declaration order for `#[repr(C)]` structs, and the extraction captures every field including reserved ones.
- Counterexample: none found
- Conclusion: pass

### Check 6

- Check: Per-register fact preservation (field name, register name, byte offset, access kind)
- Status: `pass`
- Premise: Each declared register preserves: struct field name, register name, byte offset, access kind.
- Assertion Predicate(s):
  - For all 22 non-reserved registers: `field.register_type_name == Some(authored_type)`, `field.byte_offset == Some(authored_offset)`, `field.access_kind == Some(authored_access)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:653` (authored fact table with all 22 registers)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:685` (register_type_name assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:690` (byte_offset assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:695` (access_kind assertion)
  - All 22 non-reserved fields verified with exact four-fact match
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g. `dr: DR_REG @ 0x000 [RW]` -- four facts per register
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "dr", register_type_name: Some("DR_REG"), byte_offset: Some(0x000), access_kind: Some("RW"), ... }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: `extract_register_block` extracts: (1) field name from HIR `field.ident`, (2) register descriptor type name from the second generic argument of the wrapper type path, (3) byte offset computed cumulatively from `access_width` and array counts, (4) access kind mapped from wrapper type name (ReadWrite->RW, ReadOnly->R, WriteOnly->W).
- Counterexample: none found
- Conclusion: pass

### Check 7

- Check: No registers inserted, removed, renamed, or reordered
- Status: `pass`
- Premise: No registers are inserted, removed, renamed, or reordered.
- Assertion Predicate(s):
  - `block.fields.len() == AUTHORED_FIELD_NAMES.len()` (exact count: 25)
  - `extracted == AUTHORED_FIELD_NAMES` (exact sequence equality)
  - `surface.register_blocks.len() == 1` (no spurious blocks)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:719`
  - Field count is exactly 25, sequence is identical to authored, only 1 register block exists
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 registers + 3 reserved = 25 fields, no extras, no missing
  - Extracted Fact (Verbatim): exactly 25 fields with identical names in identical order, single register block
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Closed-world evidence: exact count match (no insertions/removals), exact sequence equality (no reordering/renaming), and single-block constraint (no spurious blocks) together prove no register modifications occurred.
- Counterexample: none found
- Conclusion: pass

### Check 8

- Check: No register-block properties invented or dropped
- Status: `pass`
- Premise: No register-block properties are invented or dropped.
- Assertion Predicate(s):
  - `block.properties.iter().map(|(k, _)| k.as_str()).collect() == ["endian", "access_width"]` (exact authored key set in authored order)
  - `block.properties[0].1 == BlockPropertyValue::Str("little")` (endian value)
  - `block.properties[1].1 == BlockPropertyValue::Int(32)` (access_width value)
  - For all fields: `field.access_width == None`, `field.endian == None`, `field.side_effects == None`, `field.hw_updates == None` (no per-field properties invented)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:756` (exact property key set assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:762` (endian value assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:767` (access_width value assertion)
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:775` (per-field property absence loop)
- Reasoning Trace:
  - Authored Fact (Verbatim): `endian = little`, `access_width = 32` on the register block
  - Extracted Fact (Verbatim): `RegisterBlockDecl { properties: [("endian", Str("little")), ("access_width", Int(32))], ... }` -- generic property bag preserves the exact authored set; no per-field properties invented
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The `parse_block_properties` function extracts all key=value pairs from the `#[rseq::register_block(...)]` attribute into a generic `Vec<(String, BlockPropertyValue)>` property bag. This preserves the full authored property set without a fixed whitelist, so future block-level properties will also be captured. The exact key sequence match proves no properties are invented or dropped. No spurious per-field properties are invented (all per-field property fields remain `None` since the PL011 fixture does not use per-field `#[rseq::register(...)]` attributes).
- Counterexample: none found
- Conclusion: pass

## Summary

| Check | Status |
|-------|--------|
| 1 - Register block identity | pass |
| 2 - Canonical path | pass |
| 3 - Block-level properties (endian, access_width) | pass |
| 4 - Reserved region offsets | pass |
| 5 - Register inventory and order | pass |
| 6 - Per-register facts (name, type, offset, access) | pass |
| 7 - Closed-world inventory | pass |
| 8 - Property set integrity | pass |

Result: 8 pass, 0 fail.

The four previously failing checks (3, 4, 6, 8) now pass. Block-level properties are stored in a generic property bag (`Vec<(String, BlockPropertyValue)>`) via `parse_block_properties`, which captures all authored key=value pairs from the `#[rseq::register_block(...)]` attribute without a fixed whitelist. Per-field metadata (`register_type_name`, `byte_offset`, `access_kind`, `array_count`) is extracted from HIR field types and cumulative layout computation.
