//! TOML parser for Layer 2 platform binding files.
//!
//! Parses the accepted TOML platform-file format into the Phase-B parsed
//! artifact defined in [`crate::platform`]. Enforces the required initial
//! contract (version, platform header) and preserves future sections without
//! premature semantic validation.

use crate::platform::{
    BusSection, ConstructSection, PlatformFile, PlatformHeader, PlatformValue, PropertyEntry,
    UnknownSection,
};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Errors produced by the platform-file parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformParseError {
    /// The input is not valid TOML.
    InvalidToml(String),
    /// A required top-level key is missing.
    MissingKey(&'static str),
    /// A required key has the wrong TOML type.
    WrongType {
        /// The key path that has the wrong type.
        key: &'static str,
        /// Human-readable description of the expected type.
        expected: &'static str,
    },
    /// A property value uses an unsupported TOML value kind.
    UnsupportedPropertyValue {
        /// The property key containing the unsupported value.
        key: String,
        /// The TOML type name that is not supported.
        toml_type: &'static str,
    },
    /// Failed to read the platform file from disk.
    IoError(String),
}

impl fmt::Display for PlatformParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToml(msg) => write!(f, "invalid TOML: {msg}"),
            Self::MissingKey(key) => write!(f, "missing required key: {key}"),
            Self::WrongType { key, expected } => {
                write!(f, "key `{key}` has wrong type, expected {expected}")
            }
            Self::UnsupportedPropertyValue { key, toml_type } => {
                write!(
                    f,
                    "property `{key}` has unsupported value type `{toml_type}`"
                )
            }
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for PlatformParseError {}

/// Parse a TOML platform-file string into the Phase-B parsed artifact.
pub fn parse_platform_file_str(input: &str) -> Result<PlatformFile, PlatformParseError> {
    let table: toml::Table = input
        .parse()
        .map_err(|e: toml::de::Error| PlatformParseError::InvalidToml(e.to_string()))?;

    // Extract required top-level `version`.
    let version = match table.get("version") {
        Some(toml::Value::String(s)) => s.clone(),
        Some(_) => {
            return Err(PlatformParseError::WrongType {
                key: "version",
                expected: "string",
            })
        }
        None => return Err(PlatformParseError::MissingKey("version")),
    };

    // Extract required `[platform]` section.
    let platform_table = match table.get("platform") {
        Some(toml::Value::Table(t)) => t,
        Some(_) => {
            return Err(PlatformParseError::WrongType {
                key: "platform",
                expected: "table",
            })
        }
        None => return Err(PlatformParseError::MissingKey("platform")),
    };

    let platform = parse_platform_header(platform_table)?;

    // Extract `[properties]` section if present.
    let properties = match table.get("properties") {
        Some(toml::Value::Table(t)) => parse_properties(t)?,
        Some(_) => {
            return Err(PlatformParseError::WrongType {
                key: "properties",
                expected: "table",
            })
        }
        None => Vec::new(),
    };

    // Extract `[bus]` section if present.
    let bus = match table.get("bus") {
        Some(toml::Value::Table(t)) => Some(parse_bus_section(t)?),
        Some(_) => {
            return Err(PlatformParseError::WrongType {
                key: "bus",
                expected: "table",
            })
        }
        None => None,
    };

    // Classify remaining sections.
    let mut construct_sections = Vec::new();
    let mut unknown_sections = Vec::new();

    for (key, value) in &table {
        match key.as_str() {
            "version" | "platform" | "properties" | "bus" => continue,
            _ => {}
        }

        if let toml::Value::Table(section_table) = value {
            classify_section(
                key,
                section_table,
                &mut construct_sections,
                &mut unknown_sections,
            );
        }
    }

    Ok(PlatformFile {
        version,
        platform,
        properties,
        bus,
        construct_sections,
        unknown_sections,
    })
}

/// Parse a TOML platform file from a filesystem path.
pub fn parse_platform_file_path(path: &Path) -> Result<PlatformFile, PlatformParseError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| PlatformParseError::IoError(e.to_string()))?;
    parse_platform_file_str(&content)
}

/// Parse the required `[platform]` header fields.
fn parse_platform_header(table: &toml::Table) -> Result<PlatformHeader, PlatformParseError> {
    let os = extract_required_string(table, "platform.os", "os")?;
    let board = extract_required_string(table, "platform.board", "board")?;
    let arch = extract_required_string(table, "platform.arch", "arch")?;

    Ok(PlatformHeader { os, board, arch })
}

/// Extract a required string field from a table.
fn extract_required_string(
    table: &toml::Table,
    full_key: &'static str,
    local_key: &str,
) -> Result<String, PlatformParseError> {
    match table.get(local_key) {
        Some(toml::Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(PlatformParseError::WrongType {
            key: full_key,
            expected: "string",
        }),
        None => Err(PlatformParseError::MissingKey(full_key)),
    }
}

/// Parse `[properties]` entries into a flat list of `PropertyEntry`.
///
/// Applies strict value-kind validation: only integer, boolean, string,
/// list, and table values are accepted. Float and datetime TOML values
/// are rejected.
fn parse_properties(table: &toml::Table) -> Result<Vec<PropertyEntry>, PlatformParseError> {
    table
        .iter()
        .map(|(key, value)| {
            let converted = convert_property_value(key, value)?;
            Ok(PropertyEntry {
                key: key.clone(),
                value: converted,
            })
        })
        .collect()
}

/// Convert a TOML value into a `PlatformValue` with strict property validation.
///
/// Rejects unsupported TOML value kinds (float, datetime) for property entries.
fn convert_property_value(
    key: &str,
    value: &toml::Value,
) -> Result<PlatformValue, PlatformParseError> {
    match value {
        toml::Value::Integer(i) => Ok(PlatformValue::Integer(*i)),
        toml::Value::Boolean(b) => Ok(PlatformValue::Boolean(*b)),
        toml::Value::String(s) => Ok(PlatformValue::String(s.clone())),
        toml::Value::Array(arr) => {
            let items: Result<Vec<_>, _> =
                arr.iter().map(|v| convert_property_value(key, v)).collect();
            Ok(PlatformValue::List(items?))
        }
        toml::Value::Table(t) => {
            let map: Result<BTreeMap<_, _>, _> = t
                .iter()
                .map(|(k, v)| {
                    let converted = convert_property_value(key, v)?;
                    Ok((k.clone(), converted))
                })
                .collect();
            Ok(PlatformValue::Table(map?))
        }
        toml::Value::Float(_) => Err(PlatformParseError::UnsupportedPropertyValue {
            key: key.to_string(),
            toml_type: "float",
        }),
        toml::Value::Datetime(_) => Err(PlatformParseError::UnsupportedPropertyValue {
            key: key.to_string(),
            toml_type: "datetime",
        }),
    }
}

/// Parse the `[bus]` section into a `BusSection`.
///
/// Requires a `type` string key. Remaining keys are preserved as
/// family-specific values for downstream normalization/validation.
fn parse_bus_section(table: &toml::Table) -> Result<BusSection, PlatformParseError> {
    let bus_type = extract_required_string(table, "bus.type", "type")?;

    let mut values = BTreeMap::new();
    for (key, value) in table {
        if key == "type" {
            continue;
        }
        values.insert(key.clone(), convert_value(value));
    }

    Ok(BusSection { bus_type, values })
}

/// Classify a top-level section as either construct-owned or unknown.
///
/// A section whose TOML table contains only sub-tables is treated as a
/// dotted-table group (e.g. `[mmio.uart_regs]`), producing one
/// `ConstructSection` per sub-table. A section whose table contains at
/// least one non-table value is treated as a simple unknown section.
fn classify_section(
    key: &str,
    section_table: &toml::Table,
    construct_sections: &mut Vec<ConstructSection>,
    unknown_sections: &mut Vec<UnknownSection>,
) {
    // Check if all values are sub-tables (dotted-table group pattern).
    let all_subtables = !section_table.is_empty()
        && section_table
            .values()
            .all(|v| matches!(v, toml::Value::Table(_)));

    if all_subtables {
        // Dotted-table group: each sub-table is a ConstructSection.
        for (sub_key, sub_value) in section_table {
            if let toml::Value::Table(sub_table) = sub_value {
                construct_sections.push(ConstructSection {
                    section_kind: key.to_string(),
                    section_key: Some(sub_key.clone()),
                    values: convert_table(sub_table),
                });
            }
        }
    } else {
        // Simple table: preserve as unknown section.
        unknown_sections.push(UnknownSection {
            section_path: vec![key.to_string()],
            values: convert_table(section_table),
        });
    }
}

/// Convert a TOML table into a `BTreeMap<String, PlatformValue>`.
fn convert_table(table: &toml::Table) -> BTreeMap<String, PlatformValue> {
    table
        .iter()
        .map(|(k, v)| (k.clone(), convert_value(v)))
        .collect()
}

/// Convert a TOML value into a `PlatformValue`.
fn convert_value(value: &toml::Value) -> PlatformValue {
    match value {
        toml::Value::Integer(i) => PlatformValue::Integer(*i),
        toml::Value::Boolean(b) => PlatformValue::Boolean(*b),
        toml::Value::String(s) => PlatformValue::String(s.clone()),
        toml::Value::Array(arr) => PlatformValue::List(arr.iter().map(convert_value).collect()),
        toml::Value::Table(t) => PlatformValue::Table(convert_table(t)),
        // TOML float/datetime/etc. map to string for forward compatibility.
        toml::Value::Float(f) => PlatformValue::String(f.to_string()),
        toml::Value::Datetime(dt) => PlatformValue::String(dt.to_string()),
    }
}
