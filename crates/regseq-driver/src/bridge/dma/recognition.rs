//! Phase-4 `#[rseq::dma_region(...)]` field recognition + Phase-5 coherent-region
//! call-site recognition (dma-management construct, M1 region path).
//!
//! Three passes, mirroring the clock/pinctrl field-recognition precedent
//! (`bridge::clock::recognition`) and the atomic body-walk precedent
//! (`bridge::atomic::validate_atomic_ops`):
//!
//!   * [`materialise_dma_regions`] (Phase 4, after
//!     `device_lifecycle::materialise_drivers`) — walk the interned
//!     `#[rseq::driver]` structs, classify each `DmaRegion<T>` field, parse the
//!     mandatory `#[rseq::dma_region(id, element_size, element_count,
//!     alignment?)]` keys (default `alignment = align_of::<T>()` is recovered by a
//!     later layout pass, GRILL C03/D08), capture the element type `T`, intern the
//!     per-driver [`DmaRegionFacts`] into `model.dma.drivers` plus the
//!     [`AttrData::DmaRegionResource`] marker into `domain_attrs.field_attrs`.
//!     Sets `dma_present = true` on a bus-master (PCI) driver that declares a
//!     coherent region (E1000-style, GRILL T05), never on a slave channel.
//!
//!   * [`recognise_dma_region_sites`] (Phase 5, after body materialisation) —
//!     walk every materialized body and, for each `Call` whose callee is a
//!     `IntrinsicKind::Dma` region/A2 op, recover the `DmaRegion` receiver field
//!     (the receiver-RESOURCE-TYPE disambiguation that splits the generic
//!     `read`/`write`/`update` verbs from `Mmio<T>`'s — GRILL T06/T08), and append
//!     the per-`element_access`-site [`ElementAccessFact`] (verb + view type `V` +
//!     index + lexical extent) onto that region's facts.
//!
//! `DmaRegion` is detected by type name, matching the name-based `Mmio` / `Clock`
//! / `PinCtrl` anchors. M1 builds only the coherent-region surface; the slave
//! `DmaChannel` + streaming recognition is M2 (T07).

use regseq_model::{
    AccessEffects, AttrData, Body, CallCallee, CompletionBinding, DmaChannelFacts, DmaFlow,
    DmaLayoutFacts, DmaOp, DmaRegionFacts, DmaRegionSpan, DmaStreamBuffer, DmaStreamFacts,
    ElementAccessFact, FieldIdx, FieldPath, GenericArg, InstanceKind, IntrinsicKind, ItemId,
    ItemKind, LexicalExtent, Operand, Place, PlaceElem, RegseqModel, SpanAddr, StringId,
    Ty as ModelTy, TerminatorKind, TyId, VariantIdx,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, DmaDiagnostic};

/// The recognized declaration form of a `DmaRegion` field. M1 builds only the
/// required `DmaRegion<T>` form; the optional/array/ref forms are recognized
/// whole and rejected (mirrors clock's `ClockForm`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DmaRegionForm {
    /// `DmaRegion<T>` — the required form (the only one M1 builds).
    Required,
    /// `Option<DmaRegion<T>>` — the optional form (designed-in, not built).
    Optional,
    /// `[DmaRegion<T>; N]` — the array form (designed-in, not built).
    Array,
    /// `&DmaRegion<T>` / `&mut DmaRegion<T>` — a reference form (not a decl).
    Ref,
}

impl DmaRegionForm {
    /// The author-facing spelling for the unsupported-form diagnostic.
    fn label(self) -> &'static str {
        match self {
            DmaRegionForm::Required => "DmaRegion<T>",
            DmaRegionForm::Optional => "Option<DmaRegion<T>>",
            DmaRegionForm::Array => "[DmaRegion<T>; N]",
            DmaRegionForm::Ref => "&DmaRegion<T>",
        }
    }
}

/// The parsed `#[rseq::dma_region(...)]` attribute. `id`/`element_size`/
/// `element_count` are mandatory; `alignment` is optional (omitted ⇒
/// `align_of::<T>()`, recovered later, GRILL C03/D08).
struct DmaRegionAttr {
    id: String,
    element_size: u64,
    element_count: u64,
    alignment: Option<u64>,
}

/// The outcome of parsing the `#[rseq::dma_region(...)]` attribute.
enum DmaRegionAttrResult {
    /// A complete, well-formed attribute.
    Ok(DmaRegionAttr),
    /// No `#[rseq::dma_region]` attribute at all.
    Absent,
    /// A mandatory key (`id`/`element_size`/`element_count`) is missing.
    MissingKey(&'static str),
    /// The attribute is structurally bad (empty id, non-literal value, duplicate
    /// or unknown key, malformed tokens).
    Malformed(String),
}

/// The `DmaRegion<T>` element type `T`, if `ty` is the typed `DmaRegion` shell.
/// Name-based detection, matching `is_mmio_resource` / `is_clock_ty`.
fn dma_region_element_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(adt, args) = ty.kind() else {
        return None;
    };
    if tcx.item_name(adt.did()).as_str() != "DmaRegion" {
        return None;
    }
    args.types().next()
}

/// Is `ty` the typed `DmaRegion<T>` resource shell (any `T`)?
fn is_dma_region_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    dma_region_element_ty(tcx, ty).is_some()
}

/// Compute the resolved ABI layout facts (size/align + conservative M1 POD
/// marker) for an element type `T` (dma-management FIX #2). Returns `None` when
/// `T`'s layout is unresolvable (the caller then SKIPs the declaration guard and
/// leaves `element_layout = None` rather than crashing). The POD marker is the
/// conservative `!T.needs_drop` proxy: a type with no Drop glue is safe to hold
/// in a memcpy'd coherent backing store (the full descriptor-POD verify is M4).
fn dma_layout_facts<'tcx>(tcx: TyCtxt<'tcx>, t: Ty<'tcx>) -> Option<DmaLayoutFacts> {
    let typing_env = rustc_middle::ty::TypingEnv::fully_monomorphized();
    let layout = tcx.layout_of(typing_env.as_query_input(t)).ok()?;
    let is_pod = !t.needs_drop(tcx, typing_env);
    Some(DmaLayoutFacts {
        size: layout.size.bytes(),
        align: layout.align.abi.bytes(),
        is_pod,
    })
}

/// Classify a driver-struct field type into its `DmaRegion` declaration form, or
/// `None` when the field is not a DMA region at all. All recognized forms are
/// returned whole; the non-`Required` forms are rejected by
/// [`materialise_dma_regions`], never silently narrowed here (mirrors clock).
fn classify_dma_region_form<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<DmaRegionForm> {
    if is_dma_region_ty(tcx, ty) {
        return Some(DmaRegionForm::Required);
    }
    match ty.kind() {
        TyKind::Adt(adt, args)
            if tcx.get_diagnostic_name(adt.did()) == Some(rustc_span::sym::Option) =>
        {
            let inner = args.types().next()?;
            is_dma_region_ty(tcx, inner).then_some(DmaRegionForm::Optional)
        }
        TyKind::Array(elem, _len) if is_dma_region_ty(tcx, *elem) => Some(DmaRegionForm::Array),
        TyKind::Ref(_, inner, _) if is_dma_region_ty(tcx, *inner) => Some(DmaRegionForm::Ref),
        _ => None,
    }
}

/// Whether `ty` is any recognized `DmaRegion` declaration form. Used by the
/// device-lifecycle field split to route DMA-region fields away from the
/// user-data classifier (whose D3 would otherwise reject the methodless
/// `DmaRegion` shell), mirroring the `Mmio`/`Clock`/`PinCtrl` routing.
pub(crate) fn is_dma_region_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    classify_dma_region_form(tcx, ty).is_some()
}

/// Whether `def_id` carries a `#[rseq::dma_region(...)]` attribute (used for the
/// attr-on-non-region check).
fn has_dma_region_attr(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("dma_region")];
    let present = tcx.get_attrs_by_path(def_id, &path).next().is_some();
    present
}

/// Parse + validate the `#[rseq::dma_region(...)]` attribute on `def_id`. The
/// admitted keys are `id = "…"` (string, mandatory), `element_size = N` /
/// `element_count = N` (unsigned int, mandatory), and `alignment = N` (unsigned
/// int, optional). Unknown/duplicate keys are rejected (mirrors clock's strict
/// key discipline, GRILL C03).
fn parse_dma_region_attr(tcx: TyCtxt<'_>, def_id: DefId) -> DmaRegionAttrResult {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("dma_region")];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return DmaRegionAttrResult::Absent;
    };
    let Attribute::Unparsed(item) = attr else {
        return DmaRegionAttrResult::Absent;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return DmaRegionAttrResult::Absent;
    };
    if delim.delim != Delimiter::Parenthesis {
        return DmaRegionAttrResult::Absent;
    }
    let mut id: Option<String> = None;
    let mut element_size: Option<u64> = None;
    let mut element_count: Option<u64> = None;
    let mut alignment: Option<u64> = None;
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return DmaRegionAttrResult::Malformed("malformed attribute tokens".to_string());
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            return DmaRegionAttrResult::Malformed("expected a `key = …` pair".to_string());
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => {
                return DmaRegionAttrResult::Malformed(format!(
                    "key `{key}` is missing its `= …` value"
                ))
            }
        }
        let value = iter.next();
        let key_str = key.as_str();
        match key_str {
            "id" => {
                if id.is_some() {
                    return DmaRegionAttrResult::Malformed("duplicate `id` key".to_string());
                }
                match expect_str_lit(value) {
                    Some(s) => id = Some(s),
                    None => {
                        return DmaRegionAttrResult::Malformed(
                            "`id` must be a string literal".to_string(),
                        )
                    }
                }
            }
            "element_size" | "element_count" | "alignment" => {
                let slot = match key_str {
                    "element_size" => &mut element_size,
                    "element_count" => &mut element_count,
                    _ => &mut alignment,
                };
                if slot.is_some() {
                    return DmaRegionAttrResult::Malformed(format!("duplicate `{key_str}` key"));
                }
                match expect_uint_lit(value) {
                    Some(n) => *slot = Some(n),
                    None => {
                        return DmaRegionAttrResult::Malformed(format!(
                            "`{key_str}` must be a non-negative integer literal"
                        ))
                    }
                }
            }
            other => {
                return DmaRegionAttrResult::Malformed(format!("unknown key `{other}`"));
            }
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => {
                return DmaRegionAttrResult::Malformed(
                    "expected `,` between attribute keys".to_string(),
                )
            }
        }
    }
    let id = match id {
        Some(s) if !s.trim().is_empty() => s,
        Some(_) => {
            return DmaRegionAttrResult::Malformed("`id` is empty or whitespace".to_string())
        }
        None => return DmaRegionAttrResult::MissingKey("id"),
    };
    let Some(element_size) = element_size else {
        return DmaRegionAttrResult::MissingKey("element_size");
    };
    let Some(element_count) = element_count else {
        return DmaRegionAttrResult::MissingKey("element_count");
    };
    DmaRegionAttrResult::Ok(DmaRegionAttr {
        id,
        element_size,
        element_count,
        alignment,
    })
}

/// Extract a string literal from a token, or `None` (mirrors
/// `clock::recognition::expect_str_lit`).
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

/// Extract a non-negative integer literal from a token, or `None`. Underscores
/// in the literal (`element_count = 1_024`) are stripped before parsing.
fn expect_uint_lit(tt: Option<&TokenTree>) -> Option<u64> {
    let TokenTree::Token(tok, _) = tt? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Integer) {
        return None;
    }
    let raw = lit.symbol.as_str().replace('_', "");
    // Accept hex/octal/binary prefixes as well as plain decimal.
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = raw.strip_prefix("0o").or_else(|| raw.strip_prefix("0O")) {
        u64::from_str_radix(oct, 8).ok()
    } else if let Some(bin) = raw.strip_prefix("0b").or_else(|| raw.strip_prefix("0B")) {
        u64::from_str_radix(bin, 2).ok()
    } else {
        raw.parse::<u64>().ok()
    }
}

// ---------------------------------------------------------------------------
// Slave `DmaChannel` field recognition (M2/T07). Mirrors the region path: a
// name-based shell detector, a strict `#[rseq::dma_channel(id, direction)]` attr
// parser (both keys mandatory; missing/malformed `direction` ⇒ reject), and the
// interned `DmaChannelFacts` + `AttrData::DmaChannelResource` field marker.
// ---------------------------------------------------------------------------

/// Is `ty` the `DmaChannel` slave-channel resource shell? Name-based detection,
/// matching `is_dma_region_ty` / `is_clock_ty`. `DmaChannel` is a non-generic
/// shell (the direction rides the attribute, not a type parameter).
fn is_dma_channel_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Adt(adt, _args) = ty.kind() else {
        return false;
    };
    tcx.item_name(adt.did()).as_str() == "DmaChannel"
}

/// Whether `ty` is a `DmaChannel` field (the required form only; M2 does not
/// build the optional/array/ref channel forms). Used by the device-lifecycle
/// field split to route channel fields away from the user-data classifier (whose
/// D3 would otherwise reject the methodless `DmaChannel` shell), mirroring the
/// `DmaRegion` routing.
pub(crate) fn is_dma_channel_field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    is_dma_channel_ty(tcx, ty)
}

/// Whether `def_id` carries a `#[rseq::dma_channel(...)]` attribute.
fn has_dma_channel_attr(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("dma_channel")];
    let present = tcx.get_attrs_by_path(def_id, &path).next().is_some();
    present
}

/// The parsed `#[rseq::dma_channel(id, direction)]` attribute. Both keys are
/// mandatory (GRILL S02): `id` is the `[dma.<id>]` join key, `direction` fixes
/// the source/destination role of a transfer endpoint.
struct DmaChannelAttr {
    id: String,
    direction: DmaFlow,
}

/// The outcome of parsing the `#[rseq::dma_channel(...)]` attribute.
enum DmaChannelAttrResult {
    /// A complete, well-formed attribute.
    Ok(DmaChannelAttr),
    /// No `#[rseq::dma_channel]` attribute at all.
    Absent,
    /// A mandatory key (`id`/`direction`) is missing, or `direction` is an
    /// unknown token, or the attribute is structurally bad.
    Bad(String),
}

/// Map a `direction = "…"` token to a [`DmaFlow`]. The OS-agnostic spellings
/// (GRILL C08/D05): `device_to_host` / `host_to_device`. `host_to_host` is
/// reserved-but-not-built (rejected here).
fn parse_dma_flow(token: &str) -> Option<DmaFlow> {
    match token {
        "device_to_host" => Some(DmaFlow::DeviceToHost),
        "host_to_device" => Some(DmaFlow::HostToDevice),
        _ => None,
    }
}

/// Parse + validate `#[rseq::dma_channel(id, direction)]` on `def_id`. Admitted
/// keys: `id = "…"` (string, mandatory) and `direction = "…"` (string, mandatory;
/// `device_to_host`/`host_to_device`). Unknown/duplicate keys and an unknown
/// direction token are rejected (mirrors the region attr's strict discipline).
fn parse_dma_channel_attr(tcx: TyCtxt<'_>, def_id: DefId) -> DmaChannelAttrResult {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("dma_channel")];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return DmaChannelAttrResult::Absent;
    };
    let Attribute::Unparsed(item) = attr else {
        return DmaChannelAttrResult::Absent;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return DmaChannelAttrResult::Absent;
    };
    if delim.delim != Delimiter::Parenthesis {
        return DmaChannelAttrResult::Absent;
    }
    let mut id: Option<String> = None;
    let mut direction: Option<DmaFlow> = None;
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return DmaChannelAttrResult::Bad("malformed attribute tokens".to_string());
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            return DmaChannelAttrResult::Bad("expected a `key = …` pair".to_string());
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => {
                return DmaChannelAttrResult::Bad(format!("key `{key}` is missing its `= …` value"))
            }
        }
        let value = iter.next();
        match key.as_str() {
            "id" => {
                if id.is_some() {
                    return DmaChannelAttrResult::Bad("duplicate `id` key".to_string());
                }
                match expect_str_lit(value) {
                    Some(s) => id = Some(s),
                    None => {
                        return DmaChannelAttrResult::Bad("`id` must be a string literal".to_string())
                    }
                }
            }
            "direction" => {
                if direction.is_some() {
                    return DmaChannelAttrResult::Bad("duplicate `direction` key".to_string());
                }
                match expect_str_lit(value) {
                    Some(s) => match parse_dma_flow(&s) {
                        Some(flow) => direction = Some(flow),
                        None => {
                            return DmaChannelAttrResult::Bad(format!(
                                "`direction` value {s:?} is not a valid flow (expected \
                                 \"device_to_host\" or \"host_to_device\")"
                            ))
                        }
                    },
                    None => {
                        return DmaChannelAttrResult::Bad(
                            "`direction` must be a string literal".to_string(),
                        )
                    }
                }
            }
            other => {
                return DmaChannelAttrResult::Bad(format!("unknown key `{other}`"));
            }
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => {
                return DmaChannelAttrResult::Bad("expected `,` between attribute keys".to_string())
            }
        }
    }
    let id = match id {
        Some(s) if !s.trim().is_empty() => s,
        Some(_) => return DmaChannelAttrResult::Bad("`id` is empty or whitespace".to_string()),
        None => return DmaChannelAttrResult::Bad("missing the mandatory `id` key".to_string()),
    };
    let Some(direction) = direction else {
        return DmaChannelAttrResult::Bad(
            "missing the mandatory `direction` key (a slave channel's direction is required)"
                .to_string(),
        );
    };
    DmaChannelAttrResult::Ok(DmaChannelAttr { id, direction })
}

/// Phase-4 materialisation: intern every driver's DMA-region facts and run the
/// M1 recognition negative set (missing mandatory key, malformed attr,
/// attr-on-non-region, unsupported form, duplicate id). Runs AFTER
/// `device_lifecycle::materialise_drivers` (the driver map is the walk root).
/// Fields are walked in declaration order (the order the `alloc_coherent`
/// preamble ops will run). Sets `dma_present = true` on a bus-master (PCI) driver
/// that declares a coherent region (E1000-style, GRILL T05).
pub(crate) fn materialise_dma_regions(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    let targets: Vec<(ItemId, DefId)> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item_id| {
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            Some((item_id, def_id))
        })
        .collect();

    for (item_id, d_def_id) in targets {
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(name_id).to_string()
        };
        let adt = tcx.adt_def(d_def_id);
        let mut regions = Vec::new();
        let mut channels = Vec::new();
        let mut field_markers = Vec::new();
        let mut seen_ids: Vec<StringId> = Vec::new();
        for variant in adt.variants() {
            for (field_idx, field) in variant.fields.iter().enumerate() {
                let field_ty = tcx.type_of(field.did).instantiate_identity();
                let Some(form) = classify_dma_region_form(tcx, field_ty) else {
                    // A slave `DmaChannel` field (M2/T07): classify it on the same
                    // walk, parse its mandatory `#[rseq::dma_channel(id, direction)]`
                    // (missing direction ⇒ reject), intern `DmaChannelFacts`, and
                    // push the `AttrData::DmaChannelResource` marker.
                    if is_dma_channel_ty(tcx, field_ty) {
                        let attr = match parse_dma_channel_attr(tcx, field.did) {
                            DmaChannelAttrResult::Ok(a) => a,
                            DmaChannelAttrResult::Absent => {
                                return Err(BridgeError::DmaValidation(DmaDiagnostic::ChannelAttr {
                                    driver: driver_name.clone(),
                                    field: field.name.to_string(),
                                    detail:
                                        "a `DmaChannel` field requires `#[rseq::dma_channel(id, \
                                         direction)]` (both keys mandatory)"
                                            .to_string(),
                                }))
                            }
                            DmaChannelAttrResult::Bad(detail) => {
                                return Err(BridgeError::DmaValidation(DmaDiagnostic::ChannelAttr {
                                    driver: driver_name.clone(),
                                    field: field.name.to_string(),
                                    detail,
                                }))
                            }
                        };
                        let id = cx.model.strings.intern(&attr.id);
                        if seen_ids.contains(&id) {
                            return Err(BridgeError::DmaValidation(DmaDiagnostic::DuplicateId {
                                driver: driver_name.clone(),
                                id: attr.id,
                            }));
                        }
                        seen_ids.push(id);
                        let name = cx.model.strings.intern(field.name.as_str());
                        channels.push(DmaChannelFacts {
                            name,
                            id,
                            direction: attr.direction,
                            binding: Default::default(),
                        });
                        field_markers.push((
                            FieldPath {
                                item: item_id,
                                variant: VariantIdx::ZERO,
                                field: FieldIdx::from_u32(field_idx as u32),
                            },
                            AttrData::DmaChannelResource {
                                id,
                                direction: attr.direction,
                            },
                        ));
                        continue;
                    }
                    // A `#[rseq::dma_channel]` attr on a non-`DmaChannel` field is a
                    // mis-applied attribute (mirror the region attr-on-non-region
                    // reject, reusing the channel diagnostic).
                    if has_dma_channel_attr(tcx, field.did) {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::ChannelAttr {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            detail: "a `#[rseq::dma_channel]` attribute on a non-`DmaChannel` field"
                                .to_string(),
                        }));
                    }
                    // An attr on a non-`DmaRegion` field.
                    if has_dma_region_attr(tcx, field.did) {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::AttrOnNonRegion {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                        }));
                    }
                    continue;
                };
                // M1 builds only the required form; reject the rest.
                if form != DmaRegionForm::Required {
                    return Err(BridgeError::DmaValidation(DmaDiagnostic::UnsupportedForm {
                        driver: driver_name.clone(),
                        field: field.name.to_string(),
                        form: form.label().to_string(),
                    }));
                }
                let attr = match parse_dma_region_attr(tcx, field.did) {
                    DmaRegionAttrResult::Ok(a) => a,
                    DmaRegionAttrResult::Absent => {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::MissingKey {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            key: "id",
                        }))
                    }
                    DmaRegionAttrResult::MissingKey(key) => {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::MissingKey {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            key,
                        }))
                    }
                    DmaRegionAttrResult::Malformed(detail) => {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::MalformedAttr {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            detail,
                        }))
                    }
                };
                let id = cx.model.strings.intern(&attr.id);
                // Duplicate L1 id within one driver.
                if seen_ids.contains(&id) {
                    return Err(BridgeError::DmaValidation(DmaDiagnostic::DuplicateId {
                        driver: driver_name.clone(),
                        id: attr.id,
                    }));
                }
                seen_ids.push(id);
                // Capture the element type `T` (the default view type + the layout
                // verify rule read it). Interned by its rendered name.
                let element_ty = dma_region_element_ty(tcx, field_ty);
                let element_type = element_ty.map(|t| cx.model.strings.intern(&t.to_string()));
                // dma-management FIX #2: resolve `T`'s ABI layout (size/align +
                // conservative POD marker). `None` when unresolvable — the
                // declaration guard below is then SKIPPED rather than crashing.
                let element_layout = element_ty.and_then(|t| dma_layout_facts(tcx, t));
                // Recover the omitted `alignment` to `align_of::<T>()` (the default
                // is `align_of::<T>()`, NOT 1 — GRILL D08/C03), and run the minimal
                // declaration-time layout guard against `T`'s resolved layout. When
                // `T`'s layout is unresolvable, leave `alignment` as declared and
                // skip the guard.
                let mut effective_alignment = attr.alignment;
                if let Some(layout) = element_layout {
                    let sz = layout.size;
                    let al = layout.align;
                    let effective_align = attr.alignment.unwrap_or(al);
                    let detail = if sz > attr.element_size {
                        Some(format!(
                            "declared element_size {} is smaller than size_of::<T>() = {} \
                             (the stride must fit the element type)",
                            attr.element_size, sz
                        ))
                    } else if al > effective_align {
                        Some(format!(
                            "effective alignment {} is below align_of::<T>() = {} \
                             (the region must be at least as aligned as its element type)",
                            effective_align, al
                        ))
                    } else if !effective_align.is_power_of_two() {
                        Some(format!(
                            "effective alignment {effective_align} is not a power of two"
                        ))
                    } else if attr.element_size % effective_align != 0 {
                        Some(format!(
                            "declared element_size {} is not a multiple of the effective \
                             alignment {} (stride must be alignment-padded)",
                            attr.element_size, effective_align
                        ))
                    } else {
                        None
                    };
                    if let Some(detail) = detail {
                        return Err(BridgeError::DmaValidation(DmaDiagnostic::LayoutMismatch {
                            driver: driver_name.clone(),
                            field: field.name.to_string(),
                            detail,
                        }));
                    }
                    // Recover the default alignment from `T` when omitted.
                    effective_alignment = Some(effective_align);
                }
                let name = cx.model.strings.intern(field.name.as_str());
                regions.push(DmaRegionFacts {
                    name,
                    id,
                    element_size: attr.element_size,
                    element_count: attr.element_count,
                    alignment: effective_alignment,
                    element_type,
                    element_layout,
                    element_accesses: Vec::new(),
                    binding: Default::default(),
                });
                field_markers.push((
                    FieldPath {
                        item: item_id,
                        variant: VariantIdx::ZERO,
                        field: FieldIdx::from_u32(field_idx as u32),
                    },
                    AttrData::DmaRegionResource {
                        id,
                        element_size: attr.element_size,
                        element_count: attr.element_count,
                        // Carry the recovered alignment (default `align_of::<T>()`,
                        // NOT 1 — GRILL D08/C03), consistent with the stored
                        // `DmaRegionFacts.alignment`.
                        alignment: effective_alignment,
                    },
                ));
            }
        }
        if regions.is_empty() && channels.is_empty() {
            continue;
        }
        // T05: a bus-master (PCI) driver declaring a coherent REGION is a DMA
        // master (E1000-style) → `dma_present = true`. NEVER for a slave channel
        // (a PL011-style `DmaChannel` is a slave; M2's channel does not set this
        // bit). Keep it narrow: only a region AND a PCI-like bus.
        let is_bus_master = !regions.is_empty()
            && cx.model.device_lifecycle.drivers[&item_id]
                .descriptor
                .bus_type
                .as_ref()
                .is_some_and(|b| b.is_pci_like());
        if is_bus_master {
            if let Some(driver) = cx.model.device_lifecycle.drivers.get_mut(&item_id) {
                driver.descriptor.dma_present = true;
            }
        }
        for (path, marker) in field_markers {
            cx.model
                .domain_attrs
                .field_attrs
                .entry(path)
                .or_default()
                .push(marker);
        }
        let driver_dma = cx.model.dma.drivers.entry(item_id).or_default();
        driver_dma.regions = regions;
        driver_dma.channels = channels;
    }
    Ok(())
}

/// Parse `#[rseq::dma_completion(channel = "…")]` on a handler method
/// (M3/T09(b)). The single mandatory key is `channel = "<id>"` (the join key back
/// to a `DmaChannel` resource). Mirrors the `dma_channel` attr parser's strict
/// discipline.
enum CompletionAttrResult {
    /// A well-formed `channel = "…"`.
    Ok(String),
    /// The attribute is malformed, the key is missing/duplicate/unknown, or
    /// `channel` is empty.
    Bad(String),
}

/// Parse the `#[rseq::dma_completion(channel = "…")]` attribute on `def_id`.
fn parse_dma_completion_attr(tcx: TyCtxt<'_>, def_id: DefId) -> CompletionAttrResult {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("dma_completion"),
    ];
    let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
        return CompletionAttrResult::Bad("missing the `#[rseq::dma_completion]` attribute".into());
    };
    let Attribute::Unparsed(item) = attr else {
        return CompletionAttrResult::Bad("malformed `#[rseq::dma_completion]` attribute".into());
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return CompletionAttrResult::Bad(
            "`#[rseq::dma_completion]` requires `(channel = \"…\")`".into(),
        );
    };
    if delim.delim != Delimiter::Parenthesis {
        return CompletionAttrResult::Bad("`#[rseq::dma_completion]` requires parentheses".into());
    }
    let mut channel: Option<String> = None;
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return CompletionAttrResult::Bad("malformed attribute tokens".into());
        };
        let Some(key) = key_tok.ident().map(|(ident, _)| ident.name) else {
            return CompletionAttrResult::Bad("expected a `key = …` pair".into());
        };
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => {
                return CompletionAttrResult::Bad(format!("key `{key}` is missing its `= …` value"))
            }
        }
        let value = iter.next();
        match key.as_str() {
            "channel" => {
                if channel.is_some() {
                    return CompletionAttrResult::Bad("duplicate `channel` key".into());
                }
                match expect_str_lit(value) {
                    Some(s) => channel = Some(s),
                    None => {
                        return CompletionAttrResult::Bad(
                            "`channel` must be a string literal".into(),
                        )
                    }
                }
            }
            other => return CompletionAttrResult::Bad(format!("unknown key `{other}`")),
        }
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => {
                return CompletionAttrResult::Bad("expected `,` between attribute keys".into())
            }
        }
    }
    match channel {
        Some(s) if !s.trim().is_empty() => CompletionAttrResult::Ok(s),
        Some(_) => CompletionAttrResult::Bad("`channel` is empty or whitespace".into()),
        None => CompletionAttrResult::Bad("missing the mandatory `channel` key".into()),
    }
}

/// Phase-4 `#[rseq::dma_completion(channel = "…")]` handler-method recognition
/// (M3/T09(b), GRILL D06/C05; registration-shape only). Item-driven, mirroring the
/// interrupt-ISR resolution: walk every author function item carrying the
/// attribute, resolve its inherent-impl self-ADT to the owning `#[rseq::driver]`,
/// parse the mandatory `channel` join key (missing/empty ⇒ reject), and record
/// BOTH an [`AttrData::DmaCompletionBinding`] on the handler item AND a
/// [`CompletionBinding`] into the driver's `DriverDma.completions`. The
/// `DmaCompletion` payload type is recorded as the handler's second-param type
/// name (the registration marker; the callback is NEVER fired this cycle, D06).
///
/// Runs at Phase-4 after `materialise_dma_regions` (the `dma.drivers` entries +
/// the driver map exist). A completion handler on a channel-less driver is still
/// recorded (the binding's `channel` join is validated at lowering, not here).
pub(crate) fn materialise_dma_completions(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    // driver-struct DefId -> model ItemId (mirrors the ISR `driver_items` map).
    let driver_items: std::collections::HashMap<DefId, ItemId> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item_id| {
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            Some((def_id, item_id))
        })
        .collect();

    // Walk model function items for the `#[rseq::dma_completion]` attribute.
    let n_items = cx.model.items.len();
    // (handler ItemId, driver ItemId, channel id, payload name, method name).
    let mut bindings: Vec<(ItemId, ItemId, String, String, String)> = Vec::new();
    for idx in 0..n_items {
        let handler_item = ItemId::from_u32(idx as u32);
        if !matches!(cx.model.items[handler_item].kind, ItemKind::Function) {
            continue;
        }
        // Synthetic items (vtable shims, ConstMemoryStatics, …) carry no source
        // def-path; `recover_source_def_id` on their StableCrateId(0) ICEs the
        // cross-crate decoder. Skip them BEFORE the lookup (the pin_state-marker
        // precedent).
        if !matches!(
            cx.model.item_synthetic_kinds[handler_item],
            regseq_model::SyntheticItemKind::None
        ) {
            continue;
        }
        let Some(def_id) = recover_source_def_id(tcx, cx.model.item_def_paths[handler_item]) else {
            continue;
        };
        if !def_id.is_local() {
            continue;
        }
        if !has_rseq_attr(tcx, def_id, "dma_completion") {
            continue;
        }
        let method_name = tcx.item_name(def_id).to_string();
        let channel = match parse_dma_completion_attr(tcx, def_id) {
            CompletionAttrResult::Ok(c) => c,
            CompletionAttrResult::Bad(detail) => {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::CompletionAttr {
                    method: method_name,
                    detail,
                }))
            }
        };
        // Resolve the handler's inherent-impl self-ADT to the owning driver.
        let parent = tcx.parent(def_id);
        let driver_item = match tcx.def_kind(parent) {
            rustc_hir::def::DefKind::Impl { of_trait: false } => {
                let self_ty = tcx.type_of(parent).instantiate_identity();
                self_ty
                    .ty_adt_def()
                    .and_then(|adt| driver_items.get(&adt.did()).copied())
            }
            _ => None,
        };
        let Some(driver_item) = driver_item else {
            return Err(BridgeError::DmaValidation(DmaDiagnostic::CompletionAttr {
                method: method_name,
                detail: "a `#[rseq::dma_completion]` handler must be an inherent method on a \
                         `#[rseq::driver]` struct"
                    .into(),
            }));
        };
        // The `DmaCompletion` payload type name = the handler's second-parameter
        // type (registration marker only; never constructed this cycle, D06).
        let payload = completion_payload_name(tcx, def_id).unwrap_or_else(|| "DmaCompletion".into());
        bindings.push((handler_item, driver_item, channel, payload, method_name));
    }

    for (handler_item, driver_item, channel, payload, method_name) in bindings {
        let channel_id = cx.model.strings.intern(&channel);
        let payload_id = cx.model.strings.intern(&payload);
        // BLOCKING #2 (M3 fix): the bound `channel` MUST name a declared
        // `#[rseq::dma_channel(id = …)]` on the owning driver — registering a
        // dangling completion for a channel that was never `request_channel`'d is a
        // hard reject (a `channel = "typo"` mismatch, GRILL C05).
        let channel_known = cx
            .model
            .dma
            .drivers
            .get(&driver_item)
            .is_some_and(|d| d.channels.iter().any(|c| c.id == channel_id));
        if !channel_known {
            let driver_name = {
                let name_id = cx.model.device_lifecycle.drivers[&driver_item].descriptor.name;
                cx.model.strings.resolve(name_id).to_string()
            };
            return Err(BridgeError::DmaValidation(
                DmaDiagnostic::UnknownCompletionChannel {
                    method: method_name,
                    channel,
                    driver: driver_name,
                },
            ));
        }
        cx.model.domain_attrs.item_attrs[handler_item].push(AttrData::DmaCompletionBinding {
            channel: channel_id,
            handler: handler_item,
            payload: payload_id,
        });
        cx.model
            .dma
            .drivers
            .entry(driver_item)
            .or_default()
            .completions
            .push(CompletionBinding {
                channel: channel_id,
                handler: handler_item,
                payload: payload_id,
            });
    }
    Ok(())
}

/// The handler's `DmaCompletion` payload type name (the SECOND parameter after
/// `&mut self`), used as the registration marker. `None` when the signature has no
/// second parameter or it has no nominal name.
fn completion_payload_name(tcx: TyCtxt<'_>, def_id: DefId) -> Option<String> {
    let sig = tcx.fn_sig(def_id).instantiate_identity();
    let inputs = sig.inputs().skip_binder();
    // inputs[0] = &mut self; inputs[1] = the DmaCompletion payload.
    let payload_ty = inputs.get(1)?;
    payload_ty.ty_adt_def().map(|adt| tcx.item_name(adt.did()).to_string())
}

/// Whether `def_id` carries an `#[rseq::<leaf>]` attribute (the item-driven attr
/// check, mirroring pinctrl's `has_rseq_attr`).
fn has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, leaf: &str) -> bool {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern(leaf)];
    let present = tcx.get_attrs_by_path(def_id, &path).next().is_some();
    present
}

/// Phase-5 coherent-region call-site recognition (GRILL T06/T08): walk every
/// materialized body and, for each `Call` whose callee is a region/A2
/// `IntrinsicKind::Dma` op, recover the `DmaRegion` receiver field and append the
/// per-`element_access`-site [`ElementAccessFact`] onto that region's facts.
///
/// Disambiguation (T08): the generic surface verbs `read`/`write`/`update`
/// collide with `Mmio<T>`'s, so the resolved `IntrinsicKind::Dma(...)` (from the
/// namespaced `#[rseq::intrinsic("dma_element_*")]` tag) is the primary
/// authority; the receiver-resource-TYPE check (recovering a `DmaRegion` field
/// place) is the redundant confirming gate, exactly as the design specifies.
///
/// Runs after every body is materialized (the same placement discipline as
/// `synchronization::recognise_acquire_sites` / `atomic::validate_atomic_ops`).
pub(crate) fn recognise_dma_region_sites(model: &mut RegseqModel) -> Result<(), BridgeError> {
    // Collect the facts first (the walk borrows `model` immutably), then splice
    // them onto the region facts in a second mutable pass — `model.dma` is
    // re-keyed by the Phase-5 item sort, so the region is looked up by its final
    // driver `ItemId` + resource id.
    struct Site {
        driver: ItemId,
        region_id: StringId,
        fact: ElementAccessFact,
    }
    // M3/T09: `region.span(range)` endpoints recorded into `DriverDma.spans`.
    struct SpanSite {
        driver: ItemId,
        span: DmaRegionSpan,
    }
    let mut sites: Vec<Site> = Vec::new();
    let mut span_sites: Vec<SpanSite> = Vec::new();
    for instance in model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            // M3/T09: `region.span(range) -> DmaRegionSpan` recognition. Recover the
            // region receiver, read the (statically-const) `Range` start/end, and
            // record a `DmaRegionSpan` (element-indexed device addr + byte len +
            // provenance) into the driver's `spans` (GRILL D05/C06).
            if let Some(IntrinsicKind::Dma(DmaOp::RegionSpan)) =
                model.instance_intrinsic(*callee_inst)
            {
                let Some(recv) = args.first() else {
                    continue;
                };
                let Some((driver, region_id)) = recover_dma_region_receiver(model, body, recv)
                else {
                    continue;
                };
                let (element_size, element_count) = model
                    .dma
                    .drivers
                    .get(&driver)
                    .and_then(|d| d.regions.iter().find(|r| r.id == region_id))
                    .map(|r| (r.element_size, r.element_count))
                    .unwrap_or((0, 0));
                // The `Range<usize>` argument (`args[1]`): extract `start`/`end`
                // where statically const.
                let region_str = model.strings.resolve(region_id).to_string();
                // BLOCKING #1 (M3 fix): a NON-const span range is rejected LOUDLY —
                // recording `start=0,len=0` would silently degenerate to a
                // zero-length transfer at lowering. Only `region.span(const..const)`
                // is supported this cycle (M4+ may admit dynamic ranges, GRILL C06).
                let Some((start, end)) = args.get(1).and_then(|op| extract_range_const(model, body, op))
                else {
                    return Err(BridgeError::DmaValidation(DmaDiagnostic::SpanBounds {
                        region: region_str,
                        detail: "non-const span range not supported this cycle (M4+); \
                                 `region.span(range)` requires a statically-const \
                                 `start..end` so the device address + length are known"
                            .to_string(),
                    }));
                };
                // T20 span-bounds checks (GRILL C06): a zero-length (`start >= end`)
                // span and an out-of-bounds (`end > element_count`) span compile-fail.
                if start >= end {
                    return Err(BridgeError::DmaValidation(DmaDiagnostic::SpanBounds {
                        region: region_str,
                        detail: format!(
                            "zero-length span (range {start}..{end}); a staged transfer needs \
                             at least one element"
                        ),
                    }));
                }
                if element_count != 0 && end > element_count {
                    return Err(BridgeError::DmaValidation(DmaDiagnostic::SpanBounds {
                        region: region_str,
                        detail: format!(
                            "range end {end} exceeds element_count {element_count} \
                             (out of bounds)"
                        ),
                    }));
                }
                let len = end.saturating_sub(start).saturating_mul(element_size);
                span_sites.push(SpanSite {
                    driver,
                    span: DmaRegionSpan {
                        device_addr: SpanAddr {
                            element_offset: start,
                            element_size,
                        },
                        len,
                        provenance: region_id,
                    },
                });
                continue;
            }
            // Only the element-access verbs append a per-site fact (the A2 ops
            // `device_addr`/`device_addr_of`/`element_count` lower from the
            // resource directly; they carry no closure borrow, GRILL S04).
            let Some(IntrinsicKind::Dma(DmaOp::ElementAccess { verb })) =
                model.instance_intrinsic(*callee_inst)
            else {
                continue;
            };
            let verb = *verb;
            let Some(recv) = args.first() else {
                continue;
            };
            // Receiver-RESOURCE-TYPE disambiguation: recover the `DmaRegion` field
            // the receiver names. A shape that does not resolve to a recognized
            // region field is skipped (its callee was tagged DMA, but the
            // receiver is not a known region — leave it to a later verify pass /
            // the backend to error rather than mis-attributing it here).
            let Some((driver, region_id)) = recover_dma_region_receiver(model, body, recv) else {
                continue;
            };
            // The view type `V` (the per-call reinterpret `::<U>`, default `T`):
            // the FIRST type generic arg of the callee instance, mirroring
            // pinctrl's `apply::<S>` resolution. `None` keeps `view_type = None`
            // (the layout pass then falls back to the declared `T`).
            let view_ty_id = first_type_arg_ty(model, *callee_inst);
            let view_type = view_ty_id.and_then(|t| type_name(model, t));
            // dma-management FIX #2: populate the view layout. When the view type
            // equals the declared `T` (same interned name), or there is no
            // turbofish, copy the region's `element_layout`. Otherwise read `V`'s
            // own size/align from the model type table (conservative `is_pod =
            // false`, since the model carries no Drop info for the view here; the
            // M4/T19 per-access verify is where POD-of-view matters). `None` when
            // unresolvable. M1 only CARRIES this; the size-fits-view verify is M4.
            let region_element = model
                .dma
                .drivers
                .get(&driver)
                .and_then(|d| d.regions.iter().find(|r| r.id == region_id));
            let region_element_layout = region_element.and_then(|r| r.element_layout);
            let region_element_type = region_element.and_then(|r| r.element_type);
            let view_layout = match view_type {
                // No turbofish, or the view names the same type as `T`: the view is
                // the declared element type — reuse its resolved layout.
                None => region_element_layout,
                Some(v) if Some(v) == region_element_type => region_element_layout,
                // A distinct, resolvable view type: read its own size/align.
                Some(_) => view_ty_id.and_then(|t| view_layout_facts(model, t)),
            };
            // The element index `i` (`args[1]`) where it is a direct integer
            // constant; a non-constant index defers the OOB check (GRILL C03).
            let index = args.get(1).and_then(|op| const_uint_operand(model, op));
            // The lexical extent of the closure borrow. PLACEHOLDER-FLAGGED
            // (GRILL R-closure-extent): the precise closure entry/exit block pair
            // has no clock/pinctrl precedent, so the call-site terminator span is
            // recorded as the bracketing extent for now (the closure call is the
            // single terminator that brackets the borrow). The exact entry/exit
            // encoding is wired with the closure-borrow backend codegen (T13).
            let lexical_extent = LexicalExtent {
                span: bb.terminator.span,
            };
            sites.push(Site {
                driver,
                region_id,
                fact: ElementAccessFact {
                    region: region_id,
                    index,
                    view_type,
                    view_layout,
                    verb,
                    effects: AccessEffects::from_verb(verb),
                    lexical_extent,
                },
            });
        }
    }
    for site in sites {
        if let Some(d) = model.dma.drivers.get_mut(&site.driver) {
            if let Some(region) = d.regions.iter_mut().find(|r| r.id == site.region_id) {
                region.element_accesses.push(site.fact);
            }
        }
    }
    for site in span_sites {
        if let Some(d) = model.dma.drivers.get_mut(&site.driver) {
            d.spans.push(site.span);
        }
    }
    Ok(())
}

/// Phase-5 `stage_transfer` endpoint verify (M3/T20, GRILL C06/I07). Walk every
/// `channel.stage_transfer(endpoint)` call-site and reject:
///
///   * **anti-form** (I07): the endpoint is a coherent `DmaRegion<T>` passed
///     DIRECTLY (`stage_transfer(&self.buf)`) instead of via `region.span(range)`.
///     Detected by recovering the endpoint operand to a recognized `DmaRegion`
///     field marker (the same receiver-resource-type chase the region ops use). A
///     correct `region.span(range)` endpoint is a `DmaRegionSpan` temp, which does
///     NOT resolve to a region FIELD, so it passes.
///   * **direction** (D05/C06): the span's role (source vs destination) is fixed by
///     the CHANNEL direction, not the span — the span is DIRECTION-NEUTRAL by
///     design, so a span endpoint is accepted for either channel direction. The
///     region-incompatible-with-direction sub-check (e.g. a non-device-writable
///     region as an RX destination) has NO IR carrier this cycle (a region carries
///     no device-writability flag) and is RECORDED-NOT-ENFORCED (documented
///     deferral; see the module limitation note). The span-bounds (OOB/zero-length)
///     check fires at span recognition above.
///
/// Runs after `recognise_dma_region_sites` (the region markers it chases exist).
pub(crate) fn validate_stage_transfer_endpoints(
    model: &RegseqModel,
) -> Result<(), BridgeError> {
    for instance in model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            let Some(IntrinsicKind::Dma(DmaOp::ChannelStageTransfer)) =
                model.instance_intrinsic(*callee_inst)
            else {
                continue;
            };
            // The endpoint is `args[1]` (`args[0]` is the channel receiver).
            let Some(endpoint) = args.get(1) else {
                continue;
            };
            // Anti-form (I07): the endpoint resolves to a recognized `DmaRegion`
            // FIELD ⇒ the bare region was passed directly. (A `region.span(range)`
            // result is a `DmaRegionSpan` temp, which does NOT resolve to a region
            // field, so it is accepted.)
            if let Some((_driver, region_id)) =
                recover_dma_region_receiver(model, body, endpoint)
            {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::AntiForm {
                    region: model.strings.resolve(region_id).to_string(),
                }));
            }
        }
    }
    Ok(())
}

/// Extract `(start, end)` of a `Range<usize>` operand where both bounds are
/// statically const (M3/T09). A `region.span(0..N)` lowers the range to an
/// `Rvalue::Aggregate(Struct, [start, end])` assigned to a temp; the operand is
/// usually `&temp` (a hoisted borrow). Compose the borrow away, find the temp's
/// defining aggregate, and read its two const-uint fields. Returns `None` when the
/// range is non-const; the `RegionSpan` caller then REJECTS loudly (a non-const
/// span range is not supported this cycle, M4+), rather than deferring (GRILL C06/T20).
fn extract_range_const(model: &RegseqModel, body: &Body, op: &Operand) -> Option<(u64, u64)> {
    // The `Range<usize>` value lives in a bare local. `0..N` lowers to
    // `_t = Aggregate(Struct, [start, end])` IN PLACE — the operand is `Move(_t)`
    // (NOT a borrow), so resolve to the local directly and read its aggregate def.
    // (A by-ref `&range` borrow form is also handled: chase one Ref level.)
    let local = match op {
        Operand::Copy(p) | Operand::Move(p) if p.projection.is_empty() => p.local,
        Operand::Copy(p) | Operand::Move(p) => {
            // A projected place (`(*_t)` deref of a borrow) — peel a Ref def.
            match (p.projection.first(), unique_assign(body, p.local)) {
                (
                    Some(PlaceElem::Deref),
                    Some(regseq_model::Rvalue::Ref(_, base) | regseq_model::Rvalue::RawPtr(_, base)),
                ) if base.projection.is_empty() => base.local,
                _ => return None,
            }
        }
        Operand::Constant(_) => return None,
    };
    // Follow a borrow temp (`_t = &range`) to the underlying range local, then read
    // the aggregate. A direct aggregate def short-circuits the borrow chase.
    let (fields_local, def) = match unique_assign(body, local) {
        Some(regseq_model::Rvalue::Aggregate(_, fields)) => (local, Some(fields)),
        Some(regseq_model::Rvalue::Ref(_, base) | regseq_model::Rvalue::RawPtr(_, base))
            if base.projection.is_empty() =>
        {
            match unique_assign(body, base.local) {
                Some(regseq_model::Rvalue::Aggregate(_, fields)) => (base.local, Some(fields)),
                _ => (base.local, None),
            }
        }
        _ => (local, None),
    };
    let _ = fields_local;
    let fields = def?;
    // `Range<usize> { start, end }` — two fields, both const-uint.
    if fields.len() != 2 {
        return None;
    }
    let start = const_uint_operand(model, &fields[0])?;
    let end = const_uint_operand(model, &fields[1])?;
    Some((start, end))
}

/// Phase-5 streaming recognition + the fallibility lint (M2/T07 + M2/T17). Walk
/// every materialized body and:
///
///   * (T07 streaming) for each `Call` whose callee is `DmaOp::StreamMap`
///     (`dma::stream(buf, flow)`), record a [`DmaStreamFacts`] — the flow + the
///     RAII lifetime — into the owning driver's `DriverDma.streams`. The lifetime
///     is carried as the call-site span (PLACEHOLDER-FLAGGED; the precise Drop
///     seam is M4/T15, mirroring the region element-access extent placeholder).
///     The `DmaStreamBuffer` accessor methods (`device_addr`/`acquire_from_device`/
///     `release_to_device`) are tagged DMA intrinsics already (no per-site fact is
///     recorded for them this cycle; their EMIT is M4/T15).
///
///   * (T17 fallibility) for each `Call` to a FALLIBLE DMA op
///     (`dma::stream`/`stage_transfer`/`start_transfer`/`remaining_bytes` — NOT
///     the infallible `stop`), reject when its `Result` destination is DISCARDED
///     — including the explicit `let _ = …` form that Rust's `unused_must_use`
///     does NOT fire on. `stop` (`ChannelStop`) is exempt. The bare-statement
///     `op();` discard is ALSO caught here (belt-and-braces with the `#[must_use]`
///     surface markers), since both lower to a destination local with no
///     downstream value-use.
///
/// Runs after `recognise_dma_region_sites`, same placement discipline.
pub(crate) fn recognise_dma_streams_and_fallibility(
    model: &mut RegseqModel,
) -> Result<(), BridgeError> {
    // Phase 1 — fallibility check (read-only). A discarded fallible-op Result is a
    // hard reject; run it first so a bad driver fails before any stream is spliced.
    for instance in model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call {
                callee,
                destination,
                ..
            } = &bb.terminator.kind
            else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            let Some(IntrinsicKind::Dma(op)) = model.instance_intrinsic(*callee_inst) else {
                continue;
            };
            let Some(op_name) = fallible_dma_op_name(*op) else {
                continue;
            };
            // The destination is a whole-local Result temp (no projection). When
            // the destination is a projected place (an unusual lowering), SKIP — a
            // documented residual gap (see `local_is_consumed`).
            if !destination.projection.is_empty() {
                continue;
            }
            let d = destination.local;
            // DISCARD-SINK reject (BLOCKING #1): the idiomatic explicit-discard
            // forms `core::mem::drop(op())` / `core::mem::forget(op())` MOVE the
            // Result into a sink call — a genuine discard the body-wide "any read"
            // scan would otherwise count as consumption. Detect it first.
            if local_moved_into_discard_sink(model, body, d) {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::DiscardedResult {
                    op: op_name,
                }));
            }
            // A discarded Result (`let _ = …` or a bare `op();` statement) leaves
            // the destination local with NO downstream value-use AND it is not the
            // returned value. A consumed Result is read (`let r = …; r?` reads `r`
            // via the `?` desugar's `Discriminant`/field projection; `op()?`
            // direct-propagation likewise reads the call temp) OR it IS the body's
            // return value (`return op();` assigns the call destination straight
            // into the return place — counted as consumption to avoid a false
            // reject on legitimate propagation).
            let returned = d == body.return_local();
            if !returned && !local_is_consumed(body, d) {
                return Err(BridgeError::DmaValidation(DmaDiagnostic::DiscardedResult {
                    op: op_name,
                }));
            }
        }
    }

    // Phase 2 — streaming recognition (collect then splice).
    struct StreamSite {
        driver: ItemId,
        fact: DmaStreamFacts,
    }
    let mut sites: Vec<StreamSite> = Vec::new();
    for instance in model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            let Some(IntrinsicKind::Dma(DmaOp::StreamMap)) =
                model.instance_intrinsic(*callee_inst)
            else {
                continue;
            };
            // The owning driver: `dma::stream` is a FREE function (no `self`
            // receiver), so attribute the stream to the driver whose method body
            // contains the call — recovered by scanning the body's locals for a
            // known DMA-driver struct type (the `&mut self` local). A body with no
            // recognized DMA driver is skipped (the stream has no home table).
            let Some(driver) = find_owning_dma_driver(model, body) else {
                continue;
            };
            // The streaming `flow` (`args[1]`): read its discriminant as a const
            // (0 ⇒ device_to_host, 1 ⇒ host_to_device). A non-const/unreadable
            // flow defaults to `DeviceToHost` (the lifetime carrier is the load-
            // bearing M2 fact; the precise flow check is M3/T20).
            let flow = args
                .get(1)
                .and_then(|op| const_uint_operand(model, op))
                .and_then(dma_flow_from_discriminant)
                .unwrap_or(DmaFlow::DeviceToHost);
            // The RAII lifetime extent: the call-site span (PLACEHOLDER-FLAGGED,
            // mirroring the region element-access extent; the precise Drop seam is
            // M4/T15).
            let lifetime = DmaStreamBuffer {
                extent: LexicalExtent {
                    span: bb.terminator.span,
                },
            };
            sites.push(StreamSite {
                driver,
                fact: DmaStreamFacts { flow, lifetime },
            });
        }
    }
    for site in sites {
        if let Some(d) = model.dma.drivers.get_mut(&site.driver) {
            d.streams.push(site.fact);
        }
    }
    Ok(())
}

/// The author-facing spelling of a FALLIBLE DMA op (the fallibility lint set,
/// GRILL C10), or `None` for an infallible op (`stop`) or a non-channel/stream op
/// (the A2/element ops are not in the discarded-Result set).
fn fallible_dma_op_name(op: DmaOp) -> Option<&'static str> {
    match op {
        DmaOp::StreamMap => Some("dma::stream"),
        DmaOp::ChannelStageTransfer => Some("stage_transfer"),
        DmaOp::ChannelStartTransfer => Some("start_transfer"),
        DmaOp::ChannelRemainingBytes => Some("remaining_bytes"),
        // `stop` is infallible (exempt); the A2/element/acquire/release ops are
        // not in the discarded-Result fallibility set this cycle.
        _ => None,
    }
}

/// Map a `flow` discriminant const (0/1) to a [`DmaFlow`], or `None` for an
/// out-of-range value.
fn dma_flow_from_discriminant(d: u64) -> Option<DmaFlow> {
    match d {
        0 => Some(DmaFlow::DeviceToHost),
        1 => Some(DmaFlow::HostToDevice),
        _ => None,
    }
}

/// Whether `local` is MOVED INTO a discard-SINK call (`core::mem::drop` /
/// `core::mem::forget`, incl. the `std::mem::*` aliases) anywhere in `body` — the
/// idiomatic explicit-discard forms that the body-wide "any read" scan
/// ([`local_is_consumed`]) would otherwise mis-count as consumption (the Result is
/// read INTO the sink as an argument). A match is a genuine DISCARD ⇒ the
/// fallibility lint rejects (BLOCKING #1). The sink is identified by its callee's
/// rendered def-path: the value-namespace leaf is `drop`/`forget` AND the path
/// runs through `::mem::` (so a user fn named `drop`/`forget` in another module is
/// NOT mistaken for the std sink). Only a WHOLE-local move (`drop(r)` with `r`
/// unprojected) counts.
fn local_moved_into_discard_sink(
    model: &RegseqModel,
    body: &Body,
    local: regseq_model::LocalId,
) -> bool {
    for bb in body.basic_blocks.iter() {
        let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
            continue;
        };
        if !is_mem_discard_sink(model, callee) {
            continue;
        }
        // The sink takes the value as its sole/first argument; a whole-local move
        // (`drop(r)`/`forget(r)`) is the discard form.
        if args.iter().any(|op| {
            matches!(op, Operand::Copy(p) | Operand::Move(p)
                if p.local == local && p.projection.is_empty())
        }) {
            return true;
        }
    }
    false
}

/// Whether `callee` resolves to `core::mem::drop` / `core::mem::forget` (or the
/// `std::mem::*` re-exports): the value-namespace leaf is `drop`/`forget` AND the
/// rendered def-path contains `::mem::` (excludes a user `drop`/`forget`).
fn is_mem_discard_sink(model: &RegseqModel, callee: &CallCallee) -> bool {
    let CallCallee::Direct(inst) = callee else {
        return false;
    };
    let item = model.instances[*inst].item;
    let def_path = &model.items[item].def_path;
    let leaf = match def_path.segments.last().map(|s| &s.data) {
        Some(regseq_model::DefPathData::ValueNs(s)) => model.strings.resolve(*s),
        _ => return false,
    };
    if leaf != "drop" && leaf != "forget" {
        return false;
    }
    let rendered = model.strings.resolve(def_path.rendered);
    rendered.contains("::mem::")
}

/// Whether `local` is CONSUMED (read as a value) anywhere in `body` — used by the
/// fallibility lint to distinguish a consumed Result (`let r = …; r?`, which reads
/// `r`) from a discarded one (`let _ = …` / a bare `op();`, which never reads the
/// destination). A "consume" is any appearance of `local` in a read position: an
/// operand (`Copy`/`Move`), a borrow/raw-ptr/discriminant source place, a cast/
/// binop/etc. operand, a switch discriminant, a call/assert operand. An ASSIGN to
/// `local` (the call's own destination) is NOT a consume. Conservative: a
/// projected appearance (`local.field`) also counts as a consume (so the lint only
/// fires on a truly-unused whole-local).
///
/// COMPLETENESS GAPS (documented, BLOCKING #1 residuals — rare, need full def-use
/// dataflow, deferred to a hardening task; each is a FALSE-ACCEPT, never a
/// false-reject, so it errs toward accepting unusual-but-possibly-valid code):
///   * **aggregate-store-never-read:** moving the Result into a struct/array field
///     (`s.f = op()`) that is then never inspected counts the move as a read.
///   * **reassignment-before-read:** `let mut r = op1(); r = op2(); r?` — the read
///     of `r` consumes BOTH defs, so `op1`'s discarded Result evades the lint.
///   * **projected call destination:** a fallible-op Call whose destination is a
///     projected place (not a whole local) is SKIPPED entirely by the caller (an
///     unusual lowering; the lint only fires on the unmistakable whole-local form).
///   * **indirect discard sink:** `let f = core::mem::drop; f(op())` calls the sink
///     through a fn-pointer (`CallCallee::Indirect`), which [`is_mem_discard_sink`]
///     does not resolve, so the move counts as a read. Aliasing `drop`/`forget`
///     through a fn-pointer in driver init is essentially never written.
/// The two GENUINE explicit-discard idioms — `let _ = …` and a DIRECT
/// `core::mem::{drop,forget}(…)` — ARE caught (the latter by
/// [`local_moved_into_discard_sink`], checked before this scan).
fn local_is_consumed(body: &Body, local: regseq_model::LocalId) -> bool {
    use regseq_model::{Rvalue, StatementKind};

    fn place_reads(place: &Place, local: regseq_model::LocalId) -> bool {
        place.local == local
    }
    fn op_reads(op: &Operand, local: regseq_model::LocalId) -> bool {
        matches!(op, Operand::Copy(p) | Operand::Move(p) if place_reads(p, local))
    }
    fn rvalue_reads(rv: &Rvalue, local: regseq_model::LocalId) -> bool {
        match rv {
            Rvalue::Use(o) | Rvalue::Repeat(o, _) | Rvalue::Cast(_, o, _) | Rvalue::UnaryOp(_, o) => {
                op_reads(o, local)
            }
            Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p) | Rvalue::Discriminant(p) => {
                place_reads(p, local)
            }
            Rvalue::BinaryOp(_, a, b) => op_reads(a, local) || op_reads(b, local),
            Rvalue::Aggregate(_, ops) => ops.iter().any(|o| op_reads(o, local)),
        }
    }

    for bb in body.basic_blocks.iter() {
        for stmt in bb.statements.iter() {
            match &stmt.kind {
                StatementKind::Assign(dst, rhs) => {
                    // A read appearing as the SOURCE of an assign counts (but the
                    // destination being `local` does not — that is the def, not a
                    // use). A `local.field = …` store also reads `local` (it is a
                    // base place), so a projected destination on `local` counts.
                    if dst.local == local && !dst.projection.is_empty() {
                        return true;
                    }
                    if rvalue_reads(rhs, local) {
                        return true;
                    }
                }
                StatementKind::SetDiscriminant(p, _) => {
                    if place_reads(p, local) {
                        return true;
                    }
                }
                StatementKind::Intrinsic(_) => {}
            }
        }
        match &bb.terminator.kind {
            TerminatorKind::SwitchInt { discr, .. } => {
                if op_reads(discr, local) {
                    return true;
                }
            }
            TerminatorKind::Call {
                args, destination, ..
            } => {
                if args.iter().any(|o| op_reads(o, local)) {
                    return true;
                }
                // A projected destination on `local` reads it as a base place.
                if destination.local == local && !destination.projection.is_empty() {
                    return true;
                }
            }
            TerminatorKind::Assert { cond, .. } => {
                if op_reads(cond, local) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Find the driver this body belongs to by scanning its locals for a known
/// DMA-driver struct type (the `&[mut] self` local of a driver method). Returns
/// the FIRST `ItemId` present in `model.dma.drivers`. `None` when the body touches
/// no recognized DMA driver (a free function unrelated to any driver).
fn find_owning_dma_driver(model: &RegseqModel, body: &Body) -> Option<ItemId> {
    for decl in body.local_decls.iter() {
        let mut ty = decl.ty;
        // Peel references/pointers to reach the underlying struct.
        loop {
            match &model.types[ty] {
                ModelTy::Ref(p, _) | ModelTy::Ptr(p, _) => ty = *p,
                ModelTy::Struct { item, .. } => {
                    if model.dma.drivers.contains_key(item) {
                        return Some(*item);
                    }
                    break;
                }
                _ => break,
            }
        }
    }
    None
}

/// Recover the `(driver item, region id)` the receiver operand of a DMA
/// element-access call names. Composes away hoisted borrow temps (mirroring the
/// atomic/pinctrl receiver resolvers) and walks the place projection to the first
/// field carrying an [`AttrData::DmaRegionResource`] marker.
fn recover_dma_region_receiver(
    model: &RegseqModel,
    body: &Body,
    recv: &Operand,
) -> Option<(ItemId, StringId)> {
    let place = canonical_receiver_place(model, body, recv, 0)?;
    walk_to_dma_region_field(model, body, &place)
}

/// Resolve a receiver operand to the canonical place it names — rooted at a
/// non-borrow-temp local with the full projection — composing away hoisted borrow
/// temps (`_t = &place`). Mirrors `pinctrl::canonical_receiver_place`.
fn canonical_receiver_place(
    model: &RegseqModel,
    body: &Body,
    op: &Operand,
    depth: u32,
) -> Option<Place> {
    if depth > 16 {
        return None;
    }
    let place = match op {
        Operand::Copy(p) | Operand::Move(p) => p.clone(),
        Operand::Constant(_) => return None,
    };
    compose_place(model, body, place, depth)
}

fn compose_place(model: &RegseqModel, body: &Body, place: Place, depth: u32) -> Option<Place> {
    if depth > 16 {
        return None;
    }
    if place.projection.is_empty() {
        return match unique_assign(body, place.local)? {
            regseq_model::Rvalue::Ref(_, src) | regseq_model::Rvalue::RawPtr(_, src) => {
                compose_place(model, body, src.clone(), depth + 1)
            }
            regseq_model::Rvalue::Use(inner) => canonical_receiver_place(model, body, inner, depth + 1),
            _ => None,
        };
    }
    if matches!(place.projection.first(), Some(PlaceElem::Deref)) {
        if let Some(
            regseq_model::Rvalue::Ref(_, base) | regseq_model::Rvalue::RawPtr(_, base),
        ) = unique_assign(body, place.local)
        {
            let mut projection: Vec<PlaceElem> = base.projection.to_vec();
            projection.extend_from_slice(&place.projection[1..]);
            let spliced = Place {
                local: base.local,
                projection: projection.into_boxed_slice(),
            };
            return compose_place(model, body, spliced, depth + 1);
        }
    }
    Some(place)
}

/// The unique whole-local definition rvalue of `local` in `body`, or `None` if
/// there is no definition or more than one (ambiguous). Mirrors
/// `pinctrl::unique_assign`.
fn unique_assign(body: &Body, local: regseq_model::LocalId) -> Option<&regseq_model::Rvalue> {
    let mut def: Option<&regseq_model::Rvalue> = None;
    for bb in body.basic_blocks.iter() {
        for stmt in bb.statements.iter() {
            let regseq_model::StatementKind::Assign(dst, rhs) = &stmt.kind else {
                continue;
            };
            if dst.local != local || !dst.projection.is_empty() {
                continue;
            }
            if def.is_some() {
                return None;
            }
            def = Some(rhs);
        }
    }
    def
}

/// Walk a canonical place's projection to the FIRST field bearing an
/// [`AttrData::DmaRegionResource`] marker, returning `(driver item, region id)`.
fn walk_to_dma_region_field(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
) -> Option<(ItemId, StringId)> {
    let mut ty = body.local_decls[place.local].ty;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &model.types[ty] {
                    ModelTy::Ref(p, _) | ModelTy::Ptr(p, _) => *p,
                    _ => return None,
                };
            }
            PlaceElem::Field(idx, field_ty) => {
                let item = match &model.types[ty] {
                    ModelTy::Struct { item, .. } => *item,
                    _ => return None,
                };
                let fp = FieldPath {
                    item,
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(idx.index() as u32),
                };
                if let Some(region_id) = dma_region_id_at(model, &fp) {
                    return Some((item, region_id));
                }
                ty = *field_ty;
            }
            _ => return None,
        }
    }
    None
}

/// The region id of the `AttrData::DmaRegionResource` marker at `fp`, if any.
fn dma_region_id_at(model: &RegseqModel, fp: &FieldPath) -> Option<StringId> {
    model.domain_attrs.field_attrs.get(fp).and_then(|attrs| {
        attrs.iter().find_map(|a| match a {
            AttrData::DmaRegionResource { id, .. } => Some(*id),
            _ => None,
        })
    })
}

/// The FIRST type generic arg `TyId` of the callee instance, recorded by M1 as a
/// best-effort `view_type`/`view_layout` carrier. NOTE: for a
/// `DmaRegion::<T>::write::<V, F>` instance this is the region's declared `T`, NOT
/// the method-level reinterpret view `V` (the generic args are `[T, V, F=closure]`
/// and `V` is only unambiguously recoverable by peeling the closure `F`'s rust-call
/// sig, which needs `TyCtxt`). So this M1 carrier is exact for the common `V = T`
/// case and a documented best-effort otherwise; the load-bearing C03 reinterpret
/// check reads `V` directly off the closure sig in the TyCtxt-aware M4/T19 verify
/// pass (`dma::layout::element_access_view_rustc`), NOT from this field.
fn first_type_arg_ty(model: &RegseqModel, callee: regseq_model::InstanceId) -> Option<TyId> {
    model.instances[callee]
        .generic_args
        .iter()
        .find_map(|a| match a {
            GenericArg::Type(t) => Some(*t),
            _ => None,
        })
}

/// Read a distinct view type `V`'s ABI layout (size/align) from the model type
/// table (dma-management FIX #2). `is_pod` is conservatively `false`: the model
/// carries no Drop info for the view at this site, and the POD-of-view obligation
/// is enforced by the M4/T19 per-access verify, not M1. Returns `None` for an
/// unsized marker (`Str`/`Slice`/`Dynamic`) so `size_align_of` is never asked to
/// panic on a bare unsized type.
fn view_layout_facts(model: &RegseqModel, view: TyId) -> Option<DmaLayoutFacts> {
    if matches!(
        &model.types[view],
        ModelTy::Str | ModelTy::Slice(_) | ModelTy::Dynamic { .. }
    ) {
        return None;
    }
    let sa = model.size_align_of(view);
    Some(DmaLayoutFacts {
        size: sa.size,
        align: sa.align,
        is_pod: false,
    })
}

/// The interned name of a model type (its nominal item's leaf name for an ADT, or
/// a rendered scalar name), used to record the view type `V`. `None` when the
/// type has no recoverable nominal name.
fn type_name(model: &RegseqModel, ty: TyId) -> Option<StringId> {
    match &model.types[ty] {
        ModelTy::Struct { item, .. }
        | ModelTy::Enum { item, .. }
        | ModelTy::Union { item, .. } => {
            let leaf = model.items[*item].def_path.segments.last()?;
            let regseq_model::DefPathData::TypeNs(name) = leaf.data else {
                return None;
            };
            Some(name)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// M4/T19 layout-verify helpers (thin `pub(crate)` re-exports of the private
// receiver/const/element-type resolvers so `dma::layout::verify_dma_layout` can
// reuse the exact recognition logic rather than re-deriving it).
// ---------------------------------------------------------------------------

/// Recover the `(driver item, region id)` a DMA element-access receiver names, for
/// the M4/T19 layout verify pass (a `pub(crate)` shim over the private
/// [`recover_dma_region_receiver`] so the verify pass reuses the SAME receiver
/// chase the recognition uses — no drift).
pub(crate) fn recover_region_for_layout(
    model: &RegseqModel,
    body: &Body,
    recv: &Operand,
) -> Option<(ItemId, StringId)> {
    recover_dma_region_receiver(model, body, recv)
}

/// Read a direct unsigned-integer constant operand (the static element index `i`)
/// for the M4/T19 layout verify pass (`pub(crate)` shim over [`const_uint_operand`]).
pub(crate) fn const_uint_for_layout(model: &RegseqModel, op: &Operand) -> Option<u64> {
    const_uint_operand(model, op)
}

/// The `DmaRegion<T>` element type `T` of a field type, for the M4/T19 layout
/// verify pass (`pub(crate)` shim over the private [`dma_region_element_ty`]).
pub(crate) fn dma_region_element_ty_pub<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    dma_region_element_ty(tcx, ty)
}

/// Read the `id = "…"` of a `#[rseq::dma_region(...)]` attribute, for the M4/T19
/// verify pass's region-id → declared-`T` map. Returns the id string, or `None`
/// when the attribute is absent/malformed (the verify pass then SKIPs that field).
pub(crate) fn dma_region_attr_id(attr: &Attribute) -> Option<String> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return None;
    };
    if delim.delim != Delimiter::Parenthesis {
        return None;
    }
    let mut iter = delim.tokens.iter().peekable();
    while iter.peek().is_some() {
        let Some(TokenTree::Token(key_tok, _)) = iter.next() else {
            return None;
        };
        let key = key_tok.ident().map(|(ident, _)| ident.name)?;
        match iter.next() {
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
            _ => return None,
        }
        let value = iter.next();
        if key.as_str() == "id" {
            return expect_str_lit(value).filter(|s| !s.trim().is_empty());
        }
        // Skip to the next `,`-separated key.
        match iter.peek() {
            None => break,
            Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Comma) => {
                iter.next();
            }
            _ => return None,
        }
    }
    None
}

/// Read a direct unsigned-integer constant operand (the element index `i`), or
/// `None` for a non-constant / non-integer operand. Mirrors the atomic bit-index
/// read.
fn const_uint_operand(model: &RegseqModel, op: &Operand) -> Option<u64> {
    let Operand::Constant(id) = op else {
        return None;
    };
    match model.consts.resolve(*id) {
        regseq_model::Const::Uint(_, value) => u64::try_from(*value).ok(),
        _ => None,
    }
}
