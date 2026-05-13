# Stage 7 -- MIR Rvalues: Stage Memory

Stage-7-specific briefing for an LLM agent picking up the
rvalue work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- that's by convention, to keep top-level
context lean.

This file captures stage-7 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-7
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 7 specifies the `Rvalue` vocabulary that the bridge
fills from rustc's `mir::Rvalue<'tcx>` and that downstream
consumers (stages 8 statements -- transitively through
`Statement::Assign`'s embedded `Rvalue`; stages 9
terminators; codegen) read as the RHS-expression surface of
MIR.

Stage 7 sits **third in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands -> rvalues
-> statements -> terminators -> basic blocks -> bodies). Its
content is referenced by:

- Stage 8 `Statement`: `StatementKind::Assign(Place,
  Rvalue)` -- every assignment carries an `Rvalue` payload.
- Stage 9 `Terminator`: indirect (Rvalues are not embedded
  in terminators directly; terminators consume `Operand`s
  and `Place`s, but the structural anchor is stage-7
  vocabulary).
- Stage 14 backends: each `Rvalue` variant lowers to a C
  RHS expression form (e.g., `Use` -> value-load,
  `BinaryOp` -> C operator or runtime helper, `Aggregate` ->
  compound literal or per-field assignment, `Cast` -> C cast
  or call to runtime conversion helper).

Stage 7 receives content from earlier stages:

- Stage 1 IDs (`LocalId`, `ConstId`, `InstanceId`, `TyId`,
  `ItemId`) -- transitively through stages 3-6.
- Stage 3 `Ty` (`Ty::Ref` shared/unique discriminator,
  region erasure precedent; `Ty::Array` length canonical
  form; `Ty::Closure` / `Ty::FnPtr` content references).
- Stage 4 `Const` (`ConstId` interning; the 11-variant
  `Const` enum) -- consumed transitively when an `Operand`
  carries `Constant(ConstId)`.
- Stage 5 `Place` (`{ local: LocalId, projection:
  Box<[PlaceElem]> }`) -- consumed by `Rvalue::Ref`,
  `Rvalue::RawPtr`, `Rvalue::Discriminant`, and any
  `Operand::Copy(Place)` / `Operand::Move(Place)` embedded
  in an Rvalue payload.
- Stage 6 `Operand` (`Copy(Place)`, `Move(Place)`,
  `Constant(ConstId)`) -- consumed by every retained Rvalue
  variant with operand-shaped payload positions: `Use`,
  `Repeat`, `Cast`, `BinaryOp`, `UnaryOp`, `Aggregate`.
  Stage 6's bridge-side `RuntimeChecks` -> `Operand::Constant`
  lowering is inherited; stage 7 sees the 3-variant
  `Operand` surface with no `RuntimeChecks` arm to handle.
  (`WrapUnsafeBinder` would have been an additional
  consumer, but it is dropped from the closed surface per
  D-7-1.1 and bridge-rejected per D-7-3.)

Stage 7 introduces no new ID type and no new top-level
interner. Its content is fully expressible in terms of
stage-1 + stages 3-6 vocabulary.

The output of stage 7 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 7 inherits the two-phase rhythm proven in stages 3,
4, 5, and 6:

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

**Decision tag convention.** Stage 7 uses the prefixed
format `D-7-K` from day one (`D-7-1`, `D-7-2`, ..., with
sub-tags `D-7-1.1`, `D-7-1.2`, etc., per `../README.md`
§5.2). Stages 1 and 3 use legacy un-prefixed forms; stage 7
does not inherit those.

## 2. Stage-7-specific principles

Corollaries of `../README.md` "Methodology rules" applied
to the rvalue vocabulary:

- **Store rustc's resolved rvalue form, not the surface
  expression.** The bridge consumes `mir::Rvalue<'tcx>`
  from `optimized_mir` with `mir_opt_level=0` and the
  cleanup-pass whitelist applied. Rvalues at this stage are
  fully resolved -- after monomorphization, after region
  erasure on the type side, after drop elaboration, after
  `CleanupPostBorrowck` (NOPs `Fake` borrows; rewrites
  `ArrayToPointer` / `MutToConstPointer` casts to
  `PtrToPtr`), after runtime lowering (`EraseDerefTemps`
  eliminates `CopyForDeref`; `ElaborateBoxDerefs` eliminates
  `ShallowInitBox`). Mirror what rustc hands us at this
  surface, not the pre-cleanup surface.

- **Closed `Rvalue` vocabulary.** The locked variant set is
  rejected if exceeded. Any rustc `Rvalue<'tcx>` variant
  the spec does not name -- including
  `CopyForDeref` / `ShallowInitBox` (cleanup-pass-discharged)
  -- is a bridge-rejection event with a diagnostic. No
  "if we see X, just skip" handling.

- **Cleanup-pass artefacts are reject-on-encounter, not
  variants.** Per the §17 spec rejection-strategy pattern,
  constructs that are discharged by required passes
  (`BorrowKind::Fake`, `CastKind::PointerCoercion(ArrayToPointer | MutToConstPointer)`,
  `Rvalue::CopyForDeref`, `Rvalue::ShallowInitBox`) are
  rejected if encountered in a well-formed `optimized_mir`
  body. The bridge does NOT carry corresponding variants
  in the RegseqModel `Rvalue` enum. This is the same
  discipline as stage 5's "I-Deref-first" structural-
  invariant style, here applied at the variant-cardinality
  level.

  **Distinct from this principle: bridge-collapse artefacts.**
  `RawPtrKind::FakeForPtrMetadata` is NOT cleanup-discharged
  (it can survive into `optimized_mir` and is reachable in
  driver code via constructs like `<*const [T]>::len()`).
  Per D-7-1.5, the bridge collapses it to `Mutability::Not`
  at translation time, matching rustc codegen's own
  `to_mutbl_lossy()` mapping at
  `rustc_middle/src/mir/statement.rs:994-1003` (which
  treats `FakeForPtrMetadata` as a `*const` approximation
  since "we have no type corresponding to a fake borrow").
  Bridge-collapse and reject-on-encounter are distinct
  disciplines and apply to distinct constructs.

- **Drop NLL/borrowck-only carriers.** `UserTypeAnnotationIndex`
  slots (in `AggregateKind::Adt`'s 4th field, in `ConstOperand`'s
  `user_ty`, in `LocalDecl.user_ty`) are cleared by required
  `CleanupPostBorrowck` (`body.user_type_annotations.raw.clear()`,
  `decl.user_ty = None`); always `None` post-cleanup.
  RegseqModel drops the slot entirely. Same principle as
  stage 6's `ConstOperand` wrapper drop (D-6-1.3) and stage
  5's `local_info` clearing.

- **Drop region material at the rvalue surface.** Stage 3
  pinned `Ty::Ref(t, SharedBorrow|UniqueBorrow)` with no
  region slot (top-level §17.x rejection chain); stage 7
  applies the same erasure to `Rvalue::Ref`. rustc's
  `Region<'tcx>` slot in `Rvalue::Ref(Region, BorrowKind,
  Place)` is dropped; codegen confirms by ignoring it
  (`mir/rvalue.rs:506-511` patterns `(_, bk, place)` and
  synthesises with `tcx.lifetimes.re_erased`). Charon and
  mrustc both drop region from rvalue-Ref. Compositional:
  follows from stage-3 region-erasure rule plus stage-7
  payload-shape decision; no new flag needed.

- **Closed sub-enum subsets follow the same evidence
  discipline as the top-level `Rvalue` set.** `BorrowKind`,
  `RawPtrKind`, `CastKind` (and `PointerCoercion`),
  `AggregateKind` -- each is curated to the variants that
  reach `optimized_mir` after required passes, with
  explicit drop rationale per dropped variant. Stage 7
  does not carry "future-proofing" variants for
  pre-cleanup constructs.

- **Compositional behavior beats special-case flags.**
  Same as stages 3, 4, 5, 6. If an rvalue-handling behavior
  derives from existing rules (e.g., Ref-without-region
  falling out of stage-3 type-side erasure plus stage-7's
  drop-the-Region-slot decision), do not add a flag.

- **`Rvalue` is body-local; no interning, no `RvalueId`.**
  `Rvalue` lives inside body content (Statement payloads at
  stage 8, transitively at stage 9), not in a top-level
  interned table. Equality and hashing are structural over
  the variant payload. Rustc's MIR has no rvalue-interner;
  `rustc_public` follows the same pattern; mrustc and
  Charon both store rvalues inline at use sites. Same scope
  reduction as stages 5 and 6.

- **F4-2 / cross-build concerns are out of scope.** Rvalue
  is body-local content; it does not participate in
  cross-build identity (the F4-2 instance ordering
  canonicalises `(InstanceKey)`, not `(BodyContents)`).
  Rvalue identity is per-build, structural, and need not
  be canonicalised across build environments. Same scope
  reduction as stages 5 and 6.

- **Renderer style is decided deliberately.**
  `pretty.rs::render_rvalue` defers to stage 11 alongside
  `render_place` (D-5-K precedent) and `render_operand`
  (D-6-K precedent). Per D-7-1.8 and §21.8 spec text,
  `Aggregate` field/variant names resolve directly from
  the `AggregateKind`'s `TyId` via the §12 type interner
  (no head-local lookup needed -- every variant carries
  the destination TyId). What forces deferral to stage 11
  is the *operand-rendering* subtree (each embedded
  `Operand` routes through `render_operand`, which itself
  defers to stage 11 per D-6-K). Not a stage-7 deliverable.

- **`Option<...>` niche optimizations: nothing new at
  stage 7.** Stage 7 introduces no new ID types. The
  4-byte `Option<X>` invariants stage 1 established for
  existing ID types are not affected.

- **C-semantics for the rvalue surface.** Each retained
  variant (the 9-variant closed surface, per D-7-1.1) maps
  to a C RHS form:
  - `Use(p)` / `Use(c)`: value-load or constant materialisation.
  - `Repeat(elem, n)`: `{[0...n-1]= elem}` compound literal
    or per-element loop (backend choice).
  - `Ref(_, p)` / `RawPtr(_, p)`: `&p` (with const/mut
    qualifier mapping per the reused stage-3 `Provenance` /
    `Mutability` discriminator).
  - `Cast(kind, op, ty)`: C `(ty)op` for primitive casts;
    runtime helper calls for unsizing or transmutes that
    require dynamic data (e.g., dynamic-trait-object vtable
    materialisation).
  - `BinaryOp` / `UnaryOp`: C operators or overflow-
    detecting runtime helpers per `BinOp` variant.
  - `Discriminant(p)`: read the discriminant field from the
    enum's C representation.
  - `Aggregate(kind, ops)`: compound-literal initialisation
    or per-field assignment.

  Dropped variants (`CopyForDeref`, `ShallowInitBox`,
  `ThreadLocalRef`, `WrapUnsafeBinder`) have no C lowering
  -- the bridge rejects them on encounter (D-7-3 §A); they
  do not reach stage-14 codegen.

  The C-shaping rule from `../README.md` "C semantics with
  information preservation" applies: store what rustc gave
  us, render in C shape on demand at stage 14.

## 3. Read order for new agents picking up stage 7

In priority:

0. **If you are opening a specific cluster**: read this
   file + `status.md` + `discussions.md` entries for the
   decisions in your cluster (once they exist) + the
   relevant spec sections (top-level `../spec.md` Section 9
   `Const` enum, Section 19 `Place` and projections,
   Section 20 `Operand` vocabulary, Section 10 `Ty` enum,
   Section 14 `FnSig` and `FnAbi`, since stage-7 Rvalue
   variants reference all of these).

1. This file (`stage-7/memory.md`) -- stage-7 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a copy
   of `../spec.md`; stage-7 contributions accumulate as new
   sections describing `Rvalue`, sub-enum subsets, and
   bridge translation rules for the rvalue surface).

4. `expectations.md` -- inbound forward refs (none filed
   at stage opening per the convention; lazy file -- not
   yet created).

5. `discussions.md` -- distilled decision records (created
   when the first decision lands).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2 methodology
   rule -- do **not** reuse stage-0 summaries). Landed:
   - `prior-art/rustc.md` -- rustc primary source: 13-
     variant `Rvalue<'tcx>`, `BorrowKind` / `RawPtrKind` /
     `CastKind` / `AggregateKind` definitions,
     `CleanupPostBorrowck` Fake / ArrayToPointer /
     MutToConstPointer discharge evidence,
     `EraseDerefTemps` / `ElaborateBoxDerefs` runtime-
     lowering precedent for `CopyForDeref` / `ShallowInitBox`
     elimination, codegen consumer patterns, size-asserts
     (Rvalue 40 bytes), rustc_public drift evidence
     (BinaryOp pair flat vs boxed; Aggregate Vec vs
     IndexVec), mapping to D-7-K candidates.
   - `prior-art/mrustc.md` -- mrustc primary source: 15-
     variant `RValue` analog, `BorrowType` / `CastType`
     handling, peer choices vs rustc_middle (preserves Len?
     Discriminant? overflow-checked? region treatment;
     boxed-vs-flat operand storage), codegen-C consumer
     notes.
   - `prior-art/charon.md` -- Charon primary source: 11-
     variant LLBC `Rvalue`, region drop at translation,
     3-tier `BorrowKind` collapse, `RawPtrKind::FakeForPtrMetadata`
     translated to `Shared`, `Cast` folded into
     `UnOp::Cast`, `PtrMetadata` lowered to
     `ProjectionElem::PtrMetadata`, rejection sites for
     `ThreadLocalRef` / `WrapUnsafeBinder` / Coroutine,
     `simplify_constants` / `ops_to_function_calls` micro-
     pass precedent for post-translation normalisation.
   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - kani / verus / prusti: rvalue-level invariants in Rust
     verification pipelines (relevant if D-7-3 invariants
     surface verification-style preconditions).
   - LLVM IR / Cranelift CLIF: SSA-values-as-rvalues
     paradigm (alternative; relevant only if the spec
     needs to articulate why the typed-rvalue paradigm was
     chosen over SSA at the model boundary).
   - C99 §6.5: target-language operator semantics
     (relevant for D-7-1.8 BinOp curation and overflow
     handling).

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-stage
   signals).

10. `../spec.md` -- merged top-level spec; consult Section 9
    (`Const` -- referenced by Operand::Constant transitively
    via stage 6), Section 10 (`Ty` -- referenced by `Cast`
    / `Aggregate::Adt` / `WrapUnsafeBinder` payloads),
    Section 19 (`Place` -- referenced by `Ref` / `RawPtr` /
    `Discriminant` / Operand::Copy/Move), Section 20
    (`Operand` -- referenced by every operand-shaped Rvalue
    payload), Section 13 (Synthesised Items -- relevant for
    `Aggregate::Adt` referring to a `ConstMemoryStatic` or
    closure-as-struct synthesised item), Section 14 (`FnSig`
    / `FnAbi` -- relevant for `Cast::PointerCoercion(ReifyFnPointer | ClosureFnPointer)`).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-7 rvalue spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-7/spec-snapshot.md (focus: <sections>).
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
  stage-7 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
