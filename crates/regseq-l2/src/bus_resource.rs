//! Bus-resource-management construct (#3): the platform-file join.
//!
//! Recognition of the L1 surface (`#[rseq::driver(bus = …)]`,
//! `#[rseq::mmio(id/bar/type)]`) lives in the rustc-driven bridge
//! (`regseq-driver`'s `bridge::device_lifecycle`). This module is the
//! **post-bridge** step that is NOT rustc-driven: it joins the parsed platform
//! file (L2) to the model's resource fields, resolving the effective BAR
//! (`L2.bar ?? L1.bar`, GRILL I01) and the `(base, size)` literals (GRILL D06)
//! into each [`regseq_model::MmioBinding`]. The generic-c backend then hands a
//! resolved `(base, size)` to register-access's `map_region` — the keystone.
//!
//! This is the first [`crate::L2Binder`] tenant ([`BusResourceBinder`]); the
//! crate-level [`bind_all`](crate::bind_all)/[`validate_l2_surface`](crate::validate_l2_surface)
//! seam routes through it.

use regseq_config::PlatformBindings;
use regseq_model::{BarType, BusType, RegseqModel, ResourceKind};

/// The resolved binding for one resource field, computed in the immutable pass.
struct Resolved {
    item: regseq_model::ItemId,
    field_idx: usize,
    base: Option<u64>,
    size: Option<u64>,
    effective_bar: Option<u8>,
    bar_type_io: Option<bool>,
}

/// Join the parsed platform-file [`PlatformBindings`] (L2) to every recognized
/// driver's MMIO resource fields, resolving the effective BAR + `(base, size)`
/// into each field's [`regseq_model::MmioBinding`].
///
/// The join key is the L1 resource `id` (`#[rseq::mmio(id = "<id>")]`), falling
/// back to the struct field name when no `id` was authored. A resource field
/// with no matching `[mmio.<id>]` section is left unresolved (its binding keeps
/// the L1 facts only) — the faked-base accessor path, unchanged.
pub fn join_platform_bindings(model: &mut RegseqModel, bindings: &PlatformBindings) {
    // Pass 1 (immutable borrow of `model`): resolve each resource field's join
    // key, look up its `[mmio.<id>]` spec, and compute the effective BAR +
    // `(base, size)`. `effective_bar = L2.bar ?? L1.bar` (GRILL I01).
    let mut updates: Vec<Resolved> = Vec::new();
    for (item, driver) in model.device_lifecycle.drivers.iter() {
        for (field_idx, rf) in driver.handle.resource_fields.iter().enumerate() {
            if rf.kind != ResourceKind::Mmio {
                continue;
            }
            let key = match rf.binding.id {
                Some(id) => model.strings.resolve(id).to_string(),
                None => model.strings.resolve(rf.name).to_string(),
            };
            let Some(spec) = bindings.mmio.get(&key) else {
                continue;
            };
            updates.push(Resolved {
                item: *item,
                field_idx,
                base: spec.base,
                size: spec.size,
                effective_bar: spec.bar.or(rf.binding.bar_index),
                bar_type_io: spec.bar_type_io,
            });
        }
    }

    // Pass 2 (mutable borrow of `model`): apply the resolved bindings.
    for r in updates {
        let Some(driver) = model.device_lifecycle.drivers.get_mut(&r.item) else {
            continue;
        };
        let Some(rf) = driver.handle.resource_fields.get_mut(r.field_idx) else {
            continue;
        };
        rf.binding.base = r.base;
        rf.binding.size = r.size;
        rf.binding.effective_bar = r.effective_bar;
        // An explicit L2 `type` overrides the L1 carrier (either direction);
        // an absent L2 `type` leaves the L1 `bar_type` in place.
        if let Some(io) = r.bar_type_io {
            rf.binding.bar_type = if io { BarType::Io } else { BarType::Mem };
        }
    }
}

/// A bus-resource-management surface diagnostic (construct #3 M6, GRILL C01).
/// Each maps to a `BRM-0NN` code; an unknown `bus` token is the non-enum `bus`
/// leg of R01 (no dedicated C01 code — a generic recognition error; a `BRM-007`
/// is a taxonomy-amendment candidate, not invented here). Every check is gated
/// on a driver that DECLARES a `bus` (`bus_type.is_some()`), so a plain
/// register-access / device-lifecycle driver (no `bus`) is never affected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrmError {
    /// BRM-002: `#[rseq::mmio(bar = …)]` on a non-PCI bus (a BAR is PCI-only).
    BarOnNonPci { id: String, bus: String },
    /// BRM-003: `#[rseq::mmio(type = "io")]` on a non-PCI bus (port-I/O is PCI-only).
    IoTypeOnNonPci { id: String, bus: String },
    /// BRM-005: a region with exactly one of `base`/`size` (they bind together, I02).
    HalfSpecifiedRegion { id: String },
    /// BRM-006: a bus-managed region with no resolvable `(base, size)` on the host
    /// (generic-c requires the `[mmio.<id>]` literal, GRILL D06 / I01 rejection leg).
    UnresolvableRegion { id: String },
    /// An unknown `bus = "…"` token (the non-enum `bus` value of R01/T02).
    UnknownBus { bus: String },
}

impl BrmError {
    /// The diagnostic code (`BRM-0NN`, or `bus` for the unknown-bus leg).
    pub fn code(&self) -> &'static str {
        match self {
            BrmError::BarOnNonPci { .. } => "BRM-002",
            BrmError::IoTypeOnNonPci { .. } => "BRM-003",
            BrmError::HalfSpecifiedRegion { .. } => "BRM-005",
            BrmError::UnresolvableRegion { .. } => "BRM-006",
            BrmError::UnknownBus { .. } => "bus",
        }
    }
}

impl std::fmt::Display for BrmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrmError::BarOnNonPci { id, bus } => write!(
                f,
                "BRM-002: region `{id}` declares a `bar`, but the bus `{bus}` is not PCI-like (a BAR is PCI-only)"
            ),
            BrmError::IoTypeOnNonPci { id, bus } => write!(
                f,
                "BRM-003: region `{id}` declares `type = \"io\"`, but the bus `{bus}` is not PCI-like (port-I/O is PCI-only)"
            ),
            BrmError::HalfSpecifiedRegion { id } => write!(
                f,
                "BRM-005: region `{id}` specifies only one of `base`/`size` (they must be specified together)"
            ),
            BrmError::UnresolvableRegion { id } => write!(
                f,
                "BRM-006: region `{id}` has no resolvable `(base, size)` (a bus-managed region needs an `[mmio.<id>]` base/size on this backend)"
            ),
            BrmError::UnknownBus { bus } => write!(
                f,
                "unknown `bus = \"{bus}\"`: not a recognized bus (pci/platform/amba/i2c/spi/usb)"
            ),
        }
    }
}

/// Validate the bus-resource surface of every **bus-declared** driver against
/// the BRM-001..006 catalog reachable from the model (construct #3 M6, T19/T20).
///
/// Runs after [`join_platform_bindings`] (so the resolved `(base, size)` /
/// `effective_bar` are visible). Returns the FIRST diagnostic found (the bridge
/// surfaces it as a fatal). Checks, per bus-declared driver:
/// - **unknown bus** — a `BusType::Other` token (R01 non-enum leg);
/// - per resource region (most-specific first): **BRM-002** (`bar` on non-PCI),
///   **BRM-003** (`type="io"` on non-PCI), **BRM-005** (`base` XOR `size`),
///   **BRM-006** (unresolvable on the host).
///
/// `BRM-001` (config intrinsic on non-PCI) and `BRM-004` (the frozen
/// surface-boundary) are NOT enforced here: BRM-001 needs call-site→driver
/// linkage (the config intrinsic is recognized on the facade fn, not the call
/// site), and BRM-004 has no trigger surface in the build-now DSL (recorded as
/// frozen-but-untestable, GRILL — not fabricated).
pub fn validate_bus_surface(model: &RegseqModel) -> Result<(), BrmError> {
    for driver in model.device_lifecycle.drivers.values() {
        let Some(bus_type) = driver.descriptor.bus_type.as_ref() else {
            continue; // a driver with no `bus` is not bus-managed — skip.
        };
        // Unknown `bus` token (the non-enum `bus` value of R01/T02).
        if let BusType::Other(sid) = bus_type {
            return Err(BrmError::UnknownBus {
                bus: model.strings.resolve(*sid).to_string(),
            });
        }
        let is_pci = bus_type.is_pci_like();
        let bus_name = bus_token(bus_type);
        for rf in &driver.handle.resource_fields {
            if rf.kind != ResourceKind::Mmio {
                continue;
            }
            let b = &rf.binding;
            let id = match b.id {
                Some(id) => model.strings.resolve(id).to_string(),
                None => model.strings.resolve(rf.name).to_string(),
            };
            // BRM-002: a BAR (L1 `bar_index` or the L2-resolved `effective_bar`)
            // on a non-PCI bus.
            if !is_pci && (b.bar_index.is_some() || b.effective_bar.is_some()) {
                return Err(BrmError::BarOnNonPci { id, bus: bus_name.to_string() });
            }
            // BRM-003: `type = "io"` on a non-PCI bus.
            if !is_pci && b.bar_type == BarType::Io {
                return Err(BrmError::IoTypeOnNonPci { id, bus: bus_name.to_string() });
            }
            // BRM-005: exactly one of `base`/`size` (most specific — checked
            // before BRM-006, which a half-pair would also trip).
            if b.base.is_some() != b.size.is_some() {
                return Err(BrmError::HalfSpecifiedRegion { id });
            }
            // BRM-006: a bus-managed region with no resolvable `(base, size)`.
            if !b.is_resolved() {
                return Err(BrmError::UnresolvableRegion { id });
            }
        }
    }
    Ok(())
}

/// The static bus token for a non-`Other` [`BusType`] (the `Other` case is
/// handled before this is reached).
fn bus_token(bus_type: &BusType) -> &'static str {
    match bus_type {
        BusType::Pci => "pci",
        BusType::Platform => "platform",
        BusType::Amba => "amba",
        BusType::I2c => "i2c",
        BusType::Spi => "spi",
        BusType::Usb => "usb",
        BusType::Other(_) => "other",
    }
}

/// The bus-resource-management L2 tenant (construct #3) — the first
/// [`crate::L2Binder`]. Its `bind` is the infallible MMIO
/// [`join_platform_bindings`]; its `validate` is [`validate_bus_surface`],
/// mapping [`BrmError`] into [`crate::L2Error`] at the crate boundary.
pub struct BusResourceBinder;

impl crate::L2Binder for BusResourceBinder {
    fn name(&self) -> &'static str {
        "bus-resource"
    }

    fn bind(
        &self,
        model: &mut RegseqModel,
        bindings: &PlatformBindings,
    ) -> Result<(), crate::L2Error> {
        join_platform_bindings(model, bindings);
        Ok(())
    }

    fn validate(&self, model: &RegseqModel) -> Result<(), crate::L2Error> {
        validate_bus_surface(model).map_err(crate::L2Error::BusResource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_config::MmioBindingSpec;
    use regseq_model::{
        ClassTag, DriverDescriptor, DriverHandle, DriverLifecycle, ItemId, LifecycleOps,
        MmioBinding, ResourceField, StableCrateId,
    };

    fn model_with_one_mmio_field(id: Option<&str>, l1_bar: Option<u8>) -> (RegseqModel, ItemId) {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let name = model.strings.intern("regs");
        let id_sid = id.map(|s| model.strings.intern(s));
        let dname = model.strings.intern("dev");
        let item = ItemId::from_u32(0);
        model.device_lifecycle.drivers.insert(
            item,
            DriverLifecycle {
                descriptor: DriverDescriptor {
                    name: dname,
                    match_set: Vec::new(),
                    device_class: ClassTag::Generic,
                    bus_type: Some(regseq_model::BusType::Pci),
                    dma_present: false,
                },
                handle: DriverHandle {
                    resource_fields: vec![ResourceField {
                        name,
                        kind: ResourceKind::Mmio,
                        binding: MmioBinding {
                            id: id_sid,
                            bar_index: l1_bar,
                            bar_type: BarType::Mem,
                            effective_bar: None,
                            base: None,
                            size: None,
                        },
                    }],
                    user_data_fields: Vec::new(),
                },
                ops: LifecycleOps::default(),
            },
        );
        (model, item)
    }

    fn binding(base: u64, size: u64, bar: Option<u8>) -> PlatformBindings {
        let mut b = PlatformBindings::default();
        b.mmio.insert(
            "regs".to_string(),
            MmioBindingSpec {
                bar,
                base: Some(base),
                size: Some(size),
                bar_type_io: None,
            },
        );
        b
    }

    fn field_binding(model: &RegseqModel, item: ItemId) -> MmioBinding {
        model.device_lifecycle.drivers[&item].handle.resource_fields[0]
            .binding
            .clone()
    }

    #[test]
    fn join_by_id_resolves_base_size_and_effective_bar() {
        let (mut model, item) = model_with_one_mmio_field(Some("regs"), Some(0));
        join_platform_bindings(&mut model, &binding(0xf000_0000, 0x2_0000, None));
        let b = field_binding(&model, item);
        assert_eq!(b.base, Some(0xf000_0000));
        assert_eq!(b.size, Some(0x2_0000));
        // L2 has no bar -> effective = L1 bar (0).
        assert_eq!(b.effective_bar, Some(0));
        assert!(b.is_resolved());
    }

    #[test]
    fn l2_bar_overrides_l1() {
        let (mut model, item) = model_with_one_mmio_field(Some("regs"), Some(0));
        join_platform_bindings(&mut model, &binding(0xc000_0000, 0x1000, Some(2)));
        // effective_bar = L2.bar (2) ?? L1.bar (0) = 2.
        assert_eq!(field_binding(&model, item).effective_bar, Some(2));
    }

    #[test]
    fn join_falls_back_to_field_name_when_no_id() {
        let (mut model, item) = model_with_one_mmio_field(None, None);
        join_platform_bindings(&mut model, &binding(0x1000, 0x100, None));
        assert_eq!(field_binding(&model, item).base, Some(0x1000));
    }

    #[test]
    fn explicit_l2_mem_type_overrides_l1_io() {
        // An L1 `type = "io"` carrier with an explicit L2 `type = "mem"` must end
        // as Mem (the L2 binding overrides the L1 carrier in either direction).
        let (mut model, item) = model_with_one_mmio_field(Some("regs"), Some(0));
        // Seed the L1 carrier as Io.
        model.device_lifecycle.drivers.get_mut(&item).unwrap().handle.resource_fields[0]
            .binding
            .bar_type = BarType::Io;
        let mut bindings = PlatformBindings::default();
        bindings.mmio.insert(
            "regs".to_string(),
            MmioBindingSpec {
                bar: None,
                base: Some(0x1000),
                size: Some(0x100),
                bar_type_io: Some(false),
            },
        );
        join_platform_bindings(&mut model, &bindings);
        assert_eq!(field_binding(&model, item).bar_type, BarType::Mem);
    }

    #[test]
    fn no_matching_section_leaves_field_unresolved() {
        let (mut model, item) = model_with_one_mmio_field(Some("other"), Some(0));
        join_platform_bindings(&mut model, &binding(0xf000_0000, 0x2_0000, None));
        assert!(!field_binding(&model, item).is_resolved());
    }

    #[test]
    fn non_pci_no_bar_region_resolves_with_effective_bar_none() {
        // Construct #3 M4 (non-PCI degradation, R07): a platform/DT region — no
        // L1 `bar` and a `[mmio.<id>]` carrying only `base`/`size` (the DT-reg
        // stand-in, no `bar`) — still RESOLVES (`is_resolved()`), with
        // `effective_bar = None` (-> the backend maps it with `bar=-1`). This is
        // the join half of the degradation: the acquire path resolves `(base,
        // size)` exactly as on PCI, only without a BAR index.
        let (mut model, item) = model_with_one_mmio_field(Some("regs"), None);
        join_platform_bindings(&mut model, &binding(0x4000_0000, 0x1000, None));
        let b = field_binding(&model, item);
        assert_eq!(b.base, Some(0x4000_0000));
        assert_eq!(b.size, Some(0x1000));
        assert_eq!(b.effective_bar, None, "a no-BAR DT region has no effective BAR");
        assert!(b.is_resolved(), "base+size present => resolved even without a BAR");
    }

    // ----- BRM surface validation (construct #3 M6, T19/T20) -----

    /// A driver on `bus` with a single MMIO field carrying `binding`.
    fn validation_model(bus: Option<BusType>, binding: MmioBinding) -> RegseqModel {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let name = model.strings.intern("regs");
        let dname = model.strings.intern("dev");
        let item = ItemId::from_u32(0);
        model.device_lifecycle.drivers.insert(
            item,
            DriverLifecycle {
                descriptor: DriverDescriptor {
                    name: dname,
                    match_set: Vec::new(),
                    device_class: ClassTag::Generic,
                    bus_type: bus,
                    dma_present: false,
                },
                handle: DriverHandle {
                    resource_fields: vec![ResourceField {
                        name,
                        kind: ResourceKind::Mmio,
                        binding,
                    }],
                    user_data_fields: Vec::new(),
                },
                ops: LifecycleOps::default(),
            },
        );
        model
    }

    fn resolved(bar: Option<u8>) -> MmioBinding {
        MmioBinding {
            id: None,
            bar_index: bar,
            bar_type: BarType::Mem,
            effective_bar: bar,
            base: Some(0xf000_0000),
            size: Some(0x1000),
        }
    }

    #[test]
    fn validate_accepts_a_resolved_pci_region_and_skips_non_bus_drivers() {
        // A resolved PCI region is well-formed.
        assert!(validate_bus_surface(&validation_model(Some(BusType::Pci), resolved(Some(0)))).is_ok());
        // A non-bus (register-access) driver is never validated, even with an
        // otherwise-rejectable binding (a bar + half-pair) — zero impact.
        let mut half = resolved(Some(0));
        half.size = None; // base present, size absent
        assert!(validate_bus_surface(&validation_model(None, half)).is_ok());
    }

    #[test]
    fn brm_002_bar_on_non_pci_is_rejected() {
        let mut b = resolved(Some(2)); // a BAR on an amba (non-PCI) bus
        b.base = None;
        b.size = None;
        let err = validate_bus_surface(&validation_model(Some(BusType::Amba), b)).unwrap_err();
        assert_eq!(err.code(), "BRM-002", "got {err}");
    }

    #[test]
    fn brm_003_io_type_on_non_pci_is_rejected() {
        let b = MmioBinding {
            id: None,
            bar_index: None,
            bar_type: BarType::Io, // port-I/O on amba (non-PCI)
            effective_bar: None,
            base: Some(0x4000_0000),
            size: Some(0x1000),
        };
        let err = validate_bus_surface(&validation_model(Some(BusType::Amba), b)).unwrap_err();
        assert_eq!(err.code(), "BRM-003", "got {err}");
    }

    #[test]
    fn brm_005_half_specified_region_is_rejected() {
        let mut b = resolved(Some(0));
        b.size = None; // base present, size absent
        let err = validate_bus_surface(&validation_model(Some(BusType::Pci), b)).unwrap_err();
        assert_eq!(err.code(), "BRM-005", "got {err}");
    }

    #[test]
    fn brm_006_unresolvable_region_is_rejected() {
        // A PCI driver with a BAR but no resolvable (base,size) on the host.
        let b = MmioBinding {
            id: None,
            bar_index: Some(0),
            bar_type: BarType::Mem,
            effective_bar: Some(0),
            base: None,
            size: None,
        };
        let err = validate_bus_surface(&validation_model(Some(BusType::Pci), b)).unwrap_err();
        assert_eq!(err.code(), "BRM-006", "got {err}");
    }

    #[test]
    fn unknown_bus_token_is_rejected() {
        // An unrecognized `bus = "frobnicate"` (parsed to `BusType::Other`) is
        // the non-enum `bus` leg of R01. Build the model so the token is interned
        // in its OWN string table (a `StringId` is table-relative).
        let mut model = validation_model(Some(BusType::Pci), resolved(Some(0)));
        let sid = model.strings.intern("frobnicate");
        model
            .device_lifecycle
            .drivers
            .values_mut()
            .next()
            .unwrap()
            .descriptor
            .bus_type = Some(BusType::Other(sid));
        let err = validate_bus_surface(&model).unwrap_err();
        assert_eq!(err.code(), "bus", "got {err}");
        assert!(format!("{err}").contains("frobnicate"));
    }

    // ----- crate-level registry seam (T0) -----

    #[test]
    fn bind_all_and_validate_l2_surface_route_through_the_bus_resource_binder() {
        // bind_all == join_platform_bindings for the single BRM tenant: the real
        // base flows through the registry into the model.
        let (mut model, item) = model_with_one_mmio_field(Some("regs"), Some(0));
        crate::bind_all(&mut model, &binding(0xf000_0000, 0x2_0000, None)).unwrap();
        assert_eq!(field_binding(&model, item).base, Some(0xf000_0000));

        // validate_l2_surface surfaces the BRM-002 code via L2Error (delegated).
        let mut b = resolved(Some(2)); // a BAR on amba (non-PCI)
        b.base = None;
        b.size = None;
        let err = crate::validate_l2_surface(&validation_model(Some(BusType::Amba), b)).unwrap_err();
        assert_eq!(err.code(), "BRM-002", "got {err}");
    }
}
