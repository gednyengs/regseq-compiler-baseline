//! Bridge integration for the atomic-memory-access-and-ordering (atomicity)
//! M2 frontend legality (C01/C02): the per-op valid-ordering matrix
//! (`ATOMIC-ERR-DEGENERATE-ACQREL` / `ATOMIC-ERR-FENCE-RELAXED`), the
//! const-`Ordering` discipline (`ATOMIC-ERR-ORDERING-NONCONST`, D14), the
//! bit-test element/bounds rule (`ATOMIC-ERR-BIT-TEST-TYPE`, C07/D08), the
//! element-type rule (`ATOMIC-ERR-ELEMENT-TYPE`), and the facade-signature
//! residual (`ATOMIC-ERR-MISSING-ORDERING`, D15).
//!
//! The atomic ops live in a `#[no_mangle] extern "C"` `regseq_main` entry whose
//! body the bridge materializes (the codegen-fixture entry shape), so the
//! Phase-5 op-legality pass walks the calls. The atomic state is a PLAIN struct
//! (zero L2, no lifecycle — D11), never a `#[rseq::driver]`, so the
//! device-lifecycle user-data classifier never sees the `Atomic` field.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
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

fn get_sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-bridge-atomic-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; `Ok(())` on success, else the rendered
/// `BridgeError` for diagnostic assertions.
fn bridge(crate_name: &str, source: &str) -> Result<(), String> {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Result<(), String>>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(_model) => Ok(()),
                Err(err) => Err(err.to_string()),
            };
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = get_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

/// The canonical atomic facade (Ordering + Atomic<T> + the M1 ops + fence),
/// inlined per the per-fixture convention.
const FACADE: &str = r#"
#[derive(Clone, Copy)]
pub enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }
pub struct Atomic<T> { value: T }
impl<T: Copy> Atomic<T> {
    pub const fn new(value: T) -> Self { Self { value } }
    #[inline(never)] #[rseq::intrinsic("atomic_load")]
    pub fn load(&self, _o: Ordering) -> T { self.value }
    #[inline(never)] #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _v: T, _o: Ordering) {}
    #[inline(never)] #[rseq::intrinsic("atomic_bit_test_set")]
    pub fn bit_test_set(&self, _b: u32, _o: Ordering) -> bool { false }
    #[inline(never)] #[rseq::intrinsic("atomic_bit_test_clear")]
    pub fn bit_test_clear(&self, _b: u32, _o: Ordering) -> bool { false }
}
pub mod atomic {
    use super::Ordering;
    #[inline(never)] #[rseq::intrinsic("atomic_fence")]
    pub fn fence(_o: Ordering) {}
}
"#;

/// Build a bridgeable crate: the facade, `struct_def`, and a `regseq_main`
/// entry running `body`.
fn program(struct_def: &str, body: &str) -> String {
    format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {FACADE}\n{struct_def}\n\
         #[no_mangle]\npub extern \"C\" fn regseq_main() {{\n{body}\n}}\n"
    )
}

/// A `u32` driver-state struct + its construction (the common case).
const U32_STATE: &str = "struct S { f: Atomic<u32> }";
fn u32_state_body(ops: &str) -> String {
    format!("    let s = S {{ f: Atomic::new(0) }};\n{ops}")
}

// ---- positive: the valid M1 slice compiles ---------------------------------

#[test]
fn valid_orderings_are_accepted() {
    let ok = bridge(
        "atm_ok",
        &program(
            U32_STATE,
            &u32_state_body(
                "    s.f.store(1, Ordering::Release);\n\
                 \tlet _ = s.f.load(Ordering::Acquire);\n\
                 \tlet _ = s.f.bit_test_set(3, Ordering::SeqCst);\n\
                 \tlet _ = s.f.bit_test_clear(3, Ordering::Relaxed);",
            ),
        ),
    );
    assert!(ok.is_ok(), "the valid M1 ordering slice must bridge: {ok:?}");
}

// ---- ordering matrix (C01) -------------------------------------------------

#[test]
fn acqrel_on_load_is_degenerate() {
    let err = bridge(
        "atm_load_acqrel",
        &program(U32_STATE, &u32_state_body("    let _ = s.f.load(Ordering::AcqRel);")),
    )
    .expect_err("AcqRel on load must reject");
    assert!(err.contains("ATOMIC-ERR-DEGENERATE-ACQREL") || err.contains("not valid for load"), "got: {err}");
}

#[test]
fn release_on_load_is_degenerate() {
    let err = bridge(
        "atm_load_release",
        &program(U32_STATE, &u32_state_body("    let _ = s.f.load(Ordering::Release);")),
    )
    .expect_err("Release on load must reject");
    assert!(err.contains("not valid for load"), "got: {err}");
}

#[test]
fn acquire_on_store_is_degenerate() {
    let err = bridge(
        "atm_store_acquire",
        &program(U32_STATE, &u32_state_body("    s.f.store(1, Ordering::Acquire);")),
    )
    .expect_err("Acquire on store must reject");
    assert!(err.contains("not valid for store"), "got: {err}");
}

#[test]
fn relaxed_fence_is_rejected() {
    let err = bridge(
        "atm_fence_relaxed",
        &program(U32_STATE, &u32_state_body("    atomic::fence(Ordering::Relaxed);")),
    )
    .expect_err("fence(Relaxed) must reject");
    assert!(err.contains("ATOMIC-ERR-FENCE-RELAXED") || err.contains("Relaxed fence"), "got: {err}");
}

#[test]
fn nonconst_ordering_is_rejected() {
    // A runtime-valued Ordering (an `#[inline(never)]` boundary keeps it out of
    // const-folding) reaches the call as a non-const operand -> ORDERING-NONCONST.
    let err = bridge(
        "atm_nonconst",
        &program(
            "#[inline(never)] fn rt() -> Ordering { Ordering::Acquire }\nstruct S { f: Atomic<u32> }",
            &u32_state_body("    let _ = s.f.load(rt());"),
        ),
    )
    .expect_err("a runtime Ordering must reject");
    assert!(err.contains("ATOMIC-ERR-ORDERING-NONCONST") || err.contains("compile-time constant"), "got: {err}");
}

// ---- element / bit-test legality (C07/D08) ---------------------------------

#[test]
fn signed_bit_test_is_rejected() {
    let err = bridge(
        "atm_bittest_signed",
        &program(
            "struct S { f: Atomic<i32> }",
            "    let s = S { f: Atomic::new(0) };\n\tlet _ = s.f.bit_test_set(3, Ordering::SeqCst);",
        ),
    )
    .expect_err("bit_test on a signed atomic must reject");
    assert!(err.contains("ATOMIC-ERR-BIT-TEST-TYPE") || err.contains("unsigned integer atomic"), "got: {err}");
}

#[test]
fn const_out_of_range_bit_is_rejected() {
    let err = bridge(
        "atm_bit_oob",
        &program(U32_STATE, &u32_state_body("    let _ = s.f.bit_test_set(40, Ordering::SeqCst);")),
    )
    .expect_err("an out-of-range const bit index must reject");
    assert!(err.contains("ATOMIC-ERR-BIT-TEST-TYPE") || err.contains("out of range"), "got: {err}");
}

#[test]
fn unsupported_element_type_is_rejected() {
    // `Atomic<f32>` is outside {u32,i32,u64,i64}+pointer. The field is
    // constructed so the struct is interned and walked.
    let err = bridge(
        "atm_elem",
        &program(
            "struct S { f: Atomic<f32> }",
            "    let _s = S { f: Atomic::new(0.0f32) };",
        ),
    )
    .expect_err("Atomic<f32> must reject");
    assert!(err.contains("ATOMIC-ERR-ELEMENT-TYPE") || err.contains("unsupported element type"), "got: {err}");
}

// ---- facade-signature residual (D15) ---------------------------------------

#[test]
fn misdeclared_facade_missing_ordering_is_rejected() {
    // A method tagged `atomic_load` whose signature omits the `Ordering`
    // parameter is a mis-declared facade -> MISSING-ORDERING (the canonical
    // call-site omission is a rustc error, type-system-subsumed).
    let src = format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         #[derive(Clone, Copy)] pub enum Ordering {{ Relaxed, Acquire, Release, AcqRel, SeqCst }}\n\
         pub struct Atomic<T> {{ value: T }}\n\
         impl<T: Copy> Atomic<T> {{\n\
         \tpub const fn new(value: T) -> Self {{ Self {{ value }} }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_load\")]\n\
         \tpub fn load(&self) -> T {{ self.value }}\n}}\n\
         struct S {{ f: Atomic<u32> }}\n\
         #[no_mangle] pub extern \"C\" fn regseq_main() {{\n\
         \tlet s = S {{ f: Atomic::new(0) }};\n\tlet _ = s.f.load();\n}}\n"
    );
    let err = bridge("atm_missing_ord", &src).expect_err("mis-declared facade must reject");
    assert!(err.contains("ATOMIC-ERR-MISSING-ORDERING") || err.contains("must take an explicit `Ordering`"), "got: {err}");
}

#[test]
fn misdeclared_store_wrong_arity_is_rejected() {
    // A `store` facade missing its VALUE parameter declares `Ordering` at
    // position 1, not the canonical position 2. The signature check rejects it
    // (MISSING-ORDERING) so the ops check cannot silently skip the absent
    // position-2 ordering and accept an invalid Acquire-on-store.
    let src = format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         #[derive(Clone, Copy)] pub enum Ordering {{ Relaxed, Acquire, Release, AcqRel, SeqCst }}\n\
         pub struct Atomic<T> {{ value: T }}\n\
         impl<T: Copy> Atomic<T> {{\n\
         \tpub const fn new(value: T) -> Self {{ Self {{ value }} }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_store\")]\n\
         \tpub fn store(&self, _o: Ordering) {{}}\n}}\n\
         struct S {{ f: Atomic<u32> }}\n\
         #[no_mangle] pub extern \"C\" fn regseq_main() {{\n\
         \tlet s = S {{ f: Atomic::new(0) }};\n\ts.f.store(Ordering::Acquire);\n}}\n"
    );
    let err = bridge("atm_store_arity", &src).expect_err("wrong-arity store facade must reject");
    assert!(
        err.contains("ATOMIC-ERR-MISSING-ORDERING"),
        "wrong-arity store must be MISSING-ORDERING (not silently accepted); got: {err}"
    );
}

#[test]
fn mmio_fence_collapse_is_designed_in() {
    // M5/T19: `ATOMIC-ERR-MMIO-FENCE-COLLAPSE` is in the catalog (C02/I02) but
    // DESIGNED-IN with NO live trigger (D02/O1) — the trigger (an MMIO-ordering
    // op smuggled onto the atomic surface) is defined only when register-access
    // lands its `io_fence`/`ordered!` ordering surface. This asserts the variant
    // EXISTS and renders its ID; the absence of a live trigger is structural —
    // no `bridge::atomic` pass ever constructs
    // `AtomicDiagnostic::MmioFenceCollapse` (grep-verifiable). `atomic::fence`
    // itself is CPU-only and never collapses to MMIO (I03).
    let d = regseq_driver::bridge::AtomicDiagnostic::MmioFenceCollapse {
        mmio_op: "io_fence".to_string(),
    };
    let rendered = d.to_string();
    assert!(
        rendered.contains("ATOMIC-ERR-MMIO-FENCE-COLLAPSE"),
        "the designed-in variant must render its ID; got: {rendered}"
    );
}
