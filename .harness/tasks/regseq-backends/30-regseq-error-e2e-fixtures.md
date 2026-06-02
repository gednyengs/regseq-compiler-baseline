# regseq-backends-30: RegseqError e2e fixtures (Ok→0, EINVAL, `?`/match-reclassify) (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (tasks **T11+T12**
> + the M1 EINVAL slice), decision **D07**, contract **C01**, requirements
> **R01/R02/R04**, invariants **I01/I06/I07/I09**.

## Description

The end-to-end fixtures that prove the pipeline + authored-flow surface on the
host. Each self-declares an attribute-marked `RegseqError` (D06) and drives
`regseq_main` through bridge → model → emit → `cc` → run → assert **process exit
code** (the regseq-backends-29 oracle). The full 9-row table is already covered by
the regseq-backends-26 unit test, so only a handful of fixtures are needed (D07).

## Input Context

### Code anchors (confirm at HEAD)

- `crates/codegen-coverage-runner/fixtures/` — fixture layout (`input.rs`,
  `manifest.toml`, blessed `golden.c`); `b4_enums` as the enum precedent.
- `crates/codegen-coverage-runner/fixtures/diag_levels/input.rs` — the
  `#![no_std]` + `#![register_tool(rseq)]` + inline `mod` convention to mirror
  for declaring the tool attribute and the type.
- The `corpus_e2e.rs` isolated-process snapshot pattern (memory
  [[golden-c-emit-process-determinism]]).
- Oracle field `expected.exit` from regseq-backends-29.

## Implementation Notes

Three fixtures:
1. **Ok→0:** `regseq_main() -> Result<(), RegseqError> { Ok(()) }`; `expect = pass`,
   `expected.exit = "0"` (or omitted), empty `expected.stdout`.
2. **EINVAL (the M1 slice):** `… { Err(RegseqError::InvalidParameter) }`;
   `expected.exit = "EINVAL"`.
3. **`?` + match/reclassify:** a nested helper returning `Result<(),RegseqError>`,
   propagated with `?`, then a `match` reclassifying one variant to another (e.g.
   `DeviceBusy → Timeout`) before escaping; `expected.exit = "ETIMEDOUT"`.

The declared `RegseqError` MUST NOT implement `Display`/`std::error::Error` and the
carrier is `core::result::Result` (**I06/I07**) — verify in fixture review. Bless
goldens; emitted C stays symbolic.

## Non-Goals

- No per-variant e2e fixtures (table is unit-tested in regseq-backends-26).
- No negative fixtures (regseq-backends-31).

## Test Plan

1. Run each fixture e2e: compile + run + assert exit code (+ empty stderr for the
   nonzero cases); stdout matches (empty).
2. `?`/match/reclassify lower through the existing body emitter as ordinary MIR
   (no special handling) — only the boundary value is mapped (**R02/I01**).
3. Goldens byte-stable; gate `pass`.

## Quality Gate

`cargo fmt --all -- --check`; `RUST_TEST_THREADS=1 cargo test
-p codegen-coverage-runner -- --test-threads=1`; `cargo run -p
codegen-coverage-runner` → `pass`; ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. Ok→0, EINVAL, and the `?`/reclassify→ETIMEDOUT fixtures run and assert their
   exit codes end-to-end (**R01/R04**, **D07**).
2. Authored-internal `?`/match/reclassify is unmapped; only the boundary is
   (**R02/I01**).
3. Each fixture's `RegseqError` conforms to C01 (no `Display`/`Error`;
   `core::result::Result`) (**C01/I06/I07**).
4. Goldens byte-stable; gate `pass` (**I09**); fmt clean.
