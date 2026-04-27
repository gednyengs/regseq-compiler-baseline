# Methodology

The rules that govern phase-2 device-pressure hand-lowering for the
pl011 fixture. Phase 1 (device-independent linguistic work on the
RegseqModel spec and construction pass) lives at
`docs/discussions/regseq-model/` and develops its own methodology as
evidence accumulates; do not import rules from there.

New sessions should read this document in full before contributing.

## 1. Disable-and-keep, not delete-and-rewrite

The post-HIR/MIR pipeline (the dormant `crates/regseq-driver/src/construction/`,
`crates/regseq-model/`, `crates/regseq-backends/` paths plus their associated
tests) is preserved as a reference oracle during the redesign, not deleted.
The per-crate `post-monorust-tests` cargo feature gates the dormant tests off
by default; they remain re-enable-able for cross-checks against the new
pipeline as it grows.

**Why**: the dormant pipeline encodes invariants and edge cases that aren't
documented elsewhere. When the new pipeline encounters an unfamiliar MIR
shape, the old pipeline's handling is the cheapest cross-check available.

**Practical implication**: do not modify dormant code to make the new pipeline
pass through it. The new pipeline is built fresh and replaces the old one
atomically when the retirement criterion is met.

## 2. The MIR shape we hand-lower against is a sandbox

`scripts/dump-fixture-mir.sh` applies a cleanup-pass whitelist deliberately
ahead of what the wrapper's `regseq_mir_passes()` applies. The script's
flag set is the canonical "clean MIR" shape; the wrapper stays conservative
until the new extractor is designed against the cleaned shape.

**Why**: the dormant `try_folding` recognizer in `construction/` keys on
the uncleaned MIR shape. Modifying it to handle cleaned MIR before the
new extractor exists would be wasted motion: when the new RegseqModel
construction encodes its own recognition strategy, that strategy is
informed by hand-lowering against the cleaned shape, and the dormant
recognizer becomes irrelevant either way.

**Practical implication**: hand-lower against `<fixture>/dump/` output.
Do not regenerate dumps using the wrapper-driven path; that emits the
uncleaned shape and serves a different purpose.

## 3. Decisions are classified by owning layer

Every captured decision must name the layer that owns it. The two layers
that survive after the MonoRust pivot:

- **RegseqModel construction** -- the single pass that consumes rustc's
  `optimized_mir` and produces RegseqModel. Owns both the Rust-shape
  filtering (what MIR constructs are kept, dropped silently, rejected,
  or invisible at MIR level) AND the domain-shape recognition
  (intrinsics, ordered regions, register access, ISR classification,
  error cleanup analysis, callback context inference). Driven by
  the inclusion principle preserved in
  `docs/discussions/monorust/overview.md` Section 4 and the domain
  rules in `docs/discussions/models/regseq-model/spec.md`.
- **C backend** -- decisions about C rendering: type lowering (`const T*`
  vs `T*`, `restrict`, `_Bool` vs `uint8_t`), function name mangling,
  ordering-region barriers, error-cleanup goto chains, register field
  bit-shift expressions.

Decisions that don't classify cleanly between these two layers are an
open question -- record them as such rather than forcing a choice.

Within RegseqModel construction, two sub-categories may be useful when
the distinction matters: **Rust-shape filtering** (kept/dropped/rejected
at the MIR level) and **domain recognition** (intrinsics, ordered
regions, register access, etc.). When relevant, note the sub-category
in the decision entry; otherwise just say "RegseqModel construction."

**Why**: a flat "MIR pattern -> C output" cookbook is negative-value debt.
It captures the surface transformation but locks in wrong-layer thinking
that the eventual compiler must unwind. Layered classification produces
direct input to two separate design surfaces.

**Practical implication**: every decision-log entry includes an explicit
"owning layer" field. If you are unsure, log the alternatives and ask in
the next session.

## 4. Adversarial review for non-trivial decisions

Per the conversation that established the methodology: when a decision is
non-trivial (pattern recognition, layer attribution, semantic equivalence),
the captured entry is reviewed by an independent agent against the rules
in this document. The reviewer answers a discrete category lookup, not a
judgment call. Trivial uniform rules (drop StorageLive, drop unwind
annotations) do not require review; capture them once at the methodology
level and apply implicitly.

The reviewer's role:
1. Confirm the disposition matches the inclusion-principle category cited.
2. Confirm the owning layer attribution matches the kind of decision
   being made.
3. Flag if the entry hides a non-local analysis as a local decision
   (the trap noted in the conversation: hand-lowering "fixes" a
   cleanup chain inline because it is obvious to the human, but the
   compiler cannot do the same without a real analysis).

## 5. Treat `core` calls as opaque

Per the RegseqModel spec and the inclusion-principle notes preserved
in `docs/discussions/monorust/overview.md` Section 5.1, calls into
`core` (e.g. `Option::map`, `Result::and_then`, `Try::branch`,
`FromResidual::from_residual`) are recognized by lang-item lookup or
by canonical name, not by walking their MIR bodies. Hand-lowering
follows the same rule: do not consult `core`'s MIR. Treat the call
as "this is `?`, lower as conditional return"; "this is `Option::map`,
lower as conditional construct"; etc.

**Why**: the redesigned pipeline cannot pull in `core`'s MIR without
`-Zbuild-std` (which we have deliberately not enabled). The opaque-call
treatment is the design contract; hand-lowering must respect it or the
captured decisions don't transfer.

**Relationship to the facade collapse**: the proof-of-concept stage
collapsed the previously-separate `regseq` facade crate into a
`crate::regseq` submodule of each spec. That eliminated *infrastructure*
cross-crate complexity (three-tier dispatch, Cargo metadata routing,
RegseqAwareCallbacks, cross-crate MIR encoding). The same logic does
not apply to `core` for two reasons. First, `core` is non-collapsible:
`#![no_std]` Rust requires `core` for `Option`, `Result`, the `?`
desugar lang items, and operator traits. We cannot embed `core` as a
submodule the way we did the facade. Second, the kind of complexity
`core` introduces is different: it is *recognition* complexity (know
the lang-item identity of each call and its lowering), not the plumbing
complexity the facade introduced. We address recognition complexity by
treating `core` as a fixed vocabulary of opaque operations, recognized
by lang-item lookup. Items in the recognized vocabulary are lowered to
known structural or C constructs; items outside it are rejected. The
recognized vocabulary will accumulate as hand-lowering surfaces each
`core` call actually used by driver specs.

## 6. Reject defensive fallbacks

Per the global project rule and `docs/discussions/compiler-refactoring/...`
methodology: do not add error-handling, fallbacks, or "graceful degradation"
for situations that cannot occur given the pinned toolchain and the
documented spec surface.

If hand-lowering encounters a MIR construct that should not be possible
(e.g. an `asm!` invocation in a `#![no_std]` driver spec, an unwind
edge that would actually fire), the correct response is to record the
construct as **rejected** by the inclusion principle and stop. Do not
hand-lower the rejected construct; that gives the compiler permission
to handle it later, which it must not.

## 7. Spec is mutable

If hand-lowering surfaces friction in the source spec (an awkward
construct, a missing intrinsic, an unclear convention), revise the spec
rather than work around it. The spec is `docs/discussions/models/regseq-model/spec.md`
and `docs/discussions/monorust/overview.md`. A clean fixture-source
write-up beats a pile of compiler workarounds.

## 8. Don't optimize the C beyond what the compiler could plausibly emit

Resist hand-inlining things rustc would monomorphize but the eventual
compiler will not fold. The golden C must look like compiler output,
not human-curated output, or it stops being a credible target.

If a transformation is desirable but non-obvious, log it as a planned
backend pass with rationale rather than applying it inline.
