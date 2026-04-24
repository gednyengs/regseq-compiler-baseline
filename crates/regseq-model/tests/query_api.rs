//! Unit tests for the `regseq_model::query` module.
//!
//! Each public query function gets a minimal hand-built model that
//! exercises the function's core path plus a determinism test that
//! asserts two calls return identical output.

use regseq_model::identity::{
    BasicBlockId, DeclId, FunctionId, LocalId, ModuleId, SourceOrigin, Symbol, SymbolInterner,
    TypeDefId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, IntrinsicMetadata, LocalDecl, Place,
    RegseqIntrinsic, RegseqType, Terminator, TerminatorKind,
};
use regseq_model::model::{
    CallbackFamily, DriverIdentity, DriverStruct, Function, ImplBlock, Module, ModuleKind,
    RegseqModel,
};
use regseq_model::query::{
    bus_binding, bus_identity, callback_families_present, callback_impls, completion_call_sites,
    duration_ctor_call_sites, lock_call_sites, log_call_sites, pm_gate_call_sites, poll_call_sites,
    reachable_helper_names, register_access_sites, resource_field_names, timed_delay_call_sites,
    transitively_reachable_from, wrapper_call_sites, ResourceKind,
};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------
// Minimal hand-built-model helpers
// ---------------------------------------------------------------------

fn compiler_generated_span() -> SourceOrigin {
    SourceOrigin::CompilerGenerated
}

fn empty_body() -> Body {
    Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: true,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        }],
        source_map: BodySourceMap {
            body_span: compiler_generated_span(),
            locals: vec![compiler_generated_span()],
            blocks: vec![BasicBlockSourceMap {
                statements: vec![],
                terminator: compiler_generated_span(),
            }],
        },
        ordered_regions: vec![],
    }
}

fn destination_place() -> Place {
    Place {
        local: LocalId(0),
        projection: vec![],
    }
}

/// Build a body with a single basic block whose terminator is an
/// `IntrinsicCall` of the given kind. The return block is implicit
/// (block index 1) and contains a `Return` terminator.
fn intrinsic_call_body(intrinsic: RegseqIntrinsic, metadata: Option<IntrinsicMetadata>) -> Body {
    Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: true,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![
            BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::IntrinsicCall {
                        intrinsic,
                        args: vec![],
                        metadata,
                        destination: destination_place(),
                        target: BasicBlockId(1),
                        error_target: None,
                    },
                },
            },
            BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                },
            },
        ],
        source_map: BodySourceMap {
            body_span: compiler_generated_span(),
            locals: vec![compiler_generated_span()],
            blocks: vec![
                BasicBlockSourceMap {
                    statements: vec![],
                    terminator: compiler_generated_span(),
                },
                BasicBlockSourceMap {
                    statements: vec![],
                    terminator: compiler_generated_span(),
                },
            ],
        },
        ordered_regions: vec![],
    }
}

/// Build a body whose sole basic block's terminator is a `Call` to
/// `callee`. The return block is implicit and contains `Return`.
fn call_body(callee: FunctionId) -> Body {
    Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: true,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![
            BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Call {
                        callee,
                        args: vec![],
                        destination: destination_place(),
                        target: Some(BasicBlockId(1)),
                        error_target: None,
                    },
                },
            },
            BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                },
            },
        ],
        source_map: BodySourceMap {
            body_span: compiler_generated_span(),
            locals: vec![compiler_generated_span()],
            blocks: vec![
                BasicBlockSourceMap {
                    statements: vec![],
                    terminator: compiler_generated_span(),
                },
                BasicBlockSourceMap {
                    statements: vec![],
                    terminator: compiler_generated_span(),
                },
            ],
        },
        ordered_regions: vec![],
    }
}

fn intern(interner: &mut SymbolInterner, name: &str) -> Symbol {
    interner.intern(name)
}

fn empty_module(symbols: &mut SymbolInterner, name: &str, id: u32, kind: ModuleKind) -> Module {
    let name_sym = intern(symbols, name);
    Module {
        id: ModuleId::from(DeclId::new(id)),
        name: name_sym,
        kind,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: None,
        trait_impls: vec![],
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    }
}

fn make_function(symbols: &mut SymbolInterner, id: u32, name: &str, body: Body) -> Function {
    let fn_name = intern(symbols, name);
    Function {
        id: FunctionId::from(DeclId::new(id)),
        name: fn_name,
        params: vec![],
        return_type: RegseqType::Unit,
        body,
        attrs: Default::default(),
    }
}

fn make_model_with_driver(symbols: SymbolInterner, root_submodules: Vec<Module>) -> RegseqModel {
    let mut symbols = symbols;
    let root = Module {
        id: ModuleId::from(DeclId::new(1)),
        name: intern(&mut symbols, "crate_root"),
        kind: ModuleKind::Normal,
        submodules: root_submodules,
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: None,
        trait_impls: vec![],
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    // Use the crate-private escape hatch via a full struct literal
    // so tests don't have to depend on `from_verified_components_unchecked`.
    RegseqModel {
        symbols,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    }
}

fn dummy_driver_struct(symbols: &mut SymbolInterner) -> DriverStruct {
    let drv_name = intern(symbols, "MyDriver");
    let drv_identity_name = intern(symbols, "my-driver");
    DriverStruct {
        type_id: TypeDefId::from(DeclId::new(100)),
        name: drv_name,
        identity: DriverIdentity {
            name: drv_identity_name,
            compatible: vec![],
            bus: None,
        },
        device_class: None,
        pm_info: None,
        platform_metadata: None,
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        interrupts: vec![],
        mmio: vec![],
        sync: vec![],
        properties: vec![],
        user_data: vec![],
    }
}

// ---------------------------------------------------------------------
// call_sites.rs -- one test per public helper
// ---------------------------------------------------------------------

fn model_with_single_free_fn_intrinsic(
    intrinsic: RegseqIntrinsic,
    metadata: Option<IntrinsicMetadata>,
) -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let mut driver_module = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
    driver_module.driver = Some(dummy_driver_struct(&mut symbols));
    let mut normal_module = empty_module(&mut symbols, "util", 3, ModuleKind::Normal);
    let fn_body = intrinsic_call_body(intrinsic, metadata);
    normal_module
        .free_functions
        .push(make_function(&mut symbols, 10, "do_thing", fn_body));
    make_model_with_driver(symbols, vec![driver_module, normal_module])
}

#[test]
fn wrapper_call_sites_picks_up_register_read() {
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::RegisterRead, None);
    let sites = wrapper_call_sites(&model);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].intrinsic, RegseqIntrinsic::RegisterRead);
    assert_eq!(sites[0].block_id, BasicBlockId(0));
    assert_eq!(sites[0].function_id, FunctionId::from(DeclId::new(10)));
}

#[test]
fn wrapper_call_sites_ignores_non_register_intrinsics() {
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::PollUntil, None);
    assert!(wrapper_call_sites(&model).is_empty());
}

#[test]
fn register_access_sites_covers_all_four_register_variants() {
    for intrinsic in [
        RegseqIntrinsic::RegisterRead,
        RegseqIntrinsic::RegisterReadField,
        RegseqIntrinsic::RegisterWrite,
        RegseqIntrinsic::RegisterWriteField,
    ] {
        let model = model_with_single_free_fn_intrinsic(intrinsic, None);
        assert_eq!(
            register_access_sites(&model).len(),
            1,
            "intrinsic {intrinsic:?} must register as a register-access site",
        );
    }
}

#[test]
fn poll_call_sites_match_only_poll_until() {
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::PollUntil, None);
    let sites = poll_call_sites(&model);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].intrinsic, RegseqIntrinsic::PollUntil);

    // A non-poll intrinsic must not match.
    let other = model_with_single_free_fn_intrinsic(RegseqIntrinsic::BusyWait, None);
    assert!(poll_call_sites(&other).is_empty());
}

#[test]
fn log_call_sites_cover_every_log_level() {
    for intrinsic in [
        RegseqIntrinsic::LogError,
        RegseqIntrinsic::LogWarn,
        RegseqIntrinsic::LogInfo,
        RegseqIntrinsic::LogDebug,
    ] {
        let model = model_with_single_free_fn_intrinsic(intrinsic, None);
        assert_eq!(
            log_call_sites(&model).len(),
            1,
            "log level {intrinsic:?} must register as a log site",
        );
    }
}

#[test]
fn timed_delay_call_sites_cover_busywait_and_sleep() {
    for intrinsic in [RegseqIntrinsic::BusyWait, RegseqIntrinsic::Sleep] {
        let model = model_with_single_free_fn_intrinsic(intrinsic, None);
        assert_eq!(
            timed_delay_call_sites(&model).len(),
            1,
            "delay intrinsic {intrinsic:?} must register as a timed-delay site",
        );
    }
}

#[test]
fn completion_and_lock_call_sites_return_empty_today() {
    // No matching RegseqIntrinsic variant exists yet; the helpers are
    // placeholders that will light up when those variants are added.
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::PollUntil, None);
    assert!(completion_call_sites(&model).is_empty());
    assert!(lock_call_sites(&model).is_empty());
}

#[test]
fn duration_ctor_and_pm_gate_call_sites_return_empty_today() {
    // Same rationale as completion/lock: no matching RegseqIntrinsic
    // variant exists yet. Two calls must also be byte-identical
    // (determinism).
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::BusyWait, None);
    assert!(duration_ctor_call_sites(&model).is_empty());
    assert!(pm_gate_call_sites(&model).is_empty());

    let once_dur = duration_ctor_call_sites(&model);
    let twice_dur = duration_ctor_call_sites(&model);
    assert_eq!(once_dur.len(), twice_dur.len());
    let once_pm = pm_gate_call_sites(&model);
    let twice_pm = pm_gate_call_sites(&model);
    assert_eq!(once_pm.len(), twice_pm.len());
}

#[test]
fn call_sites_are_deterministic_across_repeated_calls() {
    let model = model_with_single_free_fn_intrinsic(RegseqIntrinsic::RegisterRead, None);
    let a = wrapper_call_sites(&model);
    let b = wrapper_call_sites(&model);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.function_id, y.function_id);
        assert_eq!(x.block_id, y.block_id);
        assert_eq!(x.intrinsic, y.intrinsic);
    }
}

// ---------------------------------------------------------------------
// reachability.rs
// ---------------------------------------------------------------------

fn three_helper_probe_model() -> (RegseqModel, FunctionId, FunctionId, FunctionId) {
    // probe (callback) -> helper_a -> helper_b
    let mut symbols = SymbolInterner::new();

    let helper_b_id = FunctionId::from(DeclId::new(22));
    let helper_a_id = FunctionId::from(DeclId::new(21));
    let probe_id = FunctionId::from(DeclId::new(20));

    let helper_b = make_function(&mut symbols, 22, "helper_b", empty_body());
    let helper_a = make_function(&mut symbols, 21, "helper_a", call_body(helper_b_id));
    let probe = make_function(&mut symbols, 20, "probe", call_body(helper_a_id));

    let mut driver_module = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
    driver_module.driver = Some(dummy_driver_struct(&mut symbols));

    let trait_name = intern(&mut symbols, "Lifecycle");
    driver_module.trait_impls.push(ImplBlock {
        target_type: TypeDefId::from(DeclId::new(100)),
        trait_name: Some(trait_name),
        methods: vec![probe],
        associated_constants: vec![],
    });
    driver_module.driver_impl_blocks.push(ImplBlock {
        target_type: TypeDefId::from(DeclId::new(100)),
        trait_name: None,
        methods: vec![helper_a, helper_b],
        associated_constants: vec![],
    });

    let model = make_model_with_driver(symbols, vec![driver_module]);
    (model, probe_id, helper_a_id, helper_b_id)
}

#[test]
fn transitively_reachable_from_seeds_follows_call_edges() {
    let (model, probe_id, helper_a_id, helper_b_id) = three_helper_probe_model();
    let mut seeds = BTreeSet::new();
    seeds.insert(probe_id);

    let reachable = transitively_reachable_from(&model, &seeds);
    assert!(reachable.contains(&probe_id));
    assert!(reachable.contains(&helper_a_id));
    assert!(reachable.contains(&helper_b_id));
    assert_eq!(reachable.len(), 3);
}

#[test]
fn reachable_helper_names_excludes_the_callbacks_themselves() {
    let (model, _probe, _a, _b) = three_helper_probe_model();
    let names: BTreeSet<String> = reachable_helper_names(&model)
        .into_iter()
        .map(|s| model.symbols.resolve(s).to_string())
        .collect();
    let expected: BTreeSet<String> = ["helper_a", "helper_b"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(names, expected);
}

#[test]
fn reachable_helper_names_deterministic_across_repeated_calls() {
    let (model, _, _, _) = three_helper_probe_model();
    let once = reachable_helper_names(&model);
    let twice = reachable_helper_names(&model);
    assert_eq!(once, twice);
}

#[test]
fn transitively_reachable_from_deterministic_across_repeated_calls() {
    let (model, probe_id, _, _) = three_helper_probe_model();
    let mut seeds = BTreeSet::new();
    seeds.insert(probe_id);
    assert_eq!(
        transitively_reachable_from(&model, &seeds),
        transitively_reachable_from(&model, &seeds),
    );
}

// ---------------------------------------------------------------------
// topology.rs
// ---------------------------------------------------------------------

fn driver_with_clock_and_mmio() -> RegseqModel {
    use regseq_model::identity::{RegisterBlockId, ResourceId};
    use regseq_model::model::{ClockDecl, ClockResource, MmioDecl, MmioResource, Shape};

    let mut symbols = SymbolInterner::new();
    let mut driver = dummy_driver_struct(&mut symbols);

    let clk_name = intern(&mut symbols, "apb_clk");
    let clk_id = intern(&mut symbols, "apb");
    driver.clocks.push(ClockResource {
        id: ResourceId::from(DeclId::new(10)),
        field_name: clk_name,
        shape: Shape::Single,
        decl: ClockDecl { clock_id: clk_id },
        binding: None,
    });

    let mmio_field = intern(&mut symbols, "regs");
    let mmio_id = intern(&mut symbols, "uart_regs");
    driver.mmio.push(MmioResource {
        id: ResourceId::from(DeclId::new(11)),
        field_name: mmio_field,
        shape: Shape::Single,
        decl: MmioDecl {
            mmio_id,
            register_block: RegisterBlockId::from(DeclId::new(200)),
        },
        binding: None,
    });

    let mut driver_module = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
    driver_module.driver = Some(driver);
    make_model_with_driver(symbols, vec![driver_module])
}

#[test]
fn resource_field_names_reports_each_family_entry() {
    let model = driver_with_clock_and_mmio();
    let names = resource_field_names(&model);
    assert_eq!(names.len(), 2);
    // Output is sorted by (ResourceKind, field_name); ResourceKind's
    // enum ordering puts Mmio before Clock.
    assert_eq!(names[0], (ResourceKind::Mmio, "regs"));
    assert_eq!(names[1], (ResourceKind::Clock, "apb_clk"));
}

fn driver_with_two_callback_impls() -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let mut driver_module = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
    driver_module.driver = Some(dummy_driver_struct(&mut symbols));

    let lifecycle_trait = intern(&mut symbols, "Lifecycle");
    let serial_trait = intern(&mut symbols, "Serial");
    let drv_type = TypeDefId::from(DeclId::new(100));

    let init = make_function(&mut symbols, 30, "init", empty_body());
    let deinit = make_function(&mut symbols, 31, "deinit", empty_body());
    driver_module.trait_impls.push(ImplBlock {
        target_type: drv_type,
        trait_name: Some(lifecycle_trait),
        methods: vec![init, deinit],
        associated_constants: vec![],
    });

    let tx = make_function(&mut symbols, 40, "tx_byte", empty_body());
    driver_module.trait_impls.push(ImplBlock {
        target_type: drv_type,
        trait_name: Some(serial_trait),
        methods: vec![tx],
        associated_constants: vec![],
    });

    make_model_with_driver(symbols, vec![driver_module])
}

#[test]
fn callback_families_present_reports_only_implemented_families() {
    let model = driver_with_two_callback_impls();
    let families = callback_families_present(&model);
    let expected: BTreeSet<CallbackFamily> = [CallbackFamily::Lifecycle, CallbackFamily::Serial]
        .into_iter()
        .collect();
    assert_eq!(families, expected);
}

#[test]
fn callback_impls_reports_one_entry_per_method() {
    let model = driver_with_two_callback_impls();
    let impls = callback_impls(&model);
    assert_eq!(impls.len(), 3, "init + deinit + tx_byte = 3 methods");
    // Output must be sorted by (family, function_id).
    assert_eq!(impls[0].family, CallbackFamily::Lifecycle);
    assert_eq!(impls[0].function_id.0.index(), 30);
    assert_eq!(impls[1].family, CallbackFamily::Lifecycle);
    assert_eq!(impls[1].function_id.0.index(), 31);
    assert_eq!(impls[2].family, CallbackFamily::Serial);
    assert_eq!(impls[2].function_id.0.index(), 40);
}

#[test]
fn bus_binding_returns_none_when_platform_metadata_absent() {
    let model = driver_with_two_callback_impls();
    assert!(bus_binding(&model).is_none());
}

#[test]
fn bus_identity_returns_the_authored_bus_symbol() {
    let mut symbols = SymbolInterner::new();
    let mut driver_module = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
    let mut driver = dummy_driver_struct(&mut symbols);
    let amba_sym = intern(&mut symbols, "amba");
    driver.identity.bus = Some(amba_sym);
    driver_module.driver = Some(driver);
    let model = make_model_with_driver(symbols, vec![driver_module]);

    let bus = bus_identity(&model).expect("bus identity must be populated");
    assert_eq!(model.symbols.resolve(bus), "amba");
}

#[test]
fn bus_identity_returns_none_when_unset() {
    let model = driver_with_two_callback_impls();
    assert!(bus_identity(&model).is_none());
}

#[test]
fn topology_queries_are_deterministic() {
    let model = driver_with_two_callback_impls();
    assert_eq!(
        callback_families_present(&model),
        callback_families_present(&model),
    );

    let a = callback_impls(&model);
    let b = callback_impls(&model);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.family, y.family);
        assert_eq!(x.function_id, y.function_id);
        assert_eq!(x.driver_type, y.driver_type);
    }

    let res_model = driver_with_clock_and_mmio();
    let res_a = resource_field_names(&res_model);
    let res_b = resource_field_names(&res_model);
    assert_eq!(res_a, res_b);
}
