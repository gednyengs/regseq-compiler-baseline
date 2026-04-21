//! RegseqModel-native portable lowering entry (Phase-7B).
//!
//! Builds a [`DriverLoweringPlan`] directly from a verified
//! [`RegseqModel`] plus backend-neutral [`LoweringConfig`] knobs. Does
//! NOT go through `AnalysisInput` or the legacy
//! declaration/joined/semantic projections.
//!
//! This entry coexists with
//! [`crate::lowering::planner::build_driver_lowering_plan`] during
//! Phases 7C/7D; `regseq-lowering-14` eventually deletes the legacy
//! entry. The new entry does not produce a [`SemanticModel`] -- body
//! access for backends is handled through a separate
//! `FunctionBodyIndex` introduced by `regseq-linux-backend-03` (task
//! 8).
//!
//! Internal helpers walk `model.root_module` and invoke
//! [`regseq_model::query`] helpers from task
//! `regseq-model-61`. No string matching against canonical paths; no
//! `CanonicalPath` lookups; typed ids throughout.

use regseq_model::declaration::callbacks::{CallbackContext, CallbackFamily};
use regseq_model::declaration::driver::{BusType as DeclBusType, DeviceClass};
use regseq_model::declaration::provenance::Provenance;
use regseq_model::declaration::registers::WrapperMethod;
use regseq_model::declaration::synchronization::SyncFieldKind;
use regseq_model::identity::{Symbol, SymbolInterner};
use regseq_model::ir::intrinsic::RegseqIntrinsic;
use regseq_model::model::driver::{DriverStruct, PlatformBusBinding};
use regseq_model::model::module::{Module, ModuleKind};
use regseq_model::model::regseq_model::RegseqModel;
use regseq_model::model::resources::{ResourceKind, SyncKind};
use regseq_model::platform::PlatformValue;
use regseq_model::query;

use super::plan::{
    AcquisitionStep, BitFieldInfo, BusIdentity, BusType as PlanBusType, CallbackEntry,
    CallbackInventory, ClockObligation, DeviceClassRoles, DriverIdentity, DriverLoweringPlan,
    InterruptObligation, MmioObligation, NamedValueInfo, PinCtrlObligation, PollObligation,
    PropertyObligation, RegisterAccessObligation, RegisterBlockObligation, RegisterDescriptorInfo,
    RegisterFieldInfo, RegisterObligations, ResetObligation, ResourceObligations, ResourcePolicy,
    SyncFieldObligation, SynchronizationObligations, TeardownStep,
};
use super::planner::{build_diagnostics as build_diagnostics_legacy, PlanBase};

/// Backend-neutral configuration knobs for portable lowering.
///
/// Left empty on purpose: `regseq-lowering-10` introduces the type so
/// the new entry's signature is stable, and later tasks fill it with
/// knobs (debug provenance depth, policy toggles, etc.) as they
/// appear.
#[derive(Debug, Clone, Default)]
pub struct LoweringConfig {}

/// Errors produced by
/// [`build_driver_lowering_plan_from_model`].
///
/// The `InvariantViolated` variant is the catch-all for structural
/// states that `verify_phase_1` / `verify_phase_2` should already
/// have rejected; reaching it indicates a regressed invariant rather
/// than a driver-author bug.
#[derive(Debug, thiserror::Error)]
pub enum LoweringError {
    /// A structural invariant on the input model was violated.
    #[error("lowering invariant violated: {0}")]
    InvariantViolated(String),
}

/// Build a portable lowering plan directly from a verified
/// [`RegseqModel`].
///
/// Does NOT go through `AnalysisInput` or the legacy declaration /
/// joined / semantic projections. All plan fields are derived by
/// walking `model.root_module` and calling
/// [`regseq_model::query`] helpers.
///
/// This is the `RegseqModel`-era entry; the legacy
/// [`crate::lowering::planner::build_driver_lowering_plan`] remains
/// alongside until `regseq-driver-35` cuts `emit_driver` over.
pub fn build_driver_lowering_plan_from_model(
    model: &RegseqModel,
    _config: &LoweringConfig,
) -> Result<DriverLoweringPlan, LoweringError> {
    let driver = find_driver_struct(&model.root_module)
        .ok_or_else(|| LoweringError::InvariantViolated("model has no driver module".into()))?;

    let identity = build_identity(model, driver);
    let callbacks = build_callback_inventory(model);
    let resources = build_resource_obligations(model, driver)?;
    let acquisition_order = build_acquisition_order(&resources);
    let teardown_order = build_teardown_order(&acquisition_order);
    let device_class_roles = build_device_class_roles(model, driver);
    let register_obligations = build_register_obligations(model);
    let synchronization = build_sync_obligations(model, driver);

    let base = PlanBase {
        identity: identity.clone(),
        callbacks: callbacks.clone(),
        resources: resources.clone(),
        acquisition_order: acquisition_order.clone(),
        teardown_order: teardown_order.clone(),
        device_class_roles: device_class_roles.clone(),
        register_obligations: register_obligations.clone(),
        synchronization: synchronization.clone(),
    };
    let diagnostics = build_diagnostics_legacy(&base);

    Ok(DriverLoweringPlan {
        identity,
        callbacks,
        resources,
        acquisition_order,
        teardown_order,
        device_class_roles,
        register_obligations,
        synchronization,
        diagnostics,
    })
}

// ---------------------------------------------------------------------------
// Driver identity
// ---------------------------------------------------------------------------

fn build_identity(model: &RegseqModel, driver: &DriverStruct) -> DriverIdentity {
    let driver_name = sym_string(&model.symbols, driver.identity.name);
    let compatible = driver
        .identity
        .compatible
        .iter()
        .map(|c| sym_string(&model.symbols, *c))
        .collect();
    let device_class = device_class_from_symbol(&model.symbols, driver.device_class.as_ref());

    DriverIdentity {
        driver_name: driver_name.clone(),
        compatible,
        device_class,
        source_modules: Vec::new(),
        provenance: provenance_for(&driver_name, "identity"),
    }
}

// ---------------------------------------------------------------------------
// Callback inventory
// ---------------------------------------------------------------------------

fn build_callback_inventory(model: &RegseqModel) -> CallbackInventory {
    let mut inventory = CallbackInventory {
        lifecycle: Vec::new(),
        serial: Vec::new(),
        serial_console: Vec::new(),
        power_management: Vec::new(),
    };

    let Some(driver_mod) = find_driver_module(&model.root_module) else {
        return inventory;
    };
    let Some(driver) = driver_mod.driver.as_ref() else {
        return inventory;
    };
    let driver_type_name = sym_string(&model.symbols, driver.name);

    for impl_block in &driver_mod.trait_impls {
        let family = classify_family(&model.symbols, impl_block.trait_name);
        let Some(family) = family else {
            continue;
        };

        let entry = CallbackEntry {
            driver_type: driver_type_name.clone(),
            context: default_context_for(family),
            family,
            provenance: provenance_for(&driver_type_name, "callback_impl"),
        };
        match family {
            CallbackFamily::Lifecycle => inventory.lifecycle.push(entry),
            CallbackFamily::Serial => inventory.serial.push(entry),
            CallbackFamily::SerialConsole => inventory.serial_console.push(entry),
            CallbackFamily::PowerManagement => inventory.power_management.push(entry),
        }
    }

    inventory
}

// ---------------------------------------------------------------------------
// Resource obligations
// ---------------------------------------------------------------------------

fn build_resource_obligations(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<ResourceObligations, LoweringError> {
    let interner = &model.symbols;
    let all_blocks = collect_register_blocks(&model.root_module);

    let mmio = driver
        .mmio
        .iter()
        .map(|m| {
            let field_name = sym_string(interner, m.field_name);
            let block = all_blocks
                .iter()
                .find(|b| b.id == m.decl.register_block)
                .ok_or_else(|| {
                    LoweringError::InvariantViolated(format!(
                        "mmio field '{field_name}' references undeclared register block id {:?}",
                        m.decl.register_block
                    ))
                })?;
            let register_block_type = Some(sym_string(interner, block.name));
            Ok::<MmioObligation, LoweringError>(MmioObligation {
                field_name: field_name.clone(),
                mmio_id: sym_string(interner, m.decl.mmio_id),
                register_block_type,
                policy: ResourcePolicy::Required,
                provenance: provenance_for(&field_name, "mmio"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let interrupts = driver
        .interrupts
        .iter()
        .map(|i| {
            let field_name = sym_string(interner, i.field_name);
            let binding = i.binding.as_ref();
            InterruptObligation {
                field_name: field_name.clone(),
                interrupt_id: sym_string(interner, i.decl.interrupt_id),
                trigger_type: binding.and_then(|b| b.trigger_type.clone()),
                shared: binding.map(|b| b.shared),
                policy: ResourcePolicy::Required,
                provenance: provenance_for(&field_name, "interrupt"),
            }
        })
        .collect();

    let properties = driver
        .properties
        .iter()
        .map(|p| {
            let field_name = sym_string(interner, p.field_name);
            let default_literal = p.default_value.as_ref().map(format_literal);
            let policy = if p.required && default_literal.is_some() {
                ResourcePolicy::OptionalWithFallback
            } else if p.required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            PropertyObligation {
                property_id: sym_string(interner, p.property_id),
                field_name: field_name.clone(),
                field_type: scalar_type_as_str(&p.field_type).to_string(),
                required: p.required,
                default_literal,
                policy,
                provenance: provenance_for(&field_name, "property"),
            }
        })
        .collect();

    let clocks = driver
        .clocks
        .iter()
        .map(|c| {
            let field_name = sym_string(interner, c.field_name);
            let required = shape_required(&c.shape);
            let default_frequency = c.binding.as_ref().map(|b| b.default_frequency as i64);
            let policy = if required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            ClockObligation {
                field_name: field_name.clone(),
                clock_id: sym_string(interner, c.decl.clock_id),
                required,
                default_frequency,
                policy,
                provenance: provenance_for(&field_name, "clock"),
            }
        })
        .collect();

    let resets = driver
        .resets
        .iter()
        .map(|r| {
            let field_name = sym_string(interner, r.field_name);
            let required = shape_required(&r.shape);
            let binding = r.binding.as_ref();
            let policy = if required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            ResetObligation {
                field_name: field_name.clone(),
                required,
                reset_line_id: binding.map(|b| b.reset_line_id.clone()),
                reset_shared: binding.map(|b| b.reset_shared),
                policy,
                provenance: provenance_for(&field_name, "reset"),
            }
        })
        .collect();

    let pinctrl = driver
        .pinctrl
        .iter()
        .map(|p| {
            let field_name = sym_string(interner, p.field_name);
            let required = shape_required(&p.shape);
            let binding = p.binding.as_ref();
            let policy = if required {
                ResourcePolicy::Required
            } else {
                ResourcePolicy::OptionalSkip
            };
            PinCtrlObligation {
                field_name: field_name.clone(),
                required,
                pinctrl_id: Some(sym_string(interner, p.decl.pinctrl_id)),
                pin_states: binding.map(|b| b.pin_states.clone()),
                policy,
                provenance: provenance_for(&field_name, "pinctrl"),
            }
        })
        .collect();

    let bus_identity = build_bus_identity(model, driver)?;

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

fn build_bus_identity(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<BusIdentity, LoweringError> {
    let compatible: Vec<String> = driver
        .identity
        .compatible
        .iter()
        .map(|c| sym_string(&model.symbols, *c))
        .collect();

    let driver_name = sym_string(&model.symbols, driver.identity.name);

    let bus_symbol = driver.identity.bus.ok_or_else(|| {
        LoweringError::InvariantViolated(format!(
            "missing explicit bus identity for driver '{driver_name}'"
        ))
    })?;

    let bus_str = model.symbols.resolve(bus_symbol);
    let decl_bus = DeclBusType::from_raw(bus_str).ok_or_else(|| {
        LoweringError::InvariantViolated(format!("unrecognized bus type '{bus_str}'"))
    })?;

    let bus_type = match decl_bus {
        DeclBusType::Integrated => PlanBusType::Platform,
        DeclBusType::Amba => PlanBusType::Amba,
        DeclBusType::Pci => PlanBusType::Pci,
    };

    let platform_bus = query::bus_binding(model);
    let (amba_id, amba_revision, amba_mask) = amba_values_from_platform_bus(platform_bus);

    Ok(BusIdentity {
        bus_type,
        compatible,
        amba_id,
        amba_revision,
        amba_mask,
    })
}

fn amba_values_from_platform_bus(
    binding: Option<&PlatformBusBinding>,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    let Some(binding) = binding else {
        return (None, None, None);
    };
    let amba_id = binding
        .values
        .get("amba_id")
        .and_then(platform_value_as_u32);
    let amba_revision = binding
        .values
        .get("revision")
        .and_then(platform_value_as_u32);
    let amba_mask = binding
        .values
        .get("amba_mask")
        .and_then(platform_value_as_u32);
    (amba_id, amba_revision, amba_mask)
}

fn platform_value_as_u32(value: &PlatformValue) -> Option<u32> {
    match value {
        PlatformValue::Integer(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Acquisition/teardown ordering (matches legacy policy)
// ---------------------------------------------------------------------------

fn build_acquisition_order(resources: &ResourceObligations) -> Vec<AcquisitionStep> {
    let mut steps = Vec::new();
    let mut order = 0usize;

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

// ---------------------------------------------------------------------------
// Device-class roles
// ---------------------------------------------------------------------------

fn build_device_class_roles(model: &RegseqModel, driver: &DriverStruct) -> DeviceClassRoles {
    let families = query::callback_families_present(model);
    let device_class = device_class_from_symbol(&model.symbols, driver.device_class.as_ref());

    let serial_ops_realized: Vec<String> = if families.contains(&CallbackFamily::Serial) {
        vec![sym_string(&model.symbols, driver.name)]
    } else {
        Vec::new()
    };

    DeviceClassRoles {
        device_class,
        serial_ops_realized,
        has_lifecycle: families.contains(&CallbackFamily::Lifecycle),
        has_power_management: families.contains(&CallbackFamily::PowerManagement),
    }
}

// ---------------------------------------------------------------------------
// Register obligations
// ---------------------------------------------------------------------------

fn build_register_obligations(model: &RegseqModel) -> RegisterObligations {
    let interner = &model.symbols;

    let blocks: Vec<RegisterBlockObligation> = collect_register_blocks(&model.root_module)
        .into_iter()
        .map(|block| {
            let name = sym_string(interner, block.name);
            let register_fields = block
                .fields
                .iter()
                .map(|f| RegisterFieldInfo {
                    name: sym_string(interner, f.name),
                    access_width: Some(block.block_attributes.access_width),
                    byte_offset: f.byte_offset,
                })
                .collect::<Vec<_>>();
            RegisterBlockObligation {
                field_count: block.fields.len(),
                name: name.clone(),
                register_fields,
                provenance: provenance_for(&name, "register_block"),
            }
        })
        .collect();

    let descriptors: Vec<RegisterDescriptorInfo> = collect_register_descriptors(&model.root_module)
        .into_iter()
        .map(|desc| RegisterDescriptorInfo {
            register_name: sym_string(interner, desc.name),
            bit_fields: desc
                .fields
                .iter()
                .map(|f| BitFieldInfo {
                    name: sym_string(interner, f.name),
                    offset: f.offset,
                    width: f.width,
                    named_values: f
                        .named_values
                        .iter()
                        .map(|nv| NamedValueInfo {
                            name: sym_string(interner, nv.name),
                            value: nv.value as u32,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    let access_sites = query::wrapper_call_sites(model)
        .into_iter()
        .filter_map(|site| {
            let method = wrapper_method_from_intrinsic(site.intrinsic)?;
            Some(RegisterAccessObligation {
                method,
                containing_fn: function_name(model, site.function_id),
                provenance: provenance_for("register_access", "register_access"),
            })
        })
        .collect();

    let poll_sites = query::poll_call_sites(model)
        .into_iter()
        .map(|site| PollObligation {
            containing_fn: function_name(model, site.function_id),
            provenance: provenance_for("poll", "poll"),
        })
        .collect();

    RegisterObligations {
        blocks,
        access_sites,
        poll_sites,
        descriptors,
    }
}

// ---------------------------------------------------------------------------
// Synchronization obligations
// ---------------------------------------------------------------------------

fn build_sync_obligations(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> SynchronizationObligations {
    let interner = &model.symbols;

    let fields = driver
        .sync
        .iter()
        .map(|s| {
            let field_name = sym_string(interner, s.field_name);
            SyncFieldObligation {
                field_name: field_name.clone(),
                kind: sync_kind_to_field_kind(s.sync_kind),
                payload_type: s.payload_type.as_ref().map(|_| "<payload>".to_string()),
                provenance: provenance_for(&field_name, "sync_field"),
            }
        })
        .collect();

    SynchronizationObligations {
        fields,
        lock_scopes: Vec::new(),
        completion_uses: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn find_driver_module(module: &Module) -> Option<&Module> {
    if module.kind == ModuleKind::Driver {
        return Some(module);
    }
    for submodule in &module.submodules {
        if let Some(found) = find_driver_module(submodule) {
            return Some(found);
        }
    }
    None
}

fn find_driver_struct(module: &Module) -> Option<&DriverStruct> {
    find_driver_module(module).and_then(|m| m.driver.as_ref())
}

fn collect_register_blocks(module: &Module) -> Vec<&regseq_model::model::register::RegisterBlock> {
    let mut out: Vec<&regseq_model::model::register::RegisterBlock> = Vec::new();
    push_register_blocks(module, &mut out);
    out
}

fn push_register_blocks<'a>(
    module: &'a Module,
    out: &mut Vec<&'a regseq_model::model::register::RegisterBlock>,
) {
    for block in &module.register_blocks {
        out.push(block);
    }
    for submodule in &module.submodules {
        push_register_blocks(submodule, out);
    }
}

fn collect_register_descriptors(
    module: &Module,
) -> Vec<&regseq_model::model::register::RegisterDescriptor> {
    let mut out: Vec<&regseq_model::model::register::RegisterDescriptor> = Vec::new();
    push_register_descriptors(module, &mut out);
    out
}

fn push_register_descriptors<'a>(
    module: &'a Module,
    out: &mut Vec<&'a regseq_model::model::register::RegisterDescriptor>,
) {
    for desc in &module.register_descriptors {
        out.push(desc);
    }
    for submodule in &module.submodules {
        push_register_descriptors(submodule, out);
    }
}

fn sym_string(interner: &SymbolInterner, symbol: Symbol) -> String {
    interner.resolve(symbol).to_string()
}

fn provenance_for(origin: &str, raw_family: &str) -> Provenance {
    Provenance::new(raw_family).with_containing_item(origin)
}

fn device_class_from_symbol(
    interner: &SymbolInterner,
    model_class: Option<&regseq_model::model::driver::DeviceClass>,
) -> DeviceClass {
    match model_class {
        None => DeviceClass::Other(String::new()),
        Some(cls) => DeviceClass::from_raw(interner.resolve(cls.class)),
    }
}

fn classify_family(
    interner: &SymbolInterner,
    trait_name: Option<Symbol>,
) -> Option<CallbackFamily> {
    let symbol = trait_name?;
    match interner.resolve(symbol) {
        "Lifecycle" => Some(CallbackFamily::Lifecycle),
        "Serial" => Some(CallbackFamily::Serial),
        "SerialConsole" => Some(CallbackFamily::SerialConsole),
        "PowerManagement" => Some(CallbackFamily::PowerManagement),
        _ => None,
    }
}

fn default_context_for(family: CallbackFamily) -> CallbackContext {
    match family {
        CallbackFamily::Serial | CallbackFamily::SerialConsole => {
            CallbackContext::SubsystemSerialized
        }
        CallbackFamily::Lifecycle | CallbackFamily::PowerManagement => CallbackContext::Framework,
    }
}

fn shape_required(shape: &regseq_model::model::resources::Shape) -> bool {
    matches!(shape, regseq_model::model::resources::Shape::Single)
        || matches!(shape, regseq_model::model::resources::Shape::Array(_))
}

fn sync_kind_to_field_kind(kind: SyncKind) -> SyncFieldKind {
    match kind {
        SyncKind::Mutex => SyncFieldKind::Mutex,
        SyncKind::SpinLock => SyncFieldKind::SpinLock,
        SyncKind::IrqSpinLock => SyncFieldKind::IrqSpinLock,
        SyncKind::Completion => SyncFieldKind::Completion,
    }
}

fn wrapper_method_from_intrinsic(intrinsic: RegseqIntrinsic) -> Option<WrapperMethod> {
    match intrinsic {
        RegseqIntrinsic::RegisterRead => Some(WrapperMethod::Read),
        RegseqIntrinsic::RegisterReadField => Some(WrapperMethod::Read),
        RegseqIntrinsic::RegisterWrite => Some(WrapperMethod::Write),
        RegseqIntrinsic::RegisterWriteField => Some(WrapperMethod::Modify),
        _ => None,
    }
}

fn function_name(model: &RegseqModel, id: regseq_model::identity::FunctionId) -> String {
    for function in function_iter(&model.root_module) {
        if function.id == id {
            return sym_string(&model.symbols, function.name);
        }
    }
    String::new()
}

fn function_iter(module: &Module) -> Vec<&regseq_model::model::function::Function> {
    let mut out: Vec<&regseq_model::model::function::Function> = Vec::new();
    push_functions(module, &mut out);
    out
}

fn push_functions<'a>(
    module: &'a Module,
    out: &mut Vec<&'a regseq_model::model::function::Function>,
) {
    for function in &module.free_functions {
        out.push(function);
    }
    for impl_block in &module.impl_blocks {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for impl_block in &module.trait_impls {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for impl_block in &module.driver_impl_blocks {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for submodule in &module.submodules {
        push_functions(submodule, out);
    }
}

fn format_literal(literal: &regseq_model::ir::LiteralValue) -> String {
    use regseq_model::ir::LiteralValue;
    match literal {
        LiteralValue::Bool(b) => b.to_string(),
        LiteralValue::U8(v) => v.to_string(),
        LiteralValue::U16(v) => v.to_string(),
        LiteralValue::U32(v) => v.to_string(),
        LiteralValue::U64(v) => v.to_string(),
        LiteralValue::I8(v) => v.to_string(),
        LiteralValue::I16(v) => v.to_string(),
        LiteralValue::I32(v) => v.to_string(),
        LiteralValue::I64(v) => v.to_string(),
        LiteralValue::Usize(v) => v.to_string(),
        LiteralValue::Isize(v) => v.to_string(),
    }
}

fn scalar_type_as_str(ty: &regseq_model::ir::ScalarType) -> &'static str {
    use regseq_model::ir::ScalarType;
    match ty {
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::Usize => "usize",
        ScalarType::Isize => "isize",
    }
}
