//! Driver identity declaration types.
//!
//! Captures the driver name, compatible strings, and device class as
//! declared by the `#[rseq::driver(...)]` attribute. These are normalized
//! from the raw artifact's `identity` family.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Normalized driver identity declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverDecl {
    /// Driver name from `#[rseq::driver(name = "...")]`.
    pub driver_name: String,
    /// Compatible strings for device matching.
    pub compatible: Vec<String>,
    /// Device class identity.
    pub device_class: DeviceClass,
    /// Portable bus/fabric identity from `#[rseq::driver(bus = "...")]`.
    pub bus: Option<BusType>,
    /// Provenance back to the raw artifact.
    pub provenance: Provenance,
}

/// Typed device class rather than a raw string tag.
///
/// Represents the accepted device class families. Unknown classes are
/// preserved as `Other` to avoid data loss during normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceClass {
    /// Serial UART device class.
    SerialUart,
    /// Network device class.
    Network,
    /// An unrecognized device class, preserved as-is.
    Other(String),
}

/// Typed portable bus/fabric identity.
///
/// Represents the accepted portable bus taxonomy from Layer 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BusType {
    /// Firmware-described integrated/on-chip device with no stronger bus identity.
    Integrated,
    /// ARM AMBA / PrimeCell-family device.
    Amba,
    /// PCI / PCIe enumerated device.
    Pci,
}

impl BusType {
    /// Parse a bus type from the raw string representation.
    ///
    /// Returns `None` for unrecognized values.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "integrated" => Some(Self::Integrated),
            "amba" => Some(Self::Amba),
            "pci" => Some(Self::Pci),
            _ => None,
        }
    }

    /// Return the canonical string label for this bus type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Integrated => "integrated",
            Self::Amba => "amba",
            Self::Pci => "pci",
        }
    }
}

impl DeviceClass {
    /// Parse a device class from the raw string representation.
    pub fn from_raw(s: &str) -> Self {
        match s {
            "serial_uart" | "serial" => Self::SerialUart,
            "network" => Self::Network,
            other => Self::Other(other.to_string()),
        }
    }

    /// Return the canonical string label for this device class.
    pub fn as_str(&self) -> &str {
        match self {
            Self::SerialUart => "serial_uart",
            Self::Network => "network",
            Self::Other(s) => s.as_str(),
        }
    }
}
