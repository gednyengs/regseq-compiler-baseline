//! The L1 kbuild gate (backend-linux-c T11; GRILL C03/D03).
//!
//! Stages a linux_c emission (the TU + its Kbuild fragment) hermetically and
//! builds it as an out-of-tree module against the kernel tree named by
//! `RSEQ_LINUX_KDIR` (`make -C $RSEQ_LINUX_KDIR M=<staged dir> modules`).
//!
//! The C03 env contract, exactly:
//! - **unset** ⇒ skip with ONE visible notice naming the variable and the
//!   6.6 LTS campaign expectation — never a silent pass;
//! - **set but invalid** (no such directory / no `Makefile` / `make` not
//!   runnable) ⇒ hard FAIL — misconfiguration never masquerades as a skip;
//! - **compile error** ⇒ FAIL with `make`'s stderr verbatim (the caller
//!   surfaces it in the `FixtureVerdict` detail).
//!
//! The eval repo's script-15 variant is the canonical `RSEQ_LINUX_KDIR`
//! exporter (the only cross-repo coupling besides the oracle trace, I03).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The env contract variable (C03).
pub const KDIR_VAR: &str = "RSEQ_LINUX_KDIR";

/// One kbuild-gate run's outcome.
#[derive(Debug)]
pub enum KbuildOutcome {
    /// `RSEQ_LINUX_KDIR` is unset: the gate is skipped; `notice` is the one
    /// visible line the caller must print.
    Skipped {
        /// The visible skip notice (names the variable + the expectation).
        notice: String,
    },
    /// The module built with zero errors against the named tree.
    Built {
        /// The kernel release the tree identifies as (read from
        /// `include/config/kernel.release`), or `"unknown"`.
        kernel_version: String,
        /// `make`'s stderr (warnings are recorded, not gating — C01.2).
        warnings: String,
    },
}

/// A kbuild-gate failure (both C03 FAIL classes).
#[derive(Debug)]
pub enum KbuildError {
    /// `RSEQ_LINUX_KDIR` is set but does not name a usable kernel build
    /// tree (C03: misconfiguration is never a skip).
    InvalidKdir(String),
    /// The module did not compile; `stderr` is `make`'s output verbatim.
    CompileFailed {
        /// `make` stderr, verbatim.
        stderr: String,
    },
    /// I/O staging or spawning `make` failed.
    Io(std::io::Error),
}

impl std::fmt::Display for KbuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KbuildError::InvalidKdir(s) => write!(f, "kbuild gate misconfiguration: {s}"),
            KbuildError::CompileFailed { stderr } => {
                write!(f, "kbuild compile failed:\n{stderr}")
            }
            KbuildError::Io(err) => write!(f, "kbuild staging I/O error: {err}"),
        }
    }
}

impl std::error::Error for KbuildError {}

/// Build `tu` + `kbuild_fragment` as module `module_name` against the tree
/// named by the `RSEQ_LINUX_KDIR` env var (read here; [`kbuild_with_kdir`]
/// is the env-free seam the unit tests drive).
pub fn kbuild_module(
    tu: &str,
    kbuild_fragment: &str,
    module_name: &str,
) -> Result<KbuildOutcome, KbuildError> {
    kbuild_with_kdir(
        std::env::var_os(KDIR_VAR).map(PathBuf::from),
        tu,
        kbuild_fragment,
        module_name,
    )
}

/// The env-free gate core: `kdir = None` is the unset (skip) path.
pub fn kbuild_with_kdir(
    kdir: Option<PathBuf>,
    tu: &str,
    kbuild_fragment: &str,
    module_name: &str,
) -> Result<KbuildOutcome, KbuildError> {
    let Some(kdir) = kdir else {
        return Ok(KbuildOutcome::Skipped {
            notice: format!(
                "kbuild gate SKIPPED: {KDIR_VAR} is unset (point it at a configured \
                 Linux kernel build tree — the eval campaign's 6.6 LTS buildroot \
                 output — to enable the L1 compile gate)"
            ),
        });
    };
    // C03: set-but-invalid is a hard FAIL, never a skip.
    if !kdir.is_dir() {
        return Err(KbuildError::InvalidKdir(format!(
            "{KDIR_VAR}={} is not a directory",
            kdir.display()
        )));
    }
    if !kdir.join("Makefile").is_file() {
        return Err(KbuildError::InvalidKdir(format!(
            "{KDIR_VAR}={} has no Makefile (not a kernel build tree)",
            kdir.display()
        )));
    }

    // Hermetic staging: <module>.c + Kbuild.
    let stage = StageDir::new().map_err(KbuildError::Io)?;
    std::fs::write(stage.path().join(format!("{module_name}.c")), tu).map_err(KbuildError::Io)?;
    std::fs::write(stage.path().join("Kbuild"), kbuild_fragment).map_err(KbuildError::Io)?;

    // ARCH / CROSS_COMPILE pass-through: a cross-configured tree (the eval
    // campaign is arm64) needs both; the script-15 variant exports them
    // alongside RSEQ_LINUX_KDIR.
    let mut cmd = std::process::Command::new("make");
    cmd.arg("-C")
        .arg(&kdir)
        .arg(format!("M={}", stage.path().display()))
        .arg("modules");
    for var in ["ARCH", "CROSS_COMPILE", "LLVM"] {
        if let Some(v) = std::env::var_os(var) {
            cmd.arg(format!("{var}={}", v.to_string_lossy()));
        }
    }
    let output = cmd.output()
        .map_err(|err| {
            KbuildError::InvalidKdir(format!("`make` is not runnable: {err}"))
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(KbuildError::CompileFailed { stderr });
    }
    Ok(KbuildOutcome::Built {
        kernel_version: kernel_release(&kdir),
        warnings: stderr,
    })
}

/// The tree's kernel release string, best-effort.
fn kernel_release(kdir: &Path) -> String {
    std::fs::read_to_string(kdir.join("include/config/kernel.release"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// A process-unique staging directory removed on drop.
struct StageDir {
    path: PathBuf,
}

impl StageDir {
    fn new() -> std::io::Result<StageDir> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stem = format!("rseq-kbuild-{}-{n}", std::process::id());
        let path = std::env::temp_dir().join(stem);
        std::fs::create_dir_all(&path)?;
        Ok(StageDir { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_kdir_skips_with_a_visible_notice() {
        let outcome = kbuild_with_kdir(None, "", "obj-m += x.o\n", "x").unwrap();
        let KbuildOutcome::Skipped { notice } = outcome else {
            panic!("unset KDIR must skip");
        };
        assert!(notice.contains(KDIR_VAR), "the notice names the variable");
        assert!(notice.contains("6.6"), "the notice names the expectation");
    }

    #[test]
    fn missing_dir_is_a_hard_failure_not_a_skip() {
        let err = kbuild_with_kdir(
            Some(PathBuf::from("/nonexistent/rseq-kdir")),
            "",
            "obj-m += x.o\n",
            "x",
        )
        .unwrap_err();
        assert!(matches!(err, KbuildError::InvalidKdir(ref s) if s.contains(KDIR_VAR)));
    }

    #[test]
    fn dir_without_makefile_is_a_hard_failure() {
        let dir = std::env::temp_dir().join(format!("rseq-kbuild-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = kbuild_with_kdir(Some(dir.clone()), "", "obj-m += x.o\n", "x").unwrap_err();
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(err, KbuildError::InvalidKdir(ref s) if s.contains("Makefile")));
    }

    #[test]
    fn compile_error_fails_with_make_stderr() {
        // A fake tree whose Makefile errors: classification must be
        // CompileFailed carrying the stderr verbatim.
        let dir = std::env::temp_dir().join(format!(
            "rseq-kbuild-test-make-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Makefile"),
            "modules:\n\t@echo \"fake-tree: no such config\" 1>&2; exit 2\n",
        )
        .unwrap();
        let err = kbuild_with_kdir(Some(dir.clone()), "int x;\n", "obj-m += x.o\n", "x")
            .unwrap_err();
        std::fs::remove_dir_all(&dir).ok();
        let KbuildError::CompileFailed { stderr } = err else {
            panic!("a failing make must classify as CompileFailed, got {err:?}");
        };
        assert!(stderr.contains("fake-tree: no such config"));
    }
}
