# RegseqModel (historical reference)

> **STATUS: HISTORICAL DESIGN REFERENCE**
>
> This folder contains the older RegseqModel specification and its
> associated planning, audit, and migration documents. It is preserved
> as reference material only.
>
> The older spec was designed against a pre-monomorphization view of
> MIR that left source-level constructs opaque in the IR. The
> compiler now consumes rustc's `optimized_mir` (with
> `mir_opt_level=0`) plus a curated cleanup-pass whitelist; the
> constructs that matter for driver lowering are visible at that
> boundary in ways the older spec did not anticipate.
>
> Prior decisions stand only when they survive re-derivation against
> the new evidence. Do not cite documents in this folder as authority.

## Where current work lives

- `docs/discussions/regseq-model/` -- the current RegseqModel
  refactor; device-independent (linguistic) phase. RegseqModel
  specification and the MIR/HIR -> RegseqModel construction pass.
- `docs/discussions/manual-lowering/` -- the pl011 device-pressure
  exercise (phase 2). Authoring golden C for the pl011-full-spec
  fixture and validating end-to-end.
- `docs/discussions/monorust/overview.md` -- the inclusion-principle
  taxonomy still applies to RegseqModel construction.

## Folder contents

- `spec.md` -- older RegseqModel specification (banner at top).
- `compiler-knowledge.md`, `ir-reference.md`, `context-handoff.md`,
  `migration.md`, `execution-plan.md`, `legacy-removal-plan.md`,
  `wave-c-execution-plan.md` -- planning and audit documents
  surrounding the older spec.
- `regseq-backends-audit.md` -- backend audit against the older
  model.
- `shadow-registers.md`, `user-data-initialization.md` -- focused
  design discussions, with their respective Codex review folders.
