//! Matrix metadata sidecar loader per
//! `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
//! section 4 (sidecar) + section 4.1 (drift check).
//!
//! Strategy section 4.1 sketches a strict drift check: refuse to
//! run if any row ID is in one but not the other. F1 adopts a
//! `subset-OK` variant: sidecar rows MUST exist in the matrix
//! (hard fail); matrix rows MAY exceed the sidecar (the 10-row F1
//! demo set legitimately covers a subset of the 445-row matrix).
//! Strict-mode becomes feasible when the sidecar reaches full
//! coverage in F4. This module-level note is searchable via grep
//! when that transition lands.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

/// Parsed `rows.toml` sidecar contents.
#[derive(Debug, Deserialize)]
pub struct RowsFile {
    /// All row entries in the sidecar.
    #[serde(rename = "row")]
    pub rows: Vec<Row>,
}

/// A single sidecar row.
///
/// Some fields (`section`, `construct`, `status`, `fixture_path`)
/// are part of the canonical TOML schema per strategy section 4 and
/// are populated for every well-formed sidecar entry. They are
/// surfaced to the report via [`row_context`].
#[derive(Debug, Deserialize)]
pub struct Row {
    /// Matrix row ID (e.g., `TY-018`, `RV-006-A`, `OP-003-G-1`).
    pub id: String,
    /// Matrix section the row lives under (e.g., `1.1`).
    pub section: String,
    /// Human-readable construct identifier.
    pub construct: String,
    /// Expectation class.
    pub expectation: Expectation,
    /// Coverage status (e.g., `covered`, `uncovered`).
    pub status: String,
    /// Explicit cluster parent per strategy section 8.
    pub cluster_key: String,
    /// Optional fixture path (unused by F1).
    pub fixture_path: Option<String>,
    /// Snippet file path relative to the snippets directory.
    pub snippet_path: Option<String>,
    /// Optional inspection predicate (positive rows only).
    pub inspect: Option<crate::inspect::InspectionPredicate>,
    /// Expected rejection shape for `rejected` rows.
    pub expected_rejection: Option<ExpectedRejection>,
    /// Optional per-row build-flag overrides forwarded to the helper
    /// via `try_bridge_fixture_path_with_overrides`. `None` preserves
    /// the helper's default config; this is the D1 surface introduced
    /// per `.harness/audits/2026-05-21-post-f4-triage.md:349-415`.
    pub build_overrides: Option<RowBuildOverrides>,
}

/// Mirror of [`test_ir_fixtures::BuildOverrides`] expressed as a
/// `Deserialize` TOML schema. The runner-side type lives next to
/// the other row schema types so the loader can `deny_unknown_fields`
/// and enforce the `-Z` allow-list at load time before the helper
/// workhorse ever runs.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct RowBuildOverrides {
    /// Mirrors `BuildOverrides::overflow_checks`.
    pub overflow_checks: Option<bool>,
    /// Mirrors `BuildOverrides::z_flags`. Each entry is validated
    /// at load time against [`test_ir_fixtures::Z_FLAG_ALLOWLIST`].
    #[serde(default)]
    pub z_flags: Vec<String>,
}

impl From<&RowBuildOverrides> for test_ir_fixtures::BuildOverrides {
    fn from(value: &RowBuildOverrides) -> Self {
        Self {
            overflow_checks: value.overflow_checks,
            z_flags: value.z_flags.clone(),
        }
    }
}

/// Per-row expectation class.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Expectation {
    /// Bridge succeeds; inspection predicate must hold.
    Positive,
    /// Bridge rejects with the named `BridgeError` variant.
    Rejected,
    /// Bridge succeeds but the validator surfaces an invariant
    /// violation. Out of scope for F1 per strategy section 2.
    ValidatorFailing,
}

/// Expected rejection shape for `rejected` rows.
///
/// Sub-axis fields (`kind`, `position`, `reason`, `diagnostic`)
/// are optional with `None` => no constraint semantics. When `Some`,
/// the runner's matcher enforces the field against the actual
/// `BridgeError` variant's matching sub-axis tag. The applicable
/// sub-axis depends on `variant`: `DslRejection` consumes `kind`,
/// `UnsupportedCAbiType` consumes `position`, `DiagLogFormat`
/// consumes `reason`, `DiagLogValidation` consumes `diagnostic`.
#[derive(Debug, Deserialize)]
pub struct ExpectedRejection {
    /// `BridgeError` variant name (e.g., `DslRejection`, `Coroutine`).
    pub variant: String,
    /// Sub-kind for `DslRejection` (e.g., `CVoid`, `Box`).
    pub kind: Option<String>,
    /// Sub-position for `UnsupportedCAbiType` (`Input`, `Output`,
    /// `Field`).
    pub position: Option<String>,
    /// Sub-reason for `DiagLogFormat` (e.g.,
    /// `DebugFormatterRejected`, `PositionalArgRejected`).
    pub reason: Option<String>,
    /// Sub-diagnostic for `DiagLogValidation` (`D1`, `D2`, `D4`,
    /// `D6`, `D7`, `D8`).
    pub diagnostic: Option<String>,
}

impl Row {
    /// One-line human-readable context string composed from the
    /// sidecar fields. Used by the report to surface the canonical
    /// `section` / `construct` / `status` data alongside the
    /// outcome and inspection result.
    pub fn row_context(&self) -> String {
        let mut out = format!("[{}] {}", self.section, self.construct);
        out.push_str(" (status=");
        out.push_str(&self.status);
        out.push(')');
        if let Some(fixture) = &self.fixture_path {
            out.push_str(" fixture=");
            out.push_str(fixture);
        }
        out
    }
}

/// Errors returned by the sidecar loader.
#[derive(Debug)]
pub enum MatrixError {
    /// The sidecar file could not be read.
    Io(std::io::Error),
    /// The sidecar file could not be parsed as TOML.
    Toml(toml::de::Error),
    /// One or more sidecar rows reference IDs that are NOT present
    /// in the matrix markdown. Hard fail per F1 subset-OK rules:
    /// the sidecar must never invent row IDs.
    DriftHardFail {
        /// Row IDs found in the sidecar but absent from the matrix.
        rows: Vec<String>,
    },
    /// A row carries a `build_overrides.z_flags` entry whose value
    /// is not in [`test_ir_fixtures::Z_FLAG_ALLOWLIST`]. Load-time
    /// fail is the first line of defense; the helper workhorse's
    /// panic is the second per the F68 D1 proposal at
    /// `.harness/audits/2026-05-21-post-f4-triage.md:349-415`.
    UnknownZFlag {
        /// Owning row ID.
        row_id: String,
        /// The unrecognized `-Z` flag value.
        flag: String,
    },
}

impl std::fmt::Display for MatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatrixError::Io(err) => write!(f, "matrix sidecar I/O error: {err}"),
            MatrixError::Toml(err) => write!(f, "matrix sidecar TOML parse error: {err}"),
            MatrixError::DriftHardFail { rows } => {
                write!(
                    f,
                    "drift-hard-fail: sidecar row(s) absent from matrix: {}",
                    rows.join(", ")
                )
            }
            MatrixError::UnknownZFlag { row_id, flag } => {
                write!(
                    f,
                    "row {row_id}: unrecognized -Z override {flag:?}; allow-list = {allowlist:?}",
                    allowlist = test_ir_fixtures::Z_FLAG_ALLOWLIST,
                )
            }
        }
    }
}

impl std::error::Error for MatrixError {}

impl From<std::io::Error> for MatrixError {
    fn from(value: std::io::Error) -> Self {
        MatrixError::Io(value)
    }
}

impl From<toml::de::Error> for MatrixError {
    fn from(value: toml::de::Error) -> Self {
        MatrixError::Toml(value)
    }
}

/// Drift report produced by [`check_drift`].
///
/// `missing_in_matrix` (sidecar rows absent from matrix) is a hard
/// fail surfaced as [`MatrixError::DriftHardFail`] instead of
/// populated in the report. `missing_in_sidecar` (matrix rows the
/// sidecar does not yet cover) is informational data only --
/// never a fail condition in F1.
#[derive(Debug, Default)]
pub struct DriftReport {
    /// Matrix row IDs NOT present in the sidecar (informational).
    pub missing_in_sidecar: Vec<String>,
}

/// Parse the sidecar TOML at `metadata_path` and validate every
/// `build_overrides.z_flags` entry against
/// [`test_ir_fixtures::Z_FLAG_ALLOWLIST`]. Unrecognized values
/// surface as [`MatrixError::UnknownZFlag`] so the runner fails
/// fast at load time instead of letting the helper's
/// defense-in-depth panic fire mid-sweep.
pub fn load(metadata_path: &Path) -> Result<RowsFile, MatrixError> {
    let text = fs::read_to_string(metadata_path)?;
    let parsed: RowsFile = toml::from_str(&text)?;
    for row in parsed.rows.iter() {
        if let Some(overrides) = row.build_overrides.as_ref() {
            for flag in overrides.z_flags.iter() {
                if !test_ir_fixtures::Z_FLAG_ALLOWLIST.contains(&flag.as_str()) {
                    return Err(MatrixError::UnknownZFlag {
                        row_id: row.id.clone(),
                        flag: flag.clone(),
                    });
                }
            }
        }
    }
    Ok(parsed)
}

/// Subset-OK drift check per strategy section 4.1 (F1 adjudication).
///
/// `missing_in_matrix` (sidecar row not in matrix) is a hard fail
/// because the sidecar must never invent row IDs. `missing_in_sidecar`
/// (matrix row not in sidecar) is acceptable in F1 because the
/// sidecar legitimately covers a 10-row subset of the matrix.
pub fn check_drift(rows: &RowsFile, matrix_md_path: &Path) -> Result<DriftReport, MatrixError> {
    let matrix_text = fs::read_to_string(matrix_md_path)?;
    let matrix_ids = extract_matrix_row_ids(&matrix_text);
    let sidecar_ids: BTreeSet<String> = rows.rows.iter().map(|r| r.id.clone()).collect();

    let missing_in_matrix: Vec<String> = sidecar_ids
        .iter()
        .filter(|id| !matrix_ids.contains(*id))
        .cloned()
        .collect();
    if !missing_in_matrix.is_empty() {
        return Err(MatrixError::DriftHardFail {
            rows: missing_in_matrix,
        });
    }

    let missing_in_sidecar: Vec<String> = matrix_ids
        .iter()
        .filter(|id| !sidecar_ids.contains(*id))
        .cloned()
        .collect();
    Ok(DriftReport { missing_in_sidecar })
}

/// Extract every `ID` token that appears as the leftmost cell of a
/// markdown table row. Matches `TY-001`, `RV-006-A`, `OP-003-G-1`,
/// and other observed matrix row-ID shapes (2-3 letter family
/// prefix, three-digit number, optional `-LETTER` sub-axis,
/// optional `-NUMBER` nested sub-row).
fn extract_matrix_row_ids(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let stripped = line.trim_start();
        if !stripped.starts_with('|') {
            continue;
        }
        // First cell: everything between leading `|` and the next `|`.
        let body = &stripped[1..];
        let cell = match body.split('|').next() {
            Some(c) => c.trim(),
            None => continue,
        };
        if is_row_id(cell) {
            out.insert(cell.to_string());
        }
    }
    out
}

/// Returns true iff `s` matches the matrix row-ID shape:
/// 2-3 ASCII-uppercase letters, then `-`, then 3 ASCII digits,
/// then optionally `-LETTER` (sub-axis), then optionally `-NUMBER`
/// (nested sub-row).
fn is_row_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    // Family prefix: 2-3 uppercase ASCII letters.
    let mut prefix_len = 0;
    while i < bytes.len() && (bytes[i] as char).is_ascii_uppercase() {
        i += 1;
        prefix_len += 1;
        if prefix_len > 3 {
            return false;
        }
    }
    if !(2..=3).contains(&prefix_len) {
        return false;
    }
    // Separator.
    if i >= bytes.len() || bytes[i] != b'-' {
        return false;
    }
    i += 1;
    // Three digits.
    let digits_start = i;
    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
        i += 1;
    }
    if i - digits_start != 3 {
        return false;
    }
    if i == bytes.len() {
        return true;
    }
    // Optional sub-axis: `-LETTER`.
    if bytes[i] != b'-' {
        return false;
    }
    i += 1;
    if i >= bytes.len() || !(bytes[i] as char).is_ascii_uppercase() {
        return false;
    }
    i += 1;
    if i == bytes.len() {
        return true;
    }
    // Optional nested sub-row: `-NUMBER` (1+ digits).
    if bytes[i] != b'-' {
        return false;
    }
    i += 1;
    let nested_start = i;
    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
        i += 1;
    }
    if i == nested_start {
        return false;
    }
    i == bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_row_id_accepts_top_level() {
        assert!(is_row_id("TY-001"));
        assert!(is_row_id("RJ-028"));
        assert!(is_row_id("IX-002"));
    }

    #[test]
    fn is_row_id_accepts_sub_axis() {
        assert!(is_row_id("RV-006-A"));
        assert!(is_row_id("TY-014-A"));
    }

    #[test]
    fn is_row_id_accepts_nested() {
        assert!(is_row_id("OP-003-G-1"));
    }

    #[test]
    fn is_row_id_rejects_non_matches() {
        assert!(!is_row_id(""));
        assert!(!is_row_id("Construct"));
        assert!(!is_row_id("TY-1"));
        assert!(!is_row_id("ty-001"));
        assert!(!is_row_id("TY-001-"));
        assert!(!is_row_id("TY-001-a"));
        assert!(!is_row_id("TY-001-A-"));
    }

    #[test]
    fn extract_matrix_row_ids_from_table_rows() {
        let text = "\
| ID | Construct |\n\
| -- | -- |\n\
| TY-001 | foo |\n\
| RV-006-A | bar |\n\
| OP-003-G-1 | baz |\n\
not a table line\n\
| not-a-row | x |\n";
        let ids = extract_matrix_row_ids(text);
        assert!(ids.contains("TY-001"));
        assert!(ids.contains("RV-006-A"));
        assert!(ids.contains("OP-003-G-1"));
        assert_eq!(ids.len(), 3);
    }

    /// Writes `text` to a temp file (scoped to this test process) and
    /// returns its path. Tests inspect the returned path with
    /// [`load`] and delete the file on drop via the returned guard.
    fn write_tmp_rows(label: &str, text: &str) -> (PathBuf, TmpFileGuard) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("brcr-matrix-{label}-{nanos}.toml"));
        std::fs::write(&path, text).expect("must write temp rows.toml");
        let guard = TmpFileGuard(path.clone());
        (path, guard)
    }

    struct TmpFileGuard(PathBuf);

    impl Drop for TmpFileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// B3 (1): a row with `z_flags = ["mir-emit-retag"]` loads cleanly
    /// because the value sits inside `Z_FLAG_ALLOWLIST`.
    #[test]
    fn load_accepts_allow_listed_z_flag() {
        let text = "\
[[row]]
id = \"RJ-006\"
section = \"6.1\"
construct = \"DslRejectionKind::Statement\"
expectation = \"rejected\"
status = \"covered\"
cluster_key = \"RJ-006\"
snippet_path = \"RJ-006.rs\"
expected_rejection = { variant = \"DslRejection\", kind = \"Statement\" }
build_overrides = { z_flags = [\"mir-emit-retag\"] }
";
        let (path, _guard) = write_tmp_rows("allow-listed", text);
        let parsed = load(&path).expect("allow-listed z_flag value must load cleanly");
        assert_eq!(parsed.rows.len(), 1);
        let overrides = parsed.rows[0]
            .build_overrides
            .as_ref()
            .expect("build_overrides must round-trip");
        assert_eq!(overrides.z_flags, vec!["mir-emit-retag".to_string()]);
    }

    /// B3 (2): a row with a `z_flags` value outside the allow-list
    /// surfaces [`MatrixError::UnknownZFlag`] whose message names
    /// both the offending value and the allow-list itself.
    #[test]
    fn load_rejects_unknown_z_flag_with_typed_error() {
        let text = "\
[[row]]
id = \"BOGUS-001\"
section = \"6.1\"
construct = \"DslRejectionKind::Statement\"
expectation = \"rejected\"
status = \"covered\"
cluster_key = \"BOGUS\"
snippet_path = \"BOGUS-001.rs\"
expected_rejection = { variant = \"DslRejection\", kind = \"Statement\" }
build_overrides = { z_flags = [\"bogus-flag\"] }
";
        let (path, _guard) = write_tmp_rows("bogus-flag", text);
        let err = load(&path).expect_err("bogus-flag must surface MatrixError::UnknownZFlag");
        match &err {
            MatrixError::UnknownZFlag { row_id, flag } => {
                assert_eq!(row_id, "BOGUS-001");
                assert_eq!(flag, "bogus-flag");
            }
            other => panic!("expected MatrixError::UnknownZFlag, got {other:?}"),
        }
        let rendered = format!("{err}");
        assert!(
            rendered.contains("bogus-flag"),
            "error display must name the offending value; got: {rendered:?}"
        );
        assert!(
            rendered.contains("mir-emit-retag"),
            "error display must surface the allow-list; got: {rendered:?}"
        );
    }

    /// B3 (3): an unknown override key (e.g., `unknown_flag = true`)
    /// fails to deserialize because [`RowBuildOverrides`] is marked
    /// `deny_unknown_fields`. The error message must name the
    /// unknown field so future override-surface drift is caught at
    /// load time rather than silently accepted.
    #[test]
    fn load_rejects_unknown_build_overrides_field() {
        let text = "\
[[row]]
id = \"UF-001\"
section = \"6.1\"
construct = \"DslRejectionKind::Statement\"
expectation = \"rejected\"
status = \"covered\"
cluster_key = \"UF\"
snippet_path = \"UF-001.rs\"
expected_rejection = { variant = \"DslRejection\", kind = \"Statement\" }
build_overrides = { unknown_flag = true }
";
        let (path, _guard) = write_tmp_rows("unknown-field", text);
        let err = load(&path).expect_err("unknown override key must fail deserialization");
        match &err {
            MatrixError::Toml(_) => {}
            other => panic!("expected MatrixError::Toml from deny_unknown_fields, got {other:?}"),
        }
        let rendered = format!("{err}");
        assert!(
            rendered.contains("unknown_flag"),
            "TOML error must name the unknown field; got: {rendered:?}"
        );
    }
}
