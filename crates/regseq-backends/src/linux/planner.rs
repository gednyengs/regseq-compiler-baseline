//! Linux backend planner.
//!
//! Converts a portable `DriverLoweringPlan` into a `LinuxBackendPlan`.
//! This planner makes Linux-specific realization decisions without
//! re-deriving driver semantics from the raw models.

use regseq_model::compat::{
    DeviceCallbackRole, DeviceClass, DeviceClassKind, FunctionRole, SemanticModel,
};

use crate::lowering::plan::{BusType, DriverLoweringPlan, ResourceKind, ResourcePolicy};

use super::plan::{
    LinuxArtifactLayout, LinuxBackendPlan, LinuxBehaviorMapping, LinuxBitFieldEntry,
    LinuxCallbackGroup, LinuxCallbackMapping, LinuxEmittedFile, LinuxFileRole, LinuxNamedValue,
    LinuxPortType, LinuxProbeOrchestration, LinuxProbeStep, LinuxPropertyRealization,
    LinuxRegisterBlock, LinuxRegisterDescriptor, LinuxRegisterFieldEntry, LinuxRegisterLayout,
    LinuxRegistrationShape, LinuxRemoveOrchestration, LinuxRemoveStep, LinuxSerialCoreRealization,
    LinuxSharedStructField, LinuxUartOpsBody, LinuxUartOpsEntry, ParamBinding, ReturnAdaptation,
};

/// Errors that can occur during Linux backend plan construction.
#[derive(Debug, thiserror::Error)]
pub enum LinuxPlanError {
    /// The portable plan is missing information needed for Linux realization.
    #[error("linux plan error: {reason}")]
    PlanError {
        /// Description of the failure.
        reason: String,
    },
}

/// Build a `LinuxBackendPlan` from a portable `DriverLoweringPlan`.
///
/// This is the Linux backend planner entry point. It translates portable
/// driver obligations into Linux-specific realization decisions. The
/// caller also supplies the projected [`SemanticModel`] that the
/// transitional Linux emitter still string-formats from;
/// `regseq-lowering-10` trimmed that handle off the portable plan, so
/// the Linux plan owns it directly until
/// `regseq-linux-backend-03` (task 8) replaces it with a
/// `FunctionBodyIndex`.
pub fn build_linux_backend_plan(
    plan: &DriverLoweringPlan,
    behavior: SemanticModel,
) -> Result<LinuxBackendPlan, LinuxPlanError> {
    let registration = build_registration(plan);
    let serial_core = build_serial_core(plan);
    let probe = build_probe(plan);
    let remove = build_remove(plan);
    let artifact_layout = build_artifact_layout(plan);
    let register_layout = build_register_layout(plan);
    let property_realization = build_property_realization(plan);
    let behavior_mapping = build_behavior_mapping(plan, &behavior);
    let uart_ops_realization = build_uart_ops_realization(plan);
    let shared_struct_fields = build_shared_struct_fields(plan, serial_core.is_some());
    let has_optional_clock = plan.resources.clocks.iter().any(|c| !c.required);

    Ok(LinuxBackendPlan {
        registration,
        serial_core,
        probe,
        remove,
        artifact_layout,
        register_layout,
        property_realization,
        behavior_mapping,
        uart_ops_realization,
        behavior,
        driver_name: plan.identity.driver_name.clone(),
        shared_struct_fields,
        has_optional_clock,
    })
}

/// Build the driver-state struct field layout from portable resource
/// obligations.
///
/// Mirrors the Linux C types previously assembled inline in
/// `emitter::emit_shared_header`. Pre-computing here lets the emitter
/// take only `&LinuxBackendPlan`.
fn build_shared_struct_fields(
    plan: &DriverLoweringPlan,
    has_serial_core: bool,
) -> Vec<LinuxSharedStructField> {
    let mut fields = Vec::new();

    for mmio in &plan.resources.mmio {
        fields.push(LinuxSharedStructField {
            c_type: "void __iomem *".into(),
            field_name: mmio.field_name.clone(),
            doc: format!("MMIO base for {}", mmio.mmio_id),
        });
    }
    for irq in &plan.resources.interrupts {
        fields.push(LinuxSharedStructField {
            c_type: "int".into(),
            field_name: irq.field_name.clone(),
            doc: format!("IRQ number for {}", irq.interrupt_id),
        });
    }
    for clk in &plan.resources.clocks {
        fields.push(LinuxSharedStructField {
            c_type: "struct clk *".into(),
            field_name: clk.field_name.clone(),
            doc: format!("Clock {}", clk.clock_id),
        });
    }
    for prop in &plan.resources.properties {
        let c_type = match prop.field_type.as_str() {
            "u32" => "u32",
            "u16" => "u16",
            "u8" => "u8",
            "bool" => "bool",
            _ => "u32",
        };
        fields.push(LinuxSharedStructField {
            c_type: c_type.into(),
            field_name: prop.field_name.clone(),
            doc: format!("Property: {}", prop.property_id),
        });
    }
    if has_serial_core {
        fields.push(LinuxSharedStructField {
            c_type: "struct uart_port".into(),
            field_name: "port".into(),
            doc: "Serial core port".into(),
        });
    }

    fields
}

fn build_registration(plan: &DriverLoweringPlan) -> LinuxRegistrationShape {
    let snake_name = to_snake_case(&plan.identity.driver_name);
    let is_amba = plan.resources.bus_identity.bus_type == BusType::Amba;

    let amba_id = plan.resources.bus_identity.amba_id;
    let amba_mask = amba_id.map(|_| 0x000f_ffff_u32);

    LinuxRegistrationShape {
        driver_struct_name: format!("{snake_name}_driver"),
        match_table_name: format!("{snake_name}_of_match"),
        compatible_entries: plan.identity.compatible.clone(),
        module_name: snake_name,
        module_license: "GPL".into(),
        is_amba,
        amba_id,
        amba_mask,
    }
}

fn build_serial_core(plan: &DriverLoweringPlan) -> Option<LinuxSerialCoreRealization> {
    if plan.identity.device_class != DeviceClass::SerialUart {
        return None;
    }

    let snake_name = to_snake_case(&plan.identity.driver_name);
    let upper_name = snake_name.to_uppercase();

    // Port type: symbolic name from driver identity, deterministic numeric
    // value from a stable hash of the driver name in the out-of-tree range.
    let port_type = LinuxPortType {
        define_name: format!("PORT_{upper_name}"),
        value: compute_port_type_value(&plan.identity.driver_name),
    };

    // Device name: explicit backend naming policy for serial UART devices.
    // Uses "tty" + abbreviated module name — this is a backend policy
    // decision, not derived from device-specific identifiers.
    let dev_name = serial_dev_name_policy(&snake_name);

    Some(LinuxSerialCoreRealization {
        uart_driver_name: format!("{snake_name}_uart_driver"),
        uart_ops_name: format!("{snake_name}_uart_ops"),
        realized_ops: plan.device_class_roles.serial_ops_realized.clone(),
        port_type,
        dev_name,
    })
}

/// Compute a deterministic port-type numeric value for out-of-tree modules.
///
/// Uses a simple stable hash of the driver name, mapped into the
/// 100..199 range reserved for out-of-tree serial port types.
fn compute_port_type_value(driver_name: &str) -> u32 {
    let hash = driver_name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    100 + (hash % 100)
}

/// Backend naming policy for serial TTY device names.
///
/// Produces the `dev_name` for `uart_driver` based on the module name.
/// This is a backend-level policy, not a device-specific derivation.
fn serial_dev_name_policy(module_name: &str) -> String {
    // Abbreviate by taking the first character of each underscore-separated
    // segment, prefixed with "tty".
    let abbrev: String = module_name
        .split('_')
        .filter_map(|seg| seg.chars().next())
        .collect();
    format!("tty{abbrev}")
}

fn build_probe(plan: &DriverLoweringPlan) -> LinuxProbeOrchestration {
    let mut steps = Vec::new();

    for step in &plan.acquisition_order {
        match step.resource_kind {
            ResourceKind::Clock => {
                steps.push(LinuxProbeStep::GetClock {
                    clock_id: step.resource_id.clone(),
                    optional: step.policy != ResourcePolicy::Required,
                });
            }
            ResourceKind::Reset => {
                steps.push(LinuxProbeStep::DeassertReset {
                    field_name: step.resource_id.clone(),
                    optional: step.policy != ResourcePolicy::Required,
                });
            }
            ResourceKind::PinCtrl => {
                steps.push(LinuxProbeStep::SelectPinCtrl {
                    field_name: step.resource_id.clone(),
                    optional: step.policy != ResourcePolicy::Required,
                });
            }
            ResourceKind::Mmio => {
                steps.push(LinuxProbeStep::MapMmio {
                    mmio_id: step.resource_id.clone(),
                });
            }
            ResourceKind::Property => {
                let has_default = plan
                    .resources
                    .properties
                    .iter()
                    .any(|p| p.property_id == step.resource_id && p.default_literal.is_some());
                steps.push(LinuxProbeStep::ReadProperty {
                    property_id: step.resource_id.clone(),
                    has_default,
                });
            }
            ResourceKind::Interrupt => {
                steps.push(LinuxProbeStep::RequestIrq {
                    interrupt_id: step.resource_id.clone(),
                });
            }
        }
    }

    if plan.identity.device_class == DeviceClass::SerialUart {
        steps.push(LinuxProbeStep::RegisterUartPort);
    }

    LinuxProbeOrchestration { steps }
}

fn build_remove(plan: &DriverLoweringPlan) -> LinuxRemoveOrchestration {
    let mut steps = Vec::new();

    if plan.identity.device_class == DeviceClass::SerialUart {
        steps.push(LinuxRemoveStep::UnregisterUartPort);
    }

    for step in &plan.teardown_order {
        match step.resource_kind {
            ResourceKind::Interrupt => {
                steps.push(LinuxRemoveStep::FreeIrq {
                    interrupt_id: step.resource_id.clone(),
                });
            }
            ResourceKind::Clock => {
                steps.push(LinuxRemoveStep::PutClock {
                    clock_id: step.resource_id.clone(),
                });
            }
            ResourceKind::Reset => {
                steps.push(LinuxRemoveStep::AssertReset {
                    field_name: step.resource_id.clone(),
                });
            }
            ResourceKind::Mmio => {
                steps.push(LinuxRemoveStep::UnmapMmio {
                    mmio_id: step.resource_id.clone(),
                });
            }
            ResourceKind::PinCtrl | ResourceKind::Property => {
                // No explicit Linux remove step for these.
            }
        }
    }

    LinuxRemoveOrchestration { steps }
}

fn build_artifact_layout(plan: &DriverLoweringPlan) -> LinuxArtifactLayout {
    let snake_name = to_snake_case(&plan.identity.driver_name);

    let mut files = vec![
        LinuxEmittedFile {
            filename: format!("regseq_{snake_name}_module.c"),
            role: LinuxFileRole::DriverMain,
        },
        LinuxEmittedFile {
            filename: format!("regseq_{snake_name}_shared.h"),
            role: LinuxFileRole::SharedHeader,
        },
        LinuxEmittedFile {
            filename: "Makefile".into(),
            role: LinuxFileRole::Makefile,
        },
        LinuxEmittedFile {
            filename: "Kbuild".into(),
            role: LinuxFileRole::KbuildFragment,
        },
    ];

    if !plan.register_obligations.blocks.is_empty() {
        files.push(LinuxEmittedFile {
            filename: format!("{snake_name}_regs.h"),
            role: LinuxFileRole::RegisterHelpers,
        });
    }

    LinuxArtifactLayout {
        emitted_files: files,
    }
}

/// Device-class callback roles that have DirectBody/WrappedBody realization
/// in uart_ops (not emitted as helpers).
const UART_OPS_REALIZED: &[DeviceCallbackRole] = &[
    DeviceCallbackRole::PortOpen,
    DeviceCallbackRole::PortClose,
    DeviceCallbackRole::Configure,
    DeviceCallbackRole::TxEmpty,
    DeviceCallbackRole::SendBreak,
];

fn build_behavior_mapping(
    plan: &DriverLoweringPlan,
    behavior: &SemanticModel,
) -> LinuxBehaviorMapping {
    let snake_name = to_snake_case(&plan.identity.driver_name);

    let callback_mappings = behavior
        .functions
        .iter()
        .map(|f| {
            let linux_callback = format!("{snake_name}_{}", f.name);
            let group = match &f.role {
                FunctionRole::Lifecycle { .. } => LinuxCallbackGroup::ProbeRemove,
                FunctionRole::DeviceClassCallback { callback, .. } => {
                    if UART_OPS_REALIZED.contains(callback) {
                        LinuxCallbackGroup::UartOps
                    } else {
                        LinuxCallbackGroup::Helper
                    }
                }
                FunctionRole::Isr { .. } => LinuxCallbackGroup::IsrHandler,
                FunctionRole::Helper => LinuxCallbackGroup::Helper,
            };

            LinuxCallbackMapping {
                portable_function: f.name.clone(),
                linux_callback,
                group,
            }
        })
        .collect();

    LinuxBehaviorMapping {
        callback_mappings,
        portable_function_count: behavior.functions.len(),
        interrupt_class_count: behavior.interrupt_classes.len(),
    }
}

fn build_register_layout(plan: &DriverLoweringPlan) -> LinuxRegisterLayout {
    let blocks = plan
        .register_obligations
        .blocks
        .iter()
        .map(|b| LinuxRegisterBlock {
            block_name: b.name.clone(),
            reg_prefix: derive_reg_prefix(&b.name),
            fields: b
                .register_fields
                .iter()
                .map(|f| LinuxRegisterFieldEntry {
                    name: f.name.clone(),
                    byte_offset: f.byte_offset,
                    access_width_bytes: f.access_width.unwrap_or(32) / 8,
                })
                .collect(),
        })
        .collect();

    let descriptors = plan
        .register_obligations
        .descriptors
        .iter()
        .map(|d| LinuxRegisterDescriptor {
            register_name: d.register_name.clone(),
            bit_fields: d
                .bit_fields
                .iter()
                .map(|f| LinuxBitFieldEntry {
                    name: f.name.clone(),
                    bit_offset: f.offset,
                    bit_width: f.width,
                    named_values: f
                        .named_values
                        .iter()
                        .map(|nv| LinuxNamedValue {
                            name: nv.name.clone(),
                            value: nv.value,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    LinuxRegisterLayout {
        blocks,
        descriptors,
    }
}

fn build_property_realization(plan: &DriverLoweringPlan) -> Vec<LinuxPropertyRealization> {
    plan.resources
        .properties
        .iter()
        .map(|p| {
            let linux_read_api = match p.field_type.as_str() {
                "u8" => "device_property_read_u8",
                "u16" => "device_property_read_u16",
                "u32" => "device_property_read_u32",
                "u64" => "device_property_read_u64",
                "bool" => "device_property_read_bool",
                _ => "device_property_read_u32",
            };
            LinuxPropertyRealization {
                property_id: p.property_id.clone(),
                field_name: p.field_name.clone(),
                c_type: p.field_type.clone(),
                required: p.required,
                default_literal: p.default_literal.clone(),
                linux_read_api: linux_read_api.into(),
            }
        })
        .collect()
}

fn build_uart_ops_realization(plan: &DriverLoweringPlan) -> Vec<LinuxUartOpsEntry> {
    if plan.identity.device_class != DeviceClass::SerialUart {
        return vec![];
    }

    let snake = to_snake_case(&plan.identity.driver_name);

    vec![
        LinuxUartOpsEntry {
            op_name: "tx_empty".into(),
            signature: format!(
                "static unsigned int {snake}_tx_empty(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::WrappedBody {
                role: FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::TxEmpty,
                },
                param_bindings: vec![],
                return_adaptation: Some(ReturnAdaptation::BoolToUint {
                    true_value: "TIOCSER_TEMT".into(),
                    false_value: "0".into(),
                }),
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "set_mctrl".into(),
            signature: format!(
                "static void {snake}_set_mctrl(struct uart_port *port, unsigned int mctrl)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec!["/* Modem control not realized in first slice */".into()],
            },
            needs_container_of: false,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "get_mctrl".into(),
            signature: format!(
                "static unsigned int {snake}_get_mctrl(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec!["return TIOCM_CAR | TIOCM_CTS | TIOCM_DSR;".into()],
            },
            needs_container_of: false,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "stop_tx".into(),
            signature: format!(
                "static void {snake}_stop_tx(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec![
                    "/* TODO: TX disable not yet realized from lowered plan */".into(),
                ],
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "start_tx".into(),
            signature: format!(
                "static void {snake}_start_tx(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec![
                    "/* TODO: TX enable not yet realized from lowered plan */".into(),
                ],
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "stop_rx".into(),
            signature: format!(
                "static void {snake}_stop_rx(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec![
                    "/* TODO: RX disable not yet realized from lowered plan */".into(),
                ],
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "break_ctl".into(),
            signature: format!(
                "static void {snake}_break_ctl(struct uart_port *port, int break_state)"
            ),
            body_source: LinuxUartOpsBody::WrappedBody {
                role: FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::SendBreak,
                },
                param_bindings: vec![ParamBinding {
                    semantic_name: "on".into(),
                    c_type: "bool".into(),
                    c_expr: "break_state != 0".into(),
                }],
                return_adaptation: None,
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "startup".into(),
            signature: format!(
                "static int {snake}_startup(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::WrappedBody {
                role: FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::PortOpen,
                },
                param_bindings: vec![],
                return_adaptation: Some(ReturnAdaptation::ResultToInt),
            },
            needs_container_of: true,
            trailing: vec!["return 0;".into()],
        },
        LinuxUartOpsEntry {
            op_name: "shutdown".into(),
            signature: format!(
                "static void {snake}_shutdown(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::DirectBody {
                role: FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::PortClose,
                },
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "set_termios".into(),
            signature: format!(
                "static void {snake}_set_termios(struct uart_port *port, struct ktermios *termios, const struct ktermios *old)"
            ),
            body_source: LinuxUartOpsBody::WrappedBody {
                role: FunctionRole::DeviceClassCallback {
                    class: DeviceClassKind::Serial,
                    callback: DeviceCallbackRole::Configure,
                },
                param_bindings: vec![ParamBinding {
                    semantic_name: "_config".into(),
                    c_type: "const struct ktermios *".into(),
                    c_expr: "termios".into(),
                }],
                return_adaptation: Some(ReturnAdaptation::DropResult),
            },
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "type".into(),
            signature: format!(
                "static const char *{snake}_type(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec![format!("return \"{snake}\";")],
            },
            needs_container_of: false,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "config_port".into(),
            signature: format!(
                "static void {snake}_config_port(struct uart_port *port, int flags)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec![
                    "if (flags & UART_CONFIG_TYPE)".into(),
                    format!(
                        "\tport->type = PORT_{};",
                        to_snake_case(&plan.identity.driver_name).to_uppercase()
                    ),
                ],
            },
            needs_container_of: false,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "request_port".into(),
            signature: format!(
                "static int {snake}_request_port(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub {
                lines: vec!["return 0;".into()],
            },
            needs_container_of: false,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "release_port".into(),
            signature: format!(
                "static void {snake}_release_port(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::Stub { lines: vec![] },
            needs_container_of: false,
            trailing: vec![],
        },
    ]
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch == '-' {
            result.push('_');
        } else if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Derive a C identifier prefix from a register block name.
///
/// This is a formatting policy for generated C defines, not a semantic
/// fact. Strips the `"Registers"` suffix and converts to SCREAMING_SNAKE.
///
/// Examples:
/// - `"FooRegisters"` → `"FOO"`
/// - `"E1000Registers"` → `"E1000"`
/// - `"FooBarRegs"` → `"FOO_BAR_REGS"`
fn derive_reg_prefix(block_name: &str) -> String {
    let base = block_name.strip_suffix("Registers").unwrap_or(block_name);
    to_screaming_snake(base)
}

/// Convert a CamelCase identifier to SCREAMING_SNAKE_CASE.
///
/// Runs of uppercase characters followed by a lowercase character are
/// treated as acronyms (e.g., "FooBar" → "FOO_BAR").
fn to_screaming_snake(name: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            // Insert underscore before:
            // - uppercase after lowercase: "fooB" → "FOO_B"
            // - uppercase before lowercase when preceded by uppercase: "FOOBar" → "FOO_BAR"
            if prev.is_lowercase()
                || (prev.is_uppercase() && i + 1 < chars.len() && chars[i + 1].is_lowercase())
            {
                result.push('_');
            }
        }
        result.push(ch.to_uppercase().next().unwrap());
    }
    result
}
