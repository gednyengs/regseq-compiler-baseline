# regseq-driver-68: Post-F4 batch triage (propose decomposition per strategy §10 Mode B)

## Description

Post-F4 triage of the bridge-coverage-runner's accumulated
discovery output. Walk the F2/F3/F4 expectations files plus a
fresh `report.md` from the committed sidecar, cluster the
findings by root cause, and produce a triage document at
`.harness/audits/2026-05-21-post-f4-triage.md` that **proposes** a set of
follow-up `regseq-driver-NN` tasks for human approval.

This task does NOT file any tasks itself. Per strategy §10
Mode B at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md:465-472`,
agents "propose decomposition", not "file from report"; the
harness rule at `.harness/task-spec-workflow.md:118-123` requires
explicit human approval before `task.py add-task`. The user
reads the triage document, approves a subset of proposals,
and a follow-up session files each approved proposal as a
distinct `regseq-driver-NN` task.

### Why now

After F4 close (regseq-driver-67 at `c8c16f3`), the runner's
sidecar covers 123 rows -- 10 F1 + 22 F2 + 91 F4 = 91% of the
~135 authorable matrix rows. The runner's deterministic
report.md surfaces zero failures (`Total = Pass = 123`,
`Fail = 0`); the bug-discovery output lives in the per-batch
expectations files as ESCALATIONS:

- F2 (regseq-driver-65) escalated **IX-014** (closure FnOnce
  consumption; OPEN BUG family) and **IX-017** (diag-log
  intrinsic; resolved in F4 via `BodyAbsent` predicate).
- F3 (regseq-driver-66) added no escalations; F3 unblocked
  IX-017 via the `BodyAbsent` dispatch arm.
- F4 (regseq-driver-67) escalated **11 rows**: OP-003-I,
  RV-007-C, RV-009-G, TM-006-C, FF-004, FF-005, FF-012,
  FF-013, RJ-006, RJ-024, RJ-032-F.

Net active triage queue: **12 escalations** (11 F4 + IX-014).
These cluster naturally into ~5 root-cause categories the
F4 expectations file outlines but does not formally propose as
tasks. F68's job is to convert each cluster into a concrete
`regseq-driver-NN` task PROPOSAL.

### Out of scope

- Filing the proposed tasks. F68 produces proposals only.
- Modifying the runner, the bridge, or any code. F68 is an
  analytical task.
- Modifying spec.md, the matrix doc, or the strategy doc.
- Proposing tasks for rows that are documented matrix blocks
  (blocked-rustc-infeasible, blocked-bridge-rejects-by-design
  without positive-coverage shape, Section 7 validator
  invariants, etc.). F4 documented these as out-of-scope; F68
  does not relitigate them.
- Authoring the proposed tasks' full spec markdown. F68
  produces a 1-2 paragraph SCOPE SKETCH per proposal -- enough
  for human approval. Each approved proposal becomes its own
  task-spec-drafting session.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  §10 Mode B (`:464-473`) for the "propose decomposition" rule.
- `.harness/task-spec-workflow.md:118-123` for the
  human-approval-before-`add-task` constraint.

### Expectations files (the triage raw material)

- `.harness/expectations/regseq-driver-65.md:223-281` -- F2
  IX-014 + IX-017 escalations.
- `.harness/expectations/regseq-driver-66.md:287-312` -- F3
  IX-017 BodyAbsent probe outcome (resolved IX-017).
- `.harness/expectations/regseq-driver-67.md:207-477` -- F4
  escalation log (11 rows) + per-row root-cause analysis +
  recommended follow-up directions.

### Runner state at HEAD (read-only)

- `crates/bridge-coverage-runner/matrix_metadata/rows.toml` --
  123 committed sidecar rows. F68 may run the runner once to
  obtain a fresh `report.md` snapshot for reference; no
  modifications to sidecar or snippets.
- `crates/bridge-coverage-runner/src/inspect.rs` -- full F3
  dispatch surface. Cross-reference for predicate vocabulary
  in proposal sketches.

### Matrix doc

- `docs/discussions/regseq-model/bridge-coverage-matrix.md` --
  authority for row-ID classifications. F68 cross-references
  row IDs in proposal sketches; F68 does NOT modify the matrix.

### Prior tasks

- regseq-driver-67 (closed at `c8c16f3`): F4 corpus close.
- regseq-driver-66 (closed at `e2e77c0`): F3 dispatch hardening.
- regseq-driver-65 (closed at `3a01888`): F2 first batch.

## Implementation Notes

### Change A: produce the triage document

**New file**: `.harness/audits/2026-05-21-post-f4-triage.md`.

The document is the deliverable. Structure:

1. **Summary** -- aggregate counts:
   - Total escalations analyzed (12).
   - Proposed follow-up tasks (target: 5-8 depending on
     cluster granularity).
   - Priority tiers (HIGH / MEDIUM / LOW) + recommended
     ordering.
   - Out-of-scope items (documented matrix blocks; not
     proposals).

2. **Per-cluster proposals** -- one section per cluster:
   - Cluster name (e.g., "Bridge type normalization gaps").
   - Affected rows (with matrix row IDs).
   - Root-cause analysis (1-2 paragraphs citing
     bridge file:line anchors).
   - Empirical evidence (point to the expectations file
     section that documents the empirical probe).
   - Proposed task: title + scope sketch (1-2 paragraphs of
     "what the implementer would do" -- enough for human
     approval).
   - Priority (HIGH / MEDIUM / LOW) with rationale.
   - Dependencies (which other proposals must land first).
   - Estimated effort (rough -- "small bridge change",
     "moderate bridge change", "large bridge change",
     "infrastructure change", "documentation-only").

3. **Closing recommendations** -- recommended priority order
   for the user to approve. Reflects dependency chains and
   highest-leverage unblocks first.

### Cluster expectations (the implementer's starting point)

The F4 expectations file at
`.harness/expectations/regseq-driver-67.md:207-471` already
performs preliminary clustering in the "Escalation log" and
"Post-F4 readiness signal" sections. F68's job is to formalize
the clusters into PROPOSALS with explicit task-shape sketches.

Suggested cluster taxonomy (the implementer may refine):

| Cluster | Affected escalations | Root-cause family |
| ------- | -------------------- | ------------------ |
| (a) Bridge type-projection normalization | RV-007-C, RV-009-G, FF-005 | `Pointee::Metadata` and `<fn() as FnOnce>::Output` projections don't normalize through to concrete types |
| (b) Shim materialization gaps | FF-004 (ReifyShim), FF-012 (CloneShim), FF-013 (FnPtrAddrShim) | distinct shim families with distinct root causes; may be 1, 2, or 3 separate tasks |
| (c) Const-eval surface gaps | OP-003-I (`Const::EnumValue`), FF-013 (overlap on fn-pointer-provenance const) | `mir::Const::eval` over unevaluated const blocks / fn-pointer provenance |
| (d) Helper / build-contract gaps | TM-006-C (`AssertKind::OverflowNeg` -- helper `overflow_checks=false`), RJ-006 (`DslRejectionKind::Statement` -- cleanup-pass strips), RJ-032-F (`DiagLogValidation::D8` -- target-binding wiring) | infrastructure / helper / wiring concerns, not bridge bugs per se |
| (e) Bridge-internal invariant rejections with no source path | RJ-024 (`UnsupportedEnumLayout`) | rejection variant whose triggering shapes are bridge-internal-only |
| (f) OPEN BUG family | IX-014 (closure FnOnce direct consumption + IX-004/IX-010 root cause) | distinct from F4 escalations; the original blocked-open-bridge-bug family |

The implementer may collapse, split, or rebalance these
clusters. The F68 acceptance criteria require that EVERY
escalation appears in some cluster + at least one proposal;
clustering granularity is the implementer's call.

### Format conventions for proposals

Each proposed task should follow this skeleton:

```markdown
### Proposal: regseq-driver-NN -- <Short Title>

**Affected escalations**: <list of row IDs and the originating
F-batch>

**Root cause**: <1-2 sentences>. See
<expectations file:line> for empirical evidence.

**Proposed scope**:
<1-2 paragraph sketch of what the task would do. Examples of
the level of detail:
- "Extend `translate_ty` at `crates/regseq-driver/src/bridge/ty.rs:XXX`
  to normalize `<T as Pointee>::Metadata` projections through to
  concrete types: `usize` for slice, `DynMetadata<dyn Tr>` for
  dyn, `()` for sized. Test: bring RV-007-C and RV-009-G
  snippets to PASS via runner re-attempt."
- "Probe rustc-side alternative shapes that produce
  InstanceKind::ReifyShim (e.g., reifying #[rustc_intrinsic]
  functions). If none surface in the pinned nightly, document
  FF-004 as deferred until the bridge gains a synthetic
  ReifyShim producer."
The sketch should be detailed enough that the human reviewer
can decide whether to approve / decline / request refinement;
NOT a complete task spec.>

**Priority**: HIGH | MEDIUM | LOW
**Rationale**: <why this priority>

**Dependencies**: <list of other proposals that must land
first, or "none">

**Estimated effort**: small / moderate / large / infrastructure
/ documentation-only
```

### Change B: NO code modifications

F68 is purely analytical. The expectations file at
`.harness/expectations/regseq-driver-68.md` is created as a
formality recording that F68 ran; the deliverable is
`.harness/audits/2026-05-21-post-f4-triage.md` per Change A.

The expectations file should be minimal: a one-paragraph
summary pointing at the audit document as the load-bearing
deliverable.

## Non-Goals

- Do NOT call `task.py add-task` for any proposed task. Filing
  is a separate step after human approval.
- Do NOT modify the runner, the bridge, or any source code.
- Do NOT modify the matrix, the spec, the strategy doc, or the
  pretty-printer / generic-c-backend docs.
- Do NOT modify the F4 sidecar or snippets. F4 is closed.
- Do NOT re-attempt any F4 escalation as a sidecar row. The
  escalations remain matrix-uncovered until the proposed
  follow-up tasks land.
- Do NOT propose tasks for the F4 documented matrix blocks
  (blocked-rustc-infeasible / blocked-bridge-rejects-by-design /
  Section 7 validator invariants / Section 2.2 closure-call
  family / Section 3 closure features / Section 4
  cross-edition / Section 6 hard-to-author / aliased-by-IX /
  no-F3-predicate / negative-assertion / bridge-producer-missing).
  Those are intentionally out of scope per F4's task spec.
- Do NOT propose NEW investigation tasks for the OPEN BUG
  family (IX-004 / IX-010 are pre-existing investigation
  items per the matrix). The single F68 proposal that touches
  the OPEN BUG family is a HIGH-LEVEL POINTER proposal for
  IX-014 (the F2 escalation): the proposal documents that
  IX-014 unblocks once IX-004/IX-010 are resolved upstream
  and recommends watching the OPEN BUG family rather than
  filing a parallel investigation. This satisfies the
  "every active escalation appears in a proposal" rule (B2 +
  Acceptance Criterion 2) without re-proposing the underlying
  IX-004/IX-010 work.
- Do NOT modify `tests/fixtures/playground/`.

## Test Plan

### B1: workspace stays green

`cargo test --workspace -- --test-threads=1` green. F68
makes no code changes; this is a sanity check.

### B2: every escalation appears in the triage document

Verification: each of the 12 active escalations (11 F4 + IX-014
F2) appears in at least one cluster + at least one proposal in
`.harness/audits/2026-05-21-post-f4-triage.md`. Cross-reference the F4
expectations file's escalation log (`:207-417`) and the F2
expectations file's escalation log (`:223-247`).

### B3: every proposal carries the required fields

For each proposal in the triage document, verify the skeleton
fields are populated: affected escalations, root cause,
empirical evidence (expectations-file or matrix-doc `file:line`
anchor), proposed scope, priority, dependencies, estimated
effort. Missing fields are blockers.

### B4: priority + dependency consistency

Verification: if proposal X depends on proposal Y, then Y's
priority is at least as high as X's, OR Y is annotated as
"prerequisite". No circular dependencies.

### B5: out-of-scope discipline

Verification: the triage document does NOT propose tasks for
any of the F4-documented matrix blocks (Non-Goals enumeration).
A spot-check on the F4 task spec's "Out-of-scope rows" section
confirms.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/68-post-f4-batch-triage.md`).
- The F4 expectations file
  (`.harness/expectations/regseq-driver-67.md`) as the primary
  triage raw material.
- The F2 expectations file
  (`.harness/expectations/regseq-driver-65.md`) for the
  IX-014 escalation source.
- The strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  for the §10 Mode B "propose decomposition" rule.

### Reviewer focus

- **Cluster coherence**: does the suggested cluster taxonomy
  in Change A correctly group the 12 active escalations by
  root cause? Are any escalations mis-clustered? Should any
  cluster be split or merged?
- **Out-of-scope discipline**: does the spec correctly exclude
  the F4-documented matrix blocks from the proposal scope?
- **Proposal-vs-filing separation**: is the
  "F68 produces proposals only; user approves; separate
  session files" workflow clearly demarcated? Any text that
  could be misread as authorizing auto-filing?
- **Empirical evidence chain**: does the spec require every
  proposal to cite expectations-file file:line evidence?
- **Authoring protocol clarity**: is the
  cluster-then-propose loop concrete enough for an implementer
  agent to follow?
- **Priority + dependency rules**: are the priority tiers
  (HIGH/MEDIUM/LOW) and dependency-encoding rules clear?
- **Citation drift**: every file:line citation against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/68-post-f4-batch-triage.md.

Identify:
- Cluster coherence: do the suggested clusters (a)-(f) correctly group the 12 active escalations (11 F4 + IX-014 F2)?
- Out-of-scope discipline: does the spec correctly exclude the F4-documented matrix blocks?
- Proposal-vs-filing separation: clear demarcation between propose / approve / file phases per strategy §10 Mode B?
- Empirical evidence chain: every proposal must cite expectations-file evidence?
- Authoring protocol clarity for the implementer.
- Priority + dependency encoding clarity.
- Citation drift against HEAD.
</task>

<grounding_rules>
- Cite file:line for every claim.
- For cluster coherence, inspect .harness/expectations/regseq-driver-67.md and .harness/expectations/regseq-driver-65.md for the actual escalation log entries.
- For the propose-not-file constraint, inspect docs/discussions/regseq-model/bridge-coverage-runner-strategy.md §10 Mode B.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
CLUSTER COHERENCE CHECK: PASS / FAIL with citation
OUT-OF-SCOPE DISCIPLINE: PASS / FAIL with citation
PROPOSAL-VS-FILING SEPARATION: PASS / FAIL with citation
EMPIRICAL EVIDENCE CHAIN: PASS / FAIL
AUTHORING PROTOCOL CLARITY: PASS / FAIL
PRIORITY + DEPENDENCY CLARITY: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean (no code changes).
- `cargo clippy --workspace --all-targets -- -D warnings` clean
  (no code changes).
- `cargo test --workspace -- --test-threads=1` green (no code
  changes; sanity check).
- `git diff --stat main..` shows changes confined to:
  - `.harness/audits/2026-05-21-post-f4-triage.md` (Change A: the
    triage document)
  - `.harness/expectations/regseq-driver-68.md` (Change B:
    minimal pointer to the audit doc)
  - `.harness/tasks.json` (registration entry)
- No source / runner / bridge / model / spec / matrix /
  strategy / pretty-printer / generic-c-backend changes.
- No new sidecar rows; no new snippets.
- Codex Task Review verdict: APPROVED or
  APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Triage document lands**:
   `.harness/audits/2026-05-21-post-f4-triage.md` exists with the
   structure described in Change A (Summary + per-cluster
   proposals + closing recommendations).

2. **All 12 active escalations covered**: every escalation
   from F2 (IX-014) + F4 (OP-003-I / RV-007-C / RV-009-G /
   TM-006-C / FF-004 / FF-005 / FF-012 / FF-013 / RJ-006 /
   RJ-024 / RJ-032-F) appears in at least one cluster + at
   least one proposal in the triage document.

3. **Each proposal carries the required fields**: title,
   affected escalations, root cause, proposed scope, priority,
   dependencies, estimated effort. No fields omitted.

4. **Empirical evidence cited**: every proposal cites at least
   one expectations-file `file:line` anchor or matrix-doc
   anchor as evidence for the root-cause claim.

5. **No auto-filing**: `git diff main -- .harness/tasks.json`
   shows only the F68 registration entry; no other
   `task.py add-task` invocations occurred.

6. **Out-of-scope discipline**: no proposal targets the
   F4-documented matrix blocks (per Non-Goals enumeration).

7. **Priority + dependency consistency**: the recommended
   priority ordering + dependency graph is internally
   consistent (no circular deps; dependencies precede
   dependents).

8. **Expectations file lands**:
   `.harness/expectations/regseq-driver-68.md` exists as a
   minimal pointer to the audit doc.

9. **No code modifications**: `git diff main` touches ONLY
   the three files listed in Quality Gate. No source / bridge
   / runner / matrix / spec changes.

10. **Closing recommendation lands**: the triage document's
    closing section recommends a specific priority order for
    the user to approve, justified by dependency chains and
    leverage analysis.
