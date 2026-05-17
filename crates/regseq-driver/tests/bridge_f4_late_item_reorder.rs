#![feature(rustc_private)]

//! Integration regression for regseq-driver-47 (spec.md sections 3.4
//! Phase 3/4 + 16.1).
//!
//! Bridges a fixture that forces a Phase-5 late `ConstMemoryStatic`
//! Item under the primary crate. Without the items-side F4 sort that
//! lands in this task, the late item's `<regseq_synth>` `TypeNs`
//! segment would sort before a `ValueNs`-leaf source function's
//! segment, but it is appended past the original F4-sorted batch and
//! `model.validate()` trips `InvariantViolation::F4OutOfOrder`. With
//! the sort wired into the bridge pipeline,
//! `construct_instances_and_resolve_cross_references` re-sorts the
//! items via `RegseqModel::sort_items_by_f4` and remaps every
//! `ItemId` reference, including bridge-side `HandleTable` bindings
//! used by the subsequent `construct_const_memory_static_instances`
//! step.

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::{BridgeInput as ModelBridgeInput, RegseqModel};

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
    std::env::temp_dir().join(format!("regseq-bridge-f4-reorder-{crate_name}-{nanos}"))
}

fn compile_with_bridge(crate_name: &str, source: &str) -> RegseqModel {
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
        result: Arc<Mutex<Option<RegseqModel>>>,
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
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let model = run_bridge(input).expect("bridge should run");
            *self.result.lock().unwrap() = Some(model);
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

/// Source fixture: a `ValueNs`-leaf `emit` function that passes a
/// byte-slice literal to a `sink` callee. The slice literal forces
/// Phase 5 body materialisation to invoke `translate_const`, which
/// in turn calls `synthesise_const_memory_static` to add a
/// `ConstMemoryStatic` Item under `cx.model.primary_crate` with a
/// `<regseq_synth>` `TypeNs` segment leaf. The synth segment sorts
/// before `emit` / `sink`'s `ValueNs` leaves per spec.md section 16.1
/// (`TypeNs` rank < `ValueNs` rank), so without `sort_items_by_f4`
/// the late-pending drain leaves `model.items` out of F4 order and
/// `model.validate()` trips `InvariantViolation::F4OutOfOrder`.
fn late_const_memory_static_fixture() -> &'static str {
    r#"#![no_std]

#[inline(never)]
pub fn sink(_s: &[u8]) {}

#[no_mangle]
pub extern "C" fn emit() {
    sink(b"x={}")
}
"#
}

// Test Plan item 9 (spec.md section 3.4 Phase 3/4 + section 16.1).
#[test]
fn diag_logging_shape_fixture_validates_post_sort() {
    let mut model =
        compile_with_bridge("late_reorder_validate", late_const_memory_static_fixture());
    let result = model.validate(&ModelBridgeInput::matching(&model));
    assert_eq!(
        result,
        Ok(()),
        "validate should accept the model after sort_items_by_f4 + ItemId remap"
    );
}

// Test Plan item 10 (spec.md section 16.1).
#[test]
fn diag_logging_shape_fixture_items_are_in_f4_order() {
    let model = compile_with_bridge("late_reorder_monotonic", late_const_memory_static_fixture());
    for idx in 0..model.items.len().saturating_sub(1) {
        let first = &model.items[regseq_model::ItemId::from_u32(idx as u32)];
        let second = &model.items[regseq_model::ItemId::from_u32((idx + 1) as u32)];
        assert!(
            model.compare_items_f4(first, second) != core::cmp::Ordering::Greater,
            "F4 monotonicity violated between adjacent items at positions {} and {}",
            idx,
            idx + 1,
        );
    }
}

// Test Plan item 11: no-op fixture (regression-clean). Asserts that
// running `run_bridge` on a fixture with no late ConstMemoryStatic
// synthesis leaves `model.items` in the same F4 order it would have
// reached without the new sort -- i.e., the sort_items_by_f4 call is
// a no-op for this shape and does not perturb the rendered ordering.
#[test]
fn no_op_fixture_items_unchanged_by_sort() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;
    let mut model = compile_with_bridge("late_reorder_noop", source);
    let result = model.validate(&ModelBridgeInput::matching(&model));
    assert_eq!(result, Ok(()), "no-op fixture must validate post-sort");
    // `model.items` order is the F4-deterministic order produced by
    // Phase 3 / Phase 4 without late additions. Capture the rendered
    // paths and re-run sort_items_by_f4 directly: idempotency means
    // the order is unchanged.
    let before: Vec<String> = model
        .items
        .iter()
        .map(|item| model.strings.resolve(item.def_path.rendered).to_string())
        .collect();
    let remap = model.sort_items_by_f4();
    let after: Vec<String> = model
        .items
        .iter()
        .map(|item| model.strings.resolve(item.def_path.rendered).to_string())
        .collect();
    assert_eq!(
        before, after,
        "no-op fixture must not change model.items order under sort_items_by_f4"
    );
    // Identity remap: every old ItemId maps to the same new ItemId.
    for (old, new) in remap.iter().copied().enumerate() {
        assert_eq!(
            new.as_u32() as usize,
            old,
            "no-op fixture must yield identity remap"
        );
    }
}
