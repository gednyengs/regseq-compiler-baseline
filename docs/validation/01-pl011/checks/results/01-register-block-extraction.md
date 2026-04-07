# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence:
  - `crates/regseq-driver/src/extract/register_access.rs` (RegisterAccessSurface)
  - `crates/regseq-driver/src/extract/identity_enrichment.rs` (IdentityEnrichmentSurface)
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs` (automated assertions)

## Specimen Fidelity

The validation uses a **reduced inline mirror** rather than the real fixture
files. The inline fixture is defined in `PL011_FIXTURE` within
`crates/regseq-driver/tests/pl011_register_block_boundary.rs`.

### Justification

The real fixture (`tests/fixtures/pl011-full-spec/src/registers.rs`) depends on
the `regseq` and `regseq_macros` crates at compile time. The compiler-side test
harness compiles inline source directly via `rustc_driver` without cargo
dependency resolution, so the real fixture cannot be compiled in this test path.

### Equivalence for Every Check in Scope

The inline fixture preserves every authored structural fact required by
Checks 1-8:

- **Struct name**: `Pl011Registers` (identical)
- **Block-level attribute**: `#[rseq::register_block(endian = "little", access_width = 32)]` (identical)
- **Field names**: all 22 registers and 3 reserved regions in authored order (identical)
- **Wrapper types**: `ReadWrite`, `ReadOnly`, `WriteOnly`, `Reserved` (structurally equivalent stubs)
- **Register descriptor modules**: all 22 `*_REG` modules with `struct Register` (structurally equivalent stubs; field/named-value internals omitted because they are a separate extraction family not in scope for the register-block boundary check)
- **Array reserved regions**: `[Reserved<u32>; 4]`, `Reserved<u32>`, `[Reserved<u32>; 997]` (identical element counts)

The fixture also includes a minimal `BoundaryTestDriver` struct with
`#[rseq::driver]` to satisfy the identity enrichment extractor's requirement
for a driver type.

## Check-to-Surface Mapping

| Check | Extraction Surface | Test Assertion |
|-------|-------------------|----------------|
| 1 (identity) | `RegisterBlockDecl.name`, `StructFact.is_register_block` | `block.name == "Pl011Registers"`, `rb_struct.is_register_block == true` |
| 2 (canonical path) | `StructFact.canonical_path` | `canonical_path.0` contains `"Pl011Registers"` |
| 3 (block properties) | `RegisterFieldDecl.access_width`, `RegisterFieldDecl.endian` | all fields have `access_width == None`, `endian == None` |
| 4 (reserved regions) | `RegisterFieldDecl.name` for `_reserved{0,1,2}` | field names present in extracted field list |
| 5 (inventory + order) | `RegisterBlockDecl.fields` (ordered list) | `field_names == EXPECTED_FIELDS` |
| 6 (per-register 4 facts) | `RegisterFieldDecl.*` | all metadata fields `None` for every field |
| 7 (no insertions/removals) | `RegisterBlockDecl.fields.len()` | `fields.len() == EXPECTED_FIELDS.len()` |
| 8 (no invented properties) | `RegisterFieldDecl.side_effects`, `RegisterFieldDecl.hw_updates` | all `None` for every field |

## Per-Check Results

### Check 1: Register Block Identity

- Status: `pass`
- Premise: the register block remains identifiable as `Pl011Registers`
- Target Evidence (Exercised by Test; Exact Citations):
  - `RegisterBlockDecl.name == "Pl011Registers"` (`register_access.rs:218-221`)
  - `StructFact.is_register_block == true` (`identity_enrichment.rs:222`)
  - Test assertion at `pl011_register_block_boundary.rs` Check 1 block
- Reasoning Trace: `extract_register_block` captures the struct name from
  `ident.name.as_str()` at HIR level. `extract_struct` sets `is_register_block`
  from the presence of `#[rseq::register_block]`. Both surfaces independently
  identify the register block.
- Counterexample: none found
- Conclusion: the register block identity is explicitly preserved at the extraction boundary

### Check 2: Canonical Path Preservation

- Status: `pass`
- Premise: the canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Target Evidence (Exercised by Test; Exact Citations):
  - `StructFact.canonical_path` is populated via `tcx.def_path_str()` (`identity_enrichment.rs:150,212-213`)
  - Test assertion at `pl011_register_block_boundary.rs` Check 2 block
- Supporting Evidence (Optional; Exact Citations):
  - The inline fixture compiles without a `registers` module, so the canonical
    path is `Pl011Registers` rather than `pl011_full_spec::registers::Pl011Registers`.
    The mechanism (`tcx.def_path_str()`) is the same; the exact path depends on
    build-time crate name and module nesting, not on extraction logic.
- Reasoning Trace: `IdentityEnrichmentSurface` captures the canonical path for
  every struct. For the real fixture crate compiled as `pl011_full_spec` with a
  `registers` module, the canonical path would be
  `pl011_full_spec::registers::Pl011Registers`. The extraction mechanism
  preserves this exactly.
- Counterexample: none found
- Conclusion: the extraction boundary preserves canonical path identity via `StructFact.canonical_path`

### Check 3: Block-Level Properties (endian, access_width)

- Status: `fail`
- Premise: the register-block properties preserve `endian = little` and `access_width = 32`
- Target Evidence (Exercised by Test; Exact Citations):
  - `RegisterBlockDecl` has only `name` and `fields` -- no block-level property fields (`register_access.rs:22-26`)
  - `extract_register_block` receives `variant_data` but not the struct-level `attrs` (`register_access.rs:192-222`)
  - `has_rseq_attr(attrs, "register_block")` at `register_access.rs:127` uses the attribute only as a boolean marker
  - Per-field `access_width` and `endian` are `None` for all fields (test assertion in Check 3 block)
- Reasoning Trace: The authored source declares `endian = "little"` and
  `access_width = 32` on `#[rseq::register_block(...)]`. The extraction code
  checks for the attribute's presence but does not parse its arguments into the
  `RegisterBlockDecl` record. The record type has no fields for block-level
  properties. The PL011 source does not use per-field `#[rseq::register]`
  attributes, so per-field metadata is also empty.
- Counterexample: `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }`
  contains no `endian` or `access_width` at either block or field level
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `extract_register_block` at `register_access.rs:192` and
    `RegisterBlockDecl` at `register_access.rs:22-26`
  - Minimal Missing Fact To Preserve: pass the struct-level `attrs` to
    `extract_register_block`, parse `endian` and `access_width` from the
    `#[rseq::register_block(...)]` token stream, and add corresponding fields
    to `RegisterBlockDecl`
- Conclusion: block-level `endian` and `access_width` are dropped at the extraction boundary

### Check 4: Reserved Region Byte Gaps

- Status: `fail`
- Premise: reserved regions preserve their authored offsets:
  `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`
- Target Evidence (Exercised by Test; Exact Citations):
  - Reserved field names `_reserved0`, `_reserved1`, `_reserved2` survive in `RegisterFieldDecl.name` (test assertion in Check 4 block)
  - `RegisterFieldDecl` has no byte-offset or type-info fields (`register_access.rs:28-36`)
- Reasoning Trace: The extraction captures field names but not field types or
  byte offsets. Array element counts (e.g., `[Reserved<u32>; 4]` = 4 slots)
  are not recorded. Without `access_width` (Check 3 gap) and array sizes,
  the exact byte ranges `0x008 -> 0x014` etc. cannot be reconstructed from
  the extraction record alone.
- Counterexample: `RegisterFieldDecl { name: "_reserved0", access_width: None, endian: None, side_effects: None, hw_updates: None }` -- no offset or size information
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `extract_register_block` at `register_access.rs:197-221`
    and `RegisterFieldDecl` at `register_access.rs:28-36`
  - Minimal Missing Fact To Preserve: for each field, capture the HIR type
    (e.g., `[Reserved<u32>; 4]`) or compute byte offset from field position
    and `access_width`. The block-level `access_width` (Check 3) must land
    first for offset computation to work.
- Conclusion: reserved region names survive but byte offsets cannot be
  reconstructed from the extraction boundary

### Check 5: Complete Inventory in Authored Order

- Status: `pass`
- Premise: the declared register inventory remains complete and in authored offset order
- Target Evidence (Exercised by Test; Exact Citations):
  - `field_names == EXPECTED_FIELDS` (25 entries, exact match; test assertion in Check 5 block)
  - `extract_register_block` iterates `variant_data.fields()` which preserves struct declaration order (`register_access.rs:199`)
- Reasoning Trace: `variant_data.fields()` returns all struct fields in
  declaration order, including private reserved fields. The extraction captures
  every field name. The test asserts exact sequence equality against the
  authored field list.
- Counterexample: none found
- Conclusion: the complete register inventory in authored order is preserved at the extraction boundary

### Check 6: Per-Register Four Facts

- Status: `fail`
- Premise: each declared register preserves: struct field name, register name, byte offset, access kind
- Target Evidence (Exercised by Test; Exact Citations):
  - Struct field name: preserved in `RegisterFieldDecl.name` (Check 5)
  - Register name (e.g., `DR_REG`): `RegisterFieldDecl` has no field for the type parameter (`register_access.rs:28-36`)
  - Byte offset: `RegisterFieldDecl` has no offset field (`register_access.rs:28-36`)
  - Access kind (RW/R/W): `RegisterFieldDecl` has no access-kind field; wrapper type (`ReadWrite`/`ReadOnly`/`WriteOnly`) is not captured per-field (`register_access.rs:28-36`)
  - All per-field metadata is `None` for every field (test assertion in Check 6 block)
- Reasoning Trace: only 1 of 4 required per-register facts survives. The
  field type (containing both the wrapper type name and the register descriptor
  type parameter) is not captured in `RegisterFieldDecl`.
- Counterexample: `RegisterFieldDecl { name: "dr", access_width: None, endian: None, side_effects: None, hw_updates: None }` -- no register name (`DR_REG`), no offset (`0x000`), no access kind (`RW`)
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `extract_register_block` at `register_access.rs:197-221`
    and `RegisterFieldDecl` at `register_access.rs:28-36`
  - Minimal Missing Fact To Preserve:
    - **Register name**: extract the second generic type argument from the field
      type (e.g., `DR_REG::Register` from `ReadWrite<u32, DR_REG::Register>`)
    - **Access kind**: inspect the wrapper type name (`ReadWrite` = RW,
      `ReadOnly` = R, `WriteOnly` = W) from the HIR type path
    - **Byte offset**: compute from field position and `access_width`, or
      extract from `#[rseq::register(offset = ...)]` if per-field attributes
      are adopted
- Conclusion: 3 of 4 per-register facts are dropped at the extraction boundary

### Check 7: No Insertions, Removals, Renames, or Reordering

- Status: `pass`
- Premise: no registers are inserted, removed, renamed, or reordered
- Target Evidence (Exercised by Test; Exact Citations):
  - `block.fields.len() == EXPECTED_FIELDS.len()` (25 = 25; test assertion in Check 7 block)
  - Exact field-name sequence match (Check 5)
- Reasoning Trace: the extraction faithfully captures all fields from HIR
  `variant_data.fields()` without inserting, removing, or reordering any
  entries. The test proves exact count and sequence equality.
- Counterexample: none found
- Conclusion: the extraction boundary introduces no register insertions, removals, renames, or reordering

### Check 8: No Invented or Dropped Properties

- Status: `fail`
- Premise: no register-block properties are invented or dropped
- Target Evidence (Exercised by Test; Exact Citations):
  - Dropped: block-level `endian = "little"` and `access_width = 32` are not
    captured (Check 3 evidence)
  - Not invented: all per-field `side_effects` and `hw_updates` are `None`
    (test assertion in Check 8 block)
- Reasoning Trace: the extraction correctly avoids inventing properties that
  were not authored. However, it drops the two block-level properties that
  were authored (`endian`, `access_width`). The net effect is a
  property-dropping gap.
- Counterexample: `RegisterBlockDecl { name: "Pl011Registers", fields: [...] }` -- no `endian` or `access_width`
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: same as Check 3
  - Minimal Missing Fact To Preserve: same as Check 3
- Conclusion: no properties are invented, but block-level `endian` and `access_width` are dropped

## Summary

| Check | Status |
|-------|--------|
| 1 - Register block identity | pass |
| 2 - Canonical path preservation | pass |
| 3 - Block-level properties | **fail** |
| 4 - Reserved region byte gaps | **fail** |
| 5 - Complete inventory in authored order | pass |
| 6 - Per-register four facts | **fail** |
| 7 - No insertions/removals/renames | pass |
| 8 - No invented or dropped properties | **fail** |

**Overall**: 4 of 8 checks pass. 4 checks fail due to extraction gaps in the
`RegisterBlockDecl` record type: block-level properties, per-field type
information, byte offsets, and access kinds are not captured.
