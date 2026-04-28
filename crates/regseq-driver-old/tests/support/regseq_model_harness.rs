#![allow(dead_code)]

use std::sync::{Arc, Mutex, Once};

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_driver_old::construction::{
    build_body, build_declaration_skeleton, build_pass1_output, ConstantIndex, ConstructionError,
    DerefCollapseLookup, ExtractionRegistries, FunctionIndex, IntrinsicLookup, LogArgUnwrap,
    OrderedMarkers, Pass1Output, PinStateRegistry, TryMarkers, TypeDefIndex,
};
use regseq_driver_old::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver_old::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver_old::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model_old::identity::{FunctionId, Symbol};
use regseq_model_old::ir::{Body, Operand, Rvalue, StatementKind, TerminatorKind};
use regseq_model_old::model::{
    Constant, DriverStruct, Function, ImplBlock, Module, ModuleKind, RegisterBlock,
    RegisterDescriptor, RegseqModel,
};

pub type ConstructionOutcome = Result<RegseqModel, ConstructionError>;

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
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn compile_and_build(source: &str, test_name: &str) -> ConstructionOutcome {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();
    let result: Arc<Mutex<Option<ConstructionOutcome>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<ConstructionOutcome>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let outcome = build_declaration_skeleton(tcx, &registries);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();
    outcome
}

pub fn with_pass1<R, F>(source: &str, test_name: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &Pass1Output) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();
    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    type Callback<R> = Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>;
    let callback_cell: Arc<Mutex<Option<Callback<R>>>> = Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: Arc<Mutex<Option<Callback<R>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let pass1 =
                build_pass1_output(tcx, &registries).expect("Pass 1 must succeed on the specimen");
            let f = self
                .callback
                .lock()
                .unwrap()
                .take()
                .expect("callback taken once");
            *self.out.lock().unwrap() = Some(f(tcx, &pass1));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        out: out.clone(),
        callback: callback_cell,
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();
    result
}

pub fn build_model_with_method_bodies(
    source: &str,
    test_name: &str,
    targets: &[(&str, &str)],
) -> Result<RegseqModel, ConstructionError> {
    let owned_targets: Vec<(String, String)> = targets
        .iter()
        .map(|(self_ty, method)| ((*self_ty).to_string(), (*method).to_string()))
        .collect();

    with_pass1(source, test_name, move |tcx, pass1| {
        let mut model = pass1.model.clone();
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constants = ConstantIndex::new(&pass1.cross_refs.constants);
        let functions = FunctionIndex::new(&pass1.cross_refs.functions);
        let abstraction_registry = AbstractionTypeRegistry::default();
        let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
            .expect("Deref lang item must resolve on the specimen");
        let ordered_markers = OrderedMarkers::resolve(tcx)?;
        let try_markers = TryMarkers::resolve(tcx);
        let log_arg = LogArgUnwrap::resolve(tcx);

        for (self_ty, method) in &owned_targets {
            let def_id = find_method_def_id(tcx, self_ty, method);
            let function_id =
                *pass1.cross_refs.functions.get(&def_id).unwrap_or_else(|| {
                    panic!("Pass 1 missing FunctionId for `{self_ty}::{method}`")
                });

            let body = build_body(
                tcx,
                def_id,
                function_id,
                &type_def_index,
                &constants,
                &functions,
                &IntrinsicLookup::default(),
                &deref_lookup,
                &ordered_markers,
                &try_markers,
                &log_arg,
                &pass1.cross_refs.register_index,
                &pass1.cross_refs.register_blocks,
                &abstraction_registry,
                &pass1.cross_refs.pin_state_lookup,
                &mut model.symbols,
            )?;

            find_function_mut(&mut model, function_id).body = body;
        }

        Ok(model)
    })
}

pub fn find_driver_module(model: &RegseqModel) -> &Module {
    fn walk(m: &Module) -> Option<&Module> {
        if m.kind == ModuleKind::Driver {
            return Some(m);
        }
        for sub in &m.submodules {
            if let Some(found) = walk(sub) {
                return Some(found);
            }
        }
        None
    }

    walk(&model.root_module).expect("model must contain one driver module")
}

pub fn driver_struct(model: &RegseqModel) -> &DriverStruct {
    find_driver_module(model)
        .driver
        .as_ref()
        .expect("driver module must carry a DriverStruct")
}

pub fn find_driver_impl_block(model: &RegseqModel) -> &ImplBlock {
    find_driver_module(model)
        .driver_impl_blocks
        .first()
        .expect("driver module must contain a driver impl block")
}

pub fn find_driver_impl_method<'a>(model: &'a RegseqModel, name: &str) -> &'a Function {
    find_driver_impl_block(model)
        .methods
        .iter()
        .find(|f| resolve_symbol(model, f.name) == name)
        .unwrap_or_else(|| panic!("driver impl method `{name}` not found"))
}

pub fn find_trait_impl<'a>(model: &'a RegseqModel, trait_name: &str) -> &'a ImplBlock {
    find_driver_module(model)
        .trait_impls
        .iter()
        .find(|block| {
            block
                .trait_name
                .map(|sym| resolve_symbol(model, sym) == trait_name)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("trait impl `{trait_name}` not found"))
}

pub fn find_trait_method<'a>(
    model: &'a RegseqModel,
    trait_name: &str,
    method_name: &str,
) -> &'a Function {
    find_trait_impl(model, trait_name)
        .methods
        .iter()
        .find(|f| resolve_symbol(model, f.name) == method_name)
        .unwrap_or_else(|| panic!("trait method `{trait_name}::{method_name}` not found"))
}

pub fn find_driver_constant<'a>(model: &'a RegseqModel, name: &str) -> &'a Constant {
    find_driver_impl_block(model)
        .associated_constants
        .iter()
        .find(|c| resolve_symbol(model, c.name) == name)
        .unwrap_or_else(|| panic!("driver constant `{name}` not found"))
}

pub fn find_register_block<'a>(model: &'a RegseqModel, name: &str) -> &'a RegisterBlock {
    fn walk<'a>(m: &'a Module, model: &'a RegseqModel, name: &str) -> Option<&'a RegisterBlock> {
        for block in &m.register_blocks {
            if resolve_symbol(model, block.name) == name {
                return Some(block);
            }
        }
        for sub in &m.submodules {
            if let Some(found) = walk(sub, model, name) {
                return Some(found);
            }
        }
        None
    }

    walk(&model.root_module, model, name)
        .unwrap_or_else(|| panic!("register block `{name}` not found"))
}

pub fn find_register_descriptor<'a>(model: &'a RegseqModel, name: &str) -> &'a RegisterDescriptor {
    fn walk<'a>(
        m: &'a Module,
        model: &'a RegseqModel,
        name: &str,
    ) -> Option<&'a RegisterDescriptor> {
        for desc in &m.register_descriptors {
            if resolve_symbol(model, desc.name) == name {
                return Some(desc);
            }
        }
        for sub in &m.submodules {
            if let Some(found) = walk(sub, model, name) {
                return Some(found);
            }
        }
        None
    }

    walk(&model.root_module, model, name)
        .unwrap_or_else(|| panic!("register descriptor `{name}` not found"))
}

pub fn resolve_symbol(model: &RegseqModel, symbol: Symbol) -> &str {
    model.symbols.resolve(symbol)
}

pub fn collect_call_names(model: &RegseqModel, body: &Body) -> Vec<String> {
    let mut out = Vec::new();
    for block in &body.basic_blocks {
        if let TerminatorKind::Call { callee, .. } = block.terminator.kind {
            out.push(resolve_symbol(model, find_function(model, callee).name).to_string());
        }
    }
    out
}

pub fn has_call_named(model: &RegseqModel, body: &Body, name: &str) -> bool {
    body.basic_blocks.iter().any(|block| {
        matches!(
            block.terminator.kind,
            TerminatorKind::Call { callee, .. }
                if resolve_symbol(model, find_function(model, callee).name) == name
        )
    })
}

pub fn has_call_named_with_error_target(model: &RegseqModel, body: &Body, name: &str) -> bool {
    body.basic_blocks.iter().any(|block| {
        matches!(
            block.terminator.kind,
            TerminatorKind::Call {
                callee,
                error_target: Some(_),
                ..
            } if resolve_symbol(model, find_function(model, callee).name) == name
        )
    })
}

pub fn count_calls_named(model: &RegseqModel, body: &Body, name: &str) -> usize {
    body.basic_blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator.kind,
                TerminatorKind::Call { callee, .. }
                    if resolve_symbol(model, find_function(model, callee).name) == name
            )
        })
        .count()
}

pub fn count_switch_int(body: &Body) -> usize {
    body.basic_blocks
        .iter()
        .filter(|block| matches!(block.terminator.kind, TerminatorKind::SwitchInt { .. }))
        .count()
}

pub fn body_mentions_constant(
    body: &Body,
    constant_id: regseq_model_old::identity::ConstantId,
) -> bool {
    fn operand_has_constant(op: &Operand, constant_id: regseq_model_old::identity::ConstantId) -> bool {
        matches!(op, Operand::Constant(id) if *id == constant_id)
    }

    for block in &body.basic_blocks {
        for stmt in &block.statements {
            match &stmt.kind {
                StatementKind::Assign(_, rvalue) => match rvalue {
                    Rvalue::Use(op)
                    | Rvalue::Cast(_, op, _)
                    | Rvalue::UnaryOp(_, op)
                    | Rvalue::Repeat(op, _) => {
                        if operand_has_constant(op, constant_id) {
                            return true;
                        }
                    }
                    Rvalue::BinaryOp(_, lhs, rhs) => {
                        if operand_has_constant(lhs, constant_id)
                            || operand_has_constant(rhs, constant_id)
                        {
                            return true;
                        }
                    }
                    Rvalue::Aggregate(_, ops) => {
                        if ops.iter().any(|op| operand_has_constant(op, constant_id)) {
                            return true;
                        }
                    }
                    Rvalue::Ref(_, _) | Rvalue::Discriminant(_) => {}
                },
                StatementKind::MirIntrinsic(_) => {}
                StatementKind::SetDiscriminant { .. } => {}
                StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => {}
            }
        }

        match &block.terminator.kind {
            TerminatorKind::Call { args, .. } | TerminatorKind::IntrinsicCall { args, .. } => {
                if args.iter().any(|op| operand_has_constant(op, constant_id)) {
                    return true;
                }
            }
            TerminatorKind::SwitchInt { discriminant, .. } => {
                if operand_has_constant(discriminant, constant_id) {
                    return true;
                }
            }
            TerminatorKind::Goto(_)
            | TerminatorKind::Drop { .. }
            | TerminatorKind::Return
            | TerminatorKind::Unreachable => {}
        }
    }

    false
}

fn find_method_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let self_ty_middle = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{self_ty_middle}") != self_ty {
                continue;
            }
            for ii in impl_block.items {
                let impl_item = tcx.hir_impl_item(*ii);
                if let rustc_hir::ImplItemKind::Fn(_, _) = &impl_item.kind {
                    if impl_item.ident.name.as_str() == method {
                        return impl_item.owner_id.def_id.to_def_id();
                    }
                }
            }
        }
    }
    panic!("method `{self_ty}::{method}` not found in specimen");
}

fn find_function_mut(model: &mut RegseqModel, id: FunctionId) -> &mut Function {
    fn walk(m: &mut Module, id: FunctionId) -> Option<&mut Function> {
        for f in &mut m.free_functions {
            if f.id == id {
                return Some(f);
            }
        }
        for block in &mut m.impl_blocks {
            for f in &mut block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for block in &mut m.trait_impls {
            for f in &mut block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for block in &mut m.driver_impl_blocks {
            for f in &mut block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for sub in &mut m.submodules {
            if let Some(found) = walk(sub, id) {
                return Some(found);
            }
        }
        None
    }

    walk(&mut model.root_module, id).expect("FunctionId must exist in the model")
}

fn find_function(model: &RegseqModel, id: FunctionId) -> &Function {
    fn walk(m: &Module, id: FunctionId) -> Option<&Function> {
        for f in &m.free_functions {
            if f.id == id {
                return Some(f);
            }
        }
        for block in &m.impl_blocks {
            for f in &block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for block in &m.trait_impls {
            for f in &block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for block in &m.driver_impl_blocks {
            for f in &block.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for sub in &m.submodules {
            if let Some(found) = walk(sub, id) {
                return Some(found);
            }
        }
        None
    }

    walk(&model.root_module, id).expect("FunctionId must resolve in the model")
}
