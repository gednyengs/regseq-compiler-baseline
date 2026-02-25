use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Scope prefix for `--param` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamScope {
    /// Parameter consumed by analysis passes.
    Analysis,
    /// Parameter consumed by codegen backends.
    Backend,
    /// General Regseq parameter.
    General,
}

impl fmt::Display for ParamScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Analysis => "analysis",
            Self::Backend => "backend",
            Self::General => "general",
        })
    }
}

/// A parsed `--param KEY=VALUE` argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegseqParam {
    /// Parameter scope.
    pub scope: ParamScope,
    /// Parameter name, validated as `id(.id)*`.
    pub name: String,
    /// Raw parameter value.
    pub value: String,
}

/// Parsing errors for `RegseqParam`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ParamParseError {
    /// Missing `=` delimiter between key and value.
    #[error("missing '=' in parameter: '{0}'")]
    MissingEquals(String),

    /// Empty parameter name after optional scope stripping.
    #[error("empty parameter name")]
    EmptyName,

    /// Invalid parameter name.
    #[error("invalid parameter name '{name}': {reason}")]
    InvalidName { name: String, reason: String },

    /// Unknown scope prefix.
    #[error("unknown scope prefix '{0}'; expected 'analysis', 'backend', or 'regseq'")]
    UnknownScope(String),
}

/// Validate that `name` matches the `id(.id)*` pattern.
pub fn validate_dotted_ident(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    if name.starts_with('.') {
        return Err("leading dot is not allowed".to_string());
    }
    if name.ends_with('.') {
        return Err("trailing dot is not allowed".to_string());
    }
    if name.contains("..") {
        return Err("empty segment is not allowed".to_string());
    }

    for segment in name.split('.') {
        let mut chars = segment.chars();
        let first = chars
            .next()
            .ok_or_else(|| "empty segment is not allowed".to_string())?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(format!(
                "segment '{segment}' must start with [a-zA-Z_] but starts with '{first}'"
            ));
        }

        for ch in chars {
            if !(ch.is_ascii_alphanumeric() || ch == '_') {
                return Err(format!(
                    "segment '{segment}' contains invalid character '{ch}', expected [a-zA-Z0-9_]"
                ));
            }
        }
    }

    Ok(())
}

impl FromStr for RegseqParam {
    type Err = ParamParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (key, value) = input
            .split_once('=')
            .ok_or_else(|| ParamParseError::MissingEquals(input.to_string()))?;

        if key.is_empty() {
            return Err(ParamParseError::EmptyName);
        }

        let (scope, name) = if let Some(name) = key.strip_prefix("analysis.") {
            (ParamScope::Analysis, name)
        } else if let Some(name) = key.strip_prefix("backend.") {
            (ParamScope::Backend, name)
        } else if let Some(name) = key.strip_prefix("regseq.") {
            (ParamScope::General, name)
        } else {
            (ParamScope::General, key)
        };

        if name.is_empty() {
            return Err(ParamParseError::EmptyName);
        }

        validate_dotted_ident(name).map_err(|reason| ParamParseError::InvalidName {
            name: name.to_string(),
            reason,
        })?;

        Ok(Self {
            scope,
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}
