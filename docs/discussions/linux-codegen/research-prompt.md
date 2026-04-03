# Linux Codegen Research Prompt

This file records the prompt sent to the external researchers for the
Linux-codegen strategy review associated with `phase-d-01`.

## Prompt

Please review the Linux codegen strategy questions for the Regseq compiler repo
and send back concrete recommendations.

Project context:

- Regseq is an embedded DSL in Rust for portable driver specification.
- Layer 1: Rust Regseq driver spec.
- Layer 2: platform/binding artifact.
- Layer 3: compiler lowering/backends.
- Current repo state: frontend extraction, declaration IR, binding IR, joined
  artifact, and analysis/bootstrap are complete through Phase C-03.
- Next step is Phase D Linux lowering planning, not implementation yet.
- Linux first, Zephyr later.

What is already settled:

- Ubuntu 24.04 is the source-of-truth Phase D environment.
- First Linux validation path should be out-of-tree module style.
- Generated Linux output should be multi-file.
- One fileset is special and backend-owned for OS integration/registration.
- Remaining generated implementation files should be partitioned by authored
  top-level source-module provenance.
- No handwritten per-driver wrapper glue.

Please read these repo-local docs first:

- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/handoff.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/.harness/spec-batches/phase-d-01.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/linux-codegen/README.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/linux-codegen/linux-driver-layout-strategy.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-kvm-simple-linux-driver-smoke-baseline.md`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/archive/guide/lowering-contract.md`

Useful code/context refs:

- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/crates/regseq-model/src/lib.rs`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/crates/regseq-analysis/src/input.rs`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/tests/e2e/tests/pl011_lowering_input_stability.rs`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/tests/fixtures/pl011-full-spec/src/driver.rs`
- `/Users/nyengele/Documents/sekekama/regseq-compiler-v2/crates/regseq-backends/src/linux.rs`

Questions to answer:

1. Lowering manifest shape: what should the Linux backend consume after
   Phase C-03? JSON artifact, Rust structs, or both? What must survive into it
   for traceability/debugging?
2. Backend-owned scaffold/content ownership: what exactly should live in the
   fixed backend-owned integration fileset, and what should never leak into
   module-partitioned implementation files?
3. Canonical output layout: what exact file/directory/naming scheme would you
   recommend for the first PL011 Linux bring-up?
4. Module partitioning rule details: how should top-level source-module
   provenance map to emitted C/H units in a deterministic way? What should
   happen with modules that have no meaningful emitted content?
5. Header policy: when should the backend emit per-module headers versus one
   shared header? What should go where?
6. Shared support layer: should any checked-in shared Linux helper layer be
   allowed in the first backend iteration? If yes, how small/generic should it
   be?
7. Build packaging / out-of-tree module contract: what exact generated build
   files and integration assumptions should the first backend own?
8. Compile/debug loop contract: what should be the canonical
   regenerate/build/error/log loop for agents on Ubuntu?
9. Smoke-validation boundary: what is the first Linux smoke proof after
   compile success, and how should it connect to the pre-PL011 simple-driver
   baseline?
10. Generated-C readability/reviewability rules: what should be explicitly
    standardized now rather than left to implementation taste?
11. Traceability in emitted output: how should generated C point back to
    lowering-manifest or input facts without becoming noisy?
12. First supported PL011 Linux slice: for the first compile/build proof, what
    should be required versus explicitly allowed to stub/defer?

Please answer in a structured way:

- recommendations per question
- any cross-question coupling to watch out for
- top 3 decisions that most affect `phase-d-01` task quality
- any places where the current docs still leave too much ambiguity

Do not broaden to Zephyr or E1000 here. Keep it Linux-first and
`phase-d-01` scoped.

## Threads

- `researcher-1`: `81e47ea8-0078-4a7e-8034-41ff61a49dc6`
- `researcher-2`: `dd303006-57a3-4a0a-970c-5fdf1f5b0705`
