# Validation Certificate: PL011 ISR Extraction Check

Source premise: `docs/validation/01-pl011/01-authored-source.md` (ISR: handle_irq section)
Target boundary evidence: `crates/regseq-driver/tests/isr_extraction.rs`

## Specimen Fidelity

A reduced inline specimen is used. It reproduces the exact ISR body and all
authored facts from the PL011 fixture source (`tests/fixtures/pl011-full-spec/src/driver.rs`).

Fidelity Assertion Predicate(s):
- `fixture.contains("#[rseq::isr]")` -- ISR annotation present in real fixture
- `fixture.contains("pub fn handle_irq(&mut self) -> IrqResult")` -- signature match
- `fixture.contains("let status = self.regs.mis.read_raw();")` -- MIS read match
- `fixture.contains("if status == 0 {")` -- early zero-check match
- `fixture.contains("return IrqResult::NotHandled;")` -- early NotHandled return match
- `fixture.contains("let mut handled = false;")` -- handled init match
- `fixture.contains("if status & Self::RX_INTERRUPT_MASK != 0 {")` -- RX branch match
- `fixture.contains("if status & Self::TX_INTERRUPT_MASK != 0 {")` -- TX branch match
- `fixture.contains("handled |= self.service_rx_fifo();")` -- RX dispatch match
- `fixture.contains("handled |= self.service_tx_fifo();")` -- TX dispatch match
- `fixture.contains("self.clear_explicit_interrupts(status);")` -- clear call match
- `fixture.contains("if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {")` -- final condition match
- `fixture.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);")` -- RX mask value (= 0x50)
- `fixture.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;")` -- TX mask value (= 0x20)
- `fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;")` -- clear mask alias
- `fixture.contains("const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);")` -- error mask value (= 0x0780)
- Every fact is also asserted present in SPECIMEN with identical string predicates

Fidelity Evidence (Exact Citations):
- Test: `precondition_specimen_fidelity` in `crates/regseq-driver/tests/isr_extraction.rs`

## Check-to-Surface Mapping

| Check | Extraction Surface |
|-------|-------------------|
| 1a: ISR binding identity | `InterruptSurface.isr_bindings` |
| 1b: ISR marker in identity enrichment | `IdentityEnrichmentSurface.impl_blocks` |
| 1c: ISR signature and kind in MIR | `MirBodySurface.functions` |
| 2a: Helper dispatch | `MirBodySurface.functions[handle_irq].helper_calls` |
| 2b: Status read and branch conditions | `MirBodySurface.functions[handle_irq].body` |
| 2c: Explicit clear mask | `MirBodySurface.functions[handle_irq].body` |
| 2d: IrqResult variant distinguishability | `MirBodySurface.functions[handle_irq].body` |
| 2e: Handled accumulation via BitOr | `MirBodySurface.functions[handle_irq].body` |
| 2f: Return terminators | `MirBodySurface.functions[handle_irq].body` |
| 2g: Handled local is bool | `MirBodySurface.functions[handle_irq].body.local_decls` |
| 2h: Status read call | `MirBodySurface.functions[handle_irq].body` |
| 2i: Helpers extracted as functions | `MirBodySurface.functions` |

## Check Group 1: ISR identity, annotation, signature, return type survive

### Check 1a: ISR binding identity

- Check: ISR binding for handle_irq exists with correct name, Auto bind target, no index param
- Status: `pass`
- Premise: ISR identity and `#[rseq::isr]` annotation survive in the interrupt extraction surface
- Assertion Predicate(s):
  - `result.interrupt.isr_bindings.len() == 1`
  - `binding.handler_name == "handle_irq"`
  - `binding.bind_target == IsrBindTarget::Auto`
  - `!binding.has_index_param`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1a_isr_binding_identity` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::isr]` bare annotation on `handle_irq`, single interrupt resource
  - Extracted Fact (Verbatim): `IsrBinding { handler_name: "handle_irq", bind_target: Auto, has_index_param: false }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The extraction surface preserves the ISR handler name, auto-bind target (bare annotation), and absence of index parameter exactly as authored.
- Counterexample: none
- Conclusion: pass

### Check 1b: ISR marker in identity enrichment

- Check: handle_irq appears as a method with `is_isr = true` in an inherent impl for Pl011FullSpec
- Status: `pass`
- Premise: ISR identity survives in the identity enrichment surface
- Assertion Predicate(s):
  - inherent impl block for Pl011FullSpec exists
  - method named "handle_irq" found in inherent impl methods
  - `method.is_isr == true`
  - `method.canonical_path.0.contains("handle_irq")`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1b_identity_enrichment_isr_marker` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `#[rseq::isr]` on `handle_irq` method
  - Extracted Fact (Verbatim): `ImplMethodFact { name: "handle_irq", is_isr: true, canonical_path: ... }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Identity enrichment explicitly marks the method as ISR and preserves its canonical path identity.
- Counterexample: none
- Conclusion: pass

### Check 1c: ISR signature and kind in MIR body surface

- Check: handle_irq is classified as Isr with ref_mut receiver, IrqResult return type, no extra params
- Status: `pass`
- Premise: ISR signature and return type survive in MIR body extraction
- Assertion Predicate(s):
  - `handle_irq.kind == MirFnKind::Isr`
  - `handle_irq.receiver == "ref_mut"`
  - `handle_irq.return_type == Some("IrqResult")`
  - `handle_irq.params.is_empty()`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check1c_mir_isr_identity_and_signature` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub fn handle_irq(&mut self) -> IrqResult`
  - Extracted Fact (Verbatim): `MirExtractedFn { name: "handle_irq", kind: Isr, receiver: "ref_mut", return_type: Some("IrqResult"), params: [] }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: All four authored identity properties (ISR kind, receiver, return type, no params) are individually asserted.
- Counterexample: none
- Conclusion: pass

## Check Group 2: ISR body preservation

### Check 2a: Helper dispatch

- Check: handle_irq calls service_rx_fifo, service_tx_fifo, and clear_explicit_interrupts
- Status: `pass`
- Premise: ISR body preserves helper dispatch calls
- Assertion Predicate(s):
  - `handle_irq.helper_calls.contains("service_rx_fifo")`
  - `handle_irq.helper_calls.contains("service_tx_fifo")`
  - `handle_irq.helper_calls.contains("clear_explicit_interrupts")`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2a_helper_dispatch` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `handled |= self.service_rx_fifo()`, `handled |= self.service_tx_fifo()`, `self.clear_explicit_interrupts(status)`
  - Extracted Fact (Verbatim): `helper_calls: ["clear_explicit_interrupts", "service_rx_fifo", "service_tx_fifo"]`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: All three helper calls are present in the extracted helper_calls list.
- Counterexample: none
- Conclusion: pass

### Check 2b: Status read and branch conditions

- Check: MIR preserves SwitchInt with value 0 (early status check), BitAnd with 0x50 (RX mask), BitAnd with 0x20 (TX mask)
- Status: `pass`
- Premise: Status read, early branch, RX/TX branch conditions survive
- Assertion Predicate(s):
  - body contains SwitchInt terminator with value 0
  - body contains BitAnd with scalar constant 0x50 or unevaluated ref to RX_INTERRUPT_MASK
  - body contains BitAnd with scalar constant 0x20 or unevaluated ref to TX_INTERRUPT_MASK
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2b_status_read_and_branch_conditions` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `if status == 0`, `status & Self::RX_INTERRUPT_MASK != 0`, `status & Self::TX_INTERRUPT_MASK != 0`
  - Extracted Fact (Verbatim): SwitchInt(value=0), BitAnd with 0x50 constant, BitAnd with 0x20 constant
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The early zero check becomes a SwitchInt on 0, and the mask constants are preserved as BitAnd operands.
- Counterexample: none
- Conclusion: pass

### Check 2c: Explicit clear mask in final condition

- Check: MIR preserves BitAnd with EXPLICIT_ISR_CLEAR_MASK (0x0780)
- Status: `pass`
- Premise: Final handled condition preserves the explicit clear mask
- Assertion Predicate(s):
  - body contains BitAnd with scalar constant 0x0780 or unevaluated ref to EXPLICIT_ISR_CLEAR_MASK
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2c_explicit_clear_mask_in_final_condition` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `status & Self::EXPLICIT_ISR_CLEAR_MASK != 0` where EXPLICIT_ISR_CLEAR_MASK = 0x0780
  - Extracted Fact (Verbatim): BitAnd with 0x0780 constant in MIR
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The mask value is preserved in the MIR BitAnd operation.
- Counterexample: none
- Conclusion: pass

### Check 2d: IrqResult variant distinguishability

- Check: MIR contains at least 2 distinct IrqResult variant indices (Handled and NotHandled)
- Status: `pass`
- Premise: `IrqResult::{Handled, NotHandled}` remain nominally distinguishable
- Assertion Predicate(s):
  - collect all Adt aggregate variant_index values for paths containing "IrqResult"
  - deduplicated set has >= 2 distinct indices
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2d_irq_result_variants_distinguishable` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `IrqResult::Handled` and `IrqResult::NotHandled` as distinct enum variants
  - Extracted Fact (Verbatim): Adt aggregate constructors with distinct variant indices for IrqResult
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The two variants produce distinct variant_index values in MIR aggregates, proving they are not collapsed.
- Counterexample: none
- Conclusion: pass

### Check 2e: Handled accumulation via BitOr

- Check: MIR contains BitOr operation for `handled |= ...` pattern
- Status: `pass`
- Premise: `handled |= self.service_rx_fifo()` and `handled |= self.service_tx_fifo()` produce BitOr
- Assertion Predicate(s):
  - body contains at least one BinaryOp with op = BitOr
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2e_handled_accumulation_via_bitor` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `handled |= self.service_rx_fifo()` and `handled |= self.service_tx_fifo()`
  - Extracted Fact (Verbatim): BitOr rvalue in MIR assignments
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The `|=` operator lowers to BitOr in MIR, which is preserved.
- Counterexample: none
- Conclusion: pass

### Check 2f: Return terminators present

- Check: MIR contains Return terminators
- Status: `pass`
- Premise: Both Handled and NotHandled branches return
- Assertion Predicate(s):
  - return_count >= 1 (basic blocks with Return terminator)
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2f_return_terminators_present` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): ISR returns `IrqResult::Handled` or `IrqResult::NotHandled`
  - Extracted Fact (Verbatim): Return terminator(s) present in MIR CFG
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: Return terminators prove that the function has explicit return paths.
- Counterexample: none
- Conclusion: pass

### Check 2g: Handled local is bool

- Check: MIR local_decls contains a bool-typed local for `handled`
- Status: `pass`
- Premise: `let mut handled = false` initializes a bool local
- Assertion Predicate(s):
  - at least one local_decl has `ty == MirTy::Bool`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2g_handled_local_is_bool` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `let mut handled = false;`
  - Extracted Fact (Verbatim): MirLocalDecl with ty = Bool
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The bool type of the `handled` variable is preserved in MIR.
- Counterexample: none
- Conclusion: pass

### Check 2h: Status read call present

- Check: MIR contains Call terminator referencing read_raw (MIS status read)
- Status: `pass`
- Premise: `let status = self.regs.mis.read_raw()` is a function call
- Assertion Predicate(s):
  - body contains Call terminator where func is Constant with FnDef path containing "read_raw"
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2h_status_read_call_present` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `let status = self.regs.mis.read_raw()`
  - Extracted Fact (Verbatim): Call terminator with func type FnDef { path: "...::read_raw", ... }
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: The read_raw call is preserved as a Call terminator in MIR.
- Counterexample: none
- Conclusion: pass

### Check 2i: Helpers extracted as functions

- Check: service_rx_fifo, service_tx_fifo, and clear_explicit_interrupts are extracted as Helper-kind functions
- Status: `pass`
- Premise: Helper functions reachable from the ISR are extracted with their bodies
- Assertion Predicate(s):
  - `result.body.functions` contains "service_rx_fifo" with `kind == MirFnKind::Helper`
  - `result.body.functions` contains "service_tx_fifo" with `kind == MirFnKind::Helper`
  - `result.body.functions` contains "clear_explicit_interrupts" with `kind == MirFnKind::Helper`
- Target Evidence (Exercised by Test; Exact Citations):
  - Test `check2i_helpers_extracted_as_functions` in `crates/regseq-driver/tests/isr_extraction.rs`
- Reasoning Trace:
  - Authored Fact (Verbatim): `self.service_rx_fifo()`, `self.service_tx_fifo()`, `self.clear_explicit_interrupts(status)`
  - Extracted Fact (Verbatim): MirExtractedFn entries with kind = Helper for each helper name
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: All three helpers are transitively discovered from the ISR root and extracted with their MIR bodies.
- Counterexample: none
- Conclusion: pass

## Summary

All checks pass. The authored ISR facts for PL011 `handle_irq` are preserved
at the compiler-side extraction boundary across all three extraction surfaces:

- **InterruptSurface**: ISR binding identity, auto-bind target, no index param
- **IdentityEnrichmentSurface**: method identity, is_isr marker, canonical path
- **MirBodySurface**: ISR kind, signature, return type, helper dispatch, status
  read, branch conditions (0x50, 0x20, 0x0780), handled accumulation (BitOr),
  IrqResult variant distinguishability, return paths, helper function extraction
