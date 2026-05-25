# regseq-driver-78 — Section 7 validator-invariant coverage audit (Category 4, Type-B analytical)

## 1. Description

`bridge-coverage-matrix.md` **Section 7** lists the `InvariantViolation`
variants — the IR invariants that `RegseqModel::validate()` /
`validate_bridge_completeness()` enforce — with a per-variant column
"exercised in `id_referential_integrity_validator.rs`?". Section 7 marks
**VL-001..VL-011 `yes`** and **VL-012..VL-060 (49 rows) `unknown -- verify`**.
The matrix's own follow-up note (matrix `:738-742`) is explicit: cross-check
each `unknown -- verify` cell against the validator test files and mark it
`yes` / `partial` / `no`; variants that end up `no` become fixture-batch
follow-up tasks (one constructed-model test per uncovered variant).

This task is that audit. It is **analytical (Type-B), no source code changes**.
The deliverable is an audit document plus the resulting Section 7 doc update.

**Two facts established at HEAD that this spec is grounded on (the implementer
must re-confirm, not trust):**

1. **The enum has 61 variants; Section 7 lists 60 — the matrix is stale.**
   `InvariantViolation` at `crates/regseq-model/src/model.rs:163-831` now has
   **61** variants. Section 7 has **60** rows (VL-001..VL-060). The missing
   variant is **`DuplicateInstanceKey`** (enum variant #12, `model.rs:269`),
   added by the R1/driver-73 assessor hardening (commit `e89e02d`) AFTER
   Section 7 was authored. Consequence: Section 7's VL-012..VL-060 are
   **off-by-one** from the enum — matrix VL-012 (`DefPathRenderMismatch`) is
   enum variant #13, etc. The audit must (a) add `DuplicateInstanceKey` and
   (b) restore enum-order correspondence (see Change B).

2. **Coverage is spread across FIVE validator test files, not the one Section 7
   names.** Section 7's header only references
   `id_referential_integrity_validator.rs`, but the validator suite is:
   `id_referential_integrity_validator.rs` (57 tests), `layout_validator.rs`
   (63), `place_validator.rs` (19), `validate_bridge_completeness_buckets.rs`
   (13), `validate_item_instance_coverage.rs` (5). A first-pass grep
   (variant-name → file, see §3) shows e.g. all the `*Malformed` /
   layout / instance-layout variants live in `layout_validator.rs`, the
   bucket/completeness variants in `validate_bridge_completeness_buckets.rs`,
   `PlaceWalk` in `place_validator.rs`. The audit must search **all five**
   (plus any `e2e_*` siblings) per variant.

These validator-failing fixtures are **constructed-model tests** (build a
`RegseqModel` with a deliberate defect, assert `validate()` returns the
specific variant) — a **different surface** from the bridge-coverage-runner
sidecar (which runs Rust source snippets through the bridge). This audit does
**not** touch the runner sidecar; it audits `regseq-model/tests/`.

## 2. Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-matrix.md:663-742` — Section 7
  table (60 rows) + the follow-up note (`:738-742`) defining this task's
  mandate (mark `yes`/`no`/`partial`; `no` → fixture-batch).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md:765-770` — Section 8
  Q6 frames the cadence question this task answers: "one task (audit
  validator-test coverage and update Section 7)" vs. rolled into each batch.
  This task takes the **one-task** option.
- HANDOFF "§ Current state" Next-actions #2 — Category 4, "~49 unverified
  `InvariantViolation` invariants; likely a Type-B analytical/triage task
  first, then per-invariant coverage rows."

### The variant registry (modified-by-doc-edit target)

- `crates/regseq-model/src/model.rs:163-831` — `pub enum InvariantViolation`,
  **61 variants** (read-only; the audit cites enforcement anchors but does NOT
  modify the enum). Variant-name → enforcing method map is built in §3.

### Validator enforcement surface (consumed read-only)

`RegseqModel::validate()` at `model.rs:1665` dispatches to ~33 `validate_*`
helpers; `validate_bridge_completeness()` at `model.rs:3592` enforces the
bucket invariants. The per-variant enforcing method is part of the deliverable;
spot-checked anchors confirming the zero-reference variants are LIVE (not dead):

- `SyntheticKindItemKindMismatch` → `validate_synthetic_kinds`, returned at
  `model.rs:2379`.
- `DiagLogCallArityBelowMinimum` / `DiagLogCallArg0NotStrRef` /
  `DiagLogCallArgNotAdmissible` → `validate_log_intrinsic_call`, returned at
  `model.rs:3178 / 3186,3194 / 3213,3222`.
- `SynthesizedSentinelInvalid` → `validate_synthesized_sentinel`
  (`model.rs:2047`); `StringInternerSentinelInvalid` / `DuplicateString` →
  `validate_string_interner` (`model.rs:2321`).

### Validator test files (consumed read-only — the cross-check corpus)

- `crates/regseq-model/tests/id_referential_integrity_validator.rs`
- `crates/regseq-model/tests/layout_validator.rs`
- `crates/regseq-model/tests/place_validator.rs`
- `crates/regseq-model/tests/validate_bridge_completeness_buckets.rs`
- `crates/regseq-model/tests/validate_item_instance_coverage.rs`
- Also scan the `e2e_*` siblings if a variant is referenced nowhere in the five
  above (e.g. an invariant might be exercised incidentally by a round-trip test).

### Prior tasks

- regseq-driver-73 (R1) — added `DuplicateInstanceKey` + `validate_unique_instance_keys`
  (`model.rs:3361`) and 2 bucket tests in `validate_bridge_completeness_buckets.rs`;
  this is the variant Section 7 is missing.
- regseq-driver-68 (F5) — the prior Type-B analytical/triage task; the
  assessment template for this one (`.harness/audits/2026-05-21-post-f4-triage.md`).

### Fact established for the renumber decision

- `grep -rn "VL-0[0-9][0-9]"` over the whole repo (`--include=*.rs --include=*.md`)
  finds VL-ids only inside `bridge-coverage-matrix.md` and, descriptively, in
  HANDOFF / task-spec prose (this spec itself names "VL-001..VL-011" etc.).
  Crucially, **no source code, no test, and no other doc binds to a specific
  VL-id value as a load-bearing cross-reference** — the task-spec mentions are
  narrative, not anchors. So renumbering Section 7 rows breaks nothing
  executable or load-bearing (Change B).

## 3. Audit Method (the per-variant procedure)

Produce the deliverable at
**`.harness/audits/2026-05-25-section7-validator-audit.md`**. For **all 61**
variants (the 11 `yes` get a quick re-confirm; the 49 `unknown` + the 1 missing
`DuplicateInstanceKey` get the full treatment):

### Step 1 — build the variant → enforcing-method map

For each variant, locate where `validate*` *returns* it (`grep -n
"InvariantViolation::<Variant>" crates/regseq-model/src/model.rs`, the
`Err(... { ... })` construction site, not the enum definition). Record the
enforcing method name + `model.rs:line`. A variant that is **never constructed
in `model.rs`** is a genuinely dead variant — flag it explicitly (do not assume;
verify by grep).

### Step 2 — build the variant → exercising-test map

For each variant, search all five validator test files (+ `e2e_*` if needed)
for a **genuine exercising test**: a `#[test]` that (a) constructs/mutates a
`RegseqModel` to inject the specific defect, (b) calls `validate()` or
`validate_bridge_completeness()`, and (c) asserts the result is **that specific**
`InvariantViolation` variant (`assert!(matches!(.., InvariantViolation::X { .. }))`
or equivalent). **A grep hit is NOT sufficient** — the name may appear in a
helper's match arm, an error-message mapping, a doc comment, or a different
variant's negative-control. Open each hit and confirm it is an asserting test.
Record `file:test_fn_name` for every exercising test (a variant may have
several).

First-pass grep map (variant → file(s) referencing the name — **starting point,
must be confirmed per Step 2, not taken as the answer**):

```
DuplicateInstanceKey                 validate_bridge_completeness_buckets.rs
DefPathRenderMismatch                id_referential_integrity_validator.rs
SpanOutOfBounds                      id_referential_integrity_validator.rs
SpanNotOnUtf8Boundary                id_referential_integrity_validator.rs
DuplicateVTablePrincipal             id_referential_integrity_validator.rs
SynthesizedSentinelInvalid           <<< NONE >>>
StringInternerSentinelInvalid        <<< NONE >>>
DuplicateString                      <<< NONE >>>
PlaceWalk                            id_referential_integrity_validator.rs place_validator.rs
SyntheticKindItemKindMismatch        <<< NONE >>>
DirectCalleeOutOfRange               place_validator.rs id_referential_integrity_validator.rs
TargetMalformed                      layout_validator.rs
BuildMalformed                       layout_validator.rs
FnSigMalformed                       layout_validator.rs
CAbiLoweringMalformed                layout_validator.rs
LayoutMalformed                      layout_validator.rs
TagScalarOutOfRange                  layout_validator.rs
InstanceLayoutMismatch               layout_validator.rs
InstanceLayoutItemArgsMismatch       layout_validator.rs
ConstGraphCycle                      layout_validator.rs
DomainAttrMalformed                  layout_validator.rs
OpaqueHandleInconsistent             layout_validator.rs
StableCrateIdZeroCollision           layout_validator.rs
SyntheticDomainBoundary              layout_validator.rs
UnitLayoutMismatch                   layout_validator.rs
SourceFileMalformed                  layout_validator.rs
DuplicateSourceFilePath              layout_validator.rs
BodyTyIdOutOfRange                   id_referential_integrity_validator.rs
StatementAssignTypeMismatch          id_referential_integrity_validator.rs
StatementAssignMalformed             id_referential_integrity_validator.rs
SetDiscriminantVariantIdxOutOfRange  id_referential_integrity_validator.rs
SetDiscriminantNonEnumTarget         id_referential_integrity_validator.rs
AssumeOperandNotBool                 id_referential_integrity_validator.rs
CopyNonOverlappingOperandType        id_referential_integrity_validator.rs
SwitchIntDiscrTypeMismatch           id_referential_integrity_validator.rs
AssertCondNotBool                    id_referential_integrity_validator.rs
CallDestinationTypeMismatch          id_referential_integrity_validator.rs
CallArgsArityMismatch                id_referential_integrity_validator.rs
CallDirectCalleeNotFunction          id_referential_integrity_validator.rs
CallIndirectCalleeNotFnPtr           id_referential_integrity_validator.rs
CallArgTypeMismatch                  id_referential_integrity_validator.rs
CallVirtualIdxInconsistent           id_referential_integrity_validator.rs
CallTargetDivergenceMismatch         id_referential_integrity_validator.rs
DiagLogCallArg0NotStrRef             <<< NONE >>>
DiagLogCallArityBelowMinimum         <<< NONE >>>
DiagLogCallArgNotAdmissible          <<< NONE >>>
ItemMissingInstance                  validate_bridge_completeness_buckets.rs validate_item_instance_coverage.rs
Bucket1MultipleInstances             validate_bridge_completeness_buckets.rs
ForeignInstanceBodyOrInitNotNone     validate_bridge_completeness_buckets.rs
Bucket4HasInstance                   validate_bridge_completeness_buckets.rs
```

The **7 zero-reference variants** (`SynthesizedSentinelInvalid`,
`StringInternerSentinelInvalid`, `DuplicateString`, `SyntheticKindItemKindMismatch`,
`DiagLogCallArg0NotStrRef`, `DiagLogCallArityBelowMinimum`,
`DiagLogCallArgNotAdmissible`) are the strongest `no`/uncovered candidates — all
are confirmed LIVE (enforced in `validate*`, §2). Confirm there is genuinely no
exercising test anywhere, then classify them `no`.

### Step 3 — classify coverage (axis 1)

Each variant gets exactly one:
- **`yes`** — at least one genuine exercising test (construct → validate →
  assert this variant).
- **`partial`** — the variant is *touched* (e.g. a test trips it as a
  by-product, or only one of several enforcement sub-paths/fields is asserted)
  but not directly/completely exercised. Define precisely in the doc what is
  vs. isn't covered.
- **`no`** — no exercising test.

### Step 4 — for every `no` (and `partial`), triage how to cover it (axis 2)

Record one fixture-form per gap:
- **`constructed-model`** — author a `RegseqModel` with the precise defect and
  assert the variant. (Expected default for nearly all integrity/layout/bucket
  invariants — these are not bridge-producible from ordinary source.)
- **`source-snippet`** — a real Rust source through the bridge would naturally
  trip it (flag if so; rare — would indicate the bridge can emit an invalid
  model, itself noteworthy).
- **`unreachable-by-construction`** — the variant cannot be triggered even by a
  hand-built model because an earlier validator check shadows it, or the
  enclosing type/struct shape makes the defect unrepresentable. Justify with a
  citation. (These do NOT become fixture tasks; they are flagged for a possible
  dead-variant follow-up.)

For each `constructed-model` gap, sketch the **defect to inject** in one line
(e.g. "VL-xxx `SetDiscriminantNonEnumTarget`: a `SetDiscriminant` statement
whose place type is a `Ty::Struct`, expect `validate()` → that variant"). This
sketch is the seed for the follow-up fixture task — it is NOT authored here.

### Step 5 — summary roll-up

- Count: `yes` / `partial` / `no` / `unreachable` / `dead`, over all 61.
- The ordered list of `no` variants that become fixture-batch follow-up
  candidates, grouped by enforcing method / test file (so the follow-up can be
  decomposed into per-file sub-batches, matching Section 8 Q7's "break into
  sub-batches by topic" guidance).
- Note any **second-order findings** (e.g. a variant never constructed in
  `model.rs` = dead; a Section 7 cell already marked `yes` that the audit finds
  is actually `no`/`partial`; the bridge able to emit an invalid model).

## 4. Non-Goals

- **Do NOT author the missing fixtures.** Filling the `no` gaps with
  constructed-model tests is the follow-up batch (one or more later tasks),
  decomposed from this audit's Step-5 roll-up. This task only classifies + sketches.
- **Do NOT modify `model.rs`, the `InvariantViolation` enum, any `validate_*`
  method, or any validator/bridge source.** Read-only over all source.
- **Do NOT touch the bridge-coverage-runner sidecar** (`rows.toml`, `snippets/`,
  runner crate). Section 7 is a constructed-model test surface, not a runner row
  surface; this audit does not add or move any sidecar row, and does not run
  Mode A as a deliverable (it may be run only as a no-regression sanity check).
- **Do NOT resolve the KNOWN matrix↔sidecar drift** (Section 1-6 status column);
  that is a separate Category 5 item. This task touches only Section 7.
- No new dependencies, no new tooling scripts. (If a throwaway grep/awk helps,
  fine, but it is not a committed deliverable.)

## 5. Deliverable structure + verification

### Deliverable A — audit document `.harness/audits/2026-05-25-section7-validator-audit.md`

A per-variant table covering **all 61** variants, in enum order:

| VL-id | Variant | Enforcing method (`model.rs:line`) | Coverage | Exercising test(s) `file:fn` | If `no`/`partial`: fixture-form + 1-line defect sketch |

Followed by the Step-5 summary roll-up (counts; the grouped `no` follow-up
list; second-order findings).

### Deliverable B — Section 7 doc update (`bridge-coverage-matrix.md`)

- **Add the missing `DuplicateInstanceKey` row** and **restore enum-order
  correspondence** by renumbering: insert `DuplicateInstanceKey` at its enum
  position (between `F4OutOfOrderInstance` and `DefPathRenderMismatch`) as the
  new VL-012, and renumber the existing VL-012..VL-060 → VL-013..VL-061. This is
  safe — §2 established that no source/test/other-doc binds to a VL-id value as a
  load-bearing cross-reference (only the matrix + narrative spec/HANDOFF prose
  mention them) — and restores the matrix's stated invariant that ids are
  "sourced via `awk` against the enum
  block" (matrix `:666`). **(If Codex prefers append-as-VL-061-with-a-note over
  renumber to minimise churn, that alternative is acceptable; the renumber is
  the recommendation, not a hard mandate — the hard requirement is that all 61
  variants appear exactly once with a verified classification.)**
- **Flip every `unknown -- verify` cell** to the audit's verified `yes` /
  `partial` / `no`, with the `notes` column carrying the exercising test
  (for `yes`/`partial`) or the fixture-form + cross-reference to the audit doc
  (for `no`).
- **Re-confirm the existing `yes` rows** (VL-001..VL-011); if any is found to be
  `no`/`partial`, correct it and flag it as a second-order finding in the audit
  doc.
- Update the Section 7 header prose (`:665-673`) to reflect 61 variants and the
  five-test-file corpus.

### Verification (how the assessor checks completeness)

- **Count parity**: the audit table has exactly 61 rows = the enum's variant
  count (`awk` over `model.rs:163-831`); Section 7 likewise has 61 rows after
  Change B.
- **Every variant accounted for**: set of audit-table variant names ==
  set of enum variant names (no omissions, no inventions).
- **Every `yes`/`partial` cites a real test**: spot-check ≥6 of the cited
  `file:fn` tests actually exist, construct the defect, and assert the variant.
- **Every `no` is genuinely uncovered**: spot-check that the 7 zero-reference
  candidates (+ any others classified `no`) have no exercising test in any of
  the five files.
- **Coherence**: each `no`/`partial` carries a fixture-form + defect sketch;
  each `unreachable`/`dead` carries a justification citation.

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/78-section7-validator-audit.md` (this spec) — at
  spec-review time.
- At assessment time: the deliverable audit doc + the Section 7 diff, plus
  `model.rs` (enum + the `validate_*` methods) and the five validator test files
  for cross-checking.

### Reviewer focus

1. Does the audit cover **all 61** enum variants (incl. `DuplicateInstanceKey`),
   in enum order, with no omissions or invented variants?
2. Is every `yes`/`partial` backed by a **genuine exercising test** (construct →
   validate → assert the specific variant), not just a grep hit?
3. Are the 7 zero-reference variants (and any other `no`) correctly confirmed as
   uncovered, with a sound fixture-form (constructed-model unless justified
   otherwise) and a defect sketch?
4. Is each `unreachable-by-construction` / dead-variant claim justified by a
   citation (not asserted)?
5. Section 7 doc: is the renumber correct and complete (61 rows, enum order),
   and are no VL-id cross-references broken (there are none, per §2)?
6. Scope: deliverable confined to the audit doc + Section 7 of the matrix +
   tasks.json status flip? No source edits, no sidecar/runner edits, no other
   matrix sections touched?

### Adversarial prompt template (four-block)

```
<task>
Assess the regseq-driver-78 deliverable (cwd = repo root): an audit of Section 7
validator-invariant coverage. The deliverable is .harness/audits/2026-05-25-section7-validator-audit.md
plus an updated Section 7 in docs/discussions/regseq-model/bridge-coverage-matrix.md. The audit
classifies all 61 InvariantViolation variants (model.rs:163-831) as yes/partial/no by
cross-checking the five validator test files in crates/regseq-model/tests/ for a genuine
exercising test (construct a defective RegseqModel -> validate() -> assert the specific variant).
A grep hit on the variant name is NOT a pass; open each cited test and confirm it asserts that
variant. Verify all 61 variants appear once, DuplicateInstanceKey (the variant Section 7 was
missing) is present, the renumber is correct, and every `no` has a sound fixture-form + defect
sketch. You may build/run cargo test -p regseq-model and grep the tree.
</task>
<grounding_rules>
Cite file:line for every claim. Open the cited tests; do not accept a grep hit as coverage.
Mark hypotheses as hypotheses.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ALL_61_COVERED: PASS|FAIL + cite
YES_PARTIAL_BACKED_BY_REAL_TEST: PASS|FAIL + cite (name any miscategorised row)
NO_ROWS_GENUINELY_UNCOVERED: PASS|FAIL + cite
RENUMBER_CORRECT: PASS|FAIL + cite
SCOPE_CONFINED: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo test -p regseq-model -- --test-threads=1` green (sanity only; no code
  changes expected — confirms the audited tests pass at HEAD).
- **No source modifications**: `git diff --stat` touches ONLY
  `.harness/audits/2026-05-25-section7-validator-audit.md` (new),
  `docs/discussions/regseq-model/bridge-coverage-matrix.md` (Section 7 only),
  and `.harness/tasks.json` (this task's status flip). No `.rs`, no
  `rows.toml`, no `snippets/`, no other matrix section.
- Audit table row count == 61 == enum variant count.
- (No clippy/build gate beyond the test sanity — this is a doc-only task.)

## 8. Acceptance Criteria

1. `.harness/audits/2026-05-25-section7-validator-audit.md` exists with a
   per-variant table covering **all 61** `InvariantViolation` variants in enum
   order, each with: enforcing method (`model.rs:line`), coverage
   (`yes`/`partial`/`no`), exercising test(s) `file:fn` (for `yes`/`partial`),
   and fixture-form + defect sketch (for `no`/`partial`).
2. Every `yes`/`partial` classification is backed by a genuine exercising test
   (construct → validate → assert the specific variant), confirmed by opening
   the test — not by a grep hit alone.
3. The 7 zero-reference variants are confirmed uncovered (or reclassified with
   justification if a hidden exercising test is found), and every `no` carries a
   fixture-form + one-line defect sketch.
4. Any `unreachable-by-construction` or dead (never-constructed-in-`model.rs`)
   variant is flagged with a justification citation.
5. Section 7 of `bridge-coverage-matrix.md` is updated: `DuplicateInstanceKey`
   added, enum-order correspondence restored (renumber, or append-with-note if
   Codex prefers), all 61 variants present exactly once, every cell carries a
   verified `yes`/`partial`/`no` (no remaining `unknown -- verify`), header prose
   updated for 61 variants + the five-file corpus.
6. Step-5 summary roll-up present: counts by class + the grouped `no`
   follow-up list (by test file / enforcing method) + second-order findings.
7. No source/runner/sidecar changes; diff confined per the Quality Gate;
   `cargo test -p regseq-model` green.
