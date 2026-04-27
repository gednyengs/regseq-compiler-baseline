# rustc_codegen_clr

**One-line summary**: A third-party rustc codegen backend (FractalFir) that lowers MIR to .NET CIL and, via a `C_MODE` switch, to C source -- a single backend whose internal IR was designed to print to either CIL or C.

**URLs**:
- Repo: https://github.com/FractalFir/rustc_codegen_clr
- README: https://github.com/FractalFir/rustc_codegen_clr/blob/main/README.md
- v0.1.0 retrospective (most detailed design write-up): https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_1_0.html
- v0.0.1 intro: https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_0_1.html
- v0.1.3 (sized vs unsized layout): https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_1_3.html

## Pipeline shape

- Source language: full Rust (the project compiles unmodified `core`/`std`).
- Extraction point in rustc: post-monomorphization, per-codegen-unit MIR. Like `rustc_codegen_c`, it implements rustc's `CodegenBackend` trait and is loaded via `-Zcodegen-backend`.
- Output: by default, .NET assemblies (CIL). With the env var `C_MODE=1`, the same internal IR is emitted as C source.
- How they invoke rustc: as a codegen-backend plugin, against a pinned nightly.

## IR design

- Owned Rust data structures around a CIL-style stack-machine IR. The author describes nodes informally as "CIL nodes" and "CIL roots" in blog posts; the README does not name them.
- Translation is described as "type by type, method by method, op by op" with each MIR statement mapping to a short sequence of CIL instructions (see v0.1.0 retrospective). MIR is a CFG of basic blocks with statements + terminators; CIL is a stack machine -- the backend lowers each MIR rvalue into push/pop sequences.
- Notable design claim from the README: ".NET's IR maps nicely to C, which means I was able to add support for compiling Rust to C in 2-3K LOC ... almost all of the codebase is reused, with the C and .NET specific code only present in the very last stage of compilation." So the IR is single-source-of-truth; only the printer differs.
- v0.1.3 documents an explicit-layout strategy for sized/DST/enum types so that the same memory layout works for both CIL value types and C structs.
- Optional `TRACE_STATEMENTS` feature inserts per-statement runtime logging -- mrustc's `MRUSTC_FULL_VALIDATE` analogue, but at runtime rather than as a checker.

## Key design decisions

- Monomorphization handling: free, inherited from `rustc_codegen_ssa`'s mono-item walking (same as `rustc_codegen_c`).
- `core` / `std` treatment: walked. The retrospective claims unmodified `std` builds and runs, with ~95% completeness on .NET and ~80% on C.
- Drop handling: by the time MIR is consumed, drop is already elaborated into explicit calls; the backend treats them as ordinary calls.
- Panic strategy: on .NET, mapped to .NET exceptions (the runtime provides unwinding for free). For C mode this is necessarily different and is one of the gaps the author flags as "early development".
- Unsupported constructs: README states "most Rust features (except proc macros)". Bugs surface as miscompilation rather than rejection.
- Reference rules: `&T`/`&mut T` lower to managed references on .NET (`ref T`) or to raw pointers in C mode. Fat pointers are explicit two-field structs with explicit layout (v0.1.3).
- Trait dispatch / vtables: handed off to rustc's monomorphization for static dispatch; dyn dispatch is lowered to function-pointer-table calls in both backends.

## Test methodology

- The retrospective describes running unmodified Rust test programs (incl. Rust's own examples and select crates) against both backends and checking they execute correctly.
- Per-statement runtime tracing (`TRACE_STATEMENTS`) for diagnosing miscompilation.
- No FileCheck-style snapshot testing is documented in the README.

## What's worth stealing

- **One IR, two printers**: the strongest design lesson here. By keeping the IR neutral and only specialising the very last stage, the project gets a second backend (C) for "2-3K LOC". RegseqModel should be designed so that a Linux C printer and a Zephyr C printer share an IR and only diverge at the print layer.
- **Direct, statement-by-statement MIR -> IR mapping**: "each MIR statement, you can easily find the corresponding sequence of [target] instructions in the final assembly." This is exactly the discipline RegseqModel needs for debuggability and review-ability of generated drivers.
- **Optional per-statement tracing for triage**: very cheap to leave compiled out, very valuable when a driver mis-behaves on hardware.
- **Explicit layout decisions for fat pointers, enums, and DSTs done once in the IR**: do not let the C printer make layout choices.

## What's specific to their constraints

- The primary target is .NET CIL, a stack-machine IR with managed references, GC, and exceptions. Many of its design choices (CIL stack discipline, `ref T` lowering, exception-based panic) do not translate to a register-machine C output for OS drivers.
- Like `rustc_codegen_c`, this is a `CodegenBackend` plugin loaded inside rustc. Regseq is an external driver that consumes `optimized_mir` via `rustc_driver::Callbacks`; we are not registering a backend.
- The project compiles unmodified `core`/`std`. RegseqModel only needs to handle a small `#![no_std]` DSL surface; we can reject most of `core`'s complexity.
- C mode is acknowledged as the lower-priority target with more rough edges (~80% vs ~95%). Their C printer is a port of the .NET printer, not a primary design driver. We are designing primarily for C from day one.

## Sources consulted

- https://github.com/FractalFir/rustc_codegen_clr (README)
- https://github.com/FractalFir/rustc_codegen_clr/blob/main/README.md
- https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_1_0.html (half-year retrospective: MIR -> CIL mapping, TRACE_STATEMENTS, completeness numbers)
- https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_0_1.html (project intro)
- https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_1_3.html (sized/unsized layout)
