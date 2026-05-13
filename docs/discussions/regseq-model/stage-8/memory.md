# Stage 8 -- MIR Statements: Stage Memory

Stage-8-specific briefing for an LLM agent picking up the
statement work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- that's by convention, to keep top-level
context lean.

This file captures stage-8 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-8
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 8 specifies the `Statement` vocabulary that the
bridge fills from rustc's `mir::StatementKind<'tcx>` and
that downstream consumers (stage 10 basic blocks --
`BasicBlock.statements: Vec<Statement>`; stage 11 bodies;
stage 14 backends) read as the **sub-terminator
instruction surface** of MIR.

Stage 8 sits **fourth in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands ->
rvalues -> **statements** -> terminators -> basic blocks
-> bodies). Its content is referenced by:

- Stage 10 `BasicBlock`: `BasicBlock.statements:
  Vec<Statement>` (or `Box<[Statement]>` per stage-10's
  storage decision) -- every basic block carries a
  sequence of stage-8 Statements before the terminator.
- Stage 11 `Body`: bodies own basic blocks transitively;
  per-statement walk visitors (D-8-4 walk-impl deferral)
  surface here.
- Stage 14 backends: each `Statement` variant lowers to a
  C statement or runtime-helper call (e.g., `Assign(p, rv)`
  -> `<lhs> = <rhs>;` for primitive assignments;
  `SetDiscriminant{place, variant_index}` -> tag-write
  via the enum's C representation;
  `Intrinsic::CopyNonOverlapping{src, dst, count}` ->
  `memcpy(dst, src, count * sizeof(T))`;
  `Intrinsic::Assume(op)` -> `__builtin_assume(op)` /
  `__assume(op)` per backend choice).

Stage 8 receives content from earlier stages:

- Stage 1 IDs (`LocalId`, `ConstId`, `InstanceId`, `TyId`,
  `ItemId`) -- transitively through stages 3-7.
- Stage 1 `Span` / `SpanId` material -- referenced by
  D-8-3 (Statement wrapper struct).
- Stage 3 `Ty` (`VariantIdx` lookup; type-equality on
  `Assign`'s LHS/RHS via `place_ty == rvalue_ty` invariant
  D-8-1.2).
- Stage 5 `Place` -- consumed by `Assign`,
  `SetDiscriminant`, and any `Operand::Copy(Place)` /
  `Operand::Move(Place)` embedded in `Intrinsic`'s
  per-variant payloads.
- Stage 6 `Operand` -- consumed by `Intrinsic::Assume`
  and `Intrinsic::CopyNonOverlapping` operand positions.
  Stage 6's bridge-side `RuntimeChecks` ->
  `Operand::Constant` lowering is inherited; stage 8 sees
  the 3-variant `Operand` surface with no `RuntimeChecks`
  arm to handle. Stage 6's outbound expectation to stage
  9 (`Spanned<Operand>` per-arg span preservation) is
  stage-9's concern; stage 8 only fixes the
  Statement-level span surface.
- Stage 7 `Rvalue` -- consumed by `Assign(Place, Rvalue)`.
  All 9 stage-7 closed `Rvalue` variants reach stage 8
  through the `Assign` payload.

Stage 8 introduces no new ID type and no new top-level
interner. Its content is fully expressible in terms of
stage-1 + stages 3, 5, 6, 7 vocabulary.

The output of stage 8 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 8 inherits the two-phase rhythm proven in stages 3,
4, 5, 6, and 7:

**Phase A -- Discussion pass.** In conversation: lay out
the open decisions in a cluster with options + recommendation
+ question(s). The user weighs in. Sub-questions are
answered. Reasoning chains are validated. Decisions emerge.

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

**Reviews are per-cluster, not per-decision and not
big-bang.** Smaller scope sharpens the review (Codex's slot
count is finite); cluster decisions cascade so catching
errors early is cheaper.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 8 uses the prefixed
format `D-8-K` from day one (`D-8-1`, `D-8-2`, ..., with
sub-tags `D-8-1.1`, `D-8-1.2`, ..., per `../README.md`
§5.2). Stages 1 and 3 use legacy un-prefixed forms; stage 8
does not inherit those.

## 2. Stage-8-specific principles

Corollaries of `../README.md` "Methodology rules" applied
to the statement vocabulary:

- **Store rustc's resolved statement form, not the surface
  expression.** The bridge consumes `mir::StatementKind<'tcx>`
  from `optimized_mir` with `mir_opt_level=0` and the
  cleanup-pass whitelist applied. Statements at this stage
  are post-monomorphization, post-region-erasure-on-the-
  type-side, post-drop-elaboration, post-`CleanupPostBorrowck`
  (NOPs `FakeRead`, `AscribeUserType`,
  `BackwardIncompatibleDropHint`, `Coverage(BlockMarker |
  SpanMarker)`, `Assign(_, Rvalue::Ref(_, BorrowKind::Fake,
  _))`; clears `user_type_annotations` and
  `LocalDecl.user_ty`), post-`RemovePlaceMention` (removes
  `PlaceMention`, `Nop`), post-`StateTransform` (removes
  coroutine ops), post-`ElaborateBoxDerefs` /
  `EraseDerefTemps` (rvalue-side cleanup transitively
  affecting `Assign`'s payload). Mirror what rustc hands us
  at this surface, not the pre-cleanup surface.

- **Closed `Statement` vocabulary.** The locked variant set
  is rejected if exceeded. Any rustc `StatementKind<'tcx>`
  variant the spec does not name -- including
  `FakeRead` / `AscribeUserType` /
  `BackwardIncompatibleDropHint` (CleanupPostBorrowck-
  NOPed); `PlaceMention` / `Nop` (RemovePlaceMention);
  `StorageLive` / `StorageDead` (RemoveStorageMarkers --
  see open-question note below); `Retag` (only with `-Z
  mir-emit-retag`); `ConstEvalCounter` (only injected by
  `CtfeLimit` for const-eval bodies); `Coverage` (only
  with `-C instrument-coverage`) -- is a bridge-rejection
  event with a diagnostic. No "if we see X, just skip"
  handling.

- **Cleanup-pass artefacts are reject-on-encounter, not
  variants.** Per the §17 spec rejection-strategy pattern,
  constructs that are discharged by required passes
  (`FakeRead`, `AscribeUserType`,
  `BackwardIncompatibleDropHint` via `CleanupPostBorrowck`;
  `PlaceMention` via `RemovePlaceMention`; `Nop` via either
  `RemovePlaceMention` or `RemoveStorageMarkers`) are
  rejected if encountered in a well-formed `optimized_mir`
  body. The bridge does NOT carry corresponding variants
  in the RegseqModel `Statement` enum. Same discipline as
  stages 5, 6, and 7.

- **Out-of-pipeline constructs are reject-on-encounter,
  not variants.** Same discipline applied at the
  feature-flag / build-mode level: `Retag` (only with
  `-Z mir-emit-retag`), `ConstEvalCounter` (only injected
  for const-eval bodies via `CtfeLimit`), `Coverage` (only
  with `-C instrument-coverage`) cannot reach the
  driver-build optimized_mir we consume. These reject on
  encounter with a diagnostic naming the absent build
  flag, not a "feature might appear" carry.

- **OPEN QUESTION (stage-8-specific): `RemoveStorageMarkers`
  enable-gate vs whitelist.** Per `prior-art/rustc.md` §2.4
  / §3.2, `RemoveStorageMarkers::is_enabled` returns
  `mir_opt_level() > 0 && !emit_lifetime_markers()`
  [`rustc_mir_transform/src/remove_storage_markers.rs:10-12`].
  At our pipeline's `mir_opt_level=0`, this gate evaluates
  false and the pass would not run -- yet the project's
  whitelist [`../README.md` §1] lists `RemoveStorageMarkers`.
  The `is_required = true` flag on the pass is consulted
  only under `Optimizations::Suppressed`
  [`rustc_mir_transform/src/pass_manager.rs:213-239`], not
  under default `Optimizations::Allowed`. Phase-A must
  resolve: (a) does our wrapper actually flip `mir_opt_level`
  for this pass, or run it explicitly outside the standard
  pass schedule? (b) if not, do `StorageLive` / `StorageDead`
  reach the optimized_mir we consume (in which case D-8-1.1
  must accept them as variants, OR reject them with a
  fixture-verifiable diagnostic)? Resolution path: read
  the project's pipeline-driver code (`crates/regseq-driver/`
  or analogous wrapper) for the explicit pass schedule,
  or run the dump script (`scripts/dump-fixture-mir.sh`)
  on a stage-8 fixture and inspect for `StorageLive` /
  `StorageDead` survivors.

- **Drop NLL/borrowck-only carriers.** `UserTypeAnnotationIndex`
  was already dropped at stages 6 (D-6-1.3) and 7 (per
  D-7-1.6 / D-7-1.8 + `CleanupPostBorrowck` clearing).
  At the Statement surface, the same discipline applies to
  `source_info.scope` and `debuginfos`: scope is debuginfo-
  only and contributes nothing to C lowering;
  `StmtDebugInfos` carry Rust-level debuginfo (per stage-1
  spec rejection of debuginfo storage on grounds of C-shape
  preservation). Drop both at the bridge translation site;
  preserve only `source_info.span` -> a single `span:
  SpanId` field on the wrapper. Per D-8-3.

- **`Statement` is body-local; no interning, no
  `StatementId`.** Statement lives inside body content
  (basic-block payloads at stage 10, transitively at stage
  11), not in a top-level interned table. Equality and
  hashing are structural over the variant payload.
  Compositional with stage-5 / stage-6 / stage-7 inline-
  storage choices. Rustc has no statement-interner; rustc_
  public follows the same pattern; mrustc and Charon both
  store statements inline at use sites in `BasicBlock.statements`.
  Per D-8-2.

- **F4 / F4-2 cross-build concerns are out of scope.**
  `Statement` is body-local content; it does not
  participate in cross-build identity (the F4-2 instance
  ordering canonicalises `(InstanceKey)`, not
  `(BodyContents)`). Statement identity is per-build,
  structural, and need not be canonicalised across build
  environments. Same scope reduction as stages 5, 6, 7.

- **Renderer style is decided deliberately.**
  `pretty.rs::render_statement` defers to stage 11
  alongside `render_place` (D-5-K precedent),
  `render_operand` (D-6-K precedent), and `render_rvalue`
  (D-7-K precedent). What forces deferral: every variant's
  payload either is a Place (D-5-K-deferred), embeds an
  Rvalue carrying Operands (D-7-K + D-6-K-deferred), or
  carries Operands directly (D-6-K-deferred). The
  embedded-rendering subtree is the deferral driver. Per
  D-8-K. Outbound expectation files in
  `stage-11/expectations.md` "From stage 8" at lock time.

- **`Option<...>` niche optimizations: nothing new at
  stage 8.** Stage 8 introduces no new ID types. The
  4-byte `Option<X>` invariants stage 1 established for
  existing ID types are not affected.

- **C-semantics for the statement surface.** Each retained
  variant maps to a C statement form:
  - `Assign(p, rv)`: `<lhs> = <rhs>;` for value assignment;
    or compound-literal initialisation when `rv` is an
    `Aggregate`; or runtime-helper call for unsizing /
    transmute casts (per stage-7 D-7-1.6).
  - `SetDiscriminant{place, variant_index}`: tag-write at
    the enum's C representation (the discriminant field
    location depends on the chosen Rust ADT layout per
    stage-3 §13). Tag value comes from the
    `VariantIdx`-indexed discriminant table for the
    target enum.
  - `Intrinsic::Assume(op)`: backend-specific assume
    builtin (`__builtin_assume(op)` in clang;
    `__assume(op)` in MSVC; `if (!op) __builtin_unreachable()`
    fallback in GCC). Driver targets are clang+gcc, so
    `__builtin_assume` or fallback applies.
  - `Intrinsic::CopyNonOverlapping{src, dst, count}`:
    `memcpy(dst, src, count * sizeof(T))` where T is the
    pointee type recovered from src/dst types.

  Dropped variants (FakeRead, AscribeUserType, etc.) have
  no C lowering -- the bridge rejects them on encounter
  (D-8-4 §A); they do not reach stage-14 codegen.

  The C-shaping rule from `../README.md` "C semantics with
  information preservation" applies: store what rustc gave
  us, render in C shape on demand at stage 14.

- **Compositional behavior beats special-case flags.**
  Same as stages 3-7. Examples for stage 8:
  - `Assign`'s type-equality invariant
    (`place_ty(lhs) == rvalue_ty(rhs)`) falls out of
    stage-3 `Ty` interning + stage-5 `Place.head_ty(via
    LocalDecl)` + stage-7 `Rvalue.ty(...)` -- no parallel
    `is_typed_assign` flag.
  - `Intrinsic::Assume`'s `Operand: Ty::Bool` constraint
    falls out of operand-type lookup against stage-3 `Ty`,
    not a parallel `is_bool_intrinsic` flag.
  - `SetDiscriminant`'s target-enum constraint falls out
    of stage-3 §13.x type rejection of coroutine target
    types (stage 3 type-rejected coroutines, so by the time
    stage 8 sees a `SetDiscriminant{place, _}`, `place_ty`
    must be enum, not coroutine) -- no parallel
    `target_kind: Enum | Coroutine` flag.

## 3. Read order for new agents picking up stage 8

In priority:

0. **If you are opening a specific cluster**: read this
   file + `status.md` + `discussions.md` entries for the
   decisions in your cluster (once they exist) + the
   relevant spec sections (top-level `../spec.md` Section 9
   `Const`, Section 10 `Ty`, Section 13 layout +
   discriminant tables, Section 14 `FnSig` and `FnAbi`,
   Section 19 `Place`, Section 20 `Operand`, Section 21
   `Rvalue` -- since stage-8 Statement variants reference
   all of these).

1. This file (`stage-8/memory.md`) -- stage-8 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a copy
   of `../spec.md`; stage-8 contributions accumulate as new
   sections describing `Statement` and bridge translation
   rules for the statement surface).

4. `expectations.md` -- inbound forward refs (none filed
   at stage opening per the convention; lazy file -- not
   created since no inbound refs exist).

5. `discussions.md` -- distilled decision records (created
   when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2 methodology
   rule -- do **not** reuse stage-0 summaries). Landed:
   - `prior-art/rustc.md` (1779 lines) -- rustc primary
     source: 13-variant `StatementKind<'tcx>`,
     `Statement<'tcx>` wrapper struct, `NonDivergingIntrinsic`
     + `CopyNonOverlapping` shapes, cleanup-pass discharge
     mechanics (`CleanupPostBorrowck` NOP rules,
     `RemoveStorageMarkers` + `RemovePlaceMention` retain
     filters, `is_enabled` vs `is_required` semantics in
     the pass manager), validator's three statement-side
     invariant clusters (Assign type-equality;
     SetDiscriminant target-type; Intrinsic per-variant
     operand types), 16-byte size pin on `StatementKind`
     driving rustc's `Box` payload choices, codegen
     consumer pattern. Surfaces the **`RemoveStorageMarkers`
     enable-gate vs whitelist contradiction** (see stage-
     8-specific principles, "OPEN QUESTION").
   - `prior-art/rustc_public.md` (1389 lines) -- rustc_
     public primary source: 12-variant `StatementKind`
     (drops only `BackwardIncompatibleDropHint`); flat
     `Assign(Place, Rvalue)` / `SetDiscriminant{place: Place,
     variant_index}` / `Intrinsic(NonDivergingIntrinsic)`
     payloads (no Box); `Statement { kind, span }` (drops
     `source_info.scope` + `debuginfos`); inline storage
     in `BasicBlock.statements: Vec<Statement>`; partial
     `pretty_statement` with FIX-ME comments; bridge
     translator's per-variant treatment; closest existing
     analog to RegseqModel's owned-IR shape.
   - `prior-art/mrustc.md` (955 lines) -- mrustc primary
     source: 8-variant tagged union `MIR::Statement`
     (Assign, Asm, Asm2, SetDropFlag, SaveDropFlag,
     LoadDropFlag, Drop, ScopeEnd); peer-narrowest
     surface; folds SetDiscriminant into `RValue::EnumVariant`
     (mrustc only constructs whole enums, never tag-writes
     in place); routes ALL intrinsics through
     `Terminator::Call(CallTarget::Intrinsic{...})`;
     statements carry no source-info / span material at
     all (spans are diagnostic-only and discarded by
     builders); two-layer validator (`MIR_Validate` shape +
     `MIR_Validate_Full` move-state) with fatal `MIR_BUG`
     on violation; full pretty-printer via `Statement::operator<<`.
     Master HEAD `7392eca5bd4958cb184fb47cefbd4c7e4f43547b`
     (2026-04-13).
   - `prior-art/charon.md` (1022 lines) -- Charon primary
     source: 9-variant ULLBC `StatementKind` (Assign,
     SetDiscriminant, CopyNonOverlapping, StorageLive,
     StorageDead, Deinit, Assert, Nop, Error); flat
     `Assign(Place, Rvalue)`; SetDiscriminant kept at
     Statement-level; **no generic `Intrinsic(...)`
     variant** -- Assume fans out to `Assert{on_failure:
     UndefinedBehavior}`, CopyNonOverlapping is its own
     dedicated boxed variant; `Statement { span, kind,
     comments_before: Vec<String> }` (the
     `comments_before` field is novel to Charon for Aeneas
     source-fidelity); inline storage with positional
     `(BlockId, usize)` identity; **no Statement-level
     rejection** (all 7 borrowck/lint/scaffolding variants
     return `None` and are absorb-or-drop); `Deinit(Place)`
     declared but never produced from rustc; full
     pretty-printer via `FmtWithCtx`. Local clone at
     `/Users/nyengele/Documents/sekekama/charon-1/charon/`
     pinned to commit `46ab49f627e1a8e72b17c437f05ae4d1c4c456df`.

   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - kani / verus / prusti: statement-level invariants in
     Rust verification pipelines (relevant if D-8-4
     invariants surface verification-style preconditions).
   - LLVM IR `store` / `memcpy` / `assume` semantics
     (relevant for D-8-1.4 Intrinsic codegen direction;
     informs whether driver targets' compiler-builtin
     surface aligns with Rust's intrinsic surface).
   - C99 / C11 §5.1.2.3, §6.5.16 (assignment expression
     semantics), §7.21.2.1 (memcpy) -- target-language
     statement semantics for D-8-K render_statement
     curation at stage 11.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-stage
   signals).

10. `../spec.md` -- merged top-level spec; consult Section
    9 (`Const` -- referenced by Operand::Constant
    transitively via stage 6, transitively via Rvalue at
    stage 7, transitively via Assign at stage 8), Section
    10 (`Ty`), Section 13 (layout + discriminant tables --
    relevant for `SetDiscriminant`'s target-enum
    constraint and the `VariantIdx`-indexed
    discriminant lookup), Section 19 (`Place` -- consumed
    by Assign, SetDiscriminant), Section 20 (`Operand` --
    consumed by Intrinsic), Section 21 (`Rvalue` --
    consumed by Assign).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-8 statement spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-8/spec-snapshot.md (focus: <sections>).
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
- **Not append-only.** Update sections in place when
  stage-8 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
