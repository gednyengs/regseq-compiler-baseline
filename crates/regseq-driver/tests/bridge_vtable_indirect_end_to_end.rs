//! End-to-end Phase-1a vtable-indirect bringup against the
//! `trait_object` fixture. Per regseq-driver-50 Test Plan D4:
//! drives the bridge against the extended fixture (concrete
//! `StaticByte` impl + `Cast::Unsize` site in `make_and_drive`),
//! and asserts the post-bridge shape with `model.validate` as the
//! central gate.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput as DriverBridgeInput};
use regseq_model::{
    args_key_for_generic_args, BridgeInput, CallCallee, CastKind, GenericArg, PointerCoercion,
    PrincipalTraitKey, RegseqModel, Rvalue, StatementKind, SyntheticItemKind, TerminatorKind, Ty,
};
use test_ir_fixtures::fixture_names;

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn host_sysroot() -> String {
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
        .expect("system time is past UNIX epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-vtable-e2e-{crate_name}-{nanos}"))
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test-ir-fixtures")
        .join("fixtures")
        .join(format!("{name}.rs"))
}

fn compile_with_bridge(name: &str) -> Result<RegseqModel, String> {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let src_path = fixture_path(name);
    let test_dir = fresh_test_dir(name);
    std::fs::create_dir_all(&test_dir).unwrap();

    let result: Arc<Mutex<Option<Result<RegseqModel, String>>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Result<RegseqModel, String>>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            // Mirror the production `RegseqCallbacks::config` (see
            // `crates/regseq-driver/src/callbacks.rs`): no codegen,
            // mir_opt_level=0 + always_encode_mir, the regseq MIR pass
            // whitelist, and runtime checks disabled. Without
            // `always_encode_mir` the bridge cannot see every body it
            // needs for body materialisation.
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
            let input = DriverBridgeInput::new(tcx);
            let outcome = match run_bridge(input) {
                Ok(model) => Ok(model),
                Err(err) => Err(format!("{err}")),
            };
            *self.result.lock().expect("result mutex") = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = host_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path
            .to_str()
            .expect("fixture path is valid UTF-8")
            .to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
    };
    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must populate the result slot");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    outcome
}

fn rendered(model: &RegseqModel, item: regseq_model::ItemId) -> String {
    model
        .strings
        .resolve(model.items[item].def_path.rendered)
        .to_string()
}

fn find_function_body<'a>(model: &'a RegseqModel, fn_name: &str) -> &'a regseq_model::Body {
    for instance in model.instances.iter() {
        let Some(function) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let item_rendered = rendered(model, instance.item);
        if item_rendered.ends_with(fn_name) || item_rendered.contains(&format!("::{fn_name}")) {
            return body;
        }
    }
    panic!("function `{fn_name}` not found among instance bodies");
}

#[test]
fn trait_object_fixture_bridges_and_validates_end_to_end() {
    // Step 1: run_bridge must return Ok. This catches Defect A
    // (missing VTableInstance synthesis), Defect B
    // (CAbiLoweringMalformed surfaced inside run_bridge's joint
    // validation), and the rewrite-side hard-errors in
    // bridge/body/vtable_indirect.rs.
    let mut model = compile_with_bridge(fixture_names::TRAIT_OBJECT)
        .expect("run_bridge must return Ok for the extended trait_object fixture");

    // Step 2: model.validate against BridgeInput::matching must
    // return Ok. This is the joint-validation gate; it catches the
    // CAbiLoweringMalformed (Defect B) and CallArgTypeMismatch
    // (Defect C) the spec calls out.
    let bridge_input = BridgeInput::matching(&model);
    model
        .validate(&bridge_input)
        .expect("model.validate must succeed; B-3 / B-6 per-arg type-match and CAbiLowering side table invariants must hold");

    // Step 3: model.vtables contains exactly one entry whose key is
    // the PrincipalTraitKey for `Reader`. The fixture only declares
    // a single trait and creates exactly one unsizing site, so
    // exactly one PrincipalTraitKey is synthesised. We further
    // anchor the key to `Reader` by matching the principal's
    // `trait_path.stable_crate_id` against the primary fixture
    // crate's stable id (the source-derived `Reader` trait lives
    // there) and by checking the VTableType Item's rendered
    // `<regseq_synth>` / `vtable_ty` synthetic def-path segments per
    // synthetic_def_path_segments.
    assert_eq!(
        model.vtables.len(),
        1,
        "exactly one vtable entry must be present for the Reader principal; got {:?}",
        model.vtables.keys().collect::<Vec<_>>()
    );
    let (reader_principal, vtable_type_item) = model
        .vtables
        .iter()
        .next()
        .map(|(k, v)| (k.clone(), *v))
        .unwrap();
    let primary_stable_crate_id = model.crates[model.primary_crate].stable_crate_id;
    assert_eq!(
        reader_principal.trait_path.stable_crate_id, primary_stable_crate_id,
        "the Reader PrincipalTraitKey.trait_path must live in the trait_object primary crate"
    );
    let vtable_type_rendered = rendered(&model, vtable_type_item);
    assert!(
        vtable_type_rendered.contains("vtable_ty"),
        "the vtable entry must reference a synthesised VTableType item; got rendered={vtable_type_rendered:?}"
    );

    // Step 4: at least one Item carries SyntheticItemKind::VTableInstance
    // for (Reader, StaticByte). The fixture only unsizes one
    // concrete impl (StaticByte), so exactly one VTableInstance
    // synthetic kind is expected. The self_ty is the canonical
    // ArgsKey computed from the StaticByte TyId; locate StaticByte
    // by name on a source-derived Struct Item, then compute its
    // ArgsKey via `args_key_for_generic_args`.
    let static_byte_item = model
        .items
        .iter()
        .enumerate()
        .find_map(|(idx, item)| {
            let rendered_path = model.strings.resolve(item.def_path.rendered);
            if rendered_path.contains("StaticByte") && !rendered_path.contains("vtable") {
                Some(regseq_model::ItemId::from_u32(idx as u32))
            } else {
                None
            }
        })
        .expect("StaticByte Item must exist in model.items after bridge");
    let static_byte_ty = model
        .types
        .iter()
        .find_map(|(ty_id, ty)| match ty {
            Ty::Struct { item, args, .. } if *item == static_byte_item && args.is_empty() => {
                Some(ty_id)
            }
            _ => None,
        })
        .expect("Ty::Struct view of StaticByte must be interned");
    let expected_self_ty =
        args_key_for_generic_args(&model, &vec![GenericArg::Type(static_byte_ty)]);
    let matching_vtable_instances: Vec<_> = model
        .item_synthetic_kinds
        .iter()
        .filter(|kind| {
            matches!(
                kind,
                SyntheticItemKind::VTableInstance { principal, self_ty }
                    if *principal == reader_principal && *self_ty == expected_self_ty
            )
        })
        .collect();
    assert_eq!(
        matching_vtable_instances.len(),
        1,
        "exactly one SyntheticItemKind::VTableInstance for (Reader, StaticByte) must exist; got count={}",
        matching_vtable_instances.len()
    );

    // Step 5: drive(...)'s Body contains exactly one
    // TerminatorKind::Call, and that Call's callee is
    // CallCallee::Indirect (NOT Direct) per the spec.md section
    // 25.7.2 rewrite contract.
    let drive_body = find_function_body(&model, "drive");
    let call_blocks: Vec<_> = drive_body
        .basic_blocks
        .iter()
        .filter(|block| matches!(block.terminator.kind, TerminatorKind::Call { .. }))
        .collect();
    assert_eq!(
        call_blocks.len(),
        1,
        "drive() body must contain exactly one Call terminator; got {}",
        call_blocks.len()
    );
    let TerminatorKind::Call { callee, .. } = &call_blocks[0].terminator.kind else {
        unreachable!("filtered above");
    };
    assert!(
        matches!(callee, CallCallee::Indirect(_)),
        "drive() Call callee must be CallCallee::Indirect after vtable-indirect rewrite; got {callee:?}"
    );

    // Step 6: drive() body's local_decls.len() >= 6 (the 2 original
    // locals + the 4-5 fresh synthetic locals from spec.md
    // 11725-11748: recv, meta, func, data, and optionally raw).
    assert!(
        drive_body.local_decls.len() >= 6,
        "drive() body must have at least 6 locals after rewrite (2 original + 4-5 fresh); got {}",
        drive_body.local_decls.len()
    );

    // Step 7: make_and_drive()'s Body contains the Cast::Unsize
    // rvalue chain that triggered VTableInstance synthesis. Per
    // spec.md table at §13 synthesis triggers (1148-1152), the
    // `&StaticByte -> &dyn Reader` coercion emits an Rvalue::Cast
    // with kind CastKind::PointerCoercion(Unsize).
    let make_and_drive_body = find_function_body(&model, "make_and_drive");
    let mut found_unsize = false;
    for block in make_and_drive_body.basic_blocks.iter() {
        for stmt in block.statements.iter() {
            if let StatementKind::Assign(_, Rvalue::Cast(kind, _, _)) = &stmt.kind {
                if matches!(kind, CastKind::PointerCoercion(PointerCoercion::Unsize)) {
                    found_unsize = true;
                }
            }
        }
    }
    assert!(
        found_unsize,
        "make_and_drive() body must contain a Cast::Unsize rvalue at the &StaticByte -> &dyn Reader coercion site per spec.md table at section 13 synthesis triggers (lines 1148-1152)"
    );

    // Sanity: principal we extracted from model.vtables is the same
    // PrincipalTraitKey carried by the VTableInstance synthetic kind
    // (guards against mis-keying between vtables map and
    // item_synthetic_kinds).
    let principal_clone: PrincipalTraitKey = reader_principal;
    assert!(model.vtables.contains_key(&principal_clone));
}
