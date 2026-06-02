# regseq-driver-90: `cargo-regseq --emit bin` exit-adapter parity (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (task **T15**),
> contract **C04**, requirement **R10**, invariant **I02**.

## Description

Bring the **second** consumer of the entry shim — `cargo-regseq --emit bin` — to
parity with the codegen-coverage runner. `emit.rs::Bin` consumes the entry-aware
shim shape (regseq-backends-27) so an error-returning entry gets the int shim
(`int main(void){ return <entry>__regseq_exit(); }`); the adapter itself is already
in the TU from `emit_generic_c` (regseq-backends-28). A built binary's exit code
then maps per C02.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-driver/src/emit.rs` (~:66/:70/:88) — `Bin`: today it re-resolves
  the entry **after** TU emit and calls `generate_main_c`. Switch it to take the
  shim shape returned by the entry-aware emit API (regseq-backends-27).
- `regseq-driver-88` (`--emit bin` runnable binary) — the precedent this extends.
- Existing emit tests (`generic_c_emit.rs` / `emit_seam.rs` patterns).

## Implementation Notes

- Same backend single-source-of-truth as the runner — `emit.rs` must **not**
  re-derive the shim shape.
- The adapter symbol is `extern`-declared in the shim and defined **once** in the
  TU (no duplicate/undefined symbol at link).
- Unit-void `--emit bin` output **unchanged**.

## Non-Goals

- No backend adapter changes (regseq-backends-28).
- No new oracle (reuses the regseq-backends-29 exit semantics conceptually).

## Test Plan

1. `--emit bin` of an error-returning entry → a binary whose **exit code** maps per
   C02 (compile + run + assert).
2. Unit-void `--emit bin` unchanged (regression).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`cargo clippy -p regseq-driver --
-D warnings`); `RUST_TEST_THREADS=1 cargo test -p regseq-driver --
--test-threads=1`.

## Acceptance Criteria

1. Both consumers emit the same shim shape from the same backend SoT; `--emit bin`
   of an error-returning entry produces a binary whose exit maps per C02
   (**C04/R10**).
2. Adapter `extern`-declared in shim, defined once in TU (clean link).
3. Unit-void `--emit bin` unchanged (**I02**).
4. fmt + scoped clippy clean; `regseq-driver` tests green.
