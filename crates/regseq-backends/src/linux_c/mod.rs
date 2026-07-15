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
mod clock;
mod dma;
mod naming;
mod pinctrl;
mod serial;
mod sync;
mod types;

#[cfg(test)]
mod mod_tests;

use std::collections::BTreeMap;

use regseq_model::{
    AttrData, BusType, EnumShape, FrameworkType, InstanceId, InstanceKind, InstanceRef, LayoutKind,
    MatchEntry, RegseqModel, TerminatorKind, Ty, TyId,
};

use crate::capability;

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
    /// The per-target conformance report (amendment 6, C05): delivered /
    /// absent-dispatch / rejected slots with their ratified row IDs.
    /// linux_c delivers every authored slot it emits; `rejected_slots` is
    /// structurally empty — a rejection returns `Err` before this struct
    /// exists (I07).
    pub conformance: capability::ConformanceReport,
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

/// The two scaffolded bus arms (every other bus is a loud rejection in the
/// dispatch above the scaffold).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScaffoldBus {
    Amba,
    Pci,
}

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

/// Lower `model` to a Linux kernel-module TU + Kbuild fragment.
///
/// Mirrors the `emit_generic_c` function-API family (GRILL D01): the
/// function call *is* backend selection; variants (`_with`, `_for_entry`)
/// are added only when a consumer actually needs them.
pub fn emit_linux_c(model: &RegseqModel) -> Result<LinuxEmit, EmitError> {
    // -- the driver (exactly one for a single-module TU) -------------------
    let mut drivers = model.device_lifecycle.drivers.iter();
    let (driver_item, driver) = drivers.next().ok_or_else(|| {
        EmitError::Driver(
            "no recognized device-lifecycle driver in the model; a Linux kernel module needs \
             exactly one `#[rseq::driver]`"
                .into(),
        )
    })?;
    if drivers.next().is_some() {
        return Err(EmitError::Driver(
            "more than one recognized driver in the model; one Linux module per driver".into(),
        ));
    }

    // -- unit scope gates (GRILL D02; R04; e1000-unit PCI arm) ---------------
    let bus = match &driver.descriptor.bus_type {
        Some(BusType::Amba) => ScaffoldBus::Amba,
        Some(BusType::Pci) => ScaffoldBus::Pci,
        Some(other) => {
            return Err(EmitError::UnsupportedInUnit {
                what: format!("the `{other:?}` bus scaffold"),
                unit: "a post-first-light unit",
            })
        }
        None => {
            return Err(EmitError::Driver(
                "driver declares no bus; linux_c lowers AMBA and PCI drivers only \
                 (declare `bus = \"amba\"` or `bus = \"pci\"`)"
                    .into(),
            ))
        }
    };
    if model.serial_port.drivers.contains_key(driver_item) && bus != ScaffoldBus::Amba {
        return Err(EmitError::UnsupportedInUnit {
            what: "a non-AMBA locked serial-port binding".into(),
            unit: "the Linux serial-framework-binding for the selected bus",
        });
    }
    if driver.descriptor.match_set.is_empty() {
        return Err(EmitError::Driver(
            "driver has an empty match set; a kernel id table needs at least one entry".into(),
        ));
    }
    // -- the C06 mixed-match rule (spec-convergence-pl011 D08/C06, lifting
    //    both former per-entry rejects): consume every NATIVE entry (native =
    //    the scaffold's ratified registration form, NOT the OS — Linux could
    //    carry an of_match_table, but linux_c's ratified forms are
    //    amba_driver/pci_driver and never-synthesize binds); ignore each
    //    recognized FOREIGN entry with a triple-rendered MatchAudit; reject
    //    an all-foreign set loudly naming the fix (the A-13 analog). The
    //    filtered native list — not the full match_set — feeds the table
    //    emitters (A2: their non-native arms are unreachable! by this
    //    construction).
    let ratified_form = match bus {
        ScaffoldBus::Amba => "amba_driver",
        ScaffoldBus::Pci => "pci_driver",
    };
    let mut native_entries: Vec<&MatchEntry> = Vec::new();
    let mut ignored_audits: Vec<crate::match_audit::MatchAudit> = Vec::new();
    for entry in &driver.descriptor.match_set {
        // The D03(a) license covers RECOGNIZED foreign entries only: an
        // entry with no ratified registration form on ANY backend (the
        // deferred acpi/usb/i2c/spi families) keeps the D.2 fail-loud
        // default — never audited-and-ignored.
        if !crate::match_audit::has_ratified_form(entry) {
            return Err(EmitError::Driver(format!(
                "match entry {entry:?} has no ratified registration form on any backend \
                 (deferred bus family); the D03(a) rule licenses ignoring recognized \
                 foreign-transport entries only — fail-loud (D.2 default)"
            )));
        }
        let native = matches!(
            (bus, entry),
            (ScaffoldBus::Amba, MatchEntry::Amba { .. })
                | (ScaffoldBus::Pci, MatchEntry::Pci { .. })
        );
        if native {
            native_entries.push(entry);
        } else {
            ignored_audits.push(crate::match_audit::MatchAudit::foreign(
                model,
                entry,
                ratified_form,
            ));
        }
    }
    if native_entries.is_empty() {
        return Err(EmitError::Driver(format!(
            "match set carries no native entries for the ratified {ratified_form} registration \
             form (all {} recognized entries are foreign-transport); the D03(a) rule licenses \
             ignoring foreign entries only when >=1 native entry exists — author a native {} \
             entry for this scaffold",
            driver.descriptor.match_set.len(),
            match bus {
                ScaffoldBus::Amba => "`amba = [{ id, mask }]`",
                ScaffoldBus::Pci => "`pci = [{ vendor, device }]`",
            },
        )));
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

    // -- driver facts for register-access lowering (T06) --------------------
    let mut resources = Vec::new();
    for f in &driver.handle.resource_fields {
        let c_name = c_ident(model.strings.resolve(f.name));
        let join_id = f
            .binding
            .id
            .map(|id| model.strings.resolve(id).to_string())
            .unwrap_or_else(|| c_name.clone());
        resources.push((c_name, f.binding.is_resolved(), join_id));
    }
    let resource_c_names: Vec<String> = resources.iter().map(|(n, _, _)| n.clone()).collect();
    let reg_macros = register_macros(model, driver_ty, &drv, &resource_c_names)?;

    // -- the lowered-function closure (callbacks + transitive callees) ------
    let mut cx = LowerCx {
        model,
        names: types::TypeNames::new(),
        fn_symbols: BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: body::DriverFacts {
            driver_ty,
            resources,
            reg_macros,
            bus_type: driver.descriptor.bus_type.clone(),
            drv: drv.clone(),
        },
        width8_used: false,
        uses_pr: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
        uses_command_transport: false,
        elided_locals: std::collections::BTreeSet::new(),
    };

    // -- synchronization (locks, T07/C02): the lock FIELDS substitute their
    //    declaration to the per-OS lock object (`field_override`, below) and pull
    //    in the per-kind header; the body acquire/release/completion CALLS are
    //    replaced by the C02 primitives in `body::emit_fn`. ------------------
    let mut sync_field_decls: BTreeMap<String, String> = BTreeMap::new();
    if let Some(sync) = model.synchronization.drivers.get(driver_item) {
        for res in &sync.resources {
            let fname = c_ident(model.strings.resolve(res.name));
            for inc in sync::includes_for(res.kind) {
                cx.extra_includes.insert(inc);
            }
            sync_field_decls.insert(fname.clone(), sync::lock_field_decl(res.kind, &fname));
        }
    }

    // -- peripheral-clock-management (clock construct, M3/T14, C04/D08): a MANAGED
    //    clock FIELD morphs to a `struct clk *` member (the field_override, below);
    //    the acquire preamble (`devm_clk_get`) rides the probe scaffold. A STATIC
    //    clock leaves the ZST `Clock` field dropped. `<linux/clk.h>` declares the
    //    morphed type + the clk_* ops; `<linux/err.h>` carries IS_ERR/PTR_ERR. ----
    let clock_field_decls = clock::field_decls(model, driver_ty);
    if !clock_field_decls.is_empty() {
        cx.extra_includes.insert("<linux/clk.h>");
        cx.extra_includes.insert("<linux/err.h>");
    }

    // -- pin-controller-and-GPIO-configuration (pinctrl construct, M4/T20,
    //    C04/C07/D10): a RESOLVED REQUIRED `PinCtrl` FIELD morphs to a `struct
    //    pinctrl *` member (the field_override, below) + the acquire preamble
    //    (`devm_pinctrl_get` + select default) rides the probe scaffold; an
    //    OPTIONAL `Option<PinCtrl>` keeps no field (handle-less PM helpers). Only
    //    a RESOLVED resource pulls the consumer header (absent-only TU
    //    byte-unchanged, I07/Codex M4 #5). `<linux/pinctrl/consumer.h>` declares
    //    the morphed handle + the pinctrl ops; `<linux/err.h>` carries
    //    IS_ERR/PTR_ERR. ---------------------------------------------------------
    let pinctrl_field_decls = pinctrl::field_decls(model, driver_ty);
    if pinctrl::has_resolved_pinctrl(model, driver_ty) {
        cx.extra_includes.insert("<linux/pinctrl/consumer.h>");
        cx.extra_includes.insert("<linux/err.h>");
    }

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
    let serial_roots = serial::roots(model, *driver_item)?;
    roots.extend(serial_roots.iter().copied());
    let work_handlers: Vec<InstanceId> = model
        .deferred_work
        .drivers
        .get(driver_item)
        .map(|d| {
            d.resources
                .iter()
                .map(|r| {
                    r.handler
                        .and_then(|h| resolve_instance_ref(model, h))
                        .ok_or_else(|| EmitError::Driver("deferred-work handler does not resolve to an instance".into()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if !work_handlers.is_empty() {
        cx.extra_includes.insert("<linux/workqueue.h>");
        cx.extra_includes.insert("<linux/container_of.h>");
    }
    // -- device-interrupt facts (construct #6, T09): guards + the handler
    //    body joins the lowered closure (its symbol comes from the leaf
    //    loop; the irqreturn_t WRAPPER is scaffold-synthesized) ------------
    let irq = linux_irq_facts(model, driver_item, &drv, bus)?;
    let closure = reachable_closure(
        model,
        roots
            .iter()
            .map(|(_, id)| *id)
            .chain(irq.iter().map(|i| i.handler))
            .chain(work_handlers.iter().copied()),
    )?;
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
    let with_fn_context = |err: EmitError, model: &RegseqModel, id: InstanceId| match err {
        EmitError::Unsupported(s) => EmitError::Unsupported(format!(
            "{s} [while lowering `{}`]",
            model.strings.resolve(model.instances[id].emission_name)
        )),
        other => other,
    };
    for id in &closure {
        let head = body::fn_head(&mut cx, *id).map_err(|e| with_fn_context(e, model, *id))?;
        bodies.push_str(&head);
        bodies.push_str(";\n");
    }
    bodies.push('\n');
    for id in &closure {
        let f = body::emit_fn(&mut cx, *id).map_err(|e| with_fn_context(e, model, *id))?;
        bodies.push_str(&f);
        bodies.push('\n');
    }

    // -- lifecycle callbacks: probe / remove (D04 bank 5) -------------------
    let scaffold = match bus {
        ScaffoldBus::Amba => emit_scaffold(
            &mut cx,
            &drv,
            driver_ty,
            &resource_names,
            &roots,
            irq.as_ref(),
        )?,
        ScaffoldBus::Pci => {
            emit_scaffold_pci(&mut cx, &drv, driver_ty, &resource_names, &roots, driver)?
        }
    };

    let serial_binding = serial::emit(&mut cx, *driver_item, &drv)?;
    let dma_binding = dma::emit(&mut cx, *driver_item, &drv);

    // -- type definitions (D04 bank 2/3), now that every use is known -------
    let type_defs = emit_type_defs(
        &mut cx,
        driver_ty,
        &resource_names,
        &sync_field_decls,
        &clock_field_decls,
        &pinctrl_field_decls,
    )?;

    // -- match table + driver struct + module macros ------------------------
    let tail = match bus {
        ScaffoldBus::Amba => emit_module_tail(
            model,
            &drv,
            driver,
            &native_entries,
            &ignored_audits,
            scaffold.shutdown_sym.as_deref(),
        )?,
        ScaffoldBus::Pci => emit_module_tail_pci(
            model,
            &drv,
            driver,
            &native_entries,
            &ignored_audits,
            scaffold.shutdown_sym.as_deref(),
        )?,
    };

    // -- includes (demand-light: the scaffold set; <linux/errno.h> rides the
    //    adapter, <linux/io.h> arrives with register access in T06) ---------
    let mut tu = String::new();
    tu.push_str(&format!(
        "// SPDX-License-Identifier: GPL-2.0\n/*\n * {drv}.c - Regseq-generated Linux driver ({} match).\n *\n * Generated by the regseq linux_c backend; do not edit.\n */\n\n",
        match bus {
            ScaffoldBus::Amba => "AMBA",
            ScaffoldBus::Pci => "PCI",
        }
    ));
    if cx.uses_pr {
        // The pr_fmt module prefix must precede every include (A-3 / kernel
        // convention).
        tu.push_str("#define pr_fmt(fmt) KBUILD_MODNAME \": \" fmt\n\n");
    }
    let bus_include = match bus {
        ScaffoldBus::Amba => "<linux/amba/bus.h>",
        ScaffoldBus::Pci => "<linux/pci.h>",
    };
    let mut includes: std::collections::BTreeSet<&str> = [
        bus_include,
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
    if cx.uses_pr {
        includes.insert("<linux/printk.h>");
    }
    includes.extend(cx.extra_includes.iter().copied());
    for inc in includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');
    if cx.width8_used {
        // A-12 (ratified): width-8 MMIO lowers to readq/writeq for
        // CONFIG_64BIT targets only — never a silent non-atomic split.
        tu.push_str(
            "#ifndef CONFIG_64BIT\n#error \"this Regseq-generated driver performs 64-bit MMIO and requires CONFIG_64BIT (ratified A-12: no io-64-nonatomic split)\"\n#endif\n\n",
        );
    }
    if !cx.driver.reg_macros.is_empty() {
        tu.push_str("/* register offsets, recovered from the spec's #[repr(C)] block layout */\n");
        for (name, off) in &cx.driver.reg_macros {
            tu.push_str(&format!("#define {name} {off:#05x}\n"));
        }
        tu.push('\n');
    }
    tu.push_str(&type_defs);
    tu.push('\n');
    if cx.uses_command_transport {
        tu.push_str(&format!(
            "/* L2 resolves and owns this non-blocking mailbox binding. */\nstruct {drv}_command_transport_endpoint {{\n\tstruct mbox_chan *channel;\n\tconst struct mbox_client *client;\n}};\n\n"
        ));
    }
    tu.push_str(&scaffold.container);
    // Timed-cpu-delay-and-sleep lowering helpers (TCD construct, M2/T08),
    // injected before the bodies that call them, gated on the demand flags the
    // body lowering set when a busy_wait/sleep was emitted.
    if cx.uses_delay_busy || cx.uses_delay_sleep {
        tu.push_str(&body::delay_helpers(cx.uses_delay_busy, cx.uses_delay_sleep));
    }
    tu.push_str(&bodies);
    tu.push_str(&serial_binding);
    tu.push_str(&dma_binding);
    tu.push_str(&scaffold.text);
    tu.push('\n');
    tu.push_str(&tail);

    // -- the per-target conformance report (amendment 6, C05): every
    //    authored slot that emits is delivered, in LifecycleOps declaration
    //    order (the callbacks-array order above); the PCI surprise slot is
    //    the one table-ratified delivery (D16(b): row ID iff a ratified row
    //    exists, rendered from the const — never a literal); rejected_slots
    //    structurally empty (I07).
    let mut delivered_slots: Vec<capability::SlotEntry> = roots
        .iter()
        .map(|(name, _)| capability::SlotEntry {
            slot: (*name).to_string(),
            row: (bus == ScaffoldBus::Pci && *name == "on_surprise_remove")
                .then(|| capability::CAP_LINUX_PCI_SURPRISE_1.id.to_string()),
            citation: None,
        })
        .collect();
    // Device-interrupt deregister slot (construct #6, T12 linux leg): the
    // manual `free_irq` DELIVERS the deregister operation on Linux — the D09
    // contrast with zephyr's structurally-absent static-connect deregister
    // (C02). No ratified row backs a delivery; the slot string is the
    // non-lifecycle domain spelling the capability schema widens to (T10).
    if irq.is_some() {
        delivered_slots.push(capability::SlotEntry {
            slot: "interrupt.register_handler.deregister".to_string(),
            row: None,
            citation: None,
        });
    }
    let conformance = capability::ConformanceReport {
        target: "linux_c",
        os_version_pin: capability::LINUX_PIN,
        delivered_slots,
        absent_dispatch_slots: Vec::new(),
        structurally_absent_slots: Vec::new(),
        rejected_slots: Vec::new(),
        ignored_match_entries: ignored_audits,
        // linux_c carries a declared class unvalidated (the D14 evidence:
        // the class annotation surfaces nowhere in the linux artifacts);
        // the deferred-class note is the zephyr_c disposition's.
        deferred_class: None,
    };

    Ok(LinuxEmit {
        kbuild: format!("obj-m += {drv}.o\n"),
        module_name: drv,
        tu,
        conformance,
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
        let body_ref = model.instance_ref_of(id);
        for (idx, bb) in b.basic_blocks.iter().enumerate() {
            if let TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                ..
            } = &bb.terminator.kind
            {
                // A recognized sync acquire/release/completion `Call` is replaced
                // by its C02 primitive (T07) — its callee (`lock()`, the guard's
                // drop glue, `signal`/`wait`/`reset`) is NEVER lowered, exactly
                // like an intrinsic-anchored or trivial-accessor callee.
                if sync::is_sync_call_block(model, body_ref, regseq_model::BlockId::from_u32(idx as u32))
                    || is_intrinsic_anchored(model, *callee)
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

/// The device-interrupt lowering facts for one driver (construct #6, T09).
struct LinuxIrq {
    /// The lowered handler-body instance (joins the closure; its symbol comes
    /// from the leaf naming loop).
    handler: InstanceId,
    /// The `request_irq` name string (the descriptor's logical name).
    dev_name: String,
    /// The `IrqResult` tag cell + the `Handled` tag value, for the wrapper's
    /// C04 mapping (`Handled -> IRQ_HANDLED`, everything else `IRQ_NONE` —
    /// never suppressed, R08; `DeferWork` is guard-proven unreachable).
    ret_ty: TyId,
    tag_offset: u64,
    tag_size: u64,
    handled_tag: u64,
}

impl LinuxIrq {
    /// The irqreturn_t wrapper symbol (C04 names it `rseq_isr_<drv>`).
    fn wrapper_sym(&self, drv: &str) -> String {
        format!("rseq_isr_{drv}")
    }

    /// The request flags: registered DISABLED (D02). A shared line is
    /// emit-rejected upstream — `IRQF_NO_AUTOEN | IRQF_SHARED` is `-EINVAL`
    /// in the kernel (`kernel/irq/manage.c` `__setup_irq`), so the PLAN's
    /// flag formula cannot be lowered as written (Codex M3 MUST-FIX; the
    /// shared-line registration contract is a recorded design amendment for
    /// the unit that lands a shared-irq fixture).
    fn flags(&self) -> &'static str {
        "IRQF_NO_AUTOEN"
    }
}

/// Build the device-interrupt facts for the driver, firing the emit-stage
/// guards (I05 — loud, never partial): single `Single`-form resource only
/// this slice (D06/R02), AMBA only (the PCI `pdev->irq` leg arrives with an
/// e1000 irq fixture), `DeferWork` proven unreachable (R09/D04), and the
/// handler's `IrqResult` must be the marked framework type with a `Handled`
/// arm. `Ok(None)` for an interrupt-less driver — zero churn. The line
/// identity is runtime-resolved (`adev->irq[0]`, R10); `irq_number` is not
/// consumed here.
fn linux_irq_facts(
    model: &RegseqModel,
    driver_item: &regseq_model::ItemId,
    drv: &str,
    bus: ScaffoldBus,
) -> Result<Option<LinuxIrq>, EmitError> {
    let Some(interrupts) = model.interrupt.drivers.get(driver_item) else {
        return Ok(None);
    };
    if interrupts.resources.is_empty() {
        return Ok(None);
    }
    if bus == ScaffoldBus::Pci {
        return Err(EmitError::UnsupportedInUnit {
            what: "the PCI interrupt scaffold (`pdev->irq` line identity)".to_string(),
            unit: "the unit that lands an e1000 irq fixture",
        });
    }
    if interrupts.resources.len() > 1 {
        return Err(EmitError::Unsupported(
            "more than one Interrupt resource per driver (AMBA `adev->irq[0]` single-line \
             slice)"
                .into(),
        ));
    }
    let resource = &interrupts.resources[0];
    if resource.binding.shared {
        // The kernel REJECTS `IRQF_NO_AUTOEN | IRQF_SHARED` with -EINVAL
        // (`kernel/irq/manage.c` `__setup_irq`): a shared line cannot be held
        // request-disabled, so the D02 registered-disabled contract and the
        // D05 shared flag cannot compose as the PLAN's flag formula wrote
        // them. Fail loud (I05) rather than emit a module whose probe always
        // fails; the shared-line registration contract is a recorded design
        // amendment for the unit that lands a shared-irq fixture (e1000,
        // whose PCI leg is deferred anyway).
        return Err(EmitError::Unsupported(
            "device-interrupt: a shared line (L2 `shared = true`) cannot be lowered with \
             the registered-disabled contract — the kernel rejects \
             IRQF_NO_AUTOEN | IRQF_SHARED with -EINVAL; the shared-line registration \
             form needs a design amendment (deferred to the shared-irq fixture unit)"
                .into(),
        ));
    }
    if !matches!(resource.form, regseq_model::InterruptForm::Single) {
        return Err(EmitError::Unsupported(
            "device-interrupt: Option<Interrupt> / [Interrupt; N] lowering is not \
             implemented this slice (the form is recognized; emit-stage rejection per \
             GRILL D06/R02)"
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

    // The IrqResult tag cell + the Handled arm's tag, by variant NAME (the
    // C04 contract names the arms; the marker certifies the type).
    let InstanceKind::Function(data) = &model.instances[handler_id].kind else {
        return Err(EmitError::Driver("handler is not a function".into()));
    };
    let ret_ty = model.sigs.resolve(data.fn_sig).output;
    let ret_item = match &model.types[ret_ty] {
        Ty::Enum { item, .. } => *item,
        other => {
            return Err(EmitError::Driver(format!(
                "handler return type is not a nominal enum: {other:?}"
            )))
        }
    };
    let marked = (ret_item.as_u32() as usize) < model.domain_attrs.item_attrs.len()
        && model.domain_attrs.item_attrs[ret_item]
            .iter()
            .any(|a| matches!(a, AttrData::FrameworkType(FrameworkType::IrqResult)));
    if !marked {
        return Err(EmitError::Driver(
            "handler return enum is not the marked irq-result framework type".into(),
        ));
    }
    let layout = types::layout_of(model, ret_ty)
        .ok_or_else(|| EmitError::Driver("IrqResult has no layout".into()))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(EmitError::Driver(
            "IrqResult is not a multi-variant enum".into(),
        ));
    };
    let handled_tag = variants
        .iter()
        .find(|v| model.strings.resolve(v.name) == "Handled")
        .and_then(|v| v.tag_value)
        .ok_or_else(|| EmitError::Driver("IrqResult lacks a tagged Handled arm".into()))?;
    let (tag_offset, tag_size) = types::enum_tag_cell(model, ret_ty)?;

    // The descriptor name doubles as the request_irq name string.
    let driver = &model.device_lifecycle.drivers[driver_item];
    let dev_name = model.strings.resolve(driver.descriptor.name).to_string();
    let _ = drv;
    Ok(Some(LinuxIrq {
        handler: handler_id,
        dev_name,
        ret_ty,
        tag_offset,
        tag_size,
        handled_tag: handled_tag as u64,
    }))
}

/// The emitted scaffold pieces plus facts the include set needs.
struct Scaffold {
    /// The per-device private container definition (driver-state bank, D04).
    container: String,
    /// The `.probe`/`.remove`/`.shutdown` callback definitions (callback
    /// bank, D04).
    text: String,
    uses_errno: bool,
    /// The shutdown wrapper symbol iff `on_system_shutdown` is overridden
    /// (the omit-when-absent `.drv.shutdown` initializer).
    shutdown_sym: Option<String>,
}

struct LinuxWorkEmission {
    fields: String,
    wrappers: String,
    init: String,
    cancel: String,
}

fn linux_work_emission(
    cx: &body::LowerCx<'_>,
    drv: &str,
    driver_ty: TyId,
) -> Result<LinuxWorkEmission, EmitError> {
    let driver_item = match &cx.model.types[driver_ty] {
        Ty::Struct { item, .. } => *item,
        _ => return Err(EmitError::Driver("driver type is not a struct".into())),
    };
    let Some(work) = cx.model.deferred_work.drivers.get(&driver_item) else {
        return Ok(LinuxWorkEmission { fields: String::new(), wrappers: String::new(), init: String::new(), cancel: String::new() });
    };
    let mut fields = String::new();
    let mut wrappers = String::new();
    let mut init = String::new();
    let mut cancel = String::new();
    for (idx, resource) in work.resources.iter().enumerate() {
        let handler = resource
            .handler
            .and_then(|h| resolve_instance_ref(cx.model, h))
            .and_then(|id| cx.fn_symbols.get(&id))
            .ok_or_else(|| EmitError::Driver("deferred-work handler symbol is unavailable".into()))?;
        fields.push_str(&format!("\tstruct work_struct __rseq_work_{idx};\n"));
        wrappers.push_str(&format!(
            "static void {drv}_work_{idx}(struct work_struct *work)\n{{\n\tstruct {drv}_priv *priv = container_of(work, struct {drv}_priv, __rseq_work_{idx});\n\n\t{handler}(&priv->inner);\n}}\n\n"
        ));
        init.push_str(&format!("\tINIT_WORK(&priv->__rseq_work_{idx}, {drv}_work_{idx});\n"));
    }
    for idx in (0..work.resources.len()).rev() {
        cancel.push_str(&format!("\tcancel_work_sync(&priv->__rseq_work_{idx});\n"));
    }
    Ok(LinuxWorkEmission { fields, wrappers, init, cancel })
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
    irq: Option<&LinuxIrq>,
) -> Result<Scaffold, EmitError> {
    let model = cx.model;
    let driver_item = match &model.types[driver_ty] {
        Ty::Struct { item, .. } => *item,
        _ => return Err(EmitError::Driver("driver type is not a struct".into())),
    };
    let driver_struct = cx.names.name_of(model, driver_ty)?;
    let find = |name: &str| roots.iter().find(|(n, _)| *n == name).map(|(_, id)| *id);
    let on_init = find("on_init");
    let on_deinit = find("on_deinit");
    if find("on_surprise_remove").is_some() {
        // The spec's present-predicate examples (pci_device_is_present /
        // pci_channel_offline) are PCI-shaped; no AMBA surprise-removal
        // predicate is pinned by the ratified spec, and AMBA devices do not
        // hot-unplug on the first-light target. Fail loud rather than guess.
        return Err(EmitError::Unsupported(
            "on_surprise_remove on an AMBA driver (no ratified present predicate; \
             the PCI dispatch arrives with backend/linux-c-e1000)"
                .into(),
        ));
    }
    let on_init_failure = find("on_init_failure");
    let on_system_shutdown = find("on_system_shutdown");

    // .probe always carries -ENOMEM (the alloc guard), so the TU always
    // includes <linux/errno.h>.
    let uses_errno = true;
    let mut out = String::new();
    let work = linux_work_emission(cx, drv, driver_ty)?;

    // The per-device private container (the morph, `linux-lowering-per-construct.md`
    // §C.7): kernel device back-pointer + the spec driver embedded verbatim.
    // Emitted in the driver-state bank (D04), before the lowered bodies.
    let container = format!(
        "/* per-device private container: kernel back-pointer + the spec driver */\nstruct {drv}_priv {{\n\tstruct amba_device *adev;\n\tstruct {driver_struct} inner;\n{}{}}};\n\n",
        serial::container_field(model, driver_item),
        work.fields,
    );
    out.push_str(&work.wrappers);

    // The irqreturn_t ISR wrapper (construct #6, C04): recover the driver
    // from the registered dev_id (the priv container), run the authored
    // handler, map the dispatch result — `Handled -> IRQ_HANDLED`, everything
    // else `IRQ_NONE`, never suppressed (R08; `DeferWork` is guard-proven
    // unreachable). The wrapper acquires NO locks (R18/D11 — the
    // ISR↔mainline race is the documented phase limitation until the
    // Synchronization construct lands). Emitted before `.probe`, which
    // passes it to `request_irq`.
    if let Some(irq) = irq {
        cx.extra_includes.insert("<linux/interrupt.h>");
        let wrapper = irq.wrapper_sym(drv);
        let handler_sym = cx.fn_symbols[&irq.handler].clone();
        let ret_c = types::c_type(cx.model, &mut cx.names, irq.ret_ty)?;
        let tag_read = match irq.tag_size {
            1 => format!("\tu8 __tag = __r._raw[{}];\n", irq.tag_offset),
            2 => format!(
                "\tu16 __tag; memcpy(&__tag, &__r._raw[{}], 2);\n",
                irq.tag_offset
            ),
            4 => format!(
                "\tu32 __tag; memcpy(&__tag, &__r._raw[{}], 4);\n",
                irq.tag_offset
            ),
            _ => format!(
                "\tu64 __tag; memcpy(&__tag, &__r._raw[{}], 8);\n",
                irq.tag_offset
            ),
        };
        out.push_str(&format!(
            "/* the bound ISR (construct #6, C04): no locks (R18) */\nstatic irqreturn_t {wrapper}(int irq, void *dev_id)\n{{\n\tstruct {drv}_priv *priv = dev_id;\n\t{ret_c} __r = {handler_sym}(&priv->inner);\n{tag_read}\n\tif (__tag == {:#x})\n\t\treturn IRQ_HANDLED;\n\treturn IRQ_NONE;\n}}\n\n",
            irq.handled_tag
        ));
    }

    // .probe
    out.push_str(&format!(
        "static int {drv}_probe(struct amba_device *adev, const struct amba_id *id)\n{{\n\tstruct {drv}_priv *priv;\n\n"
    ));
    out.push_str(
        "\tpriv = devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL);\n\tif (!priv)\n\t\treturn -ENOMEM;\n",
    );
    out.push_str("\tpriv->adev = adev;\n\tamba_set_drvdata(adev, priv);\n");
    out.push_str(&serial::probe_init(model, driver_item, drv));
    // Synchronization init_primitives (C05/I05): initialize the declared locks
    // BEFORE the region map + IRQ register + on_init (lock-live ⊇ line-live).
    let sync_inits = sync::init_primitive_lines(model, driver_ty);
    if !sync_inits.is_empty() {
        out.push_str("\n\t/* init_primitives (C05): locks initialized before region/IRQ/on_init */\n");
        for line in &sync_inits {
            out.push_str(&format!("\t{line}\n"));
        }
    }
    if resource_names.is_empty() {
        out.push_str("\n\t/* preamble: no bus-managed regions in this model */\n");
    } else {
        if resource_names.len() > 1 {
            return Err(EmitError::Unsupported(
                "more than one MMIO resource region per driver (model gate B-5; \
                 single-region only)"
                    .into(),
            ));
        }
        let fname = &resource_names[0];
        let resolved = cx.driver.resources.iter().any(|(n, r, _)| n == fname && *r);
        if resolved {
            // The canonical acquisition (integration doc §2, AMBA column):
            // get_handle = &adev->res; map_region = devm_ioremap_resource
            // (claim∘map FUSED on the devm path — claim_regions is a no-op
            // here, not silently dropped: ratified A-2, invariant 6). The
            // failure is the NATIVE channel: PTR_ERR propagates directly,
            // never wrapped in RegseqError (-EPROBE_DEFER survives).
            cx.extra_includes.insert("<linux/io.h>");
            cx.extra_includes.insert("<linux/err.h>");
            out.push_str(&format!(
                "\n\t/* preamble: claim+map the AMBA region (devm: claim_regions fuses\n\t * into the ioremap and the postamble unwinds automatically, A-2) */\n\tpriv->inner.{fname} = devm_ioremap_resource(&adev->dev, &adev->res);\n\tif (IS_ERR(priv->inner.{fname}))\n\t\treturn PTR_ERR(priv->inner.{fname});\n"
            ));
        } else {
            // A-10 (ratified, option (a)): the L2 literals are the resolution
            // trigger; an unresolved region emits NO preamble, and any MMIO
            // op against it is the op-site hard error.
            out.push_str(
                "\n\t/* preamble: region binding unresolved (no L2 literals) — not mapped */\n",
            );
        }
    }
    // Pin-controller-and-GPIO-configuration acquire preamble (pinctrl construct,
    // C07/D10, M4/T20): AFTER the region map, BEFORE the clock acquire —
    // pins-before-clock (C07). Required ⇒ `devm_pinctrl_get` + select default
    // (hard init error on failure, I02); optional ⇒ auto-bind no-op. Empty for a
    // pinctrl-less / absent-only driver (I07).
    out.push_str(&pinctrl::acquire_lines(model, driver_ty, "&adev->dev")?);
    // Peripheral-clock-management acquire preamble (clock construct, C07/D08):
    // AFTER the pinctrl acquire, BEFORE the interrupt request — `devm_clk_get` per
    // managed clock with PTR_ERR propagation (a hard init error, I02). Empty for
    // a clock-less / static-only driver (I07).
    out.push_str(&clock::acquire_lines(model, driver_ty, "&adev->dev")?);
    // Interrupt preamble (construct #6, D14): register the handler with the
    // line DISABLED (IRQF_NO_AUTOEN — D02, generalizing G-INT-04), LAST among
    // the resource preambles, via the manual request/free pair (D09, no
    // devm). A request failure is a native preamble failure: the probe fails
    // BEFORE on_init (R16/D12) with the kernel status unwrapped; devm unwinds
    // the region. Line identity is runtime-resolved (`adev->irq[0]`, R10);
    // no IRQF_TRIGGER_* is emitted (D10/R12 — the DT is authoritative).
    if let Some(irq) = irq {
        out.push_str(&format!(
            "\n\t/* interrupt preamble (construct #6): register DISABLED (D02);\n\t * manual request/free pair (D09) */\n\t{{\n\t\tint __irq_ret = request_irq(adev->irq[0], {wrapper},\n\t\t\t\t\t    {flags}, \"{name}\", priv);\n\t\tif (__irq_ret)\n\t\t\treturn __irq_ret;\n\t}}\n",
            wrapper = irq.wrapper_sym(drv),
            flags = irq.flags(),
            name = c_ident(&irq.dev_name),
        ));
    }
    out.push_str(&work.init);
    // Atomicity (R10/S10): seed the Atomic fields before any callback, regardless
    // of whether on_init is authored (the fields live in the private struct, so
    // a non-zero initial must be applied even with no on_init).
    out.push_str(&emit_atomic_init(cx));
    if let Some(id) = on_init {
        let sym = cx.fn_symbols[&id].clone();
        let mut hook = String::new();
        if let Some(fid) = on_init_failure {
            hook.push_str(&format!(
                "\t\t\t/* authored on_init -> Err: the failure hook IS the cleanup\n\t\t\t * (the kernel does not call .remove on probe failure; devm\n\t\t\t * unwinds the preamble) */\n\t\t\t(void){}(&priv->inner);\n",
                cx.fn_symbols[&fid]
            ));
        }
        if let Some(irq) = irq {
            // The interrupt reverse unwind (R16): request_irq is NOT devm —
            // free it explicitly on the Err path. The line was registered
            // disabled and never enabled here (D02), so no disable step.
            let _ = irq;
            hook.push_str(
                "\t\t\t/* interrupt unwind (R16): never enabled; free the manual\n\t\t\t * request (no devm) */\n\t\t\tfree_irq(adev->irq[0], priv);\n",
            );
        }
        hook.push_str(&work.cancel.replace('\t', "\t\t\t"));
        let failure_hook = (!hook.is_empty()).then_some(hook);
        let adapter = emit_result_adapter(
            cx,
            id,
            &format!("{sym}(&priv->inner)"),
            failure_hook.as_deref(),
        )?;
        out.push_str(&adapter);
    }
    if irq.is_some() {
        // The enable point (D02/R05/I01): only after on_init returned Ok —
        // the adapter above has already returned the negated errno on every
        // Err path (and when on_init is not overridden, the trait default
        // cannot fail).
        out.push_str(
            "\n\t/* enable the line only after on_init Ok (D02/R05) */\n\tenable_irq(adev->irq[0]);\n",
        );
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
        None => out
            .push_str("\t/* on_deinit not overridden: default no-op teardown */\n\t(void)priv;\n"),
    }
    if irq.is_some() {
        // Interrupt teardown (construct #6, D09/R06): disable then deregister
        // — the manual pair, AFTER the teardown callback and BEFORE the
        // (devm-automatic) resource postambles; the reverse of the preamble
        // where the interrupt registration came last.
        out.push_str(
            "\t/* interrupt teardown (construct #6): disable -> deregister (D09) */\n\tdisable_irq(adev->irq[0]);\n\tfree_irq(adev->irq[0], priv);\n",
        );
    }
    out.push_str(&work.cancel);
    out.push_str("\t/* postamble: devm-managed resources release automatically (A-2) */\n}\n\n");

    // .shutdown — the lone omit-when-absent slot, on device_driver (.drv)
    // for AMBA (scaffold doc §4): quiesce only, Result discarded (void).
    let mut shutdown_sym = None;
    if let Some(id) = on_system_shutdown {
        let sym = format!("{drv}_shutdown");
        let cb = cx.fn_symbols[&id].clone();
        out.push_str(&format!(
            "static void {sym}(struct device *dev)\n{{\n\tstruct {drv}_priv *priv = dev_get_drvdata(dev);\n\n\t(void){cb}(&priv->inner);\n}}\n\n"
        ));
        shutdown_sym = Some(sym);
    }

    Ok(Scaffold {
        container,
        text: out,
        uses_errno,
        shutdown_sym,
    })
}

/// Synthesize the private container, `.probe`, `.remove`, and `.shutdown`
/// for the PCI scaffold (backend-linux-c-e1000 T04; canonical order
/// create → associate → preamble → on_init, mirroring the AMBA arm). The
/// acquisition preamble is the T03-pinned managed cascade (ledger C-7):
/// `pcim_enable_device` → `pcim_iomap_region(pdev, effective_bar, <name>)`,
/// devres releasing map+region then disabling on detach — `.remove` emits
/// no release ladder (ratified A-2). Native preamble failures propagate
/// their kernel status unwrapped (two-channel rule).
fn emit_scaffold_pci(
    cx: &mut LowerCx<'_>,
    drv: &str,
    driver_ty: TyId,
    resource_names: &[String],
    roots: &[(&'static str, InstanceId)],
    driver: &regseq_model::DriverLifecycle,
) -> Result<Scaffold, EmitError> {
    let model = cx.model;
    let driver_struct = cx.names.name_of(model, driver_ty)?;
    let dev_name = model.strings.resolve(driver.descriptor.name).to_string();
    let find = |name: &str| roots.iter().find(|(n, _)| *n == name).map(|(_, id)| *id);
    let on_init = find("on_init");
    let on_deinit = find("on_deinit");
    let on_surprise_remove = find("on_surprise_remove");
    let on_init_failure = find("on_init_failure");
    let on_system_shutdown = find("on_system_shutdown");

    // .probe always carries -ENOMEM (the alloc guard), so the TU always
    // includes <linux/errno.h>.
    let uses_errno = true;
    let mut out = String::new();
    let work = linux_work_emission(cx, drv, driver_ty)?;

    // The per-device private container (the morph, §C.7): kernel device
    // back-pointer + the spec driver embedded verbatim. `pdev` is the C01
    // premise: the scaffold is the sole driver instantiation site, so every
    // `&<drv>` points at `inner` and `container_of` recovery is well-defined.
    let container = format!(
        "/* per-device private container: kernel back-pointer + the spec driver */\nstruct {drv}_priv {{\n\tstruct pci_dev *pdev;\n\tstruct {driver_struct} inner;\n{}}};\n\n",
        work.fields
    );
    out.push_str(&work.wrappers);

    // The single resolved region's BAR (D02: per-region-shaped via
    // effective_bar; >1 region is the loud B-5 gate).
    let preamble = if resource_names.is_empty() {
        None
    } else {
        if resource_names.len() > 1 {
            return Err(EmitError::Unsupported(
                "more than one MMIO resource region per driver (model gate B-5; \
                 single-region only)"
                    .into(),
            ));
        }
        let fname = resource_names[0].clone();
        let resolved = cx
            .driver
            .resources
            .iter()
            .any(|(n, r, _)| n == &fname && *r);
        if resolved {
            let bar = driver
                .handle
                .resource_fields
                .first()
                .and_then(|f| f.binding.effective_bar)
                .ok_or_else(|| {
                    EmitError::Driver(format!(
                        "PCI MMIO region `{fname}` resolved with no BAR index \
                         (effective_bar = L2.bar ?? L1.bar); declare `bar` on the \
                         resource or in [mmio.{fname}]"
                    ))
                })?;
            Some((fname, bar))
        } else {
            None
        }
    };

    // .probe
    out.push_str(&format!(
        "static int {drv}_probe(struct pci_dev *pdev, const struct pci_device_id *id)\n{{\n\tstruct {drv}_priv *priv;\n"
    ));
    if preamble.is_some() {
        out.push_str("\tint ret;\n");
    }
    out.push('\n');
    out.push_str(
        "\tpriv = devm_kzalloc(&pdev->dev, sizeof(*priv), GFP_KERNEL);\n\tif (!priv)\n\t\treturn -ENOMEM;\n",
    );
    out.push_str("\tpriv->pdev = pdev;\n\tpci_set_drvdata(pdev, priv);\n");
    // Synchronization init_primitives (C05/I05): locks initialized before the
    // region map + IRQ register + on_init (lock-live ⊇ line-live).
    let sync_inits = sync::init_primitive_lines(model, driver_ty);
    if !sync_inits.is_empty() {
        out.push_str("\n\t/* init_primitives (C05): locks initialized before region/IRQ/on_init */\n");
        for line in &sync_inits {
            out.push_str(&format!("\t{line}\n"));
        }
    }
    match &preamble {
        Some((fname, bar)) => {
            // The T03-pinned managed acquisition (ledger C-7): enable, then
            // the fused request+map of the effective BAR. The base/size L2
            // literals are dead on Linux (A-10(a)): probe-time enumeration
            // inside pcim_iomap_region supplies the real extent. The failure
            // is the NATIVE channel: ret / PTR_ERR propagate directly, never
            // wrapped in RegseqError (-EPROBE_DEFER survives).
            cx.extra_includes.insert("<linux/err.h>");
            out.push_str(&format!(
                "\n\t/* preamble: managed enable + fused request+map of BAR{bar} (T03 pin\n\t * C-7: pcim_iomap_region; devres unwinds the map, the region, then\n\t * the enable on detach and on probe failure — A-2) */\n\tret = pcim_enable_device(pdev);\n\tif (ret)\n\t\treturn ret;\n\tpriv->inner.{fname} = pcim_iomap_region(pdev, {bar}, \"{dev_name}\");\n\tif (IS_ERR(priv->inner.{fname}))\n\t\treturn PTR_ERR(priv->inner.{fname});\n"
            ));
        }
        None if resource_names.is_empty() => {
            out.push_str("\n\t/* preamble: no bus-managed regions in this model */\n");
        }
        None => {
            // A-10 (ratified, option (a)): the L2 literals are the resolution
            // trigger; an unresolved region emits NO preamble, and any MMIO
            // op against it is the op-site hard error.
            out.push_str(
                "\n\t/* preamble: region binding unresolved (no L2 literals) — not mapped */\n",
            );
        }
    }
    // Pin-controller-and-GPIO-configuration acquire preamble (pinctrl construct,
    // C07/D10, M4/T20): AFTER the region map, BEFORE the clock acquire —
    // pins-before-clock (C07). Required ⇒ `devm_pinctrl_get` + select default;
    // optional ⇒ auto-bind no-op. Empty for a pinctrl-less / absent-only driver
    // (I07). (PL011 is AMBA; this keeps a pinctrl-bearing PCI driver sound.)
    out.push_str(&pinctrl::acquire_lines(model, driver_ty, "&pdev->dev")?);
    // Peripheral-clock-management acquire preamble (clock construct, C07/D08):
    // AFTER the pinctrl acquire — `devm_clk_get` per managed clock with PTR_ERR
    // propagation (a hard init error, I02). Empty for a clock-less / static-only
    // driver (I07). (PL011 is AMBA; this keeps a clock-bearing PCI driver sound.)
    out.push_str(&clock::acquire_lines(model, driver_ty, "&pdev->dev")?);
    out.push_str(&work.init);
    // Atomicity (R10/S10): seed the Atomic fields before any callback, regardless
    // of whether on_init is authored (the fields live in the private struct, so
    // a non-zero initial must be applied even with no on_init).
    out.push_str(&emit_atomic_init(cx));
    if let Some(id) = on_init {
        let sym = cx.fn_symbols[&id].clone();
        let mut hook = on_init_failure.map(|fid| {
            format!(
                "\t\t\t/* authored on_init -> Err: the failure hook IS the cleanup\n\t\t\t * (the kernel does not call .remove on probe failure; devres\n\t\t\t * unwinds the preamble) */\n\t\t\t(void){}(&priv->inner);\n",
                cx.fn_symbols[&fid]
            )
        }).unwrap_or_default();
        hook.push_str(&work.cancel.replace('\t', "\t\t\t"));
        let failure_hook = (!hook.is_empty()).then_some(hook);
        let adapter = emit_result_adapter(
            cx,
            id,
            &format!("{sym}(&priv->inner)"),
            failure_hook.as_deref(),
        )?;
        out.push_str(&adapter);
    }
    out.push_str("\treturn 0;\n}\n\n");

    // .remove — one `void` slot. With `on_surprise_remove` overridden it
    // branches on the ratified present predicate (GRILL D06(3), ledger A-7):
    // orderly arm runs on_deinit (HW still reachable), surprise arm runs
    // on_surprise_remove with NO MMIO-touching releases (devres still
    // unwinds; the mappings are dead, not leaked). Without the override the
    // remove is plain orderly.
    out.push_str(&format!(
        "static void {drv}_remove(struct pci_dev *pdev)\n{{\n\tstruct {drv}_priv *priv = pci_get_drvdata(pdev);\n\n"
    ));
    let orderly_arm = |indent: &str| match on_deinit {
        Some(id) => format!("{indent}(void){}(&priv->inner);\n", cx.fn_symbols[&id]),
        None => {
            format!("{indent}/* on_deinit not overridden: default no-op teardown */\n{indent}(void)priv;\n")
        }
    };
    match on_surprise_remove {
        Some(id) => {
            let sym = cx.fn_symbols[&id].clone();
            out.push_str("\tif (pci_device_is_present(pdev)) {\n");
            out.push_str(&orderly_arm("\t\t"));
            out.push_str("\t} else {\n");
            out.push_str(&format!(
                "\t\t/* surprise removal: the device is gone — no MMIO-touching\n\t\t * releases run on this arm (D06(3)) */\n\t\t(void){sym}(&priv->inner);\n"
            ));
            out.push_str("\t}\n");
        }
        None => out.push_str(&orderly_arm("\t")),
    }
    out.push_str(&work.cancel);
    out.push_str("\t/* postamble: devres-managed resources release automatically (A-2) */\n}\n\n");

    // .shutdown — the lone omit-when-absent slot; pci_driver carries its own
    // `void (*shutdown)(struct pci_dev *)` member (pci.h:980).
    let mut shutdown_sym = None;
    if let Some(id) = on_system_shutdown {
        let sym = format!("{drv}_shutdown");
        let cb = cx.fn_symbols[&id].clone();
        out.push_str(&format!(
            "static void {sym}(struct pci_dev *pdev)\n{{\n\tstruct {drv}_priv *priv = pci_get_drvdata(pdev);\n\n\t(void){cb}(&priv->inner);\n}}\n\n"
        ));
        shutdown_sym = Some(sym);
    }

    Ok(Scaffold {
        container,
        text: out,
        uses_errno,
        shutdown_sym,
    })
}

/// The RegseqError → negated-errno boundary adapter, emitted at the `.probe`
/// return site (`linux-lowering-per-construct.md` §E.3: polarity + placement
/// are the only Linux deltas; symbols are the C02 nine).
fn emit_result_adapter(
    cx: &mut LowerCx<'_>,
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

    let mut out = String::new();
    out.push_str(&format!("\n\t{{\n\t\t{result_c} __r = {call_expr};\n"));
    // Read the Result tag cell.
    out.push_str(&read_scalar(
        "__tag",
        "__r",
        facts.result_tag_offset,
        facts.result_tag_size,
    ));
    out.push_str(&format!("\t\tif (__tag != {:#x}) {{\n", facts.ok_tag_value));
    if let Some(hook) = failure_hook {
        out.push_str(hook);
    }
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
/// Seed each `Atomic<T>` driver field from its `#[rseq::atomic(initial = N)]`
/// literal at probe time, before `on_init` (atomicity R10/S10). The private
/// struct is `devm_kzalloc`'d (zero), so only non-zero initials are emitted, via
/// the width-correct `atomic{,64,_long}_set`. The initial is masked to the
/// element width (two's-complement low bits for a negative signed initial).
fn emit_atomic_init(cx: &LowerCx<'_>) -> String {
    let mut s = String::new();
    for (name, element, initial) in cx.model.atomic_fields_of(cx.driver.driver_ty) {
        let Some(init) = initial else {
            continue;
        };
        let (_, api, valty) = body::linux_atomic(element);
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
            "\t{api}_set(&priv->inner.{}, ({valty})0x{masked:x}ULL);\n",
            c_ident(&name)
        ));
    }
    s
}

fn emit_type_defs(
    cx: &mut LowerCx<'_>,
    driver_ty: TyId,
    resource_names: &[String],
    sync_field_decls: &BTreeMap<String, String>,
    clock_field_decls: &BTreeMap<String, String>,
    pinctrl_field_decls: &BTreeMap<String, String>,
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
            let name_for_err = cx.names.name_of(model, ty)?;
            let with_ctx = |err: EmitError| match err {
                EmitError::Unsupported(s) => {
                    EmitError::Unsupported(format!("{s} [while defining `struct {name_for_err}`]"))
                }
                other => other,
            };
            // Atomicity (construct #7): override each `Atomic<T>` field's storage
            // to the Linux kernel-atomic type by width (C07), for EVERY struct
            // that has atomic fields (not only the driver struct — a nested
            // struct's atomic fields must be overridden too, else its receiver
            // recovery would point at the un-overridden facade type). The include
            // is required whenever an `atomic_t`/etc. declaration is emitted.
            let atomic_fields: Vec<(String, &'static str)> = model
                .atomic_fields_of(ty)
                .into_iter()
                .map(|(name, element, _)| (c_ident(&name), body::linux_atomic(element).0))
                .collect();
            if !atomic_fields.is_empty() {
                cx.extra_includes.insert("<linux/atomic.h>");
            }
            let chunk = if ty == driver_ty {
                let rn = resource_names.to_vec();
                let af = atomic_fields;
                let sync = sync_field_decls.clone();
                let clk = clock_field_decls.clone();
                let pin = pinctrl_field_decls.clone();
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    if rn.iter().any(|r| r == fname) {
                        Some(format!("void __iomem *{fname}"))
                    } else if let Some((_, storage)) = af.iter().find(|(n, _)| n == fname) {
                        Some(format!("{storage} {fname}"))
                    } else if let Some(decl) = sync.get(fname) {
                        // T07/C02: a recognized lock field lowers to its per-OS
                        // lock object (the payload `T` is not stored this slice).
                        Some(decl.clone())
                    } else if let Some(decl) = clk.get(fname) {
                        // clock construct (C04/D08): a MANAGED clock field morphs
                        // to a `struct clk *`; a STATIC clock is absent here (its
                        // ZST `Clock` field is dropped — nothing references it).
                        Some(decl.clone())
                    } else {
                        // pinctrl construct (C04/D10, M4/T20): a RESOLVED REQUIRED
                        // `PinCtrl` field morphs to a `struct pinctrl *`; an
                        // OPTIONAL `Option<PinCtrl>` is absent here (handle-less PM
                        // helpers — its ZST is dropped).
                        pin.get(fname).cloned()
                    }
                })
                .map_err(with_ctx)?
            } else {
                let af = atomic_fields;
                types::emit_aggregate_def(model, &mut cx.names, ty, &move |fname: &str| {
                    af.iter()
                        .find(|(n, _)| n == fname)
                        .map(|(_, storage)| format!("{storage} {fname}"))
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

/// The match table, driver struct, registration macro, and module metadata
/// (`linux-scaffold-and-dispatch.md` §1; every NATIVE entry, zero-sentinel,
/// MODULE_DEVICE_TABLE). Takes the C06-filtered native list — foreign
/// entries feed ONLY the audit blocks rendered adjacent to the table (A2:
/// they must never reach the `unreachable!` arm below).
fn emit_module_tail(
    model: &RegseqModel,
    drv: &str,
    driver: &regseq_model::DriverLifecycle,
    native_entries: &[&MatchEntry],
    ignored_audits: &[crate::match_audit::MatchAudit],
    shutdown_sym: Option<&str>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    for audit in ignored_audits {
        out.push_str(&audit.c_block());
        out.push('\n');
    }
    out.push_str(&format!("static const struct amba_id {drv}_ids[] = {{\n"));
    for entry in native_entries {
        let MatchEntry::Amba { id, mask } = entry else {
            unreachable!("the C06 filter passes only Amba entries to the amba table");
        };
        out.push_str(&format!(
            "\t{{\n\t\t.id\t= {id:#010x},\n\t\t.mask\t= {mask:#010x},\n\t}},\n"
        ));
    }
    out.push_str("\t{ 0, 0 },\n};\n\n");
    out.push_str(&format!("MODULE_DEVICE_TABLE(amba, {drv}_ids);\n\n"));

    let shutdown_field = shutdown_sym
        .map(|s| format!("\n\t\t.shutdown = {s},"))
        .unwrap_or_default();
    out.push_str(&format!(
        "static struct amba_driver {drv}_driver = {{\n\t.drv = {{\n\t\t.name\t= \"{}\",{shutdown_field}\n\t}},\n\t.id_table\t= {drv}_ids,\n\t.probe\t\t= {drv}_probe,\n\t.remove\t\t= {drv}_remove,\n}};\n\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    out.push_str(&format!("module_amba_driver({drv}_driver);\n\n"));
    out.push_str(&format!(
        "MODULE_DESCRIPTION(\"Regseq-generated {} driver\");\nMODULE_LICENSE(\"GPL\");\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    Ok(out)
}

/// The PCI match table, driver struct, registration macro, and module
/// metadata (backend-linux-c-e1000 T05: every NATIVE entry, zero-sentinel,
/// `MODULE_DEVICE_TABLE(pci, ...)` — B-6 never-truncate within the native
/// form; the PCI analogue of [`emit_module_tail`]). Takes the C06-filtered
/// native list; foreign entries feed ONLY the audit blocks (A2).
fn emit_module_tail_pci(
    model: &RegseqModel,
    drv: &str,
    driver: &regseq_model::DriverLifecycle,
    native_entries: &[&MatchEntry],
    ignored_audits: &[crate::match_audit::MatchAudit],
    shutdown_sym: Option<&str>,
) -> Result<String, EmitError> {
    let mut out = String::new();
    for audit in ignored_audits {
        out.push_str(&audit.c_block());
        out.push('\n');
    }
    out.push_str(&format!(
        "static const struct pci_device_id {drv}_ids[] = {{\n"
    ));
    for entry in native_entries {
        let MatchEntry::Pci {
            vendor,
            device,
            subsystem,
            class,
        } = entry
        else {
            unreachable!("the C06 filter passes only Pci entries to the pci table");
        };
        if subsystem.is_some() || class.is_some() {
            // The composed surface carries vendor/device rows only; a
            // qualified row would need a subvendor/subdevice/class-mask
            // rendering this unit has no witness for — loud, never guessed
            // (I03).
            return Err(EmitError::Unsupported(
                "PCI match entry with subsystem/class qualifiers (no witness this \
                 unit; vendor/device rows only)"
                    .into(),
            ));
        }
        out.push_str(&format!(
            "\t{{\n\t\t.vendor\t\t= {vendor:#06x},\n\t\t.device\t\t= {device:#06x},\n\t\t.subvendor\t= PCI_ANY_ID,\n\t\t.subdevice\t= PCI_ANY_ID,\n\t}},\n"
        ));
    }
    out.push_str("\t{ 0, },\n};\n\n");
    out.push_str(&format!("MODULE_DEVICE_TABLE(pci, {drv}_ids);\n\n"));

    let shutdown_field = shutdown_sym
        .map(|s| format!("\n\t.shutdown\t= {s},"))
        .unwrap_or_default();
    out.push_str(&format!(
        "static struct pci_driver {drv}_driver = {{\n\t.name\t\t= \"{}\",\n\t.id_table\t= {drv}_ids,\n\t.probe\t\t= {drv}_probe,\n\t.remove\t\t= {drv}_remove,{shutdown_field}\n}};\n\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    out.push_str(&format!("module_pci_driver({drv}_driver);\n\n"));
    out.push_str(&format!(
        "MODULE_DESCRIPTION(\"Regseq-generated {} driver\");\nMODULE_LICENSE(\"GPL\");\n",
        model.strings.resolve(driver.descriptor.name)
    ));
    Ok(out)
}

/// The const bank (GRILL D04: register/bitfield consts): `(macro, offset)`
/// per named register of the single resource block, recovered from the
/// `#[repr(C)]` block layout. Underscore-named spacer fields are skipped.
fn register_macros(
    model: &RegseqModel,
    driver_ty: TyId,
    drv: &str,
    resource_c_names: &[String],
) -> Result<Vec<(String, u64)>, EmitError> {
    use regseq_model::LayoutKind;
    let mut out = Vec::new();
    let Some(layout) = types::layout_of(model, driver_ty) else {
        return Ok(out);
    };
    let LayoutKind::Struct { fields } = &layout.kind else {
        return Ok(out);
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
    Ok(out)
}
