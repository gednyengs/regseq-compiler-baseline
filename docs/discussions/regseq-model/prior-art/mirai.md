# MIRAI

**One-line summary**: Abstract interpreter that runs as a rustc plugin and
analyzes MIR directly (no intermediate IR of its own); uses functional state
maps and per-function summaries.

**URLs**:
- Repo: https://github.com/facebookexperimental/MIRAI
- Architecture doc:
  https://github.com/facebookexperimental/MIRAI/blob/main/documentation/Architecture.md
- Design discussions:
  https://github.com/facebookexperimental/MIRAI/blob/main/documentation/DesignDiscussions.md

## Pipeline shape

- Source language: plain Rust + MIRAI annotation crate
  (`#[contracts]`-style attributes for preconditions/postconditions, e.g.
  `mirai_annotations::checked_assume!`).
- Extraction point in rustc: **MIR** (level not specified in their docs;
  effectively post-borrowck since they read MIR after rustc's type analysis).
- Output: nothing serializable; analysis results are compiler diagnostics
  (errors and warnings) plus an in-memory function summary database.
- How they invoke rustc: a `rustc_driver::Callbacks` plugin
  (`src/callbacks.rs`); replaces rustc as Cargo's compiler.

## IR design

- **No custom IR.** MIRAI operates directly on `rustc_middle::mir::Body<'tcx>`
  and abstract-interprets the CFG.
- The "model" is a functional persistent map from **paths** (think: lvalue
  expressions) to **abstract values**. Per the architecture doc: "a map from
  paths to values as its model of the current state of memory, which is a
  functional data structure, making it cheap to copy".
- Notable types: a `Path` enum (variable, field projection, index, etc.) and
  an `AbstractValue` lattice that mixes concrete and symbolic values --
  effectively a hand-rolled extension of MIR with abstraction lattices.
- Function summaries are stored in an in-memory key-value store
  (incremental, parallel-friendly), keyed on a hash of the function and its
  preconditions.
- Serialization: function summaries are persisted as a binary cache between
  runs; not a published interchange format.

## Key design decisions

- **Monomorphization**: per-call-site specialization. Summaries are cached
  per (function, type-argument) pair; the analyzer specializes summaries when
  inlining at a call.
- **`core` / `std` treatment**: walked when bodies are available; abstracted
  via summaries when they aren't. Where an std function is opaque, MIRAI uses
  its annotation crate to declare contracts (e.g. `Vec::push` has a contract
  spec).
- **Drop handling**: drops are abstract-interpreted just like other MIR
  terminators -- the drop glue body, when reachable, contributes its summary
  to the call site.
- **Panic strategy**: panics are reachability targets. MIRAI emits diagnostics
  when it cannot rule out a panic; uses path conditions to prune
  unreachable panic edges.
- **Unsupported constructs**: rather than rejecting, MIRAI degrades the
  abstraction to *Top* (unknown) and logs a warning. The whole-program
  analysis continues with reduced precision.
- **Reference rules**: references are tracked as paths in the abstract state.
  `&mut` produces a path that can be updated through; `&` produces a
  read-only alias. There is no formal separation logic.
- **Trait dispatch / vtables**: dynamic dispatch is approximated by joining
  summaries of all candidate impls; static dispatch is precise.

## Test methodology

- Custom regression test suite (per the contributor docs); each test is a
  `.rs` source plus an expected diagnostic output.
- Test runner is part of the `mirai` cargo workspace.
- The architecture doc explicitly notes the analysis is *modular* (per
  function) and *incremental* (cached); test runs exercise both the analysis
  and the cache layer.

## What's worth stealing

- **Persistent / functional state on a CFG**: storing the abstract state on
  every CFG edge cheaply via persistent maps is the textbook abstract-
  interpretation pattern. Useful even for codegen if we ever do per-edge
  analyses (e.g. "is this register handle still owned here?").
- **Path-keyed maps as the abstraction over places**: cleaner than indexing
  by `Local` + projection at call sites. Charon and Creusot do similar
  things but less explicitly.
- **Function summaries cached on disk**: incremental compilation is mostly
  free if all per-function output is hashable and serializable.

## What's specific to their constraints

- MIRAI consumes MIR directly with no IR layer because it's an *analyzer*, not
  a transformer. Codegen requires an IR you can transform; we should not skip
  that layer.
- Abstract interpretation tolerates over-approximation by design (Top is a
  legal answer). Codegen cannot -- if we don't know what to emit, we must
  reject.
- The Facebook project is described in its own README as orphaned. Treat its
  design as historical reference, not active practice.
- MIRAI's analysis runs *alongside* normal rustc compilation rather than
  replacing the codegen backend. Regseq replaces the entire downstream
  pipeline, so we have different integration constraints.

## Sources consulted

- https://github.com/facebookexperimental/MIRAI
- `documentation/Architecture.md` (state model, modularity, callbacks
  integration)
- `documentation/DesignDiscussions.md`
- `documentation/Overview.md`
- `src/callbacks.rs`, `src/main.rs` (plugin wiring)
