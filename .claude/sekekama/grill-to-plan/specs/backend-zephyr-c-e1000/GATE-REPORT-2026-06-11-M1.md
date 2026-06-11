---
feature_id:   F-backend-zephyr-c-e1000
feature_slug: backend-zephyr-c-e1000
artifact_id:  GATE-REPORT-2026-06-11-M1
type:         gate-report
status:       closed
source:       [GRILL-2026-06-11-100437, PRD-2026-06-11-100716, PLAN-2026-06-11-101303]
created:      2026-06-11
---

# M1 gate-report — backend-zephyr-c-e1000 (chunks C1+C2, the L0 vertical slice)

**Milestone verdict: M1 CLOSED. Both e1000 fixtures emit complete Zephyr
PCIe TUs through the runner; frozen artifacts proven unchanged
UNBLESSED-FIRST; the full C05 zephyr set blessed; two Codex chunk reviews
(C1 round 2 ok=true, C2 round 1 ok=true).**

## Evidence, item by item

1. **cargo green at tip (935e1de): 2379 passed, 0 failed.** Baseline 2355;
   every count move accounted per commit (+5 T01 net, +7 T02 net, +3 T03,
   +5 T04, +1 C1-fixes net, ±0 T05, +3 T06).
2. **R08 unblessed-first proof (T05, 892a078):**
   - `e1000_composed_e2e` ran GREEN UNBLESSED over the converged fixture —
     `golden.c` + `expected.stdout` BYTE-IDENTICAL (no git hunk exists for
     either, both fixtures); generic_c source untouched (I01).
   - The linux legs failed unblessed exactly at the snapshot stage; the
     post-bless diff is EXACTLY one `RSEQ-IGNORED-MATCH` C block per
     `golden.linux.c` + one `ignored_match_entries` row per
     `conformance.linux.json` — the licensed C04 audit lines (I02). No
     linux_c/generic_c code change was needed.
3. **C05 artifact sets:** composed = platform.zephyr.toml, golden.zephyr.c,
   golden.zephyr.binding.yaml, golden.zephyr-oracle.c,
   expected.zephyr.stdout, conformance.zephyr.json (delivered =
   on_init/on_init_failure/on_deinit:CAP-ZEPHYR-DT-DEINIT-1; absent =
   on_surprise_remove:CAP-ZEPHYR-DT-SURPRISE-1; ignored = both Pci rows;
   deferred_class = network/D14). initfail = platform.zephyr.toml,
   golden.zephyr.c, conformance.zephyr.json, NO oracle pair, **PLUS
   golden.zephyr.binding.yaml — a recorded additive C05 delta**: the
   pipeline byte-locks all three zephyr_c artifacts unconditionally
   (run_zephyr_fixture; a file-exists conditional would be a silent skip)
   and the pl011-initfail-zephyr precedent carries one. Codex C2 review
   examined and accepted the delta (observation 3).
4. **Charter-saving dispositions exercised for real:** the fixture's
   `write_config::<u8>(CLS@0x0C)` lowered via the D05 containing-word RMW
   (golden.zephyr.c:197ff) and the declared `device_class(network)` emitted
   class-neutral with the C06 audit comment (golden.zephyr.c:448ff) — the
   born-converged charter held with ZERO fixture forks (I03).
5. **One backend loop-back during T06 (the planned rail):** the fixture's
   runtime-index register arrays hit the zephyr receiver chase's missing
   `Index` arm; extended with the certified `base + (i)*stride` recovery
   (F-1 shared mechanism). pl011 zephyr snapshots stayed green with ZERO
   re-bless across all of C1+C2 — the continuous frozen-surface proof
   (I08/C06 item 3).
6. **Codex reviews:** C1 round 1 ok=false (2 blocking: region-less-PCIe
   partial preamble; u128→u64 offset truncation) — both fixed in 4aab8ad,
   round 2 ok=true. C2 round 1 ok=true, no findings.
7. **Disk:** 129G free at the C2 close (≳20G bar).

## Open items carried forward (all non-blocking, owned by plan tasks)

O1 capture spike (T10), O2 amendment-8 wording (T13), O3 board final
(T10d), O4 PCIe Kconfig set (T07 fail-loud discovery).
