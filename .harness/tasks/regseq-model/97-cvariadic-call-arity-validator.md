# regseq-model-97: `c_variadic` Call-arity validator exception

## Description

The model's B-4 Call-arity joint validator rejects any call to a `c_variadic`
function that passes more arguments than the fixed-parameter count: it checks
`args.len() != inputs.len()` with **no `c_variadic` exception**. For a variadic
callee, the call legitimately has `args.len() >= inputs.len()` (the fixed inputs
plus the variadic tail), so a valid variadic call is rejected at validation
(Acquire stage), making `c_variadic` unconstructible end-to-end.

Surfaced by the §06 push (b7_fn_sigs). Blocks **FS-5** (`FnSig.c_variadic` → C
`...`).

## Input Context

### Code anchors (confirm at HEAD — per workflow + Codex review)

- `crates/regseq-model/src/model.rs:~3053-3061` — the Call-arity joint validator
  `if args.len() != inputs.len()` (no `c_variadic` branch).
- `crates/regseq-model/src/model.rs:~3063-3070` — the per-arg type check that indexes
  `inputs[i]`; for a variadic call this MUST only run over the fixed-input prefix
  (else it indexes out of range / mistypes the variadic tail args) — Codex review.
- The callee `FnSig` carries `c_variadic: bool` (`fn_sig.rs:~27`, which also requires
  `abi == C` + ≥1 fixed input).
- Matrix FS-5 (`trailing C ...`), FS-1 (`FnSig { ..., c_variadic, ... }`).
- Backend `...` emission already exists (`generic_c/mod.rs:~824-871` for functions,
  `generic_c/types.rs:~284-285` for fn pointers) — Codex confirmed the validator is
  the ONLY blocker; no backend follow-up needed for FS-5.

## Implementation Notes

When the callee `FnSig` is `c_variadic`, relax the arity check to
`args.len() >= inputs.len()` (the fixed inputs must match positionally; the tail is
the variadic args), AND bound the per-arg type check to the fixed-input prefix
(`inputs[0..inputs.len()]`) so it never indexes `inputs[i]` for a variadic tail arg.
Keep the exact-equality check + full per-arg typing for non-variadic callees. Confirm
the callee's `c_variadic` flag is reachable at the Call validation site (via the
callee `FnSigId`); if the joint validator only sees arities, thread the flag.

## Non-Goals

- No backend change unless the §14.5 `...` emission is also incomplete (the matrix
  notes FS-5 emission is "not pinned → confirm in delta"); if the backend can't emit
  the variadic `...` yet, that is a separate backend follow-up — this task removes
  the *validator* blocker so a variadic call reaches emission.
- Not new c_variadic admissibility rules (the existing `abi==C` + ≥1 fixed input
  invariant stays).

## Test Plan

1. Build + workspace green; the regseq-model validator test surfaces
   (incl. `src/model_tests.rs`).
2. Validator unit test: a `c_variadic` callee with `args.len() > inputs.len()`
   passes; a non-variadic callee with a mismatch still fails; a variadic callee with
   `args.len() < inputs.len()` (fewer than the fixed params) still fails.
3. `kind=rust` codegen-coverage fixture for FS-5 (if backend `...` emission is
   ready): a variadic `extern "C"` call that compiles-and-runs → FS-5 green; else
   record the backend `...`-emission follow-up.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-model`, and `regseq-backends`
/ `codegen-coverage-runner` if the fixture lands); single-threaded scoped tests;
ledger byte-stable (if a fixture lands).

## Acceptance Criteria

1. A `c_variadic` callee with extra (variadic) args passes the Call-arity validator;
   non-variadic arity mismatches still rejected.
2. Validator unit test covering the variadic + non-variadic cases passes.
3. FS-5 green via a fixture, OR an explicit recorded follow-up for the backend
   `...` emission if that is the remaining blocker.
4. fmt + clippy (scoped) clean; scoped tests green.
