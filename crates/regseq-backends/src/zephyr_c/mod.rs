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
//! of the composed five-construct surface, devicetree-matched. The PCIe
//! path (DEVICE_PCIE scaffold, the ratified probe → enable → map preamble,
//! config-space lowering) is delivered by backend/zephyr-c-e1000. Every
//! legal input shape with no Zephyr lowering contract is a loud
//! [`EmitError`] carrying its D.2-ledger disposition verbatim — never
//! silent emission, never silent omission. Structurally there is no
//! emit-what-we-can branch (I06).

mod body;
mod clock;
mod naming;
mod pinctrl;
mod sync;
mod types;

#[cfg(test)]
mod mod_tests;

use regseq_model::{
    AttrData, BusType, ClassTag, ConfigOp, EnumShape, InstanceId, InstanceKind, InstanceRef,
    IntrinsicKind, LayoutKind, MatchEntry, RegseqModel, TagEncoding, TargetOs, TerminatorKind, Ty,
    TyId, UserDataKind,
};

use crate::capability;
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
    /// The per-target conformance report (amendment 6, C05): delivered /
    /// absent-dispatch / rejected slots with their ratified row IDs.
    /// `rejected_slots` is structurally empty — a rejection returns `Err`
    /// before this struct exists (I07).
    pub conformance: capability::ConformanceReport,
}

/// zephyr_c emission errors. Every failure path is a loud, named diagnostic
/// (I06): D.2-ledger rejections carry their ratified wording, unit-scope
/// gaps name the delivering unit, and the shrinking-subset placeholders
/// name the construct; nothing silently truncates.
#[derive(Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A surface outside this unit's scope, with the unit that delivers it
    /// (GRILL D12/R04): e.g. an exotic bus scaffold → a post-first-light
    /// unit.
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
    // per-construct §B.2, or no selector) plus the PCIe-over-DT arm
    // (backend-zephyr-c-e1000 R01: DEVICE_PCIE scaffold + the C01 preamble).
    let pcie = match &driver.descriptor.bus_type {
        Some(BusType::Pci) => true,
        Some(BusType::Platform) | Some(BusType::Amba) | None => false,
        Some(other) => {
            return Err(EmitError::UnsupportedInUnit {
                what: format!("the `{other:?}` bus scaffold"),
                unit: "a post-first-light unit",
            })
        }
    };

    // Device class: a declared non-Generic class registers class-neutral
    // (the SAME NULL-api scaffold as ClassTag::Generic) with the C06 audit
    // rendering — the former A-16 reject is replaced by the D14 audited
    // class-neutral emission (backend-zephyr-c-e1000); class API delivery
    // itself stays the deferred Device Class construct's territory (B-9).
    let deferred_class: Option<String> = match &driver.descriptor.device_class {
        ClassTag::Generic => None,
        ClassTag::Serial => Some("serial".into()),
        ClassTag::Network => Some("network".into()),
        ClassTag::Other(s) => Some(model.strings.resolve(*s).to_string()),
    };

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
    }
    // A mixed native+foreign set is licensed by the C06 rule
    // (spec-convergence-pl011 D08: zephyr_c's ratified static-DT registration
    // form consumes Devicetree entries; recognized foreign entries are
    // ignorable with a triple-rendered audit once >=1 native entry exists —
    // the former unratified-rule rejection is lifted by that unit). The
    // license covers RECOGNIZED foreign entries only: an entry with no
    // ratified registration form on ANY backend (the deferred acpi/usb/
    // i2c/spi families) keeps the D.2 fail-loud default.
    let mut ignored_audits: Vec<crate::match_audit::MatchAudit> = Vec::new();
    for entry in &driver.descriptor.match_set {
        if matches!(entry, MatchEntry::Devicetree { .. }) {
            continue;
        }
        if !crate::match_audit::has_ratified_form(entry) {
            return Err(EmitError::Rejected(format!(
                "match entry {entry:?} has no ratified registration form on any backend \
                 (deferred bus family); the D03(a) rule licenses ignoring recognized \
                 foreign-transport entries only — fail-loud (D.2 default)"
            )));
        }
        ignored_audits.push(crate::match_audit::MatchAudit::foreign(
            model,
            entry,
            "static-devicetree",
        ));
    }
    if dt_compatibles.len() > 1 {
        return Err(EmitError::Unsupported(
            "multi-compatible match sets (scaffold §1 repeat-blocks idiom) are not exercised \
             by this unit; one Devicetree entry per driver"
                .into(),
        ));
    }
    let compatible = dt_compatibles[0];

    // Lifecycle ops: bring-up (D12) + teardown (spec-convergence-pl011 C04:
    // the former scheduling rejection is delivered — `on_deinit` lowers to a
    // deinit fn behind `DEVICE_DT_INST_DEINIT_DEFINE`, row
    // CAP-ZEPHYR-DT-DEINIT-1 flipped Delivered in place, D05). The remaining
    // scheduling rejection (shutdown) is table-cited (D14).
    // `on_surprise_remove`: the one ratified OS-absent slot. The capability
    // table decides — an `AbsentDispatch` row licenses auditable non-emission
    // (lowered in full into a throwaway context below, audit block in place);
    // anything else keeps the D.2 fail-loud default (I01).
    let absent_surprise: Option<&'static capability::CapabilityRow> =
        if driver.ops.on_surprise_remove.is_some() {
            let bus_family = match &driver.descriptor.bus_type {
                Some(BusType::Platform) => "platform",
                Some(BusType::Amba) => "amba",
                // The PCIe-over-DT form still registers via StaticDevicetree
                // (D03: the FORM carries the impossibility claim); the
                // existing wildcard row resolves the "pci" query.
                Some(BusType::Pci) => "pci",
                // Exotic buses were rejected above; a DT-registered driver
                // with no selector queries the wildcard.
                _ => "any",
            };
            match capability::lookup(
                TargetOs::Zephyr,
                capability::ZEPHYR_PIN,
                bus_family,
                capability::RegistrationForm::StaticDevicetree,
                capability::LifecycleSlot::OnSurpriseRemove,
            ) {
                Some(row) if row.disposition == capability::Disposition::AbsentDispatch => {
                    Some(row)
                }
                _ => {
                    return Err(EmitError::Rejected(
                        "authored `on_surprise_remove` under TargetOs::Zephyr: no hotplug \
                         dispatch exists for DT devices — the surprise scenario may not be \
                         representable on this target (D.2/B-7; A-12 recommends exactly this \
                         diagnostic)"
                            .into(),
                    ))
                }
            }
        } else {
            None
        };
    if driver.ops.on_system_shutdown.is_some() {
        // NOT a capability gap: the PM-device slot is witnessed and the
        // dispatch wrapper is designed (scaffold §4); its emission is
        // scheduled behind the C-4 trigger-contract pin per A-18.
        return Err(EmitError::Rejected(format!(
            "authored `on_system_shutdown`: the PM-device action wrapper is designed and \
             scheduled per A-18 (slot witnessed, scaffold §4; trigger contract pending the \
             C-4 pin) — not emitted by this unit [capability row {}]",
            capability::CAP_ZEPHYR_DT_SHUTDOWN_1.id
        )));
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
        ("on_deinit", &driver.ops.on_deinit),
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
    // -- device-interrupt facts (construct #6, T11): guards + the handler
    //    body joins the lowered closure (leaf symbol); the void ISR wrapper
    //    is scaffold-synthesized before the init fn --------------------------
    let irq = zephyr_irq_facts(model, driver_item, &driver.ops)?;
    let closure = reachable_closure(
        model,
        roots
            .iter()
            .map(|(_, id)| *id)
            .chain(irq.iter().map(|i| i.handler)),
    );

    // -- frame scan over the closure: op-shape rejections that must fire
    //    regardless of how far the body banks have grown (D12 frame) --------
    scan_closure_ops(model, &closure, pcie)?;

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

    // The PCIe-arm preamble facts (C01): the BAR NUMBER feeding
    // `pcie_get_mbar` is the platform-toml `bar` value (D04 — never
    // `pcie_probe_mbar`'s memory-BAR ordinal). The C01 preamble is
    // region-shaped (BDF rung → hoisted BAR probe → MEM enable → map): a
    // PCIe driver with no MMIO region has no preamble contract and keeps
    // the D.2 fail-loud default (C1 review round-1 finding 1 — a partial
    // preamble is an uncontracted shape, never emitted).
    let pcie_preamble: Option<PciePreamble> = if pcie {
        let field = driver.handle.resource_fields.first().ok_or_else(|| {
            EmitError::Rejected(
                "bus = \"pci\" with no MMIO resource region: the C01 preamble is \
                 region-shaped (BDF rung → hoisted BAR probe → MEM enable → map) and no \
                 region-less PCIe contract exists (D.2 default)"
                    .into(),
            )
        })?;
        let fname = c_ident(model.strings.resolve(field.name));
        let bar = field.binding.effective_bar.ok_or_else(|| {
            EmitError::Driver(format!(
                "PCIe MMIO region `{fname}` resolved with no BAR number \
                 (effective_bar = L2.bar ?? L1.bar); declare `bar` on the resource \
                 or in [mmio.{fname}] (D04: pcie_get_mbar takes a BAR NUMBER)"
            ))
        })?;
        Some(PciePreamble { bar })
    } else {
        None
    };
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
            pcie,
        },
        width8_used: false,
        uses_log: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
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

    // -- the ratified OS-absent slot (amendments 4/7; D06/D10): the absent
    //    callback's body runs the COMPLETE body-lowering pipeline — its own
    //    closure (the emitter roots only on_init/on_init_failure above), the
    //    D12 frame scan, fn_head/emit_fn — in a THROWAWAY LowerCx, and the
    //    output is discarded. The throwaway context is load-bearing: a shared
    //    one would let the discarded lowering perturb includes/log-module/
    //    width8/symbols of the TU (D06: no observable effect beyond the
    //    audit block). Exactly one C04 block is emitted where the callback's
    //    lowered definition would have sat in the bodies bank.
    let mut absent_entries: Vec<capability::SlotEntry> = Vec::new();
    if let Some(row) = absent_surprise {
        let r = driver
            .ops
            .on_surprise_remove
            .expect("absent_surprise implies the slot is authored");
        let id = resolve_instance_ref(model, r).ok_or_else(|| {
            EmitError::Driver(
                "callback `on_surprise_remove` does not resolve to an instance".into(),
            )
        })?;
        let absent_closure = reachable_closure(model, std::iter::once(id));
        // Any construct the delivered path rejects still yields the D.2
        // EmitError — the scan first, then full lowering (never scan-only).
        scan_closure_ops(model, &absent_closure, pcie)?;
        let mut tcx = body::LowerCx {
            model,
            names: TypeNames::new(),
            fn_symbols: std::collections::BTreeMap::new(),
            extra_includes: std::collections::BTreeSet::new(),
            driver: body::DriverFacts {
                driver_ty,
                resources: cx.driver.resources.clone(),
                reg_macros: cx.driver.reg_macros.clone(),
                base_symbol: cx.driver.base_symbol.clone(),
                pcie: cx.driver.pcie,
            },
            width8_used: false,
            uses_log: false,
            uses_delay_busy: false,
            uses_delay_sleep: false,
            elided_locals: std::collections::BTreeSet::new(),
        };
        // Seed fn_symbols for the WHOLE absent closure before fn_head/emit_fn
        // (fn_head indexes cx.fn_symbols[&instance]).
        tcx.fn_symbols
            .insert(id, format!("{drv}_on_surprise_remove"));
        for cid in &absent_closure {
            if tcx.fn_symbols.contains_key(cid) {
                continue;
            }
            let leaf =
                naming::path_leaf(model.strings.resolve(model.instances[*cid].emission_name));
            let base = format!("{drv}_{}", c_ident(leaf));
            let mut sym = base.clone();
            let mut n = 1;
            while tcx.fn_symbols.values().any(|v| v == &sym) {
                n += 1;
                sym = format!("{base}_{n}");
            }
            tcx.fn_symbols.insert(*cid, sym);
        }
        for cid in &absent_closure {
            body::fn_head(&mut tcx, *cid).map_err(|e| with_fn_context(e, model, *cid))?;
        }
        for cid in &absent_closure {
            body::emit_fn(&mut tcx, *cid).map_err(|e| with_fn_context(e, model, *cid))?;
        }
        // The type-definition pass is a legality check too (aggregate
        // definitions outside the subset reject HERE, not during body
        // lowering) — run it over the throwaway names and discard (Codex M1
        // review finding 1: skipping it would let an absent body carry a
        // type a delivered body rejects).
        emit_type_defs(&mut tcx, driver_ty)?;
        bodies.push_str(&row.audit_block());
        bodies.push('\n');
        absent_entries.push(capability::SlotEntry {
            slot: capability::LifecycleSlot::OnSurpriseRemove
                .model_name()
                .into(),
            row: Some(row.id.into()),
            citation: Some(row.citation.into()),
        });
    }

    // -- the init function (scaffold §3) + the fused errno adapter ----------
    let find = |name: &str| roots.iter().find(|(n, _)| *n == name).map(|(_, id)| *id);
    let mut init_fn = emit_init_fn(
        &mut cx,
        &drv,
        &base_symbol,
        has_region,
        pcie_preamble.as_ref(),
        find("on_init"),
        find("on_init_failure"),
        irq.as_ref(),
    )?;
    // The void ISR wrapper (construct #6, C04) precedes the init fn that
    // IRQ_CONNECTs it: recover the device from the build-time-constant
    // sw_isr_table parameter, run the authored handler over the embedded
    // spec driver, and DISCARD the IrqResult structurally (R08 zephyr leg:
    // the sw_isr_table consumer is void — no spurious-return channel
    // exists). The wrapper acquires NO locks (R18/D11).
    if let Some(irq) = &irq {
        let handler_sym = cx.fn_symbols[&irq.handler].clone();
        init_fn = format!(
            "/* the bound ISR (construct #6, C04): void dispatch — the IrqResult is\n * structurally discarded (R08: zephyr's sw_isr_table consumer is void);\n * no locks (R18) */\nstatic void rseq_isr_{drv}(const void *arg)\n{{\n\tconst struct device *dev = arg;\n\tstruct {drv}_data *data = dev->data;\n\n\t(void){handler_sym}(&data->inner);\n}}\n\n{init_fn}"
        );
    }
    // -- the deinit function (spec-convergence-pl011 C04): authored
    //    `on_deinit` only — unauthored leaves the scaffold byte-identical
    //    (I05: the absentslot/init-fail goldens stay frozen).
    let deinit_fn = find("on_deinit")
        .map(|id| emit_deinit_fn(&mut cx, &drv, id, irq.as_ref()))
        .transpose()?;

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
        &ignored_audits,
        driver_ty,
        &base_symbol,
        has_region,
        pcie,
        deferred_class.as_deref(),
        resolved_region,
        &type_defs,
        &bodies,
        &init_fn,
        deinit_fn.as_deref(),
    )?;
    let binding_yaml = emit_binding_yaml(
        model,
        &drv,
        compatible,
        pcie,
        pinctrl::has_resolved_pinctrl(model, driver_ty),
        &ignored_audits,
    );

    // -- the per-target conformance report (amendment 6, C05): slots in
    //    LifecycleOps declaration order; delivered rows carry a ratified row
    //    ID only when one exists (`on_deinit` cites CAP-ZEPHYR-DT-DEINIT-1,
    //    the D05 in-place flip; the bring-up slots have none);
    //    rejected_slots structurally empty (I07).
    let delivered_slots = roots
        .iter()
        .map(|(name, _)| capability::SlotEntry {
            slot: (*name).to_string(),
            row: (*name == "on_deinit")
                .then(|| capability::CAP_ZEPHYR_DT_DEINIT_1.id.to_string()),
            citation: None,
        })
        .collect();
    // Device-interrupt deregister slot (construct #6, T12 zephyr leg): the
    // operation is STRUCTURALLY ABSENT on the static-connect form — the
    // ratified row carries the connected-but-masked statement in its
    // citation; non-emission is never silent (I04).
    let structurally_absent_slots = if irq.is_some() {
        vec![capability::SlotEntry {
            slot: capability::CAP_ZEPHYR_STATICIRQ_DEREGISTER_1
                .slot
                .model_name()
                .to_string(),
            row: Some(capability::CAP_ZEPHYR_STATICIRQ_DEREGISTER_1.id.to_string()),
            citation: Some(capability::CAP_ZEPHYR_STATICIRQ_DEREGISTER_1.citation.to_string()),
        }]
    } else {
        Vec::new()
    };
    let conformance = capability::ConformanceReport {
        target: "zephyr_c",
        os_version_pin: capability::ZEPHYR_PIN,
        delivered_slots,
        absent_dispatch_slots: absent_entries,
        structurally_absent_slots,
        rejected_slots: Vec::new(),
        ignored_match_entries: ignored_audits,
        // The C06 deferred-class note rides the report iff a non-Generic
        // class registered class-neutral (D14) — same license const as the
        // TU audit comment.
        deferred_class: deferred_class.map(|class| capability::DeferredClassNote {
            class,
            license: capability::DEFERRED_CLASS_LICENSE,
        }),
    };

    Ok(ZephyrEmit {
        tu,
        binding_yaml,
        driver_name: drv,
        conformance,
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
        // Pin-controller-and-GPIO-configuration construct (pinctrl, GRILL D12/C08,
        // T19): the optional `Option<PinCtrl>` absent path compile-time-folds to
        // `Err(RegseqError::PinctrlAbsent)` (D06/C04); the scaffold errno adapter
        // maps it to `ENODEV` (no such device — the pinctrl resource is absent).
        // Each backend fails-loud on an unmapped variant, so every backend that can
        // emit the fold MUST carry this row (generic-c in M3, linux/zephyr in M4).
        "PinctrlAbsent" => "ENODEV",
        _ => return None,
    })
}

/// The PCIe-arm facts the C01 init preamble consumes. Constructed only for
/// a region-carrying PCIe driver (the region-less shape is the D.2
/// rejection above), so the full C01 preamble always emits.
struct PciePreamble {
    /// The effective BAR number feeding `pcie_get_mbar` (D04).
    bar: u8,
}

/// Synthesize the per-instance init function (scaffold §3): the one runtime
/// dispatch hub — resource preamble (claim_regions = ZERO calls, the
/// ratified elision; `DEVICE_MMIO_MAP` + the TU-static base install, A-2;
/// on the PCIe arm the C01 probe → enable → map preamble with its failure
/// rungs), then the authored `on_init` through the fused negated-errno
/// adapter with the `on_init_failure` cleanup slot sequenced BEFORE the
/// error return.
///
/// I10 is enforced structurally here: an authored `RegseqError` crosses the
/// adapter at this boundary ONLY; native preamble failures (the C01 rungs)
/// return unwrapped negative errnos directly — the two-channel rule (I09).
/// The Zephyr `Atomic`-field seeding lines (R10/S10): `__atomic_store_n` over the
/// `data->inner.<field>` cell for each non-zero `#[rseq::atomic(initial = N)]`
/// (the data struct is zero-filled by the device model, so 0/None need no init).
/// Empty when no field needs seeding.
fn zephyr_atomic_init(cx: &body::LowerCx<'_>) -> String {
    let mut s = String::new();
    for (name, element, initial) in cx.model.atomic_fields_of(cx.driver.driver_ty) {
        let Some(init) = initial else { continue };
        let cty = body::zephyr_atomic_cty(element);
        let raw = init as u64;
        let masked = if element.width_bits() == 32 {
            raw & 0xffff_ffff
        } else {
            raw
        };
        if masked == 0 {
            continue;
        }
        s.push_str(&format!(
            "\t__atomic_store_n(&data->inner.{}, ({cty})0x{masked:x}ULL, __ATOMIC_RELAXED);\n",
            c_ident(&name)
        ));
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn emit_init_fn(
    cx: &mut body::LowerCx<'_>,
    drv: &str,
    base_symbol: &str,
    has_region: bool,
    pcie: Option<&PciePreamble>,
    on_init: Option<InstanceId>,
    on_init_failure: Option<InstanceId>,
    irq: Option<&ZephyrIrq>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* the one runtime dispatch hub (scaffold §3) */\nstatic int {drv}_init(const struct device *dev)\n{{\n"
    ));
    // Atomicity (R10/S10): seed each Atomic field from #[rseq::atomic(initial=N)]
    // before any callback, regardless of whether on_init is authored. Computed
    // up front so `data` is declared when a non-zero initial needs it.
    let atomic_init = zephyr_atomic_init(cx);
    // Synchronization init_primitives (C05/I05): initialize the declared locks
    // BEFORE the region map + IRQ connect + on_init (lock-live ⊇ line-live). A
    // k_spinlock needs no runtime init (zero-init valid); destroy is a no-op on
    // Zephyr (static primitives). Computed before `uses_data` so a lock-bearing
    // driver with init lines but NO on_init still declares `data` (the init
    // references `data->inner.<f>`).
    let sync_inits = sync::init_primitive_lines(cx.model, cx.driver.driver_ty);
    // `data` is needed if ANY of: pcie, on_init(_failure), a non-empty atomic
    // seed (atomicity), or a non-empty lock-init (synchronization). The merge
    // must OR both construct conditions — neither alone is sufficient.
    let uses_data = pcie.is_some()
        || on_init.is_some()
        || on_init_failure.is_some()
        || !atomic_init.is_empty()
        || !sync_inits.is_empty();
    if uses_data {
        out.push_str(&format!("\tstruct {drv}_data *data = dev->data;\n\n"));
    }
    if !sync_inits.is_empty() {
        out.push_str("\t/* init_primitives (C05): locks initialized before region/IRQ/on_init */\n");
        for line in &sync_inits {
            out.push_str(&format!("\t{line}\n"));
        }
        out.push('\n');
    }
    if let Some(p) = pcie {
        // The C01 preamble, PCIe arm, in emission order — the ratified
        // probe → enable → map deviation (D02), both rungs unwrapped
        // `-ENODEV` (D11, the native channel of the two-channel rule).
        cx.extra_includes.insert("<errno.h>");
        out.push_str(
            "\t/* C01 preamble (PCIe arm): BDF rung -> carrier install -> probe ->\n\t * enable -> map */\n",
        );
        out.push_str(
            "\tif (data->pcie->bdf == PCIE_BDF_NONE)\n\t\treturn -ENODEV; /* native channel, unwrapped (D11) */\n",
        );
        out.push_str(
            "\t__rseq_pcie = data->pcie; /* TU-static carrier install (D12), pre-body */\n",
        );
        let bar = p.bar;
        out.push_str(
            "\n\t/* RSEQ-DEVIATION: get_resource_handle hoisted before enable_device —\n\t * the single ratified per-OS preamble deviation; BAR sizing with\n\t * memory decode off (witness eth_e1000.c:219-224).\n\t * [license: backend-zephyr-c-e1000 GRILL D02] */\n",
        );
        out.push_str(&format!(
            "\tstruct pcie_bar mbar;\n\tif (!pcie_get_mbar(data->pcie->bdf, {bar}, &mbar))\n\t\treturn -ENODEV; /* native channel, unwrapped (D11) */\n"
        ));
        out.push_str(
            "\t/* claim_regions: zero-call elision (the ratified zephyr claim\n\t * disposition) */\n",
        );
        out.push_str(
            "\t/* enable_device: MEM gate only — set_bus_master (PCIE_CONF_CMDSTAT_MASTER)\n\t * fires only when that op is authored; single-call fusion is a permitted\n\t * peephole when both gates fire (unexercised here) */\n",
        );
        out.push_str("\tpcie_set_cmd(data->pcie->bdf, PCIE_CONF_CMDSTAT_MEM, true);\n");
        out.push_str(&format!(
            "\tdevice_map(&{base_symbol}, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE);\n\t/* TU-static base install (A-2; the single-instance BUILD_ASSERT below\n\t * covers BOTH TU-statics: base + __rseq_pcie) */\n\n"
        ));
    } else if has_region {
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
    // Pin-controller-and-GPIO-configuration acquire preamble (pinctrl construct,
    // C07/D10, M4/T21): AFTER the region map, BEFORE the clock gate —
    // pins-before-clock (C07). Per resolved resource, apply the default pin state
    // over the build-time DT pin config (required checks rc, optional ignores).
    // Empty for a pinctrl-less / absent-only driver (I07).
    if pinctrl::has_resolved_pinctrl(cx.model, cx.driver.driver_ty) {
        cx.extra_includes.insert("<zephyr/drivers/pinctrl.h>");
        cx.extra_includes.insert("<errno.h>");
        out.push_str(&pinctrl::acquire_lines(cx.model, cx.driver.driver_ty));
    }
    // Peripheral-clock-management acquire preamble (clock construct, C07/D12):
    // AFTER the pinctrl acquire, BEFORE the interrupt connect — the managed clock
    // controller readiness gate (`device_is_ready` → -ENODEV, a hard init error,
    // I02). Empty for a clock-less / static-only driver (I07).
    if clock::has_managed_clock(cx.model, cx.driver.driver_ty) {
        cx.extra_includes.insert("<zephyr/drivers/clock_control.h>");
        cx.extra_includes.insert("<errno.h>");
        let acquire = clock::acquire_lines(cx.model, cx.driver.driver_ty);
        out.push_str(&acquire);
    }
    // Interrupt preamble (construct #6, D14): the static connect — LAST
    // among the resource preambles. The sw_isr_table install is BUILD-TIME
    // (the macro must appear in executed code; its runtime cost is nil);
    // deliberately WITHOUT the stock immediate irq_enable (D02/R04 — the
    // enable splits to the post-on_init-Ok point below). On the Err path the
    // line stays connected-but-masked: no deregister exists on this form
    // (CAP-ZEPHYR-STATICIRQ-DEREGISTER-1).
    if let Some(irq) = irq {
        cx.extra_includes.insert("<zephyr/irq.h>");
        if let Some(advisory) = irq.advisory_irq_number {
            out.push_str(&format!(
                "\t/* [interrupt.{id}] irq_number={advisory} is ADVISORY on zephyr:\n\t * DT_INST_IRQN(0) is authoritative (R10) */\n",
                id = irq.l2_id,
            ));
        }
        let priority = match irq.priority {
            // The L2 priority lowers as the IRQ_CONNECT argument ONLY (R12).
            Some(p) => p.to_string(),
            // Unspecified: the DT irq cell's priority is authoritative (D10).
            None => "DT_INST_IRQ(0, priority)".to_string(),
        };
        out.push_str(&format!(
            "\tIRQ_CONNECT(DT_INST_IRQN(0), {priority}, rseq_isr_{drv},\n\t\t    DEVICE_DT_INST_GET(0), 0);\n\n"
        ));
    }
    // Seed the Atomic fields before on_init (runs regardless of on_init).
    out.push_str(&atomic_init);
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
    if irq.is_some() {
        // The enable point (D02/R05/I01): only after on_init returned Ok —
        // the adapter above has already returned the negated errno on every
        // Err path (and an unoverridden on_init cannot fail).
        out.push_str(
            "\n\t/* enable the line only after on_init Ok (D02/R05) */\n\tirq_enable(DT_INST_IRQN(0));\n",
        );
    }
    out.push_str("\treturn 0;\n}\n");
    Ok(out)
}

/// Synthesize the per-instance deinit function (spec-convergence-pl011 C04):
/// the authored `on_deinit` body through the SAME lowering pipeline and the
/// SAME fused nine-row negated-errno adapter as init (deterministic variant
/// order). Error-channel semantics are the kernel's, cited not invented
/// (v4.4.0 kernel/device.c:199-218): our negative rc is returned by
/// `device_deinit()` VERBATIM and `initialized` stays true on failure; on
/// success it clears, so `device_is_ready()` flips false. No failure hook:
/// `on_init_failure` is the init path's cleanup slot only.
fn emit_deinit_fn(
    cx: &mut body::LowerCx<'_>,
    drv: &str,
    on_deinit: InstanceId,
    irq: Option<&ZephyrIrq>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* the teardown dispatch hub (spec-convergence-pl011 C04) */\nstatic int {drv}_deinit(const struct device *dev)\n{{\n"
    ));
    out.push_str(&format!("\tstruct {drv}_data *data = dev->data;\n\n"));
    let sym = cx.fn_symbols[&on_deinit].clone();
    out.push_str(
        "\t/* authored on_deinit over the embedded spec driver; the authored\n\t * RegseqError crosses the negated-errno adapter exactly as in init\n\t * (kernel/device.c:199: the rc is returned verbatim; `initialized`\n\t * stays true on failure, clears on success) */\n",
    );
    let adapter = emit_result_adapter(cx, on_deinit, &format!("{sym}(&data->inner)"), None)?;
    out.push_str(&adapter);
    if irq.is_some() {
        // Interrupt teardown (construct #6, D09/R06): disable after the
        // teardown callback — then the deregister SLOT, where the static
        // connect renders its ratified audit block INSTEAD of code: exactly
        // one block, from the same const as the conformance row (I04); the
        // line is left connected-but-masked.
        out.push_str(
            "\t/* interrupt teardown (construct #6): mask the line (D09/R06) */\n\tirq_disable(DT_INST_IRQN(0));\n",
        );
        for line in capability::CAP_ZEPHYR_STATICIRQ_DEREGISTER_1
            .audit_block()
            .lines()
        {
            out.push('\t');
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(
        "\t/* postamble (integration §5.2): map_region's unmap mirror is a\n\t * deliberate NO-OP — no DEVICE_MMIO unmap exists on Zephyr; nothing\n\t * is invented (C04) */\n",
    );
    out.push_str("\treturn 0;\n}\n");
    Ok(out)
}

/// The device-interrupt lowering facts for the zephyr scaffold (construct
/// #6, T11): the lowered handler instance + the L2 facts the `IRQ_CONNECT`
/// emission consumes.
struct ZephyrIrq {
    /// The lowered handler-body instance (joins the closure; leaf symbol).
    handler: InstanceId,
    /// The L2 `priority` — lowers as the `IRQ_CONNECT` argument only (R12);
    /// `None` defers to the DT irq cell (`DT_INST_IRQ(0, priority)`).
    priority: Option<u32>,
    /// The L2 `irq_number`, ADVISORY on zephyr (R10): rendered as a comment;
    /// `DT_INST_IRQN(0)` is authoritative.
    advisory_irq_number: Option<u32>,
    /// The L2 join id (the advisory comment names its section).
    l2_id: String,
}

/// Build the zephyr device-interrupt facts, firing the emit-stage guards
/// (I05 — loud, never partial), mirroring the linux/generic legs; plus two
/// zephyr-specific gates: a shared line cannot be expressed by the static
/// connect this slice, and an interrupt-bearing driver needs an authored
/// `on_deinit` (the deinit fn hosts the disable + the deregister-slot audit
/// block). `Ok(None)` for an interrupt-less driver — zero churn.
fn zephyr_irq_facts(
    model: &RegseqModel,
    driver_item: &regseq_model::ItemId,
    ops: &regseq_model::LifecycleOps,
) -> Result<Option<ZephyrIrq>, EmitError> {
    let Some(interrupts) = model.interrupt.drivers.get(driver_item) else {
        return Ok(None);
    };
    if interrupts.resources.is_empty() {
        return Ok(None);
    }
    if interrupts.resources.len() > 1 {
        return Err(EmitError::Unsupported(
            "more than one Interrupt resource per driver (DT_INST_IRQN(0) single-line \
             slice)"
                .into(),
        ));
    }
    let resource = &interrupts.resources[0];
    if !matches!(resource.form, regseq_model::InterruptForm::Single) {
        return Err(EmitError::Unsupported(
            "device-interrupt: Option<Interrupt> / [Interrupt; N] lowering is not \
             implemented this slice (the form is recognized; emit-stage rejection per \
             GRILL D06/R02)"
                .into(),
        ));
    }
    if resource.binding.shared {
        return Err(EmitError::Unsupported(
            "device-interrupt: a shared line (L2 `shared = true`) cannot be lowered to \
             the static IRQ_CONNECT registration form (a shared sw_isr_table entry needs \
             CONFIG_SHARED_INTERRUPTS / the dynamic-connect form — a future unit)"
                .into(),
        ));
    }
    if ops.on_deinit.is_none() {
        return Err(EmitError::Unsupported(
            "device-interrupt: an interrupt-bearing zephyr driver requires an authored \
             on_deinit this slice (the deinit fn hosts the irq_disable and the \
             deregister-slot audit block)"
                .into(),
        ));
    }
    let handler = interrupts
        .handlers
        .iter()
        .find(|h| h.resource == 0)
        .ok_or_else(|| {
            EmitError::Driver("a lowered Interrupt resource has no bound handler".into())
        })?;
    if handler.arms.defer_work != regseq_model::ArmReachability::ProvenUnreachable {
        return Err(EmitError::Unsupported(
            "device-interrupt: the handler's IrqResult::DeferWork arm is not proven \
             unreachable — deferring work from an ISR requires the \
             deferred-work-item-scheduling construct (not yet built); rejected at emit \
             per R09/D04"
                .into(),
        ));
    }
    let iref = handler
        .body
        .ok_or_else(|| EmitError::Driver("handler body InstanceRef missing".into()))?;
    let handler_id = resolve_instance_ref(model, iref)
        .ok_or_else(|| EmitError::Driver("handler InstanceRef does not resolve".into()))?;
    Ok(Some(ZephyrIrq {
        handler: handler_id,
        priority: resource.binding.priority,
        advisory_irq_number: resource.binding.irq_number,
        l2_id: model.strings.resolve(resource.id).to_string(),
    }))
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
    // Synchronization (locks, T07/C02): the lock FIELDS substitute their
    // declaration to the per-OS lock object and pull in the per-kind header; the
    // driver struct's `ItemId` keys the sync table.
    let mut sync_field_decls: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    if let Ty::Struct { item, .. } = &cx.model.types[driver_ty] {
        if let Some(s) = cx.model.synchronization.drivers.get(item) {
            for res in &s.resources {
                let fname = c_ident(cx.model.strings.resolve(res.name));
                for inc in sync::includes_for(res.kind) {
                    cx.extra_includes.insert(inc);
                }
                sync_field_decls.insert(fname.clone(), sync::lock_field_decl(res.kind, &fname));
            }
        }
    }

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
            // Atomicity (construct #7): override each `Atomic<T>` field's storage
            // to the plain width-`T` cell the `__atomic_*` builtins operate on
            // (C07), for EVERY struct with atomic fields (not just the driver —
            // a nested struct's atomic fields must be overridden too). Unlike the
            // dead `mm_reg_t` MMIO carrier, this is REAL storage.
            let atomic_fields: Vec<(String, &'static str)> = model
                .atomic_fields_of(ty)
                .into_iter()
                .map(|(name, element, _)| (c_ident(&name), body::zephyr_atomic_cty(element)))
                .collect();
            let chunk = if ty == driver_ty {
                let rn: Vec<String> = cx
                    .driver
                    .resources
                    .iter()
                    .map(|(n, _, _)| n.clone())
                    .collect();
                let af = atomic_fields;
                let sync = sync_field_decls.clone();
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    if rn.iter().any(|r| r == fname) {
                        Some(format!(
                            "mm_reg_t {fname} /* dead under the TU-static base carrier (A-2) */"
                        ))
                    } else if let Some((_, cty)) = af.iter().find(|(n, _)| n == fname) {
                        Some(format!("{cty} {fname}"))
                    } else {
                        // T07/C02: a recognized lock field lowers to its per-OS
                        // lock object (the payload `T` is not stored this slice).
                        sync.get(fname).cloned()
                    }
                })
                .map_err(with_ctx)?
            } else {
                let af = atomic_fields;
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    af.iter()
                        .find(|(n, _)| n == fname)
                        .map(|(_, cty)| format!("{cty} {fname}"))
                })
                .map_err(with_ctx)?
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
/// callees do not join the closure (they are intercepted at emission) and
/// neither do trivial projection accessors (`Mmio::deref` — inlined
/// analytically at the receiver chase, their calls elided); but
/// [`scan_closure_ops`] inspects intrinsics from the caller side.
/// Deterministic order: roots first, then discovery order.
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
        let body_ref = model.instance_ref_of(id);
        for (idx, bb) in b.basic_blocks.iter().enumerate() {
            if let TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                ..
            } = &bb.terminator.kind
            {
                // A recognized sync acquire/release/completion `Call` is replaced
                // by its C02 primitive (T07); its callee is never lowered.
                if sync::is_sync_call_block(model, body_ref, regseq_model::BlockId::from_u32(idx as u32))
                    || intrinsic_of(model, *callee).is_some()
                    || body::trivial_accessor_tail(model, *callee).is_some()
                {
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
/// contract for — `poll_until` (B-1) and the unlicensed config-space shapes
/// (the B-3 remainder; B-4/BRM-001 non-PCI access; the [`config_shape`]
/// classifier). Runs BEFORE body lowering so the rejections hold however
/// far the T06–T08 banks have grown.
fn scan_closure_ops(
    model: &RegseqModel,
    closure: &[InstanceId],
    pcie: bool,
) -> Result<(), EmitError> {
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
                    // The same classifier the body lowering runs (C02/C03
                    // legality): everything outside the licensed shapes
                    // rejects HERE, before any bank lowers the call.
                    let TerminatorKind::Call { args, .. } = &bb.terminator.kind else {
                        unreachable!("matched Call above");
                    };
                    config_shape(model, *callee, op, args, pcie)?;
                }
                Some(IntrinsicKind::Log(_)) => {
                    // The A-5 gate must fire on the PUBLIC path BEFORE any
                    // local/signature C-type emission can trip over the fat
                    // pointer (C2 review finding #1): a runtime &str log
                    // argument is the unexercised bounce-buffer shape.
                    let TerminatorKind::Call { args, .. } = &bb.terminator.kind else {
                        unreachable!("matched Call above");
                    };
                    for arg in args.iter().skip(1) {
                        let place = match arg {
                            regseq_model::Operand::Copy(p) | regseq_model::Operand::Move(p) => p,
                            regseq_model::Operand::Constant(_) => continue,
                        };
                        let Ok(ty) = body::place_ty(model, b, place) else {
                            continue;
                        };
                        let is_str_ref = matches!(
                            &model.types[ty],
                            Ty::Ref(p, _) if matches!(&model.types[*p], Ty::Str)
                        );
                        if is_str_ref {
                            return Err(EmitError::Rejected(
                                "diag-log non-static &str argument under deferred mode: the \
                                 call-site bounce-buffer delivery (ratified A-5 \
                                 recommendation) is unexercised by this unit's corpus — \
                                 rejected fail-loud, never silently over-read"
                                    .into(),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The emit-time facts of a LICENSED config-space call shape (the C02/C03
/// contracts, backend-zephyr-c-e1000): width from the value type, byte
/// offset from the constant first argument.
pub(crate) struct ConfigShape {
    /// Access width in bytes (1, 2, or 4).
    pub width: u64,
    /// Constant byte offset into config space.
    pub off: u64,
}

/// Classify one config-space call against the C02/C03 contracts; everything
/// outside the licensed shapes keeps its D.2 rejection wording (I04):
/// width-8 / 64-bit (the B-3 remainder), non-PCI buses (B-4/BRM-001),
/// non-constant offsets (the word-0 cache + alignment legality are
/// emit-time decisions), and misaligned sub-word access. Shared verbatim by
/// the D12 frame scan and the body lowering so the rejections hold however
/// far the banks grow.
pub(crate) fn config_shape(
    model: &RegseqModel,
    callee: InstanceId,
    op: &ConfigOp,
    args: &[regseq_model::Operand],
    pcie: bool,
) -> Result<ConfigShape, EmitError> {
    let dir = if matches!(op, ConfigOp::Write) {
        "write"
    } else {
        "read"
    };
    let width = match config_width(model, callee, op) {
        Some(8) => {
            return Err(EmitError::Rejected(format!(
                "config-space {dir} width 8: the two-word split is undesigned (D.2/B-3; \
                 the ratified B-3 partial lift covers width-1/2 WRITES via the \
                 containing-word RMW only)"
            )))
        }
        Some(w @ (1 | 2 | 4)) => w,
        _ => {
            return Err(EmitError::Rejected(format!(
                "config-space {dir} with a non-uint value type: no lowering contract \
                 (D.2/B-3); the specified shapes are u8/u16/u32"
            )))
        }
    };
    if !pcie {
        return Err(EmitError::Rejected(
            "config-space access on a non-PCI-like bus: no carrier (the upstream BRM-001 \
             rejection diagnostic is pending, D.2/B-4); the PCIe scaffold (bus = \"pci\") \
             carries the only ratified contract"
                .into(),
        ));
    }
    let off = match args.first() {
        Some(regseq_model::Operand::Constant(id)) => match &model.consts[*id] {
            // The intrinsic surface types the offset u32 (the fixture's
            // `read_config<T>(_off: u32)`): a wider constant is an
            // out-of-domain shape — rejected, never silently truncated
            // (C1 review round-1 finding 2).
            regseq_model::Const::Uint(_, v) if *v > u32::MAX as u128 => {
                return Err(EmitError::Rejected(format!(
                    "config-space {dir} offset {v:#x} exceeds the u32 offset domain of \
                     the config intrinsics: no lowering contract (D.2)"
                )))
            }
            regseq_model::Const::Uint(_, v) => *v as u64,
            other => {
                return Err(EmitError::Rejected(format!(
                    "config-space {dir} offset constant {other:?}: only uint-literal \
                     offsets are specified — the C03 word-0 cache and the alignment \
                     legality are emit-time decisions (D.2)"
                )))
            }
        },
        _ => {
            return Err(EmitError::Rejected(format!(
                "config-space {dir} with a non-constant offset: the C03 word-0 cache and \
                 the alignment legality are emit-time decisions — no runtime-offset \
                 lowering contract exists (D.2)"
            )))
        }
    };
    if off % width != 0 {
        return Err(EmitError::Rejected(format!(
            "config-space {dir} width {width} at offset {off:#x}: misaligned access keeps \
             the D.2 fail-loud default (B-3; the ratified partial lift covers \
             naturally-aligned shapes only)"
        )));
    }
    Ok(ConfigShape { width, off })
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
    ignored_audits: &[crate::match_audit::MatchAudit],
    driver_ty: TyId,
    base_symbol: &str,
    has_region: bool,
    pcie: bool,
    deferred_class: Option<&str>,
    resolved_region: Option<(u64, u64)>,
    type_defs: &str,
    bodies: &str,
    init_fn: &str,
    deinit_fn: Option<&str>,
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
    if deinit_fn.is_some() {
        // The A-9 pin row for teardown (spec-convergence-pl011 C04): the
        // hazard is compile-SILENT — Z_DEVICE_OPS drops deinit_fn under
        // IF_ENABLED when the Kconfig is off (device.h:1175-1180), the build
        // stays clean, and device_deinit() returns -ENOTSUP at runtime.
        tu.push_str(
            " *\n * Teardown Kconfig pin (A-9, documented not asserted):\n * CONFIG_DEVICE_DEINIT_SUPPORT=y — Z_DEVICE_OPS silently DROPS deinit_fn\n * under IF_ENABLED when off (device.h:1175-1180); the build passes and\n * device_deinit() returns -ENOTSUP.\n",
        );
    }
    tu.push_str(" */\n\n");

    // §1 item 1: the compatible pin — with the C03 audit blocks adjacent
    // (A1: zephyr's consumed match surface is the DT_DRV_COMPAT pin; there
    // is no match table to anchor on).
    for audit in ignored_audits {
        tu.push_str(&audit.c_block());
        tu.push('\n');
    }
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
    if pcie {
        includes.insert("<zephyr/drivers/pcie/pcie.h>");
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

    // Pin-controller-and-GPIO-configuration build-time DT pin-config table
    // (pinctrl construct, M4/T21, D10/C04): defined once when the driver has a
    // RESOLVED pinctrl resource (DT-authoritative); the init hub applies states
    // via `PINCTRL_DT_INST_DEV_CONFIG_GET(0)`. Absent for a pinctrl-less /
    // absent-only driver (I07).
    if pinctrl::has_resolved_pinctrl(cx.model, cx.driver.driver_ty) {
        tu.push_str(
            "/* pinctrl construct (M4/T21): build-time DT pin-config table */\nPINCTRL_DT_INST_DEFINE(0);\n\n",
        );
    }

    // §1 item 4: per-instance state — type definitions (the spec driver +
    // every carrier the lowered bodies use), then config(ROM)/data(RAM)
    // with the MMIO macros as FIRST members, the spec driver embedded in
    // data (A-17), and the retained lifecycle state word (zero in the
    // static image).
    tu.push_str(type_defs);
    tu.push_str(&format!("struct {drv}_config {{\n"));
    if pcie {
        tu.push_str(
            "\t/* PCIe: no DT `reg` capture — the BAR map rides the TU-static base\n\t * (C01); the config struct carries no DEVICE_MMIO_ROM */\n\tchar __rseq_empty;\n",
        );
    } else if has_region {
        tu.push_str("\tDEVICE_MMIO_ROM; /* FIRST member (scaffold §1) */\n");
    } else {
        tu.push_str("\t/* no bus-managed region in this model */\n\tchar __rseq_empty;\n");
    }
    tu.push_str("};\n\n");
    tu.push_str(&format!("struct {drv}_data {{\n"));
    if has_region && !pcie {
        tu.push_str("\tDEVICE_MMIO_RAM; /* FIRST member (scaffold §1) */\n");
    }
    tu.push_str(&format!(
        "\tstruct {inner_tag} inner;\n\tuint32_t __rseq_state; /* retained; zero in the static image */\n"
    ));
    if pcie {
        tu.push_str(
            "\tstruct pcie_dev *pcie; /* PCIe context (scaffold §1 item 4;\n\t                        * witness eth_e1000_priv.h:93) */\n",
        );
    }
    tu.push_str("};\n\n");
    if has_region {
        tu.push_str(&format!(
            "/* the TU-static base every MMIO op reads (A-2: the faithful first\n * realization, paired with the single-instance BUILD_ASSERT below) */\nstatic mm_reg_t {base_symbol};\n\n"
        ));
    }
    if pcie {
        tu.push_str(
            "/* the TU-static BDF/ID carrier config-op lowerings read (D12: the A-2\n * pattern extended — installed in the init fn after the BDF rung, before\n * the authored body; graduates to per-device threading with the\n * container-morph seam) */\nstatic struct pcie_dev *__rseq_pcie;\n\n",
        );
    }

    // Timed-cpu-delay-and-sleep lowering helpers (TCD construct, M2/T09),
    // injected before the bodies that call them, gated on the demand flags the
    // body lowering set when a busy_wait/sleep was emitted.
    if cx.uses_delay_busy || cx.uses_delay_sleep {
        tu.push_str(&body::delay_helpers(cx.uses_delay_busy, cx.uses_delay_sleep));
    }
    // §1 items 5-7: lowered helper/callback bodies, then the init function
    // — the one runtime dispatch hub (§3).
    tu.push_str(bodies);
    tu.push_str(init_fn);
    tu.push('\n');
    if let Some(deinit) = deinit_fn {
        tu.push_str(deinit);
        tu.push('\n');
    }

    // §1 item 8: the build-time instantiation block —
    // register/match/create/associate as build-time facts (§2); init level
    // hardcoded (D13); api NULL for ClassTag::Generic ONLY (A-16). With an
    // authored on_deinit the macro swaps to DEVICE_DT_INST_DEINIT_DEFINE
    // (device.h:272→243 — DEVICE_DT_DEFINE plus the deinit_fn arg);
    // unauthored stays byte-identical (C04/I05).
    tu.push_str("/* build-time register/match/create/associate (scaffold §2) */\n");
    if pcie {
        // The PCIe instantiation form (scaffold §1 item 8, Pci-only blocks;
        // witness eth_e1000.c:313-318): the iterable pcie_dev context is
        // declared per instance and the data struct's `pcie` member points
        // at it; the config struct carries no DEVICE_MMIO_ROM_INIT (no DT
        // `reg` on the PCIe form — C05's generated binding requires none).
        tu.push_str(&format!(
            "#define RSEQ_INIT(n)\\\n\tDEVICE_PCIE_INST_DECLARE(n);\\\n\tstatic struct {drv}_data {drv}_data_##n = {{\\\n\t\tDEVICE_PCIE_INST_INIT(n, pcie),\\\n\t}};\\\n"
        ));
        tu.push_str(&format!(
            "\tstatic const struct {drv}_config {drv}_config_##n;\\\n"
        ));
    } else {
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
    }
    if let Some(class) = deferred_class {
        // The C06 audit comment, immediately above the instantiation define
        // (D14: the declared class is NOT delivered — audited, never silent;
        // the define itself stays byte-identical to the Generic form).
        tu.push_str(&format!(
            "\t/* RSEQ-DEFERRED-CLASS: device_class \"{class}\" declared by the spec is NOT\\\n\t * delivered on this target: the class api vtable + macro family are the\\\n\t * deferred Device Class construct's territory (B-9); this device registers\\\n\t * class-neutral (NULL api). [license: {}] */\\\n",
            capability::DEFERRED_CLASS_LICENSE
        ));
    }
    if deinit_fn.is_some() {
        tu.push_str(&format!(
            "\tDEVICE_DT_INST_DEINIT_DEFINE(n, {drv}_init, {drv}_deinit, NULL,\\\n\t\t&{drv}_data_##n, &{drv}_config_##n,\\\n\t\tPOST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\\\n\t\tNULL /* api: ClassTag::Generic only (A-16) */);\n"
        ));
    } else {
        tu.push_str(&format!(
            "\tDEVICE_DT_INST_DEFINE(n, {drv}_init, NULL,\\\n\t\t&{drv}_data_##n, &{drv}_config_##n,\\\n\t\tPOST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\\\n\t\tNULL /* api: ClassTag::Generic only (A-16) */);\n"
        ));
    }
    tu.push_str("DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)\n\n");

    // §1 item 9: the mandatory single-instance guard (A-2/D18) — a
    // TU-static with >1 status-okay instance would silently alias one
    // base/carrier across devices; multi-instance models are rejected at C
    // compile time. On the PCIe arm the wording covers BOTH TU-statics
    // (base + __rseq_pcie, I07).
    if pcie {
        tu.push_str(
            "BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1,\n\t     \"TU-static MMIO base and __rseq_pcie carrier require a single status-okay instance\");\n",
        );
    } else {
        tu.push_str(
            "BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1,\n\t     \"TU-static MMIO base requires a single status-okay instance\");\n",
        );
    }

    // The A-1 cross-check (ratified D11): DT `reg` is the normative map
    // source; the platform.toml literals are REQUIRED and re-asserted here
    // against the DT at C compile time, with a named message. NOT emitted
    // on the PCIe arm: the PCIe form carries no DT `reg` (the C05 binding
    // requires none) and the L2 base/size literals are dead there (A-10(a)
    // — probe-time BAR sizing supplies the real extent).
    if !pcie {
        if let Some((base, size)) = resolved_region {
            tu.push_str(&format!(
                "\n/* A-1/D11: the DT `reg` the map consumes must agree with the L2\n * platform literals the spec was resolved against */\nBUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == {base:#x} &&\n\t     DT_REG_SIZE(DT_DRV_INST(0)) == {size:#x},\n\t     \"DT reg disagrees with the resolved [mmio] platform literals (D11)\");\n"
            ));
        }
    }

    Ok(tu)
}

/// The generated DT binding yaml (D07): `compatible:` from the match set —
/// the same string the TU pins — closing the compatible-drift channel;
/// description from the descriptor name; `base.yaml` included per the
/// vendored convention (arm_pl011.yaml). The PCIe form
/// (backend-zephyr-c-e1000 D10) additionally includes `pcie-device.yaml`
/// (the witnessed intel,e1000.yaml:8 idiom) so vendor-id/device-id
/// validate — required, with NO `reg` requirement — and renders each
/// ignored Pci row's audit comment adjacent to the consumed `compatible:`
/// surface (the R05 third render); the platform/Amba form is byte-frozen.
fn emit_binding_yaml(
    model: &RegseqModel,
    drv: &str,
    compatible: &str,
    pcie: bool,
    has_pinctrl: bool,
    ignored_audits: &[crate::match_audit::MatchAudit],
) -> String {
    let name = model.strings.resolve(
        model
            .device_lifecycle
            .drivers
            .values()
            .next()
            .expect("caller validated exactly one driver")
            .descriptor
            .name,
    );
    let mut out = format!(
        "# Regseq-generated devicetree binding ({drv}.c sidecar; GRILL D07).\n# Generated by the regseq zephyr_c backend; do not edit.\n\ndescription: {name} (Regseq-generated driver)\n\n"
    );
    // The DT binding `include:` set. `pinctrl-device.yaml` is added when the
    // driver has a RESOLVED pinctrl resource (pinctrl construct, M4/T21, Codex M4
    // #6): a real Zephyr build needs it so `pinctrl-0`/`pinctrl-names` validate
    // for `PINCTRL_DT_INST_DEFINE`. Absent for a pinctrl-less driver (byte-frozen).
    if pcie {
        for audit in ignored_audits {
            out.push_str(&audit.yaml_block());
        }
        let includes = if has_pinctrl {
            "[base.yaml, pcie-device.yaml, pinctrl-device.yaml]"
        } else {
            "[base.yaml, pcie-device.yaml]"
        };
        out.push_str(&format!(
            "compatible: \"{compatible}\"\n\ninclude: {includes}\n\nproperties:\n  vendor-id:\n    required: true\n  device-id:\n    required: true\n"
        ));
    } else if has_pinctrl {
        out.push_str(&format!(
            "compatible: \"{compatible}\"\n\ninclude: [base.yaml, pinctrl-device.yaml]\n"
        ));
    } else {
        out.push_str(&format!(
            "compatible: \"{compatible}\"\n\ninclude: base.yaml\n"
        ));
    }
    out
}
