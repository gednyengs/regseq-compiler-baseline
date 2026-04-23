//! Tests for the RegseqModel-native portable lowering entry
//! (`regseq-lowering-10`).
//!
//! Each test exercises [`build_driver_lowering_plan_from_model`]
//! directly, either on a hand-built minimal `RegseqModel` or on the
//! Phase-6 PL011 golden. The PL011 test is an integration smoke test
//! that also cross-checks the new entry's output against what the
//! legacy `AnalysisInput`-based entry produces on the equivalent
//! input, so divergences between the two paths (beyond the three
//! trimmed fields) are caught early.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use regseq_backends::lowering::from_model::{
    build_driver_lowering_plan_from_model, LoweringConfig, LoweringError,
};
use regseq_backends::lowering::plan::{BusType, ResourcePolicy};
use regseq_backends::lowering::planner::build_driver_lowering_plan_with_constants;
use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::declaration::driver::DeviceClass;
use regseq_model::identity::{DeclId, ModuleId, SymbolInterner, TypeDefId};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LocalDecl, RegseqType, Terminator,
    TerminatorKind,
};
use regseq_model::model::driver::{DeviceClass as ModelDeviceClass, DriverIdentity, DriverStruct};
use regseq_model::model::function::Function;
use regseq_model::model::impl_block::ImplBlock;
use regseq_model::model::module::{Module, ModuleKind};
use regseq_model::model::register::{RegisterBlock, RegisterBlockAttributes};
use regseq_model::model::regseq_model::RegseqModel;
use regseq_model::model::resources::{
    ClockDecl, ClockResource, MmioDecl, MmioResource, ResourceKind, Shape,
};
use regseq_model::model::type_def::Attrs;

// ---------------------------------------------------------------------------
// Minimal model construction helpers
// ---------------------------------------------------------------------------

/// Build a minimal [`RegseqModel`] with a driver submodule, one clock
/// resource, one MMIO resource, and a Lifecycle trait impl. The
/// returned model is **not** verifier-checked -- it is intentionally
/// skeletal so each test can focus on one plan field at a time.
fn build_minimal_model() -> RegseqModel {
    let mut symbols = SymbolInterner::new();

    let drv_name = symbols.intern("TestDriver");
    let drv_identity = symbols.intern("test-driver");
    let compat = symbols.intern("vendor,test-driver");
    let bus = symbols.intern("amba");
    let class = symbols.intern("serial_uart");

    let clk_field = symbols.intern("core_clk");
    let clk_id = symbols.intern("apb");
    let mmio_field = symbols.intern("regs");
    let mmio_id = symbols.intern("uart_regs");

    let lifecycle = symbols.intern("Lifecycle");
    let init_name = symbols.intern("init");

    let drv_type = TypeDefId::from(DeclId::new(100));

    let driver = DriverStruct {
        type_id: drv_type,
        name: drv_name,
        identity: DriverIdentity {
            name: drv_identity,
            compatible: vec![compat],
            bus: Some(bus),
        },
        device_class: Some(ModelDeviceClass { class }),
        pm_info: None,
        platform_metadata: None,
        clocks: vec![ClockResource {
            id: regseq_model::identity::ResourceId::from(DeclId::new(10)),
            field_name: clk_field,
            shape: Shape::Single,
            decl: ClockDecl { clock_id: clk_id },
            binding: None,
        }],
        resets: Vec::new(),
        pinctrl: Vec::new(),
        interrupts: Vec::new(),
        mmio: vec![MmioResource {
            id: regseq_model::identity::ResourceId::from(DeclId::new(11)),
            field_name: mmio_field,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id,
                register_block: regseq_model::identity::RegisterBlockId::from(DeclId::new(200)),
            },
            binding: None,
        }],
        sync: Vec::new(),
        properties: Vec::new(),
        user_data: Vec::new(),
    };

    let init_body = Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: true,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![BasicBlock {
            statements: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        }],
        source_map: BodySourceMap {
            body_span: regseq_model::identity::SourceOrigin::CompilerGenerated,
            locals: vec![regseq_model::identity::SourceOrigin::CompilerGenerated],
            blocks: vec![BasicBlockSourceMap {
                statements: Vec::new(),
                terminator: regseq_model::identity::SourceOrigin::CompilerGenerated,
            }],
        },
        ordered_regions: Vec::new(),
    };

    let init_method = Function {
        id: regseq_model::identity::FunctionId::from(DeclId::new(500)),
        name: init_name,
        params: Vec::new(),
        return_type: RegseqType::Unit,
        body: init_body,
        attrs: Attrs::default(),
    };

    let lifecycle_impl = ImplBlock {
        target_type: drv_type,
        trait_name: Some(lifecycle),
        methods: vec![init_method],
        associated_constants: Vec::new(),
    };

    let block_name = symbols.intern("TestRegisters");
    let endian = symbols.intern("little");
    let register_block = RegisterBlock {
        id: regseq_model::identity::RegisterBlockId::from(DeclId::new(200)),
        name: block_name,
        block_attributes: RegisterBlockAttributes {
            endian,
            access_width: 32,
        },
        fields: Vec::new(),
    };

    let driver_mod = Module {
        id: ModuleId::from(DeclId::new(2)),
        name: symbols.intern("drv"),
        kind: ModuleKind::Driver,
        submodules: Vec::new(),
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: vec![register_block],
        impl_blocks: Vec::new(),
        driver: Some(driver),
        trait_impls: vec![lifecycle_impl],
        driver_impl_blocks: Vec::new(),
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    let root = Module {
        id: ModuleId::from(DeclId::new(1)),
        name: symbols.intern("crate_root"),
        kind: ModuleKind::Normal,
        submodules: vec![driver_mod],
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: Vec::new(),
        impl_blocks: Vec::new(),
        driver: None,
        trait_impls: Vec::new(),
        driver_impl_blocks: Vec::new(),
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    RegseqModel {
        symbols,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    }
}

// ---------------------------------------------------------------------------
// Unit tests: one per plan-field family
// ---------------------------------------------------------------------------

#[test]
fn from_model_populates_driver_identity() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(plan.identity.driver_name, "test-driver");
    assert_eq!(plan.identity.compatible, vec!["vendor,test-driver"]);
    assert_eq!(plan.identity.device_class, DeviceClass::SerialUart);
}

#[test]
fn from_model_populates_callback_inventory() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(plan.callbacks.lifecycle.len(), 1);
    assert_eq!(
        plan.callbacks.lifecycle[0].family,
        CallbackFamily::Lifecycle
    );
    assert!(plan.callbacks.serial.is_empty());
}

#[test]
fn from_model_populates_resource_obligations() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(plan.resources.clocks.len(), 1);
    assert_eq!(plan.resources.clocks[0].clock_id, "apb");
    assert_eq!(plan.resources.clocks[0].policy, ResourcePolicy::Required);

    assert_eq!(plan.resources.mmio.len(), 1);
    assert_eq!(plan.resources.mmio[0].mmio_id, "uart_regs");
}

#[test]
fn from_model_populates_bus_identity() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(plan.resources.bus_identity.bus_type, BusType::Amba);
    assert_eq!(plan.resources.bus_identity.compatible.len(), 1);
}

#[test]
fn from_model_populates_acquisition_and_teardown_orderings() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    // Clocks acquired before MMIO per the portable ordering policy.
    let clock_index = plan
        .acquisition_order
        .iter()
        .position(|s| s.resource_kind == ResourceKind::Clock)
        .expect("clock in acquisition order");
    let mmio_index = plan
        .acquisition_order
        .iter()
        .position(|s| s.resource_kind == ResourceKind::Mmio)
        .expect("mmio in acquisition order");
    assert!(clock_index < mmio_index);

    // Teardown is the reverse of acquisition.
    assert_eq!(plan.teardown_order.len(), plan.acquisition_order.len());
    assert_eq!(
        plan.teardown_order.first().map(|s| s.resource_kind),
        plan.acquisition_order.last().map(|s| s.resource_kind)
    );
}

#[test]
fn from_model_populates_device_class_roles() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(
        plan.device_class_roles.device_class,
        DeviceClass::SerialUart
    );
    assert!(plan.device_class_roles.has_lifecycle);
    assert!(!plan.device_class_roles.has_power_management);
}

#[test]
fn from_model_populates_diagnostics() {
    let model = build_minimal_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    // Diagnostics are populated by the shared helper so the Linux
    // resource-sequencing report contains at least one entry (the
    // clock + MMIO from the minimal driver).
    assert!(!plan
        .diagnostics
        .linux_resource_sequencing
        .acquisition_order
        .is_empty());
    assert_eq!(plan.diagnostics.linux_deployment_metadata.clocks.len(), 1);
}

#[test]
fn from_model_populates_user_data_obligations() {
    use regseq_backends::lowering::plan::UserDataShape;
    use regseq_model::identity::UserDataId;
    use regseq_model::ir::ScalarType;
    use regseq_model::model::resources::{UserDataField, UserDataKind};

    let mut model = build_minimal_model();

    let driver_mod = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .expect("driver submodule");
    let driver = driver_mod.driver.as_mut().expect("driver struct");

    let ud_field_sym = model.symbols.intern("imsc_shadow");
    driver.user_data.push(UserDataField {
        id: UserDataId::from(DeclId::new(40)),
        field_name: ud_field_sym,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Scalar(ScalarType::U32),
        },
    });

    let ud_opt_field_sym = model.symbols.intern("maybe_counter");
    driver.user_data.push(UserDataField {
        id: UserDataId::from(DeclId::new(41)),
        field_name: ud_opt_field_sym,
        kind: UserDataKind::Option {
            inner_type: RegseqType::Scalar(ScalarType::U64),
        },
    });

    let plan = build_driver_lowering_plan_from_model(&model, &LoweringConfig::default())
        .expect("plan with user-data");

    assert_eq!(plan.resources.user_data.len(), 2);
    let scalar = plan
        .resources
        .user_data
        .iter()
        .find(|u| u.field_name == "imsc_shadow")
        .expect("imsc_shadow obligation");
    match &scalar.shape {
        UserDataShape::ZeroInit { inner_type } => {
            assert_eq!(*inner_type, RegseqType::Scalar(ScalarType::U32));
        }
        other => panic!("expected ZeroInit, got {other:?}"),
    }

    let opt = plan
        .resources
        .user_data
        .iter()
        .find(|u| u.field_name == "maybe_counter")
        .expect("maybe_counter obligation");
    match &opt.shape {
        UserDataShape::Option { inner_type } => {
            assert_eq!(*inner_type, RegseqType::Scalar(ScalarType::U64));
        }
        other => panic!("expected Option, got {other:?}"),
    }
}

#[test]
fn from_model_rejects_missing_bus_identity() {
    // A driver without a declared bus identity should fail the
    // structural check, surfacing LoweringError::InvariantViolated.
    let mut model = build_minimal_model();

    // Drop the bus identity from the driver.
    if let Some(ref mut driver_mod) = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
    {
        if let Some(ref mut driver) = driver_mod.driver {
            driver.identity.bus = None;
        }
    }

    let result = build_driver_lowering_plan_from_model(&model, &LoweringConfig::default());
    match result {
        Err(LoweringError::InvariantViolated(msg)) => {
            assert!(
                msg.contains("bus"),
                "error message should mention bus identity: {msg}"
            );
        }
        other => panic!("expected InvariantViolated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// PL011 integration smoke test
// ---------------------------------------------------------------------------

fn fixture_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../tests/fixtures/pl011-full-spec");
    p
}

fn load_pl011_model() -> RegseqModel {
    let raw_path = fixture_root().join("expected/raw-frontend.json");
    let platform_path = fixture_root().join("platform.toml");

    let raw_text = std::fs::read_to_string(&raw_path).expect("read raw-frontend.json fixture");
    let raw: regseq_model::normalize::raw_artifact::RawFrontendArtifact =
        serde_json::from_str(&raw_text).expect("deserialize RawFrontendArtifact");
    let platform = regseq_model::platform_parser::parse_platform_file_path(&platform_path)
        .expect("parse platform.toml");

    let mut model = regseq_model::normalize(&raw).expect("normalize");
    regseq_model::verify::verify_phase_1(&model).expect("phase-1 verify");
    regseq_model::populate_bindings(&mut model, &platform).expect("populate_bindings");
    regseq_model::verify::verify_phase_2(&model).expect("phase-2 verify");
    model
}

#[test]
fn pl011_from_model_matches_legacy_entry_core_shape() {
    // Loads the Phase-6 PL011 golden model and confirms the new
    // `from_model` entry agrees with the legacy `AnalysisInput`-based
    // entry on the portable-plan fields that both paths populate.
    // Fields trimmed by `regseq-lowering-10` (`behavior`,
    // `legacy_behavior`, `callback_actions`) are out of scope.
    let model = load_pl011_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert_eq!(plan.identity.driver_name, "pl011-full-spec");
    assert_eq!(plan.resources.bus_identity.bus_type, BusType::Amba);
    assert!(
        !plan.resources.mmio.is_empty(),
        "PL011 has at least one MMIO"
    );
    assert!(
        !plan.resources.interrupts.is_empty(),
        "PL011 has at least one interrupt"
    );
    assert!(!plan.acquisition_order.is_empty());
    assert!(!plan.register_obligations.blocks.is_empty());
}

#[test]
fn pl011_from_model_produces_non_empty_diagnostics() {
    let model = load_pl011_model();
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");

    assert!(!plan
        .diagnostics
        .linux_resource_sequencing
        .acquisition_order
        .is_empty());
    assert!(!plan.diagnostics.linux_deployment_metadata.clocks.is_empty());
}

// ---------------------------------------------------------------------------
// Consistency check with the legacy entry
// ---------------------------------------------------------------------------

#[test]
fn pl011_from_model_matches_legacy_entry_on_shared_fields() {
    // The new entry and the legacy entry produce the same non-behavior
    // portable obligations for the PL011 golden, where both paths read
    // equivalent data.
    let model = load_pl011_model();
    let new_plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("new");

    // Legacy entry requires AnalysisInput, not RegseqModel. Build it
    // from the same fixture via the normal declaration/binding/join
    // pipeline.
    let raw_path = fixture_root().join("expected/raw-frontend.json");
    let platform_path = fixture_root().join("platform.toml");
    let raw_text = std::fs::read_to_string(&raw_path).expect("raw");
    let raw: regseq_model::normalize::raw_artifact::RawFrontendArtifact =
        serde_json::from_str(&raw_text).expect("raw artifact");
    let json = serde_json::to_string(&raw).expect("reserialize raw");
    let decl = regseq_model::declaration::normalize_from_json(&json).expect("decl");
    let platform_file =
        regseq_model::platform_parser::parse_platform_file_path(&platform_path).expect("platform");
    let platform_source = platform_path.to_string_lossy();
    let declaration_bus = decl.driver.bus.as_ref();
    let binding =
        regseq_model::binding::normalize(&platform_file, Some(&platform_source), declaration_bus)
            .expect("binding");
    let bus_binding = binding.bus.clone();
    let joined = regseq_model::joined::perform_join(&decl, &binding).expect("join");
    #[allow(deprecated)]
    let input = regseq_analysis::legacy_input::AnalysisInput::new(decl, joined, bus_binding);
    let legacy_plan = build_driver_lowering_plan_with_constants(&input, &[]).expect("legacy");

    assert_eq!(
        new_plan.identity.driver_name,
        legacy_plan.identity.driver_name
    );
    assert_eq!(
        new_plan.resources.bus_identity.bus_type,
        legacy_plan.resources.bus_identity.bus_type
    );
    assert_eq!(
        new_plan.resources.mmio.len(),
        legacy_plan.resources.mmio.len()
    );
    assert_eq!(
        new_plan.resources.interrupts.len(),
        legacy_plan.resources.interrupts.len()
    );
    assert_eq!(
        new_plan.resources.clocks.len(),
        legacy_plan.resources.clocks.len()
    );
    assert_eq!(
        new_plan.acquisition_order.len(),
        legacy_plan.acquisition_order.len()
    );
    assert_eq!(
        new_plan.register_obligations.blocks.len(),
        legacy_plan.register_obligations.blocks.len()
    );
}

// ---------------------------------------------------------------------------
// regseq-lowering-12: amba_mask binding override (model-era path)
// ---------------------------------------------------------------------------

fn pl011_model_with_amba_mask(mask: Option<i64>) -> RegseqModel {
    use regseq_model::platform::PlatformValue;

    let mut model = load_pl011_model();
    // Walk submodules to find the driver struct and edit its
    // platform_metadata.bus.values map.
    fn edit_driver(module: &mut regseq_model::model::module::Module, mask: Option<i64>) -> bool {
        if module.kind == regseq_model::model::module::ModuleKind::Driver {
            if let Some(driver) = module.driver.as_mut() {
                if let Some(meta) = driver.platform_metadata.as_mut() {
                    if let Some(bus) = meta.bus.as_mut() {
                        match mask {
                            Some(v) => {
                                bus.values
                                    .insert("amba_mask".into(), PlatformValue::Integer(v));
                            }
                            None => {
                                bus.values.remove("amba_mask");
                            }
                        }
                        return true;
                    }
                }
            }
        }
        for sub in &mut module.submodules {
            if edit_driver(sub, mask) {
                return true;
            }
        }
        false
    }
    assert!(
        edit_driver(&mut model.root_module, mask),
        "driver not found"
    );
    model
}

#[test]
fn from_model_threads_amba_mask_override_into_bus_identity() {
    let model = pl011_model_with_amba_mask(Some(0x00ff_ffff));
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");
    assert_eq!(plan.resources.bus_identity.amba_mask, Some(0x00ff_ffff_u32));
    assert!(!plan.diagnostics.amba_mask_default_applied);
}

#[test]
fn from_model_leaves_amba_mask_none_when_absent() {
    // PL011 fixture has no amba_mask key; amba_id is present.
    let model = pl011_model_with_amba_mask(None);
    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");
    assert_eq!(plan.resources.bus_identity.amba_mask, None);
    assert!(plan.resources.bus_identity.amba_id.is_some());
    assert!(plan.diagnostics.amba_mask_default_applied);
}

#[test]
fn from_model_amba_mask_non_integer_value_falls_back_to_none() {
    use regseq_model::platform::PlatformValue;

    let mut model = load_pl011_model();
    fn edit_driver(module: &mut regseq_model::model::module::Module) -> bool {
        if module.kind == regseq_model::model::module::ModuleKind::Driver {
            if let Some(driver) = module.driver.as_mut() {
                if let Some(meta) = driver.platform_metadata.as_mut() {
                    if let Some(bus) = meta.bus.as_mut() {
                        bus.values
                            .insert("amba_mask".into(), PlatformValue::String("oops".into()));
                        return true;
                    }
                }
            }
        }
        for sub in &mut module.submodules {
            if edit_driver(sub) {
                return true;
            }
        }
        false
    }
    assert!(edit_driver(&mut model.root_module));

    let plan =
        build_driver_lowering_plan_from_model(&model, &LoweringConfig::default()).expect("plan");
    assert_eq!(plan.resources.bus_identity.amba_mask, None);
    assert!(plan.diagnostics.amba_mask_default_applied);
}
