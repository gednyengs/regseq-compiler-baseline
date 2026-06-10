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

    // -- body lowering: the T06–T08 banks ------------------------------------
    if !roots.is_empty() {
        // Fail-loud placeholder that shrinks as the lowering grows: T06
        // (MMIO width family), T07 (lifecycle/BRM + errno adapter +
        // authored bodies), T08 (LOG_* reassembly).
        return Err(EmitError::Unsupported(format!(
            "authored lifecycle body lowering (`{}`): delivered by the T06-T08 banks",
            roots[0].0
        )));
    }

    // -- demand facts ---------------------------------------------------------
    // The log-module block is demand-driven (scaffold §1 item 2): present
    // iff ≥1 log call site is reachable. With no authored bodies the demand
    // is structurally zero; T08 wires the real closure scan in.
    let log_demand = false;

    // -- TU assembly, scaffold §1 top to bottom ------------------------------
    let mut names = TypeNames::new();
    let tu = emit_tu(
        model, &mut names, &drv, compatible, driver_ty, driver, log_demand,
    )?;
    let binding_yaml = emit_binding_yaml(model, &drv, compatible, driver);

    Ok(ZephyrEmit {
        tu,
        binding_yaml,
        driver_name: drv,
    })
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
fn config_rejection(model: &RegseqModel, callee: InstanceId, op: &ConfigOp) -> EmitError {
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

/// Render the embedded spec-driver struct definition (A-17: the whole spec
/// driver lands in `data`; the Mmio resource fields are dead weight under
/// the TU-static base carrier and documented as such).
fn emit_driver_struct(
    model: &RegseqModel,
    names: &mut TypeNames,
    driver_ty: TyId,
    driver: &regseq_model::DriverLifecycle,
) -> Result<(String, String), EmitError> {
    let tag = names.name_of(model, driver_ty)?;
    let mut out = String::new();
    out.push_str(&format!(
        "/* the spec driver, embedded verbatim in the data image (A-17) */\nstruct {tag} {{\n"
    ));
    let mut any = false;
    for field in &driver.handle.resource_fields {
        let name = c_ident(model.strings.resolve(field.name));
        out.push_str(&format!(
            "\tmm_reg_t {name}; /* dead under the TU-static base carrier (A-2) */\n"
        ));
        any = true;
    }
    for field in &driver.handle.user_data_fields {
        let name = c_ident(model.strings.resolve(field.name));
        let cty = types::c_type(model, names, field.ty)?;
        out.push_str(&format!("\t{cty} {name};\n"));
        any = true;
    }
    if !any {
        out.push_str("\t/* fieldless spec driver */\n");
    }
    out.push_str("};\n");
    Ok((tag, out))
}

/// Assemble the TU per the scaffold §1 skeleton, top to bottom.
#[allow(clippy::too_many_arguments)]
fn emit_tu(
    model: &RegseqModel,
    names: &mut TypeNames,
    drv: &str,
    compatible: &str,
    driver_ty: TyId,
    driver: &regseq_model::DriverLifecycle,
    log_demand: bool,
) -> Result<String, EmitError> {
    let has_region = !driver.handle.resource_fields.is_empty();
    let (inner_tag, inner_def) = emit_driver_struct(model, names, driver_ty, driver)?;

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
    tu.push_str(" */\n\n");

    // §1 item 1: the compatible pin.
    tu.push_str(&format!(
        "#define DT_DRV_COMPAT {}\n\n",
        compat_token(compatible)
    ));

    // §1 item 2: the demand-driven log-module block (T08 wires the demand).
    if log_demand {
        tu.push_str(&format!(
            "#define LOG_MODULE_NAME {drv}\n#include <zephyr/logging/log.h>\nLOG_MODULE_REGISTER(LOG_MODULE_NAME);\n\n"
        ));
    }

    // §1 item 3: demand-driven includes.
    let mut includes: std::collections::BTreeSet<&str> =
        ["<zephyr/device.h>"].into_iter().collect();
    if has_region {
        includes.insert("<zephyr/sys/device_mmio.h>");
    }
    if !driver.handle.user_data_fields.is_empty() {
        includes.insert("<stdint.h>");
        includes.insert("<stdbool.h>");
    }
    for inc in &includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');

    // §1 item 4: per-instance state — config(ROM)/data(RAM), MMIO macros
    // FIRST members, the spec driver embedded in data (A-17), the retained
    // lifecycle state word (zero in the static image).
    tu.push_str(&inner_def);
    tu.push('\n');
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

    // §1 items 5-7: helper bodies (T06-T08 banks) + the init function — the
    // one runtime dispatch hub (§3). The resource preamble, the A-1
    // cross-check and the authored on_init body land with T07.
    tu.push_str(&format!(
        "/* the one runtime dispatch hub (scaffold §3) */\nstatic int {drv}_init(const struct device *dev)\n{{\n\tARG_UNUSED(dev);\n\treturn 0;\n}}\n\n"
    ));

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
