# Task: regseq-linux-backend-02 - PL011 Linux driver emission for supported slice

## Description

Emit the first build-oriented generated Linux output for the supported PL011
slice.

This task should turn the now-richer lowering plans into a reviewable
out-of-tree Linux module artifact set for `pl011-full-spec`, including
AMBA-shaped registration, serial-core integration scaffolding, and the first
generated build collateral needed for host-side compilation.

## Input Context

Read before starting:

- `.harness/tasks/regseq-linux-backend/01-linux-artifact-layout-and-backend-emission-scaffold.md`
- `.harness/tasks/regseq-driver/24-semantic-ir-phase0-extraction-enrichment.md`
- `.harness/tasks/regseq-model/14-item-hierarchy-schema-and-canonical-cross-link-model.md`
- `.harness/tasks/regseq-lowering/05-semantic-procedural-ir-types-and-boundary.md`
- `.harness/tasks/regseq-lowering/06-item-hierarchy-construction-from-enriched-extraction.md`
- `.harness/tasks/regseq-lowering/07-supported-regseq-semantic-body-lowering-validated-on-pl011-representative-set.md`
- `.harness/tasks/regseq-lowering/08-semanticmodel-projection-and-driverloweringplan-behavior-migration.md`
- `.harness/tasks/track-a-validation/10-pl011-full-spec-lowering-completeness-and-backend-independence-proof.md`
- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/linux-codegen/pl011-linux-first-slice.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`
- generated-output fixture snapshots only if narrow checked-in expectations are
  needed for deterministic tests

The emitted output should be capable of host-side out-of-tree module builds and
should live under the repo-local generated output root.

## Implementation Notes

This task should emit the first Linux generated-driver slice for PL011,
including enough code and build collateral to support the first real build loop.

Semantic-IR continuity note:

- this task is now explicitly downstream of the semantic-IR prerequisite chain
  (`regseq-driver-24`, `regseq-model-14`, `regseq-lowering-05`,
  `regseq-lowering-06`, `regseq-lowering-07`, `regseq-lowering-08`)
- the required backend input boundary is now `DriverLoweringPlan.behavior:
  SemanticModel`
- do not route around that chain by reviving `BehaviorPlan`-centric emitter
  logic or re-deriving semantics from raw MIR/HIR-shaped data inside Linux
  emission

The emitted Linux path should explicitly cover:

- AMBA-shaped driver registration for PL011
- Linux match/ID-table collateral appropriate to the supported PL011 path
- `uart_driver` definition
- `uart_ops` definition
- probe/remove orchestration for the supported slice
- MMIO/register helper realization
- interrupt registration/ISR realization
- firmware-property reads with fallback behavior
- Linux-specific realization of property default values using lowered authored
  defaults, not backend-generated placeholder constants
- optional clock/reset/pinctrl hooks where required by the supported slice
- synchronization, ordering, logging, and timed-delay realization already
  justified by the first slice
- Linux port-type allocation/naming derived from backend-plan data rather than
  hardcoded literals
- out-of-tree build files needed for the first generated module build
- Linux API realization targeted to the pinned evaluation kernel rather than
  stale serial-core API assumptions

The emitted code does not yet need to satisfy the full hot-path functional loop,
but it should be honestly structured for it rather than using fake stubs where
the supported slice already requires real lowering.

Implementation continuity note:

- the current `regseq-linux-backend-02` implementation is not accepted and has
  been reopened because generic backend source still hardcodes PL011 facts
- the latest rejected implementation also exposed a second class of failure:
  plan-driven emission is still incomplete even where obvious PL011 literals
  were removed
- reviewers and implementers must treat the following as concrete rejection
  reasons, not optional cleanup:
  - authored helper signatures are not yet emitted correctly. The current
    backend path still assumes every helper can be emitted as
    `static void <name>(struct <driver>_data *d)`, even though the full-spec
    includes parameterized helpers such as `set_baud_rate(uart_clk_hz,
    baud_rate)` and `clear_explicit_interrupts(status)`. The backend must not
    erase those authored parameters or pretend they do not exist.
  - Linux callback realization is still reconstructing behavior in the backend
    by mapping the wrong portable roles onto Linux callbacks. In particular, it
    is not acceptable to reuse `SerialTxByte` behavior for both `start_tx` and
    `stop_tx`, or to reuse `SerialFlushRxFifo` behavior for `stop_rx`, unless
    the lowered plan explicitly justifies those Linux callbacks as the correct
    realization target.
  - the default lowering path still does not honestly resolve the full-spec's
    unevaluated symbolic interrupt-mask constants. If backend tests pass only
    because they call an explicit `*_with_constants(...)` entry point, then the
    claimed normal-path correctness is not real yet.
  - source-level zero-hardcoding audits must include comments/examples in
    generic backend emitter source as well as executable code. Residual PL011
    examples in generic emitter modules are still a regression against the
    accepted architectural rule.
- tests that only validate Rust-level determinism or string-shape properties are
  not sufficient for acceptance if they do not exercise:
  - helper signature correctness
  - helper call-site argument correctness
  - Linux callback/body mapping correctness
  - default-path interrupt-mask resolution correctness
- reuse valid backend scaffold and file-layout work from earlier implementation
  where it remains compatible with the corrected architecture
- remove or rewrite any generic backend logic that hardcodes PL011 register
  layouts, interrupt classes, callback logic, naming policy, or Linux-only
  assumptions that should instead come from lowered plans
- do not paper over plan gaps by adding new backend-owned PL011 shortcuts
- rewrite the currently hardcoded emitter paths rather than routing around
  them with new special cases. In particular, generic backend source must stop
  hardcoding behavior in:
  - `crates/regseq-backends/src/linux/emit_module.rs`
  - `crates/regseq-backends/src/linux/emit_regs.rs`
  Specific emitter responsibilities that must become plan-driven include:
  - module constants and property default realization
  - ISR body emission
  - startup/shutdown/set_termios emission
  - register-offset/bitfield/named-value emission
  - device naming / port-type realization
- emitter functions that accept plan inputs must actually consume them; unused
  `_plan`-style parameters are not an acceptable way to preserve handwritten
  templates behind a nominally plan-driven interface
- if the backend cannot emit a correct helper or callback body from the current
  lowered plan surface, the correct fix is to extend the backend-consumed plan
  contract or the portable lowering output, not to substitute handwritten Linux
  behavior in the emitter
- the latest host-build proof attempt surfaced a third class of rejection that
  is now part of this task's explicit contract:
  - the backend is still emitting producer-side behavioral output that is not
    valid C, even though top-level file layout, AMBA registration, and other
    structural Linux scaffolding are present. The rejection is specifically in
    the helper/callback body emission path and must be treated as a correctness
    failure in the compiler, not as a cosmetic cleanup item.
  - raw Rust path syntax from the source/MIR world is leaking into emitted C.
    Observed examples include forms such as:
    - `regseq::PinCtrl::apply(...)`
    - `regseq::Clock::enable(...)`
    - generic wrapper spellings such as `regseq::ReadWrite::<T, R>::write(...)`
    These are Rust-language expressions and are not acceptable in final C
    output under any circumstances.
  - raw MIR local / SSA names such as `_3`, `_8`, `_13`, `_14`, `_25` are
    leaking into final C output. This is not just a naming problem. These
    locals are part of the MIR semantics, so the backend must lower them into
    valid emitted C locals/temporaries in a way that preserves:
    - evaluation order
    - data dependencies
    - parameter flow
    - branch behavior
    - cleanup-bearing error-path behavior
    Simply stripping `_N` names out of the emitted source is not a valid fix.
  - helper declaration/definition ordering is still not C-correct. Generated
    helpers may be called before they are defined without the required forward
    declarations, prototype emission, or alternative stable ordering strategy.
    The generated output must be compile-correct as C translation-unit text,
    not merely structurally suggestive.
  - the host-build proof infrastructure is functioning correctly and should be
    treated as authoritative evidence for this rejection. This task must
    harden the producer until the canonical compiler-driven host build passes.
- the latest wrapper-driven canonical emission check surfaced a fourth class of
  rejection that must now be treated as explicit task-owned evidence, not as a
  downstream validation-only concern:
  - fresh compiler-driven emission under
    `tests/fixtures/pl011-full-spec/regseq-out/` no longer leaks raw Rust path
    spellings and now uses underscore-safe filenames, but the generated helper
    and ISR bodies are still not compile-valid C. This proves the current
    producer is still failing after the earlier hardcoding and Rust-syntax
    cleanup fixes.
  - concrete observed failures in the fresh emitted
    `regseq_pl011_full_spec_module.c` include:
    - undeclared temporaries such as `tmp3`, `tmp6`, `tmp11`, `tmp14`,
      `tmp24`, and `tmp30` used in conditions, register writes, and helper
      logic without matching declarations or dominating assignments
    - undeclared `ret` usage and impossible duplicated return structures such
      as `if (ret) { return 0; return ret; }`, which show malformed lowering of
      fallible/resource paths rather than merely incomplete naming cleanup
    - `uart_clock_hz` emitted as `static u32 ... { return; }`, which is invalid
      C and proves the `Option`/resource-read value path is still not lowered
      into a concrete return expression
    - helper bodies such as `clear_explicit_interrupts`, `enable_tx_interrupts`,
      `set_baud_rate`, `clear_all_interrupts`, `enable_uart`, and ISR logic
      that still reference nonexistent temporaries instead of valid lowered
      C-local state
    - malformed ISR text, including a bare register-read statement without a
      terminating semicolon, use of undefined temporaries throughout the
      decision tree, and `return 0;` in an `irqreturn_t` function instead of a
      correct `IRQ_*` result
  - these failures mean the current backend-local test suite is too weak: it
    passes even while the canonical wrapper-driven artifact is still invalid.
    That weakness is part of this task's rejection, not a separate cleanup.
  - the wrapper-driven e2e emission test is also stale in one specific way: it
    still expects the old hyphenated filename inventory even though the
    producer now emits underscore-safe names. That test must be updated so it
    can serve as a trustworthy regression gate for this task rather than a
    stale expectation harness.

The new producer-side rule for this task is:

- final emitted C must contain only lowered C concepts, not raw Rust syntax,
  raw MIR spellings, or unresolved intermediate compiler notation

That implies, concretely:

- no `regseq::...` path spellings in emitted C
- no `::<...>` generic Rust syntax in emitted C
- no raw MIR locals such as `_3` or `_14` in emitted C unless they have been
  deliberately lowered into valid C declarations / uses under a stable naming
  strategy
- no helper call-before-definition compile failures
- no backend textual cleanup that discards MIR semantics without a real lowering
  step

Producer-side lowering guidance for this reopened defect:

- semantic calls must lower to their Linux/C realization, not print Rust spellings
  into the output
- MIR locals/temporaries must lower through an explicit environment that maps
  MIR values to valid emitted C locals / expressions
- authored parameter flow must remain intact through helper signatures, helper
  call sites, and temporary/value lowering
- if generic wrapper/resource/register call sites need type/substitution facts,
  use the facts already present in the captured MIR/model surface where
  available; do not invent Rust-like generic spellings in the C emitter
- if the current owned MIR/model surface proves insufficient to lower a class
  of calls or temporaries correctly, the implementer must surface that as a
  producer-side contract gap rather than hiding it in emitted text

The key architectural rule for this task is:

- generic Linux backend source must not encode PL011 hardware facts or callback
  behavior that should come from the authored spec plus lowering plans

That means, for example:

- no hardcoded PL011 register offsets, masks, or IRQ-class facts in generic
  backend source
- no hardcoded `ttyPL`-style device naming policy tied to this one device
- no hardcoded Linux `PORT_*` constant assignment that is not derived from
  backend-plan policy/data
- no backend reconstruction of property field names/defaults from string
  heuristics
- no backend erasure of authored helper parameters, call arguments, or
  signature shape
- no backend substitution of one authored callback's behavior for a different
  Linux callback merely because the signatures are superficially convenient
- no handwritten TX/RX/ISR behavior templates in generic emitter code
- no handwritten convenience bitfield defines in generic emitter code when the
  lowered register/named-value data already contains the needed facts
- no stale Linux serial-core API usage carried over from older handwritten templates

If a generated artifact needs a device-specific constant or behavior, the
backend should materialize it from lowered plan data into the emitted output,
not encode it in the compiler source.

For PL011 specifically:

- treat the driver as `bus = "amba"`
- retain concrete PL011 identity alongside the AMBA registration path
- do not regress to a generic `platform_driver` interpretation
- coordinate the emitted `amba_id` table with the DT
  `arm,primecell-periphid` override used by the evaluation environment's
  reserved second-UART strategy

Do not turn this task into:

- guest boot/load validation
- benchmark harness work
- console-takeover integration
- PM, DMA, modem-control, RS-485, or family-wide variant machinery
- handwritten per-driver wrapper glue that owns core Linux registration duties

## Test Plan

Add tests to verify:

1. the backend can emit a complete PL011 artifact set for the supported slice
2. the emitted driver includes AMBA-shaped registration and serial-core
   scaffolding
3. emitted build collateral is sufficient for the later out-of-tree build task
4. emitted output remains deterministic and reviewer-readable
5. emitted Linux code is driven from lowered plan data rather than generic-source PL011 hardcoding
6. emitted Linux code targets the pinned evaluation kernel API rather than stale handwritten serial-core assumptions
7. emitted helper and callback bodies are valid C-shaped lowered output rather
   than raw Rust/MIR text
8. helper declaration/prototype ordering is sufficient for C compilation before
   the later host-build gate runs
9. emitted code contains no raw Rust path syntax, generic Rust syntax, or raw
   MIR local names in final C output
10. canonical compiler-driven host-build evidence from `track-a-validation-11`
    no longer surfaces producer-side behavioral-emission defects attributable
    to this task

The strengthened backend-local test strategy for this reopened task should
include targeted assertions over final emitted source text, not only structural
artifact presence. At minimum, add tests that fail if emitted `_module.c`
contains any of the following patterns:

- `regseq::`
- `::<`
- raw MIR local forms matching a pattern like `\\b_[0-9]+\\b`

unless such a match is part of a deliberately emitted, valid, declared C local
under the backend's stable local-lowering policy.

Also add tests that prove:

- helper functions are either defined before first use or have valid forward
  declarations/prototypes before first use
- lowered helper/callback bodies use C-valid local names and call forms
- register/resource/wrapper operations that originate as MIR/Rust constructs
  are emitted as Linux/C realization code rather than Rust surface spelling
- any producer-side name-flattening policy (for driver/file/helper identifiers)
  is deterministic and C-valid
- fresh wrapper-driven emission through the real `EmitSpec::Driver` path
  produces the same compile-valid helper/ISR body shapes as backend-local
  emission tests assume
- positive C-validity assertions over emitted helper/ISR bodies catch:
  - invalid `return;` in non-void helpers
  - undeclared `ret` usage
  - uses of undeclared `tmp*` temporaries
  - malformed statements lacking required semicolons
  - invalid `irqreturn_t` return shapes
- value-producing helper paths such as `uart_clock_hz` emit real C expressions
  of the correct type rather than empty returns or placeholders
- fallible/resource helper paths emit compile-valid control flow and return
  structure rather than duplicated or contradictory return statements
- e2e emission tests use the current underscore-safe filename policy and fail
  if the canonical compiler-driven artifact inventory regresses

Test commands:

- `cargo test -p regseq-backends`
- `cargo test -p regseq-e2e --test pl011_driver_emission -- --ignored`
- `cargo clippy -p regseq-backends -- -D warnings`
- `cargo clippy -p regseq-backends --all-targets -- -D warnings`

## Acceptance Criteria

- generated PL011 Linux output exists for the supported slice
- the output is structured for out-of-tree builds
- AMBA-shaped registration and serial-core integration are emitted by the
  backend
- generic backend source does not encode PL011-specific hardware or callback logic
- generic backend source does not encode handwritten PL011 register offset
  tables, convenience defines, device naming, or termios/startup/ISR templates
- final emitted helper/callback bodies are valid C output, not raw Rust/MIR
  text
- final emitted C contains no raw Rust path syntax, no Rust generic syntax, and
  no unlowered MIR local spellings
- final emitted helper/callback/ISR bodies from the canonical wrapper-driven
  path contain no undeclared temporaries, no undeclared `ret` usage, no empty
  `return;` in non-void functions, and no malformed statement fragments
- generated helper ordering/prototype strategy is sufficient for C compilation
- backend-local tests are strong enough to catch these producer-side failures
  before the host-build gate runs
- wrapper-driven e2e emission tests are updated to the current filename policy
  and pass against the canonical emitted artifact inventory
- output remains deterministic and readable enough for review
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
