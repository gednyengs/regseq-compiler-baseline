# Gap inventory beyond F68 -- 2026-05-22

This document complements `.harness/audits/2026-05-21-post-f4-triage.md`
(F68 post-F4 triage) by enumerating gap categories that the F68
audit's seven proposals (A1, B1, B2, C1, C2, D1, F1) do NOT cover.
The F68 audit's scope is the twelve active escalations surfaced by
the F4 corpus sweep; this document characterizes the strategic
landscape OUTSIDE that escalation queue, so future task-drafting
sessions can plan beyond the F68 slate.

This document was Codex-reviewed against the matrix doc, the
F4 / F2 / F3 expectations, the strategy doc, the F68 audit, and
the bridge / runner source at HEAD before being persisted.

## Summary

| Field | Value |
| ----- | ----- |
| Matrix total row IDs (HEAD) | 359 (299 non-validator + 60 validator) |
| Sidecar rows at F69 close (`660bc7f`) | 125 |
| Matrix IDs missing from sidecar (subset-OK per strategy §4.1) | 234 |
| F68 audit slate status | D1 closed (driver-69 at `660bc7f`); A1 filed (driver-70 at `861177f`); B1 / B2 / C1 / C2 / F1 still PROPOSED, pending user approval |
| New categories enumerated below | 6 |
| Items recommended for follow-up filing beyond F68 | 5 (with priority tiers; see §Closing) |

### Quantitative baseline (Codex-corrected)

The matrix-row-ID count derives from the same parser rule the
runner uses: "extract every `ID` token that appears as the
leftmost cell of a markdown table row"
(`crates/bridge-coverage-runner/src/matrix.rs:276-281`).
Applied to the matrix doc at HEAD, the rule produces
**359 unique row IDs** total, of which 60 are Section 7
validator rows. The matrix doc's own audit note at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:724-726`
confirms this count ("~359 total table rows ... (299 non-VL,
excluding the 60 validator-invariant rows)").

The non-validator status taxonomy at HEAD:

| Status | Non-validator rows |
| ------ | ------------------ |
| `covered` | 102 |
| `partial` | 24 |
| `uncovered` | 157 |
| `blocked-bridge-rejects-by-design` | 7 |
| `blocked-open-bridge-bug` | 4 |
| `blocked-rustc-infeasible` | 4 |
| `blocked-generator-gap` | 1 |
| Section 7 validator rows totaled separately below | (60) |
| Total non-validator | 299 (= 102 + 24 + 157 + 7 + 4 + 4 + 1, matching the matrix doc's own audit note at `:724-726`) |

(There is no `blocked-spec-gap` row in the matrix at HEAD; the
strategy-doc filter category exists in the F4 task spec's
documented-blocks classification at
`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:84-85`
but no matrix row currently carries that status. The category
is retained in the filter taxonomy for future use.)

Section 7 (validator invariants): 60 rows, 11 marked `yes`, 49
marked `unknown -- verify` at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:641-689`.

The HANDOFF's "~445-row matrix" mention is **stale**; the
matrix doc's own audit note at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:724-730`
gives 359 as the current HEAD count.

The 234 matrix IDs missing from the F69-close sidecar decompose
as follows (Codex-derived; cross-checked against the runner's
drift output):

| Missing-row bucket | Count |
| ------------------ | ----- |
| Already matrix-`covered` parent rows (sidecar covers via IX-aliasing) | 96 |
| Matrix-`partial` rows (not committed as standalone sidecar rows) | 20 |
| Section 7 validator rows | 60 |
| Blocked rows (rustc-infeasible / bridge-by-design / open-bridge-bug / generator-gap / spec-gap) | 15 |
| **Actually `uncovered` and unauthored as sidecar rows** | **43** |

The four buckets sum to 96 + 20 + 60 + 15 + 43 = **234**, which
matches the F69-close drift count exactly.

The **43 actually-uncovered unauthored rows** is the pre-exclusion
figure; see Category 3 below for the post-F68-and-Category-1
exclusion count (27).

## Category 1 -- Matrix-doc OPEN BUG prose refresh + closure-family reattempts

**Affected matrix rows** (matrix annotated `blocked-open-bridge-bug`
or downstream-blocked at HEAD):

- `IX-004` -- closure × `Fn::call` site translation
  (matrix `:485`).
- `IX-010` -- generic fn × trait-method resolve (matrix `:491`).
- `BD-007` -- body materialised for synthetic ClosureCall
  Instance (matrix `:420`).
- `FT-011` -- closure with zero captures, called by ref
  (matrix `:441-444`).
- `FT-012` / `FT-013` / `FT-014` -- closure features
  (matrix; rows blocked by IX-004/IX-010 family).
- `SK-003` -- partial `SyntheticItemKind::ClosureCall` row
  (matrix `:379`).
- `SK-004` / `SK-005` -- FnMut / FnOnce closure-invocation
  rows (matrix; blocked by IX-004/IX-010).
- `FF-007` -- ClosureOnceShim (matrix; blocked by IX-014
  family).
- `RJ-009` -- partial `NonRigidTyKind::Param` row (matrix
  `:554`).
- `IX-014` -- closure consumed via FnOnce (matrix; F2
  escalation; F68's F1 proposal carries the pointer-only
  policy).

**Root cause (corrected from the user's pre-Codex analysis)**:
The matrix doc's "Notes on the open bug" prose at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:508-523`
**is stale at HEAD**. It claims that the Phase-2 synthesis
pre-scan at `crates/regseq-driver/src/bridge/mod.rs:769` walks
`tcx.optimized_mir(local_def_id)` (polymorphic body) and tries
to resolve the polymorphic `<F as Fn>::call` callee via
`Instance::try_resolve` at `mod.rs:1039`. In reality, **the
prescan args-substitution bug was already fixed by
regseq-driver-62**: HEAD's `bridge/mod.rs:769-789` calls
`instance_mir(instance.def)` plus
`instantiate_mir_and_normalize_erasing_regions(...)`, per the
driver-62 expectations at
`.harness/expectations/regseq-driver-62.md:51-68` and the
driver-62 task spec at
`.harness/tasks/regseq-driver/62-prescan-args-substitution.md:48-71`.

The "same shape as driver-60" historical claim in the matrix is
accurate for the pre-fix bug but should NOT be read as the
current bridge state.

**Empirical implication**: the downstream rows above are
NOT blocked by an open bridge bug. They are **stale-matrix
annotations + reattempt candidates** that may PASS against
HEAD's bridge given an appropriate snippet shape and an F3
predicate. The work is:

1. **Matrix-doc refresh**: rewrite the "Notes on the open bug
   (IX-004 / IX-010)" section at `:508-523` to reflect the
   regseq-driver-62 fix and re-classify the blocked rows.
2. **Reattempt sweep**: author snippets + sidecar entries for
   the closure-family rows (IX-004, IX-010, BD-007, FT-011/012/
   013/014, SK-003 [as committed not partial], SK-004, SK-005,
   FF-007, IX-014, RJ-009 [as committed not partial]). The
   per-row 3-retry budget per the F4 authoring protocol
   applies; rows that fail all 3 retries escalate.

**Proposed scope**: a single follow-up task that combines (1)
matrix-doc refresh + (2) closure-family reattempt sweep. The
matrix-refresh portion is mechanical; the reattempt portion is
authoring-shaped per the F4 + F69 pattern.

**Priority**: **MEDIUM**. Lower than F68's HIGH bridge fixes
(A1 / D1) because the bridge work is already done; what remains
is documentation + authoring. Higher than F5 automation because
it closes a real coverage gap.

**Effort**: documentation pass (small) + 13-row authoring batch
(moderate; bounded by the closure-family scope, no per-row
bridge changes expected). The 13-row cohort enumerated above is
IX-004, IX-010, BD-007, FT-011, FT-012, FT-013, FT-014, SK-003
(partial → committed), SK-004, SK-005, FF-007, RJ-009 (partial
→ committed), IX-014.

**Dependencies**: independent of F68's A1/B1/B2/C1/C2; can run
in parallel.

**Out-of-scope discipline note**: F68's F1 proposal is a
pointer-only "watch-list" for IX-014 + RJ-032-F per
`.harness/audits/2026-05-21-post-f4-triage.md:430-495`. F1's
non-goal at the F68 task spec
(`.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:232-243`)
explicitly forbids parallel IX-004 / IX-010 investigation. The
Category 1 reattempt sweep proposed here does NOT relitigate
F1's pointer-only framing for IX-014; it operates on the
RESOLVED-by-driver-62 bridge state and treats IX-014 as one
member of the reattempt cohort, with the understanding that
the F1 spec-policy framing might still apply per the F68 audit's
revised wording at
`.harness/audits/2026-05-21-post-f4-triage.md:430-495`.

## Category 2 -- FF-002 likely matrix false-positive

**Affected matrix row**: `FF-002` -- `FunctionInstanceFlavor::Intrinsic`
(matrix `:390-396`), marked `covered` via `diag_logging.rs`.

**Root cause**: per F2 IX-017 evidence at
`.harness/expectations/regseq-driver-65.md:248-268`, the diag-log
peel pass leaves the log function as
`FunctionInstanceFlavor::Item` (NOT `Intrinsic`), with the
distinguishing diag-log signal living in
`AttrData::Intrinsic(IntrinsicKind::Log(_))` inside
`model.domain_attrs.item_attrs`. F3 unblocked IX-017 via
the `BodyAbsent` predicate (per
`.harness/expectations/regseq-driver-66.md:287-312`), NOT via
an Intrinsic-flavor predicate. The bridge's mapping from
rustc's `InstanceKind::Intrinsic` to model
`FunctionInstanceFlavor::Intrinsic` lives at
`crates/regseq-driver/src/bridge/mod.rs:2504-2513`; the peel
pass keys off `AttrData::Intrinsic` per
`crates/regseq-driver/src/bridge/diag_log/peel.rs:1-16`.

**Empirical implication**: the matrix's claim that FF-002 is
`covered` via `diag_logging.rs` is **likely a false positive**.
`diag_logging.rs` may contain an un-peeled intrinsic call that
DOES surface `FunctionInstanceFlavor::Intrinsic`, in which case
FF-002 is correctly marked. But the post-peel diag-log functions
the matrix points at do NOT carry the `Intrinsic` flavor.

**Proposed scope**: a one-pass audit of `diag_logging.rs`'s
bridge output against the `FF-002=Intrinsic` predicate. If the
predicate matches some non-peeled production in the fixture, the
matrix is correct; document the matching path. If not, re-classify
FF-002 as `uncovered` and add it to the next discovery batch (or
fold into Category 1's reattempt sweep, since FF-002 is in the
FFlavor cluster).

**Priority**: **LOW**. One-row precision-audit; the matrix's
covered claim doesn't block any other work.

**Effort**: documentation-only (audit) OR
documentation-only + one snippet reattempt if FF-002 turns out to
be uncovered.

**Dependencies**: none.

## Category 3 -- F7 discovery batch over 27 post-exclusion actually-uncovered rows

**Affected rows**: per the Codex-derived sidecar/matrix
reconciliation, **43 matrix IDs** are marked `uncovered` AND
missing from the F69-close sidecar. After excluding F68's A1-F1
proposal target set and Category 1's closure-family reattempt
cohort, **27 rows** remain as the genuine "F4 didn't get to
them, no other triage covers them" discovery remainders.

The 27-row set is NOT enumerated row-by-row in this audit; the
implementer of the eventual F7 task spec derives the list from
the matrix doc + the F69-close `rows.toml` + the F68 audit's
target set + Category 1's cohort. The selection criterion is:
- Matrix status = `uncovered`.
- Row ID NOT present in F69-close `rows.toml`.
- Row ID NOT in the F68 audit's per-row target set at
  `.harness/audits/2026-05-21-post-f4-triage.md:28-39`
  (A1 = RV-007-C / RV-009-G / FF-005; B1 = FF-012; B2 = FF-004 +
  RJ-024; C1 = OP-003-I; C2 = FF-013; D1 = TM-006-C + RJ-006
  [closed]; F1 = IX-014 + RJ-032-F).
- Row ID NOT in Category 1's closure-family reattempt cohort
  (IX-004, IX-010, BD-007, FT-011, FT-012, FT-013, FT-014,
  SK-003 partial, SK-004, SK-005, FF-007, RJ-009 partial, IX-014).

**TM-006-F/G/H runtime-assert source-repro gap** is included
in the 27-row post-exclusion set above (NOT in addition to it).
Per the F4 task spec at
`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:360-364`,
TM-006-F (`AssertKind::MisalignedPointerDereference`),
TM-006-G (`AssertKind::NullPointerDereference`), and
TM-006-H (`AssertKind::InvalidEnumConstruction`) were
"escalation-by-default" rows in F4 because no clean source
repro was known under the build contract; per the matrix at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:345-347`
they remain `uncovered`. None of the three IDs appear in F68's
target set or Category 1's closure cohort, so all three pass
through the exclusion filter and land in the 27-row set. An F7
implementer attempts these as part of the 27-row sweep; if no
source repro surfaces within the 3-retry budget, the row
escalates the same way F4 escalations did. The runtime-assert
family may benefit from the F69 `BuildOverrides` surface (e.g.,
a `-Z` flag that triggers the specific assert variant,
analogously to `-Z mir-emit-retag` for RJ-006); the implementer
surveys the available `-Z` flags during snippet authoring.

**Provisional nature of the 27 count**: the 27 figure is a
snapshot derived against the F69-close sidecar + the F68
audit's target list + Category 1's cohort. The matrix-precision
pass (Category 6) may reclassify some currently-`uncovered`
matrix rows as covered-via-aliasing (the SK-006 case is the
template), which would shrink the 27. F7 implementer should
pull a fresh derivation at the time the F7 task spec is
drafted; if Category 6 has landed by then, ingest its findings.
The 27 is not a hard contract on the F7 sweep size.

**Proposed scope**: an F7-style discovery batch in the F4
shape -- per-row 3-retry budget, snippet authoring, sidecar
entries, escalations stay OUT of sidecar. The 27 post-exclusion rows are well
below F4's ~102-target scope; the runner's sidecar coverage
moves from 125 to (125 + however many PASS).

**Priority**: **MEDIUM**. Closes the discovery arc once F68's
six remaining proposals (A1-F1) plus Category 1's reattempt
sweep land. Premature to file before F68 closes most of its
slate, because A1/B1/B2/C1/C2 may surface unexpected new
escalations that should be triaged before F7 starts.

**Effort**: moderate (full F4-style sweep across the 27-row
post-exclusion cohort; ~2-3 agent-hours per F4's empirical
timing, scaled down from F4's ~102-row scope).

**Dependencies**: best filed AFTER A1 (driver-70) closes and at
least one of B1/C1/C2 closes; F7's value is highest when the
bridge surface it sweeps has the F68 fixes already landed (so
F7 can attempt rows whose F4-era rejection was just an A1-class
projection gap).

## Category 4 -- Section 7 validator invariant audit (49 unknown rows)

**Affected matrix rows**: 49 of 60 Section 7 (validator
invariants) rows are marked `unknown -- verify` at
`docs/discussions/regseq-model/bridge-coverage-matrix.md:641-689`.
The remaining 11 are marked `yes`.

**Root cause**: Section 7 rows are constructed-model tests
(per strategy §2 "Out of scope (initial landing)"). The
bridge-coverage runner does NOT exercise these; they require a
separate constructed-model harness that feeds the validator
hand-built model instances and asserts violation production.
The `unknown -- verify` annotation reflects that NO test
artifact currently proves whether the validator catches the
named invariant violation.

**Empirical implication**: each `unknown` row is a latent
soundness concern: the validator MAY catch the invariant, but
no test exists to prove it. Backsliding via future bridge or
validator changes would go undetected. The right work is a
constructed-model harness + per-row tests.

**Proposed scope**: a Section 7 verifier audit -- separate track
from the bridge-coverage discovery work. Either:
- (a) a dedicated `validator-coverage-harness` crate that feeds
  hand-built `RegseqModel` instances and asserts the validator
  emits the expected `InvariantViolation`; OR
- (b) extend the existing `crates/regseq-model/tests/` corpus
  with per-invariant constructed-model tests.

**Priority**: **MEDIUM**. Independent of the bridge-coverage
arc; doesn't compete with F68's slate for bridge-side resources.
Worth filing as a parallel track if the user wants validator
soundness coverage to track the bridge coverage.

**Effort**: moderate-to-large (49 row-level tests + harness
scaffolding). Could be batched 5-10 rows per task to keep PRs
reviewable.

**Dependencies**: none from the bridge-coverage arc; depends
only on validator stability.

## Category 5 -- F5 automation / operational gap

**Affected items** (per strategy `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md:449-485`):

- **JSON report output**: the runner currently emits Markdown
  (`report.md`) only. Strategy §10 specifies a JSON variant
  for machine consumers.
- **Mode B agent integration**: a runner-driver loop that lets
  an agent (Claude / Codex / etc.) author sidecar rows from
  the runner's report, with human approval gates per the F68
  Mode B Non-Goal at the task spec
  (`.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:46-60`).
- **Mode C CI gating**: an invocation form that fails CI when
  Total != Pass.

F4 explicitly excluded all three at
`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:520-521`.

**Empirical implication**: the discovery-engine arc closes
once F68's six remaining proposals land + Categories 1/3
optionally land. After that, the runner becomes a
maintenance-mode artifact; the automation gap blocks scaling
it into a continuous-coverage tool that future RegSeq work
can use to detect bridge regressions.

**Proposed scope**: three independent task specs, one per
item. JSON report is the cheapest (small runner change,
serde-driven). Mode C is the next cheapest (subprocess exit
code + CI workflow). Mode B is the largest (agent integration
+ approval gates + harness state).

**Priority**: **LOW**. Strategy §10 explicitly deferred until
corpus stabilises; F68 + Category 1 + Category 3 should land
first to validate the corpus shape before automation locks it
in.

**Effort**: small (JSON) / small (Mode C) / large (Mode B).

**Dependencies**: deferred until after F4 close (already met)
+ F68 close (in progress).

## Category 6 -- Matrix-precision pass + runner-side precision triggers

This category bundles the "actionable Category 1 sub-items" from
the pre-Codex draft. Codex verified each is grounded and
recommended broadening the matrix-precision pass.

**Affected items**:

- **Add `InspectionPredicate::Not<P>` composite**. Per
  `crates/bridge-coverage-runner/src/inspect.rs:43-90`, the
  current predicate set has `All` and `Any` composites but
  no negation. Unblocks FT-004 (`ManuallyDrop` NEGATIVE
  assertion) per F4 spec
  `.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:132-134`.
- **Add `InspectionPredicate::ByteVariant` leaf**. Per
  `crates/bridge-coverage-runner/src/inspect.rs:188-196`,
  `ConstVariant` currently stops at `Const::Bytes` without
  distinguishing inner `Byte::Uninit / Value / Reloc` per
  `crates/regseq-model/src/const_value.rs:67-75`. Unblocks
  OP-003-G-1 (`Byte::Uninit`) and OP-003-G-3 (`Byte::Reloc`)
  per F4 spec `:126-131`.
- **Add `Linkage::Weak` (and likely `NoMangle`) producers in
  bridge**. The inspector can already match `Weak` at
  `crates/bridge-coverage-runner/src/inspect.rs:386-394`, but
  bridge eager item production at HEAD only emits `Default` and
  `Foreign` per `crates/regseq-driver/src/bridge/mod.rs:908-956`
  (Codex round-2 verified that `NoMangle` is NOT in the HEAD
  producer set either, contrary to F4's pre-driver-69 spec
  language). The actionable trigger therefore covers both
  `Weak` and `NoMangle` production. Unblocks CT-010 (`Weak`)
  per F4 spec `:135-143` and provides positive coverage for
  the `NoMangle` linkage variant that the bridge already
  accepts in `BridgeError` rejection paths.
- **Matrix-precision pass** (broadened per Codex critique):
  annotate the four known IX-aliased rows (OP-003-K, TM-005-D,
  FT-003, FT-009) as covered-by-IX-N per F4 spec `:117-125`,
  AND reconcile the 96 matrix-`covered` rows that are missing
  from the sidecar (some are correctly aliased; some may be
  stale `covered` claims like FF-002 in Category 2), AND
  reconcile matrix-`uncovered` rows that are nonetheless
  exercised by sidecar entries through aliasing. Codex
  round-2 surfaced **SK-006** (`SyntheticItemKind::ClosureDropGlue`)
  as one such case: matrix `:382` marks SK-006 `uncovered`, but
  IX-013's sidecar row at
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml:177-184`
  carries `inspect = { kind = "SyntheticItemKind", arg = "ClosureDropGlue" }`,
  and the matrix at `:494` itself notes IX-013 "exercises
  ClosureDropGlue (SK-006)". The broader pass scope is
  justified because the precise count of "actually uncovered"
  rows in Category 3 depends on which of the 96 are genuinely
  aliased vs. stale AND on how many `uncovered` rows are
  similarly aliased like SK-006. Either misclassification can
  shift the 43-pre-exclusion / 27-post-exclusion counts.

**Proposed scope**: four sub-proposals, each fileable as a
separate task spec. The first three are bounded (each adds one
enum variant + dispatch arm + one row reattempt). The fourth
is documentation-driven and may grow as the audit surfaces more
false positives.

**Priority**: **LOW** individually. The four together close
small precision gaps but don't unblock substantive coverage. The
matrix-precision pass is the highest-leverage of the four
because it sharpens the inputs to future triages.

**Effort**: small (each of the first three); moderate-to-large
(the matrix-precision pass, depending on findings).

**Dependencies**: none.

## Closing recommendations

Recommended priority order for the user to approve filings
beyond F68's existing slate (A1 / B1 / B2 / C1 / C2 / F1):

| Order | Category | Item | Priority | Effort | Recommended action |
| ----- | -------- | ---- | -------- | ------ | ------------------ |
| 1 | 1 | Matrix-doc OPEN BUG refresh + closure-family reattempt sweep (13 rows) | MEDIUM | small docs + moderate authoring | Approve. Highest-leverage gap-closing work after F68's bridge fixes. |
| 2 | 4 | Section 7 validator audit (49 unknown rows) | MEDIUM | moderate-to-large (batched) | Approve as a parallel track if validator soundness coverage matters; defer if not. |
| 3 | 3 | F7 discovery batch over the 27 post-exclusion uncovered rows (TM-006-F/G/H are included in the 27, not additional) | MEDIUM | moderate F4-style sweep | Approve AFTER A1 closes and at least one of B1/C1/C2 closes (F7's value is highest with the F68 fixes already landed). The 27 is provisional pending Category 6's matrix-precision pass; pull a fresh count when filing the F7 spec. |
| 4 | 2 | FF-002 covered-status audit | LOW | docs-only | Approve as a 30-minute audit BEFORE Category 6's matrix-precision pass runs (so the matrix-precision pass has FF-002's correct status to work from). |
| 5 | 6 | Matrix-precision pass + small runner / bridge precision triggers (Not / ByteVariant / Linkage::Weak + NoMangle / matrix annotations including SK-006-style uncovered aliases) | LOW | small per item | Approve as a single bundled task or four small tasks at the user's discretion. Runs after Order 4 so it can ingest the FF-002 audit's findings. |
| 6 | 5 | F5 automation slate (JSON report / Mode C CI / Mode B agent integration) | LOW | small / small / large | Defer until F68 + Categories 1 + 3 close; revisit when the discovery-engine arc is functionally complete. |

**Priority + dependency consistency check**:
- Order 4 (FF-002 audit, Category 2) is a prerequisite
  quality-improver for Order 5 (matrix-precision pass, Category
  6). The table ordering reflects this dependency: Category 2
  precedes Category 6 in the proposed sequence even though both
  are LOW priority.
- Order 3 (F7) is gated on F68's A1/B1/C1/C2 closing, per the
  rationale in Category 3. Order 3's 27-row count is also
  PROVISIONAL relative to Order 5 (Category 6): the
  matrix-precision pass may reclassify some currently-`uncovered`
  rows as aliased (the SK-006 template), shrinking F7's effective
  scope. The Closing order schedules Order 3 ahead of Order 5
  because (a) Order 3 is MEDIUM priority and Order 5 is LOW;
  (b) F4's empirical timing absorbs a few percentage-point
  row-count drift without restructuring the sweep; (c) if Order
  5 lands before the F7 spec is drafted, the F7 implementer
  ingests the refined count at spec-drafting time, eliminating
  the conflict.
- No other internal dependencies; categories are independent.

**Coverage check**: each gap surfaced by the pre-Codex analysis
+ the Codex round-1 + round-2 critiques is represented in some
category above. The Codex REQUIRED FIXES are addressed:
- FIX 1 (counts: 359 / 299 + 60 / 234 missing): §Quantitative
  baseline.
- FIX 2 (OPEN BUG stale at HEAD per driver-62): Category 1.
- FIX 3 (Category 4 math + scope; 43 pre-exclusion / 27
  post-exclusion): Category 3.
- FIX 4 (missed `TM-006-F/G/H`, Section 7, F5 automation): Category
  3 explicitly scopes `TM-006-F/G/H`; Category 4 covers Section
  7; Category 5 covers F5 automation.
- FIX 5 (FF-002 as likely matrix false-positive): Category 2.
- Codex round-2 follow-ups (359 not 356; 234 not 231; 27 not 43
  post-exclusion; Linkage `NoMangle` + `Weak` both missing from
  HEAD producers; 13-row Category-1 cohort not 11-row; SK-006-style
  matrix-uncovered-but-aliased reconciliation): all integrated
  into the relevant category texts above.

**No auto-filing reminder**: this document is a PROPOSAL artifact
only, mirroring the F68 audit's Mode B discipline. No
`task.py add-task` invocations have occurred for the proposed
follow-up tasks. The user reviews this document, approves a
subset, and follow-up sessions draft the approved proposals into
task specs through the spec → Codex → file flow per the F69 /
F70 pattern.

## Out-of-scope discipline

This document does NOT:
- Duplicate F68's seven proposals (A1 / B1 / B2 / C1 / C2 / D1 /
  F1). Each remains owned by the F68 audit at
  `.harness/audits/2026-05-21-post-f4-triage.md`.
- Touch the matrix doc, spec.md, strategy doc, pretty-printer
  doc, generic-c-backend doc, or any code.
- Re-enumerate the F4-documented matrix blocks as proposals;
  blocks are documented at
  `.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md:70-144`
  and remain intentional out-of-scope per F4 policy. This
  document surfaces the SUBSET of those blocks for which an
  unblock trigger exists (Category 6).
- Propose changes to the F68 audit doc. Its 7 proposals stand
  as the canonical post-F4 triage output.
