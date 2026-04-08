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
- Justification: The inline specimen now proves fidelity exhaustively for every in-scope register-block field declaration, not by representative wrapper/access samples. Facts not modeled (register_bitfields! expansion, driver struct methods, trait impls) are not needed for the register-block extraction checks.

## Check-to-Surface Mapping

| Check | Surface(s) | Test Function |
|-------|-----------|---------------|
| 1 | RegisterAccessSurface.register_blocks | check_1_register_block_identity |
| 2 | IdentityEnrichmentSurface.structs, .modules | check_2_canonical_path |
| 3 | RegisterAccessSurface.register_blocks[].fields | check_3_register_block_properties |
| 4 | RegisterAccessSurface.register_blocks[].fields | check_4_reserved_regions |
| 5 | RegisterAccessSurface.register_blocks[].fields | check_5_register_inventory_and_order |
| 6 | RegisterAccessSurface.register_blocks[].fields | check_6_per_register_facts |
| 7 | RegisterAccessSurface.register_blocks, .register_blocks[].fields | check_7_closed_world_inventory |
| 8 | RegisterAccessSurface.register_blocks[].fields | check_8_property_set_integrity |

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
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }`
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
- Status: `fail`
- Premise: The register-block properties preserve `endian = little` and `access_width = 32`.
- Assertion Predicate(s):
  - For every field in block: `field.endian == None` and `field.access_width == None`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:507`
  - All 25 fields have `endian == None` and `access_width == None`
  - `RegisterBlockDecl` struct definition (`crates/regseq-driver/src/extract/register_access.rs`, lines 22-26) has only `name: String` and `fields: Vec<RegisterFieldDecl>` -- no block-level property fields
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::register_block(endian = "little", access_width = 32)]`
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }` -- no endian, no access_width at block level
  - Correspondence: `absent`
  - Gap / Loss Description: `RegisterBlockDecl` has no fields to carry block-level `endian` or `access_width`. The `extract_register_block` function (`register_access.rs`, line 192) detects the `#[rseq::register_block]` attribute via `has_rseq_attr(attrs, "register_block")` but does not parse its key-value arguments. Per-field `endian`/`access_width` in `RegisterFieldDecl` come from `#[rseq::register(...)]` per-field attributes, which are absent from the PL011 fixture.
  - Why This Status Follows: Both authored properties are structurally absent from the extraction surface. No block-level property data survives.
- Counterexample: `RegisterBlockDecl` for `Pl011Registers` has no endian or access_width data at all.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `extract_register_block` in `crates/regseq-driver/src/extract/register_access.rs` (line 192)
  - Minimal Missing Fact To Preserve: Add `endian: Option<String>` and `access_width: Option<u32>` fields to `RegisterBlockDecl`, and parse the `#[rseq::register_block(...)]` attribute arguments using the same token-parsing logic as `parse_register_metadata`.
- Conclusion: fail

### Check 4

- Check: Reserved regions preserve authored offsets
- Status: `fail`
- Premise: The reserved regions preserve their authored offsets: `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`.
- Assertion Predicate(s):
  - `reserved_names == vec!["_reserved0", "_reserved1", "_reserved2"]` (names present)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:547`
  - Reserved field names are extracted correctly
  - `RegisterFieldDecl` struct definition (`crates/regseq-driver/src/extract/register_access.rs`, lines 29-36) has no `byte_offset`, `array_size`, or layout fields
- Reasoning Trace:
  - Authored Fact (Verbatim): `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "_reserved0", access_width: None, endian: None, ... }` -- name only, no offset or size
  - Correspondence: `weakened` (names preserved, offsets/sizes absent)
  - Gap / Loss Description: `RegisterFieldDecl` does not carry byte offset, array element count, or start/end address. The authored gap offsets cannot be reconstructed from the extraction boundary.
  - Why This Status Follows: The boundary preserves reserved region names but not the layout information needed to reconstruct exact byte gaps. The check spec requires offset preservation or enough layout data for exact reconstruction.
- Counterexample: `_reserved0` is known to be at name position 2 in the field list, but its byte offset 0x008 and end 0x014 are nowhere in the extraction output.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `extract_register_block` in `crates/regseq-driver/src/extract/register_access.rs`
  - Minimal Missing Fact To Preserve: Add byte offset computation from `#[repr(C)]` layout (using `tcx.layout_of` or field order + known access_width), or store array element counts for reserved fields.
- Conclusion: fail

### Check 5

- Check: Declared register inventory complete and in authored offset order
- Status: `pass`
- Premise: The declared register inventory remains complete and in authored offset order.
- Assertion Predicate(s):
  - `extracted_names == AUTHORED_FIELD_NAMES` (exact 25-element sequence equality)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:581`
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
- Status: `fail`
- Premise: Each declared register preserves: struct field name, register name, byte offset, access kind.
- Assertion Predicate(s):
  - `non_reserved == AUTHORED_REGISTER_NAMES` (field names preserved)
  - For all non-reserved fields: `field.access_width == None && field.endian == None` (confirming absence of other facts)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:607`
  - Field names match authored exactly (22 non-reserved registers)
  - `RegisterFieldDecl` has no `register_type_name`, `byte_offset`, or `access_kind` fields (structural absence in type definition at lines 29-36 of `register_access.rs`)
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g. `dr: DR_REG @ 0x000 [RW]` -- four facts per register
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "dr", access_width: None, endian: None, side_effects: None, hw_updates: None }` -- only field name
  - Correspondence: `weakened` (1 of 4 facts preserved)
  - Gap / Loss Description: Register descriptor type name (e.g. `DR_REG`), byte offset (e.g. `0x000`), and access kind (e.g. `RW`) are not fields in `RegisterFieldDecl` and are not extracted at all.
  - Why This Status Follows: The premise requires all four facts. Only struct field name survives. Three facts are structurally absent.
- Counterexample: Register `dr` has authored facts `DR_REG @ 0x000 [RW]` -- none of these three are present in the extracted `RegisterFieldDecl`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `RegisterFieldDecl` struct and `extract_register_block` in `crates/regseq-driver/src/extract/register_access.rs`
  - Minimal Missing Fact To Preserve: Add `register_type_name: Option<String>` (from the second generic arg of `ReadWrite<u32, DR_REG::Register>`), `byte_offset: Option<u32>` (from layout computation), `access_kind: Option<String>` (from wrapper type name: ReadWrite->RW, ReadOnly->R, WriteOnly->W).
- Conclusion: fail

### Check 7

- Check: No registers inserted, removed, renamed, or reordered
- Status: `pass`
- Premise: No registers are inserted, removed, renamed, or reordered.
- Assertion Predicate(s):
  - `block.fields.len() == AUTHORED_FIELD_NAMES.len()` (exact count: 25)
  - `extracted == AUTHORED_FIELD_NAMES` (exact sequence equality)
  - `surface.register_blocks.len() == 1` (no spurious blocks)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:657`
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
- Status: `fail`
- Premise: No register-block properties are invented or dropped.
- Assertion Predicate(s):
  - For all fields: `field.access_width == None`, `field.endian == None`, `field.side_effects == None`, `field.hw_updates == None`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs:694`
  - All 25 fields have all property fields as `None` -- no properties invented
  - `RegisterBlockDecl` struct has no block-level property fields (lines 22-26 of `register_access.rs`)
- Reasoning Trace:
  - Authored Fact (Verbatim): `endian = little`, `access_width = 32` on the register block
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }` -- no property data at any level
  - Correspondence: `absent` (properties dropped)
  - Gap / Loss Description: The authored property set `{endian, access_width}` is dropped entirely. No properties are invented (good), but both authored properties are lost because `RegisterBlockDecl` has no block-level property storage and the block-level `#[rseq::register_block(...)]` attribute arguments are not parsed.
  - Why This Status Follows: The check requires preservation of exactly the authored property set. Both authored properties are absent from the extraction surface. While no spurious properties are invented, the drop of the two authored properties constitutes a failure.
- Counterexample: `RegisterBlockDecl` for `Pl011Registers` has zero properties; the authored source specifies two.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `RegisterBlockDecl` and `extract_register_block` in `crates/regseq-driver/src/extract/register_access.rs`
  - Minimal Missing Fact To Preserve: Add `endian: Option<String>` and `access_width: Option<u32>` to `RegisterBlockDecl` and parse the `#[rseq::register_block(...)]` attribute arguments.
- Conclusion: fail

## Summary

| Check | Status |
|-------|--------|
| 1 - Register block identity | pass |
| 2 - Canonical path | pass |
| 3 - Block-level properties (endian, access_width) | fail |
| 4 - Reserved region offsets | fail |
| 5 - Register inventory and order | pass |
| 6 - Per-register facts (name, type, offset, access) | fail |
| 7 - Closed-world inventory | pass |
| 8 - Property set integrity | fail |

Result: 4 pass, 4 fail.

The four failures share a common root cause: `RegisterBlockDecl` and `RegisterFieldDecl` capture struct field names but not block-level attribute properties (endian, access_width), per-field layout data (byte offset, array size), per-field type information (register descriptor type name), or access kind classification (ReadWrite/ReadOnly/WriteOnly). The `extract_register_block` function recognizes `#[rseq::register_block]` but does not parse its arguments, and does not inspect field types for access classification or register descriptor identity.
