//! The codegen-coverage ledger (Doc 3 sections 5-6): aggregate the
//! matrix row universe against the fixture results into a per-row
//! pass/fail/n-a table, detect drift in both directions, compute the
//! phase verdict, and write `coverage_ledger.{json,md}`.
//!
//! The committed ledger is a **pure function of `(matrix x fixture
//! results)`** -- no wall-clock or run metadata -- so re-runs are
//! byte-stable (Doc 3 section 4) and a `git` diff reflects only real
//! coverage drift, not run-timestamp noise (Doc 3 section 5).
//!
//! Doc 3 section 6 verdict rules:
//! - `out-of-scope` rows never gate.
//! - `deferred` rows are not gated this phase (deferred by design).
//! - any remaining `unspecced` row forces `not-pass` (an unresolved
//!   in-scope gap).
//! - `pass` iff no `unspecced` rows remain AND every `covered` row has at
//!   least one green fixture AND no fixture FAILed AND no
//!   green-without-covered drift; else `not-pass`.

use std::path::Path;

use serde::Serialize;

use crate::matrix::{MatrixRow, Status};
use crate::pipeline::FixtureResult;

/// The Phase-1 exit-gate verdict.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhaseVerdict {
    /// Gate is green: every in-scope row resolved + covered + green.
    Pass,
    /// Gate is red: open gaps, missing coverage, drift, or a fixture FAIL.
    NotPass,
}

/// Per-row aggregation result.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RowResult {
    /// A `covered` row with >= 1 green fixture and no covering FAIL.
    Pass,
    /// A `covered` row with a covering FAIL or no green fixture.
    Fail,
    /// Not gated this phase (`deferred` / `unspecced` / `out-of-scope`).
    #[serde(rename = "n/a")]
    NotApplicable,
}

/// One row of the ledger: a matrix row plus its coverage outcome.
#[derive(Debug, Serialize)]
pub struct LedgerRow {
    /// Doc-1 row ID.
    pub id: String,
    /// Design status (verbatim from the matrix).
    pub status: Status,
    /// Names of fixtures that declare this row in their `covers`.
    pub fixtures: Vec<String>,
    /// Aggregated per-row result.
    pub result: RowResult,
}

/// Host-`cc` reproducibility record (Doc 3 section 7).
#[derive(Debug, Clone, Serialize)]
pub struct ToolchainInfo {
    /// The `cc` path/command used.
    pub cc: String,
    /// First line of `cc --version`.
    pub cc_version: String,
}

/// A green fixture covering a row the matrix does not mark `covered`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GreenWithoutCovered {
    /// The green fixture's name.
    pub fixture: String,
    /// The covered row ID.
    pub id: String,
    /// The row's (non-`covered`) status.
    pub status: Status,
}

/// The two soft drift directions recorded in the ledger (the third, an
/// absent row ID, is a hard [`LedgerError`]).
#[derive(Debug, Serialize, Default)]
pub struct Drift {
    /// `covered` rows with no green fixture (covered-without-green).
    pub covered_without_green: Vec<String>,
    /// Green fixtures whose `covers` names a non-`covered` row.
    pub green_without_covered: Vec<GreenWithoutCovered>,
}

/// The full ledger artifact.
#[derive(Debug, Serialize)]
pub struct Ledger {
    /// The phase verdict.
    pub verdict: PhaseVerdict,
    /// Per-matrix-row coverage rows (sorted by ID).
    pub rows: Vec<LedgerRow>,
    /// Drift record (both soft directions).
    pub drift: Drift,
    /// Host toolchain reproducibility record.
    pub toolchain: ToolchainInfo,
}

/// Errors from ledger aggregation or writing.
#[derive(Debug)]
pub enum LedgerError {
    /// A fixture's `covers` names a row ID absent from the matrix (the
    /// fixture invented a row) -- a hard drift error.
    AbsentRowId {
        /// The fixture that referenced the unknown ID.
        fixture: String,
        /// The unknown row ID.
        id: String,
    },
    /// I/O error writing the ledger files.
    Io(std::io::Error),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::AbsentRowId { fixture, id } => write!(
                f,
                "drift-hard-fail: fixture {fixture:?} covers row {id:?} \
                 which is absent from the matrix"
            ),
            LedgerError::Io(err) => write!(f, "ledger I/O error: {err}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(value: std::io::Error) -> Self {
        LedgerError::Io(value)
    }
}

/// Aggregate `(matrix rows x fixture results)` into the ledger + verdict.
///
/// Returns [`LedgerError::AbsentRowId`] if any fixture covers a row ID
/// not present in the matrix (the first Doc-3 section 5 drift direction,
/// a hard error). The other two drift directions are recorded in
/// [`Ledger::drift`] and steer the verdict to `not-pass`.
pub fn build(
    rows: &[MatrixRow],
    results: &[FixtureResult],
    toolchain: ToolchainInfo,
) -> Result<Ledger, LedgerError> {
    // Index matrix rows by ID.
    let mut status_of: std::collections::BTreeMap<&str, Status> = std::collections::BTreeMap::new();
    for row in rows {
        status_of.insert(row.id.as_str(), row.status);
    }

    // Hard-fail on any fixture covering an absent row ID (direction 1).
    for result in results {
        for id in &result.covers {
            if !status_of.contains_key(id.as_str()) {
                return Err(LedgerError::AbsentRowId {
                    fixture: result.name.clone(),
                    id: id.clone(),
                });
            }
        }
    }

    // Build per-row coverage rows. `covered_without_green` (Doc-3 drift
    // direction 2) is collected here, where `any_green` is in scope: it is
    // a `Covered` row with NO green fixture -- independent of whether some
    // covering fixture also FAILed (that is the separate fixture-FAIL gate).
    let mut ledger_rows: Vec<LedgerRow> = Vec::with_capacity(rows.len());
    let mut covered_without_green: Vec<String> = Vec::new();
    for row in rows {
        let mut covering: Vec<&FixtureResult> = results
            .iter()
            .filter(|r| r.covers.iter().any(|c| c == &row.id))
            .collect();
        covering.sort_by(|a, b| a.name.cmp(&b.name));

        let fixtures: Vec<String> = covering.iter().map(|r| r.name.clone()).collect();
        let any_green = covering.iter().any(|r| r.is_green());
        let any_fail = covering.iter().any(|r| r.is_fail());

        let result = match row.status {
            Status::Covered => {
                if any_fail || !any_green {
                    RowResult::Fail
                } else {
                    RowResult::Pass
                }
            }
            Status::Deferred | Status::Unspecced | Status::OutOfScope => RowResult::NotApplicable,
        };

        if row.status == Status::Covered && !any_green {
            covered_without_green.push(row.id.clone());
        }

        ledger_rows.push(LedgerRow {
            id: row.id.clone(),
            status: row.status,
            fixtures,
            result,
        });
    }
    ledger_rows.sort_by(|a, b| a.id.cmp(&b.id));
    covered_without_green.sort();

    // Drift direction 3: green-without-covered.
    let mut green_without_covered: Vec<GreenWithoutCovered> = Vec::new();
    for result in results.iter().filter(|r| r.is_green()) {
        for id in &result.covers {
            if let Some(&status) = status_of.get(id.as_str()) {
                if status != Status::Covered {
                    green_without_covered.push(GreenWithoutCovered {
                        fixture: result.name.clone(),
                        id: id.clone(),
                        status,
                    });
                }
            }
        }
    }
    green_without_covered.sort_by(|a, b| (&a.fixture, &a.id).cmp(&(&b.fixture, &b.id)));

    // Verdict (Doc 3 section 6).
    let any_fixture_fail = results.iter().any(|r| r.is_fail());
    let any_unspecced = rows.iter().any(|r| r.status == Status::Unspecced);
    let verdict = if !any_fixture_fail
        && !any_unspecced
        && covered_without_green.is_empty()
        && green_without_covered.is_empty()
    {
        PhaseVerdict::Pass
    } else {
        PhaseVerdict::NotPass
    };

    Ok(Ledger {
        verdict,
        rows: ledger_rows,
        drift: Drift {
            covered_without_green,
            green_without_covered,
        },
        toolchain,
    })
}

/// Probe the host `cc` for the reproducibility record: its path and the
/// first line of `cc --version`. A failed probe records `<unknown>` so a
/// version-query failure does not abort the run.
pub fn probe_toolchain(cc: &Path) -> ToolchainInfo {
    let cc_str = cc.display().to_string();
    let cc_version = std::process::Command::new(cc)
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "<unknown>".to_string());
    ToolchainInfo {
        cc: cc_str,
        cc_version,
    }
}

/// Write both `coverage_ledger.json` (pretty, sorted rows) and
/// `coverage_ledger.md` (a human rollup regenerated from the same data)
/// into `out_dir`, creating it if absent. Existing unrelated files in
/// `out_dir` are left untouched.
pub fn write_ledger(ledger: &Ledger, out_dir: &Path) -> Result<(), LedgerError> {
    std::fs::create_dir_all(out_dir)?;
    let json = serde_json::to_string_pretty(ledger).expect("ledger serializes to JSON");
    std::fs::write(out_dir.join("coverage_ledger.json"), format!("{json}\n"))?;
    std::fs::write(out_dir.join("coverage_ledger.md"), render_md(ledger))?;
    Ok(())
}

/// Render the human-readable `coverage_ledger.md` rollup from `ledger`.
fn render_md(ledger: &Ledger) -> String {
    let mut out = String::new();
    out.push_str("# Codegen-coverage ledger\n\n");
    out.push_str(&format!(
        "- **Verdict:** `{}`\n",
        match ledger.verdict {
            PhaseVerdict::Pass => "pass",
            PhaseVerdict::NotPass => "not-pass",
        }
    ));
    out.push_str(&format!(
        "- **Toolchain:** `{}` ({})\n",
        ledger.toolchain.cc, ledger.toolchain.cc_version
    ));
    out.push_str(&format!("- **Matrix rows:** {}\n", ledger.rows.len()));

    let covered = ledger
        .rows
        .iter()
        .filter(|r| r.status == Status::Covered)
        .count();
    let green = ledger
        .rows
        .iter()
        .filter(|r| r.result == RowResult::Pass)
        .count();
    out.push_str(&format!(
        "- **Covered rows:** {covered} ({green} with a green fixture)\n\n"
    ));

    if !ledger.drift.covered_without_green.is_empty() {
        out.push_str("## Drift: covered-without-green\n\n");
        for id in &ledger.drift.covered_without_green {
            out.push_str(&format!("- `{id}`\n"));
        }
        out.push('\n');
    }
    if !ledger.drift.green_without_covered.is_empty() {
        out.push_str("## Drift: green-without-covered\n\n");
        for d in &ledger.drift.green_without_covered {
            out.push_str(&format!(
                "- `{}` covers `{}` (status `{:?}`)\n",
                d.fixture, d.id, d.status
            ));
        }
        out.push('\n');
    }

    out.push_str("## Rows\n\n");
    out.push_str("| id | status | result | fixtures |\n");
    out.push_str("| -- | ------ | ------ | -------- |\n");
    for row in &ledger.rows {
        out.push_str(&format!(
            "| {} | {:?} | {:?} | {} |\n",
            row.id,
            row.status,
            row.result,
            if row.fixtures.is_empty() {
                "-".to_string()
            } else {
                row.fixtures.join(", ")
            }
        ));
    }
    out
}
