//! Integration test: verify Phase 2 model types are reachable through
//! the `regseq_model::model::*` public re-export surface, as required by
//! task regseq-model-40's test plan.
//!
//! This test uses only external paths (no `crate::...` imports); if
//! `model/mod.rs` stops re-exporting any of these types, compilation
//! fails here.

use std::collections::{BTreeMap, BTreeSet};

use regseq_model::identity::{DeclId, ModuleId, SymbolInterner, TypeDefId};
use regseq_model::model::{DriverIdentity, DriverStruct, Module, ModuleKind, RegseqModel};

#[test]
fn regseq_model_is_reachable_via_public_re_export_path() {
    let mut symbols = SymbolInterner::new();

    let drv_type_id = TypeDefId::from(DeclId::new(3));
    let drv_name = symbols.intern("MyDriver");
    let drv_identity_name = symbols.intern("my-driver");
    let driver_name = symbols.intern("drv");
    let root_name = symbols.intern("crate_root");

    let driver_mod = Module {
        id: ModuleId::from(DeclId::new(2)),
        name: driver_name,
        kind: ModuleKind::Driver,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: Some(DriverStruct {
            type_id: drv_type_id,
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
        }),
        trait_impls: vec![],
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    let root_module = Module {
        id: ModuleId::from(DeclId::new(1)),
        name: root_name,
        kind: ModuleKind::Normal,
        submodules: vec![driver_mod],
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

    let model = RegseqModel {
        symbols,
        source_locations: BTreeMap::new(),
        root_module,
        format_version: 1,
    };

    assert_eq!(model.root_module.kind, ModuleKind::Normal);
    assert_eq!(model.root_module.submodules[0].kind, ModuleKind::Driver);
    assert!(model.root_module.submodules[0].driver.is_some());
    assert_eq!(model.format_version, 1);
}
