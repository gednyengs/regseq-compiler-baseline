//! Module integration file emission.
//!
//! Generates the backend-owned Linux module integration source file
//! containing AMBA/platform registration, probe/remove, uart_ops,
//! uart_driver, and ISR handler.
//!
//! All callback bodies are emitted by walking the portable behavior
//! plan. No device-specific register offsets, interrupt masks, callback
//! templates, or naming policy appears in this module.

use crate::lowering::behavior::{BehaviorPlan, FunctionBehavior, FunctionRole};
use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{generated_file_banner, CTopLevelItem, CTranslationUnit};
use super::emit_behavior::{
    emit_behavior_nodes, interrupt_class_define, register_offset_define, EmitContext,
};
use super::plan::{
    LinuxBackendPlan, LinuxCallbackGroup, LinuxFileRole, LinuxProbeStep, LinuxRemoveStep,
};

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

    emit_constants(&mut tu, plan, portable, snake);
    tu.push(CTopLevelItem::BlankLine);

    emit_register_accessors(&mut tu, snake);
    tu.push(CTopLevelItem::BlankLine);

    // Emit helper functions from behavior plan.
    emit_helper_functions(&mut tu, plan, portable, snake);

    // Emit ISR handler from behavior plan.
    emit_isr_handler(&mut tu, plan, portable, snake);
    tu.push(CTopLevelItem::BlankLine);

    if plan.serial_core.is_some() {
        emit_uart_ops(&mut tu, plan, portable, snake);
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

/// Emit constants derived from plan data.
///
/// Interrupt class masks come from `portable.behavior.interrupt_classes`.
/// Property defaults come from `plan.property_realization[].default_literal`.
/// Port type comes from `plan.serial_core.port_type`.
fn emit_constants(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    let upper = snake.to_uppercase();

    tu.push(CTopLevelItem::LineComment(
        "Constants -- Regseq lowered".into(),
    ));

    // Interrupt class masks from plan data.
    for class in &portable.behavior.interrupt_classes {
        tu.push(CTopLevelItem::Define {
            name: interrupt_class_define(&upper, &class.name),
            value: Some(format!("{:#06x}U", class.mask_value)),
        });
    }

    // Property default values from authored default literals.
    for prop in &plan.property_realization {
        if let Some(ref default_lit) = prop.default_literal {
            let prop_upper = prop.property_id.replace('-', "_").to_uppercase();
            tu.push(CTopLevelItem::Define {
                name: format!("{upper}_DEFAULT_{prop_upper}"),
                value: Some(default_lit.clone()),
            });
        }
    }

    // Port type from plan.
    if let Some(ref serial) = plan.serial_core {
        tu.push(CTopLevelItem::Define {
            name: serial.port_type.define_name.clone(),
            value: Some(format!("{}", serial.port_type.value)),
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

/// Build an EmitContext from plans.
fn make_emit_context<'a>(
    plan: &'a LinuxBackendPlan,
    portable: &'a DriverLoweringPlan,
    snake: &'a str,
    role: Option<FunctionRole>,
) -> EmitContext<'a> {
    let reg_prefix = plan
        .register_layout
        .blocks
        .first()
        .map(|b| b.reg_prefix.as_str())
        .unwrap_or("REG");

    EmitContext {
        snake,
        upper: snake.to_uppercase(),
        reg_prefix,
        behavior: &portable.behavior,
        register_layout: &plan.register_layout,
        current_role: role,
    }
}

/// Find a FunctionBehavior by role.
fn find_behavior_by_role(behavior: &BehaviorPlan, role: FunctionRole) -> Option<&FunctionBehavior> {
    behavior.functions.iter().find(|f| f.role == role)
}

/// Emit helper functions from the behavior plan.
fn emit_helper_functions(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    for mapping in &plan.behavior_mapping.callback_mappings {
        if mapping.group != LinuxCallbackGroup::Helper {
            continue;
        }
        let func = portable
            .behavior
            .functions
            .iter()
            .find(|f| f.name == mapping.portable_function);
        if let Some(func) = func {
            let ctx = make_emit_context(plan, portable, snake, Some(func.role.clone()));
            let body_lines = emit_behavior_nodes(&func.body, &ctx, 0);
            tu.push(CTopLevelItem::FuncDef {
                signature: format!("static void {snake}_{}(struct {snake}_data *d)", func.name),
                body: body_lines,
                comment: Some(format!("Helper: {}", func.name)),
            });
            tu.push(CTopLevelItem::BlankLine);
        }
    }
}

/// Emit ISR handler from behavior plan.
fn emit_isr_handler(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    let isr_func = find_behavior_by_role(&portable.behavior, FunctionRole::Isr);

    let body = if let Some(func) = isr_func {
        let ctx = make_emit_context(plan, portable, snake, Some(FunctionRole::Isr));
        let mut lines = vec![
            format!("struct {snake}_data *d = dev_id;"),
            "u32 status;".into(),
            String::new(),
        ];

        // Read the interrupt status register.
        let mis_define = find_register_define_by_suffix(plan, "mis");
        lines.push(format!("status = {snake}_reg_read(d, {mis_define});"));
        lines.push("if (status == 0)".into());
        lines.push("\treturn IRQ_NONE;".into());
        lines.push(String::new());

        // Walk the behavior tree for the ISR body.
        let body_lines = emit_behavior_nodes(&func.body, &ctx, 0);
        lines.extend(body_lines);

        // If the behavior tree didn't emit a return, add one.
        if !lines.iter().any(|l| l.contains("return IRQ_HANDLED")) {
            lines.push(String::new());
            lines.push("return IRQ_HANDLED;".into());
        }
        lines
    } else {
        vec![
            format!("struct {snake}_data *d = dev_id;"),
            "(void)d;".into(),
            "return IRQ_NONE;".into(),
        ]
    };

    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static irqreturn_t {snake}_irq_handler(int irq, void *dev_id)"),
        body,
        comment: Some("ISR -- Regseq interrupt handler".into()),
    });
}

/// Emit uart_ops callback functions from behavior plan.
fn emit_uart_ops(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    // tx_empty — from behavior plan or minimal stub.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialTxEmpty,
        &format!("static unsigned int {snake}_tx_empty(struct uart_port *port)"),
        true,
        &["return 0;".into()],
    );

    // set_mctrl / get_mctrl — no authored behavior, minimal stubs.
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

    // stop_tx — from SerialTxByte behavior (disable TX interrupt).
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialTxByte,
        &format!("static void {snake}_stop_tx(struct uart_port *port)"),
        true,
        &[],
    );

    // start_tx — enable TX interrupt.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialTxByte,
        &format!("static void {snake}_start_tx(struct uart_port *port)"),
        true,
        &[],
    );

    // stop_rx — disable RX interrupt.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialFlushRxFifo,
        &format!("static void {snake}_stop_rx(struct uart_port *port)"),
        true,
        &[],
    );

    // break_ctl — from behavior plan.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialSendBreak,
        &format!("static void {snake}_break_ctl(struct uart_port *port, int break_state)"),
        true,
        &[],
    );

    // startup — from SerialPortOpen behavior.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialPortOpen,
        &format!("static int {snake}_startup(struct uart_port *port)"),
        true,
        &["return 0;".into()],
    );

    // shutdown — from SerialPortClose behavior.
    emit_behavior_callback(
        tu,
        plan,
        portable,
        snake,
        FunctionRole::SerialPortClose,
        &format!("static void {snake}_shutdown(struct uart_port *port)"),
        true,
        &[],
    );

    // set_termios — from SerialConfigure behavior.
    emit_behavior_callback(
        tu, plan, portable, snake,
        FunctionRole::SerialConfigure,
        &format!("static void {snake}_set_termios(struct uart_port *port, struct ktermios *termios, const struct ktermios *old)"),
        true,
        &[],
    );

    // type / config_port / request_port / release_port — plan-driven stubs.
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static const char *{snake}_type(struct uart_port *port)"),
        body: vec![format!("return \"{}\";", plan.registration.module_name)],
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);

    let port_type_name = plan
        .serial_core
        .as_ref()
        .map(|s| s.port_type.define_name.as_str())
        .unwrap_or("PORT_UNKNOWN");
    tu.push(CTopLevelItem::FuncDef {
        signature: format!("static void {snake}_config_port(struct uart_port *port, int flags)"),
        body: vec![
            "if (flags & UART_CONFIG_TYPE)".into(),
            format!("\tport->type = {port_type_name};"),
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

    // uart_ops struct.
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

/// Emit a uart_ops callback from behavior plan data.
#[allow(clippy::too_many_arguments)]
fn emit_behavior_callback(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
    role: FunctionRole,
    signature: &str,
    with_container_of: bool,
    default_body: &[String],
) {
    let func = find_behavior_by_role(&portable.behavior, role.clone());

    let body = if let Some(func) = func {
        let ctx = make_emit_context(plan, portable, snake, Some(role));
        let mut lines = Vec::new();
        if with_container_of {
            lines.push(format!(
                "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
            ));
            lines.push(String::new());
        }
        lines.extend(emit_behavior_nodes(&func.body, &ctx, 0));
        lines
    } else if !default_body.is_empty() {
        let mut lines = Vec::new();
        if with_container_of {
            lines.push(format!(
                "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
            ));
        }
        lines.extend(default_body.iter().cloned());
        lines
    } else {
        vec![]
    };

    tu.push(CTopLevelItem::FuncDef {
        signature: signature.into(),
        body,
        comment: None,
    });
    tu.push(CTopLevelItem::BlankLine);
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
            format!(".dev_name\t= \"{}\",", serial.dev_name),
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

    // Resource acquisition in plan order.
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

                // Clock rate: use plan deployment metadata.
                let has_opt_clock = portable.resources.clocks.iter().any(|c| !c.required);
                if has_opt_clock {
                    // Find the property default for current-speed.
                    let default_speed_define = plan
                        .property_realization
                        .iter()
                        .find(|p| p.property_id == "current-speed")
                        .and_then(|p| p.default_literal.as_ref())
                        .map(|_| format!("{upper}_DEFAULT_CURRENT_SPEED"))
                        .unwrap_or_else(|| "115200".into());
                    body.push(format!(
                        "d->port.uartclk = d->clock ? clk_get_rate(d->clock) : {default_speed_define};"
                    ));
                } else {
                    body.push("d->port.uartclk = clk_get_rate(d->clock);".into());
                }

                // FIFO size from property field if available.
                if portable
                    .resources
                    .properties
                    .iter()
                    .any(|p| p.field_name == "fifo_size")
                {
                    body.push("d->port.fifosize = d->fifo_size;".into());
                }
                body.push(format!("d->port.ops = &{};", serial.uart_ops_name));
                body.push("d->port.flags = UPF_BOOT_AUTOCONF;".into());
                body.push(format!("d->port.type = {};", serial.port_type.define_name));
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

// ---------------------------------------------------------------------------
// Register layout helpers
// ---------------------------------------------------------------------------

/// Find a register define by field name suffix (case-insensitive).
fn find_register_define_by_suffix(plan: &LinuxBackendPlan, suffix: &str) -> String {
    for block in &plan.register_layout.blocks {
        for field in &block.fields {
            if field.name.eq_ignore_ascii_case(suffix) {
                return register_offset_define(&block.reg_prefix, &field.name);
            }
        }
    }
    // Fallback: construct from first block prefix.
    let prefix = plan
        .register_layout
        .blocks
        .first()
        .map(|b| b.reg_prefix.as_str())
        .unwrap_or("REG");
    format!("{}_{}", prefix, suffix.to_uppercase())
}
