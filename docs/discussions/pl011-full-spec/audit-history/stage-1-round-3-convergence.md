**Q1**

Yes. `line_control_value` now validates and computes `LCR_H` without writes, returning `Unsupported` only for `DataBits::Nine` and non-`None` flow control: `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:174-200`. `write_line_control` only writes a precomputed `FieldValue` and has no fallible path: `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:203-215`.

Both lifecycle callers validate before disabling: `bring_up_port` calls `line_control_value` before `disable_uart` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:284-288`, and `reprogram_line_format` does the same at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:311-315`. The valid path still disables, waits, writes divisors, writes `LCR_H`, then enables at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:317-320`, matching the PL011 disable/reprogram ordering in `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-minimized-datasheet.md:839-840` and `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-minimized-datasheet.md:1039-1042`.

**Q2**

| REQ-ID | verdict | fixture-cite | delta-from-round-2 |
|---|---|---|---|
| REQ-CFG-03 | captured-correctly | `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:174-200`; `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:487-492` | Cite refreshed: old combined line-control helper is now pure validation plus later write; verdict unchanged from `/tmp/pl011-round1-codex.out:13`. |
| REQ-CFG-04 | captured-correctly | `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:291-297`; `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:317-320` | Cite refreshed; valid configs still write divisors before `LCR_H` and before enable, so no regression from `/tmp/pl011-round1-codex.out:14`. |
| REQ-LIFE-02 | captured-correctly | `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:311-320` | Fixed: Round 2 was `captured-with-issue` because unsupported configs could return after disable; validation now happens before disable, closing `/tmp/pl011-round1-codex.out:19` and `/tmp/pl011-round1-codex.out:101`. |
| REQ-CORR-REGORDER-01 | captured-correctly | `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:291-294`; `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:317-320` | Cite refreshed for helper split; no regression from `/tmp/pl011-round1-codex.out:37`. |
| REQ-CORR-REGORDER-02 | captured-correctly | `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:291-297`; `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:317-320` | Cite refreshed for helper split; no regression from `/tmp/pl011-round1-codex.out:38`. |

All other Stage 1 REQs listed in `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-spec-requirements.md:67-77` are no change from the Round 2 verdicts in `/tmp/pl011-round1-codex.out:5-18` and `/tmp/pl011-round1-codex.out:20-44`. No previously-green REQ regressed, and I found no new Stage 1 fixture issue.

**Q3**

The status doc update is accurate for fixture-level Stage 1. The “Already captured” section now includes the previously stale items: `REQ-CFG-03/04`, `REQ-IRQ-03/04`, `REQ-CORR-RESERVED-01`, and `REQ-CORR-MASK-SHADOW` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-full-spec-status.md:32-47`. The reserved-bit rationale is now recorded as framework-level capture at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-full-spec-status.md:49-58`, matching the lowering distinction between `RegWrite` and `RegModify` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:47-65`.

The “Scheduled fixture work for Stage 1” section is truly empty: it says no Rust DSL fixture changes remain after Round 3 at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-full-spec-status.md:60-63`. The remaining Linux/Zephyr items are explicitly backend-emission work, not fixture work, at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-full-spec-status.md:65-79`.

**Q4**

stage-1-complete

The Round 2 REQ-LIFE-02 regression is closed by pre-validating `SerialConfig` before any disable/wait/write sequence, while preserving the required divisor then `LCR_H` then enable order for valid configs. The status doc now matches the fixture state, and the only Stage 1 items still called out are backend-emission-level Linux/Zephyr work, not fixture gaps.

CONVERGENCE SIGNAL: converged
