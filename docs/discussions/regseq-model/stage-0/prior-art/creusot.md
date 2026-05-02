# Creusot

**One-line summary**: Deductive verifier for Rust that translates pre-borrowck
MIR into "fMIR" (a Creusot-internal IR) and then into Coma (a Why3 verification
language) for SMT-backed proof.

**URLs**:
- Repo: https://github.com/creusot-rs/creusot
- Site: https://creusot.rs/
- Architecture doc: https://github.com/creusot-rs/creusot/blob/master/ARCHITECTURE.md
- Paper (SAS 2022): https://jhjourdan.mketjh.fr/pdf/denis2022creusot.pdf

## Pipeline shape

- Source language: Rust + Pearlite spec language (logical assertions written in
  Rust syntax, embedded as macros / closures and erased before borrowck).
- Extraction point in rustc: **MIR**, but via
  `rustc_borrowck::consumers::get_body_with_borrowck_facts`, which yields the
  pre-optimization MIR *together with* Polonius borrow facts. They override
  `mir_built` and `mir_borrowck` in `creusot/src/callbacks.rs` so that
  Pearlite spec closures are stripped before drop elaboration runs (otherwise
  rustc would insert drops for non-Drop ghost types and crash).
- Output: WhyML/Coma source files (`*.coma`), one per crate or per-module,
  consumed by Why3 + SMT solvers.
- How they invoke rustc: three binaries layered like `flowistry`:
  `cargo-creusot` (cargo extension) -> `creusot-rustc` (rustc wrapper) ->
  `creusot-driver` (the actual `Callbacks` impl in `creusot/src/callbacks.rs`).

## IR design

- **Owned-ish but borrows from `'tcx`**. Their IR is called **fMIR**
  (functional MIR). Defined in `creusot/src/translation/fmir.rs`. It is a
  Rust-side data structure carrying `'tcx` lifetime parameters
  (`Place<'tcx>`, `RValue<'tcx>`, `StatementKind<'tcx>`, `Body<'tcx>`).
- Shape relative to MIR: still a CFG of basic blocks, but with assignments
  expressed as functional-style `RValue`s, prophecy borrows
  (`MutBorrow(BorrowKind, Place, Place)`), explicit assertions, snapshots, and
  ghost code as first-class statement kinds.
- Notable variants/types:
  - `BorrowKind::{Mut, Final(usize)}` -- "Final" tracks whether a reborrow can
    inherit the prophecy id of its source.
  - `StatementKind::{Assignment, MutBorrow, Assertion {cond, check, assume},
    ...}`.
  - Pearlite `Term` IR (`creusot/src/translation/pearlite.rs`) is a separate
    pure logical-expression language used inside specs.
- Serialization: there is a metadata format (`creusot/src/metadata.rs`,
  `very_stable_hash.rs`) for cross-crate spec import; the main output is text
  WhyML.

## Key design decisions

- **Monomorphization**: not done. Generic items translate to WhyML modules
  parameterized over opaque types; trait impls become module clones. The
  prophecy semantics for `&mut T` is the centerpiece -- a mutable borrow is
  modeled as a pair `(current, final)`, and `^x` reads the prophesied final
  value, sidestepping separation logic.
- **`core` / `std` treatment**: hybrid. `creusot-std` provides hand-written
  WhyML specifications for std items consumers actually call. Other std
  items are opaque. There is an `external.rs` module dedicated to translating
  references to external (non-current-crate) items.
- **Drop handling**: drops are largely *not* preserved as runtime operations.
  Pearlite spec closures are removed before drop elaboration so that ghost
  code never gets drop glue. The functional WhyML target has no notion of
  destructors; resource correctness is encoded via prophecy and ownership
  invariants instead.
- **Panic strategy**: assertions (`Assertion { check: true }`) generate
  verification conditions; user `panic!` calls become unreachable proof
  obligations. There is no runtime panic semantics.
- **Unsupported constructs**: explicit `TranslationError` enum with span-aware
  fatal errors. Example: `PtrDeref` -> `dcx().span_fatal(...)`. Raw pointer
  dereference is rejected wholesale.
- **Reference rules**: shared `&T` becomes a value (read-only); `&mut T`
  becomes a `(current, final)` pair with prophecies. Reborrows can inherit
  prophecy ids when "final-place" analysis proves the source is dead. This
  is implemented in `creusot/src/analysis/` (e.g. `NotFinalPlaces`).
- **Trait dispatch / vtables**: static dispatch becomes a WhyML module clone
  per impl. `dyn Trait` support is documented as in-flux. Trait method calls
  go through a resolver pass (`backend/projections.rs`,
  `translation/traits.rs`).

## Test methodology

- Snapshot tests under `tests/` (organized by feature -- `should_succeed/`,
  `should_fail/`).
- Each Rust test file has a corresponding generated `.coma` (and historically
  `.mlcfg`) golden file.
- A regression script runs Creusot, then runs Why3 on the generated WhyML and
  compares results to expected verdicts.
- Backend hashing (`very_stable_hash.rs`) used to detect spurious diffs in
  generated output across rustc versions.

## What's worth stealing

- **`mir_built` override to delete artificial bodies before drop elaboration**:
  this is the cleanest example of "the *type* of MIR you want to consume
  depends on what you stripped before rustc generated drop glue". Highly
  relevant if Regseq ever needs to remove DSL-only constructs prior to
  optimization.
- **A typed `TranslationError` enum** with span-aware emission rather than
  panicking. Spans are propagated through context so errors can be raised
  late but blamed correctly.
- **Stripping spec closures via `cleanup_spec_closures`** as the canonical
  pattern for excising DSL-only artifacts from MIR. Even though we don't have
  Pearlite, Regseq has DSL constructs (declared registers, intrinsics) that
  may need similar surgery.
- **Borrow facts via `rustc_borrowck::consumers::get_body_with_borrowck_facts`**
  is the documented public hook for getting Polonius output -- a useful API to
  know about even if we don't currently need it.

## What's specific to their constraints

- The fMIR is `'tcx`-borrowed. We explicitly want an owned IR (the cleanup
  whitelist + downstream backend = serializable, no `'tcx`). Don't copy the
  borrowed shape.
- Prophecy semantics for `&mut` is a verification-only construct. C codegen
  cares about the actual pointer, not the prophesied final value.
- Generics are kept polymorphic at the IR level. We monomorphize.
- Drops are mostly erased because pure functional Why3 has no destructors.
  We must preserve drop ordering for resource cleanup in C.
- Pre-optimization MIR (`mir_built`) is used. We use `optimized_mir`. Their
  reasons (preserve structure, control over what got stolen) partially apply,
  but our cleanup whitelist achieves a similar effect downstream.

## Sources consulted

- https://github.com/creusot-rs/creusot/blob/master/ARCHITECTURE.md
- `creusot/src/lib.rs`, `creusot/src/callbacks.rs`
- `creusot/src/translation/fmir.rs` (StatementKind, Place, BorrowKind)
- `creusot/src/translation/function.rs` (BodyTranslator)
- `creusot/src/translation/{pearlite.rs, traits.rs, external.rs}`
- `creusot/src/cleanup_spec_closures.rs`, `metadata.rs`,
  `very_stable_hash.rs`
- Denis et al., "Creusot: A Foundry for the Deductive Verification of Rust
  Programs", SAS 2022 (jhjourdan.mketjh.fr/pdf/denis2022creusot.pdf)
