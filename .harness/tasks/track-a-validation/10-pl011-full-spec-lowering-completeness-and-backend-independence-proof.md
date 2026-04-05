# Task: track-a-validation-10 - PL011 full-spec lowering completeness and backend-independence proof

## Description

Prove that the authored `pl011-full-spec` semantics required for the supported
slice are present in lowering plans and do not require generic Linux backend
code to hardcode PL011 facts.

This is the reviewer-facing gate between richer lowering and resumed Linux
backend completion.

## Input Context

Read before starting:

- `.harness/tasks/regseq-analysis/05-authored-body-capture-and-supported-subset-validation.md`
- `.harness/tasks/regseq-lowering/03-pl011-register-resource-and-property-realization-lowering.md`
- `.harness/tasks/regseq-lowering/04-pl011-serial-lifecycle-and-isr-behavior-lowering.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `tests/fixtures/pl011-full-spec/`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/src/linux/`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/query/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_mir_transform/src/lib.rs`

## Output Specification

Modify:

- `crates/regseq-backends/tests/`
- `tests/e2e/tests/` only if a reviewer-facing proof is clearer there
- narrow retained proof artifacts only if required for inspection

Add proof tests showing that the authored full-spec behavior is visible in the
plans and that generic backend code no longer needs PL011-specific encoded
facts to complete the supported slice.

Regression-context note:

- this proof exists because an earlier backend draft passed internal tests while
  still embedding PL011-specific knowledge in generic emitter code
- the validation should therefore guard against both accidental reintroduction
  of PL011 hardcoding and overly coarse lowering plans that force the backend
  to reconstruct authored behavior

## Implementation Notes

This task should prove at least the following:

- register offsets and bitfields needed by the supported slice are present in lowered data
- property field names and authored defaults are preserved in lowered data
- optional clock/reset/pinctrl metadata needed by the supported slice is preserved
- authored `Lifecycle`, `Serial`, ISR, and reachable-helper behavior is represented in structured plans
- Linux planning consumes those facts rather than inferring them from
  `compatible`, naming heuristics, or handwritten PL011 templates
- behavior-plan content is traceable to captured MIR callback/helper bodies,
  including cleanup-bearing control-flow where relevant, rather than
  compiler-synthesized PL011 templates
- the proof should make clear that the body source of truth is
  `mir_drops_elaborated_and_const_checked`, not an optimized MIR flavor that
  could hide relevant structure
- backend planning can consume the resulting plans without panicking or
  falling back to plan-external reconstruction

Traceability should not be hand-wavy. The proof should define a concrete
mechanism, such as:

- per-function coverage assertions
- per-helper coverage assertions
- provenance/query-flavor assertions on normalized MIR bodies
- structural coverage checks tying plan content back to captured MIR elements

The goal is to show that portable behavior is derived from the captured MIR,
not merely correlated with it.

This proof should also guard against backend regression. It is acceptable to
use narrow source-level assertions or regression tests if needed, but the goal
is not stylistic policing; the goal is to prove that generic compiler source is
not smuggling in PL011 hardware facts that belong in authored spec plus
lowering.

The regression scope should cover both:

- lowering source that could still synthesize PL011 behavior
- backend source that could still encode PL011/Linux templates
- explanatory examples/comments in generic lowering/backend source where those
  comments materially encode PL011 mask values, offset tables, or handwritten
  behavior templates

Do not turn this task into guest/QEMU smoke validation.

## Test Plan

Add tests to verify:

1. the lowered plans expose the authored full-spec semantics needed for the supported slice
2. Linux planning sees those semantics without backend reconstruction
3. generic backend code does not need hardcoded PL011 facts for the supported slice
4. the proof remains deterministic and reviewer-readable
5. at least one regression test verifies generic backend Rust source does not
   contain hardcoded PL011 register offsets, mask values, convenience defines,
   port-type literals, or device-naming literals outside plan-derived emission paths
6. every authored callback and reachable helper required by the supported slice
   has corresponding lowered behavior coverage
7. representative cleanup/drop behavior needed for the supported slice is
   visible in lowering plans and does not require backend reconstruction
8. provenance or equivalent metadata proves the body source of truth is
   `mir_drops_elaborated_and_const_checked`

Test commands:

- `cargo test -p regseq-backends`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- reviewer-facing proof exists that the supported full-spec semantics survive lowering
- reviewer-facing proof exists that portable behavior derives from captured
  MIR bodies rather than compiler-owned PL011 templates
- backend completion can proceed without relying on generic-source PL011 hardcoding
- the proof is strong enough to justify reopening Linux backend completion on a plan-driven basis
- the proof uses a concrete traceability mechanism rather than narrative-only claims
- tests pass
