//! regseq-driver-55 Test Plan A: Phase-5 Instance-construction
//! completeness for source `static` / `const` and source/synth
//! `struct` / `enum` / `union` Items.
//!
//! Each `#[test]` drives the full `regseq-driver` bridge against an
//! inline fixture and asserts the symmetric `Instance` shape the
//! Phase-5 passes (Changes C/D/E) must construct. The harness
//! mirrors the production `RegseqCallbacks` config consumed by
//! `cargo-regseq` and `test-ir-fixtures` (no_codegen,
//! mir_opt_level=0, always_encode_mir, the regseq MIR pass
//! whitelist, runtime checks off, panic=abort) so dyn-dispatch /
//! match fixtures materialise without tripping the body
//! translator's panic=abort assertion.

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
use regseq_model::{Const, InstanceKind, ItemKind, Mutability, RegseqModel, SyntheticItemKind};

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
    std::env::temp_dir().join(format!("regseq-phase5-instcomp-{crate_name}-{nanos}"))
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

/// Resolved emission name (the `Instance.emission_name` StringId
/// rendered through `model.strings`).
fn emission_name<'m>(model: &'m RegseqModel, instance: &regseq_model::Instance) -> &'m str {
    model.strings.resolve(instance.emission_name)
}

/// All instances whose emission name ends with `::<suffix>`.
fn instances_named<'m>(model: &'m RegseqModel, suffix: &str) -> Vec<&'m regseq_model::Instance> {
    let want = format!("::{suffix}");
    model
        .instances
        .iter()
        .filter(|inst| emission_name(model, inst).ends_with(&want))
        .collect()
}

// A1. Source struct -> Struct Instance.
#[test]
fn a1_source_struct_produces_struct_instance() {
    let model = compile_with_bridge(
        "instcomp_a1",
        r#"#![no_std]
pub struct Point { x: i32, y: i32 }
pub fn touch(p: &Point) -> i32 { p.x }
"#,
    );
    let points = instances_named(&model, "Point");
    assert_eq!(
        points.len(),
        1,
        "exactly one Point Struct Instance expected, got {}",
        points.len()
    );
    let inst = points[0];
    let InstanceKind::Struct { layout } = inst.kind else {
        panic!(
            "Point Instance must be InstanceKind::Struct, got {:?}",
            inst.kind
        );
    };
    // Layout TyId must resolve to a Ty::Struct whose (item, args)
    // matches the Instance (validate_instance_layouts enforces this
    // too, but assert directly for the Test Plan).
    match model.types.resolve(layout) {
        regseq_model::Ty::Struct { item, args, .. } => {
            assert_eq!(*item, inst.item);
            assert_eq!(args, &inst.generic_args);
        }
        other => panic!("Struct.layout must resolve to Ty::Struct, got {other:?}"),
    }
    assert!(matches!(model.items[inst.item].kind, ItemKind::Struct));
}

// A2. Source enum -> Enum Instance.
#[test]
fn a2_source_enum_produces_enum_instance() {
    // The discriminator is kept straight-line: a `&State` in the
    // fn signature forces Phase-2 `Ty::Enum` discovery without a
    // `match` body, which would trip the bridge's joint-validation
    // DFS-RPO pre-check (the `enum_round_trip` fixture documents the
    // same straight-line-body constraint -- the bridge has no
    // block-reorder pass).
    let model = compile_with_bridge(
        "instcomp_a2",
        r#"#![no_std]
pub enum State { Idle, Active(u32), Error }
pub fn classify(_s: &State) -> u32 { 0 }
"#,
    );
    let states = instances_named(&model, "State");
    assert_eq!(states.len(), 1, "exactly one State Enum Instance expected");
    assert!(
        matches!(states[0].kind, InstanceKind::Enum { .. }),
        "State Instance must be InstanceKind::Enum, got {:?}",
        states[0].kind
    );
    assert!(matches!(model.items[states[0].item].kind, ItemKind::Enum));
}

// A3. Source union -> Union Instance.
#[test]
fn a3_source_union_produces_union_instance() {
    let model = compile_with_bridge(
        "instcomp_a3",
        r#"#![no_std]
pub union Pair { x: u32, y: i32 }
pub fn touch(p: &Pair) -> u32 { unsafe { p.x } }
"#,
    );
    let pairs = instances_named(&model, "Pair");
    assert_eq!(pairs.len(), 1, "exactly one Pair Union Instance expected");
    assert!(
        matches!(pairs[0].kind, InstanceKind::Union { .. }),
        "Pair Instance must be InstanceKind::Union, got {:?}",
        pairs[0].kind
    );
    assert!(matches!(model.items[pairs[0].item].kind, ItemKind::Union));
}

// A4. Generic struct produces one Instance per monomorphisation.
#[test]
fn a4_generic_struct_one_instance_per_mono() {
    let model = compile_with_bridge(
        "instcomp_a4",
        r#"#![no_std]
pub struct Wrap<T> { v: T }
pub fn use_u32(w: &Wrap<u32>) -> u32 { w.v }
pub fn use_u64(w: &Wrap<u64>) -> u64 { w.v }
"#,
    );
    let wraps: Vec<_> = instances_named(&model, "Wrap")
        .into_iter()
        .filter(|inst| matches!(inst.kind, InstanceKind::Struct { .. }))
        .collect();
    assert_eq!(
        wraps.len(),
        2,
        "Wrap<u32> + Wrap<u64> must produce two Struct Instances, got {}",
        wraps.len()
    );
    // The two Instances' layout TyIds resolve to distinct
    // Ty::Struct { args } entries (distinct monomorphisations).
    let layouts: Vec<_> = wraps
        .iter()
        .map(|inst| match inst.kind {
            InstanceKind::Struct { layout } => layout,
            _ => unreachable!(),
        })
        .collect();
    assert_ne!(layouts[0], layouts[1], "monomorphisations must differ");
    assert_ne!(
        wraps[0].generic_args, wraps[1].generic_args,
        "the two Wrap Instances must carry distinct generic_args"
    );
}

// A5. Source static -> Static Instance with Some initialiser.
#[test]
fn a5_source_static_produces_static_instance() {
    let model = compile_with_bridge(
        "instcomp_a5",
        r#"#![no_std]
pub static COUNTER: u32 = 0;
"#,
    );
    let counters = instances_named(&model, "COUNTER");
    assert_eq!(counters.len(), 1, "exactly one COUNTER Static Instance");
    let InstanceKind::Static { init, .. } = counters[0].kind else {
        panic!(
            "COUNTER must be InstanceKind::Static, got {:?}",
            counters[0].kind
        );
    };
    let init = init.expect("source static COUNTER must have init: Some(_)");
    // `Const::Int(0)` or equivalent: the byte-level translation
    // yields the zero value of a `u32` (four zero bytes).
    let is_zero = match model.consts.resolve(init) {
        Const::Int(_, v) | Const::Uint(_, v) => *v == 0,
        Const::Bytes(bytes) => bytes
            .iter()
            .all(|b| matches!(b, regseq_model::Byte::Value(0))),
        other => panic!("unexpected COUNTER init const {other:?}"),
    };
    assert!(is_zero, "COUNTER init must be the zero value");
}

// A6. Source `static mut` -> parent Item carries Mutability::Mut.
#[test]
fn a6_source_static_mut_carries_mut_mutability() {
    let model = compile_with_bridge(
        "instcomp_a6",
        r#"#![no_std]
pub static mut FLAG: u32 = 0;
"#,
    );
    let flags = instances_named(&model, "FLAG");
    assert_eq!(flags.len(), 1, "exactly one FLAG Static Instance");
    assert!(matches!(flags[0].kind, InstanceKind::Static { .. }));
    assert!(
        matches!(
            model.items[flags[0].item].kind,
            ItemKind::Static {
                mutability: Mutability::Mut
            }
        ),
        "FLAG parent Item must carry ItemKind::Static {{ mutability: Mut }}, got {:?}",
        model.items[flags[0].item].kind
    );
}

// A7. Source const -> Const Instance.
#[test]
fn a7_source_const_produces_const_instance() {
    let model = compile_with_bridge(
        "instcomp_a7",
        r#"#![no_std]
pub const LIMIT: u32 = 64;
"#,
    );
    let limits = instances_named(&model, "LIMIT");
    assert_eq!(limits.len(), 1, "exactly one LIMIT Const Instance");
    let InstanceKind::Const { value, .. } = limits[0].kind else {
        panic!(
            "LIMIT must be InstanceKind::Const, got {:?}",
            limits[0].kind
        );
    };
    let ok = match model.consts.resolve(value) {
        Const::Uint(_, v) | Const::Int(_, v) => *v == 64,
        other => panic!("unexpected LIMIT const {other:?}"),
    };
    assert!(ok, "LIMIT const value must be 64");
    assert!(matches!(model.items[limits[0].item].kind, ItemKind::Const));
}

// A8. Synth VTableType -> Struct Instance.
#[test]
fn a8_synth_vtable_type_produces_struct_instance() {
    // `make_and_dispatch` constructs a `Concrete` and coerces
    // `&Concrete` to `&dyn Reader` -- the `Cast::Unsize` site that
    // synthesises the VTableInstance the virtual `r.read()` call
    // requires (mirrors the working `trait_object` fixture's
    // `make_and_drive`). Without it the bridge rejects the virtual
    // call ("matching VTableInstance item is absent").
    let model = compile_with_bridge(
        "instcomp_a8",
        r#"#![no_std]
pub trait Reader { fn read(&self) -> u32; }
pub struct Concrete;
impl Reader for Concrete { fn read(&self) -> u32 { 0 } }
pub fn dispatch(r: &dyn Reader) -> u32 { r.read() }
pub fn make_and_dispatch() -> u32 { let c = Concrete; dispatch(&c) }
"#,
    );
    let vtable_struct_instances: Vec<_> = model
        .instances
        .iter()
        .filter(|inst| matches!(inst.kind, InstanceKind::Struct { .. }))
        .filter(|inst| {
            matches!(
                model.item_synthetic_kinds[inst.item],
                SyntheticItemKind::VTableType { .. }
            )
        })
        .collect();
    assert!(
        !vtable_struct_instances.is_empty(),
        "synth VTableType Item must carry an InstanceKind::Struct row"
    );
}

// A9. Idempotency: the Phase-5 nominal-ADT pass dedups on
// `(item, args)`, so the bridged model carries NO duplicate nominal
// Instance rows. `construct_nominal_adt_instances` is a private
// Phase-5 pass not callable from an integration test; the
// observable invariant the spec's "no duplicate Instance rows" arm
// requires is asserted directly on the produced model (the helper's
// `existing` HashSet is exactly what enforces it, and would also
// suppress a re-entry push).
#[test]
fn a9_no_duplicate_nominal_instances() {
    let model = compile_with_bridge(
        "instcomp_a9",
        r#"#![no_std]
pub struct Point { x: i32, y: i32 }
pub struct Wrap<T> { v: T }
pub enum State { A, B }
pub union Pair { x: u32, y: i32 }
pub fn touch_point(p: &Point) -> i32 { p.x }
pub fn touch_pair(p: &Pair) -> u32 { unsafe { p.x } }
pub fn touch_state(_s: &State) -> u32 { 0 }
pub fn use_u32(w: &Wrap<u32>) -> u32 { w.v }
pub fn use_u64(w: &Wrap<u64>) -> u64 { w.v }
"#,
    );
    let mut seen = std::collections::HashSet::new();
    for inst in model.instances.iter() {
        if matches!(
            inst.kind,
            InstanceKind::Struct { .. } | InstanceKind::Enum { .. } | InstanceKind::Union { .. }
        ) {
            let key = (inst.item, format!("{:?}", inst.generic_args));
            assert!(
                seen.insert(key.clone()),
                "duplicate nominal Instance row for {key:?}"
            );
        }
    }
    // Sanity: the two Wrap monomorphisations are present (so the
    // dedup did not over-collapse distinct monomorphisations).
    let wrap_structs = instances_named(&model, "Wrap")
        .into_iter()
        .filter(|i| matches!(i.kind, InstanceKind::Struct { .. }))
        .count();
    assert_eq!(wrap_structs, 2, "both Wrap monomorphisations must survive");
}
