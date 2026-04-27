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
- `architecture.md` -- settled stage-0 architectural decisions
  (Q1-Q5: crate layout, `'tcx` boundary policy, IR layering,
  rustc integration point, tests + serialization format). Treat
  as binding unless explicitly revised.
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
architectural foundation is complete (Q1-Q5 in `architecture.md`);
stage 1 (vocabulary) is the next work to start.

Stage 1 (vocabulary) not yet started. Methodology and per-construct
decision log not yet authored. The three findings carried over with
the migration are starter material -- they may be re-validated,
refined, or displaced as phase-1 work proceeds.

## Cross-session continuity

A new session should:

1. Read `memory.md` for the self-contained project briefing
   (pivot context, methodology rules, operational pointers, read
   order).
2. Read this `README.md` for folder-level orientation.
3. Read `status.md` for current stage progress, active focus, and
   next action.
4. Read `architecture.md` for settled stage-0 decisions (Q1-Q5).
5. Skim `prior-art/README.md` for the synthesis of prior-art
   research that shaped the architectural decisions.
6. Skim `regseq-background.md` for DSL framing.
7. Skim `findings.md` for prior empirical observations.
8. Consult `rustc-references.md` when a rustc-internal question
   surfaces.
9. Pick up from `status.md`'s "Next action"; update `status.md`
   at session end if anything material changed.
