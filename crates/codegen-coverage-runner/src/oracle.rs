//! The dual oracle (Doc 3 section 2): a byte-exact snapshot oracle
//! (stage 3) and a behavioral oracle (stage 6 stdout, or stage 5/6 panic
//! stderr + non-zero exit). Pure comparison logic -- strings in, verdict
//! out -- so it is unit-testable independently of emit and `cc`.
//!
//! The harness NEVER rewrites a golden here: golden (re)generation is the
//! explicit human-invoked `--bless` path, not an auto-bless (Doc 3
//! section 2).

use crate::toolchain::RunOutput;

/// A failed oracle comparison, carrying a short diff summary for the
/// report (the first differing line plus a little context).
#[derive(Debug, PartialEq, Eq)]
pub struct OracleMismatch {
    /// Human-readable summary of where and how the two sides diverged.
    pub summary: String,
}

/// Stage 3: emitted `.c` vs the committed golden `.c`. Byte-exact.
pub fn snapshot(emitted: &str, golden: &str) -> Result<(), OracleMismatch> {
    compare_exact("emitted C", emitted, golden)
}

/// Stage 6 (expect = pass): captured stdout vs the golden stdout.
/// Byte-exact.
pub fn behavioral_stdout(captured: &str, golden: &str) -> Result<(), OracleMismatch> {
    compare_exact("stdout", captured, golden)
}

/// Stage 5/6 (expect = pass): assert the process exited with exactly
/// `expected_exit` AND that stderr is empty.
///
/// `expected_exit` is the numeric resolved from the manifest's symbolic
/// `expected.exit` (default `0`). The empty-stderr guard is what stops a
/// `panic = abort` SIGABRT -- which dies with a non-zero status WITH a panic
/// message on stderr -- from masquerading as a clean non-zero errno return
/// (requirement R11). Stdout is byte-compared separately by
/// [`behavioral_stdout`] (stage 6).
pub fn behavioral_exit(run: &RunOutput, expected_exit: i32) -> Result<(), OracleMismatch> {
    if run.exit_code != Some(expected_exit) {
        return Err(OracleMismatch {
            summary: format!(
                "expected process exit {expected_exit}, got exit_code {:?}",
                run.exit_code
            ),
        });
    }
    if !run.stderr.is_empty() {
        return Err(OracleMismatch {
            summary: format!(
                "expected empty stderr for an exit-{expected_exit} pass, got {:?}",
                run.stderr
            ),
        });
    }
    Ok(())
}

/// Stage 5/6 (expect = panic): assert the process did NOT exit 0 and its
/// stderr matches the golden stderr byte-for-byte. A `None` exit code (a
/// signal death, e.g. `abort()`) counts as non-zero.
pub fn behavioral_panic(run: &RunOutput, golden_stderr: &str) -> Result<(), OracleMismatch> {
    if run.exit_code == Some(0) {
        return Err(OracleMismatch {
            summary: "expected a panic (non-zero exit) but the process exited 0".to_string(),
        });
    }
    compare_exact("stderr", &run.stderr, golden_stderr)
}

/// Byte-exact comparison; on mismatch, report the first differing line
/// (1-based) with both sides, or a length note if one side is a prefix
/// of the other.
fn compare_exact(label: &str, actual: &str, expected: &str) -> Result<(), OracleMismatch> {
    if actual == expected {
        return Ok(());
    }
    let mut actual_lines = actual.lines();
    let mut expected_lines = expected.lines();
    let mut line_no = 0;
    loop {
        line_no += 1;
        match (actual_lines.next(), expected_lines.next()) {
            (Some(a), Some(e)) if a == e => continue,
            (Some(a), Some(e)) => {
                return Err(OracleMismatch {
                    summary: format!(
                        "{label} differs at line {line_no}:\n  actual:   {a:?}\n  expected: {e:?}"
                    ),
                });
            }
            (Some(a), None) => {
                return Err(OracleMismatch {
                    summary: format!(
                        "{label} has an extra line {line_no}: {a:?} (expected end of output)"
                    ),
                });
            }
            (None, Some(e)) => {
                return Err(OracleMismatch {
                    summary: format!(
                        "{label} is missing line {line_no}: {e:?} (output ended early)"
                    ),
                });
            }
            (None, None) => {
                // Equal line-by-line but unequal overall: a trailing-newline
                // difference that `lines()` elides.
                return Err(OracleMismatch {
                    summary: format!(
                        "{label} differs only in trailing newline(s) \
                         (actual {} bytes, expected {} bytes)",
                        actual.len(),
                        expected.len()
                    ),
                });
            }
        }
    }
}
