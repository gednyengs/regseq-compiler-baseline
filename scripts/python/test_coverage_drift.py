"""Test suite for the feature-complete-gate reconciliation engine (G1).

Covers B1-B19 from the regseq-driver-79 task spec: schema-aware parser
shapes, per-row derivation with adjudication preservation, and
integration checks against the real matrix + sidecar at HEAD.

Run with `uv run pytest`. This file imports `coverage_drift` from the
same directory (configured via `[tool.pytest.ini_options] pythonpath`);
the tool itself stays stdlib + tomllib only.
"""

from __future__ import annotations

import ast
from pathlib import Path

import coverage_drift as c


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _parse_one(text: str) -> c.MatrixRow:
    """Parse a single-data-row inline table and return that row."""
    rows = c.parse_matrix_text(text)
    assert len(rows) == 1, f"expected one row, got {[r.id for r in rows]}"
    return rows[0]


def _row(
    rid: str = "XX-001",
    *,
    section: str = "1",
    schema: str = "canonical",
    axis: str = "coverage",
    expectation: str | None = None,
    coverage_token: str | None = None,
    reason_text: str = "",
    evidence_ref: str = "",
) -> c.MatrixRow:
    return c.MatrixRow(
        id=rid,
        section=section,
        schema=schema,
        axis=axis,
        expectation=expectation,
        coverage_token=coverage_token,
        reason_text=reason_text,
        evidence_ref=evidence_ref,
    )


REPO_ROOT = c.REPO_ROOT


# ---------------------------------------------------------------------------
# Parser unit tests (one synthetic table per schema shape)
# ---------------------------------------------------------------------------


def test_b1_canonical_row_coverage_from_status_evidence_from_fixture_path():
    row = _parse_one(
        """
# Section 1: x
| id | construct | expectation | status | fixture_path | notes |
| -- | --- | --- | --- | --- | --- |
| TY-006 | `Ty::Str` | positive | covered | `diag_logging.rs` | note |
"""
    )
    assert row.schema == "canonical"
    assert row.axis == "coverage"
    assert row.coverage_token == "covered"
    assert row.evidence_ref == "diag_logging.rs"
    assert row.expectation == "positive"


def test_b2_subaxis_row_coverage_from_status_evidence_from_notes():
    row = _parse_one(
        """
# Section 1: x
| id | sub-construct | status | notes |
| -- | --- | --- | --- |
| TY-003-C | `IntKind::I32` | covered | `instance_completeness.rs` (Point.x) |
"""
    )
    assert row.schema == "subaxis"
    assert row.coverage_token == "covered"
    assert row.evidence_ref == "instance_completeness.rs (Point.x)"


def test_b3_interaction_row_coverage_from_status_evidence_from_fixture_or_bugtask():
    row = _parse_one(
        """
# Section 5: x
| id | interaction | expectation | status | fixture_path / bug-task | notes |
| -- | --- | --- | --- | --- | --- |
| IX-001 | GenericArgs x Call | positive | uncovered | -- | driver-60 B4 |
"""
    )
    assert row.schema == "interaction"
    assert row.coverage_token == "uncovered"
    assert row.evidence_ref == "--"


def test_b4_section6_6a_status_rejected_coverage_in_notes():
    row = _parse_one(
        """
# Section 6: x
| id | rejection variant | source trigger | status | notes |
| -- | --- | --- | --- | --- |
| RJ-001 | `Box` | Box<T> field | rejected | uncovered |
"""
    )
    assert row.schema == "section6main"
    assert row.coverage_token == "uncovered"
    assert row.expectation == "rejected"
    assert row.reason_text == ""


def test_b5_section6_6a_parenthetical_reason_stripped():
    row = _parse_one(
        """
# Section 6: x
| id | rejection variant | source trigger | status | notes |
| -- | --- | --- | --- | --- |
| RJ-004 | `PlaceProjection` | proj | rejected | uncovered (no known surface) |
"""
    )
    assert row.coverage_token == "uncovered"
    assert row.reason_text == "no known surface"


def test_b6_section6_6b_status_is_coverage():
    row = _parse_one(
        """
# Section 6: x
| id | rejection variant | source trigger | status | notes |
| -- | --- | --- | --- | --- |
| RJ-009 | `Param` | bucket-4 carve-out | covered | regseq-driver-72 sub-task C |
"""
    )
    assert row.coverage_token == "covered"


def test_b7_section6_6c_extra_cell_blocked_token_adjudicated():
    row = _parse_one(
        """
# Section 6: x
| id | rejection variant | source trigger | status | notes |
| -- | --- | --- | --- | --- |
| RJ-014 | `Error` | rustc Error type | rejected | blocked-rustc-infeasible | rustc rejects first |
"""
    )
    assert row.coverage_token == "blocked-rustc-infeasible"
    assert c.is_adjudicated(row) is True


def test_b8_section6_subaxis_coverage_from_status():
    row = _parse_one(
        """
# Section 6: x
| id | sub-construct | status | notes |
| -- | --- | --- | --- |
| RJ-028-A | `Input(usize)` | uncovered | non-C-ABI input |
"""
    )
    assert row.schema == "subaxis"
    assert row.coverage_token == "uncovered"


def test_b9_section7_row_tagged_validator_axis_not_coverage():
    text = """
# Section 1: x
| id | construct | expectation | status | fixture_path | notes |
| -- | --- | --- | --- | --- | --- |
| TY-006 | `Ty::Str` | positive | covered | `diag_logging.rs` | n |
# Section 7: x
| id | variant | exercised? | notes |
| -- | --- | --- | --- |
| VL-001 | `DanglingId` | yes | model_tests.rs:invariant_1 |
"""
    rows = c.parse_matrix_text(text)
    by_id = {r.id: r for r in rows}
    assert by_id["VL-001"].axis == "validator"
    assert by_id["VL-001"].exercised == "yes"
    assert by_id["VL-001"].coverage_token is None
    # The Section-7 row must not be a Sections-1-6 coverage row.
    coverage_rows = [r for r in rows if r.axis == "coverage"]
    assert "VL-001" not in {r.id for r in coverage_rows}
    assert "TY-006" in {r.id for r in coverage_rows}


# ---------------------------------------------------------------------------
# Derivation unit tests (synthetic rows)
# ---------------------------------------------------------------------------


def test_b10_runner_signal_id_in_sidecar_yields_validated_runner(tmp_path):
    row = _row("TY-018", coverage_token="uncovered", evidence_ref="--")
    sig = c.per_row_signals(row, {"TY-018"}, tmp_path, REPO_ROOT)
    assert sig.runner is True
    assert c.derive_truth(row, sig)[0] == "validated-runner"


def test_b11_legacy_named_universal_and_stale(tmp_path):
    (tmp_path / "no_op.rs").write_text("// fixture\n")

    named = _row("BD-001", coverage_token="covered", evidence_ref="no_op.rs")
    sig = c.per_row_signals(named, set(), tmp_path, REPO_ROOT)
    assert sig.legacy_kind == "named-resolved"
    assert c.derive_truth(named, sig)[0] == "validated-legacy"

    universal = _row("TY-004", coverage_token="partial", evidence_ref="many")
    sig = c.per_row_signals(universal, set(), tmp_path, REPO_ROOT)
    assert sig.legacy_kind == "universal"
    assert c.derive_truth(universal, sig)[0] == "validated-legacy-universal"

    stale = _row("BD-009", coverage_token="covered", evidence_ref="ghost_missing.rs")
    sig = c.per_row_signals(stale, set(), tmp_path, REPO_ROOT)
    assert sig.legacy_kind == "named-stale"
    assert c.derive_truth(stale, sig)[0] == "needs-validation"


def test_b11a_playground_evidence_is_not_validated(tmp_path):
    row = _row("FT-019", coverage_token="covered", evidence_ref="playground (uncommitted)")
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    assert sig.legacy_kind == "playground"
    assert sig.runner is False
    assert c.derive_truth(row, sig)[0] == "needs-validation"

    row2 = _row("IX-099", coverage_token="covered", evidence_ref="tests/fixtures/playground/foo.rs")
    sig2 = c.per_row_signals(row2, set(), tmp_path, REPO_ROOT)
    assert c.derive_truth(row2, sig2)[0] == "needs-validation"


def test_b11b_runner_snippet_evidence_folds_into_runner(tmp_path):
    row = _row(
        "IX-004",
        coverage_token="covered",
        evidence_ref="crates/bridge-coverage-runner/snippets/IX-004.rs",
    )
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    assert sig.legacy_kind == "runner-snippet"
    assert sig.runner is True
    assert c.derive_truth(row, sig)[0] == "validated-runner"


def test_b12_adjudication_preserved_blocked_with_no_signal(tmp_path):
    row = _row(
        "RJ-024",
        expectation="rejected",
        coverage_token="blocked-rustc-infeasible",
        reason_text="feature gated",
        evidence_ref="--",
    )
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    assert sig.runner is False
    truth, _evidence, reason = c.derive_truth(row, sig)
    assert truth == "out-of-scope"
    assert reason == "feature gated"


def test_b12b_reason_bearing_uncovered_rejected_is_adjudicated(tmp_path):
    row = _row(
        "RJ-004",
        section="6",
        schema="section6main",
        expectation="rejected",
        coverage_token="uncovered",
        reason_text="no known surface",
        evidence_ref="uncovered (no known surface)",
    )
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    truth, _ev, reason = c.derive_truth(row, sig)
    assert truth == "out-of-scope"
    assert reason == "no known surface"


def test_b13_no_signal_yields_needs_validation(tmp_path):
    row = _row("TY-005", coverage_token="uncovered", evidence_ref="--")
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    assert sig.runner is False
    assert sig.adjudicated is False
    assert c.derive_truth(row, sig)[0] == "needs-validation"


# ---------------------------------------------------------------------------
# Integration tests (real matrix + sidecar at HEAD)
# ---------------------------------------------------------------------------


def _real_reconcile() -> tuple[list[c.MatrixRow], c.Reconciliation]:
    rows = c.parse_matrix(c.DEFAULT_MATRIX)
    sidecar = c.load_sidecar_ids(c.DEFAULT_SIDECAR)
    recon = c.reconcile(rows, sidecar)
    return rows, recon


def test_b14_every_sections_1_6_row_parses_and_known_four_classify():
    rows, recon = _real_reconcile()
    # 0 unparseable Sections-1-6 rows (or only explicitly-listed anomalies).
    assert recon.anomalies == [], [r.id for r in recon.anomalies]
    by_id = {e.id: e for e in recon.entries}
    for rid in ("CT-002", "RJ-004", "RJ-007", "RJ-008"):
        assert rid in by_id
        assert by_id[rid].truth != "anomaly"


def test_b15_ledger_accounts_every_row_exactly_once():
    rows, recon = _real_reconcile()
    coverage = [r for r in rows if r.axis == "coverage"]
    validator = [r for r in rows if r.axis == "validator"]
    assert len(recon.entries) == len(coverage) + len(validator)
    ids = [e.id for e in recon.entries]
    assert len(ids) == len(set(ids)), "an id appears twice in the ledger"
    matrix_ids = {r.id for r in rows}
    assert set(ids) == matrix_ids, "ledger invents or drops an id"
    assert len(validator) == 61


def test_b16_all_sidecar_ids_classify_validated_runner():
    rows, recon = _real_reconcile()
    sidecar = c.load_sidecar_ids(c.DEFAULT_SIDECAR)
    # The committed sidecar grows as coverage batches land (146 at
    # driver-79; 174 after the F-b authoring track). A monotonic floor is
    # a loader-sanity guard; the per-id loop below is the real invariant
    # (every committed sidecar row classifies as validated-runner).
    assert len(sidecar) >= 146
    by_id = {e.id: e for e in recon.entries}
    for rid in sidecar:
        assert rid in by_id, f"sidecar id {rid} absent from the matrix/ledger"
        assert by_id[rid].truth == "validated-runner", (rid, by_id[rid].truth)


def test_b17_known_adjudications_preserved():
    _rows, recon = _real_reconcile()
    by_id = {e.id: e for e in recon.entries}
    expected = {
        "RJ-024": "blocked-rustc-infeasible",
        "RJ-032-F": "blocked-unimplemented-phase",
        "OP-003-I": "blocked-bridge-rejects-by-design",
    }
    for rid, klass in expected.items():
        assert by_id[rid].truth == "out-of-scope", (rid, by_id[rid].truth)
        assert by_id[rid].matrix_current_status == klass
        assert klass in by_id[rid].reason


def test_b18_write_ledger_is_idempotent(tmp_path):
    _rows, recon = _real_reconcile()
    first_json = c.render_ledger_json(recon)
    first_md = c.render_ledger_md(recon)
    c.write_ledger(recon, tmp_path)
    json_path = tmp_path / "coverage_ledger.json"
    md_path = tmp_path / "coverage_ledger.md"
    assert json_path.read_text() == first_json
    assert md_path.read_text() == first_md
    # A second write produces byte-identical files.
    json_before = json_path.read_text()
    md_before = md_path.read_text()
    c.write_ledger(recon, tmp_path)
    assert json_path.read_text() == json_before
    assert md_path.read_text() == md_before


def test_b19_tool_runtime_imports_are_stdlib_plus_tomllib_only():
    import sys

    source = Path(c.__file__).read_text()
    tree = ast.parse(source)
    top_modules: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                top_modules.add(alias.name.split(".")[0])
        elif isinstance(node, ast.ImportFrom):
            if node.level == 0 and node.module:
                top_modules.add(node.module.split(".")[0])
    stdlib = set(sys.stdlib_module_names)
    assert "tomllib" in stdlib
    non_stdlib = top_modules - stdlib
    assert non_stdlib == set(), f"tool imports non-stdlib modules: {non_stdlib}"


def test_verdict_is_honest_not_yet_with_residuals():
    rows, recon = _real_reconcile()
    verdict, s16_needs, open_residual = c.compute_verdict(recon, rows)
    # At HEAD the gate is not yet passable: there is a real
    # needs-validation tail over Sections 1-6.
    assert verdict == "not-yet"
    # The remaining not-yet driver is the Sections-1-6 needs-validation tail
    # (the Bucket-C / Bucket-D rows); F-a closed the Section-7 residual.
    assert len(s16_needs) > 0
    # F-a (VL-021 + VL-041 + VL-045) closed the Section-7 open residual to
    # empty; VL-018/019 remain accepted unreachable-by-construction backstops,
    # not blockers.
    open_res, accepted = c.validator_residual(rows)
    assert set(open_res) == set()
    assert set(accepted) == {"VL-018", "VL-019"}
