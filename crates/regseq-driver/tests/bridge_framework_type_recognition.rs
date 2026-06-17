//! regseq-driver-89 Test Plan: bridge integration for item/type-driven
//! `#[rseq::framework_type("regseq-error")]` recognition.
//!
//! Covers:
//!   - a marked enum's `ItemId` carries exactly one `FrameworkType`
//!     attr, and nowhere else;
//!   - an unmarked enum carries none;
//!   - an enum used ONLY as a function return type is still recognized
//!     (recognition is item/type-driven, not call-driven);
//!   - a multiply-annotated enum is rejected (recognizer returns None);
//!   - a wrong-string annotation is rejected;
//!   - a model containing a synthetic Item (closure captures struct)
//!     is processed without panic or mis-attribution;
//!   - a marked STRUCT (`#[rseq::framework_type("duration")]`,
//!     timed-cpu-delay-and-sleep construct) is recognized just like a
//!     marked enum — the materialization pass accepts both item kinds —
//!     and an unmarked struct carries no marker.

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
use regseq_model::{AttrData, DefPathData, FrameworkType, ItemId, RegseqModel};
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
    std::env::temp_dir().join(format!("regseq-bridge-framework-type-{crate_name}-{nanos}"))
}

fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> R
where
    R: Send + 'static,
    F: FnOnce(&RegseqModel) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<R>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(&RegseqModel) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
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
            let model = run_bridge(BridgeInput::new(tcx)).expect("bridge should run");
            let f = self.callback.lock().unwrap().take().expect("callback once");
            *self.result.lock().unwrap() = Some(f(&model));
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
        callback,
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

/// Source whose marked enum `DriverError` appears ONLY as a function
/// return type (never constructed), alongside an unmarked enum, a
/// doubly-annotated enum, a wrong-string enum, and a closure (forcing
/// a synthetic captures-struct Item into the model).
const SOURCE: &str = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::framework_type("regseq-error")]
pub enum DriverError {
    Io,
    Timeout,
}

pub enum Unmarked {
    A,
    B,
}

#[rseq::framework_type("regseq-error")]
#[rseq::framework_type("regseq-error")]
pub enum DoublyMarked {
    X,
    Y,
}

#[rseq::framework_type("not-an-error")]
pub enum WrongString {
    Z,
    W,
}

// Underscore typo: outside the closed dash-form vocabulary.
#[rseq::framework_type("regseq_error")]
pub enum Typo {
    P,
    Q,
}

// DriverError appears only here, as a return type (the diverging body
// never constructs it). Recognition must still fire: it is driven by
// the attribute on the definition, not by any call site.
pub fn returns_error() -> DriverError {
    loop {}
}

// A marked STRUCT (timed-cpu-delay-and-sleep `Duration`). Unlike the
// enum framework types above, this exercises the struct arm of the
// materialization pass. It appears only as a return type, mirroring
// `DriverError`, so recognition is proven to be item/type-driven.
#[rseq::framework_type("duration")]
pub struct Duration {
    ns: u64,
}

pub fn returns_duration() -> Duration {
    loop {}
}

// An unmarked struct: the materialization pass now walks structs too,
// so this must NOT pick up a marker.
pub struct UnmarkedStruct {
    field: u64,
}

pub fn returns_unmarked_struct() -> UnmarkedStruct {
    loop {}
}

// Forces a synthetic closure-captures Item into the model so the
// materialisation pass is exercised against a model that contains
// synthetic Items.
pub fn use_closure() -> u32 {
    let c = |x: u32| x + 1;
    c(41)
}

pub fn entry() {}
"#;

/// Count the `AttrData::FrameworkType(expected)` markers on the type
/// Item (enum OR struct) whose terminal def-path segment is `leaf`.
///
/// The kind is intentionally not filtered: enum framework types
/// (`RegseqError`/`IrqResult`) and the struct framework type
/// (`Duration`) both name their type in `TypeNs`, and the materialization
/// pass accepts both. Function names live in `ValueNs`, so the `TypeNs`
/// filter still excludes them.
fn framework_type_markers(model: &RegseqModel, leaf: &str, expected: FrameworkType) -> usize {
    let Some((idx, _)) = model.items.iter().enumerate().find(|(_, item)| {
        let Some(segment) = item.def_path.segments.last() else {
            return false;
        };
        let DefPathData::TypeNs(name) = segment.data else {
            return false;
        };
        model.strings.resolve(name) == leaf
    }) else {
        return 0;
    };
    model.domain_attrs.item_attrs[ItemId::from_u32(idx as u32)]
        .iter()
        .filter(|attr| matches!(attr, AttrData::FrameworkType(ty) if *ty == expected))
        .count()
}

/// Total `FrameworkType` markers recorded across every Item in the
/// model (used to confirm the marker lands nowhere unexpected).
fn total_framework_type_markers(model: &RegseqModel) -> usize {
    model
        .domain_attrs
        .item_attrs
        .iter()
        .flat_map(|attrs| attrs.iter())
        .filter(|attr| matches!(attr, AttrData::FrameworkType(_)))
        .count()
}

#[test]
fn marked_return_type_only_enum_is_recognized() {
    let count = with_model("fwtype_marked", SOURCE, |model| {
        framework_type_markers(model, "DriverError", FrameworkType::RegseqError)
    });
    assert_eq!(count, 1, "the marked enum carries exactly one marker");
}

#[test]
fn unmarked_enum_carries_no_marker() {
    let count = with_model("fwtype_unmarked", SOURCE, |model| {
        framework_type_markers(model, "Unmarked", FrameworkType::RegseqError)
    });
    assert_eq!(count, 0);
}

#[test]
fn doubly_annotated_enum_is_rejected() {
    let count = with_model("fwtype_doubled", SOURCE, |model| {
        framework_type_markers(model, "DoublyMarked", FrameworkType::RegseqError)
    });
    assert_eq!(count, 0, "multiple framework_type attrs => None");
}

#[test]
fn wrong_string_enum_is_rejected() {
    let count = with_model("fwtype_wrong", SOURCE, |model| {
        framework_type_markers(model, "WrongString", FrameworkType::RegseqError)
    });
    assert_eq!(count, 0);
}

#[test]
fn typo_string_enum_is_rejected() {
    let count = with_model("fwtype_typo", SOURCE, |model| {
        framework_type_markers(model, "Typo", FrameworkType::RegseqError)
    });
    assert_eq!(count, 0, "underscore typo => None");
}

#[test]
fn marked_return_type_only_struct_is_recognized() {
    // The struct arm of the materialization pass: a `Duration` struct
    // marked `#[rseq::framework_type("duration")]`, appearing only as a
    // return type, still receives exactly one `FrameworkType::Duration`
    // marker (timed-cpu-delay-and-sleep construct). This is the channel
    // by which backends later identify a Duration operand.
    let count = with_model("fwtype_struct_marked", SOURCE, |model| {
        framework_type_markers(model, "Duration", FrameworkType::Duration)
    });
    assert_eq!(count, 1, "the marked struct carries exactly one marker");
}

#[test]
fn unmarked_struct_carries_no_marker() {
    let count = with_model("fwtype_struct_unmarked", SOURCE, |model| {
        framework_type_markers(model, "UnmarkedStruct", FrameworkType::Duration)
    });
    assert_eq!(count, 0, "an unmarked struct is never attributed");
}

#[test]
fn markers_land_on_exactly_the_recognized_items() {
    // Exactly two items are recognized across the whole model: the marked
    // enum `DriverError` (RegseqError) and the marked struct `Duration`
    // (Duration). The unmarked/doubly/wrong/typo enums, the unmarked
    // struct, and the synthetic closure-captures Item all carry none.
    let total = with_model("fwtype_total", SOURCE, total_framework_type_markers);
    assert_eq!(total, 2);
}
