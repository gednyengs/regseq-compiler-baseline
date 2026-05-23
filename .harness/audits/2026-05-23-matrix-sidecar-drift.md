# Matrix ↔ sidecar coverage drift (finding; remedy deferred)

**Status**: OPEN. Diagnosed 2026-05-23 at HEAD `9186248` (post-F72).
Remedy deferred to gap-inventory Category 5 (derived coverage
reporting). This is a *diagnosis*, not a fix — the matrix is
unchanged.

## Finding

Bridge coverage is tracked by two mechanisms that are NOT
auto-reconciled, and they disagree:

1. **Matrix** (`docs/discussions/regseq-model/bridge-coverage-matrix.md`)
   — per-row `status` column. Hand-backfilled per task. Reflects
   legacy `crates/test-ir-fixtures/fixtures/` coverage plus whatever
   runner rows a task chose to backfill.
2. **Runner sidecar**
   (`crates/bridge-coverage-runner/matrix_metadata/rows.toml` +
   `snippets/`) — every committed row is a passing experiment.

At HEAD `9186248`, the read-only report
(`scripts/python/coverage_drift.py`) shows:

```
sidecar committed rows                 : 140
matrix rows parsed                     : 295 (64 unparseable)
matrix rows marked covered             : 113
  consistent (sidecar + matrix=covered):  19
  DRIFT (sidecar, matrix!=covered)     : 121
  matrix-only (covered, not in sidecar):  94
```

So the runner sidecar covers **140** rows but the matrix `status`
column acknowledges only **19** of them as `covered`; the other
**121** still read `uncovered`/`partial`/`blocked-generator-gap`.
Separately, **94** matrix-`covered` rows are not in the sidecar
(legacy test-ir-fixtures coverage). The two covered sets are largely
disjoint (~19 overlap), so **neither artifact alone reports total
coverage** — it is the union.

The 64 unparseable matrix rows are **not** a tool defect: 60 are
Section 7 `VL-*` validator-invariant rows (a separate track using
`exercised in …` / `unknown -- verify` markers, not the coverage
status taxonomy), and 4 are rows with qualified status cells
(CT-002 `covered (rejection-path covered)`, RJ-004/RJ-007/RJ-008).
The tool flags these for manual review rather than guessing. None of
the 64 are sidecar rows, so they do not affect the drift count.

## Root cause

The matrix `status` is *supposed* to track runner coverage — F71 and
F72 backfilled their rows, citing runner snippets as `fixture_path`
(e.g., the SK-003/FT-011/IX-004/FF-007 rows). But the bulk F4 corpus
run (regseq-driver-67) committed ~100+ rows to the sidecar **without**
backfilling the matrix `status`. Only the small targeted follow-up
tasks (F71, F72) backfilled. Hence the matrix systematically
understates runner coverage by the ~121 un-backfilled rows.

Consequence: the matrix `status` column is NOT a reliable view of
actual bridge coverage. Anyone prioritizing next work off matrix
`status == uncovered` risks authoring snippets for rows the runner
already covers.

## Remedy (deferred to Category 5)

Do **not** hand-backfill the 121 rows — that re-drifts on the next
bulk corpus run and is the same hand-maintenance trap the gap-ledger
proposal (`.harness/audits/2026-05-23-gap-ledger-proposal.md`) was
deferred for. The durable fix is to **derive** the matrix `status`
from the sidecar (+ a legacy-fixture scan), as part of gap-inventory
Category 5 (`.harness/audits/2026-05-22-gap-inventory.md:349-388`,
JSON-report / automation track). A deliberate Category 5 cutover also
avoids the half-migrated "some rows derived, some hand-edited" state
that flipping to derived-status mid-stream would create.

Both of the 2026-05-23 threads — the gap ledger and this drift —
point at the same conclusion: a single *derived* coverage/gap report,
built when Category 5 is undertaken.

## Interim mitigation

`scripts/python/coverage_drift.py` is a read-only report that surfaces
this drift on demand and would catch future drift early, without
locking in a still-moving status taxonomy. Run:

```
uv run python scripts/python/coverage_drift.py        # summary + drift list
uv run python scripts/python/coverage_drift.py --all  # + matrix-only + unparseable
```

It never edits the matrix or sidecar; exit code is always 0 (report,
not gate — CI gating is Category 5 Mode C).
