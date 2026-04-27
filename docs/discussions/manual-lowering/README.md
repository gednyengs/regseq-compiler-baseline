# Manual Lowering -- pl011 (phase 2 / device pressure)

Working directory for the pl011 device-pressure exercise: read
rustc-emitted MIR for the pl011-full-spec fixture, hand-write the
equivalent C, validate end-to-end in QEMU. This is **phase-2** work
in the two-phase RegseqModel construction effort:

1. **Phase 1 -- device-independent (linguistic).** RegseqModel
   specification and the MIR/HIR -> RegseqModel construction pass,
   driven by synthetic linguistic snippets. Lives at
   `docs/discussions/regseq-model/`.
2. **Phase 2 -- device pressure (this folder).** Authoring golden C
   for real driver specifications. Each device stress-tests
   RegseqModel synthesis at a holistic level and surfaces gaps the
   linguistic phase couldn't predict. pl011 is the first device.

Findings here that indicate a phase-1 spec gap are escalated to
`regseq-model/` rather than handled inline.

## Why this exercise exists

- **Codegen bootstrapping.** Hand-written golden C lets us validate
  the backend contract before the compiler can produce it.
- **Device pressure on RegseqModel.** Real-driver MIR exercises
  combinations that synthetic linguistic tests don't anticipate
  (ordering-region patterns interacting with error-cleanup chains,
  ISR shapes, register-access call-site conventions). Each gap
  surfaced is a phase-1 escalation candidate.
- **Spec-language friction.** Where the source spec is awkward to
  translate, that's a signal to revise the spec rather than work
  around it.

## Folder layout

- `README.md` -- this document; entry point and status.
- `methodology.md` -- rules for phase-2 work.
- `workflow.md` -- how to run the dump script and capture decisions.
- `decisions.md` -- per-body decision log for pl011, indexed by spec
  body. Format proposed; not yet finalized.

References elsewhere:

- `docs/discussions/regseq-model/` -- phase-1 home (DSL background,
  rustc-source pointers, MIR-shape findings).
- `docs/discussions/models/regseq-model/` -- the older RegseqModel
  specification, non-normative reference.
- `docs/discussions/monorust/overview.md` -- historical design notes;
  the inclusion-principle taxonomy still applies.
- `docs/discussions/compiler-refactoring/test-audit.md` -- test
  classification under the disable-and-keep methodology.

## Status

**Phase 2 not yet started.** Tooling (dump script, fixture,
cleanup-whitelist) is in place; first body not yet hand-lowered.

Phase 2 effectively waits on phase 1 to stabilize enough of
RegseqModel that authoring golden C against the device fixture is
tractable. Interleaving is expected: phase-2 attempts may surface
phase-1 gaps that get fixed in `regseq-model/`, then phase 2 resumes.

What's in place:

- `scripts/dump-fixture-mir.sh` produces `<fixture>/dump/` with
  `pl011_full_spec.mir`, `spec.hir.txt`, `mono-items.txt`. The
  facade lives inside each spec as a `crate::regseq` submodule, so
  facade bodies are inlined into the spec's MIR rather than emitted
  as a separate artifact.
- The script's flag set applies the full cleanup whitelist
  (`+RemoveStorageMarkers,+RemoveZsts,+SimplifyLocals-final,+ReorderBasicBlocks,+ReorderLocals`)
  for hand-lowering readability. The wrapper's `regseq_mir_passes()`
  in `crates/regseq-driver/src/callbacks.rs` is conservative
  (UB-check force-offs only) until extraction is rewritten to expect
  the cleaned shape.
- `docs/discussions/compiler-refactoring/test-audit.md` classifies
  pre-existing tests as KEEP (HIR/MIR-boundary or pre-rustc) or VOID
  (post-HIR/MIR output). VOID tests are gated behind a per-crate
  `post-monorust-tests` cargo feature.

**Open items before hand-lowering can start in earnest**:

1. Decision-log entry format. Sketched in `decisions.md`; needs
   sign-off before the first entry lands.
2. First body to lower. Candidates: `field_val` (~36 lines),
   `raw_reg_val` (~22 lines), `mask_all_interrupts` (~22 lines,
   register access).

## Retirement criterion

Phase-2 work for pl011 retires when the redesigned RegseqModel
construction (consuming `optimized_mir` directly) produces output
that compiles, boots under QEMU, and exercises the device. The
dormant post-HIR/MIR pipeline retires in a single deletion commit at
that point.

## Cross-session continuity

A new session should:

1. Confirm phase-1 state by reading
   `docs/discussions/regseq-model/README.md` -- phase-2 work generally
   waits on phase-1 progress.
2. Read this `README.md` for orientation.
3. Read `methodology.md` for phase-2 rules.
4. Read `workflow.md` for operational details.
5. Skim `decisions.md` for the per-body log.
6. Pick up where the previous session left off (recorded in "Status"
   above; update at end of session).
