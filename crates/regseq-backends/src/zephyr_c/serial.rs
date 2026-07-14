//! Locked serial-port to Zephyr `uart_driver_api` binding.
//!
//! Polling and FIFO callbacks adapt Zephyr buffers to the portable
//! pointer+length values and retain the returned progress count. Optional
//! runtime configuration is emitted only when authored and only under the
//! matching Zephyr Kconfig surface. No callback or terminal event is invented.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel, Ty, TyId};

use super::body::LowerCx;
use super::types;
use super::{resolve_instance_ref, EmitError};

pub(super) struct Emission {
    pub data_fields: String,
    pub text: String,
    pub api: String,
}

impl Emission {
    pub fn absent() -> Self {
        Self {
            data_fields: String::new(),
            text: String::new(),
            api: "NULL /* api: ClassTag::Generic only (A-16) */".into(),
        }
    }
}

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

pub(super) fn emit(
    cx: &mut LowerCx<'_>,
    driver: ItemId,
    drv: &str,
) -> Result<Emission, EmitError> {
    let Some(serial) = cx.model.serial_port.drivers.get(&driver) else {
        return Ok(Emission::absent());
    };
    cx.extra_includes.insert("<errno.h>");
    cx.extra_includes.insert("<zephyr/drivers/uart.h>");

    let transmit = op(cx, &serial.ops.transmit)?;
    let receive = op(cx, &serial.ops.receive)?;
    let configure = op(cx, &serial.ops.configure)?;
    let get_config = op(cx, &serial.ops.get_config)?;
    let await_drain = op(cx, &serial.ops.await_drain)?;

    let mut data_fields = String::new();
    let mut out = String::from(
        "/* locked serial-port Zephyr binding: only authored capabilities receive\n * uart_driver_api slots; event delivery and endpoint identity remain L2. */\n",
    );

    let mut error_ty = None;
    if let Some(op) = &receive {
        let buffer = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let errors = types::c_type(
            cx.model,
            &mut cx.names,
            pointee_field(cx.model, op.inputs[1], 2)?,
        )?;
        error_ty = Some(errors.clone());
        data_fields.push_str(&format!("\t{errors} __rseq_line_errors;\n"));
        out.push_str(&format!(
            "static int {drv}_uart_poll_in(const struct device *dev, unsigned char *c)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{buffer} buffer = {{ .data = c, .len = 1, .errors = &data->__rseq_line_errors }};\n\n\treturn {sym}(&data->inner, buffer) == 1 ? 0 : -1;\n}}\n\n#ifdef CONFIG_UART_INTERRUPT_DRIVEN\nstatic int {drv}_uart_fifo_read(const struct device *dev, uint8_t *buf, const int size)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{buffer} buffer = {{ .data = buf, .len = size > 0 ? (size_t)size : 0, .errors = &data->__rseq_line_errors }};\n\n\treturn (int){sym}(&data->inner, buffer);\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &transmit {
        let buffer = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        out.push_str(&format!(
            "static void {drv}_uart_poll_out(const struct device *dev, unsigned char c)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{buffer} buffer = {{ .data = &c, .len = 1 }};\n\n\t(void){sym}(&data->inner, buffer);\n}}\n\n#ifdef CONFIG_UART_INTERRUPT_DRIVEN\nstatic int {drv}_uart_fifo_fill(const struct device *dev, const uint8_t *buf, int size)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{buffer} buffer = {{ .data = (uint8_t *)buf, .len = size > 0 ? (size_t)size : 0 }};\n\n\treturn (int){sym}(&data->inner, buffer);\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(errors) = &error_ty {
        out.push_str(&format!(
            "static int {drv}_uart_err_check(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{errors} errors = data->__rseq_line_errors;\n\tint result = 0;\n\n\tdata->__rseq_line_errors = ({errors}){{0}};\n\tif (errors.break_detected) result |= UART_BREAK;\n\tif (errors.framing) result |= UART_ERROR_FRAMING;\n\tif (errors.parity) result |= UART_ERROR_PARITY;\n\tif (errors.overrun) result |= UART_ERROR_OVERRUN;\n\treturn result;\n}}\n\n"
        ));
    }

    if let Some(op) = &configure {
        let params = types::c_type(cx.model, &mut cx.names, op.inputs[1])?;
        let status = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\nstatic int {drv}_uart_configure(const struct device *dev, const struct uart_config *cfg)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{params} params = {{ .baud_rate = cfg->baudrate }};\n\t{status} status;\n\n\tswitch (cfg->data_bits) {{\n\tcase UART_CFG_DATA_BITS_5: params.data_bits._raw[0] = 0; break;\n\tcase UART_CFG_DATA_BITS_6: params.data_bits._raw[0] = 1; break;\n\tcase UART_CFG_DATA_BITS_7: params.data_bits._raw[0] = 2; break;\n\tcase UART_CFG_DATA_BITS_8: params.data_bits._raw[0] = 3; break;\n\tcase UART_CFG_DATA_BITS_9: params.data_bits._raw[0] = 4; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (cfg->stop_bits) {{\n\tcase UART_CFG_STOP_BITS_0_5: params.stop_bits._raw[0] = 0; break;\n\tcase UART_CFG_STOP_BITS_1: params.stop_bits._raw[0] = 1; break;\n\tcase UART_CFG_STOP_BITS_1_5: params.stop_bits._raw[0] = 2; break;\n\tcase UART_CFG_STOP_BITS_2: params.stop_bits._raw[0] = 3; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (cfg->parity) {{\n\tcase UART_CFG_PARITY_NONE: params.parity._raw[0] = 0; break;\n\tcase UART_CFG_PARITY_ODD: params.parity._raw[0] = 1; break;\n\tcase UART_CFG_PARITY_EVEN: params.parity._raw[0] = 2; break;\n\tcase UART_CFG_PARITY_MARK: params.parity._raw[0] = 3; break;\n\tcase UART_CFG_PARITY_SPACE: params.parity._raw[0] = 4; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (cfg->flow_ctrl) {{\n\tcase UART_CFG_FLOW_CTRL_NONE: params.flow_control_mode._raw[0] = 0; break;\n\tcase UART_CFG_FLOW_CTRL_RTS_CTS: params.flow_control_mode._raw[0] = 1; break;\n\tcase UART_CFG_FLOW_CTRL_DTR_DSR: params.flow_control_mode._raw[0] = 2; break;\n\tcase UART_CFG_FLOW_CTRL_RS485: params.flow_control_mode._raw[0] = 3; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tstatus = {sym}(&data->inner, params);\n\tswitch (status._raw[0]) {{\n\tcase 0: return 0;\n\tcase 1: return -ENOTSUP;\n\tcase 2: return -EBUSY;\n\tcase 3: return -EFAULT;\n\tcase 4: return -EINVAL;\n\tcase 5: return -ENOMEM;\n\tdefault: return -EINVAL;\n\t}}\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &get_config {
        let params = types::c_type(cx.model, &mut cx.names, op.output)?;
        out.push_str(&format!(
            "#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\nstatic int {drv}_uart_config_get(const struct device *dev, struct uart_config *cfg)\n{{\n\tstruct {drv}_data *data = dev->data;\n\t{params} params = {sym}(&data->inner);\n\n\tcfg->baudrate = params.baud_rate;\n\tswitch (params.data_bits._raw[0]) {{\n\tcase 0: cfg->data_bits = UART_CFG_DATA_BITS_5; break;\n\tcase 1: cfg->data_bits = UART_CFG_DATA_BITS_6; break;\n\tcase 2: cfg->data_bits = UART_CFG_DATA_BITS_7; break;\n\tcase 3: cfg->data_bits = UART_CFG_DATA_BITS_8; break;\n\tcase 4: cfg->data_bits = UART_CFG_DATA_BITS_9; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (params.stop_bits._raw[0]) {{\n\tcase 0: cfg->stop_bits = UART_CFG_STOP_BITS_0_5; break;\n\tcase 1: cfg->stop_bits = UART_CFG_STOP_BITS_1; break;\n\tcase 2: cfg->stop_bits = UART_CFG_STOP_BITS_1_5; break;\n\tcase 3: cfg->stop_bits = UART_CFG_STOP_BITS_2; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (params.parity._raw[0]) {{\n\tcase 0: cfg->parity = UART_CFG_PARITY_NONE; break;\n\tcase 1: cfg->parity = UART_CFG_PARITY_ODD; break;\n\tcase 2: cfg->parity = UART_CFG_PARITY_EVEN; break;\n\tcase 3: cfg->parity = UART_CFG_PARITY_MARK; break;\n\tcase 4: cfg->parity = UART_CFG_PARITY_SPACE; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\tswitch (params.flow_control_mode._raw[0]) {{\n\tcase 0: cfg->flow_ctrl = UART_CFG_FLOW_CTRL_NONE; break;\n\tcase 1: cfg->flow_ctrl = UART_CFG_FLOW_CTRL_RTS_CTS; break;\n\tcase 2: cfg->flow_ctrl = UART_CFG_FLOW_CTRL_DTR_DSR; break;\n\tcase 3: cfg->flow_ctrl = UART_CFG_FLOW_CTRL_RS485; break;\n\tdefault: return -ENOTSUP;\n\t}}\n\treturn 0;\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }
    if let Some(op) = &await_drain {
        out.push_str(&format!(
            "#ifdef CONFIG_UART_INTERRUPT_DRIVEN\nstatic int {drv}_uart_irq_tx_complete(const struct device *dev)\n{{\n\tstruct {drv}_data *data = dev->data;\n\n\treturn {sym}(&data->inner);\n}}\n#endif\n\n",
            sym = op.symbol,
        ));
    }

    out.push_str(&format!("static DEVICE_API(uart, {drv}_uart_api) = {{\n"));
    if transmit.is_some() {
        out.push_str(&format!("\t.poll_out = {drv}_uart_poll_out,\n"));
    }
    if receive.is_some() {
        out.push_str(&format!("\t.poll_in = {drv}_uart_poll_in,\n\t.err_check = {drv}_uart_err_check,\n"));
    }
    if configure.is_some() || get_config.is_some() {
        out.push_str("#ifdef CONFIG_UART_USE_RUNTIME_CONFIGURE\n");
        if configure.is_some() {
            out.push_str(&format!("\t.configure = {drv}_uart_configure,\n"));
        }
        if get_config.is_some() {
            out.push_str(&format!("\t.config_get = {drv}_uart_config_get,\n"));
        }
        out.push_str("#endif\n");
    }
    if transmit.is_some() || receive.is_some() || await_drain.is_some() {
        out.push_str("#ifdef CONFIG_UART_INTERRUPT_DRIVEN\n");
        if transmit.is_some() {
            out.push_str(&format!("\t.fifo_fill = {drv}_uart_fifo_fill,\n"));
        }
        if receive.is_some() {
            out.push_str(&format!("\t.fifo_read = {drv}_uart_fifo_read,\n"));
        }
        if await_drain.is_some() {
            out.push_str(&format!("\t.irq_tx_complete = {drv}_uart_irq_tx_complete,\n"));
        }
        out.push_str("#endif\n");
    }
    out.push_str("};\n\n");
    for capability in serial.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-SERIAL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Signals, event delivery, lifecycle, purge, and abort have no Zephyr uart_driver_api slot here; absence is explicit and no result/event is fabricated. */\n\n");

    Ok(Emission {
        data_fields,
        text: out,
        api: format!("&{drv}_uart_api"),
    })
}
