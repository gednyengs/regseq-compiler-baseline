//! Tests for SemanticModel projection (Phase 2C, regseq-lowering-08).
//!
//! Validates that:
//! - `project_semantic_model` selects exactly the lowered function set
//! - only referenced constants are projected
//! - interrupt classes are copied verbatim
//! - the canonical `build_driver_lowering_plan_from_hierarchy` produces a
//!   real populated `SemanticModel`
//! - projection is deterministic and OS-neutral
//!
//! Exercises the deprecated `regseq_analysis::legacy_input` shim during
//! the transitional window that ends with `regseq-lowering-10`.
#![allow(deprecated)]

use std::collections::{BTreeMap, BTreeSet};

use regseq_analysis::legacy_input::AnalysisInput;
use regseq_backends::lowering::hierarchy::{
    build_item_hierarchy, CallbackTraitPaths, ItemHierarchy,
};
use regseq_backends::lowering::planner::build_driver_lowering_plan_from_hierarchy;
use regseq_backends::lowering::projection::project_semantic_model;
use regseq_backends::lowering::semantic_lowering::lower_and_populate_hierarchy;
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::raw_input::{
    RawAssociatedConstantFactInput, RawFieldFactInput, RawIdentityEnrichmentInput,
    RawImplBlockFactInput, RawImplMethodFactInput, RawModuleFactInput, RawRseqFieldKindInput,
    RawStructFactInput,
};
use regseq_model::item::constant::ConstValue;
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::types::Item;
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;

// ===========================================================================
// Shared fixtures (same PL011 enrichment as semantic_body_lowering.rs)
// ===========================================================================

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
                        rseq_kind: None,
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
                    // An unreferenced constant for testing exclusion.
                    RawAssociatedConstantFactInput {
                        name: "UNREFERENCED_CONST".into(),
                        canonical_path:
                            "pl011_full_spec::driver::Pl011FullSpec::UNREFERENCED_CONST".into(),
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
        (
            CanonicalPath::new("pl011_full_spec::driver::Pl011FullSpec::UNREFERENCED_CONST"),
            ConstValue::Integer(42),
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
            "Failed to read raw-frontend.json at {}: {e}",
            fixture_path.display()
        )
    });
    regseq_model::declaration::normalize::normalize_from_json(&json)
        .expect("Failed to normalize raw artifact")
}

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

fn make_real_analysis_input() -> AnalysisInput {
    let decl = load_pl011_decl_from_fixture();
    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

fn authored_interrupt_constants() -> Vec<(String, u32)> {
    vec![
        (
            "pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK".into(),
            0x0050,
        ),
        (
            "pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK".into(),
            0x0020,
        ),
    ]
}

// ===========================================================================
// Test 1: empty hierarchy projects empty model
// ===========================================================================

#[test]
fn project_from_empty_hierarchy_produces_empty_model() {
    let enrichment = RawIdentityEnrichmentInput {
        modules: vec![],
        structs: vec![],
        enums: vec![],
        impl_blocks: vec![],
        free_functions: vec![],
    };
    let hierarchy = build_item_hierarchy(
        &enrichment,
        &[],
        &[],
        &BTreeMap::new(),
        &CallbackTraitPaths {
            lifecycle: "".into(),
            serial: "".into(),
        },
    )
    .unwrap();

    let model = project_semantic_model(&hierarchy, &BTreeSet::new());
    assert!(model.functions.is_empty());
    assert!(model.constants.is_empty());
    assert!(model.interrupt_classes.is_empty());
}

// ===========================================================================
// Test 2: projected functions are exactly the lowered set
// ===========================================================================

#[test]
fn projected_functions_are_exactly_the_lowered_set() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    // Every projected function must be in the lowered set.
    for func in &model.functions {
        assert!(
            lowered_ids.contains(&func.id),
            "projected function '{}' (id {:?}) not in lowered set",
            func.name,
            func.id
        );
    }

    // Every lowered function must be projected.
    let projected_ids: BTreeSet<ItemId> = model.functions.iter().map(|f| f.id).collect();
    for id in &lowered_ids {
        assert!(
            projected_ids.contains(id),
            "lowered function id {:?} not in projected set",
            id
        );
    }

    assert_eq!(model.functions.len(), lowered_ids.len());
}

// ===========================================================================
// Test 3: projected constants are referenced only
// ===========================================================================

#[test]
fn projected_constants_are_referenced_only() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    // There should be some constants (the PL011 fixture references
    // DEFAULT_UART_CLOCK_HZ in uart_clock_hz, and interrupt masks in handle_irq).
    assert!(
        !model.constants.is_empty(),
        "expected at least one referenced constant"
    );

    // All projected constants should have non-empty canonical paths.
    for c in &model.constants {
        assert!(
            !c.canonical_path.as_str().is_empty(),
            "projected constant has empty canonical path"
        );
    }
}

// ===========================================================================
// Test 4: unreferenced constants are excluded
// ===========================================================================

#[test]
fn unreferenced_constants_excluded() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    // UNREFERENCED_CONST was added to the enrichment but is not referenced
    // by any function body. It should NOT appear in the projected model.
    let has_unreferenced = model.constants.iter().any(|c| {
        c.canonical_path.as_str() == "pl011_full_spec::driver::Pl011FullSpec::UNREFERENCED_CONST"
    });
    assert!(
        !has_unreferenced,
        "UNREFERENCED_CONST should not be in projected SemanticModel"
    );

    // But it should exist in the hierarchy (to prove the exclusion is active).
    let in_hierarchy = (0..populated.store.len()).any(|i| {
        let id = ItemId(i as u32);
        matches!(
            populated.store.get(id),
            Some(Item::Constant(c)) if c.canonical_path.as_str()
                == "pl011_full_spec::driver::Pl011FullSpec::UNREFERENCED_CONST"
        )
    });
    assert!(
        in_hierarchy,
        "UNREFERENCED_CONST should exist in the hierarchy"
    );
}

// ===========================================================================
// Test 5: interrupt classes match hierarchy
// ===========================================================================

#[test]
fn projected_interrupt_classes_match_hierarchy() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    assert_eq!(
        model.interrupt_classes.len(),
        populated.interrupt_classes.len()
    );
    for (projected, original) in model
        .interrupt_classes
        .iter()
        .zip(populated.interrupt_classes.iter())
    {
        assert_eq!(projected.name, original.name);
        assert_eq!(projected.mask_value, original.mask_value);
    }
}

// ===========================================================================
// Test 6: projection is deterministic
// ===========================================================================

#[test]
fn projection_is_deterministic() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model1 = project_semantic_model(&populated, &lowered_ids);
    let model2 = project_semantic_model(&populated, &lowered_ids);

    assert_eq!(model1, model2);
}

// ===========================================================================
// Test 7: canonical builder populates real SemanticModel
// ===========================================================================

#[test]
fn canonical_builder_populates_real_semantic_model() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let output = build_driver_lowering_plan_from_hierarchy(
        &input,
        &populated,
        &lowered_ids,
        &authored_interrupt_constants(),
    )
    .expect("canonical builder should succeed");

    // semantic_model (SemanticModel) must be populated.
    assert!(
        !output.semantic_model.functions.is_empty(),
        "canonical builder must produce non-empty SemanticModel.functions"
    );
    assert_eq!(output.semantic_model.functions.len(), lowered_ids.len());

    // Interrupt classes must be present.
    assert_eq!(
        output.semantic_model.interrupt_classes.len(),
        populated.interrupt_classes.len()
    );

    // legacy_behavior must also be populated for transitional Linux support.
    assert!(
        !output.legacy_behavior.functions.is_empty(),
        "legacy_behavior must be populated for transitional compatibility"
    );
}

// ===========================================================================
// Test 8: projection preserves function identity and body
// ===========================================================================

#[test]
fn projection_preserves_function_identity_and_body() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    for func in &model.functions {
        // Canonical path must be non-empty.
        assert!(
            !func.canonical_path.as_str().is_empty(),
            "function '{}' has empty canonical path",
            func.name
        );
        // Name must be non-empty.
        assert!(!func.name.is_empty());
        // Body must have statements (it was lowered).
        assert!(
            !func.body.body.stmts.is_empty(),
            "function '{}' should have a non-empty body",
            func.name
        );

        // The projected function must be identical to the one in the hierarchy.
        let hierarchy_func = populated.store.get(func.id).unwrap();
        if let Item::Function(hf) = hierarchy_func {
            assert_eq!(func.canonical_path, hf.canonical_path);
            assert_eq!(func.name, hf.name);
            assert_eq!(func.role, hf.role);
            assert_eq!(func.body, hf.body);
        } else {
            panic!("expected Function item for id {:?}", func.id);
        }
    }
}

// ===========================================================================
// Test 9: projection preserves constant identity
// ===========================================================================

#[test]
fn projection_preserves_constant_identity() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    for constant in &model.constants {
        // Canonical path and stable name must be non-empty.
        assert!(!constant.canonical_path.as_str().is_empty());
        assert!(!constant.stable_name.is_empty());

        // The projected constant must match the hierarchy.
        let hierarchy_const = populated.store.get(constant.id).unwrap();
        if let Item::Constant(hc) = hierarchy_const {
            assert_eq!(constant.canonical_path, hc.canonical_path);
            assert_eq!(constant.stable_name, hc.stable_name);
            assert_eq!(constant.value, hc.value);
            assert_eq!(constant.owner, hc.owner);
        } else {
            panic!("expected Constant item for id {:?}", constant.id);
        }
    }
}

// ===========================================================================
// Test 10: projection is OS-neutral
// ===========================================================================

#[test]
fn projection_is_os_neutral() {
    let input = make_real_analysis_input();
    let hierarchy = build_pl011_hierarchy();
    let (populated, lowered_ids) = lower_and_populate_hierarchy(&hierarchy, &input).unwrap();

    let model = project_semantic_model(&populated, &lowered_ids);

    let os_specific = ["linux", "zephyr", "Linux", "Zephyr", "LINUX", "ZEPHYR"];

    for func in &model.functions {
        for keyword in &os_specific {
            assert!(
                !func.name.contains(keyword),
                "function '{}' contains OS-specific keyword '{}'",
                func.name,
                keyword
            );
        }
    }

    for constant in &model.constants {
        for keyword in &os_specific {
            assert!(
                !constant.name.contains(keyword),
                "constant '{}' contains OS-specific keyword '{}'",
                constant.name,
                keyword
            );
        }
    }
}
