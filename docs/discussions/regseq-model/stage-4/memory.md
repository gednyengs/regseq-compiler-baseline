# Stage 4 -- Constants and Literals: Stage Memory

Stage-4-specific briefing for an LLM agent picking up the
constants-and-literals work. Read in addition to (not instead of)
the top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s "Active
focus" block. The top-level `../README.md` does not reference it
-- that's by convention, to keep top-level context lean.

This file captures stage-4 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-4 *current
state* (which decisions are locked, what's open, active cluster,
next concrete unit of work) lives in `status.md`.

## 0. Mission

Stage 4 specifies the `Const` enum and `ConstIdentityKey` schema
that the bridge fills from rustc-evaluated const values and that
the IR consumes wherever value-level constants appear: static
initialisers, const items, generic-const arguments, vtable
instance values, string-literal backing for `&str`, aggregate
constants, and reference-typed constants (function pointers,
vtable pointers, typed null pointers, static addresses).

Stage 3 left an opaque `Const` placeholder with a content-
uniqueness invariant (construction invariant 11 in §3.1) and a
Phase B validation slot (§3.3). Stage 4 fills both: the variant
set, the identity-key shape, the two-phase validation discipline,
and the cross-build canonical encoding for `InstanceId`
relocations.

The output of stage 4 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how* decisions
about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 4 inherits the two-phase rhythm proven in stage 3:

**Phase A -- Discussion pass.** In conversation: lay out the open
decisions in a cluster with options + recommendation +
question(s). The user weighs in. Sub-questions are answered.
Reasoning chains are validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked decisions
land in `spec-snapshot.md` with the full audit shape (Decision /
Reasoning / Methodology rules / Prior-art evidence / Worked
example). A Codex review-correction loop runs against the
cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) + numbered
   issues.
3. Address each issue; record corrections in `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not big-bang.**
Smaller scope sharpens the review (Codex's slot count is finite);
cluster decisions cascade so catching errors early is cheaper.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 4 uses the new prefixed format
`D-4-K` from day one (`D-4-1`, `D-4-2`, ..., with sub-tags
`D-4-1.4`, `D-4-1.6.c` per `../README.md` §5.2). Stages 1 and 3
use legacy un-prefixed forms (`F1`-`F4` and `D1`-`D16`); stage 4
does not inherit those.

## 2. Stage-4-specific principles

Corollaries of `../README.md` "Methodology rules" applied to the
const vocabulary:

- **Store the const-eval *result*, not the const expression.**
  Same posture as stage-3 layout. The `Const` enum carries fully-
  evaluated bytes and scalar values from rustc's const-eval, never
  `ConstKind::Unevaluated`, never expression-level AST. If the
  bridge encounters an unevaluated or symbolic const that survived
  monomorphisation, it's a bridge bug -- raise a diagnostic per
  the no-defensive-fallbacks rule.

- **Cross-build canonicalisation uses stable identity.** Where a
  const value references another item (`InstanceId` for function
  pointers, vtable pointers, static addresses), the
  `ConstIdentityKey` encodes the relocation as
  `(regseq_instance_kind_rank, item_def_path_hash,
  canonical_generic_args, fn_shim_disambig_or_None)` -- not as
  the local `InstanceId` integer. Two builds satisfying the
  build-equivalence quad must produce identical canonical keys
  for relocations to the same Instance.

- **Two-phase validation, type-level before value-level.** Stage 3
  validates `ConstInterner` in Phase B (before `TyInterner`)
  because `GenericArg::Const(ConstId)` participates in nominal-
  type identity. That ordering is sound only for the type-level
  subset (scalar primitives, evaluated array lengths) which can
  be canonicalised without any `Ty` or `Instance` reference.
  Value-level consts (statics, vtable instances, aggregates,
  relocations) need `TyId` (typed-null canonicalisation) and
  `InstanceId` (relocation encoding); they validate in a later
  phase. The phase split is the spec contract; whether the
  implementation uses one interner with two-pass validation or
  two distinct interner tables is a downstream choice.

- **Typed scalars: `1u32` and `1u64` are different consts.** The
  identity key includes the scalar type tag, not just the value
  bits. ZSTs collapse to a single canonical representative.

- **Distinguish "uninit" from any specific byte pattern.** Padding
  / uninitialised bytes inside aggregate constants are first-
  class in the canonicalisation: an aggregate with `[0u8, uninit,
  0u8]` does not equal one with `[0u8, 0u8, 0u8]`.

- **`Option<ConstId>` 4-byte niche must hold.** Stage 1's
  `lib.rs` asserts `size_of::<Option<ConstId>>() == 4`. Don't
  break it -- if a stage-4 schema choice would force a wider
  `ConstId`, surface the trade-off explicitly rather than
  silently breaking the invariant.

- **Renderer style is a separate decision.** `pretty.rs::render_const`
  is in scope for stage 4, but the renderer-style decision may
  legitimately defer to a later stage if the renderer's input
  depends on richer content not yet pinned -- mirroring D14's
  deferral from stage 3 to stage 13. Decide deliberately, not by
  default.

- **Compositional behavior beats special-case flags.** Same as
  stage 3. If a const-handling behavior derives from existing
  rules (e.g. `&str` literals fall out of `Ref(Str)` over a byte-
  array `Const`), do not introduce a flag.

## 3. Read order for new agents picking up stage 4

In priority:

0. **If you are opening a specific cluster**: read this file +
   `status.md` + `discussions.md` entries for the decisions in
   your cluster (once they exist) + the relevant spec sections
   (top-level `../spec.md` §9.7 ConstInterner, §3.3 validation
   order, §13.2 vtable family, §15.1 rejections).
1. This file (`stage-4/memory.md`) -- stage-4 invariants.
2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).
3. `spec-snapshot.md` -- the working spec (initially a copy of
   `../spec.md`; stage-4 contributions accumulate under §9.7 and
   §3.3 as the stage progresses).
4. `expectations.md` -- inbound forward refs from stage 1 (4
   items: pin canonical const-eval result form; const types
   covered; fill `render_const` stub; preserve `Option<ConstId>`
   niche).
5. `discussions.md` -- distilled decision records (created when
   the first decision lands).
6. `review-log.md` -- round-by-round Codex audit trail (created
   when the first review round runs).
7. `prior-art/<project>.md` -- per-stage primary-source research
   (fresh pass per `../README.md` §2 methodology rule -- do **not**
   reuse stage-0 summaries). Likely candidates:
   `rustc_middle::ty::Const` / `ConstKind` and miri's allocation +
   relocation model (cluster A); charon's const handling and
   kani's const resolution (cluster B); MLIR `arith` constants and
   LLVM constant expressions (cluster C). Final candidate set
   pinned when the cluster split is decided.
8. `../README.md` -- top-level briefing (invariants only).
9. `../status.md` -- top-level progress tracker (cross-stage
   signals).
10. `../spec.md` -- merged top-level spec; consult §9.7
    (ConstInterner), §3.3 (validation order), §13.2 (vtable
    family -- consumer of stage-4 aggregate consts with
    relocations), §15.1 (rejections -- includes the "Const value
    referencing the type" stage-4-pinned-shape row).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-4 const-vocabulary spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-4/spec-snapshot.md (focus: <sections>).
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

with `run_in_background: true`. Capture output to `/tmp/...`.
Record each round in `review-log.md` with verdict, issue list,
and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for current
  state (locked decisions, active cluster, next concrete unit of
  work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec) or
  `discussions.md` (the decision records).** Briefing only.
- **Not append-only.** Update sections in place when stage-4
  invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
