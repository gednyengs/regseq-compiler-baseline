use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{ColorChoice, DumpStage, EmitSpec, LogLevel, RegseqParam};

/// Complete compiler configuration passed from `cargo-regseq` to
/// `regseq-driver` via the `REGSEQ_OPTIONS` JSON environment variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerOptions {
    /// Platform file path. The `[platform]` table within is the single source of
    /// target identity (os/board/arch) — D4/D6; there is no `target_os` field.
    pub platform_file: PathBuf,
    /// Output directory for generated artifacts.
    pub output_dir: PathBuf,
    /// Emit directives.
    pub emit: Vec<EmitSpec>,
    /// Run in check-only mode.
    pub check: bool,
    /// Analyses to force-enable.
    pub add_analysis: Vec<String>,
    /// Analyses to skip.
    pub skip_analysis: Vec<String>,
    /// Denied diagnostics.
    pub deny: Vec<String>,
    /// Allowed diagnostics.
    pub allow: Vec<String>,
    /// Arbitrary namespaced parameters.
    pub params: Vec<RegseqParam>,
    /// Log verbosity.
    pub log_level: LogLevel,
    /// Optional stage to dump.
    pub dump_stage: Option<DumpStage>,
    /// Terminal color behavior.
    pub color: ColorChoice,
}
