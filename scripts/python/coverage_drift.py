#!/usr/bin/env python3
"""Feature-complete-gate reconciliation engine (regseq-driver-79, G1).

This tool collapses the bridge-coverage trackers into ONE authoritative
per-row truth so we can declare the bridge IR-structurally
feature-complete before authoring/lowering device-driver specs. The
verification boundary is IR-structural acceptance: a construct bridges
to a `RegseqModel` that passes `model.validate()`. NOT C codegen, NOT
runtime.

It consumes three un-reconciled coverage sources:

  1. The bridge-coverage **matrix** (`bridge-coverage-matrix.md`). A
     human-owned, prose-rich, schema-HETEROGENEOUS enumeration. The
     `status`/coverage column is hand-backfilled and systematically
     understates runner coverage.
  2. The bridge-coverage-**runner** sidecar
     (`matrix_metadata/rows.toml`). Every committed row is an
     executable Mode-A experiment that PASSes; presence of a row id is
     the highest-confidence "validated" signal (INDEPENDENT of the
     sidecar row's own `status` string, which may itself be stale).
  3. Legacy **test-ir-fixtures** coverage, cited per-row in the matrix
     evidence column.

For each Sections-1-6 matrix row the tool DERIVES a coverage truth
(`validated-runner` / `validated-legacy[-universal]` / `needs-validation`)
from the three sources, while PRESERVING human out-of-scope
adjudications (`blocked-*` classes and reason-bearing rejected/uncovered
rows) verbatim as `out-of-scope`. Section 7 (validator invariants) is a
SEPARATE axis (`exercised? = yes/partial/no`), reconciled by
driver-78; it is folded into the verdict as an already-audited axis and
accounted on its own axis in the ledger.

Outputs: a read-only reconciliation summary (default), and on
`--write-ledger` the authoritative `coverage_ledger.{json,md}` keyed by
row id. The tool NEVER edits the matrix or the sidecar (the schema-aware
in-place de-drift write is the gated follow-up G1b).

Runtime dependencies are the standard library plus `tomllib` only, so
the tool runs under a bare `python3`. The test suite
(`test_coverage_drift.py`) uses `pytest`, added as a uv DEV dependency.

Exit code is always 0 (report, not gate). CI gating is deferred.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

# Closed coverage/adjudication taxonomy as DECLARED in the matrix doc
# "How to read" section. `rejected` is an EXPECTATION class, not a
# coverage value, so it is deliberately absent here -- a bare `rejected`
# cell must never be read as coverage. `out-of-scope` is an adjudication
# (not a `blocked-*`): the shape is rustc-producible but lies on a
# non-IR axis (e.g. an edition front-end) with no RegseqModel construct
# to validate beyond what existing rows cover.
STATUS_VALUES = {
    "covered",
    "partial",
    "uncovered",
    "out-of-scope",
    "blocked-spec-gap",
    "blocked-rustc-infeasible",
    "blocked-bridge-rejects-by-design",
    "blocked-generator-gap",
    "blocked-open-bridge-bug",
    "blocked-unimplemented-phase",
}

# Matrix row ids: 2-3 uppercase letters, hyphen, 3 digits, then zero or
# more hyphen-suffix segments (e.g., RV-006-B, OP-003-G-2).
ROW_ID_RE = re.compile(r"^[A-Z]{2,3}-[0-9]{3}(?:-[A-Z0-9]+)*$")

# Leading taxonomy token of a coverage/adjudication cell. The token is
# the maximal run of letters/digits/hyphens at the start of the cell;
# any trailing parenthetical is captured separately as the reason.
_LEAD_TOKEN_RE = re.compile(r"^([A-Za-z][A-Za-z0-9-]*)")
_PARENTHETICAL_RE = re.compile(r"\(([^)]*)\)")

# Section markers in the matrix prose.
_SECTION_RE = re.compile(r"^#\s+Section\s+(\d+)\b")
_SUBSECTION_RE = re.compile(r"^##\s+(\d+(?:\.\d+)*)\b")

# Per-table-schema detection, keyed by the (lower-cased, stripped)
# header cell tuple. Each schema fixes which cell carries coverage and
# which carries evidence (see parse_row).
HEADER_SCHEMAS: dict[tuple[str, ...], str] = {
    ("id", "construct", "expectation", "status", "fixture_path", "notes"): "canonical",
    ("id", "sub-construct", "status", "notes"): "subaxis",
    (
        "id",
        "interaction",
        "expectation",
        "status",
        "fixture_path / bug-task",
        "notes",
    ): "interaction",
    ("id", "rejection variant", "source trigger", "status", "notes"): "section6main",
    ("id", "variant", "exercised?", "notes"): "section7",
}

# Markers, read from a Section-7 row's notes, that document a non-`yes`
# validator row as a permanently-accepted residual (an
# unreachable-by-construction backstop, not a fixture-batch gap). Used
# only to keep the verdict honest; derived from matrix prose, not a
# hard-coded id list.
_ACCEPTED_VALIDATOR_MARKERS = ("unreachable-by-construction", "backstop")

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MATRIX = REPO_ROOT / "docs/discussions/regseq-model/bridge-coverage-matrix.md"
DEFAULT_SIDECAR = REPO_ROOT / "crates/bridge-coverage-runner/matrix_metadata/rows.toml"
DEFAULT_FIXTURES_DIR = REPO_ROOT / "crates/test-ir-fixtures/fixtures"
LEDGER_DIR = REPO_ROOT / "docs/discussions/regseq-model"

CAVEAT = (
    "CAVEAT: coverage of the enumerated rows is NOT a proof the enumeration is "
    "exhaustive over the Rust surface."
)


# ---------------------------------------------------------------------------
# Records
# ---------------------------------------------------------------------------


@dataclass
class MatrixRow:
    """One parsed matrix row.

    `coverage_token` is the canonicalized leading taxonomy word of the
    schema's coverage cell (for coverage-axis rows); `reason_text` is the
    parenthetical stripped from that cell. `evidence_ref` is the raw
    evidence string (fixture_path or notes, per schema). For Section-7
    rows `axis == "validator"` and `exercised` carries yes/partial/no.
    `anomaly`, when set, explains why a Sections-1-6 row could not be
    classified; such a row is reported by id, never silently dropped.
    """

    id: str
    section: str
    schema: str
    axis: str  # "coverage" | "validator"
    expectation: str | None = None
    coverage_token: str | None = None
    reason_text: str = ""
    evidence_ref: str = ""
    exercised: str | None = None
    notes: str = ""
    anomaly: str | None = None


@dataclass
class RowSignals:
    """Per-row source signals feeding the derivation."""

    runner: bool = False
    legacy_kind: str = "none"  # named-resolved | universal | named-stale | playground | none
    adjudicated: bool = False
    evidence: str = ""


@dataclass
class LedgerEntry:
    """An authoritative per-row truth, keyed by id in the ledger."""

    id: str
    section: str
    axis: str
    truth: str
    evidence: str
    matrix_current_status: str
    reason: str = ""


@dataclass
class Reconciliation:
    """The full reconciliation result over a matrix + sidecar snapshot."""

    entries: list[LedgerEntry] = field(default_factory=list)
    anomalies: list[MatrixRow] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Loading
# ---------------------------------------------------------------------------


def load_sidecar_ids(path: Path) -> set[str]:
    """Return the set of committed sidecar row ids.

    Presence of an id is the runner signal: every committed row is a
    passing Mode-A experiment, so the id being present -- not its
    `status` string -- is what the derivation trusts.
    """
    with path.open("rb") as f:
        data = tomllib.load(f)
    return {row["id"] for row in data.get("row", []) if row.get("id")}


def _cells(line: str) -> list[str]:
    """Split a markdown table row into stripped cells (backticks removed)."""
    parts = line.split("|")
    if parts and parts[0].strip() == "":
        parts = parts[1:]
    if parts and parts[-1].strip() == "":
        parts = parts[:-1]
    return [c.replace("`", "").strip() for c in parts]


def _is_separator(cells: list[str]) -> bool:
    """True for a markdown table separator row (e.g. `--- | :---:`)."""
    return bool(cells) and all(
        c != "" and set(c) <= set("-: ") for c in cells
    )


# ---------------------------------------------------------------------------
# Coverage-token canonicalization
# ---------------------------------------------------------------------------


def canonicalize_coverage(cell: str) -> tuple[str | None, str]:
    """Canonicalize a coverage/adjudication cell.

    Returns `(token, reason_text)` where `token` is the leading word iff
    it is in the closed taxonomy (else `None`), and `reason_text` is the
    cell's first parenthetical (else `""`). E.g. `covered (rejection-path
    covered)` -> `("covered", "rejection-path covered")`; a bare
    `rejected` -> `(None, "")` (an expectation, never coverage).
    """
    s = cell.strip()
    if not s:
        return None, ""
    m = _LEAD_TOKEN_RE.match(s)
    if not m:
        return None, ""
    lead = m.group(1)
    reason = ""
    pm = _PARENTHETICAL_RE.search(s)
    if pm:
        reason = pm.group(1).strip()
    if lead in STATUS_VALUES:
        return lead, reason
    return None, ""


# ---------------------------------------------------------------------------
# Schema-aware parser
# ---------------------------------------------------------------------------


def parse_row(
    rid: str,
    cells: list[str],
    schema: str,
    section: str,
) -> MatrixRow:
    """Parse a single matrix data row under its detected table schema."""
    if schema == "canonical":
        expectation = cells[2] if len(cells) > 2 else None
        cov_cell = cells[3] if len(cells) > 3 else ""
        evidence = cells[4] if len(cells) > 4 else ""
        notes = cells[5] if len(cells) > 5 else ""
        token, reason = canonicalize_coverage(cov_cell)
        row = MatrixRow(
            id=rid,
            section=section,
            schema=schema,
            axis="coverage",
            expectation=expectation,
            coverage_token=token,
            reason_text=reason,
            evidence_ref=evidence,
            notes=notes,
        )
        if token is None:
            row.anomaly = f"canonical status cell not in taxonomy: {cov_cell!r}"
        return row

    if schema == "subaxis":
        cov_cell = cells[2] if len(cells) > 2 else ""
        notes = cells[3] if len(cells) > 3 else ""
        token, reason = canonicalize_coverage(cov_cell)
        row = MatrixRow(
            id=rid,
            section=section,
            schema=schema,
            axis="coverage",
            coverage_token=token,
            reason_text=reason,
            evidence_ref=notes,
            notes=notes,
        )
        if token is None:
            row.anomaly = f"sub-axis status cell not in taxonomy: {cov_cell!r}"
        return row

    if schema == "interaction":
        expectation = cells[2] if len(cells) > 2 else None
        cov_cell = cells[3] if len(cells) > 3 else ""
        evidence = cells[4] if len(cells) > 4 else ""
        notes = cells[5] if len(cells) > 5 else ""
        token, reason = canonicalize_coverage(cov_cell)
        row = MatrixRow(
            id=rid,
            section=section,
            schema=schema,
            axis="coverage",
            expectation=expectation,
            coverage_token=token,
            reason_text=reason,
            evidence_ref=evidence,
            notes=notes,
        )
        if token is None:
            row.anomaly = f"interaction status cell not in taxonomy: {cov_cell!r}"
        return row

    if schema == "section6main":
        # The declared header is 5-col, but rows take three shapes (6a
        # status=expectation+coverage-in-notes / 6b status=coverage / 6c
        # six-cell with an extra adjudication cell). Scan every cell
        # AFTER the rejection-variant (cells[1]) for the unique
        # canonicalized coverage/adjudication token; a bare `rejected` is
        # the expectation and never matches the taxonomy.
        notes = cells[-1] if len(cells) >= 2 else ""
        expectation = "rejected" if any(c.strip().lower() == "rejected" for c in cells) else None
        hits: list[tuple[str, str]] = []
        for c in cells[2:]:
            tok, rsn = canonicalize_coverage(c)
            if tok is not None:
                hits.append((tok, rsn))
        row = MatrixRow(
            id=rid,
            section=section,
            schema=schema,
            axis="coverage",
            expectation=expectation,
            evidence_ref=notes,
            notes=notes,
        )
        if len(hits) == 1:
            row.coverage_token, row.reason_text = hits[0]
        elif not hits:
            row.anomaly = "section-6 row: no coverage/adjudication token found"
        else:
            row.anomaly = (
                "section-6 row: ambiguous coverage tokens "
                + repr([t for t, _ in hits])
            )
        return row

    if schema == "section7":
        exercised = cells[2].strip().lower() if len(cells) > 2 else None
        notes = cells[3] if len(cells) > 3 else ""
        return MatrixRow(
            id=rid,
            section=section,
            schema=schema,
            axis="validator",
            expectation="validator-failing",
            exercised=exercised,
            evidence_ref=notes,
            notes=notes,
        )

    # No schema in scope (row before any recognized header): record as an
    # anomaly rather than guess.
    return MatrixRow(
        id=rid,
        section=section,
        schema="unknown",
        axis="coverage",
        anomaly="row not under a recognized table schema",
    )


def parse_matrix_text(text: str) -> list[MatrixRow]:
    """Parse matrix markdown text into a list of MatrixRow records.

    Routes each row by the most recent recognized table header
    (per-table-schema aware) and tracks the enclosing `# Section N`.
    """
    rows: list[MatrixRow] = []
    section = ""
    schema: str | None = None
    for raw in text.splitlines():
        sm = _SECTION_RE.match(raw)
        if sm:
            section = sm.group(1)
            schema = None
            continue
        if _SUBSECTION_RE.match(raw):
            continue
        if not raw.lstrip().startswith("|"):
            continue
        cells = _cells(raw)
        if not cells:
            continue
        header_key = tuple(c.lower() for c in cells)
        if header_key in HEADER_SCHEMAS:
            schema = HEADER_SCHEMAS[header_key]
            continue
        if _is_separator(cells):
            continue
        rid = cells[0]
        if not ROW_ID_RE.match(rid):
            continue
        rows.append(parse_row(rid, cells, schema or "unknown", section))
    return rows


def parse_matrix(path: Path) -> list[MatrixRow]:
    """Parse the matrix file at `path`."""
    return parse_matrix_text(path.read_text())


# ---------------------------------------------------------------------------
# Per-row signals + evidence resolution
# ---------------------------------------------------------------------------

_SNIPPET_RE = re.compile(r"snippets/[A-Za-z0-9_./-]+\.rs")
_RS_FILE_RE = re.compile(r"[A-Za-z0-9_./-]+\.rs")
_UNIVERSAL_RE = re.compile(r"\b(every fixture|implicit|many)\b", re.IGNORECASE)


def classify_evidence(
    evidence_ref: str,
    fixtures_dir: Path = DEFAULT_FIXTURES_DIR,
    repo_root: Path = REPO_ROOT,
) -> str:
    """Classify a row's evidence string into a legacy/runner evidence kind.

    Returns one of:
      - `runner-snippet`  -- cites `snippets/<id>.rs`; this is the runner
        signal, not legacy.
      - `universal`       -- `every fixture` / `many` / `implicit[ly]`;
        true by construction as long as any fixture runs.
      - `named-resolved`  -- cites a concrete `<file>.rs` that exists
        under the fixtures dir (bare name) or relative to the repo root
        (path); a passing-test surface.
      - `named-stale`     -- cites a `<file>.rs` that does not resolve.
      - `playground`      -- only cites `tests/fixtures/playground*`
        (experimentation, not committed coverage).
      - `none`            -- `--`, empty, or no resolvable evidence.
    """
    s = evidence_ref.strip()
    if not s or s == "--":
        return "none"

    if _SNIPPET_RE.search(s):
        return "runner-snippet"

    if _UNIVERSAL_RE.search(s):
        return "universal"

    # Concrete .rs citations that name a legacy test surface. Only two
    # forms count as fixture evidence: a BARE fixture filename resolved
    # under the fixtures dir, or a PATH explicitly under a `tests/` or
    # `fixtures/` directory. Source-file citations (`.../src/*.rs`, which
    # appear pervasively as rationale prose in the notes column) are NOT
    # coverage evidence and must not resolve.
    named_seen = False
    for m in _RS_FILE_RE.finditer(s):
        ref = m.group(0)
        if "snippets/" in ref or "playground" in ref:
            continue
        if "/" not in ref:
            named_seen = True
            if (fixtures_dir / ref).exists():
                return "named-resolved"
        elif "/src/" not in ref and ("/tests/" in ref or "/fixtures/" in ref):
            named_seen = True
            if (repo_root / ref).exists():
                return "named-resolved"
    if named_seen:
        return "named-stale"

    if "playground" in s:
        return "playground"

    return "none"


def is_adjudicated(row: MatrixRow) -> bool:
    """True iff the row carries a human out-of-scope adjudication.

    Adjudication = a `blocked-*` coverage token, the explicit
    `out-of-scope` token (a rustc-producible but non-IR-axis row), OR a
    Section-6 / `rejected`-expectation row whose parsed coverage is
    `uncovered`/`covered` carrying a non-empty reason that documents why
    no fixture exists. Detected from the parsed `(coverage_token,
    reason_text)` pair, not the taxonomy token alone.
    """
    tok = row.coverage_token
    if tok is not None and (tok.startswith("blocked-") or tok == "out-of-scope"):
        return True
    is_rejected_ctx = row.expectation == "rejected" or row.section == "6"
    if (
        is_rejected_ctx
        and tok in ("uncovered", "covered")
        and row.reason_text.strip() != ""
    ):
        return True
    return False


def per_row_signals(
    row: MatrixRow,
    sidecar_ids: set[str],
    fixtures_dir: Path = DEFAULT_FIXTURES_DIR,
    repo_root: Path = REPO_ROOT,
) -> RowSignals:
    """Compute the runner / legacy / adjudicated signals for one row."""
    kind = classify_evidence(row.evidence_ref, fixtures_dir, repo_root)
    runner = (row.id in sidecar_ids) or (kind == "runner-snippet")
    return RowSignals(
        runner=runner,
        legacy_kind=kind,
        adjudicated=is_adjudicated(row),
        evidence=row.evidence_ref.strip(),
    )


# ---------------------------------------------------------------------------
# Derivation
# ---------------------------------------------------------------------------


def derive_truth(row: MatrixRow, signals: RowSignals) -> tuple[str, str, str]:
    """Derive `(truth, evidence, reason)` for a coverage-axis row.

    Precedence (executable > passing test > human judgment > nothing):
      1. `runner`                      -> `validated-runner`
      2. legacy `named-resolved`       -> `validated-legacy`
      3. legacy `universal`            -> `validated-legacy-universal`
      4. `adjudicated`                 -> `out-of-scope` (reason carried)
      5. otherwise                     -> `needs-validation`

    A runner/legacy signal wins over an adjudication because it is a
    closed gap (an upgrade), never a demotion; adjudication only wins
    over the `needs-validation` fallback, so an adjudicated row is never
    derived to `needs-validation`.
    """
    if signals.runner:
        ev = signals.evidence if signals.legacy_kind == "runner-snippet" else "sidecar:rows.toml"
        return "validated-runner", ev, ""
    if signals.legacy_kind == "named-resolved":
        return "validated-legacy", signals.evidence, ""
    if signals.legacy_kind == "universal":
        return "validated-legacy-universal", signals.evidence, ""
    if signals.adjudicated:
        reason = row.reason_text.strip() or (row.coverage_token or "")
        return "out-of-scope", signals.evidence, reason
    return "needs-validation", "", ""


def _validator_truth(row: MatrixRow) -> str:
    """Map a Section-7 row's `exercised?` onto a validator-axis truth."""
    if row.exercised == "yes":
        return "validator-exercised"
    if row.exercised == "partial":
        return "validator-partial"
    return "validator-gap"


def _is_accepted_validator_residual(row: MatrixRow) -> bool:
    """True iff a non-`yes` Section-7 row is a documented-accepted backstop."""
    notes = row.notes.lower()
    return any(marker in notes for marker in _ACCEPTED_VALIDATOR_MARKERS)


# Truth values that count as "resolved" when rolling an aggregate header
# row up from its sub-axes. All are produced by the initial derivation
# (derive_truth) BEFORE the roll-up runs, so the roll-up is a single
# order-independent pass. `validated-aggregate` is deliberately excluded
# (no aggregate-of-aggregate exists in the matrix; excluding it keeps the
# pass from depending on iteration order).
_ROLLUP_RESOLVED = {
    "validated-runner",
    "validated-legacy",
    "validated-legacy-universal",
    "out-of-scope",
}

# A row id appearing inside another row's notes. Used ONLY to resolve an
# aggregate ALIAS (e.g. FT-021, whose coverage IS the Section-6 RJ-031
# family) when a row has no direct `<id>-*` sub-axes of its own.
_ID_IN_TEXT_RE = re.compile(r"\b[A-Z]{2,3}-[0-9]{3}\b")


def apply_aggregate_rollup(recon: Reconciliation, rows: list[MatrixRow]) -> None:
    """Resolve aggregate header rows from their sub-axes, in place.

    An aggregate row (e.g. `RV-005` = `Rvalue::Cast`, whose coverage is
    enumerated per sub-axis `RV-005-A..F`) carries no direct fixture of
    its own, so the initial derivation lands it on `needs-validation`. But
    the construct IS validated the moment >=1 enumerated sub-axis is
    resolved (runner / legacy / out-of-scope). This pass upgrades such a
    row to `validated-aggregate` -- an UPGRADE only: it touches
    `needs-validation` rows exclusively, so it can never demote a derived
    truth, and any genuinely-uncovered sub-axis stays on the worklist on
    its own row.

    Sub-axes are the rows whose id is `<aggregate-id>-<suffix>`. A row
    with no such direct sub-axes may instead ALIAS a sibling family named
    by id in its notes (FT-021 -> RJ-031); that family's sub-axes form the
    pool. Threshold: >=1 resolved sub-axis.
    """
    notes_by_id = {r.id: r.notes for r in rows if r.axis == "coverage"}
    cov_entries = [e for e in recon.entries if e.axis == "coverage"]
    for e in cov_entries:
        if e.truth != "needs-validation":
            continue
        pool = [s for s in cov_entries if s.id != e.id and s.id.startswith(e.id + "-")]
        if not pool:
            # No direct sub-axes: fall back to an aliased family named in notes.
            alias_ids = {
                aid
                for aid in _ID_IN_TEXT_RE.findall(notes_by_id.get(e.id, ""))
                if aid != e.id
            }
            for aid in alias_ids:
                pool.extend(s for s in cov_entries if s.id.startswith(aid + "-"))
        resolved = sorted(s.id for s in pool if s.truth in _ROLLUP_RESOLVED)
        if resolved:
            shown = ", ".join(resolved[:6]) + (" ..." if len(resolved) > 6 else "")
            e.truth = "validated-aggregate"
            e.evidence = f"rolled up from sub-axes: {shown}"
            e.reason = ""


def reconcile(
    rows: list[MatrixRow],
    sidecar_ids: set[str],
    fixtures_dir: Path = DEFAULT_FIXTURES_DIR,
    repo_root: Path = REPO_ROOT,
) -> Reconciliation:
    """Derive the authoritative ledger entries for every matrix row."""
    result = Reconciliation()
    for row in rows:
        if row.axis == "validator":
            result.entries.append(
                LedgerEntry(
                    id=row.id,
                    section=row.section,
                    axis="validator",
                    truth=_validator_truth(row),
                    evidence=row.notes.strip(),
                    matrix_current_status=row.exercised or "",
                    reason="",
                )
            )
            continue

        matrix_status = row.coverage_token or ("unparseable" if row.anomaly else "")
        if row.anomaly:
            result.anomalies.append(row)
            result.entries.append(
                LedgerEntry(
                    id=row.id,
                    section=row.section,
                    axis="coverage",
                    truth="anomaly",
                    evidence=row.evidence_ref.strip(),
                    matrix_current_status=matrix_status,
                    reason=row.anomaly,
                )
            )
            continue

        signals = per_row_signals(row, sidecar_ids, fixtures_dir, repo_root)
        truth, evidence, reason = derive_truth(row, signals)
        result.entries.append(
            LedgerEntry(
                id=row.id,
                section=row.section,
                axis="coverage",
                truth=truth,
                evidence=evidence,
                matrix_current_status=matrix_status,
                reason=reason,
            )
        )
    apply_aggregate_rollup(result, rows)
    return result


# ---------------------------------------------------------------------------
# Verdict + worklist
# ---------------------------------------------------------------------------


def needs_validation_worklist(recon: Reconciliation) -> dict[str, list[str]]:
    """Group the Sections-1-6 `needs-validation` ids by section."""
    grouped: dict[str, list[str]] = defaultdict(list)
    for e in recon.entries:
        if e.axis == "coverage" and e.truth == "needs-validation":
            grouped[e.section].append(e.id)
    return {sec: sorted(ids) for sec, ids in sorted(grouped.items())}


def validator_residual(rows: list[MatrixRow]) -> tuple[list[str], list[str]]:
    """Return `(open_residual_ids, accepted_backstop_ids)` for Section 7.

    `open_residual_ids` are non-`yes` validator rows that are NOT a
    documented-accepted backstop (the genuine F-a gap); accepted
    backstops are reported separately and do not block the verdict.
    """
    open_residual: list[str] = []
    accepted: list[str] = []
    for row in rows:
        if row.axis != "validator" or row.exercised == "yes":
            continue
        if _is_accepted_validator_residual(row):
            accepted.append(row.id)
        else:
            open_residual.append(row.id)
    return sorted(open_residual), sorted(accepted)


def compute_verdict(
    recon: Reconciliation, rows: list[MatrixRow]
) -> tuple[str, list[str], list[str]]:
    """Return `(verdict, s16_needs_ids, validator_open_residual)`.

    `verdict` is `pass` iff there are no Sections-1-6 `needs-validation`
    rows AND no open (non-accepted) Section-7 residual; else `not-yet`.
    """
    s16_needs = sorted(
        e.id
        for e in recon.entries
        if e.axis == "coverage" and e.truth == "needs-validation"
    )
    open_residual, _accepted = validator_residual(rows)
    verdict = "pass" if (not s16_needs and not open_residual) else "not-yet"
    return verdict, s16_needs, open_residual


# ---------------------------------------------------------------------------
# Ledger emission
# ---------------------------------------------------------------------------


def _sorted_entries(recon: Reconciliation) -> list[LedgerEntry]:
    return sorted(recon.entries, key=lambda e: e.id)


def render_ledger_json(recon: Reconciliation) -> str:
    """Render the authoritative ledger as deterministic JSON (keyed by id)."""
    obj = {
        e.id: {
            "id": e.id,
            "section": e.section,
            "axis": e.axis,
            "truth": e.truth,
            "evidence": e.evidence,
            "matrix_current_status": e.matrix_current_status,
            "reason": e.reason,
        }
        for e in _sorted_entries(recon)
    }
    return json.dumps(obj, indent=2, sort_keys=True) + "\n"


def render_ledger_md(recon: Reconciliation) -> str:
    """Render the authoritative ledger as a deterministic markdown table."""
    lines = [
        "# Bridge coverage ledger (derived -- DO NOT hand-edit)",
        "",
        "Authoritative per-row coverage truth, DERIVED by "
        "`scripts/python/coverage_drift.py` (regseq-driver-79, G1) from the "
        "bridge-coverage matrix + runner sidecar + legacy fixtures. Regenerate "
        "with `uv run python scripts/python/coverage_drift.py --write-ledger`; "
        "do not edit by hand. The matrix remains the human-owned prose source.",
        "",
        f"{CAVEAT}",
        "",
        "| id | section | axis | truth | matrix_current_status | evidence | reason |",
        "| -- | ------- | ---- | ----- | --------------------- | -------- | ------ |",
    ]
    for e in _sorted_entries(recon):
        evidence = e.evidence.replace("|", "\\|")
        reason = e.reason.replace("|", "\\|")
        lines.append(
            f"| {e.id} | {e.section} | {e.axis} | {e.truth} | "
            f"{e.matrix_current_status} | {evidence} | {reason} |"
        )
    return "\n".join(lines) + "\n"


def write_ledger(recon: Reconciliation, out_dir: Path = LEDGER_DIR) -> list[Path]:
    """Write `coverage_ledger.{json,md}` into `out_dir`. Idempotent."""
    json_path = out_dir / "coverage_ledger.json"
    md_path = out_dir / "coverage_ledger.md"
    json_path.write_text(render_ledger_json(recon))
    md_path.write_text(render_ledger_md(recon))
    return [json_path, md_path]


# ---------------------------------------------------------------------------
# G1b: schema-aware matrix coverage-cell de-drift write
# ---------------------------------------------------------------------------

# Where the coverage token sits, per schema, as a `_cells` index. For
# `section6main` the position varies per row (the token may be in the
# `status` cell, the notes cell, or a 6c extra cell), so it is located by
# scanning rather than fixed here.
_COVERAGE_CELL_INDEX = {"canonical": 3, "subaxis": 2, "interaction": 3}

# Derived truths that G1b will write a bare coverage token for. Adjudicated
# rows (truth `out-of-scope`) and anomalies are NEVER written -- their cell
# is human-owned and preserved verbatim.
_WRITABLE_TRUTHS = {
    "validated-runner",
    "validated-legacy",
    "validated-legacy-universal",
    "validated-aggregate",
    "needs-validation",
}


@dataclass
class MatrixWriteResult:
    """Outcome of a (dry-run or applied) matrix coverage-cell de-drift."""

    new_text: str
    changed: list[tuple[str, str, str]] = field(default_factory=list)  # (id, old, new)
    flagged: list[tuple[str, str]] = field(default_factory=list)  # (id, why)
    preserved_adjudicated: int = 0


def _section6_coverage_index(cells: list[str]) -> int | None:
    """Return the unique `_cells` index (>=2) carrying a coverage token.

    Mirrors the `section6main` parser: scan cells after the rejection
    variant for the single canonicalized taxonomy token (a bare `rejected`
    expectation never matches). Returns `None` if zero or several cells
    carry a token (the row is then flagged for manual review).
    """
    hits = [i for i in range(2, len(cells)) if canonicalize_coverage(cells[i])[0] is not None]
    return hits[0] if len(hits) == 1 else None


def _rewrite_coverage_cell(line: str, cell_index: int, new_token: str) -> str | None:
    """Replace ONLY the bare coverage token in `_cells`-index `cell_index`.

    Returns the rewritten line, or `None` if the cell does not hold a bare
    taxonomy token (i.e. it carries trailing content such as a
    parenthetical reason -> flag for manual review, never mangle). The
    field's surrounding whitespace is preserved so the diff stays local to
    the one token.
    """
    parts = line.split("|")
    raw_idx = cell_index + 1  # a row begins with '|', so parts[0] == ''
    if raw_idx >= len(parts):
        return None
    field = parts[raw_idx]
    stripped = field.strip().replace("`", "")
    m = _LEAD_TOKEN_RE.match(stripped)
    if not m:
        return None
    old_token = m.group(1)
    if stripped[len(old_token):].strip():
        # Trailing content beyond the bare token (e.g. a parenthetical) --
        # refuse to rewrite, flag instead.
        return None
    lead_ws = field[: len(field) - len(field.lstrip())]
    trail_ws = field[len(field.rstrip()):]
    parts[raw_idx] = f"{lead_ws}{new_token}{trail_ws}"
    return "|".join(parts)


def compute_matrix_write(
    text: str,
    recon: Reconciliation,
    rows: list[MatrixRow],
) -> MatrixWriteResult:
    """Compute the de-drifted matrix text (schema-aware, no I/O).

    For every Sections-1-6 coverage row whose derived truth differs from
    the matrix's visible coverage class, rewrite ONLY that row's coverage
    cell (per its table schema) to the bare derived token. NEVER touches:
    the `expectation` cell, any adjudicated row (preserved verbatim, incl.
    its reason text), Section-7 rows, or a cell carrying trailing content.
    Un-locatable coverage cells are flagged, not rewritten.
    """
    truth_by_id = {
        e.id: e.truth for e in recon.entries if e.axis == "coverage"
    }
    row_by_id = {r.id: r for r in rows}
    result = MatrixWriteResult(new_text=text)

    out_lines: list[str] = []
    section = ""
    schema: str | None = None
    for raw in text.splitlines():
        sm = _SECTION_RE.match(raw)
        if sm:
            section = sm.group(1)
            schema = None
            out_lines.append(raw)
            continue
        if _SUBSECTION_RE.match(raw) or not raw.lstrip().startswith("|"):
            out_lines.append(raw)
            continue
        cells = _cells(raw)
        header_key = tuple(c.lower() for c in cells)
        if header_key in HEADER_SCHEMAS:
            schema = HEADER_SCHEMAS[header_key]
            out_lines.append(raw)
            continue
        if _is_separator(cells) or not cells or not ROW_ID_RE.match(cells[0]):
            out_lines.append(raw)
            continue

        rid = cells[0]
        row = row_by_id.get(rid)
        if row is None or row.axis != "coverage" or section == "7":
            out_lines.append(raw)
            continue
        if row.anomaly is not None:
            result.flagged.append((rid, f"anomaly: {row.anomaly}"))
            out_lines.append(raw)
            continue
        if is_adjudicated(row):
            result.preserved_adjudicated += 1
            out_lines.append(raw)
            continue

        truth = truth_by_id.get(rid)
        target = _TRUTH_TO_CLASS.get(truth or "")
        if truth not in _WRITABLE_TRUTHS or target is None:
            result.flagged.append((rid, f"non-writable truth: {truth}"))
            out_lines.append(raw)
            continue

        if (schema or "") in _COVERAGE_CELL_INDEX:
            idx = _COVERAGE_CELL_INDEX[schema or ""]
        elif schema == "section6main":
            idx = _section6_coverage_index(cells)
        else:
            idx = None
        if idx is None:
            result.flagged.append((rid, f"coverage cell not locatable (schema {schema})"))
            out_lines.append(raw)
            continue

        current = canonicalize_coverage(cells[idx])[0]
        if current == target:
            out_lines.append(raw)
            continue
        rewritten = _rewrite_coverage_cell(raw, idx, target)
        if rewritten is None:
            result.flagged.append((rid, f"cell {idx} not a bare token: {cells[idx]!r}"))
            out_lines.append(raw)
            continue
        result.changed.append((rid, current or "<none>", target))
        out_lines.append(rewritten)

    trailing_nl = "\n" if text.endswith("\n") else ""
    result.new_text = "\n".join(out_lines) + trailing_nl
    return result


def write_matrix(
    matrix_path: Path,
    sidecar_ids: set[str],
    fixtures_dir: Path = DEFAULT_FIXTURES_DIR,
    repo_root: Path = REPO_ROOT,
    apply: bool = False,
) -> MatrixWriteResult:
    """De-drift the matrix's coverage cells in place (G1b).

    Computes the schema-aware rewrite, then enforces two safety
    invariants before any disk write: (1) re-deriving from the rewritten
    text yields byte-identical per-row truths (the write changed only
    visible tokens, never the derivation), and (2) the rewrite is
    idempotent (re-running on the new text produces no further change). If
    either fails, the function raises and writes nothing. With
    `apply=False` it is a pure dry run (no disk write).
    """
    text = matrix_path.read_text()
    rows = parse_matrix_text(text)
    recon = reconcile(rows, sidecar_ids, fixtures_dir, repo_root)
    res = compute_matrix_write(text, recon, rows)

    before = {e.id: e.truth for e in recon.entries}
    rows2 = parse_matrix_text(res.new_text)
    recon2 = reconcile(rows2, sidecar_ids, fixtures_dir, repo_root)
    after = {e.id: e.truth for e in recon2.entries}
    if before != after:
        diff = {k: (before.get(k), after.get(k)) for k in before | after if before.get(k) != after.get(k)}
        raise RuntimeError(f"matrix write changed the derivation (refusing to write): {diff}")

    res2 = compute_matrix_write(res.new_text, recon2, rows2)
    if res2.changed:
        raise RuntimeError(f"matrix write is not idempotent: {res2.changed}")

    if apply:
        matrix_path.write_text(res.new_text)
    return res


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

_TRUTH_TO_CLASS = {
    "validated-runner": "covered",
    "validated-legacy": "covered",
    "validated-legacy-universal": "covered",
    # An aggregate header row is covered VIA its sub-axes, not a direct
    # fixture of its own -- which is exactly what the matrix `partial`
    # status denotes ("a fixture interns ... a closely-related sub-shape").
    # Mapping it to `partial` keeps the matrix's honest aggregate marking
    # drift-consistent rather than over-claiming a bare `covered`.
    "validated-aggregate": "partial",
    "out-of-scope": "out-of-scope",
    "needs-validation": "uncovered",
}

_MATRIX_TO_CLASS = {
    "covered": "covered",
    "partial": "partial",
    "uncovered": "uncovered",
}


def _matrix_class(status: str) -> str:
    if status in _MATRIX_TO_CLASS:
        return _MATRIX_TO_CLASS[status]
    if status.startswith("blocked-"):
        return "out-of-scope"
    if status in ("", "unparseable"):
        return "unparseable"
    return status


def drift_rows(recon: Reconciliation) -> list[LedgerEntry]:
    """Coverage-axis rows where the matrix hand-set status disagrees with
    the derived truth (understatement or stale-legacy overstatement).

    Rows derived `out-of-scope` are excluded: that truth is read FROM the
    matrix's own human adjudication, so it can never drift against it. (A
    stale adjudication superseded by a runner row derives
    `validated-runner`, not `out-of-scope`, and IS surfaced as drift.)
    """
    out: list[LedgerEntry] = []
    for e in recon.entries:
        if e.axis != "coverage" or e.truth in ("anomaly", "out-of-scope"):
            continue
        if _TRUTH_TO_CLASS.get(e.truth, e.truth) != _matrix_class(e.matrix_current_status):
            out.append(e)
    return sorted(out, key=lambda e: e.id)


def print_report(
    recon: Reconciliation,
    rows: list[MatrixRow],
    sidecar_ids: set[str],
    matrix_path: Path,
    sidecar_path: Path,
    show_all: bool,
) -> None:
    """Print the read-only reconciliation summary + verdict."""
    coverage_entries = [e for e in recon.entries if e.axis == "coverage"]
    validator_entries = [e for e in recon.entries if e.axis == "validator"]
    truth_counts: dict[str, int] = defaultdict(int)
    for e in coverage_entries:
        truth_counts[e.truth] += 1
    validator_counts: dict[str, int] = defaultdict(int)
    for e in validator_entries:
        validator_counts[e.truth] += 1

    def rel(p: Path) -> str:
        try:
            return str(p.relative_to(REPO_ROOT))
        except ValueError:
            return str(p)

    print("=== bridge feature-complete reconciliation (READ-ONLY; G1) ===")
    print(f"matrix:  {rel(matrix_path)}")
    print(f"sidecar: {rel(sidecar_path)}")
    print()
    print(f"sidecar committed rows            : {len(sidecar_ids)}")
    print(f"matrix Sections-1-6 coverage rows : {len(coverage_entries)}")
    print(f"matrix Section-7 validator rows   : {len(validator_entries)}")
    print(f"unparseable Sections-1-6 rows     : {len(recon.anomalies)}")
    print()
    print("--- derived coverage truth (Sections 1-6) ---")
    for truth in (
        "validated-runner",
        "validated-legacy",
        "validated-legacy-universal",
        "validated-aggregate",
        "out-of-scope",
        "needs-validation",
        "anomaly",
    ):
        if truth_counts.get(truth):
            print(f"  {truth:<28}: {truth_counts[truth]:>4}")
    print()
    print("--- validator axis (Section 7) ---")
    for truth in ("validator-exercised", "validator-partial", "validator-gap"):
        if validator_counts.get(truth):
            print(f"  {truth:<28}: {validator_counts[truth]:>4}")

    drift = drift_rows(recon)
    print()
    print(f"=== DRIFT: matrix hand-set status != derived truth ({len(drift)}) ===")
    if drift:
        for e in drift:
            print(
                f"  {e.id:<12} matrix={e.matrix_current_status or '<none>':<24} "
                f"derived={e.truth}"
            )
    else:
        print("  (none)")

    worklist = needs_validation_worklist(recon)
    total_needs = sum(len(ids) for ids in worklist.values())
    print()
    print(f"=== needs-validation worklist ({total_needs}; grouped by section) ===")
    if worklist:
        for sec, ids in worklist.items():
            print(f"  Section {sec or '?'} ({len(ids)}): {', '.join(ids)}")
    else:
        print("  (none)")

    verdict, s16_needs, open_residual = compute_verdict(recon, rows)
    open_res, accepted = validator_residual(rows)
    print()
    print("=== FEATURE-COMPLETE VERDICT ===")
    print(f"  verdict: {verdict}")
    print(f"  Sections-1-6 needs-validation : {len(s16_needs)}")
    print(
        f"  Section-7 open residual       : {len(open_res)}"
        + (f" ({', '.join(open_res)})" if open_res else "")
    )
    print(
        f"  Section-7 accepted backstops  : {len(accepted)}"
        + (f" ({', '.join(accepted)})" if accepted else "")
    )
    print(f"  {CAVEAT}")

    if show_all and recon.anomalies:
        print()
        print("=== unparseable Sections-1-6 rows (reported by id) ===")
        for row in recon.anomalies:
            print(f"  {row.id:<12} {row.anomaly}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    ap.add_argument("--sidecar", type=Path, default=DEFAULT_SIDECAR)
    ap.add_argument("--fixtures-dir", type=Path, default=DEFAULT_FIXTURES_DIR)
    ap.add_argument(
        "--all",
        action="store_true",
        help="also list the unparseable rows by id",
    )
    ap.add_argument(
        "--write-ledger",
        action="store_true",
        help="write coverage_ledger.{json,md} under docs/discussions/regseq-model/",
    )
    ap.add_argument(
        "--write-matrix",
        action="store_true",
        help=(
            "G1b: schema-aware in-place de-drift of the matrix coverage cells "
            "(preserves expectation + adjudication cells; safety-checked)"
        ),
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="with --write-matrix, report the changes without writing the file",
    )
    args = ap.parse_args(argv)

    sidecar_ids = load_sidecar_ids(args.sidecar)
    rows = parse_matrix(args.matrix)
    recon = reconcile(rows, sidecar_ids, args.fixtures_dir, REPO_ROOT)

    print_report(recon, rows, sidecar_ids, args.matrix, args.sidecar, args.all)

    if args.write_ledger:
        paths = write_ledger(recon)
        print()
        for p in paths:
            try:
                print(f"wrote {p.relative_to(REPO_ROOT)}")
            except ValueError:
                print(f"wrote {p}")

    if args.write_matrix:
        res = write_matrix(
            args.matrix, sidecar_ids, args.fixtures_dir, REPO_ROOT, apply=not args.dry_run
        )
        print()
        verb = "would change" if args.dry_run else "changed"
        print(f"=== G1b matrix de-drift ({'DRY-RUN' if args.dry_run else 'APPLIED'}) ===")
        print(f"  coverage cells {verb}      : {len(res.changed)}")
        print(f"  adjudicated rows preserved : {res.preserved_adjudicated}")
        print(f"  flagged for manual review  : {len(res.flagged)}")
        for rid, old, new in res.changed:
            print(f"    {rid:<12} {old:<10} -> {new}")
        for rid, why in res.flagged:
            print(f"    FLAG {rid:<12} {why}")
        if not args.dry_run:
            try:
                print(f"  wrote {args.matrix.relative_to(REPO_ROOT)}")
            except ValueError:
                print(f"  wrote {args.matrix}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
