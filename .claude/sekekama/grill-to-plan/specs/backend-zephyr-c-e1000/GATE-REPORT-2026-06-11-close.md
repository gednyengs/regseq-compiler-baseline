---
feature_id:   F-backend-zephyr-c-e1000
feature_slug: backend-zephyr-c-e1000
artifact_id:  GATE-REPORT-2026-06-11-close
type:         gate-report
status:       closed
source:       [GRILL-2026-06-11-100437, PRD-2026-06-11-100716, PLAN-2026-06-11-101303]
created:      2026-06-11
---

# Closing gate-report — backend-zephyr-c-e1000 (T14, the verification gate)

**Unit verdict: DONE — UNMERGED at the gate per precedent. All five
milestones closed; every R/I/C row evidenced or gate-reported; final
whole-unit Codex review CLEAN (ok=true, zero findings, evidence
spot-re-run by the reviewer).**

The headline is now literal across the full 2×2: ONE `input.rs` per e1000
fixture serves generic_c, linux_c, AND zephyr_c — the PCIe-over-DT path on
Zephyr emits, builds, binds, boots, tears down, and trace-matches the
canonical oracle.

## The evidence checklist, item by item

1. **cargo green at tip: 2380 passed, 0 failed** (baseline 2355; every
   count move accounted per commit: +5 T01 net, +7 T02 net, +3 T03, +5
   T04, +1 C1-fix net, ±0 T05, +3 T06, +1 T07).
2. **L0 structure suites + both zephyr snapshot binaries green**
   (`zephyr_e1000_e2e`, `zephyr_e1000_initfail_e2e`,
   `e1000_composed_zephyr_e2e` — C01/C02/C03/C06 landmarks locked in the
   committed goldens).
3. **R08 unblessed-first proof** (T05/892a078, detailed in
   GATE-REPORT-2026-06-11-M1): generic pair byte-identical UNBLESSED;
   linux artifacts changed by exactly the licensed C04 audit lines; no
   linux_c/generic_c source change.
4. **L1 gates re-verified at tip:** compiler west gate — pl011
   (qemu_cortex_a53) + BOTH e1000 TUs (qemu_x86) built against the v4.4.0
   workspace; eval bind oracle re-run green (regseq_e1000 → pcie0/
   regseq_dut only; no status-okay intel_e1000; no COMPAT_MATCHES leak).
5. **L2 legs current:** composed PASS (ready + deinit rc==0/ready→false +
   authored dbg/inf/wrn live + clean console); initfail PASS
   (FAIL device_is_ready `init_res=5` — the authored EIO channel, distinct
   from the C01 rungs' unwrapped 19 (I09) — + SKIP three-outcome + both
   authored LOG_ERR lines). The D08(1) four-level wording is amended by
   GATE-REPORT-2026-06-11-D08-log-levels (the fixture's err sites are
   failure-path-only; the err witness rides the initfail leg).
6. **L3 current:** trace differential PASSED — 16/16 DUT MMIO events
   aligned in op/offset/size/order; nine cited whitelist entries consumed;
   the amendment-8 absent-slot entry set EMPTY by inspection (the oracle
   carries zero surprise-remove markers; the binding citation rule
   recorded — O2 closed); the zephyr config containment pass green (D06
   zero word-0 driver reads; D05 CLS = one word read + one word write of
   word 3, closing the stream).
7. **Docs landed** (T08 + C3 fix): C-1 closed with the five live-verified
   `pcie.h` pins; B-3 partial-lift; A-3/A-4/A-15 records; B-9/A-16
   disposition evolution; D.2 rows updated. Capability-table doc untouched
   (D03).
8. **Final Codex review: ok=true, zero findings** (session
   019eb823-96b3-7db2-a631-3291ecb2c477) — traceability sweep complete (no
   R/I/C row without evidence), frozen-surface audit clean over the full
   range, no later-chunk regression of earlier acceptance, evidence
   spot-re-run (cargo, west both legs, bind oracle, both smoke gates, the
   containment checker).
9. **Terminal audits (I10):** eval repo path audit — every unit commit
   touches `e1000/zephyr/campaign/` only; the eval-refinement tree
   untouched; no-AI-attribution audit clean on both repos; **the branch
   `backend/zephyr-c-e1000` is left UNMERGED.**

## Review trail

C1: round 1 ok=false (2 blocking: region-less-PCIe partial preamble;
u128→u64 offset truncation) → fixed 4aab8ad → round 2 ok=true.
C2 (M1): ok=true, no findings. C3 (M2): 1 should-fix (misalignment
wording) → fixed 0e7eb98. C4 (M3): ok=true. C5 (M4): round 1 ok=false (1
blocking: the D08(1) log-level wording) → gate-report disposition → round
2 ok=true. Final (T14): ok=true, zero findings.

## Open items at close

None blocking. O1/O3 discharged by the T10 spike (pristine 8.2.2
captures; qemu_x86 final); O2 discharged at T13 (empty set + binding
rule); O4 discharged at T07 (CONFIG_PCIE=y + the inherited pins).
Recorded deltas: the initfail `golden.zephyr.binding.yaml` additive C05
delta (pipeline-forced, pl011-initfail precedent, accepted at the C2
review); the D08(1) criteria amendment (its own gate-report).

## What the successor units inherit

- The zephyr_c PCIe arm is live end-to-end with the D05 RMW, D06 word-0
  cache, D12 `__rseq_pcie` carrier, and D14 audited class-neutral
  emission; the platform/Amba and ClassTag::Generic paths stayed
  byte-frozen throughout (zero pl011 re-blesses — the continuous proof).
- The eval `e1000/zephyr/campaign/` subtree carries reusable L1/L2/L3
  machinery; the capture mechanism is the pinned pristine 8.2.2 with
  built-in tracepoints (the spike memo is the mechanism record).
- Runtime-index register arrays now lower on zephyr_c (the F-1 certified
  base + (i)*stride mechanism, mirrored from linux_c during T06).
