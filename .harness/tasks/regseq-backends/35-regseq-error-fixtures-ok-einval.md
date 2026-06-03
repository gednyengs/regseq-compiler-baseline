# regseq-backends-35: RegseqError e2e fixtures 1 & 2 (Ok→0, EINVAL) + coverage rows (Phase-2a)

> Source: rescope of the failed `regseq-backends-30` (archived) — the `?`-using
> fixture 3 is split out (`regseq-backends-36`, gated on the `?` work
> `regseq-driver-92`). These two fixtures use **no `?`**, so they land **now** and
> give RegseqError its first real codegen-coverage e2e. Folds the O2
> ledger/matrix-row wiring (the archived `regseq-backends-32`). Decision **D07/D06**,
> contract **C01/C04**, requirements **R01/R04**, invariants **I01/I06/I07/I09**.

## Description

Land the two RegseqError e2e fixtures the failed `-30` validated but never committed
(it failed only on fixture 3's `?`), and wire their codegen-coverage matrix rows so
the gate aggregates them. Each fixture self-declares an attribute-marked
`RegseqError` (D06) and drives `regseq_main` through bridge → model → emit → `cc` →
run → assert the **process exit code** (the `regseq-backends-29` symbolic oracle).

## Input Context

### Code anchors (confirm at HEAD)

- `crates/codegen-coverage-runner/fixtures/` — fixture layout (`input.rs`,
  `manifest.toml`, blessed `golden.c`); `diag_levels/input.rs` for the
  `#![register_tool(rseq)]` + inline-`mod` convention; `b4_enums` for enum prior art.
- The `corpus_e2e.rs` isolated-process snapshot pattern (memory
  [[golden-c-emit-process-determinism]]).
- The symbolic `expected.exit` oracle (`regseq-backends-29`,
  `codegen-coverage-runner/src/{manifest,oracle,toolchain,pipeline}.rs`).
- The coverage matrix/ledger + Doc-1 matrix markdown (the O2 row taxonomy).
- The certified entry classifier (`regseq-backends-34`) — these fixtures exercise it.
- **`--bless` exit handling:** `crates/codegen-coverage-runner/src/pipeline.rs:330`
  (`bless_fixture`) currently flags any **nonzero** exit as not-OK, and
  `crates/codegen-coverage-runner/src/main.rs:108` exits 1 on a not-OK bless — so the
  `EINVAL` (nonzero) fixture cannot be blessed cleanly. `run_fixture`/`oracle.rs:31`
  already honor `expected.exit`; `bless_fixture` must too.
- Recorded design relaxations to reference: **G01** is already committed in
  `docs/discussions/generic-c-backend/roadmap.md:121`; **D06** (no-facade /
  attribute-identified `RegseqError`) is at GRILL `…233339.md:264`.

## Implementation Notes

Two fixtures (no `?`):
1. **Ok→0:** `regseq_main() -> Result<(), RegseqError> { Ok(()) }`; `expect = pass`,
   `expected.exit = "0"` (or omitted), empty `expected.stdout`.
2. **EINVAL:** `... { Err(RegseqError::InvalidParameter) }`; `expected.exit =
   "EINVAL"`.

Each fixture's `RegseqError` MUST NOT implement `Display`/`std::error::Error` and uses
`core::result::Result` (**I06/I07**). Bless goldens; emitted C stays symbolic.
**O2:** add one combined coverage-matrix row ("RegseqError boundary mapping +
exit-code propagation") and wire both fixtures' `covers`; keep the gate `pass` and
the ledger byte-stable across two runs.

**`--bless` fix (folded from archived `-32`'s scope):** make `bless_fixture` honor
`manifest.expected.exit` (mirroring `run_fixture`) so the nonzero-exit `EINVAL`
fixture blesses cleanly instead of failing the bless run; this also unblocks the
`ETIMEDOUT` fixture (`regseq-backends-36`).

**Conformance note (folded from archived `-32`):** record the design relaxations the
RegseqError generic-C phase relies on — **D06** (identity is the
`#[rseq::framework_type("regseq-error")]` attribute, NOT a facade crate; fixtures
self-declare the enum inline) and **G01** (the Phase-3+ guardrail). G01 is already in
`roadmap.md`; either add a short D06 note where the gate docs live or cite the GRILL
`…233339.md:264` D06 decision as the authoritative record. No behavior change.

## Non-Goals

- **No `?`** — fixture 3 (`?`/reclassify→ETIMEDOUT) is `regseq-backends-36`.
- No per-variant e2e fixtures (the 9-row table is unit-tested in `regseq-backends-26`).
- No negatives (those are `regseq-backends-31`, done).

## Test Plan

1. Run both fixtures e2e: compile + run + assert exit (0 / `EINVAL`); stdout empty;
   `EINVAL` also requires empty stderr (the R11 guard).
2. `--bless` of the nonzero `EINVAL` fixture succeeds (writes `golden.c`, exits 0) —
   proving `bless_fixture` honors `expected.exit`.
3. `cargo run -p codegen-coverage-runner` → `pass`; the new row is attributed by the
   two fixtures; ledger byte-stable across two runs.
4. Existing fixtures/goldens byte-identical (**I02/I09**).

## Quality Gate

`cargo fmt --all -- --check`; `RUST_TEST_THREADS=1 cargo test
-p codegen-coverage-runner -- --test-threads=1`; `cargo run -p
codegen-coverage-runner` → `pass`; ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. `Ok→0` and `EINVAL` fixtures run and assert their exit codes end-to-end
   (**R01/R04**, **C04**, **D07**).
2. Each fixture's `RegseqError` conforms to C01 (no `Display`/`Error`;
   `core::result::Result`) (**C01/I06/I07**).
3. One combined RegseqError coverage row added + attributed; gate `pass`; ledger
   byte-stable (**I09**, O2); emitted C symbolic/deterministic (**I03**).
4. `bless_fixture` honors `expected.exit` — the nonzero `EINVAL` fixture blesses
   without a spurious failure.
5. The D06 (no-facade / attribute identity) + G01 conformance relaxations are
   recorded (or cited to their committed home).
6. Existing goldens byte-identical (**I02**); fmt clean.
