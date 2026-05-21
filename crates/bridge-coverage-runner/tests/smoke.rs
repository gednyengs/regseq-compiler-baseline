//! End-to-end smoke tests for the bridge-coverage-runner binary.
//!
//! Each test spawns the runner via `env!("CARGO_BIN_EXE_bridge-coverage-runner")`
//! against a per-test mini-corpus written into a tempdir. The
//! committed `matrix_metadata/rows.toml` is NOT loaded by the mini-
//! sidecar tests; only the full-demo-set test reads it. This
//! isolation lets future demo-set expansion proceed without
//! reauthoring the smoke tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn project_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut current = manifest;
    loop {
        if current.join("Cargo.toml").exists()
            && current
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n != "bridge-coverage-runner")
                .unwrap_or(true)
        {
            // Walk up until we find the workspace root (the parent of
            // the `crates/` directory).
            if current.join("crates").exists() {
                return current;
            }
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => panic!("workspace root not found from CARGO_MANIFEST_DIR"),
        }
    }
}

fn matrix_md_path() -> PathBuf {
    project_root().join("docs/discussions/regseq-model/bridge-coverage-matrix.md")
}

fn committed_snippets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("snippets")
}

fn committed_metadata_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("matrix_metadata")
        .join("rows.toml")
}

fn make_tempdir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("bridge-coverage-runner-smoke-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("must create tempdir");
    fs::create_dir_all(dir.join("snippets")).expect("must create snippets subdir");
    fs::create_dir_all(dir.join("report")).expect("must create report subdir");
    dir
}

fn copy_committed_snippet(tmp: &Path, row_id: &str) {
    let src = committed_snippets_dir().join(format!("{row_id}.rs"));
    let dst = tmp.join("snippets").join(format!("{row_id}.rs"));
    fs::copy(&src, &dst)
        .unwrap_or_else(|err| panic!("must copy {} to {}: {err}", src.display(), dst.display()));
}

fn write_positive_mini_sidecar(tmp: &Path, rows: &[(&str, &str, &str, &str)]) {
    // rows: (id, cluster_key, predicate kind, predicate arg)
    let mut body = String::new();
    body.push_str("# Mini-corpus for smoke test (positive rows).\n");
    for (id, cluster_key, kind, arg) in rows {
        body.push_str(&format!(
            "[[row]]\n\
id = \"{id}\"\n\
section = \"smoke\"\n\
construct = \"smoke {id}\"\n\
expectation = \"positive\"\n\
status = \"covered\"\n\
cluster_key = \"{cluster_key}\"\n\
snippet_path = \"{id}.rs\"\n\
inspect = {{ kind = \"{kind}\", arg = \"{arg}\" }}\n\n"
        ));
    }
    fs::write(tmp.join("rows.toml"), body).expect("must write rows.toml");
}

fn run_runner(tmp: &Path) -> std::process::Output {
    let runner = env!("CARGO_BIN_EXE_bridge-coverage-runner");
    Command::new(runner)
        .arg("--metadata")
        .arg(tmp.join("rows.toml"))
        .arg("--snippets")
        .arg(tmp.join("snippets"))
        .arg("--matrix-md")
        .arg(matrix_md_path())
        .arg("--out")
        .arg(tmp.join("report"))
        .output()
        .expect("runner subprocess must spawn")
}

#[test]
fn smoke_3_positive_rows() {
    let tmp = make_tempdir("pos3");
    copy_committed_snippet(&tmp, "TY-018");
    copy_committed_snippet(&tmp, "IK-002");
    copy_committed_snippet(&tmp, "IK-005");
    write_positive_mini_sidecar(
        &tmp,
        &[
            ("TY-018", "TY-018", "TyVariant", "Enum"),
            ("IK-002", "IK-002", "ItemKind", "Static"),
            ("IK-005", "IK-005", "ItemKind", "Struct"),
        ],
    );

    let output = run_runner(&tmp);
    assert!(
        output.status.success(),
        "runner exited non-zero: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report =
        fs::read_to_string(tmp.join("report").join("report.md")).expect("report.md must exist");
    assert!(report.contains("# Bridge Coverage Runner Report"));
    assert!(report.contains("TY-018"));
    assert!(report.contains("IK-002"));
    assert!(report.contains("IK-005"));
    assert!(report.contains("## Summary"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn smoke_drift_hard_fail_on_invented_row_id() {
    let tmp = make_tempdir("drift");
    // Write a snippet named for a row id absent from the matrix.
    fs::write(
        tmp.join("snippets").join("ZZ-999.rs"),
        "#![no_std]\npub fn f() {}\n",
    )
    .expect("must write ZZ-999 snippet");
    write_positive_mini_sidecar(&tmp, &[("ZZ-999", "ZZ-999", "TyVariant", "Bool")]);

    let output = run_runner(&tmp);
    assert!(
        !output.status.success(),
        "runner expected to exit non-zero on drift; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        stderr.contains("drift-hard-fail"),
        "expected drift-hard-fail in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("ZZ-999"),
        "expected stderr to name invented row ZZ-999; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn smoke_full_demo_set_runs_to_completion() {
    let tmp = make_tempdir("full");
    let report_dir = tmp.join("report");
    let runner = env!("CARGO_BIN_EXE_bridge-coverage-runner");

    let output = Command::new(runner)
        .arg("--metadata")
        .arg(committed_metadata_path())
        .arg("--snippets")
        .arg(committed_snippets_dir())
        .arg("--matrix-md")
        .arg(matrix_md_path())
        .arg("--out")
        .arg(&report_dir)
        .output()
        .expect("runner subprocess must spawn");

    assert!(
        output.status.success(),
        "runner exited non-zero on full demo set: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report = fs::read_to_string(report_dir.join("report.md")).expect("report.md must exist");
    for row_id in [
        "TY-018", "IK-002", "IK-005", "TY-014-A", "FF-011", "IX-002", "RJ-002", "RJ-016", "RJ-026",
        "RJ-028-A",
    ] {
        assert!(
            report.contains(row_id),
            "report must mention row {row_id}; got: {report}"
        );
    }
    assert!(report.contains("## Summary"));
    // The committed sidecar grows monotonically as F* phases land
    // more rows; assert Total == Pass and Fail == 0 instead of
    // hardcoding a row count.
    assert!(
        report.contains("| Fail | 0 |"),
        "expected zero failures in committed-set report; got: {report}"
    );

    let _ = fs::remove_dir_all(&tmp);
}
