//! Declaration-level source location extraction tests (regseq-driver-31).
//!
//! Drives the extractor against both a synthetic dependent crate and the
//! PL011 full-spec fixture to verify that every item with a [`DefId`]
//! lands in the [`RawSourceLocationSurface`] with a non-dummy span.
//!
//! GATED OUT: this harness builds the standalone `regseq` crate as a
//! dependency, but that crate was collapsed into a `crate::regseq`
//! submodule of each spec fixture. The functional coverage continues
//! through end-to-end spec compilation. Restoring a
//! synthetic-dependent-crate variant would require vendoring the
//! collapsed facade source into the test harness; deferred until the
//! PoC stage demands it.

#![cfg(false)]

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/source_locations.rs"]
mod source_locations;

use source_locations::{extract_source_locations, RawSourceLocation, RawSourceLocationSurface};

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

fn compile_source_and_extract(source: &str, test_name: &str) -> RawSourceLocationSurface {
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
    let result: Arc<Mutex<Option<RawSourceLocationSurface>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<RawSourceLocationSurface>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
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
            *self.result.lock().unwrap() = Some(extract_source_locations(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
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
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();
    extracted
}

fn compile_pl011_and_extract(test_name: &str) -> RawSourceLocationSurface {
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
    let result: Arc<Mutex<Option<RawSourceLocationSurface>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<RawSourceLocationSurface>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            // The migrated pl011-full-spec fixture self-declares
            // `feature(register_tool)` and `register_tool(rseq)`.
            config.opts.unstable_opts.no_codegen = true;
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
            *self.result.lock().unwrap() = Some(extract_source_locations(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
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

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();
    extracted
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_valid_location(loc: &RawSourceLocation, context: &str) {
    assert!(
        !loc.file.is_empty(),
        "{context}: file path must not be empty (loc = {loc:?})",
    );
    assert!(
        loc.start_line > 0,
        "{context}: start_line must be 1-based and non-zero (loc = {loc:?})",
    );
    assert!(
        loc.end_line > 0,
        "{context}: end_line must be 1-based and non-zero (loc = {loc:?})",
    );
    assert!(
        loc.end_line > loc.start_line
            || (loc.end_line == loc.start_line && loc.end_col >= loc.start_col),
        "{context}: end must come after or equal start (loc = {loc:?})",
    );
}

fn find_by_suffix<'a>(
    surface: &'a RawSourceLocationSurface,
    suffix: &str,
) -> Option<(&'a str, &'a RawSourceLocation)> {
    surface
        .locations
        .iter()
        .find(|(k, _)| k.ends_with(suffix))
        .map(|(k, v)| (k.as_str(), v))
}

// ---------------------------------------------------------------------------
// Synthetic tests
// ---------------------------------------------------------------------------

#[test]
fn extracts_all_item_kinds_from_synthetic_source() {
    let source = r#"
        pub mod nested {
            pub struct Inner;
            pub const NESTED_CONST: u32 = 7;
        }

        pub struct Top;

        pub enum Mode {
            A,
            B,
        }

        pub const TOP_CONST: u32 = 42;
        pub static TOP_STATIC: u32 = 43;

        pub fn free_function() -> u32 { 0 }

        impl Top {
            pub const ASSOC_CONST: u32 = 1;
            pub fn method(&self) -> u32 { 0 }
        }
    "#;

    let surface = compile_source_and_extract(source, "source_locations_synthetic");

    assert!(!surface.is_empty(), "surface must not be empty");

    for (path, loc) in &surface.locations {
        assert_valid_location(loc, path);
    }

    // Every item kind from the fixture must be represented.
    for suffix in [
        "Top",
        "Mode",
        "TOP_CONST",
        "TOP_STATIC",
        "free_function",
        "nested",
        "nested::Inner",
        "nested::NESTED_CONST",
        "Top::method",
        "Top::ASSOC_CONST",
    ] {
        assert!(
            find_by_suffix(&surface, suffix).is_some(),
            "expected a source location for item ending in {suffix:?}; surface keys: {:?}",
            surface.locations.keys().collect::<Vec<_>>(),
        );
    }
}

#[test]
fn records_impl_block_source_location() {
    // The impl block itself carries a DefId that is distinct from its
    // associated items. Ensure we record a span for the impl block too,
    // separate from the struct it targets.
    let source = r#"
        pub struct Dev;

        impl Dev {
            pub fn go(&self) {}
        }
    "#;

    let surface = compile_source_and_extract(source, "source_locations_impl");

    let impl_key = surface
        .locations
        .keys()
        .find(|k| k.contains("{impl#"))
        .unwrap_or_else(|| {
            panic!(
                "expected an impl-block entry in the surface; keys were: {:?}",
                surface.locations.keys().collect::<Vec<_>>(),
            )
        });
    let impl_loc = surface
        .get(impl_key)
        .expect("get should return the recorded impl location");
    assert_valid_location(impl_loc, impl_key);

    let (_, struct_loc) = find_by_suffix(&surface, "Dev").unwrap();
    assert_ne!(
        impl_loc, struct_loc,
        "impl block and struct must have distinct source locations",
    );
}

// ---------------------------------------------------------------------------
// PL011 full-spec tests
// ---------------------------------------------------------------------------

#[test]
fn extracts_at_least_ten_locations_from_pl011_full_spec() {
    let surface = compile_pl011_and_extract("source_locations_pl011_count");

    assert!(
        surface.len() >= 10,
        "expected at least 10 source locations from PL011, got {} (keys: {:?})",
        surface.len(),
        surface.locations.keys().collect::<Vec<_>>(),
    );
}

#[test]
fn pl011_driver_struct_has_source_location() {
    let surface = compile_pl011_and_extract("source_locations_pl011_driver");

    let (path, loc) = find_by_suffix(&surface, "Pl011FullSpec").unwrap_or_else(|| {
        panic!(
            "driver struct Pl011FullSpec must appear in the surface; keys: {:?}",
            surface.locations.keys().collect::<Vec<_>>(),
        )
    });

    assert_valid_location(loc, path);
    assert!(
        loc.file.ends_with("driver.rs"),
        "driver struct should resolve to driver.rs, got {loc:?}",
    );
}

#[test]
fn pl011_locations_have_non_zero_line_numbers() {
    let surface = compile_pl011_and_extract("source_locations_pl011_line_numbers");

    assert!(!surface.is_empty(), "PL011 surface must not be empty");
    for (path, loc) in &surface.locations {
        assert!(
            loc.start_line > 0 && loc.end_line > 0,
            "zero line number for {path}: {loc:?}",
        );
        assert!(!loc.file.is_empty(), "empty file for {path}: {loc:?}");
    }
}
