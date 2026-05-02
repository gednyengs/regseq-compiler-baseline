# mrustc

**One-line summary**: A standalone C++ reimplementation of rustc whose primary purpose is to bootstrap rustc by emitting C from Rust source, then handing off to a C compiler.

**URLs**:
- Repo: https://github.com/thepowersgang/mrustc
- Phase overview: https://github.com/thepowersgang/mrustc/blob/master/Notes/PhaseOverview.md
- MIR design: https://github.com/thepowersgang/mrustc/blob/master/Notes/MIR.md
- MIR optimisations: https://github.com/thepowersgang/mrustc/blob/master/Notes/MIR-Optimisations.md
- Improved C backend notes: https://github.com/thepowersgang/mrustc/blob/master/Notes/ImprovedCBackend.md.txt
- Test harness: `TestRustcBootstrap.sh` in repo root

## Pipeline shape

- Source language: full Rust (a moving target tracking rustc 1.19 -> 1.90)
- Extraction point in rustc: N/A -- mrustc does not use rustc. It re-parses Rust from source, builds its own AST, lowers to its own HIR, then to its own MIR.
- Output: C source (called "very ugly C" by the README); README also mentions LLVM/Cranelift/GIMPLE as plausible alternative backends.
- How they invoke rustc: do not. mrustc replaces rustc end-to-end; the compiled output is then used to bootstrap a real rustc.

## IR design

- Owned C++ data structures throughout (no `'tcx` analogue; mrustc is C++ and does its own arena/RAII management).
- HIR is a "simpler format" derived from the AST after path/type-alias resolution and macro expansion (`Notes/PhaseOverview.md`).
- MIR is a CFG of basic blocks: blocks are vectors with integer indices; LValues = locals/temps/args plus field/deref/index/downcast projections; RValues = reads, borrows, binops, aggregate construction, fat-pointer ops via `meta()`/`fatptr()`, constants. Every call terminates a block. Single-static-assignment-ish: variables are write-once but mutable through references.
- Two C backends in `src/trans/`: `codegen_c.cpp` (primary) and `codegen_c_structured.cpp` (structured-control-flow variant). A separate `codegen_mmir.cpp` emits a "monomorphic MIR" textual format.
- Notes/MIR-V2.md.txt and Notes/ImprovedCBackend.md.txt sketch a planned "(simplified) C AST representation" with multiple codegen units and a cleanup pass that drops unused ZSTs and folds initialisation chains. Status: design notes only.

## Key design decisions

- Monomorphization handling: mrustc monomorphises itself in a "Trans Monomorph" phase before C emission (`Notes/PhaseOverview.md`). It does not consume optimized_mir; it reproduces monomorphization in C++.
- `core` / `std` treatment: walked. mrustc compiles libcore, liballoc, libstd from source as part of bootstrapping rustc.
- Drop handling: explicit drop scopes in MIR with scope IDs, RAII guards, hierarchical containment, deferred completion during match codegen, and panic cleanup drops (`Notes/MIR.md` "Drop Scope Management").
- Panic strategy: full unwind support is implemented because rustc bootstrap requires it; mrustc emits the cleanup paths and drop flags into C.
- Unsupported constructs: README states mrustc "assumes the code it's compiling is valid (and any errors in the generated code are mrustc bugs)". There is no defensive front-end; bugs surface as miscompilation.
- Reference rules: `&T`/`&mut T` are emitted as C pointers; fat pointers are explicit two-word structs built via the dedicated `meta()`/`fatptr()` MIR rvalues.
- Trait dispatch / vtables: static dispatch is resolved during monomorphization; dynamic dispatch goes through emitted vtable structs of function pointers.

## Test methodology

Bootstrap-as-test: `TestRustcBootstrap.sh` builds rustc with mrustc, then uses that rustc to rebuild rustc, and compares. The README claims binary-equal results for 1.90.0. There is also an optional exhaustive MIR validator gated on the `MRUSTC_FULL_VALIDATE` env var, plus targeted MIR-validation rules listed in `Notes/MIR.md`.

## What's worth stealing

- Explicit, named MIR optimisation passes that exist solely to produce cleaner C: De-Tuple, De-Temporary (forward and reverse), Return Backprop, Dead-Drop-Flag Elimination, Borrow Elimination, Slice-Transmute (`Notes/MIR-Optimisations.md`). These are exactly the kind of cleanups RegseqModel will want before C emission.
- Drop-scope model in MIR (scope IDs + hierarchical containment + deferred completion in match codegen). RegseqModel needs a similar story even with restricted drops, and mrustc's terminology is a useful starting point.
- The "(simplified) C AST" idea in `ImprovedCBackend.md.txt`: do not pretty-print directly from MIR; lower to a small C-AST first and let a cleanup pass remove ZST/unit noise. `rustc_codegen_c` has independently arrived at the same design.
- Optional exhaustive validator gated by env var -- cheap to keep off in production, invaluable for development.

## What's specific to their constraints

- mrustc reimplements the entire front-end in C++ because its goal is to be independent of rustc. RegseqModel is the opposite: we trust the pinned rustc and consume `optimized_mir`. We should not duplicate parsing, name resolution, type checking, or borrow checking.
- Full unwind/panic support is required for rustc bootstrap. Driver specs target panic=abort (or no-panic at all); we do not need landing pads or drop-flag spaghetti in the emitted C.
- Whole-`std` support drives much of mrustc's complexity (intrinsic recognition, lang items, `core::ptr::*`). RegseqModel only sees a small `#![no_std]` surface and a bounded set of intrinsics.
- Monomorphization is reimplemented in C++. We get monomorphic, post-generic MIR for free from `optimized_mir`; do not rebuild this.

## Sources consulted

- https://github.com/thepowersgang/mrustc (README)
- https://github.com/thepowersgang/mrustc/tree/master/src/trans (file listing for codegen_c, codegen_c_structured, codegen_mmir, monomorphise, mangling_v2, enumerate, auto_impls, allocator, target, trans_list)
- https://github.com/thepowersgang/mrustc/blob/master/Notes/PhaseOverview.md
- https://github.com/thepowersgang/mrustc/blob/master/Notes/MIR.md
- https://github.com/thepowersgang/mrustc/blob/master/Notes/MIR-Optimisations.md
- https://github.com/thepowersgang/mrustc/blob/master/Notes/ImprovedCBackend.md.txt
- https://github.com/thepowersgang/mrustc/blob/master/Notes/MIR-V2.md.txt
- https://news.ycombinator.com/item?id=26296318 (background discussion)
