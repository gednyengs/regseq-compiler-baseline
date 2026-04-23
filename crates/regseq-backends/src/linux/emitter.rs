//! Linux backend emitter.
//!
//! Consumes a `LinuxBackendPlan` together with a borrowed
//! [`FunctionBodyIndex`] of CFG bodies keyed by `FunctionId`. The Linux
//! plan is self-sufficient for emission -- no field on the portable
//! `DriverLoweringPlan` is read here; all Linux realization decisions
//! are carried on the plan itself.
//!
//! Returns a typed [`EmittedArtifactSet`] describing every file
//! written.

use std::fs;
use std::path::{Path, PathBuf};

use super::body_index::FunctionBodyIndex;
use crate::c::{include_guard_name, CStructField, CTopLevelItem, CTranslationUnit};
use super::conventions::generated_file_banner;
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
/// `bodies` is a borrowed function-body index keyed by `FunctionId`.
/// Phase-7B threads the index through the emission signature so tasks
/// 9/10 can retarget emitters to walk `ir::Body` directly without
/// another signature churn; the current emitters still string-format
/// the transitional `SemanticModel` handle carried on `plan.behavior`.
pub fn emit_linux_artifacts(
    plan: &LinuxBackendPlan,
    bodies: &FunctionBodyIndex<'_>,
    output_root: &Path,
) -> Result<EmittedArtifactSet, LinuxEmitError> {
    let _ = bodies;
    fs::create_dir_all(output_root)?;

    let mut artifacts = Vec::new();

    for file in &plan.artifact_layout.emitted_files {
        let content = match file.role {
            LinuxFileRole::DriverMain => emit_module::emit_module_integration(plan),
            LinuxFileRole::SharedHeader => emit_shared_header(plan),
            LinuxFileRole::RegisterHelpers => emit_regs::emit_register_helpers(plan),
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
        driver_id: plan.driver_name.clone(),
        artifacts,
    })
}

// ---------------------------------------------------------------------------
// Shared header emission
// ---------------------------------------------------------------------------

/// Emit the shared header with driver state struct and forward declarations.
pub(super) fn emit_shared_header(plan: &LinuxBackendPlan) -> String {
    let reg = &plan.registration;
    let snake = &reg.module_name;
    let driver_name = &plan.driver_name;

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

    // Driver private data struct (pre-computed by the Linux planner).
    let fields = plan
        .shared_struct_fields
        .iter()
        .map(|f| CStructField {
            type_name: f.c_type.clone(),
            field_name: f.field_name.clone(),
            doc: Some(f.doc.clone()),
        })
        .collect();

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
        if ch == '-' {
            result.push('_');
        } else if ch.is_uppercase() {
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
