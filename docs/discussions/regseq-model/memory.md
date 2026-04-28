# Memory

Briefing for an LLM agent landing in this project fresh and needing
to work productively on RegseqModel. Read this in addition to (not
instead of) `README.md`, `architecture.md`, and `prior-art/README.md`
in this folder. Treat as authoritative -- every claim here has been
distilled from extended discussion and is the current state, not
speculation.

This file exists because external memory systems (per-user,
per-tool) do not travel with the codebase; this one does.

## What Regseq is (one paragraph)

A Rust-to-C compiler for OS-agnostic device drivers. Driver authors
write `#![no_std]` Rust specs (with a `#[rseq::]` attribute
namespace and an embedded DSL); the compiler consumes rustc's
`optimized_mir` (with `mir_opt_level=0` and a curated cleanup-pass
whitelist) and produces C for target OSes (Linux, Zephyr, ...).
The bridge between MIR and the C backend is an internal IR called
RegseqModel.

For DSL background, see `regseq-background.md`. For rustc-side
reference, see `rustc-references.md`.

## The current pivot

The compiler has been redesigned. Two important shifts that
previous documents may not yet reflect:

1. **MonoRust dropped.** MonoRust is no longer pursued as a
   separate IR; the compiler consumes `optimized_mir` directly
   into RegseqModel construction. See `../monorust/overview.md`
   (historical reference; the inclusion-principle taxonomy in
   Sections 4 and 6-7 still applies).
2. **RegseqModel itself is being refactored.** The previous
   specification at `../models/regseq-model/spec.md` is
   non-normative reference. The new specification is being built
   bottom-up from MIR/HIR evidence in this folder.

## Two-phase split

A complete RegseqModel is reached through two sequential paths:

1. **Phase 1 -- device-independent (linguistic).** RegseqModel
   specification plus the MIR/HIR -> RegseqModel construction
   pass. Tests are synthetic linguistic snippets (e.g., "how does
   `?` lower?"). Lives in this folder
   (`docs/discussions/regseq-model/`).
2. **Phase 2 -- device pressure.** Authoring golden C for real
   driver specs. pl011 is the first device. Lives at
   `../manual-lowering/`. Each device stress-tests RegseqModel
   synthesis at a holistic level.

Findings from phase 2 that indicate a phase-1 spec gap are
escalated here.

## Settled architectural decisions

Captured in `architecture.md`. One line per decision:

- **Q1 Crate architecture.** Two-crate split: bridge inside
  `regseq-driver` as internal modules; RegseqModel in
  `regseq-model` with zero rustc deps.
- **Q2 `'tcx` boundary.** Eager translation. Bridge produces a
  fully-owned RegseqModel; `'tcx` does not survive past the
  bridge call. No TLS / RefCell-Tables / PhantomData-ZST
  machinery.
- **Q3 IR layering.** Single-level IR; stages refine the same IR.
  Not a CompCert-style layered ladder.
- **Q4 rustc integration point.** `rustc_driver::Callbacks`. Not
  `CodegenBackend` (despite convergent prior art); rationale lives
  in Q4.
- **Q5 Tests + serialization.** `insta` snapshots plus a custom
  textual MIR-inspired pretty-printer (primary) plus serde-derived
  JSON (secondary).
- **Q6 Multi-crate strategy.** `regseq` facade is a separate crate
  (reverting an earlier proof-of-concept collapse). Three-tier
  wrapper dispatch (`PassthroughCallbacks` / `RegseqCallbacks` /
  `RegseqAwareCallbacks`) with cargo-metadata recognition of
  regseq-aware crates via `[package.metadata.regseq]`.
  Infrastructure rebuilt and verified end-to-end; the `regseq`
  crate starts empty, items get promoted from each spec's
  `crate::regseq` submodule incrementally as redesign work
  touches them.
- **Q7 Intrinsic recognition by attribute name.** The bridge
  identifies domain intrinsics by the `NAME` argument of
  `#[rseq::intrinsic("NAME")]` on the callee, not by the callee's
  path. Names form a closed vocabulary maintained in the bridge;
  unknown names are rejected.

These are binding unless explicitly revised. Future architectural
decisions get appended as Q8, Q9, ... in `architecture.md`.

## Methodology rules

These are not folklore; they have been deliberately decided. They
override any inferred convention from the codebase or older docs.

- **`core` is opaque.** The bridge recognizes calls into `core`
  (`Option::map`, `?` desugar, operator traits, `FromResidual`,
  etc.) by lang-item identity or canonical name. We do NOT walk
  `core` MIR. Items outside the recognized vocabulary are
  rejected, not handled by walking.
- **No defensive fallbacks.** The toolchain is pinned. Do not add
  fallbacks to guard against rustc/toolchain regressions or
  "what if X is missing." If something cannot happen given the
  pinned toolchain and the documented spec surface, do not handle
  it -- reject if it appears unexpectedly.
- **Re-derive from evidence; do not import baggage.** When
  specifying RegseqModel, do not lift content from
  `../models/regseq-model/` and treat it as authoritative. The
  older spec is reference material to consult; the current spec
  is built from MIR/HIR observations.
- **Codex review for non-trivial decisions.** Pattern recognition,
  layer attribution, semantic equivalence -- these warrant
  adversarial review. Use `scripts/python/codex_runner.py`. See
  `../../../CLAUDE.md` (project root) for the runner contract.
- **Spec is mutable.** If hand-lowering or specification surfaces
  awkward source-spec constructs, revise the spec rather than
  work around them in the compiler.
- **Compiler-theory-first.** Solve pipeline gaps fundamentally via
  established compiler/language theory; prefer proper IR variants
  over narrow workarounds.
- **Source declares its own compiler-relevant attributes.**
  Crate-level declarations that enable tool-namespace recognition
  (`#![feature(register_tool)]`, `#![register_tool(rseq)]`) live
  in the regseq-aware source itself; neither `RegseqCallbacks` nor
  `RegseqAwareCallbacks` injects them. Symmetric rule across both
  callback paths. Reason: without source-side declarations,
  `cargo check` and `rust-analyzer` on the `regseq` facade (or any
  regseq-aware crate) fail outside the wrapper, because tool
  attributes are unrecognized at parse time. The same principle
  applies to `#![no_std]` and other source-required declarations
  -- if removing the declaration breaks plain rustc invocation,
  it must stay in the source.
- **C semantics with information preservation.** RegseqModel uses
  C-shaped lowerings rather than mirroring Rust-internal compiler
  shapes. Drop-glue calls are regular Calls, trait objects are
  vtables, closures are structs + methods, generics are
  monomorphized away, `impl` blocks are lifted. The bridge does
  the desugaring; consumers see C-shape. Preserve information
  rustc gives us for free even when C doesn't strictly require
  it, but always interrogated per construct: ask what it carries,
  who consumes it, what shape best serves them. Don't pass
  annotations through as opaque source-side metadata. Where
  rustc has done the work (layout resolution, monomorphization,
  region erasure), store the result, not the input. Examples:
  `&mut T` carries `provenance: UniqueBorrow` (the stage-3
  `Pointer` field) so codegen MAY emit `T* restrict` in
  declaration contexts where uniqueness is preserved -- not
  baked into the type unconditionally; `repr(C)` becomes a
  resolved `Layout` on `Ty::Struct`; ABI annotations become an
  `Abi` enum on `FnSig`.

## Operational pointers

- `scripts/dump-fixture-mir.sh <fixture>` -- refresh the MIR dump
  for a fixture. Applies the cleanup-pass whitelist.
- `scripts/python/codex_runner.py` -- non-interactive Codex
  invocation. Full contract in `../../../CLAUDE.md` (project root).
- Pinned toolchain (paths in `rustc-references.md`).
- `crates/regseq-driver/` and `crates/regseq-driver-old/` are the
  only crates with `rustc_private = true`. All other crates are
  rustc-free by Cargo enforcement.
- Existing pl011 fixture: `tests/fixtures/pl011-full-spec/`.
- `cargo check --workspace --tests` is the standard smoke check
  (warnings are expected from the `regseq-driver-old/src/extract/`
  inherited dead code; errors are not).

## Crate layout (post-refactor)

The workspace was reorganized in commit `9352ea0` to give stage-1
implementation a clean grep / autocomplete surface:

- *Active crates (un-suffixed)*: `regseq-model`, `regseq-analysis`,
  `regseq-backends` are stubs (single `lib.rs` each) waiting for
  stage-by-stage population. `regseq-driver` carries only the new
  Q6 multi-crate dispatch layer (`callbacks.rs`, `args.rs`,
  `lib.rs`, `main.rs`, plus the four kept tests).
- *Reference crates (`*-old`)*: `regseq-model-old`,
  `regseq-analysis-old`, `regseq-backends-old`,
  `regseq-driver-old`, `tests/e2e-old` hold the prior IR design.
  Self-contained and fully buildable
  (`cargo build -p regseq-model-old` etc.); not on the active
  build path of the un-suffixed stack. Treat as reference per the
  "re-derive from evidence; do not import baggage" methodology
  rule -- consult deliberately, do not lift wholesale.
- *Unchanged*: `regseq` (empty facade per Q6), `regseq-config`,
  `regseq-diagnostics`, `regseq-macros`, `cargo-regseq`.
- *Tag*: `pre-stage-1-impl-archive` points at the parent of
  `9352ea0` if a future agent needs to inspect the un-renamed
  pre-refactor state directly.

## Where current work stands

See `status.md` for the consolidated tracker (stage progress,
active focus, blockers, next action). At time of writing the
foundation snapshot is: stage-0 architectural decisions Q1-Q7
complete and verified end-to-end; stage-1 spec
(`stage-1-vocabulary.md`) complete after three review rounds;
workspace prepped for implementation (old IR stack moved to
`*-old` siblings, un-suffixed crates wiped to stubs). The next
concrete work is populating `crates/regseq-model/` with the
stage-1 types in foundation-first order. `status.md` is the
source of truth -- this paragraph may go stale.

## Read order for catching up

In priority:

1. This file (`memory.md`).
2. `README.md` -- folder-level orientation.
3. `status.md` -- current stage progress, active focus, next
   action.
4. `architecture.md` -- the architectural decisions (Q1-Q7
   currently) in detail.
5. `prior-art/README.md` -- the synthesis of prior-art research
   that shaped the architectural decisions.
6. `stage-1-research.md` -- prior-art research and locked sub-
   decisions (F1-F4) for stage 1 (vocabulary). Read once stage 1
   work is active.
7. `stage-1-vocabulary.md` -- the stage-1 spec proper.
8. `regseq-background.md` -- DSL background.
9. `findings.md` -- empirical observations about MIR/HIR shape.
10. `rustc-references.md` -- pointers into the pinned rustc-src.
11. Per-project files in `prior-art/` -- consult on demand when a
    specific external project comes up.

## What this file is NOT

- Not a substitute for reading `architecture.md` and the other
  documents linked above. It is a briefing, not the spec.
- Not append-only. Update sections in place when the project state
  changes (decisions added, methodology refined, phase status
  shifts). Stale memory is worse than no memory.
- Not where decisions live. Decisions live in `architecture.md`
  (or future stage-spec docs); this file references them.
