//! `bridge-coverage-runner` binary entry point.
//!
//! CLI per strategy section 10 Mode A:
//!
//! ```text
//! bridge-coverage-runner \
//!   --metadata <path/to/rows.toml> \
//!   --snippets <path/to/snippets/> \
//!   --matrix-md <path/to/bridge-coverage-matrix.md> \
//!   --out <path/to/output/dir>
//! ```

#![feature(rustc_private)]

extern crate rustc_driver;

mod cluster;
mod inspect;
mod matrix;
mod oracle;
mod report;
mod snippet;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::cluster::{bridge_error_variant, oracle_outcome_tag, ActualOutcomeTag, RowResult};
use crate::inspect::InspectionPredicate;
use crate::matrix::{Expectation, ExpectedRejection};
use crate::oracle::OracleOutcome;
use regseq_driver::bridge::{
    BridgeError, CAbiTypePosition, DiagLogFormatReason, DiagLogValidationDiagnostic,
    DslRejectionKind,
};

struct Args {
    metadata: PathBuf,
    snippets: PathBuf,
    matrix_md: PathBuf,
    out: PathBuf,
}

fn parse_args<I: IntoIterator<Item = String>>(iter: I) -> Result<Args, String> {
    let mut metadata = None;
    let mut snippets = None;
    let mut matrix_md = None;
    let mut out = None;

    let mut iter = iter.into_iter();
    let _exe = iter.next();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--metadata" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--metadata requires a value".to_string())?;
                metadata = Some(PathBuf::from(v));
            }
            "--snippets" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--snippets requires a value".to_string())?;
                snippets = Some(PathBuf::from(v));
            }
            "--matrix-md" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--matrix-md requires a value".to_string())?;
                matrix_md = Some(PathBuf::from(v));
            }
            "--out" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--out requires a value".to_string())?;
                out = Some(PathBuf::from(v));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        metadata: metadata.ok_or_else(|| "missing --metadata".to_string())?,
        snippets: snippets.ok_or_else(|| "missing --snippets".to_string())?,
        matrix_md: matrix_md.ok_or_else(|| "missing --matrix-md".to_string())?,
        out: out.ok_or_else(|| "missing --out".to_string())?,
    })
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args()) {
        Ok(a) => a,
        Err(err) => {
            eprintln!("argument error: {err}");
            return ExitCode::from(2);
        }
    };

    let rows_file = match matrix::load(&args.metadata) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("failed to load sidecar: {err}");
            return ExitCode::from(2);
        }
    };

    let drift = match matrix::check_drift(&rows_file, &args.matrix_md) {
        Ok(d) => d,
        Err(err) => {
            eprintln!("drift-hard-fail: {err}");
            return ExitCode::from(2);
        }
    };

    let snippets = match snippet::load_snippets(&args.snippets, &rows_file.rows) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to load snippets: {err}");
            return ExitCode::from(2);
        }
    };

    let mut results: Vec<RowResult> = Vec::with_capacity(rows_file.rows.len());
    for (row, snip) in rows_file.rows.iter().zip(snippets.iter()) {
        let overrides: Option<test_ir_fixtures::BuildOverrides> = row
            .build_overrides
            .as_ref()
            .map(test_ir_fixtures::BuildOverrides::from);
        let outcome = oracle::invoke(&snip.path, overrides.as_ref());
        let (pass, classification_detail) = classify(row, &outcome, row.inspect.as_ref());
        let variant_or_phase = variant_or_phase(&outcome);
        let detail = format!("{} -- {}", row.row_context(), classification_detail);
        results.push(RowResult {
            row_id: row.id.clone(),
            expectation: row.expectation,
            cluster_key: row.cluster_key.clone(),
            snippet_path: snip.path.clone(),
            byte_len: snip.byte_len,
            snippet_hash: snip.hash_prefix.clone(),
            actual_outcome: oracle_outcome_tag(&outcome),
            variant_or_phase,
            pass,
            detail,
        });
    }

    let clusters = cluster::group(&results);
    let report = report::render(&args.metadata, &args.snippets, &results, &clusters, &drift);

    if let Err(err) = std::fs::create_dir_all(&args.out) {
        eprintln!("failed to create output directory: {err}");
        return ExitCode::from(2);
    }
    let report_path = args.out.join("report.md");
    if let Err(err) = std::fs::write(&report_path, &report) {
        eprintln!("failed to write report: {err}");
        return ExitCode::from(2);
    }

    let any_fail = results.iter().any(|r| !r.pass);
    if any_fail {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn variant_or_phase(outcome: &OracleOutcome) -> String {
    match outcome {
        OracleOutcome::Ok(_) => "Ok".to_string(),
        OracleOutcome::ValidatorFailed { violation } => format!("{violation:?}")
            .split(['(', ' ', '{'])
            .next()
            .unwrap_or("")
            .to_string(),
        OracleOutcome::Rejected(err) => bridge_error_variant(err).to_string(),
        OracleOutcome::RustcCleanReject { .. } => "clean-reject".to_string(),
        OracleOutcome::RustcICE { .. } => "ICE".to_string(),
        OracleOutcome::RustcNoAfterAnalysis => "no-after-analysis".to_string(),
        OracleOutcome::Panic { message } => {
            let truncated: String = message.chars().take(80).collect();
            truncated
        }
    }
}

fn classify(
    row: &matrix::Row,
    outcome: &OracleOutcome,
    inspect: Option<&InspectionPredicate>,
) -> (bool, String) {
    match (row.expectation, outcome) {
        (Expectation::Positive, OracleOutcome::Ok(model)) => match inspect {
            None => (true, "OK (no inspection)".to_string()),
            Some(pred) => match inspect::check(model, pred) {
                Ok(()) => (true, "OK (inspection passed)".to_string()),
                Err(err) => (false, format!("inspection failed: {err}")),
            },
        },
        (Expectation::Positive, other) => (
            false,
            format!(
                "positive row expected Ok, got {} ({})",
                cluster_tag_str(&oracle_outcome_tag(other)),
                outcome_diagnostic(other),
            ),
        ),
        (Expectation::Rejected, OracleOutcome::Rejected(err)) => {
            let expected = match &row.expected_rejection {
                Some(er) => er,
                None => return (false, "rejected row has no expected_rejection".to_string()),
            };
            match expected_rejection_matches(err, expected) {
                Ok(()) => (
                    true,
                    format!(
                        "rejected with expected variant {}",
                        bridge_error_variant(err)
                    ),
                ),
                Err(reason) => (false, reason),
            }
        }
        (Expectation::Rejected, other) => (
            false,
            format!(
                "rejected row expected BridgeError, got {} ({})",
                cluster_tag_str(&oracle_outcome_tag(other)),
                outcome_diagnostic(other),
            ),
        ),
        (Expectation::ValidatorFailing, _) => (
            false,
            "validator-failing rows are out of scope for F1".to_string(),
        ),
    }
}

fn outcome_diagnostic(outcome: &OracleOutcome) -> String {
    match outcome {
        OracleOutcome::Ok(_) => "ok".to_string(),
        OracleOutcome::ValidatorFailed { violation } => format!("{violation:?}"),
        OracleOutcome::Rejected(err) => format!("{err}"),
        OracleOutcome::RustcCleanReject { stderr } => format!("rustc stderr: {}", short(stderr)),
        OracleOutcome::RustcICE { stderr } => format!("rustc ICE stderr: {}", short(stderr)),
        OracleOutcome::RustcNoAfterAnalysis => "rustc accepted but no after_analysis".to_string(),
        OracleOutcome::Panic { message } => format!("panic: {message}"),
    }
}

fn short(s: &str) -> String {
    let trimmed: String = s.replace('\n', " ");
    trimmed.chars().take(160).collect()
}

fn cluster_tag_str(tag: &ActualOutcomeTag) -> &'static str {
    match tag {
        ActualOutcomeTag::Ok => "Ok",
        ActualOutcomeTag::Rejected => "Rejected",
        ActualOutcomeTag::ValidatorFailed => "ValidatorFailed",
        ActualOutcomeTag::RustcCleanReject => "RustcCleanReject",
        ActualOutcomeTag::RustcICE => "RustcICE",
        ActualOutcomeTag::RustcNoAfterAnalysis => "RustcNoAfterAnalysis",
        ActualOutcomeTag::Panic => "Panic",
    }
}

fn expected_rejection_matches(
    err: &BridgeError,
    expected: &ExpectedRejection,
) -> Result<(), String> {
    let actual_variant = bridge_error_variant(err);
    if actual_variant != expected.variant {
        return Err(format!(
            "expected BridgeError::{} but got BridgeError::{actual_variant}",
            expected.variant
        ));
    }
    match err {
        BridgeError::DslRejection { kind, .. } => {
            if let Some(expected_kind) = &expected.kind {
                let actual_kind = dsl_rejection_kind_tag(*kind);
                if actual_kind != expected_kind {
                    return Err(format!(
                        "DslRejection: expected kind {expected_kind} but got {actual_kind}"
                    ));
                }
            }
        }
        BridgeError::UnsupportedCAbiType { position, .. } => {
            if let Some(expected_position) = &expected.position {
                let actual_position = cabi_position_tag(position);
                if actual_position != expected_position {
                    return Err(format!(
                        "UnsupportedCAbiType: expected position {expected_position} but got {actual_position}"
                    ));
                }
            }
        }
        BridgeError::DiagLogFormat { reason, .. } => {
            if let Some(expected_reason) = &expected.reason {
                let actual_reason = diag_log_format_reason_tag(*reason);
                if actual_reason != expected_reason {
                    return Err(format!(
                        "DiagLogFormat: expected reason {expected_reason} but got {actual_reason}"
                    ));
                }
            }
        }
        BridgeError::DiagLogValidation { diagnostic, .. } => {
            if let Some(expected_diagnostic) = &expected.diagnostic {
                let actual_diagnostic = diag_log_validation_diagnostic_tag(*diagnostic);
                if actual_diagnostic != expected_diagnostic {
                    return Err(format!(
                        "DiagLogValidation: expected diagnostic {expected_diagnostic} but got {actual_diagnostic}"
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn dsl_rejection_kind_tag(kind: DslRejectionKind) -> &'static str {
    match kind {
        DslRejectionKind::Box => "Box",
        DslRejectionKind::CVoid => "CVoid",
        DslRejectionKind::Atomic => "Atomic",
        DslRejectionKind::PlaceProjection => "PlaceProjection",
        DslRejectionKind::Rvalue => "Rvalue",
        DslRejectionKind::Statement => "Statement",
        DslRejectionKind::Terminator => "Terminator",
        DslRejectionKind::BasicBlock => "BasicBlock",
    }
}

fn cabi_position_tag(position: &CAbiTypePosition) -> &'static str {
    match position {
        CAbiTypePosition::Input(_) => "Input",
        CAbiTypePosition::Output => "Output",
        CAbiTypePosition::Field => "Field",
    }
}

fn diag_log_format_reason_tag(reason: DiagLogFormatReason) -> &'static str {
    match reason {
        DiagLogFormatReason::DebugFormatterRejected => "DebugFormatterRejected",
        DiagLogFormatReason::PositionalArgRejected => "PositionalArgRejected",
        DiagLogFormatReason::NamedArgRejected => "NamedArgRejected",
        DiagLogFormatReason::NonSubsetModifier => "NonSubsetModifier",
        DiagLogFormatReason::UnclosedBrace => "UnclosedBrace",
        DiagLogFormatReason::UnmatchedClosingBrace => "UnmatchedClosingBrace",
    }
}

fn diag_log_validation_diagnostic_tag(diagnostic: DiagLogValidationDiagnostic) -> &'static str {
    match diagnostic {
        DiagLogValidationDiagnostic::D1 => "D1",
        DiagLogValidationDiagnostic::D2 => "D2",
        DiagLogValidationDiagnostic::D4 => "D4",
        DiagLogValidationDiagnostic::D6 => "D6",
        DiagLogValidationDiagnostic::D7 => "D7",
        DiagLogValidationDiagnostic::D8 => "D8",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(
        variant: &str,
        kind: Option<&str>,
        position: Option<&str>,
        reason: Option<&str>,
        diagnostic: Option<&str>,
    ) -> ExpectedRejection {
        ExpectedRejection {
            variant: variant.to_string(),
            kind: kind.map(str::to_string),
            position: position.map(str::to_string),
            reason: reason.map(str::to_string),
            diagnostic: diagnostic.map(str::to_string),
        }
    }

    #[test]
    fn expected_rejection_matches_variant_only_when_sub_axis_absent() {
        let err = BridgeError::DslRejection {
            kind: DslRejectionKind::Atomic,
            ty_repr: "AtomicU32".to_string(),
        };
        let exp = expected("DslRejection", None, None, None, None);
        assert!(expected_rejection_matches(&err, &exp).is_ok());
    }

    #[test]
    fn expected_rejection_matches_kind_when_kind_present() {
        let err = BridgeError::DslRejection {
            kind: DslRejectionKind::Atomic,
            ty_repr: "AtomicU32".to_string(),
        };
        let exp_ok = expected("DslRejection", Some("Atomic"), None, None, None);
        assert!(expected_rejection_matches(&err, &exp_ok).is_ok());

        let err_box = BridgeError::DslRejection {
            kind: DslRejectionKind::Box,
            ty_repr: "Box<u8>".to_string(),
        };
        let exp_fail = expected("DslRejection", Some("Atomic"), None, None, None);
        let result = expected_rejection_matches(&err_box, &exp_fail);
        assert!(result.is_err());
        let detail = result.unwrap_err();
        assert!(detail.contains("DslRejection"));
        assert!(detail.contains("Atomic"));
        assert!(detail.contains("Box"));
    }

    #[test]
    fn expected_rejection_matches_reason_when_reason_present() {
        let err = BridgeError::DiagLogFormat {
            reason: DiagLogFormatReason::DebugFormatterRejected,
            template: "{:?}".to_string(),
        };
        let exp_ok = expected(
            "DiagLogFormat",
            None,
            None,
            Some("DebugFormatterRejected"),
            None,
        );
        assert!(expected_rejection_matches(&err, &exp_ok).is_ok());

        let exp_fail = expected(
            "DiagLogFormat",
            None,
            None,
            Some("PositionalArgRejected"),
            None,
        );
        let result = expected_rejection_matches(&err, &exp_fail);
        assert!(result.is_err());
        let detail = result.unwrap_err();
        assert!(detail.contains("DiagLogFormat"));
        assert!(detail.contains("PositionalArgRejected"));
        assert!(detail.contains("DebugFormatterRejected"));
    }

    #[test]
    fn expected_rejection_matches_position_when_position_present() {
        let err_field = BridgeError::UnsupportedCAbiType {
            ty_repr: "WithRustFn".to_string(),
            position: CAbiTypePosition::Field,
        };
        let exp_ok = expected("UnsupportedCAbiType", None, Some("Field"), None, None);
        assert!(expected_rejection_matches(&err_field, &exp_ok).is_ok());

        let err_output = BridgeError::UnsupportedCAbiType {
            ty_repr: "(u32, u32)".to_string(),
            position: CAbiTypePosition::Output,
        };
        let exp_fail = expected("UnsupportedCAbiType", None, Some("Field"), None, None);
        let result = expected_rejection_matches(&err_output, &exp_fail);
        assert!(result.is_err());
        let detail = result.unwrap_err();
        assert!(detail.contains("UnsupportedCAbiType"));
        assert!(detail.contains("Field"));
        assert!(detail.contains("Output"));
    }

    #[test]
    fn expected_rejection_matches_diagnostic_when_diagnostic_present() {
        let err = BridgeError::DiagLogValidation {
            diagnostic: DiagLogValidationDiagnostic::D1,
            detail: "arity".to_string(),
        };
        let exp_ok = expected("DiagLogValidation", None, None, None, Some("D1"));
        assert!(expected_rejection_matches(&err, &exp_ok).is_ok());

        let err_d6 = BridgeError::DiagLogValidation {
            diagnostic: DiagLogValidationDiagnostic::D6,
            detail: "literal".to_string(),
        };
        let exp_fail = expected("DiagLogValidation", None, None, None, Some("D1"));
        let result = expected_rejection_matches(&err_d6, &exp_fail);
        assert!(result.is_err());
        let detail = result.unwrap_err();
        assert!(detail.contains("DiagLogValidation"));
        assert!(detail.contains("D1"));
        assert!(detail.contains("D6"));
    }
}
