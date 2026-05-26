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


def test_verdict_passes_after_c_d_gate_close():
    rows, recon = _real_reconcile()
    verdict, s16_needs, open_residual = c.compute_verdict(recon, rows)
    # The Bucket-C reclassifications (-> out-of-scope) + Bucket-D aggregate
    # roll-up drove the Sections-1-6 needs-validation tail to empty; with the
    # Section-7 axis already closed (F-a), the gate now passes.
    assert verdict == "pass"
    assert s16_needs == []
    # F-a (VL-021 + VL-041 + VL-045) closed the Section-7 open residual to
    # empty; VL-018/019 remain accepted unreachable-by-construction backstops,
    # not blockers.
    open_res, accepted = c.validator_residual(rows)
    assert set(open_res) == set()
    assert set(accepted) == {"VL-018", "VL-019"}


# ---------------------------------------------------------------------------
# out-of-scope adjudication (Bucket-C non-`blocked-*` axis)
# ---------------------------------------------------------------------------


def test_out_of_scope_token_is_adjudicated(tmp_path):
    row = _row(
        "CT-099",
        section="4",
        expectation="positive",
        coverage_token="out-of-scope",
        reason_text="front-end axis",
        evidence_ref="--",
    )
    assert c.is_adjudicated(row) is True
    sig = c.per_row_signals(row, set(), tmp_path, REPO_ROOT)
    truth, _ev, reason = c.derive_truth(row, sig)
    assert truth == "out-of-scope"
    assert reason == "front-end axis"


# ---------------------------------------------------------------------------
# Bucket-D aggregate roll-up
# ---------------------------------------------------------------------------


def _entry(rid, truth, *, section="1"):
    return c.LedgerEntry(
        id=rid,
        section=section,
        axis="coverage",
        truth=truth,
        evidence="",
        matrix_current_status="",
    )


def test_rollup_aggregate_with_one_resolved_subaxis():
    recon = c.Reconciliation(
        entries=[
            _entry("RV-099", "needs-validation"),
            _entry("RV-099-A", "validated-runner"),
            _entry("RV-099-B", "needs-validation"),
        ]
    )
    rows = [_row("RV-099"), _row("RV-099-A"), _row("RV-099-B")]
    c.apply_aggregate_rollup(recon, rows)
    by_id = {e.id: e for e in recon.entries}
    assert by_id["RV-099"].truth == "validated-aggregate"
    assert "RV-099-A" in by_id["RV-099"].evidence
    # A genuinely-uncovered sub-axis is NOT hidden: it stays on the worklist.
    assert by_id["RV-099-B"].truth == "needs-validation"


def test_rollup_never_demotes_a_resolved_row():
    recon = c.Reconciliation(
        entries=[
            _entry("RV-098", "out-of-scope"),
            _entry("RV-098-A", "validated-runner"),
        ]
    )
    c.apply_aggregate_rollup(recon, [_row("RV-098"), _row("RV-098-A")])
    assert {e.id: e.truth for e in recon.entries}["RV-098"] == "out-of-scope"


def test_rollup_no_resolved_subaxis_stays_needs_validation():
    recon = c.Reconciliation(
        entries=[
            _entry("RV-097", "needs-validation"),
            _entry("RV-097-A", "needs-validation"),
        ]
    )
    c.apply_aggregate_rollup(recon, [_row("RV-097"), _row("RV-097-A")])
    assert {e.id: e.truth for e in recon.entries}["RV-097"] == "needs-validation"


def test_rollup_alias_family_from_notes_when_no_direct_subaxes():
    # FT-021-shaped: no direct `FT-099-*` sub-axes; coverage IS the RJ-099 family.
    recon = c.Reconciliation(
        entries=[
            _entry("FT-099", "needs-validation", section="3"),
            _entry("RJ-099-A", "validated-runner", section="6"),
        ]
    )
    rows = [
        c.MatrixRow(
            id="FT-099", section="3", schema="canonical", axis="coverage",
            notes="Section 6 RJ-099 sub-axes",
        ),
        c.MatrixRow(id="RJ-099-A", section="6", schema="subaxis", axis="coverage"),
    ]
    c.apply_aggregate_rollup(recon, rows)
    assert {e.id: e.truth for e in recon.entries}["FT-099"] == "validated-aggregate"


def test_real_matrix_bucket_c_out_of_scope_and_d_aggregate():
    _rows, recon = _real_reconcile()
    by_id = {e.id: e for e in recon.entries}
    bucket_c = (
        "TM-006-F", "TM-006-G", "TM-006-H", "CT-004",
        "RJ-010", "RJ-011", "RJ-012", "RJ-023", "RJ-030",
    )
    for rid in bucket_c:
        assert by_id[rid].truth == "out-of-scope", (rid, by_id[rid].truth)
        assert by_id[rid].reason.strip() != ""
    bucket_d = (
        "RV-005", "RV-006", "RV-007", "RV-009", "ST-003",
        "TM-006", "FT-021", "RJ-028", "RJ-031", "RJ-032",
    )
    for rid in bucket_d:
        assert by_id[rid].truth == "validated-aggregate", (rid, by_id[rid].truth)


def test_real_matrix_no_needs_validation_and_zero_drift():
    _rows, recon = _real_reconcile()
    needs = [e.id for e in recon.entries if e.axis == "coverage" and e.truth == "needs-validation"]
    assert needs == []
    assert c.drift_rows(recon) == []


# ---------------------------------------------------------------------------
# G1b: schema-aware matrix coverage-cell de-drift write
# ---------------------------------------------------------------------------


_G1B_SYNTH = """
# Section 1: x
| id | construct | expectation | status | fixture_path | notes |
| -- | --- | --- | --- | --- | --- |
| AA-001 | x | positive | uncovered | -- | n |
| AA-002 | x | positive | uncovered (legacy note) | -- | n |
# Section 6: x
| id | rejection variant | source trigger | status | notes |
| -- | --- | --- | --- | --- |
| RJ-097 | `Box` | trig | rejected | blocked-rustc-infeasible (by design) |
| RJ-098 | `CVoid` | trig | rejected | uncovered |
"""


def _synth_write(tmp_path):
    rows = c.parse_matrix_text(_G1B_SYNTH)
    recon = c.reconcile(rows, {"AA-001", "AA-002", "RJ-098"}, tmp_path, REPO_ROOT)
    return rows, recon, c.compute_matrix_write(_G1B_SYNTH, recon, rows)


def test_g1b_rewrites_understated_canonical_and_section6_notes_cells(tmp_path):
    _rows, _recon, res = _synth_write(tmp_path)
    changed = {rid: (old, new) for rid, old, new in res.changed}
    # canonical status cell understated; section6 6a coverage-in-notes cell.
    assert changed["AA-001"] == ("uncovered", "covered")
    assert changed["RJ-098"] == ("uncovered", "covered")
    # The section6 expectation token must survive untouched.
    assert "| rejected | covered |" in res.new_text


def test_g1b_preserves_adjudicated_rows(tmp_path):
    _rows, _recon, res = _synth_write(tmp_path)
    assert all(rid != "RJ-097" for rid, _o, _n in res.changed)
    assert res.preserved_adjudicated == 1
    assert "blocked-rustc-infeasible (by design)" in res.new_text


def test_g1b_flags_cell_with_trailing_content_never_mangles(tmp_path):
    _rows, _recon, res = _synth_write(tmp_path)
    # AA-002's status cell carries a parenthetical -> refuse to rewrite, flag.
    assert all(rid != "AA-002" for rid, _o, _n in res.changed)
    assert any(rid == "AA-002" for rid, _why in res.flagged)
    assert "uncovered (legacy note)" in res.new_text


def test_g1b_is_idempotent_on_synthetic(tmp_path):
    _rows, _recon, res = _synth_write(tmp_path)
    rows2 = c.parse_matrix_text(res.new_text)
    recon2 = c.reconcile(rows2, {"AA-001", "AA-002", "RJ-098"}, tmp_path, REPO_ROOT)
    res2 = c.compute_matrix_write(res.new_text, recon2, rows2)
    assert res2.changed == []


def test_g1b_real_matrix_write_is_safe_idempotent_and_structurally_intact():
    sidecar = c.load_sidecar_ids(c.DEFAULT_SIDECAR)
    text = c.DEFAULT_MATRIX.read_text()
    rows = c.parse_matrix_text(text)
    recon = c.reconcile(rows, sidecar)
    res = c.compute_matrix_write(text, recon, rows)
    # The committed matrix is already de-drifted: 0 changes, 0 flags.
    assert res.changed == []
    assert res.flagged == []
    # Structural integrity: the write invents/drops no row id, and re-deriving
    # from the (identical) text yields identical truths.
    rows2 = c.parse_matrix_text(res.new_text)
    assert {r.id for r in rows2} == {r.id for r in rows}
    recon2 = c.reconcile(rows2, sidecar)
    assert {e.id: e.truth for e in recon2.entries} == {
        e.id: e.truth for e in recon.entries
    }
    # write_matrix(apply=False) runs the re-derive + idempotence guards and
    # must not raise.
    c.write_matrix(c.DEFAULT_MATRIX, sidecar, apply=False)
