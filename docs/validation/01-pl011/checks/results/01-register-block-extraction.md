# Validation Certificate: PL011 Register-Block Extraction Check

Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
Target boundary evidence: `crates/regseq-driver/src/extract/register_access.rs`, `crates/regseq-driver/src/extract/identity_enrichment.rs`

## Specimen Fidelity

The validation uses an inline specimen embedded in
`crates/regseq-driver/tests/pl011_register_block_boundary.rs` rather than the
real fixture files. The real fixture depends on `regseq` and `regseq_macros`
crates at compile time, which the extraction test harness cannot provide.

The inline specimen preserves every authored structural fact required by the
checks:

- Struct name `Pl011Registers` in `mod registers` block
- `#[rseq::register_block(endian = "little", access_width = 32)]` attribute
- All 25 fields (22 declared registers + 3 reserved) in exact authored order
- Field access wrapper types (`ReadWrite`, `ReadOnly`, `WriteOnly`, `Reserved`)
  with matching type parameters
- Reserved region array sizes (`[Reserved<u32>; 4]`, `Reserved<u32>`,
  `[Reserved<u32>; 997]`)
- Compiled with `--crate-name pl011_full_spec` to match the authored canonical
  path prefix

The specimen omits register-bitfield descriptor internals (field offsets, named
values) because no check in scope inspects sub-register descriptor facts.

## Check-to-Surface Mapping

| Check | Extraction Surface                  | Test Assertion(s)                                       |
|-------|-------------------------------------|---------------------------------------------------------|
| 1     | `RegisterBlockDecl.name`            | `block.name == "Pl011Registers"`                        |
| 2     | `StructFact.canonical_path`, `StructFact.module_path`, `ModuleFact.canonical_path` | Struct path + crate root = `pl011_full_spec::registers::Pl011Registers` |
| 3     | `RegisterBlockDecl` (no block-level property fields), `RegisterFieldDecl.endian/access_width` | All fields have `None` for endian and access_width |
| 4     | `RegisterFieldDecl.name`            | Reserved names present; no offset/type/size metadata    |
| 5     | `RegisterFieldDecl` sequence        | Extracted field names == authored field order            |
| 6     | `RegisterFieldDecl`                 | Field names match; endian/access_width/side_effects/hw_updates all `None` |
| 7     | `RegisterFieldDecl` sequence, `RegisterBlockDecl` count | Exact field list match, single block |
| 8     | `RegisterBlockDecl` fields only     | No endian/access_width metadata present anywhere        |

---

## Check 1

- Check: Register block identity preserved
- Status: `pass`
- Premise: The register block remains identifiable as `Pl011Registers`.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check1_register_block_identity` asserts `surface.register_blocks[0].name == "Pl011Registers"`
  - `crates/regseq-driver/src/extract/register_access.rs:218`:
    `RegisterBlockDecl { name: name.to_string(), .. }` captures the struct ident
    from HIR
- Reasoning Trace:
  The extraction code in `extract_register_block()` copies the struct identifier
  name directly from HIR. The test compiles the inline specimen and verifies
  the extracted block name matches exactly.
- Counterexample: None found. The extraction faithfully copies the struct
  identifier.
- Conclusion: PASS. Block identity is explicitly preserved as `Pl011Registers`.

---

## Check 2

- Check: Canonical path preserved
- Status: `pass`
- Premise: The canonical path remains
  `pl011_full_spec::registers::Pl011Registers`.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check2_canonical_path` asserts:
    - `reg_block_struct.canonical_path.0 == "registers::Pl011Registers"`
    - `reg_block_struct.module_path.0 == "registers"`
    - `crate_root.canonical_path.0 == "pl011_full_spec"`
    - Reconstructed path equals `"pl011_full_spec::registers::Pl011Registers"`
  - `crates/regseq-driver/src/extract/identity_enrichment.rs:63`:
    `CanonicalPath(tcx.def_path_str(item.owner_id.def_id.to_def_id()))` captures
    the compiler-resolved canonical path
- Supporting Evidence:
  - `StructFact.is_register_block == true` confirms the struct is identified as
    a register block in the identity enrichment surface
- Reasoning Trace:
  `def_path_str()` produces crate-relative paths for local items (e.g.,
  `registers::Pl011Registers`). The crate root module separately preserves the
  crate name (`pl011_full_spec`). The full canonical path is mechanically
  reconstructable: `crate_root.canonical_path + "::" + struct.canonical_path`.
  The test exercises this reconstruction and asserts it matches the authored
  premise exactly.
- Counterexample: None found. The two-part representation is exactly
  reconstructable.
- Conclusion: PASS. The full canonical path
  `pl011_full_spec::registers::Pl011Registers` is exactly reconstructable from
  the extraction evidence.

---

## Check 3

- Check: Block-level properties preserved (endian = little, access_width = 32)
- Status: `fail`
- Premise: The register-block properties preserve `endian = little` and
  `access_width = 32`.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check3_block_properties_not_extracted` asserts all fields have
    `endian == None` and `access_width == None`
  - `crates/regseq-driver/src/extract/register_access.rs:126-128`:
    `has_rseq_attr(attrs, "register_block")` only checks attribute existence,
    never parses arguments
  - `crates/regseq-driver/src/extract/register_access.rs:192-222`:
    `extract_register_block()` receives `variant_data` but NOT the struct-level
    `attrs`, so the `register_block` attribute arguments (`endian`, `access_width`)
    are never read
- Reasoning Trace:
  The `RegisterBlockDecl` struct has only `name: String` and
  `fields: Vec<RegisterFieldDecl>`. There is no field for block-level `endian`
  or `access_width`. The extraction function `extract_register_block()` is not
  passed the struct-level attributes, so it cannot parse the `register_block`
  attribute's arguments. Per-field `#[rseq::register()]` metadata is a separate
  mechanism that the authored source does not use. The block-level properties are
  silently dropped.
- Counterexample:
  The authored source declares
  `#[rseq::register_block(endian = "little", access_width = 32)]` on
  `Pl011Registers`. After extraction, `RegisterBlockDecl` has no endian or
  access_width representation. These properties are irretrievably lost at this
  boundary.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`,
    `RegisterBlockDecl` struct and `extract_register_block()` function
  - Minimal Missing Fact To Preserve: Add `endian: Option<String>` and
    `access_width: Option<u32>` fields to `RegisterBlockDecl`. Pass the
    struct-level `attrs` into `extract_register_block()` and parse the
    `register_block` attribute arguments using the existing token-parsing
    helpers (same pattern as `parse_register_metadata`).
- Conclusion: FAIL. Block-level endian and access_width are dropped because the
  extraction never parses the `#[rseq::register_block(...)]` attribute arguments.

---

## Check 4

- Check: Reserved region offset preservation
- Status: `fail`
- Premise: The reserved regions preserve their authored offsets:
  `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`,
  `_reserved2: 0x04C -> 0xFDC`.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check4_reserved_regions_names_present_offsets_absent` asserts the three
    reserved field names exist in order
  - `crates/regseq-driver/src/extract/register_access.rs:29-36`:
    `RegisterFieldDecl` carries `name`, `access_width`, `endian`,
    `side_effects`, `hw_updates` -- no byte offset, no type information, no
    array length
- Reasoning Trace:
  The reserved field names (`_reserved0`, `_reserved1`, `_reserved2`) are
  captured. However, `RegisterFieldDecl` has no byte-offset field, no type
  representation, and no array-length metadata. Without this data, the
  extraction cannot distinguish `_reserved0: [Reserved<u32>; 4]` (covering
  `0x008..0x014`) from a single `Reserved<u32>` field. The authored byte ranges
  are irretrievably lost.
- Counterexample:
  `_reserved0` is an array of 4 elements spanning `0x008..0x014`, but the
  extraction records only the name `_reserved0` with no size or offset data.
  The range `0x008 -> 0x014` cannot be reconstructed.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`,
    `RegisterFieldDecl` and `extract_register_block()`
  - Minimal Missing Fact To Preserve: Add `byte_offset: Option<u64>` and either
    `field_type_repr: Option<String>` or `element_count: Option<usize>` to
    `RegisterFieldDecl`. Compute byte offsets from struct layout via
    `tcx.layout_of()` or from `#[repr(C)]` field ordering with known access
    widths.
- Conclusion: FAIL. Reserved field names survive but their byte-range
  information is lost.

---

## Check 5

- Check: Declared register inventory complete and in authored offset order
- Status: `pass`
- Premise: The declared register inventory remains complete and in authored
  offset order.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check5_inventory_complete_and_ordered` asserts extracted field names ==
    `AUTHORED_ALL_FIELDS` (25 entries in exact authored order)
  - `crates/regseq-driver/src/extract/register_access.rs:199`:
    `for field in variant_data.fields()` iterates HIR fields in declaration
    order
- Reasoning Trace:
  The HIR preserves struct fields in declaration order, which matches the
  authored offset order for `#[repr(C)]` structs. The test asserts all 25
  fields (22 declared registers + 3 reserved) match the authored sequence
  exactly.
- Counterexample: None found.
- Conclusion: PASS. The complete field inventory is preserved in authored
  declaration/offset order.

---

## Check 6

- Check: Per-register four-fact preservation (field name, register name, byte
  offset, access kind)
- Status: `fail`
- Premise: Each declared register preserves: struct field name, register name,
  byte offset, access kind.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check6_per_register_facts` asserts declared register field names match
    `AUTHORED_DECLARED_REGISTERS`, and verifies all per-field metadata is `None`
  - `crates/regseq-driver/src/extract/register_access.rs:29-36`:
    `RegisterFieldDecl { name, access_width, endian, side_effects, hw_updates }`
    -- no `register_name`, `byte_offset`, or `access_kind` fields
- Reasoning Trace:
  Of the four authored per-register facts:
  1. **Struct field name**: PRESERVED (e.g., `dr`, `fr`, `icr`)
  2. **Register name** (e.g., `DR_REG`): NOT PRESERVED. The type parameter
     linking `ReadWrite<u32, DR_REG::Register>` to the `DR_REG` descriptor
     module is not captured in `RegisterFieldDecl`. Neither
     `RegisterAccessSurface` nor `IdentityEnrichmentSurface` records per-field
     type parameters.
  3. **Byte offset** (e.g., `0x000`): NOT PRESERVED. `RegisterFieldDecl` has no
     offset field.
  4. **Access kind** (e.g., `RW`, `R`, `W`): NOT PRESERVED. The wrapper type
     (`ReadWrite`/`ReadOnly`/`WriteOnly`) is not captured in
     `RegisterFieldDecl`.
- Counterexample:
  The authored register `icr: WriteOnly<u32, ICR_REG::Register>` at offset
  `0x044` with access kind `W` is extracted as
  `RegisterFieldDecl { name: "icr", access_width: None, endian: None, side_effects: None, hw_updates: None }`.
  Three of the four authored facts (register name `ICR_REG`, offset `0x044`,
  access kind `W`) are absent.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs`,
    `RegisterFieldDecl` and `extract_register_block()`
  - Minimal Missing Fact To Preserve:
    - `access_kind: Option<String>` from the wrapper type name (`ReadWrite` ->
      `RW`, `ReadOnly` -> `R`, `WriteOnly` -> `W`) via `field.ty` HIR type
      inspection
    - `register_type_name: Option<String>` from the second generic type argument
      via `extract_first_generic_arg_name()` variant that reads type params
    - `byte_offset: Option<u64>` from layout computation or `#[repr(C)]`
      sequential offset tracking
- Conclusion: FAIL. Only struct field name (1 of 4 facts) is preserved.
  Register name, byte offset, and access kind are dropped.

---

## Check 7

- Check: No registers inserted, removed, renamed, or reordered
- Status: `pass`
- Premise: No registers are inserted, removed, renamed, or reordered.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check7_no_spurious_changes` asserts:
    - extracted field names == `AUTHORED_ALL_FIELDS` exactly
    - `surface.register_blocks.len() == 1`
  - `crates/regseq-driver/src/extract/register_access.rs:199-216`:
    `extract_register_block()` iterates all `variant_data.fields()` without
    filtering, reordering, or renaming
- Reasoning Trace:
  The extraction copies all struct fields from HIR in declaration order without
  mutation. The test proves the extracted sequence matches the authored sequence
  exactly (25 fields, same names, same order). No extra register blocks are
  created.
- Counterexample: None found.
- Conclusion: PASS. The extraction introduces no insertions, removals, renames,
  or reorderings.

---

## Check 8

- Check: No register-block properties invented or dropped
- Status: `fail`
- Premise: No register-block properties are invented or dropped.
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs`:
    `check8_block_properties_dropped` asserts no field carries endian or
    access_width metadata
  - `crates/regseq-driver/src/extract/register_access.rs:22-25`:
    `RegisterBlockDecl` struct has only `name` and `fields` -- no block-level
    property fields
  - `crates/regseq-driver/src/extract/register_access.rs:126`:
    `has_rseq_attr(attrs, "register_block")` discards attribute arguments
- Reasoning Trace:
  The authored source declares `endian = "little"` and `access_width = 32` via
  `#[rseq::register_block(...)]`. The extraction checks for attribute existence
  only and never parses the arguments. `RegisterBlockDecl` has no representation
  for block-level properties. Both authored properties are dropped. No
  properties are invented.
- Counterexample:
  `endian = "little"` and `access_width = 32` are authored but absent from
  `RegisterBlockDecl`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Same as Check 3
  - Minimal Missing Fact To Preserve: Same as Check 3
- Conclusion: FAIL. Two authored block-level properties are dropped.

---

## Summary

| Check | Status    | Fact(s) Preserved               | Fact(s) Dropped                          |
|-------|-----------|---------------------------------|------------------------------------------|
| 1     | `pass`    | block name                      | --                                       |
| 2     | `pass`    | canonical path (reconstructable)| --                                       |
| 3     | `fail`    | --                              | endian, access_width                     |
| 4     | `fail`    | reserved field names            | byte offsets, array sizes                |
| 5     | `pass`    | complete ordered field inventory| --                                       |
| 6     | `fail`    | field names (1/4)               | register name, byte offset, access kind  |
| 7     | `pass`    | no spurious mutations           | --                                       |
| 8     | `fail`    | no invented properties          | endian, access_width dropped             |

Overall: **4 pass, 4 fail**

The failing checks share a common root cause: `extract_register_block()` in
`register_access.rs` captures only field names and optional per-field
`#[rseq::register()]` metadata. It does not parse the `#[rseq::register_block]`
attribute arguments and does not extract per-field type information (access
wrapper kind, register type parameter, byte offset).
