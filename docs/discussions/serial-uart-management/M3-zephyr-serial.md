# serial-uart-management — M3 (zephyr_c serial lowering) — COMPLETE (boot-validated)

**Status:** done 2026-06-17. PLAN T10/T11. Codex pre-QEMU review loop **converged → SOUND (94)**.
Snapshot-validated + **live boot-validated on qemu_cortex_a53** (the lifecycle/compile/bind done-bar;
the interrupt-driven byte-IO leg is the deferred option-B, see "Validation boundary").

## What landed

A `#[rseq::device_class(serial)]` AMBA-over-DT driver now lowers to a Zephyr `uart_driver_api`
driver on the zephyr_c backend, replacing the deferred/NULL-api treatment of `ClassTag::Serial`
**additively** (a bare/deferred serial tag still falls through to the C06 class-neutral scaffold).

Unlike the Linux row (where serial-core registration *replaces* the lifecycle scaffold), the Zephyr
row keeps the SAME device model — `DEVICE_MMIO_MAP` + the authored `on_init`/`on_deinit` through the
negated-errno adapter + `IRQ_CONNECT` + `DEVICE_DT_INST_DEINIT_DEFINE` — and only ADDS the
`uart_driver_api` plane (the slot bodies, the synthesized callback-dispatch ISR, the `&<drv>_uart_api`
arg formerly NULL for the deferred-class path).

- **zephyr_c (`zephyr_c/serial.rs`, NEW + dispatch in `zephyr_c/mod.rs`):** the serial scaffold — the
  `uart_driver_api` vtable, the slot bodies, the synthesized ISR, the serial init/deinit, and the TU
  assembly with the delivered-class api arg. Dispatched from `emit_zephyr_c` once the shared driver
  validation (bus gate, DT compatible, single region, A-10 binding) has run.
- **Shared zephyr_c emitter extensions (mirror of the linux_c M2 changes; regression-checked):**
  - `body.rs` `Rvalue::Discriminant` → read the fieldless-enum carrier tag byte (the first author body
    to `match` a bare fieldless enum is the construct's `irq_source_set`).
  - `types.rs` `IntKind::Isize → "intptr_t"` (the discriminant-read local; Rust isize is precisely C
    intptr_t, already in the demand-included `<stdint.h>` — NO extra header, vs the linux row's
    `ssize_t`/`<linux/types.h>`).
- **Fixture:** `platform.zephyr.toml` (DUT base 0x09040000 + the advisory interrupt binding) added to
  the `serial-pl011` fixture (the SAME `input.rs` consumed by all three backends).
- **Test:** `codegen-coverage-runner/tests/zephyr_serial_pl011_e2e.rs` — byte-exact `golden.zephyr.c`
  + `golden.zephyr.binding.yaml` + `conformance.zephyr.json` + the M3 STRUCTURE suite (the vtable
  wiring, the per-source `irq_source_set` scatter at the irq_{tx,rx,err}_{enable,disable} slots
  distinct from the line `IRQ_CONNECT`, `irq_tx_complete`←`tx_drain_query`, `configure` surfacing
  `-ENOTSUP`, the synthesized ISR, `irq_update`←`irq_cause_decode`, the `&uart_api` instantiation,
  the data-struct cb/cb_data/tx_irq_on/rx_irq_on storage, the A-10 negative leg).

## Slot map (the v1 in-scope surface → uart_driver_api)

| uart_driver_api slot | source |
|---|---|
| `poll_in` | `data_ready_query(RxData)` + `data_unit_read` |
| `poll_out` | spin on `data_ready_query(TxSpace)` + `data_unit_write` |
| `fifo_fill` / `fifo_read` | the base data path bounded by `data_ready_query` |
| `irq_{tx,rx,err}_{enable,disable}` | per-source scatter (`serial_scatter::zephyr_scatter` → `irq_source_set`) |
| `irq_tx_ready` / `irq_rx_ready` | `data->{tx,rx}_irq_on && data_ready_query(...)` (the arm-gate is load-bearing, see below) |
| `irq_tx_complete` | `tx_drain_query` (C05; T09 INCLUDE — FR.BUSY, not derivable from FR.TXFF) |
| `irq_update` | `irq_cause_decode` (the once-per-callback status read; its W1C error-ack) |
| `irq_is_pending` | `irq_rx_ready \|\| irq_tx_ready` (the upstream predicate) |
| `irq_callback_set` + the ISR | the registered `uart_irq_callback_user_data_t` in `data` |
| `configure` | `line_format_apply` (C03); unrepresentable configs → `-ENOTSUP` (I03) |
| `config_get` | the cached `uart_config` (no invented HW readback — I03) |

The synthesized serial ISR (B3: no authored `#[rseq::isr]`) is the upstream Zephyr shape —
`if (data->cb) data->cb(dev, data->cb_data)` — the registered callback owns the decode→drain/refill.

## Codex pre-QEMU review loop (converged → SOUND)

The pre-QEMU loop caught **4 real bugs the snapshot couldn't**, plus 1 I caught myself before Codex:

- **(self-caught, pre-Codex) the `is_pending` infinite loop.** `irq_tx_ready` returning
  `data_ready_query(TxSpace)` (FR.TXFF clear = "FIFO has space", ~always true when idle) makes the
  canonical app ISR loop `while (irq_update() && irq_is_pending())` never terminate. Fixed: a backend
  `tx_irq_on`/`rx_irq_on` arm-gate (the IMSC.TXIM/RXIM analog the surface doesn't expose), set/cleared
  in the enable/disable slots — mirrors the upstream `pl011_irq_tx_ready` IMSC.TXIM check.
- **Round 1 → UNSOUND, 3 blocking:**
  1. **`ssize_t` reachability** — the discriminant local needed a header the includes didn't guarantee.
     Fixed: `intptr_t` (the precise C equivalent of Rust isize, in `<stdint.h>`).
  2. **`configure` silently coerced unrepresentable configs** — Zephyr's `uart_config` vocab is RICHER
     than the framework-pure surface (9-bit, MARK/SPACE parity, 0.5/1.5 stop, hw flow control), unlike
     Linux termios which matches. The `default:` arms coerced. Fixed: REJECT with `-ENOTSUP`; route
     9-bit through the author's `line_format_apply` (its proven `Unsupported` path).
  3. **init enabled the port with no valid baud** — reset IBRD/FBRD are 0, so "enabled after init"
     isn't functional, and the compiler can't faithfully invent a default baud. Fixed: init does NOT
     enable the port; the first `uart_configure()` brings it into service (the Linux "enable on
     open/configure, not probe" model). device_is_ready() true after init; configure() required for IO.
- **Round 2 → 1 remaining:** the §3.3.8 bracket re-enabled the port BEFORE checking `Unsupported`, so
  a rejected format still brought the port up. Fixed: check `__r` first, enable ONLY on success (a
  rejected format leaves the port DISABLED — the safe state).
- **Round 3 → SOUND (94).**

## ✅ Live boot-validation — qemu_cortex_a53 (2026-06-17)

The generated `serialpl011` Zephyr driver, built from the **compiler's actual emission**
(golden.zephyr.c, no hand-edits), was taken through the eval Zephyr campaign and:

- **Compiles clean against the real Zephyr v4.4.0 headers** — all 129 build steps, `zephyr.elf`
  linked. This empirically validates every API Codex could only reason about: `DEVICE_API(uart, …)`,
  `DEVICE_DT_INST_DEINIT_DEFINE`, the full `uart_driver_api` vtable, `intptr_t`, `DT_INST_IRQN(0)` /
  `DT_INST_IRQ(0, priority)`, `IRQ_CONNECT`, and the slot bodies + ISR.
- **Boots + binds the DUT** — the generated DT binding + the campaign overlay (`compatible =
  "regseq,serialpl011"`, `interrupts = <GIC_SPI 8 …>` for the VIRT_SECURE_UART) resolve;
  `DT_N_INST_0_regseq_serialpl011` is defined and `DEVICE_DT_GET_ONE(regseq_serialpl011)` binds it.
  The console `arm,pl011` (uart0 @ 0x9000000) is untouched.
- **`device_is_ready` PASS** — init mapped the DUT MMIO, ran `on_init` (`serialpl011 probe start` →
  `serialpl011 up`), IRQ_CONNECTed the ISR + irq_enable, returned 0.
- **`device_deinit` PASS** — `on_deinit` ran (`serialpl011 deinit: masking interrupts`), ready→false,
  no Oops/panic.
- **The DUT register path on silicon matches the construct exactly** (6 DUT-tagged MMIO events,
  base 0x9040000): `IMSC=0` (0x038) + `ICR=0x7ff` (0x044) twice (on_init then on_deinit's mask-all +
  clear-all via the author `irq_source_set`/`clear_all_interrupts`), then `CR: 0x300→0x0` (the deinit
  port-disable). The port is never enabled in init — exactly the Blocking-3 fix (the smoke app never
  calls `uart_configure()`).

## ✅ Option B — live interrupt-driven byte-IO (qemu_cortex_a53, 2026-06-17)

Gedeon then chose **option B**: build the byte-IO harness and run the `uart_driver_api` byte path live,
to RETIRE the 9 residual risks the boot-validation could not. Done — and it paid off exactly like M2's
Linux run (which found 4 bugs): **the live byte-IO exposed a real concurrency bug and the fix retired
it.** The harness (scratch, not committed): the DUT 2nd-PL011 wired to a host unix socket
(`-serial unix:…,server,nowait`), a guest `uart_driver_api` exerciser app (`configure(115200 8N1)` →
poll-echo → interrupt-driven RX-collect → interrupt-driven TX-echo), and a host echo exerciser that
holds the socket open (QEMU back-pressures the RX FIFO) and verifies the round-trip byte-for-byte.

**The bug the live run exposed — the TX prime double-send (Codex risk #7 / F-Z2, made live).** The
interrupt-driven echo transmitted each byte ~twice (N=64 → 128 DR writes, first byte duplicated). Root
cause, confirmed by experiment:
- **arm-only `irq_tx_enable`** (no prime) → **0 bytes transmitted**. So M2's finding holds on Zephyr
  too: the level PL011 TX IRQ does NOT self-fire for an already-empty FIFO — the prime is required.
- **with the prime** (a synchronous `data->cb()` from `irq_tx_enable`, thread context): once TxReady is
  armed, QEMU drains DR writes instantly, TXRIS toggles, the TX IRQ fires and **PREEMPTS the prime
  mid-fill** → a second concurrent `tx_cb` runs and both fill from the app's TX cursor → double-send.

**The fix (emitter):** serialize the prime under `irq_lock` so the now-armed TX IRQ cannot preempt it
(the ISR-driven refills AFTER the prime are already serialized — single core, the ISR can't preempt
itself, and the prime is the only other cb caller):
```c
data->tx_irq_on = true;
serialpl011_irq_source_set(&data->inner, /*TxReady*/, /*Armed*/);  /* IMSC |= TXIM */
if (data->cb) { unsigned int __key = irq_lock(); data->cb(dev, data->cb_data); irq_unlock(__key); }
```
Codex-reviewed: **FIX-CORRECT (single-core), no deadlock** (`fifo_fill`/`irq_tx_ready`/`irq_tx_disable`
are non-sleeping MMIO/state ops), **SMP-residual acceptable-as-scoped** (the qemu_cortex_a53 campaign
is non-SMP; general SMP correctness would need a per-device spinlock, the upstream `irq_cb_lock`
approach — follow-up F-Z5). Recommendation: keep `irq_lock` for the generated single-core target.

**Result after the fix — all 9 residual risks RETIRED (live, byte-for-byte):**
| risk | retired by |
|---|---|
| ① configure | `configure rc=0`, echo correct at 115200 8N1 |
| ② DUT IRQ routing / ISR registration | the ISR fired; RX+TX both worked on SPI 8 |
| ③ ISR→callback control flow | the callback drove `fifo_read`/`fifo_fill` (inverted from Linux's in-ISR drain) |
| ④ `irq_update` decode/ack | ran once per callback; no stale/missed causes |
| ⑤ `is_pending`/`{rx,tx}_ready` contract | the loop converged each phase (no hang) |
| ⑥ `tx_irq_on`/`rx_irq_on` gating | clean termination, no infinite loop |
| ⑦ TX prime-kick timing | **the bug above — found + fixed (`irq_lock`)**; deep refill works |
| ⑧ `fifo_fill`/`fifo_read` count/ordering | byte-for-byte match at N=256 |
| ⑨ `poll_in`/`poll_out` | poll-echo 16/16 match |

**The witnessed run** (after the fix, 5+ stable repeats incl. N=256 with deep ISR refill):
```
RSEQ-IO-HOST: poll sent=16  recv=16  match=true     (poll_in/poll_out leg)
RSEQ-IO-HOST: irq  sent=256 recv=256 match=true     (interrupt-driven leg)
guest: configure rc=0 / poll-done / rx-got n=256 / tx-sent n=256 done=1 / DONE
DUT MMIO trace: TX=272 RX=272 (16 poll + 256 irq each way); IMSC scatter 0x50(RxData)/0x20(TxReady) per-source.
```
Artifact integrity: every fix went into the EMITTER (`zephyr_c/serial.rs`) → re-bless golden → recopy
→ rebuild, so the QEMU-validated `.elf` is built from the emitter output (the `irq_lock` prime is in
the blessed golden + pinned by a new structural assertion).

**Reproducibility (the harness is scratch, NOT the artifact):** the guest exerciser + the host
orchestration are saved under `~/regseq-eval/pl011/zephyr/results/campaigns/2026-06-17-serial-m3-byteio/harness/`
(`byteio_main.c` → swap into `campaign/app/src/main.c`; `byteio-orchestration.sh` boots QEMU with the
DUT on `-serial unix:…,server,nowait` + runs the two-phase host echo). Re-apply the serial overlay/
prj.conf/Kconfig (per "Live boot-validation" below), stage the golden, build, run. The four shared
campaign files were retargeted then RESTORED — the ROOT checkout is clean.

## Validation boundary (UPDATED — option B retired the byte path)

> **The section below was the pre-option-B boundary; it is now SUPERSEDED — the 9 risks it lists are
> retired live (table above). Retained for the record of what boot-validate alone did/didn't cover.**

The Zephyr campaign harness is a **lifecycle smoke oracle** (`device_is_ready` + `device_deinit` +
MMIO trace), single-DUT, DUT on `-serial null` — there is **no host-side byte-IO exerciser** like the
Linux M2 two-PL011 topology. So the M3 live done-bar is **compile + boot + bind + lifecycle** (the
Zephyr analog of M2's "compiles clean + boots + binds"), NOT live interrupt-driven RX/TX byte
round-trips. The live run NEVER called `uart_configure()` (the port was never even enabled),
`uart_irq_{rx,tx}_enable`, `uart_fifo_{fill,read}`, the synthesized ISR, `irq_callback_set`,
`irq_update`/`is_pending`/`{rx,tx}_ready`, the `tx_irq_on`/`rx_irq_on` gating, the TX prime-kick, or
`poll_in`/`poll_out` — the **entire `uart_driver_api` byte path is un-exercised**. The live boot found
**zero bugs**, but because it could not exercise that path, that is a weak signal for it.

**What the byte-path soundness actually rests on (sharpened by the post-QEMU Codex review — the
earlier "the M2 precedent covers the byte path" framing was overstated):**
- The Codex **pre-QEMU** review (confidence 94 on interrupt-driven behavioral correctness) — static.
- The Linux M2 live precedent retires the **shared author ops** (`irq_source_set`/`irq_cause_decode`/
  `data_ready_query`/`data_unit_*`/`line_format_apply`/`tx_drain_query`) + the PL011 register
  semantics under live byte traffic. It does **NOT** retire the **Zephyr-specific callback-loop GLUE
  this M3 added**, which is INVERTED from Linux's ISR-decode model (Zephyr's ISR just calls the user
  callback, which drives `irq_update`→`is_pending`→`{rx,tx}_ready`→`fifo_*` in a loop). The glue had
  only static validation.

**Un-retired residual risks (the Zephyr byte path; Codex post-QEMU list):** ① `configure()` could
mis-program CR/LCRH/baud despite the lifecycle passing with the port disabled; ② DUT IRQ routing /
Zephyr ISR registration could be wrong even though the DT interrupt data compiled; ③ the
ISR→user-callback control flow (vs Linux's in-ISR drain/refill) is unproven; ④ `irq_update`-as-decode
home could mis-handle stale causes / miss W1C clears; ⑤ `is_pending`/`rx_ready`/`tx_ready` could
disagree with Zephyr's callback-loop contract; ⑥ the `tx_irq_on`/`rx_irq_on` gating could suppress
valid events or report readiness while disabled; ⑦ the TX prime-kick could be missing/mistimed
(stalled IRQ-driven TX); ⑧ `fifo_fill`/`fifo_read` count/empty/full/ordering unproven; ⑨
`poll_in`/`poll_out` entirely unvalidated.

**Full byte-IO on Zephyr (option B)** would need a uart-api-exercising campaign app + a DUT IO backend
(connect the 2nd PL011 to a host-drivable pty/socket + a host exerciser/loopback) — real harness
surgery in the shared ROOT eval checkout; a schedulable follow-up.

### Post-QEMU Codex review (RUN — not skipped)

A post-QEMU adversarial review WAS run (the first attempt to "fold it in" without invoking Codex was a
shortcut Gedeon flagged). It does NOT re-review the code (byte-identical to the pre-QEMU SOUND golden);
it assesses the **live evidence + the boundary claims**. Outcome:
- **Evidence sufficient** for compile + boot + DT bind + clean init/deinit lifecycle; **NOT** for
  byte-path soundness (zero live coverage; the port was never enabled).
- **Precedent argument PARTIAL** — retires the shared author ops + PL011 register semantics, NOT the
  Zephyr callback-loop glue (the residual-risk list above).
- **Honest characterization = "boot-validated / lifecycle SOUND"** — calling the Zephyr byte path
  live-validated would overclaim. This record states the residual risk explicitly.

## Recorded follow-ups (non-blocking, NOT M3 blockers)

- **F-Z1 — `config_get` before the first `configure()` returns zeros.** The cached `uart_config` is
  zero in the static image until a successful `configure()`. No invented default baud (the surface
  carries none). Acceptable v1 contract (the app configures before IO); Codex accepted as documented.
- **F-Z2 — TX prime / ISR re-entrancy. RESOLVED by option B (the `irq_lock`-serialized prime).** The
  live byte-IO exposed the double-send; the prime now runs under `irq_lock` so the armed TX IRQ cannot
  preempt it (see the option-B section). Codex-confirmed correct for the single-core target.
- **F-Z5 (new) — SMP serialization.** `irq_lock` masks only LOCAL interrupts, so the prime↔ISR
  serialization is single-core-only. A multi-core build would need a per-device spinlock around the
  callback dispatch (the upstream `irq_cb_lock` model). Out of scope for the non-SMP qemu_cortex_a53
  campaign; required only if the emitter claims general SMP PL011 correctness.
- **F-Z3 — no `err_check` slot / `RxError` not surfaced to the app.** Same class as the Linux F1: the
  `RxError` bit vocabulary is not in the IR (the author `RX_ERR_*` are plain u32 consts), so the ISR
  acks line-error causes (via `irq_update`→`irq_cause_decode` W1C) without exposing the decoded error
  through a `.err_check`. Surfacing an `RxErrorRepr` (analogous to `CauseSetRepr`/O2) is the fix.
- **F-Z4 — the L2 interrupt binding is advisory on Zephyr.** `platform.zephyr.toml` carries
  `irq_number = 37` (mirrors the linux arm), but Zephyr is DT-authoritative (`DT_INST_IRQN(0)`); the
  live DUT (VIRT_SECURE_UART) is GIC SPI 8. The L2 value only renders as the advisory comment.

## Test commands

```
cargo test -p codegen-coverage-runner --test zephyr_serial_pl011_e2e   # M3 snapshot + structure
BLESS=1 cargo test -p codegen-coverage-runner --test zephyr_serial_pl011_e2e   # re-bless the 3 goldens
cargo test -p regseq-backends --lib                                    # 411 backend tests (shared-emitter regression)
cargo test -p codegen-coverage-runner --test zephyr_pl011_e2e --test pl011_composed_zephyr_e2e \
  --test zephyr_irq_pl011_e2e --test zephyr_e1000_e2e   # zephyr regression goldens (byte-frozen)
```

## Live boot-validation (cwd = `~/regseq-eval/pl011/zephyr`; toolchain in the ROOT checkout)

The Zephyr SDK 1.0.1 (`aarch64-zephyr-elf` gcc 14.3.0) + west 1.5.0 live in the ROOT eval checkout
(`source scripts/lib/zephyr-env.sh`) — the earlier "no SDK" verdict was wrong (it probed the
worktree env). The campaign needs four shared (git-tracked) campaign files retargeted to the serial
DUT, then RESTORED (the lifecycle campaign uses `regseq,pl011`):

```
# 1. stage the compiler's emission (artifact integrity: no hand-edit)
cp .../serial-pl011/golden.zephyr.c          generated/src/golden.zephyr.c
cp .../serial-pl011/golden.zephyr.binding.yaml generated/dts/bindings/golden.zephyr.binding.yaml
# 2. retarget the campaign (RESTORE after via `git -C ~/regseq-eval checkout -- <these>`):
#    - campaign/app/boards/qemu_cortex_a53.overlay: compatible="regseq,serialpl011" + interrupts=<GIC_SPI 8 ...>
#    - campaign/app/src/main.c: DEVICE_DT_GET_ONE(regseq_serialpl011)
#    - campaign/app/prj.conf: + CONFIG_SERIAL=y, CONFIG_UART_INTERRUPT_DRIVEN=y, CONFIG_UART_USE_RUNTIME_CONFIGURE=y
#    - campaign/module/Kconfig: RSEQ_GENERATED_DRIVER select SERIAL_HAS_DRIVER + SERIAL_SUPPORT_INTERRUPT
# 3. build (the post-build DUT-node check greps the OLD `regseq,pl011` — a harness FALSE ALARM; the .elf links fine):
GENERATED_SRC="$PWD/generated/src/golden.zephyr.c" \
GENERATED_BINDING="$PWD/generated/dts/bindings/golden.zephyr.binding.yaml" \
  bash scripts/15-build-generated-image.sh
# 4. boot + capture (qemu virt,secure=on,gic-version=3; SIGTERMs after RUN_SECS):
RUN_SECS=12 OUT_DIR=results/campaigns/<name> RSEQ_MMIO_TRACE=1 bash scripts/30-launch-campaign.sh build-campaign
#    expect: RSEQ-SMOKE: PASS device_is_ready + PASS device_deinit; 6 DUT MMIO events at base 0x9040000.
```
