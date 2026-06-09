use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Parse failure for shared CLI enums.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid value '{value}' for {type_name}; expected one of: {expected}")]
pub struct ParseEnumError {
    /// Raw input value.
    pub value: String,
    /// Enum name.
    pub type_name: &'static str,
    /// Comma-separated expected values.
    pub expected: &'static str,
}

// `TargetOs` was relocated to `regseq-model` (`regseq_model::TargetOs`) as part
// of making the platform file the single source of target identity (R5/D6/D8):
// it is consumed by `regseq-backends` (which depends on `regseq-model` only),
// and a platform-file os token is validated by `regseq-l2::resolve_platform_identity`.
// `regseq-config` no longer defines or parses it.

/// Log verbosity level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    /// Error diagnostics only.
    Error,
    /// Warnings and errors.
    #[default]
    Warn,
    /// Informational logs.
    Info,
    /// Debug logs.
    Debug,
    /// Trace-level logs.
    Trace,
}

impl FromStr for LogLevel {
    type Err = ParseEnumError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseEnumError {
                value: value.to_string(),
                type_name: "LogLevel",
                expected: "error, warn, info, debug, trace",
            }),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        })
    }
}

/// Pipeline stage for debug dumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DumpStage {
    /// High-level IR stage.
    Hir,
    /// Mid-level IR stage.
    Mir,
    /// Internal model stage.
    Model,
    /// After analysis passes.
    PostAnalysis,
    /// Before backend code generation.
    PreCodegen,
}

impl FromStr for DumpStage {
    type Err = ParseEnumError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "hir" => Ok(Self::Hir),
            "mir" => Ok(Self::Mir),
            "model" => Ok(Self::Model),
            "post-analysis" => Ok(Self::PostAnalysis),
            "pre-codegen" => Ok(Self::PreCodegen),
            _ => Err(ParseEnumError {
                value: value.to_string(),
                type_name: "DumpStage",
                expected: "hir, mir, model, post-analysis, pre-codegen",
            }),
        }
    }
}

impl fmt::Display for DumpStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hir => "hir",
            Self::Mir => "mir",
            Self::Model => "model",
            Self::PostAnalysis => "post-analysis",
            Self::PreCodegen => "pre-codegen",
        })
    }
}

/// Terminal color setting.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorChoice {
    /// Force colored output.
    Always,
    /// Automatically detect TTY support.
    #[default]
    Auto,
    /// Disable colored output.
    Never,
}

impl FromStr for ColorChoice {
    type Err = ParseEnumError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "always" => Ok(Self::Always),
            "auto" => Ok(Self::Auto),
            "never" => Ok(Self::Never),
            _ => Err(ParseEnumError {
                value: value.to_string(),
                type_name: "ColorChoice",
                expected: "always, auto, never",
            }),
        }
    }
}

impl fmt::Display for ColorChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Always => "always",
            Self::Auto => "auto",
            Self::Never => "never",
        })
    }
}
