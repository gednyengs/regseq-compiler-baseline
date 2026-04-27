# rust-stainless

**One-line summary**: Experimental EPFL frontend that runs as a rustc driver,
extracts a subset of Rust from HIR/THIR, and emits Stainless (Scala-based
verifier) AST nodes for verification.

**URLs**:
- Repo: https://github.com/epfl-lara/rust-stainless
- Stainless: https://github.com/epfl-lara/stainless
- Master thesis: https://infoscience.epfl.ch/bitstreams/0b5ccf1b-79da-4a61-9907-899d8abadb63/download

## Pipeline shape

- Source language: a subset of Rust, plus `libstainless`/`libstainless_macros`
  which provide attribute-style specs (`#[pre]`, `#[post]`, `#[measure]`).
- Extraction point in rustc: **HIR + THIR**. The lib.rs imports
  `rustc_hir`, `rustc_mir_build::thir`, and `rustc_middle::ty::TyCtxt`.
  Extraction is post-typeck but does *not* go through MIR -- it walks THIR for
  expression bodies. Source: `stainless_extraction/src/lib.rs`.
- Output: Stainless `Symbols<'l>` (a collection of `ClassDef`,
  `FunDef`, `ADTSort`, `TypeClass`), serialized to a binary file and consumed
  by Stainless running on the JVM.
- How they invoke rustc: as an alternative driver. The `stainless_frontend`
  crate registers a `rustc_driver::Callbacks` impl, runs standard
  type-checking, then calls `extract_crate(tcx, factory, name)` to walk HIR.

## IR design

- The "IR" is **Stainless's own AST**, with Rust-side mirror types in
  `stainless_data::ast` (a faithful Rust transcription of Stainless's Scala
  case-class hierarchy).
- Lifetime parameter `'l` represents the arena/factory lifetime
  (`Symbols<'l>`, `Factory`); IR nodes are arena-allocated, not `'tcx`-tied.
- Notable types: `FunDef`, `ClassDef`, `ADTSort`, `Type`, `Expr`,
  `SymbolIdentifier`, `ValDef`. There is also a `synth` module
  (`stainless_extraction/src/synth/`) producing synthetic items for
  type-class encodings.
- Serialization: a Stainless-defined wire format consumed by `stainless-noxt`
  (no-extraction mode); implemented in the `stainless_data` and
  `stainless_backend` crates. The frontend emits `.json`/binary then invokes
  Stainless on the JVM.

## Key design decisions

- **Monomorphization**: not done. Generics are translated to type-parameter
  `ClassDef`/`FunDef`s; type classes encode trait constraints. The thesis
  (Chapter 3) describes the type-class encoding in detail.
- **`core` / `std` treatment**: explicit allow-list via `std_items.rs` --
  `StdItems::collect(tcx)` enumerates the supported `core`/`std` items.
  Anything not on the list is unsupported and surfaces as an error. There
  is no transparent walking into std bodies.
- **Drop handling**: ignored. Stainless verifies pure functional Scala; Rust
  Drop semantics has no encoding. Drop-relevant constructs are typically
  rejected.
- **Panic strategy**: panics are verification obligations -- the verifier
  attempts to prove unreachability. Compile-time `assert!` calls become
  Stainless assertions.
- **Unsupported constructs**: rejected with a span-tagged error
  (`MultiSpan`); the extraction is fail-fast on unsupported THIR shapes.
- **Reference rules**: thesis describes a translation from `&T`/`&mut T` into
  Stainless reference cells with a special imperative-fragment encoding;
  borrows are heavily restricted to a tractable subset.
- **Trait dispatch / vtables**: encoded as type classes (Stainless type
  classes mirror Scala traits with implicits). `classes.rs` and `fns.rs`
  contain the encoding. `dyn Trait` is not supported in any meaningful form.

## Test methodology

- `stainless_frontend/tests/pass/` -- programs expected to verify.
- `stainless_frontend/tests/fail/` -- programs expected to fail extraction
  or verification.
- Each `.rs` file is fed to the frontend; expected outcomes encoded by
  directory placement and per-file directives.
- Tests are described in the README as the de-facto spec for what works.
  The supported fragment is small enough that the test corpus is the
  authoritative documentation.

## What's worth stealing

- **Run as an "alternative rustc driver" with extraction post-typeck**: this
  is the most lightweight extractor pattern -- a `Callbacks` impl that runs
  rustc's normal pipeline through type checking, then walks HIR. Useful as
  a baseline architecture sketch.
- **Explicit allow-listing of std/core items via `StdItems`**: pattern is
  similar to what we'll need for `core::ptr::*`, intrinsics, traits. A
  centralized item-id-to-internal-tag map is much easier to maintain than
  scattered string-matching.
- **A "synth" module for compiler-generated items**: when extraction needs
  to invent items (e.g. type-class instances, vtable shims), having a
  dedicated module that synthesizes them keeps the main translator clean.

## What's specific to their constraints

- THIR-level extraction is too high. THIR is pre-MIR and lacks the explicit
  control-flow that drivers need; we want MIR.
- Stainless's pure-functional encoding means they erase Drop entirely. Our C
  output has destructor-like obligations (deinit ordering, irq disable).
- Type-class encoding of traits is verification-specific -- Stainless
  understands type classes natively, so encoding traits as them is cheap.
  C output has no trait-equivalent; we monomorphize.
- The project is "experimental" and "very limited" in its supported subset
  per its own README. It is a weaker reference than Charon/Kani/Creusot.

## Sources consulted

- https://github.com/epfl-lara/rust-stainless (top-level layout, README)
- `stainless_extraction/src/lib.rs` (extract_crate entry, module imports)
- `stainless_extraction/src/{krate.rs, fns.rs, classes.rs, ty.rs, expr/,
  std_items.rs, synth/}`
- `stainless_data/`, `stainless_backend/`, `stainless_frontend/`
- "Formal Verification of Rust with Stainless" (EPFL master thesis, 2021),
  Chapters 2-3 for IR mapping and type-class encoding
