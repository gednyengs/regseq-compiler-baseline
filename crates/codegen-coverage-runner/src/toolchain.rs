//! Host-`cc` subprocess toolchain (Doc 3 sections 4 and 7) -- the
//! emit-independent half of the pipeline (stages 4-5).
//!
//! Takes a C source *string* (not a model), so it is unit-testable with
//! hand-written C. Each invocation compiles `tu.c` + a generated `main.c`
//! in a per-call hermetic temp dir with the pinned flags, runs the linked
//! binary, and captures stdout / stderr / exit code. The temp dir is
//! cleaned on every path (RAII), keeping the host isolated and the
//! harness crash-safe.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Captured result of running a compiled fixture binary.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Process exit code, or `None` if the process was killed by a
    /// signal (e.g. `abort()` from `regseq_panic`).
    pub exit_code: Option<i32>,
}

/// Errors from the compile/run half of the pipeline.
#[derive(Debug)]
pub enum ToolchainError {
    /// I/O error setting up the temp dir, writing sources, or spawning a
    /// subprocess.
    Io(std::io::Error),
    /// `cc` exited non-zero compiling the translation unit (stage-4
    /// failure). Carries the compiler's stderr.
    CompileFailed {
        /// Captured `cc` stderr.
        stderr: String,
    },
    /// A symbolic `expected.exit` identifier could not be resolved to a
    /// numeric on the fixture's libc: the probe compiled and ran but did not
    /// print a parseable integer.
    SymbolResolution {
        /// The errno identifier that failed to resolve.
        symbol: String,
        /// Why resolution failed.
        detail: String,
    },
}

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolchainError::Io(err) => write!(f, "toolchain I/O error: {err}"),
            ToolchainError::CompileFailed { stderr } => {
                write!(f, "cc failed to compile the translation unit:\n{stderr}")
            }
            ToolchainError::SymbolResolution { symbol, detail } => {
                write!(
                    f,
                    "could not resolve symbol {symbol:?} on the fixture libc: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ToolchainError {}

impl From<std::io::Error> for ToolchainError {
    fn from(value: std::io::Error) -> Self {
        ToolchainError::Io(value)
    }
}

/// The Doc-2 D3 entry shim generator + shim shape, re-exported from the backend
/// (the single source of truth shared with `cargo-regseq`'s `--emit bin`).
pub use regseq_backends::{generate_main_c, EntryShim};
/// The pinned host-`cc` flags, re-exported from `regseq-config` (shared with
/// `cargo-regseq`'s linker).
pub use regseq_config::CC_FLAGS;

/// Record-only fake-clock implementation of the timed-cpu-delay-and-sleep host
/// hooks (TCD construct, contract C06 / GRILL D08).
///
/// The backend emits only the extern DECLARATIONS + CALLS (see
/// `generic_c::prelude::DELAY_HOOKS` / `emit_delay`); this harness owns the
/// IMPLEMENTATION, so the fake clock is never baked into compiler output (the
/// T05/T06 split). It is linked beside the emitted TU only when the TU
/// references the hooks (a delay intrinsic was lowered).
///
/// Each hook records an ordered `{kind, ns, seq}` event by printing it at call
/// time — so the events interleave with the `regseq_mmio_*` / IRQ trace by
/// execution order, making "settle is after the reset write, before the
/// restore" directly observable — and advances a VIRTUAL clock by `ns`. No real
/// time passes. The stdout stream IS the event log; tests diff it against
/// `expected.stdout`. Hook identity preserves author intent (`busy_wait`
/// vs `sleep`), so the oracle proves the compiler never silently converts one
/// to the other (I04).
const DELAY_HARNESS_C: &str = r#"/* Timed-cpu-delay-and-sleep host fake clock (TCD construct, C06/D08).
 * Harness-linked record-only implementation; no real time passes. */
#include <stdint.h>
#include <stdio.h>

static uint64_t regseq_delay_clock_ns = 0;
static uint64_t regseq_delay_seq = 0;

void regseq_delay_busy_wait_ns(uint64_t ns) {
    regseq_delay_clock_ns += ns;
    printf("DELAY busy_wait ns=%llu seq=%llu t=%lluns\n",
           (unsigned long long)ns,
           (unsigned long long)regseq_delay_seq++,
           (unsigned long long)regseq_delay_clock_ns);
}

void regseq_delay_sleep_ns(uint64_t ns) {
    regseq_delay_clock_ns += ns;
    printf("DELAY sleep ns=%llu seq=%llu t=%lluns\n",
           (unsigned long long)ns,
           (unsigned long long)regseq_delay_seq++,
           (unsigned long long)regseq_delay_clock_ns);
}
"#;

/// Whether the emitted TU references the timed-delay host hooks (i.e. a delay
/// intrinsic was lowered), gating whether [`DELAY_HARNESS_C`] is linked.
fn tu_uses_delay_hooks(tu_c: &str) -> bool {
    tu_c.contains("regseq_delay_busy_wait_ns") || tu_c.contains("regseq_delay_sleep_ns")
}

/// Compile `tu_c` together with the generated `main.c` for the backend-resolved
/// `shim` using the pinned flags, run the linked binary in a hermetic temp dir,
/// and capture stdout / stderr / exit code.
///
/// The `shim` shape comes from the backend (no re-derivation, contract C04). A
/// non-zero `cc` exit surfaces as [`ToolchainError::CompileFailed`] (stage-4
/// failure). The temp dir is removed on every return path.
pub fn compile_and_run(
    tu_c: &str,
    shim: &EntryShim,
    cc: &Path,
) -> Result<RunOutput, ToolchainError> {
    let scratch = ScratchDir::new()?;
    let dir = scratch.path();

    let tu_path = dir.join("tu.c");
    let main_path = dir.join("main.c");
    let bin_path = dir.join("fixture_bin");
    std::fs::write(&tu_path, tu_c)?;
    std::fs::write(&main_path, generate_main_c(shim))?;

    let mut compile = Command::new(cc);
    compile
        .args(CC_FLAGS)
        .arg("-o")
        .arg(&bin_path)
        .arg(&tu_path)
        .arg(&main_path);
    // Timed-cpu-delay-and-sleep (TCD construct, C06/D08): when the TU lowered a
    // delay intrinsic it references the extern delay hooks, so link the
    // harness-owned record-only fake-clock implementation. Skipped otherwise so
    // a delay-free fixture's build is unchanged.
    let delay_path = dir.join("regseq_delay_harness.c");
    if tu_uses_delay_hooks(tu_c) {
        std::fs::write(&delay_path, DELAY_HARNESS_C)?;
        compile.arg(&delay_path);
    }
    let compile = compile.output()?;
    if !compile.status.success() {
        return Err(ToolchainError::CompileFailed {
            stderr: String::from_utf8_lossy(&compile.stderr).into_owned(),
        });
    }

    let run = Command::new(&bin_path).output()?;
    Ok(RunOutput {
        stdout: String::from_utf8_lossy(&run.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&run.stderr).into_owned(),
        exit_code: run.status.code(),
    })
}

/// Resolve a symbolic `expected.exit` identifier to its numeric value on the
/// SAME `cc`/libc that compiles and runs the fixture, by compiling and running
/// a tiny probe translation unit that prints the constant.
///
/// The literal `"0"` short-circuits to `0` with no probe (the exit-0 default,
/// so an absent or `"0"` field costs nothing). Any other `symbol` must be a
/// bare errno identifier (a C identifier); it is spelled into a
/// `printf("%d", <symbol>)` probe over `<errno.h>`, and a symbol the libc does
/// not define fails the probe compile and surfaces as
/// [`ToolchainError::CompileFailed`]. A non-identifier -- in particular a raw
/// numeric like `"110"` or a C expression -- is rejected up front as a
/// [`ToolchainError::SymbolResolution`], so no host-libc numeric is ever stored
/// in a manifest and no arbitrary C is interpolated (contract C04, requirement
/// R10, invariant I03). The temp dir is removed on every return path.
pub fn resolve_errno(symbol: &str, cc: &Path) -> Result<i32, ToolchainError> {
    if symbol == "0" {
        return Ok(0);
    }
    if !is_errno_identifier(symbol) {
        return Err(ToolchainError::SymbolResolution {
            symbol: symbol.to_string(),
            detail: "expected an errno identifier (a C identifier) or the literal \"0\"; \
                     a raw numeric or expression is not allowed -- no numeric is stored in a \
                     manifest (C04/R10/I03)"
                .to_string(),
        });
    }
    let scratch = ScratchDir::new()?;
    let dir = scratch.path();

    let probe_path = dir.join("probe.c");
    let bin_path = dir.join("probe_bin");
    let probe = format!(
        "#include <errno.h>\n\
         #include <stdio.h>\n\
         int main(void) {{ printf(\"%d\\n\", {symbol}); return 0; }}\n"
    );
    std::fs::write(&probe_path, probe)?;

    let compile = Command::new(cc)
        .args(CC_FLAGS)
        .arg("-o")
        .arg(&bin_path)
        .arg(&probe_path)
        .output()?;
    if !compile.status.success() {
        return Err(ToolchainError::CompileFailed {
            stderr: String::from_utf8_lossy(&compile.stderr).into_owned(),
        });
    }

    let run = Command::new(&bin_path).output()?;
    let printed = String::from_utf8_lossy(&run.stdout);
    printed
        .trim()
        .parse::<i32>()
        .map_err(|err| ToolchainError::SymbolResolution {
            symbol: symbol.to_string(),
            detail: format!("probe printed {printed:?}, not an integer ({err})"),
        })
}

/// Whether `s` has the shape of a C identifier (the shape an errno macro
/// takes): a non-empty string whose first character is an ASCII letter or
/// underscore, with the rest ASCII letters, digits, or underscores. This is
/// the guard that keeps a raw numeric (e.g. `"110"`) or a C expression out of
/// `expected.exit`, so only a symbolic identifier (or the special `"0"`) is
/// ever resolved (contract C04, invariant I03).
fn is_errno_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A process-unique scratch directory removed on drop.
struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Create a hermetic temp dir under [`std::env::temp_dir`] with a
    /// process-unique stem (pid + a monotonic counter), so concurrent
    /// invocations never collide.
    fn new() -> std::io::Result<ScratchDir> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stem = format!("ccr-{}-{n}", std::process::id());
        let path = std::env::temp_dir().join(stem);
        std::fs::create_dir_all(&path)?;
        Ok(ScratchDir { path })
    }

    /// The scratch directory path.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
