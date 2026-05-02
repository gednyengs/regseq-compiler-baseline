# Verus

**One-line summary**: SMT-based verifier for systems-Rust; converts a
*verus-flavored* Rust dialect from rustc HIR through three internal IRs
(VIR-AST, VIR-SST, AIR) to SMT-LIB queries.

**URLs**:
- Repo: https://github.com/verus-lang/verus
- Architecture doc: https://github.com/verus-lang/verus/blob/main/source/CODE.md
- Paper (OOPSLA 2023): https://dl.acm.org/doi/10.1145/3586037

## Pipeline shape

- Source language: Rust + Verus dialect (Spec/Proof/Exec modes,
  `Ghost<T>`/`Tracked<T>`, Verus macro syntax). Programs are processed by the
  `verus_macros` proc macro before rustc sees them.
- Extraction point in rustc: **HIR** (post-expansion, post-typeck, *not* MIR).
  Hooked from `after_expansion` in `VerifierCallbacksEraseMacro`. The
  HIR-to-VIR conversion lives in `source/rust_verify/src/rust_to_vir*.rs`.
- Output: SMT-LIB queries, sent to Z3 (and experimentally cvc5) over an
  external process via `air/src/smt_process.rs`.
- How they invoke rustc: a `rustc_driver::Callbacks` impl
  (`VerifierCallbacksEraseMacro`) that runs *two* rustc invocations -- one with
  ghost code preserved (for verification), one with ghost erased (for
  compilation).

## IR design

- **VIR-AST** (`vir/src/ast.rs`): a Rust-subset AST with verification
  metadata. `Mode` is either `Spec`, `Proof`, or `Exec`. Statements may appear
  inside expressions (mirroring Rust). Owned data; not `'tcx`-borrowed.
- **VIR-SST** (`vir/src/sst.rs`): Statement-oriented Syntax Tree, with
  expressions purified -- `Exp` cannot contain `Stm`. Conversion in
  `vir/src/ast_to_sst*.rs` performs statement extraction and variable
  renaming.
- **AIR** (`air/src/ast.rs`): Assertion IR -- `assert`, `assume`, mutable
  local update. Closest to SMT-LIB. Has its own crate (`air`) separate from
  `vir`.
- Notable types: `Function`, `FunctionX`, `Krate`, `KrateX`, `Typ`/`TypX`,
  `Datatype`, `Trait`. The `X`-suffix pattern is from sharing-via-Arc -- a
  pattern Verus uses heavily.
- Serialization: none of the in-memory IRs are persisted. Verification logs
  (`--log-all`, `.verus-log/`) emit pretty-printed forms for debugging.

## Key design decisions

- **Monomorphization**: not done in the IR pipeline. Generics are encoded in
  SMT via type parameters / type abstractions. Trait resolution is custom
  (`vir/src/traits.rs`, `rust_verify/src/trait_*.rs`).
- **`core` / `std` treatment**: replaced wholesale by the `vstd` crate (Verus
  Standard Library), which provides verified specs for sequences, sets, maps,
  arithmetic, concurrency primitives, and `vstd/std_specs/` for std library
  shims.
- **Drop handling**: erasure-based. Ghost code (Spec/Proof) is erased before
  compilation; only Exec code reaches rustc's normal pipeline. Drop is
  rustc's job for Exec; verification reasons about `vstd`-defined invariants
  rather than runtime drop semantics.
- **Panic strategy**: panics in Exec are verification obligations -- the
  verifier proves they are unreachable. There is no abstract panic semantics
  in the IR; `panic!` calls become reachability obligations.
- **Unsupported constructs**: enforced by mode checking
  (`vir/src/modes.rs`) -- ghost-only operations in Exec or vice versa are
  rejected with a clear error. There is also a "well-formedness" pass
  (`vir/src/well_formed.rs`).
- **Reference rules**: `&T` is treated as a value at the spec level (with
  prophecy-like reasoning for `&mut T` via `Ghost`/`Tracked`). The main
  ownership story is enforced by linear ghost types -- this is the central
  contribution of the Verus paper.
- **Trait dispatch / vtables**: static. `dyn Trait` is supported in limited
  forms; trait conflict checking lives in `rust_verify/src/trait_*.rs`.

## Test methodology

- `source/rust_verify_test/tests/` is the main test suite -- many `.rs` files
  organized by feature.
- Tests are usually `cargo test`-driven and assert on verification verdicts
  via expected-output annotations.
- A heavy use of integration tests: each test compiles a full Verus program
  and asserts pass/fail. Smaller unit tests exist in `vir/` and `air/`.
- Debug logging via `--log-all` produces `.verus-log/` snapshots of every IR
  level; useful for diagnosing pipeline regressions.

## What's worth stealing

- **The IR-per-purpose pattern**: VIR-AST (close to source) -> VIR-SST
  (statements/expressions split) -> AIR (assertion-level). Each has clearly
  delineated invariants. If Regseq ever splits "ownership-aware" from
  "C-shaped", this is the canonical example.
- **`X`-suffix pattern (`FunctionX`/`Function = Arc<FunctionX>`)**: cheap
  cloning of large IR nodes via `Arc` while keeping public types ergonomic.
  Worth adopting for any IR with deeply nested shared subterms.
- **External-process SMT communication via stdin/stdout**: the `air` crate's
  `SmtProcess` is a model for clean external-tool integration if Regseq ever
  needs to shell out to an external linter / formatter.
- **Per-IR log dumps gated on a flag**: standard practice in compilers and
  worth ensuring Regseq does it from day one.

## What's specific to their constraints

- Verus extracts from **HIR**, not MIR. They do this because they want pre-
  borrowck source-level shape (their ghost types are erased and never reach
  borrowck). For Regseq, MIR is the right level -- we *want* the post-typeck,
  post-borrowck, post-monomorphization optimized form.
- Three IR levels are justified by the SMT-encoding pipeline (purify
  expressions, then encode assertions). Codegen has different needs;
  copy the pattern only where it earns its keep.
- Verus replaces std with `vstd`. We can't replace `core` -- driver code uses
  it. We need a recognition-and-rewrite story (intrinsics, traits) rather
  than wholesale replacement.
- The Spec/Proof/Exec mode system is verification-specific; we have no
  analogue. Erasure of ghost code does parallel our DSL erasure, though.
- They need the source-level `&mut T` because verification reasons about it;
  we need the post-MIR shape because we lower it.

## Sources consulted

- https://github.com/verus-lang/verus
- `source/CODE.md` (architecture overview, IR pipeline diagram)
- `source/rust_verify/src/{driver.rs, verifier.rs, callbacks.rs,
  rust_to_vir.rs, rust_to_vir_func.rs, rust_to_vir_expr.rs,
  rust_to_vir_base.rs, trait_check.rs}`
- `source/vir/src/{ast.rs, sst.rs, ast_to_sst.rs, sst_to_air.rs, modes.rs,
  traits.rs, well_formed.rs, prune.rs}`
- `source/air/src/{ast.rs, smt_process.rs}`
- `source/vstd/`, `source/rust_verify_test/tests/`
- Lattuada et al., "Verus: Verifying Rust Programs Using Linear Ghost
  Types", PACMPL/OOPSLA 2023 (DOI 10.1145/3586037)
