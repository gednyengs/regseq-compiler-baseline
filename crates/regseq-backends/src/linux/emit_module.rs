//! Module integration file emission.
//!
//! Generates the backend-owned Linux module integration source file
//! containing AMBA/platform registration, probe/remove, uart_ops,
//! uart_driver, and ISR handler.

use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{generated_file_banner, CTopLevelItem, CTranslationUnit};
use super::plan::{LinuxBackendPlan, LinuxFileRole, LinuxProbeStep, LinuxRemoveStep};

/// Emit the module integration file with real C code.
pub(super) fn emit_module_integration(
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
) -> String {
    let reg = &plan.registration;
    let snake = &reg.module_name;
    let driver_name = &portable.identity.driver_name;

    let mut tu = CTranslationUnit::new();

    let filename = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::DriverMain)
        .map(|f| f.filename.as_str())
        .unwrap_or("module.c");
    tu.push(generated_file_banner(filename, driver_name));
    tu.push(CTopLevelItem::BlankLine);

    emit_includes(&mut tu, plan);
    tu.push(CTopLevelItem::BlankLine);

    emit_constants(&mut tu, plan, snake);
    tu.push(CTopLevelItem::BlankLine);

    emit_register_accessors(&mut tu, snake);
    tu.push(CTopLevelItem::BlankLine);

    emit_isr_handler(&mut tu, plan, snake);
    tu.push(CTopLevelItem::BlankLine);

    if plan.serial_core.is_some() {
        emit_uart_ops(&mut tu, plan, snake);
        tu.push(CTopLevelItem::BlankLine);
        emit_uart_driver_def(&mut tu, plan, snake);
        tu.push(CTopLevelItem::BlankLine);
    }

    emit_probe(&mut tu, plan, portable, snake);
    tu.push(CTopLevelItem::BlankLine);

    emit_remove(&mut tu, plan, snake);
    tu.push(CTopLevelItem::BlankLine);

    if reg.is_amba {
        emit_amba_registration(&mut tu, plan, snake);
    } else {
        emit_platform_registration(&mut tu, plan, snake);
    }
    tu.push(CTopLevelItem::BlankLine);

    if plan.serial_core.is_some() {
        emit_module_init_exit(&mut tu, plan, snake);
        tu.push(CTopLevelItem::BlankLine);
    } else {
        let macro_name = if reg.is_amba {
            "module_amba_driver"
        } else {
            "module_platform_driver"
        };
        tu.push(CTopLevelItem::Raw(format!(
            "{macro_name}({});",
            reg.driver_struct_name
        )));
        tu.push(CTopLevelItem::BlankLine);
    }

    tu.push(CTopLevelItem::MacroCall {
        name: "MODULE_LICENSE".into(),
        args: format!("\"{}\"", reg.module_license),
    });
    tu.push(CTopLevelItem::MacroCall {
        name: "MODULE_DESCRIPTION".into(),
        args: format!("\"Regseq-generated {} driver\"", driver_name),
    });

    tu.render()
}

fn emit_includes(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan) {
    let reg = &plan.registration;
    let snake = &reg.module_name;

    tu.push(CTopLevelItem::Include {
        path: "linux/module.h".into(),
        system: true,
    });
    if reg.is_amba {
        tu.push(CTopLevelItem::Include {
            path: "linux/amba/bus.h".into(),
            system: true,
        });
    } else {
        tu.push(CTopLevelItem::Include {
            path: "linux/platform_device.h".into(),
            system: true,
        });
        tu.push(CTopLevelItem::Include {
            path: "linux/of.h".into(),
            system: true,
        });
    }
    if plan.serial_core.is_some() {
        tu.push(CTopLevelItem::Include {
            path: "linux/serial_core.h".into(),
            system: true,
        });
        tu.push(CTopLevelItem::Include {
            path: "linux/tty_flip.h".into(),
            system: true,
        });
    }
    tu.push(CTopLevelItem::Include {
        path: "linux/io.h".into(),
        system: true,
    });
    tu.push(CTopLevelItem::Include {
        path: "linux/clk.h".into(),
        system: true,
    });
    tu.push(CTopLevelItem::Include {
        path: "linux/interrupt.h".into(),
        system: true,
    });
    tu.push(CTopLevelItem::Include {
        path: "linux/property.h".into(),
        system: true,
    });

    // Local headers
    let shared_header = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::SharedHeader)
        .map(|f| f.filename.clone())
        .unwrap_or_else(|| format!("regseq_{snake}_shared.h"));
    tu.push(CTopLevelItem::Include {
        path: shared_header,
        system: false,
    });
    if plan
        .artifact_layout
        .emitted_files
        .iter()
        .any(|f| f.role == LinuxFileRole::RegisterHelpers)
    {
        let regs_header = plan
            .artifact_layout
            .emitted_files
            .iter()
            .find(|f| f.role == LinuxFileRole::RegisterHelpers)
            .map(|f| f.filename.clone())
            .unwrap_or_else(|| format!("{snake}_regs.h"));
        tu.push(CTopLevelItem::Include {
            path: regs_header,
            system: false,
        });
    }
}

fn emit_constants(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let upper = snake.to_uppercase();

    tu.push(CTopLevelItem::LineComment(
        "Constants -- Regseq lowered".into(),
    ));
    tu.push(CTopLevelItem::Define {
        name: format!("{upper}_ALL_IRQ_MASK"),
        value: Some("0x07ffU".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: format!("{upper}_RX_IRQ_MASK"),
        value: Some("((1U << 4) | (1U << 6))".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: format!("{upper}_TX_IRQ_MASK"),
        value: Some("(1U << 5)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: format!("{upper}_ERR_IRQ_MASK"),
        value: Some("((1U << 7) | (1U << 8) | (1U << 9) | (1U << 10))".into()),
    });

    // Default values from properties
    for prop in &plan.probe.steps {
        if let LinuxProbeStep::ReadProperty {
            property_id,
            has_default,
        } = prop
        {
            if *has_default {
                let prop_upper = property_id.replace('-', "_").to_uppercase();
                tu.push(CTopLevelItem::Define {
                    name: format!("{upper}_DEFAULT_{prop_upper}"),
                    value: None,
                });
            }
        }
    }

    if plan.serial_core.is_some() {
        tu.push(CTopLevelItem::Define {
            name: format!("PORT_{upper}"),
            value: Some("113".into()),
        });
    }
}

fn emit_register_accessors(tu: &mut CTranslationUnit, snake: &str) {
    tu.push(CTopLevelItem::LineComment("Register access helpers".into()));
    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static inline u32 {snake}_reg_read(struct {snake}_data *d, unsigned int off)"
        ),
        body: vec!["return readl(d->regs + off);".into()],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static inline void {snake}_reg_write(struct {snake}_data *d, unsigned int off, u32 val)"
        ),
        body: vec!["writel(val, d->regs + off);".into()],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static inline void {snake}_reg_modify(struct {snake}_data *d, unsigned int off, u32 clr, u32 set)"
        ),
        body: vec![
            format!("u32 tmp = {snake}_reg_read(d, off);"),
            "tmp = (tmp & ~clr) | set;".into(),
            format!("{snake}_reg_write(d, off, tmp);"),
        ],
        comment: None,
    });
}

fn emit_isr_handler(tu: &mut CTranslationUnit, _plan: &LinuxBackendPlan, snake: &str) {
    let upper = snake.to_uppercase();

    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static irqreturn_t {snake}_irq_handler(int irq, void *dev_id)"
        ),
        body: vec![
            format!("struct {snake}_data *d = dev_id;"),
            "u32 status;".into(),
            String::new(),
            format!("status = {snake}_reg_read(d, PL011_MIS);"),
            "if (status == 0)".into(),
            "\treturn IRQ_NONE;".into(),
            String::new(),
            format!("if (status & {upper}_RX_IRQ_MASK) {{"),
            format!("\tstruct uart_port *port = &d->port;"),
            format!(
                "\twhile (!({snake}_reg_read(d, PL011_FR) & PL011_FR_RXFE)) {{"
            ),
            format!("\t\tu32 dr = {snake}_reg_read(d, PL011_DR);"),
            "\t\tu8 ch = dr & 0xff;".into(),
            "\t\tunsigned int flag = TTY_NORMAL;".into(),
            String::new(),
            "\t\tport->icount.rx++;".into(),
            "\t\tif (dr & PL011_DR_OE)".into(),
            "\t\t\tport->icount.overrun++;".into(),
            "\t\tif (dr & PL011_DR_BE)".into(),
            "\t\t\tflag = TTY_BREAK;".into(),
            "\t\tif (dr & PL011_DR_PE)".into(),
            "\t\t\tflag = TTY_PARITY;".into(),
            "\t\tif (dr & PL011_DR_FE)".into(),
            "\t\t\tflag = TTY_FRAME;".into(),
            String::new(),
            "\t\tuart_insert_char(port, dr, PL011_DR_OE, ch, flag);".into(),
            "\t}".into(),
            "\ttty_flip_buffer_push(&port->state->port);".into(),
            "}".into(),
            String::new(),
            format!("if (status & {upper}_TX_IRQ_MASK) {{"),
            format!("\tstruct circ_buf *xmit = &d->port.state->xmit;"),
            String::new(),
            "\tif (uart_circ_empty(xmit) || uart_tx_stopped(&d->port)) {".into(),
            format!(
                "\t\t{snake}_reg_modify(d, PL011_IMSC, {upper}_TX_IRQ_MASK, 0);"
            ),
            "\t} else {".into(),
            format!(
                "\t\twhile (!uart_circ_empty(xmit) && !({snake}_reg_read(d, PL011_FR) & PL011_FR_TXFF)) {{"
            ),
            format!(
                "\t\t\t{snake}_reg_write(d, PL011_DR, xmit->buf[xmit->tail]);"
            ),
            "\t\t\txmit->tail = (xmit->tail + 1) & (UART_XMIT_SIZE - 1);".into(),
            "\t\t\td->port.icount.tx++;".into(),
            "\t\t}".into(),
            "\t\tif (uart_circ_chars_pending(xmit) < WAKEUP_CHARS)".into(),
            "\t\t\tuart_write_wakeup(&d->port);".into(),
            "\t}".into(),
            "}".into(),
            String::new(),
            format!("if (status & {upper}_ERR_IRQ_MASK)"),
            format!(
                "\t{snake}_reg_write(d, PL011_ICR, status & {upper}_ERR_IRQ_MASK);"
            ),
            String::new(),
            "return IRQ_HANDLED;".into(),
        ],
        comment: Some("ISR -- Regseq interrupt handler".into()),
    });
}

fn emit_uart_ops(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let upper = snake.to_uppercase();

    // tx_empty
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static unsigned int {snake}_tx_empty(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            format!("return ({snake}_reg_read(d, PL011_FR) & PL011_FR_BUSY) ? 0 : TIOCSER_TEMT;"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    // set_mctrl / get_mctrl (minimal)
    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static void {snake}_set_mctrl(struct uart_port *port, unsigned int mctrl)"
        ),
        body: vec!["/* Modem control not realized in first slice */".into()],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static unsigned int {snake}_get_mctrl(struct uart_port *port)"),
        body: vec!["return TIOCM_CAR | TIOCM_CTS | TIOCM_DSR;".into()],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    // stop_tx / start_tx / stop_rx
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_stop_tx(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            format!("{snake}_reg_modify(d, PL011_IMSC, {upper}_TX_IRQ_MASK, 0);"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_start_tx(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            format!("{snake}_reg_modify(d, PL011_IMSC, {upper}_TX_IRQ_MASK, {upper}_TX_IRQ_MASK);"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_stop_rx(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            format!("{snake}_reg_modify(d, PL011_IMSC, {upper}_RX_IRQ_MASK, 0);"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    // break_ctl
    tu.push(CTopLevelItem::FuncDef {
        signature: format!(
            "static void {snake}_break_ctl(struct uart_port *port, int break_state)"
        ),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            "if (break_state == -1)".into(),
            format!("\t{snake}_reg_modify(d, PL011_LCRH, PL011_LCRH_BRK, PL011_LCRH_BRK);"),
            "else".into(),
            format!("\t{snake}_reg_modify(d, PL011_LCRH, PL011_LCRH_BRK, 0);"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    // startup
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static int {snake}_startup(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            "u32 uart_clk_hz;".into(),
            String::new(),
            "if (d->clock) {".into(),
            "\tint ret = clk_prepare_enable(d->clock);".into(),
            "\tif (ret)".into(),
            "\t\treturn ret;".into(),
            "}".into(),
            "uart_clk_hz = d->clock ? clk_get_rate(d->clock) : port->uartclk;".into(),
            String::new(),
            format!("/* Regseq lifecycle::init -- disable, configure, enable */"),
            format!("{snake}_reg_write(d, PL011_CR, 0);"),
            format!("{snake}_reg_modify(d, PL011_LCRH, PL011_LCRH_FEN, 0);"),
            String::new(),
            "/* Baud-rate programming */".into(),
            "{".into(),
            "\tu64 div64 = (u64)uart_clk_hz * 4;".into(),
            "\tdo_div(div64, d->current_speed);".into(),
            format!("\t{snake}_reg_write(d, PL011_IBRD, (u32)(div64 >> 6));"),
            format!("\t{snake}_reg_write(d, PL011_FBRD, (u32)(div64 & 0x3f));"),
            "}".into(),
            String::new(),
            format!("{snake}_reg_write(d, PL011_LCRH, PL011_LCRH_WLEN_8 | PL011_LCRH_FEN);"),
            format!("{snake}_reg_write(d, PL011_IFLS, PL011_IFLS_RX_HALF | PL011_IFLS_TX_EIGHTH);"),
            format!("{snake}_reg_write(d, PL011_IMSC, 0);"),
            format!("{snake}_reg_write(d, PL011_ICR, {upper}_ALL_IRQ_MASK);"),
            format!(
                "{snake}_reg_write(d, PL011_CR, PL011_CR_UARTEN | PL011_CR_TXE | PL011_CR_RXE);"
            ),
            format!("{snake}_reg_modify(d, PL011_IMSC, {upper}_RX_IRQ_MASK, {upper}_RX_IRQ_MASK);"),
            String::new(),
            "return 0;".into(),
        ],
        comment: Some("startup -- Regseq lifecycle::init".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    // shutdown
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_shutdown(struct uart_port *port)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            String::new(),
            format!("/* Regseq lifecycle::deinit -- mask, clear, disable */"),
            format!("{snake}_reg_write(d, PL011_IMSC, 0);"),
            format!("{snake}_reg_write(d, PL011_ICR, {upper}_ALL_IRQ_MASK);"),
            format!("{snake}_reg_modify(d, PL011_LCRH, PL011_LCRH_FEN, 0);"),
            format!("{snake}_reg_write(d, PL011_CR, 0);"),
            String::new(),
            "if (d->clock)".into(),
            "\tclk_disable_unprepare(d->clock);".into(),
        ],
        comment: Some("shutdown -- Regseq lifecycle::deinit".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    // set_termios
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_set_termios(struct uart_port *port, struct ktermios *termios, const struct ktermios *old)"),
        body: vec![
            format!("struct {snake}_data *d = container_of(port, struct {snake}_data, port);"),
            "unsigned int baud;".into(),
            "u32 uart_clk_hz, lcr_h;".into(),
            String::new(),
            "baud = uart_get_baud_rate(port, termios, old, 0, port->uartclk / 16);".into(),
            "uart_clk_hz = d->clock ? clk_get_rate(d->clock) : port->uartclk;".into(),
            String::new(),
            format!("{snake}_reg_write(d, PL011_CR, 0);"),
            String::new(),
            "/* Baud-rate programming */".into(),
            "{".into(),
            "\tu64 div64 = (u64)uart_clk_hz * 4;".into(),
            "\tdo_div(div64, baud);".into(),
            format!("\t{snake}_reg_write(d, PL011_IBRD, (u32)(div64 >> 6));"),
            format!("\t{snake}_reg_write(d, PL011_FBRD, (u32)(div64 & 0x3f));"),
            "}".into(),
            String::new(),
            "lcr_h = PL011_LCRH_FEN;".into(),
            "switch (termios->c_cflag & CSIZE) {".into(),
            "case CS5: break;".into(),
            "case CS6: lcr_h |= PL011_LCRH_WLEN_6; break;".into(),
            "case CS7: lcr_h |= PL011_LCRH_WLEN_7; break;".into(),
            "case CS8: default: lcr_h |= PL011_LCRH_WLEN_8; break;".into(),
            "}".into(),
            "if (termios->c_cflag & CSTOPB)".into(),
            "\tlcr_h |= PL011_LCRH_STP2;".into(),
            "if (termios->c_cflag & PARENB) {".into(),
            "\tlcr_h |= PL011_LCRH_PEN;".into(),
            "\tif (!(termios->c_cflag & PARODD))".into(),
            "\t\tlcr_h |= PL011_LCRH_EPS;".into(),
            "}".into(),
            format!("{snake}_reg_write(d, PL011_LCRH, lcr_h);"),
            format!("{snake}_reg_write(d, PL011_CR, PL011_CR_UARTEN | PL011_CR_TXE | PL011_CR_RXE);"),
            String::new(),
            "uart_update_timeout(port, termios->c_cflag, baud);".into(),
        ],
        comment: Some("set_termios -- Regseq Serial::configure".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    // type / config_port / request_port / release_port
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static const char *{snake}_type(struct uart_port *port)"),
        body: vec![format!("return \"{}\";", plan.registration.module_name)],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_config_port(struct uart_port *port, int flags)"),
        body: vec![
            "if (flags & UART_CONFIG_TYPE)".into(),
            format!("\tport->type = PORT_{upper};"),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static int {snake}_request_port(struct uart_port *port)"),
        body: vec!["return 0;".into()],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_release_port(struct uart_port *port)"),
        body: vec![],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    // uart_ops struct
    tu.push(CTopLevelItem::LineComment("uart_ops definition".into()));
    let serial = plan.serial_core.as_ref().unwrap();
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static const".into(),
        type_name: "struct uart_ops".into(),
        name: serial.uart_ops_name.clone(),
        initializer: vec![
            format!(".tx_empty\t= {snake}_tx_empty,"),
            format!(".set_mctrl\t= {snake}_set_mctrl,"),
            format!(".get_mctrl\t= {snake}_get_mctrl,"),
            format!(".stop_tx\t= {snake}_stop_tx,"),
            format!(".start_tx\t= {snake}_start_tx,"),
            format!(".stop_rx\t= {snake}_stop_rx,"),
            format!(".break_ctl\t= {snake}_break_ctl,"),
            format!(".startup\t= {snake}_startup,"),
            format!(".shutdown\t= {snake}_shutdown,"),
            format!(".set_termios\t= {snake}_set_termios,"),
            format!(".type\t\t= {snake}_type,"),
            format!(".config_port\t= {snake}_config_port,"),
            format!(".request_port\t= {snake}_request_port,"),
            format!(".release_port\t= {snake}_release_port,"),
        ],
    });
}

fn emit_uart_driver_def(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let serial = plan.serial_core.as_ref().unwrap();

    tu.push(CTopLevelItem::LineComment("uart_driver definition".into()));
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static".into(),
        type_name: "struct uart_driver".into(),
        name: serial.uart_driver_name.clone(),
        initializer: vec![
            ".owner\t\t= THIS_MODULE,".into(),
            format!(".driver_name\t= \"{snake}\","),
            ".dev_name\t= \"ttyPL\",".into(),
            ".nr\t\t= 1,".into(),
        ],
    });
}

fn emit_probe(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    let upper = snake.to_uppercase();
    let mut body = Vec::new();

    body.push(format!("struct {snake}_data *d;"));
    body.push("int ret;".into());
    body.push(String::new());

    if plan.registration.is_amba {
        body.push("d = devm_kzalloc(&adev->dev, sizeof(*d), GFP_KERNEL);".into());
    } else {
        body.push("d = devm_kzalloc(&pdev->dev, sizeof(*d), GFP_KERNEL);".into());
    }
    body.push("if (!d)".into());
    body.push("\treturn -ENOMEM;".into());
    body.push(String::new());

    let dev_expr = if plan.registration.is_amba {
        "&adev->dev"
    } else {
        "&pdev->dev"
    };

    // Resource acquisition in plan order
    for step in &plan.probe.steps {
        match step {
            LinuxProbeStep::GetClock {
                clock_id: _,
                optional,
            } => {
                if *optional {
                    body.push(format!(
                        "d->clock = devm_clk_get_optional({dev_expr}, NULL);"
                    ));
                    body.push("if (IS_ERR(d->clock))".into());
                    body.push("\treturn PTR_ERR(d->clock);".into());
                } else {
                    body.push(format!("d->clock = devm_clk_get({dev_expr}, NULL);"));
                    body.push("if (IS_ERR(d->clock))".into());
                    body.push("\treturn PTR_ERR(d->clock);".into());
                }
                body.push(String::new());
            }
            LinuxProbeStep::DeassertReset { optional, .. } => {
                let opt = if *optional { " (optional)" } else { "" };
                body.push(format!(
                    "/* TODO: devm_reset_control_get + deassert{opt} */"
                ));
                body.push(String::new());
            }
            LinuxProbeStep::SelectPinCtrl { optional, .. } => {
                let opt = if *optional { " (optional)" } else { "" };
                body.push(format!("/* TODO: devm_pinctrl_get + select{opt} */"));
                body.push(String::new());
            }
            LinuxProbeStep::MapMmio { .. } => {
                if plan.registration.is_amba {
                    body.push(format!(
                        "d->regs = devm_ioremap_resource({dev_expr}, &adev->res);"
                    ));
                } else {
                    body.push(format!(
                        "d->regs = devm_ioremap_resource({dev_expr}, platform_get_resource(pdev, IORESOURCE_MEM, 0));"
                    ));
                }
                body.push("if (IS_ERR(d->regs))".into());
                body.push("\treturn PTR_ERR(d->regs);".into());
                body.push(String::new());
            }
            LinuxProbeStep::ReadProperty {
                property_id,
                has_default,
            } => {
                let field_name = property_id.replace('-', "_");
                let prop_upper = property_id.replace('-', "_").to_uppercase();

                body.push(format!(
                    "ret = device_property_read_u32({dev_expr}, \"{property_id}\", &d->{field_name});"
                ));
                if *has_default {
                    body.push("if (ret)".into());
                    body.push(format!("\td->{field_name} = {upper}_DEFAULT_{prop_upper};"));
                } else {
                    body.push("if (ret)".into());
                    body.push("\treturn ret;".into());
                }
                body.push(String::new());
            }
            LinuxProbeStep::RequestIrq { .. } => {
                if plan.registration.is_amba {
                    body.push("d->irq = adev->irq[0];".into());
                } else {
                    body.push("d->irq = platform_get_irq(pdev, 0);".into());
                    body.push("if (d->irq < 0)".into());
                    body.push("\treturn d->irq;".into());
                }
                body.push(format!(
                    "ret = devm_request_irq({dev_expr}, d->irq, {snake}_irq_handler, IRQF_SHARED, dev_name({dev_expr}), d);"
                ));
                body.push("if (ret)".into());
                body.push("\treturn ret;".into());
                body.push(String::new());
            }
            LinuxProbeStep::RegisterUartPort => {
                let serial = plan.serial_core.as_ref().unwrap();
                body.push("/* Serial port setup */".into());
                body.push(format!("d->port.dev = {dev_expr};"));
                if plan.registration.is_amba {
                    body.push("d->port.mapbase = adev->res.start;".into());
                }
                body.push("d->port.membase = d->regs;".into());
                body.push("d->port.irq = d->irq;".into());

                // Find clock rate default
                let has_opt_clock = portable.resources.clocks.iter().any(|c| !c.required);
                if has_opt_clock {
                    body.push(format!(
                        "d->port.uartclk = d->clock ? clk_get_rate(d->clock) : {upper}_DEFAULT_CURRENT_SPEED;"
                    ));
                } else {
                    body.push("d->port.uartclk = clk_get_rate(d->clock);".into());
                }

                body.push("d->port.fifosize = d->fifo_size;".into());
                body.push(format!("d->port.ops = &{};", serial.uart_ops_name));
                body.push("d->port.flags = UPF_BOOT_AUTOCONF;".into());
                body.push(format!("d->port.type = PORT_{upper};"));
                body.push(String::new());
                if plan.registration.is_amba {
                    body.push("amba_set_drvdata(adev, d);".into());
                } else {
                    body.push("platform_set_drvdata(pdev, d);".into());
                }
                body.push(String::new());
                body.push(format!(
                    "ret = uart_add_one_port(&{}, &d->port);",
                    serial.uart_driver_name
                ));
                body.push("if (ret)".into());
                body.push("\treturn ret;".into());
                body.push(String::new());
            }
        }
    }

    body.push("return 0;".into());

    let sig = if plan.registration.is_amba {
        format!("static int {snake}_probe(struct amba_device *adev, const struct amba_id *id)")
    } else {
        format!("static int {snake}_probe(struct platform_device *pdev)")
    };

    tu.push(CTopLevelItem::FuncDef {
        signature: sig,
        body,
        comment: Some("probe -- Regseq lifecycle".into()),
    });
}

fn emit_remove(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let mut body = Vec::new();

    if plan.registration.is_amba {
        body.push(format!("struct {snake}_data *d = amba_get_drvdata(adev);"));
    } else {
        body.push(format!(
            "struct {snake}_data *d = platform_get_drvdata(pdev);"
        ));
    }
    body.push(String::new());

    for step in &plan.remove.steps {
        match step {
            LinuxRemoveStep::UnregisterUartPort => {
                let serial = plan.serial_core.as_ref().unwrap();
                body.push(format!(
                    "uart_remove_one_port(&{}, &d->port);",
                    serial.uart_driver_name
                ));
            }
            LinuxRemoveStep::FreeIrq { .. } => {
                body.push("/* IRQ freed by devm */".into());
            }
            LinuxRemoveStep::PutClock { .. } => {
                body.push("/* Clock released by devm */".into());
            }
            LinuxRemoveStep::AssertReset { .. } => {
                body.push("/* Reset released by devm */".into());
            }
            LinuxRemoveStep::UnmapMmio { .. } => {
                body.push("/* MMIO unmapped by devm */".into());
            }
        }
    }

    let sig = if plan.registration.is_amba {
        format!("static void {snake}_remove(struct amba_device *adev)")
    } else {
        format!("static void {snake}_remove(struct platform_device *pdev)")
    };

    tu.push(CTopLevelItem::FuncDef {
        signature: sig,
        body,
        comment: Some("remove -- Regseq lifecycle".into()),
    });
}

fn emit_amba_registration(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let reg = &plan.registration;

    let mut init_lines = Vec::new();
    if let (Some(id), Some(mask)) = (reg.amba_id, reg.amba_mask) {
        init_lines.push(format!("{{ .id = {id:#010x}, .mask = {mask:#010x} }},"));
    }
    init_lines.push("{ 0 },".into());

    tu.push(CTopLevelItem::LineComment("AMBA match table".into()));
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static const".into(),
        type_name: "struct amba_id".into(),
        name: format!("{}[]", reg.match_table_name),
        initializer: init_lines,
    });
    tu.push(CTopLevelItem::Raw(format!(
        "MODULE_DEVICE_TABLE(amba, {});",
        reg.match_table_name
    )));
    tu.push(CTopLevelItem::BlankLine);

    tu.push(CTopLevelItem::LineComment(
        "AMBA driver registration".into(),
    ));
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static".into(),
        type_name: "struct amba_driver".into(),
        name: reg.driver_struct_name.clone(),
        initializer: vec![
            ".drv = {".into(),
            format!("\t.name = \"{snake}\","),
            "},".into(),
            format!(".id_table = {},", reg.match_table_name),
            format!(".probe = {snake}_probe,"),
            format!(".remove = {snake}_remove,"),
        ],
    });
}

fn emit_platform_registration(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let reg = &plan.registration;

    let mut init_lines = Vec::new();
    for compat in &reg.compatible_entries {
        init_lines.push(format!("{{ .compatible = \"{compat}\" }},"));
    }
    init_lines.push("{ },".into());

    tu.push(CTopLevelItem::LineComment("OF match table".into()));
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static const".into(),
        type_name: "struct of_device_id".into(),
        name: format!("{}[]", reg.match_table_name),
        initializer: init_lines,
    });
    tu.push(CTopLevelItem::Raw(format!(
        "MODULE_DEVICE_TABLE(of, {});",
        reg.match_table_name
    )));
    tu.push(CTopLevelItem::BlankLine);

    tu.push(CTopLevelItem::LineComment(
        "Platform driver registration".into(),
    ));
    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static".into(),
        type_name: "struct platform_driver".into(),
        name: reg.driver_struct_name.clone(),
        initializer: vec![
            format!(".probe = {snake}_probe,"),
            format!(".remove = {snake}_remove,"),
            ".driver = {".into(),
            format!("\t.name = \"{snake}\","),
            format!("\t.of_match_table = {},", reg.match_table_name),
            "},".into(),
        ],
    });
}

fn emit_module_init_exit(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let serial = plan.serial_core.as_ref().unwrap();
    let reg = &plan.registration;

    let register_fn = if reg.is_amba {
        "amba_driver_register"
    } else {
        "platform_driver_register"
    };
    let unregister_fn = if reg.is_amba {
        "amba_driver_unregister"
    } else {
        "platform_driver_unregister"
    };

    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static int __init {snake}_init(void)"),
        body: vec![
            "int ret;".into(),
            String::new(),
            format!("ret = uart_register_driver(&{});", serial.uart_driver_name),
            "if (ret)".into(),
            "\treturn ret;".into(),
            String::new(),
            format!("ret = {register_fn}(&{});", reg.driver_struct_name),
            "if (ret)".into(),
            format!("\tuart_unregister_driver(&{});", serial.uart_driver_name),
            String::new(),
            "return ret;".into(),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void __exit {snake}_exit(void)"),
        body: vec![
            format!("{unregister_fn}(&{});", reg.driver_struct_name),
            format!("uart_unregister_driver(&{});", serial.uart_driver_name),
        ],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::Raw(format!("module_init({snake}_init);")));
    tu.push(CTopLevelItem::Raw(format!("module_exit({snake}_exit);")));
}
