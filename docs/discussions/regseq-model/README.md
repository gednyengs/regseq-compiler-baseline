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

- `README.md` -- this file.
- `memory.md` -- briefing for an LLM agent landing fresh in this
  project. Self-contained context handoff; the right entry point
  for any new session.
- `status.md` -- consolidated status tracker for phase-1 work
  (stage progress, active focus, blockers, next action). Update
  at the end of every session that changes anything material.
- `architecture.md` -- settled architectural decisions (Q1-Q7
  currently: crate layout, `'tcx` boundary policy, IR layering,
  rustc integration point, tests + serialization format,
  multi-crate strategy, intrinsic recognition by attribute name).
  Treat as binding unless explicitly revised; future decisions
  append as Q8, Q9, ...
- `stage-1-research.md` -- prior-art research synthesis for stage 1
  (vocabulary): four-cluster survey (rustc internals, Rust-to-C
  compilers, Rust verification IRs, general IR design), convergent
  patterns settled by precedent, and the four locked sub-decisions
  (F1-F4: span byte representation, span placement, source content,
  ItemId determinism). Evidence base for the stage-1 spec.
- `stage-1-vocabulary.md` -- the stage-1 spec proper. ID conventions,
  the StringInterner, SourceFile and Span types, DefPath form,
  Instance form, top-level RegseqModel struct, determinism rules,
  serialization conventions, pretty-printer interface sketch.
  Type signatures are sketches; field names and minor details are
  at the implementation's discretion.
- `stage-1-review-log.md` -- summary of two prior Codex adversarial
  review rounds against the stage-1 work and how each issue was
  addressed. Useful for future reviewers (don't re-litigate
  resolved concerns) and for a future agent picking up the work.
- `stage-1-review-prompt.md` -- self-contained prompt for an
  external reviewer (no repo access). Pairs with the file
  attachments listed inside it.
- `rustc-excerpts.md` -- curated rustc-src quotations the stage-1
  spec depends on, gathered for an external reviewer.
  Authoritative for rustc-internal claims when the reviewer
  cannot open the rustc-src tree directly.
- `regseq-background.md` -- DSL background: `#![no_std]` spec
  environment, the three mechanisms (intrinsics, properties,
  traits).
- `rustc-references.md` -- pointers into the pinned rustc-src for
  designing the construction pass.
- `findings.md` -- empirical observations about MIR/HIR shape;
  append-only.
- `prior-art/` -- per-project research files plus a synthesis
  README identifying convergent patterns, the (now resolved)
  forks in the road, and implications for the bottom-up plan.

Methodology rules, the decision log, and the spec itself accumulate
here as evidence comes in.

## Status

See `status.md` for the consolidated status tracker (stage progress,
active focus, blockers, next action). Brief snapshot: stage-0
architectural foundation complete (Q1-Q7 in `architecture.md`);
stage-1 spec (`stage-1-vocabulary.md`) complete after three review
rounds, sixteen issues addressed (see `stage-1-review-log.md`);
workspace prepped (old IR stack moved to `*-old` sibling crates,
un-suffixed crates wiped to stubs). Stage-1 implementation in
`crates/regseq-model/` is the next concrete work unit;
foundation-first order is documented in `status.md`.

Methodology and per-construct decision log not yet authored. The
three findings carried over with the migration are starter material
-- they may be re-validated, refined, or displaced as phase-1 work
proceeds.

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
7. Skim `findings.md` for prior empirical observations.
8. Consult `rustc-references.md` when a rustc-internal question
   surfaces.
9. Pick up from `status.md`'s "Next action"; update `status.md`
   at session end if anything material changed.
