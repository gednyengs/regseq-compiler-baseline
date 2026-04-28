//! Platform-join section logic.
//!
//! Every construct section is dispatched through one global iteration
//! in `(kind, id)` lexicographic order (see
//! [`crate::join::determinism::sorted_construct_sections`]). This is
//! the canonical Phase-2 iteration order prescribed by the task spec
//! -- collecting the per-kind dispatch into a single loop instead of
//! per-kind sub-passes guarantees that any future append of a new
//! kind slots into the same global order without a per-kind
//! rebalancing.
//!
//! [`join_platform_bus`] is separate because it owns
//! [`DriverStruct::platform_metadata`] rather than a resource-binding
//! slot. [`join_properties`] iterates `[properties]` entries by
//! `property_id` lexicographic order, with the declared
//! [`crate::ir::ScalarType`] driving [`PropertyTypeMismatch`]
//! detection.
//!
//! Unknown platform sections are rejected directly with
//! [`JoinErrorKind::UnknownPlatformSection`] and full platform-section
//! provenance -- this detection does not run from a verifier
//! side-record (per the task spec's explicit rule).
//!
//! # Binding field extraction
//!
//! The concrete per-kind binding structs in
//! [`crate::model::resources`] carry typed required and optional
//! fields. The extractors here default missing optional-in-struct
//! fields to their absent value (`None`, `false`, empty `Vec`) and
//! default missing required-in-struct fields to a zero-initialized
//! value -- the post-commit `verify_phase_2` pass is the authority on
//! whether the resulting binding is structurally acceptable, not the
//! join step. This keeps the joiner simple while still surfacing true
//! omissions through the commit-time verifier wrapper.
//!
//! [`DriverStruct::platform_metadata`]: crate::model::driver::DriverStruct::platform_metadata
//! [`MutationGuard::modify_driver_struct`]: crate::model::MutationGuard::modify_driver_struct
//! [`PropertyTypeMismatch`]: JoinErrorKind::PropertyTypeMismatch

use std::collections::BTreeMap;

use super::determinism::{sorted_construct_sections, sorted_properties};
use super::{DeclIndex, JoinError, JoinErrorKind, JoinProvenance, JoinResult};
use crate::ir::{LiteralValue, ScalarType};
use crate::model::driver::{PlatformBusBinding, PlatformMetadata};
use crate::model::mutation::MutationGuard;
use crate::model::resources::{
    ClockBinding, InterruptBinding, MmioBinding, PinCtrlBinding, ResetBinding,
};
use crate::platform::{BusSection, ConstructSection, PlatformHeader, PlatformValue, PropertyEntry};

/// Join the `[platform]` + `[bus]` sections into
/// [`DriverStruct::platform_metadata`].
///
/// Records `os`, `board`, `arch` from the header, plus the optional
/// bus-family `bus_type` and any family-specific key-value pairs
/// (e.g., `amba_id`, `revision`) into [`PlatformBusBinding::values`].
/// The values map is a [`std::collections::BTreeMap`], so its
/// serialized iteration order is lexicographic by key -- this is how
/// the task spec's "`platform.bus.amba_id` / `revision` recorded in
/// the driver metadata canonically (string order)" rule is honored.
///
/// [`DriverStruct::platform_metadata`]: crate::model::driver::DriverStruct::platform_metadata
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn join_platform_bus(
    guard: &mut MutationGuard<'_>,
    header: &PlatformHeader,
    bus: Option<&BusSection>,
) -> JoinResult {
    let metadata = PlatformMetadata {
        os: header.os.clone(),
        board: header.board.clone(),
        arch: header.arch.clone(),
        bus: bus.map(|b| PlatformBusBinding {
            bus_type: b.bus_type.clone(),
            values: b.values.clone(),
        }),
    };
    guard.modify_driver_struct(|driver| {
        driver.platform_metadata = Some(metadata);
    });
    Ok(())
}

/// Dispatch every construct section in global `(kind, id)`
/// lexicographic order to the matching per-kind resource slot.
///
/// This is the canonical Phase-2 iteration order (task spec,
/// "Phase-2 determinism" section): sections of every kind are
/// interleaved into a single sorted stream so adding a new kind
/// does not shift the order of existing kinds.
pub(crate) fn join_construct_sections(
    guard: &mut MutationGuard<'_>,
    sections: &[ConstructSection],
    index: &DeclIndex,
) -> JoinResult {
    let items = sorted_construct_sections(sections);

    // Validate in the same global order before any mutation; ensures
    // an unknown-id rejection happens without touching the driver.
    for (kind, id, _) in &items {
        match *kind {
            "clock" => {
                if !index.clocks.contains_key(*id) {
                    return Err(unknown_section("clock", id));
                }
            }
            "reset" => {
                if !index.resets.contains_key(*id) {
                    return Err(unknown_section("reset", id));
                }
            }
            "pinctrl" => {
                if !index.pinctrl.contains_key(*id) {
                    return Err(unknown_section("pinctrl", id));
                }
            }
            "interrupt" => {
                if !index.interrupts.contains_key(*id) {
                    return Err(unknown_section("interrupt", id));
                }
            }
            "mmio" => {
                if !index.mmio.contains_key(*id) {
                    return Err(unknown_section("mmio", id));
                }
            }
            // Construct sections whose `section_kind` string does
            // not name one of the five resource kinds are silently
            // ignored. The platform parser already categorizes
            // syntactically-unknown sections into
            // `PlatformFile::unknown_sections`; anything reaching
            // `construct_sections` with an unexpected kind is not a
            // resource section by spec.
            _ => {}
        }
    }

    // Apply mutations in the same global order so serialized
    // writes follow the canonical (kind, id) sequence.
    guard.modify_driver_struct(|driver| {
        for (kind, id, section) in items {
            match kind {
                "clock" => {
                    let idx = index.clocks[id];
                    driver.clocks[idx].binding = Some(ClockBinding {
                        default_frequency: extract_u64(&section.values, "default_frequency"),
                    });
                }
                "reset" => {
                    let idx = index.resets[id];
                    driver.resets[idx].binding = Some(ResetBinding {
                        reset_line_id: extract_string(&section.values, "reset_line_id"),
                        reset_shared: extract_bool(&section.values, "reset_shared", false),
                    });
                }
                "pinctrl" => {
                    let idx = index.pinctrl[id];
                    driver.pinctrl[idx].binding = Some(PinCtrlBinding {
                        pin_states: extract_string_list(&section.values, "pin_states"),
                    });
                }
                "interrupt" => {
                    let idx = index.interrupts[id];
                    driver.interrupts[idx].binding = Some(InterruptBinding {
                        irq_number: extract_u32(&section.values, "irq_number"),
                        trigger_type: extract_optional_string(&section.values, "trigger_type"),
                        trigger_polarity: extract_optional_string(
                            &section.values,
                            "trigger_polarity",
                        ),
                        shared: extract_bool(&section.values, "shared", false),
                        // ISR target routing runs at task 56's Phase-2
                        // cross-item rule against the driver's ISR
                        // bindings; platform files cannot name it.
                        isr_target: None,
                    });
                }
                "mmio" => {
                    let idx = index.mmio[id];
                    driver.mmio[idx].binding = Some(MmioBinding {
                        base: extract_u64(&section.values, "base"),
                        size: extract_u64(&section.values, "size"),
                    });
                }
                _ => {}
            }
        }
    });
    Ok(())
}

/// Join `[properties]` entries.
///
/// For each entry in property-id lexicographic order:
/// 1. Look up the declared property by id. Missing id raises
///    [`JoinErrorKind::UnknownPlatformSection`] with
///    `kind = "property"`.
/// 2. Check that the value kind matches the declared
///    [`ScalarType`]; the mapping is:
///    - Any integer scalar (`U8..U64`, `I8..I64`, `Usize`, `Isize`)
///      accepts [`PlatformValue::Integer`].
///    - No declared property type accepts booleans, strings, lists,
///      or tables -- the declared `ScalarType` set is integer-only.
///
///    A mismatch raises [`JoinErrorKind::PropertyTypeMismatch`].
/// 3. Set `bound_value = Some(...)` on the matching property field.
pub(crate) fn join_properties(
    guard: &mut MutationGuard<'_>,
    properties: &[PropertyEntry],
    index: &DeclIndex,
) -> JoinResult {
    let entries = sorted_properties(properties);

    let mut bindings: Vec<(usize, LiteralValue)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let Some(meta) = index.properties.get(&entry.key) else {
            return Err(JoinError {
                kind: JoinErrorKind::UnknownPlatformSection {
                    kind: String::from("property"),
                    id: entry.key.clone(),
                },
                provenance: JoinProvenance {
                    platform_section: format!("[properties].{}", entry.key),
                    resource_id: Some(entry.key.clone()),
                },
            });
        };
        let literal = coerce_property_value(&entry.key, meta.field_type, &entry.value)?;
        bindings.push((meta.index, literal));
    }

    guard.modify_driver_struct(|driver| {
        for (idx, lit) in bindings {
            driver.properties[idx].bound_value = Some(lit);
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn unknown_section(kind: &'static str, id: &str) -> JoinError {
    JoinError {
        kind: JoinErrorKind::UnknownPlatformSection {
            kind: kind.to_string(),
            id: id.to_string(),
        },
        provenance: JoinProvenance {
            platform_section: format!("[{kind}.{id}]"),
            resource_id: Some(id.to_string()),
        },
    }
}

fn coerce_property_value(
    property_id: &str,
    declared: ScalarType,
    value: &PlatformValue,
) -> Result<LiteralValue, JoinError> {
    match value {
        PlatformValue::Integer(i) => Ok(literal_from_integer(declared, *i)),
        other => Err(JoinError {
            kind: JoinErrorKind::PropertyTypeMismatch {
                property_id: property_id.to_string(),
                declared: scalar_type_tag(declared).to_string(),
                provided: platform_value_tag(other).to_string(),
            },
            provenance: JoinProvenance {
                platform_section: format!("[properties].{property_id}"),
                resource_id: Some(property_id.to_string()),
            },
        }),
    }
}

fn literal_from_integer(declared: ScalarType, v: i64) -> LiteralValue {
    match declared {
        ScalarType::U8 => LiteralValue::U8(v as u8),
        ScalarType::U16 => LiteralValue::U16(v as u16),
        ScalarType::U32 => LiteralValue::U32(v as u32),
        ScalarType::U64 => LiteralValue::U64(v as u64),
        ScalarType::Usize => LiteralValue::Usize(v as u64),
        ScalarType::I8 => LiteralValue::I8(v as i8),
        ScalarType::I16 => LiteralValue::I16(v as i16),
        ScalarType::I32 => LiteralValue::I32(v as i32),
        ScalarType::I64 => LiteralValue::I64(v),
        ScalarType::Isize => LiteralValue::Isize(v),
    }
}

fn scalar_type_tag(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::Usize => "usize",
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::Isize => "isize",
    }
}

fn platform_value_tag(value: &PlatformValue) -> &'static str {
    match value {
        PlatformValue::Integer(_) => "integer",
        PlatformValue::Boolean(_) => "boolean",
        PlatformValue::String(_) => "string",
        PlatformValue::List(_) => "list",
        PlatformValue::Table(_) => "table",
    }
}

fn extract_u32(values: &BTreeMap<String, PlatformValue>, key: &str) -> u32 {
    match values.get(key) {
        Some(PlatformValue::Integer(i)) => *i as u32,
        _ => 0,
    }
}

fn extract_u64(values: &BTreeMap<String, PlatformValue>, key: &str) -> u64 {
    match values.get(key) {
        Some(PlatformValue::Integer(i)) => *i as u64,
        _ => 0,
    }
}

fn extract_bool(values: &BTreeMap<String, PlatformValue>, key: &str, default: bool) -> bool {
    match values.get(key) {
        Some(PlatformValue::Boolean(b)) => *b,
        _ => default,
    }
}

fn extract_string(values: &BTreeMap<String, PlatformValue>, key: &str) -> String {
    match values.get(key) {
        Some(PlatformValue::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn extract_optional_string(values: &BTreeMap<String, PlatformValue>, key: &str) -> Option<String> {
    match values.get(key) {
        Some(PlatformValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn extract_string_list(values: &BTreeMap<String, PlatformValue>, key: &str) -> Vec<String> {
    match values.get(key) {
        Some(PlatformValue::List(list)) => list
            .iter()
            .filter_map(|v| match v {
                PlatformValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
