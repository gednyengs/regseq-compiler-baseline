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
//! Two sections are parsed (in one pass — [`parse_platform_file_str`]): the
//! `[mmio.<id>]` bindings above, and the `[platform]` table into a
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

/// The L2 bindings parsed from a platform file: the `[mmio.<id>]` sections keyed
/// by resource id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformBindings {
    /// `[mmio.<id>]` sections, keyed by the resource id (`#[rseq::mmio(id =
    /// "<id>")]`).
    pub mmio: BTreeMap<String, MmioBindingSpec>,
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

    let identity = parse_platform_identity(&table)?;

    Ok(PlatformFile {
        bindings: PlatformBindings { mmio },
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
