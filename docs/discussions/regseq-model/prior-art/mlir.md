# MLIR

**One-line summary**: Multi-Level Intermediate Representation; an extensible compiler infrastructure that hosts many "dialects" (domain-specific IRs) inside one common in-memory data model with shared parsing, printing, verification, and pass tooling.

**URLs**:
- Repo: https://github.com/llvm/llvm-project/tree/main/mlir
- Paper: "MLIR: A Compiler Infrastructure for the End of Moore's Law" (Lattner et al., CGO 2021), https://arxiv.org/abs/2002.11054
- Docs: https://mlir.llvm.org/docs/
- Language reference: https://mlir.llvm.org/docs/LangRef/
- Bytecode format: https://mlir.llvm.org/docs/BytecodeFormat/
- Dialect conversion: https://mlir.llvm.org/docs/DialectConversion/

## Pipeline shape

- Source language: any. MLIR is a substrate; frontends (Flang, CIRCT, IREE, TensorFlow, mlir-clang/Polygeist) build their own dialects.
- Extraction / construction point: frontends emit MLIR ops directly via the C++/Python builder API or via parsing the textual `.mlir` form.
- Output: lowered to LLVM IR via the `llvm` dialect + `mlir-translate`, or to SPIR-V, or to C through CIRCT/`emitc` dialect.
- N/A: How they invoke rustc.

## IR design

- Owned, with implicit context. Every `Operation`, `Type`, and `Attribute` is owned by an `MLIRContext`. Types and attributes are uniqued (interned) per context; ops are reference-counted via `OpBuilder`/region ownership. There are no free-standing values; everything carries an implicit pointer back to its context.
- Layered. The IR has a single uniform structure (Operation / Region / Block / Value / Type / Attribute) but the *vocabulary* is open: dialects register new ops, types, and attributes. `func.func`, `arith.addi`, `scf.for`, `llvm.load`, `gpu.launch` are all the same kind of object.
- Notable variants: built-in dialect (`builtin`, `module`, function-like ops), `func`, `arith`, `scf` (structured control flow), `cf` (unstructured CFG), `memref`, `tensor`, `linalg`, `affine`, `llvm`, `spirv`, `emitc`, `gpu`, `async`. Each dialect is a C++ library; ops are typically declared in TableGen (`.td`) via the ODS (Operation Definition Specification) framework.
- Serialization: textual `.mlir` (human-readable, S-expression-flavored but uses `%ssa` SSA names and `op (operands) : type` syntax) plus a binary "MLIR Bytecode" format (since 2022). Both are round-trip stable for any registered dialect.
- Versioning: bytecode has an explicit version number on the file header and per-dialect version hooks (`BytecodeDialectInterface::writeVersion` / `readVersion`). Textual form is not versioned; backward compatibility is per-dialect best-effort.

## Key design decisions

- N/A: monomorphization.
- N/A: core/std treatment.
- Scoping / module structure: top-level is typically a `builtin.module` op containing nested ops; symbols are resolved via `SymbolTable` interfaces. Regions provide nested scopes; `IsolatedFromAbove` is a trait that marks region boundaries SSA values cannot escape.
- Layered abstractions interact via **Dialect Conversion**: a target-legality framework where you declare which ops are "legal" in the target dialect set, register `ConversionPattern`s (rewrites that may change types), and run either partial or full conversion. Type conversions are handled by a `TypeConverter` that materializes casts at boundaries. Lowering is therefore *pattern-driven* and incremental.
- Type system: types are first-class IR objects, also dialect-extensible. Dialects can introduce types that have no LLVM equivalent (e.g. `tensor<?x?xf32>`, `memref<...>`, `!gpu.async.token`); they get lowered when entering a lower dialect. Attributes carry compile-time constants, including structured ones (arrays, dicts, dialect-defined).
- Serialization schema versioning: bytecode dialects implement read/write hooks per version; the framework records the version each dialect serialized at, so old files can still be parsed by newer dialects that handle their old encodings explicitly.
- Negative design choices: MLIR deliberately does *not* fix a single type system, ABI, or memory model at the substrate level; those live in dialects. It also does not bake in a specific control-flow shape -- both structured (`scf`) and CFG-style coexist.

## Test methodology

Tests are LLVM-style **lit + FileCheck**. A `.mlir` test file contains a `// RUN:` line invoking `mlir-opt` (or another tool) with specific passes, and `// CHECK:` directives that match the textual output line-by-line. Variants `CHECK-NEXT`, `CHECK-DAG`, `CHECK-NOT`, `CHECK-LABEL`, and `CHECK-SAME` cover ordering, grouping, negation, and substring matching. The textual form's stability makes this robust: passes are tested by "feed in this MLIR, run these passes, the printed output must match these patterns." The verifier (`mlir-opt -verify-each`) and `--mlir-print-op-on-diagnostic` make round-trip and well-formedness checks part of every test. Unit tests in C++ exist for builder/data-structure code, but the *vast majority* of MLIR's transformation tests are FileCheck on textual IR. There is also a built-in `mlir-translate` "round-trip" test pattern (parse -> print -> parse -> print, expect equality).

## What's worth stealing for RegseqModel

- Textual form first, designed for FileCheck-style tests. The textual `.mlir` syntax is the contract for tests; this scales to thousands of pass tests far better than `insta` snapshots because FileCheck lets you assert *partial* shape (key SSA edges, op kinds) instead of full file equality.
- Dialect conversion as the layered-IR mental model: separate "MIR-mirror" ops from "Regseq domain" ops as distinct dialects, then write `ConversionPattern`s that lower domain ops into the lower-level vocabulary. This avoids one flat enum that grows forever.
- Round-trip-stable textual format with a verifier is a strong invariant: every IR object has a printer, parser, and verifier; `parse(print(x)) == x` is a property test.
- TableGen-style declarative op definitions (ODS) reduce boilerplate; if RegseqModel grows many op variants, a similar declarative source-of-truth (a `build.rs` codegen, a macro DSL) is worth considering.

## What's specific to their constraints

- MLIR is general-purpose substrate: open vocabulary, plugin-style dialects, JIT and AOT consumers. RegseqModel has one frontend (rustc MIR) and a few backends (Linux/Zephyr C); the open-vocabulary machinery is overkill.
- The C++ context/uniquing infrastructure assumes long-lived in-process compilation; a Rust compiler that round-trips through files needs different ownership.
- MLIR's dialect-conversion framework supports type changes at conversion time; RegseqModel emits C and can keep a closed type set.
- Bytecode versioning matters for MLIR because frontends and backends ship independently; for RegseqModel the compiler is a monolith and IR-on-disk is a debugging artifact, not an interchange format.

## Sources consulted

- https://mlir.llvm.org/docs/LangRef/
- https://mlir.llvm.org/docs/BytecodeFormat/
- https://mlir.llvm.org/docs/DialectConversion/
- https://mlir.llvm.org/docs/DefiningDialects/Operations/ (ODS)
- https://mlir.llvm.org/docs/Tutorials/Toy/ (Toy tutorial, dialect layering)
- https://arxiv.org/abs/2002.11054 (MLIR paper, sections 3-5)
- https://llvm.org/docs/CommandGuide/FileCheck.html
- https://llvm.org/docs/CommandGuide/lit.html
