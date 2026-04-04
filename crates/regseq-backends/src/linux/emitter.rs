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
use super::emit_module;
use super::emit_regs;
use super::plan::{LinuxBackendPlan, LinuxFileRole};

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
            LinuxFileRole::DriverMain => emit_module::emit_module_integration(plan, portable),
            LinuxFileRole::SharedHeader => emit_shared_header(plan, portable),
            LinuxFileRole::RegisterHelpers => emit_regs::emit_register_helpers(plan, portable),
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
// Shared header emission
// ---------------------------------------------------------------------------

/// Emit the shared header with driver state struct and forward declarations.
pub(super) fn emit_shared_header(plan: &LinuxBackendPlan, portable: &DriverLoweringPlan) -> String {
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

    // Kernel type includes
    tu.push(CTopLevelItem::Include {
        path: "linux/types.h".into(),
        system: true,
    });
    if plan.serial_core.is_some() {
        tu.push(CTopLevelItem::Include {
            path: "linux/serial_core.h".into(),
            system: true,
        });
    }
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
    for prop in &portable.resources.properties {
        let ctype = match prop.field_type.as_str() {
            "u32" => "u32",
            "u16" => "u16",
            "u8" => "u8",
            "bool" => "bool",
            _ => "u32",
        };
        fields.push(CStructField {
            type_name: ctype.into(),
            field_name: prop.field_name.clone(),
            doc: Some(format!("Property: {}", prop.property_id)),
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

// ---------------------------------------------------------------------------
// Build file emission
// ---------------------------------------------------------------------------

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

pub(super) fn to_snake_case(name: &str) -> String {
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
