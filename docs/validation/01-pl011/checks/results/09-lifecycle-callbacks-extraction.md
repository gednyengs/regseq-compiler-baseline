# Validation Certificate: PL011 Lifecycle Callbacks Extraction Check

- Source premise: `docs/validation/01-pl011/01-authored-source.md` (Lifecycle Callbacks section)
- Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_09_lifecycle_callbacks.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - The reduced specimen's `init` body contains exactly:
    `self.toggle_reset_if_present()?`, `self.apply_default_pins_if_present()?`,
    `self.enable_clock_if_present()?`,
    `self.program_polling_port(self.uart_clock_hz(), self.current_speed)`,
    and `Ok(())` -- each verified to be present in the real fixture via
    `fixture.contains(...)` assertions.
  - The reduced specimen's `deinit` body contains exactly:
    `self.shutdown_port()`, `self.disable_clock_if_present()`,
    `self.apply_sleep_pins_if_present()?`, and `Ok(())` -- each verified
    against the real fixture.
  - Helper fallibility signatures match the fixture: `toggle_reset_if_present`,
    `apply_default_pins_if_present`, `enable_clock_if_present`, and
    `apply_sleep_pins_if_present` return `Result<(), RegseqError>`;
    `program_polling_port`, `shutdown_port`, and `disable_clock_if_present`
    are infallible -- all verified with exact signature substring matches.
  - The `impl Lifecycle for Pl011FullSpec` block is present in both the
    fixture and the specimen.
- Fidelity Evidence (Exact Citations):
  - Test `specimen_fidelity_against_fixture` in
    `crates/regseq-driver/tests/source_to_extraction_09_lifecycle_callbacks.rs`
    (32 assertion predicates covering init body lines, deinit body lines,
    callback signatures, trait impl presence, helper fallibility, and
    specimen-to-fixture cross-checks).

## Check-to-Surface Mapping

| Check | Extraction Surface(s) |
|---|---|
| Check Group 1 (identity, trait, signature, return) | `TraitSurface.lifecycle_impls`, `IdentityEnrichmentSurface.impl_blocks`, `MirBodySurface.functions` (kind, receiver, params, return_type) |
| Check Group 2 (bodies: sequencing, propagation, return, argument flow) | `MirBodySurface.functions` (body basic blocks, helper_calls, Call terminator args/destination) |

---

## Check Group 1

- Check: lifecycle callback identity, trait role, signature, and return type survive
- Status: `pass`
- Premise: lifecycle callback identity, trait role, signature, and return type survive
- Assertion Predicate(s):
  - `surface.traits.lifecycle_impls.len() == 1` -- exactly one Lifecycle impl found
  - `surface.traits.lifecycle_impls[0].driver_type == "Pl011FullSpec"` -- correct driver type
  - `lifecycle_impl.trait_name == Some("Lifecycle")` -- Lifecycle trait impl block found in identity surface
  - `lifecycle_impl.self_type_name == "Pl011FullSpec"` -- correct self type
  - `method_names.contains("init")` and `method_names.contains("deinit")` -- both methods present
  - `init_fn.kind == Callback { trait_name: "Lifecycle", method_name: "init" }` -- correct callback classification
  - `init_fn.receiver == "ref_mut"` -- `&mut self` receiver
  - `init_fn.return_type contains "Result<(), RegseqError>"` -- full generic return type preserved
  - `init_fn.params.is_empty()` -- no extra parameters
  - (same four predicates for `deinit`)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_trait_surface_lifecycle_identity`: asserts exactly one Lifecycle impl for Pl011FullSpec on `TraitSurface`
  - `check_group_1_identity_enrichment_lifecycle_impl_block`: asserts Lifecycle impl block with init and deinit methods on `IdentityEnrichmentSurface`
  - `check_group_1_body_surface_init_signature`: asserts init is a Lifecycle Callback with ref_mut receiver, `Result<(), RegseqError>` return, no extra params on `MirBodySurface`
  - `check_group_1_body_surface_deinit_signature`: asserts deinit is a Lifecycle Callback with ref_mut receiver, `Result<(), RegseqError>` return, no extra params on `MirBodySurface`
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn init(&mut self) -> Result<(), RegseqError>` and `fn deinit(&mut self) -> Result<(), RegseqError>` in `impl Lifecycle for Pl011FullSpec`
  - Extracted Fact (Verbatim): `MirFnKind::Callback { trait_name: "Lifecycle", method_name: "init" }`, `receiver: "ref_mut"`, `return_type: Some("Result<(), RegseqError>")`; same for deinit
  - Correspondence: `exact`
  - Gap / Loss Description: none. The `render_path_segment` helper was enhanced to include generic type arguments in the rendered return type string (previously only the bare type name was emitted).
  - Why This Status Follows: all four named properties (identity, trait role, signature, return type) are independently verified by non-tautological assertions against the extraction surfaces
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- lifecycle callback identity, trait association, signature, and return type are fully preserved through extraction

---

## Check Group 2

- Check: lifecycle callback bodies preserve the downstream checks captured in the authored-source entries
- Status: `pass`
- Premise: extraction preserves enough body information to prove helper sequencing, fallible propagation, success return, and argument flow
- Assertion Predicate(s):
  - **init helper sequencing**: `pos(toggle_reset_if_present) < pos(apply_default_pins_if_present) < pos(enable_clock_if_present) < pos(program_polling_port)` -- ordering verified from `extract_call_sequence` over non-cleanup MIR basic blocks
  - **init error propagation**: `has_error_propagation_after_call(body, "toggle_reset_if_present")`, `has_error_propagation_after_call(body, "apply_default_pins_if_present")`, `has_error_propagation_after_call(body, "enable_clock_if_present")` all return true (verified by following Call -> Try::branch -> Discriminant+SwitchInt pattern in MIR)
  - **init infallible program_polling_port**: `!has_error_propagation_after_call(body, "program_polling_port")` -- program_polling_port has no `?` propagation
  - **init success return**: `has_ok_unit_return(body)` -- MIR body contains `Aggregate { Adt { variant_index: 0 } }` assigned to local 0 (return place)
  - **init argument flow (uart_clock_hz -> program_polling_port arg 1)**:
    - `find_call_terminator(body, "uart_clock_hz")` yields destination local `L_clk`
    - `find_call_terminator(body, "program_polling_port")` yields args list with >= 3 entries
    - `operand_references_local(args[1], L_clk)` -- arg 1 of program_polling_port is a Copy/Move of the local holding uart_clock_hz()'s return value
  - **init argument flow (self.current_speed -> program_polling_port arg 2)**:
    - `operand_is_field_on_local(args[2], 1)` or indirect field assignment tracing -- arg 2 is a field projection on the self parameter (local 1), proving `self.current_speed` flows as the second argument
  - **deinit helper sequencing**: `pos(shutdown_port) < pos(disable_clock_if_present) < pos(apply_sleep_pins_if_present)` -- ordering verified
  - **deinit error propagation**: `!has_error_propagation_after_call(body, "shutdown_port")` and `!has_error_propagation_after_call(body, "disable_clock_if_present")` -- both infallible; `has_error_propagation_after_call(body, "apply_sleep_pins_if_present")` -- fallible with `?`
  - **deinit success return**: `has_ok_unit_return(body)` -- same Ok(()) return pattern
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_helper_call_order`: asserts helper_calls membership and positional ordering for all four init helpers
  - `check_group_2_init_error_propagation`: asserts `?` propagation present for first three helpers and absent for program_polling_port
  - `check_group_2_init_success_return`: asserts Ok(()) return path exists
  - `check_group_2_init_argument_flow`: inspects MIR Call terminator operands -- verifies that program_polling_port's arg 1 references the destination local of the uart_clock_hz() call, and arg 2 references a Field projection on self (local 1), proving actual operand data flow rather than mere call ordering
  - `check_group_2_deinit_helper_call_order`: asserts helper_calls membership and positional ordering for all three deinit helpers
  - `check_group_2_deinit_error_propagation`: asserts infallible helpers have no `?` and apply_sleep_pins_if_present has `?`
  - `check_group_2_deinit_success_return`: asserts Ok(()) return path exists
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): init calls `toggle_reset_if_present()?`, `apply_default_pins_if_present()?`, `enable_clock_if_present()?`, `program_polling_port(self.uart_clock_hz(), self.current_speed)`, then `Ok(())`; deinit calls `shutdown_port()`, `disable_clock_if_present()`, `apply_sleep_pins_if_present()?`, then `Ok(())`
  - Extracted Fact (Verbatim): MIR body call sequence preserves the exact helper ordering; `?` desugars to `Try::branch` + `Discriminant` + `SwitchInt` + `FromResidual::from_residual` pattern in the CFG; Ok(()) constructed as `Aggregate { Adt { variant_index: 0 } }` on return place; program_polling_port Call terminator args[1] is Move/Copy of uart_clock_hz destination local, args[2] is a Field projection on self (local 1)
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: every callback-specific required predicate from the check specification (helper ordering, fallible propagation, infallible absence of propagation, success return, argument flow) is covered by non-tautological assertions that inspect the actual MIR body structure, including operand-level data flow for argument flow
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- lifecycle callback bodies preserve helper sequencing, error propagation patterns, success return, and argument flow through MIR extraction
