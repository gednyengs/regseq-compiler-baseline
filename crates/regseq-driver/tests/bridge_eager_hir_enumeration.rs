//! regseq-driver-58 Test Plan B1-B5: eager HIR enumeration of
//! source-defined top-level items and bucket-keyed Instance
//! coverage.
//!
//! Covers:
//!   - B1: every source-derived top-level item across the six
//!     `ItemKind` variants (Function / Static / Const / Struct /
//!     Enum / Union) lands in `model.items` with the right kind,
//!     linkage, and `BridgeItemFlags`.
//!   - B2: bucket 1 (non-generic + sig-rigid + non-foreign) ->
//!     exactly 1 Instance per Item.
//!   - B3: bucket 2 (foreign) -> exactly 1 Instance with
//!     `body: None` (Function) / `init: None` (Static).
//!   - B4: bucket 3 (generic) -> Item present with `is_generic ==
//!     true`; zero Instances when no monomorphisation is reached;
//!     1+ Instances when monomorphisations are forced through a
//!     downstream caller.
//!   - B5: bucket 4 (Function + signature-non-rigid via RPIT) ->
//!     Item present with `signature_non_rigid == true` and zero
//!     Instances.

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
use regseq_model::{
    BridgeItemFlags, InstanceKind, ItemId, ItemKind, Linkage, Mutability, RegseqModel,
    SyntheticItemKind,
};

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
    std::env::temp_dir().join(format!("regseq-driver-58-{crate_name}-{nanos}"))
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

    let result: Arc<Mutex<Option<RegseqModel>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<RegseqModel>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
            config.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
            config.opts.cg.overflow_checks = Some(false);
            config.opts.cg.debug_assertions = Some(false);
            config.opts.debug_assertions = false;
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
        "-C".to_string(),
        "panic=abort".to_string(),
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

/// Locate the source-derived Item whose rendered `DefPath` ends with
/// `::<leaf>` (e.g. `crate::S` matches leaf `"S"`,
/// `crate::{impl#0}::new` matches leaf `"{impl#0}::new"`). Returns
/// `(ItemId, &Item)`.
fn find_item<'m>(model: &'m RegseqModel, leaf: &str) -> (ItemId, &'m regseq_model::Item) {
    let want = format!("::{leaf}");
    for (idx, item) in model.items.iter().enumerate() {
        let rendered = model.strings.resolve(item.def_path.rendered);
        if rendered.ends_with(&want)
            && matches!(
                model.item_synthetic_kinds[ItemId::from_u32(idx as u32)],
                SyntheticItemKind::None
            )
        {
            return (ItemId::from_u32(idx as u32), item);
        }
    }
    panic!(
        "no source Item ending in `::{leaf}` found; items: {:?}",
        model
            .items
            .iter()
            .map(|it| model.strings.resolve(it.def_path.rendered))
            .collect::<Vec<_>>()
    );
}

fn instances_for(model: &RegseqModel, item_id: ItemId) -> Vec<&regseq_model::Instance> {
    model
        .instances
        .iter()
        .filter(|inst| inst.item == item_id)
        .collect()
}

fn flags_for(model: &RegseqModel, item_id: ItemId) -> BridgeItemFlags {
    model
        .item_bridge_flags
        .get(&item_id)
        .copied()
        .unwrap_or_default()
}

// ------------------------------------------------------------------
// B1: HIR enumeration completeness across all 6 ItemKinds.
// B2: Bucket 1 -> exactly 1 Instance per Item.
// ------------------------------------------------------------------
#[test]
fn b1_hir_enumeration_covers_six_item_kinds() {
    let model = compile_with_bridge(
        "b1_all_kinds",
        r#"#![no_std]
pub struct S { pub x: i32 }
pub enum E { A, B }
pub union U { pub f: u32, pub g: f32 }
pub static FOO: u32 = 1;
pub const BAR: u32 = 2;
pub fn baz() {}
"#,
    );

    // Struct S -> ItemKind::Struct, Linkage::Default, is_generic == false.
    let (s_id, s_item) = find_item(&model, "S");
    assert!(matches!(s_item.kind, ItemKind::Struct));
    assert_eq!(s_item.linkage, Linkage::Default);
    assert_eq!(flags_for(&model, s_id).is_generic, false);

    // Enum E -> ItemKind::Enum.
    let (e_id, e_item) = find_item(&model, "E");
    assert!(matches!(e_item.kind, ItemKind::Enum));
    assert_eq!(e_item.linkage, Linkage::Default);
    assert_eq!(flags_for(&model, e_id).is_generic, false);

    // Union U -> ItemKind::Union.
    let (u_id, u_item) = find_item(&model, "U");
    assert!(matches!(u_item.kind, ItemKind::Union));
    assert_eq!(u_item.linkage, Linkage::Default);
    assert_eq!(flags_for(&model, u_id).is_generic, false);

    // Static FOO -> ItemKind::Static { Not }, Linkage::Default.
    let (foo_id, foo_item) = find_item(&model, "FOO");
    assert!(matches!(
        foo_item.kind,
        ItemKind::Static {
            mutability: Mutability::Not,
        }
    ));
    assert_eq!(foo_item.linkage, Linkage::Default);
    assert_eq!(flags_for(&model, foo_id).is_generic, false);

    // Const BAR -> ItemKind::Const.
    let (_bar_id, bar_item) = find_item(&model, "BAR");
    assert!(matches!(bar_item.kind, ItemKind::Const));
    assert_eq!(bar_item.linkage, Linkage::Default);

    // Fn baz -> ItemKind::Function, is_generic == false,
    // signature_non_rigid == false.
    let (baz_id, baz_item) = find_item(&model, "baz");
    assert!(matches!(baz_item.kind, ItemKind::Function));
    assert_eq!(baz_item.linkage, Linkage::Default);
    let baz_flags = flags_for(&model, baz_id);
    assert_eq!(baz_flags.is_generic, false);
    assert_eq!(baz_flags.signature_non_rigid, false);
}

#[test]
fn b2_bucket1_items_have_exactly_one_instance() {
    let model = compile_with_bridge(
        "b2_bucket1",
        r#"#![no_std]
pub struct S { pub x: i32 }
pub enum E { A, B }
pub union U { pub f: u32, pub g: f32 }
pub static FOO: u32 = 1;
pub const BAR: u32 = 2;
pub fn baz() {}
"#,
    );

    for leaf in ["S", "E", "U", "FOO", "BAR", "baz"] {
        let (item_id, item) = find_item(&model, leaf);
        let instances = instances_for(&model, item_id);
        assert_eq!(
            instances.len(),
            1,
            "expected exactly 1 Instance for {leaf} (kind {:?}), got {}",
            item.kind,
            instances.len()
        );
        // ADT Instance layouts must be `Some(_)`, Function fn_sig
        // must resolve. Validator enforces these on its own (via
        // `validate_instance_layouts` and the FnSig interner), but
        // a smoke check here makes the Test Plan B2 assertion
        // explicit.
        match (&item.kind, &instances[0].kind) {
            (ItemKind::Struct, InstanceKind::Struct { layout })
            | (ItemKind::Enum, InstanceKind::Enum { layout })
            | (ItemKind::Union, InstanceKind::Union { layout }) => {
                let _ = model.types.resolve(*layout);
            }
            (ItemKind::Static { .. }, InstanceKind::Static { ty, init }) => {
                let _ = model.types.resolve(*ty);
                // Non-foreign source static MUST carry an init per
                // spec.md section 9.4 (`spec.md:2668-2677`).
                assert!(
                    init.is_some(),
                    "non-foreign Static {leaf} must have init: Some(_)"
                );
            }
            (ItemKind::Const, InstanceKind::Const { ty, value }) => {
                let _ = model.types.resolve(*ty);
                let _ = &model.consts[*value];
            }
            (ItemKind::Function, InstanceKind::Function(data)) => {
                let _ = model.sigs.resolve(data.fn_sig);
            }
            (item_kind, inst_kind) => {
                panic!("Item kind {item_kind:?} mismatches Instance kind {inst_kind:?} for {leaf}",)
            }
        }
    }
}

// ------------------------------------------------------------------
// B3: bucket 2 (foreign) -> exactly 1 Instance with body / init None.
// ------------------------------------------------------------------
#[test]
fn b3_foreign_function_and_static_produce_body_init_none_instances() {
    let model = compile_with_bridge(
        "b3_foreign",
        r#"#![no_std]
extern "C" {
    pub fn extern_helper(x: u32) -> u32;
    pub static EXTERN_VALUE: u32;
}
// Keep the crate non-empty so MIR collection runs.
pub fn entry() {}
"#,
    );

    // Foreign fn.
    let (fn_id, fn_item) = find_item(&model, "extern_helper");
    assert!(matches!(fn_item.kind, ItemKind::Function));
    assert_eq!(fn_item.linkage, Linkage::Foreign);
    let fn_instances = instances_for(&model, fn_id);
    assert_eq!(fn_instances.len(), 1);
    match &fn_instances[0].kind {
        InstanceKind::Function(data) => {
            assert!(
                data.body.is_none(),
                "foreign fn Instance must have body: None per spec.md section 9.4"
            );
            assert_eq!(
                data.flavor,
                regseq_model::FunctionInstanceFlavor::Item,
                "foreign fn Instance must carry FunctionInstanceFlavor::Item"
            );
        }
        other => panic!("foreign fn Instance must be Function, got {other:?}"),
    }

    // Foreign static.
    let (st_id, st_item) = find_item(&model, "EXTERN_VALUE");
    assert!(matches!(
        st_item.kind,
        ItemKind::Static {
            mutability: Mutability::Not,
        }
    ));
    assert_eq!(st_item.linkage, Linkage::Foreign);
    let st_instances = instances_for(&model, st_id);
    assert_eq!(st_instances.len(), 1);
    match &st_instances[0].kind {
        InstanceKind::Static { init, .. } => {
            assert!(
                init.is_none(),
                "foreign static Instance must have init: None per spec.md section 9.4"
            );
        }
        other => panic!("foreign static Instance must be Static, got {other:?}"),
    }
}

// ------------------------------------------------------------------
// B4: bucket 3 (generic) -> Item present with is_generic == true and
// 0 Instances when no monomorphisation is reached.
// ------------------------------------------------------------------
#[test]
fn b4_generic_items_with_zero_monomorphisations_have_no_instances() {
    let model = compile_with_bridge(
        "b4_generic",
        r#"#![no_std]
pub struct Slot<T>(pub T);
pub enum Either<L, R> { Left(L), Right(R) }
pub union Bag<T: Copy> { pub a: T, pub b: u32 }
pub fn id<T>(x: T) -> T { x }
pub fn entry() {}
"#,
    );

    for leaf in ["Slot", "Either", "Bag", "id"] {
        let (item_id, _) = find_item(&model, leaf);
        let flags = flags_for(&model, item_id);
        assert!(
            flags.is_generic,
            "generic Item {leaf} must have is_generic == true"
        );
        let instances = instances_for(&model, item_id);
        assert_eq!(
            instances.len(),
            0,
            "generic Item {leaf} with no monomorphisations must have zero Instances, got {}",
            instances.len()
        );
    }
}

// ------------------------------------------------------------------
// B4 (continuation): when a downstream caller forces a
// monomorphisation, the generic Item gains exactly one Instance for
// the reached args; the other generic Items still carry zero
// Instances. The bucket-1 caller itself carries one Instance per
// spec.md regseq-driver-58 Rule 2 bucket 1.
// ------------------------------------------------------------------
#[test]
fn b4_generic_monomorphisation_forced_via_caller_yields_one_instance() {
    let model = compile_with_bridge(
        "b4_forced",
        r#"#![no_std]
pub struct Slot<T>(pub T);
pub enum Either<L, R> { Left(L), Right(R) }
pub union Bag<T: Copy> { pub a: T, pub b: u32 }
pub fn id<T>(x: T) -> T { x }
// Force exactly one monomorphisation of `Slot<u32>` via a
// non-generic caller. The other generic Items stay at zero
// monomorphisations and zero Instances.
pub fn use_slot() -> Slot<u32> { Slot(7) }
"#,
    );

    // Generic Items still present with `is_generic == true`.
    for leaf in ["Slot", "Either", "Bag", "id"] {
        let (item_id, _) = find_item(&model, leaf);
        let flags = flags_for(&model, item_id);
        assert!(
            flags.is_generic,
            "generic Item {leaf} must have is_generic == true"
        );
    }

    // `Slot` now has EXACTLY one Instance keyed on `Slot<u32>`.
    let (slot_id, _) = find_item(&model, "Slot");
    let slot_instances = instances_for(&model, slot_id);
    assert_eq!(
        slot_instances.len(),
        1,
        "Slot must have exactly one monomorphisation Instance after use_slot, got {}",
        slot_instances.len()
    );
    match &slot_instances[0].kind {
        InstanceKind::Struct { layout } => match model.types.resolve(*layout) {
            regseq_model::Ty::Struct { item, args, .. } => {
                assert_eq!(*item, slot_id);
                assert_eq!(args.len(), 1, "Slot<u32> must carry one generic arg");
            }
            other => panic!("Slot instance layout must resolve to Ty::Struct, got {other:?}"),
        },
        other => panic!("Slot instance must be InstanceKind::Struct, got {other:?}"),
    }

    // `Either`, `Bag`, `id` still have zero Instances.
    for leaf in ["Either", "Bag", "id"] {
        let (item_id, _) = find_item(&model, leaf);
        let instances = instances_for(&model, item_id);
        assert_eq!(
            instances.len(),
            0,
            "generic Item {leaf} should still have zero Instances, got {}",
            instances.len()
        );
    }

    // `use_slot` itself is a bucket-1 Item with one Instance.
    let (use_slot_id, _) = find_item(&model, "use_slot");
    assert_eq!(instances_for(&model, use_slot_id).len(), 1);
}

// ------------------------------------------------------------------
// B5: bucket 4 (RPIT) -> Item present with signature_non_rigid ==
// true and 0 Instances. The validator accepts this (bucket-4
// carve-out).
// ------------------------------------------------------------------
#[test]
fn b5_rpit_function_is_bucket4_with_zero_instances() {
    let model = compile_with_bridge(
        "b5_rpit",
        r#"#![no_std]
pub fn make_counter() -> impl Fn(u32) -> u32 {
    |x| x + 1
}
pub fn entry() {}
"#,
    );

    let (item_id, item) = find_item(&model, "make_counter");
    assert!(matches!(item.kind, ItemKind::Function));
    assert_eq!(item.linkage, Linkage::Default);
    let flags = flags_for(&model, item_id);
    assert_eq!(flags.is_generic, false);
    assert!(
        flags.signature_non_rigid,
        "make_counter must have signature_non_rigid == true (RPIT carve-out)"
    );
    let instances = instances_for(&model, item_id);
    assert_eq!(
        instances.len(),
        0,
        "bucket-4 Function Item must have zero Instances, got {}",
        instances.len()
    );
}

// ------------------------------------------------------------------
// regseq-driver-94: the trait-declaration AssocFn `own_params` carve-out
// must survive the move of the `is_generic` formula into the shared
// `ty::source_bridge_item_flags` helper. A NON-generic trait default
// method lives at the trait's DefPath, so its `generics_of().count()`
// is inflated by the trait's inherited `Self` (count() > 0), but its
// `own_params` is empty -- it must stay bucket 1 (is_generic == false),
// NOT flip to bucket 3. If a raw `count() > 0` had been substituted in
// the helper, `Self` would misclassify it; this test guards exactly that.
// ------------------------------------------------------------------
#[test]
fn trait_default_method_stays_bucket1_after_helper_refactor() {
    let model = compile_with_bridge(
        "driver94_trait_default",
        r#"#![no_std]
pub trait Greeter {
    fn greet(&self) -> u32 {
        42
    }
}
pub struct S;
impl Greeter for S {}
pub fn entry(s: &S) -> u32 {
    s.greet()
}
"#,
    );

    // The default body surfaces as a Function Item at the trait DefPath.
    let (greet_id, greet_item) = find_item(&model, "Greeter::greet");
    assert!(matches!(greet_item.kind, ItemKind::Function));
    let greet_flags = flags_for(&model, greet_id);
    assert_eq!(
        greet_flags.is_generic, false,
        "a non-generic trait default method must stay is_generic == false (own_params carve-out); \
         a raw count() would misclassify it via the inherited `Self`"
    );
    assert_eq!(greet_flags.signature_non_rigid, false);

    // Bucket 1: the caller `entry` forces exactly one covering Instance.
    assert_eq!(
        instances_for(&model, greet_id).len(),
        1,
        "the called trait default method must have exactly one bucket-1 Instance"
    );
}
