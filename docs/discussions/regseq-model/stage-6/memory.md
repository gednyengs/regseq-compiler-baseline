# Stage 6 -- MIR Operands: Stage Memory

Stage-6-specific briefing for an LLM agent picking up the
operand work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- that's by convention, to keep top-level
context lean.

This file captures stage-6 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-6
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 6 specifies the `Operand` vocabulary that the bridge
fills from rustc's `mir::Operand<'tcx>` and that downstream
consumers (stages 7-9 body content, codegen) read as the
RHS-atom surface of MIR.

Stage 6 sits **second in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands -> rvalues
-> statements -> terminators -> basic blocks -> bodies). Its
content is referenced by:

- Stage 7 `Rvalue`: `Rvalue::Use(Operand)`, `Rvalue::Repeat`,
  `Rvalue::BinaryOp`, `Rvalue::UnaryOp`, `Rvalue::Cast`, etc.
  -- nearly every Rvalue variant carries one or more `Operand`s.
- Stage 8 `Statement`: through Rvalue (inherited) plus direct
  uses in some Statement variants.
- Stage 9 `Terminator`: `Call::func`, `Call::args`,
  `SwitchInt::discr`, `Assert::cond` all carry `Operand`s.

Stage 6 receives content from earlier stages:

- Stage 4 `Const` (closed 11-variant `Const` enum + `ConstId`
  interning via `ConstInterner`): `Operand::Constant(ConstId)`.
- Stage 5 `Place` (`{ local: LocalId, projection:
  Box<[PlaceElem]> }`, body-local, no `PlaceId`):
  `Operand::Copy(Place)` and `Operand::Move(Place)`.
- Stage 1 `LocalId` and `ConstId` ID conventions (transitively
  through stages 4 and 5).

Stage 6 introduces no new ID type and no new top-level
interner. Its content is fully expressible in terms of
stage-1 + stage-4 + stage-5 vocabulary.

The output of stage 6 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 6 inherits the two-phase rhythm proven in stages 3, 4,
and 5:

**Phase A -- Discussion pass.** In conversation: lay out the
open decisions in a cluster with options + recommendation +
question(s). The user weighs in. Sub-questions are answered.
Reasoning chains are validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked
decisions land in `spec-snapshot.md` with the full audit
shape (Decision / Reasoning / Methodology rules / Prior-art
evidence / Worked example). A Codex review-correction loop
runs against the cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) +
   numbered issues.
3. Address each issue; record corrections in
   `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not big-bang.**
Smaller scope sharpens the review (Codex's slot count is
finite); cluster decisions cascade so catching errors early
is cheaper.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 6 uses the prefixed
format `D-6-K` from day one (`D-6-1`, `D-6-2`, ..., with
sub-tags `D-6-1.1`, `D-6-1.2`, etc., per `../README.md`
§5.2). Stages 1 and 3 use legacy un-prefixed forms; stage 6
does not inherit those.

## 2. Stage-6-specific principles

Corollaries of `../README.md` "Methodology rules" applied to
the operand vocabulary:

- **Store rustc's resolved operand form, not the surface
  RHS.** The bridge consumes `mir::Operand<'tcx>` from
  `optimized_mir` with `mir_opt_level=0` and the cleanup-
  pass whitelist applied. Operands at this stage are fully
  resolved -- `Constant` carries a collector-pre-evaluated
  `mir::Const`, not an `Unevaluated` form. Mirror what rustc
  hands us.

- **Closed `Operand` vocabulary.** The 3-variant locked set
  (`Copy(Place)`, `Move(Place)`, `Constant(ConstId)`) is
  rejected if exceeded. Any rustc `Operand<'tcx>` variant
  the spec does not name (notably `RuntimeChecks` -- handled
  via the bridge-side lowering rule below) is a bridge-rejection
  event with a diagnostic. No "if we see X, just skip"
  handling.

- **`RuntimeChecks` is lowered at bridge time, not preserved
  as a variant.** Per Q4 (`rustc_driver::Callbacks`
  integration), the bridge runs inside a single rustc
  invocation per build with one `Session` in scope; all
  `RuntimeChecks` operands -- whether sourced from the
  primary spec crate's MIR or from regseq-aware dependency
  MIR consumed via `tcx.optimized_mir(def_id)` (Q6) --
  resolve against the bridge-host's session, mirroring
  rustc's standard codegen behavior (consumer's session
  determines the value). The bridge intercepts
  `mir::Operand::RuntimeChecks(checks)`, computes the bool
  via `checks.value(tcx.sess)`, interns `Const::Bool(b)` in
  the `ConstInterner`, and emits `Operand::Constant(ConstId)`.
  This accommodates legitimate source-level
  `cfg!(ub_checks)` etc. without expanding the closed
  vocabulary -- gentler than reject-on-encounter while
  preserving the variant-set discipline.

- **Drop rustc's `ConstOperand` wrapper.** rustc's
  `Operand::Constant(Box<ConstOperand>)` carries `span`,
  `user_ty`, and `const_`. RegseqModel's
  `Operand::Constant(ConstId)` drops the wrapper:
  - `user_ty` is NLL/borrowck-only and dead in
    `optimized_mir` (NLL has run before the cleanup-pass
    whitelist applies).
  - Use-site `span` is recoverable from the containing
    Statement's `SourceInfo` at stage 8. Operand-level span
    duplication is removed.
  - `const_` becomes `ConstId` via stage-4 interning.

- **Keep Copy/Move distinct.** rustc's `Copy(Place)` and
  `Move(Place)` are operationally collapsed at codegen time
  in `codegen_operand` (the only difference is debug-info
  annotation tied to the `compiler_move` / `compiler_copy`
  lang items). However: (1) Move's distinguishing Call-site
  in-place semantics may matter for backend-specific lowering;
  (2) preserving rustc-given information is free; (3) mrustc
  carries an explicit `TODO: Split "Use" into "Copy" and "Move"`
  at `src/mir/mir.hpp:645` -- the closest existing peer
  collapsed the distinction and now considers it a deficiency;
  (4) Charon preserves the distinction in its verification
  pipeline. Stage 6 keeps both variants.

- **`Operand` is body-local; no interning, no `OperandId`.**
  `Operand` lives inside body content (Rvalue / Statement /
  Terminator payloads at stages 7-9), not in a top-level
  interned table. Equality and hashing are structural over
  the variant payload. Rustc's MIR has no operand-interner;
  `rustc_public` follows the same pattern; mrustc and Charon
  both store operands inline at use sites. No downstream
  consumer of operand-content-uniqueness exists (contrast
  `Const`, which has multiple downstream identity consumers).

- **C-semantics for the operand surface.** Each variant maps
  to a C RHS construct:
  - `Copy(p)` / `Move(p)`: C value-load from the place's C
    expression. The C backend treats both identically at
    the load level; backend-specific specialisation (e.g.,
    `memcpy` vs inline copy for large moves) may consult the
    variant tag.
  - `Constant(id)`: C literal or compound literal materialised
    from the resolved `Const` in the model's `ConstInterner`.
  The C-shaping rule from `../README.md` "C semantics with
  information preservation" applies: store what rustc gave
  us, render in C shape on demand.

- **Compositional behavior beats special-case flags.** Same
  as stages 3, 4, 5. If an operand-handling behavior derives
  from existing rules (e.g., a thin-pointer constant operand
  falls out of `Operand::Constant(ConstId)` referencing a
  `Const::Reloc` value), do not add a flag.

- **Operand-site invariants are post-drop-elaboration.**
  rustc's `Operand::Copy(p)` documents the type-`Copy`
  invariant as required pre-drop-elaboration; explicitly gone
  post-drop-elaboration. RegseqModel reads `optimized_mir`
  (post-drop-elaboration), so this invariant does not apply.
  No type-`Copy` enforcement at operand construction. Place-ty
  resolution lives in stage 11 (`LocalDecls`) + stage 5
  (PlaceTy machinery); stage 6 does not duplicate it.

- **Renderer style is decided deliberately.**
  `pretty.rs::render_operand` defers to stage 11 alongside
  `render_place` (D-5-K precedent). Operand rendering needs
  the same field/variant name tables that `render_place`
  reaches via `LocalDecl.ty`. Not a stage-6 deliverable.

- **F4-2 / cross-build concerns are out of scope.** Operand
  is body-local content; it does not participate in
  cross-build identity (the F4-2 instance ordering
  canonicalises `(InstanceKey)`, not `(BodyContents)`).
  Operand identity is per-build, structural, and need not
  be canonicalised across build environments. Same scope
  reduction as stage 5.

- **`Option<...>` niche optimizations: nothing new at stage 6.**
  Stage 6 introduces no new ID types. The 4-byte
  `Option<X>` invariants stage 1 established for existing
  ID types (including `ConstId` from stage 4) are not
  affected.

## 3. Read order for new agents picking up stage 6

In priority:

0. **If you are opening a specific cluster**: read this file +
   `status.md` + `discussions.md` entries for the decisions in
   your cluster (once they exist) + the relevant spec sections
   (top-level `../spec.md` Section 9 `Const` enum, Section 19
   `Place` and projections, since `Operand` references both).

1. This file (`stage-6/memory.md`) -- stage-6 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a copy
   of `../spec.md`; stage-6 contributions accumulate as new
   sections describing `Operand` and the bridge-side
   `RuntimeChecks` lowering rule).

4. `expectations.md` -- inbound forward refs (none filed at
   stage opening per the convention; the file documents what
   the next stages depend on stage 6 to deliver, if/when
   stage 6 files outbound).

5. `discussions.md` -- distilled decision records (created
   when the first decision lands).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2 methodology
   rule -- do **not** reuse stage-0 summaries). Landed:
   - `prior-art/rustc.md` -- rustc primary source: 4-variant
     `Operand<'tcx>`, `ConstOperand` wrapper, `RuntimeChecks`
     enum, `codegen_operand` consumer match, cross-stage
     anchors, mapping to D-6-1 through D-6-K.
   - `prior-art/mrustc.md` -- mrustc primary source:
     collapsed `Use(LValue)` with explicit
     `TODO: Split "Use" into "Copy" and "Move"` at
     `src/mir/mir.hpp:645` (peer regret of the collapse);
     no `RuntimeChecks` analog; constant inlined by value via
     `unique_ptr<HIR::Path>`; no operand interning;
     type-driven validity-state tracking via
     `MIR_Validate_ValState`.
   - `prior-art/charon.md` -- Charon primary source:
     3-variant LLBC `Operand` (no `RuntimeChecks` at operand
     level); preserves Copy/Move; lowers rustc's
     `RuntimeChecks` to `Rvalue::NullaryOp` + `Move(local)`
     (peer precedent for option-(c) bridge-side lowering);
     `HashConsed<T>` infrastructure exists but is not applied
     to `Operand`; `simplify_constants` micro-pass enforces
     post-pass operand-constant invariant.
   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - kani / verus / prusti: operand-level invariants in Rust
     verification pipelines.
   - LLVM IR / Cranelift CLIF: SSA-values-as-operands
     paradigm (alternative; relevant only if the spec needs
     to articulate why the place-or-constant paradigm was
     chosen over SSA).
   - C99 §6.5.2.2 (function call argument passing): target
     language semantics for the Move-to-Call passing path.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-stage
   signals).

10. `../spec.md` -- merged top-level spec; consult Section 9
    (`Const` -- consumed by `Operand::Constant`), Section 19
    (`Place` -- consumed by `Operand::Copy` / `Operand::Move`),
    Section 13.2 / 13.3 (Static-family content -- relevant
    when the bridge sees a `Const::Reloc` operand referring
    to a `ConstMemoryStatic`).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-6 operand spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-6/spec-snapshot.md (focus: <sections>).
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
Record each round in `review-log.md` with verdict, issue
list, and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next
  concrete unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec) or
  `discussions.md` (the decision records).** Briefing only.
- **Not append-only.** Update sections in place when stage-6
  invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
