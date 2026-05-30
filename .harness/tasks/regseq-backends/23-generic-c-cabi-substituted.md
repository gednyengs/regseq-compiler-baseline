# regseq-backends-23: Generic-C `CAbiShape::Substituted` transparent-newtype boundary (Phase-1)

## Description

A `#[repr(transparent)] struct W(u32)` passed across an `extern "C"` boundary
computes (correctly, in the bridge) `CAbiShape::Substituted { emit_as: u32, reason:
TransparentChain }` — the position should be emitted at the C boundary using
`emit_as`'s natural C repr (`uint32_t`), not the original `W` struct. The backend
does not honor the substitution, producing wrong/invalid C at the boundary.

Surfaced by the §06 push (b7_fn_sigs). Blocks **FS-8** (`CAbiShape::Substituted`)
and **FS-9** (`SubstReason` — diagnostic-only, rides on FS-8's emission).

## Input Context

### Code anchors (confirm at HEAD — per workflow diagnosis)

- The C-ABI signature emission reads `model.sig_c_abi_lowerings[FnSigId]`
  (`CAbiLowering { inputs, output }`); for a `Substituted` position it must emit
  `emit_as`'s natural C repr, not the original `TyId`'s. Grep the signature /
  parameter emission in `crates/regseq-backends/src/generic_c/` for where the
  per-position `CAbiShape` is consulted (FS-7 Direct works; FS-8 Substituted does
  not).
- Matrix FS-8 (`emit `emit_as`'s natural C repr at the boundary, fully composed`),
  FS-9 (diagnostic-only).
- Spec §14.5 (C-ABI admissibility + `CAbiShape`).

## Implementation Notes

At an `FnAbi::C` position whose `CAbiShape` is `Substituted { emit_as, .. }`, emit
`emit_as`'s C type (and compose any further substitution) for both the prototype
and the call site, while the body still sees the original `TyId`. Confirm the
caller and callee agree (both read `emit_as`) so the emitted call type-checks.

## Non-Goals

- No change to FS-7 (Direct) which already works.
- Not the C-ABI admissibility decision itself (bridge-side; the backend trusts the
  `emit_as` certificate).

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture: `extern "C" fn id_w(w: W) -> u32 { w.0 }`
   with `#[repr(transparent)] struct W(u32)`, called from `regseq_main` with the
   result logged, compiles-and-runs → FS-8 (+FS-9) green; re-run the runner, ledger
   byte-stable.
3. Backend unit test: a `Substituted` input/output position emits `emit_as`'s C
   repr in the signature.

## Quality Gate

`cargo fmt --all -- --check`; `cargo clippy -p regseq-backends -p codegen-coverage-runner
--all-targets -- -D warnings` (scoped); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture passing a `repr(transparent)` newtype across `extern "C"`
   compiles-and-runs; FS-8 + FS-9 green in the ledger.
2. The emitted prototype + call site use `emit_as`'s C repr and type-check.
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
