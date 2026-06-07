//! Item-driven `#[rseq::driver]` recognition + descriptor interning (T05/T07/T08).
//!
//! Mirrors `bridge::framework_type` (item/type-driven, not call-driven): walk
//! the interned struct Items, read the `#[rseq::driver(...)]` +
//! `#[rseq::device_class(...)]` tool attributes on each struct's source
//! definition, parse them into a [`DriverDescriptor`], and record a
//! [`DriverLifecycle`] into `model.device_lifecycle.drivers` keyed by the
//! driver-struct `ItemId`.
//!
//! Scope split: T05 parses the logical `name` and the devicetree
//! `compatible = [...]` match sugar and installs default `DriverHandle` /
//! `LifecycleOps`. **T07** extends the same parser with the PCI/AMBA record
//! sugar (`pci = [{...}]` / `amba = [{...}]`) and rejects the deferred
//! `acpi/usb/i2c/spi` families as a `BridgeError` (CR-5). **T08** reads the
//! separate `#[rseq::device_class(<tag>)]` attribute into a `ClassTag`
//! (unvalidated, D06/D09). The `impl Lifecycle` override-check (T09) and the
//! resource/user-data field split (T10) run in the `resolve` pass.

use regseq_model::{
    ClassTag, DriverDescriptor, DriverHandle, DriverLifecycle, ItemId, ItemKind, LifecycleOps,
    MatchEntry, RegseqModel, SyntheticItemKind,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::TyCtxt;
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, DeviceLifecycleDiagnostic};

/// Raw `#[rseq::driver(...)]` + `#[rseq::device_class(...)]` payload, parsed off
/// the tool attributes but not yet interned (strings are still owned `String`s;
/// interning needs `&mut model`).
struct ParsedDriver {
    name: String,
    match_set: Vec<ParsedMatch>,
    /// Whether a device-match attribute (`compatible`/`pci`/`amba`) was present
    /// at all — set once its bracket group is successfully consumed, even if the
    /// list is empty. Distinguishes D1 (no match attribute → `match_set` empty +
    /// `!match_attr_present`) from D2 (a match attribute present but empty, e.g.
    /// `compatible=[]` → `match_set` empty + `match_attr_present`). T17.
    match_attr_present: bool,
    device_class: ParsedClass,
}

/// Typed match sugar parsed from `#[rseq::driver(...)]`: devicetree (T05) and
/// the PCI/AMBA records (T07). The deferred `acpi/usb/i2c/spi` families are
/// rejected at parse time (CR-5), so they never reach this enum.
enum ParsedMatch {
    Devicetree {
        compatible: String,
    },
    Pci {
        vendor: u16,
        device: u16,
        subsystem: Option<u32>,
        class: Option<u32>,
    },
    Amba {
        id: u32,
        mask: u32,
    },
}

/// `#[rseq::device_class(<tag>)]` tag, text-only (interning `Other` needs
/// `&mut model`). Defaults to `Generic` for an absent attribute or any
/// shape other than a single identifier (unvalidated this stage, D06/D09).
enum ParsedClass {
    Generic,
    Serial,
    Network,
    Other(String),
}

/// Outcome of a failed `#[rseq::driver(...)]` parse.
enum DriverParseError {
    /// Malformed / incomplete attribute — the struct is silently dropped from
    /// the driver set. The missing/empty-match D1/D2 diagnostics are a later
    /// validation pass (T17), not a parse-time rejection.
    Drop,
    /// A deferred bus family appeared in the match sugar (CR-5) — a loud
    /// `BridgeError`, surfaced by the caller. Carries the offending token.
    UnsupportedBus(&'static str),
}

/// Read `#[rseq::driver(name = "...", <match sugar>)]` on `def_id`.
///
/// - `Ok(None)`  — no driver attribute (or a duplicate / non-parenthesized /
///   malformed one); the struct is not a recognized driver.
/// - `Ok(Some)`  — a well-formed descriptor payload (`device_class` defaulted
///   to `Generic`; the caller fills it from the separate attribute).
/// - `Err(_)`    — a deferred bus family in the match sugar (CR-5).
///
/// The payload is NOT a legacy meta-item list: `compatible`/`pci`/`amba` carry
/// bracketed array / record values that `Attribute::meta_item_list()` cannot
/// parse (it would silently return `None` and drop the driver), so we walk the
/// RAW token stream of the `Unparsed` tool attribute.
fn recognise_driver_attr(
    tcx: TyCtxt<'_>,
    def_id: DefId,
) -> Result<Option<ParsedDriver>, BridgeError> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("driver")];
    let mut attrs = tcx.get_attrs_by_path(def_id, &path);
    let Some(attr) = attrs.next() else {
        return Ok(None);
    };
    if attrs.next().is_some() {
        // More than one `#[rseq::driver]` on the same struct is ill-formed.
        return Ok(None);
    }
    // A tool attribute lowers to `Attribute::Unparsed`; its args must be the
    // parenthesized list `#[rseq::driver(...)]`.
    let Attribute::Unparsed(item) = attr else {
        return Ok(None);
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return Ok(None);
    };
    if delim.delim != Delimiter::Parenthesis {
        return Ok(None);
    }
    match parse_driver_args(&delim.tokens) {
        Ok(parsed) => {
            // T17: a well-formed driver with no device-match is invalid — D1 (no
            // match attribute at all) / D2 (a match attribute present but empty,
            // e.g. `compatible=[]`). A MALFORMED match is a silent Drop (handled
            // above as `DriverParseError::Drop`), NOT a D1/D2 rejection.
            if parsed.match_set.is_empty() {
                let diag = if parsed.match_attr_present {
                    DeviceLifecycleDiagnostic::EmptyMatch {
                        driver: parsed.name,
                    }
                } else {
                    DeviceLifecycleDiagnostic::MissingMatch {
                        driver: parsed.name,
                    }
                };
                return Err(BridgeError::DeviceLifecycleValidation(diag));
            }
            Ok(Some(parsed))
        }
        Err(DriverParseError::Drop) => Ok(None),
        Err(DriverParseError::UnsupportedBus(bus)) => {
            Err(BridgeError::DeviceLifecycleUnsupportedBus {
                bus: bus.to_string(),
            })
        }
    }
}

/// Walk the comma-separated `key = value` token stream inside
/// `#[rseq::driver(...)]`. Recognized keys: `name = "<str>"`,
/// `compatible = ["<str>", ...]` (devicetree), `pci = [{ vendor, device,
/// [subsystem], [class] }, ...]`, and `amba = [{ id, mask }, ...]`. The deferred
/// `acpi/usb/i2c/spi` families are a hard [`DriverParseError::UnsupportedBus`]
/// (CR-5). Any other key's single value token tree is skipped (forward-compat).
/// Returns [`DriverParseError::Drop`] on a malformed entry or a missing `name`.
fn parse_driver_args(tokens: &TokenStream) -> Result<ParsedDriver, DriverParseError> {
    let mut name: Option<String> = None;
    let mut match_set: Vec<ParsedMatch> = Vec::new();
    // Set once a match attribute's bracket group is consumed (T17 D1-vs-D2).
    let mut match_attr_present = false;
    let mut iter = tokens.iter().peekable();

    while iter.peek().is_some() {
        // key identifier
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return Err(DriverParseError::Drop);
        };
        let key = key_tok
            .ident()
            .map(|(ident, _)| ident.name)
            .ok_or(DriverParseError::Drop)?;
        // `=`
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => return Err(DriverParseError::Drop),
        }
        // value by key
        match key.as_str() {
            "name" => {
                name = Some(expect_str_lit(iter.next()).ok_or(DriverParseError::Drop)?);
            }
            "compatible" => {
                let Some(TokenTree::Delimited(.., Delimiter::Bracket, inner)) = iter.next() else {
                    return Err(DriverParseError::Drop);
                };
                match_attr_present = true;
                let mut elems = inner.iter().peekable();
                while elems.peek().is_some() {
                    let compatible = expect_str_lit(elems.next()).ok_or(DriverParseError::Drop)?;
                    match_set.push(ParsedMatch::Devicetree { compatible });
                    expect_separator(&mut elems)?;
                }
            }
            "pci" => {
                let Some(TokenTree::Delimited(.., Delimiter::Bracket, inner)) = iter.next() else {
                    return Err(DriverParseError::Drop);
                };
                match_attr_present = true;
                for record in record_groups(inner)? {
                    match_set.push(parse_pci_record(record)?);
                }
            }
            "amba" => {
                let Some(TokenTree::Delimited(.., Delimiter::Bracket, inner)) = iter.next() else {
                    return Err(DriverParseError::Drop);
                };
                match_attr_present = true;
                for record in record_groups(inner)? {
                    match_set.push(parse_amba_record(record)?);
                }
            }
            // Deferred bus families (CR-5): a loud rejection, not a silent skip.
            "acpi" => return Err(DriverParseError::UnsupportedBus("acpi")),
            "usb" => return Err(DriverParseError::UnsupportedBus("usb")),
            "i2c" => return Err(DriverParseError::UnsupportedBus("i2c")),
            "spi" => return Err(DriverParseError::UnsupportedBus("spi")),
            // Unknown key (a typo, or a future-construct attr that snuck into
            // the list): skip its single value token tree so the comma-walk
            // stays in sync. `device_class` is its own attribute, not a key here.
            // The value token must exist (`foo =` with nothing after is malformed).
            _ => {
                if iter.next().is_none() {
                    return Err(DriverParseError::Drop);
                }
            }
        }
        // Require a separating comma between top-level entries (or end-of-list).
        // A missing comma (`name = "x" compatible = [...]`) is malformed → Drop.
        expect_separator(&mut iter)?;
    }

    Ok(ParsedDriver {
        name: name.ok_or(DriverParseError::Drop)?,
        match_set,
        match_attr_present,
        device_class: ParsedClass::Generic,
    })
}

/// After a list element, require either a separating comma (consumed) or the
/// end of the list. Any other token between elements is malformed
/// ([`DriverParseError::Drop`]). This is what makes the parse *strict* about
/// separators (so `a b`, `[{..}{..}]`, `{x=1 y=2}` are rejected, not silently
/// merged) while still allowing an empty list and a single trailing comma.
fn expect_separator<'a, I: Iterator<Item = &'a TokenTree>>(
    iter: &mut std::iter::Peekable<I>,
) -> Result<(), DriverParseError> {
    match iter.peek() {
        None => Ok(()),
        Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
            iter.next();
            Ok(())
        }
        _ => Err(DriverParseError::Drop),
    }
}

/// Extract the `{...}` record bodies from a `[ {..}, {..} ]` bracket inner token
/// stream, skipping the separator commas. A token that is neither a comma nor a
/// brace group is a malformed record list ([`DriverParseError::Drop`]). An empty
/// list (`pci = []`) yields no records (an empty match, deferred to D2).
fn record_groups(inner: &TokenStream) -> Result<Vec<&TokenStream>, DriverParseError> {
    let mut records = Vec::new();
    let mut iter = inner.iter().peekable();
    while iter.peek().is_some() {
        match iter.next() {
            Some(TokenTree::Delimited(.., Delimiter::Brace, body)) => records.push(body),
            _ => return Err(DriverParseError::Drop),
        }
        // Require a comma between records (`[{..}{..}]` without one is malformed).
        expect_separator(&mut iter)?;
    }
    Ok(records)
}

/// Parse a `{ key = <int>, ... }` record body into its `(key, value)` pairs
/// (values widened to `u64`; range-checked by the per-bus caller). Malformed
/// (missing `=`, non-integer value, dangling key) → [`DriverParseError::Drop`].
fn parse_int_record(tokens: &TokenStream) -> Result<Vec<(RustcSymbol, u64)>, DriverParseError> {
    let mut pairs = Vec::new();
    let mut iter = tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return Err(DriverParseError::Drop);
        };
        let key = key_tok
            .ident()
            .map(|(ident, _)| ident.name)
            .ok_or(DriverParseError::Drop)?;
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => return Err(DriverParseError::Drop),
        }
        let value = expect_int_lit(iter.next()).ok_or(DriverParseError::Drop)?;
        pairs.push((key, value));
        // Require a comma between record fields (`{ x = 1 y = 2 }` is malformed).
        expect_separator(&mut iter)?;
    }
    Ok(pairs)
}

/// Parse a `pci = [{ ... }]` record into [`ParsedMatch::Pci`]. `vendor`/`device`
/// are required `u16`; `subsystem`/`class` are optional `u32`. An out-of-range
/// value or an unknown key → [`DriverParseError::Drop`].
fn parse_pci_record(tokens: &TokenStream) -> Result<ParsedMatch, DriverParseError> {
    let mut vendor: Option<u16> = None;
    let mut device: Option<u16> = None;
    let mut subsystem: Option<u32> = None;
    let mut class: Option<u32> = None;
    for (key, value) in parse_int_record(tokens)? {
        match key.as_str() {
            "vendor" => vendor = Some(u16::try_from(value).map_err(|_| DriverParseError::Drop)?),
            "device" => device = Some(u16::try_from(value).map_err(|_| DriverParseError::Drop)?),
            "subsystem" => {
                subsystem = Some(u32::try_from(value).map_err(|_| DriverParseError::Drop)?)
            }
            "class" => class = Some(u32::try_from(value).map_err(|_| DriverParseError::Drop)?),
            _ => return Err(DriverParseError::Drop),
        }
    }
    Ok(ParsedMatch::Pci {
        vendor: vendor.ok_or(DriverParseError::Drop)?,
        device: device.ok_or(DriverParseError::Drop)?,
        subsystem,
        class,
    })
}

/// Parse an `amba = [{ id, mask }]` record into [`ParsedMatch::Amba`]. Both
/// fields are required `u32`. Out-of-range or unknown key → `Drop`.
fn parse_amba_record(tokens: &TokenStream) -> Result<ParsedMatch, DriverParseError> {
    let mut id: Option<u32> = None;
    let mut mask: Option<u32> = None;
    for (key, value) in parse_int_record(tokens)? {
        match key.as_str() {
            "id" => id = Some(u32::try_from(value).map_err(|_| DriverParseError::Drop)?),
            "mask" => mask = Some(u32::try_from(value).map_err(|_| DriverParseError::Drop)?),
            _ => return Err(DriverParseError::Drop),
        }
    }
    Ok(ParsedMatch::Amba {
        id: id.ok_or(DriverParseError::Drop)?,
        mask: mask.ok_or(DriverParseError::Drop)?,
    })
}

/// Extract the content of a string-literal token tree (`"acme,fake"` →
/// `acme,fake`). Accepts both normal (`"..."`) and raw (`r"..."`/`r#"..."#`)
/// string literals — `Lit.symbol` is the literal content for both (escape-free
/// for the names/compatibles this parses). Returns `None` for any other token.
fn expect_str_lit(tt: Option<&TokenTree>) -> Option<String> {
    let TokenTree::Token(tok, _) = tt? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Str | TokenLitKind::StrRaw(_)) {
        return None;
    }
    Some(lit.symbol.as_str().to_string())
}

/// Extract an unsigned integer-literal token's value (`0x8086`, `4096`,
/// `0b1010`). A type suffix (`0x8086u16`) is carried on `Lit.suffix`, not
/// `Lit.symbol`, so it does not interfere. Returns `None` for any non-integer
/// token or a value that does not parse as `u64`.
fn expect_int_lit(tt: Option<&TokenTree>) -> Option<u64> {
    let TokenTree::Token(tok, _) = tt? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Integer) {
        return None;
    }
    parse_int_symbol(lit.symbol.as_str())
}

/// Parse a Rust integer-literal spelling (optional `0x`/`0o`/`0b` base prefix +
/// `_` digit separators) to `u64`.
fn parse_int_symbol(symbol: &str) -> Option<u64> {
    let digits = symbol.replace('_', "");
    let (radix, body) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16u32, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        (8, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        (2, rest)
    } else {
        (10, digits.as_str())
    };
    u64::from_str_radix(body, radix).ok()
}

/// Read `#[rseq::device_class(<tag>)]` on `def_id` into a [`ParsedClass`] (T08).
/// Absent (or any shape other than `(<ident>)`) → `Generic` (unvalidated,
/// D06/D09); `generic`/`serial`/`network` map to their tags, any other
/// identifier is an open-vocabulary `Other`. A duplicate attribute takes the
/// first; the tag is stored verbatim, validated nowhere this stage.
fn recognise_device_class_attr(tcx: TyCtxt<'_>, def_id: DefId) -> ParsedClass {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("device_class"),
    ];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return ParsedClass::Generic;
    };
    let Attribute::Unparsed(item) = attr else {
        return ParsedClass::Generic;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return ParsedClass::Generic;
    };
    if delim.delim != Delimiter::Parenthesis {
        return ParsedClass::Generic;
    }
    parse_device_class(&delim.tokens)
}

/// Map the single identifier inside `#[rseq::device_class(<tag>)]`. A
/// non-identifier, empty, or multi-token argument list → `Generic` (the tag is
/// a *bare* identifier; `(serial, network)` is malformed, not `Serial`).
fn parse_device_class(tokens: &TokenStream) -> ParsedClass {
    let mut iter = tokens.iter();
    let Some(TokenTree::Token(tok, _)) = iter.next() else {
        return ParsedClass::Generic;
    };
    let Some((ident, _)) = tok.ident() else {
        return ParsedClass::Generic;
    };
    // Exactly one identifier token: anything trailing is malformed → Generic.
    if iter.next().is_some() {
        return ParsedClass::Generic;
    }
    match ident.name.as_str() {
        "generic" => ParsedClass::Generic,
        "serial" => ParsedClass::Serial,
        "network" => ParsedClass::Network,
        other => ParsedClass::Other(other.to_string()),
    }
}

/// Item-driven materialization pass: walk every struct Item, read its
/// `#[rseq::driver(...)]` + `#[rseq::device_class(...)]` tool attributes, and
/// intern a [`DriverLifecycle`] into `model.device_lifecycle.drivers` keyed by
/// the struct's `ItemId`.
///
/// Mirrors `framework_type::materialise_framework_types`: synthetic Items
/// (reserved `StableCrateId(0)`, which panics inside `def_path_hash_to_def_id`)
/// and non-source (dependency-crate) Items are filtered before recovery — a
/// driver is identified purely by the attribute on its source definition in the
/// crate under compilation. Idempotent: an already-recorded driver is skipped.
/// Returns a [`BridgeError`] if a driver's match sugar names a deferred bus
/// family (CR-5).
pub(crate) fn materialise_drivers(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    // 1. Find the local, non-synthetic struct Items (immutable borrow of model).
    let candidates: Vec<(ItemId, DefId)> = cx
        .model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let item_id = ItemId::from_u32(idx as u32);
            if !matches!(item.kind, ItemKind::Struct) {
                return None;
            }
            if !matches!(
                cx.model.item_synthetic_kinds[item_id],
                SyntheticItemKind::None
            ) {
                return None;
            }
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            if !def_id.is_local() {
                return None;
            }
            Some((item_id, def_id))
        })
        .collect();

    // 2. Parse each candidate's driver + device_class attrs (fallible — CR-5),
    //    then intern (mutable borrow of model).
    for (item_id, def_id) in candidates {
        if cx.model.device_lifecycle.drivers.contains_key(&item_id) {
            continue;
        }
        let Some(mut parsed) = recognise_driver_attr(tcx, def_id)? else {
            continue;
        };
        parsed.device_class = recognise_device_class_attr(tcx, def_id);
        let descriptor = build_descriptor(&mut cx.model, parsed);
        cx.model.device_lifecycle.drivers.insert(
            item_id,
            DriverLifecycle {
                descriptor,
                handle: DriverHandle::default(),
                ops: LifecycleOps::default(),
            },
        );
    }
    Ok(())
}

/// Intern a [`ParsedDriver`]'s strings into the model and build its
/// [`DriverDescriptor`] (name, typed match set, device class).
fn build_descriptor(model: &mut RegseqModel, parsed: ParsedDriver) -> DriverDescriptor {
    let name = model.strings.intern(&parsed.name);
    let match_set = parsed
        .match_set
        .into_iter()
        .map(|m| match m {
            ParsedMatch::Devicetree { compatible } => MatchEntry::Devicetree {
                compatible: model.strings.intern(&compatible),
            },
            ParsedMatch::Pci {
                vendor,
                device,
                subsystem,
                class,
            } => MatchEntry::Pci {
                vendor,
                device,
                subsystem,
                class,
            },
            ParsedMatch::Amba { id, mask } => MatchEntry::Amba { id, mask },
        })
        .collect();
    let device_class = match parsed.device_class {
        ParsedClass::Generic => ClassTag::Generic,
        ParsedClass::Serial => ClassTag::Serial,
        ParsedClass::Network => ClassTag::Network,
        ParsedClass::Other(tag) => ClassTag::Other(model.strings.intern(&tag)),
    };
    DriverDescriptor {
        name,
        match_set,
        device_class,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::StableCrateId;

    #[test]
    fn build_descriptor_interns_name_and_dt_match() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let desc = build_descriptor(
            &mut model,
            ParsedDriver {
                name: "fake".to_string(),
                match_set: vec![ParsedMatch::Devicetree {
                    compatible: "acme,fake".to_string(),
                }],
                match_attr_present: true,
                device_class: ParsedClass::Generic,
            },
        );
        assert_eq!(model.strings.resolve(desc.name), "fake");
        assert_eq!(desc.match_set.len(), 1);
        match &desc.match_set[0] {
            MatchEntry::Devicetree { compatible } => {
                assert_eq!(model.strings.resolve(*compatible), "acme,fake");
            }
            other => panic!("expected a devicetree match, got {other:?}"),
        }
        assert_eq!(desc.device_class, ClassTag::Generic);
    }

    #[test]
    fn build_descriptor_preserves_multiple_compatibles_in_order() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let desc = build_descriptor(
            &mut model,
            ParsedDriver {
                name: "multi".to_string(),
                match_set: vec![
                    ParsedMatch::Devicetree {
                        compatible: "acme,one".to_string(),
                    },
                    ParsedMatch::Devicetree {
                        compatible: "acme,two".to_string(),
                    },
                ],
                match_attr_present: true,
                device_class: ParsedClass::Generic,
            },
        );
        let got: Vec<&str> = desc
            .match_set
            .iter()
            .map(|m| match m {
                MatchEntry::Devicetree { compatible } => model.strings.resolve(*compatible),
                other => panic!("expected devicetree, got {other:?}"),
            })
            .collect();
        assert_eq!(got, vec!["acme,one", "acme,two"]);
    }

    #[test]
    fn build_descriptor_allows_empty_match_set() {
        // A descriptor with no match entries is well-formed at the IR level;
        // the missing/empty-match diagnostic (D1/D2) is a later validation pass
        // (T17), not a parse-time rejection.
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let desc = build_descriptor(
            &mut model,
            ParsedDriver {
                name: "bare".to_string(),
                match_set: vec![],
                match_attr_present: false,
                device_class: ParsedClass::Generic,
            },
        );
        assert_eq!(model.strings.resolve(desc.name), "bare");
        assert!(desc.match_set.is_empty());
    }

    #[test]
    fn build_descriptor_maps_pci_and_amba_and_class() {
        // T07/T08 at the descriptor level: the typed PCI/AMBA records and the
        // device-class tag map straight through (no interning for the numeric
        // fields; `Other` interns its tag).
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let desc = build_descriptor(
            &mut model,
            ParsedDriver {
                name: "e1000".to_string(),
                match_set: vec![
                    ParsedMatch::Pci {
                        vendor: 0x8086,
                        device: 0x100e,
                        subsystem: Some(0x1234_5678),
                        class: None,
                    },
                    ParsedMatch::Amba {
                        id: 0x0004_1011,
                        mask: 0x000f_ffff,
                    },
                ],
                match_attr_present: true,
                device_class: ParsedClass::Network,
            },
        );
        assert_eq!(desc.device_class, ClassTag::Network);
        assert_eq!(desc.match_set.len(), 2);
        assert!(matches!(
            desc.match_set[0],
            MatchEntry::Pci {
                vendor: 0x8086,
                device: 0x100e,
                subsystem: Some(0x1234_5678),
                class: None,
            }
        ));
        assert!(matches!(
            desc.match_set[1],
            MatchEntry::Amba {
                id: 0x0004_1011,
                mask: 0x000f_ffff,
            }
        ));
    }

    #[test]
    fn build_descriptor_interns_other_class_tag_verbatim() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let desc = build_descriptor(
            &mut model,
            ParsedDriver {
                name: "blk".to_string(),
                match_set: vec![],
                match_attr_present: false,
                device_class: ParsedClass::Other("block".to_string()),
            },
        );
        match desc.device_class {
            ClassTag::Other(tag) => assert_eq!(model.strings.resolve(tag), "block"),
            other => panic!("expected Other(block), got {other:?}"),
        }
    }

    #[test]
    fn parse_int_symbol_handles_bases_and_separators() {
        assert_eq!(parse_int_symbol("0x8086"), Some(0x8086));
        assert_eq!(parse_int_symbol("0X100E"), Some(0x100e));
        assert_eq!(parse_int_symbol("4096"), Some(4096));
        assert_eq!(parse_int_symbol("0b1010"), Some(0b1010));
        assert_eq!(parse_int_symbol("0o17"), Some(0o17));
        assert_eq!(parse_int_symbol("0x000f_ffff"), Some(0x000f_ffff));
        assert_eq!(parse_int_symbol("0xZZ"), None);
    }
}
