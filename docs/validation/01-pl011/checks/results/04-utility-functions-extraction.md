# Validation Certificate: PL011 Utility Functions Extraction Check

- Source premise: `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`

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
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:279`
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:303`
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:327`
  - The specimen preserves all 3 function names, 3 signatures, 3 exact body expressions, and the authored `driver::` module nesting.

## Check-to-Surface Mapping

| Check | Extraction Surface | Field(s) Inspected |
|-------|-------------------|-------------------|
| Check Group 1 (identity/canonical path) | `IdentityEnrichmentSurface.free_functions` | `FunctionFact.name`, `FunctionFact.canonical_path`, `FunctionFact.module_path` |
| Check Group 1 (generics/params/return) | `IdentityEnrichmentSurface.free_functions` | `FunctionFact.generics`, `FunctionFact.params`, `FunctionFact.return_type` |
| Check Group 2 (body) | `MirBodySurface.functions` | `MirExtractedFn.kind`, `MirExtractedFn.body` (basic blocks, terminators, statements) |

## Check Group 1: Identity Preservation

### Check 1a: Function name and canonical path preservation

- Check: each utility function preserves identity and canonical path
- Status: `pass`
- Premise: `field_val`, `raw_reg_val`, `field_from_raw` should be discoverable by name and canonical path in the identity enrichment surface
- Assertion Predicate(s):
  - `fn_names.contains(name)` for each name in `["field_val", "raw_reg_val", "field_from_raw"]`
  - crate root module exists with `name == "pl011_full_spec"` and `canonical_path.0 == "pl011_full_spec"`
  - `driver` module exists as a child of the crate root
  - `cp == "driver::field_val"`, `cp == "driver::raw_reg_val"`, `cp == "driver::field_from_raw"` -- verifies source-local function canonical paths
  - `module_path.0 == "driver"` for all three free functions
  - `authored_count == 3` confirming all three functions present
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:389`
  - The test asserts all three free-function names exist, the crate root and `driver` module relationship is preserved, each function has the expected source-local path `driver::<fn_name>`, and each function's `module_path` is `driver`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `field_val` at `pl011_full_spec::driver::field_val`, `raw_reg_val` at `pl011_full_spec::driver::raw_reg_val`, `field_from_raw` at `pl011_full_spec::driver::field_from_raw`
  - Extracted Fact (Verbatim): All three names found in `free_functions`; canonical paths are `driver::field_val`, `driver::raw_reg_val`, and `driver::field_from_raw`; each has `module_path = "driver"`; crate root module is `pl011_full_spec` and contains `driver`
  - Correspondence: `exact` (reconstructible from exercised crate/module/function facts)
  - Gap / Loss Description: The free-function `canonical_path` is source-local rather than crate-prefixed, but the crate root module and containment facts make the full authored path exactly reconstructable.
  - Why This Status Follows: Combining (1) crate root `pl011_full_spec`, (2) `driver` is a child of crate root, (3) each function canonical_path is `driver::<fn_name>`, and (4) each function module_path is `driver` yields the exact authored canonical path `pl011_full_spec::driver::<fn_name>` without heuristics.
- Counterexample: none
- Suggested Fix Direction / Implementation Hints: none needed

### Check 1b: Generics, parameter list, and return type preservation

- Check: each utility function preserves generics, parameter list, and return type
- Status: `pass`
- Premise: each function should preserve generic type parameter `R`, full parameter list, and return type
- Assertion Predicate(s):
  - `fv.generics == ["R"]` for `field_val`
  - `fv.params[0].name == "field"`, `fv.params[0].ty == "Field<u32, R>"`, `fv.params[1].name == "raw"`, `fv.params[1].ty == "u32"` for `field_val`
  - `fv.return_type == Some("FieldValue<u32, R>")` for `field_val`
  - `rrv.generics == ["R"]` for `raw_reg_val`
  - `rrv.params[0].name == "mask"`, `rrv.params[0].ty == "u32"`, `rrv.params[1].name == "value"`, `rrv.params[1].ty == "u32"` for `raw_reg_val`
  - `rrv.return_type == Some("FieldValue<u32, R>")` for `raw_reg_val`
  - `ffr.generics == ["R"]` for `field_from_raw`
  - `ffr.params[0].name == "field"`, `ffr.params[0].ty == "Field<u32, R>"`, `ffr.params[1].name == "raw"`, `ffr.params[1].ty == "u32"` for `field_from_raw`
  - `ffr.return_type == Some("u32")` for `field_from_raw`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:476`
  - The test asserts exact generics, parameter names, parameter types, and return types for all three functions against the authored signatures.
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>`; `fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>`; `fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32`
  - Extracted Fact (Verbatim): `FunctionFact.generics = ["R"]`, `FunctionFact.params = [FnParamFact { name: "field", ty: "Field<u32, R>" }, FnParamFact { name: "raw", ty: "u32" }]`, `FunctionFact.return_type = Some("FieldValue<u32, R>")` for `field_val`; analogous exact matches for `raw_reg_val` and `field_from_raw`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Each authored signature property (generics, parameter names, parameter types, return type) is individually asserted against the extraction surface. All assertions pass with exact string equality.
- Counterexample: none
- Suggested Fix Direction / Implementation Hints: none needed

## Check Group 2: Body Extraction

### Check 2a: Free function body presence

- Check: each utility body is present in the body extraction surface
- Status: `pass`
- Premise: body extraction should include free-function bodies
- Assertion Predicate(s):
  - `body_fn_names.contains(name)` for each name in `["field_val", "raw_reg_val", "field_from_raw"]`
  - `func.kind == MirFnKind::FreeFunction` for all three functions
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:548`
  - The test asserts all three utility function names appear in `MirBodySurface.functions` and all have `kind == FreeFunction`.
- Reasoning Trace:
  - Authored Fact (Verbatim): three free utility functions exist at module scope
  - Extracted Fact (Verbatim): all three appear in `MirBodySurface.functions` with `kind = FreeFunction`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: `discover_roots` now scans `ItemKind::Fn` at crate root and inside nested modules, producing `FnRoot` entries with `MirFnKind::FreeFunction`. `extract_body_surface` extracts MIR bodies for these roots.

### Check 2b: field_val body structure

- Check: `field_val` body preserves call to `FieldValue::new` and `Shl` operation
- Status: `pass`
- Premise: `field_val` body is `FieldValue::new(field.mask, raw << field.shift)`
- Assertion Predicate(s):
  - MIR body contains a `Call` terminator with callee `FnDef` path containing `FieldValue` and `new`
  - MIR body contains an `Assign` statement with `BinaryOp { op: Shl, .. }` rvalue
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:575`
  - The test finds a `Call` terminator targeting `FieldValue::new` and a `Shl` binary operation in the extracted MIR body.
- Reasoning Trace:
  - Authored Fact (Verbatim): `FieldValue::new(field.mask, raw << field.shift)`
  - Extracted Fact (Verbatim): MIR body contains `Call { func: Constant { ty: FnDef { path: "...FieldValue...new..." } } }` and `Assign { rvalue: BinaryOp { op: Shl, .. } }`
  - Correspondence: `exact`
  - Why This Status Follows: The callee identity (`FieldValue::new`) and the shift operation (`Shl`) together prove the authored body shape: a constructor call with a shifted argument.

### Check 2c: raw_reg_val body structure

- Check: `raw_reg_val` body preserves call to `FieldValue::new`
- Status: `pass`
- Premise: `raw_reg_val` body is `FieldValue::new(mask, value)`
- Assertion Predicate(s):
  - MIR body contains a `Call` terminator with callee `FnDef` path containing `FieldValue` and `new`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:636`
  - The test finds a `Call` terminator targeting `FieldValue::new` in the extracted MIR body.
- Reasoning Trace:
  - Authored Fact (Verbatim): `FieldValue::new(mask, value)`
  - Extracted Fact (Verbatim): MIR body contains `Call { func: Constant { ty: FnDef { path: "...FieldValue...new..." } } }`
  - Correspondence: `exact`
  - Why This Status Follows: The body is a direct constructor call with parameter forwarding. The callee identity proves the authored body shape.

### Check 2d: field_from_raw body structure

- Check: `field_from_raw` body preserves `BitAnd` and `Shr` operations
- Status: `pass`
- Premise: `field_from_raw` body is `(raw & field.mask) >> field.shift`
- Assertion Predicate(s):
  - MIR body contains an `Assign` statement with `BinaryOp { op: BitAnd, .. }` rvalue
  - MIR body contains an `Assign` statement with `BinaryOp { op: Shr, .. }` rvalue
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs:666`
  - The test finds both a `BitAnd` and a `Shr` binary operation in the extracted MIR body.
- Reasoning Trace:
  - Authored Fact (Verbatim): `(raw & field.mask) >> field.shift`
  - Extracted Fact (Verbatim): MIR body contains `Assign { rvalue: BinaryOp { op: BitAnd, .. } }` and `Assign { rvalue: BinaryOp { op: Shr, .. } }`
  - Correspondence: `exact`
  - Why This Status Follows: The masking (`BitAnd`) and shifting (`Shr`) operations together prove the authored arithmetic structure without source-only reasoning.

## Conclusion

- Check Group 1 (identity/canonical path): **pass** -- names and full canonical paths are preserved exactly
- Check Group 1 (generics/params/return type): **pass** -- `FunctionFact` now carries `generics`, `params`, and `return_type` fields with exact authored values
- Check Group 2 (body presence): **pass** -- free functions are now included in body extraction root discovery
- Check Group 2 (body structure): **pass** -- MIR bodies prove callee identity, argument flow, and arithmetic/masking/shifting structure for all three helpers
- Overall: **pass** -- all authored utility-function facts are preserved at the extraction boundary
