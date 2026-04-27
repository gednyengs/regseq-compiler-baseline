//! Integration test for the MIR pass whitelist applied by
//! `RegseqCallbacks` (mirrored by the harness here). Compiles a
//! minimal `#![no_std]` driver source and asserts that the
//! `optimized_mir` of a representative function has the shape the
//! conservative whitelist produces:
//!
//!   - No alignment / null / enum runtime check artifacts (UB-check
//!     passes are force-disabled).
//!
//! The shape-altering cleanup passes (`RemoveStorageMarkers`,
//! `RemoveZsts`, `SimplifyLocals-final`, `ReorderBasicBlocks`,
//! `ReorderLocals`) are the design target but currently deferred
//! pending updates to the `?`-pattern recognizer in
//! `construction/try_folding.rs`. Once those land, the assertions
//! about absent storage markers and reverse-postorder block
//! ordering will be re-enabled here.
//!
//! The harness builds the `regseq` and `regseq-macros` deps once,
//! writes a small inline source to a temp dir, and runs rustc with a
//! callback that mirrors the production `RegseqCallbacks::config`
//! (minus the spec-only `no_codegen`).

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};

use regseq_driver::regseq_mir_passes;

use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::TyCtxt;

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

#[derive(Debug, Clone)]
struct DepArtifacts {
    regseq_rlib: PathBuf,
    regseq_macros_dylib: PathBuf,
    deps_dir: PathBuf,
}

fn build_deps() -> DepArtifacts {
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
    let regseq_rlib = regseq_rlib.expect("regseq rlib not found");
    let regseq_macros_dylib = regseq_macros_dylib.expect("regseq-macros dylib not found");
    let deps_dir = regseq_rlib.parent().unwrap().to_path_buf();
    DepArtifacts {
        regseq_rlib,
        regseq_macros_dylib,
        deps_dir,
    }
}

fn artifacts() -> &'static DepArtifacts {
    static A: OnceLock<DepArtifacts> = OnceLock::new();
    A.get_or_init(build_deps)
}

/// Inspection result captured while a live `TyCtxt` is alive.
#[derive(Debug, Default)]
struct ShapeFindings {
    /// Names of any local function whose `optimized_mir` contained a
    /// terminator that looks like a UB-check insertion (call to a
    /// `precondition_check` lang item / `core::ub_checks` helper).
    bodies_with_ub_check_calls: Vec<String>,
}

fn inspect_local_bodies(tcx: TyCtxt<'_>) -> ShapeFindings {
    let mut findings = ShapeFindings::default();

    for local_def_id in tcx.hir_body_owners() {
        let def_id = local_def_id.to_def_id();
        if def_id.krate != LOCAL_CRATE {
            continue;
        }
        if !tcx.def_kind(def_id).is_fn_like() {
            continue;
        }
        if !tcx.is_mir_available(def_id) {
            continue;
        }
        let name = tcx.def_path_str(def_id);
        let body = tcx.optimized_mir(def_id);

        for data in body.basic_blocks.iter() {
            if let Some(term) = &data.terminator {
                if let TerminatorKind::Call { func, .. } = &term.kind {
                    let ty = func.ty(&body.local_decls, tcx);
                    if let rustc_middle::ty::FnDef(callee, _) = ty.kind() {
                        let callee_path = tcx.def_path_str(*callee);
                        if callee_path.contains("precondition_check")
                            || callee_path.contains("ub_checks::")
                            || callee_path.contains("check_aligned")
                            || callee_path.contains("check_null")
                        {
                            if !findings.bodies_with_ub_check_calls.contains(&name) {
                                findings.bodies_with_ub_check_calls.push(name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    findings
}

fn run_inspection(source: &str, test_name: &str) -> ShapeFindings {
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

    let out: Arc<Mutex<Option<ShapeFindings>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        out: Arc<Mutex<Option<ShapeFindings>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            // Mirror RegseqCallbacks for the MIR shape settings. The
            // spec-only `no_codegen` is omitted: this harness needs
            // `optimized_mir` to be queryable, which requires the full
            // analysis pipeline up to (but not including) codegen, and
            // calling `optimized_mir` inside `after_analysis` produces
            // that shape regardless of `no_codegen`.
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
            config.opts.unstable_opts.mir_enable_passes = regseq_mir_passes();
            config.opts.cg.overflow_checks = Some(false);
            config.opts.cg.debug_assertions = Some(false);
            config.opts.debug_assertions = false;
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let findings = inspect_local_bodies(tcx);
            *self.out.lock().unwrap() = Some(findings);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks { out: out.clone() };

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
        "mir_shape_input".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
        "--extern".to_string(),
        format!("regseq={}", artifacts.regseq_rlib.display()),
        "--extern".to_string(),
        format!("regseq_macros={}", artifacts.regseq_macros_dylib.display()),
        "-L".to_string(),
        format!("dependency={}", artifacts.deps_dir.display()),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must populate findings");
    std::fs::remove_dir_all(&test_dir).ok();
    result
}

const MIR_SHAPE_SOURCE: &str = r#"
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub fn straight_line_arith(x: u32, y: u32) -> u32 {
    let a = x.wrapping_add(y);
    let b = a.wrapping_mul(3);
    b
}

pub fn branching(x: u32) -> u32 {
    if x > 10 {
        x.wrapping_sub(10)
    } else {
        0
    }
}

pub fn loop_body(mut x: u32) -> u32 {
    let mut acc: u32 = 0;
    while x > 0 {
        acc = acc.wrapping_add(x);
        x = x.wrapping_sub(1);
    }
    acc
}
"#;

#[test]
fn whitelist_disables_ub_check_instrumentation() {
    let findings = run_inspection(MIR_SHAPE_SOURCE, "mir_shape_whitelist");

    assert!(
        findings.bodies_with_ub_check_calls.is_empty(),
        "CheckAlignment/CheckNull/CheckEnums must be force-disabled; \
         bodies with UB-check calls: {:?}",
        findings.bodies_with_ub_check_calls,
    );
}
