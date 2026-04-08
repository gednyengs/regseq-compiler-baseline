# Validation Certificate: PL011 Utility Functions Extraction Check

- Source premise: `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/utility_functions_extraction.rs`

## Specimen Fidelity

- Type: Reduced inline specimen
- Fidelity Assertion Predicate(s):
  - `fixture.contains("fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>")` -- true
  - `fixture.contains("FieldValue::new(field.mask, raw << field.shift)")` -- true
  - `fixture.contains("fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>")` -- true
  - `fixture.contains("FieldValue::new(mask, value)")` -- true
  - `fixture.contains("fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32")` -- true
  - `fixture.contains("(raw & field.mask) >> field.shift")` -- true
  - Each of the above is also verified present in the specimen text.
- Fidelity Evidence (Exact Citations):
  - Test `precondition_specimen_fidelity` in `crates/regseq-driver/tests/utility_functions_extraction.rs` lines 188-264

## Check-to-Surface Mapping

| Check | Surface |
|-------|---------|
| Check Group 1 (identity, canonical path, generics, params, return type) | `IdentityEnrichmentSurface.free_functions` |
| Check Group 2 (body call shape and parameter flow) | `MirBodySurface.functions` |

## Check Group 1: Identity, canonical path, generics, parameter list, return type

### Check 1a: Function identity presence

- Check: Each utility function preserves identity and canonical path in the free-function identity surface
- Status: `pass`
- Premise: each utility function preserves identity and canonical path
- Assertion Predicate(s):
  - For each name in `["field_val", "raw_reg_val", "field_from_raw"]`:
    - `result.identity.free_functions.iter().find(|f| f.name == name).is_some()` -- true
    - `func.canonical_path.0.contains(name)` -- true
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1a_utility_function_identity_presence` in `crates/regseq-driver/tests/utility_functions_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` has canonical path `pl011_full_spec::driver::field_val`; `raw_reg_val` has canonical path `pl011_full_spec::driver::raw_reg_val`; `field_from_raw` has canonical path `pl011_full_spec::driver::field_from_raw`
  - Extracted Fact (Verbatim): Each function appears in `IdentityEnrichmentSurface.free_functions` with `name` matching the authored name and `canonical_path` containing the authored name
  - Correspondence: `exact` (for name and canonical path containment)
  - Gap / Loss Description: None for identity and canonical path
  - Why This Status Follows: The extraction surface preserves function identity and canonical path exactly

### Check 1b: Closed-world count

- Check: Exactly one instance of each utility function name exists (no duplication or loss)
- Status: `pass`
- Premise: no duplication or loss of utility function identities
- Assertion Predicate(s):
  - For each name in `["field_val", "raw_reg_val", "field_from_raw"]`:
    - `result.identity.free_functions.iter().filter(|f| f.name == name).count() == 1` -- true
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1b_utility_function_count` in `crates/regseq-driver/tests/utility_functions_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): Three distinct utility functions exist
  - Extracted Fact (Verbatim): Exactly one of each name in the free_functions list
  - Correspondence: `exact`
  - Gap / Loss Description: None
  - Why This Status Follows: Closed-world count confirms no collapse or duplication

### Check 1c: Generics, parameter list, and return type

- Check: Each utility function preserves generics, parameter list, and return type
- Status: `fail`
- Premise: extraction preserves generic parameters (e.g., `<R>`), parameter list (e.g., `field: Field<u32, R>, raw: u32`), and return type (e.g., `FieldValue<u32, R>`) for each utility function
- Assertion Predicate(s):
  - `FunctionFact` struct has only `name`, `canonical_path`, `module_path` fields -- no fields for generics, parameter types, or return type exist in the struct definition
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1c_identity_surface_lacks_generics_params_return_type` in `crates/regseq-driver/tests/utility_functions_extraction.rs`
  - Structural evidence: `FunctionFact` in `crates/regseq-driver/src/extract/identity_enrichment_types.rs` lines 187-195 defines only `name: String`, `canonical_path: CanonicalPath`, `module_path: CanonicalPath`
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>`
  - Extracted Fact (Verbatim): `FunctionFact { name: "field_val", canonical_path: CanonicalPath("...::field_val"), module_path: ... }`
  - Correspondence: `weakened`
  - Gap / Loss Description: The `FunctionFact` type does not carry generic type parameters, function parameter types, or return type. These declaration facts are lost at the identity enrichment extraction boundary.
  - Why This Status Follows: The authored premise requires generics, parameter list, and return type to be preserved. The extraction surface structurally cannot carry these facts, so the premise is not met.
- Counterexample: Any function with signature `fn field_val(x: i32) -> bool` would produce an identical `FunctionFact` as the authored generic `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/identity_enrichment_types.rs` (add generics, params, return_type fields to `FunctionFact`)
  - Minimal Missing Fact To Preserve: Generic parameter names, parameter (name, type) pairs, return type string

## Check Group 2: Body call shape and parameter flow

### Check 2a: Utility function bodies in extraction

- Check: Each utility body preserves its exact authored call shape and parameter flow
- Status: `fail`
- Premise: extraction preserves enough body information to prove exact call target and argument mapping for each utility function
- Assertion Predicate(s):
  - For each name in `["field_val", "raw_reg_val", "field_from_raw"]`:
    - `result.body.functions.iter().find(|f| f.name == name).is_none()` -- true (bodies are absent)
  - `result.body.functions.iter().filter(|f| matches!(f.kind, MirFnKind::FreeFunction)).count() == 0` -- true
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2a_utility_functions_absent_from_body_surface` in `crates/regseq-driver/tests/utility_functions_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` body is `FieldValue::new(field.mask, raw << field.shift)`; `raw_reg_val` body is `FieldValue::new(mask, value)`; `field_from_raw` body is `(raw & field.mask) >> field.shift`
  - Extracted Fact (Verbatim): None of the three functions appear in `MirBodySurface.functions`
  - Correspondence: `absent`
  - Gap / Loss Description: The body extraction surface (`extract_body_surface`) only discovers roots from impl blocks on the driver type (trait callbacks, ISR handlers, inherent helper methods). Free functions at module scope are not included as extraction roots. The helper-candidate map only indexes inherent impl methods on the driver type, so free functions are not reachable via transitive call discovery either.
  - Why This Status Follows: The authored premise requires body information. The extraction surface does not extract free-function bodies at all, so no body facts are preserved.
- Counterexample: Any free function body -- or no free function at all -- would produce the same empty result from `MirBodySurface`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/body.rs` (extend root discovery to include free functions reachable from callback/helper call graphs, or add free functions as explicit extraction roots)
  - Minimal Missing Fact To Preserve: MIR body for each free function, including call targets, argument operands, and return value construction

## Conclusion

| Check | Status |
|-------|--------|
| Check 1a: Identity presence (name + canonical path) | `pass` |
| Check 1b: Closed-world count (no duplication/loss) | `pass` |
| Check 1c: Generics, parameter list, return type | `fail` |
| Check 2a: Body call shape and parameter flow | `fail` |

The identity enrichment surface preserves function name and canonical path for
all three utility functions. However, the full Check Group 1 premise (identity,
canonical path, generics, parameter list, return type) is only partially met
because `FunctionFact` does not carry generic parameters, parameter types, or
return type.

Check Group 2 fails entirely because the body extraction surface does not
extract free-function bodies. The current root-discovery algorithm in
`extract_body_surface` is limited to driver-type impl blocks and does not
include module-scope free functions.
