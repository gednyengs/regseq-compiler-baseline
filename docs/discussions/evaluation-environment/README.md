# Evaluation Environment Discussion

This directory is the discussion area for the evaluation environment strategy
used by the ASPLOS artifact and paper.

It exists to settle questions that cut across lowering, backend validation,
and performance evaluation, including:

- which machine is the primary development and validation environment
- whether Linux lowering work should be developed and debugged on Ubuntu rather
  than macOS
- whether QEMU/KVM is the primary evaluation environment for PL011 and E1000
- what claims are relative-comparison claims versus absolute hardware claims
- what validation can be performed locally versus what requires later native or
  kernel-integrated checks

Current direction:

- Ubuntu 24.04 workstation is the primary Phase D development and validation
  environment
- QEMU/KVM is the likely primary evaluation environment for both:
  - PL011
  - E1000
- the paper should emphasize controlled relative comparison between handwritten
  and generated drivers under the same environment, rather than absolute
  physical-hardware performance claims

Follow-on notes in this directory should define:

- concrete VM/emulation setup
- workload and measurement methodology
- validation/debugging workflow for generated collateral
- limits of the evaluation claims and any deferred physical-hardware work

Current repo-local notes:

- [qemu-relative-evaluation-strategy.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md)
- [qemu-relative-evaluation-runbook.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md)
