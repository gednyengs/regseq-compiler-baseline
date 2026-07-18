//! Device-interrupt (construct #6) L2 tenant: the `[interrupt.<id>]` join.
//!
//! `bind` joins each recognized `Interrupt` resource to its `[interrupt.<id>]`
//! platform section by the mandatory L1 `id` (R01/D08 — no field-name
//! fallback), validating the canonical trigger spellings (`level|edge`;
//! `active_high|active_low|rising|falling` — non-canonical tokens are
//! REJECTED, never coerced; D10/R12) and filling the typed
//! [`InterruptBinding`] on the model. `shared` is the single-source physical
//! board fact (D05/I08), already defaulted `false` by the structural parse.
//!
//! `validate` enforces section presence: a missing `[interrupt.<id>]` on a
//! non-`Optional` resource is a frontend error (C03); `Option<Interrupt>`
//! absence is the supported board-doesn't-wire-it case (D06) and leaves the
//! binding unresolved. Per-target line identity is derived by each backend
//! from the bound facts (R10: generic-c needs the `irq_number` literal and
//! emit-errors without it; linux resolves at runtime (`dev->irq[0]` /
//! `pdev->irq`); zephyr's `DT_INST_IRQN` is authoritative with `irq_number`
//! advisory) — the tenant stays target-neutral.

use regseq_config::{InterruptBindingSpec, PlatformBindings};
use regseq_model::{ArmReachability, InterruptForm, RegseqModel, TriggerPolarity, TriggerType};

/// `[interrupt.<id>]` surface diagnostics (construct #6). The `IRQ-L2-*`
/// family is harness-named (the C03 contract names the checks, not the IDs),
/// mirroring the `BRM-*` convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptL2Error {
    /// IRQ-L2-001: `trigger_type` outside the canonical vocabulary.
    BadTriggerType {
        /// The resource id.
        id: String,
        /// The rejected token, verbatim.
        token: String,
    },
    /// IRQ-L2-002: `trigger_polarity` outside the canonical vocabulary.
    BadTriggerPolarity {
        /// The resource id.
        id: String,
        /// The rejected token, verbatim.
        token: String,
    },
    /// IRQ-L2-003: a non-`Optional` `Interrupt` resource has no
    /// `[interrupt.<id>]` section (C03).
    MissingSection {
        /// The resource id (the missing section's key).
        id: String,
    },
}

impl InterruptL2Error {
    /// The diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            InterruptL2Error::BadTriggerType { .. } => "IRQ-L2-001",
            InterruptL2Error::BadTriggerPolarity { .. } => "IRQ-L2-002",
            InterruptL2Error::MissingSection { .. } => "IRQ-L2-003",
        }
    }
}

impl std::fmt::Display for InterruptL2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterruptL2Error::BadTriggerType { id, token } => write!(
                f,
                "IRQ-L2-001: [interrupt.{id}] trigger_type {token:?} is not canonical \
                 (expected \"level\" or \"edge\")"
            ),
            InterruptL2Error::BadTriggerPolarity { id, token } => write!(
                f,
                "IRQ-L2-002: [interrupt.{id}] trigger_polarity {token:?} is not canonical \
                 (expected \"active_high\", \"active_low\", \"rising\", or \"falling\")"
            ),
            InterruptL2Error::MissingSection { id } => write!(
                f,
                "IRQ-L2-003: interrupt resource `{id}` has no [interrupt.{id}] platform \
                 section (required for a non-Option Interrupt; C03)"
            ),
        }
    }
}

/// The device-interrupt L2 tenant.
pub struct InterruptBinder;

impl crate::L2Binder for InterruptBinder {
    fn name(&self) -> &'static str {
        "device-interrupt"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_interrupt_bindings(model, bindings).map_err(crate::L2Error::Interrupt)
    }

    fn validate(&self, model: &RegseqModel) -> Result<(), crate::L2Error> {
        validate_interrupt_surface(model).map_err(crate::L2Error::Interrupt)
    }
}

/// Join every recognized resource's `[interrupt.<id>]` section (by `id`) into
/// its typed model binding. Canonical-spelling validation happens here, at the
/// join (D10/R12): a present-but-non-canonical token is an error, never a
/// coercion. An absent section leaves the binding unresolved (`validate`
/// decides whether that is an error, per the resource form).
fn join_interrupt_bindings(
    model: &mut RegseqModel,
    bindings: &PlatformBindings,
) -> Result<(), InterruptL2Error> {
    // Pass 1 (immutable): resolve each resource's typed binding.
    struct Resolved {
        driver: regseq_model::ItemId,
        resource_idx: usize,
        binding: regseq_model::InterruptBinding,
    }
    let mut resolved: Vec<Resolved> = Vec::new();
    for (&driver, tables) in model.interrupt.drivers.iter() {
        for (resource_idx, resource) in tables.resources.iter().enumerate() {
            let id = model.strings.resolve(resource.id);
            let Some(spec) = bindings.interrupt.get(id) else {
                continue;
            };
            let binding = typed_binding(id, spec)?;
            resolved.push(Resolved {
                driver,
                resource_idx,
                binding,
            });
        }
    }
    // Pass 2 (mutable): apply.
    for r in resolved {
        if let Some(tables) = model.interrupt.drivers.get_mut(&r.driver) {
            tables.resources[r.resource_idx].binding = r.binding;
        }
    }
    Ok(())
}

/// Convert one structural [`InterruptBindingSpec`] into the typed model
/// binding, validating the canonical vocabularies (C03/R12).
fn typed_binding(
    id: &str,
    spec: &InterruptBindingSpec,
) -> Result<regseq_model::InterruptBinding, InterruptL2Error> {
    let trigger_type = match spec.trigger_type.as_deref() {
        None => None,
        Some("level") => Some(TriggerType::Level),
        Some("edge") => Some(TriggerType::Edge),
        Some(other) => {
            return Err(InterruptL2Error::BadTriggerType {
                id: id.to_string(),
                token: other.to_string(),
            })
        }
    };
    let trigger_polarity = match spec.trigger_polarity.as_deref() {
        None => None,
        Some("active_high") => Some(TriggerPolarity::ActiveHigh),
        Some("active_low") => Some(TriggerPolarity::ActiveLow),
        Some("rising") => Some(TriggerPolarity::Rising),
        Some("falling") => Some(TriggerPolarity::Falling),
        Some(other) => {
            return Err(InterruptL2Error::BadTriggerPolarity {
                id: id.to_string(),
                token: other.to_string(),
            })
        }
    };
    Ok(regseq_model::InterruptBinding {
        irq_number: spec.irq_number,
        trigger_type,
        trigger_polarity,
        priority: spec.priority,
        shared: spec.shared,
        resolved: true,
    })
}

/// The R11 shared-line discipline WARNING (C06 warning row, the frontend
/// half): one `IRQ-WARN-SHARED-NOTHANDLED` line per `shared = true` resource
/// whose handler's `IrqResult::NotHandled` arm is not PROVEN reachable.
///
/// The direction is the ratified floor: **warn-on-unknown** — the warning
/// fires unless reachability is PROVEN (a syntactically present but
/// unreachable/unprovable arm still warns; the analysis may over-warn, never
/// under-warn). `shared` is the single-source L2 board fact (I08), so this
/// runs after the join; non-fatal — the production caller routes the strings
/// to the compiler's warning channel (`validate` itself stays side-effect-free
/// and error-only per the tenant contract).
pub fn shared_discipline_warnings(model: &RegseqModel) -> Vec<String> {
    let mut warnings = Vec::new();
    for tables in model.interrupt.drivers.values() {
        for (idx, resource) in tables.resources.iter().enumerate() {
            if !resource.binding.shared {
                continue;
            }
            let Some(handler) = tables.handlers.iter().find(|h| h.resource as usize == idx) else {
                continue;
            };
            if handler.arms.not_handled != ArmReachability::ProvenReachable {
                warnings.push(format!(
                    "IRQ-WARN-SHARED-NOTHANDLED: interrupt resource `{}` is shared \
                     (the board fact) but handler `{}` has no PROVEN-reachable \
                     IrqResult::NotHandled return — a shared-line handler must be \
                     able to disclaim a foreign interrupt (warn-on-unknown: an \
                     unprovable conditional arm reads Unknown and still warns)",
                    model.strings.resolve(resource.name),
                    model.strings.resolve(handler.method_name),
                ));
            }
        }
    }
    warnings
}

/// Section-presence validation (C03): every non-`Optional` resource must have
/// joined a section; `Option<Interrupt>` absence is supported (D06).
fn validate_interrupt_surface(model: &RegseqModel) -> Result<(), InterruptL2Error> {
    for tables in model.interrupt.drivers.values() {
        for resource in &tables.resources {
            if resource.binding.resolved {
                continue;
            }
            if matches!(resource.form, InterruptForm::Optional) {
                continue;
            }
            return Err(InterruptL2Error::MissingSection {
                id: model.strings.resolve(resource.id).to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::L2Binder;
    use regseq_model::{DriverInterrupts, InterruptResource, ItemId, RegseqModel, StableCrateId};

    fn model_with_resource(id: &str, form: InterruptForm) -> RegseqModel {
        let mut model = RegseqModel::with_primary("irq-l2-test", StableCrateId(1));
        let name = model.strings.intern("irq_field");
        let id = model.strings.intern(id);
        model.interrupt.drivers.insert(
            ItemId::from_u32(0),
            DriverInterrupts {
                resources: vec![InterruptResource {
                    name,
                    id,
                    form,
                    binding: Default::default(),
                }],
                handlers: Vec::new(),
            },
        );
        model
    }

    fn bindings(toml: &str) -> PlatformBindings {
        regseq_config::parse_platform_bindings_str(toml).expect("parse")
    }

    fn sole_binding(model: &RegseqModel) -> regseq_model::InterruptBinding {
        model.interrupt.drivers[&ItemId::from_u32(0)].resources[0]
            .binding
            .clone()
    }

    #[test]
    fn joins_full_section_with_canonical_spellings() {
        let mut model = model_with_resource("uart_irq", InterruptForm::Single);
        let b = bindings(
            "[interrupt.uart_irq]\nirq_number = 37\ntrigger_type = \"level\"\n\
             trigger_polarity = \"active_high\"\npriority = 3\nshared = false\n",
        );
        InterruptBinder.bind(&mut model, &b).expect("bind");
        InterruptBinder.validate(&model).expect("validate");
        let binding = sole_binding(&model);
        assert_eq!(binding.irq_number, Some(37));
        assert_eq!(binding.trigger_type, Some(TriggerType::Level));
        assert_eq!(binding.trigger_polarity, Some(TriggerPolarity::ActiveHigh));
        assert_eq!(binding.priority, Some(3));
        assert!(!binding.shared);
        assert!(binding.resolved);
    }

    #[test]
    fn shared_true_is_the_board_truth() {
        let mut model = model_with_resource("nic_irq", InterruptForm::Single);
        let b = bindings("[interrupt.nic_irq]\nirq_number = 11\nshared = true\n");
        InterruptBinder.bind(&mut model, &b).expect("bind");
        assert!(sole_binding(&model).shared);
    }

    #[test]
    fn non_canonical_polarity_is_rejected_not_coerced() {
        // A historical `"high"` spelling: D10/R12 demand rejection
        // (fixtures migrate, the binder does not bend).
        let mut model = model_with_resource("uart_irq", InterruptForm::Single);
        let b = bindings("[interrupt.uart_irq]\ntrigger_polarity = \"high\"\n");
        let err = InterruptBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "IRQ-L2-002");
        assert!(err.to_string().contains("\"high\""), "{err}");
    }

    #[test]
    fn non_canonical_trigger_type_is_rejected() {
        let mut model = model_with_resource("uart_irq", InterruptForm::Single);
        let b = bindings("[interrupt.uart_irq]\ntrigger_type = \"LEVEL\"\n");
        let err = InterruptBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "IRQ-L2-001");
    }

    #[test]
    fn missing_section_on_single_form_is_an_error() {
        let mut model = model_with_resource("uart_irq", InterruptForm::Single);
        let b = bindings("[mmio.regs]\nbase = 0\nsize = 16\n");
        InterruptBinder
            .bind(&mut model, &b)
            .expect("bind is a no-op join");
        let err = InterruptBinder.validate(&model).unwrap_err();
        assert_eq!(err.code(), "IRQ-L2-003");
        assert!(err.to_string().contains("uart_irq"), "{err}");
    }

    #[test]
    fn missing_section_on_array_form_is_an_error() {
        let mut model = model_with_resource("lines", InterruptForm::Array { len: 2 });
        let b = bindings("version = \"1\"\n");
        InterruptBinder.bind(&mut model, &b).expect("bind");
        assert_eq!(
            InterruptBinder.validate(&model).unwrap_err().code(),
            "IRQ-L2-003"
        );
    }

    #[test]
    fn missing_section_on_optional_form_is_supported_absence() {
        let mut model = model_with_resource("maybe_irq", InterruptForm::Optional);
        let b = bindings("version = \"1\"\n");
        InterruptBinder.bind(&mut model, &b).expect("bind");
        InterruptBinder
            .validate(&model)
            .expect("Option absence is valid (D06)");
        assert!(!sole_binding(&model).resolved);
    }

    #[test]
    fn interruptless_model_binds_and_validates_clean() {
        let mut model = RegseqModel::with_primary("plain", StableCrateId(1));
        let b = bindings("[interrupt.orphan]\nirq_number = 5\n");
        // An orphan section (no resource joins it) is accepted-and-unused,
        // matching the [mmio.<id>] discipline.
        InterruptBinder.bind(&mut model, &b).expect("bind");
        InterruptBinder.validate(&model).expect("validate");
    }

    // --- R11 shared-line discipline warning (warn-on-unknown floor) ---

    fn model_with_shared_handler(shared: bool, not_handled: ArmReachability) -> RegseqModel {
        let mut model = model_with_resource("nic_irq", InterruptForm::Single);
        let method = model.strings.intern("handle_irq");
        let tables = model
            .interrupt
            .drivers
            .get_mut(&ItemId::from_u32(0))
            .unwrap();
        tables.resources[0].binding.shared = shared;
        tables.resources[0].binding.resolved = true;
        tables.handlers.push(regseq_model::IsrHandler {
            method_name: method,
            resource: 0,
            array_bind: false,
            body: None,
            arms: regseq_model::IrqArmReachability {
                handled: ArmReachability::Unknown,
                not_handled,
                defer_work: ArmReachability::ProvenUnreachable,
            },
        });
        model
    }

    #[test]
    fn shared_with_unknown_nothandled_warns() {
        let model = model_with_shared_handler(true, ArmReachability::Unknown);
        let warnings = shared_discipline_warnings(&model);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("IRQ-WARN-SHARED-NOTHANDLED"),
            "{}",
            warnings[0]
        );
    }

    #[test]
    fn shared_with_unreachable_nothandled_still_warns() {
        // The PLAN's pinned direction: a syntactically present but
        // PROVEN-UNREACHABLE NotHandled arm must still warn (the warning is
        // reachability-based, never syntactic presence).
        let model = model_with_shared_handler(true, ArmReachability::ProvenUnreachable);
        assert_eq!(shared_discipline_warnings(&model).len(), 1);
    }

    #[test]
    fn shared_with_proven_reachable_nothandled_does_not_warn() {
        let model = model_with_shared_handler(true, ArmReachability::ProvenReachable);
        assert!(shared_discipline_warnings(&model).is_empty());
    }

    #[test]
    fn unshared_line_never_warns() {
        let model = model_with_shared_handler(false, ArmReachability::Unknown);
        assert!(shared_discipline_warnings(&model).is_empty());
    }

    #[test]
    fn registry_runs_interrupt_tenant() {
        // End-to-end through bind_all/validate_l2_surface: the tenant is
        // registered and its missing-section check fires through the crate
        // surface.
        let mut model = model_with_resource("uart_irq", InterruptForm::Single);
        let b = bindings("version = \"1\"\n");
        crate::bind_all(&mut model, &b).expect("bind_all");
        let err = crate::validate_l2_surface(&model).unwrap_err();
        assert_eq!(err.code(), "IRQ-L2-003");
    }
}
