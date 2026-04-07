# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence:
  - `crates/regseq-driver/src/extract/register_access.rs` (register access surface extraction)
  - `crates/regseq-driver/src/extract/identity_enrichment.rs` (identity enrichment extraction)
  - `crates/regseq-driver/src/extract/identity_enrichment_types.rs` (enrichment types)
  - `crates/regseq-driver/tests/pl011_register_block_extraction.rs` (programmatic validation)

## Summary

3 of 8 checks pass. 5 checks fail. 0 unclear.

The register-block struct identity, field inventory, and ordering survive the
extraction boundary. Block-level properties (`endian`, `access_width`),
canonical paths with crate prefix, byte offsets, register type names, access
kinds, and reserved-region layout do not survive.

---

## Check 1

- Status: **pass**
- Premise: the register block remains identifiable as `Pl011Registers`
- Target Evidence: `IdentityEnrichmentSurface.structs` contains a `StructFact`
  with `name = "Pl011Registers"` and `is_register_block = true`.
  `RegisterAccessSurface.register_blocks` also contains a `RegisterBlockDecl`
  with `name = "Pl011Registers"`.
  (`crates/regseq-driver/src/extract/identity_enrichment_types.rs:69-82`,
  `crates/regseq-driver/src/extract/register_access.rs:22-26`)
- Reasoning Trace: both extraction families independently preserve the struct
  name. The identity enrichment surface flags it as `is_register_block = true`
  via the `#[rseq::register_block]` attribute. The register access surface
  preserves it via the same attribute detection. Identity is explicit, not
  inferred.
- Counterexample: none found
- Conclusion: the register block identity survives at this boundary.

## Check 2

- Status: **fail**
- Premise: the canonical path remains `pl011_full_spec::registers::Pl011Registers`
- Target Evidence: `IdentityEnrichmentSurface.structs[Pl011Registers].canonical_path`
  equals `registers::Pl011Registers` (source-local, no crate prefix).
  (`crates/regseq-driver/src/extract/identity_enrichment.rs:63` -- `tcx.def_path_str(...)`)
- Reasoning Trace: the extraction uses `tcx.def_path_str(DefId)` to derive
  canonical paths. On the pinned toolchain (nightly-2026-02-01), this API returns
  paths without the crate name prefix for local items. The crate name
  `pl011_full_spec` is separately available in `crate_name` on the root
  `ModuleFact`, but it is not joined into the struct's `canonical_path`.
- Counterexample: authored canonical path is
  `pl011_full_spec::registers::Pl011Registers`; boundary produces
  `registers::Pl011Registers`.
- Suggested Fix Direction / Implementation Hints: in
  `extract_identity_enrichment()`, prepend the crate name to the path returned
  by `def_path_str()` for local items, producing
  `{crate_name}::{def_path_str_result}`. This is a narrow change in the
  identity enrichment extractor.
- Conclusion: the boundary preserves only the source-local path, not the full
  canonical path. Per the check specification, source-local path preservation
  alone does not count as pass.

## Check 3

- Status: **fail**
- Premise: the register-block properties preserve `endian = little` and
  `access_width = 32`
- Target Evidence: `RegisterBlockDecl` has fields `name` and `fields` only
  (`crates/regseq-driver/src/extract/register_access.rs:22-26`). There is no
  block-level `endian` or `access_width` field. The extraction function
  `extract_register_block()` checks for `has_rseq_attr(attrs, "register_block")`
  to detect the struct, but does not parse the attribute arguments
  (`crates/regseq-driver/src/extract/register_access.rs:126-128`).
  Per-field `RegisterFieldDecl` carries `access_width` and `endian` from
  `#[rseq::register(...)]`, but no PL011 fields carry that attribute --
  the properties are on the struct-level `#[rseq::register_block(...)]`.
- Reasoning Trace: the authored attribute
  `#[rseq::register_block(endian = "little", access_width = 32)]` is detected
  only as a boolean flag (`is_register_block`). Its key-value arguments are
  never parsed into the `RegisterBlockDecl` record.
- Counterexample: authored `endian = "little"` and `access_width = 32` on
  `Pl011Registers`; boundary `RegisterBlockDecl` has no `endian` or
  `access_width` fields.
- Suggested Fix Direction / Implementation Hints: add `endian: Option<String>`
  and `access_width: Option<u32>` to `RegisterBlockDecl`. In
  `extract_register_block()`, call the existing `parse_register_metadata()`
  (or a variant) on the struct-level `#[rseq::register_block(...)]` attribute
  to populate those fields. The argument format is the same key-value syntax
  already handled by `parse_register_metadata()`.
- Conclusion: the authored register-block properties do not survive at this
  boundary.

## Check 4

- Status: **fail**
- Premise: the reserved regions preserve their authored offsets:
  `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`,
  `_reserved2: 0x04C -> 0xFDC`
- Target Evidence: the identity enrichment surface preserves reserved field
  names (`_reserved0`, `_reserved1`, `_reserved2`) in the field sequence at
  their authored positions. However, neither `FieldFact` nor
  `RegisterFieldDecl` carries byte offset, array extent, or gap-size data.
  (`crates/regseq-driver/src/extract/identity_enrichment_types.rs:86-93`,
  `crates/regseq-driver/src/extract/register_access.rs:28-36`)
- Reasoning Trace: the field names survive and their relative position in the
  sequence is correct. But the check specification requires "enough
  register-block layout information to reconstruct those gaps exactly."
  Without byte offsets, field sizes, or array counts, the reserved-region
  boundaries cannot be reconstructed.
- Counterexample: `_reserved0` is `[Reserved<u32>; 4]` covering `0x008..0x014`
  (4 elements x 4 bytes); the boundary preserves only the name `_reserved0`
  with no size or offset evidence.
- Suggested Fix Direction / Implementation Hints: extend the field extraction
  (in both `RegisterFieldDecl` and/or `FieldFact`) with `byte_offset: Option<u64>`
  and `element_count: Option<u32>`. For `#[repr(C)]` structs, these can be
  computed from `tcx.layout_of()` or by inspecting field types for array
  lengths. The byte offset of each field can be derived from the struct layout.
- Conclusion: reserved-region boundaries are not reconstructable at this
  boundary.

## Check 5

- Status: **pass**
- Premise: the declared register inventory remains complete and in authored
  offset order
- Target Evidence: the extracted field sequence from
  `IdentityEnrichmentSurface.structs[Pl011Registers].fields` is:
  `["dr", "rsr_ecr", "_reserved0", "fr", "_reserved1", "ilpr", "ibrd",
  "fbrd", "lcr_h", "cr", "ifls", "imsc", "ris", "mis", "icr", "dmacr",
  "_reserved2", "periph_id0", "periph_id1", "periph_id2", "periph_id3",
  "pcell_id0", "pcell_id1", "pcell_id2", "pcell_id3"]`
  This matches the 25-element authored field sequence exactly.
- Reasoning Trace: the check specification notes that "a complete extracted
  field sequence in authored order is sufficient for the inventory/order
  checks even if per-field offset and access metadata is not preserved."
  The sequence is complete and in the correct authored order.
- Counterexample: none found
- Conclusion: the register inventory and ordering survive at this boundary.

## Check 6

- Status: **fail**
- Premise: each declared register preserves struct field name, register name,
  byte offset, and access kind
- Target Evidence: `FieldFact` preserves `name` (struct field name) and
  `rseq_kind` (resource classification if present). It does not carry register
  type name, byte offset, or access kind. `RegisterFieldDecl` preserves
  `name` only for fields within a `#[rseq::register_block]`-annotated struct;
  it does not recover the register descriptor type (e.g., `DR_REG`) or the
  wrapper type (e.g., `ReadWrite`, `ReadOnly`, `WriteOnly`).
  (`crates/regseq-driver/src/extract/identity_enrichment_types.rs:86-93`,
  `crates/regseq-driver/src/extract/register_access.rs:28-36`)
- Reasoning Trace: struct field names survive (e.g., `dr`, `fr`, `icr`). But:
  - Register type names (e.g., `DR_REG::Register`) are not extracted from
    the field type.
  - Byte offsets are not computed.
  - Access kinds (`ReadWrite`, `ReadOnly`, `WriteOnly`) are not derived from
    the wrapper type.
- Counterexample: `dr` is authored as `ReadWrite<u32, DR_REG::Register> @ 0x000`;
  the boundary preserves only `name = "dr"` with no register name, offset, or
  access kind.
- Suggested Fix Direction / Implementation Hints: for each field in a
  register-block struct, resolve the field type via `tcx` to extract:
  (a) the wrapper type name (`ReadWrite`, `ReadOnly`, `WriteOnly`, `Reserved`)
  to determine access kind,
  (b) the generic type parameter to determine the register descriptor type,
  (c) the byte offset from struct layout.
  These can populate new fields on `RegisterFieldDecl` or a dedicated
  per-register record.
- Conclusion: only struct field names survive; register name, byte offset,
  and access kind are dropped at this boundary.

## Check 7

- Status: **pass**
- Premise: no registers are inserted, removed, renamed, or reordered
- Target Evidence: the extracted 25-element field sequence matches the authored
  25-element field sequence in name and order. No extra entries appear and no
  authored entries are missing.
  (programmatic validation in
  `crates/regseq-driver/tests/pl011_register_block_extraction.rs:401`)
- Reasoning Trace: the extraction walks HIR struct fields in source order.
  Since the authored struct is `#[repr(C)]`, source order equals layout order
  equals authored offset order.
- Counterexample: none found
- Conclusion: no register entries were inserted, removed, renamed, or
  reordered at this boundary.

## Check 8

- Status: **fail**
- Premise: no register-block properties are invented or dropped
- Target Evidence: the authored property set is `{endian = "little",
  access_width = 32}`. The boundary drops both: `RegisterBlockDecl` has no
  `endian` or `access_width` field, and the `#[rseq::register_block(...)]`
  attribute arguments are not parsed. No spurious properties are invented.
- Reasoning Trace: this check requires preservation of "exactly the authored
  register-block property set relevant to Regseq lowering." Since both
  authored properties are dropped entirely, the property set is weakened.
- Counterexample: authored `endian = "little"` is not present in any
  extraction record for this block; authored `access_width = 32` is likewise
  absent.
- Suggested Fix Direction / Implementation Hints: same fix as Check 3: parse
  the `#[rseq::register_block(...)]` attribute arguments into
  `RegisterBlockDecl` block-level fields.
- Conclusion: the authored register-block property set is dropped at this
  boundary.

---

## Automated Validation Test

Test file: `crates/regseq-driver/tests/pl011_register_block_extraction.rs`

This test compiles the PL011 fixture through the real rustc compiler path,
invokes both `extract_register_access_surface()` and
`extract_identity_enrichment()`, and programmatically validates all 8 checks
against the authored fact inventory. It asserts the expected
pass/fail classification for each check.

Command: `cargo test -p regseq-driver --test pl011_register_block_extraction`
