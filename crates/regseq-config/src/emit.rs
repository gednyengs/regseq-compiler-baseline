use core::fmt;
use core::str::FromStr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Parsing errors for emit specifications.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EmitParseError {
    /// Unsupported emit type prefix.
    #[error("unknown emit type '{0}'; expected 'driver' or 'analysis'")]
    UnknownType(String),
}

/// What the compiler should produce as output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum EmitSpec {
    /// Generate C driver source files.
    Driver {
        /// Optional match ID for multi-driver selection (future).
        match_id: Option<String>,
    },
    /// Emit analysis results.
    Analysis {
        /// Specific analysis name, or None for aggregate report.
        analysis_name: Option<String>,
        /// Output path, or None for default location.
        path: Option<PathBuf>,
    },
    /// Emit the raw frontend artifact (pre-normalized extraction snapshot).
    RawArtifact,
}

impl FromStr for EmitSpec {
    type Err = EmitParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (type_spec, value) = match input.split_once('=') {
            Some((lhs, rhs)) => (lhs, Some(rhs)),
            None => (input, None),
        };

        let (base_type, subtype) = match type_spec.split_once('.') {
            Some((lhs, rhs)) => (lhs, Some(rhs)),
            None => (type_spec, None),
        };

        match base_type.to_ascii_lowercase().as_str() {
            "driver" => Ok(Self::Driver {
                match_id: value.map(ToString::to_string),
            }),
            "analysis" => Ok(Self::Analysis {
                analysis_name: subtype.map(ToString::to_string),
                path: value.map(PathBuf::from),
            }),
            "raw-artifact" => Ok(Self::RawArtifact),
            _ => Err(EmitParseError::UnknownType(base_type.to_string())),
        }
    }
}

impl fmt::Display for EmitSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver { match_id: None } => f.write_str("driver"),
            Self::Driver {
                match_id: Some(match_id),
            } => write!(f, "driver={match_id}"),
            Self::Analysis {
                analysis_name: None,
                path: None,
            } => f.write_str("analysis"),
            Self::Analysis {
                analysis_name: None,
                path: Some(path),
            } => write!(f, "analysis={}", path.to_string_lossy()),
            Self::Analysis {
                analysis_name: Some(name),
                path: None,
            } => write!(f, "analysis.{name}"),
            Self::Analysis {
                analysis_name: Some(name),
                path: Some(path),
            } => write!(f, "analysis.{name}={}", path.to_string_lossy()),
            Self::RawArtifact => f.write_str("raw-artifact"),
        }
    }
}
