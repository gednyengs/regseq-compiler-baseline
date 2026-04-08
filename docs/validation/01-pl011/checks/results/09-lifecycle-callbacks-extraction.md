# Validation Certificate: PL011 Lifecycle Callbacks Extraction Check

Source premise: `docs/validation/01-pl011/checks/09-lifecycle-callbacks-extraction.md`
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_09_lifecycle_callbacks.rs`

## Specimen Fidelity

- Specimen type: reduced inline specimen (`LIFECYCLE_SPECIMEN`)
- Fidelity Assertion Predicate(s):
  - `fixture.contains("self.toggle_reset_if_present()?;")` -- init call 1
  - `fixture.contains("self.apply_default_pins_if_present()?;")` -- init call 2
  - `fixture.contains("self.enable_clock_if_present()?;")` -- init call 3
  - `fixture.contains("self.program_polling_port(self.uart_clock_hz(), self.current_speed)")` -- init call 4 with exact args
  - `fixture.contains("fn init(&mut self) -> Result<(), RegseqError>")` -- init signature
  - `fixture.contains("self.shutdown_port();")` -- deinit call 1
  - `fixture.contains("self.disable_clock_if_present();")` -- deinit call 2
  - `fixture.contains("self.apply_sleep_pins_if_present()?;")` -- deinit call 3
  - `fixture.contains("fn deinit(&mut self) -> Result<(), RegseqError>")` -- deinit signature
  - `fixture.contains("impl Lifecycle for Pl011FullSpec")` -- trait impl
  - `fixture.contains("fn toggle_reset_if_present(&self) -> Result<(), RegseqError>")` -- helper fallibility
  - `fixture.contains("fn apply_default_pins_if_present(&self) -> Result<(), RegseqError>")` -- helper fallibility
  - `fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>")` -- helper fallibility
  - `fixture.contains("fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)")` -- infallible with exact params
  - `fixture.contains("fn shutdown_port(&mut self)")` -- infallible
  - `fixture.contains("fn disable_clock_if_present(&self)")` -- infallible
  - `fixture.contains("fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError>")` -- fallible
  - Corresponding specimen-side assertions for all of the above
- Fidelity Evidence (Exact Citations):
  - Test `specimen_fidelity_against_fixture` in `source_to_extraction_09_lifecycle_callbacks.rs`
  - Reads `tests/fixtures/pl011-full-spec/src/driver.rs` at runtime
  - Every authored fact is checked in both fixture and specimen

## Check-to-Surface Mapping

| Check | Extraction Surface | Test Function |
|-------|--------------------|---------------|
| CG1: trait identity | `TraitSurface.lifecycle_impls` | `check_group_1_trait_surface_lifecycle_identity` |
| CG1: impl block identity | `IdentityEnrichmentSurface.impl_blocks` | `check_group_1_identity_enrichment_lifecycle_impl_block` |
| CG1: init signature | `MirBodySurface.functions[init].kind/receiver/return_type/params` | `check_group_1_body_surface_init_signature` |
| CG1: deinit signature | `MirBodySurface.functions[deinit].kind/receiver/return_type/params` | `check_group_1_body_surface_deinit_signature` |
| CG2: init helper order | `MirBodySurface.functions[init].body` basic block call sequence | `check_group_2_init_helper_call_order` |
| CG2: init error propagation | `MirBodySurface.functions[init].body` Try::branch/SwitchInt pattern | `check_group_2_init_error_propagation` |
| CG2: init success return | `MirBodySurface.functions[init].body` Adt(Ok, variant_index=0) at _0 | `check_group_2_init_success_return` |
| CG2: init argument flow | `MirBodySurface.functions[init].body` call args + def-chain tracing | `check_group_2_init_argument_flow_into_program_polling_port` |
| CG2: deinit helper order | `MirBodySurface.functions[deinit].body` basic block call sequence | `check_group_2_deinit_helper_call_order` |
| CG2: deinit error propagation | `MirBodySurface.functions[deinit].body` Try::branch/SwitchInt pattern | `check_group_2_deinit_error_propagation` |
| CG2: deinit success return | `MirBodySurface.functions[deinit].body` Adt(Ok, variant_index=0) at _0 | `check_group_2_deinit_success_return` |

## Check Group 1

### Check: Lifecycle trait identity survives extraction

- Status: `pass`
- Premise: lifecycle callback identity, trait role, signature, and return type survive
- Assertion Predicate(s):
  - `surface.traits.lifecycle_impls.len() == 1`
  - `surface.traits.lifecycle_impls[0].driver_type == "Pl011FullSpec"`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_trait_surface_lifecycle_identity`: extracts `TraitSurface` from the reduced specimen and asserts exactly one `Lifecycle` impl for `Pl011FullSpec`
- Reasoning Trace:
  - Authored Fact (Verbatim): `impl Lifecycle for Pl011FullSpec`
  - Extracted Fact (Verbatim): `lifecycle_impls[0].driver_type == "Pl011FullSpec"` with length 1
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the extraction surface yields exactly one lifecycle impl for the correct driver type

### Check: Lifecycle impl block identity survives in enrichment surface

- Status: `pass`
- Premise: lifecycle callback identity and trait association survive in identity enrichment
- Assertion Predicate(s):
  - `lifecycle_impl.trait_name == Some("Lifecycle")`
  - `lifecycle_impl.self_type_name == "Pl011FullSpec"`
  - `method_names.contains(&"init")`
  - `method_names.contains(&"deinit")`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_identity_enrichment_lifecycle_impl_block`: finds the Lifecycle impl block in `IdentityEnrichmentSurface.impl_blocks` and asserts init and deinit methods are present
- Reasoning Trace:
  - Authored Fact (Verbatim): `impl Lifecycle for Pl011FullSpec { fn init...; fn deinit...; }`
  - Extracted Fact (Verbatim): impl block with `trait_name=Some("Lifecycle")`, `self_type_name="Pl011FullSpec"`, methods ["init", "deinit"]
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: identity enrichment surfaces both methods under the correct trait impl

### Check: init callback signature survives extraction

- Status: `pass`
- Premise: init callback identity, trait classification, receiver, return type, and parameter count survive
- Assertion Predicate(s):
  - `init_fn.kind == Callback { trait_name: "Lifecycle", method_name: "init" }`
  - `init_fn.receiver == "ref_mut"`
  - `init_fn.return_type contains "Result"`
  - `init_fn.params.is_empty()`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_body_surface_init_signature`: inspects `MirBodySurface.functions` for the init function and asserts all four properties
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn init(&mut self) -> Result<(), RegseqError>` with no extra params
  - Extracted Fact (Verbatim): `kind=Callback{Lifecycle,init}`, `receiver=ref_mut`, `return_type=...Result...`, `params=[]`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all four signature properties match

### Check: deinit callback signature survives extraction

- Status: `pass`
- Premise: deinit callback identity, trait classification, receiver, return type, and parameter count survive
- Assertion Predicate(s):
  - `deinit_fn.kind == Callback { trait_name: "Lifecycle", method_name: "deinit" }`
  - `deinit_fn.receiver == "ref_mut"`
  - `deinit_fn.return_type contains "Result"`
  - `deinit_fn.params.is_empty()`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_1_body_surface_deinit_signature`: inspects `MirBodySurface.functions` for the deinit function and asserts all four properties
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn deinit(&mut self) -> Result<(), RegseqError>` with no extra params
  - Extracted Fact (Verbatim): `kind=Callback{Lifecycle,deinit}`, `receiver=ref_mut`, `return_type=...Result...`, `params=[]`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all four signature properties match

## Check Group 2

### Check: init helper-call order survives extraction

- Status: `pass`
- Premise: init helper-call ordering is preserved exactly: toggle_reset_if_present, apply_default_pins_if_present, enable_clock_if_present, program_polling_port
- Assertion Predicate(s):
  - `init_fn.helper_calls` contains all five helpers (including uart_clock_hz)
  - `pos(toggle_reset_if_present) < pos(apply_default_pins_if_present)`
  - `pos(apply_default_pins_if_present) < pos(enable_clock_if_present)`
  - `pos(enable_clock_if_present) < pos(program_polling_port)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_helper_call_order`: extracts call sequence from MIR basic blocks (non-cleanup) via `extract_call_sequence()`, finds positional indices of each helper, asserts strict ordering
- Reasoning Trace:
  - Authored Fact (Verbatim): init calls toggle_reset_if_present, apply_default_pins_if_present, enable_clock_if_present, program_polling_port in that order
  - Extracted Fact (Verbatim): MIR call sequence yields these functions in strictly increasing position order
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: positional ordering in MIR call sequence matches authored order exactly

### Check: init error propagation survives extraction

- Status: `pass`
- Premise: first three init calls preserve early RegseqError propagation; program_polling_port is infallible
- Assertion Predicate(s):
  - `has_error_propagation_after_call(body, "toggle_reset_if_present") == true`
  - `has_error_propagation_after_call(body, "apply_default_pins_if_present") == true`
  - `has_error_propagation_after_call(body, "enable_clock_if_present") == true`
  - `has_error_propagation_after_call(body, "program_polling_port") == false`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_error_propagation`: walks MIR CFG forward from each call site looking for Try::branch -> Discriminant+SwitchInt pattern (the MIR desugaring of `?`)
- Reasoning Trace:
  - Authored Fact (Verbatim): first three calls use `?`, fourth is infallible
  - Extracted Fact (Verbatim): first three show Try::branch+SwitchInt pattern; fourth does not
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the `?` desugaring pattern is present for exactly the fallible calls and absent for the infallible one

### Check: init success return path survives extraction

- Status: `pass`
- Premise: init success path returns Ok(())
- Assertion Predicate(s):
  - `has_ok_unit_return(init_fn.body) == true`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_success_return`: scans MIR for Aggregate(Adt, variant_index=0) assigned to _0 (return place)
- Reasoning Trace:
  - Authored Fact (Verbatim): `Ok(())`
  - Extracted Fact (Verbatim): Adt construction with variant_index 0 (Ok) at local 0 (return place)
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: variant_index 0 of Result is Ok; assignment to _0 is the return place

### Check: init argument flow into program_polling_port survives extraction

- Status: `pass`
- Premise: first argument to program_polling_port comes from self.uart_clock_hz(); second from self.current_speed
- Assertion Predicate(s):
  - `pos(uart_clock_hz) < pos(program_polling_port)` in call sequence
  - `args.len() >= 3` (self + 2 params)
  - `local_traces_to_call(body, args[1].local, "uart_clock_hz") == true`
  - `local_traces_to_self_field(body, args[2].local) == true` OR `operand_is_self_field_access(args[2]) == true`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_init_argument_flow_into_program_polling_port`:
    1. Verifies uart_clock_hz is called before program_polling_port
    2. Finds the Call terminator for program_polling_port and inspects its args
    3. Traces args[1] backward through the def-chain to prove it originates from the uart_clock_hz call destination
    4. Traces args[2] backward through the def-chain to prove it originates from a self-field access (local 1 with Deref+Field projection)
- Reasoning Trace:
  - Authored Fact (Verbatim): `self.program_polling_port(self.uart_clock_hz(), self.current_speed)`
  - Extracted Fact (Verbatim): MIR Call to program_polling_port with arg[1] defined by uart_clock_hz call result, arg[2] defined by field projection on _1 (self)
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: def-chain tracing proves the actual data flow, not just call ordering

### Check: deinit helper-call order survives extraction

- Status: `pass`
- Premise: deinit helper-call ordering is preserved exactly: shutdown_port, disable_clock_if_present, apply_sleep_pins_if_present
- Assertion Predicate(s):
  - `deinit_fn.helper_calls` contains all three helpers
  - `pos(shutdown_port) < pos(disable_clock_if_present)`
  - `pos(disable_clock_if_present) < pos(apply_sleep_pins_if_present)`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_deinit_helper_call_order`: extracts call sequence from MIR basic blocks and asserts strict positional ordering
- Reasoning Trace:
  - Authored Fact (Verbatim): deinit calls shutdown_port, disable_clock_if_present, apply_sleep_pins_if_present in that order
  - Extracted Fact (Verbatim): MIR call sequence yields these in strictly increasing position order
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: positional ordering matches authored order

### Check: deinit error propagation survives extraction

- Status: `pass`
- Premise: shutdown_port and disable_clock_if_present are infallible; apply_sleep_pins_if_present preserves early RegseqError propagation
- Assertion Predicate(s):
  - `has_error_propagation_after_call(body, "shutdown_port") == false`
  - `has_error_propagation_after_call(body, "disable_clock_if_present") == false`
  - `has_error_propagation_after_call(body, "apply_sleep_pins_if_present") == true`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_deinit_error_propagation`: walks MIR CFG forward from each call site checking for Try::branch+SwitchInt pattern
- Reasoning Trace:
  - Authored Fact (Verbatim): first two are infallible, third uses `?`
  - Extracted Fact (Verbatim): first two lack Try::branch pattern; third has it
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: error propagation pattern present only for the fallible call

### Check: deinit success return path survives extraction

- Status: `pass`
- Premise: deinit success path returns Ok(())
- Assertion Predicate(s):
  - `has_ok_unit_return(deinit_fn.body) == true`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check_group_2_deinit_success_return`: scans MIR for Aggregate(Adt, variant_index=0) assigned to _0
- Reasoning Trace:
  - Authored Fact (Verbatim): `Ok(())`
  - Extracted Fact (Verbatim): Adt construction with variant_index 0 at local 0
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: variant_index 0 of Result is Ok; _0 is the return place

## Conclusion

All checks pass. The authored lifecycle-callback facts for PL011 -- callback identity,
trait association, signature, helper-call sequencing, error propagation patterns,
success return paths, and argument flow into `program_polling_port` -- are fully
preserved at the compiler-side extraction boundary.
