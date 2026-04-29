# RegseqModel

The home for the RegseqModel refactor against rustc's `optimized_mir`
(with `mir_opt_level=0`) and the dump-script cleanup whitelist. The
older RegseqModel material at `docs/discussions/models/regseq-model/`
is non-normative reference; this folder is where the new model takes
shape.

## Why this folder exists

The pre-existing RegseqModel was designed against a pre-monomorphization
view of MIR, which left source-level constructs opaque in the IR. The
shift to `optimized_mir` plus a curated cleanup-pass whitelist exposes
the constructs that actually matter for driver lowering. RegseqModel
needs to be re-shaped against that evidence rather than carried
forward from the prior design.

The methodology is evidence-based: the spec, methodology rules, and
decision log accumulate as MIR/HIR observations come in, not by
importing the older spec. Prior decisions stand only when they survive
re-derivation against the new evidence.

## Two phases

A complete RegseqModel is reached through two sequential paths:

1. **Device-independent (linguistic).** RegseqModel specification and
   the MIR/HIR -> RegseqModel construction pass. Tests at this layer
   exercise specific Rust language features in isolation -- the `?`
   desugar, `Option::map`, `&self` vs `&mut self`, generic
   monomorphization, trait dispatch -- with synthetic `#![no_std]`
   spec snippets. No device involved.
2. **Device pressure.** Authoring golden C for real driver
   specifications stress-tests RegseqModel synthesis at a holistic
   level. pl011 is the first device. See
   `docs/discussions/manual-lowering/` for that work.

This folder is the home of phase 1. Findings from phase 2 that
indicate a phase-1 spec gap are escalated here.

## Contents

Layout: cross-stage docs at top level; per-stage discussion
artifacts in `stage-N/` subfolders. Each stage's folder is created
lazily when its first content lands (which can be an earlier
stage's forward expectation; see `memory.md` for the convention).

### Top-level (cross-stage)

- `README.md` -- this file.
- `memory.md` -- briefing for an LLM agent landing fresh in this
  project. Self-contained context handoff; the right entry point
  for any new session. Holds the methodology rules.
- `status.md` -- consolidated status tracker for phase-1 work
  (stage progress, active focus, blockers, next action). Update
  at the end of every session that changes anything material.
- `architecture.md` -- settled architectural decisions (Q1-Q7
  currently: crate layout, `'tcx` boundary policy, IR layering,
  rustc integration point, tests + serialization format,
  multi-crate strategy, intrinsic recognition by attribute name).
  Treat as binding unless explicitly revised; future decisions
  append as Q8, Q9, ...
- `regseq-background.md` -- DSL background: `#![no_std]` spec
  environment, the three mechanisms (intrinsics, properties,
  traits).
- `rustc-references.md` -- pointers into the pinned rustc-src for
  designing the construction pass.
- `prior-art/` -- per-project research files plus a synthesis
  README identifying convergent patterns and implications for the
  bottom-up plan.

### Per-stage subfolders

- `stage-1/` -- stage-1 (vocabulary) discussion artifacts.
  - `vocabulary.md` -- the stage-1 spec proper. ID conventions,
    StringInterner, SourceFile/Span, DefPath form, the
    Rust -> C lowering reference, `impl` lifting principle,
    Instance / InstanceKind / monomorphization, bridge-synthesized
    Items, top-level RegseqModel, F4/F4-2 determinism with pinned
    rank tables, serialization conventions, pretty-printer
    interface sketch.
  - `research.md` -- prior-art synthesis (rustc internals,
    Rust-to-C compilers, verification IRs, IR design) and the four
    locked sub-decisions F1-F4.
  - `review-log.md` -- record of seven adversarial review passes
    (R1-R6 + R5.7-R5.21).
  - `review-prompt.md` -- self-contained prompt for an external
    reviewer (no repo access).
  - `findings.md` -- empirical observations about MIR/HIR shape
    that informed stage-1 decisions; append-only.
  - `rustc-excerpts.md` -- curated rustc-src quotations stage 1
    depends on. Authoritative for rustc-internal claims.
  - `expectations.md` -- incoming forward expectations from other
    stages (currently: none; stage 1 closed before any later stage
    surfaced retrospective concerns).

Future stage folders (`stage-3/`, `stage-4/`, `stage-9/`,
`stage-10/`, `stage-11/`, `stage-12/`, `stage-13/`) follow the
same pattern. Each may contain its own `vocabulary.md`,
`research.md`, `findings.md`, `rustc-excerpts.md`, etc., and an
`expectations.md` listing forward refs accumulated from earlier
stages.

Methodology rules, the decision log, and per-stage specs all
accumulate here as evidence comes in.

## Status

See `status.md` for the consolidated tracker. Brief snapshot:
stage-0 architectural foundation complete (Q1-Q7 in
`architecture.md`); stage-1 spec (`stage-1/vocabulary.md`)
implementation-ready after seven review passes (see
`stage-1/review-log.md`); stage-1 implementation in
`crates/regseq-model/` is closed -- all seven foundation steps
committed; 229 integration tests passing. Next-action menu: stage 3
(Type system), bridge implementation, or stage 13 (domain
extensions); see `status.md` "Next action" for the current pick.

## Cross-session continuity

A new session should:

1. Read `memory.md` for the self-contained project briefing
   (pivot context, methodology rules, operational pointers, read
   order).
2. Read this `README.md` for folder-level orientation.
3. Read `status.md` for current stage progress, active focus, and
   next action.
4. Read `architecture.md` for settled architectural decisions
   (Q1-Q7 currently).
5. Skim `prior-art/README.md` for the synthesis of prior-art
   research that shaped the architectural decisions.
6. Skim `regseq-background.md` for DSL framing.
7. Skim `stage-1/findings.md` for prior empirical observations
   that fed stage-1 design.
8. Consult `rustc-references.md` when a rustc-internal question
   surfaces.
9. **If picking up a specific stage N**: read
   `stage-N/expectations.md` as your first action under that
   stage. It lists incoming forward refs earlier stages surfaced;
   treat as a checklist and tick entries off as you address them.
10. Pick up from `status.md`'s "Next action"; update `status.md`
    at session end if anything material changed.
