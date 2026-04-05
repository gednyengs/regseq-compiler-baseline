//! Linux backend planner.
//!
//! Converts a portable `DriverLoweringPlan` into a `LinuxBackendPlan`.
//! This planner makes Linux-specific realization decisions without
//! re-deriving driver semantics from the raw models.

use regseq_model::declaration::driver::DeviceClass;

use crate::lowering::behavior::FunctionRole;
use crate::lowering::plan::{BusType, DriverLoweringPlan, ResourceKind, ResourcePolicy};

use super::plan::{
    LinuxAcquisitionEntry, LinuxArtifactLayout, LinuxBackendPlan, LinuxBehaviorMapping,
    LinuxBitFieldEntry, LinuxCallbackGroup, LinuxCallbackMapping, LinuxClockEntry,
    LinuxDeploymentMetadata, LinuxEmittedFile, LinuxFileRole, LinuxNamedValue, LinuxPinCtrlEntry,
    LinuxPortType, LinuxProbeOrchestration, LinuxProbeStep, LinuxPropertyRealization,
    LinuxRegisterBlock, LinuxRegisterDescriptor, LinuxRegisterFieldEntry, LinuxRegisterLayout,
    LinuxRegistrationShape, LinuxReleaseEntry, LinuxRemoveOrchestration, LinuxRemoveStep,
    LinuxResetEntry, LinuxResourceSequencing, LinuxSerialCoreRealization, LinuxUartOpsBody,
    LinuxUartOpsEntry,
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
/// driver obligations into Linux-specific realization decisions.
pub fn build_linux_backend_plan(
    plan: &DriverLoweringPlan,
) -> Result<LinuxBackendPlan, LinuxPlanError> {
    let registration = build_registration(plan);
    let serial_core = build_serial_core(plan);
    let probe = build_probe(plan);
    let remove = build_remove(plan);
    let resource_acquisition = build_resource_sequencing(plan);
    let artifact_layout = build_artifact_layout(plan);
    let register_layout = build_register_layout(plan);
    let property_realization = build_property_realization(plan);
    let deployment_metadata = build_deployment_metadata(plan);
    let behavior_mapping = build_behavior_mapping(plan);
    let uart_ops_realization = build_uart_ops_realization(plan);

    Ok(LinuxBackendPlan {
        registration,
        serial_core,
        probe,
        remove,
        resource_acquisition,
        artifact_layout,
        register_layout,
        property_realization,
        deployment_metadata,
        behavior_mapping,
        uart_ops_realization,
    })
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

fn build_resource_sequencing(plan: &DriverLoweringPlan) -> LinuxResourceSequencing {
    let acquisition_order: Vec<LinuxAcquisitionEntry> = plan
        .acquisition_order
        .iter()
        .map(|step| {
            let api_family = match step.resource_kind {
                ResourceKind::Mmio => "devm_ioremap_resource",
                ResourceKind::Interrupt => "devm_request_irq",
                ResourceKind::Clock => "devm_clk_get",
                ResourceKind::Reset => "devm_reset_control_get",
                ResourceKind::PinCtrl => "devm_pinctrl_get",
                ResourceKind::Property => "device_property_read",
            };
            LinuxAcquisitionEntry {
                order: step.order,
                api_family: api_family.into(),
                resource_id: step.resource_id.clone(),
                required: step.policy == ResourcePolicy::Required,
            }
        })
        .collect();

    let release_order: Vec<LinuxReleaseEntry> = plan
        .teardown_order
        .iter()
        .filter_map(|step| {
            let api_family = match step.resource_kind {
                ResourceKind::Clock => Some("clk_disable_unprepare"),
                ResourceKind::Reset => Some("reset_control_assert"),
                ResourceKind::Interrupt => Some("free_irq"),
                ResourceKind::Mmio => Some("iounmap"),
                ResourceKind::PinCtrl | ResourceKind::Property => None,
            };
            api_family.map(|api| LinuxReleaseEntry {
                order: step.order,
                api_family: api.into(),
                resource_id: step.resource_id.clone(),
            })
        })
        .collect();

    LinuxResourceSequencing {
        acquisition_order,
        release_order,
    }
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

fn build_behavior_mapping(plan: &DriverLoweringPlan) -> LinuxBehaviorMapping {
    let snake_name = to_snake_case(&plan.identity.driver_name);

    let callback_mappings = plan
        .behavior
        .functions
        .iter()
        .map(|f| {
            let (linux_callback, group) = match f.role {
                FunctionRole::LifecycleInit => (
                    format!("{snake_name}_probe"),
                    LinuxCallbackGroup::ProbeRemove,
                ),
                FunctionRole::LifecycleDeinit => (
                    format!("{snake_name}_remove"),
                    LinuxCallbackGroup::ProbeRemove,
                ),
                FunctionRole::SerialPortOpen => {
                    (format!("{snake_name}_startup"), LinuxCallbackGroup::UartOps)
                }
                FunctionRole::SerialPortClose => (
                    format!("{snake_name}_shutdown"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialConfigure => (
                    format!("{snake_name}_set_termios"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialTxByte => (
                    format!("{snake_name}_start_tx"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialRxChar => (
                    format!("{snake_name}_rx_chars"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialTxReady => (
                    format!("{snake_name}_tx_ready"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialTxEmpty => (
                    format!("{snake_name}_tx_empty"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialFlushRxFifo => (
                    format!("{snake_name}_flush_buffer"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialFlushTxFifo => (
                    format!("{snake_name}_flush_buffer"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::SerialSendBreak => (
                    format!("{snake_name}_break_ctl"),
                    LinuxCallbackGroup::UartOps,
                ),
                FunctionRole::Isr => (
                    format!("{snake_name}_irq_handler"),
                    LinuxCallbackGroup::IsrHandler,
                ),
                FunctionRole::Helper => (f.name.clone(), LinuxCallbackGroup::Helper),
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
        portable_function_count: plan.behavior.functions.len(),
        interrupt_class_count: plan.behavior.interrupt_classes.len(),
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

fn build_deployment_metadata(plan: &DriverLoweringPlan) -> LinuxDeploymentMetadata {
    let clocks = plan
        .resources
        .clocks
        .iter()
        .map(|c| LinuxClockEntry {
            clock_id: c.clock_id.clone(),
            required: c.required,
            default_frequency_hz: c.default_frequency,
        })
        .collect();

    let resets = plan
        .resources
        .resets
        .iter()
        .map(|r| LinuxResetEntry {
            field_name: r.field_name.clone(),
            required: r.required,
            reset_line_id: r.reset_line_id.clone(),
            shared: r.reset_shared,
        })
        .collect();

    let pinctrl = plan
        .resources
        .pinctrl
        .iter()
        .map(|p| LinuxPinCtrlEntry {
            field_name: p.field_name.clone(),
            required: p.required,
            states: p.pin_states.clone(),
        })
        .collect();

    LinuxDeploymentMetadata {
        clocks,
        resets,
        pinctrl,
    }
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
            body_source: LinuxUartOpsBody::FromPortableRole(FunctionRole::SerialTxEmpty),
            needs_container_of: true,
            trailing: vec!["return 0;".into()],
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
            body_source: LinuxUartOpsBody::FromPortableRole(FunctionRole::SerialSendBreak),
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "startup".into(),
            signature: format!(
                "static int {snake}_startup(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::FromPortableRole(FunctionRole::SerialPortOpen),
            needs_container_of: true,
            trailing: vec!["return 0;".into()],
        },
        LinuxUartOpsEntry {
            op_name: "shutdown".into(),
            signature: format!(
                "static void {snake}_shutdown(struct uart_port *port)"
            ),
            body_source: LinuxUartOpsBody::FromPortableRole(FunctionRole::SerialPortClose),
            needs_container_of: true,
            trailing: vec![],
        },
        LinuxUartOpsEntry {
            op_name: "set_termios".into(),
            signature: format!(
                "static void {snake}_set_termios(struct uart_port *port, struct ktermios *termios, const struct ktermios *old)"
            ),
            body_source: LinuxUartOpsBody::FromPortableRole(FunctionRole::SerialConfigure),
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
