//! Locked serial-port to Linux serial-core binding.
//!
//! The serial-class construct owns both the `uart_ops` binding and serial-core
//! publication.  The endpoint name is derived from the generated driver
//! identity: driver `foo` publishes line zero as `ttyfoo0`.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel, Ty, TyId};

use super::body::{local_name, LowerCx};
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
    if serial.hardware_ops.break_send_enabled {
        let id = resolved(model, &serial.hardware_ops.break_send)?.ok_or_else(|| {
            EmitError::Driver("enabled serial-hardware-ops.break-send hook does not resolve".into())
        })?;
        out.push(("serial_break_send", id));
    }
    Ok(out)
}

fn resolved(
    model: &RegseqModel,
    op: &Option<InstanceRef>,
) -> Result<Option<InstanceId>, EmitError> {
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
    match model.serial_port.drivers.get(&driver) {
        Some(serial) if serial.ops.set_signals.is_some() => {
            "\tstruct uart_port __rseq_uart_port;\n\tunsigned int __rseq_mctrl;\n"
        }
        Some(_) => "\tstruct uart_port __rseq_uart_port;\n",
        None => "",
    }
}

pub(super) fn container_globals(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    model
        .serial_port
        .drivers
        .contains_key(&driver)
        .then(|| format!("static struct uart_port *{drv}_serial_event_port;\n\n"))
        .unwrap_or_default()
}

pub(super) fn present(model: &RegseqModel, driver: ItemId) -> bool {
    model.serial_port.drivers.contains_key(&driver)
}

/// Initialization performed after the private object is associated.
pub(super) fn probe_init(
    model: &RegseqModel,
    driver: ItemId,
    drv: &str,
    mapped_field: Option<&str>,
) -> String {
    if let Some(serial) = model.serial_port.drivers.get(&driver) {
        let membase = mapped_field
            .map(|field| format!("\tpriv->__rseq_uart_port.membase = priv->inner.{field};\n"))
            .unwrap_or_default();
        let mctrl_seed = serial
            .ops
            .set_signals
            .is_some()
            .then_some(
                "\t/* Seed the glue shadow before authored initialization; successful\n\t * authored signal changes advance it in the lowered setter. */\n\tpriv->__rseq_mctrl = 0;\n",
            )
            .unwrap_or_default();
        format!(
            "\n\t/* Serial-class native state is complete before publication. */\n\tpriv->__rseq_uart_port.dev = &adev->dev;\n\tpriv->__rseq_uart_port.mapbase = adev->res.start;\n{membase}\tpriv->__rseq_uart_port.irq = adev->irq[0];\n\tpriv->__rseq_uart_port.iotype = UPIO_MEM32;\n\tpriv->__rseq_uart_port.type = PORT_AMBA;\n\tpriv->__rseq_uart_port.fifosize = 16;\n\tpriv->__rseq_uart_port.flags = UPF_BOOT_AUTOCONF | UPF_FIXED_PORT | UPF_FIXED_TYPE;\n\tpriv->__rseq_uart_port.line = 0;\n\tpriv->__rseq_uart_port.uartclk = 24000000;\n\tpriv->__rseq_uart_port.private_data = priv;\n\tpriv->__rseq_uart_port.ops = &{drv}_uart_ops;\n{mctrl_seed}"
        )
    } else {
        String::new()
    }
}

/// Publish only after authored initialization has succeeded.  A failed add is
/// still a failed probe, so the caller supplies the reverse-order native and
/// authored cleanup that must precede the return.
pub(super) fn probe_publish(
    model: &RegseqModel,
    driver: ItemId,
    drv: &str,
    failure_cleanup: &str,
) -> String {
    let Some(_) = model.serial_port.drivers.get(&driver) else {
        return String::new();
    };
    format!(
        "\n\t/* Serial endpoint publication is the visibility boundary. */\n\tret = uart_add_one_port(&{drv}_uart_driver, &priv->__rseq_uart_port);\n\tif (ret) {{\n{failure_cleanup}\t\treturn ret;\n\t}}\n\t{drv}_serial_event_port = &priv->__rseq_uart_port;\n",
        failure_cleanup = failure_cleanup.replace('\t', "\t\t"),
    )
}

pub(super) fn set_signals_shadow_update(
    cx: &mut LowerCx<'_>,
    instance: InstanceId,
    body: &regseq_model::Body,
) -> Result<Option<String>, EmitError> {
    let Some(serial) = cx.model.serial_port.drivers.values().next() else {
        return Ok(None);
    };
    let Some(set_signals) = resolved(cx.model, &serial.ops.set_signals)? else {
        return Ok(None);
    };
    if set_signals != instance {
        return Ok(None);
    }
    let mut args = body.arg_locals();
    let receiver = args
        .next()
        .ok_or_else(|| EmitError::Driver("serial set_signals receiver is missing".into()))?;
    let signals = args
        .next()
        .ok_or_else(|| EmitError::Driver("serial set_signals value is missing".into()))?;
    cx.extra_includes.insert("<linux/container_of.h>");
    Ok(Some(format!(
        "\tif (_0._raw[0] == 0)\n\t\tcontainer_of({}, struct {}_priv, inner)->__rseq_mctrl =\n\t\t\t({}.dtr ? TIOCM_DTR : 0) | ({}.rts ? TIOCM_RTS : 0);\n",
        local_name(cx.model, body, receiver),
        cx.driver.drv,
        local_name(cx.model, body, signals),
        local_name(cx.model, body, signals),
    )))
}

pub(super) fn remove(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    present(model, driver)
        .then(|| {
            format!(
                "\t/* Withdraw the tty endpoint before authored hardware teardown. */\n\tuart_remove_one_port(&{drv}_uart_driver, &priv->__rseq_uart_port);\n\t{drv}_serial_event_port = NULL;\n"
            )
        })
        .unwrap_or_default()
}

/// Emit the Linux native binding after author bodies and before lifecycle
/// callbacks. The direct transfer functions are the queue-neutral seam used by
/// an L2 tty/console adapter: they never claim that acceptance is completion.
pub(super) fn emit(cx: &mut LowerCx<'_>, driver: ItemId, drv: &str) -> Result<String, EmitError> {
    let Some(serial) = cx.model.serial_port.drivers.get(&driver) else {
        return Ok(String::new());
    };
    cx.extra_includes.insert("<linux/serial_core.h>");
    cx.extra_includes.insert("<linux/tty.h>");
    cx.extra_includes.insert("<linux/tty_flip.h>");
    cx.extra_includes.insert("<linux/kfifo.h>");
    cx.extra_includes.insert("<linux/irqnr.h>");

    let transmit = op(cx, &serial.ops.transmit)?;
    let receive = op(cx, &serial.ops.receive)?;
    let configure = op(cx, &serial.ops.configure)?;
    let get_config = op(cx, &serial.ops.get_config)?;
    let set_signals = op(cx, &serial.ops.set_signals)?;
    let get_signals = op(cx, &serial.ops.get_signals)?;
    let open = op(cx, &serial.ops.open)?;
    let close = op(cx, &serial.ops.close)?;
    let purge = op(cx, &serial.ops.purge)?;
    let await_drain = op(cx, &serial.ops.await_drain)?;
    let break_send = serial
        .hardware_ops
        .break_send_enabled
        .then(|| op(cx, &serial.hardware_ops.break_send))
        .transpose()?
        .flatten();

    let mut out =
        "/* Serial-class Linux binding: authored operations plus the minimal\n * serial-core queue and publication realization. */\n"
            .to_string();

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
            "static void {drv}_uart_set_termios(struct uart_port *port, struct ktermios *termios,\n\t\t\t\t    const struct ktermios *old)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{params} params = {{ .baud_rate = uart_get_baud_rate(port, termios, old, 0,\n\t\t\t\t\t\t       port->uartclk / 16) }};\n\t{status} status;\n\n\tswitch (termios->c_cflag & CSIZE) {{\n\tcase CS5: params.data_bits._raw[0] = 0; break;\n\tcase CS6: params.data_bits._raw[0] = 1; break;\n\tcase CS7: params.data_bits._raw[0] = 2; break;\n\tcase CS8: params.data_bits._raw[0] = 3; break;\n\tdefault:\n\t\tif (old) *termios = *old;\n\t\treturn;\n\t}}\n\tparams.stop_bits._raw[0] = (termios->c_cflag & CSTOPB) ? 3 : 1;\n#ifdef CMSPAR\n\tif ((termios->c_cflag & PARENB) && (termios->c_cflag & CMSPAR))\n\t\tparams.parity._raw[0] = (termios->c_cflag & PARODD) ? 3 : 4;\n\telse\n#endif\n\tif (!(termios->c_cflag & PARENB))\n\t\tparams.parity._raw[0] = 0;\n\telse\n\t\tparams.parity._raw[0] = (termios->c_cflag & PARODD) ? 1 : 2;\n\tparams.flow_control_mode._raw[0] =\n\t\t(termios->c_cflag & CRTSCTS) ? 1 : 0;\n\tstatus = {sym}(&priv->inner, params);\n\t/* uart_ops.set_termios cannot return an errno. Preserve unsupported or\n\t * invalid configuration by restoring the prior applied snapshot when\n\t * serial-core supplies one (resume may pass NULL). */\n\tif (status._raw[0] != 0) {{\n\t\tif (old) *termios = *old;\n\t\treturn;\n\t}}\n\tuart_update_timeout(port, termios->c_cflag, params.baud_rate);\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_set_termios(struct uart_port *port, struct ktermios *termios,\n\t\t\t\t    const struct ktermios *old)\n{{\n\t(void)port;\n\t/* No authored configure capability: preserve the prior serial-core\n\t * snapshot when one exists; no device configuration is synthesized. */\n\tif (old)\n\t\t*termios = *old;\n}}\n\n"
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
    } else {
        out.push_str(&format!(
            "static int {drv}_uart_startup(struct uart_port *port)\n{{\n\t(void)port;\n\t/* No authored session transition is required; serial-core still requires\n\t * a startup callback for a published port. */\n\treturn 0;\n}}\n\n"
        ));
    }
    if let Some(op) = &close {
        out.push_str(&format!(
            "static void {drv}_uart_shutdown(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\t(void){sym}(&priv->inner);\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_shutdown(struct uart_port *port)\n{{\n\t(void)port;\n\t/* No authored session transition is required. */\n}}\n\n"
        ));
    }
    if let Some(op) = &await_drain {
        out.push_str(&format!(
            "static unsigned int {drv}_uart_tx_empty(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\treturn {sym}(&priv->inner) ? TIOCSER_TEMT : 0;\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static unsigned int {drv}_uart_tx_empty(struct uart_port *port)\n{{\n\t(void)port;\n\t/* serial-core documents TIOCSER_TEMT as the fallback when transmit\n\t * emptiness cannot be observed by the authored surface. */\n\treturn TIOCSER_TEMT;\n}}\n\n"
        ));
    }
    if let Some(op) = &set_signals {
        let signals = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let status = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static void {drv}_uart_set_mctrl(struct uart_port *port, unsigned int mctrl)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\tunsigned int driven = mctrl & (TIOCM_DTR | TIOCM_RTS);\n\t{signals} signals;\n\t{status} status;\n\n\t/* serial-core may repeat the applied state during registration/resume. */\n\tif (driven == priv->__rseq_mctrl)\n\t\treturn;\n\tsignals = ({signals}){{\n\t\t.dtr = !!(driven & TIOCM_DTR), .rts = !!(driven & TIOCM_RTS),\n\t}};\n\tstatus = {sym}(&priv->inner, signals);\n\tif (status._raw[0] == 0)\n\t\tpriv->__rseq_mctrl = driven;\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_set_mctrl(struct uart_port *port, unsigned int mctrl)\n{{\n\t(void)port;\n\t(void)mctrl;\n\t/* No authored output-signal capability: do not invent DTR/RTS writes. */\n}}\n\n"
        ));
    }
    if let Some(op) = &get_signals {
        let signals = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "static unsigned int {drv}_uart_get_mctrl(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{signals} signals = {sym}(&priv->inner);\n\tunsigned int value = 0;\n\n\tif (signals.cts) value |= TIOCM_CTS;\n\tif (signals.dsr) value |= TIOCM_DSR;\n\tif (signals.dcd) value |= TIOCM_CAR;\n\tif (signals.ri) value |= TIOCM_RNG;\n\treturn value;\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static unsigned int {drv}_uart_get_mctrl(struct uart_port *port)\n{{\n\t(void)port;\n\t/* serial-core requires unsupported CTS/DCD/DSR permanently active;\n\t * RI remains inactive. */\n\treturn TIOCM_CTS | TIOCM_DSR | TIOCM_CAR;\n}}\n\n"
        ));
    }

    if let Some(op) = &purge {
        let direction = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        out.push_str(&format!(
            "static void {drv}_uart_flush_buffer(struct uart_port *port)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\t{direction} direction = {{ 0 }};\n\n\t/* serial-core flush_buffer is transmit-only; the authored direction\n\t * enum is bridge-validated as Receive, Transmit, Both. */\n\tdirection._raw[0] = 1;\n\t(void){sym}(&priv->inner, direction);\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_flush_buffer(struct uart_port *port)\n{{\n\t(void)port;\n\t/* No authored purge capability: serial-core has already discarded its\n\t * transmit queue; no device-side purge is synthesized. */\n}}\n\n"
        ));
    }

    if transmit.is_some() {
        out.push_str(&format!(
            "static void {drv}_uart_start_tx(struct uart_port *port)\n{{\n\tstruct tty_port *tport = &port->state->port;\n\tunsigned char ch;\n\n\twhile (kfifo_peek(&tport->xmit_fifo, &ch)) {{\n\t\tif ({drv}_serial_transmit_bytes(port, &ch, 1) != 1)\n\t\t\tbreak;\n\t\tuart_xmit_advance(port, 1);\n\t}}\n\tif (kfifo_len(&tport->xmit_fifo) < WAKEUP_CHARS)\n\t\tuart_write_wakeup(port);\n}}\n\n"
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_start_tx(struct uart_port *port)\n{{\n\t(void)port;\n\t/* No authored transmit capability: leave the serial-core queue intact. */\n}}\n\n"
        ));
    }
    out.push_str(&format!(
        "static void {drv}_uart_stop_tx(struct uart_port *port)\n{{\n\t(void)port;\n\t/* The authored surface has no pause operation; abort is a distinct,\n\t * outcome-bearing capability and must not be substituted. */\n}}\n\nstatic void {drv}_uart_stop_rx(struct uart_port *port)\n{{\n\t(void)port;\n\t/* The authored surface has no receive-pause operation. */\n}}\n\n"
    ));
    if let Some(op) = &break_send {
        out.push_str(&format!(
            "static void {drv}_uart_break_ctl(struct uart_port *port, int ctl)\n{{\n\tstruct {drv}_priv *priv = port->private_data;\n\n\t{sym}(&priv->inner, ctl != 0);\n}}\n\n",
            sym = op.symbol,
        ));
    } else {
        out.push_str(&format!(
            "static void {drv}_uart_break_ctl(struct uart_port *port, int ctl)\n{{\n\t(void)port;\n\t(void)ctl;\n\t/* serial-hardware-ops.break-send is not enabled for this platform. */\n}}\n\n"
        ));
    }
    out.push_str(&format!(
        "static void {drv}_uart_release_port(struct uart_port *port)\n{{\n\t(void)port;\n\t/* devm owns the already-claimed AMBA region and releases it after\n\t * serial-core withdrawal. */\n}}\n\nstatic int {drv}_uart_request_port(struct uart_port *port)\n{{\n\t(void)port;\n\t/* devm owns the already-claimed AMBA region; there is no second request. */\n\treturn 0;\n}}\n\n"
    ));
    out.push_str(&format!(
        "static const char *{drv}_uart_type(struct uart_port *port)\n{{\n\treturn port->type == PORT_AMBA ? \"{drv}\" : NULL;\n}}\n\nstatic void {drv}_uart_config_port(struct uart_port *port, int flags)\n{{\n\t/* AMBA matching and the authored fixed register model establish the\n\t * device type; no probing or resource behavior is invented here. */\n\tif (flags & UART_CONFIG_TYPE)\n\t\tport->type = PORT_AMBA;\n}}\n\nstatic int {drv}_uart_verify_port(struct uart_port *port, struct serial_struct *serial)\n{{\n\tif (serial->type != PORT_UNKNOWN && serial->type != PORT_AMBA)\n\t\treturn -EINVAL;\n\tif (serial->irq < 0 || serial->irq >= irq_get_nr_irqs())\n\t\treturn -EINVAL;\n\tif (serial->baud_base < 9600)\n\t\treturn -EINVAL;\n\tif (port->mapbase != (unsigned long)serial->iomem_base)\n\t\treturn -EINVAL;\n\treturn 0;\n}}\n\n"
    ));

    out.push_str(&format!(
        "/* Every callback serial_core invokes without a NULL guard is present.\n * Guarded resource and break hooks are also explicit because this is a fixed,\n * devm-managed AMBA port. Optional hooks with documented core fallbacks remain\n * NULL. */\nstatic const struct uart_ops {drv}_uart_ops = {{\n\t.tx_empty = {drv}_uart_tx_empty,\n\t.set_mctrl = {drv}_uart_set_mctrl,\n\t.get_mctrl = {drv}_uart_get_mctrl,\n\t.stop_tx = {drv}_uart_stop_tx,\n\t.start_tx = {drv}_uart_start_tx,\n\t.stop_rx = {drv}_uart_stop_rx,\n\t.break_ctl = {drv}_uart_break_ctl,\n\t.startup = {drv}_uart_startup,\n\t.shutdown = {drv}_uart_shutdown,\n\t.flush_buffer = {drv}_uart_flush_buffer,\n\t.set_termios = {drv}_uart_set_termios,\n\t.type = {drv}_uart_type,\n\t.release_port = {drv}_uart_release_port,\n\t.request_port = {drv}_uart_request_port,\n\t.config_port = {drv}_uart_config_port,\n\t.verify_port = {drv}_uart_verify_port,\n}};\n\nstatic struct uart_driver {drv}_uart_driver = {{\n\t.owner = THIS_MODULE,\n\t.driver_name = \"{drv}\",\n\t.dev_name = \"tty{drv}\",\n\t.nr = 1,\n}};\n\n"
    ));
    for capability in serial.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-SERIAL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Absent authored capabilities use only the serial-core contract fallbacks documented above; no completion, abort outcome, or device behavior is synthesized. */\n\n");
    Ok(out)
}

#[derive(Clone, Copy)]
enum EventShim {
    Event,
    Signals,
}

fn event_shim(model: &RegseqModel, instance: InstanceId) -> Option<EventShim> {
    let item = model.instances[instance].item;
    if model.items[item].def_path.crate_id != model.primary_crate {
        return None;
    }
    let path = model.strings.resolve(model.items[item].def_path.rendered);
    if path.ends_with("serial::raise") {
        Some(EventShim::Event)
    } else if path.ends_with("serial::raise_signals") {
        Some(EventShim::Signals)
    } else {
        None
    }
}

pub(super) fn emit_event_shim(
    cx: &mut LowerCx<'_>,
    instance: InstanceId,
    head: &str,
) -> Result<Option<String>, EmitError> {
    let Some(kind) = event_shim(cx.model, instance) else {
        return Ok(None);
    };
    let Some(serial) = cx.model.serial_port.drivers.values().next() else {
        return Ok(None);
    };
    let drv = cx.driver.drv.clone();
    let body = match kind {
        EventShim::Event => {
            let receive = op(cx, &serial.ops.receive)?.ok_or_else(|| {
                EmitError::Driver("RxDataReady is raised without an authored receive hook".into())
            })?;
            let buffer = types::c_type(cx.model, &mut cx.names, receive.inputs[1])?;
            let errors = types::c_type(
                cx.model,
                &mut cx.names,
                pointee_field(cx.model, receive.inputs[1], 2)?,
            )?;
            format!(
                "{head}\n{{\n\tstruct uart_port *port = {drv}_serial_event_port;\n\tstruct {drv}_priv *priv;\n\tu8 ch;\n\tchar flag;\n\t{errors} errors;\n\t{buffer} buffer;\n\n\tif (!port || !port->state)\n\t\treturn;\n\tpriv = port->private_data;\n\tswitch (_1._raw[0]) {{\n\tcase 0: /* RxDataReady */\n\t\tfor (;;) {{\n\t\t\terrors = ({errors}){{0}};\n\t\t\tbuffer = ({buffer}){{ .data = &ch, .len = 1, .errors = &errors }};\n\t\t\tif ({receive_sym}(&priv->inner, buffer) != 1)\n\t\t\t\tbreak;\n\t\t\tflag = TTY_NORMAL;\n\t\t\tport->icount.rx++;\n\t\t\tif (errors.break_detected) {{\n\t\t\t\tport->icount.brk++;\n\t\t\t\tif (uart_handle_break(port))\n\t\t\t\t\tcontinue;\n\t\t\t\tflag = TTY_BREAK;\n\t\t\t}} else if (errors.framing) {{\n\t\t\t\tport->icount.frame++;\n\t\t\t\tflag = TTY_FRAME;\n\t\t\t}} else if (errors.parity) {{\n\t\t\t\tport->icount.parity++;\n\t\t\t\tflag = TTY_PARITY;\n\t\t\t}}\n\t\t\tif (errors.overrun)\n\t\t\t\tport->icount.overrun++;\n\t\t\tuart_insert_char(port, errors.overrun, true, ch, flag);\n\t\t}}\n\t\ttty_flip_buffer_push(&port->state->port);\n\t\tbreak;\n\tcase 1: /* TxProgressOrIdle */\n\t\tuart_write_wakeup(port);\n\t\tbreak;\n\tdefault:\n\t\tbreak;\n\t}}\n}}\n",
                receive_sym = receive.symbol,
            )
        }
        EventShim::Signals => format!(
            "{head}\n{{\n\tstruct uart_port *port = {drv}_serial_event_port;\n\tstruct ModemSignalSet signals = _1;\n\n\tif (!port || !port->state)\n\t\treturn;\n\tif (signals.changed_cts)\n\t\tuart_handle_cts_change(port, signals.cts);\n\tif (signals.changed_dcd)\n\t\tuart_handle_dcd_change(port, signals.dcd);\n\tif (signals.changed_dsr)\n\t\tport->icount.dsr++;\n\tif (signals.changed_ri)\n\t\tport->icount.rng++;\n\tif (signals.changed_cts || signals.changed_dsr || signals.changed_dcd || signals.changed_ri)\n\t\twake_up_interruptible(&port->state->port.delta_msr_wait);\n}}\n"
        ),
    };
    Ok(Some(body))
}
