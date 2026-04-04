//! Linux backend emitter.
//!
//! Consumes a `LinuxBackendPlan` (and a `DriverLoweringPlan` for
//! supplementary details) to produce the generated Linux artifact set.
//! Returns a typed `EmittedArtifactSet` describing every file written.

use std::fs;
use std::path::{Path, PathBuf};

use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{
    generated_file_banner, include_guard_name, CStructField, CTopLevelItem, CTranslationUnit,
};
use super::plan::{LinuxBackendPlan, LinuxFileRole, LinuxProbeStep, LinuxRemoveStep};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The complete set of artifacts emitted by the Linux backend.
#[derive(Debug, Clone)]
pub struct EmittedArtifactSet {
    /// Root directory where artifacts were written.
    pub output_root: PathBuf,
    /// Driver identifier used for this emission.
    pub driver_id: String,
    /// Individual emitted artifacts.
    pub artifacts: Vec<EmittedArtifact>,
}

/// A single emitted artifact file.
#[derive(Debug, Clone)]
pub struct EmittedArtifact {
    /// Absolute path to the emitted file.
    pub path: PathBuf,
    /// Role of this file in the generated module.
    pub role: LinuxFileRole,
    /// Size in bytes of the written file.
    pub size_bytes: usize,
}

/// Errors during Linux artifact emission.
#[derive(Debug, thiserror::Error)]
pub enum LinuxEmitError {
    /// I/O error while writing artifacts.
    #[error("emission I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Plan is missing information needed for emission.
    #[error("emission plan error: {reason}")]
    PlanError {
        /// Description of the missing information.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Emit Linux artifacts from a backend plan into the given output directory.
///
/// This is the primary Linux backend emission entry point. It writes all
/// files declared in `plan.artifact_layout` and returns a typed manifest
/// of what was emitted.
///
/// The `portable` plan provides supplementary details (register blocks,
/// resource obligations) that the Linux plan references but does not
/// duplicate.
pub fn emit_linux_artifacts(
    plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
    output_root: &Path,
) -> Result<EmittedArtifactSet, LinuxEmitError> {
    fs::create_dir_all(output_root)?;

    let mut artifacts = Vec::new();

    for file in &plan.artifact_layout.emitted_files {
        let content = match file.role {
            LinuxFileRole::DriverMain => emit_module_integration(plan, portable),
            LinuxFileRole::SharedHeader => emit_shared_header(plan, portable),
            LinuxFileRole::RegisterHelpers => emit_register_helpers(plan, portable),
            LinuxFileRole::Makefile => emit_makefile(plan),
            LinuxFileRole::KbuildFragment => emit_kbuild(plan),
        };

        let path = output_root.join(&file.filename);
        fs::write(&path, &content)?;

        artifacts.push(EmittedArtifact {
            path,
            role: file.role.clone(),
            size_bytes: content.len(),
        });
    }

    Ok(EmittedArtifactSet {
        output_root: output_root.to_path_buf(),
        driver_id: portable.identity.driver_name.clone(),
        artifacts,
    })
}

// ---------------------------------------------------------------------------
// Per-file emitters
// ---------------------------------------------------------------------------

/// Emit the module integration file (backend-owned registration scaffold).
fn emit_module_integration(plan: &LinuxBackendPlan, portable: &DriverLoweringPlan) -> String {
    let reg = &plan.registration;
    let snake = &reg.module_name;
    let driver_name = &portable.identity.driver_name;

    let mut tu = CTranslationUnit::new();

    // Banner
    let filename = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::DriverMain)
        .map(|f| f.filename.as_str())
        .unwrap_or("module.c");
    tu.push(generated_file_banner(filename, driver_name));
    tu.push(CTopLevelItem::BlankLine);

    // Includes
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
    }
    tu.push(CTopLevelItem::Include {
        path: "linux/io.h".into(),
        system: true,
    });

    // Local shared header
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
    tu.push(CTopLevelItem::BlankLine);

    // Probe function
    emit_probe_function(&mut tu, plan, snake);
    tu.push(CTopLevelItem::BlankLine);

    // Remove function
    emit_remove_function(&mut tu, plan, snake);
    tu.push(CTopLevelItem::BlankLine);

    // Match table and registration
    if reg.is_amba {
        emit_amba_registration(&mut tu, plan, snake);
    } else {
        emit_platform_registration(&mut tu, plan, snake);
    }
    tu.push(CTopLevelItem::BlankLine);

    // Module metadata
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

/// Emit the shared header with driver state struct and forward declarations.
fn emit_shared_header(plan: &LinuxBackendPlan, portable: &DriverLoweringPlan) -> String {
    let reg = &plan.registration;
    let snake = &reg.module_name;
    let driver_name = &portable.identity.driver_name;

    let mut tu = CTranslationUnit::new();

    let filename = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::SharedHeader)
        .map(|f| f.filename.as_str())
        .unwrap_or("shared.h");
    let guard = include_guard_name(filename);

    tu.push(generated_file_banner(filename, driver_name));
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::IncludeGuardOpen(guard.clone()));
    tu.push(CTopLevelItem::BlankLine);

    // Driver private data struct
    let mut fields = Vec::new();
    for mmio in &portable.resources.mmio {
        fields.push(CStructField {
            type_name: "void __iomem *".into(),
            field_name: mmio.field_name.clone(),
            doc: Some(format!("MMIO base for {}", mmio.mmio_id)),
        });
    }
    for irq in &portable.resources.interrupts {
        fields.push(CStructField {
            type_name: "int".into(),
            field_name: irq.field_name.clone(),
            doc: Some(format!("IRQ number for {}", irq.interrupt_id)),
        });
    }
    for clk in &portable.resources.clocks {
        fields.push(CStructField {
            type_name: "struct clk *".into(),
            field_name: clk.field_name.clone(),
            doc: Some(format!("Clock {}", clk.clock_id)),
        });
    }
    if plan.serial_core.is_some() {
        fields.push(CStructField {
            type_name: "struct uart_port".into(),
            field_name: "port".into(),
            doc: Some("Serial core port".into()),
        });
    }

    tu.push(CTopLevelItem::StructDef {
        name: format!("{snake}_data"),
        fields,
    });
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::IncludeGuardClose(guard));

    tu.render()
}

/// Emit the register helpers header.
fn emit_register_helpers(_plan: &LinuxBackendPlan, portable: &DriverLoweringPlan) -> String {
    let snake = to_snake_case(&portable.identity.driver_name);
    let driver_name = &portable.identity.driver_name;

    let mut tu = CTranslationUnit::new();

    let filename = format!("{snake}_regs.h");
    let guard = include_guard_name(&filename);

    tu.push(generated_file_banner(&filename, driver_name));
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::IncludeGuardOpen(guard.clone()));
    tu.push(CTopLevelItem::BlankLine);

    tu.push(CTopLevelItem::LineComment(
        "Register block definitions".into(),
    ));
    tu.push(CTopLevelItem::BlankLine);

    for block in &portable.register_obligations.blocks {
        tu.push(CTopLevelItem::LineComment(format!(
            "Block: {} ({} registers)",
            block.name, block.field_count
        )));
        tu.push(CTopLevelItem::Raw(format!(
            "/* TODO: register offset definitions for {} */",
            block.name
        )));
        tu.push(CTopLevelItem::BlankLine);
    }

    tu.push(CTopLevelItem::IncludeGuardClose(guard));

    tu.render()
}

/// Emit the out-of-tree Makefile.
fn emit_makefile(plan: &LinuxBackendPlan) -> String {
    let module_obj = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::DriverMain)
        .map(|f| f.filename.replace(".c", ".o"))
        .unwrap_or_else(|| "module.o".into());

    format!(
        "# Regseq-generated out-of-tree module build\n\
         #\n\
         # Usage:\n\
         #   make -C <kernel-build-dir> M=$(PWD) modules\n\
         #   make -C <kernel-build-dir> M=$(PWD) modules \\\n\
         #       ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu-\n\
         \n\
         KDIR ?= /lib/modules/$(shell uname -r)/build\n\
         \n\
         obj-m := {module_obj}\n\
         \n\
         all:\n\
         \t$(MAKE) -C $(KDIR) M=$(PWD) modules\n\
         \n\
         clean:\n\
         \t$(MAKE) -C $(KDIR) M=$(PWD) clean\n"
    )
}

/// Emit the Kbuild fragment.
fn emit_kbuild(plan: &LinuxBackendPlan) -> String {
    let module_obj = plan
        .artifact_layout
        .emitted_files
        .iter()
        .find(|f| f.role == LinuxFileRole::DriverMain)
        .map(|f| f.filename.replace(".c", ".o"))
        .unwrap_or_else(|| "module.o".into());

    format!("obj-m := {module_obj}\n")
}

// ---------------------------------------------------------------------------
// Sub-emitters for the module integration file
// ---------------------------------------------------------------------------

fn emit_probe_function(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let mut body = Vec::new();

    // Skeleton probe showing resource acquisition order.
    for step in &plan.probe.steps {
        match step {
            LinuxProbeStep::GetClock { clock_id, optional } => {
                let opt = if *optional { " (optional)" } else { "" };
                body.push(format!(
                    "/* TODO: devm_clk_get + clk_prepare_enable \"{clock_id}\"{opt} */"
                ));
            }
            LinuxProbeStep::DeassertReset {
                field_name,
                optional,
            } => {
                let opt = if *optional { " (optional)" } else { "" };
                body.push(format!(
                    "/* TODO: devm_reset_control_get + deassert \"{field_name}\"{opt} */"
                ));
            }
            LinuxProbeStep::SelectPinCtrl {
                field_name,
                optional,
            } => {
                let opt = if *optional { " (optional)" } else { "" };
                body.push(format!(
                    "/* TODO: devm_pinctrl_get + select \"{field_name}\"{opt} */"
                ));
            }
            LinuxProbeStep::MapMmio { mmio_id } => {
                body.push(format!("/* TODO: devm_ioremap_resource \"{mmio_id}\" */"));
            }
            LinuxProbeStep::ReadProperty {
                property_id,
                has_default,
            } => {
                let def = if *has_default { " (has default)" } else { "" };
                body.push(format!(
                    "/* TODO: device_property_read \"{property_id}\"{def} */"
                ));
            }
            LinuxProbeStep::RequestIrq { interrupt_id } => {
                body.push(format!("/* TODO: devm_request_irq \"{interrupt_id}\" */"));
            }
            LinuxProbeStep::RegisterUartPort => {
                body.push("/* TODO: uart_add_one_port */".into());
            }
        }
    }
    body.push(String::new());
    body.push("return 0;".into());

    if plan.registration.is_amba {
        tu.push(CTopLevelItem::FuncDef {
            signature: format!(
                "static int {snake}_probe(struct amba_device *adev, const struct amba_id *id)"
            ),
            body,
            comment: Some("probe -- Regseq lifecycle::init".into()),
        });
    } else {
        tu.push(CTopLevelItem::FuncDef {
            signature: format!("static int {snake}_probe(struct platform_device *pdev)"),
            body,
            comment: Some("probe -- Regseq lifecycle::init".into()),
        });
    }
}

fn emit_remove_function(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let mut body = Vec::new();

    for step in &plan.remove.steps {
        match step {
            LinuxRemoveStep::UnregisterUartPort => {
                body.push("/* TODO: uart_remove_one_port */".into());
            }
            LinuxRemoveStep::FreeIrq { interrupt_id } => {
                body.push(format!("/* TODO: free_irq \"{interrupt_id}\" */"));
            }
            LinuxRemoveStep::PutClock { clock_id } => {
                body.push(format!("/* TODO: clk_disable_unprepare \"{clock_id}\" */"));
            }
            LinuxRemoveStep::AssertReset { field_name } => {
                body.push(format!("/* TODO: reset_control_assert \"{field_name}\" */"));
            }
            LinuxRemoveStep::UnmapMmio { mmio_id } => {
                body.push(format!("/* TODO: iounmap \"{mmio_id}\" (devm) */"));
            }
        }
    }

    if plan.registration.is_amba {
        tu.push(CTopLevelItem::FuncDef {
            signature: format!("static void {snake}_remove(struct amba_device *adev)"),
            body,
            comment: Some("remove -- Regseq lifecycle::deinit".into()),
        });
    } else {
        tu.push(CTopLevelItem::FuncDef {
            signature: format!("static void {snake}_remove(struct platform_device *pdev)"),
            body,
            comment: Some("remove -- Regseq lifecycle::deinit".into()),
        });
    }
}

fn emit_amba_registration(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let reg = &plan.registration;

    // AMBA match table
    let mut init_lines = Vec::new();
    if let (Some(id), Some(mask)) = (reg.amba_id, reg.amba_mask) {
        init_lines.push(format!("{{ .id = {id:#010x}, .mask = {mask:#010x} }},"));
    } else {
        init_lines.push("/* TODO: AMBA ID and mask */".into());
        init_lines.push("{ 0 },".into());
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

    // AMBA driver struct
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
    tu.push(CTopLevelItem::Raw(format!(
        "module_amba_driver({});",
        reg.driver_struct_name
    )));
}

fn emit_platform_registration(tu: &mut CTranslationUnit, plan: &LinuxBackendPlan, snake: &str) {
    let reg = &plan.registration;

    // OF match table
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

    // Platform driver struct
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
    tu.push(CTopLevelItem::Raw(format!(
        "module_platform_driver({});",
        reg.driver_struct_name
    )));
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}
