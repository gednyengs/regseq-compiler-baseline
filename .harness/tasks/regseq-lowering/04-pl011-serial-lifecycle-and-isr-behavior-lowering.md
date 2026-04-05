# Task: regseq-lowering-04 - PL011 serial, lifecycle, and ISR behavior lowering

## Description

Lower the executable body semantics in `pl011-full-spec` from
`mir_drops_elaborated_and_const_checked` MIR into structured, backend-usable plans so Linux emission does
not invent PL011 TX/RX, init, interrupt, or cleanup behavior on its own.

This task should capture the meaningful executable behavior of the current
fixture across `Lifecycle`, `Serial`, ISR handling, relevant helper paths, and
cleanup-bearing control-flow paths.

## Input Context

Read before starting:

- `.harness/tasks/regseq-lowering/03-pl011-register-resource-and-property-realization-lowering.md`
- `.harness/tasks/regseq-analysis/05-authored-body-capture-and-supported-subset-validation.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/src/linux/`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/statement.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/terminator.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/query/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_mir_transform/src/lib.rs`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`

Add the structured behavioral lowering needed for the current full PL011 slice,
including lifecycle sequencing, serial TX/RX logic, and interrupt-service
logic.

## Implementation Notes

This task should consume the captured MIR bodies from analysis and lower them
into structured, backend-usable portable behavior plans. It must not
synthesize PL011 callback logic from compiler-owned templates.

The lowering work should keep two distinct internal phases conceptually
separate, even if they share implementation files:

1. MIR interpretation:
   - walk the captured MIR
   - identify supported register/property/resource/control-flow semantics
   - preserve cleanup-bearing paths
2. portable semantic lowering:
   - transform those recognized semantics into the portable behavior plan
   - keep the result OS-neutral and backend-consumable

Do not collapse these into a single handwritten PL011 template generator.

The exact type names are up to the implementation, but the lowering boundary
must clearly distinguish:

- captured MIR body representation coming from analysis
- portable semantic behavior representation produced by lowering

If the current `CallbackBodyPlan` / `BehaviorPlan` split is redundant or weak,
this task must resolve that ambiguity. There should be one unambiguous
portable behavior representation that later backend work actually consumes for
code emission.

The portable behavior representation should be able to represent at least:

- lifecycle behavior:
  - init/deinit
  - open/close/configure
  - clock/reset/pinctrl participation
  - ordered sequencing
- serial behavior:
  - tx-byte fast path
  - staged pending-TX path
  - rx-char path
  - tx-ready / tx-empty predicates
  - fifo flush hooks
  - break-control behavior
- interrupt behavior:
  - ISR handler identity and authored bind target
  - interrupt-status read
  - interrupt class discrimination using lowered interrupt/register facts rather
    than backend hardcoded PL011 mask literals
  - RX/TX/error class discrimination
  - explicit interrupt clear
  - TX service / wakeup behavior
  - handled vs not-handled outcomes
- helper/body actions:
  - register reads/writes/modifies
  - property/resource interactions
  - conditionals
  - loops
  - assignments
  - intermediate computations and expressions
  - delays
  - cleanup/drop effects that are semantically relevant to correct generated code
  - portable subsystem-semantic actions where truly necessary

If the current portable behavior vocabulary cannot represent cleanup-bearing
regions or failure-path cleanup strongly enough, extend it explicitly rather
than hand-waving cleanup as incidental control flow.

Implementation continuity note:

- build on the lowering structures already introduced in `regseq-lowering-01/02`
  and extended by `regseq-lowering-03`
- do not throw away working plan scaffolding just because the earlier backend
  draft was incorrect
- the corrective goal is richer, more precisely scoped lowering data, not a
  parallel replacement pipeline
- all reachable helpers required by the supported slice must either have their
  own portable behavior entries or be explicitly inlined into the lowered
  callback behavior; opaque helper references are not enough if backend
  emission would still need to guess their bodies
- cleanup-bearing control-flow paths from MIR, including failure exits and
  scope exits relevant to correctness, must be represented strongly enough
  that later backend work does not need to reconstruct cleanup insertion
- if `?`/fallible callbacks appear, their MIR-desugared control flow must be
  lowered from the explicit `SwitchInt`/return/cleanup structure rather than
  from a source-level `Try` abstraction
- `ordered!` does not survive macro expansion as a named construct; if ordered
  regions are represented in the portable plan, derive them from the expanded
  synchronization call pattern (for example paired barrier calls), not from a
  preserved macro marker

The rule for this task is:

- generic Linux backend code must not contain handwritten PL011 callback logic;
  it must consume structured lowered behavior
- structured callback actions in the portable plan must use OS-neutral semantic
  vocabulary rather than Linux serial-core API names; each OS backend should
  map those portable semantic actions to its own subsystem APIs
- structured behavioral plans must scope actions to individual callback
  functions such as `init`, `deinit`, `port_open`, `port_close`, `configure`,
  `tx_byte`, `rx_char`, and `handle_irq`, not just callback families
- register-write sequences authored inside source-level `ordered!` blocks, if
  represented specially at all, must be recognized from the expanded barrier
  calls in MIR and then carried as explicit ordering constraints rather than as
  a source-macro artifact
- portable lowering must not hardcode interrupt-class masks, FIFO service
  templates, baud-programming formulas, callback flows, or cleanup placement
  unless those facts are directly materialized from captured MIR bodies and
  already-lowered register/resource/property data
- do not assume `optimized_mir` as the extraction source for this task; the
  portable lowering input should be the less aggressively transformed
  `mir_drops_elaborated_and_const_checked` flavor
- if the current semantic node vocabulary is too UART-shaped or too close to
  Linux realization, refactor it toward a more generic computation/control-flow
  representation rather than preserving device-specific portable nodes

Do not permit the backend to hardcode:

- PL011 RX/TX FIFO service logic
- PL011 interrupt mask classes
- baud-programming sequences
- startup/shutdown ordering
- serial-driver naming policy tied to this one device
- cleanup/drop placement for scope/failure exits

Those semantics must be present in the lowering plan.

Do not widen scope to deferred PL011 features such as DMA, PM save/restore,
family-wide quirks, modem control, or console takeover.

## Test Plan

Add tests to verify:

1. `Lifecycle`, `Serial`, ISR, and relevant cleanup behavior are represented in structured plan data
2. the structured plans preserve ordering and branch/loop boundaries needed for code emission
3. callback actions are scoped to individual authored functions rather than only callback families
4. Linux planning can consume the structured behavior without inventing PL011 facts
5. the plans remain deterministic and reviewer-readable
6. missing behavior required by the supported slice is detected rather than silently synthesized
7. all reachable helpers required by the supported slice are represented
8. if coarse legacy callback-action summaries remain, they are not the backend-consumed source of truth for behavior emission
9. cleanup-bearing MIR paths needed by the supported slice survive lowering strongly enough for later codegen
10. any ordering semantics originating from `ordered!` are derived from MIR-visible barrier behavior, not from a preserved macro node

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- lowering plans capture the full-spec executable behavior needed for the supported slice
- lowering derives behavior from captured MIR bodies rather than compiler-owned PL011 templates
- Linux-specific backend work can rely on plan data rather than backend-owned PL011 logic
- the full-spec TX/RX/init/ISR paths are materially represented, not summarized away
- portable callback/body actions remain OS-neutral and do not encode Linux
  serial-core APIs directly
- all reachable helpers required by the supported slice are represented strongly enough that later backend emission does not need to invent helper logic
- cleanup/drop semantics required for correct code generation are represented strongly enough that later backend emission does not need to reconstruct them
- MIR interpretation and portable semantic lowering are cleanly separated in the
  resulting design, even if implemented in one crate/module
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
