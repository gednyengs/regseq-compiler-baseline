#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::const_::{construct_const_memory_static_instances, translate_const};
use regseq_driver::bridge::{BridgeCtxt, BridgeInput};
use regseq_model::{Const, FloatKind, InstanceKind, IntKind, SyntheticItemKind, Ty, UintKind};
use rustc_hir::def_id::{DefId, LocalModDefId};
use rustc_middle::mir;
use rustc_middle::mir::interpret::GlobalId;
use rustc_middle::ty::{self, TyCtxt};

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
    std::env::temp_dir().join(format!("regseq-bridge-const-{crate_name}-{nanos}"))
}

fn with_bridge<R, F>(crate_name: &str, source: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send + 'static,
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
    let callback =
        Arc::new(Mutex::new(Some(Box::new(f)
            as Box<
                dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send,
            >)));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<R>>>,
        callback: Arc<
            Mutex<
                Option<Box<dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send>>,
            >,
        >,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let mut cx = BridgeCtxt::new(input);
            let f = self.callback.lock().unwrap().take().expect("callback once");
            *self.result.lock().unwrap() = Some(f(tcx, &mut cx));
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

fn find_const_def_id(tcx: TyCtxt<'_>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Const(ident, ..) = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id.to_def_id();
            }
        }
    }
    panic!("const `{name}` not found");
}

fn const_as_val<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> mir::Const<'tcx> {
    let def_id = find_const_def_id(tcx, name);
    let value = tcx.const_eval_poly(def_id).expect("const should evaluate");
    let ty = tcx.type_of(def_id).instantiate_identity();
    mir::Const::Val(value, ty)
}

fn const_as_valtree<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> mir::Const<'tcx> {
    let def_id = find_const_def_id(tcx, name);
    let args = ty::GenericArgs::identity_for_item(tcx, def_id);
    let instance = ty::Instance::new_raw(def_id, args);
    let cid = GlobalId {
        instance,
        promoted: None,
    };
    let typing_env = ty::TypingEnv::post_analysis(tcx, def_id);
    let valtree = tcx
        .eval_to_valtree(typing_env.as_query_input(cid))
        .expect("const should evaluate to valtree");
    let ty = tcx.type_of(def_id).instantiate_identity();
    mir::Const::Ty(ty, ty::Const::new_value(tcx, valtree, ty))
}

#[test]
fn translates_scalar_and_zst_const_rows() {
    let source = r#"
pub const N: i32 = -7;
pub const U: usize = 42;
pub const F: f32 = 1.0;
pub const B: bool = true;
pub const C: char = 'a';
pub const Z: () = ();
pub const NULL: *const u32 = core::ptr::null();
pub static S: u32 = 7;
pub const SP: *const u32 = &S as *const u32;
"#;

    with_bridge("bridge_const_scalars", source, |tcx, cx| {
        let cases = [
            ("N", Const::Int(IntKind::I32, (-7_i32 as i128) as u128)),
            ("U", Const::Uint(UintKind::Usize, 42)),
            ("F", Const::Float(FloatKind::F32, 0x3f80_0000)),
            ("B", Const::Bool(true)),
            ("C", Const::Char('a' as u32)),
            ("Z", Const::ZST),
        ];

        for (name, expected) in cases {
            let id = translate_const(cx, const_as_val(tcx, name)).unwrap();
            assert_eq!(cx.model.consts.resolve(id), &expected);
        }

        let null = translate_const(cx, const_as_val(tcx, "NULL")).unwrap();
        let Const::NullPtr(pointee) = cx.model.consts.resolve(null) else {
            panic!("null raw pointer should translate to Const::NullPtr");
        };
        assert_eq!(cx.model.types.resolve(*pointee), &Ty::Uint(UintKind::U32));

        let reloc = translate_const(cx, const_as_val(tcx, "SP")).unwrap();
        let Const::Reloc(instance) = cx.model.consts.resolve(reloc) else {
            panic!("static pointer should translate to Const::Reloc");
        };
        assert_eq!(instance.regseq_kind_rank, 1);
    });
}

#[test]
fn translates_aggregate_enum_and_bytes_valtree_rows() {
    let source = r#"
pub enum E { A, B(u32) }
pub const T: (u32, u8) = (1, 2);
pub const X: E = E::A;
pub const BS: [u8; 3] = [1, 2, 3];
"#;

    with_bridge("bridge_const_structural", source, |tcx, cx| {
        let tuple = translate_const(cx, const_as_valtree(tcx, "T")).unwrap();
        let Const::Aggregate(fields) = cx.model.consts.resolve(tuple) else {
            panic!("tuple should translate to Const::Aggregate");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(
            cx.model.consts.resolve(fields[0]),
            &Const::Uint(UintKind::U32, 1)
        );
        assert_eq!(
            cx.model.consts.resolve(fields[1]),
            &Const::Uint(UintKind::U8, 2)
        );

        let enum_value = translate_const(cx, const_as_valtree(tcx, "X")).unwrap();
        assert!(matches!(
            cx.model.consts.resolve(enum_value),
            Const::EnumValue {
                variant_idx,
                payload
            } if variant_idx.as_u32() == 0 && payload.is_empty()
        ));

        let bytes = translate_const(cx, const_as_valtree(tcx, "BS")).unwrap();
        assert_eq!(
            cx.model.consts.resolve(bytes),
            &Const::Bytes(vec![
                regseq_model::Byte::Value(1),
                regseq_model::Byte::Value(2),
                regseq_model::Byte::Value(3),
            ])
        );
    });
}

#[test]
fn reference_typed_const_synthesises_const_memory_static() {
    let source = r#"
pub const R: &str = "abc";
pub const T: &u32 = &42;
"#;

    with_bridge("bridge_const_ref", source, |tcx, cx| {
        let parent = translate_const(cx, const_as_val(tcx, "R")).unwrap();
        let Const::Aggregate(fields) = cx.model.consts.resolve(parent) else {
            panic!("wide reference should translate to Aggregate([Reloc, Uint])");
        };
        assert_eq!(fields.len(), 2);
        let Const::Reloc(wide_reloc) = cx.model.consts.resolve(fields[0]) else {
            panic!("wide reference data field should be Const::Reloc");
        };
        assert_eq!(
            cx.model.consts.resolve(fields[1]),
            &Const::Uint(UintKind::Usize, 3)
        );
        assert_eq!(wide_reloc.def_path_hash, cx.pending_items[0].def_path_hash);
        assert!(cx.model.types.iter().any(|(_, ty)| matches!(
            ty,
            Ty::Ptr(pointee, regseq_model::Mutability::Not)
                if cx.model.types.resolve(*pointee) == &Ty::Str
        )));
        assert!(cx
            .model
            .types
            .iter()
            .any(|(_, ty)| ty == &Ty::Uint(UintKind::Usize)));
        let thin = translate_const(cx, const_as_val(tcx, "T")).unwrap();
        let Const::Reloc(thin_reloc) = cx.model.consts.resolve(thin) else {
            panic!("thin reference should translate to Const::Reloc");
        };
        assert_eq!(cx.const_memory_static_cache.len(), 2);
        assert_eq!(cx.pending_items.len(), 2);
        assert_eq!(thin_reloc.def_path_hash, cx.pending_items[1].def_path_hash);

        let mut items = Vec::new();
        for pending in std::mem::take(&mut cx.pending_items) {
            let item = cx.model.items.push(pending.item);
            cx.model.item_def_paths.push(pending.def_path_hash);
            cx.model.item_synthetic_kinds.push(pending.synthetic_kind);
            cx.model.domain_attrs.item_attrs.push(Vec::new());
            cx.handles.bind_final_item_id(pending.handle, item);
            items.push(item);
        }

        construct_const_memory_static_instances(cx).unwrap();

        for item in items {
            assert!(matches!(
                cx.model.item_synthetic_kinds[item],
                SyntheticItemKind::ConstMemoryStatic { .. }
            ));
        }
        assert_eq!(cx.model.instances.len(), 2);
        assert!(cx
            .model
            .instances
            .iter()
            .all(|instance| matches!(&instance.kind, InstanceKind::Static { init: Some(_), .. })));
    });
}
