# c2rust

**One-line summary**: A C-to-Rust transpiler that lowers Clang AST into unsafe Rust that "closely mirrors" the input C; the inverse direction of what Regseq does, but a useful reference for how to map low-level memory semantics across a Rust/C boundary.

**URLs**:
- Repo: https://github.com/immunant/c2rust
- Manual: https://c2rust.com/manual/print.html
- ICSE 2022 paper "In Rust We Trust" (Ling, Cordy et al.): https://research.cs.queensu.ca/home/cordy/Papers/ICSE22-InRustWeTrust.pdf
- Key crates: `c2rust-transpile`, `c2rust-ast-exporter`, `c2rust-refactor`, `c2rust-bitfields`, `c2rust-analyze`

## Pipeline shape

- Source language: C99 (preprocessed against a `compile_commands.json` compilation database).
- Extraction point in rustc: N/A. c2rust runs in the opposite direction. The closest analogue is the Clang AST: `c2rust-ast-exporter` links against Clang and exports a serialised AST to `c2rust-transpile`.
- Output: unsafe Rust source. (Optionally refined by `c2rust-refactor` and `c2rust-analyze` toward safer Rust.)
- How they invoke rustc: do not, for transpilation. `c2rust-refactor` uses `rustc_private` and is pinned to a specific nightly, but the transpiler proper does not.

## IR design

- Owned Rust data structures; no `'tcx` borrowing involved (no rustc on the producing side).
- Closer to a translation context than a true IR: the central type is a `Translation` struct holding a `TypeConverter`, a `Renamer<CDeclId>`, a `FuncContext`, an `IndexMap` of `MacroExpansion`s, an `ItemStore` per `FileId`, a `CommentStore`, deferred-import sets, and a `ZeroInits` table (`c2rust-transpile/src/translator/mod.rs`).
- Per-expression context is carried in an `ExprContext` enum/struct (`is_const`, `decay_ref: DecayRef`, ...) that controls how a given C expression is lowered.
- The C side is represented by exporter-defined IDs (`CDeclId`, `CTypeId`, `CExprId`, `CStmtId`) referencing arena-allocated nodes from the AST exporter.
- The Rust side is built directly as `syn`/`proc_macro2`-style items via `c2rust-ast-builder` -- there is no separate "RustIR".
- Serialization: the Clang AST is serialized via the exporter into a CBOR-like format that the Rust transpiler reads.

## Key design decisions

- Monomorphization handling: not applicable -- C has no generics. Macros are recorded in a `MacroExpansion` map but not generally inlined.
- `core` / `std` treatment: c2rust emits Rust against `libc` and (optionally) `core`, and assumes the user provides a runtime; standard-library functions are referenced as-is.
- Drop handling: not applicable -- generated code is unsafe and uses raw pointers; there are no Drop impls to glue.
- Panic strategy: not applicable -- C abort/exit semantics map to Rust calls into libc.
- Unsupported constructs: explicit reject. The Relooper (borrowed from Emscripten) handles arbitrary `goto` control flow into structured Rust; constructs that cannot be Relooped are flagged.
- Reference rules: every C pointer becomes a Rust raw pointer (`*const T` / `*mut T`) by default. The "decay reference" machinery in `ExprContext` controls when a Rust reference can be introduced safely; `c2rust-analyze` and `c2rust-refactor` later promote raw pointers to `&`/`&mut`.
- Trait dispatch / vtables: not applicable. C function pointers map to Rust `unsafe extern "C" fn` pointers.
- Bitfields: handled by a bespoke `c2rust-bitfields` derive that gives byte-compatible layout with C bitfields while still allowing references to non-bitfield fields.

## Test methodology

- Functional preservation by construction: the manual states the goal is that "test suites should continue to pass after translation" -- the original C test suite is the oracle.
- Cross-checking infrastructure: Clang and rustc plugins instrument both binaries to log function-call-level events; logs are diffed to confirm behavioural equivalence.
- Per-feature test corpora under `tests/`, exercised via the standard `cargo test` workflow.

## What's worth stealing

- The "translation context as the central type" pattern (`Translation` holding type converter, renamer, deferred imports, comment store): RegseqModel will need an analogous container that owns lowering-state shared across items. This is a cleaner model than threading a dozen `&mut` parameters.
- Carrying a small `ExprContext` value through expression lowering to capture out-of-band requirements (constness, reference-decay, unsafe-block nesting). RegseqModel will face similar context-sensitive lowering for `volatile`, register-access intrinsics, and MMIO ordering.
- Comment-and-source-location preservation via a side `CommentStore`: useful for emitting `/* from <file>:<line> */` markers in the C output for debuggability.
- The `c2rust-bitfields` design (byte-compatible layout via custom derive plus accessor methods) is a direct precedent for how Regseq's `register_bitfields!` should map to the C side.

## What's specific to their constraints

- The pipeline is C -> Rust. None of c2rust's parsing, Clang-AST handling, or Relooper machinery is relevant for Regseq. We consume rustc's MIR; structured control flow already exists.
- c2rust deliberately produces unsafe Rust and relies on a *separate* later pass (refactor / analyze) to lift safety. Regseq has the inverse pressure: we start from safe Rust and need to emit C that preserves the relevant semantics; we do not have a "make it safer later" stage.
- Translation correctness is judged by runtime test suites. RegseqModel's correctness story will lean on snapshot/IR-shape tests and (eventually) formal review of the lowering rules; we cannot run a "C test suite" as the oracle for an OS-agnostic driver.
- c2rust has no notion of a target OS abstraction layer; it emits straight-line `extern` Rust against libc. Regseq has to thread Linux/Zephyr backend differences through the IR.

## Sources consulted

- https://github.com/immunant/c2rust (README and crate listing)
- https://c2rust.com/manual/print.html
- https://github.com/immunant/c2rust/blob/master/c2rust-transpile/src/translator/mod.rs (Translation, ExprContext, FuncContext)
- https://research.cs.queensu.ca/home/cordy/Papers/ICSE22-InRustWeTrust.pdf
- https://www.infoq.com/news/2020/08/c2rust-transpiler/
