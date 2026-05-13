# Stage 5 -- MIR Places and Projections: Stage Memory

Stage-5-specific briefing for an LLM agent picking up the
places-and-projections work. Read in addition to (not instead of)
the top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s "Active
focus" block. The top-level `../README.md` does not reference it
-- that's by convention, to keep top-level context lean.

This file captures stage-5 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-5 *current
state* (which decisions are locked, what's open, active cluster,
next concrete unit of work) lives in `status.md`.

## 0. Mission

Stage 5 specifies the `Place` and `PlaceElem` vocabulary that
the bridge fills from rustc's `mir::Place<'tcx>` /
`mir::PlaceElem<'tcx>` and that downstream consumers (stages
6-11, codegen) read as the lvalue surface of MIR.

Stage 5 is the **head of the MIR pipeline**. Its content is
referenced by:

- Stage 6 `Operand`: `Operand::Move(Place)` and `Operand::Copy(Place)`.
- Stage 7 `Rvalue`: `Rvalue::Ref`, `Rvalue::AddressOf`,
  `Rvalue::Discriminant`, `Rvalue::Len` all take a `Place`.
- Stage 8 `Statement`: `Statement::Assign(Place, Rvalue)` is the
  canonical body statement form.
- Stage 11 `LocalDecls` / `Body`: the head `LocalId` of every
  Place must resolve to an entry in the body's `local_decls`.

Specifying stage 5 before 6-11 is the dependency-chain order
that the spec-before-implementation discipline mandates: a
`Statement` cannot be specified without `Rvalue`, which cannot
be specified without `Operand`, which cannot be specified
without `Place`.

The output of stage 5 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how* decisions
about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 5 inherits the two-phase rhythm proven in stages 3 and 4:

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

**Decision tag convention.** Stage 5 uses the prefixed format
`D-5-K` from day one (`D-5-1`, `D-5-2`, ..., with sub-tags
`D-5-1.4`, `D-5-1.6.c` per `../README.md` §5.2). Stages 1 and 3
use legacy un-prefixed forms (`F1`-`F4` and `D1`-`D16`); stage 5
does not inherit those.

## 2. Stage-5-specific principles

Corollaries of `../README.md` "Methodology rules" applied to the
place / projection vocabulary:

- **Store rustc's resolved place form, not the surface lvalue.**
  The bridge consumes `mir::Place<'tcx>` from `optimized_mir`
  with `mir_opt_level=0` and the cleanup-pass whitelist applied.
  Projections at this stage are fully resolved -- no unresolved
  field-of-deref shorthand, no inlined coercions. Mirror what
  rustc hands us. If a non-rigid or unresolved projection
  appears, raise a diagnostic per the no-defensive-fallbacks
  rule.

- **Closed `PlaceElem` vocabulary.** The locked variant set is
  rejected if exceeded. Any rustc `PlaceElem<'tcx>` variant the
  spec does not name is a bridge-rejection event with a
  diagnostic. No "if we see X, just skip" handling.

- **Type-coherent projections.** Each `PlaceElem` step narrows
  the type from the previous step's type; a `Field(idx, _)`
  starting from `Ty::Adt(struct)` produces the field's `TyId`,
  a `Deref` starting from `Ty::Ref(_, inner)` produces `inner`,
  etc. Place validation in §3.3 Phase B walks the chain;
  type mismatches are bridge bugs, not user errors.

- **Index uses `LocalId`, not `Operand`.** Rustc's
  `PlaceElem::Index(Local)` takes a `Local` directly (the local
  holds the runtime index value). Preserving this shape keeps
  `Place` self-contained, avoiding a circular dependency on
  stage-6 `Operand`. Constant-known indices use
  `PlaceElem::ConstantIndex { offset, min_length, from_end }`
  per rustc.

- **`Place` is body-local; no interning, no `PlaceId`.** A
  `Place` lives inside body content (operand / rvalue /
  statement payloads), not in a top-level interned table.
  Equality and hashing are structural (head `LocalId` plus the
  projection sequence). Rustc stores the projection list as
  `&'tcx List<PlaceElem<'tcx>>` (arena-interned in `TyCtxt`);
  per Q2 (eager translation; no `'tcx` survives), Regseq does
  not inherit tcx interning -- D-5-1 picks owned storage
  (`Box<[PlaceElem]>` vs `Vec<PlaceElem>`) per
  `prior-art/rustc.md` §5.

- **C-semantics for the lvalue surface.** Rustc's
  `ProjectionElem` enum has 8 variants in source-declaration
  order (`prior-art/rustc.md` §2): `Deref`, `Field`, `Index`,
  `ConstantIndex`, `Subslice`, `Downcast`, `OpaqueCast`,
  `UnwrapUnsafeBinder`. Each reachable variant maps to a C
  lvalue construct: `Deref` -> unary `*`, `Field` -> `.fname`,
  `Index` -> `[i]`, `ConstantIndex` -> `[N]` with a literal
  offset, `Subslice` -> a slice expression, `Downcast` ->
  tagged-union arm projection. `OpaqueCast` is stripped before
  `MirPhase::Runtime(Optimized)` by `PostAnalysisNormalize`
  (`prior-art/rustc.md` §3.7, HYPOTHESIS 1 -- cross-check at
  D-5-1 under the next-trait-solver path). `UnwrapUnsafeBinder`
  is gated behind the unstable `unsafe_binders` feature and
  is not expected in Regseq DSL inputs (`prior-art/rustc.md`
  §3.8, HYPOTHESIS 2 -- reject per closed-vocabulary rule
  pending confirmation). The C-shaping rule from
  `../README.md` "C semantics with information preservation"
  applies: store what rustc gave us, render in C shape on
  demand.

- **Compositional behavior beats special-case flags.** Same as
  stages 3 and 4. If a place-handling behavior derives from
  existing rules (e.g., a `&str` lvalue falls out of `Deref`
  over a `Ty::Ref(_, Ty::Str)` Place), do not add a flag.

- **Renderer style is decided deliberately.** `pretty.rs::
  render_place` may land in stage 5, or defer to a later stage
  if the renderer's input depends on richer content not yet
  pinned (e.g., field names from `Item` come at stage 12;
  variant names come at stage 12 with `Item` content). Mirror
  the D14 -> stage-13 (`render_ty`) and D-4-K -> stage-14
  (`render_const`) precedents: decide by dependency-chain
  analysis, not by default.

- **F4-2 / cross-build concerns are out of scope.** Place is
  body-local content; it does not participate in cross-build
  identity (the F4-2 instance ordering canonicalises
  `(InstanceKey)`, not `(BodyContents)`). Place identity is
  per-build, structural, and need not be canonicalised across
  build environments. This is a deliberate scope reduction
  vs. stage-3 (`Ty` participates in cross-build identity via
  `TyId` interning) and stage-4 (`Const` participates via
  `ConstIdentityKey`). Place's lack of cross-build identity
  follows from its body-local nature.

- **`Option<...>` niche optimizations: nothing new at stage 5
  unless a Place-related `Option<T>` is added.** Stage 5
  introduces no new ID types (Place is structural, no
  `PlaceId`). The 4-byte `Option<X>` invariants stage 1
  established for the existing ID types are not affected.

## 3. Read order for new agents picking up stage 5

In priority:

0. **If you are opening a specific cluster**: read this file +
   `status.md` + `discussions.md` entries for the decisions in
   your cluster (once they exist) + the relevant spec sections
   (top-level `../spec.md` Section 1 `Ty` enum and Section 2
   layout, since projection narrowing reads from those).
1. This file (`stage-5/memory.md`) -- stage-5 invariants.
2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).
3. `spec-snapshot.md` -- the working spec (initially a copy of
   `../spec.md`; stage-5 contributions accumulate as new
   sections describing `Place`, `PlaceElem`, the projection
   chain, and §3.3 Phase B place-validation rules).
4. `expectations.md` -- inbound forward refs (none filed at
   stage opening per the convention; the file documents what
   the next stages depend on stage 5 to deliver).
5. `discussions.md` -- distilled decision records (created when
   the first decision lands).
6. `review-log.md` -- round-by-round Codex audit trail (created
   when the first review round runs).
7. `prior-art/<project>.md` -- per-stage primary-source research
   (fresh pass per `../README.md` §2 methodology rule -- do
   **not** reuse stage-0 summaries). Landed:
   - `prior-art/rustc.md` (1220 lines, 44 KiB) -- the
     mandatory baseline for D-5-1: `Place<'tcx>` shape, the
     8-variant closed `ProjectionElem` enum, per-variant
     payload semantics, type-narrowing helpers (`PlaceTy::
     projection_ty` and friends), storage representation
     (`&'tcx List<PlaceElem<'tcx>>` interned in `TyCtxt`),
     `MirPhase` shape we receive (`Runtime(Optimized)`),
     cleanup-pass interactions for the regseq driver
     whitelist, `OpaqueCast` and `UnwrapUnsafeBinder`
     semantics, stable-MIR analog. §11 lists 8 grounded
     Phase A questions; "Hypotheses requiring confirmation"
     lists 4 hypotheses needing cross-check before final
     spec language.
   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - charon: place handling, `transform_dyn_trait_calls`'s
     receiver-pointer projection patterns, place validation.
   - kani: place coercion handling for verification, projection
     normalisation.
   - MLIR `memref` / `tensor` projection ops (alternative
     SSA-style projection design).
   - LLVM `getelementptr` (flat-index alternative; relevant for
     codegen lowering even though we choose structured
     projections).
   - C99 §6.3.2.1 lvalue grammar (target language; informs the
     `PlaceElem`-to-C mapping).
8. `../README.md` -- top-level briefing (invariants only).
9. `../status.md` -- top-level progress tracker (cross-stage
   signals).
10. `../spec.md` -- merged top-level spec; consult Section 1
    (`Ty` enum -- consumed by projection narrowing), Section 2
    (Layout -- `FieldIdx` and `VariantIdx` indexing), Section 9
    (`Const` -- consumed by `PlaceElem::ConstantIndex` offsets),
    Section 3.3 (Phase B validation -- where stage-5 place
    validation lands).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-5 place/projection spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-5/spec-snapshot.md (focus: <sections>).
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
- **Not append-only.** Update sections in place when stage-5
  invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
