//! Normalization from parsed Layer 2 platform artifacts into the binding model.
//!
//! Converts `PlatformFile` (the parser-facing representation) into
//! `BindingModel` (the normalized binding IR). Normalization is
//! deterministic and local: it does not widen the Layer 2 contract,
//! infer declaration-side defaults, or perform joins.

use std::fmt;

use crate::declaration::driver::BusType;
use crate::platform::{BusSection, ConstructSection, PlatformFile, PlatformValue};

use super::bus::BusBinding;
use super::header::BindingHeader;
use super::join_key::JoinKey;
use super::properties::PropertyBinding;
use super::provenance::BindingProvenance;
use super::resources::{ClockBinding, InterruptBinding, MmioBinding, PinCtrlBinding, ResetBinding};
use super::BindingModel;

/// Errors produced during binding-model normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingNormalizeError {
    /// A construct section is missing its section key (the `{id}` part).
    MissingSectionKey {
        /// The construct family (e.g. `"mmio"`).
        section_kind: String,
    },
    /// The Layer 2 `[bus]` section specifies an unrecognized bus type.
    UnrecognizedBusType {
        /// The unrecognized bus type string.
        bus_type: String,
    },
    /// The Layer 1 and Layer 2 bus types do not match.
    BusTypeMismatch {
        /// Bus type declared in Layer 1.
        layer1: String,
        /// Bus type specified in Layer 2.
        layer2: String,
    },
    /// A family-specific key in the `[bus]` section is not valid for the
    /// selected bus family.
    InvalidBusKey {
        /// The invalid key name.
        key: String,
        /// The bus family that does not accept this key.
        bus_type: String,
    },
}

impl fmt::Display for BindingNormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSectionKey { section_kind } => {
                write!(
                    f,
                    "construct section `{section_kind}` is missing its section key"
                )
            }
            Self::UnrecognizedBusType { bus_type } => {
                write!(f, "unrecognized bus type `{bus_type}` in [bus] section")
            }
            Self::BusTypeMismatch { layer1, layer2 } => {
                write!(
                    f,
                    "bus type mismatch: Layer 1 declares `{layer1}` but Layer 2 specifies `{layer2}`"
                )
            }
            Self::InvalidBusKey { key, bus_type } => {
                write!(f, "key `{key}` is not valid for bus family `{bus_type}`")
            }
        }
    }
}

impl std::error::Error for BindingNormalizeError {}

/// Normalize a parsed `PlatformFile` into a `BindingModel`.
///
/// The optional `source_file` parameter is attached to provenance records
/// for traceability back to the original platform file on disk.
///
/// When `declaration_bus` is provided, the normalizer validates that the
/// Layer 2 bus type matches the Layer 1 declaration. Pass `None` to skip
/// cross-layer bus consistency checking.
pub fn normalize(
    pf: &PlatformFile,
    source_file: Option<&str>,
    declaration_bus: Option<&BusType>,
) -> Result<BindingModel, BindingNormalizeError> {
    let header = normalize_header(pf, source_file);
    let properties = normalize_properties(pf, source_file);

    let bus = match &pf.bus {
        Some(bus_section) => Some(normalize_bus(bus_section, source_file, declaration_bus)?),
        None => None,
    };

    let mut mmio = Vec::new();
    let mut interrupts = Vec::new();
    let mut clocks = Vec::new();
    let mut resets = Vec::new();
    let mut pinctrl = Vec::new();

    for section in &pf.construct_sections {
        match section.section_kind.as_str() {
            "mmio" => mmio.push(normalize_mmio(section, source_file)?),
            "interrupt" => interrupts.push(normalize_interrupt(section, source_file)?),
            "clock" => clocks.push(normalize_clock(section, source_file)?),
            "reset" => resets.push(normalize_reset(section, source_file)?),
            "pinctrl" => pinctrl.push(normalize_pinctrl(section, source_file)?),
            _ => {
                // Unrecognized construct sections are not normalized into
                // the binding model. They remain in the parser output only.
            }
        }
    }

    Ok(BindingModel {
        header,
        properties,
        mmio,
        interrupts,
        clocks,
        resets,
        pinctrl,
        bus,
    })
}

fn normalize_header(pf: &PlatformFile, source_file: Option<&str>) -> BindingHeader {
    let mut prov = BindingProvenance::new("platform");
    if let Some(sf) = source_file {
        prov = prov.with_source_file(sf);
    }

    BindingHeader {
        version: pf.version.clone(),
        os: pf.platform.os.clone(),
        arch: pf.platform.arch.clone(),
        board: pf.platform.board.clone(),
        provenance: prov,
    }
}

fn normalize_properties(pf: &PlatformFile, source_file: Option<&str>) -> Vec<PropertyBinding> {
    pf.properties
        .iter()
        .map(|entry| {
            let mut prov = BindingProvenance::new("properties").with_section_key(&entry.key);
            if let Some(sf) = source_file {
                prov = prov.with_source_file(sf);
            }
            PropertyBinding {
                key: entry.key.clone(),
                value: entry.value.clone(),
                provenance: prov,
            }
        })
        .collect()
}

fn require_section_key(section: &ConstructSection) -> Result<&str, BindingNormalizeError> {
    section
        .section_key
        .as_deref()
        .ok_or_else(|| BindingNormalizeError::MissingSectionKey {
            section_kind: section.section_kind.clone(),
        })
}

fn make_provenance(section: &ConstructSection, source_file: Option<&str>) -> BindingProvenance {
    let mut prov = BindingProvenance::new(&section.section_kind);
    if let Some(key) = &section.section_key {
        prov = prov.with_section_key(key);
    }
    if let Some(sf) = source_file {
        prov = prov.with_source_file(sf);
    }
    prov
}

/// Extract an optional integer field from the section values.
fn extract_integer(
    values: &std::collections::BTreeMap<String, PlatformValue>,
    key: &str,
) -> Option<i64> {
    match values.get(key) {
        Some(PlatformValue::Integer(i)) => Some(*i),
        _ => None,
    }
}

/// Extract an optional string field from the section values.
fn extract_string(
    values: &std::collections::BTreeMap<String, PlatformValue>,
    key: &str,
) -> Option<String> {
    match values.get(key) {
        Some(PlatformValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Extract an optional boolean field from the section values.
fn extract_bool(
    values: &std::collections::BTreeMap<String, PlatformValue>,
    key: &str,
) -> Option<bool> {
    match values.get(key) {
        Some(PlatformValue::Boolean(b)) => Some(*b),
        _ => None,
    }
}

/// Extract an optional list-of-strings field from the section values.
fn extract_string_list(
    values: &std::collections::BTreeMap<String, PlatformValue>,
    key: &str,
) -> Option<Vec<String>> {
    match values.get(key) {
        Some(PlatformValue::List(items)) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|v| match v {
                    PlatformValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            if strings.len() == items.len() {
                Some(strings)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn normalize_mmio(
    section: &ConstructSection,
    source_file: Option<&str>,
) -> Result<MmioBinding, BindingNormalizeError> {
    let key = require_section_key(section)?;
    Ok(MmioBinding {
        join_key: JoinKey::ExplicitId(key.to_string()),
        base: extract_integer(&section.values, "base"),
        size: extract_integer(&section.values, "size"),
        provenance: make_provenance(section, source_file),
    })
}

fn normalize_interrupt(
    section: &ConstructSection,
    source_file: Option<&str>,
) -> Result<InterruptBinding, BindingNormalizeError> {
    let key = require_section_key(section)?;
    Ok(InterruptBinding {
        join_key: JoinKey::ExplicitId(key.to_string()),
        trigger_type: extract_string(&section.values, "trigger_type"),
        trigger_polarity: extract_string(&section.values, "trigger_polarity"),
        priority: extract_string(&section.values, "priority"),
        shared: extract_bool(&section.values, "shared"),
        provenance: make_provenance(section, source_file),
    })
}

fn normalize_clock(
    section: &ConstructSection,
    source_file: Option<&str>,
) -> Result<ClockBinding, BindingNormalizeError> {
    let key = require_section_key(section)?;
    Ok(ClockBinding {
        join_key: JoinKey::ExplicitId(key.to_string()),
        default_frequency: extract_integer(&section.values, "default_frequency"),
        provenance: make_provenance(section, source_file),
    })
}

fn normalize_reset(
    section: &ConstructSection,
    source_file: Option<&str>,
) -> Result<ResetBinding, BindingNormalizeError> {
    let key = require_section_key(section)?;
    Ok(ResetBinding {
        join_key: JoinKey::FieldName(key.to_string()),
        reset_line_id: extract_string(&section.values, "reset_line_id"),
        reset_shared: extract_bool(&section.values, "reset_shared"),
        provenance: make_provenance(section, source_file),
    })
}

fn normalize_pinctrl(
    section: &ConstructSection,
    source_file: Option<&str>,
) -> Result<PinCtrlBinding, BindingNormalizeError> {
    let key = require_section_key(section)?;
    Ok(PinCtrlBinding {
        join_key: JoinKey::ExplicitId(key.to_string()),
        pin_states: extract_string_list(&section.values, "pin_states"),
        provenance: make_provenance(section, source_file),
    })
}

/// Return the set of accepted family-specific keys for a given bus type.
fn accepted_bus_keys(bus_type: &BusType) -> &'static [&'static str] {
    match bus_type {
        BusType::Amba => &["amba_id", "revision", "amba_mask"],
        BusType::Pci => &["vendor_id", "device_id"],
        BusType::Integrated => &["firmware"],
    }
}

/// Validate that all keys in the bus section are accepted for the given family.
fn validate_bus_keys(
    bus_section: &BusSection,
    bus_type: &BusType,
) -> Result<(), BindingNormalizeError> {
    let accepted = accepted_bus_keys(bus_type);
    for key in bus_section.values.keys() {
        if !accepted.contains(&key.as_str()) {
            return Err(BindingNormalizeError::InvalidBusKey {
                key: key.clone(),
                bus_type: bus_type.as_str().to_string(),
            });
        }
    }
    Ok(())
}

fn normalize_bus(
    bus_section: &BusSection,
    source_file: Option<&str>,
    declaration_bus: Option<&BusType>,
) -> Result<BusBinding, BindingNormalizeError> {
    let bus_type = BusType::from_raw(&bus_section.bus_type).ok_or_else(|| {
        BindingNormalizeError::UnrecognizedBusType {
            bus_type: bus_section.bus_type.clone(),
        }
    })?;

    // Layer 1 / Layer 2 consistency check.
    if let Some(decl_bus) = declaration_bus {
        if *decl_bus != bus_type {
            return Err(BindingNormalizeError::BusTypeMismatch {
                layer1: decl_bus.as_str().to_string(),
                layer2: bus_type.as_str().to_string(),
            });
        }
    }

    // Validate family-specific keys.
    validate_bus_keys(bus_section, &bus_type)?;

    let mut prov = BindingProvenance::new("bus");
    if let Some(sf) = source_file {
        prov = prov.with_source_file(sf);
    }

    Ok(BusBinding {
        bus_type,
        amba_id: extract_integer(&bus_section.values, "amba_id"),
        revision: extract_integer(&bus_section.values, "revision"),
        amba_mask: extract_integer(&bus_section.values, "amba_mask"),
        provenance: prov,
    })
}
