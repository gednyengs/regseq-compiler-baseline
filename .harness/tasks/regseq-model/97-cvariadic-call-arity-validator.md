# regseq-model-97: `c_variadic` Call-arity validator exception

## Description

The model's B-4 Call-arity joint validator rejects any call to a `c_variadic`
function that passes more arguments than the fixed-parameter count: it checks
`args.len() != inputs.len()` with **no `c_variadic` exception**. For a variadic
callee, the call legitimately has `args.len() >= inputs.len()` (the fixed inputs
plus the variadic tail), so a valid variadic call is rejected at validation
(Acquire stage), making `c_variadic` unconstructible end-to-end.

Surfaced by the §06 push (b7_fn_sigs). This is the **model-side** half of **FS-5**.

> **Pilot finding (necessary-but-not-sufficient).** Implementing this fix revealed a
> SECOND, duplicate B-4 arity check in the **bridge** (`regseq-driver`
> `bridge/body/materialise.rs:589`, no `c_variadic` exception) that rejects a variadic
> call *before* this model validator runs — plus the in-fixture variadic body trips
> `DslRejection { CVoid }` and the `extern`-variadic alternative hits the foreign-symbol
> rename (the IN-3 / `regseq-driver-84` bug). So FS-5 greens end-to-end only after
> **`regseq-driver-86`** (the bridge B-4 `c_variadic` exception + a constructible
> variadic-fixture path). This task delivers the model-side validator fix; FS-5 is
> deferred to that follow-up. (The backend `...` emission already works — confirmed.)

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

1. A `c_variadic` callee with extra (variadic) args passes the model Call-arity
   validator; non-variadic arity mismatches still rejected.
2. Validator unit tests covering the variadic + non-variadic cases pass.
3. fmt + scoped tests green; scoped clippy introduces no NEW warnings (the pre-existing
   regseq-model test-target drift, [[clippy-all-targets-test-drift]], is not gated).
4. FS-5 is DEFERRED to `regseq-driver-86` (the remaining blocker is the bridge B-4
   validator + the variadic-fixture path, NOT this model fix) — recorded, not faked.
   This task's deliverable is criteria 1-3 (the model-side validator).
