# PL011 Full Spec Methodology

## REFRAMING (2026-04-20)

This note has been reframed by the round-4 Codex-reviewed strategy
synthesis. The working methodology has shifted from
"feature-inventory decides what we implement; real drivers help
confirm how that feature behaves" (Section 4 below, preserved for
history) to:

- **comparison axes**, derived from concrete PL011 system-use cases,
  decide what goes in the spec;
- **corpus drivers** are baselines for comparison and policy
  sources for convergent best-practice (e.g., ISR pass bounds, DMA
  thresholds, spurious-RX drain), **not** inclusion gates;
- the spec is evidence-driven along three requirement-sourcing
  classes (axis / correctness / backend-contract) per
  `pl011-spec-requirements.md`.

Authoritative artifacts:

- `pl011-comparison-axes.md`
- `pl011-workloads.md`
- `pl011-spec-requirements.md`

Fixture growth discipline below (Section "Workflow", etc.) remains
valid for the engineering process, but feature selection now traces
through the axes, not a standalone feature-matrix.

---

# Historical content (below)

This note records the methodology that was used to define the fuller PL011
Regseq spec target.

It should now be read primarily as historical process context. The current
concrete target fixture already exists at `tests/fixtures/pl011-full-spec/`,
and Phase C is complete in the current repo state.

## Why this methodology

The current polling-first PL011 slice is useful as a proven frontend specimen,
but it is too narrow to confidently shape the declaration model once and avoid
revisiting that model later under richer PL011 pressure or, worse, during E1000
work.

The goal of this methodology is to:

- drive the full PL011 target from a real Regseq-authored fixture
- surface missing frontend DSL/compiler support early
- convert those gaps into explicit decisions or reopened Phase B expansion work
- avoid speculative declaration-model design based on an artificially small
  slice

For the current effort, "full PL011" means a fuller single-variant ARM PL011
spec, not a family-wide PL011 variant framework. Variant/revision/erratum
machinery is intentionally deferred unless it becomes necessary for the primary
paper claims.

This also means the fixture should not absorb PM behavior that only appears in
deferred PL011 variants. In particular, generic `arm,pl011` PM behavior should
not be expanded from Ambiq-specific suspend/resume evidence.

## Core strategy

Use a dedicated external-style fixture crate:

- `tests/fixtures/pl011-full-spec/`

Grow that fixture incrementally, feature by feature, while keeping the existing
checked-in proof fixture separate:

- `tests/fixtures/pl011-spec-slice/`

The stable roles were and remain:

- `pl011-spec-slice`: accepted, narrow, already-proven Phase B evidence fixture
- `pl011-full-spec`: the fuller target fixture that was used to discover what
  the richer PL011 frontend required and now serves as the concrete full-PL011
  target fixture

## Workflow

1. Start with an empty or near-empty `pl011-full-spec` crate.
2. Choose the next feature family from the agreed full-PL011 target using the
   feature matrix and boundary notes as the scope source of truth.
3. For that feature family only, inspect existing driver sources where the
   feature is implemented and extract the hardware/driver intent that must be
   represented in Regseq.
4. Do not trace entire OS drivers end to end and do not mirror framework glue,
   allocation/setup scaffolding, or backend-only control flow into the Regseq
   spec.
5. Add the feature to the fixture in Regseq form.
6. After each change, compile the fixture through the real `regseq-driver`
   wrapper path.
7. If the feature compiles and extracts cleanly, keep it and continue.
8. If the feature exposes a missing frontend capability:
   - decide whether it is already accepted in the current taxonomy
   - if yes, record it as required reopened Phase B work
   - if no, stop and resolve the design first
9. Repeat until the intended fuller PL011 spec target is concretely expressed in
   the fixture.

### Per-feature review loop

For each feature family, use the same four-step loop:

1. Inspect the Linux, FreeBSD, and Zephyr corpora for that feature only, using
   one researcher per OS through the cross-cli messenger channel.
2. Synthesize the common hardware/driver intent and the Layer 1 / Layer 2 /
   compiler split before changing the fixture.
3. Add only that feature to `pl011-full-spec` and compile-check it through the
   real `regseq-driver` wrapper path.
4. Use the researchers again to judge whether the resulting Regseq frontend can
   lower/match the target OS correctly under Regseq's layering rules.

## Practical interpretation

The feature matrix and boundary notes answer:

- which features belong in the full PL011 target
- which are optional
- which are deferred

Real driver sources are used only as localized evidence when implementing a
chosen feature. Their role is to confirm the intent and hardware semantics of
that feature, not to dictate the structure of the Regseq spec.

In other words:

- feature inventory decides **what** we implement
- real drivers help confirm **how that feature behaves**
- the Regseq fixture is the primary authored artifact

If the source evidence shows target-specific ordering or sequencing constraints
that the compiler cannot safely infer from plain register reads/writes alone,
capture that intent explicitly in the fixture using the existing Regseq-owned
ordering surface such as `regseq::ordered!`, rather than assuming the backend
can rediscover it later.

## Current source corpus for feature evidence

When implementing a chosen feature family, use the following driver sources as
localized evidence:

- Linux
  - `reference-drivers/pl011/linux/serial_pl011.h`
  - `reference-drivers/pl011/linux/amba-pl011.c`
- FreeBSD
  - `reference-drivers/pl011/freebsd/uart_dev_pl011.c`
- Zephyr
  - `reference-drivers/pl011/zephyr/arm_pl011.yaml`
  - `reference-drivers/pl011/zephyr/uart_pl011_registers.h`
  - `reference-drivers/pl011/zephyr/uart_pl011.c`

## Proposed feature-addition order

To keep the fixture growth controlled and reduce declaration-model churn, add
features in the following order:

1. Base declarations
   - `Pl011Registers`
   - `Mmio<Pl011Registers>`
   - core properties:
     - `current-speed`
     - `fifo-size`
   - `Interrupt`
2. Polling serial bring-up
   - `Lifecycle`
   - polling `Serial`
   - basic UART enable/disable, baud programming, line-control, and FIFO setup
3. RX error handling
4. Interrupt-driven path
   - IRQ usage
   - `#[rseq::isr]`
   - `IMSC` / `RIS` / `MIS` / `ICR`
   - interrupt mask lifecycle
5. Clock participation
6. Reset participation
7. PinCtrl participation
8. PM-aware path
   - currently deferred for the generic `arm,pl011` target
   - revisit only if we intentionally widen to a PM-capable PL011 variant
9. Optional richer serial features
   - hardware flow control
   - `SerialConsole` if needed
   - break control if needed
10. DMA

Explicit deferrals for this version remain:

- PM-aware path for variant-specific PL011 integrations
- modem-control/status lines
- RS-485
- loopback
- IrDA / SIR
- family-wide variant/revision/quirk machinery

This order is intended to:

- build from declarations to behavior
- expose missing frontend/compiler support in a controlled way
- make sure major declaration-model pressure lands before Phase C resumes

## Important discipline

Do not silently treat the exploratory fixture as a backlog dump.

Every time a new feature reveals a gap, record one of:

- an accepted gap that should become reopened Phase B expansion work
- an explicit deferral
- a design question that must be resolved before continuing

The full-spec fixture should therefore be evidence-driven, not an uncontrolled
accumulation of partial ideas.

## Relationship to the compiler pipeline

This methodology is still frontend-first.

It was intended to stabilize:

- the fuller PL011 Layer 1 surface
- the fuller PL011 Layer 2 expectations
- the construct set that the declaration model in Phase C must actually absorb

It was not intended for:

- normalized model design
- Layer 1 + Layer 2 join logic
- lowering
- backend realization

## Expected outcome

By the time this process is complete, we should have:

- a much fuller PL011 Regseq fixture that compiles through the compiler path
- a concrete list of missing accepted frontend capabilities that required
  reopening Phase B
- a much safer starting point for Phase C normalized declaration-model work

## Current interpretation

That initialization/growth work has already happened. The practical use of this
note now is:

- understand why the full-spec fixture contains the features it does
- understand which deferrals were intentional
- avoid reintroducing deferred variant- or PM-specific PL011 behavior by
  accident during lowering planning
