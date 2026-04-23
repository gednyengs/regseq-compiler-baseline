# PL011 Full Spec Status

## REFRAMING (2026-04-21)

Status tracking has been realigned against the 3-stage
progressive implementation plan. Stage membership for every
REQ-* is authoritatively defined in `pl011-spec-requirements.md`
Sections 1 (trace legend) and 1.2 (stage membership summary);
this document does not re-list the stage assignments and defers
to that source to avoid drift.

This document tracks a different dimension: **implementation
state** of each requirement in the live fixture at
`tests/fixtures/pl011-full-spec/src/driver.rs`. The two
dimensions are:

- **stage membership** (what section of the plan a REQ belongs
  to; authoritative source: `pl011-spec-requirements.md`).
- **implementation state** (whether the fixture currently
  captures the requirement; authoritative source: this doc).

A Stage-1 requirement can be `captured` (the fixture already
expresses it) or `in-progress` (fixture work is scheduled for
Stage 1 but not yet merged). A Stage-2 or Stage-3 requirement is
usually `not-yet-captured` until its stage opens.

### Stage 1 implementation-state snapshot (as of 2026-04-21)

**Already captured in the fixture** (no further work needed for
Stage 1 spec-surface closure):

REQ-TX-01, REQ-TX-02, REQ-RX-01, REQ-RX-02, REQ-RX-03, REQ-RX-04,
REQ-CFG-01 (divisor now uses round-to-nearest per datasheet
§3.3.6), REQ-CFG-02, REQ-CFG-03, REQ-CFG-04, REQ-FIFO-01,
REQ-FIFO-02 (static IFLS selection only; runtime
reconfigurability deferred per REQ-PERF-IFLS-01 in Stage 3),
REQ-FIFO-03, REQ-LIFE-01, REQ-LIFE-02, REQ-IRQ-01, REQ-IRQ-02
(RX/TX/error sources), REQ-IRQ-03, REQ-IRQ-04, REQ-BRK-01,
REQ-RES-CLK-01/02, REQ-RES-RST-01, REQ-RES-PIN-01,
REQ-RES-IRQ-01, REQ-RES-MMIO-01, REQ-ID-01, REQ-ID-02,
REQ-PERF-BATCH-01 (pending_tx removed; tx_byte is now an
unchecked primitive matching `pl011_tx_char` in the Linux
corpus), REQ-PERF-LATENCY-01, REQ-CORR-REGORDER-01,
REQ-CORR-REGORDER-02, REQ-CORR-RESERVED-01 (captured by
framework; see note below), REQ-CORR-ICR-W1C,
REQ-CORR-MASK-SHADOW, REQ-BC-RES-TABLE, REQ-BC-ISR-ENTRY,
REQ-BC-LIFECYCLE.

REQ-CORR-RESERVED-01 capture rationale:
`crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:47-65`
distinguishes `write_field` (pure register write; bits outside
the fields' masks become zero) from `modify` (RMW preserving
unmasked bits). Stage-1 fixture usages of `write_field` happen
to write 0 to reserved bits, which matches the datasheet's
"reserved, read as zero" convention. Stage 2 CR-bit
manipulations for flow control (REQ-FLOW-01) must switch to
`modify` so RTS/DTR state is preserved across disable/enable
cycles (REQ-LIFE-03).

**Scheduled fixture work for Stage 1** (Rust DSL changes to
`tests/fixtures/pl011-full-spec/src/driver.rs`): **none remain**.
The Stage 1 fixture is paper-min complete: the three-round
Codex audit loop (2026-04-21) converged at Round 3 with verdict
`stage-1-complete`. Transcripts preserved at
`audit-history/stage-1-round-1-gap-report.md`,
`audit-history/stage-1-round-2-verification.md`, and
`audit-history/stage-1-round-3-convergence.md`; see
`audit-history/README.md` for the round summary.

**Scheduled backend-emission work for Stage 1** (Linux and Zephyr
backend emitter work; realized by Phase-7 emitter retarget and
the hand-authored Linux golden reference; not changes to the
fixture):

- **REQ-INT-LIN-01** (Linux `uart_ops` backend emission): Stage 1
  backend work; lands with the Phase-7 emitter retarget and the
  hand-authored Linux reference driver. `uart_ops` set required:
  `startup`, `shutdown`, `start_tx`, `stop_tx`, `set_termios`,
  plus counter wiring. CRTSCTS branches in `set_termios` and
  `throttle`/`unthrottle` arrive with Stage 2.
- **REQ-INT-ZEP-01** (Zephyr `uart_driver_api` backend emission):
  Stage 1 backend work, polling and interrupt-driven subsets;
  gated on the control-channel proof per
  `pl011-experiment-methodology.md` Section 5.1.

### Known fixture issues (discovered 2026-04-23)

The Stage-1 Codex audit (2026-04-21) converged on
`stage-1-complete` for requirement coverage and structural
invariants. It did NOT exercise the full compiler pipeline on
the fixture end-to-end. A subsequent session (2026-04-23)
attempted to regenerate golden artifacts after the user-data
initialization Path A landed (task `regseq-model-64`, see
`docs/discussions/models/regseq-model/user-data-initialization.md`)
and discovered that the compiler's construction pass skips four
method bodies on the PL011 fixture due to constructs the
pipeline does not yet represent. Each skip means the
corresponding body has no semantic-IR golden; regenerating
goldens without first addressing these issues produces partial
coverage.

The four issues, in order of implementation difficulty:

1. **`line_control_value` -- tuple destructuring** at
   `tests/fixtures/pl011-full-spec/src/driver.rs:190`. The
   method destructures a tuple returned from a `match`:
   ```rust
   let (pen, eps) = match config.parity { ... };
   ```
   Compiler error: "rustc type (FieldValue<u32, lcr_h::Register>,
   FieldValue<u32, lcr_h::Register>) has no Regseq IR
   representation". This hits the Stage-1 Rule-1 tuple exclusion
   documented in
   `docs/discussions/models/regseq-model/user-data-initialization.md`
   ("Stage-1 Rule-1 whitelist narrowing (tuples excluded)").
   **Fix**: split the tuple destructuring into two separate
   `match` expressions (one for `pen`, one for `eps`). Trivial
   fixture-only edit.

2. **`uart_clock_hz` -- method-reference ZST** at
   `tests/fixtures/pl011-full-spec/src/driver.rs:334`. The
   method uses `Option::map(Clock::get_rate)`:
   ```rust
   self.clock.as_ref().map(Clock::get_rate).unwrap_or(...)
   ```
   Compiler error: "unsupported constant shape ZeroSized". The
   `Clock::get_rate` passed as a function pointer materializes
   a zero-sized function-item constant that the compiler's
   `ConstantIndex` does not accept.
   **Fix**: replace the method-reference with a closure:
   `.map(|c| c.get_rate())`. Trivial fixture-only edit.

3. **`rx_char` -- bitflags constant missing from ConstantIndex**
   at `tests/fixtures/pl011-full-spec/src/driver.rs:529`. The
   method uses bitflags constants such as
   `SerialErrors::OVERRUN`:
   ```rust
   errors = errors | SerialErrors::OVERRUN;
   ```
   Compiler error: "unsupported constant shape: unevaluated
   constant regseq::SerialErrors::OVERRUN missing from
   ConstantIndex".
   **Classification unclear** until investigated: this may be a
   compiler-extraction gap (framework-type bitflags constants
   should be auto-extracted but currently are not) or a fixture
   misuse (bitflags patterns aren't yet supported and the
   fixture should inline numeric values). Investigation effort:
   15-30 minutes.

4. **`set_baud_rate` -- rustc `Assert` terminator for division**
   at `tests/fixtures/pl011-full-spec/src/driver.rs:148`. The
   method contains integer division:
   ```rust
   let div_times_64 = (clk * 8 + baud) / (baud * 2);
   ```
   rustc inserts an `Assert (attempt to divide by zero)`
   terminator around the division. The compiler's construction
   pass rejects `Assert` terminators at
   `crates/regseq-driver/src/construction/pass2.rs:981`.
   Compiler error: "unsupported MIR terminator Assert".
   **Fix options**:
   (a) Rewrite the division to avoid rustc emitting the assert
       (difficult; rustc inserts asserts for all non-const
       integer division by default).
   (b) Compile the fixture with `-C overflow-checks=off
       -C debug-assertions=off`; the release-profile MIR path
       at `crates/regseq-driver/src/callbacks.rs` is supposed
       to set these per the Phase-3c `regseq-driver-25` task
       spec. Confirm this is still enabled and reaches
       `set_baud_rate`; if yes, the assert should not be in the
       MIR. If no, restore the release-profile semantics.
   (c) Teach pass2 to accept `Assert` terminators as safe
       no-ops when the guarded operation is known safe
       (compiler change, larger scope).
   Default assumption: (b) (release-profile MIR should suppress
   the assert; verify before pursuing (a) or (c)).

Fixes 1-3 are fixture-only edits totaling ~30-45 minutes of
work; fix 4 needs a 10-15 minute investigation to classify.
These fixes are the preconditions for meaningful
golden-regeneration work on the Stage-1 fixture and are the
primary scope of the planned `pl011-fixture-cleanup` follow-up
task (tracker item at
`.harness/task-spec-status.yaml:pl011-fixture-cleanup`;
brief at
`.harness/spec-batches/pl011-fixture-cleanup.md`).

### Stage 2 and Stage 3 implementation state

All Stage 2 and Stage 3 requirements are **not-yet-captured** in
the live fixture; each will be scheduled into its own stage's
kickoff audit. Summary:

- **Stage 2** (REQ-LIFE-03, REQ-FLOW-01, REQ-FLOW-02): fixture
  additions for CRTSCTS + throttle/unthrottle; QEMU patches 3.1,
  3.2, 3.4; Linux reference driver extensions
  (`set_termios` CRTSCTS branch, throttle/unthrottle, auto-RTS
  ordering).
- **Stage 3** (REQ-PERF-IFLS-01, REQ-PERF-DMA-01,
  REQ-PERF-DMA-02): fixture additions for runtime IFLS selection
  and DMA (DMACR + PIO-DMA threshold policy); QEMU patch 3.3;
  reference-driver DMA lowering (Linux quantitative; Zephyr as
  feature-delta observation only).

### Deferred (outside the 3-stage plan)

REQ-INT-CONSOLE-LINUX, REQ-INT-PM, REQ-INT-SYSFS-ADMIN, and all
REQ-DEF-*. See `pl011-spec-requirements.md` Sections 4.4 and 7
for rationales.

---

# Historical content (below)

This note tracks the build-out of the concrete full-PL011
fixture at:

- [pl011-full-spec](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec)

It is intentionally lightweight. Its purpose is to prevent drift between the
target fixture and the surrounding discussion docs.

The methodology note remains useful historical context for how the fixture was
grown, but this status file should reflect the current target fixture as it
exists now.

## Scope boundary

- This version targets a single concrete ARM PL011 configuration.
- Family-wide variant/revision/quirk machinery is intentionally deferred.
- PM-aware behavior that only appears in deferred PL011 variants is also out of
  scope for this version.

## Status labels

- `captured`: represented in the fixture and compile-checked through the real
  `regseq-driver` wrapper path
- `pending`: on the planned roadmap, but not yet added
- `deferred`: intentionally out of scope for this version

## Current captured frontend surface

### Base declarations

- `captured` PL011 register descriptors in
  [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
- `captured` `Pl011Registers` register block in
  [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
- `captured` driver identity for `Pl011FullSpec` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `Mmio<Pl011Registers>` field in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `current-speed` property with default `115200` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `fifo-size` property with default `32` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `Interrupt` resource in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

### Polling serial bring-up

- `captured` `Lifecycle` implementation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling `Serial` implementation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` helper sequencing for:
  - UART disable/enable
  - FIFO disable
  - baud-divisor programming
  - `LCR_H` commit after divisor programming
  - interrupt mask/clear hygiene
  - explicit ordered sequencing for the divisor/latch/enable sub-sequence
  in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling TX via `FR.TXFF` and `DR` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling RX emptiness check via `FR.RXFE` and `DR` read in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

### Interrupt-driven and richer bring-up participation

- `captured` RX-path error inspection and `RxChar` error propagation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` interrupt-driven service path via `#[rseq::isr]` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` interrupt mask / status / clear register participation through
  `IMSC`, `MIS`, and `ICR` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional clock participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional reset participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional pinctrl participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` richer serial hooks `flush_rx_fifo`, `flush_tx_fifo`, and
  `send_break` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Roadmap status

1. Base declarations
- `captured`

2. Polling serial bring-up
- `captured`

3. RX error handling
- `captured`

4. Interrupt-driven path
- `captured`

5. Clock participation
- `captured`

6. Reset participation
- `captured`

7. PinCtrl participation
- `captured`

8. PM-aware path
- `deferred`

9. Optional richer serial features
- `captured` in part
  - break control is captured
  - FIFO flush hooks are captured
  - flow control, modem-control/status, and `SerialConsole` remain outside the
    current target fixture

10. DMA
- `pending`

## Explicit deferrals

- `deferred` PM-aware path for variant-specific PL011 integrations
- `deferred` modem-control/status lines
- `deferred` RS-485
- `deferred` loopback
- `deferred` IrDA / SIR
- `deferred` family-wide variant/revision/quirk machinery

## Verification discipline

Each status change to `captured` should be backed by a successful compile check
of the fixture through the real compiler path, not just plain Cargo syntax
checking.
