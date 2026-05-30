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
    std::env::temp_dir().join(format!("regseq-bridge-{crate_name}-{nanos}"))
}

fn compile_with_bridge(crate_name: &str, source: &str) -> (usize, regseq_model::RegseqModel) {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<(usize, regseq_model::RegseqModel)>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let discovered = input.discovered_mono_items.clone();
            let model = run_bridge(input).expect("bridge should run");
            let discovered_len = discovered.lock().unwrap().len();
            *self.result.lock().unwrap() = Some((discovered_len, model));
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
        .expect("after_analysis should produce a bridge result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

#[test]
fn no_op_crate_phase_1_discovers_mono_items() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;

    let (discovered, model) = compile_with_bridge("noop_bridge", source);

    assert!(discovered > 0);
    // Per spec.md section 3.4 Phase 5 contract (`spec.md:1226`):
    // one Instance per (Item, GenericArgs, flavor) tuple. The
    // single discovered `no_op` FnDef yields exactly one Function
    // Instance after regseq-driver-46 lands the Phase-2 FnDef
    // Item-interning pass.
    assert_eq!(model.instances.len(), 1);
}

#[test]
fn no_op_crate_phase_4_is_deterministic_across_runs() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;

    let item_counts = (0..5)
        .map(|_| compile_with_bridge("noop_bridge_det", source).1.items.len())
        .collect::<Vec<_>>();

    assert!(item_counts.iter().all(|count| *count == item_counts[0]));
}

#[test]
fn empty_crate_returns_empty_instances() {
    let source = r#"#![no_std]"#;

    let (_discovered, model) = compile_with_bridge("empty_bridge", source);

    assert!(model.instances.is_empty());
}

#[test]
fn foreign_fn_emission_name_is_unmangled_abi_symbol() {
    // regseq-driver-84: a `body: None` foreign function (`extern "C" {
    // fn abs(i: i32) -> i32; }`) must carry its real ABI link name
    // (`abs`) as its `emission_name`, NOT the rendered DefPath
    // (`..._extern_0__abs`). The latter compiles but fails at link
    // because libc exports `abs`, not the mangled name.
    let source = r#"#![no_std]

extern "C" {
    fn abs(i: i32) -> i32;
}

#[no_mangle]
pub extern "C" fn caller() -> i32 {
    unsafe { abs(-5) }
}
"#;

    let (_discovered, model) = compile_with_bridge("foreign_abs_bridge", source);

    // Find the foreign `body: None` Function instance.
    let foreign = model
        .instances
        .iter()
        .find(|inst| {
            model.items[inst.item].linkage == regseq_model::Linkage::Foreign
                && inst
                    .kind
                    .as_function()
                    .is_some_and(|data| data.body.is_none())
        })
        .expect("a foreign `body: None` Function instance for `abs`");

    assert_eq!(
        model.strings.resolve(foreign.emission_name),
        "abs",
        "foreign fn emission_name must be the unmangled ABI link name"
    );
}
