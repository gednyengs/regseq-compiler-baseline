---
feature_id:   F-spec-convergence-pl011
feature_slug: spec-convergence-pl011
artifact_id:  GATE-REPORT-2026-06-10-close
type:         gate-report
status:       closed
source:       [GRILL-2026-06-10-211237, PRD-2026-06-10-212021, PLAN-2026-06-10-213314]
created:      2026-06-10
---

# Closing gate-report — spec-convergence-pl011 (T14, the C01 merge gate)

**Unit verdict: DONE. All four milestones closed; C01 satisfied in full;
final Codex review CLEAN after one dispositioned MUST-FIX + re-review.**

The headline is now literal: ONE `input.rs`
(`fixtures/_composed/pl011-composed/`) consumed by linux_c AND zephyr_c,
generic_c oracling both bindings, with the fixture pair and the identity
machinery retired.

## C01, item by item

1. **`cargo test` green at tip (03bbae5): 2355 passed, 0 failed.**
   Baseline 2353 → 2355; every count move accounted in commit messages
   (+1 T01, +5 T02, +2 T03, ±0 T04, +1 T05, +3 T06, −13 T07 identity
   deletion, +1 T13, +2 T14). Constituents verified:
   - Converged generic_c LINUX pair: **NO re-bless** — O2 discharged
     mechanically (the linux sole binary ran UNBLESSED FIRST in T07 and
     passed; `golden.c`/`expected.stdout` untouched in git).
   - Converged generic_c ZEPHYR pair: re-blessed once (T07) — teardown
     events present (`W32 imsc←0x0`, `W32 cr←0x0`, `W32 icr←0x7ff` at the
     DUT base); register marker now `amba:` metadata (D09).
   - linux_c snapshot e2e green: `golden.linux.c` carries the
     `RSEQ-IGNORED-MATCH` DT audit block; `conformance.linux.json` carries
     `ignored_match_entries` + `on_deinit` delivered.
   - zephyr_c snapshot e2e green: deinit fn + `DEVICE_DT_INST_DEINIT_DEFINE`
     swap + two amba audit blocks at the `DT_DRV_COMPAT` pin;
     `conformance.zephyr.json` carries `on_deinit:CAP-ZEPHYR-DT-DEINIT-1`
     + 2 ignored amba entries.
   - Audit-positive tests replaced every lifted gate's negative (both linux
     scaffolds + zephyr); A2 filtered-table negatives both scaffolds;
     all-foreign loud rejects both backends; deferred-family fail-loud
     guards both backends (the final-review MUST-FIX).
   - Deinit structure tests green (signature, adapter arms in deterministic
     order, macro swap, conditional emission, no-op postamble callout).
   - Capability sync + covers + staleness green with the flipped row
     (D05 in-place, doc mirror same commit as the flip).
   - Identity test GONE (`src/identity.rs`, `tests/composed_identity.rs`,
     `pub mod identity`, input.rs markers — all deleted; suite green
     without them; grep-clean).
   - absentslot + init-fail snapshot e2es green with ZERO re-bless (I05:
     the conditional macro swap left the no-deinit scaffold byte-identical).

2. **L1 (zephyr_build_e2e): PASSED at tip** against the eval v4.4.0
   workspace (`684c9e8f32e4`, SDK 1.0.1) for `qemu_cortex_a53`, converged
   TU, staged prj.conf carrying `CONFIG_DEVICE_DEINIT_SUPPORT=y`. One
   recorded non-gating warning (the known `regseq` vendor-prefix notice).

3. **L2 (eval main, commits 3c70e7f + 9cd1da8):** composed leg —
   `RSEQ-SMOKE: PASS device_is_ready` AND `RSEQ-SMOKE: PASS device_deinit`
   (rc==0, ready→false, authored `LOG_WRN` observed); init-fail leg green
   including the NEW SKIP line. Evidence:
   `pl011/zephyr/results/campaigns/2026-06-10-spec-convergence-teardown/`.

4. **L3 (eval):** TRACE DIFFERENTIAL PASSED — zero unwhitelisted deviations
   over the FULL ordered stream (oracle 18 events, capture 14 MMIO events
   incl. the 3 teardown writes, matched exactly); ZERO teardown whitelist
   entries; whitelist header rewritten recording the class shrink
   ("markers only; MMIO compared"); comparator UNTOUCHED (D11).

5. **Codex final review:** round 1 = FINDINGS-MAJOR (one MUST-FIX: the C06
   audit path covered deferred-family entries outside the D03(a) license —
   fixed in 03bbae5 with `has_ratified_form()` guards + 2 tests); re-review
   = **CLEAN** ("FULL C01.5 is now CLEAN").

**The D12 strengthener (NOT in the gate): landed GREEN** (T13,
`composed_model_equivalence.rs`) — the converged spec bridges to ONE
RegseqModel under both bindings up to the L2 base/size literals
(in-process double bridge, resolved-string comparison via the full pretty
render; no serialized dump format; no drop gate-report needed).

## Open items, all discharged

- **O1** (bridge mixed-key): zero production change; positive test (T01).
- **O2** (linux pair byte-identity): proven unblessed-first (T07).
- **O3** (LOG_WRN flush): flushed inside the 12 s window; NO settle needed
  (T10; the plan-decision-5 fallback unused).
- **O4** (strengthener mechanics): in-process double bridge, landed green.

## Chunk-close Codex review ledger

| chunk | verdict | disposition |
|---|---|---|
| C1 | FINDINGS-MINOR (2 stale comments) | fixed, 1903044 |
| C2/M1 | **CLEAN** | — |
| C3/M2 | FINDINGS-MAJOR (2 doc-claim findings) | fixed 602be06, re-review RESOLVED |
| C4/M3 | FINDINGS-MINOR (archived-evidence path) | fixed, eval 9cd1da8 |
| C5/final | FINDINGS-MAJOR (1 MUST-FIX) | fixed 03bbae5, re-review **CLEAN** |

## Cadence verification (I06)

Commit per task close (12 compiler commits 1d64ce2..03bbae5 + 3 eval
commits 3c70e7f/fb732c1/9cd1da8, eval by explicit path under
`pl011/zephyr/`); cargo green at every commit; `df -h /home` at every
chunk close (152G free throughout); no AI attribution in any commit;
BLESS events exactly the two planned (T02 conformance snapshots, T07 the
converged set) plus zero others; the linux oracle pair never blessed.

## What the successor inherits

`backend/zephyr-c-e1000` is born-converged: the C06/D08 mixed-match rule +
triple-render audit machinery, the OsBinding loader, and the D12
model-equivalence pattern are all in place. The deferred-family fail-loud
guard (03bbae5) bounds the licensed universe it must extend if it ratifies
new forms.
