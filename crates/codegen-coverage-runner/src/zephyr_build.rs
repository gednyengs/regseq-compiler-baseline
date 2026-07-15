//! The L1 west build gate (backend-zephyr-c T11; GRILL C03/D08).
//!
//! Stages a zephyr_c emission hermetically as a minimal Zephyr APP —
//! CMakeLists, prj.conf (the A-9 pins), a DUT devicetree overlay carrying a
//! status-okay `regseq,pl011` node, the generated binding yaml under the
//! app-local `dts/bindings/`, a trivial `main`, and the generated TU as an
//! app source — and builds it via `west build -b qemu_cortex_a53` against
//! the west workspace named by `RSEQ_ZEPHYR_WORKSPACE`. The app staging is
//! REQUIRED collateral, not convenience: the TU only meaningfully compiles
//! against a devicetree carrying the status-okay node — otherwise the
//! instance block and both `BUILD_ASSERT`s never expand (D08).
//!
//! The C03 env contract, exactly:
//! - **unset** ⇒ skip with ONE visible notice naming the variable and the
//!   v4.4.0 eval-workspace expectation — never a silent pass;
//! - **set but invalid** (no such directory / no `.west` topdir marker /
//!   `west` not invocable) ⇒ hard FAIL — misconfiguration never
//!   masquerades as a skip;
//! - **build error** ⇒ FAIL with west/cmake stderr verbatim (the caller
//!   surfaces it in the `FixtureVerdict` detail).
//!
//! The eval repo's `pl011/zephyr` build script is the canonical
//! `RSEQ_ZEPHYR_WORKSPACE` exporter + venv activator (T12; the only
//! cross-repo coupling besides the oracle trace, I03).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The env contract variable (C03).
pub const WORKSPACE_VAR: &str = "RSEQ_ZEPHYR_WORKSPACE";

/// The pinned board target (D08/D09; versions.lock SD-Z2).
pub const BOARD: &str = "qemu_cortex_a53";

/// The PCIe-variant board target (backend-zephyr-c-e1000 D08/plan decision
/// 2: qemu_x86 primary — its base DT models the e1000 as a pcie0 child,
/// the exact PCIe-over-DT form; qemu_x86_64 is the named fallback, final
/// per the D09 spike's item (d) — a flip rides a gate-reported amendment).
pub const PCIE_BOARD: &str = "qemu_x86";

/// One west-gate run's outcome.
#[derive(Debug)]
pub enum WestOutcome {
    /// `RSEQ_ZEPHYR_WORKSPACE` is unset: the gate is skipped; `notice` is
    /// the one visible line the caller must print.
    Skipped {
        /// The visible skip notice (names the variable + the expectation).
        notice: String,
    },
    /// The staged app built with zero errors against the named workspace.
    Built {
        /// The Zephyr version the workspace identifies as (read from
        /// `<topdir>/zephyr/VERSION`), or `"unknown"`.
        zephyr_version: String,
        /// The build's stderr (warnings are recorded, not gating — C01.2).
        warnings: String,
    },
}

/// A west-gate failure (both C03 FAIL classes).
#[derive(Debug)]
pub enum WestError {
    /// `RSEQ_ZEPHYR_WORKSPACE` is set but does not name a usable west
    /// topdir, or `west` is not invocable (C03: misconfiguration is never
    /// a skip).
    InvalidWorkspace(String),
    /// The staged app did not build; `stderr` is the build output verbatim.
    BuildFailed {
        /// west/cmake stderr, verbatim.
        stderr: String,
    },
    /// I/O staging or spawning `west` failed.
    Io(std::io::Error),
}

impl std::fmt::Display for WestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WestError::InvalidWorkspace(s) => {
                write!(f, "west gate misconfiguration: {s}")
            }
            WestError::BuildFailed { stderr } => {
                write!(f, "west build failed:\n{stderr}")
            }
            WestError::Io(err) => write!(f, "west staging I/O error: {err}"),
        }
    }
}

impl std::error::Error for WestError {}

/// The staged DUT's bus form: it decides the board, the overlay node shape,
/// and the prj.conf pin set.
pub enum StagedBus<'a> {
    /// Platform/AMBA: a memory-mapped DUT node under `/soc` carrying `reg`
    /// (the first-light form; board [`BOARD`]).
    Platform {
        /// The DUT region `(base, size)` for the overlay node's `reg` cells.
        region: (u64, u64),
        /// Optional raw `interrupts` cells for the overlay node
        /// (device-interrupt construct #6: an interrupt-bearing TU's
        /// `DT_INST_IRQN(0)` needs the property), e.g. `"<0 8 2 0>"` =
        /// `<GIC_SPI 8 IRQ_TYPE_LEVEL 0>` raw. The west gate is a COMPILE
        /// check — the cells must be well-formed; the runtime line number is
        /// the eval campaign's to pin. `None` emits no property (every
        /// pre-construct fixture, byte-identical overlay). This is a platform
        /// node fact, so it lives in the variant — PCIe lines reach the
        /// device through the `pcie0` controller, not a node property.
        interrupts: Option<&'a str>,
    },
    /// PCIe (backend-zephyr-c-e1000 T07): a vendor/device-matched DUT node
    /// under the board's `pcie0` bus — NO `reg` (the C05 binding requires
    /// none; BAR sizing is probe-time) — with the CONFIG_PCIE prj.conf pins
    /// (the O4 set, discovered fail-loud at first build). Board
    /// [`PCIE_BOARD`].
    Pcie {
        /// The DUT vendor id (overlay `vendor-id` cell).
        vendor_id: u16,
        /// The DUT device id (overlay `device-id` cell).
        device_id: u16,
    },
}

/// The staged-app inputs: the two generated artifacts plus naming facts.
pub struct StagedDriver<'a> {
    /// The generated TU text.
    pub tu: &'a str,
    /// The generated DT binding yaml.
    pub binding_yaml: &'a str,
    /// The sanitized driver name (`ZephyrEmit::driver_name`).
    pub driver_name: &'a str,
    /// The DT compatible the binding declares (`"regseq,pl011"`); names the
    /// staged binding file and the overlay node's compatible.
    pub compatible: &'a str,
    /// The DUT's bus form (board + overlay + prj.conf selector). The
    /// [`StagedBus::Platform`] variant carries any DT `interrupts` cells.
    pub bus: StagedBus<'a>,
}

/// Build the staged app against the workspace named by
/// `RSEQ_ZEPHYR_WORKSPACE` (read here; [`west_build_with_workspace`] is the
/// env-free seam the unit tests drive).
pub fn west_build(driver: &StagedDriver<'_>) -> Result<WestOutcome, WestError> {
    west_build_with_workspace(std::env::var_os(WORKSPACE_VAR).map(PathBuf::from), driver)
}

/// The env-free gate core: `workspace = None` is the unset (skip) path.
pub fn west_build_with_workspace(
    workspace: Option<PathBuf>,
    driver: &StagedDriver<'_>,
) -> Result<WestOutcome, WestError> {
    let Some(topdir) = workspace else {
        return Ok(WestOutcome::Skipped {
            notice: format!(
                "west gate SKIPPED: {WORKSPACE_VAR} is unset (point it at the eval \
                 west topdir — the pinned Zephyr v4.4.0 workspace with the SDK \
                 registered and `west` on PATH — to enable the L1 build gate)"
            ),
        });
    };
    // C03: set-but-invalid is a hard FAIL, never a skip.
    if !topdir.is_dir() {
        return Err(WestError::InvalidWorkspace(format!(
            "{WORKSPACE_VAR}={} is not a directory",
            topdir.display()
        )));
    }
    if !topdir.join(".west").is_dir() {
        return Err(WestError::InvalidWorkspace(format!(
            "{WORKSPACE_VAR}={} has no .west/ (not a west topdir)",
            topdir.display()
        )));
    }
    let west_ok = std::process::Command::new("west")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match west_ok {
        Ok(status) if status.success() => {}
        Ok(_) | Err(_) => {
            return Err(WestError::InvalidWorkspace(
                "`west` is not invocable on PATH (the eval build script is the \
                 canonical exporter + venv activator, C03)"
                    .into(),
            ));
        }
    }

    // Hermetic app staging (D08).
    let stage = StageDir::new().map_err(WestError::Io)?;
    let app = stage.path().join("app");
    std::fs::create_dir_all(app.join("src")).map_err(WestError::Io)?;
    std::fs::create_dir_all(app.join("dts/bindings")).map_err(WestError::Io)?;

    let drv = driver.driver_name;
    std::fs::write(
        app.join("CMakeLists.txt"),
        format!(
            "# Hermetic L1 gate app (backend-zephyr-c T11): compiles the generated\n\
             # driver TU against the real Zephyr tree + a DT carrying the DUT node.\n\
             cmake_minimum_required(VERSION 3.20.0)\n\
             find_package(Zephyr REQUIRED HINTS $ENV{{ZEPHYR_BASE}})\n\
             project(rseq_zephyr_gate)\n\
             target_sources(app PRIVATE src/main.c src/{drv}.c)\n"
        ),
    )
    .map_err(WestError::Io)?;
    // The A-9 logging pins (the generated TU documents them; the gate's
    // prj.conf asserts them for the build) + the teardown pin
    // (spec-convergence-pl011 C04: CONFIG_DEVICE_DEINIT_SUPPORT defaults OFF
    // and Z_DEVICE_OPS silently drops deinit_fn without it,
    // device.h:1175-1180 — the build would pass and device_deinit() would
    // return -ENOTSUP at runtime). The PCIe variant additionally pins the
    // O4 set (backend-zephyr-c-e1000 T07, discovered fail-loud at first
    // build): CONFIG_PCIE compiles drivers/pcie/host + the DEVICE_PCIE
    // context machinery the generated scaffold consumes.
    let mut prj_conf = match driver.bus {
        StagedBus::Platform { .. } => {
            "CONFIG_LOG=y\nCONFIG_LOG_MODE_DEFERRED=y\nCONFIG_CBPRINTF_COMPLETE=y\nCONFIG_CBPRINTF_FULL_INTEGRAL=y\nCONFIG_DEVICE_DEINIT_SUPPORT=y\n".to_string()
        }
        StagedBus::Pcie { .. } => {
            "CONFIG_LOG=y\nCONFIG_LOG_MODE_DEFERRED=y\nCONFIG_CBPRINTF_COMPLETE=y\nCONFIG_CBPRINTF_FULL_INTEGRAL=y\nCONFIG_DEVICE_DEINIT_SUPPORT=y\nCONFIG_PCIE=y\n".to_string()
        }
    };
    if driver.tu.contains("DEVICE_API(uart,") {
        // C1-serial: compile every emitted uart_driver_api slot, not only the
        // polling prefix selected by a board default. These are build-harness
        // facts; the portable model remains capability-gated.
        prj_conf.push_str(
            "CONFIG_SERIAL=y\nCONFIG_UART_INTERRUPT_DRIVEN=y\n\
             CONFIG_UART_USE_RUNTIME_CONFIGURE=y\n",
        );
    }
    if driver.tu.contains("<zephyr/drivers/dma.h>") {
        // C2-DMA: force the generated channel helpers through the enabled
        // public DMA surface instead of compiling only a disabled-header
        // configuration.
        prj_conf.push_str("CONFIG_DMA=y\n");
    }
    std::fs::write(app.join("prj.conf"), prj_conf).map_err(WestError::Io)?;
    // The DUT overlay, per bus form.
    let overlay = match driver.bus {
        StagedBus::Platform { region, interrupts } => {
            // A status-okay node under /soc (the qemu-virt-a53 soc node is
            // #address-cells=2 / #size-cells=2). The optional `interrupts`
            // cells (device-interrupt construct #6) ride the platform node —
            // an interrupt-bearing TU's `DT_INST_IRQN(0)` needs the property.
            let (base, size) = region;
            let base_hi = base >> 32;
            let base_lo = base & 0xffff_ffff;
            let interrupts_prop = interrupts
                .map(|cells| format!("\t\t\tinterrupts = {cells};\n"))
                .unwrap_or_default();
            format!(
                "/* The eval DUT node (GRILL D04): the second PL011, matched ONLY by\n \
                 * the generated driver's `{}` compatible. */\n\
                 / {{\n\tsoc {{\n\t\tregseq_dut: uart@{base:x} {{\n\t\t\tcompatible = \"{}\";\n\t\t\treg = <{base_hi:#x} {base_lo:#x} 0x00 {size:#x}>;\n{interrupts_prop}\t\t\tstatus = \"okay\";\n\t\t}};\n\t}};\n}};\n",
                driver.compatible, driver.compatible
            )
        }
        StagedBus::Pcie {
            vendor_id,
            device_id,
        } => {
            // The PCIe-over-DT form (backend-zephyr-c-e1000 D08(4)): a
            // vendor/device-matched child under the board's pcie0 bus, NO
            // reg (probe-time BAR sizing) — the witnessed qemu_x86.dts eth0
            // idiom, hermetically owned by the generated compatible.
            format!(
                "/* The hermetic e1000 DUT node (backend-zephyr-c-e1000 D08(4)):\n \
                 * matched ONLY by the generated driver's `{}` compatible;\n \
                 * vendor-id/device-id feed the DEVICE_PCIE context (PCIE_DT_ID). */\n\
                 &pcie0 {{\n\tregseq_dut: regseq_dut {{\n\t\tcompatible = \"{}\";\n\t\tvendor-id = <{vendor_id:#x}>;\n\t\tdevice-id = <{device_id:#x}>;\n\t\tstatus = \"okay\";\n\t}};\n}};\n",
                driver.compatible, driver.compatible
            )
        }
    };
    std::fs::write(app.join("app.overlay"), overlay).map_err(WestError::Io)?;
    // The generated binding, discoverable via the app-local dts/bindings/.
    std::fs::write(
        app.join("dts/bindings")
            .join(format!("{}.yaml", driver.compatible.replace(',', "_"))),
        driver.binding_yaml,
    )
    .map_err(WestError::Io)?;
    std::fs::write(
        app.join("src/main.c"),
        "/* Hermetic gate app: the driver under test initializes at boot via\n * its DEVICE_DT_INST_DEFINE; main has nothing to do. */\nint main(void)\n{\n\treturn 0;\n}\n",
    )
    .map_err(WestError::Io)?;
    std::fs::write(app.join("src").join(format!("{drv}.c")), driver.tu).map_err(WestError::Io)?;

    // Build from the topdir (west resolves the workspace from cwd).
    let board = match driver.bus {
        StagedBus::Platform { .. } => BOARD,
        StagedBus::Pcie { .. } => PCIE_BOARD,
    };
    let build_dir = stage.path().join("build");
    let output = std::process::Command::new("west")
        .current_dir(&topdir)
        .arg("build")
        .arg("-b")
        .arg(board)
        .arg("-d")
        .arg(&build_dir)
        .arg("-s")
        .arg(&app)
        .output()
        .map_err(|err| WestError::InvalidWorkspace(format!("`west` failed to spawn: {err}")))?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        // cmake/ninja diagnostics often land on stdout: carry both, stderr
        // first (verbatim, C03).
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        return Err(WestError::BuildFailed {
            stderr: format!("{stderr}\n--- stdout ---\n{stdout}"),
        });
    }
    Ok(WestOutcome::Built {
        zephyr_version: zephyr_version(&topdir),
        warnings: stderr,
    })
}

/// The workspace's Zephyr version string, best-effort, from
/// `<topdir>/zephyr/VERSION`.
fn zephyr_version(topdir: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(topdir.join("zephyr/VERSION")) else {
        return "unknown".to_string();
    };
    let field = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split('=').nth(1))
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    };
    let (maj, min, pl) = (
        field("VERSION_MAJOR"),
        field("VERSION_MINOR"),
        field("PATCHLEVEL"),
    );
    if maj.is_empty() {
        "unknown".to_string()
    } else {
        format!("{maj}.{min}.{pl}")
    }
}

/// A process-unique staging directory removed on drop.
struct StageDir {
    path: PathBuf,
}

impl StageDir {
    fn new() -> std::io::Result<StageDir> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let stem = format!("rseq-west-{}-{n}", std::process::id());
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

    fn driver<'a>() -> StagedDriver<'a> {
        StagedDriver {
            tu: "/* tu */",
            binding_yaml: "# binding",
            driver_name: "pl011",
            compatible: "regseq,pl011",
            bus: StagedBus::Platform {
                region: (0x0904_0000, 0x1000),
                interrupts: None,
            },
        }
    }

    #[test]
    fn unset_workspace_skips_with_a_visible_notice() {
        let outcome = west_build_with_workspace(None, &driver()).unwrap();
        let WestOutcome::Skipped { notice } = outcome else {
            panic!("unset workspace must skip");
        };
        assert!(
            notice.contains(WORKSPACE_VAR),
            "the notice names the variable"
        );
        assert!(
            notice.contains("v4.4.0"),
            "the notice names the expectation"
        );
    }

    #[test]
    fn missing_dir_is_a_hard_failure_not_a_skip() {
        let err = west_build_with_workspace(
            Some(PathBuf::from("/nonexistent/rseq-west-topdir")),
            &driver(),
        )
        .unwrap_err();
        assert!(matches!(err, WestError::InvalidWorkspace(ref s) if s.contains(WORKSPACE_VAR)));
    }

    #[test]
    fn dir_without_west_marker_is_a_hard_failure() {
        let dir = std::env::temp_dir().join(format!("rseq-west-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = west_build_with_workspace(Some(dir.clone()), &driver()).unwrap_err();
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(err, WestError::InvalidWorkspace(ref s) if s.contains(".west")));
    }

    #[test]
    fn version_reader_parses_the_zephyr_version_file() {
        let dir = std::env::temp_dir().join(format!("rseq-west-ver-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("zephyr")).unwrap();
        std::fs::write(
            dir.join("zephyr/VERSION"),
            "VERSION_MAJOR = 4\nVERSION_MINOR = 4\nPATCHLEVEL = 0\nVERSION_TWEAK = 0\nEXTRAVERSION =\n",
        )
        .unwrap();
        let v = zephyr_version(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(v, "4.4.0");
    }
}
