# Validation Certificate: PL011 Register-Block Extraction Check

- Source premise: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Target boundary evidence:
  - `crates/regseq-driver/src/extract/register_access.rs` (RegisterAccessSurface extraction)
  - `crates/regseq-driver/src/extract/identity_enrichment.rs` (IdentityEnrichmentSurface extraction)
  - `crates/regseq-driver/tests/pl011_register_block_boundary.rs` (automated validation test)

---

## Check 1

- Check: Register block identity preserved as `Pl011Registers`
- Status: `pass`
- Premise: The register block remains identifiable as `Pl011Registers`.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:219`: `RegisterBlockDecl { name: name.to_string(), fields }` stores the struct ident as `name`.
  - `crates/regseq-driver/src/extract/register_access.rs:127`: `extract_register_block(tcx, ident.name.as_str(), variant_data)` passes the struct name from HIR.
  - Test assertion: `check_1_register_block_identity_preserved` in `crates/regseq-driver/tests/pl011_register_block_boundary.rs` confirms `register_blocks[0].name == "Pl011Registers"`.
- Reasoning Trace: The extraction stores the struct ident directly from HIR. No transformation or lossy conversion occurs.
- Counterexample: None found.
- Conclusion: The register-block identity `Pl011Registers` is explicitly preserved at the extraction boundary.

---

## Check 2

- Check: Canonical path preserved as `pl011_full_spec::registers::Pl011Registers`
- Status: `pass`
- Premise: The canonical path remains `pl011_full_spec::registers::Pl011Registers`.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/identity_enrichment.rs:70-80`: For `ItemKind::Struct`, the enrichment extraction derives `item_path = CanonicalPath(tcx.def_path_str(item.owner_id.def_id.to_def_id()))` and stores it as `StructFact.canonical_path`.
  - `crates/regseq-driver/src/extract/identity_enrichment.rs:222`: `StructFact` stores `is_register_block` (true for `#[rseq::register_block]` structs) alongside the canonical path.
  - `crates/regseq-driver/src/extract/identity_enrichment_types.rs:69-81`: `StructFact` type definition carries `canonical_path: CanonicalPath`.
  - `crates/regseq-driver/src/extract/register_access.rs:219`: `RegisterBlockDecl.name` stores the short struct ident ("Pl011Registers"), which is consistent with the canonical path's terminal component.
  - Test assertion: `check_2_short_name_matches_authored_identity` confirms the short name matches.
- Reasoning Trace: The `RegisterAccessSurface` stores only the short struct name. The `IdentityEnrichmentSurface` stores the full canonical path derived from `tcx.def_path_str(DefId)`, which for the PL011 fixture would produce `pl011_full_spec::registers::Pl011Registers`. The two surfaces together provide both the register-block classification and the canonical path. The short name is consistent with the terminal component of the canonical path.
- Counterexample: None found.
- Conclusion: The canonical path is explicitly preserved in the identity enrichment surface and can be correlated with the register-block identity via the shared struct name.

---

## Check 3

- Check: Block-level properties `endian = little` and `access_width = 32` preserved
- Status: `fail`
- Premise: The register-block properties preserve `endian = little` and `access_width = 32`.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:126`: The detection path uses `has_rseq_attr(attrs, "register_block")`, which checks only attribute existence, not its parameters.
  - `crates/regseq-driver/src/extract/register_access.rs:22-26`: `RegisterBlockDecl` has two fields: `name: String` and `fields: Vec<RegisterFieldDecl>`. There is no block-level property storage for `endian` or `access_width`.
  - `crates/regseq-driver/src/extract/register_access.rs:192-222`: `extract_register_block` receives only the struct name and variant data. The struct-level attributes (which carry `endian = "little", access_width = 32`) are not passed to this function and are never parsed.
  - `crates/regseq-driver/src/extract/register_access.rs:29-36`: `RegisterFieldDecl` has `access_width` and `endian` fields, but these are populated from per-field `#[rseq::register(...)]` attributes. The PL011 fixture has no per-field register attributes.
  - Test assertion: `check_3_block_properties_not_extracted` confirms all fields have `access_width == None` and `endian == None`.
- Reasoning Trace: The `#[rseq::register_block(endian = "little", access_width = 32)]` attribute is recognized for block identification but its key-value parameters are discarded. `RegisterBlockDecl` has no block-level property storage. The per-field `RegisterFieldDecl.access_width` and `RegisterFieldDecl.endian` are designed for per-field `#[rseq::register(...)]` attributes, which the PL011 source does not use.
- Counterexample: `Pl011Registers` is authored with `endian = "little"` and `access_width = 32`, but the extracted `RegisterBlockDecl` carries no property data. There is no way to recover these values from the extraction output.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs:126-128` and the `extract_register_block` function signature.
  - Minimal Missing Fact To Preserve: Parse the `#[rseq::register_block(...)]` attribute parameters (reusing the existing `parse_register_metadata` token parser) and store block-level `endian` and `access_width` on `RegisterBlockDecl`.
- Conclusion: Block-level `endian` and `access_width` are dropped at the extraction boundary.

---

## Check 4

- Check: Reserved region byte gaps preserved
- Status: `fail`
- Premise: The reserved regions preserve their authored offsets: `_reserved0: 0x008 -> 0x014`, `_reserved1: 0x01C -> 0x01C`, `_reserved2: 0x04C -> 0xFDC`.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:198-215`: `extract_register_block` iterates `variant_data.fields()` and stores each field's name and per-field attribute metadata. No byte offset, field size, array element count, or layout information is captured.
  - `crates/regseq-driver/src/extract/register_access.rs:28-36`: `RegisterFieldDecl` stores `name`, `access_width`, `endian`, `side_effects`, `hw_updates`. There is no `offset`, `size`, or `count` field.
  - Test assertion: `check_4_reserved_fields_present_but_offsets_absent` confirms `_reserved0`, `_reserved1`, `_reserved2` appear in the field list but carry no positional data.
- Reasoning Trace: The reserved fields are captured by name (they are struct fields), preserving their existence and relative order. However, the authored byte-offset ranges (e.g., `0x008 -> 0x014` for `_reserved0`) require either explicit offset capture or enough surrounding layout data (access width per field, array counts) to reconstruct. Neither is available. The boundary spec says the boundary must "preserve enough register-block layout information to reconstruct those gaps exactly," which is not met.
- Counterexample: `_reserved0` is authored as `[Reserved<u32>; 4]` spanning `0x008 -> 0x014`. The extracted `RegisterFieldDecl { name: "_reserved0", access_width: None, ... }` preserves only the field name. The array count (4), element size (4 bytes), and start offset (0x008) are all absent.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs:192-222` (`extract_register_block`).
  - Minimal Missing Fact To Preserve: For each register-block field, capture the byte offset (computable from struct layout via `repr(C)` rules and `access_width`) and, for array fields, the element count. Alternatively, record block-level `access_width` (see Check 3) so offsets can be reconstructed from field order and type sizes.
- Conclusion: Reserved fields are present by name but their byte-offset ranges cannot be reconstructed from the extraction output.

---

## Check 5

- Check: Complete register inventory in authored offset order
- Status: `pass`
- Premise: The declared register inventory remains complete and in authored offset order.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:198-215`: `extract_register_block` iterates `variant_data.fields()`, which returns fields in declaration order. For `#[repr(C)]` structs, declaration order matches offset order.
  - Test assertion: `check_5_complete_inventory_in_authored_order` confirms the extracted field sequence matches the authored 25-element sequence exactly: `["dr", "rsr_ecr", "_reserved0", "fr", "_reserved1", "ilpr", "ibrd", "fbrd", "lcr_h", "cr", "ifls", "imsc", "ris", "mis", "icr", "dmacr", "_reserved2", "periph_id0", "periph_id1", "periph_id2", "periph_id3", "pcell_id0", "pcell_id1", "pcell_id2", "pcell_id3"]`.
- Reasoning Trace: The boundary spec states "a complete extracted field sequence in authored order is sufficient for the inventory/order checks even if per-field offset and access metadata is not preserved." The extraction yields all 25 fields in exact declaration order, satisfying this condition.
- Counterexample: None found.
- Conclusion: The complete register inventory is preserved in authored declaration order.

---

## Check 6

- Check: Per-register four-fact tuple (field name, register name, byte offset, access kind) preserved
- Status: `fail`
- Premise: Each declared register preserves its struct field name, register name, byte offset, and access kind.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:28-36`: `RegisterFieldDecl` has `name`, `access_width`, `endian`, `side_effects`, `hw_updates`. There is no field for register type name (e.g., "DR_REG"), byte offset, or access kind (ReadWrite/ReadOnly/WriteOnly).
  - `crates/regseq-driver/src/extract/register_access.rs:199-200`: The extraction captures `field.ident.name` as the field name but does not inspect the field's type to extract the register descriptor name or access-wrapper kind.
  - Test assertion: `check_6_only_field_name_preserved_per_register` confirms that for `dr`, `fr`, and `icr`, only the field name is captured; all per-field metadata is `None`.
- Reasoning Trace: Of the four required facts per register:
  - **Field name**: preserved (e.g., "dr", "fr", "icr").
  - **Register name** (e.g., "DR_REG"): NOT preserved. The type path (e.g., `ReadWrite<u32, DR_REG::Register>`) is not parsed for the inner generic argument's module name. The `register_descriptors` list captures descriptor modules separately, but there is no explicit link from a `RegisterFieldDecl` to its descriptor module.
  - **Byte offset**: NOT preserved. No offset computation or storage exists.
  - **Access kind** (RW/R/W): NOT preserved. The wrapper type (ReadWrite/ReadOnly/WriteOnly) is inspected by the call-site visitor for method filtering (line 497-503) but is NOT stored in `RegisterFieldDecl`.
- Counterexample: `dr` is authored as `pub dr: ReadWrite<u32, DR_REG::Register>` at offset `0x000` with access kind `RW`. The extracted `RegisterFieldDecl { name: "dr", access_width: None, endian: None, ... }` preserves only the field name.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/register_access.rs:198-215` (`extract_register_block`).
  - Minimal Missing Fact To Preserve: For each field, resolve the wrapper type name (ReadWrite/ReadOnly/WriteOnly/Reserved) to determine access kind, and extract the inner generic argument path to identify the register descriptor module. Byte offsets can be derived from `repr(C)` layout + `access_width` or captured explicitly via `TyCtxt` layout queries.
- Conclusion: Only field names are preserved. Register names, byte offsets, and access kinds are dropped at this boundary.

---

## Check 7

- Check: No registers inserted, removed, renamed, or reordered
- Status: `pass`
- Premise: No registers are inserted, removed, renamed, or reordered.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:198-215`: The extraction iterates struct fields exactly as declared, with no filtering, reordering, or synthetic insertion logic.
  - Test assertion: `check_7_no_insertions_or_removals` confirms:
    - Exact count: 25 extracted == 25 authored.
    - Every authored name is present.
    - No extra entries exist.
    - Order matches exactly.
- Reasoning Trace: The extraction is a direct structural walk over HIR fields. No name transformation, filtering, or reordering occurs.
- Counterexample: None found.
- Conclusion: The extraction introduces no insertions, removals, renames, or reorderings.

---

## Check 8

- Check: No register-block properties invented or dropped
- Status: `fail`
- Premise: The inspected boundary state preserves exactly the authored register-block property set relevant to Regseq lowering.
- Target Evidence (Exact Citations):
  - `crates/regseq-driver/src/extract/register_access.rs:22-26`: `RegisterBlockDecl` has no block-level property fields. The authored `endian = "little"` and `access_width = 32` cannot be stored.
  - `crates/regseq-driver/src/extract/register_access.rs:126-128`: The `#[rseq::register_block(...)]` attribute is detected for block classification but its parameters are discarded.
  - Test assertion: `check_8_no_invented_properties_and_authored_properties_dropped` confirms no per-field metadata is invented (all `None`), which also proves no block-level properties were propagated into field metadata as a workaround.
- Reasoning Trace: No properties are invented (positive). The authored block-level properties `endian` and `access_width` are dropped (negative). The check requires both: no invention AND no dropping. The dropping of authored properties is a failure condition.
- Counterexample: The authored property set is `{endian: "little", access_width: 32}`. The extracted property set is `{}` (empty). The difference is `{endian: "little", access_width: 32}` dropped.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: Same as Check 3.
  - Minimal Missing Fact To Preserve: Add `endian: Option<String>` and `access_width: Option<u32>` fields to `RegisterBlockDecl` and parse them from the `#[rseq::register_block(...)]` attribute.
- Conclusion: Authored block-level properties are dropped. No properties are invented.

---

## Summary

| Check | Status | Description |
|-------|--------|-------------|
| 1 | pass | Block identity "Pl011Registers" preserved |
| 2 | pass | Canonical path evidence available (enrichment surface) |
| 3 | fail | Block-level endian/access_width dropped |
| 4 | fail | Reserved region byte-offset ranges not reconstructable |
| 5 | pass | Complete inventory in authored order |
| 6 | fail | Only field name preserved per register (no register name, offset, access kind) |
| 7 | pass | No insertions, removals, renames, or reorderings |
| 8 | fail | Authored block-level properties dropped |

**Overall: 4 pass, 4 fail.**

The extraction boundary preserves structural identity and field inventory but drops per-register metadata (byte offsets, access kinds, register type names) and block-level properties (endian, access_width). Checks 3, 4, 6, and 8 share a common root cause: `extract_register_block` captures field names from HIR but does not inspect field types or parse block-level attribute parameters.
