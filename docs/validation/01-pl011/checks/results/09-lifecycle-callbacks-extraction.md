# Validation Certificate: PL011 Lifecycle Callbacks Extraction Check

- Source premise: `docs/validation/01-pl011/01-authored-source.md` (Lifecycle Callbacks section)
- Target boundary evidence: `crates/regseq-driver/tests/lifecycle_callbacks_extraction.rs`

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
    `crates/regseq-driver/tests/lifecycle_callbacks_extraction.rs`
    (32 assertion predicates covering init body lines, deinit body lines,
    callback signatures, trait impl presence, helper fallibility, and
    specimen-to-fixture cross-checks).

## Check-to-Surface Mapping

| Check | Extraction Surface(s) |
|---|---|
| Check Group 1 (identity, trait, signature, return) | `TraitSurface.lifecycle_impls`, `IdentityEnrichmentSurface.impl_blocks`, `MirBodySurface.functions` (kind, receiver, params, return_type) |
| Check Group 2 (bodies: sequencing, propagation, return) | `MirBodySurface.functions` (body basic blocks, helper_calls) |

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
  - `init_fn.return_type contains "Result"` -- `Result<(), RegseqError>` return type
  - `init_fn.params.is_empty()` -- no extra parameters
  - (same four predicates for `deinit`)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_trait_surface_lifecycle_identity`: asserts exactly one Lifecycle impl for Pl011FullSpec on `TraitSurface`
  - `check_group_1_identity_enrichment_lifecycle_impl_block`: asserts Lifecycle impl block with init and deinit methods on `IdentityEnrichmentSurface`
  - `check_group_1_body_surface_init_signature`: asserts init is a Lifecycle Callback with ref_mut receiver, Result return, no extra params on `MirBodySurface`
  - `check_group_1_body_surface_deinit_signature`: asserts deinit is a Lifecycle Callback with ref_mut receiver, Result return, no extra params on `MirBodySurface`
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs` lines 268-286
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn init(&mut self) -> Result<(), RegseqError>` and `fn deinit(&mut self) -> Result<(), RegseqError>` in `impl Lifecycle for Pl011FullSpec`
  - Extracted Fact (Verbatim): `MirFnKind::Callback { trait_name: "Lifecycle", method_name: "init" }`, `receiver: "ref_mut"`, `return_type: Some("Result<(), RegseqError>")`; same for deinit
  - Correspondence: `exact`
  - Gap / Loss Description: none
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
  - **init success return**: `has_ok_unit_return(body)` -- MIR body contains `Aggregate { Adt { variant_index: 0, ... } }` assigned to local 0 (return place)
  - **init argument flow**: `pos(uart_clock_hz) < pos(program_polling_port)` -- uart_clock_hz called before program_polling_port, result feeds as argument
  - **deinit helper sequencing**: `pos(shutdown_port) < pos(disable_clock_if_present) < pos(apply_sleep_pins_if_present)` -- ordering verified
  - **deinit error propagation**: `!has_error_propagation_after_call(body, "shutdown_port")` and `!has_error_propagation_after_call(body, "disable_clock_if_present")` -- both infallible; `has_error_propagation_after_call(body, "apply_sleep_pins_if_present")` -- fallible with `?`
  - **deinit success return**: `has_ok_unit_return(body)` -- same Ok(()) return pattern
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_helper_call_order`: asserts helper_calls membership and positional ordering for all four init helpers
  - `check_group_2_init_error_propagation`: asserts `?` propagation present for first three helpers and absent for program_polling_port
  - `check_group_2_init_success_return`: asserts Ok(()) return path exists
  - `check_group_2_init_argument_flow`: asserts uart_clock_hz precedes program_polling_port
  - `check_group_2_deinit_helper_call_order`: asserts helper_calls membership and positional ordering for all three deinit helpers
  - `check_group_2_deinit_error_propagation`: asserts infallible helpers have no `?` and apply_sleep_pins_if_present has `?`
  - `check_group_2_deinit_success_return`: asserts Ok(()) return path exists
- Supporting Evidence: fixture source `tests/fixtures/pl011-full-spec/src/driver.rs` lines 269-286
- Reasoning Trace:
  - Authored Fact (Verbatim): init calls `toggle_reset_if_present()?`, `apply_default_pins_if_present()?`, `enable_clock_if_present()?`, `program_polling_port(self.uart_clock_hz(), self.current_speed)`, then `Ok(())`; deinit calls `shutdown_port()`, `disable_clock_if_present()`, `apply_sleep_pins_if_present()?`, then `Ok(())`
  - Extracted Fact (Verbatim): MIR body call sequence preserves the exact helper ordering; `?` desugars to `Try::branch` + `Discriminant` + `SwitchInt` + `FromResidual::from_residual` pattern in the CFG; Ok(()) constructed as `Aggregate { Adt { variant_index: 0 } }` on return place
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: every callback-specific required predicate from the check specification (helper ordering, fallible propagation, infallible absence of propagation, success return, argument flow) is covered by non-tautological assertions that inspect the actual MIR body structure
- Counterexample: n/a
- Suggested Fix Direction / Implementation Hints: n/a
- Conclusion: pass -- lifecycle callback bodies preserve helper sequencing, error propagation patterns, success return, and argument flow through MIR extraction
