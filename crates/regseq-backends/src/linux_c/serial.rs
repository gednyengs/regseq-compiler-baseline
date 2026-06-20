//! The linux_c serial device-class lowering (construct #7, PLAN T08 / M2).
//!
//! A `#[rseq::device_class(serial)]` AMBA driver lowers to a Linux serial-core
//! driver: a `struct uart_driver` + one `struct uart_port` whose `uart_ops`
//! vtable wires the framework-pure trait surface to the kernel callbacks, with
//! the per-source interrupt mask (`irq_source_set`) SCATTERED to its Linux WHEN
//! sites (`serial_scatter::linux_scatter`) — the author owns WHICH BITS (the
//! mask-register writes in the op body), the compiler owns WHEN (R04/I01).
//!
//! Slot map (the v1 in-scope surface → `uart_ops`, modelled on the real minimal
//! `sbsa_uart_pops` in `drivers/tty/serial/amba-pl011.c` — a working PL011 with
//! no modem/break/DMA):
//!
//! | uart_ops slot | source |
//! |---|---|
//! | `start_tx` / `stop_tx` / `stop_rx` | per-source scatter (`irq_source_set`) |
//! | `startup` / `shutdown` | `request_irq`/`free_irq` (line) + scatter (per-source) |
//! | `set_termios` | `line_format_apply` (C03); `ConfigResult::Unsupported` surfaced (I03) |
//! | `tx_empty` | `tx_drain_query` (C05; T09 INCLUDE — `serial_core` derefs it unconditionally) |
//! | `set_mctrl`/`get_mctrl`/`type`/`config_port`/`verify_port` | BACKEND-SYNTHESIZED framework-mandatory stubs (NOT construct ops; the modem plane is the deferred `SerialModemControl` — mirrors `sbsa_uart_set_mctrl`) |
//!
//! The synthesized serial ISR (no authored `#[rseq::isr]`; construct #6's line
//! handler is compiler-supplied for a serial driver, B3) decodes via
//! `irq_cause_decode` and dispatches over the construct data path: RX-data /
//! RX-timeout → drain `data_unit_read` to the tty flip buffer; TX-ready → refill
//! the FIFO from the kernel write queue via `data_unit_write`, then disarm
//! TxReady when the queue empties (the `stop_tx` scatter). The TX refill closes
//! Codex M1 finding F2 (the real-OS backend IS ISR-refilled, unlike the M1
//! generic_c pump).
//!
//! **Validation boundary (T13):** this lowering is snapshot-validated (byte-exact
//! `golden.linux.c` + named structural assertions). The live boot+IO Tier-2
//! campaign (amba-pl011 QEMU, R05) is gated on the T13 toolchain and is not run
//! in-repo; the kernel-buffer bridging (kfifo / tty-flip) is modelled on the
//! in-tree 6.x amba-pl011 idiom and is pinned at that live campaign.

use regseq_model::{
    DriverSerial, EnumBinding, EnumShape, InstanceId, InstanceRef, IrqSourceKind, IrqStateKind,
    LayoutKind, MatchEntry, RegseqModel, TyId, VariantIdx,
};

use crate::capability;
use crate::serial_scatter::{linux_c02_schedule, linux_scatter, verify_c02, LinuxSerialSite};

use super::body::{self, LowerCx};
use super::naming::c_ident;
use super::types::{self, TypeNames};
use super::{
    driver_struct_ty, reachable_closure, register_macros, resolve_instance_ref, EmitError,
    LinuxEmit, ScaffoldBus,
};

fn unsup(m: impl Into<String>) -> EmitError {
    EmitError::Unsupported(m.into())
}

/// One resolved author op: its C symbol + the `FnSig` input/output `TyId`s the
/// call sites need to mint enum-literal arguments and read result tags.
struct ResolvedOp {
    sym: String,
    inputs: Vec<TyId>,
    output: TyId,
}

/// Lower a recognized serial-class AMBA driver to a Linux serial-core module.
///
/// Reuses the shared linux_c plumbing (register macros, the lowered-body
/// closure, type defs) and swaps the lifecycle scaffold for the serial
/// `uart_ops` scaffold + serial-core registration.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_linux_serial(
    model: &RegseqModel,
    driver_item: regseq_model::ItemId,
    driver: &regseq_model::DriverLifecycle,
    serial: &DriverSerial,
    bus: ScaffoldBus,
    drv: &str,
    native_entries: &[&MatchEntry],
    ignored_audits: &[crate::match_audit::MatchAudit],
) -> Result<LinuxEmit, EmitError> {
    if bus != ScaffoldBus::Amba {
        return Err(EmitError::UnsupportedInUnit {
            what: "a non-AMBA serial bus scaffold".to_string(),
            unit: "a post-first-light serial unit",
        });
    }
    // T13 / O2 / G-E1: the linux scatter placement must satisfy the C02
    // interrupt/source-ordering edges (clear-pending-before-unmask;
    // disarm-before-teardown-disable) — build-failing on a violation.
    verify_c02(&linux_c02_schedule())
        .map_err(|v| unsup(format!("C02 source-ordering violation (T13/G-E1): {v}")))?;
    let irqd = serial
        .interrupt_driven
        .as_ref()
        .ok_or_else(|| unsup("serial M2 (Linux) requires the SerialInterruptDriven plane (C01)"))?;
    let lc = serial
        .line_config
        .as_ref()
        .ok_or_else(|| unsup("serial M2 (Linux) requires SerialRuntimeLineConfig (C03)"))?;
    let qc = serial.queue_control.as_ref().ok_or_else(|| {
        unsup(
            "serial M2 (Linux) requires SerialQueueControl::tx_drain_query (C05/T09): \
             uart_ops.tx_empty is dereferenced unconditionally by serial_core and is not \
             derivable from the base data_ready_query(TxSpace)",
        )
    })?;

    let driver_ty = driver_struct_ty(model, driver_item)
        .ok_or_else(|| EmitError::Driver("serial driver struct type not in the interner".into()))?;
    let resource_names: Vec<String> = driver
        .handle
        .resource_fields
        .iter()
        .map(|f| c_ident(model.strings.resolve(f.name)))
        .collect();
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
    let reg_macros = register_macros(model, driver_ty, drv, &resource_c_names)?;

    let mut cx = LowerCx {
        model,
        names: TypeNames::new(),
        fn_symbols: std::collections::BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: body::DriverFacts {
            driver_ty,
            resources,
            reg_macros,
            bus_type: driver.descriptor.bus_type.clone(),
            drv: drv.to_string(),
        },
        width8_used: false,
        uses_pr: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
        elided_locals: std::collections::BTreeSet::new(),
    };

    // -- named roots: lifecycle callbacks + the serial ops. Each gets a stable
    //    `<drv>_<name>` symbol; the rest of the closure (helpers like
    //    commit_imsc_shadow) gets the leaf-named clash-suffixed symbols.
    let mut named_roots: Vec<(String, InstanceId)> = Vec::new();
    let add_root = |name: &str,
                    op: &Option<InstanceRef>,
                    roots: &mut Vec<(String, InstanceId)>|
     -> Result<(), EmitError> {
        if let Some(r) = op {
            let id = resolve_instance_ref(model, *r)
                .ok_or_else(|| EmitError::Driver(format!("serial op `{name}` does not resolve")))?;
            roots.push((format!("{drv}_{name}"), id));
        }
        Ok(())
    };
    add_root("on_init", &driver.ops.on_init, &mut named_roots)?;
    add_root("on_deinit", &driver.ops.on_deinit, &mut named_roots)?;
    add_root("port_service_set", &serial.base.port_service_set, &mut named_roots)?;
    add_root("data_ready_query", &serial.base.data_ready_query, &mut named_roots)?;
    add_root("data_unit_write", &serial.base.data_unit_write, &mut named_roots)?;
    add_root("data_unit_read", &serial.base.data_unit_read, &mut named_roots)?;
    add_root("line_format_apply", &lc.line_format_apply, &mut named_roots)?;
    add_root("irq_source_set", &irqd.irq_source_set, &mut named_roots)?;
    add_root("irq_cause_decode", &irqd.irq_cause_decode, &mut named_roots)?;
    add_root("tx_drain_query", &qc.tx_drain_query, &mut named_roots)?;

    let closure = reachable_closure(model, named_roots.iter().map(|(_, id)| *id))?;
    for (name, id) in &named_roots {
        cx.fn_symbols.insert(*id, name.clone());
    }
    for id in &closure {
        if cx.fn_symbols.contains_key(id) {
            continue;
        }
        let leaf =
            super::naming::path_leaf(model.strings.resolve(model.instances[*id].emission_name));
        let base = format!("{drv}_{}", c_ident(leaf));
        let mut sym = base.clone();
        let mut n = 1;
        while cx.fn_symbols.values().any(|v| v == &sym) {
            n += 1;
            sym = format!("{base}_{n}");
        }
        cx.fn_symbols.insert(*id, sym);
    }

    // -- lowered author bodies (prototypes then defs).
    let with_fn_context = |err: EmitError, id: InstanceId| match err {
        EmitError::Unsupported(s) => EmitError::Unsupported(format!(
            "{s} [while lowering `{}`]",
            model.strings.resolve(model.instances[id].emission_name)
        )),
        other => other,
    };
    let mut bodies = String::new();
    for id in &closure {
        bodies.push_str(&body::fn_head(&mut cx, *id).map_err(|e| with_fn_context(e, *id))?);
        bodies.push_str(";\n");
    }
    bodies.push('\n');
    for id in &closure {
        bodies.push_str(&body::emit_fn(&mut cx, *id).map_err(|e| with_fn_context(e, *id))?);
        bodies.push('\n');
    }

    // -- resolve the op symbols + signatures the scaffold calls.
    let resolve_op = |cx: &LowerCx<'_>, op: &Option<InstanceRef>| -> Result<ResolvedOp, EmitError> {
        let iref = op.ok_or_else(|| unsup("serial op not materialized"))?;
        let id = resolve_instance_ref(model, iref)
            .ok_or_else(|| unsup("serial op InstanceRef does not resolve"))?;
        let regseq_model::InstanceKind::Function(data) = &model.instances[id].kind else {
            return Err(unsup("serial op is not a function instance"));
        };
        let sig = model.sigs.resolve(data.fn_sig);
        Ok(ResolvedOp {
            sym: cx.fn_symbols[&id].clone(),
            inputs: sig.inputs.clone(),
            output: sig.output,
        })
    };
    let ops = SerialOps {
        port_service_set: resolve_op(&cx, &serial.base.port_service_set)?,
        data_ready_query: resolve_op(&cx, &serial.base.data_ready_query)?,
        data_unit_write: resolve_op(&cx, &serial.base.data_unit_write)?,
        data_unit_read: resolve_op(&cx, &serial.base.data_unit_read)?,
        line_format_apply: resolve_op(&cx, &lc.line_format_apply)?,
        irq_source_set: resolve_op(&cx, &irqd.irq_source_set)?,
        irq_cause_decode: resolve_op(&cx, &irqd.irq_cause_decode)?,
        tx_drain_query: resolve_op(&cx, &qc.tx_drain_query)?,
    };

    let scaffold = emit_serial_scaffold(&mut cx, drv, driver_ty, serial, &ops)?;

    // -- type defs (now every aggregate use is registered). Build the
    //    synchronization lock-field decls (T07/C02) the same way the main
    //    linux_c path does, so a serial driver that declares an IrqSpinLock
    //    (the ISR<->mainline seam) emits its lock field + pulls the header.
    let mut sync_field_decls: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    if let Some(sync) = model.synchronization.drivers.get(&driver_item) {
        for res in &sync.resources {
            let fname = c_ident(model.strings.resolve(res.name));
            for inc in super::sync::includes_for(res.kind) {
                cx.extra_includes.insert(inc);
            }
            sync_field_decls.insert(fname.clone(), super::sync::lock_field_decl(res.kind, &fname));
        }
    }
    // Peripheral-clock-management is NOT integrated into the serial-uart path
    // (construct #7) yet: the serial scaffold hardcodes `port.uartclk` (its own
    // TODO), and the clock acquire/morph is witnessed on the MAIN linux_c path.
    // Reject a clock-bearing serial driver loudly rather than morph a `struct
    // clk *` field the serial scaffold never acquires (never-wrong-code, I06).
    if model
        .clock
        .drivers
        .get(&driver_item)
        .is_some_and(|d| !d.resources.is_empty())
    {
        return Err(EmitError::Unsupported(
            "a #[rseq::clock] field on a #[rseq::serial] driver is not yet integrated \
             into the serial-uart lowering (the main linux_c path is the clock witness; \
             the serial path hardcodes port.uartclk)"
                .into(),
        ));
    }
    let clock_field_decls: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let type_defs = super::emit_type_defs(
        &mut cx,
        driver_ty,
        &resource_names,
        &sync_field_decls,
        &clock_field_decls,
    )?;

    // -- assemble the TU.
    let mut tu = String::new();
    tu.push_str(&format!(
        "// SPDX-License-Identifier: GPL-2.0\n/*\n * {drv}.c - Regseq-generated Linux serial driver (AMBA PL011-class).\n *\n * Generated by the regseq linux_c serial lowering (construct #7); do not edit.\n */\n\n"
    ));
    if cx.uses_pr {
        tu.push_str("#define pr_fmt(fmt) KBUILD_MODNAME \": \" fmt\n\n");
    }
    let mut includes: std::collections::BTreeSet<&str> = [
        "<linux/amba/bus.h>",
        "<linux/device.h>",
        "<linux/errno.h>",
        "<linux/interrupt.h>",
        "<linux/io.h>",
        "<linux/mod_devicetable.h>",
        "<linux/module.h>",
        "<linux/serial_core.h>",
        "<linux/slab.h>",
        "<linux/tty_flip.h>",
        "<linux/types.h>",
    ]
    .into_iter()
    .collect();
    if cx.uses_pr {
        includes.insert("<linux/printk.h>");
    }
    includes.extend(cx.extra_includes.iter().copied());
    for inc in includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');
    if cx.width8_used {
        tu.push_str(
            "#ifndef CONFIG_64BIT\n#error \"this Regseq-generated driver performs 64-bit MMIO and requires CONFIG_64BIT (ratified A-12)\"\n#endif\n\n",
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
    tu.push_str(&scaffold.container);
    tu.push_str(&bodies);
    tu.push_str(&scaffold.text);
    tu.push('\n');
    tu.push_str(&emit_serial_tail(model, drv, driver, native_entries, ignored_audits)?);

    // -- conformance: the delivered uart_ops slots + the per-source arming sites
    //    (R04). A serial driver DELIVERS the class (not deferred): deferred_class
    //    is None; the synthesized modem stubs are framework-mandatory glue, not
    //    construct deliveries.
    let mut delivered_slots: Vec<capability::SlotEntry> = [
        "uart_ops.startup",
        "uart_ops.shutdown",
        "uart_ops.start_tx",
        "uart_ops.stop_tx",
        "uart_ops.stop_rx",
        "uart_ops.set_termios",
        "uart_ops.tx_empty",
    ]
    .iter()
    .map(|s| capability::SlotEntry {
        slot: (*s).to_string(),
        row: None,
        citation: None,
    })
    .collect();
    for (site, label) in [
        (LinuxSerialSite::Startup, "serial.irq_source_set@startup"),
        (LinuxSerialSite::StartTx, "serial.irq_source_set@start_tx"),
        (LinuxSerialSite::StopTx, "serial.irq_source_set@stop_tx"),
        (LinuxSerialSite::StopRx, "serial.irq_source_set@stop_rx"),
        (LinuxSerialSite::Shutdown, "serial.irq_source_set@shutdown"),
    ] {
        for intent in linux_scatter(site) {
            delivered_slots.push(capability::SlotEntry {
                slot: format!("{label}:{:?}->{:?}", intent.source, intent.state),
                row: None,
                citation: None,
            });
        }
    }
    let conformance = capability::ConformanceReport {
        target: "linux_c",
        os_version_pin: capability::LINUX_PIN,
        delivered_slots,
        absent_dispatch_slots: Vec::new(),
        structurally_absent_slots: Vec::new(),
        rejected_slots: Vec::new(),
        ignored_match_entries: ignored_audits.to_vec(),
        deferred_class: None,
    };

    Ok(LinuxEmit {
        kbuild: format!("obj-m += {drv}.o\n"),
        module_name: drv.to_string(),
        tu,
        conformance,
    })
}

/// The resolved author ops the scaffold wires + calls.
struct SerialOps {
    port_service_set: ResolvedOp,
    data_ready_query: ResolvedOp,
    data_unit_write: ResolvedOp,
    data_unit_read: ResolvedOp,
    line_format_apply: ResolvedOp,
    irq_source_set: ResolvedOp,
    irq_cause_decode: ResolvedOp,
    tx_drain_query: ResolvedOp,
}

struct SerialScaffold {
    container: String,
    text: String,
}

/// A fieldless-enum byte-carrier compound literal at the variant for canonical
/// semantic position `ord` of binding `b` (the carrier's `_raw` tag bytes set
/// little-endian; the rest zero-filled — the layout-faithful value).
fn enum_carrier(
    cx: &mut LowerCx<'_>,
    enum_ty: TyId,
    binding: &Option<EnumBinding>,
    ord: usize,
) -> Result<String, EmitError> {
    let variant = binding
        .as_ref()
        .and_then(|e| e.variants.get(ord).copied())
        .ok_or_else(|| unsup("serial support enum variant map missing"))?;
    enum_carrier_variant(cx, enum_ty, variant)
}

fn enum_carrier_variant(
    cx: &mut LowerCx<'_>,
    enum_ty: TyId,
    variant: VariantIdx,
) -> Result<String, EmitError> {
    let cty = types::c_type(cx.model, &mut cx.names, enum_ty)?;
    let layout =
        types::layout_of(cx.model, enum_ty).ok_or_else(|| unsup("enum carrier of non-aggregate"))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(unsup("enum carrier of an unsupported enum shape"));
    };
    let vlayout = variants
        .get(variant.as_u32() as usize)
        .ok_or_else(|| unsup("enum carrier variant index out of range"))?;
    let tag = vlayout
        .tag_value
        .ok_or_else(|| unsup("enum carrier variant is untagged"))?;
    if tag > u64::MAX as u128 {
        return Err(unsup("128-bit enum tag value (first-light subset)"));
    }
    let (offset, size) = types::enum_tag_cell(cx.model, enum_ty)?;
    let mut elems = Vec::new();
    for i in 0..size {
        let b = ((tag as u64) >> (8 * i)) & 0xff;
        elems.push(format!("[{}] = {b:#x}", offset + i));
    }
    Ok(format!("({cty}){{ ._raw = {{ {} }} }}", elems.join(", ")))
}

/// The `(source, state)` literal call to the author `irq_source_set`. The author
/// body writes IMSC — the per-source mask write — DISTINCT from the line
/// plumbing (`request_irq`/`free_irq`) the startup/shutdown wrappers emit
/// (R04/I01; the per-source-vs-line distinction the golden pins).
fn iss_call(
    cx: &mut LowerCx<'_>,
    serial: &DriverSerial,
    ops: &SerialOps,
    src: IrqSourceKind,
    st: IrqStateKind,
) -> Result<String, EmitError> {
    let src_lit = enum_carrier(cx, ops.irq_source_set.inputs[1], &serial.types.irq_source, src as usize)?;
    let st_lit = enum_carrier(cx, ops.irq_source_set.inputs[2], &serial.types.irq_state, st as usize)?;
    Ok(format!(
        "\t{}(&priv->inner, {src_lit}, {st_lit});\n",
        ops.irq_source_set.sym
    ))
}

fn scatter_calls(
    cx: &mut LowerCx<'_>,
    serial: &DriverSerial,
    ops: &SerialOps,
    site: LinuxSerialSite,
) -> Result<String, EmitError> {
    let mut out = String::new();
    for intent in linux_scatter(site) {
        out.push_str(&iss_call(cx, serial, ops, intent.source, intent.state)?);
    }
    Ok(out)
}

/// Emit the serial scaffold: the per-device container, the synthesized ISR, the
/// `uart_ops` wrappers + the framework-mandatory backend stubs, the vtable, and
/// the `.probe`/`.remove` registering the port with serial-core.
fn emit_serial_scaffold(
    cx: &mut LowerCx<'_>,
    drv: &str,
    driver_ty: TyId,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<SerialScaffold, EmitError> {
    let driver_struct = cx.names.name_of(cx.model, driver_ty)?;

    // The per-device container: kernel back-pointer + the uart_port + the spec
    // driver embedded verbatim (its `regs` field is a `void __iomem *`).
    let container = format!(
        "/* per-device private container: kernel back-pointer + uart_port + the spec driver */\nstruct {drv}_priv {{\n\tstruct amba_device *adev;\n\tstruct uart_port port;\n\tstruct {driver_struct} inner;\n}};\n\n#define {drv}_of(p) container_of((p), struct {drv}_priv, port)\n\n"
    );

    let mut out = String::new();

    // -- the shared TX pump (defined first: start_tx and the ISR both call it).
    out.push_str(&emit_tx_pump(cx, drv, serial, ops)?);

    // -- the synthesized serial ISR (B3: no authored #[rseq::isr]; the compiler
    //    supplies the line handler for a serial driver). Decode → dispatch over
    //    the construct data path.
    out.push_str(&emit_serial_isr(cx, drv, serial, ops)?);

    // -- uart_ops wrappers ---------------------------------------------------
    // startup: request_irq (the LINE) + per-source arm scatter (Linux startup
    // WHEN). The arm writes IMSC via irq_source_set, DISTINCT from request_irq.
    out.push_str(&format!(
        "static int {drv}_startup(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n\tint ret;\n\n\tret = request_irq(port->irq, {drv}_isr, 0, \"{drv}\", priv);\n\tif (ret)\n\t\treturn ret;\n\t/* per-source arming (Linux `startup` WHEN, R04): arm RX + line-error */\n",
        drv = drv
    ));
    out.push_str(&scatter_calls(cx, serial, ops, LinuxSerialSite::Startup)?);
    out.push_str("\treturn 0;\n}\n\n");

    // shutdown: mask all (Linux shutdown WHEN) + free_irq (the LINE) + disable
    // the port (the bring-up enable's counterpart).
    let scope_block =
        enum_carrier(cx, ops.port_service_set.inputs[1], &serial.types.port_scope, 0)?;
    out.push_str(&format!(
        "static void {drv}_shutdown(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n\t/* mask all sources (Linux `shutdown` WHEN, R04) */\n"
    ));
    out.push_str(&scatter_calls(cx, serial, ops, LinuxSerialSite::Shutdown)?);
    out.push_str(&format!(
        "\tfree_irq(port->irq, priv);\n\t{pss}(&priv->inner, {scope_block}, false);\n}}\n\n",
        pss = ops.port_service_set.sym
    ));

    // start_tx / stop_tx / stop_rx: one scatter each (the construct contribution
    // — the per-source mask hoisted out of authored bodies into these sites).
    // start_tx: arm TxReady (the scatter), THEN pump — the kick. Arming alone is
    // insufficient: a level-triggered PL011 TX IRQ does not fire for an
    // already-empty FIFO, so the FIFO must be primed (the in-tree pl011_start_tx
    // likewise calls pl011_tx_chars, not just enable-IRQ). Verified against a live
    // amba-pl011 QEMU run (arm-only deadlocked: an IMSC-rearm storm, no transmit).
    out.push_str(&format!(
        "static void {drv}_start_tx(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n"
    ));
    out.push_str(&scatter_calls(cx, serial, ops, LinuxSerialSite::StartTx)?);
    out.push_str(&format!("\t{drv}_tx_pump(priv);\n}}\n\n"));
    out.push_str(&format!(
        "static void {drv}_stop_tx(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n"
    ));
    out.push_str(&scatter_calls(cx, serial, ops, LinuxSerialSite::StopTx)?);
    out.push_str("}\n\n");
    out.push_str(&format!(
        "static void {drv}_stop_rx(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n"
    ));
    out.push_str(&scatter_calls(cx, serial, ops, LinuxSerialSite::StopRx)?);
    out.push_str("}\n\n");

    // tx_empty ← tx_drain_query (C05/T09): serial_core derefs this unconditionally.
    out.push_str(&format!(
        "static unsigned int {drv}_tx_empty(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n\treturn {sym}(&priv->inner) ? TIOCSER_TEMT : 0;\n}}\n\n",
        sym = ops.tx_drain_query.sym
    ));

    // set_termios ← line_format_apply (C03); ConfigResult::Unsupported SURFACED.
    out.push_str(&emit_set_termios(cx, drv, serial, ops)?);

    // -- framework-mandatory backend stubs (NOT construct ops): the modem plane
    //    is the deferred SerialModemControl; serial-core still derefs set_mctrl/
    //    get_mctrl/type/config_port/verify_port. Minimal no-op forms, exactly as
    //    the in-tree `sbsa_uart_pops` does for a no-modem PL011.
    out.push_str(&format!(
        "/* framework-mandatory uart_ops stubs (serial-core derefs these); the modem\n * plane is the deferred SerialModemControl construct — mirror of sbsa_uart_pops */\nstatic void {drv}_set_mctrl(struct uart_port *port, unsigned int mctrl) {{ }}\nstatic unsigned int {drv}_get_mctrl(struct uart_port *port) {{ return TIOCM_CAR | TIOCM_DSR | TIOCM_CTS; }}\nstatic const char *{drv}_type(struct uart_port *port) {{ return (port->type == PORT_AMBA) ? \"{drv}\" : NULL; }}\nstatic void {drv}_config_port(struct uart_port *port, int flags) {{ if (flags & UART_CONFIG_TYPE) port->type = PORT_AMBA; }}\nstatic int {drv}_verify_port(struct uart_port *port, struct serial_struct *ser) {{ return (ser->type != PORT_UNKNOWN && ser->type != PORT_AMBA) ? -EINVAL : 0; }}\n\n"
    ));

    // -- the uart_ops vtable.
    out.push_str(&format!(
        "static const struct uart_ops {drv}_uart_ops = {{\n\t.tx_empty\t= {drv}_tx_empty,\n\t.set_mctrl\t= {drv}_set_mctrl,\n\t.get_mctrl\t= {drv}_get_mctrl,\n\t.stop_tx\t= {drv}_stop_tx,\n\t.start_tx\t= {drv}_start_tx,\n\t.stop_rx\t= {drv}_stop_rx,\n\t.startup\t= {drv}_startup,\n\t.shutdown\t= {drv}_shutdown,\n\t.set_termios\t= {drv}_set_termios,\n\t.type\t\t= {drv}_type,\n\t.config_port\t= {drv}_config_port,\n\t.verify_port\t= {drv}_verify_port,\n}};\n\n"
    ));

    // -- the uart_driver (registered once at module init). The `dev_name` (the
    //    /dev/<dev_name><line> tty-node prefix) is derived from the driver
    //    identity, NOT a fixed "ttyAMA": a generated driver names its own tty
    //    nodes, and reusing the canonical PL011 "ttyAMA" collides with an
    //    already-loaded stock amba-pl011 (the tty-namespace is global) — the
    //    kernel rejects the duplicate node ("Cannot register tty device on line
    //    0"). Major/minor are dynamic (`.major` unset) so they never clash.
    out.push_str(&format!(
        "static struct uart_driver {drv}_uart_driver = {{\n\t.owner\t\t= THIS_MODULE,\n\t.driver_name\t= \"{drv}\",\n\t.dev_name\t= \"tty{drv}\",\n\t.nr\t\t= 1,\n}};\n\n"
    ));

    // -- .probe: alloc priv, claim+map the iomem, set up the uart_port, then
    //    on_init (the spec bring-up: safe IRQ defaults), then add the port.
    out.push_str(&emit_serial_probe(cx, drv)?);

    // -- .remove: remove the port (which quiesces it via .shutdown), then run
    //    the spec on_deinit (the lifecycle device teardown — mask/clear; its
    //    Result is structurally discarded, CR-4). uart_remove_one_port alone does
    //    NOT invoke on_deinit (Codex M2 finding 5).
    let on_deinit_call = cx
        .model
        .device_lifecycle
        .drivers
        .values()
        .next()
        .and_then(|d| d.ops.on_deinit.as_ref())
        .and_then(|r| resolve_instance_ref(cx.model, *r))
        .map(|id| format!("\t(void){}(&priv->inner);\n", cx.fn_symbols[&id]))
        .unwrap_or_default();
    out.push_str(&format!(
        "static void {drv}_remove(struct amba_device *adev)\n{{\n\tstruct {drv}_priv *priv = amba_get_drvdata(adev);\n\n\tuart_remove_one_port(&{drv}_uart_driver, &priv->port);\n{on_deinit_call}}}\n\n"
    ));

    Ok(SerialScaffold {
        container,
        text: out,
    })
}

/// The shared TX pump (the compiler's TX choreography): emit any pending
/// high-priority char (XON/XOFF), refill the hardware FIFO from the kernel write
/// queue via the author data path, wake writers below the watermark, and disarm
/// TxReady (the `stop_tx` scatter) once the queue drains. Called from `start_tx`
/// (the kick — priming so the level-triggered TX IRQ fires) AND the ISR (refill,
/// closing Codex M1 F2). Both call sites hold the port lock.
///
/// `uart_xmit_advance(port, 1)` advances the kfifo AND counts `icount.tx` — it
/// REPLACES a separate `kfifo_skip` (doing both drops every other byte; Codex M2
/// finding 1). Modelled on the in-tree 6.x `pl011_tx_chars`.
fn emit_tx_pump(
    cx: &mut LowerCx<'_>,
    drv: &str,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<String, EmitError> {
    let txspace_dir =
        enum_carrier(cx, ops.data_ready_query.inputs[1], &serial.types.ready_dir, 0)?; // ReadyDir::TxSpace
    let tx_disarm = {
        let call = iss_call(cx, serial, ops, IrqSourceKind::TxReady, IrqStateKind::Disarmed)?;
        format!("\t\t{}", call.trim_start())
    };
    Ok(format!(
        "static void {drv}_tx_pump(struct {drv}_priv *priv)\n{{\n\tstruct tty_port *tport = &priv->port.state->port;\n\tunsigned char __ch;\n\n\t/* High-priority XON/XOFF: sent even when TX is stopped. If the FIFO has no\n\t * space yet, leave TxReady armed (return WITHOUT disarming) so the level TX\n\t * IRQ refires — never strand a pending x_char (Codex M2 follow-up 1). */\n\tif (priv->port.x_char) {{\n\t\tif (!{ready}(&priv->inner, {txspace_dir}))\n\t\t\treturn;\n\t\t{write}(&priv->inner, priv->port.x_char);\n\t\tpriv->port.x_char = 0;\n\t\tpriv->port.icount.tx++;\n\t}}\n\t/* Disarm TxReady when the core STOPPED TX (CTS/XOFF) OR the queue drained —\n\t * BOTH cases, so a stopped non-empty queue does not leave TxReady armed (a\n\t * level-IRQ storm; Codex M2 follow-up 2). The core re-kicks via start_tx. */\n\tif (kfifo_is_empty(&tport->xmit_fifo) || uart_tx_stopped(&priv->port)) {{\n{tx_disarm}\t\treturn;\n\t}}\n\twhile ({ready}(&priv->inner, {txspace_dir})) {{\n\t\tif (!kfifo_peek(&tport->xmit_fifo, &__ch))\n\t\t\tbreak;\n\t\t{write}(&priv->inner, __ch);\n\t\tuart_xmit_advance(&priv->port, 1);\n\t}}\n\tif (kfifo_len(&tport->xmit_fifo) < WAKEUP_CHARS)\n\t\tuart_write_wakeup(&priv->port);\n\tif (kfifo_is_empty(&tport->xmit_fifo))\n{tx_disarm}}}\n\n",
        ready = ops.data_ready_query.sym,
        write = ops.data_unit_write.sym,
    ))
}

/// The synthesized serial ISR: decode causes, then dispatch over the construct
/// data path (RX drain to tty; TX refill from the kernel write queue).
fn emit_serial_isr(
    cx: &mut LowerCx<'_>,
    drv: &str,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<String, EmitError> {
    use regseq_model::{SerialCause, SerialRxErrorKind};

    // CauseSet is a `{bits: u32}` struct → `.bits`; read the field name from the
    // layout (no hardcoded spelling).
    let cause_field = struct_first_field_name(cx.model, ops.irq_cause_decode.output)
        .ok_or_else(|| unsup("CauseSet has no bits field"))?;
    // Drain RX on data OR timeout OR line-error: a line-error cause carries an
    // errored datum still sitting in the RX FIFO (the inline-error contract,
    // I04) — draining it consumes the errored byte rather than stranding it
    // after `irq_cause_decode` W1C-acked the error (Codex M2 finding 4).
    let rx_bits = (1u32 << (SerialCause::RxData as u32))
        | (1u32 << (SerialCause::RxTimeout as u32))
        | (1u32 << (SerialCause::LineError as u32));
    let tx_bit = 1u32 << (SerialCause::TxReady as u32);

    let rxdata_dir = enum_carrier(cx, ops.data_ready_query.inputs[1], &serial.types.ready_dir, 1)?; // ReadyDir::RxData

    let cause_cty = types::c_type(cx.model, &mut cx.names, ops.irq_cause_decode.output)?;
    let rxdatum_cty = types::c_type(cx.model, &mut cx.names, ops.data_unit_read.output)?;
    let rx_unit_field = struct_first_field_name(cx.model, ops.data_unit_read.output)
        .ok_or_else(|| unsup("RxDatum has no leading unit field"))?;
    // The inline RxError carrier (`RxDatum.errors`) + its `.bits` field, and the
    // per-kind masks from the pinned RxErrorRepr (the O2-analog): map the inline
    // error to the tty flag instead of dropping it as TTY_NORMAL (F1).
    let (rx_err_field, rx_err_ty) = struct_field(cx.model, ops.data_unit_read.output, "errors")
        .ok_or_else(|| unsup("RxDatum has no `errors` field"))?;
    let rx_err_bits = struct_first_field_name(cx.model, rx_err_ty)
        .ok_or_else(|| unsup("RxError has no bits field"))?;
    let rxerr_mask = |kind: SerialRxErrorKind| -> u32 { 1u32 << (kind as u32) };
    let (m_frame, m_parity, m_break, m_hwovr, m_swovr) = (
        rxerr_mask(SerialRxErrorKind::Framing),
        rxerr_mask(SerialRxErrorKind::Parity),
        rxerr_mask(SerialRxErrorKind::Break),
        rxerr_mask(SerialRxErrorKind::HwOverrun),
        rxerr_mask(SerialRxErrorKind::SwOverrun),
    );

    let mut out = String::new();
    out.push_str(&format!(
        "/* the synthesized serial ISR (construct #7, B3): decode -> dispatch over the\n * construct data path. No authored #[rseq::isr]; the compiler owns the line\n * handler for a serial driver. */\nstatic irqreturn_t {drv}_isr(int irq, void *dev_id)\n{{\n\tstruct {drv}_priv *priv = dev_id;\n\tstruct tty_port *tport = &priv->port.state->port;\n\t{cause_cty} __c;\n\n\tuart_port_lock(&priv->port);\n\t__c = {decode}(&priv->inner);\n",
        decode = ops.irq_cause_decode.sym
    ));
    // RX (data OR timeout): drain to the tty flip buffer, mapping the inline
    // RxError (I04) to the tty flag — break > parity > frame (a line condition
    // suppresses the data-char flags), overrun inserted as a separate char (the
    // uart_insert_char idiom); icount tracks rx/brk/parity/frame/overrun (F1).
    out.push_str(&format!(
        "\tif ((__c.{cause_field} & {rx_bits}u) != 0u) {{\n\t\twhile ({ready}(&priv->inner, {rxdata_dir})) {{\n\t\t\t{rxdatum_cty} __d = {read}(&priv->inner);\n\t\t\tunsigned int __e = __d.{err_field}.{err_bits};\n\t\t\tunsigned int __flag = TTY_NORMAL;\n\t\t\tpriv->port.icount.rx++;\n\t\t\tif (__e & {m_break}u) {{ priv->port.icount.brk++; __flag = TTY_BREAK; }}\n\t\t\telse if (__e & {m_parity}u) {{ priv->port.icount.parity++; __flag = TTY_PARITY; }}\n\t\t\telse if (__e & {m_frame}u) {{ priv->port.icount.frame++; __flag = TTY_FRAME; }}\n\t\t\tif (__e & {m_swovr}u) priv->port.icount.buf_overrun++;\n\t\t\ttty_insert_flip_char(tport, __d.{rx_unit_field} & 0xff, __flag);\n\t\t\tif (__e & {m_hwovr}u) {{ priv->port.icount.overrun++; tty_insert_flip_char(tport, 0, TTY_OVERRUN); }}\n\t\t}}\n\t}}\n",
        ready = ops.data_ready_query.sym,
        read = ops.data_unit_read.sym,
        err_field = rx_err_field,
        err_bits = rx_err_bits,
    ));
    // TX (ready): refill via the shared TX pump (the same drain start_tx kicks).
    out.push_str(&format!(
        "\tif ((__c.{cause_field} & {tx_bit}u) != 0u)\n\t\t{drv}_tx_pump(priv);\n"
    ));
    out.push_str("\tuart_port_unlock(&priv->port);\n\ttty_flip_buffer_push(tport);\n\treturn IRQ_RETVAL((__c.");
    out.push_str(&cause_field);
    out.push_str(") != 0u);\n}\n\n");
    Ok(out)
}

/// `set_termios` ← `line_format_apply` (C03). Two design-intended M2 edges:
/// **D17** — disable, then WAIT for the current transfer to drain (FR.BUSY clear,
/// via the author's `tx_drain_query`) before reprogramming, so an in-flight
/// character is not corrupted; **D18** — clear `CRTSCTS` from the returned termios
/// (hw flow control is the deferred `SerialModemControl` plane, never silently
/// honored — the idiomatic Linux mirror of Zephyr `configure`'s `-ENOTSUP`).
/// The C03 FEN-clear/FIFO-flush edge is deferred (no register-role metadata for
/// the FIFO-enable bit, no in-scope FIFO op — see the M2 record).
fn emit_set_termios(
    cx: &mut LowerCx<'_>,
    drv: &str,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<String, EmitError> {
    let lf_ty = ops.line_format_apply.inputs[1];
    let lf_cty = types::c_type(cx.model, &mut cx.names, lf_ty)?;
    // LineFormat field names + the enum field TyIds.
    let baud_field = struct_field(cx.model, lf_ty, "baud")
        .ok_or_else(|| unsup("LineFormat has no baud field"))?;
    let (db_name, db_ty) = struct_field(cx.model, lf_ty, "data_bits")
        .ok_or_else(|| unsup("LineFormat has no data_bits field"))?;
    let (par_name, par_ty) = struct_field(cx.model, lf_ty, "parity")
        .ok_or_else(|| unsup("LineFormat has no parity field"))?;
    let (sb_name, sb_ty) = struct_field(cx.model, lf_ty, "stop_bits")
        .ok_or_else(|| unsup("LineFormat has no stop_bits field"))?;
    let baud_name = baud_field.0;

    // DataBits carriers: Five..Eight (canonical positions 0..3).
    let db5 = enum_carrier(cx, db_ty, &serial.types.data_bits, 0)?;
    let db6 = enum_carrier(cx, db_ty, &serial.types.data_bits, 1)?;
    let db7 = enum_carrier(cx, db_ty, &serial.types.data_bits, 2)?;
    let db8 = enum_carrier(cx, db_ty, &serial.types.data_bits, 3)?;
    // Parity: None=0, Even=1, Odd=2.
    let par_none = enum_carrier(cx, par_ty, &serial.types.parity, 0)?;
    let par_even = enum_carrier(cx, par_ty, &serial.types.parity, 1)?;
    let par_odd = enum_carrier(cx, par_ty, &serial.types.parity, 2)?;
    // StopBits: One=0, Two=1.
    let sb_one = enum_carrier(cx, sb_ty, &serial.types.stop_bits, 0)?;
    let sb_two = enum_carrier(cx, sb_ty, &serial.types.stop_bits, 1)?;

    // PortScope::Block carrier for the §3.3.8 disable→format→enable bracket.
    let scope_block = enum_carrier(cx, ops.port_service_set.inputs[1], &serial.types.port_scope, 0)?;
    let pss = ops.port_service_set.sym.clone();

    // The D17 busy-wait uses the author's `tx_drain_query`. Linux no longer reads
    // the `ConfigResult` tag: `set_termios` has no error return, and unsupported
    // hw flow is signaled by clearing `CRTSCTS` from the returned termios (D18) —
    // so the revert-to-old path (D18's rejected alternative) and its tag read go.
    let drain = ops.tx_drain_query.sym.clone();
    Ok(format!(
        "static void {drv}_set_termios(struct uart_port *port, struct ktermios *termios,\n\t\t\t\t  const struct ktermios *old)\n{{\n\tstruct {drv}_priv *priv = {drv}_of(port);\n\tunsigned long flags;\n\tunsigned int baud;\n\t{lf_cty} lf;\n\n\t/* D18: hw flow control (CRTSCTS) is the deferred SerialModemControl plane and\n\t * is never silently honored - clear it from the returned termios (the idiomatic\n\t * Linux \"flag not supported\" signal). Flow is permanently outside LineFormat. */\n\ttermios->c_cflag &= ~CRTSCTS;\n\n\tbaud = uart_get_baud_rate(port, termios, old, 0, port->uartclk / 16);\n\tlf.{baud_name} = baud;\n\tswitch (termios->c_cflag & CSIZE) {{\n\tcase CS5: lf.{db_name} = {db5}; break;\n\tcase CS6: lf.{db_name} = {db6}; break;\n\tcase CS7: lf.{db_name} = {db7}; break;\n\tdefault:  lf.{db_name} = {db8}; break;\n\t}}\n\tif (termios->c_cflag & PARENB)\n\t\tlf.{par_name} = (termios->c_cflag & PARODD) ? {par_odd} : {par_even};\n\telse\n\t\tlf.{par_name} = {par_none};\n\tlf.{sb_name} = (termios->c_cflag & CSTOPB) ? {sb_two} : {sb_one};\n\n\tuart_port_lock_irqsave(port, &flags);\n\tuart_update_timeout(port, termios->c_cflag, baud);\n\t/* PL011 §3.3.8 / D17 / C03: reprogram baud + line-control only while the port\n\t * is disabled, and only AFTER the current transfer drains (FR.BUSY clear, via\n\t * tx_drain_query) so an in-flight character is not corrupted. The C03\n\t * FEN-clear/FIFO-flush edge is deferred: the device-neutral backend has no\n\t * register-role metadata for the FIFO-enable bit and no in-scope FIFO op. */\n\t{pss}(&priv->inner, {scope_block}, false);\n\twhile (!{drain}(&priv->inner))\n\t\tcpu_relax();\n\t{apply}(&priv->inner, lf);\n\t{pss}(&priv->inner, {scope_block}, true);\n\tuart_port_unlock_irqrestore(port, flags);\n}}\n\n",
        apply = ops.line_format_apply.sym,
    ))
}

/// `.probe`: alloc the container, claim+map the iomem into the spec driver's
/// `regs`, set up the `uart_port`, run the spec `on_init` through the C02 errno
/// adapter (its `Result<(), RegseqError>` → negated errno on Err, R16), then add
/// the port.
fn emit_serial_probe(cx: &mut LowerCx<'_>, drv: &str) -> Result<String, EmitError> {
    // Single MMIO region only (model gate B-5); recover the resolved iomem field.
    let region = cx
        .driver
        .resources
        .iter()
        .find(|(_, resolved, _)| *resolved)
        .map(|(name, _, _)| name.clone());
    let map_line = match region {
        Some(field) => format!(
            "\tpriv->inner.{field} = devm_ioremap_resource(&adev->dev, &adev->res);\n\tif (IS_ERR(priv->inner.{field}))\n\t\treturn PTR_ERR(priv->inner.{field});\n"
        ),
        None => "\t/* no bus-managed MMIO region in this model */\n".to_string(),
    };

    // The spec on_init runs through the SAME Result→errno adapter the lifecycle
    // scaffold uses (its return is a `struct Result` carrier, NOT an int): the
    // adapter returns the negated C02 errno on every Err path and falls through
    // on Ok.
    let on_init_id = cx
        .model
        .device_lifecycle
        .drivers
        .values()
        .next()
        .and_then(|d| d.ops.on_init.as_ref())
        .and_then(|r| resolve_instance_ref(cx.model, *r));
    let on_init_block = match on_init_id {
        Some(id) => {
            let sym = cx.fn_symbols[&id].clone();
            super::emit_result_adapter(cx, id, &format!("{sym}(&priv->inner)"), None)?
        }
        None => String::new(),
    };

    // The init→add_one_port failure unwind (R16): on_init's bring-up is paired
    // with on_deinit, so a LATER probe-step failure (uart_add_one_port) must run
    // on_deinit to quiesce the hardware before returning (Codex M2 finding 5b).
    let add_fail_unwind = cx
        .model
        .device_lifecycle
        .drivers
        .values()
        .next()
        .and_then(|d| d.ops.on_deinit.as_ref())
        .and_then(|r| resolve_instance_ref(cx.model, *r))
        .map(|id| format!("\t\t(void){}(&priv->inner);\n", cx.fn_symbols[&id]))
        .unwrap_or_default();

    Ok(format!(
        "static int {drv}_probe(struct amba_device *adev, const struct amba_id *id)\n{{\n\tstruct {drv}_priv *priv;\n\tint ret;\n\n\tpriv = devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL);\n\tif (!priv)\n\t\treturn -ENOMEM;\n\tpriv->adev = adev;\n\tamba_set_drvdata(adev, priv);\n{map_line}\n\tpriv->port.dev = &adev->dev;\n\tpriv->port.mapbase = adev->res.start;\n\tpriv->port.irq = adev->irq[0];\n\tpriv->port.iotype = UPIO_MEM32;\n\tpriv->port.ops = &{drv}_uart_ops;\n\tpriv->port.type = PORT_AMBA;\n\tpriv->port.fifosize = 16;\n\tpriv->port.flags = UPF_BOOT_AUTOCONF;\n\tpriv->port.line = 0;\n\tpriv->port.private_data = priv;\n\t/* uartclk feeds uart_get_baud_rate()/uart_update_timeout(); a nonzero value\n\t * is required (else baud=0 -> div-by-zero). TODO(clock-construct): bind this\n\t * from the clock-management construct — the author line_format_apply owns the\n\t * actual divisor math. QEMU virt PL011 reference clock as the placeholder. */\n\tpriv->port.uartclk = 24000000;\n{on_init_block}\n\tret = uart_add_one_port(&{drv}_uart_driver, &priv->port);\n\tif (ret) {{\n{add_fail_unwind}\t\treturn ret;\n\t}}\n\treturn 0;\n}}\n\n"
    ))
}

/// The serial module tail: the foreign-match audits, the AMBA id table +
/// device table, the `amba_driver`, and the module init/exit registering the
/// `uart_driver` around it (serial-core needs `uart_register_driver` before any
/// port is added — so a plain `module_amba_driver` is not enough).
fn emit_serial_tail(
    model: &RegseqModel,
    drv: &str,
    driver: &regseq_model::DriverLifecycle,
    native_entries: &[&MatchEntry],
    ignored_audits: &[crate::match_audit::MatchAudit],
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

    out.push_str(&format!(
        "static struct amba_driver {drv}_driver = {{\n\t.drv = {{\n\t\t.name\t= \"{name}\",\n\t}},\n\t.id_table\t= {drv}_ids,\n\t.probe\t\t= {drv}_probe,\n\t.remove\t\t= {drv}_remove,\n}};\n\n",
        name = model.strings.resolve(driver.descriptor.name)
    ));

    // serial-core registration order: uart_register_driver THEN amba_register
    // (and the reverse on exit) — uart_add_one_port in .probe needs the driver
    // already registered.
    out.push_str(&format!(
        "static int __init {drv}_init(void)\n{{\n\tint ret;\n\n\tret = uart_register_driver(&{drv}_uart_driver);\n\tif (ret)\n\t\treturn ret;\n\tret = amba_driver_register(&{drv}_driver);\n\tif (ret)\n\t\tuart_unregister_driver(&{drv}_uart_driver);\n\treturn ret;\n}}\nmodule_init({drv}_init);\n\nstatic void __exit {drv}_exit(void)\n{{\n\tamba_driver_unregister(&{drv}_driver);\n\tuart_unregister_driver(&{drv}_uart_driver);\n}}\nmodule_exit({drv}_exit);\n\n"
    ));
    out.push_str(&format!(
        "MODULE_DESCRIPTION(\"Regseq-generated {name} serial driver\");\nMODULE_LICENSE(\"GPL\");\n",
        name = model.strings.resolve(driver.descriptor.name)
    ));
    Ok(out)
}

// -- small layout helpers ----------------------------------------------------

/// The `(c_name, ty)` of a struct field by its Rust name.
fn struct_field(model: &RegseqModel, struct_ty: TyId, rust_name: &str) -> Option<(String, TyId)> {
    let layout = types::layout_of(model, struct_ty)?;
    let LayoutKind::Struct { fields } = &layout.kind else {
        return None;
    };
    fields
        .iter()
        .find(|f| model.strings.resolve(f.name) == rust_name)
        .map(|f| (c_ident(model.strings.resolve(f.name)), f.ty))
}

/// The C name of a struct's lowest-offset field (CauseSet `.bits` / RxDatum
/// leading `.unit`).
fn struct_first_field_name(model: &RegseqModel, struct_ty: TyId) -> Option<String> {
    let layout = types::layout_of(model, struct_ty)?;
    let LayoutKind::Struct { fields } = &layout.kind else {
        return None;
    };
    fields
        .iter()
        .min_by_key(|f| f.offset)
        .map(|f| c_ident(model.strings.resolve(f.name)))
}

