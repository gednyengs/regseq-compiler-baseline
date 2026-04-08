# Validation Certificate: PL011 Utility Functions Extraction Check

- Source premise: `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`

## Specimen Fidelity

- Specimen type: inline reduced specimen
- Fidelity Assertion Predicate(s):
  - `source.contains(AUTHORED_FIELD_VAL_SIG)` where `AUTHORED_FIELD_VAL_SIG = "fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>"`
  - `source.contains(AUTHORED_RAW_REG_VAL_SIG)` where `AUTHORED_RAW_REG_VAL_SIG = "fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>"`
  - `source.contains(AUTHORED_FIELD_FROM_RAW_SIG)` where `AUTHORED_FIELD_FROM_RAW_SIG = "fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32"`
  - `source.contains(AUTHORED_FIELD_VAL_BODY)` where `AUTHORED_FIELD_VAL_BODY = "FieldValue::new(field.mask, raw << field.shift)"`
  - `source.contains(AUTHORED_RAW_REG_VAL_BODY)` where `AUTHORED_RAW_REG_VAL_BODY = "FieldValue::new(mask, value)"`
  - `source.contains(AUTHORED_FIELD_FROM_RAW_BODY)` where `AUTHORED_FIELD_FROM_RAW_BODY = "(raw & field.mask) >> field.shift"`
  - Each assertion is verified against both the inline specimen (`SPECIMEN`) and the real fixture source (`tests/fixtures/pl011-full-spec/src/driver.rs`).
- Fidelity Evidence (Exact Citations):
  - `precondition_fixture_source_contains_authored_utility_functions`: verifies all 6 authored facts against fixture source
  - `precondition_specimen_fidelity_against_fixture`: verifies all 3 function names, 3 signatures, and 3 body expressions appear in both specimen and fixture

## Check-to-Surface Mapping

| Check | Extraction Surface | Field(s) Inspected |
|-------|-------------------|-------------------|
| Check Group 1 (identity) | `IdentityEnrichmentSurface.free_functions` | `FunctionFact.name`, `FunctionFact.canonical_path`, `FunctionFact.module_path` |
| Check Group 1 (generics/params/return) | `IdentityEnrichmentSurface.free_functions` | Schema limitation: `FunctionFact` has no generics, params, or return type fields |
| Check Group 2 (body) | `MirBodySurface.functions` | Root discovery in `body.rs::discover_roots` |

## Check Group 1: Identity Preservation

### Check 1a: Function name and canonical path preservation

- Check: each utility function preserves identity and canonical path
- Status: `pass`
- Premise: `field_val`, `raw_reg_val`, `field_from_raw` should be discoverable by name and canonical path in the identity enrichment surface
- Assertion Predicate(s):
  - `fn_names.contains(name)` for each name in `["field_val", "raw_reg_val", "field_from_raw"]`
  - `cp == name || cp.ends_with(&format!("::{name}"))` for each function's canonical path
  - `authored_count == 3` confirming all three functions present
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_free_function_identity_name_and_canonical_path`: passes for all three functions
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` at `pl011_full_spec::driver::field_val`, `raw_reg_val` at `pl011_full_spec::driver::raw_reg_val`, `field_from_raw` at `pl011_full_spec::driver::field_from_raw`
  - Extracted Fact (Verbatim): All three names found in `free_functions`; canonical paths end with `::field_val`, `::raw_reg_val`, `::field_from_raw` respectively (specimen uses `test_input` crate name)
  - Correspondence: `exact` (name), `exact` (path suffix; crate-name difference is expected for inline specimen)
  - Gap / Loss Description: none for identity
  - Why This Status Follows: The identity enrichment surface discovers all three free functions by name and produces deterministic canonical paths.

### Check 1b: Generics, parameter list, and return type preservation

- Check: each utility function preserves generics, parameter list, and return type
- Status: `fail`
- Premise: each function should preserve generic type parameter `R`, full parameter list, and return type
- Assertion Predicate(s):
  - `!func.name.is_empty()`, `!func.canonical_path.0.is_empty()`, `!func.module_path.0.is_empty()` (verifies schema content)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_identity_surface_lacks_generics_params_return_type`: confirms `FunctionFact` only has `name`, `canonical_path`, `module_path`
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>` (generic `R`, two params, typed return)
  - Extracted Fact (Verbatim): `FunctionFact { name: "field_val", canonical_path: CanonicalPath("...::field_val"), module_path: CanonicalPath("...") }` -- no generics, params, or return type fields exist
  - Correspondence: `absent`
  - Gap / Loss Description: `FunctionFact` struct does not carry `generics`, `params`, or `return_type` fields. The identity enrichment surface captures free-function existence and location but not signature details.
  - Why This Status Follows: The authored premise requires preservation of generics, parameter list, and return type. The `FunctionFact` schema has no fields for these properties, so they are structurally absent from the extraction surface.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/identity_enrichment_types.rs` (`FunctionFact` struct) and `crates/regseq-driver/src/extract/identity_enrichment.rs` (extraction logic)
  - Minimal Missing Fact To Preserve: Add `generics: Vec<String>`, `params: Vec<MirFnParam>`, `return_type: Option<String>` to `FunctionFact`

## Check Group 2: Body Extraction

### Check 2: Body preservation for free functions

- Check: each utility body preserves its exact authored call shape and parameter flow
- Status: `fail`
- Premise: body extraction should capture free-function bodies to prove call target and argument mapping
- Assertion Predicate(s):
  - `!body_fn_names.contains(name)` for each name in `["field_val", "raw_reg_val", "field_from_raw"]` -- confirms absence
  - `free_fn_count == 0` -- no `FreeFunction`-kind entries exist in the body surface
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_body_surface_does_not_contain_free_functions`: proves none of the three utility functions appear in `MirBodySurface.functions`
  - `check_group_2_free_function_body_extraction_root_absence`: proves zero `FreeFunction`-kind entries exist in the body surface
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` body is `FieldValue::new(field.mask, raw << field.shift)`; `raw_reg_val` body is `FieldValue::new(mask, value)`; `field_from_raw` body is `(raw & field.mask) >> field.shift`
  - Extracted Fact (Verbatim): No entries in `MirBodySurface.functions` for any of the three free functions
  - Correspondence: `absent`
  - Gap / Loss Description: `extract_body_surface` in `body.rs` uses `discover_roots` which only scans `impl` blocks on the `#[rseq::driver]` type. Free functions at module scope are neither roots nor helper candidates (helper candidates are only inherent impl methods). Therefore free-function bodies are never extracted.
  - Why This Status Follows: The authored premise requires body-level proof of call shape and argument flow. The body extraction surface does not include free functions because its root discovery is scoped to driver-type impl blocks. No body evidence exists to validate.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/body.rs` (`discover_roots` function)
  - Minimal Missing Fact To Preserve: Extend root discovery to include free functions (or at minimum, free functions transitively called from extracted bodies) by scanning `ItemKind::Fn` at crate root in addition to impl blocks.

## Conclusion

- Check Group 1 (identity/canonical path): **pass** -- names and canonical paths preserved
- Check Group 1 (generics/params/return type): **fail** -- `FunctionFact` schema lacks these fields
- Check Group 2 (body extraction): **fail** -- free functions excluded from body extraction root discovery
- Overall: **partial fail** -- identity is preserved but signature details and body content are absent from the extraction surfaces
