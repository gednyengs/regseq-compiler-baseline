//! Layer 2 platform-file schema and parsed artifact.
//!
//! Defines the rustc-independent parsed representation of a Layer 2 platform
//! binding file. This is a Phase-B parsed artifact model that supports parser
//! behavior, validation, and boundary checks. It is not the final normalized
//! binding model from Phase C.
//!
//! The platform file is one file per concrete device instance / deployment unit.
//! It is sectioned by construct or resource family, with a required header and
//! properties section.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level parsed representation of a Layer 2 platform binding file.
///
/// Contains the required version marker, platform header, property entries,
/// construct-owned sections, and any unknown but syntactically valid sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformFile {
    /// Schema version marker (e.g. `"1"`).
    pub version: String,
    /// Required platform deployment metadata.
    pub platform: PlatformHeader,
    /// Firmware property entries keyed by property id.
    pub properties: Vec<PropertyEntry>,
    /// Parsed `[bus]` section, if present.
    pub bus: Option<BusSection>,
    /// Construct-owned sections that are preserved but may not yet be
    /// semantically validated.
    pub construct_sections: Vec<ConstructSection>,
    /// Unknown but syntactically valid sections preserved for forward
    /// compatibility.
    pub unknown_sections: Vec<UnknownSection>,
}

/// Required platform deployment metadata header.
///
/// Carries descriptive deployment facts such as target OS, board identity,
/// and architecture. This section is required in every platform file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformHeader {
    /// Target operating system (e.g. `"linux"`, `"zephyr"`).
    pub os: String,
    /// Board or deployment label (e.g. `"raspberry-pi-4"`).
    pub board: String,
    /// Target architecture (e.g. `"aarch64"`, `"x86_64"`).
    pub arch: String,
}

/// A single firmware property entry from the `[properties]` section.
///
/// Properties are keyed by their property id (matching Layer 1
/// `#[rseq::property(id = "...")]` declarations) and carry a parsed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyEntry {
    /// Property identifier matching the Layer 1 declaration id.
    pub key: String,
    /// Parsed property value.
    pub value: PlatformValue,
}

/// A construct-owned section from the platform file.
///
/// Represents sections like `[mmio.uart_regs]` or `[clock.uart_clk]` that
/// are owned by specific construct families. These sections are preserved
/// in the parsed artifact but may not yet be semantically validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructSection {
    /// The construct family that owns this section (e.g. `"mmio"`, `"clock"`).
    pub section_kind: String,
    /// Optional binding key within the construct family (e.g. `"uart_regs"`).
    pub section_key: Option<String>,
    /// Raw parsed key-value pairs from this section.
    pub values: BTreeMap<String, PlatformValue>,
}

/// Parsed `[bus]` section from the platform file.
///
/// Contains the deployment-side bus family confirmation and any
/// family-specific metadata keys. The `bus_type` string is the raw
/// value from the TOML `type` key; typed validation happens during
/// binding normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusSection {
    /// Bus family type string (e.g. `"amba"`, `"pci"`, `"integrated"`).
    pub bus_type: String,
    /// Family-specific key-value pairs (excluding `type`).
    pub values: BTreeMap<String, PlatformValue>,
}

/// An unknown but syntactically valid section from the platform file.
///
/// Preserved for forward compatibility. The parser accepts these sections
/// without semantic validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownSection {
    /// Path segments identifying this section (e.g. `["custom", "subsection"]`).
    pub section_path: Vec<String>,
    /// Raw parsed key-value pairs from this section.
    pub values: BTreeMap<String, PlatformValue>,
}

/// A parsed value from a platform file.
///
/// Supports the accepted scalar and aggregate kinds needed for property
/// entries, platform headers, and construct-owned sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformValue {
    /// Signed 64-bit integer value.
    Integer(i64),
    /// Boolean value.
    Boolean(bool),
    /// String value.
    String(String),
    /// Ordered list of values.
    List(Vec<PlatformValue>),
    /// Ordered map of string keys to values.
    Table(BTreeMap<String, PlatformValue>),
}
