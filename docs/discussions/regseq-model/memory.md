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
- **Code does not reference discussion-folder paths.** Code
  comments must NOT cite `docs/discussions/regseq-model/...`
  paths, file names like `stage-1/vocabulary.md`, section titles,
  or line numbers. Discussion docs evolve; line numbers and
  section titles drift; nothing in the build catches a stale
  citation. Two acceptable patterns: (a) restate the rule
  inline so the comment stands alone; (b) reference a stable
  architectural-decision tag (`Q1`-`Q7`, `F1`-`F4`) whose name
  does not depend on file layout. The frozen `*-old` reference
  crates retain their historical citations untouched as
  archived artifacts; the rule is forward-looking.
- **Forward expectations live in `stage-N/expectations.md`.**
  When a stage surfaces a concern or follow-up that a later
  stage must address, the entry is appended to the relevant
  later stage's `expectations.md` (incoming forward refs --
  things prior stages have asked stage N to deliver). Stage
  folders are created lazily: the first content for a stage
  -- including an inbound expectation written by an earlier
  stage -- creates the folder. Stage implementer's first
  action when opening their stage: read `stage-N/expectations.md`
  as a checklist; tick entries off as they're addressed.
  Outbound expectations (stage X writing about stage Y) are
  appended to `stage-Y/expectations.md`, not stage X's own
  folder. Cross-stage scaffold information that has no clear
  later-stage owner stays in `memory.md` here under "Known
  follow-ups" or in `status.md` deferral notes.

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
- `cargo test -p regseq-model` runs the active stage-1 crate's
  integration tests (229 tests across the ten test files
  `tests/idx_round_trip.rs`, `tests/index_vec.rs`,
  `tests/string_interner.rs`, `tests/source.rs`,
  `tests/crate_info.rs`, `tests/def_path.rs`,
  `tests/instance.rs`, `tests/model.rs`,
  `tests/render_def_path.rs`, and `tests/pretty.rs` as of
  stage-1 close). Use this during stage-3+ implementation
  iteration; the workspace-wide `cargo test` is noisier due to
  the `*-old` crates.

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
active focus, blockers, next action). `status.md` is the source
of truth -- this paragraph may go stale.

Snapshot at time of writing:

- Stage 0 (Q1-Q7) closed.
- Stage 1 spec: implementation-ready after **seven review
  passes** (Codex round 1 against research; Codex rounds 2/4
  against the spec; external round 3; Codex round 5 +
  external round 5e + Codex round 6 across the round-5
  monomorphization-and-C-semantics revision). All 39 issues
  addressed. See `stage-1/review-log.md`.
- Stage 1 implementation: **all 7 steps complete and
  committed** in `crates/regseq-model/` (`b8f1e67`, `fd0d3c8`,
  `82aaaed`, `f34df65`, `f8e7b4e`, `d78db53`, the step-6
  commit, plus the step-7 commit accompanying this update).
  - Step 1: `Idx` trait + `newtype_index!` macro + niche
    encoding via `NonMaxU32` + `CrateId`.
  - Step 2: `IndexVec<I, T>` with serde-transparent flat
    array.
  - Step 3: `StringInterner` + `StringId`; pre-interns empty
    string at `StringId::ZERO`.
  - Step 4: `SourceFile` + `Position` + `Span` + `SourceFileId`
    (with `SYNTHESIZED` alias). Resolution methods.
  - Step 5: forward-declared `ItemId` / `BodyId` / `TyId` /
    `ConstId` / `FnSigId` / `InstanceId`; `StableCrateId` +
    `CrateInfo`; `DefPathData` (19 variants) + `DefPathSegment`
    + `DefPath` + `DefPathHash`; `def_path_data_sort_rank`
    (0..=18); `GenericArg` + `Instance` + `InstanceKind` (six
    variants); `regseq_instance_kind_rank` (0..=5);
    `RustcInstanceKindTag` + `rustc_instance_kind_sort_rank`
    (0..=14, kept inside regseq-model so all rank values are
    pinned in one place; bridge converts the live rustc enum
    to a tag at the boundary).
  - Step 6: top-level `RegseqModel` + `with_primary`
    constructor (interns `<synthesized>` and primary crate
    name through the model's own interner) +
    `validate(&self)` enforcing the stage-1-checkable subset
    of the 10 spec invariants. Empty placeholder structs
    `Item` / `Ty` / `Const` / `FnSig` / `Body` ship as
    forward-decls for stages 3, 4, 11, 12.
  - Step 7: canonical `render_def_path(path, crates, strings)
    -> String` helper in `def_path.rs` -- pinned per-variant
    rendering against `nightly-2026-02-01`
    `rustc_hir/src/definitions.rs:185-204, :248-249, :505-520`
    (anonymous-namespace strings: `impl`, `extern`, `use`,
    `global_asm`, `closure`, `constructor`, `constant`,
    `constant`, `opaque`, `synthetic`, `nested`; AnonAssocTy
    hybrid `name::{anon_assoc#N}`; DesugaredAnonymousLifetime
    literal `'_` with named-segment disambig). New
    `pretty.rs` ships the `PrettyPrinter<'a>` scaffold per
    spec lines 1323-1348, with stage-1 functional methods
    (`render_span`, `render_def_path_value`) and stage-N
    stubs naming the stage that fills them.
    `RegseqModel::pretty()` is the convenience accessor.
    Plan went through three Codex rounds before
    implementation (BLOCK -> BLOCK -> APPROVE) +
    implementation review (MINOR; one missing negative test
    added).
  - 229 integration tests passing across ten test files;
    `cargo check --workspace --tests` clean.
- Phase 2 (pl011 device pressure) not yet started; gated on
  enough of stages 3, 4, 11, 12 to author golden C against the
  fixture.

Known follow-ups surfaced during stage 1 that are deferred:

- The `newtype_index!` macro currently accepts only empty-body
  invocations (`pub struct $Name {}`). When step 11 introduces
  `LocalId::RETURN_PLACE` and `BlockId::START_BLOCK` const
  associates, the macro will need a const-body arm. Plan-agent
  flagged this in the step-1 plan; deferred per "don't add
  features beyond what the task requires."
- `serde` `rc` feature was added in step 4 to derive serde for
  `Option<Arc<str>>` on `SourceFile.content`. The
  `StringInterner` deserialize path still goes through
  `Vec<String>` (one extra allocation per entry); could be
  refactored to `Vec<Arc<str>>` directly using the `rc` feature.
  Codex flagged as optional/non-blocking; deferred.
- Construction invariants 3, 6, and 9 (and the
  spec_crate_stable_id half of invariant 4) are deferred in
  `RegseqModel::validate` until stage 12 (`Item.def_path`),
  stage 9/12 (sites holding `InstanceId`), and the bridge's
  `BridgeInput`. Each deferred invariant has an in-source
  comment naming the stage that will close the gap.

## Read order for catching up

In priority:

1. This file (`memory.md`).
2. `README.md` -- folder-level orientation.
3. `status.md` -- current stage progress, active focus, next
   action.
4. `architecture.md` -- the architectural decisions (Q1-Q7
   currently) in detail.
5. `stage-1/vocabulary.md` -- the stage-1 spec (round-5
   revised; covers Instance/InstanceKind, the DefPath family,
   the Rust -> C lowering reference, and `impl` lifting).
6. `stage-1/review-log.md` -- seven review passes documented
   (R1-R6 + R5.7-R5.21). Skim before adding new revisions so
   prior resolutions aren't re-litigated.
7. `prior-art/README.md` -- synthesis of prior-art research
   that shaped the architectural decisions.
8. `stage-1/research.md` -- prior-art research and locked sub-
   decisions (F1-F4) for stage 1.
9. `stage-1/rustc-excerpts.md` -- curated quotations from the pinned
   rustc-src for the rustc-internal claims the spec depends on
   (now covers Idx, Symbol, Span / SourceMap, DefId, DefPath /
   DefPathData, DefPathHash, StableCrateId, Instance /
   InstanceKind, tcx.symbol_name, VtblEntry).
10. `regseq-background.md` -- DSL background.
11. `stage-1/findings.md` -- empirical observations about MIR/HIR shape.
12. `rustc-references.md` -- pointers into the pinned rustc-src
    (paths, not excerpts).
13. Per-project files in `prior-art/` -- consult on demand when
    a specific external project comes up.

For an agent picking up stage-3 (Type system) implementation
specifically: the load-bearing reads are 1, 3, 5, 6, and the
sections of 9 covering `Ty` representation; the empty `Ty`
placeholder lives in `crates/regseq-model/src/placeholders.rs`
and graduates to its own module once expanded.

## What this file is NOT

- Not a substitute for reading `architecture.md` and the other
  documents linked above. It is a briefing, not the spec.
- Not append-only. Update sections in place when the project state
  changes (decisions added, methodology refined, phase status
  shifts). Stale memory is worse than no memory.
- Not where decisions live. Decisions live in `architecture.md`
  (or future stage-spec docs); this file references them.
