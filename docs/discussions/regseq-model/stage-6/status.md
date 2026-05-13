# Stage 6 Status

Status: **STAGE 6 SPEC-SIDE CLOSED 2026-05-04.** All four
D-tags LOCKED at R6 MINOR (1 bookkeeping finding swept
during R6 corrections; substantive §20 content confirmed
lock-ready by Codex at R4, R5, and R6 with explicit
"Lockability: yes" verdict at R6). Total review cost: 6
Codex rounds; 17 distinct findings closed across the
trajectory (4 BLOCK + 13 MINOR + 0 NIT). Spec content in
§20 of `spec-snapshot.md`; audit shape in
`discussions.md`; outbound expectations filed in
`stage-9/expectations.md` (D-6-1.3 per-arg span
preservation) and `stage-11/expectations.md` (D-6-K
render_operand). Methodology-deviation rationale: residual
bookkeeping drift was swept during R6 corrections; per
the stage-5 R10 precedent, any remaining residuals are
absorbed by the top-level merger Codex review per README
§5.5 step 6.

Last updated: 2026-05-04 (stage 6 spec-side closed;
ready for merger contract per README §5.5).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-6
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md`. For the audit trail, see
`discussions.md` (created when the first decision is
committed). For inbound forward refs (none at opening), see
`expectations.md` (lazy; not yet created).

## Mission

Stage 6 specifies the `Operand` vocabulary that the bridge
fills from rustc's `mir::Operand<'tcx>` and that downstream
consumers (stages 7-9 body content, codegen) read as the
RHS-atom surface of MIR. See `memory.md` Section 0 for the
full mission framing and the stage's role as second link in
the 5-11 MIR pipeline chain.

## Inbound expectations (from `expectations.md`)

**None filed at stage opening.** No earlier stage filed a
forward ref tagged "stage 6" / "stage-6" in its
`spec-snapshot.md` or `discussions.md`. Mentions of stage 6
in stage 5's content (`stage-5/spec-snapshot.md:5298`,
`stage-5/memory.md:110`, `stage-5/prior-art/rustc.md:264`)
are stage-5-internal notes about the stage-5 -> stage-6
dependency edge, not forward refs filed against stage 6.
The `stage-6/` folder did not exist before this opening;
lazy creation happened with the stage's opening.

## Cluster split

**Four D-tags, single cluster.** The four decisions are
tightly coupled (variant set, payload shapes, interning,
invariants all sit on the same `Operand` enum) and small in
aggregate. Solo-cluster pattern matching stage 4's pattern:

- **D-6-1: Operand variant set + payload shapes**
  (substantive). 3-variant closed enum + RuntimeChecks
  bridge-side lowering rule + ConstOperand wrapper drop +
  Copy/Move preservation. Sub-decisions:
  - **D-6-1.1**: Variant cardinality (3 closed:
    `Copy(Place)`, `Move(Place)`, `Constant(ConstId)`).
  - **D-6-1.2**: `RuntimeChecks` bridge-side lowering rule
    (resolve via `checks.value(tcx.sess)` against the
    bridge-host's session per Q4 -- one consuming session
    per build -- intern as `Const::Bool`, emit as
    `Operand::Constant(ConstId)`). Charon precedent.
  - **D-6-1.3**: Constant payload shape
    (`Constant(ConstId)`, no wrapper, drop `user_ty` and
    use-site `span`).
  - **D-6-1.4**: Copy/Move preservation (mrustc
    regret-of-collapse + Charon preservation + rustc
    preservation).
- **D-6-2: Operand storage / interning** (small). No
  interning; operand inline at use sites in stages 7-9.
- **D-6-3: Operand-site invariants** (small). No type-
  `Copy` enforcement on `Operand::Copy(p)`;
  drop-elaboration-discharged in `optimized_mir`.
- **D-6-K: `pretty.rs::render_operand` deferral** (smallest).
  Defer to stage 11 alongside `render_place` (D-5-K
  precedent). Outbound expectation to file in
  `stage-11/expectations.md`.

D-6-2, D-6-3, D-6-K are small standalone decisions; D-6-1
carries the substantive variant-set + lowering-rule + payload
commitments.

## Locked decisions

**STAGE 6 CLUSTER LOCKED 2026-05-04 at R6 MINOR (1
bookkeeping finding; substantive §20 content confirmed
lock-ready by Codex at R4, R5, and R6).** Methodology-
deviation rationale: residual status/prompt drift in this
file is bookkeeping only and was swept during the R6
correction pass. Per the stage-5 precedent ("Cluster
closure (2026-05-04): D-5-1 LOCKED at R10 MINOR (2 NITs;
residuals deferred to top-level merger Codex review per
README §5.5 step 6)"), additional residuals discovered at
top-level merger review are absorbed there rather than
spawning further per-stage rounds.

- **D-6-1 [LOCKED]** -- 3-variant closed `Operand`:
  `Copy(Place)`, `Move(Place)`, `Constant(ConstId)`. Bridge-
  side lowering rule for rustc's `RuntimeChecks(checks)`:
  resolve via `checks.value(tcx.sess)` against the
  bridge-host's session (Q4: one consuming session per
  build; Q6's multi-crate MIR consumed via
  `tcx.optimized_mir(def_id)` against the same session),
  intern as `Const::Bool(b)` via stage-4 `ConstInterner`,
  emit as `Operand::Constant(ConstId)`. Drop rustc's
  `Box<ConstOperand>` wrapper (drops `user_ty`
  NLL-only-and-dead; drops operand-level `span` --
  Statement-level uses recover via `Statement.source_info`
  at stage 8, Call/TailCall args carry per-arg span via
  `Spanned<Operand>` at stage 9). Keep Copy/Move
  distinct (peer-validated by mrustc's regretted collapse,
  Charon's preservation, rustc's preservation).

- **D-6-2 [LOCKED]** -- No operand-level interning; operand
  lives inline at use sites in stages 7-9 payloads. Peer-
  validated: rustc has no operand interner, `rustc_public`
  follows the same pattern, mrustc inlines via spaceship
  `ord(...)`, Charon has `HashConsed<T>` but does not apply
  it to `Operand`.

- **D-6-3 [LOCKED]** -- No type-`Copy` enforcement at operand
  construction. Drop-elaboration has run before
  `optimized_mir` is produced; rustc's pre-drop-elaboration
  type-`Copy` invariant is documented as gone post-drop-
  elaboration (`syntax.rs:1312-1313`). Place-ty resolution
  lives in stage 11 + stage 5; stage 6 does not duplicate.

- **D-6-K [LOCKED]** -- `pretty.rs::render_operand` deferred
  to stage 11 alongside `render_place`. Operand rendering
  needs the same field/variant name tables that
  `render_place` reaches via `LocalDecl.ty`. Outbound
  expectation filed in `stage-11/expectations.md`
  "From stage 6".

## Open decisions

(none -- Phase A drafting complete; all four D-tags
proposed)

## Codex review cadence (status)

Single combined cluster review trajectory covering all four
D-tags (D-6-1.1-1.4 + D-6-2 + D-6-3 + D-6-K).

Trajectory:
- R1 (2026-05-04): BLOCK -- 6 issues (2 BLOCK / 4 MINOR
  / 0 NIT). RuntimeChecks Q6 grounding; ConstOperand span
  loss; stub-message drift; §9.7 cross-ref; charon.md
  stale mapping; audit-template omissions.
- R2 (2026-05-04): BLOCK -- 6 issues (2 BLOCK / 4 MINOR
  / 0 NIT). R1 fixes verified for MINOR 3 / 4 / 6;
  partial-fix residuals for BLOCK 1 (F4/F4-2 quad gap),
  BLOCK 2 (rustc.md stale single-crate paragraph),
  MINOR 5 (sess vs tcx.sess drift). New finding: stage-9
  outbound expectation needed for `Spanned<Operand>`.
- R3 (2026-05-04): MINOR -- 1 issue (0 BLOCK / 1 MINOR
  / 0 NIT). All R2 fixes verified. Residual: this
  file's review-cadence section still reading
  "No rounds run yet" (corrected pre-R4; bookkeeping
  drift only).
- R4 (2026-05-04): MINOR -- 2 issues (0 BLOCK / 2 MINOR
  / 0 NIT). R3 cadence-fix verified; R3 partial-fix
  residual: status.md "Locked decisions" intro and
  "Next concrete unit of work" still carried Phase-A
  text. New finding: §20.3 `translate_const` cited as
  "§13 bridge translation" but §13 is Synthesised Items
  (the rustc->Const construction dispatch is §9.7).
  Codex explicitly noted §20 substantive content
  appears lock-ready after these fixes.
- R5 (2026-05-04): MINOR -- 2 issues (0 BLOCK / 2
  MINOR / 0 NIT). R4 §13->§9.7 cross-ref fix verified.
  R4 status.md fix partial (cadence + Locked-decisions
  + Next-work + prompt-shape refreshed; top-level
  Status header / "Last updated" forgot to advance from
  R3 to R4). New finding: stage-9 outbound says
  "(interned) Operand" -- contradicts D-6-2's
  no-interning rule.

Round-by-round audit trail with verdicts, issues, and
corrections lives in `review-log.md`.

Prompt template: see `memory.md` Section 4.

## Stage-6-specific deferred items

(none expected; all four decisions are intra-stage with
the exception of D-6-K which defers `render_operand` to
stage 11 -- a routine deferral pattern from D-5-K)

## Next concrete unit of work

**Stage 6 spec-side work is complete.** All four D-tags
locked at R6: D-6-1 (sub-decisions D-6-1.1-1.4), D-6-2,
D-6-3, D-6-K. The next workstream is the **merger
contract per README §5.5**: top-level integration of
`spec.md`, `discussions.md`, `review-log.md`,
`decisions.md` plus a post-merge Codex review pass that
will sweep any residual cross-references deferred from
R6's bookkeeping-drift closure.

The 7-step merger protocol (per README §5.5):
1. Integrate `stage-6/discussions.md` into top-level
   `discussions.md` under a `## Stage 6` header
   (mechanical concat).
2. Integrate `stage-6/review-log.md` into top-level
   `review-log.md` under a `## Stage 6` header
   (mechanical concat).
3. Append decision entries to top-level `decisions.md`
   under `## Stage 6`: D-6-1 (parent), D-6-1.1 through
   D-6-1.4 (LOCKED), D-6-2 (LOCKED), D-6-3 (LOCKED),
   D-6-K (LOCKED).
4. (No earlier-stage decisions reopened by stage 6.)
5. Integrate `stage-6/spec-snapshot.md` into top-level
   `spec.md` per README §5.9 (topic-based; audit-shape
   stripped; new §20 Operand vocabulary section + §18.2
   per-method ownership row update for `render_operand`).
6. Run a Codex post-merge review pass on the integrated
   top-level `spec.md`. Iterate to APPROVE. **This pass
   sweeps residual cross-references deferred from R6's
   closure** (per the methodology-deviation rationale in
   `review-log.md`).
7. Update top-level `../status.md` to mark stage 6 closed
   and add a "Snapshot at close of stage 6 -- current
   spec lives in top-level `spec.md`" header to
   `stage-6/spec-snapshot.md`.

After the merger contract completes, the next stage in
the 5-11 dependency chain (stage 7: MIR Rvalues) can
open.

Per-cluster top-up research just-in-time per the
principle in `memory.md` §3.7 ("research only when the
cluster's evidence demands it").

Round-specific prompts for R1-R6 are archived inline in
the actual Codex invocations (each round's verbatim prompt
is recoverable from the conversation history) and
referenced in `review-log.md`'s per-round corrections
summaries; the canonical reusable template lives in
`memory.md` §4.

Concrete prompt-shape for the agent running the
post-merge Codex review (step 6):

> Adversarial review of stage-6-integrated top-level spec,
> ROUND 1.
>
> Spec under review:
> `docs/discussions/regseq-model/spec.md` (focus: newly
> integrated §20 Operand vocabulary section + §18.2
> per-method ownership row update for `render_operand`;
> cross-reference consistency across all sections).
>
> Audit-shape ground truth (now top-level merged):
> `docs/discussions/regseq-model/discussions.md` §"Stage 6"
> + `docs/discussions/regseq-model/decisions.md` Stage 6
> entries.
>
> Out of scope: D-6-1 / D-6-2 / D-6-3 / D-6-K decisions
> are LOCKED; only flag if the integration introduces an
> inconsistency.
>
> Special focus: any residual cross-references deferred
> from R6's MINOR closure (per
> `stage-6/review-log.md` "Round 6").

After this trajectory closes at APPROVE (or MINOR/NIT-only
state if a similar diminishing-returns pattern emerges),
stage 6's spec-side work is complete. Then the merger
contract per README §5.5 applies (top-level integration
into `../spec.md`, `../discussions.md`, `../review-log.md`,
`../decisions.md` plus post-merge Codex review).
