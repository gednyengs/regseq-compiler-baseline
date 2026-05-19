# 03 — Cut List and Technical Justification

This document records every test/axis removed from the prior strategy, the
reviewer-facing reason, and — for the consequential cuts — the source-level
evidence (file:line) that makes the cut defensible at peer review. A reviewer
will ask "why didn't you evaluate checksum offload / interrupt moderation?"
The answer must be airtight; it is given in §2.

## 1. Cut list

| Removed | Reason | Category |
| ------- | ------ | -------- |
| **F6-RX** (RX-checksum offload, runtime row) | Stock QEMU does not model RX checksum status; both native and Regseq are forced to the identical software-checksum path → the row discriminates nothing. Measuring it requires patching the emulator → would measure the patch, not the DSL. | Tests the emulator, not the DSL |
| **P4** (interrupts-per-packet / moderation) | Stock QEMU does not implement RDTR/TIDV relative timers and imposes a fixed mitigation floor; the 82540EM exposes no native ethtool coalesce interface. Native cadence is an emulator artifact. | Tests the emulator, not the DSL |
| **P7** (descriptor-ring fill/drain efficiency) | Shares the interrupt/observability dependency with P4; same patch dependency, same confound. | Tests the emulator, not the DSL |
| **The QEMU device-model patch program** (prior `e1000-experiment-methodology.md` §3): RX-checksum/descriptor-writeback (3.4), interrupt-moderation fidelity (3.3), register-state introspection (3.2/3.5), and the **LSC-IRQ-lifecycle-fidelity** intent of 3.1 | After F6-RX/P4/P7 are cut, these device-model semantic patches have no consumer. W4's link down/up is *not* one of their consumers: it is satisfied patch-free by the host-side QMP `set_link` command (stock QEMU already raises the E1000 LSC interrupt on link-status change), and W4 is pass/fail, so 3.1's *fidelity* goal is unneeded. No remaining workload requires a device-model change. | Removes emulator-measurement confound |
| **P3** (RX robustness under CPU stress) | Characterizes guest-scheduler / emulator pressure more than DSL semantics. Retain only as an observation if it surfaces during W2 bulk runs. | Tests the emulator/scheduler |
| **Fine-grained F7** (interrupt-cadence discrimination) | Keep only "interrupt-driven service works" (folded into W1/W2). Moderation timing is not load-bearing for a DSL claim. | Not load-bearing |
| **IL5** (ethtool coalesce operations) | The legacy 82540EM driver returns `-EOPNOTSUPP` for coalesce; the *correct* Regseq behavior is to also return `-EOPNOTSUPP`. A results row here would be API archaeology; it is reframed as a correctness expectation — the Regseq driver must also return `-EOPNOTSUPP` (§2.4). | Not DSL validation |
| **IL6** (sysfs / /proc visibility) | Integration cosmetics; smoke-log only, not a results row, unless absence breaks usability. | Not load-bearing |
| **P6** (small-packet latency curve), **P1/P2** as standalone perf rows | Performance is defensive; collapsed into the single W2/W3 guardrail (§02 §4). | Defensive, not offensive |
| **F9 (VLAN), flow control, jumbo, XDP, SR-IOV, MSI-X, TSO, multi-queue, PM** | Already deferred by D8/D9/D10 and the variant pin; unchanged. | Out of scope (prior) |

Use cases U1–U7 are **not** cut. The minimal workload set (§02 §3) ranges over
them; only the over-fine F/P/IL/IZ decomposition is removed.

## 2. Source-level justification for the consequential cuts

Evidence base, read directly:

- QEMU device model: `~/regseq-eval/pl011/qemu-src/qemu/hw/net/e1000.c`,
  upstream-clean **v8.2.2** (git tag `v8.2.2`, `hw/net/e1000.c` unmodified).
- Linux stock driver: `~/driverbase/os/linux/drivers/net/ethernet/intel/e1000/`
  (the legacy `e1000` driver; 82540EM is MAC type `e1000_82540`).

### 2.1 The `mac_type` enum (resolves the gating of every feature below)

`e1000_hw.h:21-37`: `e1000_82543`=3, `e1000_82544`=4, **`e1000_82540`=5**,
`e1000_82545`=6. Therefore for the 82540EM: `>= e1000_82543` is **true**,
`>= e1000_82540` is **true**, `< e1000_82545` is **true**. (Verified directly;
this resolves a flagged ambiguity — the 82540EM *does* receive the
checksum/moderation programming below, but is *excluded* from the ethtool
coalesce interface.)

### 2.2 RX-checksum offload — real gap, not moot (justifies cutting F6-RX)

The stock driver genuinely uses it on the 82540EM:

- RX checksum enabled for `mac_type >= e1000_82543` → 82540EM qualifies
  (`e1000_main.c:1031`); `e1000_rx_checksum()` early-returns only for
  `< e1000_82543` (`e1000_main.c:3956-3957`); `RXCSUM` register programmed
  (`e1000_main.c:1905-1911`); result surfaced as `CHECKSUM_UNNECESSARY`
  (`e1000_main.c:3974`) and the `hw_csum_err` ethtool stat
  (`e1000_main.c:3964`).

QEMU does not model it:

- The `RXCSUM` token does **not appear anywhere** in `hw/net/e1000.c`
  (verified by exhaustive grep — zero matches); it is defined only in shared
  headers (`e1000_common.h:66`), not in this model's register-op tables.
  Writes to it fall through QEMU's unimplemented-register path and are
  **ignored/dropped** (`e1000.c:1282-1288`) — not acted upon.
- The receive path sets only `E1000_RXD_STAT_EOP | E1000_RXD_STAT_IXSM`
  (`e1000.c:974`) and `vlan_status | E1000_RXD_STAT_DD` (`e1000.c:984`). It
  never sets `IPCS`/`TCPCS`/`UDPCS` status or `IPE`/`TCPE` error bits. `IXSM`
  is literally "ignore checksum indication": QEMU tells the driver to ignore
  checksum status.

Consequence: on stock QEMU, **both** the native and the Regseq driver fall
back to software checksumming identically; the row cannot distinguish a correct
from an incorrect RX-offload implementation. The only way to make it
measurable is to patch QEMU's receive path — at which point the experiment
measures the QEMU patch, not the DSL. Cut as a runtime row; retained as
*expressiveness* evidence (the spec declares RXCSUM; the generated driver
programs it).

### 2.3 TX-checksum offload — faithful (justifies keeping it functionally, not as a perf row)

`xmit_seg` honors `E1000_TXD_POPTS_TXSM`/`IXSM` (`e1000.c:618-623`) and
`putsum` computes and inserts real IP/TCP/UDP checksums via
`net_checksum_add` (`e1000.c:535-543`), driven by the context/data descriptor
fields (`e1000.c:651-666`). TX checksum offload is therefore exercised
*functionally* inside W2 (offload enabled, payload still correct). No
TX-checksum *performance* claim is made.

### 2.4 Interrupt moderation — partial/artifact-bound (justifies cutting P4/P7)

Stock driver programs the moderation registers on the 82540EM: `RDTR`
unconditionally (`e1000_main.c:1876`), `RADV` for `>= e1000_82540`
(`e1000_main.c:1878`), `TIDV` unconditionally (`e1000_main.c:1646`), `TADV`
for `>= e1000_82540` (`e1000_main.c:1647`), static ITR for `>= e1000_82540`
(`e1000_main.c:1880-1881`), adaptive ITR via `e1000_set_itr` for
`>= e1000_82540` (`e1000_main.c:2632`).

QEMU models it only partially: the model comment states *"relative timers
based on TIDV and RDTR are not implemented"* (`e1000.c:302`), and a hard floor
`mit_delay = (mit_delay < 500) ? 500 : mit_delay` (`e1000.c:329`) clamps the
delay regardless of programmed RDTR/RADV/ITR (mitigation logic at
`e1000.c:267`, `275-339`, `342-348`).

Additionally, the legacy driver returns `-EOPNOTSUPP` for ethtool coalesce on
the 82540EM (`mac_type < e1000_82545` gate at `e1000_ethtool.c:1752` and
`:1771`), so there is no native runtime interface to configure or read
moderation on this part at all; control is module-param-only.

Consequence: native interrupt cadence on stock QEMU reflects the emulator's
mitigation model, not the driver's moderation policy; faithful measurement
again requires patching QEMU. Cut. Moderation registers remain *expressible*
in the spec with no runtime-benchmark obligation.

### 2.5 Link/LSC fidelity (bounds W1/W4 honestly)

Autoneg/LSC is a fixed deterministic 500 ms timer (`e1000x_common.c:166`;
LSC raised at `e1000.c:360-366`; same on reset at `e1000.c:1400-1401`). W1/W4
treat link bring-up and link-flap as pass/fail and explicitly do not claim
anything about negotiation *timing*. W4's link down/up is driven by the
host-side QMP `set_link` command — **not** a device-model patch — and stock
QEMU already raises the E1000 LSC interrupt when link status changes
(link-status→LSC path, `e1000.c:810-827`). Pass/fail at this granularity needs
no patch; only LSC-lifecycle *fidelity* (the withdrawn prior patch 3.1 intent)
would, and W4 makes no fidelity claim.

### 2.6 An honest discrepancy, recorded

Two independent reads of the mitigation path disagreed on one detail: whether
the *first* interrupt is asserted immediately and then floored, or always
delayed by `mit_timer`. Both reads agree on every decision-relevant fact
(RXCSUM unmodeled; RDTR/TIDV relative timers not implemented; 500-unit floor).
The discrepancy is a patch-design detail and does **not** affect any cut here;
it is recorded for intellectual honesty and because it reinforces that P4/P7
fidelity is exactly the kind of emulator-internal question a DSL paper should
not be litigating.

## 3. Hard invariant

**No workload or guardrail in this strategy may depend on a modification to
QEMU's device model** (the C in `hw/net/e1000.c` / `e1000x_common.c`). The
boundary is precise: host-side **control-plane** facilities — QMP `set_link`,
the standard monitor, `-netdev`/bridge configuration — are *not* device-model
patches and are permitted (W4 relies only on these). What is forbidden is
changing how the emulated device computes or reports state. Any future
proposal that reintroduces a device-model dependency must first re-open
§01 §2 (the defensive/offensive boundary) and §02 §1 (expressiveness vs.
runtime), in writing, with reviewer-facing justification.

## 4. Position: do the cuts weaken the paper?

**They strengthen it.** The QEMU-relative method is sound precisely because
native and Regseq run against the *same* virtual device, so all observed
difference is attributable to the driver. The moment a row requires changing
the device model, the experiment is partly validating a new virtual NIC, and
attribution to the DSL is lost. Cutting F6-RX / P4 / P7 and the patch program
narrows the claim but removes a confound and makes what remains cleanly
attributable and believable. Performance stays a bounded guardrail; the
offensive weight moves to expressiveness and cross-OS portability — which is
where a device-driver-DSL contribution actually lives.
