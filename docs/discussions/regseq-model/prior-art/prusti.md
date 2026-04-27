# Prusti

**One-line summary**: ETH Zurich's deductive Rust verifier; encodes
borrowck-facts-enriched MIR into Viper's intermediate verification language
(VIR) through several VIR levels (high, typed, polymorphic, low).

**URLs**:
- Repo: https://github.com/viperproject/prusti-dev
- Dev guide: https://viperproject.github.io/prusti-dev/dev-guide/
- User guide: https://viperproject.github.io/prusti-dev/user-guide/
- Paper (TACAS 2022, "The Prusti Project"): https://pm.inf.ethz.ch/publications/AstrauskasBilyFialaGrannanMathejaMuellerPoliSummers22.pdf

## Pipeline shape

- Source language: Rust + Prusti specs (`#[requires]`, `#[ensures]`,
  `#[pure]`, `#[trusted]`, etc., implemented as proc macros that survive into
  attributes).
- Extraction point in rustc: **MIR**, accessed via
  `rustc_borrowck::consumers::get_body_with_borrowck_facts` for full
  Polonius/Region output. Implemented in `prusti/src/callbacks.rs`. They
  override `mir_borrowck` to capture the body+facts before they're consumed.
- Output: Viper source (Silver). Verified by the Viper backend (Silicon
  symbolic execution or Carbon translation to Boogie).
- How they invoke rustc: `prusti-rustc` wrapper sets env vars and invokes
  `prusti-driver`, which is a `rustc_driver::Callbacks` impl that registers
  query overrides. See `prusti-launch`, `prusti/src/{driver.rs,callbacks.rs}`.

## IR design

- Multiple stacked VIR levels in `vir/src/`:
  - `vir/src/high` -- close-to-source, type-aware
  - `vir/src/typed` -- resolved types
  - `vir/src/polymorphic` -- polymorphic variant
  - `vir/src/middle/core_proof` -- proof-structured form
  - `vir/src/low` -- close to Viper Silver
  - `vir/src/legacy` -- older form retained for back-compat
  - `vir/src/converter` -- inter-level converters
  - `vir/src/common` -- shared identifiers, modes
- Owned, with `'tcx` mostly absent at the higher levels (they want to outlive
  rustc invocations for caching). The MIR encoder
  (`prusti-viper/src/encoder/mir/`) does carry `'tcx`.
- Notable: explicit fold/unfold logic
  (`prusti-viper/src/encoder/foldunfold/`) for permission accounting; snapshot
  encoder for pure equality reasoning; `procedure_encoder` for procedure-level
  CFG encoding. There is also a `vir-gen` macro crate that generates much of
  the VIR types.
- Serialization: not a primary concern; in-memory IR consumed by the Viper
  bridge (`viper-sys`, `jni-gen`) which calls into Viper running on the JVM.

## Key design decisions

- **Monomorphization**: handled per-call-site via the `polymorphic` VIR
  variant. Prusti does not rely on rustc's mono pass; it encodes generics into
  Viper's parametric-domain mechanism. There is also a "type model" layer.
- **`core` / `std` treatment**: a curated set of `#[extern_spec]` definitions
  models std functions; otherwise functions are opaque or rejected. Special
  encoders for sequences, ranges, slices.
- **Drop handling**: drops are encoded but tightly constrained. A function
  with a `Drop` impl that has side effects is mostly out of scope; the
  encoder produces a Viper method call for drop glue when it cannot prove it
  is trivial.
- **Panic strategy**: panics are verification targets -- Prusti tries to prove
  unreachability of `panic_fmt`. Compile-time `assert!`/`debug_assert!` become
  Viper assertions.
- **Unsupported constructs**: a custom `EncodingError` /
  `SpannedEncodingError` system surfaces precise spans; `error_unsupported!`
  macro labels the construct. Stub encoders (`stub_function_encoder.rs`,
  `stub_procedure_encoder.rs`) emit a Viper stub so verification can proceed
  on the rest of the crate.
- **Reference rules**: encoded into Viper's permission system. `&T` becomes
  fractional read permission; `&mut T` becomes write permission with a magic
  wand back-edge tracking the reborrow. This is the verification heart of
  Prusti.
- **Trait dispatch / vtables**: static dispatch is encoded per impl. `dyn
  Trait` is partially supported via type encoding tricks but historically
  limited.

## Test methodology

`prusti-tests/tests/` is structured by intent:
- `parse/` -- spec parser tests
- `typecheck/` -- spec typechecker tests
- `verify/`, `verify_overflow/`, `verify_partial/` -- positive + negative
  verification cases
- `cargotest.rs`, `compiletest.rs` -- driver harnesses
- Many cases use `// compile-flags:` magic comments and `// expected-output:`
  directives, classic compiletest pattern.

There is also `lifetimes_dump/` for borrow-checker output snapshots, which is
similar in spirit to a MIR-shape test.

## What's worth stealing

- **Stacked IR levels with explicit converters** (`high -> typed ->
  polymorphic -> middle -> low`) is a clean way to factor a long lowering
  pipeline. Each level has well-defined invariants. Worth considering for
  RegseqModel if we end up wanting more than one variant (e.g. "ownership
  IR" -> "C-shaped IR").
- **`vir-gen` macro to generate the IR types**: a procedural macro defines
  the concrete syntax of each VIR level from a single description. Reduces
  the typing-tax of having multiple VIR variants.
- **`#[extern_spec]` mechanism**: a clean pattern for declaring "I trust this
  external function and here is its signature/contract". We will need
  something similar for OS primitives in the C backend.

## What's specific to their constraints

- Prusti's IRs encode permissions and prophecies; we don't need any of that.
- The fold/unfold accounting is verification-only.
- The polymorphic VIR exists because Viper handles parametric domains; for C
  codegen we monomorphize and discard.
- Prusti happily accepts pre-optimized MIR with all the borrowck facts -- they
  *want* the unoptimized shape because verification reasons about borrows.
  We use `optimized_mir` because we want the optimized shape after our
  cleanup whitelist runs.
- Stacked IR levels are a heavy investment. We should adopt only as many
  layers as we can justify; one well-defined IR is better than three
  half-defined ones.

## Sources consulted

- https://github.com/viperproject/prusti-dev (Cargo workspace layout)
- `prusti/src/{callbacks.rs, driver.rs, verifier.rs}`
- `prusti-viper/src/encoder/{encoder.rs, mir/, foldunfold/, snapshot/,
  procedure_encoder.rs, stub_function_encoder.rs}`
- `vir/src/{high, typed, polymorphic, middle, low, legacy, common,
  converter}`, `vir-gen/`
- `prusti-tests/tests/{parse, typecheck, verify, verify_overflow,
  verify_partial}`
- https://viperproject.github.io/prusti-dev/dev-guide/pipeline/summary.html
- https://viperproject.github.io/prusti-dev/dev-guide/pipeline/prusti.html
- Astrauskas et al., "The Prusti Project: Formal Verification for Rust",
  NFM/TACAS 2022
