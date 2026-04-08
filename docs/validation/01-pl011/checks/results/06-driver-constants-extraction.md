# Validation Certificate: PL011 Driver Constants Extraction Check

- Source premise: `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs`
- Extraction surfaces inspected:
  - `IdentityEnrichmentSurface.impl_blocks` via `extract_identity_enrichment(tcx)` (`crates/regseq-driver/src/extract/identity_enrichment.rs`)
  - `MirBodySurface.functions` via `extract_body_surface(tcx)` (`crates/regseq-driver/src/extract/body.rs`)

## Specimen Fidelity

The validation uses a reduced inline specimen compiled with a `#[rseq::driver]`-annotated
`Pl011FullSpec` struct and an inherent impl block containing all six authored constants.
A `use_constants` helper and an `#[rseq::isr]`-annotated `handle_irq` method ensure
the constants appear in MIR body extraction.

The specimen reproduces every authored constant declaration verbatim:

- `const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;`
- `const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);`
- `const TX_INTERRUPT_MASK: u32 = 1 << 5;`
- `const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);`
- `const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;`
- `const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;`

The specimen does NOT include register blocks, trait implementations, or non-constant
methods from the full PL011 fixture. These are out of scope for driver-constants
boundary validation.

- Fidelity Assertion Predicate(s):
  - `assert!(fixture.contains("const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;"))`
  - `assert!(fixture.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"))`
  - `assert!(fixture.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"))`
  - `assert!(fixture.contains("const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);"))`
  - `assert!(fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"))`
  - `assert!(fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;"))`
  - Matching assertions on `SPECIMEN` confirm the inline specimen reproduces each declaration.
- Fidelity Evidence (Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::precondition_specimen_fidelity`

## Check-to-Surface Mapping

| Check | Extraction Surface(s) | Test Function |
|-------|----------------------|---------------|
| 1a | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (name) | `check1a_all_constants_present_by_name` |
| 1b | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (ty) | `check1b_constant_types_preserved` |
| 1c | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (canonical_path) | `check1c_canonical_paths_preserved` |
| 1d | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (owner_impl_path) | `check1d_owner_impl_path_preserved` |
| 1e | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (closed-world count) | `check1e_closed_world_constant_set` |
| 1f | `MirBodySurface.functions["use_constants"]` (unevaluated constant refs) | `check1f_constant_references_in_mir_body` |
| 1g | `IdentityEnrichmentSurface` + `MirBodySurface` (value gap evidence) | `check1g_constant_values_not_evaluated` |
| 2a | `IdentityEnrichmentSurface.impl_blocks` + `MirBodySurface` (both constants survive) | `check2a_both_constants_survive_as_distinct_entries` |
| 2b | `IdentityEnrichmentSurface.impl_blocks` (no definitional linkage) | `check2b_definitional_relationship_not_on_identity_surface` |

---

## Check 1a

- Check: Constant names preserved
- Status: `pass`
- Premise: all six in-scope constants preserve their names at the extraction boundary
- Assertion Predicate(s):
  - `assert!(extracted_names.contains(&authored.name))` for each of the six constants
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1a_all_constants_present_by_name`
  - The inherent impl block for `Pl011FullSpec` contains `AssociatedConstantFact` entries with names matching all six authored constants.
- Reasoning Trace:
  - Authored Fact (Verbatim): `ALL_UART_INTERRUPTS_MASK`, `RX_INTERRUPT_MASK`, `TX_INTERRUPT_MASK`, `ERROR_INTERRUPT_MASK`, `EXPLICIT_ISR_CLEAR_MASK`, `DEFAULT_UART_CLOCK_HZ`
  - Extracted Fact (Verbatim): `AssociatedConstantFact { name: "ALL_UART_INTERRUPTS_MASK", .. }` etc.
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: All six constant names are present in the extraction output; the assertion would fail if any were missing.
- Counterexample: none found
- Conclusion: Constant names are preserved at the extraction boundary.

---

## Check 1b

- Check: Constant Rust types preserved
- Status: `pass`
- Premise: all six constants preserve their Rust type `u32`
- Assertion Predicate(s):
  - `assert_eq!(extracted.ty, authored.ty)` for each constant, where `authored.ty = "u32"`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1b_constant_types_preserved`
  - Each `AssociatedConstantFact.ty` equals `"u32"`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `u32` for all six constants
  - Extracted Fact (Verbatim): `AssociatedConstantFact { ty: "u32", .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The type string matches exactly; the assertion would fail on a type mismatch.
- Counterexample: none found
- Conclusion: Constant Rust types are preserved.

---

## Check 1c

- Check: Constant canonical paths preserved
- Status: `pass`
- Premise: all six constants have canonical paths ending with `Pl011FullSpec::<CONSTANT_NAME>`
- Assertion Predicate(s):
  - `assert!(extracted.canonical_path.0.ends_with(authored.canonical_path_suffix))` for each constant
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1c_canonical_paths_preserved`
  - Each `AssociatedConstantFact.canonical_path` ends with the expected suffix (e.g., `Pl011FullSpec::ALL_UART_INTERRUPTS_MASK`).
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK` etc.
  - Extracted Fact (Verbatim): canonical paths end with `Pl011FullSpec::ALL_UART_INTERRUPTS_MASK` etc.
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The path suffix assertion is non-tautological and would fail if the constant were not associated with the correct type.
- Counterexample: none found
- Conclusion: Canonical paths are preserved.

---

## Check 1d

- Check: Owner impl/path association preserved
- Status: `pass`
- Premise: all six constants point back to the owning inherent impl block
- Assertion Predicate(s):
  - `assert_eq!(extracted.owner_impl_path, inherent.canonical_path)` for each constant
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1d_owner_impl_path_preserved`
  - Each `AssociatedConstantFact.owner_impl_path` equals the `ImplBlockFact.canonical_path` of the inherent impl block.
- Reasoning Trace:
  - Authored Fact (Verbatim): all constants are in `impl Pl011FullSpec { ... }`
  - Extracted Fact (Verbatim): `owner_impl_path` matches the inherent impl block's canonical path
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The assertion would fail if any constant's owner path did not match the inherent impl block.
- Counterexample: none found
- Conclusion: Owner impl path association is preserved.

---

## Check 1e

- Check: Closed-world constant membership
- Status: `pass`
- Premise: exactly six constants exist in the inherent impl block, with no extras or losses
- Assertion Predicate(s):
  - `assert_eq!(inherent.constants.len(), AUTHORED_CONSTANTS.len())`
  - For each authored name: `assert!(extracted_names.contains(name))`
  - For each extracted name: `assert!(authored_names.contains(name))`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1e_closed_world_constant_set`
  - Count is exactly 6; bidirectional membership check ensures no extras and no losses.
- Reasoning Trace:
  - Authored Fact (Verbatim): 6 constants declared in `impl Pl011FullSpec`
  - Extracted Fact (Verbatim): 6 `AssociatedConstantFact` entries in the inherent impl block
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The bidirectional membership assertion provides closed-world evidence.
- Counterexample: none found
- Conclusion: The constant set is exactly preserved with no extras or losses.

---

## Check 1f

- Check: Constant named references survive in MIR body
- Status: `pass`
- Premise: all six constants are referenced by name in the MIR body of a helper function
- Assertion Predicate(s):
  - `assert!(unevaluated_refs.iter().any(|desc| desc.contains(&expected_suffix)))` for each constant, where `expected_suffix = "Pl011FullSpec::<CONST_NAME>"`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1f_constant_references_in_mir_body`
  - MIR body of `use_constants` contains `MirConstValue::Unsupported { description: "unevaluated:Pl011FullSpec::ALL_UART_INTERRUPTS_MASK" }` etc. for all six constants.
- Reasoning Trace:
  - Authored Fact (Verbatim): constants are used via `Self::CONST_NAME` in function bodies
  - Extracted Fact (Verbatim): `MirConstValue::Unsupported { description: "unevaluated:Pl011FullSpec::ALL_UART_INTERRUPTS_MASK" }` etc.
  - Correspondence: `exact` (name references survive)
  - Gap / Loss Description: constants are not evaluated to scalar values; only symbolic names are preserved
  - Why This Status Follows: The assertion would fail if any constant's name reference were not present in the MIR body.
- Counterexample: none found
- Conclusion: Constant name references survive in MIR body extraction.

---

## Check 1g

- Check: Constant literal values (evaluated numeric values) preserved
- Status: `fail`
- Premise: constant literal values (e.g., `0x07ff`, `48_000_000`) are preserved at the extraction boundary
- Assertion Predicate(s):
  - `assert!(scalars.is_empty())` -- confirms no scalar values are present
  - `assert_eq!(unevaluated.len(), AUTHORED_CONSTANTS.len())` -- confirms all 6 appear as unevaluated refs
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check1g_constant_values_not_evaluated`
  - `IdentityEnrichmentSurface`: `AssociatedConstantFact` has no `value` field
  - `MirBodySurface`: constants appear as `MirConstValue::Unsupported { description: "unevaluated:..." }` rather than `MirConstValue::Scalar { bits, size }`
- Reasoning Trace:
  - Authored Fact (Verbatim): `ALL_UART_INTERRUPTS_MASK = 0x07ff`, `RX_INTERRUPT_MASK = (1 << 4) | (1 << 6)`, `TX_INTERRUPT_MASK = 1 << 5`, `ERROR_INTERRUPT_MASK = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)`, `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`, `DEFAULT_UART_CLOCK_HZ = 48_000_000`
  - Extracted Fact (Verbatim): no scalar values; constants appear as `Unsupported { description: "unevaluated:..." }`
  - Correspondence: `absent`
  - Gap / Loss Description: The identity enrichment surface (`AssociatedConstantFact`) does not include a `value` field. The MIR body extractor classifies associated constants as `MirConstValue::Unsupported` rather than evaluating them via `tcx.const_eval_resolve()` to produce `MirConstValue::Scalar`. The evaluated numeric values are not available at the extraction boundary.
  - Why This Status Follows: The authored premise requires literal value preservation. Neither extraction surface provides the evaluated constant value.
- Counterexample: `ALL_UART_INTERRUPTS_MASK` has authored value `0x07ff` (2047), but the MIR body contains `Unsupported { description: "unevaluated:Pl011FullSpec::ALL_UART_INTERRUPTS_MASK" }` instead of `Scalar { bits: 2047, size: 4 }`.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/identity_enrichment.rs` (add `value` field to `AssociatedConstantFact`) or `crates/regseq-driver/src/extract/mir_extract.rs` (evaluate unevaluated constants via `tcx.const_eval_resolve()`)
  - Minimal Missing Fact To Preserve: evaluated `u128` value on `AssociatedConstantFact` or `MirConstValue::Scalar` conversion for unevaluated associated constants
- Conclusion: Constant literal values are NOT preserved at the extraction boundary.

---

## Check 2a

- Check: Definitional relationship -- both constants survive as distinct entries
- Status: `pass`
- Premise: both `EXPLICIT_ISR_CLEAR_MASK` and `ERROR_INTERRUPT_MASK` survive as distinct entries in the identity enrichment surface and as distinct unevaluated references in the MIR body
- Assertion Predicate(s):
  - `assert_eq!(explicit_isr.ty, "u32")` and `assert_eq!(error_mask.ty, "u32")`
  - `assert_eq!(explicit_isr.owner_impl_path, error_mask.owner_impl_path)`
  - `assert!(unevaluated.iter().any(|d| d.contains("EXPLICIT_ISR_CLEAR_MASK")))`
  - `assert!(unevaluated.iter().any(|d| d.contains("ERROR_INTERRUPT_MASK")))`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check2a_both_constants_survive_as_distinct_entries`
  - Both constants exist in `IdentityEnrichmentSurface.impl_blocks[inherent].constants` and as named references in MIR body.
- Reasoning Trace:
  - Authored Fact (Verbatim): `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`; both are `u32` in `impl Pl011FullSpec`
  - Extracted Fact (Verbatim): both exist as distinct `AssociatedConstantFact` entries with type `u32` and same owner; both appear as unevaluated MIR references
  - Correspondence: `exact` (both survive)
  - Gap / Loss Description: none for identity survival; the syntactic `Self::` reference is not preserved (see Check 2b)
  - Why This Status Follows: The assertion would fail if either constant were missing from the extraction surfaces.
- Counterexample: none found
- Conclusion: Both constants survive as distinct entries at the extraction boundary.

---

## Check 2b

- Check: Syntactic definitional relationship `Self::ERROR_INTERRUPT_MASK` preserved
- Status: `fail`
- Premise: the definitional relationship `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK` is preserved at the extraction boundary
- Assertion Predicate(s):
  - `assert_eq!(explicit_isr.ty, "u32")` and `assert_eq!(error_mask.ty, "u32")`
  - `assert_ne!(explicit_isr.canonical_path, error_mask.canonical_path)` (distinct entries)
  - `assert_eq!(explicit_isr.owner_impl_path, error_mask.owner_impl_path)` (same owner)
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs::check2b_definitional_relationship_not_on_identity_surface`
  - `AssociatedConstantFact` has fields: `name`, `canonical_path`, `ty`, `owner_impl_path` -- no `value_expr`, `definition_ref`, or `refers_to` field exists.
- Reasoning Trace:
  - Authored Fact (Verbatim): `const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;`
  - Extracted Fact (Verbatim): `AssociatedConstantFact { name: "EXPLICIT_ISR_CLEAR_MASK", ty: "u32", canonical_path: "..::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK", owner_impl_path: "..::Pl011FullSpec" }` -- no cross-reference to `ERROR_INTERRUPT_MASK`
  - Correspondence: `absent`
  - Gap / Loss Description: The `AssociatedConstantFact` type does not carry a value expression or definitional reference. The `Self::ERROR_INTERRUPT_MASK` relationship is entirely lost at the identity enrichment level. In MIR, both constants appear as independent `Unsupported` references with no symbolic link between them.
  - Why This Status Follows: The authored premise requires that the definitional relationship survives. Neither the identity enrichment surface nor the MIR body surface preserves the symbolic `Self::ERROR_INTERRUPT_MASK` reference. They are two independent entries.
- Counterexample: `EXPLICIT_ISR_CLEAR_MASK` is defined as `Self::ERROR_INTERRUPT_MASK` in source, but at the extraction boundary they appear as two unrelated constants sharing only a type and owner.
- Suggested Fix Direction / Implementation Hints:
  - Likely Touchpoint: `crates/regseq-driver/src/extract/identity_enrichment.rs` -- the `ImplItemKind::Const(ty, body)` match arm currently discards the body expression. Inspecting the HIR body could recover `Self::ERROR_INTERRUPT_MASK` as a definitional reference.
  - Minimal Missing Fact To Preserve: a `definition_ref: Option<CanonicalPath>` field on `AssociatedConstantFact` that records when a constant's body is a reference to another associated constant.
- Conclusion: The syntactic definitional relationship is NOT preserved at the extraction boundary.

---

## Summary

| Check | Description | Status |
|-------|------------|--------|
| 1a | Constant names preserved | `pass` |
| 1b | Constant Rust types preserved | `pass` |
| 1c | Canonical paths preserved | `pass` |
| 1d | Owner impl path preserved | `pass` |
| 1e | Closed-world constant set | `pass` |
| 1f | Named references in MIR body | `pass` |
| 1g | Literal values evaluated | `fail` |
| 2a | Both constants survive as distinct entries | `pass` |
| 2b | Syntactic definitional relationship | `fail` |

**Result: 7/9 checks pass. 2 checks fail due to two extraction gaps:**

1. **Constant value evaluation gap**: `AssociatedConstantFact` has no `value` field, and the MIR body extractor does not evaluate unevaluated associated constants to scalar values. Constants appear as `MirConstValue::Unsupported` in MIR rather than `MirConstValue::Scalar`.

2. **Definitional relationship gap**: `AssociatedConstantFact` has no `definition_ref` or `value_expr` field. The HIR body of `ImplItemKind::Const` is discarded during identity enrichment extraction, losing the `Self::ERROR_INTERRUPT_MASK` cross-reference.
