//! Portable lowering planner.
//!
//! Converts an `AnalysisInput` into a `DriverLoweringPlan`. The canonical
//! entry point is `build_driver_lowering_plan_from_hierarchy`, which
//! projects a real `SemanticModel` from a populated item hierarchy.
//!
//! The older `build_driver_lowering_plan` / `build_driver_lowering_plan_with_constants`
//! signatures are retained for workspace compilation with `regseq-driver`
//! (out of scope for the Phase 2C migration). They are not part of the
//! semantic-model migration and should not be used for new work.

use std::collections::BTreeSet;

use regseq_analysis::AnalysisInput;
use regseq_model::declaration::callbacks::CallbackFamily;
use regseq_model::item::identity::ItemId;
use regseq_model::semantic::SemanticModel;

use super::behavior_planner::build_behavior_plan_with_constants;
use super::hierarchy::ItemHierarchy;
use super::plan::{
    AcquisitionStep, BitFieldInfo, BusIdentity, BusType, CallbackBodyPlan, CallbackEntry,
    CallbackInventory, ClockObligation, CompletionObligation, DeviceClassRoles, DriverIdentity,
    DriverLoweringPlan, InterruptObligation, LockScopeObligation, LoweringAction, MmioObligation,
    NamedValueInfo, PinCtrlObligation, PollObligation, PropertyObligation,
    RegisterAccessObligation, RegisterBlockObligation, RegisterDescriptorInfo, RegisterFieldInfo,
    RegisterObligations, ResetObligation, ResourceKind, ResourceObligations, ResourcePolicy,
    SyncFieldObligation, SynchronizationObligations, TeardownStep,
};
use super::projection::project_semantic_model;

/// Errors that can occur during portable lowering plan construction.
#[derive(Debug, thiserror::Error)]
pub enum LoweringPlanError {
    /// The declaration model does not carry an explicit portable bus identity.
    #[error("missing explicit bus identity for driver '{driver_name}'")]
    MissingBusIdentity {
        /// Driver name missing its explicit bus declaration.
        driver_name: String,
    },
    /// A required resource is missing from the joined artifact.
    #[error("missing required resource: {family} '{id}'")]
    MissingResource {
        /// Resource family name.
        family: String,
        /// Resource identifier.
        id: String,
    },
    /// Plan construction failed for a structural reason.
    #[error("plan construction failed: {reason}")]
    PlanError {
        /// Description of the failure.
        reason: String,
    },
    /// Interrupt class mask values could not be resolved from the MIR body.
    ///
    /// The default lowering path requires that interrupt mask constants are
    /// either embedded as scalar values in MIR or resolvable from the
    /// available MIR body surface. When they remain as unevaluated symbolic
    /// references, use `build_driver_lowering_plan_with_constants` instead.
    #[error("unresolved interrupt class masks: {}", class_names.join(", "))]
    UnresolvedInterruptMasks {
        /// Class names whose mask values are still zero.
        class_names: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Canonical entry point (Phase 2C semantic-model path)
// ---------------------------------------------------------------------------

/// Build a `DriverLoweringPlan` from analysis input and a populated item
/// hierarchy.
///
/// This is the canonical plan builder. `SemanticModel` is projected from
/// the hierarchy and becomes `plan.behavior`. The legacy `BehaviorPlan`
/// is built alongside into `plan.legacy_behavior` for transitional Linux
/// emitter compatibility.
pub fn build_driver_lowering_plan_from_hierarchy(
    input: &AnalysisInput,
    hierarchy: &ItemHierarchy,
    lowered_functions: &BTreeSet<ItemId>,
    constant_values: &[(String, u32)],
) -> Result<DriverLoweringPlan, LoweringPlanError> {
    let semantic_model = project_semantic_model(hierarchy, lowered_functions);

    // Verify all interrupt classes have resolved mask values.
    let unresolved: Vec<String> = semantic_model
        .interrupt_classes
        .iter()
        .filter(|c| c.mask_value == 0 && c.name != "all")
        .map(|c| c.name.clone())
        .collect();
    if !unresolved.is_empty() {
        return Err(LoweringPlanError::UnresolvedInterruptMasks {
            class_names: unresolved,
        });
    }

    let legacy_behavior = build_behavior_plan_with_constants(input, constant_values);
    let base = build_plan_base(input)?;

    Ok(DriverLoweringPlan {
        identity: base.identity,
        callbacks: base.callbacks,
        resources: base.resources,
        acquisition_order: base.acquisition_order,
        teardown_order: base.teardown_order,
        device_class_roles: base.device_class_roles,
        register_obligations: base.register_obligations,
        synchronization: base.synchronization,
        callback_actions: base.callback_actions,
        behavior: semantic_model,
        legacy_behavior,
    })
}

// ---------------------------------------------------------------------------
// Retained compat shims (workspace compilation with regseq-driver)
// ---------------------------------------------------------------------------

/// Retained for workspace compilation with `regseq-driver`.
///
/// Not part of the semantic-model migration. Use
/// `build_driver_lowering_plan_from_hierarchy` for the canonical path.
pub fn build_driver_lowering_plan(
    input: &AnalysisInput,
) -> Result<DriverLoweringPlan, LoweringPlanError> {
    let plan = build_driver_lowering_plan_with_constants(input, &[])?;

    // Verify all interrupt classes have resolved mask values.
    let unresolved: Vec<String> = plan
        .legacy_behavior
        .interrupt_classes
        .iter()
        .filter(|c| c.mask_value == 0 && c.name != "all")
        .map(|c| c.name.clone())
        .collect();
    if !unresolved.is_empty() {
        return Err(LoweringPlanError::UnresolvedInterruptMasks {
            class_names: unresolved,
        });
    }

    Ok(plan)
}

/// Retained for workspace compilation with `regseq-driver`.
///
/// Not part of the semantic-model migration. Use
/// `build_driver_lowering_plan_from_hierarchy` for the canonical path.
pub fn build_driver_lowering_plan_with_constants(
    input: &AnalysisInput,
    constant_values: &[(String, u32)],
) -> Result<DriverLoweringPlan, LoweringPlanError> {
    let legacy_behavior = build_behavior_plan_with_constants(input, constant_values);
    let base = build_plan_base(input)?;

    Ok(DriverLoweringPlan {
        identity: base.identity,
        callbacks: base.callbacks,
        resources: base.resources,
        acquisition_order: base.acquisition_order,
        teardown_order: base.teardown_order,
        device_class_roles: base.device_class_roles,
        register_obligations: base.register_obligations,
        synchronization: base.synchronization,
        callback_actions: base.callback_actions,
        behavior: SemanticModel {
            functions: Vec::new(),
            constants: Vec::new(),
            interrupt_classes: Vec::new(),
        },
        legacy_behavior,
    })
}

// ---------------------------------------------------------------------------
// Shared plan-construction helper
// ---------------------------------------------------------------------------

/// Non-behavior fields of a `DriverLoweringPlan`, built from `AnalysisInput`.
struct PlanBase {
    identity: DriverIdentity,
    callbacks: CallbackInventory,
    resources: ResourceObligations,
    acquisition_order: Vec<AcquisitionStep>,
    teardown_order: Vec<TeardownStep>,
    device_class_roles: DeviceClassRoles,
    register_obligations: RegisterObligations,
    synchronization: SynchronizationObligations,
    callback_actions: Vec<CallbackBodyPlan>,
}

fn build_plan_base(input: &AnalysisInput) -> Result<PlanBase, LoweringPlanError> {
    let identity = build_identity(input);
    let callbacks = build_callback_inventory(input);
    let resources = build_resource_obligations(input)?;
    let acquisition_order = build_acquisition_order(&resources);
    let teardown_order = build_teardown_order(&acquisition_order);
    let device_class_roles = build_device_class_roles(input);
    let register_obligations = build_register_obligations(input);
    let synchronization = build_sync_obligations(input);
    let callback_actions = build_callback_actions(input);

    Ok(PlanBase {
        identity,
        callbacks,
        resources,
        acquisition_order,
        teardown_order,
        device_class_roles,
        register_obligations,
        synchronization,
        callback_actions,
    })
}

fn build_identity(input: &AnalysisInput) -> DriverIdentity {
    let driver = &input.declaration().driver;

    let mut source_modules = Vec::new();
    if let Some(ref sf) = driver.provenance.source_file {
        source_modules.push(sf.clone());
    }

    DriverIdentity {
        driver_name: driver.driver_name.clone(),
        compatible: driver.compatible.clone(),
        device_class: driver.device_class.clone(),
        source_modules,
        provenance: driver.provenance.clone(),
    }
}

fn build_callback_inventory(input: &AnalysisInput) -> CallbackInventory {
    let cb = input.callbacks();

    let map_entries = |impls: &[regseq_model::declaration::callbacks::CallbackImplDecl]| {
        impls
            .iter()
            .map(|i| CallbackEntry {
                driver_type: i.driver_type.clone(),
                context: i.context,
                family: i.family,
                provenance: i.provenance.clone(),
            })
            .collect()
    };

    CallbackInventory {
        lifecycle: map_entries(&cb.lifecycle_impls),
        serial: map_entries(&cb.serial_impls),
        serial_console: map_entries(&cb.serial_console_impls),
        power_management: map_entries(&cb.power_management_impls),
    }
}

fn build_resource_obligations(
    input: &AnalysisInput,
) -> Result<ResourceObligations, LoweringPlanError> {
    let decl = input.declaration();
    let joined = input.joined();

    let mmio = decl
        .resources
        .mmio
        .iter()
        .map(|m| {
            let mmio_id = m.mmio_id.clone().unwrap_or_else(|| m.field_name.clone());
            MmioObligation {
                field_name: m.field_name.clone(),
                mmio_id,
                register_block_type: m.register_block_type.clone(),
                policy: ResourcePolicy::Required,
                provenance: m.provenance.clone(),
            }
        })
        .collect();

    let interrupts = decl
        .resources
        .interrupts
        .resources
        .iter()
        .map(|i| {
            let trigger_type = joined
                .interrupts_joined
                .iter()
                .find(|j| j.interrupt_id == i.interrupt_id)
                .and_then(|j| j.trigger_type.clone());
            let shared = joined
                .interrupts_joined
                .iter()
                .find(|j| j.interrupt_id == i.interrupt_id)
                .and_then(|j| j.shared);
            InterruptObligation {
                field_name: i.field_name.clone(),
                interrupt_id: i.interrupt_id.clone(),
                trigger_type,
                shared,
                policy: ResourcePolicy::Required,
                provenance: i.provenance.clone(),
            }
        })
        .collect();

    let properties = decl
        .properties
        .fields
        .iter()
        .map(|p| {
            let policy = if p.required && p.default_literal.is_some() {
                ResourcePolicy::OptionalWithFallback
            } else if p.required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            PropertyObligation {
                property_id: p.property_id.clone(),
                field_name: p.field_name.clone(),
                field_type: p.field_type.clone(),
                required: p.required,
                default_literal: p.default_literal.clone(),
                policy,
                provenance: p.provenance.clone(),
            }
        })
        .collect();

    let clocks = decl
        .resources
        .clock
        .fields
        .iter()
        .map(|c| {
            let default_frequency = joined
                .clocks_joined
                .iter()
                .find(|j| j.clock_id == c.clock_id)
                .and_then(|j| j.default_frequency);
            let policy = if c.required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            ClockObligation {
                field_name: c.field_name.clone(),
                clock_id: c.clock_id.clone(),
                required: c.required,
                default_frequency,
                policy,
                provenance: c.provenance.clone(),
            }
        })
        .collect();

    let resets = decl
        .resources
        .reset
        .fields
        .iter()
        .map(|r| {
            let joined_reset = joined
                .resets_joined
                .iter()
                .find(|j| j.field_name == r.field_name);
            let reset_line_id = joined_reset.and_then(|j| j.reset_line_id.clone());
            let reset_shared = joined_reset.and_then(|j| j.reset_shared);
            let policy = if r.required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            ResetObligation {
                field_name: r.field_name.clone(),
                required: r.required,
                reset_line_id,
                reset_shared,
                policy,
                provenance: r.provenance.clone(),
            }
        })
        .collect();

    let pinctrl = decl
        .resources
        .pinctrl
        .fields
        .iter()
        .map(|p| {
            let joined_pin = joined.pinctrl_joined.iter().find(|j| {
                j.field_name == p.field_name
                    || p.pinctrl_id.as_ref().is_some_and(|id| j.field_name == *id)
            });
            let pin_states = joined_pin.and_then(|j| j.pin_states.clone());
            let policy = if p.required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            PinCtrlObligation {
                field_name: p.field_name.clone(),
                required: p.required,
                pinctrl_id: p.pinctrl_id.clone(),
                pin_states,
                policy,
                provenance: p.provenance.clone(),
            }
        })
        .collect();

    let bus_identity = build_bus_identity(input)?;

    Ok(ResourceObligations {
        mmio,
        interrupts,
        properties,
        clocks,
        resets,
        pinctrl,
        bus_identity,
    })
}

/// Build the bus identity from the explicit declaration bus.
fn build_bus_identity(input: &AnalysisInput) -> Result<BusIdentity, LoweringPlanError> {
    let compatible = &input.declaration().driver.compatible;

    let bus_type =
        match input
            .bus_identity()
            .ok_or_else(|| LoweringPlanError::MissingBusIdentity {
                driver_name: input.declaration().driver.driver_name.clone(),
            })? {
            regseq_model::declaration::driver::BusType::Integrated => BusType::Platform,
            regseq_model::declaration::driver::BusType::Amba => BusType::Amba,
            regseq_model::declaration::driver::BusType::Pci => BusType::Pci,
        };

    let amba_id = input
        .bus_binding()
        .and_then(|b| b.amba_id)
        .map(|id| id as u32);

    let amba_revision = input
        .bus_binding()
        .and_then(|b| b.revision)
        .map(|r| r as u32);

    Ok(BusIdentity {
        bus_type,
        compatible: compatible.to_vec(),
        amba_id,
        amba_revision,
    })
}

/// Build acquisition ordering: clocks -> resets -> pinctrl -> mmio -> properties -> interrupts.
fn build_acquisition_order(resources: &ResourceObligations) -> Vec<AcquisitionStep> {
    let mut steps = Vec::new();
    let mut order = 0;

    for c in &resources.clocks {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::Clock,
            resource_id: c.clock_id.clone(),
            policy: c.policy.clone(),
        });
        order += 1;
    }
    for r in &resources.resets {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::Reset,
            resource_id: r.field_name.clone(),
            policy: r.policy.clone(),
        });
        order += 1;
    }
    for p in &resources.pinctrl {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::PinCtrl,
            resource_id: p.field_name.clone(),
            policy: p.policy.clone(),
        });
        order += 1;
    }
    for m in &resources.mmio {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::Mmio,
            resource_id: m.mmio_id.clone(),
            policy: m.policy.clone(),
        });
        order += 1;
    }
    for prop in &resources.properties {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::Property,
            resource_id: prop.property_id.clone(),
            policy: prop.policy.clone(),
        });
        order += 1;
    }
    for i in &resources.interrupts {
        steps.push(AcquisitionStep {
            order,
            resource_kind: ResourceKind::Interrupt,
            resource_id: i.interrupt_id.clone(),
            policy: i.policy.clone(),
        });
        order += 1;
    }

    steps
}

/// Build teardown ordering: reverse of acquisition.
fn build_teardown_order(acquisition: &[AcquisitionStep]) -> Vec<TeardownStep> {
    acquisition
        .iter()
        .rev()
        .enumerate()
        .map(|(i, acq)| TeardownStep {
            order: i,
            resource_kind: acq.resource_kind,
            resource_id: acq.resource_id.clone(),
        })
        .collect()
}

fn build_device_class_roles(input: &AnalysisInput) -> DeviceClassRoles {
    let families = input.callback_families_present();

    let serial_ops_realized: Vec<String> = input
        .callbacks()
        .serial_impls
        .iter()
        .map(|i| i.driver_type.clone())
        .collect();

    DeviceClassRoles {
        device_class: input.declaration().driver.device_class.clone(),
        serial_ops_realized,
        has_lifecycle: families.contains(&CallbackFamily::Lifecycle),
        has_power_management: families.contains(&CallbackFamily::PowerManagement),
    }
}

fn build_register_obligations(input: &AnalysisInput) -> RegisterObligations {
    let blocks = input
        .register_blocks()
        .iter()
        .map(|b| {
            let mut byte_offset = 0u32;
            let register_fields = b
                .fields
                .iter()
                .map(|f| {
                    let field_offset = byte_offset;
                    let stride = f.access_width.unwrap_or(32) / 8;
                    byte_offset += stride;
                    RegisterFieldInfo {
                        name: f.name.clone(),
                        access_width: f.access_width,
                        byte_offset: field_offset,
                    }
                })
                .collect();
            RegisterBlockObligation {
                name: b.name.clone(),
                field_count: b.fields.len(),
                register_fields,
                provenance: b.provenance.clone(),
            }
        })
        .collect();

    let access_sites = input
        .wrapper_call_sites()
        .iter()
        .map(|w| RegisterAccessObligation {
            method: w.method,
            containing_fn: w.containing_fn.clone(),
            provenance: w.provenance.clone(),
        })
        .collect();

    let poll_sites = input
        .poll_call_sites()
        .iter()
        .map(|p| PollObligation {
            containing_fn: p.containing_fn.clone(),
            provenance: p.provenance.clone(),
        })
        .collect();

    let descriptors = input
        .register_descriptors()
        .iter()
        .map(|d| RegisterDescriptorInfo {
            register_name: d.register_module.clone(),
            bit_fields: d
                .fields
                .iter()
                .map(|f| BitFieldInfo {
                    name: f.field_name.clone(),
                    offset: f.offset,
                    width: f.width,
                    named_values: f
                        .named_values
                        .iter()
                        .map(|nv| NamedValueInfo {
                            name: nv.name.clone(),
                            value: nv.value,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    RegisterObligations {
        blocks,
        access_sites,
        poll_sites,
        descriptors,
    }
}

fn build_sync_obligations(input: &AnalysisInput) -> SynchronizationObligations {
    let fields = input
        .sync_fields()
        .iter()
        .map(|f| SyncFieldObligation {
            field_name: f.field_name.clone(),
            kind: f.kind,
            payload_type: f.payload_type.clone(),
            provenance: f.provenance.clone(),
        })
        .collect();

    let lock_scopes = input
        .guard_scopes()
        .iter()
        .map(|g| LockScopeObligation {
            lock_kind: g.lock_kind,
            containing_fn: g.containing_fn.clone(),
            provenance: g.provenance.clone(),
        })
        .collect();

    let completion_uses = input
        .completion_operations()
        .iter()
        .map(|c| CompletionObligation {
            method: format!("{:?}", c.method),
            containing_fn: c.containing_fn.clone(),
            provenance: c.provenance.clone(),
        })
        .collect();

    SynchronizationObligations {
        fields,
        lock_scopes,
        completion_uses,
    }
}

fn build_callback_actions(input: &AnalysisInput) -> Vec<CallbackBodyPlan> {
    let mut plans = Vec::new();

    for imp in input.all_callback_impls() {
        let mut actions = Vec::new();

        // Collect operation evidence scoped to this callback family.
        match imp.family {
            CallbackFamily::Lifecycle => {
                // Lifecycle callbacks drive resource acquisition/release,
                // property reads, delays, and logging.
                for prop in &input.declaration().properties.fields {
                    actions.push(LoweringAction::PropertyRead {
                        property_id: prop.property_id.clone(),
                        has_default: prop.default_literal.is_some(),
                    });
                }
                for delay in input.timed_delay_call_sites() {
                    actions.push(LoweringAction::TimedDelay {
                        intrinsic: delay.intrinsic,
                        path_kind: delay.path_kind,
                    });
                }
                for log in input.log_call_sites() {
                    actions.push(LoweringAction::LogEmit {
                        level: log.level,
                        template: log.template.clone(),
                    });
                }
                for irq in &input.declaration().resources.interrupts.resources {
                    actions.push(LoweringAction::InterruptRegister {
                        interrupt_id: irq.interrupt_id.clone(),
                    });
                }
            }
            CallbackFamily::Serial => {
                // Serial callbacks contain register access and lock scopes.
                for w in input.wrapper_call_sites() {
                    actions.push(LoweringAction::RegisterAccess {
                        method: w.method,
                        containing_fn: w.containing_fn.clone(),
                    });
                }
                for g in input.guard_scopes() {
                    actions.push(LoweringAction::LockAcquire {
                        kind: g.lock_kind,
                        containing_fn: g.containing_fn.clone(),
                    });
                    actions.push(LoweringAction::LockRelease {
                        kind: g.lock_kind,
                        containing_fn: g.containing_fn.clone(),
                    });
                }
                for p in input.poll_call_sites() {
                    actions.push(LoweringAction::PollRegister {
                        containing_fn: p.containing_fn.clone(),
                    });
                }
            }
            CallbackFamily::SerialConsole | CallbackFamily::PowerManagement => {
                // Placeholder for future expansion.
            }
        }

        plans.push(CallbackBodyPlan {
            family: imp.family,
            driver_type: imp.driver_type.clone(),
            context: imp.context,
            actions,
        });
    }

    plans
}
