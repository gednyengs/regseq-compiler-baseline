//! Bridge integration for item-driven `#[rseq::driver(...)]` recognition (T05),
//! the typed PCI/AMBA match sugar + deferred-bus rejection (T07), and the
//! `#[rseq::device_class]` tag (T08).
//!
//! These exercise the REAL tool-attribute token parse — which a unit test over
//! `build_descriptor` cannot reach, and which `Attribute::meta_item_list()`
//! cannot perform (the `compatible = [...]` array value is not a meta-item
//! literal, so the parser walks the raw `Unparsed` token stream instead).
//!
//! Harness mirrors `bridge_framework_type_recognition.rs`: compile an inline
//! crate under the pinned toolchain with `no_codegen`, run the bridge in the
//! `after_analysis` callback, and assert against the constructed `RegseqModel`.

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
use regseq_model::{ClassTag, DriverLifecycle, LifecycleOps, MatchEntry, RegseqModel, UserDataKind};
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
    std::env::temp_dir().join(format!("regseq-bridge-device-lifecycle-{crate_name}-{nanos}"))
}

/// Bridge `source` and run `f` against the resulting model. Returns
/// `Some(result)` on a successful bridge; `None` if the bridge REJECTED the
/// crate (e.g. the T06 non-driver-bind reject), so a test can assert rejection.
fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> Option<R>
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

    let result: Arc<Mutex<Option<Option<R>>>> = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<Option<R>>>>,
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
            // `Some(None)` => bridge rejected; `Some(Some(r))` => bridged ok.
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(model) => {
                    let f = self.callback.lock().unwrap().take().expect("callback once");
                    Some(f(&model))
                }
                Err(_) => None,
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

/// Find the recognized driver whose descriptor name resolves to `name`.
fn driver_by_name<'m>(model: &'m RegseqModel, name: &str) -> Option<&'m DriverLifecycle> {
    model
        .device_lifecycle
        .drivers
        .values()
        .find(|d| model.strings.resolve(d.descriptor.name) == name)
}

/// The devicetree `compatible` strings of a driver's match set, in order.
fn dt_compatibles(model: &RegseqModel, driver: &DriverLifecycle) -> Vec<String> {
    driver
        .descriptor
        .match_set
        .iter()
        .map(|m| match m {
            MatchEntry::Devicetree { compatible } => model.strings.resolve(*compatible).to_string(),
            other => panic!("expected a devicetree match, got {other:?}"),
        })
        .collect()
}

/// The shared device-lifecycle test-harness surface (trait + intrinsics +
/// RegseqError + DriverHandle), included so bind scenarios match the fixtures.
const SURFACE: &str = include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");

/// Build a bridgeable crate: register_tool crate attrs + the shared surface +
/// the driver/`regseq_main` snippet.
fn bind_source(driver_and_main: &str) -> String {
    format!("#![feature(register_tool)]\n#![register_tool(rseq)]\n{SURFACE}\n{driver_and_main}")
}

/// Two `#[rseq::driver]` structs (single- and multi-`compatible`) plus a plain
/// struct. Each is referenced as a return type so the bridge interns it.
const SOURCE: &str = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "fake", compatible = ["acme,fake"])]
pub struct Fake;

#[rseq::driver(name = "multi", compatible = ["acme,one", "acme,two"])]
pub struct Multi;

#[rseq::driver(name = "raw", compatible = [r"acme,raw"])]
pub struct Raw;

pub struct Plain;

pub fn make_fake() -> Fake { loop {} }
pub fn make_multi() -> Multi { loop {} }
pub fn make_raw() -> Raw { loop {} }
pub fn make_plain() -> Plain { loop {} }

pub fn entry() {}
"#;

#[test]
fn dt_driver_is_recognized_with_name_and_match() {
    let got = with_model("dl_dt", SOURCE, |model| {
        let d = driver_by_name(model, "fake").expect("fake driver recognized");
        dt_compatibles(model, d)
    })
    .expect("bridge should succeed");
    assert_eq!(got, vec!["acme,fake".to_string()]);
}

#[test]
fn multiple_compatibles_preserved_in_order() {
    let got = with_model("dl_multi", SOURCE, |model| {
        let d = driver_by_name(model, "multi").expect("multi driver recognized");
        dt_compatibles(model, d)
    })
    .expect("bridge should succeed");
    assert_eq!(got, vec!["acme,one".to_string(), "acme,two".to_string()]);
}

#[test]
fn raw_string_compatible_is_parsed() {
    let got = with_model("dl_raw", SOURCE, |model| {
        let d = driver_by_name(model, "raw").expect("raw driver recognized");
        dt_compatibles(model, d)
    })
    .expect("bridge should succeed");
    assert_eq!(got, vec!["acme,raw".to_string()]);
}

/// Which of the five `LifecycleOps` slots are `Some` (overridden + force-
/// materialized), in fixed order: init, deinit, surprise, init_failure, shutdown.
fn ops_present(d: &DriverLifecycle) -> [bool; 5] {
    [
        d.ops.on_init.is_some(),
        d.ops.on_deinit.is_some(),
        d.ops.on_surprise_remove.is_some(),
        d.ops.on_init_failure.is_some(),
        d.ops.on_system_shutdown.is_some(),
    ]
}

#[test]
fn driver_with_impl_resolves_item_driven_without_a_bind() {
    // RD-2: recognition is ITEM-driven, not bind-driven. A driver declaring an
    // `impl Lifecycle` overriding `on_init` resolves that callback even with NO
    // `lifecycle::bind` site anywhere — the old bind-gating is gone.
    let driver = r#"
#[rseq::driver(name = "fake", compatible = ["acme,fake"])]
pub struct Fake;

impl Lifecycle for Fake {
    fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) }
}

pub fn regseq_main() -> Result<(), RegseqError> { Ok(()) }
"#;
    let src = bind_source(driver);
    let present = with_model("dl_unbound_impl", &src, |model| {
        driver_by_name(model, "fake").map(ops_present)
    })
    .expect("scenario should bridge");
    assert_eq!(
        present,
        Some([true, false, false, false, false]),
        "the overridden on_init resolves item-driven, with no bind site (RD-2)"
    );
}

#[test]
fn driver_without_impl_lifecycle_is_interned_with_all_none_ops() {
    // B1/RD-3: a driver with NO `impl Lifecycle` is still a valid driver —
    // interned, field-classified (classify_fields runs regardless of the impl),
    // and with all-None callbacks (construct-level optionality). A fieldless ZST
    // driver yields an empty handle here; the resource-field classification of a
    // no-impl driver (a real `Mmio` register block) is exercised end-to-end at
    // T24 (the lifecycle-handle fixture), since a standalone driver-with-Mmio
    // bridge needs the register-block surface, not a toy `Mmio<T>`.
    let driver = r#"
#[rseq::driver(name = "bare", compatible = ["acme,bare"])]
pub struct Bare;

pub fn regseq_main() -> Result<(), RegseqError> { Ok(()) }
"#;
    let src = bind_source(driver);
    let present = with_model("dl_no_impl", &src, |model| {
        driver_by_name(model, "bare").map(ops_present)
    })
    .expect("scenario should bridge");
    assert_eq!(
        present,
        Some([false, false, false, false, false]),
        "a no-impl driver is recognized but has no resolved callbacks (B1/RD-3)"
    );
}

#[test]
fn plain_struct_is_not_recognized_as_a_driver() {
    let (count, has_plain) = with_model("dl_plain", SOURCE, |model| {
        let count = model.device_lifecycle.drivers.len();
        let has_plain = driver_by_name(model, "plain").is_some();
        (count, has_plain)
    })
    .expect("bridge should succeed");
    assert_eq!(
        count, 3,
        "exactly the three #[rseq::driver] structs are recognized"
    );
    assert!(!has_plain, "a plain struct is never a driver");
}

// ---------------------------------------------------------------------------
// T07 — typed PCI/AMBA match sugar + CR-5 deferred-bus rejection.
// ---------------------------------------------------------------------------

/// A crate exercising the typed bus-match sugar: a PCI driver (with optional
/// `subsystem`/`class`), an AMBA driver, and a bare PCI driver (required fields
/// only). Each is referenced as a return type so the bridge interns it.
const TYPED_MATCH_SOURCE: &str = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "e1000", pci = [{ vendor = 0x8086, device = 0x100e }])]
pub struct E1000;

#[rseq::driver(name = "e1000sub", pci = [{ vendor = 0x8086, device = 0x100e, subsystem = 0x12345678, class = 0x020000 }])]
pub struct E1000Sub;

#[rseq::driver(name = "pl011", amba = [{ id = 0x00041011, mask = 0x000fffff }])]
pub struct Pl011;

pub fn make_e1000() -> E1000 { loop {} }
pub fn make_e1000sub() -> E1000Sub { loop {} }
pub fn make_pl011() -> Pl011 { loop {} }
"#;

#[test]
fn pci_driver_is_recognized_with_typed_match() {
    let got = with_model("dl_pci", TYPED_MATCH_SOURCE, |model| {
        driver_by_name(model, "e1000")
            .expect("e1000 recognized")
            .descriptor
            .match_set
            .clone()
    })
    .expect("bridge should succeed");
    assert_eq!(got.len(), 1);
    assert!(
        matches!(
            got[0],
            MatchEntry::Pci {
                vendor: 0x8086,
                device: 0x100e,
                subsystem: None,
                class: None,
            }
        ),
        "got {:?}",
        got[0]
    );
}

#[test]
fn pci_optional_subsystem_and_class_are_parsed() {
    let got = with_model("dl_pci_sub", TYPED_MATCH_SOURCE, |model| {
        driver_by_name(model, "e1000sub")
            .expect("e1000sub recognized")
            .descriptor
            .match_set
            .clone()
    })
    .expect("bridge should succeed");
    assert!(
        matches!(
            got[0],
            MatchEntry::Pci {
                vendor: 0x8086,
                device: 0x100e,
                subsystem: Some(0x1234_5678),
                class: Some(0x02_0000),
            }
        ),
        "got {:?}",
        got[0]
    );
}

#[test]
fn amba_driver_is_recognized_with_typed_match() {
    let got = with_model("dl_amba", TYPED_MATCH_SOURCE, |model| {
        driver_by_name(model, "pl011")
            .expect("pl011 recognized")
            .descriptor
            .match_set
            .clone()
    })
    .expect("bridge should succeed");
    assert!(
        matches!(
            got[0],
            MatchEntry::Amba {
                id: 0x0004_1011,
                mask: 0x000f_ffff,
            }
        ),
        "got {:?}",
        got[0]
    );
}

/// CR-5: each deferred bus family is a hard `BridgeError` (the bridge rejects
/// the crate, so `with_model` returns `None`), not a silent drop.
fn unsupported_bus_source(bus: &str) -> String {
    format!(
        r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "dev", {bus} = [{{ id = 0x1 }}])]
pub struct Dev;

pub fn make_dev() -> Dev {{ loop {{}} }}
"#
    )
}

#[test]
fn deferred_bus_sugar_is_rejected_as_bridge_error() {
    for bus in ["acpi", "usb", "i2c", "spi"] {
        let src = unsupported_bus_source(bus);
        let outcome = with_model("dl_unsupported_bus", &src, |_model| ());
        assert!(
            outcome.is_none(),
            "`{bus}` bus sugar must reject the bridge (CR-5), but it bridged"
        );
    }
}

// ---------------------------------------------------------------------------
// T08 — `#[rseq::device_class(<tag>)]` recognition.
// ---------------------------------------------------------------------------

/// Four drivers: explicit `serial`, explicit `network`, an absent tag, and an
/// unknown `block` tag (open vocabulary → `Other`).
const DEVICE_CLASS_SOURCE: &str = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::device_class(serial)]
#[rseq::driver(name = "ser", compatible = ["acme,ser"])]
pub struct Ser;

#[rseq::device_class(network)]
#[rseq::driver(name = "net", compatible = ["acme,net"])]
pub struct Net;

#[rseq::driver(name = "plain", compatible = ["acme,plain"])]
pub struct Plain;

#[rseq::device_class(block)]
#[rseq::driver(name = "blk", compatible = ["acme,blk"])]
pub struct Blk;

pub fn make_ser() -> Ser { loop {} }
pub fn make_net() -> Net { loop {} }
pub fn make_plain() -> Plain { loop {} }
pub fn make_blk() -> Blk { loop {} }
"#;

#[test]
fn device_class_tags_map_to_class_tag() {
    let (ser, net, plain, blk) = with_model("dl_class", DEVICE_CLASS_SOURCE, |model| {
        let class_of = |name: &str| {
            driver_by_name(model, name)
                .unwrap_or_else(|| panic!("{name} recognized"))
                .descriptor
                .device_class
                .clone()
        };
        let blk = match class_of("blk") {
            ClassTag::Other(tag) => model.strings.resolve(tag).to_string(),
            other => panic!("expected Other for blk, got {other:?}"),
        };
        (class_of("ser"), class_of("net"), class_of("plain"), blk)
    })
    .expect("bridge should succeed");
    assert_eq!(ser, ClassTag::Serial);
    assert_eq!(net, ClassTag::Network);
    assert_eq!(plain, ClassTag::Generic, "absent tag defaults to Generic");
    assert_eq!(blk, "block", "unknown tag is carried verbatim as Other");
}

// ---------------------------------------------------------------------------
// Separator strictness (Codex M1 review): a missing comma is malformed, so the
// parse drops the driver / defaults the class rather than leniently merging.
// ---------------------------------------------------------------------------

#[test]
fn missing_comma_between_driver_entries_drops_the_driver() {
    // `name = "x" compatible = [...]` with no separating comma is malformed →
    // the struct is silently dropped from the driver set (the bridge still
    // succeeds; the struct is just not a recognized driver).
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "nocomma" compatible = ["acme,nocomma"])]
pub struct NoComma;

pub fn make_nocomma() -> NoComma { loop {} }
"#;
    let recognized = with_model("dl_nocomma", src, |model| {
        driver_by_name(model, "nocomma").is_some()
    })
    .expect("bridge should still succeed");
    assert!(
        !recognized,
        "a driver attr with a missing separator comma must be dropped, not accepted"
    );
}

#[test]
fn device_class_with_extra_tokens_defaults_to_generic() {
    // A `device_class` tag is a single bare identifier; `(serial, network)` is
    // malformed → Generic (not Serial).
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::device_class(serial, network)]
#[rseq::driver(name = "weird", compatible = ["acme,weird"])]
pub struct Weird;

pub fn make_weird() -> Weird { loop {} }
"#;
    let class = with_model("dl_class_extra", src, |model| {
        driver_by_name(model, "weird")
            .expect("weird recognized")
            .descriptor
            .device_class
            .clone()
    })
    .expect("bridge should succeed");
    assert_eq!(
        class,
        ClassTag::Generic,
        "a multi-token device_class is malformed and defaults to Generic"
    );
}

// ---------------------------------------------------------------------------
// T11 — user-data Rule-1/Rule-2 classification + D3 rejection (M6). The precise
// D3 diagnostic message is exercised by the M8 diag-harness (T20); these
// bridge-unit tests assert the classification + that a bad field is rejected.
// ---------------------------------------------------------------------------

/// The (field-name, kind) of a driver's user-data fields, in declaration order.
fn user_data_kinds(model: &RegseqModel, driver: &DriverLifecycle) -> Vec<(String, UserDataKind)> {
    driver
        .handle
        .user_data_fields
        .iter()
        .map(|f| (model.strings.resolve(f.name).to_string(), f.kind))
        .collect()
}

#[test]
fn scalar_user_data_fields_are_zero_init_in_decl_order() {
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "ud_scalar", compatible = ["acme,ud"])]
pub struct UdScalar { count: u32, flag: bool }
"#;
    let kinds = with_model("dl_ud_scalar", src, |model| {
        user_data_kinds(model, driver_by_name(model, "ud_scalar").expect("recognized"))
    })
    .expect("bridge should succeed");
    assert_eq!(
        kinds,
        vec![
            ("count".to_string(), UserDataKind::ZeroInit),
            ("flag".to_string(), UserDataKind::ZeroInit),
        ],
        "scalar-whitelist fields classify as ZeroInit, in declaration order"
    );
}

#[test]
fn option_copy_user_data_field_is_option_copy() {
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "ud_opt", compatible = ["acme,ud"])]
pub struct UdOpt { slot: Option<u32> }
"#;
    let kinds = with_model("dl_ud_opt", src, |model| {
        user_data_kinds(model, driver_by_name(model, "ud_opt").expect("recognized"))
    })
    .expect("bridge should succeed");
    assert_eq!(
        kinds,
        vec![("slot".to_string(), UserDataKind::OptionCopy)],
        "Option<T: Copy> classifies as OptionCopy"
    );
}

#[test]
fn no_impl_driver_still_classifies_user_data() {
    // RD-3/B1: field classification runs for EVERY driver, even one with no
    // `impl Lifecycle`; the driver flows on with all-None ops + classified fields.
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "ud_noimpl", compatible = ["acme,ud"])]
pub struct UdNoImpl { count: u32 }
"#;
    let (kinds, all_none) = with_model("dl_ud_noimpl", src, |model| {
        let d = driver_by_name(model, "ud_noimpl").expect("recognized");
        (user_data_kinds(model, d), d.ops == LifecycleOps::default())
    })
    .expect("bridge should succeed");
    assert_eq!(kinds, vec![("count".to_string(), UserDataKind::ZeroInit)]);
    assert!(
        all_none,
        "a no-impl driver has all-None LifecycleOps but is still field-classified"
    );
}

#[test]
fn non_scalar_user_data_field_is_rejected() {
    // A non-scalar Rule-1 field ([u8; 4]) is build-now-rejected (D3). The control
    // is `scalar_user_data_fields_are_zero_init_in_decl_order` (a scalar bridges).
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "ud_array", compatible = ["acme,ud"])]
pub struct UdArray { buf: [u8; 4] }
"#;
    let outcome = with_model("dl_ud_array", src, |_model| ());
    assert!(
        outcome.is_none(),
        "a non-scalar user-data field ([u8; 4]) must be rejected (D3)"
    );
}

#[test]
fn option_non_copy_user_data_field_is_rejected() {
    // Rule-2 requires the `Option` inner be `Copy`; `Option<NotCopy>` → D3.
    let src = r#"
#![feature(register_tool)]
#![register_tool(rseq)]

pub struct NotCopy { x: u8 }

#[rseq::driver(name = "ud_optnc", compatible = ["acme,ud"])]
pub struct UdOptNc { slot: Option<NotCopy> }
"#;
    let outcome = with_model("dl_ud_optnc", src, |_model| ());
    assert!(
        outcome.is_none(),
        "Option<T> with a non-Copy inner must be rejected (D3)"
    );
}
