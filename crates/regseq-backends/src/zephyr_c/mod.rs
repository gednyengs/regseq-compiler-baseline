//! The zephyr_c backend: lowers a post-mono [`regseq_model::RegseqModel`]
//! directly to an idiomatic Zephyr driver translation unit plus a generated
//! devicetree binding yaml, under the ratified F-1 independent-lowering
//! contract: `generic_c` is the authoritative *reference* lowering and
//! frozen oracle, `linux_c` is a structural sibling — this module owns its
//! own emission shape and shares no code with either.
//!
//! Emission ordering is the ratified TU skeleton
//! (`docs/discussions/zephyr-backend/zephyr-scaffold-and-dispatch.md` §1):
//! `DT_DRV_COMPAT` pin → demand-driven log-module block → demand-driven
//! includes → config/data structs (`DEVICE_MMIO_ROM`/`RAM` FIRST members;
//! the whole spec driver embedded in `data`, A-17) → helper bodies → init
//! function → instantiation block (`DEVICE_DT_INST_DEFINE` under
//! `DT_INST_FOREACH_STATUS_OKAY`) → the mandatory single-instance
//! `BUILD_ASSERT` (A-2/D18). Zephyr's device model is the OS: no `main`,
//! no scenario functions — registration/match/create/associate are
//! build-time facts of the instantiation block (§2).
//!
//! First-light unit scope (backend-zephyr-c GRILL D12): the bring-up half
//! of the composed five-construct surface, devicetree-matched. Every legal
//! input shape with no Zephyr lowering contract is a loud [`EmitError`]
//! carrying its D.2-ledger disposition verbatim — never silent emission,
//! never silent omission. Structurally there is no emit-what-we-can branch
//! (I06).

mod body;
mod naming;
mod types;

#[cfg(test)]
mod mod_tests;

use regseq_model::{
    AttrData, BusType, ClassTag, ConfigOp, EnumShape, InstanceId, InstanceKind, InstanceRef,
    IntrinsicKind, LayoutKind, MatchEntry, RegseqModel, TagEncoding, TerminatorKind, Ty, TyId,
    UserDataKind,
};

use naming::{c_ident, compat_token};
use types::TypeNames;

/// The zephyr_c artifact set (GRILL D07): one Zephyr-idiomatic TU plus the
/// generated DT binding yaml — the only model-derived build collateral (the
/// Kconfig pins ride the TU as a header comment, A-9; the module shell is
/// eval-side integration input).
#[derive(Debug)]
pub struct ZephyrEmit {
    /// The `<driver>.c` translation unit.
    pub tu: String,
    /// The generated devicetree binding (`compatible:` from the match set —
    /// closing the compatible-drift channel, D07).
    pub binding_yaml: String,
    /// The sanitized driver name the artifacts are keyed by (A-14).
    pub driver_name: String,
}

/// zephyr_c emission errors. Every failure path is a loud, named diagnostic
/// (I06): D.2-ledger rejections carry their ratified wording, unit-scope
/// gaps name the delivering unit, and the shrinking-subset placeholders
/// name the construct; nothing silently truncates.
#[derive(Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A surface outside this unit's scope, with the unit that delivers it
    /// (GRILL D12/R04): e.g. the PCIe scaffold → `backend/zephyr-c-e1000`.
    UnsupportedInUnit {
        /// The unsupported path, precisely named.
        what: String,
        /// The unit scheduled to deliver it.
        unit: &'static str,
    },
    /// A construct outside the current lowered subset, named. Fail-loud
    /// placeholder that shrinks as the lowering grows (T06–T08).
    Unsupported(String),
    /// A D.2-ledger rejection: a legal input shape this backend
    /// deliberately has no Zephyr lowering contract for, with the ratified
    /// disposition wording (`zephyr-open-questions.md` §D.2).
    Rejected(String),
    /// The model's driver set cannot be lowered as a Zephyr driver TU (no
    /// driver, several drivers, malformed descriptor...).
    Driver(String),
    /// A TU containing MMIO ops whose binding is not resolved — the ratified
    /// A-10/D11 hard error naming the missing L2 literals (platform.toml
    /// stays REQUIRED even though DT `reg` is the normative map source).
    UnresolvedBinding(String),
    /// A marked `RegseqError` variant has no row in the C02 errno table —
    /// fail-loud at emit time, never a silent default arm.
    ErrnoMapping(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::UnsupportedInUnit { what, unit } => write!(
                f,
                "zephyr_c emission: {what} is not supported in this unit; it is delivered by {unit}"
            ),
            EmitError::Unsupported(what) => {
                write!(f, "zephyr_c emission: unsupported construct: {what}")
            }
            EmitError::Rejected(what) => write!(f, "zephyr_c emission: {what}"),
            EmitError::Driver(what) => write!(f, "zephyr_c emission: {what}"),
            EmitError::UnresolvedBinding(what) => write!(
                f,
                "zephyr_c emission: unresolved MMIO binding (ratified A-10/D11): {what}"
            ),
            EmitError::ErrnoMapping(variant) => write!(
                f,
                "zephyr_c emission: RegseqError variant `{variant}` has no C02 errno mapping"
            ),
        }
    }
}

impl std::error::Error for EmitError {}

/// Lower `model` to a Zephyr driver TU + generated DT binding yaml.
///
/// Mirrors the `emit_generic_c`/`emit_linux_c` function-API family: the
/// function call *is* backend selection (S16 negative space: no trait, no
/// registry, no CLI); variants (`_with`, `_for_entry`) are added only when
/// a consumer actually needs them.
pub fn emit_zephyr_c(model: &RegseqModel) -> Result<ZephyrEmit, EmitError> {
    // -- the driver (exactly one for a single-TU emission) ------------------
    let mut drivers = model.device_lifecycle.drivers.iter();
    let (driver_item, driver) = drivers.next().ok_or_else(|| {
        EmitError::Driver(
            "no recognized device-lifecycle driver in the model; a Zephyr driver TU needs \
             exactly one `#[rseq::driver]` (the A-8 descriptor-less fallback is dormant in \
             this unit)"
                .into(),
        )
    })?;
    if drivers.next().is_some() {
        return Err(EmitError::Driver(
            "more than one recognized driver in the model; one Zephyr driver TU per driver".into(),
        ));
    }

    // -- unit scope gates (GRILL D12; R04) -----------------------------------
    // Bus selector: the DT path (Platform, Amba folding into it per
    // per-construct §B.2, or no selector) is this unit's scope; PCIe is the
    // e1000 successor's.
    match &driver.descriptor.bus_type {
        Some(BusType::Pci) => {
            return Err(EmitError::UnsupportedInUnit {
                what: "the PCIe scaffold (bus = \"pci\": BDF probe, BAR map, config space)".into(),
                unit: "backend/zephyr-c-e1000",
            })
        }
        Some(BusType::Platform) | Some(BusType::Amba) | None => {}
        Some(other) => {
            return Err(EmitError::UnsupportedInUnit {
                what: format!("the `{other:?}` bus scaffold"),
                unit: "a post-first-light unit",
            })
        }
    }

    // Device class: class-neutral scaffold only (A-16/B-9) — silently
    // passing `NULL` api for a declared class would hide it from its
    // class-API consumers.
    if driver.descriptor.device_class != ClassTag::Generic {
        return Err(EmitError::Rejected(format!(
            "non-Generic device class {:?}: the class selects the Zephyr macro family + api \
             vtable (D.2/B-9); the class-neutral scaffold passes NULL api for \
             ClassTag::Generic ONLY (A-16) — class wrappers are the Device Class \
             construct's territory",
            driver.descriptor.device_class
        )));
    }

    // Match set: devicetree-compatible matching is the only Zephyr match
    // target (B-5).
    if driver.descriptor.match_set.is_empty() {
        return Err(EmitError::Driver(
            "driver has an empty match set; a Zephyr DT instance needs a `compatible` entry".into(),
        ));
    }
    let dt_compatibles: Vec<&str> = driver
        .descriptor
        .match_set
        .iter()
        .filter_map(|entry| match entry {
            MatchEntry::Devicetree { compatible } => Some(model.strings.resolve(*compatible)),
            _ => None,
        })
        .collect();
    let non_dt = driver
        .descriptor
        .match_set
        .iter()
        .find(|entry| !matches!(entry, MatchEntry::Devicetree { .. }));
    if let Some(entry) = non_dt {
        if dt_compatibles.is_empty() {
            // The Amba-only (or otherwise DT-less) set: reject NAMING THE FIX
            // (A-13) — the PL011 precedent is an AMBA part matched by
            // compatible string.
            return Err(EmitError::Rejected(format!(
                "match set has no Devicetree entry (first non-DT entry: {entry:?}): no \
                 AMBA/periphid match target exists on Zephyr (D.2/B-5); add a companion \
                 `Devicetree {{ compatible }}` entry (A-13: the PL011 precedent — AMBA part, \
                 compatible-string match)"
            )));
        }
        // A mixed native+foreign-transport set: the consumption rule is
        // unratified (GRILL D03 precondition (a)) — defer loudly to the
        // convergence successor rather than silently ignoring entries.
        return Err(EmitError::Rejected(format!(
            "mixed match-set transport ({entry:?} alongside Devicetree entries): the \
             foreign-transport consumption rule is unratified (GRILL D03a); delivered by \
             spec-convergence-pl011"
        )));
    }
    if dt_compatibles.len() > 1 {
        return Err(EmitError::Unsupported(
            "multi-compatible match sets (scaffold §1 repeat-blocks idiom) are not exercised \
             by this unit; one Devicetree entry per driver"
                .into(),
        ));
    }
    let compatible = dt_compatibles[0];

    // Lifecycle ops: the bring-up half is this unit's scope (D12).
    if driver.ops.on_deinit.is_some() {
        return Err(EmitError::Rejected(
            "authored `on_deinit` under TargetOs::Zephyr: the teardown slot exists on v4.4 \
             (`device_deinit()`, v4.2.0+) but is unverified/undesigned (D.2/B-6, C-3, D06) — \
             delivered by spec-convergence-pl011"
                .into(),
        ));
    }
    if driver.ops.on_surprise_remove.is_some() {
        return Err(EmitError::Rejected(
            "authored `on_surprise_remove` under TargetOs::Zephyr: no hotplug dispatch exists \
             for DT devices — the surprise scenario may not be representable on this target \
             (D.2/B-7; A-12 recommends exactly this diagnostic)"
                .into(),
        ));
    }
    if driver.ops.on_system_shutdown.is_some() {
        // NOT a capability gap: the PM-device slot is witnessed and the
        // dispatch wrapper is designed (scaffold §4); its emission is
        // scheduled behind the C-4 trigger-contract pin per A-18.
        return Err(EmitError::Rejected(
            "authored `on_system_shutdown`: the PM-device action wrapper is designed and \
             scheduled per A-18 (slot witnessed, scaffold §4; trigger contract pending the \
             C-4 pin) — not emitted by this unit"
                .into(),
        ));
    }

    // Resource regions: the TU-static base carrier admits exactly one
    // resolved region (B-10 — aligned with the non-named DEVICE_MMIO API).
    let resolved_regions = driver
        .handle
        .resource_fields
        .iter()
        .filter(|f| f.binding.is_resolved())
        .count();
    if resolved_regions > 1 {
        return Err(EmitError::Rejected(format!(
            "{resolved_regions} resolved MMIO regions on one driver: multi-region needs \
             DEVICE_MMIO_NAMED_* and a model extension threading per-access region bases \
             (D.2/B-10); single-region only"
        )));
    }
    for field in &driver.handle.resource_fields {
        if field.binding.bar_type == regseq_model::BarType::Io {
            return Err(EmitError::Rejected(format!(
                "port-I/O region `{}` (type = \"io\"): recognized-not-lowered upstream; no \
                 invented sys_in*/sys_out* surface (D.2/B-2)",
                model.strings.resolve(field.name)
            )));
        }
    }

    // User data: only the inherited all-zero-image shapes pass through
    // (B-8) — the static `data` image is zero-filled, which IS the init for
    // Rule-1 (zero-init) and for Rule-2 (`Option<T: Copy>`) exactly when the
    // all-zero bit pattern decodes to `None`. Anything beyond that
    // guarantee is rejected, not invented.
    for field in &driver.handle.user_data_fields {
        match field.kind {
            UserDataKind::ZeroInit => {}
            UserDataKind::OptionCopy => {
                if !zero_image_is_none(model, field.ty) {
                    return Err(EmitError::Rejected(format!(
                        "user-data field `{}`: Option<T: Copy> whose all-zero bit pattern \
                         does not decode to `None` — beyond the inherited zero-image \
                         guarantee (D.2/B-8); no per-kind lowering is invented",
                        model.strings.resolve(field.name)
                    )));
                }
            }
        }
    }

    // The literal `is_resolved()` preamble gate (ratified A-10/D11): DT
    // `reg` is the normative map source on Zephyr, but platform.toml stays
    // REQUIRED — its literals are the resolution trigger AND the emitted A-1
    // cross-check's right-hand side. An unresolved binding is a hard
    // emit-time error, never a silently-skipped preamble.
    for field in &driver.handle.resource_fields {
        if !field.binding.is_resolved() {
            let fname = c_ident(model.strings.resolve(field.name));
            let join_id = field
                .binding
                .id
                .map(|id| model.strings.resolve(id).to_string())
                .unwrap_or_else(|| fname.clone());
            return Err(EmitError::UnresolvedBinding(format!(
                "resource `{fname}` has no resolved (base, size): the platform file must \
                 supply `[mmio.{join_id}] base/size` (the L2 literals are the resolution \
                 trigger, ratified A-10; D11 consumes them in the emitted BUILD_ASSERT \
                 cross-check)"
            )));
        }
    }

    let drv = c_ident(model.strings.resolve(driver.descriptor.name));

    // -- the driver struct type ----------------------------------------------
    let driver_ty = driver_struct_ty(model, *driver_item).ok_or_else(|| {
        EmitError::Driver("driver struct type not present in the type interner".into())
    })?;

    // -- callback roots + reachable closure ---------------------------------
    let callbacks = [
        ("on_init", &driver.ops.on_init),
        ("on_init_failure", &driver.ops.on_init_failure),
    ];
    let mut roots: Vec<(&'static str, InstanceId)> = Vec::new();
    for (name, op) in callbacks {
        if let Some(r) = op {
            let id = resolve_instance_ref(model, *r).ok_or_else(|| {
                EmitError::Driver(format!("callback `{name}` does not resolve to an instance"))
            })?;
            roots.push((name, id));
        }
    }
    let closure = reachable_closure(model, roots.iter().map(|(_, id)| *id));

    // -- frame scan over the closure: op-shape rejections that must fire
    //    regardless of how far the body banks have grown (D12 frame) --------
    scan_closure_ops(model, &closure)?;

    // -- the lowering context (T06 bank consumer) ----------------------------
    let resources: Vec<(String, bool, String)> = driver
        .handle
        .resource_fields
        .iter()
        .map(|f| {
            let c_name = c_ident(model.strings.resolve(f.name));
            let join_id = f
                .binding
                .id
                .map(|id| model.strings.resolve(id).to_string())
                .unwrap_or_else(|| c_name.clone());
            (c_name, f.binding.is_resolved(), join_id)
        })
        .collect();
    let resource_c_names: Vec<String> = resources.iter().map(|(n, _, _)| n.clone()).collect();
    let reg_macros = register_macros(model, driver_ty, &drv, &resource_c_names);
    let base_symbol = format!("{drv}_mmio_base");
    let has_region = !driver.handle.resource_fields.is_empty();
    let mut cx = body::LowerCx {
        model,
        names: TypeNames::new(),
        fn_symbols: std::collections::BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: body::DriverFacts {
            driver_ty,
            resources,
            reg_macros,
            base_symbol: base_symbol.clone(),
        },
        width8_used: false,
        uses_log: false,
        elided_locals: std::collections::BTreeSet::new(),
    };

    // Symbols: `<drv>_<leaf>` with deterministic clash suffixes, callbacks
    // named first so they always get the bare leaf (A-14 prefix policy).
    for (name, id) in &roots {
        cx.fn_symbols.insert(*id, format!("{drv}_{name}"));
    }
    for id in &closure {
        if cx.fn_symbols.contains_key(id) {
            continue;
        }
        let leaf = naming::path_leaf(model.strings.resolve(model.instances[*id].emission_name));
        let base = format!("{drv}_{}", c_ident(leaf));
        let mut sym = base.clone();
        let mut n = 1;
        while cx.fn_symbols.values().any(|v| v == &sym) {
            n += 1;
            sym = format!("{base}_{n}");
        }
        cx.fn_symbols.insert(*id, sym);
    }

    // -- lowered bodies (scaffold §1 item 5): forward prototypes for every
    //    lowered function first (callees may be emitted after their
    //    callers), then the definitions.
    let with_fn_context = |err: EmitError, model: &RegseqModel, id: InstanceId| match err {
        EmitError::Unsupported(s) => EmitError::Unsupported(format!(
            "{s} [while lowering `{}`]",
            model.strings.resolve(model.instances[id].emission_name)
        )),
        other => other,
    };
    let mut bodies = String::new();
    for id in &closure {
        let head = body::fn_head(&mut cx, *id).map_err(|e| with_fn_context(e, model, *id))?;
        bodies.push_str(&head);
        bodies.push_str(";\n");
    }
    if !closure.is_empty() {
        bodies.push('\n');
    }
    for id in &closure {
        let f = body::emit_fn(&mut cx, *id).map_err(|e| with_fn_context(e, model, *id))?;
        bodies.push_str(&f);
        bodies.push('\n');
    }

    // -- the init function (scaffold §3) + the fused errno adapter ----------
    let find = |name: &str| roots.iter().find(|(n, _)| *n == name).map(|(_, id)| *id);
    let init_fn = emit_init_fn(
        &mut cx,
        &drv,
        &base_symbol,
        has_region,
        find("on_init"),
        find("on_init_failure"),
    )?;

    // -- type definitions, now that every use is known -----------------------
    let type_defs = emit_type_defs(&mut cx, driver_ty)?;

    // -- TU assembly, scaffold §1 top to bottom ------------------------------
    let resolved_region = driver
        .handle
        .resource_fields
        .first()
        .map(|f| (f.binding.base.unwrap(), f.binding.size.unwrap()));
    let tu = emit_tu(
        &mut cx,
        &drv,
        compatible,
        driver_ty,
        &base_symbol,
        has_region,
        resolved_region,
        &type_defs,
        &bodies,
        &init_fn,
    )?;
    let binding_yaml = emit_binding_yaml(model, &drv, compatible, driver);

    Ok(ZephyrEmit {
        tu,
        binding_yaml,
        driver_name: drv,
    })
}

/// The C02 per-target errno table, name-keyed (symbol-identical on Zephyr
/// per the ratified A-10 recommendation — `EOPNOTSUPP` is public in Zephyr
/// libc; the negation polarity is applied at the adapter's emission site).
fn errno_symbol(variant_name: &str) -> Option<&'static str> {
    Some(match variant_name {
        "InvalidParameter" => "EINVAL",
        "OutOfMemory" => "ENOMEM",
        "HardwareError" => "EIO",
        "Timeout" => "ETIMEDOUT",
        "Unsupported" => "EOPNOTSUPP",
        "DeviceBusy" => "EBUSY",
        "PermissionDenied" => "EPERM",
        "DeviceNotFound" => "ENODEV",
        "GenericFailure" => "EIO",
        _ => return None,
    })
}

/// Synthesize the per-instance init function (scaffold §3): the one runtime
/// dispatch hub — resource preamble (claim_regions = ZERO calls, the
/// ratified elision; `DEVICE_MMIO_MAP` + the TU-static base install, A-2),
/// then the authored `on_init` through the fused negated-errno adapter with
/// the `on_init_failure` cleanup slot sequenced BEFORE the error return.
///
/// I10 is enforced structurally here: an authored `RegseqError` crosses the
/// adapter at this boundary ONLY; generated preamble code has no fallible
/// native calls in this unit (the DT map cannot fail at this layer), so no
/// native status exists to conflate.
fn emit_init_fn(
    cx: &mut body::LowerCx<'_>,
    drv: &str,
    base_symbol: &str,
    has_region: bool,
    on_init: Option<InstanceId>,
    on_init_failure: Option<InstanceId>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* the one runtime dispatch hub (scaffold §3) */\nstatic int {drv}_init(const struct device *dev)\n{{\n"
    ));
    let uses_data = on_init.is_some() || on_init_failure.is_some();
    if uses_data {
        out.push_str(&format!("\tstruct {drv}_data *data = dev->data;\n\n"));
    }
    if has_region {
        out.push_str(
            "\t/* resource preamble (scaffold §3): claim_regions = zero calls (the\n\t * ratified elision — the DT `reg` capture happened at build time);\n\t * the canonical map slot precedes every MMIO op */\n",
        );
        out.push_str("\tDEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);\n");
        out.push_str(&format!(
            "\t{base_symbol} = DEVICE_MMIO_GET(dev); /* TU-static base (A-2) */\n\n"
        ));
    } else if !uses_data {
        out.push_str("\tARG_UNUSED(dev);\n");
    }
    if let Some(init_id) = on_init {
        let sym = cx.fn_symbols[&init_id].clone();
        let failure_hook = on_init_failure.map(|fid| {
            format!(
                "\t\t\t/* authored on_init -> Err: the failure hook IS the cleanup\n\t\t\t * slot, sequenced before the error return (D12); its one and\n\t\t\t * only call site */\n\t\t\t(void){}(&data->inner);\n",
                cx.fn_symbols[&fid]
            )
        });
        out.push_str(
            "\t/* authored on_init over the embedded spec driver; an authored\n\t * RegseqError crosses the negated-errno adapter HERE and only here\n\t * (I10: native status — none exists in this preamble — would\n\t * propagate directly, never wrapped) */\n",
        );
        let adapter = emit_result_adapter(
            cx,
            init_id,
            &format!("{sym}(&data->inner)"),
            failure_hook.as_deref(),
        )?;
        out.push_str(&adapter);
    }
    out.push_str("\treturn 0;\n}\n");
    Ok(out)
}

/// The RegseqError → negated-errno boundary adapter, fused into the init
/// fn's return path (the nine C02 rows, symbol-identical, NEGATED — Zephyr
/// init contract: 0 on success, -errno on failure, recorded positive in
/// `init_res` and gating `device_is_ready()`, C-5 verified on v4.4.0).
fn emit_result_adapter(
    cx: &mut body::LowerCx<'_>,
    callee: InstanceId,
    call_expr: &str,
    failure_hook: Option<&str>,
) -> Result<String, EmitError> {
    let model = cx.model;
    let InstanceKind::Function(data) = &model.instances[callee].kind else {
        return Err(EmitError::Driver("callback is not a function".into()));
    };
    let result_ty = model.sigs.resolve(data.fn_sig).output;
    let facts = result_carrier_facts(model, result_ty)?;
    let result_c = types::c_type(model, &mut cx.names, result_ty)?;
    cx.extra_includes.insert("<errno.h>");

    let mut out = String::new();
    out.push_str(&format!("\t{{\n\t\t{result_c} __r = {call_expr};\n"));
    // Read the Result tag cell.
    out.push_str(&read_scalar(
        "__tag",
        "__r",
        facts.result_tag_offset,
        facts.result_tag_size,
        &mut cx.extra_includes,
    ));
    out.push_str(&format!("\t\tif (__tag != {:#x}) {{\n", facts.ok_tag_value));
    if let Some(hook) = failure_hook {
        out.push_str(hook);
    }
    // The Err payload is the marked RegseqError; read its tag and map every
    // variant through the C02 table, negated, in DETERMINISTIC variant
    // order. `default` is corrupt-discriminant defense only (a known-but-
    // unmapped variant is the fail-loud ErrnoMapping above, never a silent
    // EIO).
    out.push_str(&read_scalar(
        "__err",
        "__r",
        facts.err_tag_offset,
        facts.err_tag_size,
        &mut cx.extra_includes,
    ));
    out.push_str("\t\t\tswitch (__err) {\n");
    for (tag, symbol) in &facts.errno_arms {
        out.push_str(&format!(
            "\t\t\tcase {tag:#x}:\n\t\t\t\treturn -{symbol};\n"
        ));
    }
    out.push_str("\t\t\tdefault:\n\t\t\t\treturn -EIO;\n\t\t\t}\n\t\t}\n\t}\n");
    Ok(out)
}

/// `uint8_t`-exact scalar read out of an opaque carrier: emits a declaration
/// of `name` loaded from `<carrier>._raw[offset..offset+size]`.
fn read_scalar(
    name: &str,
    carrier: &str,
    offset: u64,
    size: u64,
    includes: &mut std::collections::BTreeSet<&'static str>,
) -> String {
    if size > 1 {
        includes.insert("<string.h>");
    }
    match size {
        1 => format!("\t\tuint8_t {name} = {carrier}._raw[{offset}];\n"),
        2 => format!("\t\tuint16_t {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 2);\n"),
        4 => format!("\t\tuint32_t {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 4);\n"),
        _ => format!("\t\tuint64_t {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 8);\n"),
    }
}

/// The layout facts of a callback's `Result<(), RegseqError>` carrier: where
/// the Result tag lives, the Ok encoding, where the Err payload's own tag
/// lives, and the per-variant negated-errno arms.
struct CarrierFacts {
    result_tag_offset: u64,
    result_tag_size: u64,
    ok_tag_value: u64,
    err_tag_offset: u64,
    err_tag_size: u64,
    /// `(E tag value, errno symbol)` per RegseqError variant, variant order
    /// (the deterministic arm order R03 locks).
    errno_arms: Vec<(u64, &'static str)>,
}

fn result_carrier_facts(model: &RegseqModel, result_ty: TyId) -> Result<CarrierFacts, EmitError> {
    let layout = types::layout_of(model, result_ty).ok_or_else(|| {
        EmitError::Driver("callback return type is not an aggregate Result".into())
    })?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(EmitError::Driver(
            "callback return type is not a multi-variant enum (expected Result<(), RegseqError>)"
                .into(),
        ));
    };
    let (result_tag_offset, result_tag_size) = types::enum_tag_cell(model, result_ty)?;

    // Certification: the carrier item must be the certified core Result, and
    // Ok's payload must be exactly unit/ZST.
    let result_item = match &model.types[result_ty] {
        Ty::Enum { item, .. } => *item,
        other => {
            return Err(EmitError::Driver(format!(
                "callback return type is not a nominal enum: {other:?}"
            )))
        }
    };
    if !model.items[result_item].is_result {
        return Err(EmitError::Driver(
            "callback return enum is not the certified core Result (item.is_result unset)".into(),
        ));
    }
    let mut ok_tag: Option<u64> = None;
    let mut err_field: Option<(u64, TyId)> = None;
    for v in variants {
        let name = model.strings.resolve(v.name);
        match name {
            "Ok" => {
                for f in &v.fields {
                    if !types::is_zst(model, f.ty) {
                        return Err(EmitError::Driver(
                            "callback Result Ok payload is not unit (expected Result<(), \
                             RegseqError>)"
                                .into(),
                        ));
                    }
                }
                let Some(tag) = v.tag_value else {
                    return Err(EmitError::Unsupported(
                        "Result whose Ok variant is the untagged niche (first-light subset)".into(),
                    ));
                };
                ok_tag = Some(tag as u64);
            }
            "Err" => {
                let field = v
                    .fields
                    .first()
                    .ok_or_else(|| EmitError::Driver("Err variant with no payload field".into()))?;
                err_field = Some((field.offset, field.ty));
            }
            other => {
                return Err(EmitError::Driver(format!(
                    "callback return enum has unexpected variant `{other}` \
                     (expected Result<(), RegseqError>)"
                )))
            }
        }
    }
    let ok_tag_value =
        ok_tag.ok_or_else(|| EmitError::Driver("Result has no Ok variant".into()))?;
    let (err_payload_offset, err_ty) =
        err_field.ok_or_else(|| EmitError::Driver("Result has no Err variant".into()))?;

    // The payload must be the marked RegseqError framework type.
    let err_item = match &model.types[err_ty] {
        Ty::Enum { item, .. } => *item,
        other => {
            return Err(EmitError::Driver(format!(
                "Err payload is not an enum: {other:?}"
            )))
        }
    };
    let marked = (err_item.as_u32() as usize) < model.domain_attrs.item_attrs.len()
        && model.domain_attrs.item_attrs[err_item].iter().any(|a| {
            matches!(
                a,
                AttrData::FrameworkType(regseq_model::FrameworkType::RegseqError)
            )
        });
    if !marked {
        return Err(EmitError::Driver(
            "Err payload enum is not the marked RegseqError framework type".into(),
        ));
    }

    let (err_tag_off_in_e, err_tag_size) = types::enum_tag_cell(model, err_ty)?;
    let err_layout = types::layout_of(model, err_ty).expect("enum layout");
    let LayoutKind::Enum {
        shape: EnumShape::Multiple {
            variants: err_variants,
            ..
        },
    } = &err_layout.kind
    else {
        return Err(EmitError::Driver("RegseqError is not multi-variant".into()));
    };
    let mut errno_arms = Vec::new();
    for v in err_variants {
        let name = model.strings.resolve(v.name);
        let symbol = errno_symbol(name).ok_or_else(|| EmitError::ErrnoMapping(name.into()))?;
        let Some(tag) = v.tag_value else {
            return Err(EmitError::Unsupported(
                "RegseqError variant without a direct tag (first-light subset)".into(),
            ));
        };
        errno_arms.push((tag as u64, symbol));
    }

    Ok(CarrierFacts {
        result_tag_offset,
        result_tag_size,
        ok_tag_value,
        err_tag_offset: err_payload_offset + err_tag_off_in_e,
        err_tag_size,
        errno_arms,
    })
}

/// Emit every named aggregate's definition, dependency-ordered (by-value
/// struct containment first). The driver struct's resource fields are
/// substituted with their dead-weight zephyr carrier (`mm_reg_t`, A-2).
fn emit_type_defs(cx: &mut body::LowerCx<'_>, driver_ty: TyId) -> Result<String, EmitError> {
    let model = cx.model;
    // The driver struct is always defined (the data struct embeds it, A-17).
    cx.names.name_of(model, driver_ty)?;
    let mut emitted: Vec<TyId> = Vec::new();
    let mut out_chunks: std::collections::BTreeMap<TyId, String> =
        std::collections::BTreeMap::new();
    loop {
        let pending: Vec<TyId> = cx
            .names
            .iter()
            .map(|(ty, _)| *ty)
            .filter(|ty| !emitted.contains(ty))
            .collect();
        if pending.is_empty() {
            break;
        }
        for ty in pending {
            let name_for_err = cx.names.name_of(model, ty)?;
            let with_ctx = |err: EmitError| match err {
                EmitError::Unsupported(s) => {
                    EmitError::Unsupported(format!("{s} [while defining `struct {name_for_err}`]"))
                }
                other => other,
            };
            let chunk = if ty == driver_ty {
                let rn: Vec<String> = cx
                    .driver
                    .resources
                    .iter()
                    .map(|(n, _, _)| n.clone())
                    .collect();
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    if rn.iter().any(|r| r == fname) {
                        Some(format!(
                            "mm_reg_t {fname}; /* dead under the TU-static base carrier (A-2) */"
                        ))
                    } else {
                        None
                    }
                })
                .map_err(with_ctx)?
            } else {
                types::emit_aggregate_def(model, &mut cx.names, ty, &|_| None).map_err(with_ctx)?
            };
            out_chunks.insert(ty, chunk);
            emitted.push(ty);
        }
    }

    // Dependency order: emit in topological order of by-value containment.
    let mut ordered: Vec<TyId> = Vec::new();
    let keys: Vec<TyId> = out_chunks.keys().copied().collect();
    fn visit(
        model: &RegseqModel,
        ty: TyId,
        all: &[TyId],
        ordered: &mut Vec<TyId>,
        visiting: &mut Vec<TyId>,
    ) -> Result<(), EmitError> {
        if ordered.contains(&ty) {
            return Ok(());
        }
        if visiting.contains(&ty) {
            return Err(EmitError::Driver(
                "by-value containment cycle in lowered types (model corruption)".into(),
            ));
        }
        visiting.push(ty);
        if let Some(layout) = types::layout_of(model, ty) {
            if let LayoutKind::Struct { fields } = &layout.kind {
                for f in fields {
                    if all.contains(&f.ty) {
                        visit(model, f.ty, all, ordered, visiting)?;
                    }
                }
            }
        }
        visiting.pop();
        ordered.push(ty);
        Ok(())
    }
    for ty in &keys {
        visit(model, *ty, &keys, &mut ordered, &mut Vec::new())?;
    }

    let mut out = String::new();
    for ty in ordered {
        out.push_str(&out_chunks[&ty]);
        out.push('\n');
    }
    Ok(out)
}

/// The const bank: `(macro, offset)` per named register of the single
/// resource block, recovered from the `#[repr(C)]` block layout.
/// Underscore-named spacer fields are skipped.
fn register_macros(
    model: &RegseqModel,
    driver_ty: TyId,
    drv: &str,
    resource_c_names: &[String],
) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let Some(layout) = types::layout_of(model, driver_ty) else {
        return out;
    };
    let LayoutKind::Struct { fields } = &layout.kind else {
        return out;
    };
    let prefix = drv.to_uppercase();
    for f in fields {
        if !resource_c_names
            .iter()
            .any(|n| *n == c_ident(model.strings.resolve(f.name)))
        {
            continue;
        }
        // A resource field's spec type is `Mmio<Block>`: one `inner` field
        // whose type is the register block.
        let Some(mmio_layout) = types::layout_of(model, f.ty) else {
            continue;
        };
        let LayoutKind::Struct {
            fields: mmio_fields,
        } = &mmio_layout.kind
        else {
            continue;
        };
        let [inner] = &mmio_fields[..] else { continue };
        let Some(block_layout) = types::layout_of(model, inner.ty) else {
            continue;
        };
        let LayoutKind::Struct {
            fields: block_fields,
        } = &block_layout.kind
        else {
            continue;
        };
        for reg in block_fields {
            let name = model.strings.resolve(reg.name);
            if name.starts_with('_') {
                continue;
            }
            out.push((
                format!("{prefix}_{}", c_ident(name).to_uppercase()),
                reg.offset,
            ));
        }
    }
    out.sort_by_key(|(_, off)| *off);
    out
}

/// Whether the all-zero bit pattern of an `Option`-shaped enum decodes to
/// `None` — the inherited zero-image guarantee Rule-2 user data rests on
/// (B-8). Conservative: any shape this cannot positively confirm is `false`
/// (the caller rejects loud).
fn zero_image_is_none(model: &RegseqModel, ty: TyId) -> bool {
    let Some(layout) = types::layout_of(model, ty) else {
        return false;
    };
    let LayoutKind::Enum { shape } = &layout.kind else {
        return false;
    };
    let EnumShape::Multiple {
        encoding, variants, ..
    } = shape
    else {
        return false;
    };
    // The `None` variant is the fieldless one.
    let Some(none) = variants.iter().find(|v| v.fields.is_empty()) else {
        return false;
    };
    match encoding {
        // Direct tag: zero image selects `None` iff its tag bytes are zero.
        TagEncoding::Direct => none.tag_value == Some(0),
        // Niche: `None` is niched (it has no payload to be the tag); the
        // zero image decodes to it iff its niche tag value is zero (e.g.
        // Option<NonZeroU32>; counterexample Option<bool>, niche_start 2).
        TagEncoding::Niche { .. } => none.tag_value == Some(0),
    }
}

/// First `Ty::Struct` whose item is the driver item.
fn driver_struct_ty(model: &RegseqModel, driver_item: regseq_model::ItemId) -> Option<TyId> {
    model
        .types
        .iter()
        .find(|(_, ty)| matches!(ty, Ty::Struct { item, .. } if *item == driver_item))
        .map(|(id, _)| id)
}

/// O(N) scan resolving a callback `InstanceRef` to its `InstanceId`.
fn resolve_instance_ref(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| model.instance_ref_of(*id) == target)
}

/// BFS over `Call(Direct)` edges from the callback roots. Intrinsic-anchored
/// callees do not join the closure (they are intercepted at emission), but
/// [`scan_closure_ops`] inspects them from the caller side. Deterministic
/// order: roots first, then discovery order.
fn reachable_closure(
    model: &RegseqModel,
    roots: impl Iterator<Item = InstanceId>,
) -> Vec<InstanceId> {
    let mut seen: Vec<InstanceId> = Vec::new();
    let mut queue: Vec<InstanceId> = roots.collect();
    while let Some(id) = queue.pop() {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let InstanceKind::Function(data) = &model.instances[id].kind else {
            continue;
        };
        let Some(b) = &data.body else { continue };
        for bb in b.basic_blocks.iter() {
            if let TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                ..
            } = &bb.terminator.kind
            {
                if intrinsic_of(model, *callee).is_some() {
                    continue;
                }
                if !seen.contains(callee) {
                    queue.push(*callee);
                }
            }
        }
    }
    seen
}

/// The intrinsic kind anchoring `instance`, if any.
fn intrinsic_of(model: &RegseqModel, instance: InstanceId) -> Option<&IntrinsicKind> {
    let item = model.instances[instance].item;
    if item.as_u32() as usize >= model.domain_attrs.item_attrs.len() {
        return None;
    }
    model.domain_attrs.item_attrs[item]
        .iter()
        .find_map(|attr| match attr {
            AttrData::Intrinsic(kind) => Some(kind),
            _ => None,
        })
}

/// The D12 frame scan: walk every `Call(Direct)` site reachable from the
/// lifecycle roots and reject the op shapes the D.2 ledger has no lowering
/// contract for — `poll_until` (B-1) and the config-space shapes (B-3 width
/// gaps; B-4/BRM-001 non-PCI access). Runs BEFORE body lowering so the
/// rejections hold however far the T06–T08 banks have grown.
fn scan_closure_ops(model: &RegseqModel, closure: &[InstanceId]) -> Result<(), EmitError> {
    for id in closure {
        let InstanceKind::Function(data) = &model.instances[*id].kind else {
            continue;
        };
        let Some(b) = &data.body else { continue };
        for bb in b.basic_blocks.iter() {
            let TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                ..
            } = &bb.terminator.kind
            else {
                continue;
            };
            match intrinsic_of(model, *callee) {
                Some(IntrinsicKind::PollUntil) => {
                    return Err(EmitError::Rejected(
                        "`poll_until` is recognized-not-lowered (D.2/B-1, parity with \
                         generic_c's as-built disposition; convergence §9 forbids claiming \
                         it) — no WAIT_FOR lowering is invented"
                            .into(),
                    ));
                }
                Some(IntrinsicKind::Config(op)) => {
                    return Err(config_rejection(model, *callee, op));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The D.2 disposition for a config-space call shape: width-8 (both ops)
/// and sub-word writes are the B-3 undesigned gaps; the remaining specified
/// shapes still have no carrier here because config space is PCI-only and
/// the PCIe scaffold is the e1000 successor's (the upstream BRM-001
/// non-PCI rejection diagnostic is pending, D.2/B-4).
pub(crate) fn config_rejection(
    model: &RegseqModel,
    callee: InstanceId,
    op: &ConfigOp,
) -> EmitError {
    let width = config_width(model, callee, op);
    let is_write = matches!(op, ConfigOp::Write);
    match width {
        Some(8) => EmitError::Rejected(format!(
            "config-space {} width 8: the two-word split is undesigned (D.2/B-3)",
            if is_write { "write" } else { "read" }
        )),
        Some(w @ (1 | 2)) if is_write => EmitError::Rejected(format!(
            "config-space write width {w}: the containing-word RMW is observable on the bus \
             and undesigned (D.2/B-3); width-1/2 reads and width-4 are the specified shapes"
        )),
        _ => EmitError::Rejected(
            "config-space access on a non-PCI-like bus: no carrier in this unit (the \
             upstream BRM-001 rejection diagnostic is pending, D.2/B-4); the PCIe path is \
             delivered by backend/zephyr-c-e1000"
                .into(),
        ),
    }
}

/// The access width (bytes) of a config intrinsic: the value type — the
/// return for a read, the last parameter for a write.
fn config_width(model: &RegseqModel, callee: InstanceId, op: &ConfigOp) -> Option<u64> {
    let InstanceKind::Function(data) = &model.instances[callee].kind else {
        return None;
    };
    let sig = model.sigs.resolve(data.fn_sig);
    let value_ty = match op {
        ConfigOp::Read => sig.output,
        ConfigOp::Write => *sig.inputs.last()?,
    };
    match &model.types[value_ty] {
        Ty::Uint(k) => match k {
            regseq_model::UintKind::U8 => Some(1),
            regseq_model::UintKind::U16 => Some(2),
            regseq_model::UintKind::U32 => Some(4),
            regseq_model::UintKind::U64 => Some(8),
            _ => None,
        },
        _ => None,
    }
}

/// Assemble the TU per the scaffold §1 skeleton, top to bottom.
#[allow(clippy::too_many_arguments)]
fn emit_tu(
    cx: &mut body::LowerCx<'_>,
    drv: &str,
    compatible: &str,
    driver_ty: TyId,
    base_symbol: &str,
    has_region: bool,
    resolved_region: Option<(u64, u64)>,
    type_defs: &str,
    bodies: &str,
    init_fn: &str,
) -> Result<String, EmitError> {
    let model = cx.model;
    let inner_tag = cx.names.name_of(model, driver_ty)?;
    let log_demand = cx.uses_log;

    let mut tu = String::new();
    // Header: provenance + the hardcoded init placement (D13) + the A-9
    // Kconfig pins (documented in generated collateral; asserting them is
    // the build harness's job) when the log block is present.
    tu.push_str(&format!(
        "/*\n * {drv}.c - Regseq-generated Zephyr driver (devicetree match).\n *\n * Generated by the regseq zephyr_c backend; do not edit.\n *\n * Init placement: POST_KERNEL / CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\n * hardcoded (ratified D13: no BackendOptions knob this unit).\n"
    ));
    if log_demand {
        tu.push_str(
            " *\n * Logging Kconfig pins (A-9, documented not asserted): CONFIG_LOG=y,\n * deferred mode, CONFIG_CBPRINTF_COMPLETE, CONFIG_CBPRINTF_FULL_INTEGRAL\n * (for %lld/%llu).\n",
        );
    }
    if cx.width8_used {
        tu.push_str(
            " *\n * Width-8 MMIO note (D19/C-2): sys_read64/sys_write64 are\n * arch-conditional; this TU is validated on aarch64 — a target arch\n * lacking them fails the C compile loud.\n",
        );
    }
    tu.push_str(" */\n\n");

    // §1 item 1: the compatible pin.
    tu.push_str(&format!(
        "#define DT_DRV_COMPAT {}\n\n",
        compat_token(compatible)
    ));

    // §1 item 2: the demand-driven log-module block (≥1 reachable log site).
    if log_demand {
        tu.push_str(&format!(
            "#define LOG_MODULE_NAME {drv}\n#include <zephyr/logging/log.h>\nLOG_MODULE_REGISTER(LOG_MODULE_NAME);\n\n"
        ));
    }

    // §1 item 3: demand-driven includes.
    let mut includes: std::collections::BTreeSet<&str> =
        ["<zephyr/device.h>", "<stdint.h>"].into_iter().collect();
    if has_region {
        includes.insert("<zephyr/sys/device_mmio.h>");
    }
    includes.extend(cx.extra_includes.iter().copied());
    for inc in &includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');

    // Register-offset const bank (recovered from the spec's #[repr(C)]
    // block layout).
    if !cx.driver.reg_macros.is_empty() {
        tu.push_str("/* register offsets, recovered from the spec's #[repr(C)] block layout */\n");
        for (name, off) in &cx.driver.reg_macros {
            tu.push_str(&format!("#define {name} {off:#05x}\n"));
        }
        tu.push('\n');
    }

    // §1 item 4: per-instance state — type definitions (the spec driver +
    // every carrier the lowered bodies use), then config(ROM)/data(RAM)
    // with the MMIO macros as FIRST members, the spec driver embedded in
    // data (A-17), and the retained lifecycle state word (zero in the
    // static image).
    tu.push_str(type_defs);
    tu.push_str(&format!("struct {drv}_config {{\n"));
    if has_region {
        tu.push_str("\tDEVICE_MMIO_ROM; /* FIRST member (scaffold §1) */\n");
    } else {
        tu.push_str("\t/* no bus-managed region in this model */\n\tchar __rseq_empty;\n");
    }
    tu.push_str("};\n\n");
    tu.push_str(&format!("struct {drv}_data {{\n"));
    if has_region {
        tu.push_str("\tDEVICE_MMIO_RAM; /* FIRST member (scaffold §1) */\n");
    }
    tu.push_str(&format!(
        "\tstruct {inner_tag} inner;\n\tuint32_t __rseq_state; /* retained; zero in the static image */\n}};\n\n"
    ));
    if has_region {
        tu.push_str(&format!(
            "/* the TU-static base every MMIO op reads (A-2: the faithful first\n * realization, paired with the single-instance BUILD_ASSERT below) */\nstatic mm_reg_t {base_symbol};\n\n"
        ));
    }

    // §1 items 5-7: lowered helper/callback bodies, then the init function
    // — the one runtime dispatch hub (§3).
    tu.push_str(bodies);
    tu.push_str(init_fn);
    tu.push('\n');

    // §1 item 8: the build-time instantiation block —
    // register/match/create/associate as build-time facts (§2); init level
    // hardcoded (D13); api NULL for ClassTag::Generic ONLY (A-16).
    tu.push_str("/* build-time register/match/create/associate (scaffold §2) */\n");
    tu.push_str(&format!(
        "#define RSEQ_INIT(n)\\\n\tstatic struct {drv}_data {drv}_data_##n;\\\n"
    ));
    if has_region {
        tu.push_str(&format!(
            "\tstatic const struct {drv}_config {drv}_config_##n = {{\\\n\t\tDEVICE_MMIO_ROM_INIT(DT_DRV_INST(n)),\\\n\t}};\\\n"
        ));
    } else {
        tu.push_str(&format!(
            "\tstatic const struct {drv}_config {drv}_config_##n;\\\n"
        ));
    }
    tu.push_str(&format!(
        "\tDEVICE_DT_INST_DEFINE(n, {drv}_init, NULL,\\\n\t\t&{drv}_data_##n, &{drv}_config_##n,\\\n\t\tPOST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\\\n\t\tNULL /* api: ClassTag::Generic only (A-16) */);\n"
    ));
    tu.push_str("DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)\n\n");

    // §1 item 9: the mandatory single-instance guard (A-2/D18) — a
    // TU-static mm_reg_t with >1 status-okay instance would silently alias
    // one base across devices; multi-instance models are rejected at C
    // compile time.
    tu.push_str(
        "BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1,\n\t     \"TU-static MMIO base requires a single status-okay instance\");\n",
    );

    // The A-1 cross-check (ratified D11): DT `reg` is the normative map
    // source; the platform.toml literals are REQUIRED and re-asserted here
    // against the DT at C compile time, with a named message.
    if let Some((base, size)) = resolved_region {
        tu.push_str(&format!(
            "\n/* A-1/D11: the DT `reg` the map consumes must agree with the L2\n * platform literals the spec was resolved against */\nBUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == {base:#x} &&\n\t     DT_REG_SIZE(DT_DRV_INST(0)) == {size:#x},\n\t     \"DT reg disagrees with the resolved [mmio] platform literals (D11)\");\n"
        ));
    }

    Ok(tu)
}

/// The generated DT binding yaml (D07): `compatible:` from the match set —
/// the same string the TU pins — closing the compatible-drift channel;
/// description from the descriptor name; `base.yaml` included per the
/// vendored convention (arm_pl011.yaml).
fn emit_binding_yaml(
    model: &RegseqModel,
    drv: &str,
    compatible: &str,
    _driver: &regseq_model::DriverLifecycle,
) -> String {
    format!(
        "# Regseq-generated devicetree binding ({drv}.c sidecar; GRILL D07).\n# Generated by the regseq zephyr_c backend; do not edit.\n\ndescription: {} (Regseq-generated driver)\n\ncompatible: \"{compatible}\"\n\ninclude: base.yaml\n",
        model.strings.resolve(
            model
                .device_lifecycle
                .drivers
                .values()
                .next()
                .expect("caller validated exactly one driver")
                .descriptor
                .name
        )
    )
}
