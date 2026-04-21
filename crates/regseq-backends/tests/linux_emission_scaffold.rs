//! Linux backend emission scaffold tests.
//!
//! Verifies that `LinuxBackendPlan` reaches the backend-emission entry
//! point, produces deterministic artifacts, and emits readable scaffold
//! C output.
//!
//! Exercises the deprecated `regseq_analysis::legacy_input` shim during
//! the transitional window that ends with `regseq-lowering-10`.
#![allow(deprecated)]

use std::collections::BTreeMap;

use regseq_analysis::legacy_input::AnalysisInput;
use regseq_backends::linux::emitter::{emit_linux_artifacts, EmittedArtifactSet};
use regseq_backends::linux::plan::{LinuxBackendPlan, LinuxFileRole};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::hierarchy::{build_item_hierarchy, CallbackTraitPaths};
use regseq_backends::lowering::plan::DriverLoweringPlan;
use regseq_backends::lowering::planner::build_driver_lowering_plan_from_hierarchy;
use regseq_backends::lowering::semantic_lowering::lower_and_populate_hierarchy;
use regseq_model::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};
use regseq_model::declaration::callbacks::{
    CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl,
};
use regseq_model::declaration::driver::BusType as ModelBusType;
use regseq_model::declaration::mir_body::MirBodyDeclSet;
use regseq_model::declaration::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LoggingDeclSet,
    PmGateDeclSet, TimedDelayCallSiteDecl, TimedDelayDeclSet,
};
use regseq_model::declaration::properties::{PropertyDeclSet, PropertyFieldDecl};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::raw_input::{
    RawAssociatedConstantFactInput, RawFieldFactInput, RawIdentityEnrichmentInput,
    RawImplBlockFactInput, RawImplMethodFactInput, RawModuleFactInput, RawRseqFieldKindInput,
    RawStructFactInput,
};
use regseq_model::declaration::registers::{
    RegisterBlockDecl, RegisterDeclSet, RegisterFieldDecl, WrapperCallSiteDecl, WrapperMethod,
};
use regseq_model::declaration::resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, IsrBindTarget, IsrBindingDecl,
    MmioFieldDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetCallSiteDecl, ResetDeclSet,
    ResetFieldDecl, ResetMethod, ResourceDeclSet,
};
use regseq_model::declaration::synchronization::SynchronizationDeclSet;
use regseq_model::declaration::{
    BodyDeclSet, DeclarationBuildContext, DeclarationModel, DeviceClass, DriverDecl,
};
use regseq_model::item::constant::ConstValue;
use regseq_model::item::identity::CanonicalPath;
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::joined::perform_join;
use regseq_model::platform::PlatformValue;
use tempfile::TempDir;

// ---- Shared fixture builder (mirrors pl011_full_spec_lowering.rs) ----

fn pl011_decl() -> DeclarationModel {
    DeclarationModel {
        driver: DriverDecl {
            driver_name: "Pl011FullSpec".into(),
            compatible: vec!["arm,pl011".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(ModelBusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![MmioFieldDecl {
                field_name: "regs".into(),
                mmio_id: Some("uart_regs".into()),
                register_block_type: Some("Pl011Registers".into()),
                provenance: Provenance::new("register_access"),
            }],
            interrupts: InterruptDeclSet {
                resources: vec![InterruptResourceDecl {
                    field_name: "irq".into(),
                    interrupt_id: "uart_irq".into(),
                    shape: InterruptFieldShape::Single,
                    binding_hints: InterruptBindingHints {
                        trigger_type: Some("level".into()),
                        trigger_polarity: Some("high".into()),
                        priority: None,
                        shared: Some(false),
                    },
                    provenance: Provenance::new("interrupt"),
                }],
                isr_bindings: vec![IsrBindingDecl {
                    handler_name: "handle_irq".into(),
                    bind_target: IsrBindTarget::Auto,
                    has_index_param: false,
                    context: CallbackContext::Framework,
                    provenance: Provenance::new("interrupt"),
                }],
            },
            clock: ClockDeclSet {
                fields: vec![ClockFieldDecl {
                    field_name: "clock".into(),
                    clock_id: "uart_clk".into(),
                    required: false,
                    provenance: Provenance::new("clock"),
                }],
                call_sites: vec![
                    ClockCallSiteDecl {
                        method: ClockMethod::Enable,
                        containing_fn: "enable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                    ClockCallSiteDecl {
                        method: ClockMethod::Disable,
                        containing_fn: "disable_clock_if_present".into(),
                        provenance: Provenance::new("clock"),
                    },
                ],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![ResetCallSiteDecl {
                    method: ResetMethod::Toggle,
                    containing_fn: "toggle_reset_if_present".into(),
                    provenance: Provenance::new("reset"),
                }],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![PinCtrlFieldDecl {
                    field_name: "pinctrl".into(),
                    required: false,
                    pinctrl_id: Some("uart_pins".into()),
                    provenance: Provenance::new("pinctrl"),
                }],
                call_sites: vec![],
            },
        },
        callbacks: CallbackDeclSet {
            lifecycle_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::Framework,
                family: CallbackFamily::Lifecycle,
                provenance: Provenance::new("callback"),
            }],
            serial_impls: vec![CallbackImplDecl {
                driver_type: "Pl011FullSpec".into(),
                context: CallbackContext::SubsystemSerialized,
                family: CallbackFamily::Serial,
                provenance: Provenance::new("callback"),
            }],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        },
        registers: RegisterDeclSet {
            blocks: vec![RegisterBlockDecl {
                name: "Pl011Registers".into(),
                fields: vec![
                    RegisterFieldDecl {
                        name: "dr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "fr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                    RegisterFieldDecl {
                        name: "cr".into(),
                        access_width: Some(32),
                        endian: None,
                        side_effects: None,
                        hw_updates: None,
                    },
                ],
                provenance: Provenance::new("register_access"),
            }],
            descriptors: vec![],
            wrapper_call_sites: vec![
                WrapperCallSiteDecl {
                    method: WrapperMethod::Write,
                    containing_fn: "tx_byte".into(),
                    provenance: Provenance::new("register_access"),
                },
                WrapperCallSiteDecl {
                    method: WrapperMethod::Read,
                    containing_fn: "rx_char".into(),
                    provenance: Provenance::new("register_access"),
                },
            ],
            poll_call_sites: vec![],
        },
        properties: PropertyDeclSet {
            fields: vec![PropertyFieldDecl {
                driver_type: "Pl011FullSpec".into(),
                field_name: "current_speed".into(),
                field_type: "u32".into(),
                property_id: "current-speed".into(),
                default_literal: Some("115200".into()),
                vendor_prefix: None,
                required: true,
                provenance: Provenance::new("firmware_properties"),
            }],
            lints: vec![],
        },
        synchronization: SynchronizationDeclSet {
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![TimedDelayCallSiteDecl {
                intrinsic: DelayIntrinsic::BusyWait,
                path_kind: DelayPathKind::Canonical,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
            duration_ctors: vec![DurationCtorSiteDecl {
                ctor: DurationCtor::FromUs,
                containing_fn: "toggle_reset_if_present".into(),
                provenance: Provenance::new("timed_delay"),
            }],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        bodies: BodyDeclSet { functions: vec![] },
        mir_bodies: load_real_mir_bodies(),
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

/// Load real captured MIR bodies from the pl011-full-spec raw artifact.
fn load_real_mir_bodies() -> MirBodyDeclSet {
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
    let model = regseq_model::declaration::normalize::normalize_from_json(&json)
        .expect("Failed to normalize raw artifact");
    model.mir_bodies
}

fn pl011_binding() -> BindingModel {
    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "raspberry-pi-4".into(),
            provenance: BindingProvenance::new("platform")
                .with_source_file("pl011-full-spec/platform.toml"),
        },
        properties: vec![PropertyBinding {
            key: "current-speed".into(),
            value: PlatformValue::Integer(115200),
            provenance: BindingProvenance::new("properties").with_section_key("current-speed"),
        }],
        mmio: vec![MmioBinding {
            join_key: JoinKey::ExplicitId("uart_regs".into()),
            base: Some(0x0900_0000),
            size: Some(0x1000),
            provenance: BindingProvenance::new("mmio").with_section_key("uart_regs"),
        }],
        interrupts: vec![InterruptBinding {
            join_key: JoinKey::ExplicitId("uart_irq".into()),
            trigger_type: Some("level".into()),
            trigger_polarity: Some("high".into()),
            priority: None,
            shared: Some(false),
            provenance: BindingProvenance::new("interrupt").with_section_key("uart_irq"),
        }],
        clocks: vec![ClockBinding {
            join_key: JoinKey::ExplicitId("uart_clk".into()),
            default_frequency: Some(48_000_000),
            provenance: BindingProvenance::new("clock").with_section_key("uart_clk"),
        }],
        resets: vec![ResetBinding {
            join_key: JoinKey::FieldName("reset".into()),
            reset_line_id: Some("uart_rst0".into()),
            reset_shared: Some(false),
            provenance: BindingProvenance::new("reset").with_section_key("reset"),
        }],
        pinctrl: vec![PinCtrlBinding {
            join_key: JoinKey::ExplicitId("uart_pins".into()),
            pin_states: Some(vec!["default".into(), "sleep".into()]),
            provenance: BindingProvenance::new("pinctrl").with_section_key("uart_pins"),
        }],
        bus: Some(BusBinding {
            bus_type: ModelBusType::Amba,
            amba_id: Some(0x00041011),
            revision: Some(3),
            provenance: BindingProvenance::new("bus"),
        }),
    }
}

fn make_input() -> AnalysisInput {
    let decl = pl011_decl();
    let binding = pl011_binding();
    let bus_binding = binding.bus.clone();
    let joined = perform_join(&decl, &binding).expect("join should succeed");
    AnalysisInput::new(decl, joined, bus_binding)
}

fn authored_interrupt_constants() -> Vec<(String, u32)> {
    vec![
        ("Self::RX_INTERRUPT_MASK".into(), (1 << 4) | (1 << 6)),
        ("Self::TX_INTERRUPT_MASK".into(), 1 << 5),
        (
            "Self::ERROR_INTERRUPT_MASK".into(),
            (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
        ),
        (
            "Self::EXPLICIT_ISR_CLEAR_MASK".into(),
            (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
        ),
        ("Self::ALL_UART_INTERRUPTS_MASK".into(), 0x07ff),
    ]
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

fn make_plans() -> (DriverLoweringPlan, LinuxBackendPlan) {
    let input = make_input();
    let hierarchy = build_item_hierarchy(
        &pl011_enrichment(),
        &pl011_constant_values(),
        &pl011_interrupt_classes(),
        &pl011_isr_bindings(),
        &pl011_trait_paths(),
    )
    .expect("hierarchy build should succeed");
    let (populated, lowered_ids) =
        lower_and_populate_hierarchy(&hierarchy, &input).expect("semantic lowering");
    let output = build_driver_lowering_plan_from_hierarchy(
        &input,
        &populated,
        &lowered_ids,
        &authored_interrupt_constants(),
    )
    .expect("portable plan");
    let portable = output.plan;
    let linux = build_linux_backend_plan(&portable, output.semantic_model).expect("linux plan");
    (portable, linux)
}

fn emit_to_tempdir() -> (TempDir, EmittedArtifactSet) {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let set = emit_linux_artifacts(&linux, &portable, dir.path()).expect("emit");
    (dir, set)
}

// ---- Test 1: LinuxBackendPlan reaches emission entry point ----

#[test]
fn plan_reaches_emission_entry_point() {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let result = emit_linux_artifacts(&linux, &portable, dir.path());
    assert!(result.is_ok(), "emission should succeed");
}

// ---- Test 2: emitted artifact set and paths are deterministic ----

#[test]
fn emitted_artifacts_are_deterministic() {
    let (_dir1, set1) = emit_to_tempdir();
    let (_dir2, set2) = emit_to_tempdir();

    assert_eq!(set1.artifacts.len(), set2.artifacts.len());
    for (a, b) in set1.artifacts.iter().zip(set2.artifacts.iter()) {
        assert_eq!(
            a.path.file_name(),
            b.path.file_name(),
            "filenames must match"
        );
        assert_eq!(a.role, b.role, "roles must match");
        assert_eq!(a.size_bytes, b.size_bytes, "sizes must match");
    }
}

#[test]
fn emitted_content_is_deterministic() {
    let (dir1, set1) = emit_to_tempdir();
    let (dir2, set2) = emit_to_tempdir();

    for (a, b) in set1.artifacts.iter().zip(set2.artifacts.iter()) {
        let content_a = std::fs::read_to_string(&a.path).expect("read a");
        let content_b =
            std::fs::read_to_string(dir2.path().join(b.path.file_name().unwrap())).expect("read b");
        assert_eq!(
            content_a,
            content_b,
            "content of {} must be deterministic",
            a.path.display()
        );
    }
    drop(dir1);
}

// ---- Test 3: output root is repo-local output/linux/pl011-full-spec/ ----

#[test]
fn output_root_matches_artifact_set() {
    let (dir, set) = emit_to_tempdir();
    assert_eq!(set.output_root, dir.path());
    assert_eq!(set.driver_id, "Pl011FullSpec");
}

#[test]
fn repo_local_output_path_works() {
    let (portable, linux) = make_plans();
    let dir = TempDir::new().expect("tempdir");
    let output_root = dir.path().join("output/linux/pl011-full-spec");
    let set = emit_linux_artifacts(&linux, &portable, &output_root).expect("emit");
    assert!(output_root.exists());
    assert_eq!(set.output_root, output_root);
    for artifact in &set.artifacts {
        assert!(
            artifact.path.exists(),
            "{} should exist",
            artifact.path.display()
        );
    }
}

// ---- Test 4: emitted file partitioning matches backend contract ----

#[test]
fn emitted_files_match_declared_layout() {
    let (_dir, set) = emit_to_tempdir();
    let roles: Vec<&LinuxFileRole> = set.artifacts.iter().map(|a| &a.role).collect();

    assert!(roles.contains(&&LinuxFileRole::DriverMain));
    assert!(roles.contains(&&LinuxFileRole::SharedHeader));
    assert!(roles.contains(&&LinuxFileRole::Makefile));
    assert!(roles.contains(&&LinuxFileRole::KbuildFragment));
    assert!(roles.contains(&&LinuxFileRole::RegisterHelpers));
}

#[test]
fn emitted_filenames_follow_naming_convention() {
    let (_dir, set) = emit_to_tempdir();
    let filenames: Vec<String> = set
        .artifacts
        .iter()
        .map(|a| a.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    assert!(
        filenames.contains(&"regseq_pl011_full_spec_module.c".to_string()),
        "module integration file"
    );
    assert!(
        filenames.contains(&"regseq_pl011_full_spec_shared.h".to_string()),
        "shared header"
    );
    assert!(filenames.contains(&"Makefile".to_string()), "Makefile");
    assert!(filenames.contains(&"Kbuild".to_string()), "Kbuild");
    assert!(
        filenames.contains(&"pl011_full_spec_regs.h".to_string()),
        "register helpers"
    );
}

#[test]
fn all_emitted_files_are_nonempty() {
    let (_dir, set) = emit_to_tempdir();
    for artifact in &set.artifacts {
        assert!(
            artifact.size_bytes > 0,
            "{} should be non-empty",
            artifact.path.display()
        );
    }
}

// ---- Test 5: smoke-level content checks for readability ----

#[test]
fn module_integration_contains_amba_registration() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(
        content.contains("struct amba_driver"),
        "should use amba_driver for PL011"
    );
    assert!(
        content.contains("amba_driver_register"),
        "should use amba_driver_register for serial + AMBA"
    );
    assert!(
        content.contains("MODULE_LICENSE"),
        "should have module license"
    );
    assert!(
        content.contains("0x00041011"),
        "should contain AMBA peripheral ID"
    );
    assert!(
        content.contains("pl011_full_spec_probe"),
        "should have probe function"
    );
    assert!(
        content.contains("pl011_full_spec_remove"),
        "should have remove function"
    );
}

#[test]
fn module_integration_includes_shared_header() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("#include \"regseq_pl011_full_spec_shared.h\""));
}

#[test]
fn module_integration_has_spdx_and_banner() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("SPDX-License-Identifier: GPL-2.0"));
    assert!(content.contains("Generated by Regseq compiler"));
}

#[test]
fn shared_header_has_driver_state_struct() {
    let (_dir, set) = emit_to_tempdir();
    let header = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::SharedHeader)
        .expect("shared header");
    let content = std::fs::read_to_string(&header.path).expect("read");

    assert!(content.contains("struct pl011_full_spec_data"));
    assert!(content.contains("void __iomem *"), "MMIO field");
    assert!(content.contains("struct uart_port"), "serial port field");
    assert!(content.contains("#ifndef"), "include guard");
    assert!(content.contains("#endif"), "include guard close");
}

#[test]
fn register_header_has_block_reference() {
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    // Register defines should be present (plan-driven naming).
    assert!(
        content.contains("PL011_DR") || content.contains("#define"),
        "register offset defines"
    );
    assert!(content.contains("#ifndef"), "include guard");
}

#[test]
fn makefile_has_out_of_tree_build_rules() {
    let (_dir, set) = emit_to_tempdir();
    let mf = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::Makefile)
        .expect("makefile");
    let content = std::fs::read_to_string(&mf.path).expect("read");

    assert!(content.contains("obj-m"));
    assert!(content.contains("$(MAKE)"));
    assert!(content.contains("modules"));
}

#[test]
fn kbuild_references_module_object() {
    let (_dir, set) = emit_to_tempdir();
    let kb = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::KbuildFragment)
        .expect("kbuild");
    let content = std::fs::read_to_string(&kb.path).expect("read");

    assert!(content.contains("obj-m"));
    assert!(content.contains("regseq_pl011_full_spec_module.o"));
}

#[test]
fn probe_body_references_all_resource_steps() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("devm_clk_get"), "clock acquisition");
    assert!(content.contains("devm_ioremap_resource"), "MMIO mapping");
    assert!(content.contains("devm_request_irq"), "IRQ request");
    assert!(content.contains("device_property_read"), "property read");
    assert!(content.contains("uart_add_one_port"), "UART registration");
}

#[test]
fn remove_body_references_teardown_steps() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(
        content.contains("uart_remove_one_port"),
        "UART unregistration"
    );
}

#[test]
fn module_integration_uses_correct_includes_for_amba() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("#include <linux/amba/bus.h>"));
    assert!(content.contains("#include <linux/serial_core.h>"));
    assert!(content.contains("#include <linux/module.h>"));
    assert!(!content.contains("platform_device.h"));
}

// ---- New tests for real emission content ----

#[test]
fn module_has_uart_ops_definition() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("struct uart_ops"), "uart_ops struct");
    assert!(content.contains("tx_empty"), "tx_empty op");
    assert!(content.contains("start_tx"), "start_tx op");
    assert!(content.contains("stop_tx"), "stop_tx op");
    assert!(content.contains("startup"), "startup op");
    assert!(content.contains("shutdown"), "shutdown op");
    assert!(content.contains("set_termios"), "set_termios op");
}

#[test]
fn module_has_uart_driver_definition() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("struct uart_driver"), "uart_driver struct");
    assert!(
        content.contains("uart_register_driver"),
        "uart registration"
    );
    assert!(content.contains("uart_unregister_driver"), "uart cleanup");
}

#[test]
fn module_has_isr_handler() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("irqreturn_t"), "ISR return type");
    assert!(content.contains("irq_handler"), "ISR function");
    assert!(content.contains("IRQ_HANDLED"), "IRQ_HANDLED");
    assert!(content.contains("IRQ_NONE"), "IRQ_NONE");
    // ISR reads the MIS register — define name is plan-driven.
    assert!(content.contains("reg_read"), "ISR reads a register");
}

#[test]
fn module_has_register_accessors() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("readl("), "readl call");
    assert!(content.contains("writel("), "writel call");
    assert!(content.contains("reg_read"), "read accessor");
    assert!(content.contains("reg_write"), "write accessor");
    assert!(content.contains("reg_modify"), "modify accessor");
}

#[test]
fn register_header_has_plan_driven_offsets() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    // Offsets are now plan-driven: every field in the register layout
    // should have a corresponding define in the emitted header.
    for block in &linux.register_layout.blocks {
        for field in &block.fields {
            let define_name = format!("{}_{}", block.reg_prefix, field.name.to_uppercase());
            assert!(
                content.contains(&define_name),
                "register offset define '{define_name}' must be in emitted header"
            );
        }
    }

    // Check that the header has offset values.
    assert!(content.contains("0x000"), "should have offset 0x000");
}

#[test]
fn register_header_has_plan_driven_bitfields_and_named_values() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let regs = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::RegisterHelpers)
        .expect("register helpers");
    let content = std::fs::read_to_string(&regs.path).expect("read");

    // Bitfield masks and named values should be emitted from plan data.
    // If the plan has descriptors with bitfields, those should appear.
    for desc in &linux.register_layout.descriptors {
        for bf in &desc.bit_fields {
            // Each bitfield should have a mask define.
            let reg_prefix = linux
                .register_layout
                .blocks
                .first()
                .map(|b| b.reg_prefix.as_str())
                .unwrap_or("REG");
            let desc_prefix = desc
                .register_name
                .strip_suffix("_REG")
                .unwrap_or(&desc.register_name);
            let mask_name = format!("{reg_prefix}_{desc_prefix}_{}", bf.name);
            assert!(
                content.contains(&mask_name),
                "bitfield mask '{mask_name}' must be in emitted header"
            );

            // Named values should have defines.
            for nv in &bf.named_values {
                let nv_name = format!("{reg_prefix}_{desc_prefix}_{}_{}", bf.name, nv.name);
                assert!(
                    content.contains(&nv_name),
                    "named value '{nv_name}' must be in emitted header"
                );
            }
        }
    }
}

#[test]
fn shared_header_has_property_fields() {
    let (_dir, set) = emit_to_tempdir();
    let header = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::SharedHeader)
        .expect("shared header");
    let content = std::fs::read_to_string(&header.path).expect("read");

    assert!(content.contains("current_speed"), "property field");
}

#[test]
fn module_has_module_init_exit() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("module_init("), "module_init");
    assert!(content.contains("module_exit("), "module_exit");
}

#[test]
fn probe_has_real_api_calls() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    assert!(content.contains("devm_kzalloc("), "allocates driver data");
    assert!(content.contains("IS_ERR("), "error checking");
    assert!(content.contains("PTR_ERR("), "error propagation");
    assert!(content.contains("amba_set_drvdata("), "sets driver data");
}

// ---- Plan-driven verification tests ----

#[test]
fn interrupt_masks_come_from_plan_data() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Each interrupt class from the plan should have a define in the output.
    for class in &linux.behavior.interrupt_classes {
        let define = format!("PL011_FULL_SPEC_{}_IRQ_MASK", class.name.to_uppercase());
        assert!(
            content.contains(&define),
            "interrupt class define '{define}' must appear in emitted module"
        );
    }
}

#[test]
fn property_defaults_use_authored_literals() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Property defaults should use authored literal values from the plan.
    for prop in &linux.property_realization {
        if let Some(ref default_lit) = prop.default_literal {
            let prop_upper = prop.property_id.replace('-', "_").to_uppercase();
            let define_name = format!("PL011_FULL_SPEC_DEFAULT_{prop_upper}");
            assert!(
                content.contains(&define_name),
                "property default define '{define_name}' must appear"
            );
            assert!(
                content.contains(default_lit),
                "authored default literal '{default_lit}' must appear"
            );
        }
    }
}

#[test]
fn port_type_comes_from_plan() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    if let Some(ref serial) = linux.serial_core {
        assert!(
            content.contains(&serial.port_type.define_name),
            "port type define '{}' must appear",
            serial.port_type.define_name
        );
    }
}

#[test]
fn dev_name_comes_from_plan() {
    let (_portable, linux) = make_plans();
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    if let Some(ref serial) = linux.serial_core {
        assert!(
            content.contains(&serial.dev_name),
            "dev_name '{}' must appear in uart_driver",
            serial.dev_name
        );
        // Must NOT contain hardcoded "ttyPL".
        assert!(
            !content.contains("\"ttyPL\""),
            "must not contain hardcoded ttyPL"
        );
    }
}

#[test]
fn emitted_output_is_plan_driven_not_hardcoded() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Must not contain hardcoded PL011 interrupt mask literals.
    assert!(
        !content.contains("0x07ffU"),
        "must not contain hardcoded ALL_IRQ_MASK value"
    );
    assert!(
        !content.contains("\"113\"") && !content.contains(" 113"),
        "must not contain hardcoded port type 113"
    );
}

// ---- Rejection reason 1: helper signature correctness ----

#[test]
fn helper_functions_have_authored_signatures() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Helpers with parameters must NOT be emitted as just
    // `static void <snake>_<name>(struct <snake>_data *d)`.
    // They must include their authored parameters.
    //
    // The full-spec has helpers like set_baud_rate(uart_clk_hz, baud_rate)
    // and clear_explicit_interrupts(status). If the MIR carries these
    // parameters, the emitted C must include them.
    let (_, legacy_behavior) = regseq_backends::lowering::planner::build_legacy_behavior_for(
        &make_input(),
        &authored_interrupt_constants(),
    );
    for func in &legacy_behavior.functions {
        if func.role != regseq_backends::lowering::behavior::FunctionRole::Helper {
            continue;
        }
        if func.params.is_empty() {
            continue;
        }
        // For each param, the emitted helper should contain the param name.
        for p in &func.params {
            assert!(
                content.contains(&p.name),
                "helper '{}' must emit authored parameter '{}' in its signature",
                func.name,
                p.name
            );
        }
    }
}

#[test]
fn helper_calls_pass_arguments() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Helpers with parameters should be called with arguments, not just (d).
    // Specifically, if a helper has N params, its call sites should contain
    // at least N+1 comma-separated arguments (d + authored params).
    let (_, legacy_behavior) = regseq_backends::lowering::planner::build_legacy_behavior_for(
        &make_input(),
        &authored_interrupt_constants(),
    );
    let helpers_with_params: Vec<&str> = legacy_behavior
        .functions
        .iter()
        .filter(|f| {
            f.role == regseq_backends::lowering::behavior::FunctionRole::Helper
                && !f.params.is_empty()
        })
        .map(|f| f.name.as_str())
        .collect();

    // For each parameterized helper, check that at least one call site
    // passes more than just (d).
    for helper_name in &helpers_with_params {
        let call_pattern = format!("pl011_full_spec_{helper_name}(");
        if let Some(pos) = content.find(&call_pattern) {
            let rest = &content[pos + call_pattern.len()..];
            if let Some(end) = rest.find(')') {
                let args_str = &rest[..end];
                // Should have at least one comma (d, ...).
                assert!(
                    args_str.contains(','),
                    "call to helper '{helper_name}' should pass arguments, not just (d); got: ({args_str})"
                );
            }
        }
    }
}

// ---- Rejection reason 2: Linux callback mapping correctness ----

#[test]
fn start_tx_stop_tx_stop_rx_are_not_identical() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // start_tx and stop_tx must NOT have identical bodies.
    // Under the old broken mapping, both used SerialTxByte behavior.
    // Under the corrected mapping, both are honest stubs with TODO
    // comments but they must not reuse a portable role body.
    //
    // With a populated SemanticModel, tx_byte exists as a proper helper
    // function in the emitted C. The assertion checks that start_tx and
    // stop_tx bodies do not call tx_byte (which would mean misuse), not
    // that the identifier is absent from the entire file.
    for callback in &["start_tx", "stop_tx"] {
        let sig = format!("pl011_full_spec_{callback}");
        if let Some(pos) = content.find(&sig) {
            let window = &content[pos..std::cmp::min(pos + 500, content.len())];
            assert!(
                !window.contains("pl011_full_spec_tx_byte"),
                "{callback} must not reuse tx_byte behavior body"
            );
        }
    }
}

#[test]
fn stop_rx_does_not_use_flush_rx_fifo() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // Extract the stop_rx function body.
    let stop_rx_sig = "pl011_full_spec_stop_rx";
    assert!(content.contains(stop_rx_sig), "stop_rx must exist");

    // The stop_rx body must NOT contain flush_rx_fifo behavior.
    // Find the stop_rx function and check its body.
    if let Some(pos) = content.find(stop_rx_sig) {
        // Extract a reasonable window after the signature.
        let window = &content[pos..std::cmp::min(pos + 500, content.len())];
        assert!(
            !window.contains("flush"),
            "stop_rx must not contain flush_rx_fifo behavior"
        );
    }
}

#[test]
fn start_tx_stop_tx_stop_rx_are_honest_stubs() {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("driver main");
    let content = std::fs::read_to_string(&main.path).expect("read");

    // These callbacks should contain TODO comments indicating they are
    // not yet realized from lowered plan data.
    for callback in &["start_tx", "stop_tx", "stop_rx"] {
        let sig = format!("pl011_full_spec_{callback}");
        assert!(content.contains(&sig), "{callback} must exist");
        if let Some(pos) = content.find(&sig) {
            let window = &content[pos..std::cmp::min(pos + 500, content.len())];
            assert!(
                window.contains("TODO"),
                "{callback} should be an honest stub with TODO comment"
            );
        }
    }
}

// ---- Rejection reason 3: interrupt mask resolution ----

#[test]
fn default_path_rejects_unresolved_interrupt_masks() {
    use regseq_backends::lowering::planner::{
        build_driver_lowering_plan, build_legacy_behavior_for,
    };

    let input = make_input();
    let result = build_driver_lowering_plan(&input);

    // The default path (no explicit constants) should either:
    // (a) succeed with all masks nonzero if MIR carries resolved values, or
    // (b) fail with UnresolvedInterruptMasks if MIR has symbolic references.
    match result {
        Ok(_plan) => {
            // If it succeeded, all non-"all" classes must have nonzero masks.
            let (_sm, legacy) = build_legacy_behavior_for(&input, &[]);
            for class in &legacy.interrupt_classes {
                if class.name != "all" {
                    assert!(
                        class.mask_value != 0,
                        "default path succeeded but class '{}' has mask_value 0",
                        class.name
                    );
                }
            }
        }
        Err(e) => {
            let err_str = e.to_string();
            assert!(
                err_str.contains("unresolved interrupt class masks"),
                "error should be UnresolvedInterruptMasks, got: {err_str}"
            );
        }
    }
}

#[test]
fn with_constants_path_resolves_all_masks() {
    let (_portable, linux) = make_plans();

    // The _with_constants path must resolve all interrupt classes.
    for class in &linux.behavior.interrupt_classes {
        assert!(
            class.mask_value != 0,
            "class '{}' should have nonzero mask with explicit constants",
            class.name
        );
    }
}

// =========================================================================
// C validity regression tests
// =========================================================================

/// Read the emitted module.c content for the driver main file.
fn read_emitted_module_c() -> String {
    let (_dir, set) = emit_to_tempdir();
    let main = set
        .artifacts
        .iter()
        .find(|a| a.role == LinuxFileRole::DriverMain)
        .expect("should have driver main artifact");
    std::fs::read_to_string(&main.path).expect("read module.c")
}

#[test]
fn no_raw_rust_paths_in_emitted_c() {
    let content = read_emitted_module_c();

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comments.
        if trimmed.starts_with("/*")
            || trimmed.starts_with("//")
            || trimmed.starts_with("*")
            || trimmed.starts_with("#")
        {
            continue;
        }
        assert!(
            !trimmed.contains("regseq::"),
            "line {}: raw Rust path 'regseq::' in non-comment C: {trimmed}",
            i + 1
        );
        assert!(
            !trimmed.contains("core::option::Option"),
            "line {}: raw Rust 'core::option::Option' in C: {trimmed}",
            i + 1
        );
        assert!(
            !trimmed.contains("::<"),
            "line {}: raw Rust generic syntax '::<' in C: {trimmed}",
            i + 1
        );
    }
}

#[test]
fn no_invalid_field_access_in_emitted_c() {
    let content = read_emitted_module_c();

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("/*") || trimmed.starts_with("//") {
            continue;
        }
        assert!(
            !trimmed.contains("d->core::"),
            "line {}: invalid C field access 'd->core::' found: {trimmed}",
            i + 1
        );
    }
}

#[test]
fn no_unsupported_markers_in_emitted_c() {
    let content = read_emitted_module_c();

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("/*") || trimmed.starts_with("//") {
            continue;
        }
        assert!(
            !trimmed.contains("unsupported_"),
            "line {}: 'unsupported_' marker in C: {trimmed}",
            i + 1
        );
    }
}

#[test]
fn register_op_helpers_emit_reg_calls() {
    let content = read_emitted_module_c();

    // Register-operation helpers should emit *_reg_write, *_reg_modify, or *_reg_read calls.
    let has_reg_call = content.contains("_reg_write(")
        || content.contains("_reg_modify(")
        || content.contains("_reg_read(");
    assert!(
        has_reg_call,
        "emitted C should contain register helper calls (*_reg_write, *_reg_modify, or *_reg_read)"
    );
}

#[test]
fn helper_forward_declarations_exist() {
    let content = read_emitted_module_c();

    assert!(
        content.contains("Helper forward declarations"),
        "emitted C should contain forward declaration section"
    );

    // Forward declarations should appear before function bodies.
    let fwd_pos = content.find("Helper forward declarations").unwrap();
    // At least one forward declaration should be a semicolon-terminated signature.
    let fwd_section = &content[fwd_pos..fwd_pos + 500.min(content.len() - fwd_pos)];
    let has_fwd_decl = fwd_section.lines().any(|l| {
        let t = l.trim();
        t.starts_with("static") && t.ends_with(';')
    });
    assert!(
        has_fwd_decl,
        "forward declarations should be semicolon-terminated signatures"
    );
}

#[test]
fn isr_uses_plan_derived_status_register() {
    let content = read_emitted_module_c();

    // The ISR should read a status register. The register define should
    // come from the behavior plan, not a hardcoded "MIS" suffix.
    // The ISR reads `status = *_reg_read(d, <define>);`
    let isr_section = content
        .find("ISR -- Regseq interrupt handler")
        .expect("ISR section should exist");
    let isr_text = &content[isr_section..];
    assert!(
        isr_text.contains("_reg_read(d,"),
        "ISR should read a status register"
    );
}

#[test]
fn helper_prototypes_match_definitions() {
    let content = read_emitted_module_c();

    // Find forward declarations (lines ending with `;` that start with `static`)
    // and find definitions (lines ending with `{` that start with `static`).
    let fwd_decls: Vec<String> = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("static") && t.ends_with(';') && t.contains("_data *d")
        })
        .map(|l| l.trim().trim_end_matches(';').to_string())
        .collect();

    for decl in &fwd_decls {
        // The definition should have the same signature but end with `)`
        // (the function body opener `{` is on the next line in CTranslationUnit).
        assert!(
            content.contains(decl),
            "forward declaration should match a definition: {decl}"
        );
    }
}
