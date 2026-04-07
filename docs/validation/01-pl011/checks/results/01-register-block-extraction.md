# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/pl011_register_block_boundary.rs`
- Extraction surfaces inspected:
  - `RegisterAccessSurface` via `extract_register_access_surface(tcx)` (`crates/regseq-driver/src/extract/register_access.rs`)
  - `IdentityEnrichmentSurface` via `extract_identity_enrichment(tcx)` (`crates/regseq-driver/src/extract/identity_enrichment.rs`)

## Specimen Fidelity

The validation uses a reduced inline specimen compiled with `--crate-name pl011_full_spec`
and a `mod registers` containing `Pl011Registers`. The specimen is equivalent to
`tests/fixtures/pl011-full-spec/src/registers.rs` for every authored structural fact
required by checks 1-8:

- All 22 declared registers are present with authored field names, access wrapper types
  (ReadWrite/ReadOnly/WriteOnly), and register descriptor type references (DR_REG::Register etc.).
- All 3 reserved regions are present with authored names and array sizes
  (`[Reserved<u32>; 4]`, `Reserved<u32>`, `[Reserved<u32>; 997]`).
- The `#[rseq::register_block(endian = "little", access_width = 32)]` attribute exactly
  mirrors the fixture.
- Field ordering matches the fixture.
- The `mod registers` module nesting matches the fixture crate structure.
- A minimal `#[rseq::driver]` struct is included to satisfy the `IdentityEnrichmentSurface`
  extraction requirement (no driver type would error).

The specimen does NOT include:
- `register_bitfields!` macro expansion (not needed -- checks only test block structure, not bitfield descriptors).
- `driver.rs` content (out of scope for register-block boundary checks).

This omission does not affect any check in scope because the bitfield descriptors and driver
body are separate extraction families from the register-block declaration surface.

## Check-to-Surface Mapping

| Check | Extraction Surface(s) | Test Function |
|-------|----------------------|---------------|
| 1 | `RegisterAccessSurface.register_blocks[0].name`, `IdentityEnrichmentSurface.structs` (is_register_block) | `check1_register_block_identity` |
| 2 | `IdentityEnrichmentSurface.structs` (canonical_path, module_path), `IdentityEnrichmentSurface.modules[0]` (crate root name) | `check2_canonical_path` |
| 3 | `RegisterAccessSurface.register_blocks[0].fields[*].endian`, `.access_width` | `check3_block_properties_not_extracted` |
| 4 | `RegisterAccessSurface.register_blocks[0].fields` (reserved subset) | `check4_reserved_regions_lack_offset_data` |
| 5 | `RegisterAccessSurface.register_blocks[0].fields` (non-reserved subset, authored order) | `check5_register_inventory_and_order` |
| 6 | `RegisterAccessSurface.register_blocks[0].fields` (per-field metadata) | `check6_per_register_facts_incomplete` |
| 7 | `RegisterAccessSurface.register_blocks[0].fields` (exact count + exhaustive name match) | `check7_closed_world_field_preservation` |
| 8 | `RegisterAccessSurface.register_blocks[0].fields` (block-level property presence) | `check8_block_properties_dropped` |

---

## Check 1

- Check: Register block identity
- Status: `pass`
- Premise: the register block remains identifiable as `Pl011Registers`
- Assertion Predicate(s):
  - `assert_eq!(reg.register_blocks.len(), 1)`
  - `assert_eq!(reg.register_blocks[0].name, "Pl011Registers")`
  - `assert_eq!(rb_struct.name, "Pl011Registers")` where `rb_struct.is_register_block == true`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check1_register_block_identity`
  - `RegisterBlockDecl.name = "Pl011Registers"` (from `RegisterAccessSurface`)
  - `StructFact.name = "Pl011Registers"` with `is_register_block = true` (from `IdentityEnrichmentSurface`)
- Reasoning Trace:
  - Authored Fact (Verbatim): `Pl011Registers`
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", .. }` and `StructFact { name: "Pl011Registers", is_register_block: true, .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Both extraction surfaces independently preserve the register-block identity by name and by structural marker.
- Counterexample: none found
- Conclusion: The register block is identifiable as `Pl011Registers` at the extraction boundary.

---

## Check 2

- Check: Canonical path preservation
- Status: `pass`
- Premise: the canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Assertion Predicate(s):
  - `assert_eq!(path, "registers::Pl011Registers")` (source-local canonical path)
  - `assert_eq!(rb_struct.module_path.0, "registers")`
  - `assert_eq!(crate_root.name, "pl011_full_spec")`
  - `assert_eq!(reconstructed, "pl011_full_spec::registers::Pl011Registers")`
    where `reconstructed = format!("{}::{}", crate_root.name, path)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check2_canonical_path`
  - `StructFact.canonical_path = CanonicalPath("registers::Pl011Registers")`
  - `StructFact.module_path = CanonicalPath("registers")`
  - `ModuleFact[0] (crate root).name = "pl011_full_spec"`
- Supporting Evidence (Optional):
  - `tcx.def_path_str()` returns the source-local path without crate prefix for local DefIds.
    The crate root module name is derived from `tcx.crate_name(LOCAL_CRATE)`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::registers::Pl011Registers`
  - Extracted Fact (Verbatim): source-local path `registers::Pl011Registers` + crate root name `pl011_full_spec`
  - Correspondence: `exact` (via reconstruction)
  - Gap / Loss Description: The canonical path is not stored as a single fully-qualified string. It requires combining the crate root name with the source-local path.
  - Why This Status Follows: The boundary state preserves enough path evidence to reconstruct the full canonical path exactly without heuristic guesswork. The reconstruction is `crate_root.name + "::" + canonical_path = "pl011_full_spec::registers::Pl011Registers"`.
- Counterexample: none found
- Conclusion: The canonical path is exactly reconstructible from the extraction boundary state.

---

## Check 3

- Check: Register-block properties (endian, access_width)
- Status: `fail`
- Premise: the register-block properties preserve `endian = little` and `access_width = 32`
- Assertion Predicate(s):
  - `assert_eq!(field.endian, None)` for every field in the block
  - `assert_eq!(field.access_width, None)` for every field in the block
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check3_block_properties_not_extracted`
  - All 25 fields have `endian: None` and `access_width: None`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::register_block(endian = "little", access_width = 32)]` on `Pl011Registers` struct
  - Extracted Fact (Verbatim): `RegisterBlockDecl { name: "Pl011Registers", fields: [RegisterFieldDecl { endian: None, access_width: None, .. }, ..] }`
  - Correspondence: `absent`
  - Gap / Loss Description: The extraction detects the presence of `#[rseq::register_block]` via `has_rseq_attr(attrs, "register_block")` but does NOT parse the attribute's arguments. The `endian` and `access_width` parsing logic exists only in `parse_register_metadata()`, which is called for per-field `#[rseq::register(...)]` attributes, not for the struct-level `#[rseq::register_block(...)]` attribute. PL011 fields have no per-field `#[rseq::register]` attributes.
  - Why This Status Follows: Both authored properties are entirely absent from the extraction boundary. There is no alternative extraction surface that preserves them.
- Counterexample: `RegisterBlockDecl` has only `name: String` and `fields: Vec<RegisterFieldDecl>` at the block level. There is no `endian` or `access_width` field on `RegisterBlockDecl`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, function `extract_register_block` (line ~192) and the calling site in `extract_register_access_surface` (line ~126).
  - Minimal Missing Fact To Preserve: Parse the `#[rseq::register_block(...)]` attribute arguments and add `endian: Option<String>` and `access_width: Option<u32>` fields to `RegisterBlockDecl`.
- Conclusion: Both `endian` and `access_width` block-level properties are dropped at the extraction boundary.

---

## Check 4

- Check: Reserved region offset preservation
- Status: `fail`
- Premise: the reserved regions preserve their authored offsets: `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
- Assertion Predicate(s):
  - `assert_eq!(reserved_names, vec!["_reserved0", "_reserved1", "_reserved2"])` (names present in order)
  - `assert_eq!(field.access_width, None)` / `assert_eq!(field.endian, None)` / `assert_eq!(field.side_effects, None)` / `assert_eq!(field.hw_updates, None)` for each reserved field (no layout metadata)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check4_reserved_regions_lack_offset_data`
  - Reserved field names are present: `["_reserved0", "_reserved1", "_reserved2"]`
  - No metadata fields carry offset, array-size, or type identity
- Supporting Evidence (Optional):
  - `RegisterFieldDecl` has exactly five fields: `name`, `access_width`, `endian`, `side_effects`, `hw_updates`. None represent byte offset, array size, or type identity.
- Reasoning Trace:
  - Authored Fact (Verbatim): `_reserved0: [Reserved<u32>; 4]` at 0x008..0x014, `_reserved1: Reserved<u32>` at 0x01C, `_reserved2: [Reserved<u32>; 997]` at 0x04C..0xFDC
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "_reserved0", access_width: None, endian: None, side_effects: None, hw_updates: None }` (similarly for _reserved1, _reserved2)
  - Correspondence: `weakened` -- names and ordering survive, but byte offsets, array sizes, and type information do not
  - Gap / Loss Description: The extraction captures reserved field names and their position in the field sequence, but carries no byte-offset, array-size, or type-identity data. The exact byte ranges (0x008..0x014, 0x01C..0x01C, 0x04C..0xFDC) cannot be reconstructed from the extraction surface alone.
  - Why This Status Follows: The check requires preserving authored offsets or enough layout data to reconstruct the exact gaps. The extraction preserves neither.
- Counterexample: `_reserved0` has array size 4 in the authored source (spanning 4*4=16 bytes from 0x008 to 0x017 inclusive, mapping to the 0x008->0x014 gap). This array size is not present in `RegisterFieldDecl`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, function `extract_register_block` (line ~192), field iteration loop.
  - Minimal Missing Fact To Preserve: Add byte-offset (computed from `repr(C)` layout or from the struct field ordering + type sizes) and array-size / type-identity to `RegisterFieldDecl`.
- Conclusion: Reserved region byte ranges are not reconstructible from the extraction boundary.

---

## Check 5

- Check: Register inventory completeness and ordering
- Status: `pass`
- Premise: the declared register inventory remains complete and in authored offset order
- Assertion Predicate(s):
  - `assert_eq!(extracted_registers.as_slice(), AUTHORED_REGISTER_NAMES)` where `extracted_registers` is the non-reserved field names in extraction order and `AUTHORED_REGISTER_NAMES` is the 22-element authored register name list in offset order
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check5_register_inventory_and_order`
  - Extracted register names: `["dr", "rsr_ecr", "fr", "ilpr", "ibrd", "fbrd", "lcr_h", "cr", "ifls", "imsc", "ris", "mis", "icr", "dmacr", "periph_id0", "periph_id1", "periph_id2", "periph_id3", "pcell_id0", "pcell_id1", "pcell_id2", "pcell_id3"]`
- Supporting Evidence (Optional):
  - Per the boundary expectations: "a complete extracted field sequence in authored order is sufficient for the inventory/order checks even if per-field offset and access metadata is not preserved."
- Reasoning Trace:
  - Authored Fact (Verbatim): 22 declared registers in offset order from `dr` (0x000) to `pcell_id3` (0xFFC)
  - Extracted Fact (Verbatim): 22 non-reserved field names in the same order
  - Correspondence: `exact` (names and ordering match)
  - Gap / Loss Description: Per-field byte offsets are absent, but the boundary expectations explicitly accept field-sequence ordering as sufficient.
  - Why This Status Follows: The extracted field sequence exactly matches the authored 22-register inventory in offset order. The assertion would fail if any register were missing, added, or reordered.
- Counterexample: none found
- Conclusion: The register inventory is complete and in authored offset order at the extraction boundary.

---

## Check 6

- Check: Per-register four-fact preservation (field name, register name, byte offset, access kind)
- Status: `fail`
- Premise: each declared register preserves struct field name, register name, byte offset, and access kind
- Assertion Predicate(s):
  - `assert_eq!(extracted_names.as_slice(), AUTHORED_REGISTER_NAMES)` (field names preserved)
  - `assert_eq!(field.access_width, None)` and `assert_eq!(field.endian, None)` for each register field (demonstrating absence of per-field metadata)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check6_per_register_facts_incomplete`
  - Struct field names: all 22 match authored names (preserved)
  - Register names (e.g. `DR_REG`): not present in `RegisterFieldDecl` -- the type parameter carrying the descriptor identity is not captured
  - Byte offsets: no offset field exists on `RegisterFieldDecl`
  - Access kind (ReadWrite/ReadOnly/WriteOnly): no access-kind field exists on `RegisterFieldDecl`
- Reasoning Trace:
  - Authored Fact (Verbatim): e.g. `dr: DR_REG @ 0x000 [RW]` -- field name `dr`, register name `DR_REG`, offset 0x000, access RW
  - Extracted Fact (Verbatim): `RegisterFieldDecl { name: "dr", access_width: None, endian: None, side_effects: None, hw_updates: None }`
  - Correspondence: `weakened` -- only 1 of 4 required facts is preserved (struct field name)
  - Gap / Loss Description: The extraction does not capture the type-parameter register descriptor identity (`DR_REG`), the byte offset, or the access wrapper kind (ReadWrite vs ReadOnly vs WriteOnly). `RegisterDescriptorFact` captures descriptor modules separately but provides no cross-reference linking a field to its descriptor.
  - Why This Status Follows: The check requires all four facts per register. Three of four are entirely absent. Partial preservation cannot satisfy the full premise.
- Counterexample: Field `fr` is authored as `ReadOnly<u32, FR_REG::Register>` at offset 0x018. The extraction yields `RegisterFieldDecl { name: "fr", access_width: None, endian: None, .. }` with no trace of ReadOnly, FR_REG, or 0x018.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`, function `extract_register_block` (line ~192).
  - Minimal Missing Fact To Preserve:
    - Add `register_type: Option<String>` to `RegisterFieldDecl` (extracted from the second type parameter of the field's wrapper type, e.g. `DR_REG::Register` -> `"DR_REG"`).
    - Add `byte_offset: Option<u32>` (computed from `repr(C)` layout or field-position arithmetic).
    - Add `access_kind: Option<AccessKind>` where `AccessKind` = `ReadWrite` | `ReadOnly` | `WriteOnly` (extracted from the outer type name of the field).
- Conclusion: Only struct field names are preserved; register names, byte offsets, and access kinds are absent.

---

## Check 7

- Check: No register insertion, removal, renaming, or reordering
- Status: `pass`
- Premise: no registers are inserted, removed, renamed, or reordered
- Assertion Predicate(s):
  - `assert_eq!(block.fields.len(), AUTHORED_ALL_FIELD_NAMES.len())` (exact count: 25)
  - `assert_eq!(extracted_all.as_slice(), AUTHORED_ALL_FIELD_NAMES)` (exhaustive name-by-name match in authored order)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check7_closed_world_field_preservation`
  - Extracted all 25 field names in order match the authored 25-element list exactly
- Reasoning Trace:
  - Authored Fact (Verbatim): 25 struct fields (22 registers + 3 reserved) in exact authored order
  - Extracted Fact (Verbatim): 25 `RegisterFieldDecl` entries with names `["dr", "rsr_ecr", "_reserved0", "fr", "_reserved1", ..., "pcell_id3"]`
  - Correspondence: `exact`
  - Gap / Loss Description: none -- the complete field set matches exactly
  - Why This Status Follows: Exact field count plus exhaustive ordered name match constitutes closed-world evidence. If any register were inserted, removed, renamed, or reordered, the assertion would fail.
- Counterexample: none found
- Conclusion: The extraction boundary introduces no extra fields and loses none of the authored fields.

---

## Check 8

- Check: No register-block properties invented or dropped
- Status: `fail`
- Premise: no register-block properties are invented or dropped
- Assertion Predicate(s):
  - `assert!(!any_endian)` -- no field carries an endian value
  - `assert!(!any_access_width)` -- no field carries an access_width value
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs::check8_block_properties_dropped`
  - All 25 fields have `endian: None` and `access_width: None`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::register_block(endian = "little", access_width = 32)]`
  - Extracted Fact (Verbatim): `RegisterBlockDecl` has no block-level property fields. All per-field `endian` and `access_width` are `None`.
  - Correspondence: `absent` -- both authored properties are dropped
  - Gap / Loss Description: Same root cause as Check 3. The `#[rseq::register_block(...)]` attribute arguments are not parsed by `extract_register_block`. No properties are invented either.
  - Why This Status Follows: The authored property set is `{endian, access_width}`. The extracted property set is `{}`. Two properties are dropped, none are invented.
- Counterexample: `RegisterBlockDecl` struct definition (`crates/regseq-driver/src/extract/register_access.rs:23-26`) has only `name: String` and `fields: Vec<RegisterFieldDecl>` -- no property fields exist.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Same as Check 3 -- `crates/regseq-driver/src/extract/register_access.rs`, `extract_register_block` and `extract_register_access_surface`.
  - Minimal Missing Fact To Preserve: Parse the `#[rseq::register_block(...)]` attribute arguments and add `endian: Option<String>` and `access_width: Option<u32>` to `RegisterBlockDecl`.
- Conclusion: Both authored register-block properties (`endian`, `access_width`) are dropped at the extraction boundary.

---

## Summary

| Check | Status | Description |
|-------|--------|-------------|
| 1 | `pass` | Register block identity preserved as `Pl011Registers` |
| 2 | `pass` | Canonical path exactly reconstructible from source-local path + crate root name |
| 3 | `fail` | `endian` and `access_width` block-level properties not extracted |
| 4 | `fail` | Reserved region byte offsets not preserved or reconstructible |
| 5 | `pass` | Register inventory complete and in authored offset order |
| 6 | `fail` | Only struct field name preserved; register name, byte offset, access kind absent |
| 7 | `pass` | No insertion, removal, renaming, or reordering (closed-world evidence) |
| 8 | `fail` | Both authored block-level properties dropped |

4 of 8 checks pass. 4 checks fail due to two related extraction gaps:

1. **Block-level property gap** (Checks 3, 8): The `#[rseq::register_block(...)]` attribute arguments are detected for presence but not parsed.
2. **Per-field structural gap** (Checks 4, 6): `RegisterFieldDecl` captures only the field name and optional per-field `#[rseq::register]` metadata. It does not capture byte offset, array size, type identity (register descriptor name), or access wrapper kind (ReadWrite/ReadOnly/WriteOnly).

Test command: `cargo test -p regseq-driver --test pl011_register_block_boundary`
