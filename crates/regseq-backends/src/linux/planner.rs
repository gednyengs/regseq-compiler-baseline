//! Linux backend planner.
//!
//! Converts a portable `DriverLoweringPlan` into a `LinuxBackendPlan`.
//! This planner makes Linux-specific realization decisions without
//! re-deriving driver semantics from the raw models.

use regseq_model::declaration::driver::DeviceClass;

use crate::lowering::plan::{BusType, DriverLoweringPlan, ResourceKind, ResourcePolicy};

use super::plan::{
    LinuxAcquisitionEntry, LinuxArtifactLayout, LinuxBackendPlan, LinuxEmittedFile, LinuxFileRole,
    LinuxProbeOrchestration, LinuxProbeStep, LinuxRegistrationShape, LinuxReleaseEntry,
    LinuxRemoveOrchestration, LinuxRemoveStep, LinuxResourceSequencing, LinuxSerialCoreRealization,
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

    Ok(LinuxBackendPlan {
        registration,
        serial_core,
        probe,
        remove,
        resource_acquisition,
        artifact_layout,
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

    Some(LinuxSerialCoreRealization {
        uart_driver_name: format!("{snake_name}_uart_driver"),
        uart_ops_name: format!("{snake_name}_uart_ops"),
        realized_ops: plan.device_class_roles.serial_ops_realized.clone(),
        port_type: format!("PORT_{}", plan.identity.driver_name.to_uppercase()),
    })
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

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
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
