//! Pass 2 Rule 6 logging-metadata extraction tests (regseq-model-50).
//!
//! These tests drive `build_body` against fixture crates that depend
//! on the real `regseq` facade so the recognized
//! `regseq::intrinsics::diagnostic_logging::LogArg` ADT and
//! `LogArg::from` carrier calls show up in the MIR exactly as the
//! production specs produce them. Each test compiles the fixture
//! once, builds Pass 1, then translates the targeted method bodies
//! and asserts that:
//!
//! - the IntrinsicCall carries `IntrinsicMetadata::Log { template,
//!   placeholders }`,
//! - `template` matches the source string verbatim,
//! - `placeholders.len() == args.len()`,
//! - each `placeholders[i].format` matches the source format
//!   specifier,
//! - and `args[i]` is no longer a `LogArg` carrier (the inner value
//!   shows through directly).

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_driver::construction::{
    build_body, build_pass1_output, ConstantIndex, ConstructionError, DerefCollapseLookup,
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, LogArgUnwrap, OrderedMarkers,
    PinStateRegistry, TryMarkers, TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::SymbolInterner;
use regseq_model::ir::{
    FormatKind, IntrinsicMetadata, Operand, RegseqIntrinsic, RegseqType, TerminatorKind,
};

// ---------------------------------------------------------------------------
// Compiler harness
// ---------------------------------------------------------------------------

type Callback<R> = Box<dyn FnOnce(TyCtxt<'_>) -> R + Send>;
type CallbackCell<R> = Arc<Mutex<Option<Callback<R>>>>;

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
    static A: OnceLock<BuildArtifacts> = OnceLock::new();
    A.get_or_init(build_deps)
}

/// Compile the fixture at `src_path` (under `tests/fixtures/...`) and
/// invoke `f` with a live `TyCtxt` so the callback can drive Pass 1
/// + Pass 2 directly. The result is captured outside the rustc lock.
fn with_fixture<R, F>(src_path: PathBuf, crate_name: &str, test_name: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>) -> R + Send + 'static,
{
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

    let sysroot = get_sysroot();

    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback_cell: CallbackCell<R> = Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: CallbackCell<R>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
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
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let f = self.callback.lock().unwrap().take().expect("callback");
            let result = f(tcx);
            *self.out.lock().unwrap() = Some(result);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks::<R> {
        out: out.clone(),
        callback: callback_cell,
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
        crate_name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Zmir-opt-level=0".to_string(),
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
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();
    result
}

// ---------------------------------------------------------------------------
// HIR helpers (mirror construction_intrinsic_recognition.rs)
// ---------------------------------------------------------------------------

fn find_method_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let impl_ty = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{impl_ty}") != self_ty {
                continue;
            }
            for ii in impl_block.items {
                let impl_item = tcx.hir_impl_item(*ii);
                if let rustc_hir::ImplItemKind::Fn(_, _) = &impl_item.kind {
                    if impl_item.ident.name.as_str() == method {
                        return impl_item.owner_id.def_id.to_def_id();
                    }
                }
            }
        }
    }
    panic!("method `{self_ty}::{method}` not found in fixture");
}

// ---------------------------------------------------------------------------
// Snapshot used by every smoke method assertion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LogCallSnapshot {
    intrinsic: RegseqIntrinsic,
    template: String,
    placeholders: Vec<FormatKind>,
    arg_count: usize,
    /// Per-argument resolved [`RegseqType`] (looked up from the
    /// translated body's `local_decls` for Place-backed operands; the
    /// Operand itself for `Literal`/`Constant` cases). Carries the
    /// concrete type each post-strip argument resolves to so the test
    /// matrix can assert that the `LogArg` carrier is gone (and the
    /// inner primitive shows through directly).
    arg_types: Vec<ArgKind>,
}

/// Compact rendering of an [`Operand`]'s resolved type for the
/// LogArg-stripping assertion.
#[derive(Debug, Clone, PartialEq)]
enum ArgKind {
    /// A Place operand whose root local has the recorded type.
    Place(RegseqType),
    /// An inline literal value (`u32`, `bool`, ...).
    Literal,
    /// A named constant reference -- typed at the constant
    /// declaration, not at the use site.
    Constant,
}

fn snapshot_log_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    method_def_id: DefId,
) -> Result<LogCallSnapshot, ConstructionError> {
    let intrinsic_registry = IntrinsicRegistry::build(tcx);
    let framework = FrameworkTypeRegistry::build(tcx);
    let abstraction = AbstractionTypeRegistry::build(tcx);
    let pin_states = PinStateRegistry::build(tcx);
    let registries = ExtractionRegistries {
        intrinsic: &intrinsic_registry,
        framework_types: &framework,
        abstraction_types: &abstraction,
        pin_states: &pin_states,
    };
    let pass1 =
        build_pass1_output(tcx, &registries).expect("Pass 1 must succeed on the smoke fixture");
    let function_id = *pass1
        .cross_refs
        .functions
        .get(&method_def_id)
        .expect("method must be registered in Pass 1");

    let intrinsic_lookup = IntrinsicLookup::build(tcx, &intrinsic_registry)
        .expect("IntrinsicLookup::build must accept the canonical intrinsic catalog");
    let deref_lookup =
        DerefCollapseLookup::build(tcx, &abstraction).expect("Deref lang item must be resolvable");
    let ordered_markers =
        OrderedMarkers::resolve(tcx).expect("OrderedMarkers must resolve when regseq is loaded");
    let try_markers = TryMarkers::resolve(tcx);
    let log_arg = LogArgUnwrap::resolve(tcx);
    assert!(
        log_arg.log_arg_adt().is_some(),
        "LogArgUnwrap::resolve must find the facade-side LogArg ADT",
    );

    let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
    let constants = ConstantIndex::new(&pass1.cross_refs.constants);
    let functions = FunctionIndex::new(&pass1.cross_refs.functions);
    let mut interner = SymbolInterner::new();
    let body = build_body(
        tcx,
        method_def_id,
        function_id,
        &type_def_index,
        &constants,
        &functions,
        &intrinsic_lookup,
        &deref_lookup,
        &ordered_markers,
        &try_markers,
        &log_arg,
        &mut interner,
    )?;

    let mut found: Option<LogCallSnapshot> = None;
    for bb in &body.basic_blocks {
        let TerminatorKind::IntrinsicCall {
            intrinsic,
            args,
            metadata,
            ..
        } = &bb.terminator.kind
        else {
            continue;
        };
        let Some(IntrinsicMetadata::Log {
            template,
            placeholders,
        }) = metadata
        else {
            continue;
        };
        let arg_types: Vec<ArgKind> = args
            .iter()
            .map(|op| classify_arg(op, &body.local_decls))
            .collect();
        if found.is_some() {
            panic!("more than one logging IntrinsicCall in method body");
        }
        found = Some(LogCallSnapshot {
            intrinsic: *intrinsic,
            template: template.clone(),
            placeholders: placeholders.iter().map(|p| p.format).collect(),
            arg_count: args.len(),
            arg_types,
        });
    }
    Ok(found.expect("method must contain a logging IntrinsicCall with metadata"))
}

/// Resolve an [`Operand`]'s type back to its [`ArgKind`] using the
/// surrounding body's `local_decls`. `Place` operands look up the
/// root local's [`RegseqType`]; everything else is captured by
/// variant. The smoke test asserts that no resulting [`ArgKind`]
/// references the cross-crate `LogArg` ADT -- since the
/// [`LogFoldAnalysis`] in `super::log_metadata` keeps `LogArg`
/// out of the [`TypeDefIndex`], any arg whose type came back as a
/// concrete `Defined(...)`/primitive variant is conclusive
/// evidence that the carrier was stripped.
fn classify_arg(op: &Operand, locals: &[regseq_model::ir::LocalDecl]) -> ArgKind {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            let idx = place.local.0 as usize;
            let ty = locals[idx].ty.clone();
            ArgKind::Place(ty)
        }
        Operand::Literal(_) => ArgKind::Literal,
        Operand::Constant(_) => ArgKind::Constant,
    }
}

/// Whether `ty` is a primitive type the logging facade's `LogArg::from`
/// impls accept (`u8..u64`, `i8..i64`, `usize`, `isize`, `bool`,
/// `&str`). Used by the smoke matrix to assert that every stripped
/// argument operand resolves to one of those inner shapes -- not to
/// the cross-crate `LogArg` ADT, and not to any other surprise type.
fn arg_kind_is_primitive(kind: &ArgKind) -> bool {
    match kind {
        ArgKind::Literal | ArgKind::Constant => true,
        ArgKind::Place(ty) => is_primitive_arg_ty(ty),
    }
}

fn is_primitive_arg_ty(ty: &RegseqType) -> bool {
    match ty {
        RegseqType::Scalar(_) | RegseqType::Bool => true,
        // `&str` lowers to `Slice(Scalar(U8))` once the reference is
        // unwrapped (see `map_ty`). Accept any reference to a
        // primitive or `Slice` of primitives so the &str arg the
        // facade accepts surfaces here as a primitive shape rather
        // than a Defined(TypeDefId).
        RegseqType::Ref(inner, _) => is_primitive_arg_ty(inner),
        RegseqType::Slice(inner) => is_primitive_arg_ty(inner),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Negative test specimens
// ---------------------------------------------------------------------------

const UNSUPPORTED_FORMAT_FIXTURE: &str = include_str!("./log_metadata_unsupported_fmt.rs.in");
const ARITY_MISMATCH_FIXTURE: &str = include_str!("./log_metadata_arity_mismatch.rs.in");

fn write_fixture(test_name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join("log-metadata-fixtures")
        .join(test_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();
    src_path
}

// ---------------------------------------------------------------------------
// Tests: log-intrinsic-smoke fixture exercises every FormatKind variant
// ---------------------------------------------------------------------------
//
// `FormatKind::Pointer` (`{:p}`) is verified by the parser unit test
// `parse_template_all_format_kinds` only -- the Regseq IR does not
// model raw pointer types yet (the existing
// `map_ty_errors_on_raw_pointer` test in `tests/mir_mapping.rs`
// pins that constraint), so an end-to-end `LogArg::from(*const ())`
// translation cannot succeed without an out-of-scope IR extension.
// All other variants are covered end-to-end below.

struct SmokeCase {
    method: &'static str,
    intrinsic: RegseqIntrinsic,
    template: &'static str,
    placeholders: &'static [FormatKind],
    arg_count: usize,
}

const SMOKE_METHODS: &[SmokeCase] = &[
    SmokeCase {
        method: "log_display",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "display: {}",
        placeholders: &[FormatKind::Display],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_lower_hex",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "lower hex: {:x}",
        placeholders: &[FormatKind::LowerHex],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_upper_hex",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "upper hex: {:X}",
        placeholders: &[FormatKind::UpperHex],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_lower_hex_prefixed",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "lower prefix: {:#x}",
        placeholders: &[FormatKind::LowerHexPrefixed],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_upper_hex_prefixed",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "upper prefix: {:#X}",
        placeholders: &[FormatKind::UpperHexPrefixed],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_padded_hex",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "padded: {:08x}",
        placeholders: &[FormatKind::PaddedHex {
            prefix: false,
            uppercase: false,
            width: 8,
        }],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_padded_prefix_hex",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "padded prefix: {:#010x}",
        placeholders: &[FormatKind::PaddedHex {
            prefix: true,
            uppercase: false,
            width: 10,
        }],
        arg_count: 1,
    },
    SmokeCase {
        method: "log_two_args",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "two: {} {:#x}",
        placeholders: &[FormatKind::Display, FormatKind::LowerHexPrefixed],
        arg_count: 2,
    },
    SmokeCase {
        method: "log_no_args",
        intrinsic: RegseqIntrinsic::LogInfo,
        template: "no args at all",
        placeholders: &[],
        arg_count: 0,
    },
];

#[test]
fn smoke_fixture_covers_every_format_kind() {
    let src = workspace_root().join("tests/fixtures/log-intrinsic-smoke/src/lib.rs");
    let snapshots: HashMap<String, LogCallSnapshot> =
        with_fixture(src, "log_intrinsic_smoke", "log_metadata_smoke", |tcx| {
            let mut out: HashMap<String, LogCallSnapshot> = HashMap::new();
            for case in SMOKE_METHODS {
                let did = find_method_def_id(tcx, "LogSmoke", case.method);
                let snap = snapshot_log_call(tcx, did).unwrap_or_else(|err| {
                    panic!("method `{}` translation failed: {err:?}", case.method)
                });
                out.insert(case.method.to_string(), snap);
            }
            out
        });

    for case in SMOKE_METHODS {
        let snap = snapshots
            .get(case.method)
            .unwrap_or_else(|| panic!("missing snapshot for `{}`", case.method));
        assert_eq!(
            snap.intrinsic, case.intrinsic,
            "method `{}` variant",
            case.method,
        );
        assert_eq!(
            snap.template, case.template,
            "method `{}` template",
            case.method,
        );
        assert_eq!(
            snap.placeholders.as_slice(),
            case.placeholders,
            "method `{}` placeholders",
            case.method,
        );
        assert_eq!(
            snap.placeholders.len(),
            snap.arg_count,
            "method `{}` arity matches placeholders",
            case.method,
        );
        assert_eq!(
            snap.arg_count, case.arg_count,
            "method `{}` arg count",
            case.method,
        );
        for (i, kind) in snap.arg_types.iter().enumerate() {
            assert!(
                arg_kind_is_primitive(kind),
                "method `{}` args[{i}] still carries a non-primitive type ({kind:?}); LogArg \
                 carrier stripping must produce inner Scalar/Bool/Ref/Literal operands",
                case.method,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: PL011 fixture carries the new info! call
// ---------------------------------------------------------------------------
//
// End-to-end translation of the real PL011 fixture is currently
// blocked by two unrelated Pass 1 limitations: (1) the per-module
// walk processes `mod driver;` before `pub mod registers;`, leaving
// the cross-module `register_block_map` empty when `Pl011FullSpec`'s
// `Mmio<pl011::Regs>` is built; and (2) Pass 1's
// `KNOWN_RSEQ_ATTRS` allowlist does not yet include
// `#[rseq::device_class(...)]`. Neither gap is a Rule 6 concern and
// fixing them would expand the scope of this task. Instead the
// PL011 assertion is split into two complementary checks: a
// fidelity test that the new `info!` call is present in the fixture
// source, and an end-to-end test against an inline specimen that
// mirrors `program_polling_port`'s logging shape exactly. The full
// PL011 end-to-end pipeline is the responsibility of task 54.

#[test]
fn pl011_fixture_contains_info_log_call() {
    let fixture = std::fs::read_to_string(
        workspace_root().join("tests/fixtures/pl011-full-spec/src/driver.rs"),
    )
    .expect("PL011 fixture source must be readable");
    assert!(
        fixture.contains("regseq::log_info!(\"pl011 baud rate: {}\", baud_rate);"),
        "PL011 fixture must contain the new info! call inside program_polling_port",
    );
    assert!(
        fixture.contains("fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)"),
        "the info! call must live inside `program_polling_port`",
    );
}

const PL011_LIKE_SPECIMEN: &str = r#"
#![no_std]

#[rseq::driver(name = "pl011-like")]
pub struct Pl011Like {}

impl Pl011Like {
    pub fn program_polling_port(uart_clk_hz: u32, baud_rate: u32) {
        let _ = uart_clk_hz;
        regseq::log_info!("pl011 baud rate: {}", baud_rate);
    }
}
"#;

#[test]
fn pl011_like_specimen_logs_baud_rate() {
    let src = write_fixture("pl011_like_specimen", PL011_LIKE_SPECIMEN);
    let snap = with_fixture(src, "pl011_like", "log_metadata_pl011_like", |tcx| {
        let did = find_method_def_id(tcx, "Pl011Like", "program_polling_port");
        snapshot_log_call(tcx, did)
            .expect("program_polling_port translation must succeed on PL011-like specimen")
    });

    assert_eq!(snap.intrinsic, RegseqIntrinsic::LogInfo);
    assert_eq!(snap.template, "pl011 baud rate: {}");
    assert_eq!(snap.placeholders, vec![FormatKind::Display]);
    assert_eq!(snap.arg_count, 1);
    assert_eq!(snap.placeholders.len(), snap.arg_count);
    assert!(
        arg_kind_is_primitive(&snap.arg_types[0]),
        "pl011 baud_rate arg must surface as a primitive after LogArg stripping (got {:?})",
        snap.arg_types[0],
    );
}

// ---------------------------------------------------------------------------
// Tests: negative paths
// ---------------------------------------------------------------------------

#[test]
fn debug_format_specifier_is_rejected() {
    let src = write_fixture("unsupported_format", UNSUPPORTED_FORMAT_FIXTURE);
    let outcome = with_fixture(
        src,
        "log_metadata_unsupported",
        "log_metadata_unsupported_fmt",
        |tcx| {
            let did = find_method_def_id(tcx, "BadDriver", "log_debug_fmt");
            match snapshot_log_call(tcx, did) {
                Ok(_) => Err("expected UnsupportedLogFormat, got Ok".to_string()),
                Err(ConstructionError::UnsupportedLogFormat {
                    template,
                    placeholder,
                    ..
                }) => Ok((template, placeholder)),
                Err(other) => Err(format!("expected UnsupportedLogFormat, got {other:?}")),
            }
        },
    );

    let (template, placeholder) = outcome.expect("Rule 6 must reject `{:?}` placeholders");
    assert!(template.contains("{:?}"));
    assert_eq!(placeholder, "{:?}");
}

#[test]
fn arity_mismatch_is_rejected() {
    let src = write_fixture("arity_mismatch", ARITY_MISMATCH_FIXTURE);
    let outcome = with_fixture(
        src,
        "log_metadata_arity",
        "log_metadata_arity_mismatch",
        |tcx| {
            let did = find_method_def_id(tcx, "BadDriver", "log_too_few_args");
            match snapshot_log_call(tcx, did) {
                Ok(_) => Err("expected LogPlaceholderArityMismatch, got Ok".to_string()),
                Err(ConstructionError::LogPlaceholderArityMismatch {
                    placeholders, args, ..
                }) => Ok((placeholders, args)),
                Err(other) => Err(format!(
                    "expected LogPlaceholderArityMismatch, got {other:?}",
                )),
            }
        },
    );

    let (placeholders, args) = outcome.expect("Rule 6 must reject placeholder/arg arity mismatch");
    assert_eq!(placeholders, 2);
    assert_eq!(args, 1);
}
