# regseq-backends-29: Harness symbolic `expected.exit` oracle (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (task **T09**),
> contract **C04**, decision **D03**, requirements **R10/R11**, invariants **I02/I03**.

## Description

Give the codegen-coverage runner an oracle for a **specific** process exit code,
expressed **symbolically** and resolved on the fixture's **own** libc — so error
mapping is verified end-to-end without host-libc coupling and without storing any
numeric in a manifest or golden.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/codegen-coverage-runner/src/manifest.rs` (~:20/:25) — add optional
  symbolic `expected.exit: Option<String>` (an errno identifier, or `"0"`;
  default `0`).
- `crates/codegen-coverage-runner/src/toolchain.rs` (~:60/:62/:81) — resolve the
  symbol → numeric on the **same `cc`/libc** used to compile + run the fixture
  (recommended: a tiny probe TU that prints the constant, O3c).
- `crates/codegen-coverage-runner/src/pipeline.rs` (~:175/:178) — pass mode is
  currently "exit == 0"; generalize to "exit == resolved(`expected.exit`)".
- `crates/codegen-coverage-runner/src/oracle.rs` (~:31) — keep stdout
  byte-compared; add the empty-stderr guard for a nonzero expected exit.
- `crates/codegen-coverage-runner/.../corpus_e2e.rs` — the isolated-process test
  pattern (memory [[golden-c-emit-process-determinism]]).

## Implementation Notes

- **Absent field ⇒ exit-0 semantics unchanged** (existing fixtures untouched).
- Nonzero expected ⇒ `exit_code == Some(resolved)` **AND empty stderr** — so a
  `panic=abort` SIGABRT (nonzero + stderr) cannot masquerade as a clean errno
  return.
- `expect = panic` oracle is **unchanged**. No numeric stored anywhere.

## Non-Goals

- Not the fixtures (regseq-backends-30) or negatives (regseq-backends-31).
- Not `--emit bin` (regseq-driver-90) — though that consumer reuses these semantics.

## Test Plan

1. Oracle/toolchain unit tests (string in → verdict out): matching exit; mismatched
   exit; nonzero exit with non-empty stderr (must fail).
2. `manifest.rs` parse test for the new symbolic field.
3. Resolve a known symbol (e.g. `ETIMEDOUT`) on the toolchain → a sane numeric.
4. Existing fixtures + gate unchanged; ledger byte-stable.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`codegen-coverage-runner`);
`RUST_TEST_THREADS=1 cargo test -p codegen-coverage-runner -- --test-threads=1`;
gate stays `pass`; ledger byte-stable (runner twice + diff).

## Acceptance Criteria

1. Optional symbolic `expected.exit`; resolved on the fixture's own libc; **no
   numeric** in any manifest/golden (**C04/I03/R10**).
2. Nonzero ⇒ `exit == resolved` **AND empty stderr**; `expect = panic` unchanged
   (**R11**).
3. Absent field preserves exit-0 semantics; existing fixtures unchanged (**I02**).
4. fmt + scoped clippy clean; tests green; gate `pass`; ledger byte-stable.
