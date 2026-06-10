//! The linux_c backend: lowers a post-mono [`regseq_model::RegseqModel`]
//! directly to an idiomatic Linux kernel-module translation unit plus a
//! Kbuild fragment, under the ratified F-1 independent-lowering contract
//! (`docs/discussions/linux-backend/linux-open-questions.md` §F.1):
//! `generic_c` is the authoritative *reference* lowering and frozen oracle;
//! this module owns its own emission shape and shares no code with it.
//!
//! Emission ordering is Linux-native (GRILL D04): includes →
//! register/bitfield consts → driver state → lowered bodies → lifecycle
//! callbacks → match table → driver struct → module macros. The kernel's bus
//! core is the OS (`linux-scaffold-and-dispatch.md`): no `main`, no scenario
//! functions — the driver registers once via `module_amba_driver` and the
//! kernel drives `.probe`/`.remove`.
//!
//! First-light unit scope (GRILL D02): AMBA drivers only. Every surface
//! outside it — the PCI scaffold, non-AMBA buses, `poll::until`, MIR
//! constructs beyond the lowered subset — is a loud [`EmitError`] naming the
//! unsupported path and, where scheduled, the unit that will deliver it.
//! Structurally there is no emit-what-we-can branch (I06).

mod body;
mod naming;
mod types;

#[cfg(test)]
mod mod_tests;

use std::collections::BTreeMap;

use regseq_model::{
    AttrData, BusType, EnumShape, FrameworkType, InstanceId, InstanceKind, InstanceRef,
    LayoutKind, MatchEntry, RegseqModel, TerminatorKind, Ty, TyId,
};

use body::LowerCx;
use naming::c_ident;

/// The linux_c artifact set (GRILL D04/S09): one kernel-idiomatic TU plus
/// the Kbuild fragment that builds it out-of-tree.
#[derive(Debug)]
pub struct LinuxEmit {
    /// The `<driver>.c` translation unit.
    pub tu: String,
    /// The Kbuild fragment (`obj-m += <driver>.o`).
    pub kbuild: String,
    /// The module name the fragment references (`<driver>`, C-sanitized).
    pub module_name: String,
}

/// linux_c emission errors. Every failure path is a loud, named diagnostic
/// (I06): unsupported surfaces name the path and — when scheduled — the unit
/// that will deliver them; nothing silently truncates.
#[derive(Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A surface outside this unit's scope, with the unit that delivers it
    /// (GRILL D02/R04): e.g. the PCI scaffold → `backend/linux-c-e1000`.
    UnsupportedInUnit {
        /// The unsupported path, precisely named.
        what: String,
        /// The unit scheduled to deliver it.
        unit: &'static str,
    },
    /// A construct outside the current lowered subset, named. Fail-loud
    /// placeholder that shrinks as the lowering grows (T06–T09).
    Unsupported(String),
    /// The model's driver set cannot be lowered as a Linux module (no
    /// driver, several drivers, malformed descriptor...).
    Driver(String),
    /// A TU containing MMIO ops whose binding is not resolved — the ratified
    /// A-10 hard error naming the missing L2 literals.
    UnresolvedBinding(String),
    /// A marked `RegseqError` variant has no row in the C02 errno table.
    ErrnoMapping(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::UnsupportedInUnit { what, unit } => write!(
                f,
                "linux_c emission: {what} is not supported in this unit; it is delivered by {unit}"
            ),
            EmitError::Unsupported(what) => {
                write!(f, "linux_c emission: unsupported construct: {what}")
            }
            EmitError::Driver(what) => write!(f, "linux_c emission: {what}"),
            EmitError::UnresolvedBinding(what) => write!(
                f,
                "linux_c emission: unresolved MMIO binding (ratified A-10): {what}"
            ),
            EmitError::ErrnoMapping(variant) => write!(
                f,
                "linux_c emission: RegseqError variant `{variant}` has no C02 errno mapping"
            ),
        }
    }
}

impl std::error::Error for EmitError {}

/// The C02 per-target errno table, name-keyed (identical symbols on Linux;
/// the polarity — negation — is applied at the emission site, per the
/// ratified spec `linux-lowering-per-construct.md` §E.3).
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

/// Lower `model` to a Linux kernel-module TU + Kbuild fragment.
///
/// Mirrors the `emit_generic_c` function-API family (GRILL D01): the
/// function call *is* backend selection; variants (`_with`, `_for_entry`)
/// are added only when a consumer actually needs them.
pub fn emit_linux_c(model: &RegseqModel) -> Result<LinuxEmit, EmitError> {
    // -- the driver (exactly one for a single-module TU) -------------------
    let mut drivers = model.device_lifecycle.drivers.iter();
    let (driver_item, driver) = drivers
        .next()
        .ok_or_else(|| EmitError::Driver(
            "no recognized device-lifecycle driver in the model; a Linux kernel module needs \
             exactly one `#[rseq::driver]`"
                .into(),
        ))?;
    if drivers.next().is_some() {
        return Err(EmitError::Driver(
            "more than one recognized driver in the model; one Linux module per driver"
                .into(),
        ));
    }

    // -- unit scope gates (GRILL D02; R04) ----------------------------------
    match &driver.descriptor.bus_type {
        Some(BusType::Amba) => {}
        Some(BusType::Pci) => {
            return Err(EmitError::UnsupportedInUnit {
                what: "the PCI scaffold (bus = \"pci\": pci_driver, config space, BARs)".into(),
                unit: "backend/linux-c-e1000",
            })
        }
        Some(other) => {
            return Err(EmitError::UnsupportedInUnit {
                what: format!("the `{other:?}` bus scaffold"),
                unit: "a post-first-light unit",
            })
        }
        None => {
            return Err(EmitError::Driver(
                "driver declares no bus; the linux_c first-light unit lowers AMBA drivers \
                 only (declare `bus = \"amba\"`)"
                    .into(),
            ))
        }
    }
    for entry in &driver.descriptor.match_set {
        if !matches!(entry, MatchEntry::Amba { .. }) {
            return Err(EmitError::UnsupportedInUnit {
                what: format!("non-AMBA match entry {entry:?}"),
                unit: "a post-first-light unit",
            });
        }
    }
    if driver.descriptor.match_set.is_empty() {
        return Err(EmitError::Driver(
            "driver has an empty match set; an amba_id table needs at least one entry".into(),
        ));
    }

    let drv = c_ident(model.strings.resolve(driver.descriptor.name));

    // -- the driver struct type + resource-field substitution ---------------
    let driver_ty = driver_struct_ty(model, *driver_item).ok_or_else(|| {
        EmitError::Driver("driver struct type not present in the type interner".into())
    })?;
    let resource_names: Vec<String> = driver
        .handle
        .resource_fields
        .iter()
        .map(|f| c_ident(model.strings.resolve(f.name)))
        .collect();

    // -- the lowered-function closure (callbacks + transitive callees) ------
    let mut cx = LowerCx {
        model,
        names: types::TypeNames::new(),
        fn_symbols: BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
    };
    let callbacks = [
        ("on_init", &driver.ops.on_init),
        ("on_deinit", &driver.ops.on_deinit),
        ("on_surprise_remove", &driver.ops.on_surprise_remove),
        ("on_init_failure", &driver.ops.on_init_failure),
        ("on_system_shutdown", &driver.ops.on_system_shutdown),
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
    let closure = reachable_closure(model, roots.iter().map(|(_, id)| *id))?;
    // Symbols: `<drv>_<leaf>` with deterministic clash suffixes, callbacks
    // named first so they always get the bare leaf.
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

    // -- lowered bodies (D04 bank 4): forward prototypes for every lowered
    //    function first (callees may be emitted after their callers), then
    //    the definitions.
    let mut bodies = String::new();
    for id in &closure {
        bodies.push_str(&body::fn_head(&mut cx, *id)?);
        bodies.push_str(";\n");
    }
    bodies.push('\n');
    for id in &closure {
        bodies.push_str(&body::emit_fn(&mut cx, *id)?);
        bodies.push('\n');
    }

    // -- lifecycle callbacks: probe / remove (D04 bank 5) -------------------
    let scaffold = emit_scaffold(&mut cx, &drv, driver_ty, &resource_names, &roots)?;

    // -- type definitions (D04 bank 2/3), now that every use is known -------
    let type_defs = emit_type_defs(&mut cx, driver_ty, &resource_names)?;

    // -- match table + driver struct + module macros ------------------------
    let tail = emit_module_tail(model, &drv, driver)?;

    // -- includes (demand-light: the scaffold set; <linux/errno.h> rides the
    //    adapter, <linux/io.h> arrives with register access in T06) ---------
    let mut tu = String::new();
    tu.push_str(&format!(
        "// SPDX-License-Identifier: GPL-2.0\n/*\n * {drv}.c - Regseq-generated Linux driver ({} match).\n *\n * Generated by the regseq linux_c backend; do not edit.\n */\n\n",
        "AMBA"
    ));
    let mut includes: std::collections::BTreeSet<&str> = [
        "<linux/amba/bus.h>",
        "<linux/device.h>",
        "<linux/mod_devicetable.h>",
        "<linux/module.h>",
        "<linux/slab.h>",
        "<linux/string.h>",
        "<linux/types.h>",
    ]
    .into_iter()
    .collect();
    if scaffold.uses_errno {
        includes.insert("<linux/errno.h>");
    }
    includes.extend(cx.extra_includes.iter().copied());
    for inc in includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');
    tu.push_str(&type_defs);
    tu.push('\n');
    tu.push_str(&scaffold.container);
    tu.push_str(&bodies);
    tu.push_str(&scaffold.text);
    tu.push('\n');
    tu.push_str(&tail);

    Ok(LinuxEmit {
        kbuild: format!("obj-m += {drv}.o\n"),
        module_name: drv,
        tu,
    })
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

/// BFS over `Call(Direct)` edges from the callback roots; intrinsic-anchored
/// callees are intercepted at emission, not lowered as functions, so they do
/// not join the closure. Deterministic order: roots first, then discovery
/// order.
fn reachable_closure(
    model: &RegseqModel,
    roots: impl Iterator<Item = InstanceId>,
) -> Result<Vec<InstanceId>, EmitError> {
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
                if is_intrinsic_anchored(model, *callee) {
                    continue;
                }
                if !seen.contains(callee) {
                    queue.push(*callee);
                }
            }
        }
    }
    Ok(seen)
}

fn is_intrinsic_anchored(model: &RegseqModel, instance: InstanceId) -> bool {
    let item = model.instances[instance].item;
    if item.as_u32() as usize >= model.domain_attrs.item_attrs.len() {
        return false;
    }
    model.domain_attrs.item_attrs[item]
        .iter()
        .any(|attr| matches!(attr, AttrData::Intrinsic(_)))
}

/// The emitted scaffold pieces plus facts the include set needs.
struct Scaffold {
    /// The per-device private container definition (driver-state bank, D04).
    container: String,
    /// The `.probe`/`.remove` callback definitions (callback bank, D04).
    text: String,
    uses_errno: bool,
}

/// Synthesize the private container, `.probe`, and `.remove` for the AMBA
/// scaffold (`linux-scaffold-and-dispatch.md` §2–§4; canonical order
/// create → associate → preamble → on_init).
fn emit_scaffold(
    cx: &mut LowerCx<'_>,
    drv: &str,
    driver_ty: TyId,
    resource_names: &[String],
    roots: &[(&'static str, InstanceId)],
) -> Result<Scaffold, EmitError> {
    let model = cx.model;
    let driver_struct = cx.names.name_of(model, driver_ty)?;
    let find = |name: &str| roots.iter().find(|(n, _)| *n == name).map(|(_, id)| *id);
    let on_init = find("on_init");
    let on_deinit = find("on_deinit");
    if find("on_surprise_remove").is_some() {
        return Err(EmitError::Unsupported(
            "on_surprise_remove (the .remove present-predicate dispatch lands with T07)".into(),
        ));
    }
    if find("on_init_failure").is_some() {
        return Err(EmitError::Unsupported(
            "on_init_failure (the .probe goto-error ladder lands with T07)".into(),
        ));
    }
    if find("on_system_shutdown").is_some() {
        return Err(EmitError::Unsupported(
            "on_system_shutdown (the .drv.shutdown slot lands with T07)".into(),
        ));
    }

    // .probe always carries -ENOMEM (the alloc guard), so the TU always
    // includes <linux/errno.h>.
    let uses_errno = true;
    let mut out = String::new();

    // The per-device private container (the morph, `linux-lowering-per-construct.md`
    // §C.7): kernel device back-pointer + the spec driver embedded verbatim.
    // Emitted in the driver-state bank (D04), before the lowered bodies.
    let container = format!(
        "/* per-device private container: kernel back-pointer + the spec driver */\nstruct {drv}_priv {{\n\tstruct amba_device *adev;\n\tstruct {driver_struct} inner;\n}};\n\n"
    );

    // .probe
    out.push_str(&format!(
        "static int {drv}_probe(struct amba_device *adev, const struct amba_id *id)\n{{\n\tstruct {drv}_priv *priv;\n\n"
    ));
    out.push_str(
        "\tpriv = devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL);\n\tif (!priv)\n\t\treturn -ENOMEM;\n",
    );
    out.push_str("\tpriv->adev = adev;\n\tamba_set_drvdata(adev, priv);\n");
    if resource_names.is_empty() {
        out.push_str("\n\t/* preamble: no bus-managed regions in this model */\n");
    } else {
        // The BRM acquisition block (devm ioremap of &adev->res) lands with
        // T07; until then a region-bearing AMBA model is fail-loud.
        return Err(EmitError::Unsupported(
            "bus-managed MMIO region acquisition in .probe (lands with T07)".into(),
        ));
    }
    if let Some(id) = on_init {
        let sym = cx.fn_symbols[&id].clone();
        let adapter = emit_result_adapter(cx, id, &format!("{sym}(&priv->inner)"))?;
        out.push_str(&adapter);
    }
    out.push_str("\treturn 0;\n}\n\n");

    // .remove — orderly teardown; the present-predicate (surprise) dispatch
    // is T07's deliverable and the surprise callback is gated above.
    out.push_str(&format!(
        "static void {drv}_remove(struct amba_device *adev)\n{{\n\tstruct {drv}_priv *priv = amba_get_drvdata(adev);\n\n"
    ));
    match on_deinit {
        Some(id) => {
            let sym = cx.fn_symbols[&id].clone();
            // The teardown slots are void (PC I/AMBA .remove): the Result is
            // structurally discarded (CR-4), visibly.
            out.push_str(&format!("\t(void){sym}(&priv->inner);\n"));
        }
        None => out.push_str("\t/* on_deinit not overridden: default no-op teardown */\n\t(void)priv;\n"),
    }
    out.push_str("\t/* postamble: devm-managed resources release automatically (A-2) */\n}\n\n");

    Ok(Scaffold {
        container,
        text: out,
        uses_errno,
    })
}

/// The RegseqError → negated-errno boundary adapter, emitted at the `.probe`
/// return site (`linux-lowering-per-construct.md` §E.3: polarity + placement
/// are the only Linux deltas; symbols are the C02 nine).
fn emit_result_adapter(
    cx: &mut LowerCx<'_>,
    callee: InstanceId,
    call_expr: &str,
) -> Result<String, EmitError> {
    let model = cx.model;
    let InstanceKind::Function(data) = &model.instances[callee].kind else {
        return Err(EmitError::Driver("callback is not a function".into()));
    };
    let result_ty = model.sigs.resolve(data.fn_sig).output;
    let facts = result_carrier_facts(model, result_ty)?;
    let result_c = types::c_type(model, &mut cx.names, result_ty)?;

    let mut out = String::new();
    out.push_str(&format!("\n\t{{\n\t\t{result_c} __r = {call_expr};\n"));
    // Read the Result tag cell.
    out.push_str(&read_scalar(
        "__tag",
        "__r",
        facts.result_tag_offset,
        facts.result_tag_size,
    ));
    out.push_str(&format!(
        "\t\tif (__tag != {:#x}) {{\n",
        facts.ok_tag_value
    ));
    // The Err payload is the marked RegseqError; read its tag and map every
    // variant through the C02 table, negated. `default` is corrupt-
    // discriminant defense only (a known-but-unmapped variant is the
    // fail-loud EmitError above, never a silent EIO).
    out.push_str(&read_scalar(
        "__err",
        "__r",
        facts.err_tag_offset,
        facts.err_tag_size,
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

/// `u8`-exact scalar read out of an opaque carrier: emits a declaration of
/// `name` loaded from `<carrier>._raw[offset..offset+size]`.
fn read_scalar(name: &str, carrier: &str, offset: u64, size: u64) -> String {
    match size {
        1 => format!("\t\tu8 {name} = {carrier}._raw[{offset}];\n"),
        2 => format!("\t\tu16 {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 2);\n"),
        4 => format!("\t\tu32 {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 4);\n"),
        _ => format!("\t\tu64 {name}; memcpy(&{name}, &{carrier}._raw[{offset}], 8);\n"),
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
    /// `(E tag value, errno symbol)` per RegseqError variant, variant order.
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

    // Certification (per-construct spec §E.2): the carrier item must be the
    // certified core Result, and Ok's payload must be exactly unit/ZST.
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
            "callback return enum is not the certified core Result (item.is_result unset)"
                .into(),
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
                        "Result whose Ok variant is the untagged niche (first-light subset)"
                            .into(),
                    ));
                };
                ok_tag = Some(tag as u64);
            }
            "Err" => {
                let field = v.fields.first().ok_or_else(|| {
                    EmitError::Driver("Err variant with no payload field".into())
                })?;
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
        && model.domain_attrs.item_attrs[err_item]
            .iter()
            .any(|a| matches!(a, AttrData::FrameworkType(FrameworkType::RegseqError)));
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
/// substituted with their Linux carrier (`void __iomem *`, §C.7).
fn emit_type_defs(
    cx: &mut LowerCx<'_>,
    driver_ty: TyId,
    resource_names: &[String],
) -> Result<String, EmitError> {
    // Snapshot the names registered during body/scaffold emission, then emit
    // definitions; definition emission may register nested names, so loop to
    // a fixed point.
    let model = cx.model;
    let mut emitted: Vec<TyId> = Vec::new();
    let mut out_chunks: BTreeMap<TyId, String> = BTreeMap::new();
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
            let chunk = if ty == driver_ty {
                let rn = resource_names.to_vec();
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    if rn.iter().any(|r| r == fname) {
                        Some(format!("void __iomem *{fname}"))
                    } else {
                        None
                    }
                })?
            } else {
                types::emit_aggregate_def(model, &mut cx.names, ty, &|_| None)?
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

/// The match table, driver struct, registration macro, and module metadata
/// (`linux-scaffold-and-dispatch.md` §1; full match_set, zero-sentinel,
/// MODULE_DEVICE_TABLE).
fn emit_module_tail(
    model: &RegseqModel,
    drv: &str,
    driver: &regseq_model::DriverLifecycle,
) -> Result<String, EmitError> {
    let mut out = String::new();
    out.push_str(&format!("static const struct amba_id {drv}_ids[] = {{\n"));
    for entry in &driver.descriptor.match_set {
        let MatchEntry::Amba { id, mask } = entry else {
            unreachable!("non-AMBA entries rejected by the scope gate");
        };
        out.push_str(&format!(
            "\t{{\n\t\t.id\t= {id:#010x},\n\t\t.mask\t= {mask:#010x},\n\t}},\n"
        ));
    }
    out.push_str("\t{ 0, 0 },\n};\n\n");
    out.push_str(&format!("MODULE_DEVICE_TABLE(amba, {drv}_ids);\n\n"));

    out.push_str(&format!(
        "static struct amba_driver {drv}_driver = {{\n\t.drv = {{\n\t\t.name\t= \"{}\",\n\t}},\n\t.id_table\t= {drv}_ids,\n\t.probe\t\t= {drv}_probe,\n\t.remove\t\t= {drv}_remove,\n}};\n\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    out.push_str(&format!("module_amba_driver({drv}_driver);\n\n"));
    out.push_str(&format!(
        "MODULE_DESCRIPTION(\"Regseq-generated {} driver\");\nMODULE_LICENSE(\"GPL\");\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    Ok(out)
}
