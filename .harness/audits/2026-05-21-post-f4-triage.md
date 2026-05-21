# Post-F4 batch triage -- 2026-05-21

This document is the deliverable of `regseq-driver-68`
(Post-F4 batch triage, strategy section 10 Mode B). It is a
PROPOSAL artifact only: each cluster below sketches a follow-up
`regseq-driver-NN` task for human approval. F68 does NOT file
any task itself per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md:464-473`
("propose decomposition, NOT file from report") and
`.harness/task-spec-workflow.md:118-123` (explicit human
approval before `task.py add-task`). After review, the user
approves a subset of proposals; each approved proposal becomes
its own task-spec-drafting session in a follow-up invocation.

## Summary

| Field | Value |
| ----- | ----- |
| Total active escalations analyzed | 12 (11 from F4 + IX-014 from F2) |
| Documented out-of-scope matrix blocks excluded | per `.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:71-143` |
| Proposed follow-up tasks | 7 |
| Priority tiers used | HIGH / MEDIUM / LOW |

Active escalations covered by proposals (per Acceptance Criterion 2):

| Row | F-batch source | Cluster | Proposal |
| --- | -------------- | ------- | -------- |
| OP-003-I  | F4 | (C) const-eval surface | C1 |
| RV-007-C  | F4 | (A) type normalization  | A1 |
| RV-009-G  | F4 | (A) type normalization  | A1 |
| TM-006-C  | F4 | (D) helper / build-flag | D1 |
| FF-004    | F4 | (B) deferred-shim probe | B2 |
| FF-005    | F4 | (A) type normalization  | A1 |
| FF-012    | F4 | (B) shim materialization| B1 |
| FF-013    | F4 | (C) const-eval surface  | C2 |
| RJ-006    | F4 | (D) helper / build-flag | D1 |
| RJ-024    | F4 | (B) deferred-shim probe | B2 |
| RJ-032-F  | F4 | (F) external-dependency | F1 |
| IX-014    | F2 | (F) external-dependency | F1 |

Proposed priority tier counts: HIGH=2, MEDIUM=3, LOW=2.

Out-of-scope per `.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:46-60`
and `:215-244`: F68 does NOT propose tasks for the F4-documented
matrix blocks (`blocked-rustc-infeasible` rows, Section 7
validator invariants, Section 2.2 closure-call family, Section 3
closure features, Section 4 cross-edition rows, Section 6
hard-to-author rejections, aliased-by-IX rows, rows lacking F3
predicates, negative-assertion rows, bridge-producer-missing
rows, or the OPEN BUG family's IX-004/IX-010 parallel
investigation). The single IX-014 pointer proposal (F1)
satisfies the "every active escalation appears in a proposal"
rule for the OPEN BUG family without re-proposing the underlying
IX-004/IX-010 work; see Non-Goals item 7 of the F68 task spec.

## Cluster (A) -- Bridge type-projection normalization gaps

**Affected escalations**: RV-007-C, RV-009-G, FF-005
(matrix rows
`docs/discussions/regseq-model/bridge-coverage-matrix.md:282`,
`:294`, `:398`).

**Root cause family**: `TyKind::Alias` projections of the form
`<T as Pointee>::Metadata` and `<fn(...) as FnOnce<...>>::Output`
are not normalized through to a concrete type in the bridge's
`translate_ty` path. Per
`crates/regseq-driver/src/bridge/ty.rs:296`, an
unnormalized `TyKind::Alias(..)` short-circuits to
`BridgeError::NonRigidTy { kind: Alias, .. }` (the rejection
helper is defined at `crates/regseq-driver/src/bridge/ty.rs:694-707`).
Empirically, RV-007-C's slice and sized candidates hit
`NonRigidTy` on `<[u32] as Pointee>::Metadata` and
`<u32 as Pointee>::Metadata`; the `dyn Tr` candidate hits
`Foreign` on `core::ptr::metadata::VTable` (an extern type),
which is a downstream consequence of the same projection not
being normalized through to a concrete `DynMetadata`. See
`.harness/expectations/regseq-driver-67.md:238-254` for the
per-candidate outcomes. RV-009-G
hits the same family because `core::ptr::from_raw_parts` and
`core::intrinsics::aggregate_raw_ptr` are parameterized by
`Pointee::Metadata`, per
`.harness/expectations/regseq-driver-67.md:256-262`. FF-005's
three candidates hit `NonRigidTy` on
`<fn() as core::ops::FnOnce<()>>::Output` per
`.harness/expectations/regseq-driver-67.md:303-318`.

### Proposal: regseq-driver-NN -- Normalize associated-type projections for `Pointee::Metadata` and `Fn`-family `::Output`

**Affected escalations**: RV-007-C (F4), RV-009-G (F4),
FF-005 (F4).

**Root cause**: Bridge does not normalize
`TyKind::Alias(Projection, ..)` for `Pointee::Metadata` and
`<fn(..) as FnOnce>::Output` projections, so concrete type
information is lost at the bridge boundary. See
`.harness/expectations/regseq-driver-67.md:238-262` and
`:303-318` for the empirical probes; see
`crates/regseq-driver/src/bridge/ty.rs:296` for the
short-circuit site.

**Proposed scope**:
Extend the bridge's type-translation entry to normalize the two
associated-type families to their concrete monomorphic types
before the `TyKind::Alias` short-circuit fires. For
`Pointee::Metadata`: route slice pointees to
`usize`, `dyn Trait` pointees to `DynMetadata<dyn Trait>`, and
sized pointees to `()` (the empty tuple). For
`<fn(..) as FnOnce<..>>::Output` (and the `Fn` / `FnMut`
analogues): unfold to the function-pointer return type via
`tcx.normalize_erasing_regions` or an equivalent rigid-projection
resolution. The fix lives in or adjacent to `translate_ty`
(`crates/regseq-driver/src/bridge/ty.rs:23-296`). Tests bring
RV-007-C (slice + dyn + sized PtrMetadata), RV-009-G
(`AggregateKind::RawPtr` slice + sized), and FF-005 (FnPtrShim
via `caller(fp)`) to PASS via runner re-attempt against the
three escalated snippet shapes documented in
`.harness/expectations/regseq-driver-67.md:238-318`.

**Priority**: HIGH

**Rationale**: A single normalization extension unblocks three
matrix rows across two sub-axes (`UnOp::PtrMetadata`,
`AggregateKind::RawPtr`, `FFlavor=FnPtrShim`). The fix is bounded
to one code path in `ty.rs`; the test surface is well-defined by
the F4 escalation candidate shapes. Highest leverage per unit of
bridge change.

**Dependencies**: none.

**Estimated effort**: moderate bridge change (one normalization
extension; three sidecar re-attempts).

## Cluster (B) -- Shim materialization gaps + deferred-shim probes

The F4 expectations file documents three distinct shim
materialization gaps with non-identical root causes, so this
cluster splits into two proposals: one tractable bridge-side
fix (B1: FF-012 CloneShim) and one analytical probe + defer
study (B2: FF-004 ReifyShim, RJ-024 UnsupportedEnumLayout).

### Proposal: regseq-driver-NN -- CloneShim materialization for arrays/tuples (B1)

**Affected escalations**: FF-012 (F4)
(matrix row `docs/discussions/regseq-model/bridge-coverage-matrix.md:405`).

**Root cause**: CloneShim materialization for the
`[u32; 4]` shape fails with `UnsupportedConst` on the
array-length const `4_usize`; the tuple shape fails Phase5
`ItemMissingInstance` on the tuple's Clone glue. Per
`.harness/expectations/regseq-driver-67.md:320-335`, the
array shape's `UnsupportedConst` is the simpler unblock.
The derived-Clone shape does NOT produce a CloneShim instance
(the derive generates a user `Clone` impl body, not a shim) --
the array/tuple shapes are the only source-authorable paths to
CloneShim.

**Proposed scope**:
Diagnose why the bridge rejects the array-length const
`4_usize` while materializing `<[u32; 4] as Clone>::clone`'s
CloneShim instance. Likely the rejection happens during
auxiliary-instance enumeration when the bridge re-enters
`translate_ty_const` over an unevaluated array length carried
by the shim signature. Extend the const-translation entry
(`crates/regseq-driver/src/bridge/const_.rs:42-49`) or the
auxiliary-instance scan to handle the array-length usize
const, then re-attempt FF-012 with the `[u32; 4].clone()`
shape from `.harness/expectations/regseq-driver-67.md:320-335`.
If the tuple shape's `ItemMissingInstance` is a downstream
symptom of the same array-length issue, both shapes will
unblock; if not, file a follow-up under a separate task.

**Priority**: MEDIUM

**Rationale**: One row unblock; bounded const-translation
extension. Smaller scope than Cluster A's normalization
extension and isolated from other proposals.

**Dependencies**: none.

**Estimated effort**: small-to-moderate bridge change.

### Proposal: regseq-driver-NN -- Deferred-shim probe + decision (FF-004 ReifyShim, RJ-024 UnsupportedEnumLayout) (B2)

**Affected escalations**: FF-004 (F4), RJ-024 (F4)
(matrix rows
`docs/discussions/regseq-model/bridge-coverage-matrix.md:397`,
`:574`).

**Root cause**: Both rows have empirically-confirmed no source
path under the pinned nightly. FF-004 per
`.harness/expectations/regseq-driver-67.md:280-301`: rustc's
monomorphizer does not synthesize `InstanceKind::ReifyShim`
for the three tried shapes (plain `fn` reify,
`extern "C" fn` reify, `#[track_caller]` reify); the
track-caller path that should produce a ReifyShim per
`rustc_middle/src/ty/instance.rs::resolve_for_fn_ptr` is
elided in the pinned-nightly's optimized MIR. RJ-024 per
`.harness/expectations/regseq-driver-67.md:375-391`: the
bridge's three `UnsupportedEnumLayout` paths (`Scalar::Union`
tag, `Float` tag, niche-variant out-of-range) are
bridge-internal invariant violations that rustc's layout
solver does not produce for source-authored enums under the
pinned toolchain.

**Proposed scope**:
This is a probe + decision task, NOT a bridge-fix task.
The implementer:
1. Probes broader source shapes than the F4 3-retry budget
   allowed for each row -- e.g., reifying `#[rustc_intrinsic]`
   functions for FF-004; layouts that force a niche range
   smaller than the variant count, or `repr(u128)` enums on
   targets that map `u128` through unusual tag paths, for
   RJ-024.
2. For each row, either (a) finds a source shape that surfaces
   the predicate and commits the row to the sidecar, or
   (b) documents the row as "empirically unauthorable from
   source under the current build contract" and updates the
   matrix doc's row annotation accordingly (the matrix doc
   currently lists both rows as `uncovered`; the deferral
   would re-classify them to `blocked-empirically-unauthorable`).
3. The proposal is deliberately scoped to NOT introduce a
   synthetic shim/layout producer in the bridge; that is a
   larger architectural change that should be a separate
   approval gate.

**Priority**: LOW

**Rationale**: High uncertainty payoff; the F4 probes already
exhausted the obvious shapes. Either a marginal coverage gain
(1-2 rows) or a deferral that crystallizes the matrix's
"empirically blocked" set. Lowest leverage per unit of
implementer time.

**Dependencies**: none.

**Estimated effort**: documentation-only-to-small (depending on
whether any probe succeeds).

## Cluster (C) -- Const-eval surface gaps

Two escalations require expansions to the bridge's
`mir::Const::eval` and const-translation surfaces, but along
distinct axes: OP-003-I extends ADT-enum unevaluated-const
support (a value-tree path); FF-013 extends fn-pointer-provenance
allocation support (a `Const::Reloc` path). They are split into
two proposals because the fix locations differ.

### Proposal: regseq-driver-NN -- Const-eval support for unevaluated ADT-enum consts (C1)

**Affected escalations**: OP-003-I (F4)
(matrix row `docs/discussions/regseq-model/bridge-coverage-matrix.md:200`).

**Root cause**: The bridge's `mir::Const::eval` call at
`crates/regseq-driver/src/bridge/const_.rs:42-49` cannot
evaluate the unevaluated `mir::Const::Unevaluated` carrier
emitted for anonymous const blocks and named const references
used in const-generic positions; specifically, the three
candidates documented at
`.harness/expectations/regseq-driver-67.md:216-236` all reject
with `UnsupportedConst { const_repr: "UnevaluatedConst { def: f::{constant#0}, args: [] }" }` (or
equivalent at a named-const def-id). The ADT-enum value-tree
path at `crates/regseq-driver/src/bridge/const_.rs:91-103`
that produces `Const::EnumValue` therefore never executes.

**Proposed scope**:
Extend the const-translation entry to either (a) coax MIR const
folding (`generic_const_exprs`, lazy normalization, or
`ConstantKind::eval_for_mir`) into evaluating the unevaluated
const block / named const at the const-generic call site, or
(b) accept a `mir::Const::Unevaluated` for an ADT-enum const
and route it through an alternate value-tree resolution that
yields the same `Const::EnumValue { variant_idx, payload }`
shape. Tests bring OP-003-I to PASS via runner re-attempt of
the `pub const V: E = E::A(7); W<V>` shape from
`.harness/expectations/regseq-driver-67.md:222-228`.

**Priority**: MEDIUM

**Rationale**: One row unblock; isolated to the const-eval
entry. Mid-leverage; below Cluster A in payoff but above Cluster
B2's probe study.

**Dependencies**: none.

**Estimated effort**: moderate bridge change (const-eval
surface extension + one sidecar re-attempt).

### Proposal: regseq-driver-NN -- Accept fn-pointer-provenance allocations (FnPtrAddrShim) (C2)

**Affected escalations**: FF-013 (F4)
(matrix row `docs/discussions/regseq-model/bridge-coverage-matrix.md:406`).

**Root cause**: Per
`.harness/expectations/regseq-driver-67.md:337-351`, the
`PartialEq::eq(&fp, &(target as fn()))` candidate (the only
shape that even reaches the FnPtrAddrShim materialization path)
rejects with `UnsupportedConst: allocation provenance over Function { instance: Item(target) }`.
The bridge's `translate_const_value` does not accept allocations
whose provenance points at a `Function` instance; the
auxiliary-instance scan that would otherwise materialize the
FnPtrAddrShim never runs because the rejection fires upstream.

**Proposed scope**:
Either (a) accept the `Function`-provenance allocation as a
`Const::Reloc(InstanceRef)` and let the auxiliary-instance scan
materialize the FnPtrAddrShim downstream, or (b) skip the
allocation in the const-translation scan and synthesize the
FnPtrAddrShim instance directly from the comparison-site
metadata. The decision belongs to the implementer based on
which path is more spec-faithful per spec.md section 9.7's
`Const::Reloc` semantics. Tests bring FF-013 to PASS via
runner re-attempt of the
`PartialEq::eq(&fp, &(target as fn()))` shape.

**Priority**: MEDIUM

**Rationale**: One row unblock; isolated to the
const-translation allocation-provenance path. Independent of
C1.

**Dependencies**: none.

**Estimated effort**: moderate bridge change.

## Cluster (D) -- Helper / build-contract gaps

Two F4 escalations and one external-dependency row trace to
build-flag / wiring gaps that are NOT bridge bugs per se. TM-006-C
and RJ-006 share a common fix shape (per-fixture overrides in the
test-ir-fixtures helper); they are merged into D1. RJ-032-F's
fix lives in the backend target-binding wiring (spec.md section
26.9), an independent phase; it is bundled into F1 with IX-014
since both are "deferred pending external work".

### Proposal: regseq-driver-NN -- Per-fixture build-flag override in test-ir-fixtures helper (D1)

**Affected escalations**: TM-006-C (F4), RJ-006 (F4)
(matrix rows
`docs/discussions/regseq-model/bridge-coverage-matrix.md:342`,
`:546`).

**Root cause**: The bridge fixture helper at
`crates/test-ir-fixtures/src/lib.rs:335-344` hard-wires
`overflow_checks = Some(false)` and uses the default
`mir_enable_passes = regseq_driver::regseq_mir_passes()`
without `-Z` flag overrides. As a result:
- TM-006-C: rustc's MIR builder does not insert
  `AssertKind::OverflowNeg` runtime checks (per
  `.harness/expectations/regseq-driver-67.md:264-278`); the
  `-x` source shape simply does not lower to the assertion
  variant.
- RJ-006: every MIR statement variant that
  `crates/regseq-driver/src/bridge/body/statement.rs:43-82`
  rejects (`FakeRead`, `AscribeUserType`, `PlaceMention`,
  `Nop`, `StorageLive`, `StorageDead`, `Retag`,
  `ConstEvalCounter`, etc.) is swept by required cleanup
  passes or only emitted under a `-Z` flag the helper does
  not pass (per
  `.harness/expectations/regseq-driver-67.md:353-373`).

**Proposed scope**:
Extend the test-ir-fixtures helper's build-config surface to
accept per-fixture overrides for codegen flags (at minimum,
`overflow_checks=true`) and `-Z` flags (at minimum,
`-Z mir-emit-retag` for the `Retag` statement, or another `-Z`
flag that exposes a rejection-eligible statement; the
implementer surveys
`crates/regseq-driver/src/bridge/body/statement.rs:43-82` to
pick the cheapest exposure). Once the override is wired, the
bridge-coverage-runner can author per-row build-config overrides
in `rows.toml` (or a sidecar metadata field) and re-attempt
TM-006-C and RJ-006 with the appropriate overrides. The
implementer should consult `.harness/expectations/regseq-driver-67.md:264-278`
and `:353-373` for the per-row expectations, and pick a
runner-side override surface that does not break the
"shared fixture compilation" invariant.

**Priority**: HIGH

**Rationale**: Single helper change unblocks two rows
(TM-006-C, RJ-006); the helper extension itself is a small
infrastructure change (a config-struct field plus the runner
wiring). Highest leverage per unit of helper change, and the
override surface is reusable for future rows that depend on
non-default rustc build flags.

**Dependencies**: none.

**Estimated effort**: small infrastructure change
(helper + runner config-surface extension + two sidecar
re-attempts).

## Cluster (F) -- Deferred pending external work (pointer-only proposals)

Two escalations have no immediate bridge-side fix because they
are gated on work that is either upstream (OPEN BUG family) or
in a separate phase (backend target-binding wiring). Both share
the "pointer-only proposal" shape called out in Non-Goal item 7
of the F68 task spec
(`.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:232-243`):
the proposal documents the watch-target and does NOT file
parallel investigation work. They are bundled into a single
proposal F1 because they share the same shape and produce no
implementation work.

### Proposal: regseq-driver-NN -- Deferred-pending-external-work watch list (IX-014, RJ-032-F) (F1)

**Affected escalations**: IX-014 (F2), RJ-032-F (F4)
(matrix rows
`docs/discussions/regseq-model/bridge-coverage-matrix.md:495`,
`:612`).

**Root cause**:
- IX-014: per `.harness/expectations/regseq-driver-65.md:223-246`,
  the two probed shapes for "closure consumed via FnOnce" both
  hit OPEN BUG family blockers -- Candidate A panics the bridge
  AND maps to `FunctionInstanceFlavor::Item` (not
  `ClosureOnceShim`); Candidate B hits the IX-004 / IX-010 OPEN
  BUG family. IX-014 unblocks once IX-004 / IX-010 are resolved
  upstream.
- RJ-032-F: per `.harness/expectations/regseq-driver-67.md:393-417`,
  the D8 diagnostic short-circuits to `Ok(())` because
  `target_str_arg_bound` at
  `crates/regseq-driver/src/bridge/diag_log/peel.rs:179-181` is
  hard-wired to `None`. D8 is currently unreachable from source;
  the row unblocks once the target-binding selection that
  populates `max_str_args_per_call` lands per spec.md section
  26.9 + the per-target `<target>-backend.md` artifacts.

**Proposed scope**:
This is a watch-list documentation task, NOT a bridge-fix task.
The implementer:
1. Annotates each row in
   `docs/discussions/regseq-model/bridge-coverage-matrix.md`
   with the external-dependency reason (IX-014 = "blocked on
   IX-004/IX-010 resolution"; RJ-032-F = "blocked on
   target-binding wiring per spec.md section 26.9").
2. Records a follow-up trigger condition so a future post-IX-004
   triage / post-target-binding-wiring triage knows to revisit
   both rows.
3. Does NOT file parallel investigation work for IX-004,
   IX-010, or the target-binding wiring -- those are separate
   approval gates owned by their own phases.

**Priority**: LOW

**Rationale**: No immediate implementation; the rows remain
matrix-uncovered until external work lands. Annotation gives
future triage a definitive watch-target list, which is the
cheapest unblock the F68 cluster set can offer for these rows.

**Dependencies**: IX-014 depends (externally) on resolution of
IX-004 / IX-010 per the OPEN BUG family in the matrix. RJ-032-F
depends (externally) on backend target-binding wiring per
spec.md section 26.9 and the corresponding
`<target>-backend.md` artifact. Both are out of scope for any
near-term sidecar-coverage task.

**Estimated effort**: documentation-only.

## Closing recommendations

The recommended priority order for the user to approve reflects
(i) leverage per unit of implementer time and (ii) the absence
of internal dependency edges between proposals (no proposal
unblocks another; every proposal is independent).

| Order | Proposal | Affected escalations | Priority | Effort | Recommended action |
| ----- | -------- | -------------------- | -------- | ------ | ------------------ |
| 1 | D1 | TM-006-C, RJ-006 | HIGH | small infrastructure | Approve. Smallest unblock per row count; reusable override surface. |
| 2 | A1 | RV-007-C, RV-009-G, FF-005 | HIGH | moderate bridge | Approve. Single normalization extension clears 3 rows. |
| 3 | C1 | OP-003-I | MEDIUM | moderate bridge | Approve. Isolated const-eval surface extension. |
| 4 | C2 | FF-013 | MEDIUM | moderate bridge | Approve. Independent of C1; isolated allocation-provenance fix. |
| 5 | B1 | FF-012 | MEDIUM | small-to-moderate bridge | Approve. One-row unblock; could trail behind C1/C2 if the implementer prefers a single const-surface sweep. |
| 6 | B2 | FF-004, RJ-024 | LOW | documentation-or-small | Defer or approve at the user's discretion. Probe + decision; uncertain payoff. |
| 7 | F1 | IX-014, RJ-032-F | LOW | documentation-only | Approve as a low-cost annotation task; no implementation risk. |

**Priority + dependency consistency check** (Test Plan B4):
No proposal lists any other proposal as a dependency; the
dependency graph is empty and therefore acyclic. Priority
ordering is monotonic by leverage (HIGH > MEDIUM > LOW). The
two LOW-priority proposals are tail items, neither blocks any
higher-priority work.

**Coverage check** (Acceptance Criterion 2): the proposal set
covers all 12 active escalations:
A1 (3) + B1 (1) + B2 (2) + C1 (1) + C2 (1) + D1 (2) + F1 (2)
= 12 unique escalations.

**Out-of-scope discipline check** (Acceptance Criterion 6):
no proposal targets the F4-documented matrix blocks. Matrix
blocks remain uncovered as intentional; the proposal set does
not relitigate them.

**No auto-filing reminder**
(Acceptance Criterion 5 and F68 task spec Non-Goals item 1):
F68's `task.py done` will register only the F68 closure entry
in `tasks.json`. No `task.py add-task` invocations have
occurred for the proposed `regseq-driver-NN` tasks; each
approved proposal becomes its own task-spec-drafting session
in a follow-up invocation. The user reviews this document,
approves a subset, and the follow-up session files each
approved proposal via `task.py add-task`.
