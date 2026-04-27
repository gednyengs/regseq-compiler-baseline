# Manual Lowering

Working directory for the hand-lowering exercise: read rustc-emitted MIR
for the PL011 driver spec, hand-write the equivalent C, validate
end-to-end in QEMU, capture every lowering decision in a structured
log. The captured decisions inform the design of the new RegseqModel
construction (which consumes rustc's `optimized_mir` directly with
`mir_opt_level=0`); the validated golden C becomes the target the
eventual compiler must reproduce.

This is bootstrapping in both directions: forward (we get a working
driver before the redesigned pipeline can produce one) and backward
(the redesigned pipeline is informed by the lessons learned here).

## Why this exercise exists

- **MIR is closer to RegseqModel and C than the source spec.** Lowering
  from MIR captures rustc's substitution and desugar work, which
  RegseqModel construction will also consume directly. Decisions
  captured here transfer to the eventual extractor; decisions from a
  source-level exercise would not.
- **Codegen bootstrapping.** Hand-written golden C lets us validate
  the backend contract before the compiler can produce it.
- **The noise IS data.** Every MIR construct dropped, kept, or rejected
  here populates the corresponding category in RegseqModel
  construction's inclusion taxonomy (the kept/gone-by-mono/rejected/
  invisible/dropped-silently classification preserved from the
  historical MonoRust design notes). Hand-lowering 43 bodies produces
  ~43 instances of each rule the future extractor must encode.
- **Spec-language friction surfaces.** Where the source spec is
  awkward to translate, that's a signal to revise the spec rather
  than work around it in the compiler.

## Folder layout

- `README.md` -- this document; entry point and status.
- `methodology.md` -- the rules we follow.
- `workflow.md` -- how to run the dump script, navigate the output,
  and capture decisions.
- `findings.md` -- empirical findings about MIR shape and rustc's
  desugar choices. Append-only.
- `decisions.md` -- decision log entries indexed by spec body.
  Format proposed; not yet finalized.

External references referenced from this folder:

- `docs/discussions/models/regseq-model/spec.md` -- the domain IR that
  RegseqModel construction produces directly from `optimized_mir`.
- `docs/discussions/monorust/overview.md` -- historical design notes.
  MonoRust as a separate IR is no longer pursued (see banner at the
  top of that file). The inclusion principle, Rust-surface taxonomy,
  and inclusion-category classification still apply to RegseqModel
  construction's responsibilities.
- `docs/discussions/compiler-refactoring/test-audit.md` -- the test
  classification under the disable-and-keep methodology.

## Status

**Phase**: pre-hand-lowering setup complete; first body not yet lowered.

**What's in place**:

- `scripts/dump-fixture-mir.sh` produces `<fixture>/dump/` with four
  artifacts (per-crate MIR, facade MIR, spec HIR, mono-items list).
- The script's flag set applies the full cleanup whitelist
  (`+RemoveStorageMarkers,+RemoveZsts,+SimplifyLocals-final,+ReorderBasicBlocks,+ReorderLocals`)
  for hand-lowering readability. The wrapper's `regseq_mir_passes()`
  in `crates/regseq-driver/src/callbacks.rs` is conservative
  (UB-check force-offs only) until extraction is rewritten to expect
  the cleaned shape.
- The pl011-full-spec fixture's dump under cleanup whitelist:
  - `pl011_full_spec.mir` -- 6,322 lines, 43 spec bodies.
  - `regseq.mir` -- 3,735 lines, facade.
  - `spec.hir.txt` -- 1,749 lines, post-expansion HIR.
  - `mono-items.txt` -- 78 entries, monomorphization manifest.
- `docs/discussions/compiler-refactoring/test-audit.md` classifies all
  pre-existing tests as KEEP (HIR/MIR-boundary or pre-rustc) or VOID
  (post-HIR/MIR output). VOID tests are gated behind a per-crate
  `post-monorust-tests` cargo feature and run only when explicitly
  enabled.

**Open items before hand-lowering can start in earnest**:

1. Decision-log entry format. Sketched in `decisions.md`; needs
   sign-off before the first entry lands.
2. First body to lower. Candidates: `field_val` (~36 lines), `raw_reg_val`
   (~22 lines), `mask_all_interrupts` (~22 lines, register access).

**Retirement criterion** (carried over from the disable-and-keep
methodology): the dormant post-HIR/MIR pipeline retires in a single
deletion commit when the redesigned RegseqModel construction
(consuming `optimized_mir` directly) produces semantically-equivalent
output for the pl011-full-spec fixture, validated by manual
inspection plus targeted spot-tests.

## Cross-session continuity

A new session should:

1. Read this `README.md` for orientation and current status.
2. Read `methodology.md` for the rules.
3. Read `workflow.md` for the operational details.
4. Skim `findings.md` for empirical observations made in prior
   sessions.
5. Skim `decisions.md` for the running per-body log.
6. Run `scripts/dump-fixture-mir.sh tests/fixtures/pl011-full-spec`
   to refresh the dump if needed.
7. Pick up where the previous session left off, recorded in the
   "Status" section above (which should be updated when each session
   ends).
