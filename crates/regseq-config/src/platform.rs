//! Platform-file (`platform.toml`) parsing for the bus-resource-management
//! construct (#3).
//!
//! The platform file is the L2 binding surface. This module parses the
//! `[mmio.<id>]` sections — owned by register-access (the resource id), with
//! bus-resource owning the `bar`/`base`/`size`/`type` keys (GRILL C01). The
//! parsed [`PlatformBindings`] is joined to the model's resource fields by the
//! `regseq-driver` bridge, which resolves the effective BAR + `(base, size)`
//! and runs the `BRM-005/006` validation. This module does **not** validate the
//! binding-pair or resolve effective BARs — it only deserializes the keys.
//!
//! Three sections are parsed (in one pass — [`parse_platform_file_str`]): the
//! `[mmio.<id>]` bindings above, the `[interrupt.<id>]` bindings of the
//! device-interrupt construct (#6, contract C03 — structural only; the
//! canonical-spelling validation of `trigger_type`/`trigger_polarity` is the
//! `regseq-l2` interrupt tenant's), and the `[platform]` table into a
//! [`PlatformIdentityRaw`] (os/board/arch, structural only — the os token's
//! closed-vocabulary check is `regseq-l2`'s, keeping this crate a `serde`-only
//! leaf; R5/D8). Any other top-level construct section is accepted-and-ignored
//! (forward-compat), matching the layer-2 "unknown construct sections are
//! placeholders" rule.

use std::collections::BTreeMap;
use std::path::Path;

/// One `[mmio.<id>]` binding parsed from a platform file (GRILL C01). Every key
/// is optional; the join, effective-BAR resolution, and binding-pair / range
/// validation happen in the bridge (construct #3 T05/M6). `type = "io"` is
/// recorded as `bar_type_io`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MmioBindingSpec {
    /// `bar` — the L2 supply-or-override BAR index (PCI-like bus only).
    pub bar: Option<u8>,
    /// `base` — the region physical base (host literal; GRILL D06).
    pub base: Option<u64>,
    /// `size` — the region length (binds with `base`; GRILL I02).
    pub size: Option<u64>,
    /// The `type` carrier marker (PCI-like bus only): `Some(true)` = `"io"`,
    /// `Some(false)` = an explicit `"mem"` (overrides an L1 `type = "io"`),
    /// `None` = unspecified (the L1 carrier stands).
    pub bar_type_io: Option<bool>,
}

/// One `[interrupt.<id>]` binding parsed from a platform file (device-interrupt
/// construct #6, contract C03). Structural parse only: `trigger_type` /
/// `trigger_polarity` are captured **verbatim** — the canonical-spelling
/// validation (`level|edge`; `active_high|active_low|rising|falling`, GRILL
/// D10/R12) is the `regseq-l2` interrupt tenant's. `shared` defaults `false`
/// when absent (the C03 board-truth default, GRILL D05/I08).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterruptBindingSpec {
    /// `irq_number` — the platform line number (required for generic-c/static
    /// targets, advisory for zephyr, unused for linux runtime resolution —
    /// R10; enforced downstream).
    pub irq_number: Option<u32>,
    /// `trigger_type` token, verbatim (e.g. `"level"`).
    pub trigger_type: Option<String>,
    /// `trigger_polarity` token, verbatim (e.g. `"active_high"`).
    pub trigger_polarity: Option<String>,
    /// `priority` — scheduling priority (zephyr `IRQ_CONNECT` argument only).
    pub priority: Option<u32>,
    /// `shared` — the line is electrically shared on this board (GRILL D05).
    /// Defaults `false` when the key is absent.
    pub shared: bool,
}

/// One `[clock.<id>]` binding parsed from a platform file
/// (peripheral-clock-management construct, contract C01). Structural parse only:
/// `mode` / `provider` are captured **verbatim** (the `managed | static`
/// vocabulary check and the mode↔provider invariant are the `regseq-l2` clock
/// tenant's, GRILL D14/D15); `default_frequency` is parsed as a non-negative
/// `u32` (the `> 0` requirement is the tenant's, GRILL D15). All fields are
/// `Option` at parse time — required-ness is enforced by the L2 tenant's
/// `validate`, not the parser (GRILL D14, mirroring `InterruptBindingSpec`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClockBindingSpec {
    /// `mode` token, verbatim (`"managed"` | `"static"`); validated downstream.
    pub mode: Option<String>,
    /// `default_frequency` in Hz. Non-integer / negative / `> u32::MAX` is a
    /// parse reject; the `> 0` and presence requirements are the L2 tenant's.
    pub default_frequency: Option<u32>,
    /// `provider` — the backend-interpreted controller reference (con-id /
    /// DT clock spec), verbatim. Required iff managed, forbidden iff static
    /// (enforced downstream, GRILL D14).
    pub provider: Option<String>,
}

/// One `[pinctrl.<id>]` binding parsed from a platform file
/// (pin-controller-and-GPIO-configuration construct, contract C01). Structural
/// parse only: `pin_states` is captured as a typed `Vec<String>` (the SHAPE — a
/// present, array-of-strings value — is the parser's; the CONTENT checks
/// (non-empty, unique, non-empty names, `"default"` membership) are the
/// `regseq-l2` pinctrl tenant's, GRILL C01/C06).
///
/// This is a NEW parse shape — an array of strings, with no clock/mmio/interrupt
/// scalar precedent — so `parse_pinctrl_section` is authored fresh rather than
/// copied. A typed `Vec<String>` cannot carry a non-string entry to L2, so a
/// non-string array element MUST parse-reject (`BadPinctrl`), not defer to PIN-L2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinctrlBindingSpec {
    /// `pin_states` — the names the driver may select (`default` / `sleep`).
    /// SHAPE-validated here (present, array, all-string); CONTENT downstream.
    pub pin_states: Vec<String>,
}

/// The L2 bindings parsed from a platform file: the `[mmio.<id>]` and
/// `[interrupt.<id>]` sections keyed by resource id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformBindings {
    /// `[mmio.<id>]` sections, keyed by the resource id (`#[rseq::mmio(id =
    /// "<id>")]`).
    pub mmio: BTreeMap<String, MmioBindingSpec>,
    /// `[interrupt.<id>]` sections, keyed by the resource id
    /// (`#[rseq::interrupt(id = "<id>")]`). An absent section is absence in
    /// the map (Option semantics are the L2 tenant's, C03).
    pub interrupt: BTreeMap<String, InterruptBindingSpec>,
    /// `[clock.<id>]` sections, keyed by the resource id
    /// (`#[rseq::clock(id = "<id>")]`, peripheral-clock-management construct).
    /// An absent section is absence in the map; required-ness is the L2 tenant's
    /// (C01).
    pub clock: BTreeMap<String, ClockBindingSpec>,
    /// `[pinctrl.<id>]` sections, keyed by the resource id
    /// (`#[rseq::pinctrl(id = "<id>")]`, pin-controller-and-GPIO-configuration
    /// construct). An absent section is absence in the map — for the optional
    /// `Option<PinCtrl>` form that is the allowed absent case (binds to `None`);
    /// for the required `PinCtrl` form a missing section is the L2 tenant's
    /// `PIN-L2-NO-INVENTORY` (C01/D03).
    pub pinctrl: BTreeMap<String, PinctrlBindingSpec>,
}

/// The raw, structurally-parsed `[platform]` identity (R5/D8). `regseq-config`
/// stays a `serde`-only leaf: the os token is captured verbatim as a string and
/// validated against the closed `TargetOs` vocabulary downstream, by the
/// `regseq-l2` bridge (`resolve_platform_identity`). All fields optional; an
/// absent `[platform]` table yields all-`None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformIdentityRaw {
    /// `[platform] os` token, unvalidated (e.g. `"linux"`).
    pub os: Option<String>,
    /// `[platform] board` (free-form; e.g. `"qemu-pc"`).
    pub board: Option<String>,
    /// `[platform] arch` (free-form; e.g. `"x86_64"`).
    pub arch: Option<String>,
}

/// A platform file parsed in a single pass (Codex A1 / R5): the `[mmio.<id>]`
/// [`PlatformBindings`] plus the raw `[platform]` identity. Parsing once keeps
/// the `[mmio]` and `[platform]` parse semantics from drifting across two reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformFile {
    /// `[mmio.<id>]` resource bindings (L2 → model, via `regseq-l2::bind_all`).
    pub bindings: PlatformBindings,
    /// Raw `[platform]` identity (validated downstream by `regseq-l2`).
    pub identity: PlatformIdentityRaw,
}

/// A platform-file parse failure.
#[derive(Debug, thiserror::Error)]
pub enum PlatformParseError {
    /// The file could not be read.
    #[error("platform file read error ({path}): {detail}")]
    Io { path: String, detail: String },
    /// The file is not valid TOML.
    #[error("platform file is not valid TOML: {0}")]
    InvalidToml(String),
    /// A `[mmio.<id>]` section or one of its keys has the wrong shape.
    #[error("[mmio.{id}] {key}: {detail}")]
    BadValue {
        id: String,
        key: String,
        detail: String,
    },
    /// A `[platform]` key (or the section itself) has the wrong shape. Distinct
    /// from [`PlatformParseError::BadValue`] so the diagnostic is labelled
    /// `[platform]`, not `[mmio.…]`. (The os *vocabulary* check is `regseq-l2`'s,
    /// not here — R5.)
    #[error("[platform] {key}: {detail}")]
    BadPlatform { key: String, detail: String },
    /// An `[interrupt.<id>]` section or one of its keys has the wrong shape
    /// (device-interrupt construct #6). Distinct from
    /// [`PlatformParseError::BadValue`] so the diagnostic is labelled
    /// `[interrupt.…]`, not `[mmio.…]`.
    #[error("[interrupt.{id}] {key}: {detail}")]
    BadInterrupt {
        id: String,
        key: String,
        detail: String,
    },
    /// A `[clock.<id>]` section or one of its keys has the wrong shape
    /// (peripheral-clock-management construct). Distinct from
    /// [`PlatformParseError::BadValue`] so the diagnostic is labelled
    /// `[clock.…]`, not `[mmio.…]`.
    #[error("[clock.{id}] {key}: {detail}")]
    BadClock {
        id: String,
        key: String,
        detail: String,
    },
    /// A `[pinctrl.<id>]` section or one of its keys has the wrong SHAPE
    /// (pin-controller-and-GPIO-configuration construct): a missing `pin_states`
    /// key, a non-array `pin_states`, or a non-string array entry. Distinct from
    /// [`PlatformParseError::BadValue`] so the diagnostic is labelled
    /// `[pinctrl.…]`, not `[mmio.…]`. CONTENT errors (empty/duplicate/empty-name/
    /// no-default) are the `regseq-l2` tenant's `PIN-L2-*`, not here (C01/C06).
    #[error("[pinctrl.{id}] {key}: {detail}")]
    BadPinctrl {
        id: String,
        key: String,
        detail: String,
    },
}

/// Parse a platform-file string in a single pass (Codex A1 / R5) into its
/// `[mmio.<id>]` [`PlatformBindings`] and raw `[platform]` identity. Everything
/// else is accepted and ignored (forward-compat). The os token is captured
/// verbatim — the closed-vocabulary check is `regseq-l2`'s, keeping this crate a
/// `serde`-only leaf.
pub fn parse_platform_file_str(input: &str) -> Result<PlatformFile, PlatformParseError> {
    let table: toml::Table = input
        .parse()
        .map_err(|e: toml::de::Error| PlatformParseError::InvalidToml(e.to_string()))?;

    let mut mmio = BTreeMap::new();
    if let Some(mmio_val) = table.get("mmio") {
        let toml::Value::Table(mmio_tbl) = mmio_val else {
            return Err(PlatformParseError::BadValue {
                id: "*".to_string(),
                key: "mmio".to_string(),
                detail: "expected a `[mmio.<id>]` table".to_string(),
            });
        };
        for (id, val) in mmio_tbl {
            let toml::Value::Table(section) = val else {
                return Err(PlatformParseError::BadValue {
                    id: id.clone(),
                    key: "<section>".to_string(),
                    detail: "expected a table".to_string(),
                });
            };
            mmio.insert(id.clone(), parse_mmio_section(id, section)?);
        }
    }

    let mut interrupt = BTreeMap::new();
    if let Some(interrupt_val) = table.get("interrupt") {
        let toml::Value::Table(interrupt_tbl) = interrupt_val else {
            return Err(PlatformParseError::BadInterrupt {
                id: "*".to_string(),
                key: "interrupt".to_string(),
                detail: "expected an `[interrupt.<id>]` table".to_string(),
            });
        };
        for (id, val) in interrupt_tbl {
            let toml::Value::Table(section) = val else {
                return Err(PlatformParseError::BadInterrupt {
                    id: id.clone(),
                    key: "<section>".to_string(),
                    detail: "expected a table".to_string(),
                });
            };
            interrupt.insert(id.clone(), parse_interrupt_section(id, section)?);
        }
    }

    let mut clock = BTreeMap::new();
    if let Some(clock_val) = table.get("clock") {
        let toml::Value::Table(clock_tbl) = clock_val else {
            return Err(PlatformParseError::BadClock {
                id: "*".to_string(),
                key: "clock".to_string(),
                detail: "expected a `[clock.<id>]` table".to_string(),
            });
        };
        for (id, val) in clock_tbl {
            let toml::Value::Table(section) = val else {
                return Err(PlatformParseError::BadClock {
                    id: id.clone(),
                    key: "<section>".to_string(),
                    detail: "expected a table".to_string(),
                });
            };
            clock.insert(id.clone(), parse_clock_section(id, section)?);
        }
    }

    let mut pinctrl = BTreeMap::new();
    if let Some(pinctrl_val) = table.get("pinctrl") {
        let toml::Value::Table(pinctrl_tbl) = pinctrl_val else {
            return Err(PlatformParseError::BadPinctrl {
                id: "*".to_string(),
                key: "pinctrl".to_string(),
                detail: "expected a `[pinctrl.<id>]` table".to_string(),
            });
        };
        for (id, val) in pinctrl_tbl {
            let toml::Value::Table(section) = val else {
                return Err(PlatformParseError::BadPinctrl {
                    id: id.clone(),
                    key: "<section>".to_string(),
                    detail: "expected a table".to_string(),
                });
            };
            pinctrl.insert(id.clone(), parse_pinctrl_section(id, section)?);
        }
    }

    let identity = parse_platform_identity(&table)?;

    Ok(PlatformFile {
        bindings: PlatformBindings {
            mmio,
            interrupt,
            clock,
            pinctrl,
        },
        identity,
    })
}

/// Read and parse a platform file at `path` in one pass (see
/// [`parse_platform_file_str`]).
pub fn parse_platform_file_path(path: &Path) -> Result<PlatformFile, PlatformParseError> {
    let text = std::fs::read_to_string(path).map_err(|e| PlatformParseError::Io {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    parse_platform_file_str(&text)
}

/// Parse a platform-file string into just its L2 [`PlatformBindings`]. Thin
/// wrapper over [`parse_platform_file_str`] (the single parse) so the two
/// existing `[mmio]`-only callers don't churn.
pub fn parse_platform_bindings_str(input: &str) -> Result<PlatformBindings, PlatformParseError> {
    parse_platform_file_str(input).map(|f| f.bindings)
}

/// Read and parse a platform file at `path` into just its [`PlatformBindings`]
/// (thin wrapper over [`parse_platform_file_path`]).
pub fn parse_platform_bindings_path(path: &Path) -> Result<PlatformBindings, PlatformParseError> {
    parse_platform_file_path(path).map(|f| f.bindings)
}

/// Structurally parse the `[platform]` table into [`PlatformIdentityRaw`]. Each
/// key must be a string when present; the os token is NOT validated here (that
/// is `regseq-l2`'s job — R5). An absent `[platform]` table yields all-`None`.
fn parse_platform_identity(
    table: &toml::Table,
) -> Result<PlatformIdentityRaw, PlatformParseError> {
    let Some(platform_val) = table.get("platform") else {
        return Ok(PlatformIdentityRaw::default());
    };
    let toml::Value::Table(section) = platform_val else {
        return Err(PlatformParseError::BadPlatform {
            key: "<section>".to_string(),
            detail: "expected a `[platform]` table".to_string(),
        });
    };
    let str_key = |key: &str| -> Result<Option<String>, PlatformParseError> {
        match section.get(key) {
            None => Ok(None),
            Some(toml::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(PlatformParseError::BadPlatform {
                key: key.to_string(),
                detail: "expected a string".to_string(),
            }),
        }
    };
    Ok(PlatformIdentityRaw {
        os: str_key("os")?,
        board: str_key("board")?,
        arch: str_key("arch")?,
    })
}

/// Parse one `[mmio.<id>]` section's `bar`/`base`/`size`/`type` keys. Unknown
/// keys are ignored (forward-compat). TOML integers are `i64`; a negative value
/// is rejected. `bar` must fit `u8` (the 0..5 range check is BRM-002, later).
fn parse_mmio_section(
    id: &str,
    section: &toml::Table,
) -> Result<MmioBindingSpec, PlatformParseError> {
    let mut spec = MmioBindingSpec::default();
    if let Some(v) = section.get("bar") {
        let n = as_u64(id, "bar", v)?;
        spec.bar = Some(u8::try_from(n).map_err(|_| PlatformParseError::BadValue {
            id: id.to_string(),
            key: "bar".to_string(),
            detail: format!("{n} does not fit a BAR index"),
        })?);
    }
    if let Some(v) = section.get("base") {
        spec.base = Some(as_u64(id, "base", v)?);
    }
    if let Some(v) = section.get("size") {
        spec.size = Some(as_u64(id, "size", v)?);
    }
    if let Some(v) = section.get("type") {
        let toml::Value::String(s) = v else {
            return Err(PlatformParseError::BadValue {
                id: id.to_string(),
                key: "type".to_string(),
                detail: "expected a string (\"mem\" or \"io\")".to_string(),
            });
        };
        // Explicit closed vocabulary: reject a typo rather than silently treating
        // any non-`"io"` value as `"mem"` (which would override an L1 carrier).
        spec.bar_type_io = Some(match s.as_str() {
            "io" => true,
            "mem" => false,
            other => {
                return Err(PlatformParseError::BadValue {
                    id: id.to_string(),
                    key: "type".to_string(),
                    detail: format!("expected \"mem\" or \"io\", got {other:?}"),
                })
            }
        });
    }
    Ok(spec)
}

/// Parse one `[interrupt.<id>]` section's C03 keys (`irq_number` /
/// `trigger_type` / `trigger_polarity` / `priority` / `shared`). Unknown keys
/// are ignored (forward-compat, matching `[mmio.<id>]`). Structural checks
/// only: integers must be non-negative and fit `u32`, the trigger tokens must
/// be strings (captured verbatim), `shared` must be a boolean. The
/// canonical-spelling vocabulary is the `regseq-l2` tenant's (D10/R12).
fn parse_interrupt_section(
    id: &str,
    section: &toml::Table,
) -> Result<InterruptBindingSpec, PlatformParseError> {
    let bad = |key: &str, detail: String| PlatformParseError::BadInterrupt {
        id: id.to_string(),
        key: key.to_string(),
        detail,
    };
    let u32_key = |key: &str| -> Result<Option<u32>, PlatformParseError> {
        match section.get(key) {
            None => Ok(None),
            Some(toml::Value::Integer(i)) if *i >= 0 => Ok(Some(
                u32::try_from(*i).map_err(|_| bad(key, format!("{i} does not fit u32")))?,
            )),
            Some(toml::Value::Integer(_)) => Err(bad(key, "must be non-negative".to_string())),
            Some(_) => Err(bad(key, "expected an integer".to_string())),
        }
    };
    let str_key = |key: &str| -> Result<Option<String>, PlatformParseError> {
        match section.get(key) {
            None => Ok(None),
            Some(toml::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(bad(key, "expected a string".to_string())),
        }
    };
    let mut spec = InterruptBindingSpec {
        irq_number: u32_key("irq_number")?,
        trigger_type: str_key("trigger_type")?,
        trigger_polarity: str_key("trigger_polarity")?,
        priority: u32_key("priority")?,
        shared: false,
    };
    if let Some(v) = section.get("shared") {
        let toml::Value::Boolean(b) = v else {
            return Err(bad("shared", "expected a boolean".to_string()));
        };
        spec.shared = *b;
    }
    Ok(spec)
}

/// Parse one `[clock.<id>]` section's C01 keys (`mode` / `default_frequency` /
/// `provider`). Unknown keys are ignored (forward-compat, matching
/// `[mmio.<id>]` / `[interrupt.<id>]` — this introduces no clock-specific
/// unknown-key rule). Structural checks only: `default_frequency` must be a
/// non-negative integer that fits `u32` (the `> 0` requirement is the
/// `regseq-l2` tenant's, D15), and `mode` / `provider` must be strings (captured
/// verbatim — the `managed | static` vocabulary and the mode↔provider invariant
/// are the tenant's, D14/D15).
fn parse_clock_section(
    id: &str,
    section: &toml::Table,
) -> Result<ClockBindingSpec, PlatformParseError> {
    let bad = |key: &str, detail: String| PlatformParseError::BadClock {
        id: id.to_string(),
        key: key.to_string(),
        detail,
    };
    let u32_key = |key: &str| -> Result<Option<u32>, PlatformParseError> {
        match section.get(key) {
            None => Ok(None),
            Some(toml::Value::Integer(i)) if *i >= 0 => Ok(Some(
                u32::try_from(*i).map_err(|_| bad(key, format!("{i} does not fit u32")))?,
            )),
            Some(toml::Value::Integer(_)) => Err(bad(key, "must be non-negative".to_string())),
            Some(_) => Err(bad(key, "expected an integer".to_string())),
        }
    };
    let str_key = |key: &str| -> Result<Option<String>, PlatformParseError> {
        match section.get(key) {
            None => Ok(None),
            Some(toml::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(bad(key, "expected a string".to_string())),
        }
    };
    Ok(ClockBindingSpec {
        mode: str_key("mode")?,
        default_frequency: u32_key("default_frequency")?,
        provider: str_key("provider")?,
    })
}

/// Parse one `[pinctrl.<id>]` section's C01 key (`pin_states`). Unknown keys are
/// ignored (forward-compat, matching `[clock.<id>]` / `[mmio.<id>]`). SHAPE only
/// (pin-controller-and-GPIO-configuration construct, C01): `pin_states` MUST be
/// present and be an array whose every entry is a string. A missing key, a
/// non-array value, or a non-string entry is a `BadPinctrl` parse reject (a
/// typed `Vec<String>` cannot carry a non-string to L2). CONTENT — empty array,
/// duplicate names, empty/whitespace names, missing `"default"` — is the
/// `regseq-l2` pinctrl tenant's `PIN-L2-*` (C06), NOT here.
fn parse_pinctrl_section(
    id: &str,
    section: &toml::Table,
) -> Result<PinctrlBindingSpec, PlatformParseError> {
    let bad = |key: &str, detail: String| PlatformParseError::BadPinctrl {
        id: id.to_string(),
        key: key.to_string(),
        detail,
    };
    let Some(pin_states_val) = section.get("pin_states") else {
        return Err(bad(
            "pin_states",
            "required key is missing (expected an array of state-name strings)".to_string(),
        ));
    };
    let toml::Value::Array(entries) = pin_states_val else {
        return Err(bad("pin_states", "expected an array of strings".to_string()));
    };
    let mut pin_states = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let toml::Value::String(s) = entry else {
            return Err(bad(
                "pin_states",
                format!("entry {idx} is not a string"),
            ));
        };
        pin_states.push(s.clone());
    }
    Ok(PinctrlBindingSpec { pin_states })
}

/// Coerce a TOML value to a non-negative `u64`, or a typed error.
fn as_u64(id: &str, key: &str, v: &toml::Value) -> Result<u64, PlatformParseError> {
    match v {
        toml::Value::Integer(i) if *i >= 0 => Ok(*i as u64),
        toml::Value::Integer(_) => Err(PlatformParseError::BadValue {
            id: id.to_string(),
            key: key.to_string(),
            detail: "must be non-negative".to_string(),
        }),
        _ => Err(PlatformParseError::BadValue {
            id: id.to_string(),
            key: key.to_string(),
            detail: "expected an integer".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base_size_bar_and_type() {
        let input = r#"
version = "1"
[platform]
os = "linux"
[mmio.regs]
bar = 0
base = 0xf0000000
size = 0x20000
type = "io"
[mmio.bar2]
bar = 2
"#;
        let b = parse_platform_bindings_str(input).expect("parse");
        let regs = b.mmio.get("regs").expect("regs section");
        assert_eq!(regs.bar, Some(0));
        assert_eq!(regs.base, Some(0xf000_0000));
        assert_eq!(regs.size, Some(0x2_0000));
        assert_eq!(regs.bar_type_io, Some(true));
        assert_eq!(b.mmio.get("bar2").unwrap().bar, Some(2));
    }

    #[test]
    fn empty_or_headerless_file_yields_no_bindings() {
        assert!(parse_platform_bindings_str("version = \"1\"\n")
            .unwrap()
            .mmio
            .is_empty());
    }

    #[test]
    fn rejects_non_integer_base() {
        let err = parse_platform_bindings_str("[mmio.regs]\nbase = \"nope\"\n").unwrap_err();
        assert!(matches!(err, PlatformParseError::BadValue { .. }));
    }

    #[test]
    fn unspecified_type_is_none() {
        let b = parse_platform_bindings_str("[mmio.regs]\nbase = 0\nsize = 16\n").unwrap();
        assert_eq!(b.mmio.get("regs").unwrap().bar_type_io, None);
    }

    #[test]
    fn explicit_mem_type_is_some_false() {
        let b = parse_platform_bindings_str("[mmio.regs]\ntype = \"mem\"\n").unwrap();
        assert_eq!(b.mmio.get("regs").unwrap().bar_type_io, Some(false));
    }

    #[test]
    fn rejects_unknown_type_value() {
        // A typo must be rejected, not silently coerced to `mem` (construct #3
        // holistic-review fix): only "mem"/"io" are valid.
        let err = parse_platform_bindings_str("[mmio.regs]\ntype = \"memm\"\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadValue { key, .. } if key == "type"),
            "got {err:?}"
        );
    }

    // --- [interrupt.<id>] (device-interrupt construct #6, C03): structural
    // parse only; canonical trigger spellings are validated downstream by the
    // regseq-l2 interrupt tenant, not here. ---

    #[test]
    fn parses_interrupt_section_pl011_full_spec_shape() {
        // The aspirational `pl011-full-spec` fixture's section, verbatim
        // (C03/S05). `trigger_polarity = "high"` is non-canonical and must
        // still PARSE here — rejecting it is the L2 binder's job (D10/R12).
        let input = r#"
version = "1"
[platform]
os = "linux"
[mmio.uart_regs]
base = 0x09000000
size = 4096
[interrupt.uart_irq]
irq_number = 37
trigger_type = "level"
trigger_polarity = "high"
shared = false
"#;
        let b = parse_platform_bindings_str(input).expect("parse");
        let irq = b.interrupt.get("uart_irq").expect("uart_irq section");
        assert_eq!(irq.irq_number, Some(37));
        assert_eq!(irq.trigger_type.as_deref(), Some("level"));
        assert_eq!(irq.trigger_polarity.as_deref(), Some("high"));
        assert_eq!(irq.priority, None);
        assert!(!irq.shared);
        // The mmio section still parses alongside.
        assert_eq!(b.mmio.get("uart_regs").unwrap().size, Some(4096));
    }

    #[test]
    fn absent_interrupt_section_parses_to_absence() {
        let b = parse_platform_bindings_str("[mmio.regs]\nbase = 0\nsize = 16\n").unwrap();
        assert!(b.interrupt.is_empty());
    }

    #[test]
    fn interrupt_shared_defaults_false_and_parses_true() {
        let b = parse_platform_bindings_str("[interrupt.a]\nirq_number = 1\n").unwrap();
        assert!(!b.interrupt.get("a").unwrap().shared);
        let b = parse_platform_bindings_str("[interrupt.a]\nshared = true\n").unwrap();
        assert!(b.interrupt.get("a").unwrap().shared);
    }

    #[test]
    fn interrupt_priority_parses() {
        let b = parse_platform_bindings_str("[interrupt.a]\npriority = 3\n").unwrap();
        assert_eq!(b.interrupt.get("a").unwrap().priority, Some(3));
    }

    #[test]
    fn interrupt_unknown_keys_are_ignored() {
        // Forward-compat, matching the `[mmio.<id>]` discipline.
        let b = parse_platform_bindings_str("[interrupt.a]\nirq_number = 5\nfuture = 1\n").unwrap();
        assert_eq!(b.interrupt.get("a").unwrap().irq_number, Some(5));
    }

    #[test]
    fn interrupt_rejects_non_integer_irq_number() {
        let err = parse_platform_bindings_str("[interrupt.a]\nirq_number = \"x\"\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadInterrupt { id, key, .. }
                if id == "a" && key == "irq_number"),
            "got {err:?}"
        );
    }

    #[test]
    fn interrupt_rejects_negative_and_oversized_integers() {
        let err = parse_platform_bindings_str("[interrupt.a]\nirq_number = -1\n").unwrap_err();
        assert!(matches!(&err, PlatformParseError::BadInterrupt { .. }), "got {err:?}");
        let err =
            parse_platform_bindings_str("[interrupt.a]\npriority = 4294967296\n").unwrap_err();
        assert!(matches!(&err, PlatformParseError::BadInterrupt { .. }), "got {err:?}");
    }

    #[test]
    fn interrupt_rejects_non_string_trigger_and_non_bool_shared() {
        let err = parse_platform_bindings_str("[interrupt.a]\ntrigger_type = 1\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadInterrupt { key, .. } if key == "trigger_type"),
            "got {err:?}"
        );
        let err = parse_platform_bindings_str("[interrupt.a]\nshared = \"no\"\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadInterrupt { key, .. } if key == "shared"),
            "got {err:?}"
        );
    }

    #[test]
    fn interrupt_error_is_labelled_interrupt_not_mmio() {
        let err = parse_platform_bindings_str("[interrupt.a]\nshared = 3\n").unwrap_err();
        assert!(
            err.to_string().starts_with("[interrupt.a]"),
            "got {err}"
        );
    }

    // --- [clock.<id>] (peripheral-clock-management construct, C01): structural
    // parse only; the `managed | static` vocabulary, the mode↔provider
    // invariant, and the `default_frequency > 0` requirement are validated
    // downstream by the regseq-l2 clock tenant, not here (D14/D15). ---

    #[test]
    fn parses_clock_section_managed_and_static() {
        let input = r#"
version = "1"
[platform]
os = "linux"
[clock.uartclk]
mode = "managed"
default_frequency = 24000000
provider = "uartclk"
[clock.refclk]
mode = "static"
default_frequency = 48000000
"#;
        let b = parse_platform_bindings_str(input).expect("parse");
        let managed = b.clock.get("uartclk").expect("uartclk section");
        assert_eq!(managed.mode.as_deref(), Some("managed"));
        assert_eq!(managed.default_frequency, Some(24_000_000));
        assert_eq!(managed.provider.as_deref(), Some("uartclk"));
        let static_clk = b.clock.get("refclk").expect("refclk section");
        assert_eq!(static_clk.mode.as_deref(), Some("static"));
        assert_eq!(static_clk.default_frequency, Some(48_000_000));
        assert_eq!(static_clk.provider, None);
    }

    #[test]
    fn absent_clock_section_parses_to_absence() {
        let b = parse_platform_bindings_str("[mmio.regs]\nbase = 0\nsize = 16\n").unwrap();
        assert!(b.clock.is_empty());
    }

    #[test]
    fn clock_default_frequency_zero_parses_here_rejected_downstream() {
        // 0 is a valid non-negative u32 at parse time; the `> 0` requirement
        // (D15) is the L2 tenant's, not the parser's.
        let b = parse_platform_bindings_str("[clock.a]\nmode = \"static\"\ndefault_frequency = 0\n")
            .unwrap();
        assert_eq!(b.clock.get("a").unwrap().default_frequency, Some(0));
    }

    #[test]
    fn clock_unknown_keys_are_ignored() {
        // Forward-compat, matching the `[mmio.<id>]` / `[interrupt.<id>]`
        // discipline (no clock-specific unknown-key rule).
        let b = parse_platform_bindings_str(
            "[clock.a]\nmode = \"static\"\ndefault_frequency = 1\nfuture = 1\n",
        )
        .unwrap();
        assert_eq!(b.clock.get("a").unwrap().default_frequency, Some(1));
    }

    #[test]
    fn clock_rejects_non_integer_default_frequency() {
        let err =
            parse_platform_bindings_str("[clock.a]\ndefault_frequency = \"x\"\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadClock { id, key, .. }
                if id == "a" && key == "default_frequency"),
            "got {err:?}"
        );
    }

    #[test]
    fn clock_rejects_negative_and_oversized_default_frequency() {
        let err = parse_platform_bindings_str("[clock.a]\ndefault_frequency = -1\n").unwrap_err();
        assert!(matches!(&err, PlatformParseError::BadClock { .. }), "got {err:?}");
        let err =
            parse_platform_bindings_str("[clock.a]\ndefault_frequency = 4294967296\n").unwrap_err();
        assert!(matches!(&err, PlatformParseError::BadClock { .. }), "got {err:?}");
    }

    #[test]
    fn clock_rejects_non_string_mode_and_provider() {
        let err = parse_platform_bindings_str("[clock.a]\nmode = 1\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadClock { key, .. } if key == "mode"),
            "got {err:?}"
        );
        let err = parse_platform_bindings_str("[clock.a]\nprovider = 1\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadClock { key, .. } if key == "provider"),
            "got {err:?}"
        );
    }

    #[test]
    fn clock_error_is_labelled_clock_not_mmio() {
        let err = parse_platform_bindings_str("[clock.a]\nmode = 1\n").unwrap_err();
        assert!(err.to_string().starts_with("[clock.a]"), "got {err}");
    }

    #[test]
    fn duplicate_clock_section_is_a_toml_parse_error() {
        // A duplicate `[clock.<id>]` table is a free TOML parse error (D13/D15,
        // C06 N16) — not a clock-specific reject.
        let err =
            parse_platform_bindings_str("[clock.a]\nmode = \"static\"\n[clock.a]\nmode = \"static\"\n")
                .unwrap_err();
        assert!(matches!(&err, PlatformParseError::InvalidToml(_)), "got {err:?}");
    }

    // --- [pinctrl.<id>] (pin-controller-and-GPIO-configuration construct, C01):
    // SHAPE parse only (a present, array-of-strings `pin_states`). CONTENT
    // (non-empty, unique, non-empty names, "default" membership) is checked
    // downstream by the regseq-l2 pinctrl tenant (PIN-L2-*), not here. ---

    #[test]
    fn parses_pinctrl_section_well_formed() {
        let b = parse_platform_bindings_str(
            "[pinctrl.uart_pins]\npin_states = [\"default\", \"sleep\"]\n",
        )
        .unwrap();
        let spec = b.pinctrl.get("uart_pins").expect("uart_pins section");
        assert_eq!(spec.pin_states, vec!["default".to_string(), "sleep".to_string()]);
    }

    #[test]
    fn absent_pinctrl_section_parses_to_absence() {
        // The optional-absent case: no `[pinctrl.<id>]` section ⇒ absence in the
        // map (the L2 tenant binds it to `None` for an optional resource).
        let b = parse_platform_bindings_str("[mmio.regs]\nbase = 0\nsize = 16\n").unwrap();
        assert!(b.pinctrl.is_empty());
    }

    #[test]
    fn pinctrl_unknown_keys_are_ignored() {
        // Forward-compat, matching the `[clock.<id>]`/`[mmio.<id>]` discipline.
        let b = parse_platform_bindings_str(
            "[pinctrl.uart_pins]\npin_states = [\"default\"]\nfuture = 1\n",
        )
        .unwrap();
        assert_eq!(b.pinctrl.get("uart_pins").unwrap().pin_states, vec!["default".to_string()]);
    }

    #[test]
    fn pinctrl_rejects_missing_pin_states_key() {
        // A present section MUST carry `pin_states` (SHAPE reject, not PIN-L2).
        let err = parse_platform_bindings_str("[pinctrl.uart_pins]\nother = 1\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadPinctrl { id, key, .. }
                if id == "uart_pins" && key == "pin_states"),
            "got {err:?}"
        );
    }

    #[test]
    fn pinctrl_rejects_non_array_pin_states() {
        let err =
            parse_platform_bindings_str("[pinctrl.uart_pins]\npin_states = \"default\"\n")
                .unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadPinctrl { key, .. } if key == "pin_states"),
            "got {err:?}"
        );
    }

    #[test]
    fn pinctrl_rejects_non_string_entry() {
        // A non-string array entry is a SHAPE reject — a typed Vec<String>
        // cannot carry it to L2 (C01).
        let err =
            parse_platform_bindings_str("[pinctrl.uart_pins]\npin_states = [\"default\", 7]\n")
                .unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadPinctrl { key, .. } if key == "pin_states"),
            "got {err:?}"
        );
    }

    #[test]
    fn pinctrl_empty_array_parses_here_rejected_downstream() {
        // An empty `pin_states` array is SHAPE-valid (it IS an array of strings);
        // the non-empty CONTENT requirement is the L2 tenant's (PIN-L2-EMPTY-STATES).
        let b = parse_platform_bindings_str("[pinctrl.uart_pins]\npin_states = []\n").unwrap();
        assert!(b.pinctrl.get("uart_pins").unwrap().pin_states.is_empty());
    }

    #[test]
    fn pinctrl_error_is_labelled_pinctrl_not_mmio() {
        let err = parse_platform_bindings_str("[pinctrl.a]\npin_states = 1\n").unwrap_err();
        assert!(err.to_string().starts_with("[pinctrl.a]"), "got {err}");
    }

    #[test]
    fn duplicate_pinctrl_section_is_a_toml_parse_error() {
        // A duplicate `[pinctrl.<id>]` table is a free TOML parse error, not a
        // pinctrl-specific reject.
        let err = parse_platform_bindings_str(
            "[pinctrl.a]\npin_states = [\"default\"]\n[pinctrl.a]\npin_states = [\"default\"]\n",
        )
        .unwrap_err();
        assert!(matches!(&err, PlatformParseError::InvalidToml(_)), "got {err:?}");
    }

    // --- [platform] identity (T6b, R5): structural parse only; os vocabulary is
    // validated downstream by regseq-l2, not here. ---

    #[test]
    fn parse_file_captures_bindings_and_raw_identity_in_one_pass() {
        let input = r#"
[platform]
os = "linux"
board = "qemu-pc"
arch = "x86_64"
[mmio.regs]
bar = 0
base = 0xf0000000
size = 0x20000
"#;
        let file = parse_platform_file_str(input).expect("parse");
        // Bindings.
        assert_eq!(file.bindings.mmio.get("regs").unwrap().base, Some(0xf000_0000));
        // Raw identity (os kept verbatim as a string — NOT yet typed).
        assert_eq!(file.identity.os.as_deref(), Some("linux"));
        assert_eq!(file.identity.board.as_deref(), Some("qemu-pc"));
        assert_eq!(file.identity.arch.as_deref(), Some("x86_64"));
    }

    #[test]
    fn bindings_wrapper_is_byte_identical_to_file_bindings() {
        let input = "[platform]\nos = \"zephyr\"\n[mmio.regs]\nbar = 1\nbase = 0\nsize = 8\n";
        assert_eq!(
            parse_platform_bindings_str(input).unwrap(),
            parse_platform_file_str(input).unwrap().bindings,
        );
    }

    #[test]
    fn absent_platform_table_yields_all_none_identity() {
        let file = parse_platform_file_str("[mmio.regs]\nbar = 0\n").unwrap();
        assert_eq!(file.identity, PlatformIdentityRaw::default());
    }

    #[test]
    fn unknown_os_token_is_NOT_rejected_at_parse_time() {
        // R5: structural parse accepts any os string; the closed-vocabulary check
        // lives in regseq-l2::resolve_platform_identity.
        let file = parse_platform_file_str("[platform]\nos = \"plan9\"\n").unwrap();
        assert_eq!(file.identity.os.as_deref(), Some("plan9"));
    }

    #[test]
    fn rejects_non_string_platform_value() {
        let err = parse_platform_file_str("[platform]\nos = 42\n").unwrap_err();
        assert!(
            matches!(&err, PlatformParseError::BadPlatform { key, .. } if key == "os"),
            "got {err:?}"
        );
    }
}
