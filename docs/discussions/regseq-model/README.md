# RegseqModel

The home for the RegseqModel refactor against the following configuration for rustc:
- MIR Stage: `optimized_mir`
- MIR Optimization: `mir_opt_level=0`
- Whitelisted checks and passes:
  - RemoveStorageMarkers
  - RemoveZsts
  - SimplifyLocals
  - ReorderBasicBlocks
  - ReorderLocals
  - CheckAlignment
  - CheckNull
  - CheckEnums

The older RegseqModel material at `docs/discussions/models/regseq-model/`
is non-normative reference; this folder is where the new model takes
shape.

This README is **the entry point**. It is self-contained: a fresh
agent can read it and have everything needed to begin work:
- project mission
- binding methodology
- folder layout
- stage workflow
- operational pointers
- reading order

---

## 1. Project mission

### 1.1 What Regseq is

Regseq is an intent-aware semantic device driver DSL in which device driver behavior
is expressed using portable semantic primitives consisting of:
- semantic operations: the actions
- semantic properties: declarative properties capturing correctness constraints and performance intent.

The compiler reasons over these operations and properties to generate OS-specific drivers that preserve
behavior while specializing implementation strategy to each target OS.

Regseq DSL is embedded in Rust where device driver authors write `#![no_std]` Rust specs.
Regseq DSL primitives are captured as follows:
- operations: with  `#[inline(never)]` and custom `#[rseq::intrinsic({INTRINSIC_NAME})]` attributes on some Regseq-provided functions/methods.
- properties: with `#[rseq::{DOMAIN}(...)]` or `#[rseq::{DOMAIN}::{PROPERTY}(...)]` where {DOMAIN} cover common device driver domains such as
  - Interrupt
  - Synchonization
  - MMIO
  - Clock
  - Reset
  - etc.

The the compiler consumes rustc's `optimized_mir` (with `mir_opt_level=0` and a curated
cleanup-pass whitelist) and produces C for target OSes (Linux,
Zephyr, ...). The bridge between MIR and the C backend is the
internal IR called **RegseqModel**.

DSL background: `regseq-background.md`. Rustc-side reference
paths: `rustc-references.md`.

### 1.2 Why this folder exists

The pre-existing RegseqModel was designed against a
pre-monomorphization view of MIR, which left source-level
constructs opaque in the IR. The shift to `optimized_mir` plus
a curated cleanup-pass whitelist exposes the constructs that
actually matter for driver lowering. RegseqModel is being
re-shaped against that evidence rather than carried forward
from the prior design.

The methodology is evidence-based: the spec, methodology rules,
and decision log accumulate as MIR/HIR observations come in,
not by importing the older spec. Prior decisions stand only
when they survive re-derivation against the new evidence.

### 1.3 Two-phase split

A complete RegseqModel is reached through two sequential paths:

1. **Phase 1 -- device-independent (linguistic).** RegseqModel
   specification plus the MIR/HIR -> RegseqModel construction
   pass. Tests are synthetic linguistic snippets ("how does `?`
   lower?"). Lives in this folder.
2. **Phase 2 -- device pressure.** Authoring golden C for real
   driver specs. pl011 is the first device. Lives at
   `../manual-lowering/`. Each device stress-tests RegseqModel
   synthesis at a holistic level.

Phase-2 findings that indicate a phase-1 spec gap are escalated
back here.

---

## 2. Methodology (binding)

These rules govern every stage's work. Override any inferred
convention from code or older docs.

- **`core` is opaque.** The bridge recognizes calls into `core`
  (`Option::map`, `?` desugar, operator traits, `FromResidual`,
  etc.) by lang-item identity or canonical name. We do NOT walk
  `core` MIR. Items outside the recognized vocabulary are
  rejected.

- **No defensive fallbacks.** The toolchain is pinned. Do not
  add fallbacks to guard against rustc/toolchain regressions or
  "what if X is missing." If something cannot happen given the
  pinned toolchain and the documented spec surface, do not
  handle it -- reject if it appears unexpectedly.

- **Re-derive from evidence; do not import baggage.** Older
  designs (e.g., the `*-old` reference crates, prior
  `models/regseq-model/` spec) are reference material to
  consult; the current spec is built from MIR/HIR observations.

- **Codex review for non-trivial decisions.** Pattern
  recognition, layer attribution, semantic equivalence -- these
  warrant adversarial review. Use
  `scripts/python/codex_runner.py`. Runner contract in the
  project root `CLAUDE.md`.

- **Spec is mutable.** If hand-lowering or specification
  surfaces awkward source-spec constructs, revise the spec
  rather than work around them in the compiler.

- **Compiler-theory-first.** Solve pipeline gaps fundamentally
  via established compiler/language theory; prefer proper IR
  variants over narrow workarounds.

- **Source declares its own compiler-relevant attributes.**
  Crate-level declarations that enable tool-namespace
  recognition (`#![feature(register_tool)]`,
  `#![register_tool(rseq)]`) live in the regseq-aware source
  itself; neither `RegseqCallbacks` nor `RegseqAwareCallbacks`
  injects them. Same principle for `#![no_std]` and any other
  source-required declarations -- if removing the declaration
  breaks plain rustc invocation, it must stay in the source.
  Reason: without source-side declarations, plain `cargo check`
  and `rust-analyzer` outside the wrapper fail.

- **C semantics with information preservation.** RegseqModel
  uses C-shaped lowerings rather than mirroring Rust-internal
  compiler shapes. The bridge does the desugaring (drop-glue
  calls become regular Calls, trait objects become vtables,
  closures become structs+methods, generics get monomorphized
  away, `impl` blocks are lifted); consumers see C-shape.
  Preserve information rustc gives us for free, interrogated
  per construct -- ask what it carries, who consumes it, what
  shape best serves them. Where rustc has done the work (layout
  resolution, monomorphization, region erasure), **store the
  result, not the input.**

- **Code does not reference discussion-folder paths.** Code
  comments must NOT cite `docs/discussions/regseq-model/...`
  paths, file names like `stage-N/spec-snapshot.md`, section
  titles, or line numbers. Discussion docs evolve; line numbers
  and titles drift; nothing in the build catches a stale
  citation. Two acceptable patterns: (a) restate the rule
  inline so the comment stands alone; (b) reference a stable
  architectural-decision tag (`Q1`-`Q7`) or per-stage decision
  tag (`D-{stage}-{decision}`, e.g. `D-3-11`) whose name is
  layout-independent. Frozen `*-old` reference crates retain
  their historical citations untouched as archived artifacts;
  the rule is forward-looking.

- **Forward expectations live in `stage-N/expectations.md`.**
  When a stage surfaces a concern a later stage must address,
  append the entry to the relevant later stage's
  `expectations.md` (incoming forward refs). Stage folders are
  created lazily: the first content for a stage -- including an
  inbound expectation written by an earlier stage -- creates
  the folder. Stage implementer's first action when opening
  their stage: read `stage-N/expectations.md` as a checklist;
  tick entries off as addressed. Outbound expectations (stage X
  writing about stage Y) go in `stage-Y/expectations.md`, not
  stage X's own folder.

- **Per-stage prior-art research uses primary sources, not
  stage-0 summaries.** The files in `stage-0/prior-art/` were
  written through the architectural-decisions lens (Q1-Q7) and
  cover topics like type systems, MIR lowering, and codegen
  shape only incidentally. When stage N needs prior-art
  evidence, run a fresh pass into actual project source /
  papers / docs with stage-N-specific concerns in scope, and
  store the per-project findings at
  `stage-N/prior-art/<project>.md`. Reusing stage-0 summaries
  inherits the stage-0 framing and produces shallow findings --
  research framing determines what evidence surfaces.

---

## 3. Architectural decisions (binding)

Q1-Q7 are settled in `architecture.md`. One-line summaries:

- **Q1 Crate architecture.** Two-crate split: bridge inside
  `regseq-driver`; RegseqModel in `regseq-model` with zero
  rustc deps.
- **Q2 `'tcx` boundary.** Eager translation. Bridge produces a
  fully-owned RegseqModel; `'tcx` does not survive past the
  bridge call.
- **Q3 IR layering.** Single-level IR; stages refine the same
  IR, not a layered ladder.
- **Q4 rustc integration point.** `rustc_driver::Callbacks`
  (not `CodegenBackend`).
- **Q5 Tests + serialization.** `insta` snapshots + custom
  textual MIR-inspired pretty-printer (primary) + serde-derived
  JSON (secondary).
- **Q6 Multi-crate strategy.** `regseq` facade is a separate
  crate; three-tier wrapper dispatch (`PassthroughCallbacks` /
  `RegseqCallbacks` / `RegseqAwareCallbacks`); regseq-aware
  crates declared via `[package.metadata.regseq]`.
- **Q7 Intrinsic recognition by attribute name.** The bridge
  identifies domain intrinsics by `#[rseq::intrinsic("NAME")]`
  callee attributes; closed vocabulary; unknown rejected.

These are binding unless explicitly revised. Future
architectural decisions append in `architecture.md` as Q8, Q9,
... See `architecture.md` for full rationale per decision.

---

## 4. Folder layout

```
docs/discussions/regseq-model/
  README.md                -- this file (entry point)
  status.md                -- top-level dashboard (lean; cross-stage signals)
  architecture.md          -- Q1-Q7 settled architectural decisions
  decisions.md             -- registry of all D-{stage}-{decision} tags (lightweight index)
  spec.md                  -- merged top-level spec (assembled by merger at end-of-stage)
  discussions.md           -- merged top-level audit trail (assembled by merger)
  review-log.md            -- merged top-level review trail (assembled by merger)
  regseq-background.md     -- DSL background
  rustc-references.md      -- pinned rustc-src paths

  stage-N/
    spec-snapshot.md       -- stage-N's spec contribution (canonical per-stage spec)
    discussions.md         -- stage-N's decision audit trail
    review-log.md          -- stage-N's review trail
    expectations.md        -- inbound forward refs from earlier stages
    memory.md              -- stage briefing (workflow, principles, read order)
    status.md              -- stage current-state (locked decisions, active focus, next action)
    prior-art/             -- per-stage primary-source research

    sessions/              -- created only if the stage parallelizes work
      session-X/
        spec-snapshot.md   -- session-local working copy of stage-N/spec-snapshot.md
        discussion.md      -- session-local audit
        review-log.md      -- session-local review trail
```

Top-level files (`spec.md`, `discussions.md`, `review-log.md`,
`decisions.md`) are **merged artifacts**: produced by the
merger at end-of-stage, not edited directly during stage work.

Per-stage `spec-snapshot.md`, `discussions.md`, `review-log.md`
are stage-internal during stage work; the merger absorbs them
into top-level when the stage closes.

`expectations.md`, `memory.md`, `status.md`, and `prior-art/`
are stage-internal-only -- the merger does not touch them.

---

## 5. Stage workflow

The lifecycle of a stage's work, from open to close, with the
discipline that has converged in practice (notably stage-3's
10-round Codex review iteration).

### 5.1 Stage lifecycle

**Open.**
1. Create `stage-N/` if it doesn't exist (lazy creation; an
   earlier stage may already have populated `expectations.md`).
2. Copy top-level `spec.md` to `stage-N/spec-snapshot.md` as
   the working starting point.
3. Read `stage-N/expectations.md` as the entry checklist
   (forward refs from earlier stages).
4. Allocate decision tags `D-N-1, D-N-2, ...` from 1, scoped to
   the stage. N is the stage number.
5. Capture the stage's plan in `stage-N/memory.md` (workflow
   briefing) and `stage-N/status.md` (current-state tracker).

**Work.**
1. Edit `stage-N/spec-snapshot.md` to add the stage's
   contribution.
2. Record decisions in `stage-N/discussions.md` with the
   audit-shape (Decision / Reasoning / Methodology rules
   invoked / Prior-art evidence / Worked example).
3. Record adversarial review rounds in `stage-N/review-log.md`.
4. File forward refs to later stages in those stages'
   `expectations.md` (NOT in stage-N's own folder).

**Close.**
1. Run review rounds against `stage-N/spec-snapshot.md` until
   the stage-internal verdict is APPROVE.
2. Hand off to the merger (Section 5.5).

### 5.2 Decision tags: `D-{stage}-{decision}`

Every decision uses the format `D-N-K` where N is the stage
number and K is the decision number within the stage. Examples:

- `D-3-1` -- stage 3, first decision.
- `D-3-1.4` -- a sub-decision of `D-3-1`.
- `D-3-1.6.c` -- a sub-sub-decision (matches the convention
  `discussions.md` already uses for clusters).

Properties:

- **No global coordination needed.** Stage 4 starts at `D-4-1`
  regardless of what other stages are doing.
- **Self-documenting.** `D-3-5` immediately tells the reader
  "stage 3, fifth decision."
- **Identity is permanent.** A decision keeps its tag forever,
  even when later stages refine its content. This matches how
  Q-tags work.
- **Stages own their decisions.** `D-3-11` belongs to stage 3
  permanently; if stage 7 revises it, the revision lives in
  stage 7's `discussions.md` but the tag stays `D-3-11` and
  flips to `[REOPENED]` in `decisions.md`.

Stages 1 and 3 pre-date this convention and use legacy tag
forms (stage 1: `F1`-`F4`; stage 3: `D1`-`D16` without the
stage prefix). Going forward, all new stages use the prefixed
format from day one.

### 5.3 Parallel sessions (optional)

A stage may parallelize work across sessions when its decision
cluster splits cleanly (stage-3 did this with
SESSION-A/B/C across D5+D6, D7+D8+D15, and D9+D10+D13+D16).

**Setup.**
- Allocate D-tags up front so sessions don't race over the
  namespace. Document the cluster split in
  `stage-N/status.md`'s "Active cluster" section.
- Create `stage-N/sessions/session-X/` per session.
- Each session branches `stage-N/spec-snapshot.md` into its
  local working copy.

**Per-session work.**
- Each session runs its own discussion + review loop locally.
- A session closes at APPROVE on the *session-local*
  spec-snapshot.

**Stage-internal merge (after sessions close).**
- Each session's `discussion.md` and `review-log.md` get
  appended (with session sub-headers) to
  `stage-N/discussions.md` and `stage-N/review-log.md`.
- Each session's `spec-snapshot.md` edits are *reconciled* into
  `stage-N/spec-snapshot.md` (the only step that requires real
  merge logic, not concat).
- Stage-internal cross-session Codex review pass on the merged
  `stage-N/spec-snapshot.md`. Iterate to APPROVE.

Solo stages skip the `sessions/` subfolder entirely.

### 5.4 Multi-hop merge

Records flow up two levels:

```
session-X/{spec-snapshot, discussion, review-log}.md
          |
          v  (stage-internal merge at end of parallel sessions)
stage-N/{spec-snapshot, discussions, review-log}.md
          |
          v  (top-level merge at end of stage)
top-level/{spec.md, discussions.md, review-log.md, decisions.md}
```

Each merge tier has its own review pass. The two-tier review
discipline that emerges:

```
session APPROVE  ->  stage-internal cross-session APPROVE  ->  top-level post-merge APPROVE
```

For non-parallelized stages, the middle tier collapses (stage
IS the session) and the flow is: stage APPROVE -> top-level
post-merge APPROVE.

### 5.5 Merger contract (end-of-stage, 7-step protocol)

Run by the merger when stage N is ready to close:

1. **Integrate `stage-N/discussions.md` into top-level
   `discussions.md`** under a `## Stage N` header. Mechanical
   concat -- no summarization. **Append-if-exists rule:** if
   top-level `discussions.md` does not yet exist (only one
   stage has produced one so far), the per-stage version IS
   the project's audit trail; top-level `discussions.md` is
   created on the second contributing stage's close
   (containing both stages' content under their respective
   `## Stage N` headers).
2. **Integrate `stage-N/review-log.md` into top-level
   `review-log.md`** under a `## Stage N` header. Mechanical
   concat. Same append-if-exists rule as step 1.
3. **For each decision finalized in stage N**, append a
   one-line entry to `decisions.md` under the `## Stage N`
   section, with status `[LOCKED]` / `[DEFERRED]` /
   `[REOPENED]` and a one-line summary.
4. **For each decision in earlier stages reopened by stage N**,
   flip its status in `decisions.md` to `[REOPENED]` and append
   "(revised in stage N)" to its summary line.
5. **Integrate `stage-N/spec-snapshot.md` into top-level
   `spec.md`** per the update guidelines in Section 5.9. This
   is the only step requiring real merge logic: `spec.md` is a
   live current snapshot organised by topic (not
   history-preserving concatenation), so integration strips
   audit shape, organises by topic, and updates existing
   sections in place when later stages refine earlier content.
6. **Run a Codex post-merge review pass** on the integrated
   top-level `spec.md`. Iterate to APPROVE.
7. **Update `status.md`** to mark stage N closed and announce
   the next stage's entry point. Add a "Snapshot at close of
   stage N -- current spec lives in top-level `spec.md`"
   header to `stage-N/spec-snapshot.md` so it freezes as a
   historical artifact.

Steps 1-2 are deterministic concat (with the append-if-exists
rule). Step 3 is one line per decision. Step 4 is small. Step
5 is the substantive integration; see Section 5.9. Step 6 is
the load-bearing review.

### 5.6 Two-tier Codex review

Built into the workflow:

- **Tier 1 (per-session, optional).** Each parallel session
  runs Codex review locally to APPROVE before contributing to
  stage-internal merge.
- **Tier 2 (stage-internal cross-session, only if parallel).**
  Codex review of the merged stage-N spec-snapshot. Catches
  inter-session drift.
- **Tier 3 (top-level post-merge).** Codex review of top-level
  spec.md after the merger lands. Catches cross-stage drift.

Each tier uses the same Codex template (in this README's
"Operational pointers" section, or per-stage `memory.md` for
stage-specific framing).

Stage-3's review trajectory (external + R1-R10) demonstrated
the value of multiple tiers: the BLOCK findings at later
rounds caught drift that earlier rounds missed precisely
because earlier rounds had narrower scope.

### 5.7 `decisions.md` shape

Single-writer (the merger) registry. Lightweight, easy to
scan. Stages 1 and 3 use legacy un-prefixed forms (per Section
5.2); stages 4+ use the `D-{stage}-{decision}` prefixed format.
Format:

```markdown
# Decisions

Project-wide registry. Full audit trail in `discussions.md`
(top-level merged) and `stage-N/discussions.md` (per-stage).

Status: [LOCKED] / [DEFERRED] / [REOPENED]

## Stage 1 (foundation, legacy F-tag form)
- F1 [LOCKED] -- ID conventions and Idx trait
- F2 [LOCKED] -- ...
- ...

## Stage 3 (type system, legacy un-prefixed D-tag form)
- D1 [LOCKED] -- Ty variant scope (rigid post-mono)
  - D1.1 [LOCKED] -- Variant set (19 variants)
  ...
- D14 [DEFERRED to stage 13] -- Renderer style for Ty
- ...

## Stage 4+ (going forward, prefixed D-{stage}-{decision} format)
- D-4-1 [LOCKED] -- ...
- ...
```

Useful greps: `[LOCKED]` for what's pinned, `[DEFERRED]` for
deferred decisions, `[REOPENED]` for cross-stage friction,
`D-3-` for stage-3 decisions in the prefixed form, etc.

### 5.8 What the merger does NOT touch

- `stage-N/expectations.md` -- inbound forward refs, stage-
  internal checklist
- `stage-N/memory.md` -- stage briefing
- `stage-N/status.md` -- stage current-state
- `stage-N/prior-art/` -- per-stage research

These are stage-management artifacts, not spec contributions.
They stay per-stage permanently.

### 5.9 `spec.md` content and update guidelines

`spec.md` is the **live, current snapshot** of the RegseqModel
spec. It tells a reader what RegseqModel IS today, organised by
topic. Stage closures update it via the merger contract (Section
5.5 step 5).

Five rules govern how `spec.md` evolves:

1. **Live, not historical.** `spec.md` carries the current state
   only. History -- the audit trail of how decisions were made,
   what alternatives were considered, what reasoning underpinned
   each choice -- lives in `discussions.md`. When a later stage
   refines an earlier stage's content, `spec.md` updates in
   place with the refined content; it never carries "previously
   was X, now is Y" prose.

2. **Topic-based, not stage-based.** Sections in `spec.md` are
   organised by IR topic (the `Ty` enum, Layout, the `Item`
   table, etc.) -- not by which stage contributed them. There
   are no "Stage 3 contributions" wrappers. When stage N's
   content is added, it lands in the relevant topic section,
   merging with anything already there.

3. **Audit shape stripped.** Per-stage `spec-snapshot.md`
   documents each decision in audit form (Decision / Reasoning
   / Methodology rules invoked / Prior-art evidence / Worked
   example). When content is lifted to `spec.md`, the
   Reasoning, Methodology rules, and Prior-art evidence
   sections are dropped -- they live in `discussions.md`. Only
   concrete spec content remains: type definitions, struct
   shapes, identity rules, invariants, schema definitions,
   bridge translation rules. Worked examples may stay (e.g.,
   in an appendix) if they illustrate the live spec.

4. **No forward-looking placeholders.** `spec.md` does NOT
   pre-allocate sections for content future stages will fill.
   There is no "Section N: Const (placeholder for stage 4)"
   entry. Sections appear only when their content is pinned.
   Cross-stage forward-references for content that *is*
   already pinned (e.g., the `Ty::Array(TyId, u64)` variant
   noting that the canonical-form rule for the length is
   pinned in stage 4) are fine because the pinning has
   happened; this is not the same as a stub section.

5. **D-tag inline references avoided.** Spec text should stand
   alone without depending on the audit trail's tagging
   system. Where rationale matters at a load-bearing point,
   use a `discussions.md` pointer ("see `discussions.md` D11
   for rationale") rather than inline `(per D11)` parentheticals.
   The reader of `spec.md` doesn't need to know which D-tag a
   piece of spec content traces to.

**Update workflow at stage close (operational).**

The merger reads `stage-N/spec-snapshot.md`, identifies what's
new or refined relative to top-level `spec.md`, and integrates:

- *New content* (sections covering topics not yet in
  `spec.md`): Add new sections in topic-appropriate locations.
  Strip audit shape during the lift.
- *Refined content* (later stage refines earlier stage's
  content): Edit existing sections in place with the refined
  content. Discard the previous wording. The history of the
  refinement lives in `discussions.md` under stage N's heading
  (which references the earlier stage's tag).
- *Forward-looking content in `stage-N/spec-snapshot.md`*
  (notes about what stages > N will do): Drop. `spec.md` does
  not carry forward-looking content.

After integration, run the post-merge Codex review (Section
5.5 step 6) on the updated `spec.md` to catch cross-stage
drift.

**Per-stage `spec-snapshot.md` after stage close.** Each closed
stage's `spec-snapshot.md` becomes a frozen historical
artifact: it preserves what that stage contributed at its
close, including the full audit-shape decision text. A header
note at the top of each closed stage's `spec-snapshot.md`
says: "Snapshot at close of stage N -- current spec lives in
top-level `spec.md`."

---

## 6. Operational pointers

- `scripts/dump-fixture-mir.sh <fixture>` -- MIR dump for a
  fixture (applies the cleanup-pass whitelist).
- `scripts/python/codex_runner.py` -- non-interactive Codex
  invocation. Contract: project root `CLAUDE.md`. Standard
  invocation pattern:

  ```bash
  cat <<'EOF' | uv run python scripts/python/codex_runner.py \
      --cwd <repo-root> --effort high --timeout 1500 \
      --message-only > /tmp/codex_output.txt 2>&1
  <prompt with task / grounding_rules / structured_output_contract /
   default_follow_through_policy blocks>
  EOF
  ```

  Run in background (`run_in_background: true`) for long-effort
  reviews.

- Pinned toolchain paths: `rustc-references.md`.
- `cargo check --workspace --tests` -- standard smoke check.
  Warnings from `regseq-driver-old/src/extract/` inherited dead
  code are expected; errors are not.
- `cargo test -p regseq-model` -- run the active model crate's
  integration tests in isolation. Workspace-wide `cargo test`
  is noisier due to the `*-old` crates.

### 6.1 Repository code layout

- `crates/` -- the compiler. Active crates: `regseq-model`,
  `regseq-analysis`, `regseq-backends`, `regseq-driver`,
  `regseq` (facade), `regseq-config`, `regseq-diagnostics`,
  `regseq-macros`, `cargo-regseq`. Reference (`*-old`):
  `regseq-model-old`, `regseq-analysis-old`,
  `regseq-backends-old`, `regseq-driver-old`, `tests/e2e-old`.
  Treat the `*-old` set as reference per the "re-derive from
  evidence" rule -- consult deliberately, do not lift wholesale.
- Only `regseq-driver` and `regseq-driver-old` set
  `rustc_private = true`; all other crates are rustc-free by
  Cargo enforcement.
- `tests/fixtures/` -- driver fixtures (e.g.,
  `pl011-full-spec/`).
- `scripts/` -- tooling (MIR dump, Codex runner, ...).

---

## 7. Reading order for a fresh agent

1. **This README** -- self-contained briefing.
2. **`status.md`** -- current cross-stage progress, active
   focus, next action.
3. **`architecture.md`** -- Q1-Q7 settled architectural
   decisions (binding).
4. **`decisions.md`** -- registry of all per-stage decisions,
   one line each.
5. **`regseq-background.md`** -- DSL background (skim).
6. **`rustc-references.md`** -- consult when a rustc-internal
   question surfaces.
7. **If picking up a specific stage N**:
   - First action: read `stage-N/expectations.md` -- treat as
     a checklist of inbound forward refs.
   - Then `stage-N/memory.md` for the stage briefing
     (workflow, principles, read order specific to the stage).
   - Then `stage-N/status.md` for current state.
   - Then `stage-N/spec-snapshot.md` for the canonical spec
     and `stage-N/discussions.md` for decision audit trails on
     demand.
8. **For deep cross-stage spec questions**: top-level
   `spec.md` (merged) and `discussions.md` (merged audit
   trail).
9. Update `status.md` at session end if anything material
   changed.

---

## 8. What this README is NOT

- **Not the canonical spec.** Per-stage canonical specs live
  in `stage-N/spec-snapshot.md`; the merged top-level spec
  lives in `spec.md`.
- **Not the architectural-decisions log.** That's
  `architecture.md` for Q-tags.
- **Not the decision registry.** That's `decisions.md` for
  D-tags.
- **Not the current-state tracker.** That's `status.md` for
  cross-stage signals; `stage-N/status.md` for per-stage.
- **Not append-only.** Update sections in place when
  invariants change. Stale orientation is worse than no
  orientation.
- **Not a record of history.** History lives in commit log +
  `status.md` updates + `stage-N/review-log.md`.
