//! Locked serial-port to Linux serial-core binding.
//!
//! Endpoint discovery, registration, buffering policy, IRQ ownership, and
//! register access remain outside the L1 table. This module exposes the
//! authored capabilities through a `uart_ops`-shaped binding plus explicit
//! pointer+length transfer adapters; missing operations produce no slot.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel, Ty, TyId};

use super::body::LowerCx;
use super::types;
use super::{resolve_instance_ref, EmitError};

pub(super) fn roots(
    model: &RegseqModel,
    driver: ItemId,
) -> Result<Vec<(&'static str, InstanceId)>, EmitError> {
    let Some(serial) = model.serial_port.drivers.get(&driver) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (name, op) in [
        ("serial_configure", &serial.ops.configure),
        ("serial_get_config", &serial.ops.get_config),
        ("serial_transmit", &serial.ops.transmit),
        ("serial_receive", &serial.ops.receive),
        ("serial_set_signals", &serial.ops.set_signals),
        ("serial_get_signals", &serial.ops.get_signals),
        ("serial_subscribe_events", &serial.ops.subscribe_events),
        ("serial_open", &serial.ops.open),
        ("serial_close", &serial.ops.close),
        ("serial_purge", &serial.ops.purge),
        ("serial_await_drain", &serial.ops.await_drain),
        ("serial_abort", &serial.ops.abort),
    ] {
        if let Some(id) = resolved(model, op)? {
            out.push((name, id));
        }
    }
    Ok(out)
}

fn resolved(model: &RegseqModel, op: &Option<InstanceRef>) -> Result<Option<InstanceId>, EmitError> {
    op.map(|op| {
        resolve_instance_ref(model, op).ok_or_else(|| {
            EmitError::Driver("serial-port operation does not resolve to an instance".into())
        })
    })
    .transpose()
}

struct Op {
    symbol: String,
    inputs: Vec<TyId>,
    output: TyId,
}

fn op(cx: &LowerCx<'_>, value: &Option<InstanceRef>) -> Result<Option<Op>, EmitError> {
    let Some(id) = resolved(cx.model, value)? else {
        return Ok(None);
    };
    let regseq_model::InstanceKind::Function(data) = &cx.model.instances[id].kind else {
        return Err(EmitError::Driver(
            "serial-port operation resolved to a non-function instance".into(),
        ));
    };
    let sig = cx.model.sigs.resolve(data.fn_sig);
    Ok(Some(Op {
        symbol: cx.fn_symbols[&id].clone(),
        inputs: sig.inputs.clone(),
        output: sig.output,
    }))
}

fn pointee_field(model: &RegseqModel, aggregate: TyId, index: usize) -> Result<TyId, EmitError> {
    let Ty::Struct { layout, .. } = &model.types[aggregate] else {
        return Err(EmitError::Unsupported(
            "serial buffer is not a struct in the lowered model".into(),
        ));
    };
    let regseq_model::LayoutKind::Struct { fields } = &layout.kind else {
        return Err(EmitError::Unsupported(
            "serial buffer has no struct layout".into(),
        ));
    };
    let field = fields
        .get(index)
        .ok_or_else(|| EmitError::Unsupported("serial buffer field is missing".into()))?;
    match &model.types[field.ty] {
        Ty::Ptr(pointee, _) => Ok(*pointee),
        _ => Err(EmitError::Unsupported(
            "serial buffer field is not a pointer".into(),
        )),
    }
}

/// Extra member placed in the existing per-device container.
pub(super) fn container_field(model: &RegseqModel, driver: ItemId) -> &'static str {
    if model.serial_port.drivers.contains_key(&driver) {
        "\tstruct uart_port __rseq_uart_port;\n"
    } else {
        ""
    }
}

/// Initialization performed after the private object is associated.
pub(super) fn probe_init(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if model.serial_port.drivers.contains_key(&driver) {
        format!(
            "\n\t/* serial-framework-binding: endpoint registration remains L2; bind the\n\t * generated uart_ops and private state without inventing a port identity */\n\tpriv->__rseq_uart_port.private_data = priv;\n\tpriv->__rseq_uart_port.ops = &{drv}_uart_ops;\n"
        )
    } else {
        String::new()
    }
}

/// Emit the Linux native binding after author bodies and before lifecycle
/// callbacks. The direct transfer functions are the queue-neutral seam used by
/// an L2 tty/console adapter: they never claim that acceptance is completion.
pub(super) fn emit(
    cx: &mut LowerCx<'_>,
    driver: ItemId,
    drv: &str,
) -> Result<String, EmitError> {
    let Some(serial) = cx.model.serial_port.drivers.get(&driver) else {
        return Ok(String::new());
    };
    cx.extra_includes.insert("<linux/serial_core.h>");
    cx.extra_includes.insert("<linux/tty.h>");

    let transmit = op(cx, &serial.ops.transmit)?;
    let receive = op(cx, &serial.ops.receive)?;
    let configure = op(cx, &serial.ops.configure)?;
    let get_config = op(cx, &serial.ops.get_config)?;
    let set_signals = op(cx, &serial.ops.set_signals)?;
    let get_signals = op(cx, &serial.ops.get_signals)?;
    let open = op(cx, &serial.ops.open)?;
    let close = op(cx, &serial.ops.close)?;
    let await_drain = op(cx, &serial.ops.await_drain)?;

    let mut out = format!(
        "/* locked serial-port Linux binding: unsupported capabilities have no slot;\n * endpoint/queue/IRQ ownership remains in serial-framework-binding (L2). */\n"
    );

    if let Some(op) = &transmit {
        let buffer = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        out.push_str(&format!(
            "static __maybe_unused size_t {drv}_serial_transmit_bytes(struct uart_port *port, const u8 *data, size_t len)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{buffer} buffer = {{ .data = (u8 *)data, .len = len }};\n\n\treturn {sym}(&priv->inner, buffer);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &receive {
        let buffer = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let errors = types::c_type(
            cx.model,
            &mut cx.names,
            pointee_field(cx.model, op.inputs[1], 2)?,
        )?;
        out.push_str(&format!(
            "static __maybe_unused size_t {drv}_serial_receive_bytes(struct uart_port *port, u8 *data, size_t len, {errors} *errors)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{buffer} buffer = {{ .data = data, .len = len, .errors = errors }};\n\n\treturn {sym}(&priv->inner, buffer);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &configure {
        let params = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let status = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static void {drv}_uart_set_termios(struct uart_port *port, struct ktermios *termios,\n\t\t\t\t    const struct ktermios *old)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{params} params = {{ .baud_rate = uart_get_baud_rate(port, termios, old, 0,\n\t\t\t\t\t\t       port->uartclk / 16) }};\n\t{status} status;\n\n\tswitch (termios->c_cflag & CSIZE) {{\n\tcase CS5: params.data_bits._raw[0] = 0; break;\n\tcase CS6: params.data_bits._raw[0] = 1; break;\n\tcase CS7: params.data_bits._raw[0] = 2; break;\n\tcase CS8: params.data_bits._raw[0] = 3; break;\n\tdefault: *termios = *old; return;\n\t}}\n\tparams.stop_bits._raw[0] = (termios->c_cflag & CSTOPB) ? 3 : 1;\n#ifdef CMSPAR\n\tif ((termios->c_cflag & PARENB) && (termios->c_cflag & CMSPAR))\n\t\tparams.parity._raw[0] = (termios->c_cflag & PARODD) ? 3 : 4;\n\telse\n#endif\n\tif (!(termios->c_cflag & PARENB))\n\t\tparams.parity._raw[0] = 0;\n\telse\n\t\tparams.parity._raw[0] = (termios->c_cflag & PARODD) ? 1 : 2;\n\tparams.flow_control_mode._raw[0] =\n\t\t(termios->c_cflag & CRTSCTS) ? 1 : 0;\n\tstatus = {sym}(&priv->inner, params);\n\t/* uart_ops.set_termios cannot return an errno. Preserve unsupported or\n\t * invalid configuration by restoring the prior applied snapshot. */\n\tif (status._raw[0] != 0) {{\n\t\t*termios = *old;\n\t\treturn;\n\t}}\n\tuart_update_timeout(port, termios->c_cflag, params.baud_rate);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &get_config {
        let params = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static __maybe_unused {params} {drv}_serial_get_config_snapshot(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\treturn {sym}(&priv->inner);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &open {
        let status = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static int {drv}_uart_startup(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{status} status = {sym}(&priv->inner);\n\n\t/* Semantic variant order is bridge-validated: Ok, Unsupported, Busy,\n\t * NoActiveTransfer, InvalidParameter, BufferTooSmall. */\n\tswitch (status._raw[0]) {{\n\tcase 0: return 0;\n\tcase 1: return -EOPNOTSUPP;\n\tcase 2: return -EBUSY;\n\tcase 3: return -EFAULT;\n\tcase 4: return -EINVAL;\n\tcase 5: return -ENOSPC;\n\tdefault: return -EINVAL;\n\t}}\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &close {
        out.push_str(&format!(
            "static void {drv}_uart_shutdown(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\t(void){sym}(&priv->inner);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &await_drain {
        out.push_str(&format!(
            "static unsigned int {drv}_uart_tx_empty(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\treturn {sym}(&priv->inner) ? TIOCSER_TEMT : 0;\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &set_signals {
        let signals = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        out.push_str(&format!(
            "static void {drv}_uart_set_mctrl(struct uart_port *port, unsigned int mctrl)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{signals} signals = {{\n\t\t.dtr = !!(mctrl & TIOCM_DTR), .rts = !!(mctrl & TIOCM_RTS),\n\t}};\n\n\t(void){sym}(&priv->inner, signals);\n}}\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &get_signals {
        let signals = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static unsigned int {drv}_uart_get_mctrl(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{signals} signals = {sym}(&priv->inner);\n\tunsigned int value = 0;\n\n\tif (signals.cts) value |= TIOCM_CTS;\n\tif (signals.dsr) value |= TIOCM_DSR;\n\tif (signals.dcd) value |= TIOCM_CAR;\n\tif (signals.ri) value |= TIOCM_RNG;\n\treturn value;\n}}\n\n",
            sym = op.symbol,
        ));
    }

    out.push_str(&format!("static const struct uart_ops {drv}_uart_ops = {{\n"));
    if open.is_some() {
        out.push_str(&format!("\t.startup = {drv}_uart_startup,\n"));
    }
    if close.is_some() {
        out.push_str(&format!("\t.shutdown = {drv}_uart_shutdown,\n"));
    }
    if await_drain.is_some() {
        out.push_str(&format!("\t.tx_empty = {drv}_uart_tx_empty,\n"));
    }
    if set_signals.is_some() {
        out.push_str(&format!("\t.set_mctrl = {drv}_uart_set_mctrl,\n"));
    }
    if get_signals.is_some() {
        out.push_str(&format!("\t.get_mctrl = {drv}_uart_get_mctrl,\n"));
    }
    if configure.is_some() {
        out.push_str(&format!("\t.set_termios = {drv}_uart_set_termios,\n"));
    }
    out.push_str(
        "\t/* transmit/receive are pointer+length adapters above; L2 owns tty queueing. */\n};\n\n",
    );
    for capability in serial.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-SERIAL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* No completion, abort outcome, signal sensing, or drain result is synthesized for an absent capability. */\n\n");
    Ok(out)
}
