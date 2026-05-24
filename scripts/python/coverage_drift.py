#!/usr/bin/env python3
"""Read-only coverage-drift report: bridge-coverage matrix vs runner sidecar.

Coverage is tracked by two mechanisms that are not auto-reconciled:

  1. The bridge-coverage **matrix** (`bridge-coverage-matrix.md`) per-row
     `status` column. Backfilled by hand per task; reflects legacy
     `test-ir-fixtures` coverage plus whatever runner rows a task chose
     to backfill.
  2. The bridge-coverage-**runner** sidecar
     (`matrix_metadata/rows.toml` + `snippets/`). Every committed row is
     a passing experiment.

These sets are largely disjoint: the bulk F4 corpus committed many
sidecar rows without backfilling the matrix `status`, so the matrix
systematically understates runner coverage. This tool reports the
discrepancy. It is READ-ONLY: it never edits the matrix or sidecar.

A *derived* reconciliation (rewriting matrix status from the sidecar +
a legacy-fixture scan) is deferred to gap-inventory Category 5; doing
it now would couple to a still-moving status taxonomy and force a
mid-stream "status is derived, don't hand-edit" cutover. This report is
the low-commitment slice: it surfaces the drift and would catch future
drift early, without locking in schema.

Exit code is always 0 (report, not gate). CI gating is Category 5
(Mode C).
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

# Closed status taxonomy as DECLARED in the matrix doc "How to read"
# section (bridge-coverage-matrix.md, status bullet list). Kept in sync
# with the legend deliberately: a row carrying a status value NOT in
# this set parses to zero status cells and is reported as "unparseable"
# rather than silently accepted -- that surfaces legend drift (e.g., a
# task introducing a new status class without declaring it in the
# legend, as happened transiently with `blocked-bridge-itemmissinginstance`).
STATUS_VALUES = {
    "covered",
    "partial",
    "uncovered",
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

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MATRIX = REPO_ROOT / "docs/discussions/regseq-model/bridge-coverage-matrix.md"
DEFAULT_SIDECAR = REPO_ROOT / "crates/bridge-coverage-runner/matrix_metadata/rows.toml"


def load_sidecar(path: Path) -> dict[str, str]:
    """Return {row_id: status} for every committed sidecar row."""
    with path.open("rb") as f:
        data = tomllib.load(f)
    rows = {}
    for row in data.get("row", []):
        rid = row.get("id")
        if rid:
            rows[rid] = row.get("status", "covered")
    return rows


def _cells(line: str) -> list[str]:
    """Split a markdown table row into stripped cells (backticks removed)."""
    parts = line.split("|")
    # leading/trailing empties from the bounding pipes
    if parts and parts[0].strip() == "":
        parts = parts[1:]
    if parts and parts[-1].strip() == "":
        parts = parts[:-1]
    return [c.strip().strip("`").strip() for c in parts]


def parse_matrix(path: Path) -> tuple[dict[str, str], list[str]]:
    """Return ({row_id: status}, [unparseable_row_ids]).

    A line is a coverage row iff its first cell matches ROW_ID_RE. Status
    is the unique cell whose exact value is in STATUS_VALUES. Rows with
    zero or multiple status cells are reported as unparseable rather than
    guessed.
    """
    statuses: dict[str, str] = {}
    unparseable: list[str] = []
    for raw in path.read_text().splitlines():
        if not raw.lstrip().startswith("|"):
            continue
        cells = _cells(raw)
        if not cells:
            continue
        rid = cells[0]
        if not ROW_ID_RE.match(rid):
            continue
        hits = [c for c in cells[1:] if c in STATUS_VALUES]
        if len(hits) == 1:
            statuses[rid] = hits[0]
        else:
            unparseable.append(rid)
    return statuses, unparseable


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    ap.add_argument("--sidecar", type=Path, default=DEFAULT_SIDECAR)
    ap.add_argument(
        "--all",
        action="store_true",
        help="also list matrix-only (legacy-covered) and unparseable rows",
    )
    args = ap.parse_args()

    sidecar = load_sidecar(args.sidecar)
    matrix, unparseable = parse_matrix(args.matrix)

    sidecar_ids = set(sidecar)
    matrix_covered = {rid for rid, st in matrix.items() if st == "covered"}

    # Buckets.
    consistent = sorted(sidecar_ids & matrix_covered)
    drift = sorted(rid for rid in sidecar_ids if matrix.get(rid) != "covered")
    no_matrix_row = sorted(rid for rid in sidecar_ids if rid not in matrix)
    matrix_only = sorted(matrix_covered - sidecar_ids)

    print("=== coverage-drift report (READ-ONLY) ===")
    print(f"matrix:  {args.matrix.relative_to(REPO_ROOT)}")
    print(f"sidecar: {args.sidecar.relative_to(REPO_ROOT)}")
    print()
    print(f"sidecar committed rows      : {len(sidecar_ids)}")
    print(f"matrix rows parsed          : {len(matrix)} ({len(unparseable)} unparseable)")
    print(f"matrix rows marked covered  : {len(matrix_covered)}")
    print()
    print("  consistent (sidecar + matrix=covered) : {:>4}".format(len(consistent)))
    print("  DRIFT (sidecar, matrix!=covered)       : {:>4}".format(len(drift)))
    print("      of which absent from matrix        : {:>4}".format(len(no_matrix_row)))
    print("  matrix-only (covered, not in sidecar)  : {:>4}".format(len(matrix_only)))
    print()

    print("=== DRIFT: runner-covered rows the matrix does NOT mark covered ===")
    if drift:
        for rid in drift:
            st = matrix.get(rid, "<absent>")
            print(f"  {rid:<12} matrix status: {st}")
    else:
        print("  (none)")

    if args.all:
        print()
        print("=== matrix-only (likely legacy test-ir-fixtures coverage) ===")
        for rid in matrix_only:
            print(f"  {rid}")
        print()
        print("=== unparseable matrix rows (0 or >1 status cells) ===")
        for rid in unparseable:
            print(f"  {rid}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
