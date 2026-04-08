# Validation Certificate: PL011 ISR Extraction Check

Source premise: `docs/validation/01-pl011/01-authored-source.md` (ISR: handle_irq section)
Target boundary evidence: `crates/regseq-driver/tests/source_to_extraction_12_isr.rs`

## Specimen Fidelity

- Fidelity Assertion Predicate(s):
  - `precondition_specimen_fidelity` verifies 16 authored facts against the
    real fixture source (`tests/fixtures/pl011-full-spec/src/driver.rs`) and
    then verifies that the inline SPECIMEN reproduces each fact verbatim.
- Fidelity Evidence (Exact Citations):
  - Fixture contains `#[rseq::isr]` -- asserted via `fixture.contains("#[rseq::isr]")`
  - Fixture contains `pub fn handle_irq(&mut self) -> IrqResult` -- asserted via `fixture.contains(...)`
  - Fixture contains `let status = self.regs.mis.read_raw();` -- asserted via `fixture.contains(...)`
  - Fixture contains `if status == 0 {` -- asserted via `fixture.contains(...)`
  - Fixture contains `return IrqResult::NotHandled;` -- asserted via `fixture.contains(...)`
  - Fixture contains `let mut handled = false;` -- asserted via `fixture.contains(...)`
  - Fixture contains `if status & Self::RX_INTERRUPT_MASK != 0 {` -- asserted via `fixture.contains(...)`
  - Fixture contains `if status & Self::TX_INTERRUPT_MASK != 0 {` -- asserted via `fixture.contains(...)`
  - Fixture contains `handled |= self.service_rx_fifo();` -- asserted via `fixture.contains(...)`
  - Fixture contains `handled |= self.service_tx_fifo();` -- asserted via `fixture.contains(...)`
  - Fixture contains `self.clear_explicit_interrupts(status);` -- asserted via `fixture.contains(...)`
  - Fixture contains `if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {` -- asserted via `fixture.contains(...)`
  - Fixture contains `const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);` -- asserted
  - Fixture contains `const TX_INTERRUPT_MASK: u32 = 1 << 5;` -- asserted
  - Fixture contains `const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;` -- asserted
  - Fixture contains `const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);` -- asserted
  - All 16 facts also verified present in inline SPECIMEN via matching `SPECIMEN.contains(...)` assertions

## Check-to-Surface Mapping

| Check | Surface | Function / Field |
|-------|---------|-----------------|
| 1a | InterruptSurface.isr_bindings | `extract_interrupt_surface(tcx)` |
| 1b | IdentityEnrichmentSurface.impl_blocks | `extract_identity_enrichment(tcx)` |
| 1c | MirBodySurface.functions | `extract_body_surface(tcx)` |
| 2a | MirBodySurface.functions[handle_irq].helper_calls | `extract_body_surface(tcx)` |
| 2b | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2c | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2d | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2e | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2f | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2g | MirBodySurface.functions[handle_irq].body.local_decls | `extract_body_surface(tcx)` |
| 2h | MirBodySurface.functions[handle_irq].body | `extract_body_surface(tcx)` |
| 2i | MirBodySurface.functions | `extract_body_surface(tcx)` |
| 2j | MirBodySurface.functions[handle_irq].body (CFG walk) | `extract_body_surface(tcx)` |

## Check Group 1: ISR identity, annotation, signature, return type

### Check 1a: ISR binding identity
- Check: ISR binding surface preserves handle_irq with Auto bind target
- Status: `pass`
- Premise: `#[rseq::isr]` annotation and handler identity survive extraction
- Assertion Predicate(s):
  - `result.interrupt.isr_bindings.len() == 1`
  - `binding.handler_name == "handle_irq"`
  - `binding.bind_target == IsrBindTarget::Auto`
  - `!binding.has_index_param`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check1a_isr_binding_identity` passes: exactly one ISR binding named `handle_irq` with `Auto` target and no index param
- Reasoning Trace:
  - Authored Fact (Verbatim): `ISR Annotation: #[rseq::isr]`, `Source Local Path: driver::Pl011FullSpec::handle_irq`
  - Extracted Fact (Verbatim): `handler_name = "handle_irq"`, `bind_target = Auto`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: extraction preserves ISR identity and annotation exactly
- Counterexample: N/A
- Conclusion: pass

### Check 1b: Identity enrichment ISR marker
- Check: handle_irq has `is_isr = true` in identity enrichment
- Status: `pass`
- Premise: ISR marker survives in identity enrichment surface
- Assertion Predicate(s):
  - `handle_irq_method.is_some()` (method exists in inherent impl)
  - `method.is_isr == true`
  - `method.canonical_path.0.contains("handle_irq")`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check1b_identity_enrichment_isr_marker` passes: handle_irq exists as a method in Pl011FullSpec inherent impl with `is_isr = true`
- Reasoning Trace:
  - Authored Fact (Verbatim): ISR method `handle_irq` in inherent impl with `#[rseq::isr]`
  - Extracted Fact (Verbatim): `ImplMethodFact { name: "handle_irq", is_isr: true, ... }`
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: identity enrichment preserves ISR marker
- Counterexample: N/A
- Conclusion: pass

### Check 1c: MIR ISR identity and signature
- Check: MIR body surface preserves handle_irq as Isr with correct receiver and return type
- Status: `pass`
- Premise: ISR classification, receiver, return type, and parameter count survive
- Assertion Predicate(s):
  - `handle_irq.kind == MirFnKind::Isr`
  - `handle_irq.receiver == "ref_mut"`
  - `handle_irq.return_type == Some("IrqResult")`
  - `handle_irq.params.is_empty()`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check1c_mir_isr_identity_and_signature` passes: handle_irq classified as Isr, receiver is ref_mut, return type is IrqResult, no extra params
- Reasoning Trace:
  - Authored Fact (Verbatim): `pub fn handle_irq(&mut self) -> IrqResult`
  - Extracted Fact (Verbatim): kind=Isr, receiver="ref_mut", return_type=Some("IrqResult"), params=[]
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: all signature facts preserved
- Counterexample: N/A
- Conclusion: pass

## Check Group 2: ISR body preservation

### Check 2a: Helper dispatch
- Check: handle_irq calls service_rx_fifo, service_tx_fifo, clear_explicit_interrupts
- Status: `pass`
- Premise: helper dispatch calls survive in MIR
- Assertion Predicate(s):
  - `handle_irq.helper_calls.contains("service_rx_fifo")`
  - `handle_irq.helper_calls.contains("service_tx_fifo")`
  - `handle_irq.helper_calls.contains("clear_explicit_interrupts")`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2a_helper_dispatch` passes: all three helper names present in helper_calls
- Reasoning Trace:
  - Authored Fact (Verbatim): calls `self.service_rx_fifo()`, `self.service_tx_fifo()`, `self.clear_explicit_interrupts(status)`
  - Extracted Fact (Verbatim): helper_calls contains all three names
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: MIR-based helper discovery finds all three calls
- Counterexample: N/A
- Conclusion: pass

### Check 2b: Status read and branch conditions
- Check: SwitchInt with value 0 for early check; BitAnd with RX mask 0x50 and TX mask 0x20
- Status: `pass`
- Premise: early `status == 0` branch and mask-based branching survive in MIR
- Assertion Predicate(s):
  - SwitchInt terminator exists with `targets.values.contains(&0u128)`
  - BitAnd rvalue exists with operand scalar 0x50 (or unevaluated RX_INTERRUPT_MASK)
  - BitAnd rvalue exists with operand scalar 0x20 (or unevaluated TX_INTERRUPT_MASK)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2b_status_read_and_branch_conditions` passes: found SwitchInt with 0, BitAnd with 0x50, BitAnd with 0x20
- Reasoning Trace:
  - Authored Fact (Verbatim): `if status == 0`, `status & Self::RX_INTERRUPT_MASK != 0`, `status & Self::TX_INTERRUPT_MASK != 0`
  - Extracted Fact (Verbatim): SwitchInt(0), BitAnd(0x50), BitAnd(0x20) present in MIR
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: branch conditions and mask constants preserved
- Counterexample: N/A
- Conclusion: pass

### Check 2c: EXPLICIT_ISR_CLEAR_MASK in final condition
- Check: BitAnd with 0x0780 (or named EXPLICIT_ISR_CLEAR_MASK) present
- Status: `pass`
- Premise: final handled condition mask survives
- Assertion Predicate(s):
  - BitAnd rvalue exists with scalar 0x0780 on lhs or rhs, or unevaluated const named EXPLICIT_ISR_CLEAR_MASK
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2c_explicit_clear_mask_in_final_condition` passes
- Reasoning Trace:
  - Authored Fact (Verbatim): `status & Self::EXPLICIT_ISR_CLEAR_MASK != 0` where EXPLICIT_ISR_CLEAR_MASK = 0x0780
  - Extracted Fact (Verbatim): BitAnd with 0x0780 or named ref found in MIR
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: final condition mask preserved
- Counterexample: N/A
- Conclusion: pass

### Check 2d: IrqResult variants distinguishable and mapped
- Check: IrqResult::Handled and IrqResult::NotHandled produce distinct variant indices, with the early `status == 0` branch constructing NotHandled (variant 1) and Handled being variant 0
- Status: `pass`
- Premise: IrqResult enum variants remain nominally distinguishable and correctly mapped to branches
- Assertion Predicate(s):
  - Exactly 2 distinct `variant_index` values in Adt aggregates with path containing "IrqResult"
  - The early `status == 0` true-branch block constructs IrqResult with variant_index 1 (NotHandled)
  - The other variant index is 0 (Handled)
  - `handled_variant == 0` and `not_handled_variant == 1`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2d_irq_result_variants_distinguishable` passes: found exactly 2 variant indices, early return block constructs variant 1 (NotHandled), variant 0 is Handled
- Reasoning Trace:
  - Authored Fact (Verbatim): `IrqResult::Handled` and `IrqResult::NotHandled` are distinct enum variants; early return produces NotHandled
  - Extracted Fact (Verbatim): variant_index 0 = Handled, variant_index 1 = NotHandled, early branch constructs variant 1
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: variant identity and branch mapping verified by tracing the early `status == 0` SwitchInt true-arm to its IrqResult aggregate construction
- Counterexample: N/A
- Conclusion: pass

### Check 2e: Handled accumulation via BitOr
- Check: `handled |= ...` produces BitOr in MIR, targeting the specific `handled` bool accumulator local
- Status: `pass`
- Premise: handled flag accumulation via `|=` survives, writing to the correct bool local
- Assertion Predicate(s):
  - BitOr assignments exist where `place.local == lhs.local` (self-accumulation pattern `handled = handled | x`)
  - The target local is declared as `MirTy::Bool`
  - All BitOr accumulations target the same local (consistent accumulator)
  - At least 2 BitOr accumulations exist (one for RX, one for TX)
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2e_handled_accumulation_via_bitor` passes: found >= 2 BitOr ops targeting the same bool local
- Reasoning Trace:
  - Authored Fact (Verbatim): `handled |= self.service_rx_fifo()` and `handled |= self.service_tx_fifo()`
  - Extracted Fact (Verbatim): 2+ BitOr(local_N, local_N, rhs) where local_N is a Bool local
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: `|=` lowered to BitOr targeting a specific bool accumulator local, verified for both RX and TX
- Counterexample: N/A
- Conclusion: pass

### Check 2f: Return terminators and distinct return paths
- Check: MIR has Return terminators and at least 3 IrqResult aggregate sites (early NotHandled, final Handled, final NotHandled) all reaching Return
- Status: `pass`
- Premise: multiple distinct return paths survive
- Assertion Predicate(s):
  - At least 1 `MirTerminatorKind::Return` exists
  - At least 3 IrqResult aggregate construction sites exist (early NotHandled, final Handled, final NotHandled)
  - Each IrqResult aggregate site reaches a Return terminator via CFG walk
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2f_return_terminators_present` passes: found >= 3 IrqResult aggregate bbs, each reaching Return
- Reasoning Trace:
  - Authored Fact (Verbatim): `return IrqResult::NotHandled;` (early), `IrqResult::Handled` (final true), `IrqResult::NotHandled` (final false)
  - Extracted Fact (Verbatim): 3 IrqResult aggregate sites, all with paths to Return terminator
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: each authored return path maps to a distinct IrqResult aggregate bb that reaches Return
- Counterexample: N/A
- Conclusion: pass

### Check 2g: Handled local is bool with false initialization
- Check: The specific `handled` accumulator local is a mutable bool initialized to false
- Status: `pass`
- Premise: `let mut handled = false;` survives as a specific mutable bool local initialized to false
- Assertion Predicate(s):
  - The local targeted by BitOr accumulation exists in local_decls
  - Its type is `MirTy::Bool`
  - Its mutability is `MirMutability::Mut`
  - It is assigned `Scalar { bits: 0, size: 1 }` (false) via a `Use` rvalue
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2g_handled_local_is_bool` passes: BitOr target local is Bool, Mut, initialized to false
- Reasoning Trace:
  - Authored Fact (Verbatim): `let mut handled = false;`
  - Extracted Fact (Verbatim): local_N: ty=Bool, mutability=Mut, assigned Scalar{bits:0,size:1}
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the specific accumulator local (not just any bool) is verified as mutable bool initialized to false
- Counterexample: N/A
- Conclusion: pass

### Check 2h: Status read call present
- Check: read_raw call present in MIR for MIS status read
- Status: `pass`
- Premise: `self.regs.mis.read_raw()` survives as a Call terminator
- Assertion Predicate(s):
  - Call terminator with func FnDef path containing "read_raw" exists
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2h_status_read_call_present` passes
- Reasoning Trace:
  - Authored Fact (Verbatim): `let status = self.regs.mis.read_raw();`
  - Extracted Fact (Verbatim): Call terminator referencing `read_raw` present
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: function call preserved in MIR
- Counterexample: N/A
- Conclusion: pass

### Check 2i: Helpers extracted as functions
- Check: service_rx_fifo, service_tx_fifo, clear_explicit_interrupts present as Helper kind
- Status: `pass`
- Premise: helper functions survive as extracted MIR functions
- Assertion Predicate(s):
  - Each helper name found in `body.functions` with `kind == MirFnKind::Helper`
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2i_helpers_extracted_as_functions` passes for all three helpers
- Reasoning Trace:
  - Authored Fact (Verbatim): `fn service_rx_fifo`, `fn service_tx_fifo`, `fn clear_explicit_interrupts`
  - Extracted Fact (Verbatim): all three present with kind=Helper
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: transitive helper extraction discovers all three
- Counterexample: N/A
- Conclusion: pass

### Check 2j: Control-flow ordering
- Check: CFG walk proves exact ordering of ISR body: early return before helpers, RX/TX before clear, clear before final condition, final branches produce correct variants
- Status: `pass`
- Premise: branch ordering and final variant mapping are proven exactly via CFG graph walk, not inferred from helper presence or variant existence alone
- Assertion Predicate(s):
  - `find_call_bb` locates read_raw, service_rx_fifo, service_tx_fifo, clear_explicit_interrupts as distinct basic blocks
  - Early `status == 0` SwitchInt is found (block with BinaryOp::Eq and SwitchInt on value 0)
  - Early true arm constructs IrqResult variant 1 (NotHandled) and reaches Return
  - Early true arm does NOT reach service_rx_fifo, service_tx_fifo, or clear_explicit_interrupts (no back-edges)
  - service_rx_fifo and service_tx_fifo are reachable from the status!=0 path (early false arm)
  - clear_explicit_interrupts is reachable from both RX and TX call continuations
  - EXPLICIT_ISR_CLEAR_MASK BitAnd is reachable from clear_explicit_interrupts continuation
  - clear_explicit_interrupts is NOT reachable from the final mask check (no back-edge -- ordering proven)
  - IrqResult variant 0 (Handled) aggregate is reachable from clear continuation and reaches Return
  - IrqResult variant 1 (NotHandled) aggregate is reachable from clear continuation and reaches Return
  - Final Handled and NotHandled are in distinct basic blocks
- Target Evidence (Exercised by Test; Exact Citations):
  - `check2j_control_flow_ordering` passes: all 11 CFG ordering assertions hold
- Reasoning Trace:
  - Authored Fact (Verbatim): early `status == 0` returns NotHandled first; RX/TX dispatch before `clear_explicit_interrupts(status)`; clear before final handled condition; true -> Handled, false -> NotHandled
  - Extracted Fact (Verbatim): CFG walk confirms: early SwitchInt true-arm -> variant 1 -> Return (no helper calls reachable); RX/TX call bbs reachable from status!=0; clear reachable from RX/TX continuations; mask check reachable from clear continuation; variant 0 (Handled) and variant 1 (NotHandled) both reachable from clear continuation and both reach Return
  - Correspondence: `exact`
  - Gap / Loss Description: none
  - Why This Status Follows: the CFG structure of the extracted MIR basic-block graph directly encodes the authored control-flow ordering, and every ordering constraint is verified by reachability and non-reachability assertions over the graph
- Counterexample: N/A
- Conclusion: pass

## Summary

All 13 checks (1a, 1b, 1c, 2a-2j) pass. The authored ISR facts for PL011
`handle_irq` survive at the compiler-side extraction boundary across
`InterruptSurface`, `IdentityEnrichmentSurface`, and `MirBodySurface`.

Control-flow ordering -- previously unverified -- is now proven exactly via
CFG graph walks over the MIR basic-block graph. The extraction surface
exposes full CFG structure (basic blocks with terminators containing
successor block indices), which is sufficient to verify all ordering
constraints specified in the check contract.
