//! Module integration file emission.
//!
//! Generates the backend-owned Linux module integration source file
//! containing AMBA/platform registration, probe/remove, uart_ops,
//! uart_driver, and ISR handler.
//!
//! All callback bodies are emitted by walking the semantic model from
//! `DriverLoweringPlan.behavior`. No device-specific register offsets,
//! interrupt masks, callback templates, or naming policy appears in this
//! module.

use std::collections::HashSet;

use regseq_model::item::function::FunctionRole;
use regseq_model::semantic::body::SemanticBody;
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::local::LocalId;
use regseq_model::semantic::model::SemanticModel;
use regseq_model::semantic::stmt::Stmt;

use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{CTopLevelItem, CTranslationUnit};
use super::conventions::generated_file_banner;
use super::emit_semantic::{
    emit_body_stmts, interrupt_class_define, register_offset_define, semantic_type_to_c,
    ReturnMode, SemanticEmitContext,
};
use super::plan::{
    LinuxBackendPlan, LinuxCallbackGroup, LinuxFileRole, LinuxProbeStep, LinuxRemoveStep,
    LinuxUartOpsBody, ReturnAdaptation,
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

/// Emit constants derived from plan data and the semantic model.
///
/// Interrupt class masks come from `plan.behavior.interrupt_classes`.
/// Authored constants come from `plan.behavior.constants`.
/// Property defaults come from `plan.property_realization[].default_literal`.
/// Port type comes from `plan.serial_core.port_type`.
fn emit_constants(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    _portable: &DriverLoweringPlan,
    snake: &str,
) {
    let upper = snake.to_uppercase();

    tu.push(CTopLevelItem::LineComment(
        "Constants -- Regseq lowered".into(),
    ));

    // Interrupt class masks from semantic model.
    for class in &plan.behavior.interrupt_classes {
        tu.push(CTopLevelItem::Define {
            name: interrupt_class_define(&upper, &class.name),
            value: Some(format!("{:#06x}U", class.mask_value)),
        });
    }

    // Authored constants from semantic model (only when populated).
    for cdef in &plan.behavior.constants {
        if let Some(ref val) = cdef.value {
            let val_str = match val {
                regseq_model::item::constant::ConstValue::Integer(i) => format!("{i}"),
                regseq_model::item::constant::ConstValue::Bool(b) => {
                    if *b {
                        "1".into()
                    } else {
                        "0".into()
                    }
                }
                regseq_model::item::constant::ConstValue::Str(s) => format!("\"{s}\""),
            };
            tu.push(CTopLevelItem::Define {
                name: cdef.stable_name.clone(),
                value: Some(val_str),
            });
        }
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

/// Build a SemanticEmitContext from plans and a function's locals.
fn make_semantic_context<'a>(
    plan: &'a LinuxBackendPlan,
    _portable: &'a DriverLoweringPlan,
    snake: &'a str,
    body: &'a SemanticBody,
) -> SemanticEmitContext<'a> {
    let reg_prefix = plan
        .register_layout
        .blocks
        .first()
        .map(|b| b.reg_prefix.as_str())
        .unwrap_or("REG");

    SemanticEmitContext {
        snake,
        upper: snake.to_uppercase(),
        reg_prefix,
        register_layout: &plan.register_layout,
        model: &plan.behavior,
        locals: &body.locals,
    }
}

/// Find a FunctionDef by role in the semantic model.
fn find_function_by_role<'a>(
    model: &'a SemanticModel,
    role: &FunctionRole,
) -> Option<&'a regseq_model::item::function::FunctionDef> {
    model.functions.iter().find(|f| &f.role == role)
}

/// Emit helper functions from the semantic model.
///
/// Emits forward declarations first (from the same source as definitions)
/// to ensure helpers can be called before their definition.
fn emit_helper_functions(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    emit_helper_functions_semantic(tu, plan, portable, snake);
}

/// Emit helper functions from the semantic model (canonical path).
fn emit_helper_functions_semantic(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    // Collect helper function defs from the semantic model via the behavior mapping.
    let helper_names: Vec<&str> = plan
        .behavior_mapping
        .callback_mappings
        .iter()
        .filter(|m| m.group == LinuxCallbackGroup::Helper)
        .map(|m| m.portable_function.as_str())
        .collect();

    let helpers: Vec<&regseq_model::item::function::FunctionDef> = plan
        .behavior
        .functions
        .iter()
        .filter(|f| helper_names.contains(&f.name.as_str()))
        .collect();

    // Emit forward declarations.
    if !helpers.is_empty() {
        tu.push(CTopLevelItem::LineComment(
            "Helper forward declarations".into(),
        ));
        for func in &helpers {
            let ret_ty = semantic_type_to_c(&func.body.return_type, snake);
            let param_str = build_semantic_param_list(snake, &func.body);
            tu.push(CTopLevelItem::Raw(format!(
                "static {ret_ty} {snake}_{}({param_str});",
                func.name
            )));
        }
        tu.push(CTopLevelItem::BlankLine);
    }

    // Emit function bodies.
    for func in &helpers {
        let body = &func.body;
        let ctx = make_semantic_context(plan, portable, snake, body);
        let all_param_ids: HashSet<LocalId> = body.params.iter().copied().collect();
        let body_lines = emit_body_stmts(body, &ctx, &all_param_ids, &ReturnMode::Direct);
        let ret_ty = semantic_type_to_c(&body.return_type, snake);
        let param_str = build_semantic_param_list(snake, body);
        tu.push(CTopLevelItem::FuncDef {
            signature: format!("static {ret_ty} {snake}_{}({param_str})", func.name),
            body: body_lines,
            comment: Some(format!("Helper: {}", func.name)),
        });
        tu.push(CTopLevelItem::BlankLine);
    }
}

/// Build the C parameter list for a function from its SemanticBody.
///
/// The self parameter (DriverData) is always `struct {snake}_data *d`.
/// Additional authored parameters use their semantic local names and types.
fn build_semantic_param_list(snake: &str, body: &SemanticBody) -> String {
    use regseq_model::semantic::types::SemanticType;

    let mut parts = Vec::new();
    for &param_id in &body.params {
        let local = &body.locals[param_id.0 as usize];
        if matches!(local.ty, SemanticType::DriverData) {
            parts.push(format!("struct {snake}_data *d"));
        } else {
            let c_ty = semantic_type_to_c(&local.ty, snake);
            parts.push(format!("{c_ty} {}", local.name));
        }
    }
    if parts.is_empty() {
        "void".into()
    } else {
        parts.join(", ")
    }
}

/// Emit ISR handler from the semantic model.
fn emit_isr_handler(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    emit_isr_handler_semantic(tu, plan, portable, snake);
}

/// Emit ISR handler from the semantic model (canonical path).
fn emit_isr_handler_semantic(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    // Find ISR function via FunctionRole::Isr { .. }.
    let isr_func = plan
        .behavior
        .functions
        .iter()
        .find(|f| matches!(&f.role, FunctionRole::Isr { .. }));

    let body = if let Some(func) = isr_func {
        let semantic_body = &func.body;
        let ctx = make_semantic_context(plan, portable, snake, semantic_body);

        let mut lines = vec![format!("struct {snake}_data *d = dev_id;")];

        // Find the ISR status register by walking Stmt/Expr trees for first RegReadRaw.
        if let Some(reg_name) = find_first_semantic_reg_read_raw(&semantic_body.body) {
            let status_define = register_offset_define(ctx.reg_prefix, &reg_name);
            lines.push("u32 status;".into());
            lines.push(String::new());
            lines.push(format!("status = {snake}_reg_read(d, {status_define});"));
            lines.push("if (status == 0)".into());
            lines.push("\treturn IRQ_NONE;".into());
            lines.push(String::new());
        }

        // Emit the full semantic body with all params excluded (self + dev_id).
        let all_param_ids: HashSet<LocalId> = semantic_body.params.iter().copied().collect();
        let body_lines = emit_body_stmts(semantic_body, &ctx, &all_param_ids, &ReturnMode::Direct);
        lines.extend(body_lines);

        // If the body didn't emit a return, add one.
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

/// Emit uart_ops callback functions from realization plan data.
fn emit_uart_ops(
    tu: &mut CTranslationUnit,
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    snake: &str,
) {
    for entry in &plan.uart_ops_realization {
        let body = match &entry.body_source {
            LinuxUartOpsBody::DirectBody { role } => {
                let func = find_function_by_role(&plan.behavior, role);
                if let Some(func) = func {
                    let semantic_body = &func.body;
                    let ctx = make_semantic_context(plan, portable, snake, semantic_body);
                    let mut lines = Vec::new();
                    if entry.needs_container_of {
                        lines.push(format!(
                            "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
                        ));
                        lines.push(String::new());
                    }
                    let all_param_ids: HashSet<LocalId> =
                        semantic_body.params.iter().copied().collect();
                    lines.extend(emit_body_stmts(
                        semantic_body,
                        &ctx,
                        &all_param_ids,
                        &ReturnMode::Direct,
                    ));
                    lines
                } else {
                    emit_stub_body(entry, snake)
                }
            }
            LinuxUartOpsBody::WrappedBody {
                role,
                param_bindings,
                return_adaptation,
            } => {
                let func = find_function_by_role(&plan.behavior, role);
                if let Some(func) = func {
                    let semantic_body = &func.body;
                    let ctx = make_semantic_context(plan, portable, snake, semantic_body);
                    let mut lines = Vec::new();
                    if entry.needs_container_of {
                        lines.push(format!(
                            "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
                        ));
                    }

                    // Emit parameter bindings.
                    for binding in param_bindings {
                        lines.push(format!(
                            "{} {} = {};",
                            binding.c_type, binding.semantic_name, binding.c_expr
                        ));
                    }
                    if entry.needs_container_of || !param_bindings.is_empty() {
                        lines.push(String::new());
                    }

                    // Determine return mode from adaptation.
                    let return_mode = match return_adaptation {
                        Some(ReturnAdaptation::BoolToUint {
                            true_value,
                            false_value,
                        }) => ReturnMode::BoolToUint {
                            true_value: true_value.clone(),
                            false_value: false_value.clone(),
                        },
                        Some(ReturnAdaptation::ResultToInt) => ReturnMode::ResultToInt,
                        Some(ReturnAdaptation::DropResult) => ReturnMode::DropResult,
                        None => ReturnMode::Direct,
                    };

                    let all_param_ids: HashSet<LocalId> =
                        semantic_body.params.iter().copied().collect();
                    lines.extend(emit_body_stmts(
                        semantic_body,
                        &ctx,
                        &all_param_ids,
                        &return_mode,
                    ));

                    // For ResultToInt: append trailing "return 0;" if body didn't
                    // already emit a return.
                    if matches!(return_adaptation, Some(ReturnAdaptation::ResultToInt))
                        && !entry.trailing.is_empty()
                        && !lines.iter().any(|l| l.trim_start().starts_with("return"))
                    {
                        lines.push(String::new());
                        lines.extend(entry.trailing.iter().cloned());
                    }
                    lines
                } else {
                    emit_stub_body(entry, snake)
                }
            }
            LinuxUartOpsBody::Stub { lines } => {
                let mut body = Vec::new();
                if entry.needs_container_of {
                    body.push(format!(
                        "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
                    ));
                    // Suppress unused-variable warning for stubs.
                    if lines
                        .iter()
                        .all(|l| l.starts_with("/*") || l.starts_with("//"))
                    {
                        body.push("(void)d;".into());
                    }
                    body.push(String::new());
                }
                body.extend(lines.iter().cloned());
                body
            }
        };

        tu.push(CTopLevelItem::FuncDef {
            signature: entry.signature.clone(),
            body,
            comment: None,
        });
        tu.push(CTopLevelItem::BlankLine);
    }

    // uart_ops struct.
    tu.push(CTopLevelItem::LineComment("uart_ops definition".into()));
    let serial = plan.serial_core.as_ref().unwrap();
    let initializer: Vec<String> = plan
        .uart_ops_realization
        .iter()
        .map(|entry| {
            let field = &entry.op_name;
            // Extract the function name from the signature.
            let func_name = entry
                .signature
                .split('(')
                .next()
                .and_then(|s| s.split_whitespace().last())
                .unwrap_or(field);
            if field == "type" {
                format!(".type\t\t= {func_name},")
            } else if field.len() < 8 {
                format!(".{field}\t\t= {func_name},")
            } else {
                format!(".{field}\t= {func_name},")
            }
        })
        .collect();

    tu.push(CTopLevelItem::StaticDef {
        qualifiers: "static const".into(),
        type_name: "struct uart_ops".into(),
        name: serial.uart_ops_name.clone(),
        initializer,
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
// Semantic model helpers
// ---------------------------------------------------------------------------

/// Generate a stub body for an unrealized uart_ops entry.
fn emit_stub_body(entry: &super::plan::LinuxUartOpsEntry, snake: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if entry.needs_container_of {
        lines.push(format!(
            "struct {snake}_data *d = container_of(port, struct {snake}_data, port);"
        ));
        lines.push("(void)d;".into());
    }
    lines.extend(entry.trailing.iter().cloned());
    lines
}

/// Walk Stmt/Expr trees to find the first `Expr::RegReadRaw` register name.
fn find_first_semantic_reg_read_raw(block: &regseq_model::semantic::body::Block) -> Option<String> {
    for stmt in &block.stmts {
        if let Some(name) = find_reg_read_raw_in_stmt(stmt) {
            return Some(name);
        }
    }
    None
}

/// Recursively search a statement for an `Expr::RegReadRaw`.
fn find_reg_read_raw_in_stmt(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::Let { value, .. } => find_reg_read_raw_in_expr(value),
        Stmt::Assign { value, .. } => find_reg_read_raw_in_expr(value),
        Stmt::CompoundAssign { value, .. } => find_reg_read_raw_in_expr(value),
        Stmt::Eval { expr } => find_reg_read_raw_in_expr(expr),
        Stmt::Branch {
            condition,
            then_body,
            else_body,
        } => find_reg_read_raw_in_expr(condition)
            .or_else(|| find_first_semantic_reg_read_raw(then_body))
            .or_else(|| find_first_semantic_reg_read_raw(else_body)),
        Stmt::Loop { condition, body } => {
            find_reg_read_raw_in_expr(condition).or_else(|| find_first_semantic_reg_read_raw(body))
        }
        Stmt::OrderedRegion { body } => find_first_semantic_reg_read_raw(body),
        Stmt::FallibleOp {
            body,
            cleanup_on_error,
            ..
        } => find_first_semantic_reg_read_raw(body)
            .or_else(|| find_first_semantic_reg_read_raw(cleanup_on_error)),
        Stmt::Return { value } => value.as_ref().and_then(find_reg_read_raw_in_expr),
        Stmt::RegWrite { .. } | Stmt::RegModify { .. } => None,
        Stmt::ResourceOp { .. } => None,
        Stmt::FieldAssign { value, .. } => find_reg_read_raw_in_expr(value),
        Stmt::SubsystemAction { args, .. } => {
            for arg in args {
                if let Some(name) = find_reg_read_raw_in_expr(arg) {
                    return Some(name);
                }
            }
            None
        }
    }
}

/// Recursively search an expression for an `Expr::RegReadRaw`.
fn find_reg_read_raw_in_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::RegReadRaw { register } => Some(register.name.clone()),
        Expr::BinaryOp { lhs, rhs, .. } => {
            find_reg_read_raw_in_expr(lhs).or_else(|| find_reg_read_raw_in_expr(rhs))
        }
        Expr::UnaryOp { operand, .. } => find_reg_read_raw_in_expr(operand),
        Expr::Cast { expr, .. } => find_reg_read_raw_in_expr(expr),
        Expr::RegFieldRead { .. } => None,
        Expr::FieldExtract { raw_value, .. } => find_reg_read_raw_in_expr(raw_value),
        Expr::HelperCall { args, .. } => {
            for arg in args {
                if let Some(name) = find_reg_read_raw_in_expr(arg) {
                    return Some(name);
                }
            }
            None
        }
        Expr::WithDefault { value, default } => {
            find_reg_read_raw_in_expr(value).or_else(|| find_reg_read_raw_in_expr(default))
        }
        Expr::InterruptClassTest { status, .. } => find_reg_read_raw_in_expr(status),
        Expr::IsSome { inner } | Expr::OptionSome { inner } | Expr::OptionUnwrap { inner } => {
            find_reg_read_raw_in_expr(inner)
        }
        _ => None,
    }
}
