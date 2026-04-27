# LLVM IR

**One-line summary**: A typed, SSA-form intermediate representation with three isomorphic encodings (in-memory C++, textual `.ll`, binary bitcode `.bc`); the de facto interchange format between LLVM frontends and backends, with a documented compatibility policy on bitcode.

**URLs**:
- Language reference: https://llvm.org/docs/LangRef.html
- Bitcode format: https://llvm.org/docs/BitCodeFormat.html
- Compatibility doc: https://llvm.org/docs/DeveloperPolicy.html#ir-backwards-compatibility
- Pass infrastructure: https://llvm.org/docs/NewPassManager.html
- Verifier: `llvm/lib/IR/Verifier.cpp` in https://github.com/llvm/llvm-project
- Testing guide: https://llvm.org/docs/TestingGuide.html
- FileCheck: https://llvm.org/docs/CommandGuide/FileCheck.html

## Pipeline shape

- Source language: any (Clang for C/C/Objective-C, rustc, Swift, Julia, ldc, GHC's LLVM backend, etc.).
- Extraction / construction point: frontends construct LLVM IR via the C++ `IRBuilder` API or by emitting textual `.ll` and parsing it.
- Output: target machine code via the codegen pipeline (SelectionDAG / GlobalISel / MIR-the-LLVM-one -> MC -> object file). Also consumable by other tools (`opt`, `llc`, `lli`).
- N/A: How they invoke rustc.

## IR design

- Owned with implicit context. `LLVMContext` owns interned types and constants; `Module` owns functions and globals; functions own basic blocks; basic blocks own instructions. `Value*` pointers are stable for the lifetime of their owner. Use-def chains are intrusive linked lists (`Use` objects).
- Single-level. There is one IR with a fixed instruction set (`add`, `sub`, `load`, `store`, `getelementptr`, `call`, `br`, `switch`, `phi`, `invoke`, `landingpad`, `atomicrmw`, ...). Extensibility is via **intrinsics** (`@llvm.memcpy.*`, `@llvm.dbg.value`, target-specific `@llvm.x86.*`) and **metadata** (`!dbg`, `!tbaa`, `!prof`), not new instruction kinds.
- Notable variants: types are `void | iN | half | float | double | x86_fp80 | fp128 | ptr | <N x T> | [N x T] | {T, ...} | T(T, ...) | label | token | metadata`. Since LLVM 15, pointers are *opaque* (`ptr`) -- previously typed (`i32*`). Constants are first-class IR (`ConstantInt`, `ConstantExpr`, `ConstantDataArray`). Globals have linkage, visibility, thread-local mode, and DLL storage class.
- Serialization: three forms, by design isomorphic.
  - In-memory C++ object graph.
  - Textual `.ll` (LangRef-defined grammar; printed by `Module::print`, parsed by `llvm::parseIR`).
  - Binary **bitcode** `.bc` (a self-describing block-structured format with abbreviations; defined in `BitCodeFormat.html`). Uses variable-width fields and per-block abbreviation definitions for compactness.
- Versioning: bitcode has a version number in the module block and an "Identification" block recording the producer's LLVM version. The textual form is *not* versioned; auto-upgrade logic lives in the bitcode reader (`llvm/lib/Bitcode/Reader/MetadataLoader.cpp`, `BitcodeReader::parseModule`) and in `AutoUpgrade.cpp` for both intrinsic signatures and metadata schemas.

## Key design decisions

- N/A: monomorphization.
- N/A: core/std treatment.
- Scoping / module structure: a `Module` is the top-level unit, containing global variables, functions, aliases, named metadata, and a target triple + data layout string. There is no nested module concept; "linking" is a separate pass (`llvm::Linker`) that merges modules.
- Layered abstractions: LLVM IR is *not* multi-level. The single IR is the contract between frontends and backends. Below LLVM IR, codegen has its own MIR (Machine IR) which is a separate, less-stable representation. Lowering from LLVM IR to MIR happens through SelectionDAG or GlobalISel, neither of which is "the IR" in the LangRef sense.
- Type system: structural for primitive/aggregate types; nominal for named structs (`%MyStruct = type {...}`). Opaque pointers (since LLVM 15) removed pointer-element types from the type system because frontends were lying about them anyway. There is no notion of higher-kinded or generic types -- frontends monomorphize before emission.
- Serialization schema versioning / forward-compat policy: documented as **"new versions can read old bitcode (back to LLVM 3.0); textual IR is best-effort"** (https://llvm.org/docs/DeveloperPolicy.html#ir-backwards-compatibility). The mechanism is `AutoUpgrade.cpp` which rewrites old intrinsic calls/attributes/metadata to the current schema during parsing. Old textual IR may need manual fixups.
- Negative design choices:
  - No open instruction set (intrinsics fill that role).
  - No nested IR levels (intentional simplicity vs MLIR).
  - No round-trip guarantee for textual IR across versions (only bitcode).
  - No source-language-specific constructs (everything is lowered to a generic SSA form before emission).

## Test methodology

LLVM's test infrastructure is the canonical FileCheck/lit setup:

- Tests live in `llvm/test/`, organized by component (`Transforms/InstCombine/`, `CodeGen/X86/`, `Verifier/`, `Bitcode/`).
- Each test is a `.ll` file with a `; RUN:` line at the top (e.g. `; RUN: opt -S -passes=instcombine %s | FileCheck %s`) and `; CHECK:` directives interspersed.
- `FileCheck` matches output against patterns: `CHECK`, `CHECK-NEXT`, `CHECK-DAG`, `CHECK-NOT`, `CHECK-LABEL`, `CHECK-SAME`, `CHECK-COUNT-N`, `CHECK-EMPTY`. Variables (`[[NAME:%[0-9]+]]`) capture and reuse SSA names across lines.
- `update_test_checks.py` (and friends `update_llc_test_checks.py`, `update_mir_test_checks.py`) regenerates `CHECK` lines from actual output -- so tests are *machine-maintainable* on the gold side, not hand-written.
- The **verifier** (`opt -passes=verify`, run automatically by the new pass manager) checks IR well-formedness on every pass output. A `Verifier`-failing test is itself a category (`llvm/test/Verifier/`).
- **Bitcode round-trip tests** (`llvm/test/Bitcode/`) parse old bitcode files committed to the repo and check the upgraded textual output -- these are the actual backward-compat regression net.
- C++ unit tests (`llvm/unittests/IR/`) cover API-level invariants on the in-memory representation.

The combination of `FileCheck` + `update_test_checks.py` + `Verifier` is what makes a many-thousand-pass-test suite tractable.

## What's worth stealing for RegseqModel

- **Two encodings, with the textual form as the test contract**: a printer/parser pair whose output is what FileCheck asserts on. Bitcode-equivalent (binary) is optional and only worth it if you ship the IR across processes; for an in-tree compiler, textual is enough.
- **Verifier as a first-class pass run between every transformation** catches schema violations cheaply; pair it with a `--verify-each` style flag.
- **`update_test_checks.py`-style regeneration tooling** is the secret sauce that makes FileCheck scale. If RegseqModel adopts FileCheck-on-textual-IR for tests, build a small "regenerate CHECK lines" script early.
- **Metadata over new ops** for cross-cutting concerns (debug info, profiling hints, attribution): attach as side-channel attributes/annotations rather than expanding the core op set. This keeps the IR core small while letting domain-specific layers carry the info they need.

## What's specific to their constraints

- LLVM IR is an *interchange* IR between independently-shipped frontends and backends; that drives bitcode + a documented compatibility policy + AutoUpgrade. RegseqModel is internal to one compiler; that whole story is unnecessary.
- The single-level design is a tradeoff for simplicity at the cost of expressiveness -- which is why MLIR exists. RegseqModel's domain-specific layer would be awkward as one flat instruction set.
- Opaque pointers and lack of source-language types reflect LLVM's "lower everything to bytes early" stance. RegseqModel emits C and benefits from keeping richer types longer.
- Bitcode's complexity (variable-width abbreviations, block structure) is justified only at LLVM's scale and lifetime. A simpler textual format suffices for our scale.

## Sources consulted

- https://llvm.org/docs/LangRef.html (overall IR semantics, type system)
- https://llvm.org/docs/BitCodeFormat.html (binary format, abbreviations, version blocks)
- https://llvm.org/docs/DeveloperPolicy.html#ir-backwards-compatibility (compat policy)
- https://llvm.org/docs/TestingGuide.html
- https://llvm.org/docs/CommandGuide/FileCheck.html
- https://llvm.org/docs/NewPassManager.html (pass manager + verifier integration)
- `llvm/lib/IR/AutoUpgrade.cpp`, `llvm/lib/Bitcode/Reader/BitcodeReader.cpp` (auto-upgrade mechanics)
- `llvm/utils/update_test_checks.py` (test maintenance tooling)
- https://llvm.org/devmtg/2022-04-03/slides/Opaque.Pointers.Are.Coming.pdf (opaque pointers transition)
