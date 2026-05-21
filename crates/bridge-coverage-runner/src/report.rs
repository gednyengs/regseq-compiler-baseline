//! Markdown report emitter per strategy section 9 (markdown-only
//! in F1).

use std::fmt::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cluster::RowResult;
use crate::cluster::{ActualOutcomeTag, Cluster};
use crate::matrix::{DriftReport, Expectation};

/// Build the markdown report body.
pub fn render(
    metadata_path: &Path,
    snippets_dir: &Path,
    results: &[RowResult],
    clusters: &[Cluster],
    drift: &DriftReport,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Bridge Coverage Runner Report");
    let _ = writeln!(out);
    let _ = writeln!(out, "Generated: {}", iso_timestamp());
    let _ = writeln!(out, "Sidecar: {}", metadata_path.display());
    let _ = writeln!(out, "Snippets dir: {}", snippets_dir.display());
    let _ = writeln!(out);

    let total = results.len();
    let pass = results.iter().filter(|r| r.pass).count();
    let fail = total - pass;
    let mut tally: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for r in results {
        *tally
            .entry(actual_outcome_static_str(&r.actual_outcome))
            .or_insert(0) += 1;
    }
    let by_class: Vec<String> = tally.iter().map(|(k, v)| format!("{k}={v}")).collect();

    let _ = writeln!(out, "## Summary");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Class | Count |");
    let _ = writeln!(out, "| ----- | ----- |");
    let _ = writeln!(out, "| Total rows | {total} |");
    let _ = writeln!(out, "| Pass | {pass} |");
    let _ = writeln!(out, "| Fail | {fail} |");
    let _ = writeln!(out, "| Outcomes by class | {} |", by_class.join(", "));
    let _ = writeln!(out);

    let _ = writeln!(out, "Drift report:");
    if drift.missing_in_sidecar.is_empty() {
        let _ = writeln!(out, "- Sidecar rows missing from matrix: none");
        let _ = writeln!(
            out,
            "- Matrix rows missing from sidecar: 0 (subset-OK in F1; see strategy section 4.1)"
        );
    } else {
        let _ = writeln!(out, "- Sidecar rows missing from matrix: none");
        let _ = writeln!(
            out,
            "- Matrix rows missing from sidecar: {} (subset-OK in F1; see strategy section 4.1)",
            drift.missing_in_sidecar.len()
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Per-row outcomes");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Row | Expectation | Outcome | Variant | Pass | Detail |"
    );
    let _ = writeln!(
        out,
        "| --- | ----------- | ------- | ------- | ---- | ------ |"
    );
    for r in results {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            r.row_id,
            expectation_str(&r.expectation),
            actual_outcome_static_str(&r.actual_outcome),
            escape_pipe(&r.variant_or_phase),
            if r.pass { "PASS" } else { "FAIL" },
            escape_pipe(&r.detail),
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Failure clusters");
    let _ = writeln!(out);
    let _ = writeln!(out, "(Sorted by cluster size descending.)");
    let _ = writeln!(out);

    if clusters.is_empty() {
        let _ = writeln!(out, "_No clusters produced (empty result set)._");
        return out;
    }

    for cluster in clusters {
        let header_hash = format!("{:016x}", cluster.fingerprint_hash);
        let _ = writeln!(out, "### Cluster {}", &header_hash[..8]);
        let _ = writeln!(out);
        let _ = writeln!(out, "Fingerprint:");
        let _ = writeln!(out, "- cluster_key: {}", cluster.fingerprint.cluster_key);
        let _ = writeln!(
            out,
            "- expectation_class: {}",
            expectation_str(&cluster.fingerprint.expectation_class)
        );
        let _ = writeln!(
            out,
            "- actual_outcome: {}",
            actual_outcome_static_str(&cluster.fingerprint.actual_outcome)
        );
        let _ = writeln!(
            out,
            "- variant_or_phase: {}",
            cluster.fingerprint.variant_or_phase
        );
        let _ = writeln!(out);
        let _ = writeln!(out, "Members ({}):", cluster.members.len());
        let _ = writeln!(out);
        for m in &cluster.members {
            let _ = writeln!(
                out,
                "- `{}` -- `{}` -- {} [{}]",
                m.row_id,
                m.snippet_path.display(),
                m.detail,
                if m.pass { "PASS" } else { "FAIL" }
            );
        }
        let _ = writeln!(out);
        if let Some(repr) = cluster.members.first() {
            let _ = writeln!(
                out,
                "Representative repro ({}):",
                repr.snippet_path.display()
            );
            let _ = writeln!(out);
            let snippet_src =
                std::fs::read_to_string(&repr.snippet_path).unwrap_or_else(|_| String::new());
            let _ = writeln!(out, "```rust");
            for line in snippet_src.lines() {
                let _ = writeln!(out, "{line}");
            }
            let _ = writeln!(out, "```");
            let _ = writeln!(out);
        }
    }

    out
}

fn expectation_str(e: &Expectation) -> &'static str {
    match e {
        Expectation::Positive => "positive",
        Expectation::Rejected => "rejected",
        Expectation::ValidatorFailing => "validator-failing",
    }
}

fn actual_outcome_static_str(t: &ActualOutcomeTag) -> &'static str {
    match t {
        ActualOutcomeTag::Ok => "Ok",
        ActualOutcomeTag::Rejected => "Rejected",
        ActualOutcomeTag::ValidatorFailed => "ValidatorFailed",
        ActualOutcomeTag::RustcCleanReject => "RustcCleanReject",
        ActualOutcomeTag::RustcICE => "RustcICE",
        ActualOutcomeTag::RustcNoAfterAnalysis => "RustcNoAfterAnalysis",
        ActualOutcomeTag::Panic => "Panic",
    }
}

fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// Best-effort ISO 8601 UTC timestamp (no external dep).
fn iso_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Civil-from-days arithmetic per Howard Hinnant's algorithm.
fn unix_to_ymdhms(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let s = (secs % 60) as u32;
    let m = ((secs / 60) % 60) as u32;
    let h = ((secs / 3600) % 24) as u32;
    let days = (secs / 86400) as i64;
    // Shift so 0000-03-01 is epoch day -719468 + 719468 = 0.
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y_civil = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m_civil = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m_civil <= 2 { y_civil + 1 } else { y_civil };
    (y, m_civil, d, h, m, s)
}
