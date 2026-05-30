# regseq-backends-23: Generic-C `CAbiShape::Substituted` call-side + body marshalling (Phase-1)

## Description

A `#[repr(transparent)] struct W(u32)` across an `extern "C"` boundary computes
`CAbiShape::Substituted { emit_as: u32, reason: TransparentChain }`. The **signature**
emission already honors `emit_as` (prototype + definition param/return use the ABI
type). The remaining defects (Codex spec review — the original "signatures ignore
emit_as" diagnosis was stale) are on the **call side** and in the **body**:

1. **Call-side argument types ignore the substitution.** The callee-param-types
   helper returns the raw `FnSig.inputs` (the `W` wrapper), so a direct call lowers
   its arguments at the wrapper type, not `emit_as` — diverging from the prototype.
2. **Definition body/param-local divergence.** A definition declares ABI-effective
   parameter types (`uint32_t`) but the body still uses the original body locals /
   projections (`W`, `w.0`), so a transparent-newtype body does not type-check against
   its own ABI-substituted parameters — there is no marshalling between the ABI param
   and the body local.

Surfaced by the §06 push (b7_fn_sigs). Blocks **FS-8** (`CAbiShape::Substituted`) and
**FS-9** (`SubstReason` — diagnostic-only, rides on FS-8's fixture; the backend ignores
`reason` and emits `emit_as`, so no separate construct).

## Input Context

### Code anchors (confirm at HEAD — per Codex review)

- `crates/regseq-backends/src/generic_c/mod.rs:~758-779`, `~843-878` — signature
  emission ALREADY uses `emit_as` for C-ABI positions (so the prototype/decl is right).
- `crates/regseq-backends/src/generic_c/mod.rs:~1260-1279` — the call-side helper
  returning raw `FnSig.inputs` (the `callee_param_tys` gap: ignores substituted
  effective input types).
- `crates/regseq-backends/src/generic_c/body_emit.rs:~97-115`, `~955-957` — function
  body uses original body-local types / projections (the marshalling gap).
- `model.sig_c_abi_lowerings[FnSigId]` (`CAbiLowering { inputs, output }`) — the
  per-position `CAbiShape`. Spec §14.5.

## Implementation Notes

A coherent ABI-vs-body strategy for `Substituted` positions, both inputs and return:

- **Call side:** lower each argument at the position's effective ABI type (`emit_as`),
  composing any chain — so the emitted call matches the prototype.
- **Definition:** declare each substituted parameter as `emit_as` (already done for the
  signature) AND marshal it into the original-typed body local at the prologue (for a
  `repr(transparent)` chain this is a representation-identical copy/reinterpret); marshal
  the return value back to `emit_as` at each `return`. The body keeps using the original
  `TyId` locals/projections; marshalling bridges the boundary.

Pick the minimal mechanism consistent with the existing emit (e.g. a typed copy at the
prologue / return), and make caller and callee agree.

## Non-Goals

- No change to FS-7 (Direct), which already works.
- Not the C-ABI admissibility decision (bridge-side; the backend trusts the `emit_as`
  certificate).
- Not non-transparent substitutions beyond what `emit_as` already encodes.

## Test Plan

1. Build + scoped tests green.
2. A `kind=rust` codegen-coverage fixture: `#[repr(transparent)] struct W(u32);
   extern "C" fn id_w(w: W) -> u32 { w.0 }` called from `regseq_main` with the result
   logged (and a `W`-returning extern "C" fn for the return path), compiles-and-runs →
   FS-8 (+FS-9) green; re-run the runner, ledger byte-stable.
3. Backend unit test: a `Substituted` input AND output position — prototype, call
   site, and definition body all type-check (the marshalling is emitted).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`, `codegen-coverage-runner`);
single-threaded scoped tests; ledger byte-stable.

## Acceptance Criteria

1. A `kind=rust` fixture passing AND returning a `repr(transparent)` newtype across
   `extern "C"` compiles-and-runs; FS-8 + FS-9 green in the ledger.
2. Prototype, call-site argument types, and definition body all agree on `emit_as` at
   substituted positions (with marshalling where the body uses the original type).
3. fmt + clippy (scoped) clean; scoped tests green; ledger byte-stable.
