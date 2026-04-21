//! Tests for semantic body lowering (Phase 2B, regseq-lowering-07).
//!
//! Validates that:
//! - `populate_hierarchy` preserves ItemId invariants
//! - the reference resolver resolves all representative-set refs
//! - `lower_and_populate_hierarchy` produces non-empty bodies
//! - the narrow and full representative function sets lower successfully
//!
//! Exercises the deprecated `regseq_analysis::legacy_input` shim during
//! the transitional window that ends with `regseq-lowering-10`.
#![allow(deprecated)]

use std::collections::BTreeMap;

use regseq_backends::lowering::hierarchy::{
    build_item_hierarchy, CallbackTraitPaths, ItemHierarchy,
};
use regseq_backends::lowering::semantic_lowering::populate_hierarchy;
use regseq_backends::lowering::semantic_lowering::ref_resolver::ReferenceResolver;
use regseq_backends::lowering::semantic_lowering::validation::validate_semantic_body;
use regseq_model::declaration::raw_input::{
    RawAssociatedConstantFactInput, RawFieldFactInput, RawIdentityEnrichmentInput,
    RawImplBlockFactInput, RawImplMethodFactInput, RawModuleFactInput, RawRseqFieldKindInput,
    RawStructFactInput,
};
use regseq_model::item::constant::ConstValue;
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::types::Item;
use regseq_model::semantic::body::{Block, SemanticBody};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::local::{LocalId, LocalOrigin, SemanticLocal};
use regseq_model::semantic::stmt::Stmt;
use regseq_model::semantic::SemanticType;

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

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
                        name: "fifo_size".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::fifo_size".into(),
                        rseq_kind: Some(RawRseqFieldKindInput::Property {
                            id: "fifo-size".into(),
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
                        name: "reset".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::reset".into(),
                        rseq_kind: None, // reset has no rseq attribute in the fixture
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
                    RawFieldFactInput {
                        name: "ibrd".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::ibrd".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "fbrd".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::fbrd".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "lcr_h".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::lcr_h".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "imsc".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::imsc".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "icr".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::icr".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "mis".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::mis".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "ifls".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::ifls".into(),
                        rseq_kind: None,
                    },
                    RawFieldFactInput {
                        name: "fr".into(),
                        canonical_path: "pl011_full_spec::registers::Pl011Registers::fr".into(),
                        rseq_kind: None,
                    },
                ],
            },
        ],
        enums: vec![],
        impl_blocks: vec![
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
                    RawImplMethodFactInput {
                        name: "disable_uart".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::disable_uart"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "enable_uart".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::enable_uart"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "program_polling_port".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::program_polling_port".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "enable_clock_if_present".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::enable_clock_if_present".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "toggle_reset_if_present".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::toggle_reset_if_present".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "apply_default_pins_if_present".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::apply_default_pins_if_present"
                                .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "apply_sleep_pins_if_present".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::apply_sleep_pins_if_present"
                                .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "disable_clock_if_present".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::disable_clock_if_present"
                                .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "shutdown_port".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::shutdown_port"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "enable_tx_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::enable_tx_interrupts".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "disable_tx_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::disable_tx_interrupts".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "clear_all_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::clear_all_interrupts".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "configure_interrupt_levels".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::configure_interrupt_levels"
                                .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "configure_line_control".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::configure_line_control".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "disable_fifo".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::disable_fifo"
                            .into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "enable_rx_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::enable_rx_interrupts".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "mask_all_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::mask_all_interrupts".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "clear_explicit_interrupts".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::clear_explicit_interrupts"
                                .into(),
                        is_isr: false,
                    },
                ],
                constants: vec![
                    RawAssociatedConstantFactInput {
                        name: "DEFAULT_UART_CLOCK_HZ".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ".into(),
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
                        name: "ALL_UART_INTERRUPTS_MASK".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"
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
                ],
            },
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
                        name: "tx_byte".into(),
                        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::tx_byte".into(),
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
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"),
            ConstValue::Integer(48_000_000),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK"),
            ConstValue::Integer(0x0050),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK"),
            ConstValue::Integer(0x0020),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK"),
            ConstValue::Integer(0x07ff),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK"),
            ConstValue::Integer(0x0780),
        ),
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK"),
            ConstValue::Integer(0x0780),
        ),
    ]
}

fn pl011_interrupt_classes() -> Vec<InterruptClassDef> {
    vec![
        InterruptClassDef {
            name: "rx".into(),
            mask_value: 0x0050,
        },
        InterruptClassDef {
            name: "tx".into(),
            mask_value: 0x0020,
        },
    ]
}

fn pl011_trait_paths() -> CallbackTraitPaths {
    CallbackTraitPaths {
        lifecycle: "regseq::Lifecycle".into(),
        serial: "regseq::Serial".into(),
    }
}

fn pl011_isr_bindings() -> BTreeMap<CanonicalPath, String> {
    let mut m = BTreeMap::new();
    m.insert(
        CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::handle_irq"),
        "irq".into(),
    );
    m
}

fn build_pl011_hierarchy() -> ItemHierarchy {
    build_item_hierarchy(
        &pl011_enrichment(),
        &pl011_constant_values(),
        &pl011_interrupt_classes(),
        &pl011_isr_bindings(),
        &pl011_trait_paths(),
    )
    .expect("hierarchy build should succeed")
}

// ---------------------------------------------------------------------------
// Test 1: populate_hierarchy preserves ItemId invariants
// ---------------------------------------------------------------------------

#[test]
fn populate_hierarchy_preserves_item_ids() {
    let hierarchy = build_pl011_hierarchy();
    let original_len = hierarchy.store.len();

    // Create a trivial body for one function.
    let mut lowered = BTreeMap::new();
    // Find a FunctionDef ItemId.
    let mut func_id = None;
    for i in 0..hierarchy.store.len() {
        let id = ItemId(i as u32);
        if let Some(Item::Function(_)) = hierarchy.store.get(id) {
            func_id = Some(id);
            break;
        }
    }
    let fid = func_id.expect("should have at least one function");
    lowered.insert(
        fid,
        SemanticBody {
            locals: vec![SemanticLocal {
                id: LocalId(0),
                name: "_ret".into(),
                ty: SemanticType::Void,
                origin: LocalOrigin::ReturnPlace,
            }],
            params: vec![],
            return_type: SemanticType::Void,
            body: Block {
                stmts: vec![Stmt::Return { value: None }],
            },
        },
    );

    let populated = populate_hierarchy(&hierarchy, &lowered);

    // Item count must be preserved.
    assert_eq!(populated.store.len(), original_len);

    // Every original ItemId must resolve to the same item kind.
    for i in 0..original_len {
        let id = ItemId(i as u32);
        let orig = hierarchy.store.get(id).unwrap();
        let pop = populated.store.get(id).unwrap();
        assert_eq!(
            std::mem::discriminant(orig),
            std::mem::discriminant(pop),
            "item kind changed at ItemId({i})"
        );
    }

    // The populated function should have a non-empty body.
    if let Some(Item::Function(f)) = populated.store.get(fid) {
        assert!(!f.body.body.stmts.is_empty(), "body should be populated");
    } else {
        panic!("populated item should be a Function");
    }

    // Cross-link table should be unchanged.
    assert_eq!(
        hierarchy.cross_link.item_count(),
        populated.cross_link.item_count()
    );
}

// ---------------------------------------------------------------------------
// Test 2: Reference resolver covers the representative set
// ---------------------------------------------------------------------------

#[test]
fn resolver_covers_narrow_subset_functions() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    // Narrow subset: uart_clock_hz, set_baud_rate, init
    for name in &["uart_clock_hz", "set_baud_rate", "init"] {
        let path = format!("pl011_full_spec::driver::Pl011FullSpec::{name}");
        let func = resolver.resolve_call(&path);
        assert!(func.is_ok(), "should resolve {name}: {:?}", func.err());
        assert_eq!(func.unwrap().name, *name);
    }
}

#[test]
fn resolver_covers_full_representative_set_functions() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    let representative = [
        "uart_clock_hz",
        "set_baud_rate",
        "init",
        "service_rx_fifo",
        "service_tx_fifo",
        "handle_irq",
        // Required helpers
        "disable_uart",
        "enable_uart",
        "program_polling_port",
        "enable_clock_if_present",
        "toggle_reset_if_present",
        "apply_default_pins_if_present",
        "clear_explicit_interrupts",
    ];

    for name in &representative {
        let path = format!("pl011_full_spec::driver::Pl011FullSpec::{name}");
        let func = resolver.resolve_call(&path);
        assert!(func.is_ok(), "should resolve {name}: {:?}", func.err());
    }
}

#[test]
fn resolver_covers_all_pl011_registers() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    let registers = [
        "dr", "cr", "ibrd", "fbrd", "lcr_h", "imsc", "icr", "mis", "ifls", "fr",
    ];
    for name in &registers {
        let reg = resolver.resolve_register(name);
        assert!(
            reg.is_ok(),
            "should resolve register {name}: {:?}",
            reg.err()
        );
    }
}

#[test]
fn resolver_covers_all_pl011_resources() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    let resources = ["clock", "pinctrl"];
    for name in &resources {
        let res = resolver.resolve_resource(name);
        assert!(
            res.is_ok(),
            "should resolve resource {name}: {:?}",
            res.err()
        );
    }
}

#[test]
fn resolver_covers_interrupt_classes() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    let rx = resolver.resolve_interrupt_class_by_mask(0x0050);
    assert!(rx.is_ok(), "should resolve RX class: {:?}", rx.err());
    assert_eq!(rx.unwrap().name, "rx");

    let tx = resolver.resolve_interrupt_class_by_mask(0x0020);
    assert!(tx.is_ok(), "should resolve TX class: {:?}", tx.err());
    assert_eq!(tx.unwrap().name, "tx");
}

#[test]
fn resolver_covers_constants() {
    let h = build_pl011_hierarchy();
    let resolver = ReferenceResolver::build(&h).unwrap();

    let c =
        resolver.resolve_constant("pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ");
    assert!(c.is_ok(), "should resolve constant: {:?}", c.err());
    let c = c.unwrap();
    assert_eq!(c.name, "DEFAULT_UART_CLOCK_HZ");
    assert_eq!(c.known_value, Some(48_000_000));
}

// ---------------------------------------------------------------------------
// Test 3: Bodies remain device-neutral and OS-neutral
// ---------------------------------------------------------------------------

#[test]
fn no_linux_or_zephyr_names_in_hierarchy_items() {
    let h = build_pl011_hierarchy();
    let banned = ["linux", "zephyr", "devm_", "of_", "struct device"];

    for i in 0..h.store.len() {
        let id = ItemId(i as u32);
        if let Some(Item::Function(f)) = h.store.get(id) {
            for b in &banned {
                assert!(
                    !f.name.to_lowercase().contains(b),
                    "function name '{}' contains banned term '{b}'",
                    f.name
                );
            }
        }
    }
}

// ===========================================================================
// MIR-driven integration tests
// ===========================================================================
//
// These tests call lower_and_populate_hierarchy() with a real AnalysisInput
// built from the pl011-full-spec fixture's raw-frontend.json MIR bodies.

use regseq_analysis::legacy_input::AnalysisInput;
use regseq_backends::lowering::semantic_lowering::lower_and_populate_hierarchy;
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

/// Load the full DeclarationModel from the raw-frontend.json fixture.
fn load_pl011_decl_from_fixture() -> regseq_model::declaration::DeclarationModel {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/pl011-full-spec/expected/raw-frontend.json");
    let json = std::fs::read_to_string(&fixture_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read raw-frontend.json at {}: {e}.\n\
             Generate it by running the regseq-driver on pl011-full-spec.",
            fixture_path.display()
        )
    });
    regseq_model::declaration::normalize::normalize_from_json(&json)
        .expect("Failed to normalize raw artifact")
}

/// Build a minimal BindingModel for the PL011 join.
fn pl011_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "test".into(),
            provenance: BindingProvenance::new("test"),
        },
        properties: vec![
            PropertyBinding {
                key: "current-speed".into(),
                value: PlatformValue::Integer(115200),
                provenance: BindingProvenance::new("test"),
            },
            PropertyBinding {
                key: "fifo-size".into(),
                value: PlatformValue::Integer(32),
                provenance: BindingProvenance::new("test"),
            },
        ],
        mmio: vec![MmioBinding {
            join_key: JoinKey::ExplicitId("uart_regs".into()),
            base: Some(0x0900_0000),
            size: Some(0x1000),
            provenance: BindingProvenance::new("test"),
        }],
        interrupts: vec![InterruptBinding {
            join_key: JoinKey::ExplicitId("uart_irq".into()),
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            priority: None,
            shared: Some(false),
            provenance: BindingProvenance::new("test"),
        }],
        clocks: vec![ClockBinding {
            join_key: JoinKey::ExplicitId("uart_clk".into()),
            default_frequency: Some(48_000_000),
            provenance: BindingProvenance::new("test"),
        }],
        resets: vec![ResetBinding {
            join_key: JoinKey::FieldName("reset".into()),
            reset_line_id: Some("uart_rst0".into()),
            reset_shared: Some(false),
            provenance: BindingProvenance::new("test"),
        }],
        pinctrl: vec![PinCtrlBinding {
            join_key: JoinKey::ExplicitId("uart_pins".into()),
            pin_states: Some(vec!["default".into(), "sleep".into()]),
            provenance: BindingProvenance::new("test"),
        }],
        bus: Some(BusBinding {
            bus_type: regseq_model::declaration::driver::BusType::Amba,
            amba_id: None,
            revision: None,
            provenance: BindingProvenance::new("test"),
        }),
    }
}

/// Build the real AnalysisInput from the pl011 fixture.
fn make_real_analysis_input() -> AnalysisInput {
    let decl = load_pl011_decl_from_fixture();
    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

/// Helper: find a function body by name in the populated hierarchy.
fn find_function_body<'a>(hierarchy: &'a ItemHierarchy, name: &str) -> Option<&'a SemanticBody> {
    for i in 0..hierarchy.store.len() {
        let id = ItemId(i as u32);
        if let Some(Item::Function(f)) = hierarchy.store.get(id) {
            if f.name == name {
                return Some(&f.body);
            }
        }
    }
    None
}

/// Helper: check if a body has any non-trivial statements.
fn has_statements(body: &SemanticBody) -> bool {
    !body.body.stmts.is_empty()
}

/// Helper: collect all statement kinds in a body (recursively).
fn collect_stmt_kinds(stmts: &[Stmt]) -> Vec<String> {
    let mut kinds = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Let { .. } => kinds.push("Let".into()),
            Stmt::Assign { .. } => kinds.push("Assign".into()),
            Stmt::CompoundAssign { .. } => kinds.push("CompoundAssign".into()),
            Stmt::Eval { .. } => kinds.push("Eval".into()),
            Stmt::Branch {
                then_body,
                else_body,
                ..
            } => {
                kinds.push("Branch".into());
                kinds.extend(collect_stmt_kinds(&then_body.stmts));
                kinds.extend(collect_stmt_kinds(&else_body.stmts));
            }
            Stmt::Loop { body, .. } => {
                kinds.push("Loop".into());
                kinds.extend(collect_stmt_kinds(&body.stmts));
            }
            Stmt::OrderedRegion { body } => {
                kinds.push("OrderedRegion".into());
                kinds.extend(collect_stmt_kinds(&body.stmts));
            }
            Stmt::FallibleOp {
                body,
                cleanup_on_error,
                ..
            } => {
                kinds.push("FallibleOp".into());
                kinds.extend(collect_stmt_kinds(&body.stmts));
                kinds.extend(collect_stmt_kinds(&cleanup_on_error.stmts));
            }
            Stmt::Return { .. } => kinds.push("Return".into()),
            Stmt::RegWrite { .. } => kinds.push("RegWrite".into()),
            Stmt::RegModify { .. } => kinds.push("RegModify".into()),
            Stmt::ResourceOp { .. } => kinds.push("ResourceOp".into()),
            Stmt::FieldAssign { .. } => kinds.push("FieldAssign".into()),
            Stmt::SubsystemAction { .. } => kinds.push("SubsystemAction".into()),
        }
    }
    kinds
}

/// Helper: check if any expression in a body matches a predicate.
fn has_expr_matching(stmts: &[Stmt], pred: &dyn Fn(&Expr) -> bool) -> bool {
    for stmt in stmts {
        if stmt_has_expr_matching(stmt, pred) {
            return true;
        }
    }
    false
}

fn stmt_has_expr_matching(stmt: &Stmt, pred: &dyn Fn(&Expr) -> bool) -> bool {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_matches(value, pred),
        Stmt::Eval { expr } => expr_matches(expr, pred),
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => {
            expr_matches(condition, pred)
                || has_expr_matching(&then_body.stmts, pred)
                || has_expr_matching(&else_body.stmts, pred)
        }
        Stmt::Loop { condition, body } => {
            expr_matches(condition, pred) || has_expr_matching(&body.stmts, pred)
        }
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => {
            has_expr_matching(&body.stmts, pred) || has_expr_matching(&cleanup_on_error.stmts, pred)
        }
        Stmt::OrderedRegion { body } => has_expr_matching(&body.stmts, pred),
        Stmt::Return { value: Some(v) } => expr_matches(v, pred),
        Stmt::FieldAssign { value, .. } => expr_matches(value, pred),
        _ => false,
    }
}

fn expr_matches(expr: &Expr, pred: &dyn Fn(&Expr) -> bool) -> bool {
    if pred(expr) {
        return true;
    }
    match expr {
        Expr::BinaryOp { lhs, rhs, .. } => expr_matches(lhs, pred) || expr_matches(rhs, pred),
        Expr::UnaryOp { operand, .. } | Expr::Cast { expr: operand, .. } => {
            expr_matches(operand, pred)
        }
        Expr::HelperCall { args, .. } => args.iter().any(|a| expr_matches(a, pred)),
        Expr::WithDefault { value, default } => {
            expr_matches(value, pred) || expr_matches(default, pred)
        }
        Expr::IsSome { inner } | Expr::OptionSome { inner } | Expr::OptionUnwrap { inner } => {
            expr_matches(inner, pred)
        }
        Expr::InterruptClassTest { status, .. } => expr_matches(status, pred),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// MIR-driven: narrow subset (Review Stop 1)
// ---------------------------------------------------------------------------

#[test]
fn mir_narrow_subset_lowers_successfully() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();

    let result = lower_and_populate_hierarchy(&hierarchy, &input);
    assert!(
        result.is_ok(),
        "lower_and_populate_hierarchy failed: {:?}",
        result.err()
    );
    let (populated, _lowered_ids) = result.unwrap();

    // Narrow subset: uart_clock_hz, set_baud_rate, init
    for name in &["uart_clock_hz", "set_baud_rate", "init"] {
        let body = find_function_body(&populated, name);
        assert!(
            body.is_some(),
            "function '{name}' not found in populated hierarchy"
        );
        let body = body.unwrap();
        assert!(
            has_statements(body),
            "function '{name}' has empty body after lowering"
        );
    }
}

#[test]
fn mir_set_baud_rate_has_typed_locals_and_reg_writes() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "set_baud_rate").unwrap();

    // Must have self as params[0].
    assert!(
        !body.params.is_empty(),
        "set_baud_rate must have parameters"
    );
    let self_local = body.locals.iter().find(|l| l.name == "self");
    assert!(self_local.is_some(), "set_baud_rate must have self local");

    // Must have register writes.
    let kinds = collect_stmt_kinds(&body.body.stmts);
    assert!(
        kinds.contains(&"RegWrite".to_string()) || kinds.contains(&"RegModify".to_string()),
        "set_baud_rate must have register write operations, got: {kinds:?}"
    );
}

#[test]
fn mir_init_has_fallible_sequencing() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "init").unwrap();

    let kinds = collect_stmt_kinds(&body.body.stmts);
    assert!(
        kinds.contains(&"FallibleOp".to_string()),
        "init must have FallibleOp statements for ? sequencing, got: {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// MIR-driven: full representative set (Review Stop 2)
// ---------------------------------------------------------------------------

#[test]
fn mir_full_representative_set_lowers_successfully() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let representative = [
        "uart_clock_hz",
        "set_baud_rate",
        "init",
        "service_rx_fifo",
        "service_tx_fifo",
        "handle_irq",
    ];

    for name in &representative {
        let body = find_function_body(&populated, name);
        assert!(
            body.is_some(),
            "function '{name}' not found in populated hierarchy"
        );
        assert!(
            has_statements(body.unwrap()),
            "function '{name}' has empty body after lowering"
        );
    }
}

#[test]
fn mir_uart_clock_hz_has_resource_default_semantics() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "uart_clock_hz").unwrap();

    // Must contain a WithDefault expression whose value side is
    // a ResourcePropertyRead.
    let has_with_default = has_expr_matching(&body.body.stmts, &|e| {
        matches!(e, Expr::WithDefault { value, .. }
            if matches!(value.as_ref(), Expr::ResourcePropertyRead { .. }))
    });
    assert!(
        has_with_default,
        "uart_clock_hz must contain WithDefault wrapping ResourcePropertyRead"
    );
}

#[test]
fn mir_service_tx_fifo_has_option_carrier_semantics() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "service_tx_fifo").unwrap();

    // Must contain IsSome (option presence test on pending_tx).
    let has_is_some = has_expr_matching(&body.body.stmts, &|e| matches!(e, Expr::IsSome { .. }));
    assert!(
        has_is_some,
        "service_tx_fifo must contain IsSome for option-carrier semantics"
    );

    // Must contain OptionNone (pending_tx = None).
    let has_option_none = has_expr_matching(&body.body.stmts, &|e| matches!(e, Expr::OptionNone));
    assert!(
        has_option_none,
        "service_tx_fifo must contain OptionNone for field clear semantics"
    );
}

#[test]
fn mir_service_rx_fifo_has_option_carrier_semantics() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "service_rx_fifo").unwrap();

    // Must contain a Loop (the while rx_char().is_some() loop).
    let kinds = collect_stmt_kinds(&body.body.stmts);
    assert!(
        kinds.contains(&"Loop".to_string()),
        "service_rx_fifo must contain a Loop, got: {kinds:?}"
    );

    // Must contain explicit option-carrier semantics.
    // Require IsSome, OptionUnwrap, or OptionNone in the produced body.
    // HelperCall(rx_char) alone is NOT sufficient evidence.
    let has_option_semantic = has_expr_matching(&body.body.stmts, &|e| {
        matches!(
            e,
            Expr::IsSome { .. } | Expr::OptionUnwrap { .. } | Expr::OptionNone
        )
    });
    assert!(
        has_option_semantic,
        "service_rx_fifo must contain explicit option-carrier semantics (IsSome/OptionUnwrap/OptionNone)"
    );
}

#[test]
fn mir_handle_irq_has_interrupt_tests_and_accumulated_result() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();
    let body = find_function_body(&populated, "handle_irq").unwrap();

    // Must have Branch statements (for interrupt class dispatch).
    let kinds = collect_stmt_kinds(&body.body.stmts);
    assert!(
        kinds.contains(&"Branch".to_string()),
        "handle_irq must have Branch statements, got: {kinds:?}"
    );

    // Must have InterruptClassTest expressions in branch conditions.
    let has_interrupt_test = has_expr_matching(&body.body.stmts, &|e| {
        matches!(e, Expr::InterruptClassTest { .. })
    });
    assert!(
        has_interrupt_test,
        "handle_irq must contain InterruptClassTest expressions"
    );

    // Must have accumulated result flow: CompoundAssign with |= on the
    // handled local, proving interrupt dispatch results are accumulated.
    let has_compound_assign = kinds.contains(&"CompoundAssign".to_string());
    // Also accept Assign as the MIR may lower |= differently.
    let has_assign = kinds.contains(&"Assign".to_string());
    assert!(
        has_compound_assign || has_assign,
        "handle_irq must have accumulated result flow (CompoundAssign or Assign), got: {kinds:?}"
    );

    // Must have a Return statement.
    assert!(
        kinds.contains(&"Return".to_string()),
        "handle_irq must have a Return, got: {kinds:?}"
    );
}

// ---------------------------------------------------------------------------
// Validation wired into real produced bodies
// ---------------------------------------------------------------------------

#[test]
fn mir_validation_passes_on_produced_bodies() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, _lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let representative = [
        "uart_clock_hz",
        "set_baud_rate",
        "init",
        "service_rx_fifo",
        "service_tx_fifo",
        "handle_irq",
    ];

    for name in &representative {
        let body = find_function_body(&populated, name).unwrap();
        let errors = validate_semantic_body(body, name);

        // All 6 validation passes must succeed with zero errors.
        // No filtering. Any surviving Opaque or unsupported residue is a
        // hard failure and acceptance blocker.
        assert!(
            errors.is_empty(),
            "function '{name}' failed validation with {} error(s):\n{}",
            errors.len(),
            errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
