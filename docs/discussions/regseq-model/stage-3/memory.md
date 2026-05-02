# Stage 3 -- Type System: Stage Memory

Stage-3-specific briefing for an LLM agent picking up the type-system
work. Read in addition to (not instead of) the top-level
`../README.md` (architectural foundation, global methodology rules)
and `../status.md` (top-level progress tracker).

This file is referenced **only** from `../status.md`'s "Active focus"
block. The top-level `../README.md` does not reference it -- that's
by convention, to keep top-level context lean.

This file captures stage-3 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-3 *current
state* (which decisions are locked, what's open, active cluster,
next concrete unit of work) lives in `status.md`.

## 0. Mission

Stage 3 specifies the `Ty` enum that the bridge fills from rustc's
`TyKind` and that codegen consumes to emit C type declarations.
The output of stage 3 is `spec-snapshot.md` (the canonical spec).
This file briefs the agent on *how* decisions about that spec are
reached.

## 1. Workflow (the rhythm we use)

Stage 3 uses a deliberate two-phase rhythm per decision cluster:

**Phase A -- Discussion pass.** In conversation: lay out the open
decisions in a cluster with options + recommendation + question(s).
The user weighs in. Sub-questions are answered. Reasoning chains
are validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked decisions
land in `spec-snapshot.md` with the full audit shape (Decision /
Reasoning / Methodology rules / Prior-art evidence / Worked
example). A Codex review-correction loop runs against the cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) + numbered issues.
3. Address each issue; record corrections in `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not big-bang.**
Smaller scope sharpens the review (Codex's slot count is finite);
cluster decisions cascade so catching errors early is cheaper.

`discussions.md` holds the distilled discussion record;
`review-log.md` holds the round-by-round audit trail.

## 2. Stage-3-specific principles

Corollaries of `../README.md` "Methodology rules" applied to type-
system design:

- **Don't redo work the rustc compiler has already done.** Layout
  computation (offsets, padding, niche encoding, tag selection),
  monomorphization, and region erasure are rustc's responsibility.
  Our IR stores the *result*, not the *input*. Direct application:
  `Ty::Struct/Enum/Union` carry `tcx.layout_of(...)`'s answer; the
  bridge never re-runs layout.
- **Reject what shouldn't appear; never speculate-handle.** The
  bridge folds every type through
  `instantiate_mir_and_normalize_erasing_regions`; only rigid
  post-mono types reach our `Ty`. If a non-rigid or DSL-rejected
  variant appears, raise a diagnostic.
- **Recognition flags belong on the `Item` decl, not on `Ty`.**
  Lang-item identity is a property of *which* type, not its
  *shape*. Storing flags on `Item` keeps `Ty` compact.
- **Compositional behavior beats special-case flags.** E.g.,
  MaybeUninit's drop suppression falls out of Union no-auto-drop
  + the inner ManuallyDrop flag; no `is_maybe_uninit` flag needed.
  Whenever a behavior derives from existing rules, do not add a
  flag.

## 3. Read order for new agents picking up stage 3

In priority:

0. **If you are opening a specific cluster**: read this file +
   `status.md` + `discussions.md` Section 1 entries for the
   decisions in your cluster.
1. This file (`stage-3/memory.md`) -- stage-3 invariants.
2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).
3. `spec-snapshot.md` -- the spec. Section 1 (Ty enum -- D1 locked),
   Section 2 (Layout -- D2 + D11 locked), Section 3 (Type
   identity and interning -- D3 + D4 locked + TyInterner
   walkthrough), Section 4 (worked C lowerings, including 4.4
   niche example and 4.8 packed-struct example), Section 5 (open
   decisions table), Section 6 (references).
4. `discussions.md` -- distilled decision records (Section 1) +
   pedagogical primer (Section 2: 2.1 Layout primer, 2.2 layout
   responsibility split, 2.3 MIR niche abstraction, 2.4 Rust
   `repr(X)` enumeration, 2.5 TyInterner pattern).
5. `review-log.md` -- round-by-round Codex audit trail.
6. `expectations.md` -- inbound forward refs from stage 1.
7. `findings-rustc-draft.md` -- primary-source rustc evidence
   (TyKind, LayoutData, lang-item flags). Cited extensively.
8. `prior-art/<project>.md` (7 files) -- per-project deep type-
   system research. Stage-3-specific (distinct from
   `../stage-0/prior-art/`).
9. `../README.md` -- top-level briefing (invariants only).
10. `../status.md` -- top-level progress tracker (cross-stage signals).

For an agent picking up the next cluster (D5-D10 + D13-D16
detail decisions; recommended sub-cluster ordering and rationale
in `status.md` "Next concrete unit of work"):
load-bearing reads are 1, 2, 3 (Section 5 open-decisions table
for the live status of each), 4 (Section 1 entries for
D5/D6/D7/D8/D9/D10/D13/D15/D16; D14 was deferred to stage 13),
7 (relevant Themes per
sub-cluster), 8 (relevant prior-art per sub-cluster).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the project
root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-3 type-system spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-3/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style>.
Evidence files: <full list relevant to this cluster>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency>.
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate. Mark hypotheses as hypotheses. Distinguish "what spec claims" from "what evidence file says."
</grounding_rules>

<structured_output_contract>
First line: VERDICT: <BLOCK | MINOR | APPROVE>
Then numbered issues: N. [BLOCK | MINOR | NIT] <tag> <line ref>
  <description> <suggested fix>
Hard cap 25; NITs only with budget.
Last line: TOTAL: <N> issues (<B> BLOCK / <M> MINOR / <T> NIT)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

Run via:
```bash
cat <<'EOF' | uv run python /<repo-root>/scripts/python/codex_runner.py --cwd /<repo-root> --effort high --timeout 1500 --message-only > /tmp/codex_d<cluster>_review_r<N>.txt 2>&1
<prompt>
EOF
```

with `run_in_background: true`. Capture output to `/tmp/...`. Record
each round in `review-log.md` with verdict, issue list, and
corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next concrete
  unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec) or
  `discussions.md` (the decision records).** Briefing only.
- **Not append-only.** Update sections in place when stage-3
  invariants change (rare).
- **Not where decisions live.** Decisions live in `spec-snapshot.md`.
