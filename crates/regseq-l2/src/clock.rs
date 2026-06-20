//! Peripheral-clock-management L2 tenant: the `[clock.<id>]` join.
//!
//! `bind` joins each recognized `Clock` resource to its `[clock.<id>]` platform
//! section by the mandatory L1 `id` (R06/D13 — no field-name fallback),
//! validating the `mode` token (`managed | static`) and filling the typed
//! [`ClockBinding`] on the model (`mode`, `default_frequency`, `provider`). The
//! `provider` string is interned into the model; it is opaque and
//! backend-interpreted (Linux con-id / Zephyr DT clock spec — D14).
//!
//! `validate` enforces section presence: a required `Clock` with no matching
//! `[clock.<id>]` section is a binding error (C06 N3). The full binding negative
//! set — reverse-join (N4), `default_frequency` absence/zero (N5/N18),
//! managed-without-provider (N7/N19), static-with-provider (N8/N20) — is the
//! T10/M2 extension; the mode↔provider↔frequency *validity* is encoded in
//! [`regseq_model::ClockBinding::is_resolved`], the single source of truth the
//! backends consult (GRILL C02/I03).
//!
//! Registered in the `REGISTRY` after `bus-resource` (and after `interrupt`) so
//! it may read earlier-bound state; it reads none today, the order is pinned for
//! determinism (GRILL D02).

use regseq_config::PlatformBindings;
use regseq_model::{ClockBinding, ClockMode, RegseqModel};

/// `[clock.<id>]` surface diagnostics (peripheral-clock-management construct).
/// The `CLK-L2-*` family is harness-named (the C06 contract names the checks,
/// not the IDs), mirroring the `IRQ-L2-*` / `BRM-*` convention. This is the
/// M1/T05 subset (missing-section, bad-mode); T10 extends it with the rest of
/// the binding negative set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockL2Error {
    /// CLK-L2-001: a required `Clock` resource has no `[clock.<id>]` section
    /// (C06 N3).
    MissingSection {
        /// The resource id (the missing section's key).
        id: String,
    },
    /// CLK-L2-002: the `mode` key is missing or outside the `managed | static`
    /// vocabulary (C06 N6).
    BadMode {
        /// The resource id.
        id: String,
        /// The rejected token, verbatim (`<missing>` when the key is absent).
        token: String,
    },
    /// CLK-L2-003: `default_frequency` is missing or `0` (C06 N5/N18; D15 — a
    /// 0 Hz clock cannot compute a baud divisor). The negative/overflow cases
    /// are parse rejects (T03); this is the semantic half.
    MissingDefaultFrequency {
        /// The resource id.
        id: String,
    },
    /// CLK-L2-004: a `managed` clock with no `provider` (C06 N7/N19; a managed
    /// clock MUST carry the backend-interpreted con-id / DT clock spec, D14).
    ManagedWithoutProvider {
        /// The resource id.
        id: String,
    },
    /// CLK-L2-005: a `static` clock that carries a `provider` (C06 N8/N20; a
    /// static clock MUST NOT carry a provider, D14/I03).
    StaticWithProvider {
        /// The resource id.
        id: String,
    },
    /// CLK-L2-006: a `[clock.<id>]` section that matches no L1 `#[rseq::clock]`
    /// resource (C06 N4 — the reverse join; an orphan section is a likely typo).
    OrphanSection {
        /// The orphan section's id.
        id: String,
    },
}

impl ClockL2Error {
    /// The diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            ClockL2Error::MissingSection { .. } => "CLK-L2-001",
            ClockL2Error::BadMode { .. } => "CLK-L2-002",
            ClockL2Error::MissingDefaultFrequency { .. } => "CLK-L2-003",
            ClockL2Error::ManagedWithoutProvider { .. } => "CLK-L2-004",
            ClockL2Error::StaticWithProvider { .. } => "CLK-L2-005",
            ClockL2Error::OrphanSection { .. } => "CLK-L2-006",
        }
    }
}

impl std::fmt::Display for ClockL2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClockL2Error::MissingSection { id } => write!(
                f,
                "CLK-L2-001: clock resource `{id}` has no [clock.{id}] platform section \
                 (required for a `Clock`; C06 N3)"
            ),
            ClockL2Error::BadMode { id, token } => write!(
                f,
                "CLK-L2-002: [clock.{id}] mode {token:?} is not valid \
                 (expected \"managed\" or \"static\")"
            ),
            ClockL2Error::MissingDefaultFrequency { id } => write!(
                f,
                "CLK-L2-003: [clock.{id}] default_frequency is missing or 0 \
                 (a bound clock requires a positive Hz value; C06 N5/N18, D15)"
            ),
            ClockL2Error::ManagedWithoutProvider { id } => write!(
                f,
                "CLK-L2-004: [clock.{id}] mode = \"managed\" requires a `provider` \
                 (the con-id / DT clock spec; C06 N7/N19, D14)"
            ),
            ClockL2Error::StaticWithProvider { id } => write!(
                f,
                "CLK-L2-005: [clock.{id}] mode = \"static\" must not carry a `provider` \
                 (a static clock has no controller; C06 N8/N20, D14)"
            ),
            ClockL2Error::OrphanSection { id } => write!(
                f,
                "CLK-L2-006: [clock.{id}] matches no `#[rseq::clock(id = \"{id}\")]` resource \
                 (orphan platform section; C06 N4)"
            ),
        }
    }
}

/// The peripheral-clock-management L2 tenant.
pub struct ClockBinder;

impl crate::L2Binder for ClockBinder {
    fn name(&self) -> &'static str {
        "peripheral-clock-management"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_clock_bindings(model, bindings).map_err(crate::L2Error::Clock)
    }

    fn validate(&self, model: &RegseqModel) -> Result<(), crate::L2Error> {
        validate_clock_surface(model).map_err(crate::L2Error::Clock)
    }
}

/// Join every recognized resource's `[clock.<id>]` section (by `id`) into its
/// typed model binding. The `mode` token is validated here (C06 N6); an absent
/// section leaves the binding unresolved (`validate` decides whether that is an
/// error). The `provider` is interned (opaque/backend-interpreted, D14).
fn join_clock_bindings(
    model: &mut RegseqModel,
    bindings: &PlatformBindings,
) -> Result<(), ClockL2Error> {
    // Reverse join (C06 N4): every `[clock.<id>]` section must match some L1
    // `#[rseq::clock]` resource. Collect the L1 ids (owned, so no borrow of
    // `model.strings` lingers into the mutable pass 2 below).
    let resource_ids: std::collections::BTreeSet<String> = model
        .clock
        .drivers
        .values()
        .flat_map(|d| d.resources.iter())
        .map(|r| model.strings.resolve(r.id).to_string())
        .collect();
    for section_id in bindings.clock.keys() {
        if !resource_ids.contains(section_id) {
            return Err(ClockL2Error::OrphanSection {
                id: section_id.clone(),
            });
        }
    }

    // Pass 1 (immutable): resolve each resource's mode + raw facts, validating
    // the full binding invariant (the same predicate `ClockBinding::is_resolved`
    // encodes — C02/I03). The provider string is carried verbatim and interned
    // in pass 2 (which holds `&mut`).
    struct Resolved {
        driver: regseq_model::ItemId,
        resource_idx: usize,
        mode: ClockMode,
        default_frequency: u32,
        provider: Option<String>,
    }
    let mut resolved: Vec<Resolved> = Vec::new();
    for (&driver, tables) in model.clock.drivers.iter() {
        for (resource_idx, resource) in tables.resources.iter().enumerate() {
            let id = model.strings.resolve(resource.id);
            let Some(spec) = bindings.clock.get(id) else {
                continue;
            };
            // Mode (C06 N6).
            let mode = match spec.mode.as_deref() {
                Some("managed") => ClockMode::Managed,
                Some("static") => ClockMode::Static,
                Some(other) => {
                    return Err(ClockL2Error::BadMode {
                        id: id.to_string(),
                        token: other.to_string(),
                    })
                }
                None => {
                    return Err(ClockL2Error::BadMode {
                        id: id.to_string(),
                        token: "<missing>".to_string(),
                    })
                }
            };
            // default_frequency present and > 0 (C06 N5/N18, D15).
            let default_frequency = match spec.default_frequency {
                Some(f) if f > 0 => f,
                _ => {
                    return Err(ClockL2Error::MissingDefaultFrequency { id: id.to_string() })
                }
            };
            // The mode↔provider invariant (C06 N7/N8, D14/I03) — agrees exactly
            // with `ClockBinding::is_resolved`.
            match mode {
                ClockMode::Managed if spec.provider.is_none() => {
                    return Err(ClockL2Error::ManagedWithoutProvider { id: id.to_string() })
                }
                ClockMode::Static if spec.provider.is_some() => {
                    return Err(ClockL2Error::StaticWithProvider { id: id.to_string() })
                }
                _ => {}
            }
            resolved.push(Resolved {
                driver,
                resource_idx,
                mode,
                default_frequency,
                provider: spec.provider.clone(),
            });
        }
    }
    // Pass 2 (mutable): intern the provider + apply the typed binding.
    for r in resolved {
        let provider = r.provider.map(|p| model.strings.intern(&p));
        if let Some(tables) = model.clock.drivers.get_mut(&r.driver) {
            tables.resources[r.resource_idx].binding = ClockBinding {
                mode: r.mode,
                default_frequency: Some(r.default_frequency),
                provider,
                resolved: true,
            };
        }
    }
    Ok(())
}

/// Section-presence validation (C06 N3): every required `Clock` resource must
/// have joined a section. v1 has no optional form (`Option<Clock>` is rejected
/// at recognition), so an unresolved required resource is always an error; the
/// `required` guard is forward-compat for the future optional form.
fn validate_clock_surface(model: &RegseqModel) -> Result<(), ClockL2Error> {
    for tables in model.clock.drivers.values() {
        for resource in &tables.resources {
            if resource.binding.resolved {
                continue;
            }
            if !resource.required {
                continue;
            }
            return Err(ClockL2Error::MissingSection {
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
    use regseq_model::{ClockResource, DriverClocks, ItemId, RegseqModel, StableCrateId};

    fn model_with_clock(id: &str) -> RegseqModel {
        let mut model = RegseqModel::with_primary("clock-l2-test", StableCrateId(1));
        let name = model.strings.intern("clk_field");
        let id = model.strings.intern(id);
        model.clock.drivers.insert(
            ItemId::from_u32(0),
            DriverClocks {
                resources: vec![ClockResource {
                    name,
                    id,
                    required: true,
                    binding: Default::default(),
                }],
            },
        );
        model
    }

    fn bindings(toml: &str) -> PlatformBindings {
        regseq_config::parse_platform_bindings_str(toml).expect("parse")
    }

    fn sole_binding(model: &RegseqModel) -> ClockBinding {
        model.clock.drivers[&ItemId::from_u32(0)].resources[0]
            .binding
            .clone()
    }

    #[test]
    fn joins_managed_section() {
        let mut model = model_with_clock("uartclk");
        let b = bindings(
            "[clock.uartclk]\nmode = \"managed\"\ndefault_frequency = 24000000\n\
             provider = \"uartclk\"\n",
        );
        ClockBinder.bind(&mut model, &b).expect("bind");
        ClockBinder.validate(&model).expect("validate");
        let binding = sole_binding(&model);
        assert_eq!(binding.mode, ClockMode::Managed);
        assert_eq!(binding.default_frequency, Some(24_000_000));
        assert!(binding.provider.is_some());
        assert!(binding.resolved);
        assert!(binding.is_resolved(), "a valid managed binding is fully resolved");
    }

    #[test]
    fn joins_static_section() {
        let mut model = model_with_clock("refclk");
        let b = bindings("[clock.refclk]\nmode = \"static\"\ndefault_frequency = 48000000\n");
        ClockBinder.bind(&mut model, &b).expect("bind");
        ClockBinder.validate(&model).expect("validate");
        let binding = sole_binding(&model);
        assert_eq!(binding.mode, ClockMode::Static);
        assert_eq!(binding.default_frequency, Some(48_000_000));
        assert!(binding.provider.is_none());
        assert!(binding.is_resolved(), "a valid static binding is fully resolved");
    }

    #[test]
    fn missing_section_on_required_clock_is_an_error() {
        let mut model = model_with_clock("uartclk");
        let b = bindings("[mmio.regs]\nbase = 0\nsize = 16\n");
        ClockBinder.bind(&mut model, &b).expect("bind is a no-op join");
        let err = ClockBinder.validate(&model).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-001");
        assert!(err.to_string().contains("uartclk"), "{err}");
    }

    #[test]
    fn invalid_mode_is_rejected() {
        let mut model = model_with_clock("uartclk");
        let b = bindings("[clock.uartclk]\nmode = \"fast\"\ndefault_frequency = 1\n");
        let err = ClockBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-002");
        assert!(err.to_string().contains("\"fast\""), "{err}");
    }

    #[test]
    fn missing_mode_is_rejected() {
        let mut model = model_with_clock("uartclk");
        let b = bindings("[clock.uartclk]\ndefault_frequency = 1\n");
        let err = ClockBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-002");
    }

    #[test]
    fn shared_id_joins_two_drivers_to_one_section() {
        // Two drivers declaring the same clock id legitimately share one global
        // section (one physical clock feeding two consumers — D13).
        let mut model = model_with_clock("sysclk");
        let name = model.strings.intern("clk_field");
        let id = model.strings.intern("sysclk");
        model.clock.drivers.insert(
            ItemId::from_u32(1),
            DriverClocks {
                resources: vec![ClockResource {
                    name,
                    id,
                    required: true,
                    binding: Default::default(),
                }],
            },
        );
        let b = bindings("[clock.sysclk]\nmode = \"static\"\ndefault_frequency = 1000000\n");
        ClockBinder.bind(&mut model, &b).expect("bind");
        ClockBinder.validate(&model).expect("validate");
        for item in [ItemId::from_u32(0), ItemId::from_u32(1)] {
            assert!(model.clock.drivers[&item].resources[0].binding.is_resolved());
        }
    }

    #[test]
    fn orphan_section_is_rejected() {
        // A `[clock.<id>]` section matching no L1 `#[rseq::clock]` resource is a
        // reverse-join reject (C06 N4, owned by T10) — UNLIKE the accept-and-
        // ignore discipline of `[mmio.<id>]`/`[interrupt.<id>]`. Here the model
        // has no clock resources at all, so the section is an orphan.
        let mut model = RegseqModel::with_primary("plain", StableCrateId(1));
        let b = bindings("[clock.orphan]\nmode = \"static\"\ndefault_frequency = 1\n");
        let err = ClockBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-006");
        assert!(err.to_string().contains("orphan"), "{err}");
    }

    #[test]
    fn missing_or_zero_default_frequency_is_rejected() {
        // No default_frequency.
        let mut model = model_with_clock("uartclk");
        let b = bindings("[clock.uartclk]\nmode = \"static\"\n");
        assert_eq!(ClockBinder.bind(&mut model, &b).unwrap_err().code(), "CLK-L2-003");
        // default_frequency = 0 (D15: a 0 Hz clock cannot compute a baud divisor).
        let mut model = model_with_clock("uartclk");
        let b = bindings("[clock.uartclk]\nmode = \"static\"\ndefault_frequency = 0\n");
        assert_eq!(ClockBinder.bind(&mut model, &b).unwrap_err().code(), "CLK-L2-003");
    }

    #[test]
    fn managed_without_provider_is_rejected() {
        let mut model = model_with_clock("uartclk");
        let b = bindings("[clock.uartclk]\nmode = \"managed\"\ndefault_frequency = 24000000\n");
        let err = ClockBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-004");
    }

    #[test]
    fn static_with_provider_is_rejected() {
        let mut model = model_with_clock("refclk");
        let b = bindings(
            "[clock.refclk]\nmode = \"static\"\ndefault_frequency = 48000000\nprovider = \"x\"\n",
        );
        let err = ClockBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-005");
    }

    #[test]
    fn l2_invariant_agrees_with_is_resolved() {
        // Every binding the tenant accepts must be `is_resolved()`; every shape it
        // rejects would be `!is_resolved()` (C02/I03 consistency).
        let mut model = model_with_clock("uartclk");
        let b = bindings(
            "[clock.uartclk]\nmode = \"managed\"\ndefault_frequency = 24000000\n\
             provider = \"uartclk\"\n",
        );
        ClockBinder.bind(&mut model, &b).expect("bind");
        assert!(sole_binding(&model).is_resolved());
    }

    #[test]
    fn registry_runs_clock_tenant() {
        // End-to-end through bind_all/validate_l2_surface: the tenant is
        // registered and its missing-section check fires through the crate
        // surface.
        let mut model = model_with_clock("uartclk");
        let b = bindings("version = \"1\"\n");
        crate::bind_all(&mut model, &b).expect("bind_all");
        let err = crate::validate_l2_surface(&model).unwrap_err();
        assert_eq!(err.code(), "CLK-L2-001");
    }
}
