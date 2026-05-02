# CompCert

**One-line summary**: A formally verified optimizing C compiler implemented and proved correct in Coq; its front- and back-end IRs (Csyntax, Clight, C#minor, Cminor, CminorSel, RTL, LTL, Linear, Mach, Asm) form a chain of small-step semantics where each lowering step has a machine-checked simulation proof.

**URLs**:
- Main site: https://compcert.org/
- Repo: https://github.com/AbsInt/CompCert
- Reference manual: https://compcert.org/man/
- Paper: Xavier Leroy, "Formal verification of a realistic compiler" (CACM 2009), https://xavierleroy.org/publi/compcert-CACM.pdf
- Cminor description: Leroy, "A formally verified compiler back-end" (JAR 2009), https://xavierleroy.org/publi/compcert-backend.pdf
- Csyntax/Clight: Blazy & Leroy, "Mechanized semantics for the Clight subset of the C language" (JAR 2009)

## Pipeline shape

- Source language: C (a large subset). Frontend parsed by `cparser` (an OCaml/Menhir parser outside the Coq kernel) into Csyntax.
- Extraction / construction point: parser builds Csyntax (Coq AST) which then goes Csyntax -> Clight -> C#minor -> Cminor -> CminorSel -> RTL -> LTL -> Linear -> Mach -> Asm.
- Output: assembly for x86, ARM, PowerPC, RISC-V, AArch64.
- N/A: How they invoke rustc.

## IR design

- Owned, pure functional. All IRs are Coq inductive datatypes; trees are values, no hidden context. Identifiers (`ident`) are positive integers; environments map idents to values/types.
- Layered. Each IR is a *separate* Coq module (`Csyntax.v`, `Clight.v`, `Cminor.v`, `RTL.v`, ...) with its own AST, its own small-step semantics, and a translation pass to the next IR proven correct by a forward simulation.
- Notable variants:
  - **Csyntax**: full C with side effects in expressions, sequence points modeled explicitly.
  - **Clight**: side-effect-free expressions; statements own all effects. The "stable" entry point most users target.
  - **C#minor**: drops C-specific types; values are machine integers/floats/pointers; struct/union accesses become explicit memory ops.
  - **Cminor**: simple imperative language with structured control flow (`if`, `loop`, `block`/`exit n`), expressions are pure, memory is byte-addressed. This is the *target-independent* low-level IR.
  - **CminorSel** through **Asm**: progressively closer to machine.
- Serialization: primarily an *internal* AST. Coq does not print/parse it for users. A separate **Csyntax pretty-printer** can emit C-like text for debugging (`-dparse`, `-dclight`, `-dcminor`, `-drtl` flags dump each stage). There is no canonical binary form; the AST lives only in memory inside the extracted OCaml.
- Versioning: none at the IR level. The *language* (which Csyntax/Cminor accept) is versioned implicitly by CompCert release; cross-version IR interchange is not a use case.

## Key design decisions

- N/A: monomorphization.
- N/A: core/std treatment.
- Scoping / module structure: a `program` is `(list (ident * globdef)) * ident * list ident` -- definitions, main symbol, public symbols. Local scopes are flat per-function environments mapping idents to types/stack slots. No nested modules.
- Layered abstractions interact via **explicit translation passes with simulation proofs**. `Cshmgen.transl_program : Clight.program -> res Csharpminor.program`, then `Cminorgen`, `Selection`, `RTLgen`, etc. Each pass returns `res T = OK T | Error msg`. Correctness is a Coq theorem like `forward_simulation (Clight.semantics p) (Csharpminor.semantics tp)`. Lowerings are *not* pattern-rewriting; they are total recursive functions on the source AST.
- Type system: Csyntax/Clight have full C types (with bitfields, arrays, structs, unions, function types). At Cminor and below, types collapse to `Tint | Tlong | Tfloat | Tsingle | Tany32 | Tany64` -- chunk-size + interpretation. Anything that doesn't fit C cleanly (e.g. structured returns) is lowered to memory operations on the explicit byte-addressable memory model (`Memory.v`). The memory model is the load-bearing abstraction that makes "not-quite-C" types representable at low levels.
- Serialization schema versioning / forward-compat policy: not applicable; IR is in-process Coq/OCaml values.
- Negative design choices: CompCert deliberately does *not* support a unified IR; the multi-IR chain is the design. It also avoids extensible/open op sets -- each IR has a fixed Coq inductive, because adding a constructor breaks every proof. They explicitly *do not* expose the IRs as a stable interchange format.

## Test methodology

- The headline guarantee is the **Coq simulation proof**, not tests. Each pass has `Theorem transf_program_correct : forward_simulation (S.semantics p) (T.semantics tp)`.
- Beyond proofs: `test/` directory in the repo contains C programs run under both CompCert and a reference compiler with output diff'd (`test/c/Results/`, `test/regression/`). This is end-to-end behavioral testing.
- Per-IR debugging dumps (`-dclight`, `-dcminor`, `-drtl`) produce textual output used in manual inspection; a small number of regression tests grep these.
- The pretty-printers for Csyntax/Clight emit valid-ish C, used as documentation and for the `clightgen` tool that lets users *write* in C and consume the resulting Clight AST as Coq input.
- There is no FileCheck-style framework. Verification replaces what FileCheck does in LLVM/MLIR for transformation correctness.

## What's worth stealing for RegseqModel

- The **multi-IR chain with explicit total-function lowerings** is exactly the layered-IR pattern for a closed-vocabulary compiler. Separating "MIR-mirror layer" from "Regseq-domain layer" as two distinct IR types (not two dialects in one IR) gives strong typing: a function that consumes the lower IR cannot accidentally see a higher-level op.
- A small fixed set of low-level types (`Int | Long | Float | Pointer`-style chunks) for the C-emission layer matches CompCert's Cminor philosophy and aligns well with what a C backend can emit. Push richer types to upper layers.
- Per-stage dump flags (`--emit-mir-mirror`, `--emit-regseq-model`, `--emit-c`) are cheap to add and invaluable for debugging; CompCert's `-dXXX` flags are a good template.
- Treating the IR as an *internal* artifact, not an interchange format, simplifies versioning to "none". The pretty-printer is for humans and snapshot/FileCheck tests, not third-party consumers.

## What's specific to their constraints

- Coq proofs dictate immutable inductive datatypes; Rust gives us the same shape via `enum`s but without proof obligations. We do not need simulation theorems; tests suffice.
- CompCert's source is C, which is far closer to its lowest IR than a Rust spec is to C. Their pipeline can be many small refinements; ours has at least one "semantic gap" (Rust ownership/borrowing -> C) that needs more work per step.
- They never round-trip IR through disk; we may want to dump-and-reload for caching, but should not treat that as a stability surface.
- No optimizations are user-pluggable; passes are fixed and proved. Regseq has fewer passes overall, so this matches.

## Sources consulted

- https://compcert.org/man/ (reference manual, sections on Clight/Cminor/RTL)
- https://github.com/AbsInt/CompCert/tree/master/cfrontend (Csyntax.v, Clight.v, Cminorgen.v, Cshmgen.v)
- https://github.com/AbsInt/CompCert/tree/master/backend (Cminor.v, RTL.v, RTLgen.v)
- https://xavierleroy.org/publi/compcert-backend.pdf (Cminor design, Sections 3-4)
- https://xavierleroy.org/publi/Clight.pdf (Blazy & Leroy on Clight)
- https://xavierleroy.org/publi/compcert-CACM.pdf (high-level pipeline overview, Fig. 1)
