# Validation Certificate: PL011 Driver Constants Extraction Check

- Source premise: `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs`
- Extraction surfaces inspected:
  - `IdentityEnrichmentSurface.impl_blocks` via `extract_identity_enrichment(tcx)` (`crates/regseq-driver/src/extract/identity_enrichment.rs`)
  - `MirBodySurface.functions` via `extract_body_surface(tcx)` (`crates/regseq-driver/src/extract/body.rs`)

## Specimen Fidelity

The validation uses a reduced inline specimen compiled with `--crate-name pl011_full_spec`
and a `pub mod driver` wrapper, so that canonical paths match the real fixture.
The specimen contains a `#[rseq::driver]`-annotated `Pl011FullSpec` struct and an
inherent impl block with all six authored constants. A `use_constants` helper and an
`#[rseq::isr]`-annotated `handle_irq` method ensure the constants appear in MIR body
extraction.

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
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:271`

## Check-to-Surface Mapping

| Check | Extraction Surface(s) | Test Function |
|-------|----------------------|---------------|
| 1a | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (name) | `check1a_all_constants_present_by_name` |
| 1b | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (ty) | `check1b_constant_types_preserved` |
| 1c | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (canonical_path) + `modules` (crate root) | `check1c_canonical_paths_preserved` |
| 1d | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (owner_impl_path) | `check1d_owner_impl_path_preserved` |
| 1e | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (closed-world count) | `check1e_closed_world_constant_set` |
| 1f | `MirBodySurface.functions["use_constants"]` (unevaluated constant refs) | `check1f_constant_references_in_mir_body` |
| 1g | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (evaluated_value) | `check1g_constant_values_evaluated` |
| 2a | `IdentityEnrichmentSurface.impl_blocks` + `MirBodySurface` (both constants survive) | `check2a_both_constants_survive_as_distinct_entries` |
| 2b | `IdentityEnrichmentSurface.impl_blocks[inherent].constants` (initializer_expr) | `check2b_definitional_relationship_on_definition_surface` |

---

## Check 1a

- Check: Constant names preserved
- Status: `pass`
- Premise: all six in-scope constants preserve their names at the extraction boundary
- Assertion Predicate(s):
  - `assert!(extracted_names.contains(&authored.name))` for each of the six constants
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:375`
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
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:400`
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
- Premise: all six constants preserve their full canonical paths
- Assertion Predicate(s):
  - `assert_eq!(crate_root.canonical_path.0, "pl011_full_spec")` -- crate root is correct
  - `assert_eq!(reconstructed, authored.canonical_path)` for each constant, where `reconstructed = crate_root_path + "::" + extracted.canonical_path.0`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:434`
  - `tcx.def_path_str()` produces paths without the crate-name prefix (e.g., `driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK`). The crate root module path `pl011_full_spec` is stored separately in `IdentityEnrichmentSurface.modules`. The full authored canonical path is exactly reconstructable by prepending `pl011_full_spec::`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK` etc.
  - Extracted Fact (Verbatim): `canonical_path: "driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"` with crate root `"pl011_full_spec"`
  - Correspondence: `exact` (reconstructable)
  - Gap / Loss Description: The crate-name prefix is not embedded in the constant's `canonical_path` field, but is available via the crate root module. The full path is exactly reconstructable.
  - Why This Status Follows: The reconstructed path `pl011_full_spec:: + def_path_str` matches the authored canonical path exactly; the assertion would fail on any path component mismatch.
- Counterexample: none found
- Conclusion: Constant canonical paths are exactly reconstructable at the extraction boundary.

---

## Check 1d

- Check: Owner impl/path association preserved
- Status: `pass`
- Premise: all six constants have `owner_impl_path` pointing to the owning inherent impl block
- Assertion Predicate(s):
  - `assert_eq!(extracted.owner_impl_path, inherent.canonical_path)` for each constant
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:482`
  - Each `AssociatedConstantFact.owner_impl_path` matches the canonical path of the inherent impl block for `Pl011FullSpec`.
- Reasoning Trace:
  - Authored Fact (Verbatim): constants are declared inside `impl Pl011FullSpec { ... }`
  - Extracted Fact (Verbatim): `AssociatedConstantFact { owner_impl_path: <inherent_impl_canonical_path>, .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The owner path assertion would fail if any constant pointed to a different impl block.
- Counterexample: none found
- Conclusion: Owner impl/path association is preserved.

---

## Check 1e

- Check: Closed-world constant set
- Status: `pass`
- Premise: exactly six constants exist in the inherent impl block, matching the authored set
- Assertion Predicate(s):
  - `assert_eq!(inherent.constants.len(), AUTHORED_CONSTANTS.len())` -- exactly six
  - `assert!(extracted_names.contains(name))` for each authored name
  - `assert!(authored_names.contains(name))` for each extracted name
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:511`
  - The extraction contains exactly six constants with no extras and no losses.
- Reasoning Trace:
  - Authored Fact (Verbatim): six named constants
  - Extracted Fact (Verbatim): six `AssociatedConstantFact` entries
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Count and bidirectional membership assertions would fail on any discrepancy.
- Counterexample: none found
- Conclusion: The extracted constant set is a closed-world exact match of the authored set.

---

## Check 1f

- Check: Constant references in MIR body
- Status: `pass`
- Premise: all six constants appear as named references in the MIR body of `use_constants`
- Assertion Predicate(s):
  - `assert!(unevaluated_refs.iter().any(|desc| desc == &expected))` for each constant, where `expected = "unevaluated:" + def_path_str`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:563`
  - Each constant appears as a `MirConstValue::Unsupported { description: "unevaluated:driver::Pl011FullSpec::<NAME>" }` in the MIR body. The unevaluated form preserves the constant identity at use sites.
- Reasoning Trace:
  - Authored Fact (Verbatim): six constant references in `use_constants`
  - Extracted Fact (Verbatim): six `MirConstValue::Unsupported { description: "unevaluated:driver::Pl011FullSpec::<NAME>" }` entries
  - Correspondence: `exact` (name-preserving)
  - Gap / Loss Description: The MIR form uses `def_path_str` without crate prefix, consistent with the identity surface.
  - Why This Status Follows: The exact-match assertion on the unevaluated description string would fail if any constant reference were missing or misnamed.
- Counterexample: none found
- Conclusion: Constant references survive in MIR as named unevaluated references.

---

## Check 1g

- Check: Constant literal values evaluated at extraction boundary
- Status: `pass`
- Premise: constant literal values (e.g., `0x07ff`, `48_000_000`) should be preserved
- Assertion Predicate(s):
  - `assert_eq!(extracted.evaluated_value, Some(authored.evaluated_value))` for each of the six constants
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:615`
  - The identity enrichment surface `AssociatedConstantFact.evaluated_value` is populated by `tcx.const_eval_poly(def_id)` during extraction. Each constant carries its evaluated scalar value as `Some(u128)`.
- Reasoning Trace:
  - Authored Fact (Verbatim): `ALL_UART_INTERRUPTS_MASK = 0x07ff`, `RX_INTERRUPT_MASK = (1 << 4) | (1 << 6)`, etc.
  - Extracted Fact (Verbatim): `AssociatedConstantFact { evaluated_value: Some(2047), .. }` for `ALL_UART_INTERRUPTS_MASK`, etc.
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The assertion matches each constant's evaluated_value against its authored evaluated form. The assertion would fail if any constant had an incorrect or missing value.
- Counterexample: none found
- Conclusion: Constant literal values are preserved at the extraction boundary via `AssociatedConstantFact.evaluated_value`.

---

## Check 2a

- Check: Both `EXPLICIT_ISR_CLEAR_MASK` and `ERROR_INTERRUPT_MASK` survive as distinct entries
- Status: `pass`
- Premise: both constants survive as distinct named entries in both extraction surfaces
- Assertion Predicate(s):
  - `inherent.constants.iter().find(|c| c.name == "EXPLICIT_ISR_CLEAR_MASK").expect(...)` -- exists
  - `inherent.constants.iter().find(|c| c.name == "ERROR_INTERRUPT_MASK").expect(...)` -- exists
  - `assert_eq!(explicit_isr.ty, "u32")` and `assert_eq!(error_mask.ty, "u32")`
  - `assert_eq!(explicit_isr.owner_impl_path, error_mask.owner_impl_path)` -- same owner
  - `assert_ne!(explicit_isr.canonical_path, error_mask.canonical_path)` -- distinct paths
  - MIR: `assert!(unevaluated.iter().any(|d| d.contains("EXPLICIT_ISR_CLEAR_MASK")))` and `assert!(unevaluated.iter().any(|d| d.contains("ERROR_INTERRUPT_MASK") && !d.contains("EXPLICIT_ISR_CLEAR_MASK")))`
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:679`
  - Both constants are present as distinct `AssociatedConstantFact` entries with the same type and owner but different canonical paths. Both appear as distinct unevaluated references in MIR.
- Reasoning Trace:
  - Authored Fact (Verbatim): `EXPLICIT_ISR_CLEAR_MASK` and `ERROR_INTERRUPT_MASK` as separate constants
  - Extracted Fact (Verbatim): Two distinct `AssociatedConstantFact` entries and two distinct MIR unevaluated references
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Both existence and distinctness assertions would fail if either constant were missing or if they collapsed into one entry.
- Counterexample: none found
- Conclusion: Both constants survive as distinct entries at the extraction boundary.

---

## Check 2b

- Check: Definitional relationship `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`
- Status: `pass`
- Premise: the syntactic definitional relationship between `EXPLICIT_ISR_CLEAR_MASK` and `ERROR_INTERRUPT_MASK` should be preserved
- Assertion Predicate(s):
  - `assert_ne!(explicit_isr.canonical_path, error_mask.canonical_path)` -- distinct entries exist
  - `assert_eq!(explicit_isr.owner_impl_path, error_mask.owner_impl_path)` -- same owner
  - `assert!(init_expr.contains("ERROR_INTERRUPT_MASK"))` -- initializer_expr references `ERROR_INTERRUPT_MASK`
  - `assert!(!error_init.contains("EXPLICIT_ISR_CLEAR_MASK"))` -- ERROR_INTERRUPT_MASK does not reference EXPLICIT_ISR_CLEAR_MASK
- Target Evidence (Exercised by Test; Exact Citations):
  - `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs:720`
  - The identity enrichment surface `AssociatedConstantFact.initializer_expr` is populated from the HIR body of each constant. For `EXPLICIT_ISR_CLEAR_MASK`, the initializer is rendered as `"Self::ERROR_INTERRUPT_MASK"`, preserving the authored definitional dependency.
- Reasoning Trace:
  - Authored Fact (Verbatim): `const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;`
  - Extracted Fact (Verbatim): `AssociatedConstantFact { initializer_expr: Some("Self::ERROR_INTERRUPT_MASK"), .. }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The initializer_expr for `EXPLICIT_ISR_CLEAR_MASK` contains `"ERROR_INTERRUPT_MASK"`, directly proving the authored definitional relationship. The assertion would fail if the initializer did not reference `ERROR_INTERRUPT_MASK`.
- Counterexample: none found
- Conclusion: The syntactic definitional relationship is preserved at the extraction boundary via `AssociatedConstantFact.initializer_expr`.

---

## Summary

| Check | Status |
|-------|--------|
| 1a: Constant names | `pass` |
| 1b: Constant types | `pass` |
| 1c: Canonical paths | `pass` |
| 1d: Owner impl path | `pass` |
| 1e: Closed-world set | `pass` |
| 1f: MIR body references | `pass` |
| 1g: Literal value evaluation | `pass` |
| 2a: Distinct entry survival | `pass` |
| 2b: Definitional relationship | `pass` |

9 of 9 checks pass. All extraction gaps from the previous report have been resolved:

1. **Constant value evaluation**: `AssociatedConstantFact.evaluated_value` is populated via `tcx.const_eval_poly(def_id)` during identity enrichment extraction.

2. **Definitional relationship**: `AssociatedConstantFact.initializer_expr` is populated by rendering the HIR body expression, preserving authored relationships like `Self::ERROR_INTERRUPT_MASK`.
