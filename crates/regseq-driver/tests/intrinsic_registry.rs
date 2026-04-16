//! Intrinsic registry extraction tests (regseq-driver-27).
//!
//! Compiles a dependent crate against the `regseq` facade and verifies that
//! [`IntrinsicRegistry::build`] discovers every `#[rseq::intrinsic("...")]`
//! annotation reachable from the facade.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/intrinsic_registry.rs"]
mod intrinsic_registry;

use intrinsic_registry::IntrinsicRegistry;

// ---------------------------------------------------------------------------
// Compiler harness
// ---------------------------------------------------------------------------

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn build_regseq_rlib() -> PathBuf {
    let root = workspace_root();

    let output = std::process::Command::new("cargo")
        .args(["build", "-p", "regseq", "--message-format=json"])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo build -p regseq");

    assert!(
        output.status.success(),
        "cargo build -p regseq failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("cargo output is valid UTF-8");
    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|v| v.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(target) = msg.get("target") else {
            continue;
        };
        if target.get("name").and_then(|v| v.as_str()) != Some("regseq") {
            continue;
        }
        if let Some(filenames) = msg.get("filenames").and_then(|v| v.as_array()) {
            for f in filenames {
                let path = f.as_str().unwrap_or("");
                if path.ends_with(".rlib") {
                    return PathBuf::from(path);
                }
            }
        }
    }

    panic!("could not find regseq rlib in cargo build output");
}

/// Resolved paths for compiling the PL011 full-spec fixture via a rustc driver.
#[derive(Debug, Clone)]
struct Pl011BuildArtifacts {
    regseq_rlib: PathBuf,
    regseq_macros_dylib: PathBuf,
    deps_dir: PathBuf,
}

/// Build `regseq` and `regseq-macros` from the main workspace and collect the
/// artifact paths needed to drive rustc against the PL011 source.
///
/// The PL011 fixture uses `#[rseq::*]` tool attributes which are only legal
/// under the `register_tool(rseq)` feature; plain `cargo build` on the fixture
/// therefore fails. We instead build only the dependencies via cargo and invoke
/// rustc directly on the PL011 source with `register_tool(rseq)` injected.
fn build_pl011_deps() -> Pl011BuildArtifacts {
    let root = workspace_root();

    let output = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "regseq",
            "-p",
            "regseq-macros",
            "--message-format=json",
        ])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo build for PL011 deps");

    assert!(
        output.status.success(),
        "cargo build for PL011 deps failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("cargo output is valid UTF-8");
    let mut regseq_rlib: Option<PathBuf> = None;
    let mut regseq_macros_dylib: Option<PathBuf> = None;

    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|v| v.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(target) = msg.get("target") else {
            continue;
        };
        let Some(name) = target.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(filenames) = msg.get("filenames").and_then(|v| v.as_array()) else {
            continue;
        };

        match name {
            "regseq" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".rlib") {
                        regseq_rlib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            "regseq-macros" | "regseq_macros" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".dylib") || p.ends_with(".so") || p.ends_with(".dll") {
                        regseq_macros_dylib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    let regseq_rlib = regseq_rlib.expect("regseq rlib not found in cargo build output");
    let regseq_macros_dylib =
        regseq_macros_dylib.expect("regseq-macros dylib not found in cargo build output");

    let deps_dir = regseq_rlib
        .parent()
        .expect("regseq rlib path must have a parent")
        .to_path_buf();

    Pl011BuildArtifacts {
        regseq_rlib,
        regseq_macros_dylib,
        deps_dir,
    }
}

fn pl011_artifacts() -> &'static Pl011BuildArtifacts {
    use std::sync::OnceLock;
    static A: OnceLock<Pl011BuildArtifacts> = OnceLock::new();
    A.get_or_init(build_pl011_deps)
}

/// Compile the PL011 full-spec fixture and build an [`IntrinsicRegistry`].
fn compile_pl011_and_build_registry(test_name: &str) -> RegistrySnapshot {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let artifacts = pl011_artifacts();
    let src_path = workspace_root().join("tests/fixtures/pl011-full-spec/src/lib.rs");
    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let sysroot = get_sysroot();
    let result: Arc<Mutex<Option<RegistrySnapshot>>> = Arc::new(Mutex::new(None));

    struct BuildCallbacks {
        result: Arc<Mutex<Option<RegistrySnapshot>>>,
    }

    impl rustc_driver::Callbacks for BuildCallbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let registry = IntrinsicRegistry::build(tcx);
            let mut entries = Vec::new();
            let mut lookup_results = Vec::new();
            for (id, name) in registry.iter() {
                entries.push((tcx.def_path_str(id), name.to_string()));
                lookup_results.push(registry.lookup(id).map(str::to_string));
            }
            *self.result.lock().unwrap() = Some(RegistrySnapshot {
                entries,
                len: registry.len(),
                is_empty: registry.is_empty(),
                lookup_results,
            });
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = BuildCallbacks {
        result: result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "--extern".to_string(),
        format!("regseq={}", artifacts.regseq_rlib.display()),
        "--extern".to_string(),
        format!("regseq_macros={}", artifacts.regseq_macros_dylib.display()),
        "-L".to_string(),
        format!("dependency={}", artifacts.deps_dir.display()),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let snapshot = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();
    snapshot
}

/// Registry result captured from `after_analysis`, reduced to `(def_path, name)`
/// pairs so it outlives the `TyCtxt`.
#[derive(Debug, Default)]
struct RegistrySnapshot {
    entries: Vec<(String, String)>,
    len: usize,
    is_empty: bool,
    /// `lookup(def_id)` applied to every collected DefId, preserving ordering
    /// with `entries`. Verifies the public `lookup` API while the snapshot
    /// still has access to live `DefId`s.
    lookup_results: Vec<Option<String>>,
}

fn compile_and_build_registry(
    source: &str,
    test_name: &str,
    regseq_rlib: &Path,
) -> RegistrySnapshot {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();
    let deps_dir = workspace_root().join("target/debug/deps");

    let result: Arc<Mutex<Option<RegistrySnapshot>>> = Arc::new(Mutex::new(None));

    struct BuildCallbacks {
        result: Arc<Mutex<Option<RegistrySnapshot>>>,
    }

    impl rustc_driver::Callbacks for BuildCallbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let registry = IntrinsicRegistry::build(tcx);
            let mut entries = Vec::new();
            let mut lookup_results = Vec::new();
            for (id, name) in registry.iter() {
                entries.push((tcx.def_path_str(id), name.to_string()));
                lookup_results.push(registry.lookup(id).map(str::to_string));
            }
            let snapshot = RegistrySnapshot {
                entries,
                len: registry.len(),
                is_empty: registry.is_empty(),
                lookup_results,
            };
            *self.result.lock().unwrap() = Some(snapshot);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = BuildCallbacks {
        result: result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "--extern".to_string(),
        format!("regseq={}", regseq_rlib.display()),
        "-L".to_string(),
        format!("dependency={}", deps_dir.display()),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let snapshot = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    snapshot
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn regseq_rlib() -> &'static PathBuf {
    use std::sync::OnceLock;
    static RLIB: OnceLock<PathBuf> = OnceLock::new();
    RLIB.get_or_init(build_regseq_rlib)
}

const DEPENDENT_SOURCE: &str = r#"
    extern crate regseq;

    pub fn anchor() {}
"#;

/// All canonical intrinsic names listed in the execution plan (Phase 3).
const EXPECTED_INTRINSICS: &[&str] = &[
    "mmio_read",
    "mmio_read_field",
    "mmio_write",
    "mmio_write_field",
    "clock_enable",
    "clock_disable",
    "clock_get_rate",
    "reset_assert",
    "reset_deassert",
    "reset_toggle",
    "pinctrl_apply",
    "busy_wait",
    "sleep",
    "log_error",
    "log_warn",
    "log_info",
    "log_debug",
    "poll_until",
];

#[test]
fn registry_discovers_all_canonical_intrinsics() {
    let rlib = regseq_rlib();
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "registry_all_canonical", rlib);

    assert!(
        !snap.is_empty,
        "registry must not be empty; cross-crate attr visibility may have regressed"
    );
    assert!(
        snap.len >= 18,
        "expected at least 18 intrinsics, got {} ({:?})",
        snap.len,
        snap.entries,
    );

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    for expected in EXPECTED_INTRINSICS {
        assert!(
            names.contains(expected),
            "missing canonical intrinsic {expected}; found: {names:?}",
        );
    }
}

#[test]
fn registry_contains_specific_sentinels() {
    let rlib = regseq_rlib();
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "registry_sentinels", rlib);

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    assert!(names.contains("mmio_read"), "mmio_read must be registered");
    assert!(
        names.contains("clock_enable"),
        "clock_enable must be registered"
    );
    assert!(
        names.contains("poll_until"),
        "poll_until must be registered even though IR construction is deferred"
    );
}

#[test]
fn registry_lookup_returns_canonical_name_for_known_def_id() {
    let rlib = regseq_rlib();
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "registry_lookup", rlib);

    // `lookup_results[i]` is `registry.lookup(def_id_i)` captured inside
    // `after_analysis`. It must equal the canonical name stored at `entries[i]`.
    assert_eq!(
        snap.entries.len(),
        snap.lookup_results.len(),
        "entries and lookup_results must align",
    );
    for ((path, name), looked_up) in snap.entries.iter().zip(snap.lookup_results.iter()) {
        assert_eq!(
            looked_up.as_deref(),
            Some(name.as_str()),
            "lookup on {path} must return {name}",
        );
    }

    // Spot-check path-to-name mappings for sentinel intrinsics.
    let mmio_read_entry = snap
        .entries
        .iter()
        .find(|(path, _)| path.ends_with("Mmio::<Block>::read"))
        .expect("Mmio::read must be in the registry");
    assert_eq!(mmio_read_entry.1, "mmio_read");

    let clock_enable_entry = snap
        .entries
        .iter()
        .find(|(path, _)| path.ends_with("Clock::enable"))
        .expect("Clock::enable must be in the registry");
    assert_eq!(clock_enable_entry.1, "clock_enable");
}

#[test]
fn registry_against_pl011_full_spec_compilation() {
    let snap = compile_pl011_and_build_registry("registry_pl011_full_spec");

    assert!(
        !snap.is_empty,
        "registry must not be empty under PL011 full-spec compilation",
    );
    assert!(
        snap.len >= 18,
        "expected at least 18 intrinsics under PL011 compilation, got {} ({:?})",
        snap.len,
        snap.entries,
    );

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    for expected in EXPECTED_INTRINSICS {
        assert!(
            names.contains(expected),
            "PL011 compilation missing canonical intrinsic {expected}; found: {names:?}",
        );
    }

    let mmio_read = snap
        .entries
        .iter()
        .find(|(path, _)| path.ends_with("Mmio::<Block>::read"))
        .expect("Mmio::read must be discoverable under PL011 compilation");
    assert_eq!(mmio_read.1, "mmio_read");
}

#[test]
fn registry_names_are_unique_per_def_id() {
    let rlib = regseq_rlib();
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "registry_unique", rlib);

    let mut seen_paths: HashSet<&str> = HashSet::new();
    for (path, _) in &snap.entries {
        assert!(
            seen_paths.insert(path.as_str()),
            "duplicate DefId in registry: {path}",
        );
    }
}
