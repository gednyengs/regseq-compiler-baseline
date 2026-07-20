//! Pin-controller-and-GPIO-configuration L2 tenant: the `[pinctrl.<id>]` join.
//!
//! `bind` joins each recognized `PinCtrl`/`Option<PinCtrl>` resource to its
//! `[pinctrl.<id>]` platform section by the mandatory L1 `id` (R06/D07 — no
//! field-name fallback), interning the `pin_states` inventory into the typed
//! [`regseq_model::PinctrlBinding`] on the model. Pinctrl is *state-selection
//! only* (D08): there is no mode/provider/frequency — the binding records just
//! the names the driver may select.
//!
//! `bind` validates section CONTENT during the join (T11): the reverse-join
//! orphan check (`ORPHAN-SECTION`), and per joined section the `"default"`
//! membership (`NO-DEFAULT`), non-empty inventory (`EMPTY-STATES`), non-empty
//! names (`EMPTY-STATE-NAME`), and uniqueness (`DUPLICATE-STATE`). `validate`
//! enforces section presence for the **required** `PinCtrl` form: a required
//! resource with no matching `[pinctrl.<id>]` section is a binding error
//! (`PIN-L2-NO-INVENTORY`, C06/D03). The **optional** `Option<PinCtrl>` form may
//! legitimately have no section (binds to `None`, `resolved = false`, D06).
//!
//! SHAPE errors (missing `pin_states` key, non-array value, non-string entry) are
//! the parser's `BadPinctrl` (C01/T03), NOT `PIN-L2` (SF6); `PIN-L2-MANY-MATCH` is
//! enforced-by-construction (explicit-id join + TOML duplicate-section rejection +
//! `PIN-ERR-DUPLICATE-ID`). The binding *validity* predicate is
//! [`regseq_model::PinctrlBinding::is_resolved`], the single source of truth the
//! backends consult (GRILL C02).
//!
//! Registered in the `REGISTRY` after `clock` (pins-before-clock is a lowering
//! ordering, D08; the L2 bind order reads no clock state, the order is pinned for
//! determinism, GRILL D02).

use regseq_config::PlatformBindings;
use regseq_model::{PinctrlBinding, RegseqModel};

/// `[pinctrl.<id>]` surface diagnostics (pin-controller-and-GPIO-configuration
/// construct). The `PIN-L2-*` family is harness-named (the C06 contract names the
/// checks, not the IDs), mirroring the `CLK-L2-*` / `IRQ-L2-*` convention. These
/// are the binding (CONTENT) checks; SHAPE errors (missing `pin_states` key,
/// non-array value, non-string entry) are the parser's `BadPinctrl` (C01/T03), so
/// a typed `Vec<String>` inventory reaches L2 already shape-valid (SF6).
///
/// `PIN-L2-MANY-MATCH` (a field resolving to >1 section) is **enforced by
/// construction** — the explicit-`id` join + TOML duplicate-section rejection +
/// the `PIN-ERR-DUPLICATE-ID` recognition check together leave no input that
/// resolves one field to more than one section — so it carries no variant here
/// (C06; no reject fixture is fabricated for an unreachable input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinctrlL2Error {
    /// PIN-L2-001: a required `PinCtrl` resource has no `[pinctrl.<id>]` section
    /// (C06 NO-INVENTORY / D03). The optional `Option<PinCtrl>` form is exempt
    /// (no section ⇒ binds to `None`).
    NoInventory {
        /// The resource id (the missing section's key).
        id: String,
    },
    /// PIN-L2-002: a present `[pinctrl.<id>]` section whose `pin_states` does not
    /// contain `"default"` (C06 NO-DEFAULT / D07). `acquire_pinctrl`'s
    /// default-apply has defined behavior only when the inventory declares it.
    NoDefault {
        /// The resource id.
        id: String,
    },
    /// PIN-L2-003: a present section with an empty `pin_states` array (C06
    /// EMPTY-STATES). The array is shape-valid (parser) but carries no states.
    EmptyStates {
        /// The resource id.
        id: String,
    },
    /// PIN-L2-004: a `pin_states` entry that is empty or whitespace-only (C06
    /// EMPTY-STATE-NAME). A state name must be a non-empty token.
    EmptyStateName {
        /// The resource id.
        id: String,
    },
    /// PIN-L2-005: a `pin_states` array with a duplicated state name (C06
    /// DUPLICATE-STATE). The inventory is a set of distinct selectable states.
    DuplicateState {
        /// The resource id.
        id: String,
        /// The duplicated state name.
        state: String,
    },
    /// PIN-L2-006: a `[pinctrl.<id>]` section that matches no L1
    /// `#[rseq::pinctrl(id = "…")]` resource (C06 ORPHAN-SECTION; reverse join).
    OrphanSection {
        /// The orphan section id.
        id: String,
    },
}

impl PinctrlL2Error {
    /// The diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            PinctrlL2Error::NoInventory { .. } => "PIN-L2-001",
            PinctrlL2Error::NoDefault { .. } => "PIN-L2-002",
            PinctrlL2Error::EmptyStates { .. } => "PIN-L2-003",
            PinctrlL2Error::EmptyStateName { .. } => "PIN-L2-004",
            PinctrlL2Error::DuplicateState { .. } => "PIN-L2-005",
            PinctrlL2Error::OrphanSection { .. } => "PIN-L2-006",
        }
    }
}

impl std::fmt::Display for PinctrlL2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinctrlL2Error::NoInventory { id } => write!(
                f,
                "PIN-L2-001: pinctrl resource `{id}` has no [pinctrl.{id}] platform section \
                 (required for a `PinCtrl`; C06 NO-INVENTORY, D03)"
            ),
            PinctrlL2Error::NoDefault { id } => write!(
                f,
                "PIN-L2-002: [pinctrl.{id}] pin_states must contain \"default\" \
                 (C06 NO-DEFAULT, D07)"
            ),
            PinctrlL2Error::EmptyStates { id } => write!(
                f,
                "PIN-L2-003: [pinctrl.{id}] pin_states is empty (C06 EMPTY-STATES)"
            ),
            PinctrlL2Error::EmptyStateName { id } => write!(
                f,
                "PIN-L2-004: [pinctrl.{id}] pin_states has an empty or whitespace-only entry \
                 (C06 EMPTY-STATE-NAME)"
            ),
            PinctrlL2Error::DuplicateState { id, state } => write!(
                f,
                "PIN-L2-005: [pinctrl.{id}] pin_states has a duplicate state `{state}` \
                 (C06 DUPLICATE-STATE)"
            ),
            PinctrlL2Error::OrphanSection { id } => write!(
                f,
                "PIN-L2-006: [pinctrl.{id}] matches no `#[rseq::pinctrl(id = \"{id}\")]` resource \
                 (C06 ORPHAN-SECTION)"
            ),
        }
    }
}

/// The pin-controller-and-GPIO-configuration L2 tenant.
pub struct PinctrlBinder;

impl crate::L2Binder for PinctrlBinder {
    fn name(&self) -> &'static str {
        "pin-controller-and-gpio-configuration"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_pinctrl_bindings(model, bindings).map_err(crate::L2Error::Pinctrl)
    }

    fn validate(&self, model: &RegseqModel) -> Result<(), crate::L2Error> {
        validate_pinctrl_surface(model).map_err(crate::L2Error::Pinctrl)
    }
}

/// Join every recognized resource's `[pinctrl.<id>]` section (by explicit `id`)
/// into its typed model binding, interning the `pin_states` inventory. An absent
/// section leaves the binding unresolved (`validate` decides whether that is an
/// error: required ⇒ `NO-INVENTORY`, optional ⇒ allowed `None`).
///
/// CONTENT validation (T11) happens here, during the join: the reverse-join
/// orphan check (`ORPHAN-SECTION`), then per-section the `"default"` membership
/// (`NO-DEFAULT`), non-empty inventory (`EMPTY-STATES`), non-empty names
/// (`EMPTY-STATE-NAME`), and uniqueness (`DUPLICATE-STATE`). The SHAPE — a
/// present, array-of-strings `pin_states` — was already enforced by the parser
/// (`BadPinctrl`, C01/T03), so a non-string entry never reaches here (SF6).
fn join_pinctrl_bindings(
    model: &mut RegseqModel,
    bindings: &PlatformBindings,
) -> Result<(), PinctrlL2Error> {
    // Reverse join (C06 ORPHAN-SECTION): every `[pinctrl.<id>]` section must match
    // some L1 `#[rseq::pinctrl]` resource. Collect the L1 ids (owned, so no borrow
    // of `model.strings` lingers into the mutable pass 2). A shared id legitimately
    // matches multiple drivers — the set membership check still passes.
    let resource_ids: std::collections::BTreeSet<String> = model
        .pinctrl
        .drivers
        .values()
        .flat_map(|d| d.resources.iter())
        .map(|r| model.strings.resolve(r.pinctrl_id).to_string())
        .collect();
    for section_id in bindings.pinctrl.keys() {
        if !resource_ids.contains(section_id) {
            return Err(PinctrlL2Error::OrphanSection {
                id: section_id.clone(),
            });
        }
    }

    // Pass 1 (immutable): resolve each resource's section to an owned Vec<String>
    // inventory, validating its CONTENT (the same invariant the backend emit gate
    // `is_resolved()` + the "default" default-apply rely on). The interning in
    // pass 2 holds `&mut`.
    struct Resolved {
        driver: regseq_model::ItemId,
        resource_idx: usize,
        pin_states: Vec<String>,
    }
    let mut resolved: Vec<Resolved> = Vec::new();
    for (&driver, tables) in model.pinctrl.drivers.iter() {
        for (resource_idx, resource) in tables.resources.iter().enumerate() {
            let id = model.strings.resolve(resource.pinctrl_id);
            let Some(spec) = bindings.pinctrl.get(id) else {
                continue;
            };
            // CONTENT validation (C06; SHAPE already enforced by the parser).
            if spec.pin_states.is_empty() {
                return Err(PinctrlL2Error::EmptyStates { id: id.to_string() });
            }
            let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
            let mut has_default = false;
            for state in &spec.pin_states {
                if state.trim().is_empty() {
                    return Err(PinctrlL2Error::EmptyStateName { id: id.to_string() });
                }
                if !seen.insert(state.as_str()) {
                    return Err(PinctrlL2Error::DuplicateState {
                        id: id.to_string(),
                        state: state.clone(),
                    });
                }
                if state == "default" {
                    has_default = true;
                }
            }
            if !has_default {
                return Err(PinctrlL2Error::NoDefault { id: id.to_string() });
            }
            resolved.push(Resolved {
                driver,
                resource_idx,
                pin_states: spec.pin_states.clone(),
            });
        }
    }
    // Pass 2 (mutable): intern the state names + apply the typed binding.
    for r in resolved {
        let pin_states: Vec<_> = r
            .pin_states
            .iter()
            .map(|s| model.strings.intern(s))
            .collect();
        if let Some(tables) = model.pinctrl.drivers.get_mut(&r.driver) {
            tables.resources[r.resource_idx].binding = PinctrlBinding {
                pin_states,
                resolved: true,
            };
        }
    }
    Ok(())
}

/// Section-presence validation (C06 NO-INVENTORY / D03): every **required**
/// `PinCtrl` resource must have joined a section. The **optional**
/// `Option<PinCtrl>` form may stay unresolved (binds to `None`, D06), so the
/// `required` guard gates the check.
fn validate_pinctrl_surface(model: &RegseqModel) -> Result<(), PinctrlL2Error> {
    for tables in model.pinctrl.drivers.values() {
        for resource in &tables.resources {
            if resource.binding.resolved {
                continue;
            }
            if !resource.required {
                continue;
            }
            return Err(PinctrlL2Error::NoInventory {
                id: model.strings.resolve(resource.pinctrl_id).to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::L2Binder;
    use regseq_model::{DriverPinctrls, ItemId, PinctrlResource, RegseqModel, StableCrateId};

    fn model_with_pinctrl(id: &str, required: bool) -> RegseqModel {
        let mut model = RegseqModel::with_primary("pinctrl-l2-test", StableCrateId(1));
        let name = model.strings.intern("pinctrl_field");
        let pinctrl_id = model.strings.intern(id);
        model.pinctrl.drivers.insert(
            ItemId::from_u32(0),
            DriverPinctrls {
                resources: vec![PinctrlResource {
                    name,
                    pinctrl_id,
                    required,
                    binding: Default::default(),
                }],
            },
        );
        model
    }

    fn bindings(toml: &str) -> PlatformBindings {
        regseq_config::parse_platform_bindings_str(toml).expect("parse")
    }

    fn sole_binding(model: &RegseqModel) -> PinctrlBinding {
        model.pinctrl.drivers[&ItemId::from_u32(0)].resources[0]
            .binding
            .clone()
    }

    #[test]
    fn joins_present_section_by_explicit_id() {
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"default\", \"sleep\"]\n");
        PinctrlBinder.bind(&mut model, &b).expect("bind");
        PinctrlBinder.validate(&model).expect("validate");
        let binding = sole_binding(&model);
        assert!(binding.resolved);
        assert!(binding.is_resolved(), "a present section is fully resolved");
        // The inventory is interned and non-empty.
        assert_eq!(binding.pin_states.len(), 2);
        let names: Vec<_> = binding
            .pin_states
            .iter()
            .map(|s| model.strings.resolve(*s).to_string())
            .collect();
        assert_eq!(names, vec!["default".to_string(), "sleep".to_string()]);
    }

    #[test]
    fn optional_no_section_stays_unresolved() {
        // The optional-absent case (D06): no `[pinctrl.<id>]` section ⇒ the
        // binding stays `resolved = false` (binds to `None`); validate accepts it.
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[mmio.regs]\nbase = 0\nsize = 16\n");
        PinctrlBinder
            .bind(&mut model, &b)
            .expect("bind is a no-op join");
        PinctrlBinder
            .validate(&model)
            .expect("optional-absent is allowed");
        let binding = sole_binding(&model);
        assert!(!binding.resolved);
        assert!(!binding.is_resolved());
        assert!(binding.pin_states.is_empty());
    }

    #[test]
    fn required_without_section_is_no_inventory() {
        // A required `PinCtrl` with no matching section is PIN-L2-NO-INVENTORY
        // (C06/D03), parallel to clock's CLK-L2-001.
        let mut model = model_with_pinctrl("uart_pins", true);
        let b = bindings("[mmio.regs]\nbase = 0\nsize = 16\n");
        PinctrlBinder
            .bind(&mut model, &b)
            .expect("bind is a no-op join");
        let err = PinctrlBinder.validate(&model).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-001");
        assert!(err.to_string().contains("uart_pins"), "{err}");
    }

    #[test]
    fn required_with_section_resolves() {
        let mut model = model_with_pinctrl("uart_pins", true);
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"default\"]\n");
        PinctrlBinder.bind(&mut model, &b).expect("bind");
        PinctrlBinder.validate(&model).expect("validate");
        assert!(sole_binding(&model).is_resolved());
    }

    #[test]
    fn shared_id_joins_two_drivers_to_one_section() {
        // Two drivers declaring the same pinctrl id legitimately share one global
        // section (parallel to clock's shared-id join).
        let mut model = model_with_pinctrl("uart_pins", false);
        let name = model.strings.intern("pinctrl_field");
        let pinctrl_id = model.strings.intern("uart_pins");
        model.pinctrl.drivers.insert(
            ItemId::from_u32(1),
            DriverPinctrls {
                resources: vec![PinctrlResource {
                    name,
                    pinctrl_id,
                    required: false,
                    binding: Default::default(),
                }],
            },
        );
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"default\"]\n");
        PinctrlBinder.bind(&mut model, &b).expect("bind");
        PinctrlBinder.validate(&model).expect("validate");
        for item in [ItemId::from_u32(0), ItemId::from_u32(1)] {
            assert!(model.pinctrl.drivers[&item].resources[0]
                .binding
                .is_resolved());
        }
    }

    #[test]
    fn registry_runs_pinctrl_tenant() {
        // End-to-end through bind_all/validate_l2_surface: the tenant is
        // registered and its required-form NO-INVENTORY check fires through the
        // crate surface.
        let mut model = model_with_pinctrl("uart_pins", true);
        let b = bindings("version = \"1\"\n");
        crate::bind_all(&mut model, &b).expect("bind_all");
        let err = crate::validate_l2_surface(&model).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-001");
    }

    // --- T11 CONTENT negative set (C06) ---

    #[test]
    fn section_without_default_is_no_default() {
        // A present section whose inventory lacks "default" is PIN-L2-002
        // (NO-DEFAULT, D07) — `acquire_pinctrl` has no defined default to apply.
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"sleep\"]\n");
        let err = PinctrlBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-002");
        assert!(err.to_string().contains("default"), "{err}");
    }

    #[test]
    fn empty_states_is_rejected() {
        // An empty (but shape-valid) array is PIN-L2-003 (EMPTY-STATES).
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[pinctrl.uart_pins]\npin_states = []\n");
        let err = PinctrlBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-003");
    }

    #[test]
    fn empty_state_name_is_rejected() {
        // A whitespace-only entry is PIN-L2-004 (EMPTY-STATE-NAME) — a CONTENT
        // error, distinct from a non-string entry (parser BadPinctrl, SF6).
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"default\", \"  \"]\n");
        let err = PinctrlBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-004");
    }

    #[test]
    fn duplicate_state_is_rejected() {
        // A repeated state name is PIN-L2-005 (DUPLICATE-STATE).
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings("[pinctrl.uart_pins]\npin_states = [\"default\", \"sleep\", \"sleep\"]\n");
        let err = PinctrlBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-005");
        assert!(err.to_string().contains("sleep"), "{err}");
    }

    #[test]
    fn orphan_section_is_rejected() {
        // A `[pinctrl.<id>]` section matching no L1 resource is PIN-L2-006
        // (ORPHAN-SECTION; reverse join), parallel to clock's CLK-L2-006.
        let mut model = model_with_pinctrl("uart_pins", false);
        let b = bindings(
            "[pinctrl.uart_pins]\npin_states = [\"default\"]\n\
             [pinctrl.spi_pins]\npin_states = [\"default\"]\n",
        );
        let err = PinctrlBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "PIN-L2-006");
        assert!(err.to_string().contains("spi_pins"), "{err}");
    }
}
