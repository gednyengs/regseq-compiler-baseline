# PL011 Full Spec Methodology

This note records the methodology for defining the fuller PL011 Regseq spec
target before resuming Phase C model work.

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

## Core strategy

Use a dedicated external-style fixture crate:

- `tests/fixtures/pl011-full-spec/`

Grow that fixture incrementally, feature by feature, while keeping the existing
checked-in proof fixture separate:

- `tests/fixtures/pl011-spec-slice/`

The stable roles are:

- `pl011-spec-slice`: accepted, narrow, already-proven Phase B evidence fixture
- `pl011-full-spec`: exploratory but real full-target fixture used to discover
  what the fuller PL011 frontend requires

## Workflow

1. Start with an empty or near-empty `pl011-full-spec` crate.
2. Add one PL011 feature family at a time to the fixture.
3. After each change, compile the fixture through the real `regseq-driver`
   wrapper path.
4. If the feature compiles and extracts cleanly, keep it and continue.
5. If the feature exposes a missing frontend capability:
   - decide whether it is already accepted in the current taxonomy
   - if yes, record it as required reopened Phase B work
   - if no, stop and resolve the design first
6. Repeat until the intended fuller PL011 spec target is concretely expressed in
   the fixture.

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

It is intended to stabilize:

- the fuller PL011 Layer 1 surface
- the fuller PL011 Layer 2 expectations
- the construct set that the declaration model in Phase C must actually absorb

It is not yet for:

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

## Practical next move

Initialize:

- `tests/fixtures/pl011-full-spec/`

with a minimal external-style Cargo project and an empty `src/lib.rs`, then
grow it feature by feature from there.
