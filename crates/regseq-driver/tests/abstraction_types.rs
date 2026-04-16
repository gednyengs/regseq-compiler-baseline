//! Abstraction type registry extraction tests (regseq-driver-29).
//!
//! Compiles a dependent crate against the `regseq` facade and verifies that
//! [`AbstractionTypeRegistry::build`] discovers every
//! `#[rseq::abstraction("...")]` annotation reachable from the facade.

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

#[path = "../src/extract/abstraction_types.rs"]
mod abstraction_types;

use abstraction_types::AbstractionTypeRegistry;

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

#[derive(Debug, Clone)]
struct BuildArtifacts {
    regseq_rlib: PathBuf,
    regseq_macros_dylib: PathBuf,
    deps_dir: PathBuf,
}

fn build_deps() -> BuildArtifacts {
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
        .expect("failed to run cargo build for regseq deps");

    assert!(
        output.status.success(),
        "cargo build for regseq deps failed:\n{}",
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

    BuildArtifacts {
        regseq_rlib,
        regseq_macros_dylib,
        deps_dir,
    }
}

fn artifacts() -> &'static BuildArtifacts {
    use std::sync::OnceLock;
    static A: OnceLock<BuildArtifacts> = OnceLock::new();
    A.get_or_init(build_deps)
}

#[derive(Debug, Default)]
struct RegistrySnapshot {
    entries: Vec<(String, String)>,
    len: usize,
    is_empty: bool,
    lookup_results: Vec<Option<String>>,
}

fn compile_and_build_registry(source: &str, test_name: &str) -> RegistrySnapshot {
    let artifacts = artifacts();
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
            let registry = AbstractionTypeRegistry::build(tcx);
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
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "--extern".to_string(),
        format!("regseq={}", artifacts.regseq_rlib.display()),
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

fn compile_pl011_and_build_registry(test_name: &str) -> RegistrySnapshot {
    let artifacts = artifacts();
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

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
            let registry = AbstractionTypeRegistry::build(tcx);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

const DEPENDENT_SOURCE: &str = r#"
    extern crate regseq;

    pub fn anchor() {}
"#;

/// All canonical abstraction identifiers from spec.md Section 10.1.
const EXPECTED_ABSTRACTIONS: &[&str] = &[
    "clock",
    "reset",
    "pinctrl",
    "interrupt",
    "mmio",
    "mutex",
    "spinlock",
    "irq-spinlock",
    "completion",
];

#[test]
fn registry_discovers_all_abstraction_types() {
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "abstraction_all_types");

    assert!(
        !snap.is_empty,
        "registry must not be empty; cross-crate attr visibility may have regressed",
    );
    assert_eq!(
        snap.len, 9,
        "expected exactly 9 abstraction types, got {} ({:?})",
        snap.len, snap.entries,
    );

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    for expected in EXPECTED_ABSTRACTIONS {
        assert!(
            names.contains(expected),
            "missing abstraction type {expected}; found: {names:?}",
        );
    }
}

#[test]
fn registry_contains_specific_sentinels() {
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "abstraction_sentinels");

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    assert!(names.contains("clock"), "clock must be registered");
    assert!(names.contains("mmio"), "mmio must be registered");
    assert!(names.contains("reset"), "reset must be registered");
}

#[test]
fn registry_lookup_returns_canonical_identifier_for_known_def_id() {
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "abstraction_lookup");

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

    let clock = snap
        .entries
        .iter()
        .find(|(path, _)| path.ends_with("Clock"))
        .expect("Clock must be in the registry");
    assert_eq!(clock.1, "clock");

    let mmio = snap
        .entries
        .iter()
        .find(|(path, _)| path.ends_with("Mmio"))
        .expect("Mmio must be in the registry");
    assert_eq!(mmio.1, "mmio");
}

#[test]
fn registry_against_pl011_full_spec_compilation() {
    let snap = compile_pl011_and_build_registry("abstraction_pl011_full_spec");

    assert!(
        !snap.is_empty,
        "registry must not be empty under PL011 full-spec compilation",
    );
    assert_eq!(
        snap.len, 9,
        "expected exactly 9 abstraction types under PL011 compilation, got {} ({:?})",
        snap.len, snap.entries,
    );

    let names: HashSet<&str> = snap.entries.iter().map(|(_, n)| n.as_str()).collect();
    for expected in EXPECTED_ABSTRACTIONS {
        assert!(
            names.contains(expected),
            "PL011 compilation missing abstraction type {expected}; found: {names:?}",
        );
    }
}

#[test]
fn registry_entries_are_unique_per_def_id() {
    let snap = compile_and_build_registry(DEPENDENT_SOURCE, "abstraction_unique");

    let mut seen_paths: HashSet<&str> = HashSet::new();
    for (path, _) in &snap.entries {
        assert!(
            seen_paths.insert(path.as_str()),
            "duplicate DefId in registry: {path}",
        );
    }
}
