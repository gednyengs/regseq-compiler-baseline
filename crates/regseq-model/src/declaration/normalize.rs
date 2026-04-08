//! Normalization from raw frontend artifact JSON into the declaration model.
//!
//! This module bridges the raw extraction output (deserialized from JSON
//! via [`super::raw_input`]) into the normalized declaration-model types.
//! Normalization is deterministic, local, and does not introduce join logic
//! or hidden raw-artifact prerequisites.

use super::bodies::{BodyDeclSet, BodyExpr, FnBodyDecl, FnKind, FnParamDecl};
use super::build_context::DeclarationBuildContext;
use super::callbacks::{CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl};
use super::driver::{BusType, DeviceClass, DriverDecl};
use super::mir_body::{MirBodyDeclSet, MirFnBodyDecl, MirQueryFlavor};
use super::operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LogArgKind, LogCallSiteDecl,
    LogLevel, LoggingDeclSet, PmGateDecl, PmGateDeclSet, PmGateProperties, TimedDelayCallSiteDecl,
    TimedDelayDeclSet,
};
use super::properties::{PropertyDeclSet, PropertyFieldDecl, PropertyLintDecl};
use super::provenance::Provenance;
use super::raw_input::RawArtifactInput;
use super::registers::{
    PollCallSiteDecl, RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl,
    RegisterDescriptorFieldDecl, RegisterFieldDecl, RegisterNamedValueDecl, WrapperCallSiteDecl,
    WrapperMethod,
};
use super::resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, IsrBindTarget, IsrBindingDecl,
    MmioFieldDecl, PinCtrlCallSiteDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetCallSiteDecl,
    ResetDeclSet, ResetFieldDecl, ResetMethod, ResourceDeclSet,
};
use super::synchronization::{
    CompletionMethod, CompletionOperationDecl, GuardScopeDecl, LockOperationDecl, SyncFieldDecl,
    SyncFieldKind, SynchronizationDeclSet,
};
use super::DeclarationModel;

/// Errors that can occur during normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizeError {
    /// A required raw identity section is missing.
    MissingIdentity,
    /// A stringly tag could not be parsed into its typed enum.
    UnrecognizedTag {
        family: &'static str,
        field: &'static str,
        value: String,
    },
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIdentity => write!(f, "raw artifact is missing identity section"),
            Self::UnrecognizedTag {
                family,
                field,
                value,
            } => write!(f, "unrecognized {family}.{field} tag: {value:?}"),
        }
    }
}

impl std::error::Error for NormalizeError {}

/// Normalize a raw frontend artifact into a declaration model.
///
/// Deserializes the raw JSON input and converts all families into their
/// normalized typed forms. Absent raw families produce empty declaration
/// sets rather than synthesized placeholder data.
pub fn normalize(input: &RawArtifactInput) -> Result<DeclarationModel, NormalizeError> {
    let identity = input
        .identity
        .as_ref()
        .ok_or(NormalizeError::MissingIdentity)?;

    let bus = identity
        .bus
        .as_deref()
        .map(|s| {
            BusType::from_raw(s).ok_or(NormalizeError::UnrecognizedTag {
                family: "identity",
                field: "bus",
                value: s.to_string(),
            })
        })
        .transpose()?;

    let driver = DriverDecl {
        driver_name: identity.driver_name.clone(),
        compatible: identity.compatible.clone(),
        device_class: DeviceClass::from_raw(&identity.device_class),
        bus,
        provenance: Provenance::new("identity"),
    };

    let build_context = DeclarationBuildContext {
        target_os: input.target_os.clone(),
    };

    let resources = normalize_resources(input)?;
    let synchronization = normalize_synchronization(input)?;
    let callbacks = normalize_callbacks(input)?;
    let registers = normalize_registers(input)?;
    let properties = normalize_properties(input);
    let timed_delay = normalize_timed_delay(input)?;
    let logging = normalize_logging(input)?;
    let pm_gate = normalize_pm_gate(input);
    let bodies = normalize_bodies(input);
    let mir_bodies = normalize_mir_bodies(input);

    Ok(DeclarationModel {
        driver,
        resources,
        synchronization,
        callbacks,
        registers,
        properties,
        timed_delay,
        logging,
        pm_gate,
        bodies,
        mir_bodies,
        build_context,
    })
}

/// Normalize a raw frontend artifact from a JSON string.
pub fn normalize_from_json(json: &str) -> Result<DeclarationModel, NormalizeFromJsonError> {
    let input: RawArtifactInput =
        serde_json::from_str(json).map_err(NormalizeFromJsonError::Json)?;
    normalize(&input).map_err(NormalizeFromJsonError::Normalize)
}

/// Errors from [`normalize_from_json`].
#[derive(Debug)]
pub enum NormalizeFromJsonError {
    /// JSON deserialization failed.
    Json(serde_json::Error),
    /// Normalization failed after successful deserialization.
    Normalize(NormalizeError),
}

impl std::fmt::Display for NormalizeFromJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON parse error: {e}"),
            Self::Normalize(e) => write!(f, "normalization error: {e}"),
        }
    }
}

impl std::error::Error for NormalizeFromJsonError {}

// ---------------------------------------------------------------------------
// Resource normalization
// ---------------------------------------------------------------------------

fn normalize_resources(input: &RawArtifactInput) -> Result<ResourceDeclSet, NormalizeError> {
    let mmio = normalize_mmio(input);
    let interrupts = normalize_interrupts(input)?;
    let clock = normalize_clock(input)?;
    let reset = normalize_reset(input)?;
    let pinctrl = normalize_pinctrl(input);

    Ok(ResourceDeclSet {
        mmio,
        interrupts,
        clock,
        reset,
        pinctrl,
    })
}

fn normalize_mmio(input: &RawArtifactInput) -> Vec<MmioFieldDecl> {
    let Some(reg) = &input.register_access else {
        return vec![];
    };
    reg.mmio_fields
        .iter()
        .map(|f| MmioFieldDecl {
            field_name: f.field_name.clone(),
            mmio_id: f.mmio_id.clone(),
            register_block_type: f.register_block_type.clone(),
            provenance: Provenance::new("register_access"),
        })
        .collect()
}

fn normalize_interrupts(input: &RawArtifactInput) -> Result<InterruptDeclSet, NormalizeError> {
    let Some(irq) = &input.interrupts else {
        return Ok(InterruptDeclSet {
            resources: vec![],
            isr_bindings: vec![],
        });
    };

    let resources = irq
        .resources
        .iter()
        .map(|r| {
            Ok(InterruptResourceDecl {
                field_name: r.field_name.clone(),
                interrupt_id: r.interrupt_id.clone(),
                shape: InterruptFieldShape::from_raw(&r.shape),
                binding_hints: InterruptBindingHints {
                    trigger_type: r.layer2.trigger_type.clone(),
                    trigger_polarity: r.layer2.trigger_polarity.clone(),
                    priority: r.layer2.priority.clone(),
                    shared: r.layer2.shared,
                },
                provenance: Provenance::new("interrupts"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let isr_bindings = irq
        .isr_bindings
        .iter()
        .map(|b| {
            let context =
                CallbackContext::from_raw(&b.context).ok_or(NormalizeError::UnrecognizedTag {
                    family: "interrupts",
                    field: "context",
                    value: b.context.clone(),
                })?;
            Ok(IsrBindingDecl {
                handler_name: b.handler_name.clone(),
                bind_target: IsrBindTarget::from_raw(&b.bind_target),
                has_index_param: b.has_index_param,
                context,
                provenance: Provenance::new("interrupts"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(InterruptDeclSet {
        resources,
        isr_bindings,
    })
}

fn normalize_clock(input: &RawArtifactInput) -> Result<ClockDeclSet, NormalizeError> {
    let Some(clk) = &input.clock else {
        return Ok(ClockDeclSet {
            fields: vec![],
            call_sites: vec![],
        });
    };

    let fields = clk
        .fields
        .iter()
        .map(|f| ClockFieldDecl {
            field_name: f.field_name.clone(),
            clock_id: f.clock_id.clone(),
            required: f.required,
            provenance: Provenance::new("clock"),
        })
        .collect();

    let call_sites = clk
        .call_sites
        .iter()
        .map(|s| {
            let method =
                ClockMethod::from_raw(&s.method).ok_or(NormalizeError::UnrecognizedTag {
                    family: "clock",
                    field: "method",
                    value: s.method.clone(),
                })?;
            Ok(ClockCallSiteDecl {
                method,
                containing_fn: s.containing_fn.clone(),
                provenance: Provenance::new("clock").with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ClockDeclSet { fields, call_sites })
}

fn normalize_reset(input: &RawArtifactInput) -> Result<ResetDeclSet, NormalizeError> {
    let Some(rst) = &input.reset else {
        return Ok(ResetDeclSet {
            fields: vec![],
            call_sites: vec![],
        });
    };

    let fields = rst
        .fields
        .iter()
        .map(|f| ResetFieldDecl {
            field_name: f.field_name.clone(),
            required: f.required,
            provenance: Provenance::new("reset"),
        })
        .collect();

    let call_sites = rst
        .call_sites
        .iter()
        .map(|s| {
            let method =
                ResetMethod::from_raw(&s.method).ok_or(NormalizeError::UnrecognizedTag {
                    family: "reset",
                    field: "method",
                    value: s.method.clone(),
                })?;
            Ok(ResetCallSiteDecl {
                method,
                containing_fn: s.containing_fn.clone(),
                provenance: Provenance::new("reset").with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ResetDeclSet { fields, call_sites })
}

fn normalize_pinctrl(input: &RawArtifactInput) -> PinCtrlDeclSet {
    let Some(pin) = &input.pinctrl else {
        return PinCtrlDeclSet {
            fields: vec![],
            call_sites: vec![],
        };
    };

    let fields = pin
        .fields
        .iter()
        .map(|f| PinCtrlFieldDecl {
            field_name: f.field_name.clone(),
            required: f.required,
            pinctrl_id: f.pinctrl_id.clone(),
            provenance: Provenance::new("pinctrl"),
        })
        .collect();

    let call_sites = pin
        .call_sites
        .iter()
        .map(|s| PinCtrlCallSiteDecl {
            state_type: s.state_type.clone(),
            containing_fn: s.containing_fn.clone(),
            provenance: Provenance::new("pinctrl").with_containing_item(s.containing_fn.clone()),
        })
        .collect();

    PinCtrlDeclSet { fields, call_sites }
}

// ---------------------------------------------------------------------------
// Synchronization normalization
// ---------------------------------------------------------------------------

fn normalize_synchronization(
    input: &RawArtifactInput,
) -> Result<SynchronizationDeclSet, NormalizeError> {
    let Some(sync) = &input.synchronization else {
        return Ok(SynchronizationDeclSet {
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
        });
    };

    let sync_fields = sync
        .sync_fields
        .iter()
        .map(|f| {
            let kind = SyncFieldKind::from_raw(&f.kind).ok_or(NormalizeError::UnrecognizedTag {
                family: "synchronization",
                field: "kind",
                value: f.kind.clone(),
            })?;
            Ok(SyncFieldDecl {
                field_name: f.field_name.clone(),
                kind,
                payload_type: f.payload_type.clone(),
                provenance: Provenance::new("synchronization"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let lock_operations = sync
        .lock_operations
        .iter()
        .map(|o| {
            let lock_kind =
                SyncFieldKind::from_raw(&o.lock_kind).ok_or(NormalizeError::UnrecognizedTag {
                    family: "synchronization",
                    field: "lock_kind",
                    value: o.lock_kind.clone(),
                })?;
            Ok(LockOperationDecl {
                lock_kind,
                containing_fn: o.containing_fn.clone(),
                provenance: Provenance::new("synchronization")
                    .with_containing_item(o.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let completion_operations = sync
        .completion_operations
        .iter()
        .map(|o| {
            let method =
                CompletionMethod::from_raw(&o.method).ok_or(NormalizeError::UnrecognizedTag {
                    family: "synchronization",
                    field: "method",
                    value: o.method.clone(),
                })?;
            Ok(CompletionOperationDecl {
                method,
                containing_fn: o.containing_fn.clone(),
                provenance: Provenance::new("synchronization")
                    .with_containing_item(o.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let guard_scopes = sync
        .guard_scopes
        .iter()
        .map(|g| {
            let lock_kind =
                SyncFieldKind::from_raw(&g.lock_kind).ok_or(NormalizeError::UnrecognizedTag {
                    family: "synchronization",
                    field: "lock_kind",
                    value: g.lock_kind.clone(),
                })?;
            Ok(GuardScopeDecl {
                lock_kind,
                containing_fn: g.containing_fn.clone(),
                provenance: Provenance::new("synchronization")
                    .with_containing_item(g.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SynchronizationDeclSet {
        sync_fields,
        lock_operations,
        completion_operations,
        guard_scopes,
    })
}

// ---------------------------------------------------------------------------
// Callback normalization
// ---------------------------------------------------------------------------

fn normalize_callbacks(input: &RawArtifactInput) -> Result<CallbackDeclSet, NormalizeError> {
    let Some(traits) = &input.trait_callbacks else {
        return Ok(CallbackDeclSet {
            lifecycle_impls: vec![],
            serial_impls: vec![],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        });
    };

    let normalize_impls = |impls: &[super::raw_input::RawTraitImplInput],
                           family: CallbackFamily|
     -> Result<Vec<CallbackImplDecl>, NormalizeError> {
        impls
            .iter()
            .map(|i| {
                let context = CallbackContext::from_raw(&i.context).ok_or(
                    NormalizeError::UnrecognizedTag {
                        family: "trait_callbacks",
                        field: "context",
                        value: i.context.clone(),
                    },
                )?;
                Ok(CallbackImplDecl {
                    driver_type: i.driver_type.clone(),
                    context,
                    family,
                    provenance: Provenance::new("trait_callbacks"),
                })
            })
            .collect()
    };

    Ok(CallbackDeclSet {
        lifecycle_impls: normalize_impls(&traits.lifecycle_impls, CallbackFamily::Lifecycle)?,
        serial_impls: normalize_impls(&traits.serial_impls, CallbackFamily::Serial)?,
        serial_console_impls: normalize_impls(
            &traits.serial_console_impls,
            CallbackFamily::SerialConsole,
        )?,
        power_management_impls: normalize_impls(
            &traits.power_management_impls,
            CallbackFamily::PowerManagement,
        )?,
    })
}

// ---------------------------------------------------------------------------
// Register normalization
// ---------------------------------------------------------------------------

fn normalize_registers(input: &RawArtifactInput) -> Result<RegisterDeclSet, NormalizeError> {
    let Some(reg) = &input.register_access else {
        return Ok(RegisterDeclSet {
            blocks: vec![],
            descriptors: vec![],
            wrapper_call_sites: vec![],
            poll_call_sites: vec![],
        });
    };

    let blocks = reg
        .register_blocks
        .iter()
        .map(|b| RegisterBlockDecl {
            name: b.name.clone(),
            fields: b
                .fields
                .iter()
                .map(|f| RegisterFieldDecl {
                    name: f.name.clone(),
                    access_width: f.access_width,
                    endian: f.endian.clone(),
                    side_effects: f.side_effects.clone(),
                    hw_updates: f.hw_updates.clone(),
                })
                .collect(),
            provenance: Provenance::new("register_access"),
        })
        .collect();

    let descriptors = reg
        .register_descriptors
        .iter()
        .map(|d| RegisterDescriptorDecl {
            register_module: d.register_module.clone(),
            fields: d
                .fields
                .iter()
                .map(|f| RegisterDescriptorFieldDecl {
                    field_name: f.field_name.clone(),
                    offset: f.offset,
                    width: f.width,
                    named_values: f
                        .named_values
                        .iter()
                        .map(|v| RegisterNamedValueDecl {
                            name: v.name.clone(),
                            value: v.value,
                        })
                        .collect(),
                })
                .collect(),
            provenance: Provenance::new("register_access"),
        })
        .collect();

    let wrapper_call_sites = reg
        .wrapper_call_sites
        .iter()
        .map(|s| {
            let method =
                WrapperMethod::from_raw(&s.method).ok_or(NormalizeError::UnrecognizedTag {
                    family: "register_access",
                    field: "method",
                    value: s.method.clone(),
                })?;
            Ok(WrapperCallSiteDecl {
                method,
                containing_fn: s.containing_fn.clone(),
                provenance: Provenance::new("register_access")
                    .with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let poll_call_sites = reg
        .poll_call_sites
        .iter()
        .map(|s| PollCallSiteDecl {
            containing_fn: s.containing_fn.clone(),
            provenance: Provenance::new("register_access")
                .with_containing_item(s.containing_fn.clone()),
        })
        .collect();

    Ok(RegisterDeclSet {
        blocks,
        descriptors,
        wrapper_call_sites,
        poll_call_sites,
    })
}

// ---------------------------------------------------------------------------
// Property normalization
// ---------------------------------------------------------------------------

fn normalize_properties(input: &RawArtifactInput) -> PropertyDeclSet {
    let Some(props) = &input.firmware_properties else {
        return PropertyDeclSet {
            fields: vec![],
            lints: vec![],
        };
    };

    let fields = props
        .fields
        .iter()
        .map(|f| PropertyFieldDecl {
            driver_type: f.driver_type.clone(),
            field_name: f.field_name.clone(),
            field_type: f.field_type.clone(),
            property_id: f.property_id.clone(),
            default_literal: f.default_literal.clone(),
            vendor_prefix: f.vendor_prefix.clone(),
            required: f.required,
            provenance: Provenance::new("firmware_properties"),
        })
        .collect();

    let lints = props
        .lints
        .iter()
        .map(|l| match l {
            super::raw_input::RawPropertyLintInput::RequiredWithDefault {
                driver_type,
                field_name,
                property_id,
            } => PropertyLintDecl::RequiredWithDefault {
                driver_type: driver_type.clone(),
                field_name: field_name.clone(),
                property_id: property_id.clone(),
                provenance: Provenance::new("firmware_properties"),
            },
        })
        .collect();

    PropertyDeclSet { fields, lints }
}

// ---------------------------------------------------------------------------
// Timed-delay normalization
// ---------------------------------------------------------------------------

fn normalize_timed_delay(input: &RawArtifactInput) -> Result<TimedDelayDeclSet, NormalizeError> {
    let Some(td) = &input.timed_delay else {
        return Ok(TimedDelayDeclSet {
            call_sites: vec![],
            duration_ctors: vec![],
        });
    };

    let call_sites = td
        .call_sites
        .iter()
        .map(|s| {
            let intrinsic =
                DelayIntrinsic::from_raw(&s.intrinsic).ok_or(NormalizeError::UnrecognizedTag {
                    family: "timed_delay",
                    field: "intrinsic",
                    value: s.intrinsic.clone(),
                })?;
            let path_kind =
                DelayPathKind::from_raw(&s.path_kind).ok_or(NormalizeError::UnrecognizedTag {
                    family: "timed_delay",
                    field: "path_kind",
                    value: s.path_kind.clone(),
                })?;
            Ok(TimedDelayCallSiteDecl {
                intrinsic,
                path_kind,
                containing_fn: s.containing_fn.clone(),
                provenance: Provenance::new("timed_delay")
                    .with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let duration_ctors = td
        .duration_ctors
        .iter()
        .map(|s| {
            let ctor = DurationCtor::from_raw(&s.ctor).ok_or(NormalizeError::UnrecognizedTag {
                family: "timed_delay",
                field: "ctor",
                value: s.ctor.clone(),
            })?;
            Ok(DurationCtorSiteDecl {
                ctor,
                containing_fn: s.containing_fn.clone(),
                provenance: Provenance::new("timed_delay")
                    .with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TimedDelayDeclSet {
        call_sites,
        duration_ctors,
    })
}

// ---------------------------------------------------------------------------
// Logging normalization
// ---------------------------------------------------------------------------

fn normalize_logging(input: &RawArtifactInput) -> Result<LoggingDeclSet, NormalizeError> {
    let Some(log) = &input.logging else {
        return Ok(LoggingDeclSet { call_sites: vec![] });
    };

    let call_sites = log
        .call_sites
        .iter()
        .map(|s| {
            let level = LogLevel::from_raw(&s.level).ok_or(NormalizeError::UnrecognizedTag {
                family: "logging",
                field: "level",
                value: s.level.clone(),
            })?;
            let arg_kinds = s
                .arg_kinds
                .iter()
                .map(|k| {
                    LogArgKind::from_raw(k).ok_or(NormalizeError::UnrecognizedTag {
                        family: "logging",
                        field: "arg_kinds",
                        value: k.clone(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(LogCallSiteDecl {
                level,
                containing_fn: s.containing_fn.clone(),
                canonical: s.canonical,
                template: s.template.clone(),
                arg_count: s.arg_count,
                arg_kinds,
                provenance: Provenance::new("logging")
                    .with_containing_item(s.containing_fn.clone()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LoggingDeclSet { call_sites })
}

// ---------------------------------------------------------------------------
// PM gate normalization
// ---------------------------------------------------------------------------

fn normalize_pm_gate(input: &RawArtifactInput) -> PmGateDeclSet {
    let Some(pm) = &input.pm_gate else {
        return PmGateDeclSet { gates: vec![] };
    };

    let gates = pm
        .gates
        .iter()
        .map(|g| PmGateDecl {
            driver_type: g.driver_type.clone(),
            properties: PmGateProperties {
                idle: g.properties.idle,
                wakeup_capable: g.properties.wakeup_capable,
            },
            has_pm_impl: g.has_pm_impl,
            provenance: Provenance::new("pm_gate"),
        })
        .collect();

    PmGateDeclSet { gates }
}

// ---------------------------------------------------------------------------
// Body normalization
// ---------------------------------------------------------------------------

/// Normalize MIR-derived bodies into the canonical `MirBodyDeclSet`.
///
/// The raw artifact carries typed MIR bodies from `mir_drops_elaborated_and_const_checked`.
/// This function converts each raw function entry into a `MirFnBodyDecl` with
/// the MIR body, helper-call graph edges, and provenance.
fn normalize_mir_bodies(input: &RawArtifactInput) -> MirBodyDeclSet {
    let Some(raw_bodies) = &input.bodies else {
        return MirBodyDeclSet {
            functions: vec![],
            query_flavor: MirQueryFlavor::DropsElaborated,
        };
    };

    let functions = raw_bodies
        .functions
        .iter()
        .map(|f| MirFnBodyDecl {
            name: f.name.clone(),
            def_path: f.def_path.clone(),
            kind: normalize_mir_fn_kind(&f.kind),
            receiver: f.receiver.clone(),
            params: f
                .params
                .iter()
                .map(|p| super::mir_body::FnParamDecl {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                })
                .collect(),
            return_type: f.return_type.clone(),
            body: f.body.clone(),
            helper_calls: f.helper_calls.clone(),
            monomorphization: f.monomorphization.clone(),
            provenance: Provenance::new("bodies").with_containing_item(f.name.clone()),
        })
        .collect();

    MirBodyDeclSet {
        functions,
        query_flavor: MirQueryFlavor::DropsElaborated,
    }
}

fn normalize_mir_fn_kind(raw: &super::raw_input::RawFnKindInput) -> super::mir_body::FnKind {
    use super::raw_input::RawFnKindInput;
    match raw {
        RawFnKindInput::Callback {
            trait_name,
            method_name,
        } => super::mir_body::FnKind::Callback {
            trait_name: trait_name.clone(),
            method_name: method_name.clone(),
        },
        RawFnKindInput::Isr => super::mir_body::FnKind::Isr,
        RawFnKindInput::Helper => super::mir_body::FnKind::Helper,
        RawFnKindInput::FreeFunction => super::mir_body::FnKind::FreeFunction,
    }
}

/// Transitional: populate the legacy `BodyDeclSet` with stub entries.
///
/// This fills the `bodies` field of `DeclarationModel` with per-function
/// metadata but `Unsupported` body markers. Downstream consumers should
/// migrate to `mir_bodies` for real executable-body data.
fn normalize_bodies(input: &RawArtifactInput) -> BodyDeclSet {
    let Some(raw_bodies) = &input.bodies else {
        return BodyDeclSet { functions: vec![] };
    };

    let functions = raw_bodies
        .functions
        .iter()
        .map(|f| FnBodyDecl {
            name: f.name.clone(),
            kind: normalize_fn_kind(&f.kind),
            receiver: f.receiver.clone(),
            params: f
                .params
                .iter()
                .map(|p| FnParamDecl {
                    name: p.name.clone(),
                    ty: mir_ty_display(&p.ty),
                })
                .collect(),
            return_type: f.return_type.as_ref().map(mir_ty_display),
            body: BodyExpr::Unsupported {
                hir_kind: "MirBody".to_string(),
                span_text: Some("mir_body_pending_normalization".to_string()),
            },
            provenance: Provenance::new("bodies").with_containing_item(f.name.clone()),
        })
        .collect();

    BodyDeclSet { functions }
}

fn normalize_fn_kind(raw: &super::raw_input::RawFnKindInput) -> FnKind {
    match raw {
        super::raw_input::RawFnKindInput::Callback {
            trait_name,
            method_name,
        } => FnKind::Callback {
            trait_name: trait_name.clone(),
            method_name: method_name.clone(),
        },
        super::raw_input::RawFnKindInput::Isr => FnKind::Isr,
        super::raw_input::RawFnKindInput::Helper => FnKind::Helper,
        super::raw_input::RawFnKindInput::FreeFunction => FnKind::FreeFunction,
    }
}

/// Render a `MirTy` as a display string for the legacy body model.
fn mir_ty_display(ty: &super::mir_body::MirTy) -> String {
    use super::mir_body::{FloatWidth, IntWidth, MirTy};
    match ty {
        MirTy::Bool => "bool".to_string(),
        MirTy::Char => "char".to_string(),
        MirTy::Int { width, .. } => match width {
            IntWidth::I8 => "i8",
            IntWidth::I16 => "i16",
            IntWidth::I32 => "i32",
            IntWidth::I64 => "i64",
            IntWidth::I128 => "i128",
            IntWidth::Isize => "isize",
            IntWidth::U8 => "u8",
            IntWidth::U16 => "u16",
            IntWidth::U32 => "u32",
            IntWidth::U64 => "u64",
            IntWidth::U128 => "u128",
            IntWidth::Usize => "usize",
        }
        .to_string(),
        MirTy::Float { width } => match width {
            FloatWidth::F32 => "f32",
            FloatWidth::F64 => "f64",
        }
        .to_string(),
        MirTy::Str => "str".to_string(),
        MirTy::Bytes => "[u8]".to_string(),
        MirTy::Never => "!".to_string(),
        MirTy::Struct { path, .. } | MirTy::Enum { path, .. } | MirTy::Fn { path, .. } => {
            path.clone()
        }
        MirTy::Ref { mutbl, ty } => {
            let m = if *mutbl { "mut " } else { "" };
            format!("&{m}{}", mir_ty_display(ty))
        }
        MirTy::RawPtr { mutbl, ty } => {
            let m = if *mutbl { "mut " } else { "const " };
            format!("*{m}{}", mir_ty_display(ty))
        }
        MirTy::Tuple { elements } if elements.is_empty() => "()".to_string(),
        MirTy::Tuple { elements } => {
            let inner: Vec<_> = elements.iter().map(mir_ty_display).collect();
            format!("({})", inner.join(", "))
        }
        MirTy::Array { element, len } => {
            format!("[{}; {len}]", mir_ty_display(element))
        }
        MirTy::Slice { element } => format!("[{}]", mir_ty_display(element)),
        MirTy::ZeroSizedLike { description } => description.clone(),
        MirTy::NominalRef { path } => path.clone(),
        MirTy::Unsupported { description } => description.clone(),
    }
}
