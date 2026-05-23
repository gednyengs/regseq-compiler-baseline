# Gap-ledger proposal (sketch for review)

**Status**: DEFERRED to gap-inventory Category 5 (2026-05-23). Codex
review (session `019e564f`) returned **NEED: WARRANTED_LATER** /
**SKETCH: APPROPRIATE_WITH_FIXES**. The problem this ledger solves is
real, but a *hand-maintained* ledger now is a trap — the populated
example in this very doc drifted from HEAD within minutes of being
written (F72 gaps marked `in-progress` while `tasks.json` had F72
`pending`; IX-014 marked `open` while it is a committed/covered
sidecar row; RV-009-G marked `open` at row level while committed).
A stale ledger is worse than none. The recommendation is to build
this **derived** from source artifacts (matrix + sidecar +
expectation files + tasks.json) when Category 5 (JSON report /
automation) is undertaken, NOT hand-maintained.

Schema fixes to carry forward when it is built:
- Separate `row_coverage_state` from `gap_lifecycle_state` (the
  IX-014 dual-sense proves these are different axes).
- First-class `cluster_id` (a `[[cluster]]` table), not free-text
  `cluster` — that is the N:1 root-cause→rows mapping the ledger
  exists to preserve.
- Derive `status` from task state rather than hand-asserting it.
- JSON for generated output; fix the TOML-has-no-null issue.

This doc is retained as the design note for that future work. The
sections below are the original sketch (pre-deferral).

## 1. Problem statement

The bridge-coverage workflow tracks *execution* well (task.py
lifecycle, plan→Codex→spec→Codex→implement→assess, matrix as per-row
coverage truth, sidecar as committed-pass record). But the **gap
lifecycle** (discovered → triaged → specced → resolved) is tracked
implicitly across scattered artifacts:

- **Matrix doc** (`bridge-coverage-matrix.md`): per-row *current*
  status only. Surfaces a gap as `blocked-*` ONLY when a realisable
  shape hits it. Escalations without a `blocked-*` row (e.g., FF-005
  RustCall ABI, the F68 B1/B2/C1/C2 proposals) are invisible here.
- **Sidecar** (`rows.toml`): committed passes + inline escalation
  rationale, scattered per-row.
- **Expectation files** (`.harness/expectations/regseq-driver-NN.md`):
  per-task outcome records, one file per task.
- **Audits** (F68 triage, gap inventory): point-in-time snapshots
  that go stale immediately (the 2026-05-22 gap inventory was already
  stale by 2026-05-23 after F71 closed).
- **tasks.json**: task lifecycle, but no gap↔task linkage beyond
  `depends_on`.

**Symptom**: answering "how many bridge gaps collected vs resolved?"
required reconstructing the answer by reading 5 artifacts and
assembling the task arc by hand. There is no single queryable record
of gap state, and the N:1 root-cause→rows cluster mapping lives only
in audit prose.

## 2. What the ledger is (and is NOT)

**IS**: a living, per-gap lifecycle record. One entry per *escalation*
(distinct bridge/runner/model defect or design-gap), tracking its
discovery, root-cause cluster, blocking rows, fix-path, and
resolution. Maintained at the same cadence as HANDOFF (updated at
escalation time and at task close).

**IS NOT**:
- A replacement for the matrix (matrix = per-row contract coverage;
  ledger = per-gap lifecycle + cluster mapping).
- A replacement for the sidecar (sidecar = committed passes).
- A replacement for tasks.json (tasks.json = task lifecycle).
- A fixture/snippet store.
- An external tracker system (Linear / GitHub issues / DB) — those
  are overkill at the current ~16-gap scale and add sync burden
  against the source-of-truth docs.

The ledger is the per-gap complement to the per-category gap
inventory, and the lifecycle complement to the per-row matrix.

## 3. Proposed schema

Format: TOML (matches `rows.toml`; machine-readable for eventual
derivation per gap-inventory Category 5). One `[[gap]]` table per
escalation.

```toml
[[gap]]
# Stable escalation id. Reuse the matrix/expectation id where one
# exists (FF-005, FF-007, OP-003-I, ...). For root-cause defects with
# no single row id, mint a slug (e.g., "prescan-args-substitution").
id = "FF-005"

# One-line human summary.
title = "RustCall ABI rejection blocks Fn-family closure-call sig translation"

# Where the fix lands: bridge | runner | model | docs.
kind = "bridge"

# Task that first surfaced the gap.
discovered_in = "regseq-driver-70"
discovered_date = "2026-05-22"

# Root-cause one-liner + file:line citation at discovery time.
root_cause = "translate_abi(ExternAbi::RustCall) rejects before FnPtrShim materialization"
root_cause_site = "crates/regseq-driver/src/bridge/fnsig.rs:107-115"

# Root-cause cluster (the N:1 grouping). For F68-triaged gaps this is
# the proposal id (A1/B1/C1/...). Otherwise a descriptive cluster name.
cluster = "A1-adjacent (type normalization; split off as separate)"

# Empirical (Mode 1, 3-retry + bridge-error) or design-time (Mode 2,
# predicate-precision / no realisable witness).
escalation_mode = "empirical"

# Matrix rows blocked by this gap (the N in the N:1 mapping).
blocking_rows = ["FF-005"]

# Lifecycle: open | triaged | specced | in-progress | resolved | by-design
status = "open"

# Audit that triaged it into a proposal (if any).
triaged_in = "2026-05-21-post-f4-triage.md"

# Task spec addressing it (if specced). null until specced.
spec_task = ""

# Task that resolved it (if resolved). null until resolved.
resolving_task = ""
resolved_date = ""

# Free-text. Cross-references, splits, caveats.
notes = "Surfaced during F70/A1 when the FF-005 row's Fn::call path hit RustCall ABI before FnPtrShim. Distinct from FF-007 (Phase-5 materialization). Inline rationale at rows.toml:1316-1340."
```

## 4. Populated example (real gaps, as of HEAD 43a8a26)

```toml
[[gap]]
id = "prescan-args-substitution"
title = "Phase-5 prescan walked unsubstituted MIR; polymorphic Fn::call survived"
kind = "bridge"
discovered_in = "pre-driver-62"
root_cause = "prescan used optimized_mir(local_def_id) without instantiate-and-normalize"
root_cause_site = "crates/regseq-driver/src/bridge/mod.rs:769-789 (post-fix)"
cluster = "prescan-args-substitution"
escalation_mode = "empirical"
blocking_rows = ["IX-004", "IX-010", "BD-007", "FT-011", "FT-012", "FT-013", "FT-014", "SK-003", "IX-014"]
status = "resolved"
resolving_task = "regseq-driver-62"
notes = "One fix unblocked the 9-row closure cohort; F71 authored the snippets + refreshed the stale OPEN BUG matrix prose at :508-523."

[[gap]]
id = "RV-007-C"
title = "Fn-family ::Output projection not normalized before Alias rejection"
kind = "bridge"
discovered_in = "regseq-driver-67"
cluster = "A1"
escalation_mode = "empirical"
blocking_rows = ["RV-007-C"]
status = "resolved"
triaged_in = "2026-05-21-post-f4-triage.md"
spec_task = "regseq-driver-70"
resolving_task = "regseq-driver-70"

[[gap]]
id = "RV-009-G"
title = "Pointee::Metadata projection: sized vs slice vs dyn"
kind = "bridge"
discovered_in = "regseq-driver-67"
cluster = "A1"
escalation_mode = "empirical"
blocking_rows = ["RV-009-G"]
status = "open"
triaged_in = "2026-05-21-post-f4-triage.md"
spec_task = "regseq-driver-70"
notes = "SPLIT: sized case resolved by F70; slice case escalated (blocked by unrelated rvalue.rs:197 gap); dyn excluded by design."

[[gap]]
id = "FF-005"
title = "RustCall ABI rejection blocks Fn-family closure-call sig translation"
kind = "bridge"
discovered_in = "regseq-driver-70"
root_cause_site = "crates/regseq-driver/src/bridge/fnsig.rs:107-115"
cluster = "A1-adjacent (split off)"
escalation_mode = "empirical"
blocking_rows = ["FF-005"]
status = "open"
triaged_in = "2026-05-21-post-f4-triage.md"
notes = "Surfaced during F70. Distinct from FF-007 Phase-5 materialization. Inline rationale at rows.toml:1316-1340."

[[gap]]
id = "TM-006-C"
title = "Per-fixture build-flag override needed in test-ir-fixtures helper"
kind = "runner"
discovered_in = "regseq-driver-67"
cluster = "D1"
escalation_mode = "empirical"
blocking_rows = ["TM-006-C"]
status = "resolved"
triaged_in = "2026-05-21-post-f4-triage.md"
resolving_task = "regseq-driver-69"

[[gap]]
id = "RJ-006"
title = "Build-flag override (shared with TM-006-C)"
kind = "runner"
discovered_in = "regseq-driver-67"
cluster = "D1"
escalation_mode = "empirical"
blocking_rows = ["RJ-006"]
status = "resolved"
triaged_in = "2026-05-21-post-f4-triage.md"
resolving_task = "regseq-driver-69"

[[gap]]
id = "OP-003-I"
title = "Const-eval for unevaluated ADT-enum consts (const-generic position)"
kind = "bridge"
root_cause_site = "crates/regseq-driver/src/bridge/const_.rs:42-49"
cluster = "C1"
escalation_mode = "empirical"
blocking_rows = ["OP-003-I"]
status = "triaged"
triaged_in = "2026-05-21-post-f4-triage.md"

[[gap]]
id = "FF-013"
title = "Accept fn-pointer-provenance allocations (FnPtrAddrShim)"
kind = "bridge"
cluster = "C2"
escalation_mode = "empirical"
blocking_rows = ["FF-013"]
status = "triaged"
triaged_in = "2026-05-21-post-f4-triage.md"

[[gap]]
id = "FF-012"
title = "CloneShim materialization for arrays/tuples"
kind = "bridge"
cluster = "B1"
escalation_mode = "empirical"
blocking_rows = ["FF-012"]
status = "triaged"
triaged_in = "2026-05-21-post-f4-triage.md"

[[gap]]
id = "FF-004"
title = "Deferred-shim probe (ReifyShim)"
kind = "bridge"
cluster = "B2"
escalation_mode = "empirical"
blocking_rows = ["FF-004"]
status = "triaged"
triaged_in = "2026-05-21-post-f4-triage.md"
notes = "LOW priority; probe + decision; uncertain payoff. Bundled with RJ-024 under B2."

[[gap]]
id = "RJ-024"
title = "UnsupportedEnumLayout (B2 deferred-shim probe)"
kind = "bridge"
cluster = "B2"
escalation_mode = "empirical"
blocking_rows = ["RJ-024"]
status = "triaged"
triaged_in = "2026-05-21-post-f4-triage.md"

[[gap]]
id = "IX-014"
title = "ClosureCall sub-axis predicate evidence (deferred pending external work)"
kind = "runner"
discovered_in = "regseq-driver-65"
cluster = "F1"
escalation_mode = "design-time"
blocking_rows = ["IX-014"]
status = "open"
triaged_in = "2026-05-21-post-f4-triage.md"
notes = "NEEDS-RECONCILIATION: F1 lists IX-014 as docs-only watch-list (cluster F external-dependency), but F71 ALSO committed an IX-014 sidecar row as part of the closure cohort. The two senses must be disambiguated — this ambiguity is itself an argument for the ledger."

[[gap]]
id = "RJ-032-F"
title = "Deferred-pending-external-work watch list item"
kind = "docs"
discovered_in = "regseq-driver-67"
cluster = "F1"
escalation_mode = "design-time"
blocking_rows = ["RJ-032-F"]
status = "open"
triaged_in = "2026-05-21-post-f4-triage.md"

[[gap]]
id = "FF-007"
title = "ClosureOnceShim Phase-5 ItemMissingInstance"
kind = "bridge"
discovered_in = "regseq-driver-71"
root_cause_site = "crates/regseq-driver/src/bridge/body/materialise.rs:335-389"
cluster = "F72-subtask-B"
escalation_mode = "empirical"
blocking_rows = ["FF-007"]
status = "in-progress"
spec_task = "regseq-driver-72"

[[gap]]
id = "RJ-009"
title = "NonRigidTy::Param has no surfacing path (bucket-4 carve-out absorbs it)"
kind = "bridge"
discovered_in = "regseq-driver-71"
root_cause_site = "crates/regseq-driver/src/bridge/mod.rs:1637-1660"
cluster = "F72-subtask-C"
escalation_mode = "empirical"
blocking_rows = ["RJ-009"]
status = "in-progress"
spec_task = "regseq-driver-72"
notes = "Bounded decision gate: 4 source-shape probes, else runner-outcome-model extension or independent escalation."

[[gap]]
id = "SK-004"
title = "SyntheticItemKind predicate cannot distinguish ClosureCall CallMut sub-axis"
kind = "runner"
discovered_in = "regseq-driver-71"
root_cause_site = "crates/bridge-coverage-runner/src/inspect.rs:168-182"
cluster = "F72-subtask-A"
escalation_mode = "design-time"
blocking_rows = ["SK-004"]
status = "in-progress"
spec_task = "regseq-driver-72"

[[gap]]
id = "SK-005"
title = "SyntheticItemKind predicate cannot distinguish ClosureCall CallOnce sub-axis"
kind = "runner"
discovered_in = "regseq-driver-71"
root_cause_site = "crates/bridge-coverage-runner/src/inspect.rs:168-182"
cluster = "F72-subtask-A"
escalation_mode = "design-time"
blocking_rows = ["SK-005"]
status = "in-progress"
spec_task = "regseq-driver-72"
```

## 5. Maintenance model

- **Cadence**: updated at two moments — (1) when a task escalates a
  gap (add/update entry with `status = open|triaged`), and (2) at task
  close (flip `status` to `resolved` + set `resolving_task`). Same
  rhythm as the HANDOFF refresh.
- **Owner**: whoever closes the assessment loop (currently the
  assessor agent), the same actor who refreshes HANDOFF.
- **Drift control**: every `blocking_rows` entry must exist in the
  matrix; every `resolving_task` / `spec_task` must exist in
  tasks.json. A future `task.py` subcommand (or a tiny script) can
  cross-check these invariants — and eventually *derive* the ledger
  from matrix + expectation files + tasks.json (gap-inventory
  Category 5). Until then it is hand-maintained but cross-checkable.

## 6. Queries the ledger answers in O(1)

- "How many gaps open vs resolved?" → filter on `status`.
- "What did F71 surface?" → filter `discovered_in == regseq-driver-71`.
- "What rows does FF-005 block?" → read `blocking_rows`.
- "Which gaps are bridge vs runner vs docs?" → filter `kind`.
- "What's left in the F68 slate?" → filter `cluster in {B1,B2,C1,C2,F1}` and `status != resolved`.
- "Which gap will F72 resolve?" → filter `spec_task == regseq-driver-72`.

## 7. Open questions for Codex

- **Q-NEED-1**: is a ledger warranted at the current ~16-gap scale,
  or is the implicit-across-docs approach acceptable until the gap
  count grows? Weigh reconstruction cost vs maintenance burden.
- **Q-NEED-2**: does the ledger duplicate the matrix/sidecar enough
  to create a drift hazard that outweighs its benefit?
- **Q-NEED-3**: should this be hand-maintained now, or deferred until
  it can be derived from sources (Category 5)? Is hand-maintenance a
  trap (stale ledger worse than no ledger)?
- **Q-SCHEMA-1**: are the fields right? Missing any? (e.g.,
  `priority`, `effort`, `blocked_by` for inter-gap dependencies?)
- **Q-SCHEMA-2**: is the `cluster` field the right way to encode the
  N:1 root-cause→rows mapping, or should clusters be first-class
  (a separate `[[cluster]]` table)?
- **Q-SCHEMA-3**: is TOML the right format vs a Markdown table vs
  extending an existing artifact (e.g., a section in the gap
  inventory, or new columns in the matrix)?
- **Q-SCHEMA-4**: the IX-014 NEEDS-RECONCILIATION entry — is the
  dual-sense (F1 watch-list vs F71 committed row) a real ambiguity,
  or did I mis-model it?
- **Q-SCHEMA-5**: is per-escalation the right granularity, or should
  the ledger be per-cluster (coarser) given refinements are
  per-root-cause?
- **Q-OVERLAP-1**: does this meaningfully overlap the gap inventory
  such that the gap inventory should simply BECOME this ledger
  (living vs point-in-time)?
