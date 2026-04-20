//! Driver struct and driver-level metadata model types.
//!
//! Models the driver struct, the resource manifest for a regseq driver,
//! as defined in spec.md Section 4 (Driver Struct) and Section 4.1
//! (Driver Metadata). Resource, property, and user-data field types are
//! defined in [`crate::model::resources`]; this module composes them
//! into the top-level driver struct along with the three metadata
//! groups: identity, device class, and PM info.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::identity::{Symbol, TypeDefId};
use crate::model::resources::{
    ClockResource, InterruptResource, MmioResource, PinCtrlResource, PropertyField, ResetResource,
    SyncResource, UserDataField,
};
use crate::platform::PlatformValue;

/// The driver struct: the resource manifest with driver-level metadata.
///
/// Corresponds to the struct annotated with `#[rseq::driver(..)]` in a
/// driver module. Fields are grouped into resources (OS-managed objects
/// with lifecycle), properties (firmware inputs), and user data
/// (author-managed state). Each resource category is a separate `Vec`
/// for type-safe access.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct DriverStruct {
    /// The [`TypeDefId`] of the driver struct itself. The driver struct
    /// is also present as a `TypeDef` in its owning module's type
    /// definitions; this id is the shared handle.
    pub type_id: TypeDefId,
    /// The struct's identifier as an interned symbol.
    pub name: Symbol,
    /// Driver identity from `#[rseq::driver(..)]`.
    pub identity: DriverIdentity,
    /// Device class from `#[rseq::device_class(..)]`, if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_class: Option<DeviceClass>,
    /// Power management info from `#[rseq::pm(..)]`, if present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pm_info: Option<PmInfo>,
    /// Platform-file metadata (`[platform]` + `[bus]`). `None` until
    /// Phase 2 platform join runs. Populated by
    /// [`crate::join::populate_bindings`] (`join_platform_bus`).
    ///
    /// Kept alongside the other driver-metadata fields per
    /// spec.md Section 11: "Platform metadata is stored alongside
    /// driver metadata (Section 4.1), giving consumers a single
    /// place to find both spec-declared and platform-provided
    /// identity."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_metadata: Option<PlatformMetadata>,
    /// Clock resource fields.
    pub clocks: Vec<ClockResource>,
    /// Reset resource fields.
    pub resets: Vec<ResetResource>,
    /// PinCtrl resource fields.
    pub pinctrl: Vec<PinCtrlResource>,
    /// Interrupt resource fields.
    pub interrupts: Vec<InterruptResource>,
    /// Mmio resource fields.
    pub mmio: Vec<MmioResource>,
    /// Synchronization primitive resource fields.
    pub sync: Vec<SyncResource>,
    /// Firmware property fields.
    pub properties: Vec<PropertyField>,
    /// Author-defined user data fields.
    pub user_data: Vec<UserDataField>,
}

/// Driver identity metadata extracted from `#[rseq::driver(..)]`.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct DriverIdentity {
    /// Driver name string.
    pub name: Symbol,
    /// List of device compatibility strings.
    pub compatible: Vec<Symbol>,
    /// Optional bus/fabric identity (e.g., `integrated`, `amba`, `pci`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<Symbol>,
}

/// Device class metadata extracted from `#[rseq::device_class(..)]`.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct DeviceClass {
    /// The device class identifier (e.g., `serial_uart`).
    pub class: Symbol,
}

/// Power management metadata extracted from `#[rseq::pm(..)]`.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PmInfo {
    /// Whether the driver supports idle PM.
    pub idle: bool,
    /// Whether the device is wakeup-capable.
    pub wakeup_capable: bool,
}

/// Platform-file metadata recorded on the driver at Phase 2 join
/// time (spec.md Section 11 "Platform Metadata").
///
/// Populated from the `[platform]` header and optional `[bus]`
/// section by [`crate::join::populate_bindings`]. The
/// [`PlatformBusBinding::values`] map is a [`BTreeMap`] so
/// family-specific keys (e.g., `amba_id`, `revision`) iterate in
/// lexicographic order for Phase-2 determinism.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlatformMetadata {
    /// Target operating system (e.g., `"linux"`, `"zephyr"`).
    pub os: String,
    /// Board identifier (e.g., `"raspberry-pi-4"`).
    pub board: String,
    /// Target architecture (e.g., `"aarch64"`, `"x86_64"`).
    pub arch: String,
    /// Platform-side bus-family binding, if `[bus]` is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<PlatformBusBinding>,
}

/// Platform-side bus-family binding captured from the `[bus]`
/// section.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlatformBusBinding {
    /// Bus family identifier (e.g., `"amba"`, `"pci"`,
    /// `"integrated"`).
    pub bus_type: String,
    /// Family-specific key-value pairs (e.g., `amba_id`,
    /// `revision`). [`BTreeMap`] ordering gives byte-identical
    /// serialization across runs.
    pub values: BTreeMap<String, PlatformValue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{
        DeclId, PropertyId, RegisterBlockId, ResourceId, SymbolInterner, UserDataId,
    };
    use crate::ir::{RegseqType, ScalarType};
    use crate::model::resources::{ClockDecl, MmioDecl, PropertyField, Shape, UserDataField};

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    fn empty_driver(type_id: TypeDefId, name: Symbol, identity: DriverIdentity) -> DriverStruct {
        DriverStruct {
            type_id,
            name,
            identity,
            device_class: None,
            pm_info: None,
            platform_metadata: None,
            clocks: Vec::new(),
            resets: Vec::new(),
            pinctrl: Vec::new(),
            interrupts: Vec::new(),
            mmio: Vec::new(),
            sync: Vec::new(),
            properties: Vec::new(),
            user_data: Vec::new(),
        }
    }

    #[test]
    fn constructs_driver_struct_with_all_metadata_populated() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "UartDriver");
        let driver_name = sym(&mut interner, "pl011_uart");
        let compat_a = sym(&mut interner, "arm,pl011");
        let compat_b = sym(&mut interner, "arm,primecell");
        let bus = sym(&mut interner, "amba");
        let class = sym(&mut interner, "serial_uart");

        let d = DriverStruct {
            type_id: TypeDefId::from(DeclId::new(1)),
            name,
            identity: DriverIdentity {
                name: driver_name,
                compatible: vec![compat_a, compat_b],
                bus: Some(bus),
            },
            device_class: Some(DeviceClass { class }),
            pm_info: Some(PmInfo {
                idle: true,
                wakeup_capable: false,
            }),
            platform_metadata: None,
            clocks: Vec::new(),
            resets: Vec::new(),
            pinctrl: Vec::new(),
            interrupts: Vec::new(),
            mmio: Vec::new(),
            sync: Vec::new(),
            properties: Vec::new(),
            user_data: Vec::new(),
        };

        assert_eq!(d.identity.compatible.len(), 2);
        assert!(d.identity.bus.is_some());
        assert!(d.device_class.is_some());
        assert_eq!(d.device_class.as_ref().unwrap().class, class);
        assert!(d.pm_info.as_ref().unwrap().idle);
        assert!(!d.pm_info.as_ref().unwrap().wakeup_capable);
    }

    #[test]
    fn constructs_driver_struct_without_device_class_or_pm_info() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "MinimalDriver");
        let driver_name = sym(&mut interner, "minimal");

        let d = empty_driver(
            TypeDefId::from(DeclId::new(2)),
            name,
            DriverIdentity {
                name: driver_name,
                compatible: Vec::new(),
                bus: None,
            },
        );

        assert!(d.device_class.is_none());
        assert!(d.pm_info.is_none());
        assert!(d.identity.bus.is_none());
        assert!(d.clocks.is_empty());
    }

    #[test]
    fn constructs_driver_struct_with_two_clocks_one_mmio_one_property_one_user_data() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "MixedDriver");
        let driver_name = sym(&mut interner, "mixed");

        let clk_field_a = sym(&mut interner, "apb_clk");
        let clk_field_b = sym(&mut interner, "ref_clk");
        let clk_id_a = sym(&mut interner, "apb");
        let clk_id_b = sym(&mut interner, "ref");

        let mmio_field = sym(&mut interner, "regs");
        let mmio_id = sym(&mut interner, "uart_regs");

        let prop_field = sym(&mut interner, "clock_frequency");
        let prop_id = sym(&mut interner, "clock-frequency");

        let ud_field = sym(&mut interner, "frame_count");

        let mut d = empty_driver(
            TypeDefId::from(DeclId::new(3)),
            name,
            DriverIdentity {
                name: driver_name,
                compatible: Vec::new(),
                bus: None,
            },
        );

        d.clocks.push(ClockResource {
            id: ResourceId::from(DeclId::new(10)),
            field_name: clk_field_a,
            shape: Shape::Single,
            decl: ClockDecl { clock_id: clk_id_a },
            binding: None,
        });
        d.clocks.push(ClockResource {
            id: ResourceId::from(DeclId::new(11)),
            field_name: clk_field_b,
            shape: Shape::Optional,
            decl: ClockDecl { clock_id: clk_id_b },
            binding: None,
        });
        d.mmio.push(MmioResource {
            id: ResourceId::from(DeclId::new(12)),
            field_name: mmio_field,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id,
                register_block: RegisterBlockId::from(DeclId::new(100)),
            },
            binding: None,
        });
        d.properties.push(PropertyField {
            id: PropertyId::from(DeclId::new(20)),
            field_name: prop_field,
            field_type: ScalarType::U32,
            property_id: prop_id,
            default_value: None,
            vendor_prefix: None,
            required: true,
            bound_value: None,
        });
        d.user_data.push(UserDataField {
            id: UserDataId::from(DeclId::new(30)),
            field_name: ud_field,
            inner_type: RegseqType::Scalar(ScalarType::U32),
        });

        assert_eq!(d.clocks.len(), 2);
        assert_eq!(d.mmio.len(), 1);
        assert_eq!(d.properties.len(), 1);
        assert_eq!(d.user_data.len(), 1);
        // Other categories remain empty.
        assert!(d.resets.is_empty());
        assert!(d.pinctrl.is_empty());
        assert!(d.interrupts.is_empty());
        assert!(d.sync.is_empty());
    }

    #[test]
    fn pm_info_round_trips_through_json() {
        for (idle, wakeup_capable) in [(false, false), (true, false), (false, true), (true, true)] {
            let pm = PmInfo {
                idle,
                wakeup_capable,
            };
            let json = serde_json::to_string(&pm).expect("serialize");
            let back: PmInfo = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(pm, back);
        }
    }
}
