# rustc_codegen_c

**One-line summary**: An experimental rust-lang/-org codegen backend that plugs into rustc's `CodegenBackend` trait and lowers per-codegen-unit MIR into a small C AST that is pretty-printed and compiled with clang.

**URLs**:
- Repo: https://github.com/rust-lang/rustc_codegen_c
- README: https://github.com/rust-lang/rustc_codegen_c/blob/main/README.md
- Backend entry point: `crates/rustc_codegen_c/src/lib.rs`, `crates/rustc_codegen_c/src/base.rs`
- Owned C AST: `crates/rustc_codegen_c_ast/src/{lib,arena,decl,expr,func,module,pretty,stmt,ty}.rs`

## Pipeline shape

- Source language: full Rust (whatever rustc's frontend accepts).
- Extraction point in rustc: post-monomorphization, per-codegen-unit MIR. The backend implements `rustc_codegen_ssa::traits::CodegenBackend` and `ExtraBackendMethods`; rustc itself drives codegen via `rustc_codegen_ssa::base::codegen_crate` and calls back into `compile_codegen_unit`, which iterates `cgu.items_in_deterministic_order(tcx)` (see `base.rs`). This is the same hook used by `rustc_codegen_llvm` and `rustc_codegen_cranelift`.
- Output: C source (one file per codegen unit), then clang to produce object files; the existing `rustc_codegen_ssa` linker is reused (`link_binary`).
- How they invoke rustc: as a codegen backend plugin loaded via `-Zcodegen-backend=...`. Uses `#![feature(rustc_private)]` and links against `rustc_middle`, `rustc_codegen_ssa`, `rustc_session`, etc.

## IR design

- Owned, arena-allocated C AST in a separate crate (`rustc_codegen_c_ast`). `'mx` lifetime parameter throughout for the arena; not `'tcx`-borrowed, but the same pattern.
- `ModuleArena<'mx>` owns the arena and a single `Module<'mx>`. `ModuleCtx<'mx>` is a `Copy` handle to that arena, threaded through lowering.
- AST shape mirrors a small subset of C. From `expr.rs`:
  - `CValue` = `Scalar(i128) | Local(usize) | Func(&str)` -- mirrors LLVM's split between constants and SSA values, intentionally so per the comment ("We follow the `rustc_codegen_ssa` convention").
  - `CExprKind` = `Raw | Value | Binary | Cast | Call | Member`.
  - Sibling modules: `decl`, `stmt`, `ty`, `func`, `module`.
- Pretty-printer is its own module (`pretty.rs`) using `rustc_ast_pretty`'s printer combinators -- not a `Display` impl on each node.
- No serialization format. The C AST is built and printed within a single rustc invocation.

## Key design decisions

- Monomorphization handling: free. Per-codegen-unit MIR is already monomorphic; the backend just walks `mono_items` and calls `predefine`/`define` (the standard `rustc_codegen_ssa::mono_item::MonoItemExt` methods).
- `core` / `std` treatment: relies on rustc's own codegen-unit partitioning. Anything reachable in the CGU is lowered like normal MIR; the backend does not special-case `core`/`std`.
- Drop handling: handed off to MIR. By the time MIR reaches `optimized_mir`, drop terminators have been elaborated into explicit drop calls; the backend treats them like any other call.
- Panic strategy: inherits whatever the session is configured with; the README does not document a special panic story. Helper runtime is in `crates/rustc_codegen_c/src/helper.h` and the `rust_runtime/` directory.
- Unsupported constructs: not enumerated. README is explicit that "this code is still highly experimental and not ready for production use."
- Reference rules: `&T`/`&mut T` lower to C pointers; the AST has no notion of references. `CExprKind::Member { arrow: bool, ... }` distinguishes `.` from `->` at print time.
- Trait dispatch / vtables: provided by rustc's monomorphization. Static calls become C calls; dyn calls go through whatever vtable layout `rustc_codegen_ssa` computes.

## Test methodology

- LLVM `FileCheck`-style codegen tests under `tests/codegen/`. Each `.rs` file contains `// CHECK:` directives that match against the generated C source. Required tooling: `clang` and `LLVM FileCheck` (per README).
- Test categories visible in tree: `filename`, `func_call`, `params_count`, `ret_value`. Plus `tests/bless` (golden output) and `tests/auxiliary`.
- Driven by a project-local `./y` script (a `cargo`-style task runner).

## What's worth stealing

- **Two-crate split**: `rustc_codegen_c` (the rustc-facing plugin) and `rustc_codegen_c_ast` (a self-contained C AST + pretty-printer with no rustc dependencies). RegseqModel should sit in its own crate that does not pull `rustc_private`; the rustc-facing extractor should be a thin shim.
- **Arena + `ModuleCtx<'mx>` handle pattern**: cheap to copy, threads through lowering without lifetime gymnastics. Better than passing `&mut Module` everywhere.
- **`CValue::{Scalar, Local, Func}` split**: explicitly inherited from `rustc_codegen_ssa`'s convention. Worth adopting verbatim for RegseqModel's value layer; it cleanly distinguishes constants, SSA-locals, and symbol references.
- **FileCheck-on-C-output for codegen tests**: directly applicable. We get textual matching tests without committing entire golden files when we only care about a handful of lines.
- **Plug into `rustc_codegen_ssa::base::codegen_crate`**: even if we *do not* register as a codegen backend, the `predefine`/`define` two-pass pattern over `mono_items` is the right shape for our extractor.

## What's specific to their constraints

- `rustc_codegen_c` runs *inside* rustc as a codegen backend (via `-Zcodegen-backend`). Regseq is a separate driver process that consumes `optimized_mir` via `rustc_driver::Callbacks`; we are not implementing `CodegenBackend`.
- It targets general-purpose Rust including `std`. Regseq targets a small `#![no_std]` DSL; we can reject many MIR constructs that `rustc_codegen_c` is forced to handle.
- Linking and archive building are delegated back to `rustc_codegen_ssa::back::link::link_binary` and a custom `ArArchiveBuilder`. Regseq emits a C source file for downstream OS build systems to consume; there is no link step on our side.
- It uses `rustc_codegen_ssa`'s codegen-unit partitioning, `WorkProduct` cache, and parallel-codegen plumbing. We do not need any of that for a single-driver compilation unit.

## Sources consulted

- https://github.com/rust-lang/rustc_codegen_c (README, repo tree)
- `crates/rustc_codegen_c/src/lib.rs` (CodegenBackend impl, extern crate list)
- `crates/rustc_codegen_c/src/base.rs` (`compile_codegen_unit`, `module_codegen`, `predefine`/`define` loop)
- `crates/rustc_codegen_c_ast/src/lib.rs` (`ModuleArena`, `ModuleCtx`)
- `crates/rustc_codegen_c_ast/src/expr.rs` (`CValue`, `CExprKind`)
- Test layout under `tests/codegen/` (filename, func_call, params_count, ret_value)
