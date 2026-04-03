# Handoff

## What Regseq Is

Regseq is a compiler/tooling project for expressing device drivers as a
semantic, target-independent specification and then lowering that specification
into OS-specific driver artifacts.

The core problem it is trying to solve is:

- handwritten drivers duplicate the same hardware intent across OSes
- that intent is mixed together with OS framework glue, resource-management
  boilerplate, callback registration, synchronization details, and build-system
  conventions
- this makes cross-OS portability, comparison, and compiler-driven generation
  difficult

Regseq's approach is:

- write a **Layer 1** driver spec in Rust using an embedded Regseq DSL made of
  facade constructs, traits, macros, and attributes that describe the
  hardware-facing and device-class-facing semantics
- provide a **Layer 2** platform/binding file that describes deployment-side
  facts such as platform properties and resource bindings
- extract a raw frontend artifact from Layer 1 through `regseq-driver`, which
  acts as the custom Regseq-facing `rustc` wrapper/driver path
- normalize that into compiler-owned internal models
- join declaration-side and binding-side facts
- perform analysis/bootstrap checks over the joined artifact
- lower the result into OS-specific driver output

The key architectural goal is:

- preserve semantic intent in a stable, reviewable intermediate form
- separate authored device semantics from compiler-managed OS realization

## What This Repository Is

This repository is the implementation workspace for the Regseq compiler
pipeline used for the ASPLOS work.

At a high level, it contains:

- the public Regseq facade (`crates/regseq/`)
- compiler wrapper/extraction logic (`crates/regseq-driver/`)
  - this is the custom `rustc` wrapper/driver path used to recognize Regseq
    constructs and emit raw frontend artifacts
- normalized internal models (`crates/regseq-model/`)
- analysis/bootstrap passes (`crates/regseq-analysis/`)
- backend/lowering crates (currently mostly future Phase D work)
- PL011 and later E1000 artifact tracks
- harness task specs and trackers that stage implementation in phases

The development process in this repo is phase-based:

- Phase B: frontend closure
- Phase C: normalized models, join, and analysis/bootstrap
- Phase D: PL011 lowering and validation
- later phases: E1000 closure, lowering, and evaluation

The repo also uses a harness/tasking system:

- executable tasks live in `.harness/tasks.json` and `.harness/tasks/`
- task-spec authoring progress lives in `.harness/task-spec-status.yaml`
- when there are no executable tasks, the next action may be a brief-authoring
  or task-spec-authoring step

The practical compiler pipeline already built in this repo is:

1. Layer 1 spec crate authored using Regseq
2. raw frontend extraction through `regseq-driver`
3. declaration IR in `regseq-model`
4. binding IR in `regseq-model`
5. joined artifact in `regseq-model`
6. analysis/bootstrap/readiness checks in `regseq-analysis`
7. next: Phase D lowering/backend work

## What Is Relevant vs Stale

The repo now has several documentation layers. A cold session should not treat
all of them as equally current.

### Read these first

1. This file: `docs/handoff.md`
2. The harness state:
   - `.harness/config.md`
   - `.harness/SESSION.md`
   - `.harness/task-spec-workflow.md`
   - `python3 scripts/python/task.py next-step`
3. Current execution plan:
   - `docs/plans/asplos-implementation-plan.md`
4. Broad construct baseline:
   - `docs/compiler-collateral/compiler-baseline-overview.md`
   - `docs/compiler-collateral/compiler-constraints.md`
   - `docs/compiler-collateral/constructs/`
5. Current refinement/decision docs:
   - `docs/discussions/device-class-serial/serial-device-class-spec.md`
   - `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
   - `docs/discussions/pl011-full-spec/pl011-full-spec-methodology.md`
   - `docs/discussions/regseq-model/phase-c-01-declaration-model-plan.md`
   - `docs/discussions/regseq-model/phase-c-02-binding-and-join-plan.md`
   - `docs/discussions/regseq-analysis/phase-c-03-analysis-bootstrap-plan.md`
   - `docs/discussions/evaluation-environment/*.md`

### Useful but secondary

- `archive/guide/`
  - still useful as the frozen research/planning baseline
  - especially:
    - `archive/guide/implementation-guide.md`
    - `archive/guide/lowering-contract.md`
- `docs/compiler-collateral/constructs/serial-uart-management.md`
  - broad construct baseline only
  - where it differs, implement from the device-class serial refinement
- `docs/discussions/pl011-full-spec/`
  - active PL011 target boundary and feature status
  - useful when checking what Phase B already closed

### Supporting context only

- `docs/discussions/device-class-serial/serial-abstraction-research-plan.md`
- `docs/discussions/device-class-serial/serial-taxonomy-gap-study.md`
- `docs/discussions/pl011-spec-slice/`
- older discussion areas such as:
  - `docs/discussions/layer-2-format/`
  - `docs/discussions/regseq-error/`
  - `docs/discussions/regseq-intrinsics/`
  - `docs/discussions/regseq-traits/`

These are not useless, but they are not the first authority for current Phase D
work.

### Known stale or misleading material

- `README.md`
  - useful for orientation, but partially stale
  - still points cold sessions at `docs/guide/`, which does not exist in the
    live repo layout; the current equivalents are mostly under `archive/guide/`
    and `docs/discussions/`
  - its workspace and phase descriptions lag the current implemented state
- some crate README files are aspirational rather than state-accurate:
  - `crates/regseq-driver/README.md`
  - `crates/regseq-analysis/README.md`
  - `crates/regseq-backends/README.md`
  - `crates/regseq-config/README.md`
  - these often describe planned MIR-heavy or backend-heavy responsibilities
    that are not yet implemented in the codebase
- `crates/regseq-backends/`
  - exists, but Linux and Zephyr backend modules are still placeholders
  - do not assume lowering/emission infrastructure already exists there

## Current Code Reality

The codebase is not at a uniform maturity level.

### Implemented and relevant now

- `crates/regseq/`
  - current Layer 1 facade surface
- `crates/regseq-driver/`
  - raw frontend extraction and `RawFrontendArtifact`
- `crates/regseq-model/`
  - declaration IR, binding IR, joined artifact
- `crates/regseq-analysis/`
  - analysis bootstrap and readiness checks
- `tests/fixtures/pl011-full-spec/`
  - the fuller generic ARM PL011 frontend artifact used to pressure the
    pipeline
- `tests/e2e/tests/pl011_lowering_input_stability.rs`
  - the key proof that the current lowering input is stable enough to start
    Phase D

### Real but not the current focus

- `crates/cargo-regseq/`
  - cargo wrapper/orchestration
- `crates/regseq-config/`
  - shared config/CLI types

### Present but mostly placeholder

- `crates/regseq-backends/`
  - Phase D work has not been implemented yet
- `crates/regseq-diagnostics/`
  - scaffold exists, but it is not a central live dependency for the next step

## Crucial Cold-Session Facts

If a cold agent remembers only a few things, these are the most important:

- Regseq is an embedded DSL in Rust, not a separate standalone language.
- `regseq-driver` is the custom `rustc` wrapper path that extracts Layer 1
  facts.
- Phase C is complete through the lowering-input bootstrap.
- Phase D has **not** started implementation yet.
- The very next repo action is still:
  - author `.harness/spec-batches/phase-d-01.md`
- Linux lowering is the next active backend step.
- Linux and Zephyr are both in scope for the paper, but `phase-d-01` remains
  Linux-first and should not be coupled to Zephyr implementation work.
- Ubuntu 24.04 is the source-of-truth Phase D host.
- `docs/discussions/evaluation-environment/` contains important Phase D
  environment decisions, but it is currently uncommitted in this worktree.

## Purpose

This file is the cold-start handoff for continuing the Regseq compiler work on
the Ubuntu 24.04 workstation.

The main reason for the handoff is that Phase D lowering work should proceed on
Linux, not on the current macOS machine.

## Current Repository State

As of this handoff:

- all work through `phase-c-03` is implemented and verified
- the repo is **not** clean yet; the current uncommitted paths are:
  - `docs/handoff.md`
  - `docs/discussions/evaluation-environment/`
- the harness has advanced to the next step:
  - `BRIEF_AUTHORING_BATCH`
  - tracker id: `phase-d-01`
  - expected brief: `.harness/spec-batches/phase-d-01.md`

Relevant command from the current repo state:

```bash
python3 scripts/python/task.py next-step
```

Expected result:

- `NEXT_ACTION: BRIEF_AUTHORING_BATCH`
- `TRACKER_ID: phase-d-01`

## Last Verified Milestones

Completed and committed:

- `phase-c-01`: declaration IR
- `phase-c-02`: binding IR + joined artifact
- `phase-c-03`: analysis bootstrap + lowering-input readiness + PL011 lowering-input proof

Recent commits:

- `9c1e0fd` — Author and complete phase c-03 analysis bootstrap work
- `d9f9255` — Author and complete phase c-02 binding and join work
- `a045580` — Author and complete phase c-01 declaration model work

## What Exists In The Compiler Now

### Frontend and model pipeline

The current pipeline shape is:

1. Layer 1 source
2. `regseq-driver` raw frontend artifact
3. `regseq-model` declaration IR
4. `regseq-model` binding IR
5. `regseq-model` joined artifact
6. `regseq-analysis` bootstrap/readiness layer

### Key crates

- `crates/regseq/`
- `crates/regseq-driver/`
- `crates/regseq-model/`
- `crates/regseq-analysis/`

### Analysis/bootstrap

Phase C-03 added:

- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-analysis/src/readiness.rs`

Those provide:

- a read-only `AnalysisInput` pairing declaration + joined artifact
- deterministic readiness checks
- explicit access to declaration-only operation-evidence families:
  - registers
  - synchronization
  - timed delay
  - logging
  - PM gate

### PL011 validation

The key final Phase C proof is:

- `tests/e2e/tests/pl011_lowering_input_stability.rs`

That test proves the current PL011 lowering-input contract is stable enough to
act as the Phase D starting point.

## Current Device/Spec Status

### PL011

The fuller generic ARM PL011 frontend work is substantially complete.

Captured in the full-spec work:

- base declarations
- polling serial bring-up
- RX error handling
- interrupt-driven path
- clock participation
- reset participation
- pinctrl participation

Deferred by scope:

- PM-aware path
- variant/revision/quirk machinery
- modem-control/status
- RS-485
- loopback
- IrDA / SIR

Key discussion/status files:

- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-methodology.md`

### Serial device class

The serial device-class refinement already happened and was folded back into
the compiler during `phase-b-10`.

Current authority:

- `docs/discussions/device-class-serial/serial-device-class-spec.md`

## Phase D Direction

### Immediate next task

The next step is **not** executable lowering implementation yet. It is:

- create the missing brief:
  - `.harness/spec-batches/phase-d-01.md`

That brief should cover:

- PL011 Linux lowering and backend task batch
- Linux-first only
- no Zephyr coupling in the same batch

### Source references for the brief

Use:

- `docs/plans/asplos-implementation-plan.md`
- `archive/guide/lowering-contract.md`
- `experiments/mir-structured-codegen/report.md`

### Proposed `phase-d-01` decomposition

The working decomposition discussed in-session was:

1. lowering crate/bootstrap and lowering-input boundary
2. PL011-to-Linux lowering manifest / lowering plan
3. Linux backend emission from the lowering manifest
4. PL011 Linux smoke/build validation proof

This decomposition was not yet written into the missing Phase D brief.

## Evaluation / Environment Decisions

### Primary host

Use **Ubuntu 24.04** as the source-of-truth environment for all Phase D work:

- lowering development
- generated Linux driver compile validation
- QEMU/KVM smoke validation
- paper measurement runs

Reason:

- KVM is Linux-only
- Linux kernel/module integration is much easier there
- it avoids split-host drift between generation, compile, and evaluation

### Current repo-local evaluation docs

These are currently **uncommitted** and should either be committed or carried
over carefully to the Ubuntu machine:

- `docs/discussions/evaluation-environment/README.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`
- `docs/discussions/evaluation-environment/zephyr-evaluation-and-development-strategy.md`
- `docs/discussions/evaluation-environment/zephyr-emulation-target-selection.md`

They capture these decisions:

- QEMU-based evaluation should be the primary paper methodology
- claims should be **controlled relative comparisons**, not absolute physical
  hardware claims
- Ubuntu 24.04 is the primary host
- Linux backend work should support an agent-friendly generate/compile/fix loop
- out-of-tree module workflow is the likely first Linux integration mode

### Linux evaluation stance

For Linux:

- use Ubuntu 24.04
- use QEMU/KVM
- compare handwritten vs generated under the same environment

### Zephyr stance

Zephyr is **not** future work in principle. The intended paper claim includes
Linux and Zephyr, but Linux comes first.

What we established:

- Zephyr should have its own development/validation path
- Zephyr is not the path for collecting the E1000 Linux paper metrics
- Zephyr correctness/smoke validation still matters for the cross-OS claim

### Zephyr emulation target finding

Important concrete result:

- `qemu_cortex_a53` is the first Zephyr/QEMU target to prioritize because
  Zephyr documents it as exposing both:
  - `ARM PL011 UART`
  - `Intel E1000 Ethernet controller`

This finding is captured in:

- `docs/discussions/evaluation-environment/zephyr-emulation-target-selection.md`

## What Still Needs To Be Committed

Current uncommitted paths:

```text
docs/handoff.md
docs/discussions/evaluation-environment/
```

Before switching machines, either:

1. commit these files here, or
2. make sure the Ubuntu-side work starts from a checkout that includes them via
   manual transfer

If you want the cleanest handoff, commit them before leaving this machine.

## Recommended First Steps On Ubuntu

1. clone or sync this repo to Ubuntu
2. ensure the repo includes the evaluation-environment docs above
3. run:

```bash
python3 scripts/python/task.py next-step
```

4. confirm the next step is still `phase-d-01` brief authoring
5. read:
   - `.harness/task-spec-workflow.md`
   - `.harness/spec-batches/BRIEF_TEMPLATE.md`
   - `docs/plans/asplos-implementation-plan.md` (Phase D section)
   - `archive/guide/lowering-contract.md`
   - `experiments/mir-structured-codegen/report.md`
   - `docs/discussions/evaluation-environment/*.md`
6. author:
   - `.harness/spec-batches/phase-d-01.md`

## Practical Guidance For The Next Agent

When resuming:

- do not reopen Phase C
- do not start Linux backend implementation before the `phase-d-01` brief
  exists
- treat Ubuntu as the canonical Phase D environment
- keep Linux and Zephyr planning separate:
  - `phase-d-01` = Linux first
  - `phase-d-02` = Zephyr follow-up
- preserve the evaluation claim boundary:
  - relative comparison under controlled virtualized conditions
  - no bare-metal absolute performance claims

## One-Line Summary

The compiler is ready to enter Phase D; the next job on Ubuntu is to author
the missing Linux-first lowering brief for `phase-d-01`, using the existing
Phase C lowering-input contract and the uncommitted evaluation-environment
notes as the environment strategy baseline.
