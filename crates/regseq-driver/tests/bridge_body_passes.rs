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

use regseq_driver::bridge::body::{
    materialise_function_body, rewrite_vtable_indirect, run_phase5_body_passes,
};
use regseq_driver::bridge::fnsig::translate_fn_sig;
use regseq_driver::bridge::ty::translate_ty;
use regseq_driver::bridge::{BridgeCtxt, BridgeInput};
use regseq_model::{
    validate_body_places, ArgsKey, BasicBlock, CallCallee, DefPath, DefPathData, DefPathHash,
    DefPathSegment, FnAbi, FnSig, FunctionInstanceData, FunctionInstanceFlavor, GenericArg,
    IndexVec, InlineHint, Instance, InstanceKind, Item, ItemKind, Linkage, LocalDecl, LocalId,
    Mutability, Operand, Place, PlaceElem, PrincipalTraitKey, ProjectionBindingsKey, RegseqModel,
    Safety, Span, StableCrateId, StatementKind, SyntheticItemKind, Terminator, TerminatorKind, Ty,
    UNIT_LAYOUT,
};
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
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
    std::env::temp_dir().join(format!("regseq-bridge-body-passes-{crate_name}-{nanos}"))
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
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case, nonstandard_style)".to_string());
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

fn find_fn_def_id(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
    for item_id in tcx
        .hir_module_items(LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Fn { ident, .. } = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id;
            }
        }
    }
    panic!("function `{name}` not found");
}

fn push_function_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    cx: &mut BridgeCtxt<'tcx>,
    name: &str,
) -> regseq_model::InstanceId {
    let def_id = find_fn_def_id(tcx, name).to_def_id();
    let fn_ty = translate_ty(cx, tcx.type_of(def_id).instantiate_identity()).unwrap();
    let fn_sig = translate_fn_sig(cx, tcx.fn_sig(def_id).instantiate_identity()).unwrap();
    // Per audit-finding F4 (regseq-driver-37 v4 spec lines 325-330),
    // source-derived Items are interned with placeholder ItemIds during
    // Phase 2; drive Phase 3 + Phase 4 + Phase 5 patching so the
    // FnDef Ty resolves to the final ItemId before reading
    // `model.items[item]`.
    regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
    regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();
    let Ty::FnDef(item, args) = cx.model.types.resolve(fn_ty).clone() else {
        panic!("function type should translate as Ty::FnDef");
    };
    cx.model.instances.push(Instance {
        item,
        generic_args: args,
        emission_name: cx.model.items[item].def_path.rendered,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    })
}

#[test]
fn materialises_no_op_body_with_return_block() {
    with_bridge(
        "bridge_body_passes_no_op",
        "pub fn no_op() {}\n",
        |tcx, cx| {
            let instance = push_function_instance(tcx, cx, "no_op");
            let body = materialise_function_body(cx, instance).unwrap();

            assert_eq!(body.basic_blocks.len(), 1);
            assert!(body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .statements
                .is_empty());
            assert!(matches!(
                body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                    .terminator
                    .kind,
                TerminatorKind::Return
            ));
        },
    );
}

#[test]
fn materialises_local_decls_in_rustc_declaration_order() {
    let source = "pub fn locals(a: u32) -> u32 { let b = a + 1; b }\n";

    with_bridge("bridge_body_passes_locals", source, |tcx, cx| {
        let instance = push_function_instance(tcx, cx, "locals");
        let rustc_body = tcx.optimized_mir(find_fn_def_id(tcx, "locals").to_def_id());
        let body = materialise_function_body(cx, instance).unwrap();

        assert_eq!(body.local_decls.len(), rustc_body.local_decls.len());
        assert_eq!(body.arg_count, rustc_body.arg_count);
        for (idx, local) in rustc_body.local_decls.iter().enumerate() {
            let expected = translate_ty(cx, local.ty).unwrap();
            assert_eq!(body.local_decls[LocalId::from_u32(idx as u32)].ty, expected);
        }
    });
}

#[test]
fn phase5_driver_materialises_and_joint_validates_body() {
    with_bridge(
        "bridge_body_passes_driver",
        "pub fn no_op() {}\n",
        |tcx, cx| {
            let instance = push_function_instance(tcx, cx, "no_op");

            run_phase5_body_passes(cx).unwrap();

            let body = cx.model.instances[instance]
                .kind
                .as_function()
                .and_then(|function| function.body.as_ref())
                .expect("phase 5 should attach the materialised body");
            validate_body_places(&cx.model, body).unwrap();
        },
    );
}

#[test]
fn joint_validation_rejects_bad_place_after_materialisation() {
    with_bridge(
        "bridge_body_passes_bad_place",
        "pub fn no_op() {}\n",
        |tcx, cx| {
            let instance = push_function_instance(tcx, cx, "no_op");
            let mut body = materialise_function_body(cx, instance).unwrap();
            let mut statements = body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .statements
                .to_vec();
            statements.push(regseq_model::Statement {
                kind: StatementKind::Assign(
                    Place {
                        local: LocalId::RETURN_PLACE,
                        projection: Box::new([PlaceElem::Field(
                            regseq_model::FieldIdx::from_u32(0),
                            body.local_decls[LocalId::RETURN_PLACE].ty,
                        )]),
                    },
                    regseq_model::Rvalue::Use(regseq_model::Operand::Copy(Place::from(
                        LocalId::RETURN_PLACE,
                    ))),
                ),
                span: Span::dummy(),
            });
            body.basic_blocks[regseq_model::BlockId::START_BLOCK].statements =
                statements.into_boxed_slice();

            assert!(validate_body_places(&cx.model, &body).is_err());
        },
    );
}

fn test_item(
    model: &mut RegseqModel,
    name: &str,
    synthetic: SyntheticItemKind,
) -> regseq_model::ItemId {
    let name_id = model.strings.intern(name);
    let rendered = model.strings.intern(&format!("test::{name}"));
    let item = model.items.push(Item {
        def_path: DefPath {
            crate_id: model.primary_crate,
            segments: vec![DefPathSegment {
                data: DefPathData::ValueNs(name_id),
                disambiguator: 0,
            }],
            rendered,
        },
        kind: ItemKind::Function,
        span: Span::dummy(),
        linkage: Linkage::Default,
        link_section: None,
        inline_hint: InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    });
    model.item_def_paths.push(DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: item.as_u32() as u64 + 10,
    });
    model.item_synthetic_kinds.push(synthetic);
    model.domain_attrs.item_attrs.push(Vec::new());
    item
}

fn test_local(ty: regseq_model::TyId) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Mut,
        span: Span::dummy(),
        debug_name: None,
    }
}

fn method_principal() -> PrincipalTraitKey {
    PrincipalTraitKey {
        trait_path: DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: 90,
        },
        trait_args: ArgsKey::EMPTY,
        projection_bindings: ProjectionBindingsKey(0),
    }
}

fn populate_trait_object_dispatch_model(model: &mut RegseqModel) -> regseq_model::Body {
    let unit = model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let fn_sig = model.sigs.intern(FnSig {
        inputs: vec![model.types.intern(Ty::Ptr(unit, Mutability::Mut))],
        output: unit,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    model.sig_c_abi_lowerings.push(None);
    let fn_ptr = model.types.intern(Ty::FnPtr(fn_sig));
    let vtable_item = test_item(model, "reader_vtable", SyntheticItemKind::None);
    model.items[vtable_item].kind = ItemKind::Struct;
    let vtable_ty = model.types.intern(Ty::Struct {
        item: vtable_item,
        args: Vec::new(),
        layout: regseq_model::Layout {
            size: 8,
            align: 8,
            pack: None,
            uninhabited: false,
            kind: regseq_model::LayoutKind::Struct {
                fields: vec![regseq_model::FieldLayout {
                    name: model.strings.intern("read"),
                    ty: fn_ptr,
                    offset: 0,
                }],
            },
        },
    });
    let dynamic = model.types.intern(Ty::Dynamic {
        auto_traits: Vec::new(),
        vtable_ty,
    });
    let receiver = model
        .types
        .intern(Ty::Ref(dynamic, regseq_model::Provenance::SharedBorrow));
    let principal = method_principal();
    let vtable_instance = test_item(
        model,
        "reader_vtable_instance",
        SyntheticItemKind::VTableInstance {
            principal: principal.clone(),
            self_ty: ArgsKey::EMPTY,
        },
    );
    model.items[vtable_instance].kind = ItemKind::Static {
        mutability: Mutability::Not,
    };
    model.vtables.insert(principal.clone(), vtable_item);

    let method_item = test_item(
        model,
        "reader_read_shim",
        SyntheticItemKind::VTableMethodShim {
            principal,
            self_ty: ArgsKey::EMPTY,
            vtable_entry_index: 0,
            method_def_path: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 91,
            },
        },
    );
    let method = model.instances.push(Instance {
        item: method_item,
        generic_args: vec![GenericArg::Type(receiver)],
        emission_name: model.items[method_item].def_path.rendered,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body: None,
            flavor: FunctionInstanceFlavor::Virtual,
            virtual_idx: Some(0),
        }),
    });

    let drop_item = test_item(
        model,
        "drop_glue",
        SyntheticItemKind::DropGlue {
            self_ty: ArgsKey::EMPTY,
        },
    );
    let drop = model.instances.push(Instance {
        item: drop_item,
        generic_args: Vec::new(),
        emission_name: model.items[drop_item].def_path.rendered,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body: None,
            flavor: FunctionInstanceFlavor::DropGlue,
            virtual_idx: None,
        }),
    });

    let mut locals = IndexVec::new();
    locals.push(test_local(unit));
    locals.push(test_local(receiver));
    let mut blocks = IndexVec::new();
    blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: CallCallee::Direct(method),
                args: Box::new([Operand::Copy(Place::from(LocalId::from_u32(1)))]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: Some(regseq_model::BlockId::from_u32(1)),
            },
            span: Span::dummy(),
        },
    });
    blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: CallCallee::Direct(drop),
                args: Box::new([Operand::Copy(Place::from(LocalId::from_u32(1)))]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: None,
            },
            span: Span::dummy(),
        },
    });

    regseq_model::Body {
        basic_blocks: blocks,
        local_decls: locals,
        arg_count: 1,
        span: Span::dummy(),
    }
}

#[test]
fn real_mir_drop_terminator_exposes_drop_as_call_prelude_and_validates() {
    // Audit-finding regseq-driver-43 test plan: a real-MIR fixture
    // containing a `Drop` terminator passes through
    // `run_phase5_body_passes`; the Drop-as-Call prelude is
    // observable and joint validation passes.
    let source = r#"
pub struct NeedsDrop(pub u8);
impl Drop for NeedsDrop { fn drop(&mut self) {} }
pub fn with_drop() { let _d = NeedsDrop(7); }
"#;
    with_bridge("bridge_body_passes_real_drop", source, |tcx, cx| {
        let instance = push_function_instance(tcx, cx, "with_drop");

        run_phase5_body_passes(cx).unwrap();

        let function = cx.model.instances[instance]
            .kind
            .as_function()
            .expect("with_drop is a function instance");
        let body = function
            .body
            .as_ref()
            .expect("Phase 5 should attach the materialised body");
        // The Drop-as-Call prelude allocates `_t_drop_arg` and
        // `_t_drop_ret` via push_synthetic_local; the originating
        // function's body therefore has strictly more locals than
        // the source MIR plus contains at least one
        // `Rvalue::RawPtr(Mutability::Mut, ...)` statement preceding
        // a Call whose first arg is `Operand::Move(...)`. Per spec
        // section 25.7.1 step 3.
        let has_raw_ptr = body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|stmt| {
                matches!(
                    stmt.kind,
                    StatementKind::Assign(_, regseq_model::Rvalue::RawPtr(Mutability::Mut, _))
                )
            })
        });
        assert!(
            has_raw_ptr,
            "expected the Drop-as-Call prelude `Assign(RawPtr(Mut, ...))` statement"
        );
        let drop_call_arg = body.basic_blocks.iter().find_map(|block| {
            if let TerminatorKind::Call { args, .. } = &block.terminator.kind {
                if !args.is_empty() {
                    return Some(args[0].clone());
                }
            }
            None
        });
        assert!(
            matches!(drop_call_arg, Some(Operand::Move(_))),
            "expected the Drop-as-Call args[0] to be Operand::Move(...), got {drop_call_arg:?}"
        );
        validate_body_places(&cx.model, body).expect("joint validation passes");
    });
}

#[test]
fn real_mir_dyn_trait_call_is_rewritten_to_indirect_and_validates() {
    // Audit-finding regseq-driver-43 test plan: a real-MIR fixture
    // containing a `dyn Trait` method call passes through
    // `run_phase5_body_passes`; the pre-rewrite Direct callee has
    // `FunctionInstanceFlavor::Virtual` with `virtual_idx: Some(_)`,
    // the post-rewrite Call has `CallCallee::Indirect`, the rewrite
    // allocated 4-5 synthetic locals, and joint validation passes.
    //
    // `caller` is `#[no_mangle] pub extern "C"` so the bridge's
    // Phase-1 mono-item discovery roots it; its `&x as &dyn Reader`
    // unsize cast triggers Phase-2 vtable synthesis for
    // `dyn Reader / u32`.
    let source = r#"
pub trait Reader { fn read(&self) -> u8; }
impl Reader for u32 { fn read(&self) -> u8 { 0 } }
pub fn dispatch(r: &dyn Reader) -> u8 { r.read() }
#[no_mangle]
pub extern "C" fn caller() -> u8 { let x: u32 = 1; dispatch(&x) }
"#;
    with_bridge("bridge_body_passes_real_dyn", source, |tcx, _cx| {
        let dispatch_def_id = find_fn_def_id(tcx, "dispatch").to_def_id();
        let dispatch_def_path_hash = tcx.def_path_hash(dispatch_def_id);
        let input = regseq_driver::bridge::BridgeInput::new(tcx);
        let model = regseq_driver::bridge::run_bridge(input).expect("bridge runs end-to-end");

        // Locate the `dispatch` Function instance by matching its
        // rustc DefPathHash.
        let dispatch_instance = model
            .instances
            .iter()
            .enumerate()
            .find_map(|(idx, inst)| {
                if !matches!(inst.kind, InstanceKind::Function(_)) {
                    return None;
                }
                let hash = model.item_def_paths[inst.item];
                if hash.stable_crate_id.0 == dispatch_def_path_hash.stable_crate_id().as_u64()
                    && hash.local_hash == dispatch_def_path_hash.local_hash().as_u64()
                {
                    Some(regseq_model::InstanceId::from_u32(idx as u32))
                } else {
                    None
                }
            })
            .expect("dispatch Function instance must exist post-bridge");
        let function = model.instances[dispatch_instance]
            .kind
            .as_function()
            .expect("dispatch is a function instance");
        let body = function
            .body
            .as_ref()
            .expect("Phase 5 should attach the materialised body");

        // Post-rewrite Indirect callee per spec section 25.7.2 step 6.
        let indirect_call = body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.kind,
                TerminatorKind::Call {
                    callee: CallCallee::Indirect(_),
                    ..
                }
            )
        });
        assert!(
            indirect_call,
            "expected post-rewrite Indirect callee on dyn Trait dispatch"
        );

        // Pre-rewrite Direct Virtual audit: the bridge created a
        // Function Instance with `flavor: Virtual` + `virtual_idx:
        // Some(slot)` whose owning Item carries the trait method
        // `Reader::read`'s rustc DefPathHash. That Instance was the
        // Direct callee consumed by the vtable-indirect rewrite per
        // spec section 25.7.2. It survives in the model (with
        // `body=None` per spec section 25.7.2) so the shape is
        // observable post-rewrite. Finding it keyed by the trait
        // method's DefPathHash ties the assertion to the specific
        // call site that got rewritten, not just "some" Virtual
        // instance.
        let read_def_id = tcx
            .associated_items(
                tcx.hir_module_items(LocalModDefId::CRATE_DEF_ID)
                    .free_items()
                    .find(|item_id| {
                        let item = tcx.hir_item(*item_id);
                        matches!(&item.kind, rustc_hir::ItemKind::Trait(..))
                    })
                    .expect("Reader trait def-id")
                    .owner_id
                    .def_id
                    .to_def_id(),
            )
            .in_definition_order()
            .next()
            .expect("trait method")
            .def_id;
        let read_hash = tcx.def_path_hash(read_def_id);
        let pre_rewrite_virtual = model.instances.iter().any(|inst| {
            let function = match inst.kind.as_function() {
                Some(f) => f,
                None => return false,
            };
            let item_hash = model.item_def_paths[inst.item];
            function.flavor == FunctionInstanceFlavor::Virtual
                && function.virtual_idx.is_some()
                && item_hash.stable_crate_id.0 == read_hash.stable_crate_id().as_u64()
                && item_hash.local_hash == read_hash.local_hash().as_u64()
        });
        assert!(
            pre_rewrite_virtual,
            "expected a Virtual {{ virtual_idx: Some(_) }} Function Instance keyed by Reader::read's DefPathHash"
        );

        // Spec section 25.7.2 allocates 4-5 synthetic locals per
        // rewrite site (raw-pointer receiver: 4; reference
        // receiver: 5).
        let rustc_body = tcx.optimized_mir(dispatch_def_id);
        let grew_by = body.local_decls.len() - rustc_body.local_decls.len();
        assert!(
            (4..=5).contains(&grew_by),
            "expected the vtable rewrite to allocate 4-5 synthetic locals, got {grew_by}"
        );

        validate_body_places(&model, body).expect("joint validation passes");
    });
}

#[test]
fn trait_object_dispatch_rewrites_before_drop_call_and_joint_validation() {
    let source = r#"
pub trait Reader { fn read(&self) -> u8; }
impl Reader for u32 { fn read(&self) -> u8 { 0 } }
pub struct NeedsDrop(pub u8);
impl Drop for NeedsDrop { fn drop(&mut self) {} }
pub fn f(r: &dyn Reader) -> u8 {
    let _d = NeedsDrop(1);
    r.read()
}
"#;
    with_bridge("bridge_body_passes_trait_dispatch", source, |tcx, cx| {
        let _fixture = tcx.optimized_mir(find_fn_def_id(tcx, "f").to_def_id());
        let mut body = populate_trait_object_dispatch_model(&mut cx.model);

        rewrite_vtable_indirect(cx, &mut body).unwrap();

        assert!(matches!(
            body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .terminator
                .kind,
            TerminatorKind::Call {
                callee: CallCallee::Indirect(_),
                ..
            }
        ));
        assert!(matches!(
            body.basic_blocks[regseq_model::BlockId::from_u32(1)]
                .terminator
                .kind,
            TerminatorKind::Call {
                callee: CallCallee::Direct(_),
                ..
            }
        ));
        validate_body_places(&cx.model, &body).unwrap();
    });
}
