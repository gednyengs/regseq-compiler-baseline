# regseq-driver-71: Closure-family reattempt sweep + matrix-doc OPEN BUG refresh (Category 1 from 2026-05-22 gap inventory)

## Description

This task implements **Category 1** from the 2026-05-22 gap
inventory at
`.harness/audits/2026-05-22-gap-inventory.md:131-202`. It has
two coupled parts:

1. **Matrix-doc OPEN BUG prose refresh** at
   `docs/discussions/regseq-model/bridge-coverage-matrix.md:508-523`.
   The current prose claims the bridge "currently fails" on a
   prescan args-substitution bug at `bridge/mod.rs:769` walking
   `tcx.optimized_mir(local_def_id)` and resolving polymorphic
   `<F as Fn>::call` via `Instance::try_resolve` at `mod.rs:1039`.
   This prose is **stale at HEAD**: per the gap inventory's
   Category 1 finding (cross-verified by the gap-inventory's
   Codex review at
   `.harness/audits/2026-05-22-gap-inventory.md:131-202`), the
   prescan args-substitution bug was already fixed by
   regseq-driver-62 at
   `.harness/expectations/regseq-driver-62.md:51-68`. HEAD's
   `bridge/mod.rs:769-789` calls `instance_mir(instance.def)` plus
   `instantiate_mir_and_normalize_erasing_regions(...)`. The
   refresh rewrites the section to (a) note the historical bug
   shape, (b) cite the driver-62 fix, (c) re-classify each row
   that was marked `blocked-open-bridge-bug` based on the
   reattempt outcomes from this task.

2. **Closure-family reattempt sweep** over the 13-row cohort
   enumerated at
   `.harness/audits/2026-05-22-gap-inventory.md:140-160`:
   IX-004, IX-010, BD-007, FT-011, FT-012, FT-013, FT-014,
   SK-003 (partial → committed if it passes), SK-004, SK-005,
   FF-007, RJ-009 (partial → committed if it passes), IX-014.
   Each row gets a snippet authored + sidecar entry committed
   per the F4 authoring protocol (per-row 3-retry budget; rows
   that fail all 3 retries escalate OUT of sidecar per the
   F4-established discipline at
   `.harness/HANDOFF.md` workflow section "Authoring protocol
   for snippet batches (F2, F4, future)").

The two parts are coupled because the matrix-doc refresh's
row-status updates depend on the reattempt sweep's empirical
outcomes. The implementer runs the sweep first, records
per-row PASS / escalation outcomes in
`.harness/expectations/regseq-driver-71.md`, then applies the
matrix-doc refresh + per-row status updates as a single edit.

### Why now

The 2026-05-22 gap inventory's Closing-recommendations table at
`.harness/audits/2026-05-22-gap-inventory.md:466-478` lists
Category 1 as Order 1 (MEDIUM priority, highest-leverage
gap-closing work after F68's bridge fixes). The 13-row cohort
is dependency-free relative to F68's A1 / B1 / B2 / C1 / C2 / F1
proposals; the task can run in parallel with F70's already-landed
A1 and any pending F68 follow-ups.

### Realistic outcome expectation

Honest scoping: not all 13 rows are guaranteed to PASS
post-driver-62. The implementer of A1 (regseq-driver-70 at
commit `4c1332e`) empirically discovered that the FF-005
`caller<F: Fn()>(f: F)` shape hits
`BridgeError::UnsupportedAbi` on `ExternAbi::RustCall` at
`crates/regseq-driver/src/bridge/fnsig.rs:107-115` BEFORE
FnPtrShim materialization. The Category 1 cohort contains
rows that may hit the SAME RustCall ABI rejection (FF-007
ClosureOnceShim, IX-014 closure consumed via FnOnce, possibly
others). Separately, SK-004 (FnMut path) and SK-005 (FnOnce
path) escalate against a DIFFERENT class of gap -- a
runner-side `InspectionPredicate::SyntheticItemKind`
sub-axis precision gap, not a bridge issue. The expected
outcome distribution is approximately:

- **Likely PASS post-driver-62** (9 rows; Tier 1 below):
  IX-004, IX-010, BD-007, FT-011, FT-012, FT-013, FT-014,
  SK-003 [partial → committed], RJ-009 [partial → committed].
  These rows were blocked specifically by the prescan
  args-substitution bug that driver-62 fixed; the closure-call
  FnPtr rewrite at `bridge/ty.rs:269-305` plus the
  terminator-level closure-call lowering at
  `bridge/body/terminator.rs:150-165` + `:178-184` handle the
  explicit-closure-call ABI surface (untuple the RustCall
  args into a Rust-ABI call) for them.
- **Uncertain — may escalate due to RustCall ABI** (2 rows;
  Tier 2 below): FF-007 (ClosureOnceShim), IX-014 (closure
  consumed via FnOnce). These rows route through
  `Fn::call` / `FnMut::call_mut` / `FnOnce::call_once`
  resolution paths whose FnPtr / shim branches may hit the
  same `translate_abi(ExternAbi::RustCall)` rejection at
  `crates/regseq-driver/src/bridge/fnsig.rs:107-115` that F70
  surfaced for FF-005. The implementer empirically verifies;
  rows that escalate document their per-row root cause in
  the expectations file and become inputs to a separate
  future follow-up task that addresses the closure-call sig
  translation gap.
- **Expected escalation due to missing F3 predicate sub-axis**
  (2 rows; Tier 3 below): SK-004 (CallMut path), SK-005
  (CallOnce path). The runner's
  `InspectionPredicate::SyntheticItemKind(String)` at
  `crates/bridge-coverage-runner/src/inspect.rs:50-53` only
  matches the variant TAG via `synthetic_kind_variant_tag`
  at `:168-174`, which collapses all `ClosureCall { .. }`
  sub-variants to the single tag `"ClosureCall"`. F2 IX-014
  evidence already flagged this need for a future
  sub-variant-matching predicate at
  `.harness/expectations/regseq-driver-65.md:238-240`. These
  rows escalate DESIGN-TIME (not empirically), because no
  amount of snippet authoring within F71's scope can let the
  runner distinguish CallMut / CallOnce from Call.

The Acceptance Criteria below treat all three outcome classes
as success provided the F4 escalation discipline is followed
for Tier-1 / Tier-2 empirical escalations AND the design-time
discipline is followed for Tier-3 escalations (no retry
budget; the per-row record cites the predicate-precision gap
directly). The matrix-doc refresh accurately reflects the
empirical + design-time outcomes.

### Out of scope

- **NO bridge code changes.** The driver-62 fix is already
  landed at HEAD; this task does NOT touch
  `crates/regseq-driver/src/bridge/`. Rows that escalate due
  to a separate bridge gap (e.g., the RustCall ABI rejection
  the FF-005 escalation surfaced) become inputs to a future
  task, NOT in-scope work for this one.
- **NO model / spec.md / strategy doc / pretty-printer doc /
  generic-c-backend doc changes.** Matrix doc is the only
  authoritative doc this task edits.
- **NO new `InspectionPredicate` dispatch arms.** The cohort
  rows use existing F3 arms (`SyntheticItemKind`,
  `FunctionInstanceFlavor`, etc.); if a cohort row needs an
  arm that doesn't exist at HEAD, the implementer escalates
  to the user with the inspect.rs evidence rather than
  silently adding a new arm.
- **NO new runner schema fields.** The F69 `BuildOverrides` /
  `RowBuildOverrides` surface is not needed for any of the 13
  cohort rows (none of them are gated on `overflow_checks` or
  `-Z mir-emit-retag`).
- **NO new helper dependencies, no new crate Cargo.toml
  changes.**
- **NO modifications to `tests/fixtures/playground/` or to any
  `tests/fixtures/playground-*/`** (standing playground rule
  per HANDOFF).
- **NO re-attempt of FF-005.** F70's escalation of FF-005 stands;
  the FnPtrShim sig-translation gap is a separate task class.
  Cohort row IX-014 is in scope here per the gap inventory's
  Category 1 row list, distinct from FF-005.
- **NO re-attempt of RV-009-G slice case.** F70 landed the sized
  specialization; the slice case is blocked by an unrelated
  bridge gap at `bridge/body/rvalue.rs:197` and is out of scope
  here.
- **NO new task filings via `task.py add-task`** beyond this
  task's self-registration. Rows that escalate become inputs
  to a future user-approval gate, not auto-filed follow-ups.

## Input Context

### Design source

- **Gap inventory Category 1** at
  `.harness/audits/2026-05-22-gap-inventory.md:131-202` --
  proposal source.
- **Gap inventory Closing recommendations** at
  `.harness/audits/2026-05-22-gap-inventory.md:466-478` --
  Order 1, MEDIUM priority.

### Stale matrix-doc prose (the refresh target)

- `docs/discussions/regseq-model/bridge-coverage-matrix.md:508-523`
  -- the "Notes on the open bug (IX-004 / IX-010)" section
  that currently describes the prescan args-substitution bug
  as if it were still open. Refresh target.

### HEAD bridge state (read-only; the matrix-doc refresh cites this)

- `crates/regseq-driver/src/bridge/mod.rs:769-789` --
  current `synthesise_instance_pre_scan` (or its renamed
  equivalent) calling `tcx.instance_mir(instance.def)` and
  `instantiate_mir_and_normalize_erasing_regions(...)`. This
  is the driver-62 fix's landing site.
- `crates/regseq-driver/src/bridge/mod.rs:1058-1066` -- the
  current `Instance::try_resolve` / callee-canonicalization
  call site at HEAD (the matrix prose's stale `:1039`
  reference now points at `TerminatorKind::Call` matching, not
  the resolution call). The implementer cites this updated
  range in the matrix-doc refresh.

### Driver-62 evidence (the matrix-doc refresh cites this)

- `.harness/expectations/regseq-driver-62.md:51-68` --
  driver-62 implementer's report of the prescan
  args-substitution fix.
- `.harness/tasks/regseq-driver/62-prescan-args-substitution.md:48-71`
  -- driver-62 task spec describing the bug shape.

### F2 evidence for IX-014 (existing escalation)

- `.harness/expectations/regseq-driver-65.md:223-247` -- IX-014
  per-candidate outcomes. Candidate A (direct closure
  consumption) panicked the bridge AND mapped to
  `FunctionInstanceFlavor::Item` (NOT `ClosureOnceShim`).
  Candidate B (generic `F: FnOnce`) hit the IX-004 / IX-010
  family. The implementer of this task re-runs both candidate
  shapes against HEAD's post-driver-62 bridge and records the
  outcome.

### F4 cohort blocking enumeration (existing context)

- `.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:79-96`
  -- F4 spec's blocked-open-bridge-bug + closure-feature
  enumeration. Each row in this enumeration that's in the
  Category 1 cohort gets a reattempt; rows in this enumeration
  that are NOT in the cohort (e.g., already-aliased rows like
  IX-013) stay deferred.

### F70 escalation context (the cohort's likely escalations match this pattern)

- `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1316-1340`
  -- F70's inline FF-005 escalation rationale. The same
  RustCall ABI rejection pattern at `bridge/fnsig.rs:107-115`
  may surface for cohort rows that route through
  `Fn::call` / `FnMut::call_mut` / `FnOnce::call_once` rather
  than the explicit closure-call shim.
- `crates/regseq-driver/src/bridge/fnsig.rs:107-115` --
  `translate_abi(ExternAbi::RustCall) -> Err(UnsupportedAbi)`
  rejection site.

### F3 dispatch surface (read-only; verify arms exist for each cohort row)

- `crates/bridge-coverage-runner/src/inspect.rs` -- all 22
  dispatch arms. The implementer cross-references each cohort
  row's expected outcome against this file to confirm an
  existing predicate arm matches.

### Matrix doc row anchors (the per-row status updates)

- `bridge-coverage-matrix.md:379-381` -- SK-003 / SK-004 /
  SK-005.
- `bridge-coverage-matrix.md:420` -- BD-007.
- `bridge-coverage-matrix.md:441-444` -- FT-011 / FT-012 /
  FT-013 / FT-014.
- `bridge-coverage-matrix.md:400` -- FF-007.
- `bridge-coverage-matrix.md:485` -- IX-004.
- `bridge-coverage-matrix.md:491` -- IX-010.
- `bridge-coverage-matrix.md:495` -- IX-014.
- `bridge-coverage-matrix.md:554` -- RJ-009.

### Prior tasks

- regseq-driver-62 (`.harness/expectations/regseq-driver-62.md`):
  prescan args-substitution fix.
- regseq-driver-65 (`3a01888`): F2 batch + IX-014 + IX-017
  escalations.
- regseq-driver-66 (`e2e77c0`): F3 dispatch hardening.
- regseq-driver-67 (`c8c16f3`): F4 corpus close.
- regseq-driver-68 (`c3456a5` + `4e14bb1`): F68 post-F4 triage.
- regseq-driver-69 (`660bc7f`): D1 close.
- regseq-driver-70 (`4c1332e`): A1 close (with FF-005
  escalation surfacing RustCall ABI pattern).

## Implementation Notes

### Change A: closure-family reattempt sweep

**Files**: `crates/bridge-coverage-runner/snippets/<row-id>.rs`
+ `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.

The implementer authors snippets + sidecar rows for the 13
cohort rows, per the F4 authoring protocol (per-row 3-retry
budget, escalated rows stay OUT of sidecar). The implementer
SHOULD batch the rows by their expected outcome class:

**Tier 1 -- Likely PASS post-driver-62 (9 rows; start here)**:
attempt these first because the prescan fix directly unblocks
them. Authoring should be relatively straightforward; each
should PASS within the 3-retry budget. If any tier-1 row
escalates, that's a NEW gap discovery worth flagging to the
user.

| Row | Construct (matrix anchor) | Predicate arm | Suggested shape |
| --- | -------------------------- | ------------- | --------------- |
| IX-004 | bridge-coverage-matrix.md:485 | TBD by implementer | `fn apply<F: Fn(i32, i32) -> i32>(x:i32, y:i32, f:F) -> i32 { f(x,y) } fn entry(x:i32,y:i32) -> i32 { apply(x,y,\|x,y\| x+y) }` (matrix's own minimal repro at `:512-515`) |
| IX-010 | bridge-coverage-matrix.md:491 | TBD | generic fn × trait method call inside body |
| BD-007 | bridge-coverage-matrix.md:420 | TBD | synthetic ClosureCall instance with materialised body |
| FT-011 | bridge-coverage-matrix.md:441-444 | TBD | closure zero captures via `impl Fn` (per matrix) |
| FT-012 | bridge-coverage-matrix.md:441-444 | TBD | closure with N captures |
| FT-013 | bridge-coverage-matrix.md:441-444 | TBD | closure with FnMut behavior |
| FT-014 | bridge-coverage-matrix.md:441-444 | TBD | closure with FnOnce behavior |
| SK-003 (partial → committed) | bridge-coverage-matrix.md:379 | SyntheticItemKind=ClosureCall | the `Fn::call` body path that was the OPEN BUG |
| RJ-009 (partial → committed) | bridge-coverage-matrix.md:554 | DslRejection=NonRigidTy + kind=Param | a `Param` surviving substitution; driver-60 prevented most, post-driver-62 should close the remaining gap |

**Tier 2 -- Uncertain; may escalate due to RustCall ABI (2 rows)**:
attempt after tier 1 lands. If any tier-2 row hits the
RustCall ABI rejection at
`crates/regseq-driver/src/bridge/fnsig.rs:107-115` (same
pattern as F70's FF-005 escalation), escalate per the F4
discipline and document in the expectations file.

| Row | Construct | Notes |
| --- | --------- | ----- |
| FF-007 | bridge-coverage-matrix.md:400 (ClosureOnceShim) | per F2 IX-014 evidence, this routes through FnOnce path |
| IX-014 | bridge-coverage-matrix.md:495 | F2 evidence shows two candidate shapes; the implementer re-runs Candidate A (direct closure consumption) AND Candidate B (generic FnOnce bound) against HEAD to see which (if either) works post-driver-62 |

**Tier 3 -- Expected escalation due to missing F3 predicate
sub-axis (2 rows)**: SK-004 and SK-005 require distinguishing
`SyntheticItemKind::ClosureCall { call_kind: CallMut }` and
`{ call_kind: CallOnce }` from the SK-003 `Call` variant. At
HEAD the runner's
`InspectionPredicate::SyntheticItemKind(String)` arm at
`crates/bridge-coverage-runner/src/inspect.rs:50-53` matches
only the variant TAG via `synthetic_kind_variant_tag` at
`:168-174`, which collapses all three `ClosureCall` sub-variants
to the single tag `"ClosureCall"`. F2 IX-014 evidence already
called this out as needing a "future sub-variant-matching
`SyntheticItemKind` predicate" at
`.harness/expectations/regseq-driver-65.md:238-240`. This task
does NOT add such a predicate (per the Non-Goals "No new
`InspectionPredicate` dispatch arms"); SK-004 and SK-005
therefore escalate by design with the per-row outcome record
pointing at the F2-recommended follow-up direction. The
escalation is NOT a bridge bug; it is a runner-precision gap
the implementer must NOT silently work around by claiming the
broad `SyntheticItemKind=ClosureCall` predicate covers
CallMut/CallOnce.

| Row | Construct | Notes |
| --- | --------- | ----- |
| SK-004 | bridge-coverage-matrix.md:380 (CallMut path) | escalates: existing `SyntheticItemKind` predicate cannot distinguish from SK-003's `Call` variant per `inspect.rs:168-174`. Follow-up: future runner-side sub-variant predicate task. |
| SK-005 | bridge-coverage-matrix.md:381 (CallOnce path) | escalates: same predicate-precision gap as SK-004. |

The implementer verifies each row's expected
`InspectionPredicate` arm exists at HEAD before authoring;
the F4 spec's "no F3-implemented predicate" deferral list at
`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:126-131`
remains in force.

### Change B: per-row escalation discipline (two modes)

Two escalation modes apply to this task:

**Mode 1 -- Empirical escalation** (Tier 1 + Tier 2 rows):
fires when a row fails all 3 retries against a candidate-shape
sweep. The implementer records:
1. **Row is NOT committed to sidecar** (zero-failures
   invariant preserved).
2. **Snippet file is NOT shipped** to `snippets/`.
3. **Expectations file records the per-row outcome**: the
   per-candidate shape tried, the bridge's emitted
   `BridgeError` or panic shape, the suspected root cause,
   and the recommended follow-up direction. Format mirrors
   F4's escalation log at
   `.harness/expectations/regseq-driver-67.md:207-417`.
4. **Inline rationale block in `rows.toml`** (analogous to
   F70's FF-005 rationale at
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1316-1340`)
   for each escalated row, with the citation chain.

**Mode 2 -- Design-time escalation** (Tier 3 rows; SK-004 +
SK-005 by spec construction): fires unconditionally because
the runner predicate-precision gap is known a priori. The
implementer does NOT exhaust a 3-retry budget for these rows
because no snippet shape can let the existing
`InspectionPredicate::SyntheticItemKind` arm distinguish
CallMut / CallOnce from Call. Instead:
1. **Row is NOT committed to sidecar** (same as Mode 1).
2. **Snippet file is NOT shipped** to `snippets/`.
3. **Expectations file records the design-time outcome**: a
   one-paragraph note that the row escalates due to the
   predicate-precision gap, with citations to
   `crates/bridge-coverage-runner/src/inspect.rs:50-53`,
   `:168-174`, `:604-609` and the F2 follow-up direction at
   `.harness/expectations/regseq-driver-65.md:238-240`. No
   bridge-emitted error is required because no snippet was
   compiled.
4. **Inline rationale block in `rows.toml`** for each Tier-3
   row, framing the escalation as a runner-side
   predicate-precision gap (distinct from the F70 RustCall
   ABI rationale's bridge-side framing).

### Change C: matrix-doc OPEN BUG prose refresh

**File**:
`docs/discussions/regseq-model/bridge-coverage-matrix.md`.

The "Notes on the open bug (IX-004 / IX-010)" section at
`:508-523` is rewritten. The refresh has TWO parts:

#### Part 1: prose refresh

Replace the current "currently fails" framing with historical
+ current truth. Suggested replacement text:

> ### Notes on the open bug (IX-004 / IX-010) -- historical
>
> Before regseq-driver-62 (commit at
> `.harness/expectations/regseq-driver-62.md:51-68`), the
> bridge failed on:
>
> ```rust
> fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 { f(x, y) }
> pub fn entry(x: i32, y: i32) -> i32 { apply(x, y, |x, y| x + y) }
> ```
>
> because the Phase-2 synthesis pre-scan at
> `crates/regseq-driver/src/bridge/mod.rs:769` walked
> `tcx.optimized_mir(local_def_id)` (the POLYMORPHIC body),
> not the substituted body, then attempted to resolve the
> polymorphic `<F as Fn>::call` callee via
> `Instance::try_resolve`. Same root-cause shape as
> driver-60 (process polymorphic MIR without applying
> `instance.args`), different pass.
>
> **regseq-driver-62 landed the fix**: HEAD's
> `crates/regseq-driver/src/bridge/mod.rs:769-789` now calls
> `tcx.instance_mir(instance.def)` plus
> `instantiate_mir_and_normalize_erasing_regions(...)`,
> processing the substituted body. The minimal repro above
> now translates cleanly through Phase 2.
>
> **regseq-driver-71** (the closure-family reattempt sweep
> + this refresh) re-verified the cohort that was previously
> blocked by this bug. The per-row reattempt outcomes are
> recorded in
> `.harness/expectations/regseq-driver-71.md`. The
> row-status table entries that previously read
> `blocked-open-bridge-bug` are reclassified per those
> outcomes:
> - **Rows that PASS** become `covered`.
> - **Rows that escalate against a SEPARATE bridge gap**
>   (e.g., the RustCall ABI rejection F70 surfaced at
>   `crates/regseq-driver/src/bridge/fnsig.rs:107-115`)
>   get a status note pointing at the new bridge-side
>   follow-up.
> - **Rows that escalate against a runner-side
>   predicate-precision gap** (e.g., the missing
>   `ClosureCallKind` sub-axis in
>   `InspectionPredicate::SyntheticItemKind` at
>   `crates/bridge-coverage-runner/src/inspect.rs:50-53` +
>   `:168-174` that prevents distinguishing CallMut /
>   CallOnce from Call) get a status note pointing at the
>   F2 IX-014 follow-up direction recorded at
>   `.harness/expectations/regseq-driver-65.md:238-240`.
>
> The `IX-004` and `IX-010` row entries themselves are
> updated below; downstream rows are updated in their own
> sections.

#### Part 2: per-row status updates

Based on each cohort row's outcome (empirical for Tier 1 /
Tier 2; design-time for Tier 3), each of the 13 cohort row
entries in the matrix doc gets its `status` field + notes
updated per the B6 reclassification policy. The baseline
status varies per row (see B6's baseline table); the three
reclassification kinds are:

- **PASSed rows**: status becomes `covered`. Applies
  regardless of baseline -- a `partial` row whose committed
  sidecar entry exercises one of the remaining
  specializations becomes `covered`. Notes column updated to
  point at the snippet path.
- **Empirically escalated against a SEPARATE bridge gap**
  (Tier 2 RustCall ABI rejection): status retains a
  `blocked-*` classification with refreshed notes pointing
  at the new bridge follow-up (`bridge/fnsig.rs:107-115`
  citation). The implementer may either re-use
  `blocked-open-bridge-bug` with a refreshed note, or
  introduce a new status class (e.g., `blocked-rustcall-abi`)
  if the count of bridge-side gaps grows -- the implementer's
  judgment, subject to user approval at the review stage.
- **Design-time escalated against a RUNNER predicate-precision
  gap** (Tier 3 SK-004 / SK-005): status retains the existing
  classification (`uncovered` for both rows; the matrix is
  not made stricter). Notes column updated to point at the F2
  follow-up direction at
  `.harness/expectations/regseq-driver-65.md:238-240` and the
  runner-side `inspect.rs:168-174` tag-collapse evidence. The
  implementer does NOT introduce a "blocked-runner-precision"
  status class; the `uncovered` baseline is preserved because
  the row is materially uncovered until the F2-recommended
  sub-variant predicate lands.

The implementer authors Part 1 before Part 2 because Part 1's
prose informs the per-row reclassification policy. Both parts
land in a SINGLE matrix-doc edit so the doc is internally
consistent at every commit boundary.

### Change D: NO bridge / model / spec / strategy / pretty-printer / generic-c-backend changes

This task is purely sidecar authoring + matrix-doc editing.
Any bridge change required by an escalated cohort row becomes
a NEW follow-up task; the implementer escalates to the user
with the empirical evidence rather than unilaterally expanding
scope.

## Non-Goals

- Do NOT touch any source code in
  `crates/regseq-driver/src/bridge/`,
  `crates/regseq-model/src/`,
  `crates/bridge-coverage-runner/src/`, or
  `crates/test-ir-fixtures/src/`. This task is
  authoring-and-docs only.
- Do NOT touch `docs/discussions/regseq-model/spec.md`,
  `bridge-coverage-runner-strategy.md`,
  `pretty-printer.md`, or any
  `<target>-backend.md` artifact.
- Do NOT add new `InspectionPredicate` arms.
- Do NOT add new dependencies anywhere.
- Do NOT modify `tests/fixtures/playground/` or
  `tests/fixtures/playground-*/`.
- Do NOT re-attempt F70's FF-005 escalation. The FnPtrShim
  sig-translation gap is a separate follow-up task class.
- Do NOT re-attempt F70's RV-009-G slice case (separate
  `bridge/body/rvalue.rs:197` gap).
- Do NOT re-attempt other F68 proposals (B1 / B2 / C1 / C2 /
  F1 remain owned by the F68 audit).
- Do NOT call `task.py add-task` for any escalated row's
  follow-up. Escalations become inputs to a future
  human-approval-gated triage.
- Do NOT update the matrix doc beyond the OPEN BUG section
  refresh + the cohort row-status reclassifications. Other
  matrix entries stay as-is.
- Do NOT relitigate F4's "documented blocks" enumeration; the
  blocks at `:70-144` of the F4 task spec remain intentional
  out-of-scope.

## Test Plan

### B1: workspace stays green

`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
green. This task is authoring-and-docs only; no code is
modified so the workspace test outcome is identical to the
F70-close state.

### B2: runner end-to-end Mode A reports matching the expectations file

Manual Mode A invocation:

```
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-71/
```

Expected: exit 0; `Total = 127 + N` where N is the count of
cohort rows that PASSed (committed to sidecar); `Fail = 0`.
The implementer records the exact N in the expectations file
and the matrix doc's per-row updates.

### B3: per-row PASS verification

For each cohort row committed to sidecar, the runner's per-row
report MUST show PASS against the row's `inspect` predicate
(positive rows) or `expected_rejection` shape (rejected rows,
of which there is one in the cohort: RJ-009 if it commits).

### B4: per-row escalation documentation (two modes)

**Mode 1 (empirical)**: for each cohort row in Tier 1 / Tier 2
that escalates after failing all 3 retries:
- The expectations file records the per-candidate-shape
  outcome with bridge-emitted error citations.
- The `rows.toml` carries an inline rationale block analogous
  to F70's FF-005 block at
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1316-1340`.
- The matrix doc's row entry status / notes are updated per
  B6's reclassification policy.

**Mode 2 (design-time)**: for SK-004 / SK-005 (Tier 3,
unconditional escalation):
- The expectations file records a one-paragraph design-time
  note citing the runner predicate-precision gap at
  `crates/bridge-coverage-runner/src/inspect.rs:50-53` +
  `:168-174` + `:604-609` and the F2 follow-up direction at
  `.harness/expectations/regseq-driver-65.md:238-240`. No
  per-candidate snippet outcome is recorded because no
  snippet was compiled.
- The `rows.toml` carries an inline rationale block framing
  the escalation as a runner-side predicate-precision gap
  (distinct from F70's bridge-side RustCall ABI framing).
- The matrix doc's row entry status / notes are updated per
  B6's reclassification policy.

### B5: matrix doc OPEN BUG section refresh

The matrix doc at `:508-523` no longer says "currently fails".
The refresh's prose accurately reflects HEAD's bridge state
(driver-62 fix landed; closure-family rows reattempted per this
task; remaining escalations point at NEW root causes, not the
old prescan bug).

### B6: matrix doc per-row reclassifications

Each of the 13 cohort rows in the matrix doc carries an updated
`status` field + notes reflecting its outcome. The 13 rows
have MIXED baseline statuses at HEAD (not all are
`blocked-open-bridge-bug`):

| Row | Baseline status at HEAD | Matrix anchor |
| --- | ------------------------ | ------------- |
| IX-004 | `blocked-open-bridge-bug` | `:485` |
| IX-010 | `blocked-open-bridge-bug` | `:491` |
| BD-007 | `blocked-open-bridge-bug` | `:420` |
| FT-011 | `blocked-open-bridge-bug` | `:441-444` |
| FT-012 | `uncovered` (empirically blocked) | `:442` |
| FT-013 | `uncovered` (empirically blocked) | `:443` |
| FT-014 | `uncovered` (empirically blocked) | `:444` |
| SK-003 | `partial` | `:379` |
| SK-004 | `uncovered` | `:380` |
| SK-005 | `uncovered` | `:381` |
| FF-007 | `uncovered` | `:400` |
| IX-014 | `uncovered` (F2 escalated) | `:495` |
| RJ-009 | `partial` | `:554` |

Three reclassification kinds apply (matching the Part-1
refresh prose's enumeration):

- **PASSed rows** (Tier 1 → committed): status becomes
  `covered`. Notes column updated to point at the snippet
  path. Applies regardless of baseline status -- a `partial`
  row whose committed sidecar entry exercises one of the
  remaining specializations becomes `covered`.
- **Escalated against a SEPARATE bridge gap** (Tier 2 if it
  hits RustCall ABI rejection): status retains a `blocked-*`
  classification (e.g., `blocked-open-bridge-bug` is misleading
  after driver-62 landed; the implementer may use the existing
  status with a refreshed note, or add a new status class if
  the count of bridge-side gaps grows -- the implementer's
  judgment, subject to user approval at the review stage).
  Notes column updated to point at the new bridge follow-up
  with the `bridge/fnsig.rs:107-115` citation.
- **Escalated against a RUNNER predicate-precision gap**
  (Tier 3, SK-004 / SK-005): status retains the existing
  classification (currently `uncovered`). Notes column updated
  to point at the F2 follow-up direction at
  `.harness/expectations/regseq-driver-65.md:238-240` and the
  runner-side `inspect.rs:168-174` tag-collapse evidence.

The implementer applies the reclassification kinds based on
B4's per-row outcome record. No row gets its baseline status
silently widened beyond what the empirical or design-time
evidence supports.

### B7: no regression on the existing 127 rows

`target/debug/bridge-coverage-runner` Mode A output's per-row
PASS / FAIL classification for the 127 pre-F71 rows is
identical to the F70-close state. The implementer compares
against the F70-close report at `/tmp/a1-mode-a/report.md`
(the assessor's snapshot from F70 close) OR against a fresh
pre-Change-A Mode A run.

### B8: drift check still passes

The matrix sidecar drift check (subset-OK) reports clean: all
N committed cohort rows are documented in the matrix doc; the
matrix doc's per-row updates are consistent with the sidecar.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md`).
- The gap inventory
  (`.harness/audits/2026-05-22-gap-inventory.md`) -- Category
  1 proposal source.
- The matrix doc
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md`)
  -- the refresh target.
- The driver-62 expectations
  (`.harness/expectations/regseq-driver-62.md`) -- prescan-fix
  evidence.
- The F2 IX-014 evidence
  (`.harness/expectations/regseq-driver-65.md`).
- The F4 documented-blocks enumeration
  (`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md`).
- The F70 escalation rationale at `rows.toml:1316-1340` and
  the RustCall ABI rejection at `bridge/fnsig.rs:107-115`.

### Reviewer focus

- **Cohort completeness**: is the 13-row cohort accurately
  enumerated against the gap inventory? Are there closure-family
  rows in the matrix doc that should be in this task but aren't
  (e.g., SK-003's variants)?
- **Tier-1 / Tier-2 outcome predictions**: is the predicted
  PASS / escalation distribution reasonable? Does it match the
  HEAD bridge surface?
- **Matrix-doc refresh accuracy**: does the proposed Part-1
  prose correctly reflect HEAD's bridge state? Does it
  preserve the historical context for future readers?
- **Escalation discipline**: does the spec cleanly distinguish
  PASS rows from escalation rows? Does the per-row escalation
  documentation pattern (expectations + inline rationale)
  match F70's precedent?
- **Out-of-scope discipline**: does the spec correctly carve
  around F68's remaining proposals (B1 / B2 / C1 / C2 / F1)
  and F70's escalations (FF-005, RV-009-G slice)?
- **No-code-change enforcement**: does the spec foreclose
  unilateral bridge changes if a cohort row surfaces a NEW
  bridge gap? Are the escalation paths clear?
- **Matrix-doc edit scope**: does the spec restrict matrix-doc
  edits to (a) the OPEN BUG section refresh and (b) per-row
  status updates for the 13 cohort rows ONLY? No collateral
  matrix edits.
- **Citation drift at HEAD** (HEAD is `1d92725` at time of
  spec authoring).
- **No new task filings**: every escalation routes back to
  user-approval, never to `task.py add-task`.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md.

This is the proposed task spec for Category 1 from the
2026-05-22 gap inventory. It authors snippets + sidecar entries
for a 13-row closure-family cohort whose underlying blocker
(prescan args-substitution bug) was already fixed by
regseq-driver-62, AND refreshes the matrix doc's stale OPEN BUG
prose at bridge-coverage-matrix.md:508-523.

Identify defects under the following adversarial checks:

1. COHORT COMPLETENESS: is the 13-row enumeration complete and accurate against the matrix doc + gap inventory?

2. TIER PREDICTIONS: is the predicted PASS / escalation distribution (Tier 1 = 9 likely PASS, Tier 2 = 2 uncertain due to RustCall ABI, Tier 3 = 2 expected escalation due to missing F3 predicate sub-axis) reasonable given HEAD's bridge surface? Verify against `bridge/fnsig.rs:107-115`, `bridge/body/terminator.rs:150-165` + `:178-184`, `bridge/ty.rs:269-305` closure-call FnPtr rewrite, and `inspect.rs:168-174` synthetic-kind tag function.

3. MATRIX-DOC REFRESH ACCURACY: does the proposed Part-1 prose correctly reflect HEAD's bridge state? Does the citation chain (driver-62 fix, mod.rs:769-789, F70's RustCall ABI surface) hold?

4. ESCALATION DISCIPLINE: does the spec match F70's escalation pattern (expectations + inline rationale)? Does the "rows that fail all 3 retries escalate" rule preserve the zero-failures sidecar invariant?

5. OUT-OF-SCOPE CARVING: does the spec correctly exclude F70's FF-005 + RV-009-G slice escalations? Does it correctly exclude F68's B1/B2/C1/C2/F1 proposals?

6. NO-CODE-CHANGE ENFORCEMENT: does the spec foreclose unilateral bridge changes if a cohort row surfaces a NEW bridge gap?

7. MATRIX-DOC EDIT SCOPE: does the spec restrict matrix-doc edits to (a) OPEN BUG section refresh + (b) per-row status updates for the 13 cohort rows ONLY?

8. CITATION DRIFT AT HEAD (1d92725).

9. ROW-COUNT ARITHMETIC: 127 + N total where N is the PASSed cohort count. Is the math correct?

10. NEW TASK FILINGS: does the spec correctly forbid task.py add-task invocations for escalation follow-ups?
</task>

<grounding_rules>
- Cite file:line for every claim against HEAD (`1d92725`).
- For each cited file:line in the task spec, open the file and verify the citation. Quote the source, do not paraphrase the spec's claim.
- Mark hypotheses as hypotheses.
- Pinned toolchain is `nightly-2026-02-01`; HEAD is `1d92725`.
- For closure-call shim / FnPtr rewrite semantics, read `bridge/ty.rs:269-305` and `bridge/body/terminator.rs:150-165` + `:178-184` and any referenced rewrite helpers.
- For SyntheticItemKind predicate precision, read `crates/bridge-coverage-runner/src/inspect.rs:50-53` (predicate enum), `:168-174` (tag function), and `:604-609` (dispatch).
- For RustCall ABI semantics, read `bridge/fnsig.rs:107-115` and the surrounding `translate_abi` callers.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
COHORT COMPLETENESS: PASS | FAIL with cite
TIER PREDICTIONS: PASS | FAIL with per-tier cite
MATRIX-DOC REFRESH ACCURACY: PASS | FAIL with cite
ESCALATION DISCIPLINE: PASS | FAIL with cite
OUT-OF-SCOPE CARVING: PASS | FAIL with cite
NO-CODE-CHANGE ENFORCEMENT: PASS | FAIL with cite
MATRIX-DOC EDIT SCOPE: PASS | FAIL with cite
CITATION DRIFT AT HEAD: list each drifted anchor or "none"
ROW-COUNT ARITHMETIC: PASS | FAIL with cite
NEW TASK FILINGS: PASS | FAIL with cite
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean (no source code modified).
- `cargo clippy --workspace --all-targets -- -D warnings` clean
  (no source code modified).
- `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
  green (no source code modified).
- Manual Mode A invocation of the runner produces exit 0,
  `Total = 127 + N`, `Pass = 127 + N`, `Fail = 0` (where N is
  the count of cohort rows that PASSed and were committed).
- `git diff --stat main..` shows changes confined to:
  - `docs/discussions/regseq-model/bridge-coverage-matrix.md`
    (Change C: OPEN BUG section refresh + per-row status
    updates).
  - `crates/bridge-coverage-runner/snippets/<row-id>.rs`
    (Change A: N snippet files).
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change A: N row entries + escalation rationale blocks for
    the other (13 - N) rows).
  - `.harness/expectations/regseq-driver-71.md` (the per-row
    outcome report).
- No bridge / model / runner-source / helper / spec / strategy
  / pretty-printer / generic-c-backend changes.
- No new dependencies in any crate's Cargo.toml.
- No new `InspectionPredicate` arms.
- No `task.py add-task` invocations beyond this task's self-
  registration entry.
- Codex Task Review verdict: APPROVED or
  APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. The matrix doc's "Notes on the open bug (IX-004 / IX-010)"
   section at
   `docs/discussions/regseq-model/bridge-coverage-matrix.md:508-523`
   is refreshed: no longer says "currently fails"; cites the
   driver-62 fix; notes the closure-family reattempt sweep's
   per-row outcomes pointer.
2. Each of the 13 cohort rows has a documented per-row outcome
   in `.harness/expectations/regseq-driver-71.md`. The record
   format depends on the row's escalation mode (per B4):
   - **Tier 1 / Tier 2 rows (empirical)**: per-candidate-shape
     attempts, bridge-emitted error (if any), and follow-up
     direction.
   - **Tier 3 rows (SK-004 / SK-005, design-time)**: a
     one-paragraph predicate-precision note per B4 Mode 2;
     no per-candidate snippet outcome required because no
     snippet was compiled.
3. For each cohort row that PASSed reattempt: snippet file
   shipped, sidecar entry committed, matrix doc row-status
   updated to `covered` (per B6's reclassification policy --
   any PASSed cohort row becomes `covered` regardless of
   baseline status, since the committed sidecar entry
   exercises the construct).
4. For each cohort row that escalated: snippet file NOT
   shipped, sidecar entry NOT committed, inline rationale
   block added to `rows.toml`, matrix doc row-status / notes
   updated per the reclassification policy in B6. The
   expectations-file record format depends on the escalation
   mode (per B4):
   - **Mode 1 (empirical, Tier 1 / Tier 2)**: per-candidate
     outcome with bridge-emitted error citation chain.
   - **Mode 2 (design-time, Tier 3 SK-004 / SK-005)**: a
     one-paragraph predicate-precision note citing
     `crates/bridge-coverage-runner/src/inspect.rs:50-53` +
     `:168-174` + `:604-609` and the F2 follow-up direction
     at `.harness/expectations/regseq-driver-65.md:238-240`.
     No per-candidate snippet outcome is required because
     no snippet was compiled.
5. Runner Mode A reports `Total = 127 + N`, `Pass = 127 + N`,
   `Fail = 0`, exit 0 (where N is the PASSed cohort count).
6. Existing 127 rows continue to PASS with the same per-row
   classification as the F70-close state.
7. `cargo fmt --all -- --check`,
   `cargo clippy --workspace -- -D warnings`,
   `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
   all green.
8. Codex Task Review verdict: APPROVED or
   APPROVED_WITH_MINOR_FIXES.
9. No bridge / model / runner-source / helper / spec /
   strategy / pretty-printer / generic-c-backend file
   modifications.
10. No new dependencies in any crate.
11. No new `InspectionPredicate` dispatch arms; if any
    cohort row needs one that doesn't exist at HEAD, the
    implementer escalates to the user with the inspect.rs
    evidence rather than silently adding a new arm.
12. No `task.py add-task` invocations beyond this task's
    self-registration. Escalated rows become inputs to a
    future human-approval-gated triage; this task does NOT
    file any follow-up tasks.
13. Matrix doc edits are confined to (a) the OPEN BUG section
    refresh + (b) per-row status updates for the 13 cohort
    rows ONLY. No collateral matrix edits.
14. Tier-1 vs Tier-2 outcome split: at least 5 cohort rows
    PASS (lower-bound sanity; if fewer than 5 PASS, the
    implementer escalates to the user before completing the
    matrix-doc refresh, since the wholesale escalation may
    indicate a misdiagnosis of the driver-62 fix's scope).
