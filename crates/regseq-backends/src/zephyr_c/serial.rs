//! The zephyr_c serial device-class lowering (construct #7, PLAN T10/T11 / M3).
//!
//! A `#[rseq::device_class(serial)]` AMBA-over-DT driver lowers to a Zephyr
//! serial driver: a `struct uart_driver_api` vtable installed on the device
//! (`DEVICE_DT_INST_DEINIT_DEFINE(..., &<drv>_uart_api)`), with the per-source
//! interrupt mask (`irq_source_set`) SCATTERED to its Zephyr WHEN sites
//! (`serial_scatter::zephyr_scatter`) — the author owns WHICH BITS (the
//! mask-register writes in the op body), the compiler owns WHEN (R04/I01).
//!
//! Unlike the Linux row (where serial-core registration replaces the lifecycle
//! scaffold), the Zephyr row keeps the SAME device model — `DEVICE_MMIO_MAP` +
//! the authored `on_init`/`on_deinit` through the negated-errno adapter +
//! `IRQ_CONNECT` — and only ADDS the `uart_driver_api` plane: the slot bodies,
//! the synthesized callback-dispatch ISR, and the `&<drv>_uart_api` arg on the
//! instantiation macro (formerly NULL for the deferred-class path).
//!
//! Slot map (the v1 in-scope surface → `uart_driver_api`, modelled on the
//! upstream `pl011_driver_api` in `drivers/serial/uart_pl011.c`):
//!
//! | uart_driver_api slot | source |
//! |---|---|
//! | `poll_in` | `data_ready_query(RxData)` + `data_unit_read` |
//! | `poll_out` | spin on `data_ready_query(TxSpace)` + `data_unit_write` |
//! | `fifo_fill` / `fifo_read` | the base data path (bounded by `data_ready_query`) |
//! | `irq_tx_enable` / `irq_tx_disable` | per-source scatter (`irq_source_set` TxReady) + the TX prime-kick |
//! | `irq_rx_enable` / `irq_rx_disable` | per-source scatter (`irq_source_set` RxData) |
//! | `irq_err_enable` / `irq_err_disable` | per-source scatter (`irq_source_set` LineError) |
//! | `irq_tx_ready` / `irq_rx_ready` | `data_ready_query` (TxSpace / RxData) |
//! | `irq_tx_complete` | `tx_drain_query` (C05; T09 INCLUDE — FR.BUSY, not derivable from FR.TXFF) |
//! | `irq_update` | `irq_cause_decode` (the once-per-callback status read; its W1C error-ack) |
//! | `irq_is_pending` | `irq_rx_ready || irq_tx_ready` (the upstream predicate) |
//! | `irq_callback_set` + the ISR | the registered `uart_irq_callback_user_data_t` stored in `data` |
//! | `configure` | `line_format_apply` (C03); `ConfigResult::Unsupported` → `-ENOTSUP` (I03); the §3.3.8 disable→format→enable bracket |
//! | `config_get` | the cached `uart_config` (no invented HW readback — the surface has no line-format readback, I03) |
//!
//! The synthesized serial ISR (B3: no authored `#[rseq::isr]`) is the upstream
//! Zephyr shape — `if (data->cb) data->cb(dev, data->cb_data)` — the registered
//! callback owns the decode→dispatch (it calls `irq_update` then drains via
//! `fifo_read` / refills via `fifo_fill`). The TX prime-kick in `irq_tx_enable`
//! (call the callback once after arming) closes the same level-IRQ deadlock the
//! Linux M2 live run exposed: a level/edge PL011 TX IRQ does not fire for an
//! already-empty FIFO, so the callback is primed once to seed it.

use regseq_model::{
    DriverSerial, EnumBinding, EnumShape, InstanceId, InstanceRef, IrqSourceKind, IrqStateKind,
    LayoutKind, RegseqModel, TyId, VariantIdx,
};

use crate::capability;
use crate::serial_scatter::{verify_c02, zephyr_c02_schedule, zephyr_scatter, ZephyrSerialSite};

use super::body::{self, LowerCx};
use super::naming::c_ident;
use super::types::{self, TypeNames};
use super::{
    emit_binding_yaml, emit_result_adapter, emit_type_defs, reachable_closure, register_macros,
    resolve_instance_ref, scan_closure_ops, EmitError, ZephyrEmit,
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

/// The resolved author ops the serial scaffold wires + calls.
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

/// Lower a recognized serial-class AMBA-over-DT driver to a Zephyr serial TU.
///
/// Dispatched from [`super::emit_zephyr_c`] once the shared driver validation
/// (bus gates, the DT compatible, single resolved region, the A-10 binding)
/// has run and produced `compatible`, `ignored_audits`, `drv`, `driver_ty`.
/// Reuses the shared zephyr_c plumbing (register macros, the lowered-body
/// closure, type defs, the negated-errno adapter, the DT binding yaml) and
/// adds the `uart_driver_api` plane.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_zephyr_serial(
    model: &RegseqModel,
    driver_item: regseq_model::ItemId,
    driver: &regseq_model::DriverLifecycle,
    serial: &DriverSerial,
    compatible: &str,
    ignored_audits: &[crate::match_audit::MatchAudit],
    drv: &str,
    driver_ty: TyId,
) -> Result<ZephyrEmit, EmitError> {
    // T13 / O2 / G-E1: the zephyr scatter placement must satisfy the C02
    // interrupt/source-ordering edges (clear-pending-before-unmask;
    // disarm-before-teardown-disable) — build-failing on a violation.
    verify_c02(&zephyr_c02_schedule())
        .map_err(|v| unsup(format!("C02 source-ordering violation (T13/G-E1): {v}")))?;
    // -- the required planes (C01/C03/C05) -----------------------------------
    let irqd = serial.interrupt_driven.as_ref().ok_or_else(|| {
        unsup("serial M3 (Zephyr) requires the SerialInterruptDriven plane (C01)")
    })?;
    let lc = serial
        .line_config
        .as_ref()
        .ok_or_else(|| unsup("serial M3 (Zephyr) requires SerialRuntimeLineConfig (C03)"))?;
    let qc = serial.queue_control.as_ref().ok_or_else(|| {
        unsup(
            "serial M3 (Zephyr) requires SerialQueueControl::tx_drain_query (C05/T09): \
             uart_driver_api.irq_tx_complete reads FR.BUSY, not derivable from the base \
             data_ready_query(TxSpace) = FR.TXFF",
        )
    })?;
    if driver.descriptor.bus_type == Some(regseq_model::BusType::Pci) {
        return Err(EmitError::UnsupportedInUnit {
            what: "a PCIe serial scaffold".to_string(),
            unit: "a post-first-light serial unit",
        });
    }
    // pinctrl construct (M4/T21): the dedicated serial scaffold does NOT emit the
    // `PINCTRL_DT_INST_DEFINE` / init-hub acquire / binding `pinctrl-device.yaml`,
    // but the shared `body::emit_pinctrl` would still lower an authored
    // `apply::<S>()` to `pinctrl_apply_state(...)` — so a serial driver with a
    // RESOLVED pinctrl resource would emit body calls with no DT define / binding
    // (broken). Fail LOUD rather than silently drop the scaffold (Codex M4-diff
    // #1); serial+pinctrl wiring is out of M4 scope. An optional-ABSENT resource
    // needs no scaffold (its `apply` folds), so the gate is resolved-only.
    if super::pinctrl::has_resolved_pinctrl(model, driver_ty) {
        return Err(EmitError::UnsupportedInUnit {
            what: "a resolved pinctrl resource in a device-class-serial driver".into(),
            unit: "zephyr_c serial scaffold (pinctrl serial lowering is out of M4 scope)",
        });
    }

    // -- resources / register const bank / the TU-static base ----------------
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
    let reg_macros = register_macros(model, driver_ty, drv, &resource_c_names);
    let base_symbol = format!("{drv}_mmio_base");
    let has_region = !driver.handle.resource_fields.is_empty();

    let mut cx = LowerCx {
        model,
        names: TypeNames::new(),
        fn_symbols: std::collections::BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: body::DriverFacts {
            driver_ty,
            resources,
            reg_macros,
            base_symbol: base_symbol.clone(),
            pcie: false,
        },
        width8_used: false,
        uses_log: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
        elided_locals: std::collections::BTreeSet::new(),
    };

    // -- named roots: lifecycle callbacks + the serial ops -------------------
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

    let closure = reachable_closure(model, named_roots.iter().map(|(_, id)| *id));
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

    // -- frame scan over the closure (the D12 op-shape rejections), then the
    //    lowered author bodies (prototypes, then defs).
    scan_closure_ops(model, &closure, false)?;
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

    // -- the uart_driver_api plane (slot bodies, ISR, vtable) ----------------
    let api = emit_uart_api(&mut cx, drv, serial, &ops)?;

    // -- the init + deinit functions (the shared device model + the serial
    //    ISR connect + the port enable/disable WHEN sites).
    let init_fn = emit_serial_init(&mut cx, drv, &base_symbol, has_region, driver)?;
    let deinit_fn = emit_serial_deinit(&mut cx, drv, driver, serial, &ops)?;

    // -- type defs (now that every aggregate use is registered) --------------
    let type_defs = emit_type_defs(&mut cx, driver_ty)?;

    // -- assemble the TU.
    let resolved_region = driver
        .handle
        .resource_fields
        .first()
        .map(|f| (f.binding.base.unwrap(), f.binding.size.unwrap()));
    let tu = emit_serial_tu(
        &mut cx,
        drv,
        compatible,
        ignored_audits,
        driver_ty,
        &base_symbol,
        has_region,
        resolved_region,
        &type_defs,
        &bodies,
        &api,
        &init_fn,
        &deinit_fn,
    )?;
    // pinctrl construct (M4/T21): the dedicated serial scaffold does not wire the
    // pinctrl DT define / acquire (out of M4 scope — no fixture; the pinctrl
    // witness exercises the generic AMBA scaffold). `has_pinctrl = false`.
    let binding_yaml = emit_binding_yaml(model, drv, compatible, false, false, ignored_audits);

    // -- conformance: the delivered uart_driver_api slots + the per-source
    //    arming sites (R04). A serial driver DELIVERS the class (deferred_class
    //    is None — the C06 deferred-class note is for the class-neutral path).
    let _ = driver_item;
    let mut delivered_slots: Vec<capability::SlotEntry> = [
        "uart_driver_api.poll_in",
        "uart_driver_api.poll_out",
        "uart_driver_api.err_check",
        "uart_driver_api.configure",
        "uart_driver_api.config_get",
        "uart_driver_api.fifo_fill",
        "uart_driver_api.fifo_read",
        "uart_driver_api.irq_tx_ready",
        "uart_driver_api.irq_tx_complete",
        "uart_driver_api.irq_rx_ready",
        "uart_driver_api.irq_is_pending",
        "uart_driver_api.irq_update",
        "uart_driver_api.irq_callback_set",
    ]
    .iter()
    .map(|s| capability::SlotEntry {
        slot: (*s).to_string(),
        row: None,
        citation: None,
    })
    .collect();
    for (site, label) in [
        (ZephyrSerialSite::IrqTxEnable, "serial.irq_source_set@irq_tx_enable"),
        (ZephyrSerialSite::IrqTxDisable, "serial.irq_source_set@irq_tx_disable"),
        (ZephyrSerialSite::IrqRxEnable, "serial.irq_source_set@irq_rx_enable"),
        (ZephyrSerialSite::IrqRxDisable, "serial.irq_source_set@irq_rx_disable"),
        (ZephyrSerialSite::IrqErrEnable, "serial.irq_source_set@irq_err_enable"),
        (ZephyrSerialSite::IrqErrDisable, "serial.irq_source_set@irq_err_disable"),
    ] {
        for intent in zephyr_scatter(site) {
            delivered_slots.push(capability::SlotEntry {
                slot: format!("{label}:{:?}->{:?}", intent.source, intent.state),
                row: None,
                citation: None,
            });
        }
    }
    let conformance = capability::ConformanceReport {
        target: "zephyr_c",
        os_version_pin: capability::ZEPHYR_PIN,
        delivered_slots,
        absent_dispatch_slots: Vec::new(),
        structurally_absent_slots: Vec::new(),
        rejected_slots: Vec::new(),
        ignored_match_entries: ignored_audits.to_vec(),
        deferred_class: None,
    };

    Ok(ZephyrEmit {
        tu,
        binding_yaml,
        driver_name: drv.to_string(),
        conformance,
    })
}

/// The emitted `uart_driver_api` plane: the slot bodies, the synthesized ISR,
/// and the `DEVICE_API(uart, <drv>_uart_api)` vtable.
struct UartApi {
    /// The slot function definitions + the ISR (emitted into the bodies bank).
    fns: String,
    /// The `DEVICE_API(uart, ...)` vtable literal.
    vtable: String,
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

/// The `(source, state)` literal call to the author `irq_source_set`. The
/// author body writes IMSC — the per-source mask write (R04/I01) — DISTINCT
/// from the line plumbing (`IRQ_CONNECT`/`irq_enable`) the init fn emits.
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
        "\t{}(&data->inner, {src_lit}, {st_lit});\n",
        ops.irq_source_set.sym
    ))
}

fn scatter_call(
    cx: &mut LowerCx<'_>,
    serial: &DriverSerial,
    ops: &SerialOps,
    site: ZephyrSerialSite,
) -> Result<String, EmitError> {
    let mut out = String::new();
    for intent in zephyr_scatter(site) {
        out.push_str(&iss_call(cx, serial, ops, intent.source, intent.state)?);
    }
    Ok(out)
}

/// Emit the full `uart_driver_api` plane.
fn emit_uart_api(
    cx: &mut LowerCx<'_>,
    drv: &str,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<UartApi, EmitError> {
    // irq_lock/irq_unlock serialize the TX prime against the now-armed TX IRQ.
    cx.extra_includes.insert("<zephyr/irq.h>");
    // atomic_t rx_err: fifo_read accumulates from ISR context while err_check
    // read-and-clears from thread context — a plain `|=` vs read+clear races
    // (lost/resurrected errors) even single-core via preemption (Codex F-Z3).
    cx.extra_includes.insert("<zephyr/sys/atomic.h>");
    let mut out = String::new();

    // ReadyDir carriers (ReadyDir::TxSpace=0, RxData=1) — the data-readiness
    // predicate args.
    let txspace = enum_carrier(cx, ops.data_ready_query.inputs[1], &serial.types.ready_dir, 0)?;
    let rxdata = enum_carrier(cx, ops.data_ready_query.inputs[1], &serial.types.ready_dir, 1)?;
    // RxDatum carrier + its leading unit field (data_unit_read's payload).
    let rxdatum_cty = types::c_type(cx.model, &mut cx.names, ops.data_unit_read.output)?;
    let rx_unit_field = struct_first_field_name(cx.model, ops.data_unit_read.output)
        .ok_or_else(|| unsup("RxDatum has no leading unit field"))?;
    // The inline RxError carrier (`RxDatum.errors`) + `.bits`, and the per-kind
    // masks from the pinned RxErrorRepr (the O2-analog): poll_in/fifo_read cache
    // the inline error into `data->rx_err`, surfaced via `err_check` (F-Z3).
    let (rx_err_field, rx_err_ty) = struct_field(cx.model, ops.data_unit_read.output, "errors")
        .ok_or_else(|| unsup("RxDatum has no `errors` field"))?;
    let rx_err_bits = struct_first_field_name(cx.model, rx_err_ty)
        .ok_or_else(|| unsup("RxError has no bits field"))?;
    let rxerr_mask = |k: regseq_model::SerialRxErrorKind| 1u32 << (k as u32);
    use regseq_model::SerialRxErrorKind;
    let (m_frame, m_parity, m_break, m_hwovr, m_swovr) = (
        rxerr_mask(SerialRxErrorKind::Framing),
        rxerr_mask(SerialRxErrorKind::Parity),
        rxerr_mask(SerialRxErrorKind::Break),
        rxerr_mask(SerialRxErrorKind::HwOverrun),
        rxerr_mask(SerialRxErrorKind::SwOverrun),
    );
    // The union of the pinned error-domain bits: accumulate only these into
    // rx_err so a stray high bit outside the RxError vocab can never be stored
    // (defensive, Codex F-Z3 nit).
    let m_all: u32 = serial
        .base
        .rx_error
        .kinds
        .iter()
        .fold(0u32, |acc, k| acc | (1u32 << (*k as u32)));

    // -- poll API (always present) -------------------------------------------
    out.push_str(&format!(
        "/* poll_in <- data_ready_query(RxData) + data_unit_read (the base data path) */\nstatic int {drv}_poll_in(const struct device *dev, unsigned char *c)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{rxdatum_cty} __d;\n\n\tif (!{ready}(&data->inner, {rxdata}))\n\t\treturn -1;\n\t__d = {read}(&data->inner);\n\tatomic_or(&data->rx_err, (atomic_val_t)(__d.{err_field}.{err_bits} & {m_all}u)); /* accumulate inline RX errors for err_check (F-Z3) */\n\t*c = (unsigned char)(__d.{unit} & 0xff);\n\treturn 0;\n}}\n\n",
        ready = ops.data_ready_query.sym,
        read = ops.data_unit_read.sym,
        unit = rx_unit_field,
        err_field = rx_err_field,
        err_bits = rx_err_bits,
    ));
    out.push_str(&format!(
        "/* poll_out <- spin on data_ready_query(TxSpace) + data_unit_write */\nstatic void {drv}_poll_out(const struct device *dev, unsigned char c)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\twhile (!{ready}(&data->inner, {txspace}))\n\t\t;\n\t{write}(&data->inner, c);\n}}\n\n",
        ready = ops.data_ready_query.sym,
        write = ops.data_unit_write.sym,
    ));
    // err_check (always-present base API): report + clear the accumulated inline
    // RxError, mapped from the pinned RxErrorRepr bits to Zephyr's vocab (F-Z3).
    // The surface exposes errors only inline via data_unit_read, so err_check
    // returns what poll_in/fifo_read have accumulated since the last call.
    out.push_str(&format!(
        "/* err_check <- the accumulated inline RxError (I04), mapped to Zephyr's\n * UART_ERROR_* / UART_BREAK vocab; atomic read-and-clear (no HW peek — the\n * surface carries errors only inline on data_unit_read; atomic vs the ISR\n * fifo_read accumulation, Codex F-Z3) */\nstatic int {drv}_err_check(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\tatomic_val_t __e = atomic_clear(&data->rx_err);\n\tint __r = 0;\n\n\tif (__e & {m_frame}u)\n\t\t__r |= UART_ERROR_FRAMING;\n\tif (__e & {m_parity}u)\n\t\t__r |= UART_ERROR_PARITY;\n\tif (__e & ({m_hwovr}u | {m_swovr}u))\n\t\t__r |= UART_ERROR_OVERRUN;\n\tif (__e & {m_break}u)\n\t\t__r |= UART_BREAK;\n\treturn __r;\n}}\n\n"
    ));

    // -- runtime configure (CONFIG_UART_USE_RUNTIME_CONFIGURE) ---------------
    out.push_str("#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\n");
    out.push_str(&emit_configure(cx, drv, serial, ops)?);
    out.push_str(&format!(
        "/* config_get <- the cached uart_config (the surface has no line-format\n * readback, I03; no invented HW read) */\nstatic int {drv}_config_get(const struct device *dev, struct uart_config *cfg)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\t*cfg = data->uart_cfg;\n\treturn 0;\n}}\n"
    ));
    out.push_str("#endif /* CONFIG_UART_USE_RUNTIME_CONFIGURE */\n\n");

    // -- interrupt-driven plane (CONFIG_UART_INTERRUPT_DRIVEN) ----------------
    out.push_str("#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n");
    // fifo_fill / fifo_read: the base data path bounded by the readiness query.
    out.push_str(&format!(
        "/* fifo_fill <- data_unit_write bounded by data_ready_query(TxSpace) */\nstatic int {drv}_fifo_fill(const struct device *dev, const uint8_t *tx, int len)\n{{\n\tstruct {drv}_data *data = dev->data;\n\tint n = 0;\n\n\twhile (n < len && {ready}(&data->inner, {txspace}))\n\t\t{write}(&data->inner, tx[n++]);\n\treturn n;\n}}\n\n",
        ready = ops.data_ready_query.sym,
        write = ops.data_unit_write.sym,
    ));
    out.push_str(&format!(
        "/* fifo_read <- data_unit_read bounded by data_ready_query(RxData) */\nstatic int {drv}_fifo_read(const struct device *dev, uint8_t *rx, const int size)\n{{\n\tstruct {drv}_data *data = dev->data;\n\tint n = 0;\n\n\twhile (n < size && {ready}(&data->inner, {rxdata})) {{\n\t\t{rxdatum_cty} __d = {read}(&data->inner);\n\t\tatomic_or(&data->rx_err, (atomic_val_t)(__d.{err_field}.{err_bits} & {m_all}u)); /* accumulate inline RX errors for err_check (F-Z3) */\n\t\trx[n++] = (uint8_t)(__d.{unit} & 0xff);\n\t}}\n\treturn n;\n}}\n\n",
        ready = ops.data_ready_query.sym,
        read = ops.data_unit_read.sym,
        unit = rx_unit_field,
        err_field = rx_err_field,
        err_bits = rx_err_bits,
    ));
    // irq_tx_enable: arm TxReady (scatter) THEN prime — the kick. A
    // level/edge PL011 TX IRQ does not fire for an already-empty FIFO (confirmed
    // live on Zephyr: arm-only transmits 0 bytes), so the registered callback is
    // primed once to seed the FIFO. The prime runs under irq_lock: once TxReady
    // is armed the TX IRQ can fire, and without the lock it PREEMPTS the prime in
    // thread context — a second, concurrent callback racing the first on the
    // app's TX cursor (the live byte-IO double-send finding; the upstream
    // pl011_irq_tx_enable serializes the same dispatch with a spinlock, R18-noted).
    out.push_str(&format!(
        "/* irq_tx_enable: arm TxReady, then prime-kick the callback. The arm shadow\n * (tx_irq_on) and the HW IMSC.TXIM mask are TWO mirrors of one arm state and MUST\n * be set together under irq_lock: a re-entrant disarm (from the synthesized ISR's\n * callback path) that tears them apart leaves HW TXIM set while the shadow reads\n * false — a permanent, unserviceable level TX-IRQ storm (the M3 shell-burst\n * wedge). THEN prime-kick (the level TX IRQ does not fire for an already-empty\n * FIFO — confirmed live: arm-only sends 0 bytes); the prime runs under its own\n * irq_lock so the now-armed TX IRQ cannot PREEMPT it and re-enter the callback\n * concurrently (the live byte-IO double-send finding; the upstream\n * pl011_irq_tx_enable serializes the same dispatch with a spinlock) */\nstatic void {drv}_irq_tx_enable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __arm_key = irq_lock();\n\tdata->tx_irq_on = true;\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqTxEnable)?);
    out.push_str("\tirq_unlock(__arm_key);\n");
    out.push_str("\tif (data->cb) {\n\t\tunsigned int __key = irq_lock();\n\t\tdata->cb(dev, data->cb_data);\n\t\tirq_unlock(__key);\n\t}\n}\n\n");
    out.push_str(&format!(
        "/* irq_tx_disable: disarm TxReady + clear the shadow, together under irq_lock\n * (the same atomicity irq_tx_enable needs — shadow and HW IMSC are one state) */\nstatic void {drv}_irq_tx_disable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __disarm_key = irq_lock();\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqTxDisable)?);
    out.push_str("\tdata->tx_irq_on = false;\n\tirq_unlock(__disarm_key);\n}\n\n");
    out.push_str(&format!(
        "/* irq_tx_ready: TX armed AND FR.TXFF clear (FIFO has space). The arm gate\n * is load-bearing: FR.TXFF is clear whenever the FIFO has room (i.e. nearly\n * always when idle), so an ungated predicate would make the canonical callback\n * loop `while (irq_update() && irq_is_pending())` never terminate (the upstream\n * pl011_irq_tx_ready likewise gates on IMSC.TXIM; the surface has no IMSC query,\n * so the backend tracks the arm) */\nstatic int {drv}_irq_tx_ready(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\treturn data->tx_irq_on && {ready}(&data->inner, {txspace});\n}}\n\n",
        ready = ops.data_ready_query.sym,
    ));
    out.push_str(&format!(
        "/* irq_tx_complete <- tx_drain_query = FR.BUSY|FR.TXFF clear (C05/T09; the\n * shift register is drained, NOT derivable from FR.TXFF alone) */\nstatic int {drv}_irq_tx_complete(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\treturn {drain}(&data->inner);\n}}\n\n",
        drain = ops.tx_drain_query.sym,
    ));
    out.push_str(&format!(
        "/* irq_rx_enable: arm RxData + set the shadow, together under irq_lock (the\n * same shadow/IMSC atomicity the TX path needs) */\nstatic void {drv}_irq_rx_enable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __rxen_key = irq_lock();\n\tdata->rx_irq_on = true;\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqRxEnable)?);
    out.push_str("\tirq_unlock(__rxen_key);\n}\n\n");
    out.push_str(&format!(
        "/* irq_rx_disable: disarm RxData + clear the shadow, together under irq_lock */\nstatic void {drv}_irq_rx_disable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __rxdis_key = irq_lock();\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqRxDisable)?);
    out.push_str("\tdata->rx_irq_on = false;\n\tirq_unlock(__rxdis_key);\n}\n\n");
    out.push_str(&format!(
        "/* irq_rx_ready: RX armed AND FR.RXFE clear (data present). Gated on the arm\n * for symmetry with irq_tx_ready (the data-present check self-limits, but the\n * gate mirrors the upstream IMSC.RXIM check) */\nstatic int {drv}_irq_rx_ready(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\treturn data->rx_irq_on && {ready}(&data->inner, {rxdata});\n}}\n\n",
        ready = ops.data_ready_query.sym,
    ));
    out.push_str(&format!(
        "/* irq_err_enable: arm LineError under irq_lock — every writer of the shared\n * IMSC shadow serializes (a public-API err arm must not race a TX/RX disarm on\n * imsc_shadow and re-commit a stale mask) */\nstatic void {drv}_irq_err_enable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __erren_key = irq_lock();\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqErrEnable)?);
    out.push_str("\tirq_unlock(__erren_key);\n}\n\n");
    out.push_str(&format!(
        "/* irq_err_disable: disarm LineError under irq_lock (imsc_shadow atomicity) */\nstatic void {drv}_irq_err_disable(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tunsigned int __errdis_key = irq_lock();\n"
    ));
    out.push_str(&scatter_call(cx, serial, ops, ZephyrSerialSite::IrqErrDisable)?);
    out.push_str("\tirq_unlock(__errdis_key);\n}\n\n");
    out.push_str(&format!(
        "/* irq_is_pending <- the upstream predicate (rx_ready || tx_ready) */\nstatic int {drv}_irq_is_pending(const struct device *dev)\n{{\n\treturn {drv}_irq_rx_ready(dev) || {drv}_irq_tx_ready(dev);\n}}\n\n"
    ));
    out.push_str(&format!(
        "/* irq_update <- irq_cause_decode: Zephyr's once-per-callback status read;\n * the authored decode W1C-acks the line-error causes as it reads MIS (F1/B5) */\nstatic int {drv}_irq_update(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\t(void){decode}(&data->inner);\n\treturn 1;\n}}\n\n",
        decode = ops.irq_cause_decode.sym,
    ));
    out.push_str(&format!(
        "/* irq_callback_set: store the registered uart callback in data */\nstatic void {drv}_irq_callback_set(const struct device *dev,\n\t\t\t\t   uart_irq_callback_user_data_t cb, void *cb_data)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tdata->cb = cb;\n\tdata->cb_data = cb_data;\n}}\n\n"
    ));
    // The synthesized serial ISR (B3): the upstream callback-dispatch shape —
    // the registered callback owns the decode→drain/refill (it calls
    // irq_update + irq_rx_ready/fifo_read + irq_tx_ready/fifo_fill).
    out.push_str(&format!(
        "/* the synthesized serial ISR (construct #7, B3): no authored #[rseq::isr];\n * the compiler dispatches to the registered uart callback (upstream shape) */\nstatic void {drv}_isr(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\tif (data->cb)\n\t\tdata->cb(dev, data->cb_data);\n}}\n"
    ));
    out.push_str("#endif /* CONFIG_UART_INTERRUPT_DRIVEN */\n\n");

    // -- the vtable ----------------------------------------------------------
    let mut vtable = String::new();
    vtable.push_str(&format!(
        "/* the uart_driver_api vtable (the delivered serial class, R04) */\nstatic DEVICE_API(uart, {drv}_uart_api) = {{\n\t.poll_in\t= {drv}_poll_in,\n\t.poll_out\t= {drv}_poll_out,\n\t.err_check\t= {drv}_err_check,\n#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\n\t.configure\t= {drv}_configure,\n\t.config_get\t= {drv}_config_get,\n#endif\n#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n\t.fifo_fill\t= {drv}_fifo_fill,\n\t.fifo_read\t= {drv}_fifo_read,\n\t.irq_tx_enable\t= {drv}_irq_tx_enable,\n\t.irq_tx_disable\t= {drv}_irq_tx_disable,\n\t.irq_tx_ready\t= {drv}_irq_tx_ready,\n\t.irq_tx_complete= {drv}_irq_tx_complete,\n\t.irq_rx_enable\t= {drv}_irq_rx_enable,\n\t.irq_rx_disable\t= {drv}_irq_rx_disable,\n\t.irq_rx_ready\t= {drv}_irq_rx_ready,\n\t.irq_err_enable\t= {drv}_irq_err_enable,\n\t.irq_err_disable= {drv}_irq_err_disable,\n\t.irq_is_pending\t= {drv}_irq_is_pending,\n\t.irq_update\t= {drv}_irq_update,\n\t.irq_callback_set = {drv}_irq_callback_set,\n#endif\n}};\n\n"
    ));

    Ok(UartApi { fns: out, vtable })
}

/// `configure` ← `line_format_apply` (C03), surfacing `ConfigResult::Unsupported`
/// as `-ENOTSUP` (I03 — not silently defaulted), inside the §3.3.8
/// disable→drain(FR.BUSY)→format→enable bracket. **D17** adds the drain wait (via
/// the author's `tx_drain_query`); the C03 FEN-clear/FIFO-flush edge is deferred
/// (no register-role metadata for the FIFO-enable bit, no in-scope FIFO op).
fn emit_configure(
    cx: &mut LowerCx<'_>,
    drv: &str,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<String, EmitError> {
    let lf_ty = ops.line_format_apply.inputs[1];
    let lf_cty = types::c_type(cx.model, &mut cx.names, lf_ty)?;
    let (baud_name, _) = struct_field(cx.model, lf_ty, "baud")
        .ok_or_else(|| unsup("LineFormat has no baud field"))?;
    let (db_name, db_ty) = struct_field(cx.model, lf_ty, "data_bits")
        .ok_or_else(|| unsup("LineFormat has no data_bits field"))?;
    let (par_name, par_ty) = struct_field(cx.model, lf_ty, "parity")
        .ok_or_else(|| unsup("LineFormat has no parity field"))?;
    let (sb_name, sb_ty) = struct_field(cx.model, lf_ty, "stop_bits")
        .ok_or_else(|| unsup("LineFormat has no stop_bits field"))?;

    // DataBits carriers: Five..Nine (canonical positions 0..4). Nine routes to
    // the author's line_format_apply, which returns Unsupported (I03) — the
    // proven 9-bit reject path; the configure() wrapper never silently coerces.
    let db5 = enum_carrier(cx, db_ty, &serial.types.data_bits, 0)?;
    let db6 = enum_carrier(cx, db_ty, &serial.types.data_bits, 1)?;
    let db7 = enum_carrier(cx, db_ty, &serial.types.data_bits, 2)?;
    let db8 = enum_carrier(cx, db_ty, &serial.types.data_bits, 3)?;
    let db9 = enum_carrier(cx, db_ty, &serial.types.data_bits, 4)?;
    // Parity: None=0, Even=1, Odd=2.
    let par_none = enum_carrier(cx, par_ty, &serial.types.parity, 0)?;
    let par_even = enum_carrier(cx, par_ty, &serial.types.parity, 1)?;
    let par_odd = enum_carrier(cx, par_ty, &serial.types.parity, 2)?;
    // StopBits: One=0, Two=1.
    let sb_one = enum_carrier(cx, sb_ty, &serial.types.stop_bits, 0)?;
    let sb_two = enum_carrier(cx, sb_ty, &serial.types.stop_bits, 1)?;

    // PortScope::Block carrier for the §3.3.8 bracket.
    let scope_block = enum_carrier(cx, ops.port_service_set.inputs[1], &serial.types.port_scope, 0)?;
    let pss = ops.port_service_set.sym.clone();

    // ConfigResult::Unsupported tag read.
    let cr_ty = ops.line_format_apply.output;
    let (cr_off, cr_size) = types::enum_tag_cell(cx.model, cr_ty)?;
    if cr_size != 1 {
        return Err(unsup("ConfigResult tag wider than one byte (first-light subset)"));
    }
    let unsupported_tag = enum_variant_tag(cx.model, cr_ty, &serial.types.config_result, 1)?;
    let cr_cty = types::c_type(cx.model, &mut cx.names, cr_ty)?;
    // D17: the configure-sequencing busy-wait drains the shift register (FR.BUSY
    // clear) via the author's tx_drain_query before reprogram (device-neutral).
    let drain = ops.tx_drain_query.sym.clone();

    Ok(format!(
        "/* configure <- line_format_apply (C03): translate uart_config -> LineFormat.\n * Zephyr's uart_config vocab is RICHER than the framework-pure surface\n * (9-bit, MARK/SPACE parity, 0.5/1.5 stop, hw flow control), so an\n * unrepresentable request is REJECTED as -ENOTSUP, never silently coerced\n * (I03); 9-bit routes through the author's line_format_apply -> Unsupported.\n * The modem/flow plane is the deferred SerialModemControl construct. The\n * §3.3.8 disable->format->enable bracket leaves the port enabled on success. */\nstatic int {drv}_configure(const struct device *dev, const struct uart_config *cfg)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{lf_cty} lf;\n\t{cr_cty} __r;\n\n\tif (cfg->flow_ctrl != UART_CFG_FLOW_CTRL_NONE)\n\t\treturn -ENOTSUP; /* hw flow control is the deferred modem plane */\n\tlf.{baud_name} = cfg->baudrate;\n\tswitch (cfg->data_bits) {{\n\tcase UART_CFG_DATA_BITS_5: lf.{db_name} = {db5}; break;\n\tcase UART_CFG_DATA_BITS_6: lf.{db_name} = {db6}; break;\n\tcase UART_CFG_DATA_BITS_7: lf.{db_name} = {db7}; break;\n\tcase UART_CFG_DATA_BITS_8: lf.{db_name} = {db8}; break;\n\tcase UART_CFG_DATA_BITS_9: lf.{db_name} = {db9}; break;\n\tdefault:                   return -ENOTSUP;\n\t}}\n\tswitch (cfg->parity) {{\n\tcase UART_CFG_PARITY_NONE: lf.{par_name} = {par_none}; break;\n\tcase UART_CFG_PARITY_ODD:  lf.{par_name} = {par_odd}; break;\n\tcase UART_CFG_PARITY_EVEN: lf.{par_name} = {par_even}; break;\n\tdefault:                   return -ENOTSUP; /* MARK/SPACE unrepresentable */\n\t}}\n\tswitch (cfg->stop_bits) {{\n\tcase UART_CFG_STOP_BITS_1: lf.{sb_name} = {sb_one}; break;\n\tcase UART_CFG_STOP_BITS_2: lf.{sb_name} = {sb_two}; break;\n\tdefault:                   return -ENOTSUP; /* 0.5/1.5 unrepresentable */\n\t}}\n\n\t/* PL011 §3.3.8 / D17 / C03: reprogram baud + line-control only while the port\n\t * is disabled, and only AFTER the current transfer drains (FR.BUSY clear, via\n\t * tx_drain_query) so an in-flight char is not corrupted; the FEN-clear/FIFO\n\t * flush edge is deferred (no register-role metadata / no in-scope FIFO op).\n\t * The port is re-enabled ONLY on success - a rejected format leaves the port\n\t * DISABLED (never bring an unconfigured/half-applied port into service); on\n\t * success this re-enable is the bring-into-service WHEN site. */\n\t{pss}(&data->inner, {scope_block}, false);\n\twhile (!{drain}(&data->inner)) {{\n\t\t/* drain the shift register (FR.BUSY clear) before reprogram */\n\t}}\n\t__r = {apply}(&data->inner, lf);\n\tif (__r._raw[{cr_off}] == {unsupported_tag:#x})\n\t\treturn -ENOTSUP; /* port stays disabled */\n\t{pss}(&data->inner, {scope_block}, true);\n\tdata->uart_cfg = *cfg;\n\treturn 0;\n}}\n",
        apply = ops.line_format_apply.sym,
    ))
}

/// The serial init fn: the shared device-model preamble (`DEVICE_MMIO_MAP` +
/// the TU-static base install) → the authored `on_init` through the
/// negated-errno adapter → the port enable (the construct's bring-into-service
/// WHEN site, the Zephyr analog of the Linux `startup`) → `IRQ_CONNECT` of the
/// synthesized serial ISR + `irq_enable` (the line, DT-authoritative).
fn emit_serial_init(
    cx: &mut LowerCx<'_>,
    drv: &str,
    base_symbol: &str,
    has_region: bool,
    driver: &regseq_model::DriverLifecycle,
) -> Result<String, EmitError> {
    cx.extra_includes.insert("<zephyr/irq.h>");

    let mut out = String::new();
    out.push_str(&format!(
        "/* the one runtime dispatch hub (scaffold §3) + the serial line-IRQ connect */\nstatic int {drv}_init(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n"
    ));
    if has_region {
        out.push_str(
            "\t/* resource preamble: claim_regions = zero calls (the DT `reg` capture\n\t * happened at build time); the canonical map slot precedes every MMIO op */\n\tDEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);\n",
        );
        out.push_str(&format!(
            "\t{base_symbol} = DEVICE_MMIO_GET(dev); /* TU-static base (A-2) */\n\n"
        ));
    }
    // The authored on_init over the embedded spec driver (safe IRQ defaults),
    // through the negated-errno adapter (Err -> -errno; Ok falls through).
    let on_init_id = driver
        .ops
        .on_init
        .as_ref()
        .and_then(|r| resolve_instance_ref(cx.model, *r));
    if let Some(id) = on_init_id {
        let sym = cx.fn_symbols[&id].clone();
        out.push_str(
            "\t/* authored on_init over the embedded spec driver (safe IRQ defaults:\n\t * all sources masked, nothing latched); the negated-errno adapter HERE */\n",
        );
        out.push_str(&emit_result_adapter(cx, id, &format!("{sym}(&data->inner)"), None)?);
        out.push('\n');
    }
    // The port is DELIBERATELY left disabled here: the framework-pure surface
    // carries no default line format, and the compiler does not invent a baud.
    // The first uart_configure() applies the format AND brings the port into
    // service (the §3.3.8 bracket's trailing port_service_set(Block, true)) —
    // the Zephyr analog of the Linux serial-core enabling the port on open, not
    // probe. A Zephyr app configures a UART before IO; device_is_ready() is
    // still true after init (init returned 0).
    out.push_str(
        "\t/* the port stays DISABLED until uart_configure() applies a line format\n\t * (no invented default baud); configure() brings it into service */\n",
    );
    // The line IRQ: connect the synthesized serial ISR + enable (DT
    // authoritative; the L2 irq_number is advisory on Zephyr, R10). Guarded by
    // CONFIG_UART_INTERRUPT_DRIVEN — the ISR only exists under it. A masked-source
    // line (on_init left IMSC=0) enabled at the GIC is safe: nothing interrupts
    // until the app arms a source via uart_irq_{rx,tx,err}_enable.
    out.push_str(&format!(
        "#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n\t/* connect + enable the line IRQ (DT-authoritative); the synthesized serial\n\t * ISR dispatches to the registered uart callback. Safe pre-configure: all\n\t * sources are masked until the app arms one */\n\tIRQ_CONNECT(DT_INST_IRQN(0), DT_INST_IRQ(0, priority), {drv}_isr,\n\t\t    DEVICE_DT_INST_GET(0), 0);\n\tirq_enable(DT_INST_IRQN(0));\n#endif\n\treturn 0;\n}}\n"
    ));
    Ok(out)
}

/// The serial deinit fn: mask the line IRQ → the authored `on_deinit` through
/// the negated-errno adapter → the port disable (the teardown counterpart of
/// the init bring-up).
fn emit_serial_deinit(
    cx: &mut LowerCx<'_>,
    drv: &str,
    driver: &regseq_model::DriverLifecycle,
    serial: &DriverSerial,
    ops: &SerialOps,
) -> Result<String, EmitError> {
    let scope_block =
        enum_carrier(cx, ops.port_service_set.inputs[1], &serial.types.port_scope, 0)?;
    let on_deinit_id = driver
        .ops
        .on_deinit
        .as_ref()
        .and_then(|r| resolve_instance_ref(cx.model, *r))
        .ok_or_else(|| EmitError::Driver("serial driver requires an authored on_deinit".into()))?;
    let sym = cx.fn_symbols[&on_deinit_id].clone();

    let mut out = String::new();
    out.push_str(&format!(
        "/* the teardown dispatch hub: mask the line, run on_deinit, disable the port */\nstatic int {drv}_deinit(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n\tirq_disable(DT_INST_IRQN(0));\n#endif\n"
    ));
    out.push_str(
        "\t/* authored on_deinit (mask all sources + clear latched) through the same\n\t * negated-errno adapter as init (the rc is returned verbatim) */\n",
    );
    out.push_str(&emit_result_adapter(cx, on_deinit_id, &format!("{sym}(&data->inner)"), None)?);
    out.push_str(&format!(
        "\t{pss}(&data->inner, {scope_block}, false); /* disable the port */\n\treturn 0;\n}}\n",
        pss = ops.port_service_set.sym,
    ));
    Ok(out)
}

/// Assemble the serial TU per the scaffold §1 skeleton, with the
/// `uart_driver_api` plane + the `&<drv>_uart_api` instantiation arg.
#[allow(clippy::too_many_arguments)]
fn emit_serial_tu(
    cx: &mut LowerCx<'_>,
    drv: &str,
    compatible: &str,
    ignored_audits: &[crate::match_audit::MatchAudit],
    driver_ty: TyId,
    base_symbol: &str,
    has_region: bool,
    resolved_region: Option<(u64, u64)>,
    type_defs: &str,
    bodies: &str,
    api: &UartApi,
    init_fn: &str,
    deinit_fn: &str,
) -> Result<String, EmitError> {
    let model = cx.model;
    let inner_tag = cx.names.name_of(model, driver_ty)?;
    let log_demand = cx.uses_log;
    let compat_token = super::naming::compat_token(compatible);

    let mut tu = String::new();
    tu.push_str(&format!(
        "/*\n * {drv}.c - Regseq-generated Zephyr serial driver (devicetree match).\n *\n * Generated by the regseq zephyr_c serial lowering (construct #7); do not edit.\n *\n * Init placement: POST_KERNEL / CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\n * hardcoded (ratified D13). The DUT is NOT the boot console (the stock\n * arm,pl011 owns uart0), so PRE_KERNEL_1 is not required.\n *\n * Serial Kconfig pins (documented not asserted): CONFIG_SERIAL=y,\n * CONFIG_UART_INTERRUPT_DRIVEN=y (the irq/fifo plane), and\n * CONFIG_UART_USE_RUNTIME_CONFIGURE=y (the configure/config_get plane).\n"
    ));
    if log_demand {
        tu.push_str(
            " *\n * Logging Kconfig pins (A-9, documented not asserted): CONFIG_LOG=y,\n * deferred mode, CONFIG_CBPRINTF_COMPLETE, CONFIG_CBPRINTF_FULL_INTEGRAL\n * (for %lld/%llu).\n",
        );
    }
    if cx.width8_used {
        tu.push_str(
            " *\n * Width-8 MMIO note (D19/C-2): sys_read64/sys_write64 are\n * arch-conditional; this TU is validated on aarch64.\n",
        );
    }
    tu.push_str(
        " *\n * Teardown Kconfig pin (A-9, documented not asserted):\n * CONFIG_DEVICE_DEINIT_SUPPORT=y — Z_DEVICE_OPS silently DROPS deinit_fn\n * under IF_ENABLED when off (device.h); device_deinit() returns -ENOTSUP.\n */\n\n",
    );

    // §1 item 1: the foreign-match audits adjacent to the compatible pin.
    for audit in ignored_audits {
        tu.push_str(&audit.c_block());
        tu.push('\n');
    }
    tu.push_str(&format!("#define DT_DRV_COMPAT {compat_token}\n\n"));

    // §1 item 2: the demand-driven log-module block.
    if log_demand {
        tu.push_str(&format!(
            "#define LOG_MODULE_NAME {drv}\n#include <zephyr/logging/log.h>\nLOG_MODULE_REGISTER(LOG_MODULE_NAME);\n\n"
        ));
    }

    // §1 item 3: demand-driven includes (+ the uart subsystem header for the
    // uart_driver_api / DEVICE_API / uart_irq_callback_user_data_t / uart_config).
    let mut includes: std::collections::BTreeSet<&str> = [
        "<zephyr/device.h>",
        "<zephyr/drivers/uart.h>",
        "<stdint.h>",
    ]
    .into_iter()
    .collect();
    if has_region {
        includes.insert("<zephyr/sys/device_mmio.h>");
    }
    includes.extend(cx.extra_includes.iter().copied());
    for inc in &includes {
        tu.push_str(&format!("#include {inc}\n"));
    }
    tu.push('\n');

    // Register-offset const bank.
    if !cx.driver.reg_macros.is_empty() {
        tu.push_str("/* register offsets, recovered from the spec's #[repr(C)] block layout */\n");
        for (name, off) in &cx.driver.reg_macros {
            tu.push_str(&format!("#define {name} {off:#05x}\n"));
        }
        tu.push('\n');
    }

    // §1 item 4: type defs, then config(ROM)/data(RAM). The serial data adds
    // the registered-callback storage (CONFIG_UART_INTERRUPT_DRIVEN) and the
    // cached uart_config (CONFIG_UART_USE_RUNTIME_CONFIGURE) to the embedded
    // spec driver + the retained lifecycle state word.
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
        "\tstruct {inner_tag} inner;\n\tuint32_t __rseq_state; /* retained; zero in the static image */\n\tatomic_t rx_err; /* accumulated inline RxError bits (atomic: ISR fifo_read vs thread err_check), surfaced + read-cleared by err_check (F-Z3) */\n#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n\tuart_irq_callback_user_data_t cb;\n\tvoid *cb_data;\n\tbool tx_irq_on; /* the TX arm gate for irq_tx_ready (the IMSC.TXIM analog) */\n\tbool rx_irq_on; /* the RX arm gate for irq_rx_ready (the IMSC.RXIM analog) */\n#endif\n#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\n\tstruct uart_config uart_cfg; /* cached for config_get (no HW readback, I03) */\n#endif\n"
    ));
    tu.push_str("};\n\n");
    if has_region {
        tu.push_str(&format!(
            "/* the TU-static base every MMIO op reads (A-2; paired with the\n * single-instance BUILD_ASSERT below) */\nstatic mm_reg_t {base_symbol};\n\n"
        ));
    }

    // §1 items 5-7: lowered helper/callback bodies, the uart_driver_api slot
    // bodies + ISR, the vtable, then the init/deinit dispatch hubs.
    tu.push_str(bodies);
    tu.push_str(&api.fns);
    tu.push_str(&api.vtable);
    tu.push_str(init_fn);
    tu.push('\n');
    tu.push_str(deinit_fn);
    tu.push('\n');

    // §1 item 8: the build-time instantiation block — the api arg is now
    // `&<drv>_uart_api` (the delivered serial class), NOT the deferred-class
    // NULL; the on_deinit form uses DEVICE_DT_INST_DEINIT_DEFINE.
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
        "\tDEVICE_DT_INST_DEINIT_DEFINE(n, {drv}_init, {drv}_deinit, NULL,\\\n\t\t&{drv}_data_##n, &{drv}_config_##n,\\\n\t\tPOST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE,\\\n\t\t&{drv}_uart_api);\n"
    ));
    tu.push_str("DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)\n\n");

    // §1 item 9: the mandatory single-instance guard (A-2/D18).
    tu.push_str(
        "BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1,\n\t     \"TU-static MMIO base requires a single status-okay instance\");\n",
    );

    // The A-1 cross-check (ratified D11): DT `reg` agrees with the L2 literals.
    if let Some((base, size)) = resolved_region {
        tu.push_str(&format!(
            "\n/* A-1/D11: the DT `reg` the map consumes must agree with the L2\n * platform literals the spec was resolved against */\nBUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == {base:#x} &&\n\t     DT_REG_SIZE(DT_DRV_INST(0)) == {size:#x},\n\t     \"DT reg disagrees with the resolved [mmio] platform literals (D11)\");\n"
        ));
    }

    Ok(tu)
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

/// The C name of a struct's lowest-offset field (RxDatum leading `.unit`).
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

/// The tag value of the variant at canonical semantic position `ord` of binding
/// `b`, for a tag-byte equality check.
fn enum_variant_tag(
    model: &RegseqModel,
    enum_ty: TyId,
    binding: &Option<EnumBinding>,
    ord: usize,
) -> Result<u64, EmitError> {
    let variant = binding
        .as_ref()
        .and_then(|e| e.variants.get(ord).copied())
        .ok_or_else(|| unsup("serial support enum variant map missing"))?;
    let layout =
        types::layout_of(model, enum_ty).ok_or_else(|| unsup("enum tag of non-aggregate"))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(unsup("enum tag of an unsupported enum shape"));
    };
    let tag = variants
        .get(variant.as_u32() as usize)
        .and_then(|v| v.tag_value)
        .ok_or_else(|| unsup("enum variant is untagged"))?;
    if tag > u64::MAX as u128 {
        return Err(unsup("128-bit enum tag value"));
    }
    Ok(tag as u64)
}
