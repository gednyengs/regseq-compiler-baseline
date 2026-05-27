# Phase 1 — Rust surface + diagnostic logging (plan)

Entry document for the Phase-1 plan-first design. Phase 1 lowers the full
supported **Rust (non-domain) surface** in `RegseqModel` plus the
**diagnostic-logging** domain construct to host-compilable C, and stands
up the automated codegen-verification loop. Scope, exit gate, and the
front-end dependency model are fixed by `../roadmap.md`; this folder holds
the Level-2 detail.

## The three documents (fill in order)

This plan is split into three co-equal documents + this entry:

1. **`01-construct-surface-and-matrix.md`** — re-derives the exact
   `RegseqModel` construct surface the backend consumes and pins each
   construct's C lowering + coverage status (the **codegen-coverage
   matrix**). The backbone the other two build on.
2. **`02-architecture-decisions.md`** — resolves, concretely, the open
   cross-cutting decisions (naming, declaration order, file layout, host
   panic strategy, terminator emission, diag-log call interception).
3. **`03-verification-flow.md`** — pins the end-to-end automated
   verification: the codegen-coverage harness (bridge → C → host `cc` →
   run → assert) and its dual oracle.

**Dependency:** Doc 1 (the surface) feeds Doc 2 (decisions reference exact
constructs) and Doc 3 (the matrix rows are the verification fixtures). So
**re-derive Doc 1 first**, then 2 and 3 can proceed in parallel.

## Method (binding for this phase)

- **Settle in the docs, not in implementation** (architect decision): a
  decision is "done" only when its concrete resolution + rationale is
  written here. Implementation tasks consume settled docs; they do not
  make design calls. Terminator C-emission is *fully* pinned here, not
  left as "approach."
- Flow: these plan docs → Codex → **spec delta into the canonical
  `../../regseq-model/generic-c-backend.md`** (esp. its §7-deferred
  terminator emission + any `unspecced` matrix rows) → Codex →
  implementation tasks (`task.py`).
- The canonical IR→C contract stays in `generic-c-backend.md`; these docs
  are the *working* design that produces deltas to it.

## Phase-1 exit gate

Every construct in scope (the supported Rust surface + diag-logging)
lowers to C, compiles, runs, and **auto-validates on the host**; the
codegen-coverage matrix reads "covered" across the scope; the codegen
verdict is `pass`. (Mirrors the front-end feature-complete gate.)

## Status

**Skeletons only.** The construct-surface re-derivation (Doc 1) is the
next action and is intended for a fresh session with full budget — see
the latest `.handoff/` snapshot + `../decisions-and-state.md`. Docs 2 and 3 are outlined and
embed the already-locked decisions; they are filled after Doc 1.

## Inputs / pointers

- `../roadmap.md` — Phase scope + exit gate + dependency model.
- `../../regseq-model/generic-c-backend.md` — the canonical C-emission
  spec (the input contract; §2 Ty, §3 pointers, §4 places, §5 operands,
  §6 intrinsics, §7 body [terminators DEFERRED], §8 diag-log, §9 examples).
- `../../regseq-model/spec.md` — the IR producer surface (§8–§26).
- `crates/regseq-model/src/` — the owned IR (re-derivation source of truth).
- `.harness/generic-c/` — live ledger/trackers (the codegen-coverage
  *ledger* lands here; the *matrix* is Doc 1, here in docs).
