use core::fmt;
use core::str::FromStr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Parsing errors for emit specifications.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EmitParseError {
    /// Unsupported emit type prefix.
    #[error(
        "unknown emit type '{0}'; expected 'driver', 'analysis', 'model', 'json', or 'generic-c'"
    )]
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
    /// Emit the pretty-printed textual `RegseqModel` per
    /// `pretty-printer.md` section 10.3.
    ///
    /// The compiler builds a `RegseqModel` from rustc HIR/MIR and
    /// renders it via `regseq_model::PrettyPrinter::new(&model).render()`
    /// into a single text file.
    Model {
        /// Output path, or None for default location under `output_dir`.
        path: Option<PathBuf>,
    },
    /// Emit the normalized `RegseqModel` as JSON via
    /// `serde_json::to_writer_pretty`.
    ///
    /// The compiler builds a `RegseqModel` from rustc HIR/MIR and
    /// writes its JSON form (using `RegseqModel`'s `Serialize`
    /// derive at `crates/regseq-model/src/model.rs:53`) to the
    /// output tree.
    Json {
        /// Output path, or None for default location under `output_dir`.
        path: Option<PathBuf>,
    },
    /// Emit the generic-C translation unit via
    /// `regseq_backends::emit_generic_c(&model)`.
    ///
    /// The compiler builds a `RegseqModel` from rustc HIR/MIR and
    /// lowers it to a single host-compilable C translation unit. The
    /// serde tag is `"generic-c"` (kebab-case of the variant name).
    GenericC {
        /// Output path, or None for default location under `output_dir`.
        path: Option<PathBuf>,
    },
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
            "model" => Ok(Self::Model {
                path: value.map(PathBuf::from),
            }),
            // The `json` emit spec has no subtypes; reject
            // `json.<anything>` explicitly so a typo does not silently
            // succeed. The legacy `model.<anything>` leniency is
            // preserved above for backward compatibility.
            "json" if subtype.is_none() => Ok(Self::Json {
                path: value.map(PathBuf::from),
            }),
            "json" => Err(EmitParseError::UnknownType(type_spec.to_string())),
            // Like `json`, `generic-c` has no subtypes; reject
            // `generic-c.<anything>` explicitly so a typo does not
            // silently succeed.
            "generic-c" if subtype.is_none() => Ok(Self::GenericC {
                path: value.map(PathBuf::from),
            }),
            "generic-c" => Err(EmitParseError::UnknownType(type_spec.to_string())),
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
            Self::Model { path: None } => f.write_str("model"),
            Self::Model { path: Some(path) } => write!(f, "model={}", path.to_string_lossy()),
            Self::Json { path: None } => f.write_str("json"),
            Self::Json { path: Some(path) } => write!(f, "json={}", path.to_string_lossy()),
            Self::GenericC { path: None } => f.write_str("generic-c"),
            Self::GenericC { path: Some(path) } => {
                write!(f, "generic-c={}", path.to_string_lossy())
            }
        }
    }
}
