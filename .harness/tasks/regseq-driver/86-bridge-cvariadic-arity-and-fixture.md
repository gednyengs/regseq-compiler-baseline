# regseq-driver-86: Bridge Call-arity `c_variadic` exception + variadic-fixture path (greens FS-5)

## Description

Greening **FS-5** (`FnSig.c_variadic` → C `...`) end-to-end needs the bridge-side
half that the `regseq-model-97` pilot surfaced (the model-side validator fix landed
in that task). Two blockers, both in `regseq-driver`:

1. **Duplicate B-4 arity check in the bridge.** `JointValidationVisitor` re-checks
   Call arity with `sig.inputs.len() != args.len()` and **no `c_variadic` exception**,
   so a variadic call is rejected at the bridge BEFORE the (now-fixed) model validator
   runs. Verified at `crates/regseq-driver/src/bridge/body/materialise.rs:589`.
2. **No constructible variadic fixture surface.** A Rust-defined variadic body
   (`extern "C" fn f(a, b, mut _rest: ...)`) trips `DslRejection { CVoid }` (the
   `...`/`VaListImpl` drags in `core::ffi::c_void`, which bridge type-translation
   rejects). The clean alternative — an `extern "C" { fn snprintf(...); }` libc
   declaration — emits correctly (the backend `...` prototype + call were CONFIRMED
   working: `extern int32_t input__extern_0__snprintf(..., ...)` + the variadic call
   site), but fails at LINK because the foreign callee is renamed (the IN-3 /
   `regseq-driver-84` foreign-symbol bug) → undefined reference.

## Input Context

### Code anchors (confirmed at HEAD — pilot)

- `crates/regseq-driver/src/bridge/body/materialise.rs:589` — the bridge
  `JointValidationVisitor` B-4 arity check (mirror the model fix: relax to
  `args.len() >= inputs.len()` for a `c_variadic` callee).
- `crates/regseq-model/src/model.rs:~3058-3085` — the model-side fix
  (`regseq-model-97`) to mirror: `>=` for variadic + the fixed-input-prefix bound.
- The `DslRejection { CVoid }` path (bridge type-translation rejecting
  `core::ffi::c_void` reached via `...`/`VaListImpl`) — grep the rejection in
  `crates/regseq-driver/src/bridge/`.
- `regseq-driver-84` (foreign-fn ABI symbol) — its fix is the prerequisite for the
  `extern`-variadic-declaration fixture path to link.

## Implementation Notes

1. Add the `c_variadic` exception to the bridge B-4 check (`materialise.rs:589`),
   matching `regseq-model-97`'s model-side relaxation (consistency across the two
   validators is the point — they must agree).
2. Land a constructible FS-5 fixture surface: the recommended path is an
   `extern "C" { fn snprintf(buf: *mut u8, n: usize, fmt: *const u8, ...); }`
   declaration that links once `regseq-driver-84` keeps the real ABI symbol — call it
   from `regseq_main` with an extra arg and observe a deterministic result. If that
   path is not viable, find another that does not trip the `c_void` rejection.

## Non-Goals

- The model-side validator fix (`regseq-model-97`, done).
- The backend `...` emission (already works — confirmed).
- A general fix for the `c_void` rejection beyond what FS-5 needs (note it if broader).

## Test Plan

1. Build + workspace green; bridge/model validator tests stay green.
2. A `kind=rust` codegen-coverage fixture exercising a `c_variadic` call
   compiles-AND-runs (links) and logs a deterministic result → re-run
   `cargo run -p codegen-coverage-runner`: FS-5 flips green, >=19 fixtures green / 0
   failed (no regression), ledger byte-stable.
3. Bridge unit test: the `JointValidationVisitor` B-4 check admits a `c_variadic`
   callee with extra args and still rejects non-variadic mismatches.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-model`,
`codegen-coverage-runner`) — no NEW warnings ([[clippy-all-targets-test-drift]]);
single-threaded scoped tests; ledger byte-stable.

## Acceptance Criteria

1. The bridge B-4 arity check admits `c_variadic` callees (consistent with the model
   validator); non-variadic mismatches still rejected.
2. A `kind=rust` fixture with a variadic call links-and-runs; FS-5 green in the ledger.
3. fmt + clippy (scoped, no new warnings) clean; scoped tests green; ledger byte-stable.
