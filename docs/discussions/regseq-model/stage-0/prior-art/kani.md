# Kani

**One-line summary**: Bounded model checker for Rust that compiles MIR to
CBMC's GOTO-program format via a custom rustc codegen backend.

**URLs**:
- Repo: https://github.com/model-checking/kani
- Docs: https://model-checking.github.io/kani/
- Internal IR access via `rustc_public` (Stable MIR):
  https://github.com/rust-lang/rust/tree/master/compiler/rustc_smir

## Pipeline shape

- Source language: plain Rust + `kani::` proof-harness attributes.
- Extraction point in rustc: post-monomorphization MIR, accessed through the
  **Stable MIR** API (`rustc_public::mir::Body`,
  `rustc_public::mir::mono::Instance`). See
  `kani-compiler/src/codegen_cprover_gotoc/codegen/function.rs`.
- Output: GOTO programs (CBMC's IR), serialized as either a JSON symbol table
  or, since v0.40-ish, directly as a GOTO binary.
- How they invoke rustc: Kani implements a full `CodegenBackend`
  (`GotocCodegenBackend` in
  `kani-compiler/src/codegen_cprover_gotoc/compiler_interface.rs`). The
  `kani-compiler` binary registers itself as Rust's codegen backend and
  replaces LLVM. Driven from `kani-compiler/src/main.rs` and
  `kani_compiler.rs`. The compiler may be re-invoked multiple times to apply
  stubs (see comments in `kani_compiler.rs`).

## IR design

- They do **not** define a custom owned IR. The "IR" Kani emits is CBMC's
  `goto_program` (from the `cbmc` Rust crate they vendor). Their own
  `GotocCtx` is a `'tcx`-lifetimed translation context that walks Stable MIR
  bodies and emits `cbmc::goto_program::Stmt`/`Expr`/`Symbol`.
- Shape relative to MIR: GOTO is closer to a low-level imperative C-like IR
  than MIR. Translation is per-statement, per-terminator, per-rvalue, per-place
  (see files `statement.rs`, `block.rs`, `rvalue.rs`, `operand.rs`, `place.rs`,
  `typ.rs` under `codegen_cprover_gotoc/codegen/`).
- Notable types: `MachineModel`, `SymbolTable`, `Location`, `InternedString`
  in the `cbmc` crate; `GotocCtx`, `MinimalGotocCtx` on Kani's side.
- Serialization: GOTO binary or JSON symbol table; consumed by `cbmc` and
  `goto-cc` from the CProver suite. Kani also has a separate
  `codegen_aeneas_llbc` backend (feature-gated) that emits LLBC.

## Key design decisions

- **Monomorphization**: Kani relies on rustc to monomorphize *before*
  translation. They iterate `MonoItem`s collected by their reachability pass
  (`kani_middle/reachability.rs`, `codegen_units.rs`). Each `Instance` is a
  monomorphic, fully-substituted function.
- **`core` / `std` treatment**: walked, not stubbed by default. Kani ships
  partial std model overrides via a "stubbing" mechanism
  (`kani_middle/stubbing/`) -- specific symbols can be replaced by Kani-provided
  models (e.g. for nondeterministic primitives, intrinsics, alloc).
- **Drop handling**: drop terminators are codegen'd as calls to drop-glue
  functions; rustc has already elaborated drops by the time we see optimized
  MIR.
- **Panic strategy**: panic = abort. `PanicStrategy::Abort` is enforced; calls
  to `panic_fmt` etc. are translated to GOTO `assert(false)` so CBMC reports
  them as failed properties.
- **Unsupported constructs**: tracked through a structured
  `UnsupportedConstructs` map keyed on a stable string and a `Location`;
  Kani's middle layer also has a `check_reachable_items`/`check_crate_items`
  pass that pre-rejects constructs it can't model. Generally non-fatal:
  unsupported items become CBMC stubs that fail verification rather than
  aborting compilation.
- **Reference rules**: `&T` / `&mut T` lower to GOTO pointers; CBMC has no
  borrow semantics so aliasing must be respected by Kani's translation.
  Recent work has added MIR-level alias analysis passes.
- **Trait dispatch / vtables**: static post-mono dispatch for impls; `dyn
  Trait` is encoded with explicit vtable structs in CBMC -- standard.

## Test methodology

Multi-tier suites under `tests/`:
- `tests/kani/` -- per-feature `.rs` files run via Kani; each has expected
  result inline (`// kani-flags`, `// expected:` style).
- `tests/expected/` -- output-comparison tests (snapshot-style), `.expected`
  files match against Kani diagnostic output.
- `tests/ui/` -- compile-failure UI tests.
- `tests/cargo-kani/`, `tests/cargo-coverage/` -- end-to-end cargo integration.
- `tests/perf/`, `tests/firecracker/`, `tests/std-checks/` -- larger
  benchmarks/regressions.
- Driver: `compiletest`-derived (rustc's own test harness).

## What's worth stealing

- **Implement as a `CodegenBackend`, not just a callbacks impl**: lets you
  reuse rustc's monomorphization collector and codegen pipeline, while
  short-circuiting LLVM. Worth considering for Regseq if we want to use
  rustc's existing mono machinery rather than rolling our own.
- **Reachability-driven mono unit collection**
  (`kani_middle/reachability.rs`) is a clean way to determine which symbols
  to translate from a small set of entry points (proof harnesses for Kani;
  driver entry attrs for us).
- **Stable MIR (`rustc_public`) over `rustc_middle::mir`**: insulates the
  compiler from rustc-internal churn. Kani is one of the largest Stable MIR
  consumers; they validate that the API is sufficient for serious codegen.
- **Stubbing mechanism**: pattern-match a path, replace with a Kani-provided
  body. We will need the same for `core::ptr::*`, intrinsics, and OS
  primitives; Kani's mechanism is a known reference.

## What's specific to their constraints

- GOTO is a low-level C-like IR optimized for symbolic execution. Our target
  (readable C source) is a *higher* level than GOTO -- we want to preserve
  more structure (loops, named types).
- Kani has `PanicStrategy::Abort` baked in because verification only cares
  whether a panic *can* happen. Our backend may need to translate panics to
  driver-specific error codes.
- Kani consumes optimized post-mono MIR because verification only needs a
  semantics-preserving lowering. Our refactor is committed to optimized MIR
  with `mir_opt_level=0` to keep code readable; Kani has no such constraint.
- The CBMC ecosystem absorbs huge amounts of complexity (alias model,
  bit-precise integers, malloc model). We can't lean on a downstream solver
  the way they do; our backend has to be self-contained.

## Sources consulted

- https://github.com/model-checking/kani
- `kani-compiler/src/main.rs`, `kani_compiler.rs`
- `kani-compiler/src/codegen_cprover_gotoc/{compiler_interface.rs,
  codegen/function.rs, codegen/{statement,block,rvalue,typ,place,operand}.rs}`
- `kani-compiler/src/kani_middle/{reachability.rs, codegen_units.rs,
  stubbing/, transform/}`
- https://model-checking.github.io//kani-verifier-blog/2023/08/03/turbocharging-rust-code-verification.html
  (discussion of symtab JSON -> GOTO binary direct serialization)
- https://model-checking.github.io/kani/regression-testing.html
