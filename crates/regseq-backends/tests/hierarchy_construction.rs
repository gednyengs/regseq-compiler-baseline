//! Tests for item hierarchy construction from enriched extraction.
//!
//! Verifies that `build_item_hierarchy` produces correct, deterministic
//! Layer 2 hierarchies from Phase 0 enrichment data.

use std::collections::BTreeMap;

use regseq_backends::lowering::hierarchy::{
    build_item_hierarchy, CallbackTraitPaths, HierarchyBuildError,
};
use regseq_model::declaration::raw_input::{
    RawAssociatedConstantFactInput, RawEnumFactInput, RawFieldFactInput,
    RawIdentityEnrichmentInput, RawImplBlockFactInput, RawImplMethodFactInput, RawModuleFactInput,
    RawRseqFieldKindInput, RawStructFactInput,
};
use regseq_model::item::attributes::{FieldAttribute, ItemAttribute};
use regseq_model::item::constant::ConstValue;
use regseq_model::item::function::{
    DeviceCallbackRole, DeviceClassKind, FunctionKind, FunctionRole, LifecycleRole,
};
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::type_ref::{PrimitiveType, TypeRef};
use regseq_model::item::types::Item;
use regseq_model::semantic::SemanticType;

// ---------------------------------------------------------------------------
// Shared fixture builders
// ---------------------------------------------------------------------------

fn empty_enrichment() -> RawIdentityEnrichmentInput {
    RawIdentityEnrichmentInput {
        modules: vec![],
        structs: vec![],
        enums: vec![],
        impl_blocks: vec![],
        free_functions: vec![],
    }
}

fn empty_trait_paths() -> CallbackTraitPaths {
    CallbackTraitPaths {
        lifecycle: "regseq::Lifecycle".into(),
        serial: "regseq::Serial".into(),
    }
}

fn pl011_trait_paths() -> CallbackTraitPaths {
    CallbackTraitPaths {
        lifecycle: "regseq::Lifecycle".into(),
        serial: "regseq::Serial".into(),
    }
}

fn pl011_enrichment() -> RawIdentityEnrichmentInput {
    RawIdentityEnrichmentInput {
        modules: vec![
            RawModuleFactInput {
                canonical_path: "pl011_full_spec".into(),
                name: "pl011_full_spec".into(),
                child_items: vec![
                    "pl011_full_spec::driver".into(),
                    "pl011_full_spec::registers".into(),
                ],
            },
            RawModuleFactInput {
                canonical_path: "pl011_full_spec::driver".into(),
                name: "driver".into(),
                child_items: vec![
                    "pl011_full_spec::driver::Pl011FullSpec".into(),
                    "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    "pl011_full_spec::driver::<impl Lifecycle for Pl011FullSpec>".into(),
                    "pl011_full_spec::driver::<impl Serial for Pl011FullSpec>".into(),
                ],
            },
            RawModuleFactInput {
                canonical_path: "pl011_full_spec::registers".into(),
                name: "registers".into(),
                child_items: vec!["pl011_full_spec::registers::Pl011Registers".into()],
            },
        ],
        structs: vec![
            RawStructFactInput {
                canonical_path: "pl011_full_spec::driver::Pl011FullSpec".into(),
                name: "Pl011FullSpec".into(),
                module_path: "pl011_full_spec::driver".into(),
                is_driver: true,
                is_register_block: false,
                fields: vec![
                    RawFieldFactInput {
                        name: "regs".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::regs".into(),
                        rseq_kind: Some(RawRseqFieldKindInput::Mmio {
                            id: "uart_regs".into(),
                        }),
                    },
                    RawFieldFactInput {
                        name: "current_speed".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::current_speed"
                            .into(),
                        rseq_kind: Some(RawRseqFieldKindInput::Property {
                            id: "current-speed".into(),
                        }),
                    },
                    RawFieldFactInput {
                        name: "irq".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::irq".into(),
                        rseq_kind: Some(RawRseqFieldKindInput::Interrupt {
                            id: "uart_irq".into(),
                        }),
                    },
                    RawFieldFactInput {
                        name: "clock".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::clock".into(),
                        rseq_kind: Some(RawRseqFieldKindInput::Clock {
                            id: "uart_clk".into(),
                        }),
                    },
                    RawFieldFactInput {
                        name: "pinctrl".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::pinctrl".into(),
                        rseq_kind: Some(RawRseqFieldKindInput::PinCtrl {
                            id: "uart_pins".into(),
                        }),
                    },
                    RawFieldFactInput {
                        name: "pending_tx".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::pending_tx".into(),
                        rseq_kind: None,
                    },
                ],
            },
            RawStructFactInput {
                canonical_path: "pl011_full_spec::registers::Pl011Registers".into(),
                name: "Pl011Registers".into(),
                module_path: "pl011_full_spec::registers".into(),
                is_driver: false,
                is_register_block: true,
                fields: vec![
                    RawFieldFactInput {
                        name: "dr".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::dr".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "cr".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::cr".into(),
                        rseq_kind: None,
                    },
                ],
            },
        ],
        enums: vec![],
        impl_blocks: vec![
            // Inherent impl on Pl011FullSpec
            RawImplBlockFactInput {
                canonical_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                self_type_path: "pl011_full_spec::driver::Pl011FullSpec".into(),
                self_type_name: "Pl011FullSpec".into(),
                trait_path: None,
                trait_name: None,
                is_inherent: true,
                methods: vec![
                    RawImplMethodFactInput {
                        name: "uart_clock_hz".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::uart_clock_hz"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "set_baud_rate".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::set_baud_rate"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "service_rx_fifo".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::service_rx_fifo"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "service_tx_fifo".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::service_tx_fifo"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "handle_irq".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::handle_irq".into(),
                        is_isr: true,
                    },
                ],
                constants: vec![
                    RawAssociatedConstantFactInput {
                        name: "ALL_UART_INTERRUPTS_MASK".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"
                                .into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                    RawAssociatedConstantFactInput {
                        name: "RX_INTERRUPT_MASK".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK"
                            .into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                    RawAssociatedConstantFactInput {
                        name: "TX_INTERRUPT_MASK".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK"
                            .into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                    RawAssociatedConstantFactInput {
                        name: "ERROR_INTERRUPT_MASK".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK".into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                    RawAssociatedConstantFactInput {
                        name: "EXPLICIT_ISR_CLEAR_MASK".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK".into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                    RawAssociatedConstantFactInput {
                        name: "DEFAULT_UART_CLOCK_HZ".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ".into(),
                        ty: "u32".into(),
                        owner_impl_path: "pl011_full_spec::driver::<impl Pl011FullSpec>".into(),
                    },
                ],
            },
            // impl Lifecycle for Pl011FullSpec
            RawImplBlockFactInput {
                canonical_path: "pl011_full_spec::driver::<impl Lifecycle for Pl011FullSpec>"
                    .into(),
                self_type_path: "pl011_full_spec::driver::Pl011FullSpec".into(),
                self_type_name: "Pl011FullSpec".into(),
                trait_path: Some("regseq::Lifecycle".into()),
                trait_name: Some("Lifecycle".into()),
                is_inherent: false,
                methods: vec![
                    RawImplMethodFactInput {
                        name: "init".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::init".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "deinit".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::deinit".into(),
                        is_isr: false,
                    },
                ],
                constants: vec![],
            },
            // impl Serial for Pl011FullSpec
            RawImplBlockFactInput {
                canonical_path: "pl011_full_spec::driver::<impl Serial for Pl011FullSpec>".into(),
                self_type_path: "pl011_full_spec::driver::Pl011FullSpec".into(),
                self_type_name: "Pl011FullSpec".into(),
                trait_path: Some("regseq::Serial".into()),
                trait_name: Some("Serial".into()),
                is_inherent: false,
                methods: vec![
                    RawImplMethodFactInput {
                        name: "port_open".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::port_open".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "port_close".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::port_close".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "configure".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::configure".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "tx_byte".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::tx_byte".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "rx_char".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::rx_char".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "tx_ready".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::tx_ready".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "tx_empty".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::tx_empty".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "flush_rx_fifo".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::flush_rx_fifo"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "flush_tx_fifo".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::flush_tx_fifo"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "send_break".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::send_break".into(),
                        is_isr: false,
                    },
                ],
                constants: vec![],
            },
        ],
        free_functions: vec![],
    }
}

fn pl011_constant_values() -> Vec<(CanonicalPath, ConstValue)> {
    vec![
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"),
            ConstValue::Integer(0x07ff),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK"),
            ConstValue::Integer(0x50),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK"),
            ConstValue::Integer(0x20),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK"),
            ConstValue::Integer(0x780),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK"),
            ConstValue::Integer(0x780),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"),
            ConstValue::Integer(48_000_000),
        ),
    ]
}

fn pl011_interrupt_classes() -> Vec<InterruptClassDef> {
    vec![
        InterruptClassDef {
            name: "rx".into(),
            mask_value: 0x50,
        },
        InterruptClassDef {
            name: "tx".into(),
            mask_value: 0x20,
        },
        InterruptClassDef {
            name: "error".into(),
            mask_value: 0x780,
        },
        InterruptClassDef {
            name: "all".into(),
            mask_value: 0x07f0,
        },
    ]
}

fn pl011_isr_bindings() -> BTreeMap<CanonicalPath, String> {
    let mut map = BTreeMap::new();
    map.insert(
        CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::handle_irq"),
        "uart_irq".into(),
    );
    map
}

// ---------------------------------------------------------------------------
// A. Struct/Enum construction
// ---------------------------------------------------------------------------

#[test]
fn test_struct_populates_store_and_cross_link() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::MyStruct".into(),
        name: "MyStruct".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![
            RawFieldFactInput {
                name: "x".into(),
                canonical_path: "pkg::MyStruct::x".into(),
                rseq_kind: None,
            },
            RawFieldFactInput {
                name: "y".into(),
                canonical_path: "pkg::MyStruct::y".into(),
                rseq_kind: Some(RawRseqFieldKindInput::Mmio {
                    id: "test_mmio".into(),
                }),
            },
        ],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    // Struct is in the store.
    assert_eq!(h.store.len(), 1);
    let struct_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::MyStruct"))
        .unwrap();
    let item = h.store.get(struct_id).unwrap();
    match item {
        Item::Struct(s) => {
            assert_eq!(s.name, "MyStruct");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "x");
            assert!(s.fields[0].attributes.is_empty());
            assert_eq!(s.fields[1].name, "y");
            assert_eq!(
                s.fields[1].attributes,
                vec![FieldAttribute::Mmio {
                    id: "test_mmio".into()
                }]
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }

    // Field-level cross-links.
    let (owner, idx) = h
        .cross_link
        .resolve_field(&CanonicalPath::new("pkg::MyStruct::x"))
        .unwrap();
    assert_eq!(owner, struct_id);
    assert_eq!(idx, 0);

    let (owner2, idx2) = h
        .cross_link
        .resolve_field(&CanonicalPath::new("pkg::MyStruct::y"))
        .unwrap();
    assert_eq!(owner2, struct_id);
    assert_eq!(idx2, 1);
}

#[test]
fn test_driver_struct_gets_driver_attribute() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Driver".into(),
        name: "Driver".into(),
        module_path: "pkg".into(),
        is_driver: true,
        is_register_block: false,
        fields: vec![],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Driver"))
        .unwrap();
    match h.store.get(id).unwrap() {
        Item::Struct(s) => {
            assert!(s.attributes.contains(&ItemAttribute::Driver));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_register_block_gets_attribute() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Regs".into(),
        name: "Regs".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: true,
        fields: vec![],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Regs"))
        .unwrap();
    match h.store.get(id).unwrap() {
        Item::Struct(s) => {
            assert!(s.attributes.contains(&ItemAttribute::RegisterBlock));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_enum_construction() {
    let mut enrichment = empty_enrichment();
    enrichment.enums.push(RawEnumFactInput {
        canonical_path: "pkg::Status".into(),
        name: "Status".into(),
        module_path: "pkg".into(),
        variants: vec!["Ok".into(), "Error".into(), "Pending".into()],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Status"))
        .unwrap();
    match h.store.get(id).unwrap() {
        Item::Enum(e) => {
            assert_eq!(e.name, "Status");
            assert_eq!(e.variants.len(), 3);
            assert_eq!(e.variants[0].name, "Ok");
            assert_eq!(e.variants[1].name, "Error");
            assert_eq!(e.variants[2].name, "Pending");
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// B. Impl/Method construction
// ---------------------------------------------------------------------------

#[test]
fn test_inherent_impl_with_methods_and_constants() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::MyType".into(),
        name: "MyType".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl MyType>".into(),
        self_type_path: "pkg::MyType".into(),
        self_type_name: "MyType".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![RawImplMethodFactInput {
            name: "do_thing".into(),
            canonical_path: "pkg::MyType::do_thing".into(),
            is_isr: false,
        }],
        constants: vec![RawAssociatedConstantFactInput {
            name: "MAX".into(),
            canonical_path: "pkg::MyType::MAX".into(),
            ty: "u32".into(),
            owner_impl_path: "pkg::<impl MyType>".into(),
        }],
    });

    let const_values = vec![(
        CanonicalPath::new("pkg::MyType::MAX"),
        ConstValue::Integer(100),
    )];

    let h = build_item_hierarchy(
        &enrichment,
        &const_values,
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    // ImplDef exists.
    let impl_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::<impl MyType>"))
        .unwrap();
    match h.store.get(impl_id).unwrap() {
        Item::Impl(i) => {
            assert_eq!(i.methods.len(), 1);
            assert_eq!(i.constants.len(), 1);
            assert!(i.trait_ref.is_none());

            // Method is InherentMethod.
            let method_id = i.methods[0];
            match h.store.get(method_id).unwrap() {
                Item::Function(f) => {
                    assert_eq!(f.name, "do_thing");
                    assert!(matches!(f.kind, FunctionKind::InherentMethod { .. }));
                    assert_eq!(f.role, FunctionRole::Helper);
                }
                other => panic!("expected Function, got {other:?}"),
            }

            // Constant has value and owner.
            let const_id = i.constants[0];
            match h.store.get(const_id).unwrap() {
                Item::Constant(c) => {
                    assert_eq!(c.name, "MAX");
                    assert_eq!(c.value, Some(ConstValue::Integer(100)));
                    assert!(c.owner.is_some());
                    assert_eq!(
                        c.ty,
                        TypeRef::Primitive {
                            ty: PrimitiveType::U32
                        }
                    );
                }
                other => panic!("expected Constant, got {other:?}"),
            }
        }
        other => panic!("expected Impl, got {other:?}"),
    }
}

#[test]
fn test_lifecycle_trait_classification() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Drv".into(),
        name: "Drv".into(),
        module_path: "pkg".into(),
        is_driver: true,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl Lifecycle for Drv>".into(),
        self_type_path: "pkg::Drv".into(),
        self_type_name: "Drv".into(),
        trait_path: Some("regseq::Lifecycle".into()),
        trait_name: Some("Lifecycle".into()),
        is_inherent: false,
        methods: vec![
            RawImplMethodFactInput {
                name: "init".into(),
                canonical_path: "pkg::Drv::init".into(),
                is_isr: false,
            },
            RawImplMethodFactInput {
                name: "deinit".into(),
                canonical_path: "pkg::Drv::deinit".into(),
                is_isr: false,
            },
        ],
        constants: vec![],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &pl011_trait_paths(),
    )
    .unwrap();

    let init_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Drv::init"))
        .unwrap();
    match h.store.get(init_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::Lifecycle {
                    role: LifecycleRole::Init
                }
            );
            assert!(matches!(f.kind, FunctionKind::TraitMethod { .. }));
        }
        other => panic!("expected Function, got {other:?}"),
    }

    let deinit_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Drv::deinit"))
        .unwrap();
    match h.store.get(deinit_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::Lifecycle {
                    role: LifecycleRole::Deinit
                }
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_serial_trait_classification() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Drv".into(),
        name: "Drv".into(),
        module_path: "pkg".into(),
        is_driver: true,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl Serial for Drv>".into(),
        self_type_path: "pkg::Drv".into(),
        self_type_name: "Drv".into(),
        trait_path: Some("regseq::Serial".into()),
        trait_name: Some("Serial".into()),
        is_inherent: false,
        methods: vec![
            RawImplMethodFactInput {
                name: "port_open".into(),
                canonical_path: "pkg::Drv::port_open".into(),
                is_isr: false,
            },
            RawImplMethodFactInput {
                name: "tx_byte".into(),
                canonical_path: "pkg::Drv::tx_byte".into(),
                is_isr: false,
            },
            RawImplMethodFactInput {
                name: "rx_char".into(),
                canonical_path: "pkg::Drv::rx_char".into(),
                is_isr: false,
            },
        ],
        constants: vec![],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &pl011_trait_paths(),
    )
    .unwrap();

    let po_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Drv::port_open"))
        .unwrap();
    match h.store.get(po_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::PortOpen,
                }
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }

    let tx_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Drv::tx_byte"))
        .unwrap();
    match h.store.get(tx_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::TxByte,
                }
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_isr_method_classification() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Drv".into(),
        name: "Drv".into(),
        module_path: "pkg".into(),
        is_driver: true,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl Drv>".into(),
        self_type_path: "pkg::Drv".into(),
        self_type_name: "Drv".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![RawImplMethodFactInput {
            name: "handle_irq".into(),
            canonical_path: "pkg::Drv::handle_irq".into(),
            is_isr: true,
        }],
        constants: vec![],
    });

    let mut isr_bindings = BTreeMap::new();
    isr_bindings.insert(CanonicalPath::new("pkg::Drv::handle_irq"), "my_irq".into());

    let h =
        build_item_hierarchy(&enrichment, &[], &[], &isr_bindings, &empty_trait_paths()).unwrap();

    let isr_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::Drv::handle_irq"))
        .unwrap();
    match h.store.get(isr_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::Isr {
                    binding: "my_irq".into()
                }
            );
            assert!(matches!(f.kind, FunctionKind::InherentMethod { .. }));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_unresolved_self_type_is_hard_error() {
    let mut enrichment = empty_enrichment();
    // No struct -- impl references a type not in enrichment.
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl Missing>".into(),
        self_type_path: "pkg::Missing".into(),
        self_type_name: "Missing".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![],
        constants: vec![],
    });

    let result = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        HierarchyBuildError::UnresolvedSelfType {
            impl_path,
            self_type_path,
        } => {
            assert_eq!(impl_path, "pkg::<impl Missing>");
            assert_eq!(self_type_path, "pkg::Missing");
        }
        other => panic!("expected UnresolvedSelfType, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// C. Constants
// ---------------------------------------------------------------------------

#[test]
fn test_constant_with_known_value() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::T".into(),
        name: "T".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl T>".into(),
        self_type_path: "pkg::T".into(),
        self_type_name: "T".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![],
        constants: vec![RawAssociatedConstantFactInput {
            name: "VAL".into(),
            canonical_path: "pkg::T::VAL".into(),
            ty: "u32".into(),
            owner_impl_path: "pkg::<impl T>".into(),
        }],
    });

    let const_values = vec![(CanonicalPath::new("pkg::T::VAL"), ConstValue::Integer(42))];

    let h = build_item_hierarchy(
        &enrichment,
        &const_values,
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let const_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::T::VAL"))
        .unwrap();
    match h.store.get(const_id).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.value, Some(ConstValue::Integer(42)));
        }
        other => panic!("expected Constant, got {other:?}"),
    }
}

#[test]
fn test_constant_without_value() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::T".into(),
        name: "T".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl T>".into(),
        self_type_path: "pkg::T".into(),
        self_type_name: "T".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![],
        constants: vec![RawAssociatedConstantFactInput {
            name: "UNKNOWN".into(),
            canonical_path: "pkg::T::UNKNOWN".into(),
            ty: "u32".into(),
            owner_impl_path: "pkg::<impl T>".into(),
        }],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[], // no constant values provided
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let const_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::T::UNKNOWN"))
        .unwrap();
    match h.store.get(const_id).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.value, None);
        }
        other => panic!("expected Constant, got {other:?}"),
    }
}

#[test]
fn test_stable_name_derivation() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::MyDriver".into(),
        name: "MyDriver".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl MyDriver>".into(),
        self_type_path: "pkg::MyDriver".into(),
        self_type_name: "MyDriver".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![],
        constants: vec![RawAssociatedConstantFactInput {
            name: "DEFAULT_SPEED".into(),
            canonical_path: "pkg::MyDriver::DEFAULT_SPEED".into(),
            ty: "u32".into(),
            owner_impl_path: "pkg::<impl MyDriver>".into(),
        }],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let const_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::MyDriver::DEFAULT_SPEED"))
        .unwrap();
    match h.store.get(const_id).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.stable_name, "MY_DRIVER_DEFAULT_SPEED");
        }
        other => panic!("expected Constant, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// D. Cross-link/Module
// ---------------------------------------------------------------------------

#[test]
fn test_cross_link_field_level_resolution() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::S".into(),
        name: "S".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![
            RawFieldFactInput {
                name: "a".into(),
                canonical_path: "pkg::S::a".into(),
                rseq_kind: None,
            },
            RawFieldFactInput {
                name: "b".into(),
                canonical_path: "pkg::S::b".into(),
                rseq_kind: None,
            },
        ],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    let struct_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new("pkg::S"))
        .unwrap();

    let (owner_a, idx_a) = h
        .cross_link
        .resolve_field(&CanonicalPath::new("pkg::S::a"))
        .unwrap();
    assert_eq!(owner_a, struct_id);
    assert_eq!(idx_a, 0);

    let (owner_b, idx_b) = h
        .cross_link
        .resolve_field(&CanonicalPath::new("pkg::S::b"))
        .unwrap();
    assert_eq!(owner_b, struct_id);
    assert_eq!(idx_b, 1);
}

#[test]
fn test_module_resolves_child_items() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::A".into(),
        name: "A".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.enums.push(RawEnumFactInput {
        canonical_path: "pkg::B".into(),
        name: "B".into(),
        module_path: "pkg".into(),
        variants: vec!["V1".into()],
    });
    enrichment.modules.push(RawModuleFactInput {
        canonical_path: "pkg".into(),
        name: "pkg".into(),
        child_items: vec![
            "pkg::A".into(),
            "pkg::B".into(),
            "pkg::NonExistent".into(), // should be skipped
        ],
    });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    assert_eq!(h.modules.len(), 1);
    let m = &h.modules[0];
    assert_eq!(m.name, "pkg");
    // NonExistent is silently skipped.
    assert_eq!(m.items.len(), 2);
}

#[test]
fn test_interrupt_classes_propagated() {
    let enrichment = empty_enrichment();
    let classes = vec![
        InterruptClassDef {
            name: "rx".into(),
            mask_value: 0x50,
        },
        InterruptClassDef {
            name: "tx".into(),
            mask_value: 0x20,
        },
    ];

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &classes,
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    assert_eq!(h.interrupt_classes.len(), 2);
    assert_eq!(h.interrupt_classes[0].name, "rx");
    assert_eq!(h.interrupt_classes[0].mask_value, 0x50);
    assert_eq!(h.interrupt_classes[1].name, "tx");
    assert_eq!(h.interrupt_classes[1].mask_value, 0x20);
}

// ---------------------------------------------------------------------------
// E. Error cases
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_canonical_path_is_error() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Dup".into(),
        name: "Dup".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    // Second struct with the same canonical path.
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Dup".into(),
        name: "Dup".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });

    let result = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        HierarchyBuildError::DuplicateCanonicalPath { path } => {
            assert_eq!(path, "pkg::Dup");
        }
        other => panic!("expected DuplicateCanonicalPath, got {other:?}"),
    }
}

#[test]
fn test_duplicate_field_canonical_path_is_error() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::One".into(),
        name: "One".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![RawFieldFactInput {
            name: "shared".into(),
            canonical_path: "pkg::Shared::field".into(),
            rseq_kind: None,
        }],
    });
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Two".into(),
        name: "Two".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![RawFieldFactInput {
            name: "shared".into(),
            canonical_path: "pkg::Shared::field".into(),
            rseq_kind: None,
        }],
    });

    let result = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        HierarchyBuildError::DuplicateCanonicalPath { path } => {
            assert_eq!(path, "pkg::Shared::field");
        }
        other => panic!("expected DuplicateCanonicalPath, got {other:?}"),
    }
}

#[test]
fn test_constructed_functions_have_empty_semantic_bodies() {
    let mut enrichment = empty_enrichment();
    enrichment.structs.push(RawStructFactInput {
        canonical_path: "pkg::Device".into(),
        name: "Device".into(),
        module_path: "pkg".into(),
        is_driver: false,
        is_register_block: false,
        fields: vec![],
    });
    enrichment.impl_blocks.push(RawImplBlockFactInput {
        canonical_path: "pkg::<impl Device>".into(),
        self_type_path: "pkg::Device".into(),
        self_type_name: "Device".into(),
        trait_path: None,
        trait_name: None,
        is_inherent: true,
        methods: vec![RawImplMethodFactInput {
            name: "helper".into(),
            canonical_path: "pkg::Device::helper".into(),
            is_isr: false,
        }],
        constants: vec![],
    });
    enrichment
        .free_functions
        .push(regseq_model::declaration::raw_input::RawFunctionFactInput {
            canonical_path: "pkg::top_level_helper".into(),
            name: "top_level_helper".into(),
            module_path: "pkg".into(),
        });

    let h = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &empty_trait_paths(),
    )
    .unwrap();

    for path in ["pkg::Device::helper", "pkg::top_level_helper"] {
        let id = h
            .cross_link
            .resolve_item(&CanonicalPath::new(path))
            .unwrap();
        match h.store.get(id).unwrap() {
            Item::Function(f) => {
                assert!(
                    f.body.locals.is_empty(),
                    "{path} should not have locals yet"
                );
                assert!(
                    f.body.params.is_empty(),
                    "{path} should not have params yet"
                );
                assert_eq!(
                    f.body.return_type,
                    SemanticType::Void,
                    "{path} should use the empty-body void return type"
                );
                assert!(
                    f.body.body.stmts.is_empty(),
                    "{path} should not have MIR-derived statements yet"
                );
            }
            other => panic!("expected Function for {path}, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// F. PL011 comprehensive
// ---------------------------------------------------------------------------

#[test]
fn test_pl011_full_hierarchy_item_counts() {
    let h = build_item_hierarchy(
        &pl011_enrichment(),
        &pl011_constant_values(),
        &pl011_interrupt_classes(),
        &pl011_isr_bindings(),
        &pl011_trait_paths(),
    )
    .unwrap();

    // Count item types.
    let mut struct_count = 0;
    let mut enum_count = 0;
    let mut fn_count = 0;
    let mut impl_count = 0;
    let mut const_count = 0;

    for i in 0..h.store.len() {
        match h.store.get(ItemId(i as u32)).unwrap() {
            Item::Struct(_) => struct_count += 1,
            Item::Enum(_) => enum_count += 1,
            Item::Function(_) => fn_count += 1,
            Item::Impl(_) => impl_count += 1,
            Item::Constant(_) => const_count += 1,
        }
    }

    assert_eq!(struct_count, 2, "expected 2 structs");
    assert_eq!(
        enum_count, 0,
        "expected 0 enums (PL011 fixture has no standalone enums)"
    );
    assert_eq!(impl_count, 3, "expected 3 impl blocks");
    assert_eq!(const_count, 6, "expected 6 constants");
    // 5 inherent helpers/ISR + 2 lifecycle + 10 serial = 17 functions
    assert_eq!(fn_count, 17, "expected 17 functions");
    assert_eq!(h.interrupt_classes.len(), 4, "expected 4 interrupt classes");
    assert_eq!(h.modules.len(), 3, "expected 3 modules");
}

#[test]
fn test_pl011_full_hierarchy_cross_links() {
    let h = build_item_hierarchy(
        &pl011_enrichment(),
        &pl011_constant_values(),
        &pl011_interrupt_classes(),
        &pl011_isr_bindings(),
        &pl011_trait_paths(),
    )
    .unwrap();

    // All structs resolvable.
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec"
        ))
        .is_some());
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::registers::Pl011Registers"
        ))
        .is_some());

    // Driver fields resolvable at field level.
    assert!(h
        .cross_link
        .resolve_field(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::regs"
        ))
        .is_some());
    assert!(h
        .cross_link
        .resolve_field(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::irq"
        ))
        .is_some());

    // Key functions resolvable.
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::init"
        ))
        .is_some());
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::handle_irq"
        ))
        .is_some());
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::port_open"
        ))
        .is_some());

    // Constants resolvable.
    assert!(h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"
        ))
        .is_some());

    // ISR has correct role.
    let isr_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::handle_irq",
        ))
        .unwrap();
    match h.store.get(isr_id).unwrap() {
        Item::Function(f) => {
            assert_eq!(
                f.role,
                FunctionRole::Isr {
                    binding: "uart_irq".into()
                }
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }

    // Constant has value.
    let mask_id = h
        .cross_link
        .resolve_item(&CanonicalPath::new(
            "pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK",
        ))
        .unwrap();
    match h.store.get(mask_id).unwrap() {
        Item::Constant(c) => {
            assert_eq!(c.value, Some(ConstValue::Integer(0x50)));
            assert_eq!(c.stable_name, "PL011_FULL_SPEC_RX_INTERRUPT_MASK");
        }
        other => panic!("expected Constant, got {other:?}"),
    }
}

#[test]
fn test_hierarchy_serializes_to_json() {
    let h = build_item_hierarchy(
        &pl011_enrichment(),
        &pl011_constant_values(),
        &pl011_interrupt_classes(),
        &pl011_isr_bindings(),
        &pl011_trait_paths(),
    )
    .unwrap();

    // Individual components are serializable.
    let store_json = serde_json::to_string_pretty(&h.store).unwrap();
    assert!(!store_json.is_empty());

    let modules_json = serde_json::to_string_pretty(&h.modules).unwrap();
    assert!(!modules_json.is_empty());

    let cross_link_json = serde_json::to_string_pretty(&h.cross_link).unwrap();
    assert!(!cross_link_json.is_empty());

    let classes_json = serde_json::to_string_pretty(&h.interrupt_classes).unwrap();
    assert!(!classes_json.is_empty());

    // Round-trip the store.
    let store_roundtrip: regseq_model::item::identity::ItemStore =
        serde_json::from_str(&store_json).unwrap();
    assert_eq!(store_roundtrip.len(), h.store.len());
}
