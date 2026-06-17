# serial-uart-management — surface convergence (construct #7)

The compiler-side spec contract per the three-repo convention: the converged
DSL surface + the as-built record. The eval-side design analysis
(`~/regseq-eval/docs/constructs/serial-uart-management/` — adequacy, model,
DSL-surface, the bottom-up framework-pure re-derivation + purity certification)
cross-links here. Decision IDs (D##/C##/R##/I##/O#) refer to the unit's
GRILL/PRD/PLAN chain under
`.claude/sekekama/grill-to-plan/specs/serial-uart-management/` (GRILL
`2026-06-16-144606`, PRD `…-145727`, PLAN `…-150620`). The framework-pure
surface this realizes is `serial-uart-management-framework-pure-taxonomy.md`
(eval, PROPOSAL — not yet applied to the frozen taxonomy).

**Date:** 2026-06-17 · **Status:** ✅ **v1 CROSS-OS WITNESS.** One unchanged author
spec (`fixtures/_serial/serial-pl011/input.rs`) generates three byte-frozen
backend outputs, all freshly green in-repo: generic-c runs the host behavioral
trace; Linux + Zephyr are fresh snapshot+structure, with prior documented **live**
QEMU boot + interrupt-driven byte-IO evidence against the byte-identical goldens
(Linux amba-pl011, M2; Zephyr qemu_cortex_a53, M3) — the R05/D11 done-bar.
(Generic-c is a host trace oracle, NOT an OS boot; the OS boot witnesses are the
M2/M3 live runs.) Frontend enforcement (T04) Codex-converged DEFENSIBLE. Milestone
records in this folder: `M1-architecture.md`, `M2-linux-serial.md`,
`M3-zephyr-serial.md`, `T09-tx-drain-query-decision.md`,
`T04-frontend-enforcement.md`.

---

## The converged surface (v1 in-scope subset; framework-pure 6-trait surface)

The author writes a `#[rseq::driver]` struct tagged `#[rseq::device_class(serial)]`
and implements the in-scope subset of the 6-trait framework-pure surface (GRILL
D02/D03). The construct's **confirmed contribution** is the runtime per-source
interrupt mask: `irq_source_set` is **backend-called** — the compiler emits its
calls with `(SerialIrqSource, IrqSourceState)` enum literals at each per-OS site
(WHEN), the author owns only the mask-register bits in the match arms (WHICH
BITS, I01).

- **L1 (the author surface):**
  - base **`Serial`** (C04, REQUIRED) — `port_service_set` / `data_ready_query` /
    `data_unit_write` / `data_unit_read(&mut) -> RxDatum` (inline per-unit errors,
    I04).
  - **`SerialRuntimeLineConfig`** (C03) — v1 builds ONLY `line_format_apply`
    (procedural baud→divisor + register encode; commits, does NOT read back, I03;
    returns `ConfigResult::Unsupported`, never silently defaulted). The siblings
    `line_format_readback`/`break_set`/`flow_auto_set` are DEFERRED (T04-rejected).
  - **`SerialInterruptDriven`** (C01) — `irq_source_set(source, state)` (backend-
    called) + `irq_cause_decode() -> CauseSet` (decode + W1C-ack of line-error
    causes inline; no separate ack op). `CauseSet` = the pinned O2 bitset over
    `{tx_ready, rx_data, rx_timeout, line_error, modem_delta}`, `modem_delta`
    present-but-unarmed (routes to `SerialModemControl` later).
  - **`SerialQueueControl`** (C05) — v1 builds ONLY `tx_drain_query`
    (shift-register-inclusive TX-drain completion; PL011 reads FR.BUSY; T09
    INCLUDE — see `T09-…`). The sibling `fifo_purge_hw` is DEFERRED (T04-rejected).
  - The line is a compiler-managed `#[rseq::interrupt(id="…")]` `Interrupt` field
    (construct #6); there is NO authored `#[rseq::isr]` — the serial ISR is
    compiler-synthesized (B3).
- **L2 (platform binding):** `[mmio.regs]` (PL011 register window) + the interrupt
  binding (`platform.{linux,zephyr}.toml`). Zephyr is DT-authoritative for the IRQ
  (advisory L2; F-Z4).
- **Frontend enforcement (T04, the reject matrix):** dynamic author reference to
  `irq_source_set` → `SER-ERR-DYNAMIC-IRQ-SET` (R07); override of a deferred member
  → `SER-ERR-DEFERRED-MEMBER` (R10); impl of a deferred companion trait
  (`SerialModemControl`/`SerialConsole`/`SerialBulkTransfer`) →
  `SER-ERR-DEFERRED-TRAIT` (R10).
- **DEFERRED (out of v1, recorded in the GRILL contracts C03/C05/C06/C07/C08 for the
  amendment record):** `line_format_readback`, `break_set`, `flow_auto_set`,
  `fifo_purge_hw`, modem control, console, bulk/DMA (signature un-frozen, D07).

## IR spine (`regseq-model/src/serial.rs`)

`SerialTables.drivers: BTreeMap<ItemId, DriverSerial>` (sibling on `RegseqModel`,
`#[serde(default)]` — pre-construct-#7 models still deserialize). `DriverSerial`
= `{ base: SerialBaseOps, line_config: Option, interrupt_driven: Option,
queue_control: Option, types: SerialTypeBindings }`; op bodies are
content-addressed `InstanceRef` (survive the F4 instance sort, like
`device_lifecycle`/`interrupt`). The pinned `CauseSetRepr` (O2) is a u32 bitset,
bit position = `SerialCause` discriminant, `modem_delta` armable=false; its
data-path twin `RxErrorRepr` on `SerialBaseOps` pins the inline `RxError`
vocabulary (`SerialRxErrorKind {Framing=0,Parity=1,HwOverrun=2,SwOverrun=3,
Break=4}`) so the backends DECODE the inline errors and map them to OS-native
reporting (Linux tty flags; Zephyr `uart_err_check`). Both are **default-pinned
conventions** — the author `CAUSE_*`/`RX_ERR_*` consts must match those bit
positions — and the bridge **validates** them: a present `u32` convention const
whose value ≠ `1 << discriminant` is rejected (`SER-ERR-REPR-CONST`, the O2
hardening). This NARROWS the silent-misclassification hole to the named-const
convention; it does not close it (an author who builds the bitset with raw
literals or differently-named consts in the body bypasses it — the same
guard-rail boundary as the T04 R07 check; bodies are not analyzed). The backends
read IR, never the AST (C-2). Enum literals are minted from `SerialTypeBindings`
variant-index maps captured + closed-vocab-validated by the bridge.

## The per-OS scatter contract (`regseq-backends/src/serial_scatter.rs`)

The ONE queryable source for the per-OS WHEN of each `irq_source_set(source,
state)` intent (R04/I01; the enum-form re-cut of Track-1's named-hook tables,
D03). The author owns WHICH BITS; the compiler owns WHEN:

| backend | scatter sites |
|---|---|
| generic-c | choreography phases: `Startup` arms RxData+LineError; `TxKick` arms TxReady; ISR `IsrTxDrained` disarms TxReady; `Teardown` disarms RxData+LineError (the placement oracle, T07 golden) |
| linux_c | `uart_ops`: `startup` arms RxData+LineError; `start_tx`/`stop_tx` arm/disarm TxReady; `stop_rx` disarms RxData; `shutdown` masks all |
| zephyr_c | `uart_driver_api`: `irq_{tx,rx,err}_{enable,disable}` slot bodies (one intent each) |

`rx_timeout` couples into `RxData` (Zephyr Z11) — no separate source.

## As-built (all milestones; compiler branch `construct/serial-uart-management`)

- **M1 — generic_c (T01–T07).** The traced bring-up→arm→TX→fire→RX→teardown
  harness + the synthesized ISR; the golden trace IS the placement-contract oracle
  (R09/S05): each `SerialIrqSource` mask write appears at its scattered WHEN as a
  `regseq_mmio_write` DISTINCT from the line `regseq_irq_enable` (per-source arming
  ≠ line enable). `line_format_apply → Unsupported` surfaced (9-bit). Compiles +
  runs on host `cc` (`serial_pl011_e2e`, behavioral). Record: `M1-architecture.md`.
- **M1.5 — `tx_drain_query` (T09): INCLUDE.** The sole member of an otherwise-
  deferred `SerialQueueControl` (FR.BUSY, not derivable from the base
  `data_ready_query(TxSpace)` = FR.TXFF; Linux dereferences `uart_ops.tx_empty`
  unconditionally). Record: `T09-tx-drain-query-decision.md`.
- **M2 — linux_c (T08).** `uart_driver` + `uart_port` + `uart_ops` vtable + the
  synthesized serial ISR (decode→drain-to-tty / kfifo-TX-refill) + the Linux WHEN
  scatter (conformance lists the 8 arming sites: startup RxData/LineError,
  start_tx/stop_tx TxReady, stop_rx RxData, shutdown all). `set_termios →
  line_format_apply` (§3.3.8 disable→format→enable; Unsupported surfaced).
  `tx_empty ← tx_drain_query`. **Live R05 on amba-pl011 QEMU** (boots, binds the
  reserved 2nd PL011, interrupt-driven TX/RX byte-for-byte, per-source IMSC scatter
  observable on silicon). 6 bugs the live run + post-QEMU Codex caught (TX
  arm-without-pump deadlock; dev_name collision; x_char/tx_stopped flow-control).
  Record: `M2-linux-serial.md`.
- **M3 — zephyr_c (T10/T11).** Keeps the SAME lifecycle/IRQ_CONNECT/deinit scaffold
  and ADDS the `uart_driver_api` plane (slots + synthesized callback-dispatch ISR +
  the delivered `&<drv>_uart_api`). Conformance lists the Zephyr slots + the 6
  per-source scatter sites (`irq_{tx,rx,err}_{enable,disable}`).
  `configure ← line_format_apply` (rejects Zephyr's richer-than-surface
  vocab — 9-bit/MARK-SPACE/0.5-1.5-stop/flow — with `-ENOTSUP`); init does NOT
  enable the port (no invented baud); `irq_tx_complete ← tx_drain_query`. The
  arm-gate (`tx_irq_on`/`rx_irq_on`) is the IMSC.TXIM/RXIM analog that keeps the
  canonical `while(irq_update() && irq_is_pending())` loop terminating.
  `CONFIG_UART_INTERRUPT_DRIVEN` (T11). **Live boot + byte-IO on qemu_cortex_a53**
  (option B); the live run found + fixed the TX-prime double-send (the prime runs
  under `irq_lock`). Record: `M3-zephyr-serial.md`.
- **T04 — frontend enforcement.** The 3-gate reject matrix (R07 + R10) at the
  acquire stage; 10-fixture negative corpus; Codex-converged DEFENSIBLE (the R07
  scan is a documented guard-rail — operand-level + closure-BFS catch
  call/fn-ptr/closure; the boundary is const-promotion + direct-mask-replication,
  inherently undetectable). Record: `T04-frontend-enforcement.md`.
- **RxError surfacing (F1/F-Z3 closure, 2026-06-17).** `RxErrorRepr` +
  `SerialRxErrorKind` pinned on `SerialBaseOps` (the data-path O2-analog).
  linux_c maps the inline error to the tty flag + `icount`; zephyr_c accumulates
  it (atomic) across `poll_in`/`fifo_read` and reports it via a new always-present
  `err_check` slot (mapped to `UART_ERROR_*`/`UART_BREAK`, atomic read-and-clear —
  the race the Codex review caught). Generic_c golden unchanged (the oracle does
  not consume it); linux/zephyr goldens re-blessed. Codex-converged
  (UNSOUND race → atomic fix → SOUND).

## Cross-OS witness (M4 / T12) — the v1 done-bar

The SAME `fixtures/_serial/serial-pl011/input.rs` (one author spec) drives all
three backends. Two evidence tiers — distinguished honestly: **in-repo** =
freshly re-verified each test run; **live** = the documented M2/M3 QEMU runs
(prior sessions). The `input.rs` is unchanged; the generic_c golden is
byte-identical to the M1 oracle. The Linux + Zephyr goldens have since changed:
T04 is reject-path-only (cannot alter this emission — byte-frozen-verified), but
the **RxError surfacing** (F1/F-Z3, 2026-06-17) DID re-bless
`golden.{linux,zephyr}.c`. For CLEAN byte payloads that change is behaviorally
inert beyond an added Linux `icount.rx++` and a Zephyr `err_check` that returns 0,
so the M2/M3 live CLEAN-PAYLOAD evidence still holds; the new error-surfacing code
+ the new counters/`err_check` API are snapshot+Codex-validated ONLY (no live
error-injection campaign).

| backend | artifact | in-repo (fresh) | live (documented, M2/M3) |
|---|---|---|---|
| generic_c | `golden.c` (+ `expected.stdout`) | compiles + RUNS the host behavioral trace (`serial_pl011_e2e`) — the placement oracle, NOT an OS boot | n/a (host oracle) |
| linux_c | `golden.linux.c` + `conformance.linux.json` | byte-exact snapshot + M2 structure (`linux_serial_pl011_e2e`) | amba-pl011 QEMU boot + interrupt-driven byte-IO (M2; result dir under `~/regseq-eval/pl011/linux/results/`) |
| zephyr_c | `golden.zephyr.c` + `…binding.yaml` + `conformance.zephyr.json` | byte-exact snapshot + M3 structure (`zephyr_serial_pl011_e2e`) | qemu_cortex_a53 boot + interrupt-driven byte-IO (M3; result dirs `…/zephyr/results/campaigns/2026-06-17-serial-m3{,-byteio}`) |

**Strongest supported claim:** one unchanged `input.rs` generates three byte-frozen
backend emissions that are freshly green in-repo; generic-c is freshly run
behaviorally; Linux + Zephyr carry prior documented live QEMU boot +
interrupt-driven byte-IO evidence (clean payload) against goldens byte-identical to
those live runs EXCEPT the snapshot+Codex-validated RxError error-surfacing
additions (F1/F-Z3) = **v1 done (R05/D11)**. A1 (`{TxReady, RxData, LineError}`)
and A2 (buildable subset) held across all three OSes — no cross-OS divergence
surfaced.

## Amendments (binding, discovered during implementation)

1. **The synthesized OS `start_tx`/`irq_tx_enable` is arm + PRIME, not arm-only**
   (M2 Linux live; M3 Zephyr live). A level PL011 TX IRQ does not self-fire for an
   already-empty FIFO, so arming alone transmits 0 bytes. The compiler synthesizes
   a prime (a synchronous data-path kick). On Zephyr the prime runs under
   `irq_lock` (the armed TX IRQ must not preempt it — the double-send fix). This is
   a **construct-level finding** the generic-c oracle could not surface (its harness
   does the byte write). Single-core target; general SMP would need a per-device
   spinlock (F-Z5).
2. **`tx_drain_query` is REQUIRED where the drain slot is emitted** (T09): Linux
   `uart_ops.tx_empty` is dereferenced with no NULL guard; a behaviorally-lying
   constant stub is incompatible with faithful framework-pure lowering. No
   synthesized fallback (the FR.BUSY fact is not derivable from the base ops).
3. **Zephyr `configure` REJECTS unrepresentable configs with `-ENOTSUP`** rather
   than coercing — Zephyr's `uart_config` vocab is richer than the framework-pure
   surface (unlike Linux termios, which matches). 9-bit routes through
   `line_format_apply`'s proven `Unsupported` path.
4. **T04 R07 is a guard-rail, not a sandbox** (Codex T04 review): an author owns the
   mask bits and can replicate the IMSC write directly in a base op (undetectable),
   so an `irq_source_set`-reference scan cannot be airtight. v1 catches the
   obvious/idiomatic misuse (call / fn-pointer / closure) and documents the boundary.

## Taxonomy feedback notes (recorded, NEVER applied to the frozen pages)

1. **The runtime per-source interrupt mask (`irq_source_set`, P5/C01)** is the one
   surface the frozen taxonomy lacks; it is independently the most-forced generator
   in every blind re-derivation pass (eval framework-pure doc §1). The
   `irq_cause_decode_ack → irq_cause_decode` rename + the four L2 demotions
   (`tx_ready_trigger`, `irq_ack_semantics`, `fifo_trigger_levels`, numeric
   `fifo_depth`) are the other framework-purity corrections. Feed back when the
   taxonomy thaws (the eval `…-framework-pure-taxonomy.md` is the surface artifact;
   the GRILL is the decision record).
2. **6 concern-coherent traits, not the flat 10-op `Serial`** (D02): the frameworks
   partition the capability surface (Zephyr Kconfig is the clearest witness). v1
   builds the in-scope subset; the rest is the recorded amendment.

## Phase limitations / recorded follow-ups (non-blocking)

- **F1 / F-Z3 — `RxError` surfacing. ✅ RESOLVED (2026-06-17).** The inline
  per-unit RX error is now pinned in the IR (`RxErrorRepr`/`SerialRxErrorKind`,
  the data-path O2-analog) and surfaced: Linux maps it to the tty flag
  (`TTY_BREAK`/`TTY_PARITY`/`TTY_FRAME`, overrun as a separate char) + the
  `icount` (rx/brk/parity/frame/overrun/buf_overrun); Zephyr accumulates it across
  `poll_in`/`fifo_read` and reports it via a new always-present `err_check` slot
  (mapped to `UART_ERROR_*`/`UART_BREAK`, atomic read-and-clear). Codex-converged
  (UNSOUND race → `atomic_t` fix → SOUND). **Boundary:** the error path is
  snapshot+Codex-validated, NOT live-error-injected (see the cross-OS witness
  note).
- **O2 hardening (2026-06-17) — repr-const validation.** The bridge validates the
  author `CAUSE_*`/`RX_ERR_*` bit-position consts against the pinned
  `CauseSetRepr`/`RxErrorRepr` (`SER-ERR-REPR-CONST`): a present `u32` convention
  const at the wrong bit is rejected at the acquire stage (2 reject fixtures).
  Closes the "pinned convention, not validated" gap for the named-const surface.
  Codex-converged SOUND-WITH-NITS (u32-type gate added).
- **O2 body-provenance (2026-06-17) — the full body analysis.** Going beyond the
  named-const check, a bridge MIR pass verifies the `CauseSet`/`RxError` value
  built in `irq_cause_decode`/`data_unit_read` is constructed ONLY from
  convention-domain CONSTANT bits (`SER-ERR-BITSET-PROVENANCE`). A place-type-keyed
  WHITELIST scan of every repr-producing site (from_bits / empty / struct literal)
  + a borrow-aware backward const-bit slice + prefix-aware interior-write rejection
  + repr-alias rejection. Closes the raw-literal / differently-named-const /
  device-bit-leak / helper-delegation / struct-literal / aliasing / interior-
  mutation bypasses (8 reject fixtures). **Codex-converged over 4 adversarial
  rounds → SOUND-WITHIN-DSL** (every residual evasion needs a bridge-rejected
  construct: transmute/union/FFI/raw-ptr). **Irreducible residual:** the
  device→semantic mapping (which device bit means rx_data) is author knowledge no
  static check can recover — this guarantees the OUTPUT bitset carries only
  convention-domain bits, not that each bit was set for the right device reason.
- **F2 — `uart_port.uartclk`** is a placeholder (QEMU virt 24 MHz); it should bind
  from a future clock-management construct (the author `line_format_apply` owns the
  divisor math).
- **F-Z1 — Zephyr `config_get` before the first `configure()` returns zeros** (no
  invented default baud; the app configures before IO). Codex-accepted v1 contract.
- **F-Z4 — the L2 interrupt binding is advisory on Zephyr** (DT-authoritative; the
  live DUT is GIC SPI 8 = VIRT_SECURE_UART).
- **F-Z5 — SMP serialization.** `irq_lock` masks only local interrupts; a multi-core
  build would need a per-device spinlock around the callback dispatch. Out of scope
  for the non-SMP qemu_cortex_a53 campaign.
- **Linux `port->icount.rx` not incremented** in the ISR RX drain (TIOCGICOUNT rx=0;
  data flow unaffected).

## Test commands

```
# in-repo cross-OS witness (one spec → three backends):
cargo test -p codegen-coverage-runner --test serial_pl011_e2e \
  --test linux_serial_pl011_e2e --test zephyr_serial_pl011_e2e
# frontend enforcement (T04 reject matrix):
cargo test -p codegen-coverage-runner --test serial_reject_e2e
# spine + recognition + shared-emitter regression:
cargo test -p regseq-model --lib serial                       # 26
cargo test -p regseq-driver --test bridge_serial_recognition  # 5
cargo test -p regseq-backends --lib                           # 411
```
