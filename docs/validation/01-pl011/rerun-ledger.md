# PL011 Source-to-Extraction Rerun Ledger

## Purpose
This ledger records what the shared-worktree parallel validation batch taught us before resetting the tasks for clean reruns in isolated git worktrees. It preserves useful signal, notes process contamination or proof weakness, and identifies the generated artifacts to discard before rerunning.

## Verdict Labels
- `usable signal`: the run appears to have surfaced meaningful extraction-boundary information
- `process-contaminated`: the task outcome or verification was affected by shared-worktree interference
- `proof too weak`: the report/test artifact did not fully satisfy the strengthened validation contract

## Task: source-to-extraction-01
- Check File: `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- Harness Status: `failed`
- Initial Verdict: `usable signal`, `process-contaminated`, `proof too weak`
- What We Learned:
  - extraction likely preserves block identity, reconstructable canonical path, field inventory/order, and closed-world membership, but drops block-level properties, reserved extents, per-register offsets/type names/access kinds, and exact authored property set
- Why Rerun Is Needed:
  - reduced-specimen fidelity proof was representative rather than exhaustive, and the recorded failure also included unrelated workspace-wide test breakage from concurrent task files
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`
  - `crates/regseq-driver/tests/pl011_register_block_extraction_boundary.rs`
- Rerun Notes:
  - either use the real fixture directly or prove specimen fidelity exhaustively for every authored fact relevant to checks 1-8
  - do not let workspace-wide verification determine task status in a shared tree

## Task: source-to-extraction-02
- Check File: `docs/validation/01-pl011/checks/02-register-definitions-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims full preservation of register names, field inventories/order, field offsets/widths, named values, and closed-world membership
- Why Rerun Is Needed:
  - the result came from the shared-worktree batch, so it should be regenerated in an isolated tree before being treated as the final certificate
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/02-register-definitions-extraction.md`
  - `crates/regseq-driver/tests/pl011_register_definitions_boundary.rs`
- Rerun Notes:
  - keep the closed-world assertions explicit and ensure every named-value-heavy register remains fully covered

## Task: source-to-extraction-03
- Check File: `docs/validation/01-pl011/checks/03-driver-struct-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims full preservation of driver identity, device class, all resources, properties, and `pending_tx` user-field classification
- Why Rerun Is Needed:
  - the result should be regenerated in an isolated tree to remove any shared-worktree ambiguity and confirm the strengthened report format holds
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`
  - `crates/regseq-driver/tests/pl011_driver_struct_boundary.rs`
- Rerun Notes:
  - preserve exact resource/property surface coverage, especially type, requiredness, id/join-key, and non-reclassification of `pending_tx`

## Task: source-to-extraction-04
- Check File: `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - identity, canonical paths, and closed-world membership appear preserved, but the run reports failures for preserving generics/parameter-list/return-type detail and for exact body call shape/parameter flow
- Why Rerun Is Needed:
  - the failure signal should be reproduced in an isolated tree so it can be used directly as a follow-up implementation target
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`
  - `crates/regseq-driver/tests/utility_functions_extraction.rs`
- Rerun Notes:
  - keep signatures and exact wrapper-body call shapes as separate asserted facts

## Task: source-to-extraction-05
- Check File: `docs/validation/01-pl011/checks/05-pin-state-markers-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims full preservation of marker identity, closed-world membership, `PinState` trait impl evidence, and usage-site distinction for `apply::<DefaultState>()` vs `apply::<SleepState>()`
- Why Rerun Is Needed:
  - the result should be regenerated cleanly in an isolated tree before being treated as the canonical certificate
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/05-pin-state-markers-extraction.md`
  - `crates/regseq-driver/tests/pin_state_markers_extraction.rs`
- Rerun Notes:
  - keep both declaration-side and usage-site evidence explicit

## Task: source-to-extraction-06
- Check File: `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - names, Rust types, canonical paths, owner association, closed-world membership, and MIR name references appear preserved; evaluated literal values and the syntactic definitional relationship `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK` appear to be lost
- Why Rerun Is Needed:
  - the two reported failures are good candidates for extraction fixes, but should be reproduced in isolation before spawning implementation tasks
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`
  - `crates/regseq-driver/tests/pl011_driver_constants_boundary.rs`
- Rerun Notes:
  - preserve the distinction between semantic presence of both constants and preservation of their definitional link

## Task: source-to-extraction-07
- Check File: `docs/validation/01-pl011/checks/07-helper-methods-extraction-a.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims full preservation of helper identity/signature and the detailed body-level checks for the first helper-method group, including register targets, literal values, read-modify-write patterns, and ordered sequencing
- Why Rerun Is Needed:
  - the result should be regenerated in isolation to confirm the proof remains stable without shared-worktree interference
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`
  - `crates/regseq-driver/tests/pl011_helper_methods_a_boundary.rs`
- Rerun Notes:
  - keep exact body predicates granular; do not collapse them into group-level summaries

## Task: source-to-extraction-08
- Check File: `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims full preservation of the second helper-method group, including optional-resource branching, state application, clock fallback, shutdown ordering, FIFO service logic, and explicit interrupt clearing behavior
- Why Rerun Is Needed:
  - the result should be regenerated in isolation before being used as a stable certificate
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`
  - `crates/regseq-driver/tests/helper_methods_b_extraction.rs`
- Rerun Notes:
  - keep per-method assertions explicit, especially around optional-resource branching and staged FIFO service behavior

## Task: source-to-extraction-09
- Check File: `docs/validation/01-pl011/checks/09-lifecycle-callbacks-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims lifecycle callback identity, trait association, signature, return type, helper sequencing, error propagation, success return, and argument flow are all preserved
- Why Rerun Is Needed:
  - the result should be regenerated in an isolated tree before it is treated as the final certificate
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/09-lifecycle-callbacks-extraction.md`
  - `crates/regseq-driver/tests/lifecycle_callbacks_extraction.rs`
- Rerun Notes:
  - preserve fallible-step ordering and `Ok(())` success-path checks explicitly

## Task: source-to-extraction-10
- Check File: `docs/validation/01-pl011/checks/10-serial-callbacks-extraction-a.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims serial-callback identity, trait role, signatures, return types, and the first serial-callback body group are preserved through extraction
- Why Rerun Is Needed:
  - the result should be regenerated cleanly in an isolated tree to confirm the broader body-level pass claims
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`
  - `crates/regseq-driver/tests/serial_callbacks_extraction_a.rs`
- Rerun Notes:
  - keep callback identity checks separate from detailed body checks so failures stay localized

## Task: source-to-extraction-11
- Check File: `docs/validation/01-pl011/checks/11-serial-callbacks-extraction-b.md`
- Harness Status: `failed`
- Initial Verdict: `usable signal`, `process-contaminated`
- What We Learned:
  - task-local checks appeared to pass for `tx_ready`, `tx_empty`, `flush_rx_fifo`, `flush_tx_fifo`, and `send_break`
- Why Rerun Is Needed:
  - the recorded failure was driven by unrelated workspace-wide compilation errors from other concurrent work, not by this task’s own validation findings
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`
  - `crates/regseq-driver/tests/serial_callbacks_b_extraction.rs`
- Rerun Notes:
  - rerun in an isolated worktree and judge status from the task-local proof, not unrelated workspace failures

## Task: source-to-extraction-12
- Check File: `docs/validation/01-pl011/checks/12-isr-extraction.md`
- Harness Status: `done`
- Initial Verdict: `usable signal`
- What We Learned:
  - the run claims ISR binding/classification, helper-call presence, branch/mask structure, IrqResult variant distinction, `handled |= ...` bit-or pattern, bool local, return terminators, and MIS raw-read evidence are all preserved
- Why Rerun Is Needed:
  - the result should be regenerated in isolation before being treated as the stable ISR certificate
- Artifacts To Discard:
  - `docs/validation/01-pl011/checks/results/12-isr-extraction.md`
  - `crates/regseq-driver/tests/isr_extraction.rs`
- Rerun Notes:
  - keep MIR-level assertions exact around branch constants, variant distinction, and helper-call extraction
