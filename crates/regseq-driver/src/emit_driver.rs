//! Production driver emission pipeline.
//!
//! Orchestrates the full `EmitSpec::Driver` path: takes a raw frontend
//! artifact and compiler options, then drives model normalization, join,
//! analysis bootstrap, portable lowering, and OS-specific backend emission.
//!
//! This module is an orchestrator. It does not re-implement lowering or
//! backend logic — it calls existing crate APIs in the canonical order.
//!
//! The raw artifact is serialized to JSON and re-normalized through
//! `normalize_from_json`. This is intentional: `normalize_from_json` is the
//! canonical public entry point for constructing a `DeclarationModel` from
//! extraction output, serving as the production-path bridge between the
//! extraction and model layers.

use regseq_analysis::AnalysisInput;
use regseq_backends::linux::emitter::{emit_linux_artifacts, EmittedArtifactSet};
use regseq_backends::linux::planner::build_linux_backend_plan;
use regseq_backends::lowering::planner::build_driver_lowering_plan;
use regseq_config::{CompilerOptions, TargetOs};
use regseq_model::binding;
use regseq_model::declaration;
use regseq_model::joined::perform_join;
use regseq_model::platform_parser;

/// Result of a successful driver emission.
pub struct EmitDriverResult {
    /// The set of artifacts written to disk.
    pub artifact_set: EmittedArtifactSet,
}

/// Errors that can occur during driver emission.
#[derive(Debug, thiserror::Error)]
pub enum EmitDriverError {
    /// Raw artifact could not be serialized to JSON for normalization.
    #[error("raw artifact serialization failed: {0}")]
    Serialization(String),
    /// Declaration normalization from JSON failed.
    #[error("declaration normalization failed: {0}")]
    Normalization(String),
    /// Platform file could not be read or parsed.
    #[error("platform file error: {0}")]
    PlatformFile(String),
    /// Binding normalization failed.
    #[error("binding normalization failed: {0}")]
    Binding(String),
    /// Declaration/binding join failed.
    #[error("join failed: {0}")]
    Join(String),
    /// Portable lowering plan construction failed.
    #[error("portable lowering failed: {0}")]
    Lowering(#[from] regseq_backends::lowering::planner::LoweringPlanError),
    /// Linux backend plan construction failed.
    #[error("linux backend planning failed: {0}")]
    LinuxPlan(#[from] regseq_backends::linux::planner::LinuxPlanError),
    /// Linux artifact emission failed.
    #[error("linux emission failed: {0}")]
    LinuxEmit(#[from] regseq_backends::linux::emitter::LinuxEmitError),
    /// Target OS is not yet supported for driver emission.
    #[error("unsupported target OS for driver emission: {0:?}")]
    UnsupportedOs(TargetOs),
}

/// Run the production driver emission pipeline.
///
/// Constructs the analysis input from the raw frontend artifact and
/// platform file, invokes portable lowering, then routes to the
/// appropriate backend emitter based on target OS.
pub fn emit_driver(
    raw_artifact: &crate::raw_artifact::RawFrontendArtifact,
    options: &CompilerOptions,
) -> Result<EmitDriverResult, EmitDriverError> {
    // 1. Serialize raw artifact to JSON (bridge to model normalizer).
    let json = serde_json::to_string(raw_artifact)
        .map_err(|e| EmitDriverError::Serialization(e.to_string()))?;

    // 2. Normalize to declaration model.
    let decl = declaration::normalize_from_json(&json)
        .map_err(|e| EmitDriverError::Normalization(e.to_string()))?;

    // 3. Parse platform file.
    let pf = platform_parser::parse_platform_file_path(&options.platform_file)
        .map_err(|e| EmitDriverError::PlatformFile(e.to_string()))?;

    // 4. Normalize binding with Layer 1 bus type for bus-consistency check.
    let platform_source = options.platform_file.to_string_lossy();
    let declaration_bus = decl.driver.bus.as_ref();
    let binding_model = binding::normalize(&pf, Some(&platform_source), declaration_bus)
        .map_err(|e| EmitDriverError::Binding(e.to_string()))?;

    // 5. Join declaration and binding.
    let bus_binding = binding_model.bus.clone();
    let joined =
        perform_join(&decl, &binding_model).map_err(|e| EmitDriverError::Join(e.to_string()))?;

    // 6. Bootstrap analysis input.
    let input = AnalysisInput::new(decl, joined, bus_binding);

    // 7. Portable lowering (default path — no speculative fallback).
    eprintln!("regseq-driver: building portable lowering plan...");
    let lowering_plan = build_driver_lowering_plan(&input)?;

    // 8. Route to target-OS backend.
    match options.target_os {
        TargetOs::Linux => {
            eprintln!("regseq-driver: building Linux backend plan...");
            let linux_plan = build_linux_backend_plan(&lowering_plan)?;

            eprintln!(
                "regseq-driver: emitting Linux artifacts to {}...",
                options.output_dir.display()
            );
            let artifact_set =
                emit_linux_artifacts(&linux_plan, &lowering_plan, &options.output_dir)?;

            Ok(EmitDriverResult { artifact_set })
        }
        other => Err(EmitDriverError::UnsupportedOs(other)),
    }
}
