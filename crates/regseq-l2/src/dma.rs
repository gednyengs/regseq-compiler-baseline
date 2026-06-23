//! DMA-management L2 tenant: the `[dma.<id>]` join (M1 region path).
//!
//! `bind` joins each recognized coherent `DmaRegion<T>` resource to its
//! `[dma.<id>]` platform section by the mandatory L1 `id`, filling the typed
//! [`DmaRegionBinding`] on the model: `device_address_bits` (was `dma_mask`), the
//! optional `alignment` (OMITTED ⇒ `align_of::<T>()`, NOT 1 — GRILL C02/D08; the
//! default is recovered by a later layout pass, so an omitted L2 `alignment`
//! leaves the binding's `alignment = None`), and the `region_cache_maintenance`
//! gate (`none | per_access`, validated here — GRILL D03). The
//! per_access-needs-cacheline-isolation precondition (FM4) is RECORDED only this
//! cycle (a doc note), not enforced (GRILL I03/O3).
//!
//! `validate` enforces section presence: a recognized region with no matching
//! `[dma.<id>]` section is a binding error (the M1 region is the required form).
//! The reverse-join (an orphan `[dma.<id>]` section) is recorded here too.
//!
//! Registered in the `REGISTRY` after `pinctrl` (and after `clock`) so it may
//! read earlier-bound state; it reads none today, the order is pinned for
//! determinism.

use regseq_config::PlatformBindings;
use regseq_model::{DmaRegionBinding, RegionCacheMaintenance, RegseqModel};

/// `[dma.<id>]` surface diagnostics (dma-management construct, M1 region path).
/// The `DMA-L2-*` family is harness-named (the C02 contract names the checks, not
/// the IDs), mirroring the `CLK-L2-*` / `PIN-L2-*` convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmaL2Error {
    /// DMA-L2-001: a recognized `DmaRegion` resource has no `[dma.<id>]` section
    /// (the M1 region is the required form, GRILL C02).
    MissingSection {
        /// The resource id (the missing section's key).
        id: String,
    },
    /// DMA-L2-002: the `region_cache_maintenance` key is outside the
    /// `none | per_access` vocabulary (GRILL D03).
    BadCacheMaintenance {
        /// The resource id.
        id: String,
        /// The rejected token, verbatim.
        token: String,
    },
    /// DMA-L2-003: a `[dma.<id>]` section that matches no L1 `#[rseq::dma_region]`
    /// (or `#[rseq::dma_channel]`) resource (the reverse join — an orphan section
    /// is a likely typo).
    OrphanSection {
        /// The orphan section's id.
        id: String,
    },
    /// DMA-L2-004: a `[dma.<id>]` `binding_scope` token outside the closed
    /// `concrete | template | family | conditional` vocabulary (GRILL D07).
    BadBindingScope {
        /// The resource id.
        id: String,
        /// The rejected token, verbatim.
        token: String,
    },
    /// DMA-L2-005: a `[dma.<id>]` capability flag `cap_<name>` whose value is
    /// outside the closed `present | absent | unknown` vocabulary (GRILL C02/D07).
    BadCapabilityValue {
        /// The resource id.
        id: String,
        /// The capability name (`cap_<name>` ⇒ `<name>`).
        cap: String,
        /// The rejected token, verbatim.
        token: String,
    },
    /// DMA-L2-006: a capability flag `cap_<name> = "absent"` in a NON-CONCRETE
    /// (`template` / `family` / `conditional`) `[dma.<id>]` binding (GRILL D07/C04).
    /// `absent` is authoritative ONLY in a concrete binding (a fully-resolved
    /// controller/channel); a non-concrete binding must use `unknown` (the resolved
    /// controller is not yet known, so its capability cannot be statically denied).
    AbsentCapabilityInTemplate {
        /// The resource id.
        id: String,
        /// The capability name marked `absent`.
        cap: String,
        /// The non-concrete binding scope it was found in.
        scope: String,
    },
}

impl DmaL2Error {
    /// The diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            DmaL2Error::MissingSection { .. } => "DMA-L2-001",
            DmaL2Error::BadCacheMaintenance { .. } => "DMA-L2-002",
            DmaL2Error::OrphanSection { .. } => "DMA-L2-003",
            DmaL2Error::BadBindingScope { .. } => "DMA-L2-004",
            DmaL2Error::BadCapabilityValue { .. } => "DMA-L2-005",
            DmaL2Error::AbsentCapabilityInTemplate { .. } => "DMA-L2-006",
        }
    }
}

impl std::fmt::Display for DmaL2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DmaL2Error::MissingSection { id } => write!(
                f,
                "DMA-L2-001: dma resource `{id}` has no [dma.{id}] platform section \
                 (required for a coherent `DmaRegion<T>`; C02)"
            ),
            DmaL2Error::BadCacheMaintenance { id, token } => write!(
                f,
                "DMA-L2-002: [dma.{id}] region_cache_maintenance {token:?} is not valid \
                 (expected \"none\" or \"per_access\"; D03)"
            ),
            DmaL2Error::OrphanSection { id } => write!(
                f,
                "DMA-L2-003: [dma.{id}] matches no `#[rseq::dma_region(id = \"{id}\")]` resource \
                 (orphan platform section)"
            ),
            DmaL2Error::BadBindingScope { id, token } => write!(
                f,
                "DMA-L2-004: [dma.{id}] binding_scope {token:?} is not valid \
                 (expected \"concrete\", \"template\", \"family\", or \"conditional\"; D07)"
            ),
            DmaL2Error::BadCapabilityValue { id, cap, token } => write!(
                f,
                "DMA-L2-005: [dma.{id}] cap_{cap} = {token:?} is not valid \
                 (expected \"present\", \"absent\", or \"unknown\"; C02/D07)"
            ),
            DmaL2Error::AbsentCapabilityInTemplate { id, cap, scope } => write!(
                f,
                "DMA-L2-006: [dma.{id}] cap_{cap} = \"absent\" is not allowed in a non-concrete \
                 binding (binding_scope = {scope:?}); `absent` is authoritative only in a concrete \
                 binding — a {scope} binding must use \"unknown\" (D07/C04)"
            ),
        }
    }
}

/// The dma-management L2 tenant.
pub struct DmaBinder;

impl crate::L2Binder for DmaBinder {
    fn name(&self) -> &'static str {
        "dma-management"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_dma_bindings(model, bindings).map_err(crate::L2Error::Dma)
    }

    fn validate(&self, model: &RegseqModel) -> Result<(), crate::L2Error> {
        validate_dma_surface(model).map_err(crate::L2Error::Dma)
    }
}

/// Join every recognized region's `[dma.<id>]` section (by `id`) into its typed
/// model binding. The `region_cache_maintenance` token is validated here; an
/// absent section leaves the binding unresolved (`validate` decides whether that
/// is an error). An omitted `alignment` leaves the binding's `alignment = None`
/// (the `align_of::<T>()` default is recovered by a later layout pass, GRILL
/// D08/C03).
fn join_dma_bindings(
    model: &mut RegseqModel,
    bindings: &PlatformBindings,
) -> Result<(), DmaL2Error> {
    // Reverse join: every `[dma.<id>]` section must match some L1 DMA resource
    // (a region id, or — once M2 lands — a channel id). Collect the L1 ids owned.
    let resource_ids: std::collections::BTreeSet<String> = model
        .dma
        .drivers
        .values()
        .flat_map(|d| {
            d.regions
                .iter()
                .map(|r| model.strings.resolve(r.id).to_string())
                .chain(d.channels.iter().map(|c| model.strings.resolve(c.id).to_string()))
        })
        .collect();
    for (section_id, spec) in bindings.dma.iter() {
        if !resource_ids.contains(section_id) {
            return Err(DmaL2Error::OrphanSection {
                id: section_id.clone(),
            });
        }
        // Capability-gating contract (GRILL D07/C04, PLAN T21). The capability OPS
        // are designed-not-built this cycle; the ONLY buildable C04 oracle is the L2
        // rule that an `absent` capability is authoritative ONLY in a concrete
        // binding. Validate, per section:
        //   * `binding_scope` is in the closed vocabulary (omitted ⇒ concrete);
        //   * each `cap_*` value is in the `present | absent | unknown` vocabulary;
        //   * a `cap_* = "absent"` in a NON-concrete binding is rejected (D07): the
        //     resolved controller is unknown there, so its capability cannot be
        //     statically denied — a template/family/conditional binding must use
        //     `unknown`. (A concrete binding's `absent` is ACCEPTED by this rule;
        //     there is no cap-op to reject yet — designed-not-built, Codex R1#9.)
        validate_capability_scope(section_id, spec)?;
    }

    // Pass 1 (immutable): resolve each region's raw facts, validating the
    // `region_cache_maintenance` vocabulary. (No interned strings, so a single
    // mutable pass would also work; the two-pass shape mirrors `clock.rs`.)
    struct Resolved {
        driver: regseq_model::ItemId,
        region_idx: usize,
        device_address_bits: Option<u32>,
        alignment: Option<u64>,
        cache_maintenance: Option<RegionCacheMaintenance>,
    }
    let mut resolved: Vec<Resolved> = Vec::new();
    for (&driver, tables) in model.dma.drivers.iter() {
        for (region_idx, region) in tables.regions.iter().enumerate() {
            let id = model.strings.resolve(region.id);
            let Some(spec) = bindings.dma.get(id) else {
                continue;
            };
            let cache_maintenance = match spec.region_cache_maintenance.as_deref() {
                None => None,
                Some("none") => Some(RegionCacheMaintenance::None),
                Some("per_access") => Some(RegionCacheMaintenance::PerAccess),
                Some(other) => {
                    return Err(DmaL2Error::BadCacheMaintenance {
                        id: id.to_string(),
                        token: other.to_string(),
                    })
                }
            };
            resolved.push(Resolved {
                driver,
                region_idx,
                device_address_bits: spec.device_address_bits,
                alignment: spec.alignment,
                cache_maintenance,
            });
        }
    }
    // Pass 2 (mutable): apply the typed binding.
    for r in resolved {
        if let Some(tables) = model.dma.drivers.get_mut(&r.driver) {
            tables.regions[r.region_idx].binding = DmaRegionBinding {
                device_address_bits: r.device_address_bits,
                alignment: r.alignment,
                cache_maintenance: r.cache_maintenance,
                resolved: true,
            };
        }
    }
    Ok(())
}

/// The closed `binding_scope` vocabulary (GRILL D07). `concrete` (the omitted
/// default) is a fully-resolved controller/channel; the rest are non-concrete.
fn is_concrete_scope(scope: Option<&str>) -> Result<bool, ()> {
    match scope {
        None | Some("concrete") => Ok(true),
        Some("template") | Some("family") | Some("conditional") => Ok(false),
        Some(_) => Err(()),
    }
}

/// The capability-gating contract validation for one `[dma.<id>]` binding (GRILL
/// D07/C04, PLAN T21): the `binding_scope` vocabulary, the `cap_*` value
/// vocabulary, and the D07 rule that `cap_* = "absent"` is legal ONLY in a
/// concrete binding. The capability OPS are designed-not-built this cycle; this is
/// the only buildable C04 oracle.
fn validate_capability_scope(
    id: &str,
    spec: &regseq_config::DmaBindingSpec,
) -> Result<(), DmaL2Error> {
    let scope_token = spec.binding_scope.as_deref();
    let is_concrete = is_concrete_scope(scope_token).map_err(|()| DmaL2Error::BadBindingScope {
        id: id.to_string(),
        token: scope_token.unwrap_or("").to_string(),
    })?;
    for (cap, value) in spec.capabilities.iter() {
        match value.as_str() {
            "present" | "unknown" => {}
            "absent" => {
                if !is_concrete {
                    return Err(DmaL2Error::AbsentCapabilityInTemplate {
                        id: id.to_string(),
                        cap: cap.clone(),
                        // The non-concrete scope (omitted defaults to concrete, so a
                        // reach here always has an explicit non-concrete token).
                        scope: scope_token.unwrap_or("template").to_string(),
                    });
                }
                // A concrete-binding `absent` is ACCEPTED by the rule: there is no
                // cap-op to reject this cycle (designed-not-built, Codex R1#9).
            }
            other => {
                return Err(DmaL2Error::BadCapabilityValue {
                    id: id.to_string(),
                    cap: cap.clone(),
                    token: other.to_string(),
                })
            }
        }
    }
    Ok(())
}

/// Section-presence validation: every recognized region must have joined a
/// section (the M1 region is the required form, so an unresolved region is an
/// error).
fn validate_dma_surface(model: &RegseqModel) -> Result<(), DmaL2Error> {
    for tables in model.dma.drivers.values() {
        for region in &tables.regions {
            if region.binding.resolved {
                continue;
            }
            return Err(DmaL2Error::MissingSection {
                id: model.strings.resolve(region.id).to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::L2Binder;
    use regseq_model::{
        DmaRegionFacts, DriverDma, ItemId, RegseqModel, StableCrateId,
    };

    fn model_with_region(id: &str) -> RegseqModel {
        let mut model = RegseqModel::with_primary("dma-l2-test", StableCrateId(1));
        let name = model.strings.intern("tx_ring");
        let id = model.strings.intern(id);
        model.dma.drivers.insert(
            ItemId::from_u32(0),
            DriverDma {
                regions: vec![DmaRegionFacts {
                    name,
                    id,
                    element_size: 16,
                    element_count: 256,
                    alignment: None,
                    element_type: None,
                    element_layout: None,
                    element_accesses: vec![],
                    binding: Default::default(),
                }],
                channels: vec![],
                streams: vec![],
                spans: vec![],
                completions: vec![],
            },
        );
        model
    }

    /// A model carrying a single slave `DmaChannel` with the given id (so a
    /// `[dma.<id>]` capability section is not an orphan). T21 capability-gating tests
    /// bind cap sections to this channel.
    fn model_with_channel(id: &str) -> RegseqModel {
        use regseq_model::{DmaChannelFacts, DmaFlow};
        let mut model = RegseqModel::with_primary("dma-l2-cap-test", StableCrateId(1));
        let name = model.strings.intern("rx");
        let id = model.strings.intern(id);
        model.dma.drivers.insert(
            ItemId::from_u32(0),
            DriverDma {
                regions: vec![],
                channels: vec![DmaChannelFacts {
                    name,
                    id,
                    direction: DmaFlow::DeviceToHost,
                    binding: Default::default(),
                }],
                streams: vec![],
                spans: vec![],
                completions: vec![],
            },
        );
        model
    }

    fn bindings(toml: &str) -> PlatformBindings {
        regseq_config::parse_platform_bindings_str(toml).expect("parse")
    }

    fn sole_binding(model: &RegseqModel) -> DmaRegionBinding {
        model.dma.drivers[&ItemId::from_u32(0)].regions[0]
            .binding
            .clone()
    }

    #[test]
    fn joins_region_section() {
        let mut model = model_with_region("tx_ring");
        let b = bindings(
            "[dma.tx_ring]\ndevice_address_bits = 32\nalignment = 16\n\
             region_cache_maintenance = \"none\"\n",
        );
        DmaBinder.bind(&mut model, &b).expect("bind");
        DmaBinder.validate(&model).expect("validate");
        let binding = sole_binding(&model);
        assert_eq!(binding.device_address_bits, Some(32));
        assert_eq!(binding.alignment, Some(16));
        assert_eq!(binding.cache_maintenance, Some(RegionCacheMaintenance::None));
        assert!(binding.resolved);
    }

    #[test]
    fn omitted_alignment_stays_none_for_layout_default() {
        // An omitted L2 `alignment` leaves the binding's alignment None; the
        // `align_of::<T>()` default is the layout pass's, NOT 1 (GRILL D08/C03).
        let mut model = model_with_region("tx_ring");
        let b = bindings("[dma.tx_ring]\ndevice_address_bits = 32\n");
        DmaBinder.bind(&mut model, &b).expect("bind");
        DmaBinder.validate(&model).expect("validate");
        assert_eq!(sole_binding(&model).alignment, None);
    }

    #[test]
    fn per_access_cache_maintenance_joins() {
        let mut model = model_with_region("tx_ring");
        let b = bindings("[dma.tx_ring]\nregion_cache_maintenance = \"per_access\"\n");
        DmaBinder.bind(&mut model, &b).expect("bind");
        assert_eq!(
            sole_binding(&model).cache_maintenance,
            Some(RegionCacheMaintenance::PerAccess)
        );
    }

    #[test]
    fn missing_section_on_region_is_an_error() {
        let mut model = model_with_region("tx_ring");
        let b = bindings("[mmio.regs]\nbase = 0\nsize = 16\n");
        DmaBinder.bind(&mut model, &b).expect("bind is a no-op join");
        let err = DmaBinder.validate(&model).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-001");
        assert!(err.to_string().contains("tx_ring"), "{err}");
    }

    #[test]
    fn invalid_cache_maintenance_is_rejected() {
        let mut model = model_with_region("tx_ring");
        let b = bindings("[dma.tx_ring]\nregion_cache_maintenance = \"flush\"\n");
        let err = DmaBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-002");
        assert!(err.to_string().contains("\"flush\""), "{err}");
    }

    #[test]
    fn orphan_section_is_rejected() {
        let mut model = RegseqModel::with_primary("plain", StableCrateId(1));
        let b = bindings("[dma.orphan]\ndevice_address_bits = 32\n");
        let err = DmaBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-003");
        assert!(err.to_string().contains("orphan"), "{err}");
    }

    #[test]
    fn registry_runs_dma_tenant() {
        // End-to-end through bind_all/validate_l2_surface: the tenant is
        // registered and its missing-section check fires through the crate
        // surface.
        let mut model = model_with_region("tx_ring");
        let b = bindings("version = \"1\"\n");
        crate::bind_all(&mut model, &b).expect("bind_all");
        let err = crate::validate_l2_surface(&model).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-001");
    }

    // =======================================================================
    // T21 — capability-gating contract (GRILL D07/C04). The cap OPS are
    // designed-not-built; the only buildable C04 oracle is the L2 rule that
    // `cap_* = "absent"` is legal ONLY in a concrete binding. Each negative test
    // is empirically witness-able: it FAILS if `validate_capability_scope` is
    // neutered. The positive `cap_absent_in_concrete_binding_is_accepted` is the
    // neuter baseline (a rule that always-rejects `absent` would fail it).
    // =======================================================================

    #[test]
    fn cap_absent_in_template_binding_is_rejected() {
        // D07: a `cap_pause = "absent"` in a `binding_scope = "template"` binding is
        // rejected — the resolved controller is unknown in a template, so the
        // capability cannot be statically denied (must use "unknown"). DMA-L2-006.
        let mut model = model_with_channel("rx");
        let b = bindings(
            "[dma.rx]\nbinding_scope = \"template\"\ncap_pause = \"absent\"\n",
        );
        let err = DmaBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-006");
        assert!(
            err.to_string().contains("absent") && err.to_string().contains("template"),
            "the reject must name the absent capability + the template scope; got: {err}"
        );
    }

    #[test]
    fn cap_absent_in_concrete_binding_is_accepted() {
        // The neuter baseline: a `cap_pause = "absent"` in a CONCRETE binding (the
        // omitted-default scope) is ACCEPTED by the rule — there is no cap-op to
        // reject this cycle (designed-not-built). If the rule rejected every
        // `absent`, this would fail.
        let mut model = model_with_channel("rx");
        let b = bindings("[dma.rx]\ncap_pause = \"absent\"\n");
        DmaBinder
            .bind(&mut model, &b)
            .expect("a concrete-binding `absent` capability must be accepted by the T21 rule");
        // An explicit `binding_scope = "concrete"` is equally accepted.
        let mut model2 = model_with_channel("rx");
        let b2 = bindings(
            "[dma.rx]\nbinding_scope = \"concrete\"\ncap_cyclic = \"absent\"\n",
        );
        DmaBinder
            .bind(&mut model2, &b2)
            .expect("an explicit-concrete `absent` capability must be accepted");
    }

    #[test]
    fn cap_present_and_unknown_in_template_are_accepted() {
        // Only `absent` is template-illegal: `present`/`unknown` are fine in a
        // non-concrete binding (witnesses that the rule gates on the VALUE, not the
        // mere presence of a cap flag in a template).
        let mut model = model_with_channel("rx");
        let b = bindings(
            "[dma.rx]\nbinding_scope = \"family\"\ncap_pause = \"present\"\ncap_cyclic = \"unknown\"\n",
        );
        DmaBinder
            .bind(&mut model, &b)
            .expect("present/unknown capabilities are legal in a non-concrete binding");
    }

    #[test]
    fn bad_binding_scope_is_rejected() {
        // D07: a `binding_scope` outside the closed vocabulary is rejected. DMA-L2-004.
        let mut model = model_with_channel("rx");
        let b = bindings("[dma.rx]\nbinding_scope = \"galaxy\"\n");
        let err = DmaBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-004");
        assert!(err.to_string().contains("galaxy"), "{err}");
    }

    #[test]
    fn bad_capability_value_is_rejected() {
        // C02/D07: a `cap_*` value outside `present|absent|unknown` is rejected.
        // DMA-L2-005.
        let mut model = model_with_channel("rx");
        let b = bindings("[dma.rx]\ncap_pause = \"maybe\"\n");
        let err = DmaBinder.bind(&mut model, &b).unwrap_err();
        assert_eq!(err.code(), "DMA-L2-005");
        assert!(err.to_string().contains("maybe"), "{err}");
    }
}
