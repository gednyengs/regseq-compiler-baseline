# regseq-backends-36: RegseqError e2e fixture 3 — `?` + match/reclassify → ETIMEDOUT (Phase-2a)

> Source: the third RegseqError e2e fixture from the failed `regseq-backends-30`,
> now buildable via the same-type `?` lowering (`regseq-driver-92`). Decision
> **D07**, requirements **R01/R02**, invariant **I01**.

## Description

Land the RegseqError fixture that weaves authored control flow: a nested helper
returning `Result<(), RegseqError>`, **`?`-propagated**, plus a `match` that
**reclassifies** one variant to another before escaping — asserting the escaping
variant's errno at process exit. This proves that authored-internal `?` / `match` /
reclassify lower as ordinary code (only the boundary value is mapped, R02/I01), now
that `?` is supported (same-error-type, `regseq-driver-92`).

## Input Context

### Code anchors (confirm at HEAD)

- `regseq-driver-92` — the same-type `Result`-`?` normalizer this fixture depends on;
  the `?` here is on `Result<(), RegseqError>` inside a fn returning the **same**
  `Result<(), RegseqError>` (same-error-type → in scope; no `From` conversion).
- `regseq-backends-35` — the fixtures-1&2 infra + the combined RegseqError coverage
  row this fixture extends.
- The symbolic `expected.exit` oracle (`regseq-backends-29`); the `corpus_e2e.rs`
  isolated pattern.

## Implementation Notes

- Fixture shape: a helper `fn probe(...) -> Result<(), RegseqError>`; `regseq_main`
  does `probe(...)?;` then `match probe(...) { Err(RegseqError::DeviceBusy) =>
  Err(RegseqError::Timeout), other => other }` (reclassify `DeviceBusy → Timeout`);
  `expected.exit = "ETIMEDOUT"`.
- Same `RegseqError` self-declaration + C01/I06/I07 conformance as
  `regseq-backends-35`. Bless the golden; extend the combined coverage row's `covers`.
- This fixture's expected exit (`ETIMEDOUT`) is **nonzero**, so it relies on the
  `bless_fixture`-honors-`expected.exit` fix landed in `regseq-backends-35` (without
  it, `--bless` would spuriously fail on the nonzero exit).

## Non-Goals

- No cross-type `?` / `From` conversion / `Option`-`?` (out of scope; `regseq-driver-92`
  fails those loud).
- No new oracle/adapter/classifier work.

## Test Plan

1. Run the fixture e2e: bridge (the `?` lowers via `regseq-driver-92`) → emit → `cc` →
   run → assert exit `ETIMEDOUT` + empty stderr.
2. Confirm authored-internal `?`/`match`/reclassify are unmapped — only the escaping
   boundary value maps (**R02/I01**).
3. Golden byte-stable; gate `pass`; coverage row updated.

## Quality Gate

`cargo fmt --all -- --check`; `RUST_TEST_THREADS=1 cargo test
-p codegen-coverage-runner -- --test-threads=1`; `cargo run -p
codegen-coverage-runner` → `pass`; ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. The `?`/reclassify→`ETIMEDOUT` fixture bridges, emits, runs, and asserts its exit
   code end-to-end (**R01**, **D07**).
2. Authored-internal `?`/`match`/reclassify lower as ordinary code; only the boundary
   value is mapped (**R02/I01**).
3. Golden byte-stable; gate `pass`; coverage row extended; fmt clean.
