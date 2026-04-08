# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence:
  - `crates/regseq-driver/src/extract/register_access.rs` (RegisterAccessSurface)
  - `crates/regseq-driver/src/extract/identity_enrichment.rs` (IdentityEnrichmentSurface)
  - `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs` (test file)

## Specimen Fidelity

- Specimen type: inline reduced specimen
- Fidelity Assertion Predicate(s):
  - `fixture_src.contains("pub struct Pl011Registers")` -- struct name matches
  - `fixture_src.contains(r#"#[rseq::register_block(endian = "little", access_width = 32)]"#)` -- attribute matches
  - All 25 authored field names present in fixture source in authored order (positional scan)
  - `fixture_src.contains("_reserved0: [Reserved<u32>; 4]")` -- reserved0 array size
  - `fixture_src.contains("_reserved1: Reserved<u32>")` -- reserved1 scalar
  - `fixture_src.contains("_reserved2: [Reserved<u32>; 997]")` -- reserved2 array size
  - Wrapper type checks: `ReadWrite<u32, DR_REG::Register>`, `ReadOnly<u32, FR_REG::Register>`, `WriteOnly<u32, ICR_REG::Register>`, etc.
- Fidelity Evidence (Exact Citations):
  - Test `specimen_fidelity_against_real_fixture` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - Reads `tests/fixtures/pl011-full-spec/src/registers.rs` at runtime
  - All 6 fidelity assertions pass
- Justification: The inline specimen preserves: struct name, register_block attribute with properties, all 25 field names in authored order, wrapper types (ReadWrite/ReadOnly/WriteOnly), reserved region array sizes. Facts not modeled (register_bitfields! expansion, driver struct, trait impls) are not needed for the register-block extraction checks.

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
- Status: `pass`
- Premise: the register block remains identifiable as `Pl011Registers`
- Assertion Predicate(s):
  - `surface.register_blocks.iter().filter(|b| b.name == "Pl011Registers").count() == 1`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_1_register_block_identity` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - RegisterAccessSurface contains exactly one RegisterBlockDecl with `name == "Pl011Registers"`
- Reasoning Trace:
  - Authored Fact (Verbatim): register block named `Pl011Registers`
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The extraction surface preserves the struct name via `ident.name.as_str()` in `extract_register_block`. The name matches exactly.
- Counterexample: none found
- Conclusion: pass

### Check 2
- Status: `pass`
- Premise: the canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Assertion Predicate(s):
  - `identity.structs.iter().any(|s| s.canonical_path.0 == "registers::Pl011Registers" && s.is_register_block)`
  - `identity.modules.iter().any(|m| m.name == "pl011_full_spec" && m.canonical_path.0 == "pl011_full_spec")`
  - `crate_root.child_items.iter().any(|c| c.0 == registers_module.canonical_path.0)`
  - `first_match.module_path.0 == "registers"`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_2_canonical_path` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - IdentityEnrichmentSurface preserves: source-local path `registers::Pl011Registers`, crate root module `pl011_full_spec`, module containment showing `registers` is child of crate root, struct's `module_path` is `registers`
- Supporting Evidence (Optional; Exact Citations):
  - `tcx.def_path_str(DefId)` returns source-local paths for local items (no crate prefix), but `tcx.crate_name(LOCAL_CRATE)` is captured as the crate root module name
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::registers::Pl011Registers`
  - Extracted Fact (Verbatim): struct canonical_path = `registers::Pl011Registers`, crate root = `pl011_full_spec`, module containment = `pl011_full_spec` -> `registers` -> struct
  - Correspondence: `exact` (reconstructible)
  - Gap / Loss Description: The canonical_path field itself contains only the source-local path `registers::Pl011Registers`. The full canonical path `pl011_full_spec::registers::Pl011Registers` is reconstructible from the module hierarchy.
  - Why This Status Follows: The check spec says "the inspected boundary state preserves the same canonical path or enough path evidence to reconstruct it exactly". The crate root name + module hierarchy + source-local path provide exact reconstruction evidence.
- Counterexample: none found
- Conclusion: pass

### Check 3
- Status: `fail`
- Premise: the register-block properties preserve `endian = little` and `access_width = 32`
- Assertion Predicate(s):
  - For every field in block: `field.endian == None` and `field.access_width == None`
  - These prove the properties are absent from the extraction surface
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_3_register_block_properties` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - RegisterBlockDecl struct definition (line 23-26 of register_access.rs) has only `name: String` and `fields: Vec<RegisterFieldDecl>` -- no block-level `endian` or `access_width` fields
  - RegisterFieldDecl has per-field `access_width` and `endian` but these come from per-field `#[rseq::register(...)]` attributes, which are absent on PL011 fixture fields
  - `has_rseq_attr(attrs, "register_block")` (line 128 of register_access.rs) checks presence only; attribute values are not parsed
- Reasoning Trace:
  - Authored Fact (Verbatim): `endian = little`, `access_width = 32` from `#[rseq::register_block(endian = "little", access_width = 32)]`
  - Extracted Fact (Verbatim): RegisterBlockDecl has no endian or access_width fields; all per-field values are None
  - Correspondence: `absent`
  - Gap / Loss Description: Block-level properties from `#[rseq::register_block(...)]` are recognized only as a boolean presence check. The attribute argument values (endian, access_width) are never parsed or stored in RegisterBlockDecl.
  - Why This Status Follows: Both authored properties are dropped at the extraction boundary. The check spec requires both to survive explicitly.
- Counterexample: `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }` -- no endian or access_width fields exist on the struct
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, function `extract_register_block` (line 192) and struct `RegisterBlockDecl` (line 23)
  - Minimal Missing Fact To Preserve: Add `endian: Option<String>` and `access_width: Option<u32>` fields to `RegisterBlockDecl`; parse the `#[rseq::register_block(...)]` attribute arguments using the existing `parse_register_metadata` pattern
- Conclusion: fail

### Check 4
- Status: `fail`
- Premise: the reserved regions preserve their authored offsets: `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
- Assertion Predicate(s):
  - `reserved_names == ["_reserved0", "_reserved1", "_reserved2"]` -- names are preserved
  - RegisterFieldDecl struct has no `byte_offset` or `array_size` fields -- offsets cannot be reconstructed
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_4_reserved_regions` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - RegisterFieldDecl definition (line 28-35 of register_access.rs): fields are `name`, `access_width`, `endian`, `side_effects`, `hw_updates` -- no byte offset or element count
- Reasoning Trace:
  - Authored Fact (Verbatim): `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
  - Extracted Fact (Verbatim): field names `_reserved0`, `_reserved1`, `_reserved2` are extracted; no offset or size data
  - Correspondence: `weakened`
  - Gap / Loss Description: Reserved region names are preserved but byte offsets and array sizes (needed to compute gap ranges) are not captured. The extraction surface does not record field byte offsets, field types (array vs scalar), or array element counts.
  - Why This Status Follows: The check spec says "the inspected boundary state preserves the reserved-layout facts, or preserves enough register-block layout information to reconstruct those gaps exactly". Without byte offsets or array sizes, exact gap reconstruction is impossible.
- Counterexample: `RegisterFieldDecl { name: "_reserved0", access_width: None, endian: None, ... }` -- no offset or size information
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, struct `RegisterFieldDecl` and function `extract_register_block`
  - Minimal Missing Fact To Preserve: Add `byte_offset: Option<u64>` and optionally `array_count: Option<u32>` to RegisterFieldDecl; compute byte offsets from `#[repr(C)]` struct layout using `access_width` and field ordering, or extract from HIR type information
- Conclusion: fail

### Check 5
- Status: `pass`
- Premise: the declared register inventory remains complete and in authored offset order
- Assertion Predicate(s):
  - `extracted_names == AUTHORED_FIELD_NAMES` -- exact sequence equality of all 25 field names in authored order
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_5_register_inventory_and_order` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - All 25 fields (22 registers + 3 reserved) extracted in source order by HIR field iteration
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 declared registers + 3 reserved regions in authored offset order
  - Extracted Fact (Verbatim): 25 RegisterFieldDecl entries in order: dr, rsr_ecr, _reserved0, fr, _reserved1, ilpr, ibrd, fbrd, lcr_h, cr, ifls, imsc, ris, mis, icr, dmacr, _reserved2, periph_id0, periph_id1, periph_id2, periph_id3, pcell_id0, pcell_id1, pcell_id2, pcell_id3
  - Correspondence: `exact`
  - Gap / Loss Description: none for inventory/order
  - Why This Status Follows: The check spec says "a complete extracted field sequence in authored order is sufficient for the inventory/order checks even if per-field offset and access metadata is not preserved". The exact field sequence is preserved.
- Counterexample: none found
- Conclusion: pass

### Check 6
- Status: `fail`
- Premise: each declared register preserves: struct field name, register name, byte offset, access kind
- Assertion Predicate(s):
  - `non_reserved == AUTHORED_REGISTER_NAMES` -- struct field names preserved (partial pass)
  - For all non-reserved fields: `field.access_width == None` and `field.endian == None` -- register name, byte offset, and access kind absent
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_6_per_register_facts` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - RegisterFieldDecl (line 28-35 of register_access.rs) has no fields for: register descriptor type name (e.g. DR_REG), byte offset, or access kind (R/RW/W)
- Reasoning Trace:
  - Authored Fact (Verbatim): `dr: DR_REG @ 0x000 [RW]`, `fr: FR_REG @ 0x018 [R]`, `icr: ICR_REG @ 0x044 [W]`, etc.
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "dr", access_width: None, endian: None, side_effects: None, hw_updates: None }`
  - Correspondence: `weakened` -- only struct field name is preserved; register descriptor name, byte offset, and access kind are lost
  - Gap / Loss Description: Of four required per-register facts, only struct field name survives. Register descriptor type name (e.g. DR_REG) is not captured in RegisterFieldDecl. Byte offset is not captured. Access kind (ReadWrite/ReadOnly/WriteOnly wrapper type) is not captured at the register-block level, though wrapper types are available in HIR.
  - Why This Status Follows: The check requires all four facts to survive. Three of four are absent. Multi-property premise requires all named properties.
- Counterexample: `RegisterFieldDecl { name: "dr", ... }` has no register_type_name, byte_offset, or access_kind fields
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, struct `RegisterFieldDecl` and function `extract_register_block`
  - Minimal Missing Fact To Preserve: Add `register_type_name: Option<String>` (from field type's generic arg, similar to `extract_first_generic_arg_name`), `byte_offset: Option<u64>`, and `access_kind: Option<String>` (from wrapper type name: ReadWrite->RW, ReadOnly->R, WriteOnly->W) to RegisterFieldDecl
- Conclusion: fail

### Check 7
- Status: `pass`
- Premise: no registers are inserted, removed, renamed, or reordered
- Assertion Predicate(s):
  - `block.fields.len() == AUTHORED_FIELD_NAMES.len()` -- exact count (closed-world, no insertions or removals)
  - `extracted == AUTHORED_FIELD_NAMES` -- exact sequence equality (no reordering or renaming)
  - `surface.register_blocks.len() == 1` -- no spurious register blocks
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_7_closed_world_inventory` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - 25 fields extracted, 25 authored; sequence matches exactly; one register block total
- Reasoning Trace:
  - Authored Fact (Verbatim): 25 fields (22 registers + 3 reserved), no extras
  - Extracted Fact (Verbatim): 25 RegisterFieldDecl entries, names match exactly in order, one RegisterBlockDecl total
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Closed-world evidence: exact count + exact sequence + single block proves no insertions, removals, renames, or reorderings.
- Counterexample: none found
- Conclusion: pass

### Check 8
- Status: `fail`
- Premise: no register-block properties are invented or dropped
- Assertion Predicate(s):
  - For all fields: `field.access_width == None`, `field.endian == None`, `field.side_effects == None`, `field.hw_updates == None` -- no properties invented
  - RegisterBlockDecl has no block-level property fields -- authored properties (endian, access_width) are dropped
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_8_property_set_integrity` in `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
  - RegisterBlockDecl struct (line 23-26 of register_access.rs) has only `name` and `fields` -- no property storage
- Reasoning Trace:
  - Authored Fact (Verbatim): property set = {endian: little, access_width: 32}
  - Extracted Fact (Verbatim): RegisterBlockDecl has no endian or access_width; per-field values are all None
  - Correspondence: `absent` for authored properties
  - Gap / Loss Description: No properties are invented (positive). However, both authored properties (endian=little, access_width=32) are dropped because RegisterBlockDecl has no block-level property storage. The premise requires "no properties dropped", which is violated.
  - Why This Status Follows: The authored property set is {endian, access_width}. The extracted property set is {}. Two properties are dropped.
- Counterexample: `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }` -- authored endian and access_width are not present
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Same as Check 3 -- `crates/regseq-driver/src/extract/register_access.rs`, struct `RegisterBlockDecl`
  - Minimal Missing Fact To Preserve: Add block-level `endian: Option<String>` and `access_width: Option<u32>` to RegisterBlockDecl and parse from `#[rseq::register_block(...)]` attribute arguments
- Conclusion: fail

## Summary

| Check | Status | Brief |
|-------|--------|-------|
| 1 | pass | Register block identity preserved as "Pl011Registers" |
| 2 | pass | Canonical path reconstructible from module hierarchy |
| 3 | fail | Block-level endian and access_width dropped at extraction |
| 4 | fail | Reserved region byte offsets not captured |
| 5 | pass | Complete field inventory in authored order |
| 6 | fail | Only struct field name preserved; register type, offset, access kind lost |
| 7 | pass | Closed-world evidence confirms no insertions/removals/renames/reorderings |
| 8 | fail | Authored block-level properties dropped (endian, access_width) |

Result: 4 pass, 4 fail. The extraction boundary preserves structural identity and field inventory but drops register-block properties, byte offsets, register descriptor type names, and access kinds.
