# serial-uart-management — M1 architecture (generic_c end-to-end slice)

> **PROGRESS CHECKPOINT (autonomous build).** Compiler work is in the worktree
> `~/Documents/sekekama/regseq-compiler-worktrees/serial-uart` (branch
> `construct/serial-uart-management`, off `main`). NOT committed (await Gedeon).
> - **T01 DONE** — `crates/regseq-model/src/serial.rs` (`SerialTables` sibling
>   table; `SerialTypeBindings` = variant-index maps only; `CauseSetRepr` O2 pin);
>   wired into `model.rs`/`lib.rs`; 7 unit tests pass.
> - **T02 DONE** — single-file M1 fixture
>   `crates/codegen-coverage-runner/fixtures/_serial/serial-pl011/input.rs` +
>   shared surfaces `fixtures/_serial_surface.rs` (traits/enums) +
>   `fixtures/_serial_pl011_surface.rs` (register block). Multi-file readable
>   donor at `tests/fixtures/serial-pl011/` (parallel to pl011-full-spec; NOT
>   bridged). **NOTE:** bridge can't const-translate single-field newtype structs
>   → `RxError`/`CauseSet` use `from_bits` + plain `u32` bit consts (flag to Codex).
> - **T03 DONE** — `crates/regseq-driver/src/bridge/serial.rs` (`resolve_serial`,
>   name-anchored trait recognition + variant-map capture); wired into
>   `bridge/mod.rs` (`SerialDiagnostic`, Phase-5 call) + B3 relaxation in
>   `bridge/interrupt/resolve.rs` (serial-class skips MISSING-HANDLER). Test
>   `tests/bridge_serial_recognition.rs` (5 tests) pass; lifecycle/interrupt
>   recognition green (no regressions).
> - **T05 DONE** — `crates/regseq-backends/src/serial_scatter.rs` (generic/linux/
>   zephyr WHEN tables; 6 tests). Wired into backends `lib.rs`. Also fixed a
>   `bridge-coverage-runner/src/cluster.rs` exhaustiveness match for the new
>   `BridgeError::SerialValidation`.
> - **T06 DONE** — `emit_serial_synthesis` in `crates/regseq-backends/src/generic_c/
>   body_emit.rs` (self-contained serial choreography helper + synthesized ISR;
>   serial-class drivers skip the lifecycle/interrupt scenario loop). Renders
>   enum/struct literal call args via `const_expr`. Builds clean.
> - **T07 DONE** — `crates/codegen-coverage-runner/tests/serial_pl011_e2e.rs` +
>   blessed `golden.c`/`expected.stdout`. The serial fixture passes the FULL
>   6-stage oracle (bridge→L2→generic_c→snapshot→cc→run→behavioral) + placement
>   assertions: bring-up order (line-format while disabled), per-source IMSC arm
>   DISTINCT from the line `regseq_irq_enable`, the synthesized ISR decoding +
>   dispatching (TX→disarm, RX/RX-timeout→read), teardown order. No regressions
>   (411 backend lib tests + irq/lifecycle pl011 e2e green).
> - **M1 CODEX REVIEW DONE** — SOUND-WITH-CHANGES. Folded **Finding 1** (the
>   synthesized ISR now services RxData|RxTimeout — bit mask `6u` — and the trace
>   exercises an RTMIS fire). Non-blocking notes carried to M2: **F2** the M1 TX
>   byte is written in the choreography + interrupt-armed/decoded/disarmed (not
>   ISR-refilled) — tighten claim wording; **F3** `line_format_apply`'s
>   `ConfigResult::Unsupported` + `data_ready_query`'s bool are ignored by the M1
>   oracle (always-Ok 8N1) — must surface at real-OS lowering (T08). Const
>   workaround accepted (no runtime cost; weaker type validation only).
>
> ### ✅ M1 COMPLETE (generic_c). ✅ M1.5 T09 RESOLVED = INCLUDE tx_drain_query
> (see `T09-tx-drain-query-decision.md`). ✅ T13 preflight (Linux QEMU + aarch64-gcc
> present; Zephyr SDK ABSENT). ✅ M2 (Linux T08) COMPLETE — Codex loop SOUND/M2 PASS
> AND **R05 boot+IO VALIDATED ON LIVE amba-pl011 QEMU** (interrupt-driven TX+RX both
> match; the live run caught 4 bugs snapshot+Codex missed: unlock-flags ABI, dev_name
> collision, remove-Oops, the TX-kick deadlock — all fixed; see `M2-linux-serial.md`).
> Remaining: T04 (enforcement) → M3 (Zephyr T10/T11 — BLOCKED on absent SDK) → M4
> (cross-OS T12) → spec contract.
>
> ### T06 generic_c integration map (precise insertion points)
> The generic_c serial harness is synthesized in `crates/regseq-backends/src/
> generic_c/body_emit.rs` `emit_lifecycle_synthesis(model, names, syms, options)`
> (≈L5399), which already iterates `model.device_lifecycle.drivers` and emits per-
> scenario helpers + the `__rseq_lc_run` runner (`LIFECYCLE_RUNNER_SYMBOL`, L4982).
> Add a serial branch keyed on `driver.descriptor.device_class == ClassTag::Serial`
> **and** `model.serial.drivers` having that driver — emit a serial choreography
> helper + a synthesized serial ISR wrapper (mirror `prepare_irq_emission`
> ≈L5194 which emits `static void rseq_isr_<label>(void *ctx){ struct T*__t=ctx;
> ...=sym(&__t->inner); regseq_irq_result(...);}` + registers via
> `regseq_irq_register(line, wrapper, ctx)` and fires via `regseq_irq_fire`).
> - **Call args (the crux):** `const_expr(model, names, syms, &Const, Some(param_ty),
>   false)` (free fn, L3119) renders a constant operand. Fieldless enum literals =
>   `Const::EnumValue{variant_idx, payload: vec![]}` (NO interning needed) with
>   `consumer_ty = the op's FnSig param TyId`. `LineFormat` = hand-render
>   `(struct <c_type>){.baud=115200u,.data_bits=<const_expr(DataBits::Eight)>,...}`
>   (fields are scalars+fieldless enums → no interning). variant indices come from
>   `driver.types.{port_scope,ready_dir,irq_source,irq_state,config_result,data_bits,
>   parity,stop_bits}.variants[<kind ordinal>]`.
> - **Op symbol:** `resolve_reloc(model, iref)` → InstanceId → `syms.symbol(id)`.
>   Param TyIds: `model.instances[id].kind` Function → `fn_sig` → `model.sigs[..]`.
> - **Scatter:** `serial_scatter::generic_scatter(GenericSerialPhase::{Startup,TxKick,
>   IsrTxDrained,Teardown})` gives the `(source,state)` intents → emit
>   `irq_source_set(&inst.inner, <source literal>, <state literal>)` per intent.
>   Each must trace as a `regseq_mmio_write` (IMSC) DISTINCT from the line
>   `regseq_irq_enable` (Codex B-distinction).
> - **Manifest:** `crates/codegen-coverage-runner/src/manifest.rs` `Manifest` struct
>   (add `serial_scenario`/`serial_poke`); `BackendOptions` in `generic_c/
>   backend_options.rs` (parse like `irq_fire`/`IrqFireStep`). Discovery requires
>   `manifest.toml`; runner `--bless` writes `golden.c` only; `expected.stdout`
>   hand-authored. The fixture is `fixtures/_serial/serial-pl011/` (input.rs +
>   platform.linux.toml exist; needs manifest.toml + golden.c + expected.stdout).



**Status:** design, pre-build. Foundation note for the v1 implementation of the
framework-pure serial device class (GRILL/PRD/PLAN under
`regseq-eval/.claude/sekekama/grill-to-plan/specs/serial-uart-management/`).
This note covers the M1 slice (T01–T07): IR survival set → bridge recognition →
placement-scatter → generic_c lowering → golden placement trace. Linux/Zephyr
(M2/M3) reuse the IR + scatter and add per-OS emission.

## 0. The shift from the existing fixture

The existing `tests/fixtures/pl011-full-spec` and
`docs/discussions/device-class-serial/serial-device-class-spec.md` (2026-04-02)
use a **flat 10-method `Serial`** trait where interrupt arming is **baked into
authored bodies** (`bring_up_port` calls `enable_rx_interrupts()`; the authored
`#[rseq::isr] handle_irq` calls `disable_tx_interrupts()`). The GRILL (2026-06-16,
D02/D03) **supersedes** that with the framework-pure 6-trait surface:

- **base `Serial`** (C04): `port_service_set(scope,on)->prev`, `data_ready_query(dir)->bool`,
  `data_unit_write(unit)`, `data_unit_read()->RxDatum{unit,errors}`.
- **`SerialRuntimeLineConfig`** (C03): v1 builds only `line_format_apply(LineFormat)->ConfigResult`.
- **`SerialInterruptDriven`** (C01): `irq_source_set(source:SerialIrqSource,state:IrqSourceState)`,
  `irq_cause_decode()->CauseSet`.

The construct's confirmed contribution: **the runtime per-source interrupt mask
is hoisted OUT of authored bodies into `irq_source_set`, which the COMPILER calls
at per-OS sites (WHEN), while the author owns only the mask-register bit per source
(WHICH BITS)** (D03/I01). The existing IMSC-shadow helpers map directly:
`irq_source_set(RxData,Armed)`≈`enable_rx_interrupts`, `(TxReady,Armed)`≈`enable_tx_interrupts`,
`(TxReady,Disarmed)`≈`disable_tx_interrupts`, `(LineError,Armed)`≈`enable_error_interrupts`.

## 1. New fixture (T02): `tests/fixtures/serial-pl011`

A NEW fixture (leave `pl011-full-spec` untouched — used by other tests + a parallel
construct). Re-cut from `pl011-full-spec`: **reuse** the register block
(`registers.rs`), baud→divisor math (`set_baud_rate`), line-control encode
(`line_control_value`), MMIO Form-B facade, and the IMSC-shadow constants. **Restructure**
into the 6-trait surface; **drop** deferred members (break/flow/readback/purge/modem/console)
and the authored `#[rseq::isr] handle_irq`.

Author surface (proposed):

```rust
#[rseq::framework_type("serial-irq-source")] enum SerialIrqSource { TxReady, RxData, LineError }
#[rseq::framework_type("serial-irq-state")]  enum IrqSourceState  { Armed, Disarmed }
#[rseq::framework_type("serial-cause-set")]  struct CauseSet { bits: u32 }  // bitflags-style
// supporting: PortScope{Block,Tx,Rx}, ReadyDir{TxSpace,RxData}, RxDatum, RxError, LineFormat, ConfigResult

impl Serial for Pl011 {
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState { /* CR UARTEN/TXE/RXE */ }
    fn data_ready_query(&self, dir: ReadyDir) -> bool { /* FR.TXFF / FR.RXFE */ }
    fn data_unit_write(&mut self, unit: u32) { /* DR write */ }
    fn data_unit_read(&mut self) -> RxDatum { /* DR read + inline DR error bits */ }
}
impl SerialRuntimeLineConfig for Pl011 {
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult { /* baud+lcr_h, ret Unsupported on 9-bit/flow */ }
}
impl SerialInterruptDriven for Pl011 {
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState) {
        match (source, state) {
            (SerialIrqSource::RxData, IrqSourceState::Armed)    => { self.imsc_shadow |= RX_MASK; self.commit(); }
            (SerialIrqSource::TxReady, IrqSourceState::Armed)   => { self.imsc_shadow |= TX_MASK; self.commit(); }
            (SerialIrqSource::TxReady, IrqSourceState::Disarmed)=> { self.imsc_shadow &= !TX_MASK; self.commit(); }
            (SerialIrqSource::LineError, IrqSourceState::Armed) => { self.imsc_shadow |= ERR_MASK; self.commit(); }
            ... /* the author owns these bits */
        }
    }
    fn irq_cause_decode(&mut self) -> CauseSet { /* read MIS -> CauseSet bits */ }
}
```

The driver still uses `#[rseq::device_class(serial)]`, `#[rseq::mmio]`,
`#[rseq::interrupt(id=...)]` (the interrupt LINE, construct #6 — still needed),
`Lifecycle`, clock/reset/pinctrl. The interrupt *line* register/enable/disable
stays construct #6; the per-*source* mask is construct #7 (this).

## 2. Recognition (T03) — bridge

Mirror the name-anchored trait recognition (Lifecycle/Mmio/Interrupt) + the
`framework_type` enum recognition (IrqResult):

- Recognize `impl Serial / SerialRuntimeLineConfig / SerialInterruptDriven for <driver>`
  by **trait name**; collect each method body as a content-addressed `InstanceRef`
  (force-materialize, like `LifecycleOps`/`IsrHandler.body`).
- Recognize `SerialIrqSource`/`IrqSourceState`/`CauseSet` via `#[rseq::framework_type(...)]`
  (new closed-vocabulary names), record onto `domain_attrs.item_attrs` + capture the
  enum `ItemId`s (the backend needs them to mint enum literals at scatter sites).
- Gate on `ClassTag::Serial` **+ presence of `impl Serial`** — replace the
  deferred/NULL-api treatment **additively** (non-serial deferred classes keep the fallback).

## 3. IR survival set (T01) — `crates/regseq-model/src/serial.rs`

New sibling table on `RegseqModel` (`#[serde(default)]`), mirroring `interrupt.rs`:

```rust
pub struct SerialTables { pub drivers: BTreeMap<ItemId, DriverSerial> }
pub struct DriverSerial {
    pub base: SerialBaseOps,                       // C04, all four required (diagnostic if missing)
    pub line_config: Option<SerialLineConfigOps>,  // present iff impl SerialRuntimeLineConfig
    pub interrupt_driven: Option<SerialInterruptOps>, // present iff impl SerialInterruptDriven
}
pub struct SerialBaseOps { port_service_set, data_ready_query, data_unit_write, data_unit_read: Option<InstanceRef> }
pub struct SerialLineConfigOps { line_format_apply: Option<InstanceRef> }
pub struct SerialInterruptOps {
    irq_source_set:   Option<InstanceRef>,
    irq_cause_decode: Option<InstanceRef>,
    source_enum: Option<ItemId>,   // SerialIrqSource (variant->literal mapping for scatter)
    state_enum:  Option<ItemId>,   // IrqSourceState
    cause_set:   CauseSetRepr,      // O2 pin
}
```

**O2 — `CauseSet` repr (pinned here):** a `u32` bitset over the ordered cause set
`{tx_ready=bit0, rx_data=bit1, rx_timeout=bit2, line_error=bit3, modem_delta=bit4}`.
cause↔source: `tx_ready↔TxReady`, `{rx_data,rx_timeout}↔RxData`, `line_error↔LineError`,
`modem_delta` **present-but-unarmed** in v1 (no source; routes to SerialModemControl later).

```rust
pub struct CauseSetRepr { pub width: UintKind /*U32*/, pub causes: Vec<SerialCause> }
pub enum SerialCause { TxReady, RxData, RxTimeout, LineError, ModemDelta }   // bit position = decl order
```

serde round-trips like sibling nodes; IR snapshot test + model unit tests.

## 4. Placement-scatter engine (T05) — `crates/regseq-backends/src/serial_scatter.rs`

Beside `interrupt_order.rs`/`capability.rs`. Holds the per-OS WHEN table keyed on
the `(source,state)` intent. A backend-agnostic table:

```rust
pub enum ScatterSite {            // logical per-OS call sites
    LinuxStartTx, LinuxStopTx, LinuxStartup, LinuxShutdown, LinuxStopRx,
    ZephyrIrqTxEnable, ZephyrIrqTxDisable, ZephyrIrqRxEnable, ZephyrIrqRxDisable, ZephyrIrqErrEnable, ZephyrIrqErrDisable,
    GenericBringup, GenericTeardown, GenericTxKick, ... }
pub fn linux_when(site)  -> Vec<(SerialIrqSource, IrqSourceState)>
pub fn zephyr_when(site) -> Vec<(SerialIrqSource, IrqSourceState)>
pub fn generic_schedule() -> Vec<(Phase,(SerialIrqSource,IrqSourceState))>
```

WHEN contract (R04): Linux `start_tx`→`(TxReady,Armed)`, `stop_tx`→`(TxReady,Disarmed)`,
`startup`→arm `(RxData,Armed)`+`(LineError,Armed)`, `shutdown`/`stop_rx`→mask all;
Zephyr `irq_tx_enable`→`(TxReady,Armed)` etc; generic-c → traced placement.
**Each scatter = the backend emits a CALL to the materialized `irq_source_set`
function with the SOURCE/STATE enum literals.** The author's match-arm body runs →
`regseq_mmio_write(IMSC,...)` → that mask write traces **distinct** from the
construct-#6 line-level `regseq_irq_enable`.

## 5. generic_c lowering (T06) + golden (T07)

generic_c synthesizes the traced harness (one byte each way), reusing the
construct-#6 `regseq_irq_*` (synchronous-fire D07) + `regseq_mmio_*` prelude:

```
port_service_set(Block,true)  [enable UART]
line_format_apply(115200,8N1) -> ConfigResult::Ok   (Unsupported surfaced, not defaulted)
irq_source_set(RxData,Armed); irq_source_set(LineError,Armed)   [startup scatter]
-- TX one byte --
irq_source_set(TxReady,Armed)            [tx-kick scatter]
data_ready_query(TxSpace)->true; data_unit_write(b)
poke TX cause; regseq_irq_fire(line) -> SYNTH ISR: irq_cause_decode -> tx_ready -> irq_source_set(TxReady,Disarmed)
-- RX one byte --
poke RX cause; regseq_irq_fire(line) -> SYNTH ISR: irq_cause_decode -> rx_data -> data_unit_read drain
teardown: irq_source_set(RxData,Disarmed)+(LineError,Disarmed); port_service_set(Block,false)
```

**The synthesized ISR** (compiler-owned) = `irq_cause_decode()` → for each decoded
cause, the data-path response (`rx_data`→`data_unit_read` loop; `tx_ready`→
`irq_source_set(TxReady,Disarmed)` + (real backends) refill). This replaces the
authored `handle_irq`. Golden trace (T07) asserts the bring-up→arm→IO→teardown
interleaving incl. the per-source-mask-vs-line-enable distinction; M1 exit gate.

## 6. Open design forks for review

- **F1 — Synthesized-ISR scope.** Is the compiler-synthesized ISR (decode→service→
  rearm) the right division, with the author providing ONLY `irq_cause_decode`
  (bits→CauseSet) + the data-path primitives? Or should the author retain more of
  the service logic? (GRILL says author=WHICH BITS + decode; compiler=WHEN +
  choreography. Confirm the synthesized-ISR read is faithful and tractable for M1.)
- **F2 — `port_service_set` vs bring-up.** The existing `port_open` does
  pinctrl→clock→`bring_up_port`(baud+lcr+enable+arm-rx). New `port_service_set(scope,on)`
  is just CR enable/disable; `line_format_apply` does baud+lcr; arming is delegated
  to scatter; pinctrl/clock stay in `Lifecycle`/resources. Is the decomposition
  (Lifecycle owns pinctrl/clock/reset; `port_service_set` owns CR; `line_format_apply`
  owns baud/lcr; scatter owns arming) correct, and is the resulting bring-up ORDER
  (clock→port enable→line format→arm) sound for PL011 (datasheet §3.3.8 wants
  UART disabled while reprogramming LCR/IBRD/FBRD)?
- **F3 — enum-literal emission.** Plan: backend emits `irq_source_set` calls with
  `SerialIrqSource::X`/`IrqSourceState::Y` literals via the captured enum `ItemId`s
  + variant indices. Is capturing `source_enum`/`state_enum` ItemIds in the IR
  (vs re-deriving in the backend) the right seam?
- **F4 — IR granularity.** Is per-op `Option<InstanceRef>` (mirroring `LifecycleOps`)
  the right grain, with base ops required (frontend diagnostic if absent) and the two
  capability planes `Option<...>` by impl-presence? Or model a flat op map?
- **F5 — T09 (tx_drain_query) preview.** The synthesized ISR's TX path: does the
  generic_c TX-pump need completion semantics (`tx_drain_query`) in M1, or only at
  M2/M3 close paths? (Decision formally at M1.5; want an early read.)

## 7. Codex foundation review — FOLDED (2026-06-16, SOUND-WITH-CHANGES)

- **F1 (accepted, contract pinned).** Compiler-synthesized ISR; author provides only
  `irq_cause_decode` + C04 primitives. **Synthesized-ISR contract:** `tx_ready`→
  `irq_source_set(TxReady,Disarmed)` (+ refill in real backends); `rx_data`/`rx_timeout`→
  drain `while data_ready_query(RxData) { data_unit_read() }`; `line_error`→**no separate
  ack op — the ack is performed INSIDE the authored `irq_cause_decode`** (it reads MIS and
  W1C-clears the error ICR bits, exactly as the old fixture's `clear_explicit_interrupts`),
  so error causes do not re-latch (resolves blocking #5); `modem_delta`→ignored in v1.
- **F2 (decomposition accepted, ORDER corrected).** Bring-up order is now:
  `clock/reset/pinctrl (Lifecycle) → port_service_set(Block,false) → line_format_apply
  (baud+LCR while disabled) → port_service_set(Block,true) → arm (RxData,Armed)+(LineError,Armed)`.
  The golden (T07) encodes THIS order. (PL011 §3.3.8: reprogram LCR/IBRD/FBRD only while disabled.)
- **F3 (capture + validate variant maps).** IR captures, per semantic type, the resolved
  `ItemId` **and** the variant-index→semantic map, validated by the bridge against the closed
  vocab. Backends mint/compare literals from this map, never by re-deriving names.
- **F4 (kept).** Per-op `Option<InstanceRef>` + `Option<>` capability planes; base C04 ops
  may be `Option` in IR but the **bridge rejects** a missing C04 method before lowering.
- **F5 (kept).** `tx_drain_query` NOT in M1 (`data_ready_query(TxSpace)` + controlled TxReady
  cause suffices); likely needed at M1.5/M2 for close/flush/`tx_empty`.

### Blocking findings folded into the task plan
1. **(B1) Supporting framework types in IR.** T01 captures a `SerialTypeBindings`: resolved
   `ItemId` + variant-index→semantic map for `SerialIrqSource`, `IrqSourceState`, `ConfigResult`,
   `PortScope`, `ReadyDir`, `DataBits`, `Parity`, `StopBits`, plus struct identities for
   `LineFormat`, `RxDatum`, `RxError`, `CauseSet`. The 3 irq-semantic types carry
   `#[rseq::framework_type(...)]`; the rest are captured by reading the recognized method
   signatures (no per-type attribute). `line_format_apply`'s `ConfigResult::Unsupported` is
   compared via the captured variant map (I03).
2. **(B2) `CauseSet` is a struct** (`{bits:u32}`), recognized as a struct framework type — the
   recognition path must handle structs, not enums-only.
3. **(B3) ISR registration coupling with construct #6.** A serial driver has
   `#[rseq::interrupt(id=...)]` but NO authored `#[rseq::isr]`; construct #6 would reject
   (IRQ-ERR-MISSING-HANDLER). Resolution (T03): when the driver is serial-class with
   `impl SerialInterruptDriven`, the serial construct supplies a **synthesized handler** so the
   line registration is satisfied (additive; non-serial drivers keep the authored-ISR requirement).
   Verify the exact construct-#6 seam at T03; consult Codex on the mechanism if non-trivial.
4. **(B4) Cause injection** uses the existing `regseq_irq_dev_poke`→`regseq_regfile` so the
   authored `irq_cause_decode` `regseq_mmio_read(MIS)` returns the poked cause bits.
5. **(B5) Line-error ack** — resolved by F1 (ack inside `irq_cause_decode`).

### Non-blocking folded
6. Bridge diagnostics for missing/duplicate serial framework types + wrong variants, BEFORE
   backend errors (T04 / T03). 7. `modem_delta` = bit 4, present-but-unarmed (T01). 8. Scatter
   table tested independently of the golden (T05 unit tests). 9. Fixture implements all
   `(source,state)` pairs (or a deliberate wildcard) so teardown disarms never hit an unhandled arm.
