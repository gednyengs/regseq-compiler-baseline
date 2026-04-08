# Validation Certificate: PL011 Utility Functions Extraction Check

- Source premise: `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`

## Blocking Authored-Source Transcription Error

The check specification records the following "exact body effects":

| Function | Spec Body Effect | Actual Fixture Body |
|----------|-----------------|-------------------|
| `field_val` | `field.val(raw)` | `FieldValue::new(field.mask, raw << field.shift)` |
| `raw_reg_val` | `FieldValue::<u32, R>::new(mask, value)` | `FieldValue::new(mask, value)` |
| `field_from_raw` | `field.read(raw)` | `(raw & field.mask) >> field.shift` |

All three spec body effects disagree with the fixture source. Per the check
specification's Validation Precondition: "Any discrepancy between this check
contract and the fixture source is a blocking authored-source transcription
error. Record it in the report and do not mark extraction checks `pass`."

This discrepancy is recorded and no body-related check is marked `pass`.

Exercised evidence: `precondition_check_spec_body_effects_disagree_with_fixture`
asserts `SPEC_*_BODY != FIXTURE_*_BODY` for all three functions.

## Specimen Fidelity

- Specimen type: inline reduced specimen with `mod driver` nesting
- Fidelity Assertion Predicate(s):
  - `source.contains(AUTHORED_FIELD_VAL_SIG)` where `AUTHORED_FIELD_VAL_SIG = "fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>"`
  - `source.contains(AUTHORED_RAW_REG_VAL_SIG)` where `AUTHORED_RAW_REG_VAL_SIG = "fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>"`
  - `source.contains(AUTHORED_FIELD_FROM_RAW_SIG)` where `AUTHORED_FIELD_FROM_RAW_SIG = "fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32"`
  - `source.contains(FIXTURE_FIELD_VAL_BODY)` where `FIXTURE_FIELD_VAL_BODY = "FieldValue::new(field.mask, raw << field.shift)"`
  - `source.contains(FIXTURE_RAW_REG_VAL_BODY)` where `FIXTURE_RAW_REG_VAL_BODY = "FieldValue::new(mask, value)"`
  - `source.contains(FIXTURE_FIELD_FROM_RAW_BODY)` where `FIXTURE_FIELD_FROM_RAW_BODY = "(raw & field.mask) >> field.shift"`
  - `SPECIMEN.contains("mod driver")` verifies the specimen preserves the authored `driver::` module nesting
  - Each assertion is verified against both the inline specimen (`SPECIMEN`) and the real fixture source (`tests/fixtures/pl011-full-spec/src/driver.rs`).
- Fidelity Evidence (Exact Citations):
  - `precondition_fixture_source_contains_authored_signatures`: verifies all 3 signatures against fixture source
  - `precondition_fixture_source_contains_actual_body_expressions`: verifies all 3 real body expressions against fixture source
  - `precondition_specimen_fidelity_against_fixture`: verifies all 3 function names, 3 signatures, 3 body expressions, and `mod driver` nesting appear in both specimen and fixture

## Check-to-Surface Mapping

| Check | Extraction Surface | Field(s) Inspected |
|-------|-------------------|-------------------|
| Check Group 1 (identity/canonical path) | `IdentityEnrichmentSurface.free_functions` | `FunctionFact.name`, `FunctionFact.canonical_path`, `FunctionFact.module_path` |
| Check Group 1 (generics/params/return) | `IdentityEnrichmentSurface.free_functions` | Schema limitation: `FunctionFact` has no generics, params, or return type fields |
| Check Group 2 (body) | `MirBodySurface.functions` | Root discovery in `body.rs::discover_roots` |

## Check Group 1: Identity Preservation

### Check 1a: Function name and canonical path preservation

- Check: each utility function preserves identity and canonical path
- Status: `pass`
- Premise: `field_val`, `raw_reg_val`, `field_from_raw` should be discoverable by name and canonical path in the identity enrichment surface
- Assertion Predicate(s):
  - `fn_names.contains(name)` for each name in `["field_val", "raw_reg_val", "field_from_raw"]`
  - `cp.ends_with("driver::field_val")`, `cp.ends_with("driver::raw_reg_val")`, `cp.ends_with("driver::field_from_raw")` -- verifies the full module-qualified suffix, not just the function name
  - `authored_count == 3` confirming all three functions present
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_free_function_identity_name_and_canonical_path`: passes for all three functions with module-qualified canonical path verification
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` at `pl011_full_spec::driver::field_val`, `raw_reg_val` at `pl011_full_spec::driver::raw_reg_val`, `field_from_raw` at `pl011_full_spec::driver::field_from_raw`
  - Extracted Fact (Verbatim): All three names found in `free_functions`; canonical paths end with `driver::field_val`, `driver::raw_reg_val`, `driver::field_from_raw` respectively (specimen uses `test_input` crate name, preserving `driver::` module nesting)
  - Correspondence: `exact` (name), `exact` (module-qualified path suffix; crate-name difference is inherent to inline specimen, but `driver::` module shape is preserved)
  - Gap / Loss Description: none for identity; the specimen preserves the `mod driver` nesting so canonical paths carry the authored module shape
  - Why This Status Follows: The identity enrichment surface discovers all three free functions by name and produces canonical paths that include the `driver::` module nesting matching the authored module structure.
- Counterexample: none
- Suggested Fix Direction / Implementation Hints: none needed

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
  - Extracted Fact (Verbatim): `FunctionFact { name: "field_val", canonical_path: CanonicalPath("...::driver::field_val"), module_path: CanonicalPath("...::driver") }` -- no generics, params, or return type fields exist
  - Correspondence: `absent`
  - Gap / Loss Description: `FunctionFact` struct does not carry `generics`, `params`, or `return_type` fields. The identity enrichment surface captures free-function existence and location but not signature details.
  - Why This Status Follows: The authored premise requires preservation of generics, parameter list, and return type. The `FunctionFact` schema has no fields for these properties, so they are structurally absent from the extraction surface.
- Counterexample: Inspect `FunctionFact` in `identity_enrichment_types.rs` -- it has only `name`, `canonical_path`, `module_path` fields with no mechanism to carry generics, parameter types, or return type.
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
  - Note: The check specification incorrectly records "exact body effects" as `field.val(raw)`, `FieldValue::<u32, R>::new(mask, value)`, and `field.read(raw)`. These are transcription errors (see Blocking Authored-Source Transcription Error section above). The actual fixture bodies are used for the authored-fact record.
  - Extracted Fact (Verbatim): No entries in `MirBodySurface.functions` for any of the three free functions
  - Correspondence: `absent`
  - Gap / Loss Description: `extract_body_surface` in `body.rs` uses `discover_roots` which only scans `impl` blocks on the `#[rseq::driver]` type. Free functions at module scope are neither roots nor helper candidates (helper candidates are only inherent impl methods). Therefore free-function bodies are never extracted. Additionally, the check specification contains transcription errors in the body effects, which independently block any `pass` status.
  - Why This Status Follows: The authored premise requires body-level proof of call shape and argument flow. The body extraction surface does not include free functions because its root discovery is scoped to driver-type impl blocks. No body evidence exists to validate. Furthermore, the check specification's "exact body effects" are transcription errors that disagree with the fixture source, constituting a blocking precondition failure.
- Counterexample: `discover_roots` in `body.rs` iterates only over `impl` blocks for the driver type. Free functions defined via `ItemKind::Fn` at module scope are never visited.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/body.rs` (`discover_roots` function)
  - Minimal Missing Fact To Preserve: Extend root discovery to include free functions (or at minimum, free functions transitively called from extracted bodies) by scanning `ItemKind::Fn` at crate root in addition to impl blocks.
  - Additionally, the check specification's "exact body effects" in `docs/validation/01-pl011/checks/04-utility-functions-extraction.md` must be corrected to match the actual fixture source.

## Conclusion

- Blocking Authored-Source Transcription Error: **recorded** -- all three "exact body effects" in the check specification disagree with the actual fixture source
- Check Group 1 (identity/canonical path): **pass** -- names and module-qualified canonical paths preserved (specimen preserves `driver::` module nesting)
- Check Group 1 (generics/params/return type): **fail** -- `FunctionFact` schema lacks these fields
- Check Group 2 (body extraction): **fail** -- free functions excluded from body extraction root discovery; additionally blocked by authored-source transcription error in the check specification
- Overall: **partial fail** -- identity is preserved but signature details and body content are absent from the extraction surfaces; check specification requires correction of body effects
