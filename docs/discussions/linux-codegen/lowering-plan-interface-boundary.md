# Lowering Plan Interface Boundary

## Purpose

This note captures the intended interface between the compiler's internal
modeling/lowering layers and OS-specific backend code generators.

It exists because the phrase "lowering plan" had started to drift toward a
Linux-shaped meaning during `phase-d-01` discussion. That would be a mistake:
Linux is the first backend bring-up target, but it is not the only intended
backend.

The goal is to preserve a clean architecture for:

- Linux now
- Zephyr next
- additional OS or framework backends later

## Core Position

A generic lowering plan should **not** be tied to a particular OS.

The right boundary is a layered one:

1. frontend/model layer
2. portable compiler lowering layer
3. OS-specific backend-planning layer
4. OS-specific emission layer

In repo terms, the stable input to lowering remains
[`AnalysisInput`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-analysis/src/input.rs),
which is explicitly analysis-facing and backend-neutral. The joined model in
[`JoinedArtifact`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/joined/artifact.rs)
is likewise reviewer-facing and does not contain backend/lowering structure.

That means the first backend-specific structure should not be "the lowering
plan." It should be an OS-specific plan derived from a portable lowering plan.

## Recommended Layering

The recommended pipeline is:

1. `DeclarationModel` + `JoinedArtifact`
2. `AnalysisInput`
3. `DriverLoweringPlan` (portable)
4. `LinuxBackendPlan` or `ZephyrBackendPlan`
5. generated source/build artifacts

In shorthand:

```text
AnalysisInput
  -> DriverLoweringPlan
  -> LinuxBackendPlan | ZephyrBackendPlan | ...
  -> emitted artifacts
```

## What Belongs in the Portable Lowering Plan

The portable lowering plan should capture compiler-owned driver semantics and
compiler obligations that are meaningful across backends.

Examples:

- driver identity and supported roles
- callback/control-flow inventory
- resource requirements:
  - MMIO
  - interrupts
  - properties
  - optional clocks
  - optional resets
  - optional pinctrl
- register-access obligations
- ordering obligations
- synchronization obligations
- authored semantic error/return intent
- provenance/traceability information needed for deterministic partitioning

This layer is where the compiler should stop re-deriving facts from the raw
models.

## Required Richness For `DriverLoweringPlan`

For `phase-d-01`, `DriverLoweringPlan` should be treated as a materially rich
portable plan, not as a thin summary wrapper around `AnalysisInput`.

At minimum, it should carry:

- resource acquisition ordering and teardown ordering
- optional-resource policy:
  - required
  - optional-with-fallback
  - optional-skip
- structured callback-body lowering actions, not just callback presence
- ISR classification and relevant handling obligations
- device-class role obligations, including which serial operations are realized
- provenance mapping from authored source modules to emitted constructs

This does not require committing to a final compiler IR design yet, but it does
require the portable plan to be rich enough that OS-specific backend planners
do not have to re-derive core driver semantics from the raw models.

The intent is:

- `AnalysisInput` is the stable lowering input boundary
- `DriverLoweringPlan` is the compiler-owned semantic realization plan
- `LinuxBackendPlan` converts that semantic plan into Linux-specific choices

That means callback bodies such as lifecycle init/deinit, interrupt handlers,
and serial operations should already be represented in the portable plan in a
backend-usable form.

## What Must Stay Out of the Portable Lowering Plan

The portable lowering plan should not directly encode OS/framework realization
choices.

Examples that do **not** belong there:

- Linux `platform_driver`
- Linux `uart_driver`
- Linux `uart_ops`
- Linux include selection
- Zephyr `DEVICE_DT_*` registration shape
- Zephyr subsystem glue
- OS-specific callback signatures
- exact emitted file layout for a specific OS
- per-OS build metadata details

Those belong in the OS-specific backend plan.

## What Belongs in an OS-Specific Backend Plan

An OS-specific backend plan is the realization layer between the portable
lowering plan and concrete file emission.

For Linux, that includes decisions such as:

- `platform_driver` realization
- `uart_driver` and `uart_ops` realization
- Linux probe/remove orchestration
- Linux resource-acquisition sequences
- native error propagation choices
- generated file partitioning and module/build layout

For Zephyr, that would instead include:

- device registration realization
- Zephyr init/registration hooks
- subsystem-specific integration choices
- static-build integration and emitted file layout

This keeps OS/framework policy out of the portable lowering layer while still
giving each backend enough structure to emit readable, deterministic code.

## Recommended Interface Shape

Conceptually, the interface should be:

```rust
trait BackendPlanner {
    type Plan;

    fn plan(driver: &DriverLoweringPlan) -> Result<Self::Plan, Diagnostics>;
}

trait BackendEmitter {
    type Plan;

    fn emit(plan: &Self::Plan) -> Result<GeneratedArtifacts, Diagnostics>;
}
```

The exact Rust API can change, but the architectural rule should hold:

- compiler core produces a portable lowering plan
- each backend planner maps that plan into backend-specific realization
- emitters consume backend-specific plans, not `AnalysisInput` directly

## Naming Rule

The name `LoweringPlan` should be reserved for a portable or compiler-owned
plan.

If a type contains Linux-specific constructs, it should be named honestly:

- `LinuxBackendPlan`
- `LinuxDriverPlan`
- `LinuxRealizationPlan`

not just `LoweringPlan`.

This matters because otherwise Linux-specific assumptions will leak into later
Zephyr and multi-OS work.

## Implication for Phase D

`phase-d-01` is still Linux-first, but Linux-first does not justify a
Linux-shaped global lowering type.

The right Linux-first interpretation is:

1. define a portable `DriverLoweringPlan`
2. define `LinuxBackendPlan`
3. emit Linux artifacts from `LinuxBackendPlan`

Zephyr later should be able to reuse the same portable plan while providing a
different backend plan and emitter.

## Manifest / JSON Position

The first implementation pass should **not** require a JSON lowering-manifest
artifact.

The immediate requirement is an internal typed plan boundary. External JSON can
be added later if it proves useful for:

- review/debugging
- deterministic snapshots
- traceability artifacts

But JSON should not be forced before the first backend compile/build loop
exists.

## Current Consequences for Repo Planning

This implies:

- the current Linux layout discussion should be interpreted as a Linux backend
  planning and emission discussion, not as the definition of the global
  lowering interface
- `AnalysisInput` remains the stable backend-neutral entry point into lowering
- the first backend-specific planning work should happen inside
  `regseq-backends`, but with a split between portable lowering and per-OS
  realization
- provenance improvements needed for deterministic source-file partitioning are
  still valid Phase D work, but they feed the portable plan rather than a
  Linux-only manifest

## Provisional Type Placement

For the first implementation pass, the preferred home for the portable
`DriverLoweringPlan` type is a backend-neutral submodule inside
`crates/regseq-backends`.

That keeps the first Phase D implementation contained while preserving the
architectural split between portable lowering and OS-specific realization.

If that placement creates awkward coupling or circular-dependency pressure
during implementation, introducing a dedicated `regseq-lowering` crate is an
acceptable follow-on adjustment.

The immediate planning decision is simply:

- do not put `DriverLoweringPlan` into `regseq-model`
- do not make it Linux-specific
- keep it importable by multiple backends

## Litmus Test For Plan Sufficiency

The portable plan is too thin if an OS-specific backend planner must re-walk
`AnalysisInput` or import `regseq-model` data structures to recover semantics
that should already be present in `DriverLoweringPlan`.

In practical terms:

- backend planners should consume `DriverLoweringPlan`
- backend planners may make OS-specific choices from it
- backend planners should not have to rediscover callback-body structure,
  acquisition ordering, optional-resource behavior, or provenance partitioning
  from the original analysis/model inputs

If they do, the missing information belongs in the portable plan.

## Practical Rule for Task Authors

When authoring Phase D tasks:

- use `DriverLoweringPlan`-style language for portable compiler-owned lowering
- use `LinuxBackendPlan`-style language for Linux realization choices
- do not describe a Linux-specific structure as if it were the universal
  lowering contract

That split is the safest interface between backend code generators and the
compiler's internal model.
