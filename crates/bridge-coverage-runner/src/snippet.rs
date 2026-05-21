//! Per-snippet load + name<->row mapping per strategy section 3
//! (architecture) + section 8 (snippet hash for fingerprint).
//!
//! Each row with `expectation` of `positive` or `rejected` MUST
//! carry a `snippet_path`. Validator-failing rows are out of scope
//! for F1 per strategy section 2.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::matrix::{Expectation, Row};

/// One snippet loaded from disk, with the SHA-256 prefix used as
/// fingerprint input by [`crate::cluster`].
#[derive(Debug, Clone)]
pub struct Snippet {
    /// Absolute path to the snippet file.
    pub path: PathBuf,
    /// First 8 hex chars of the SHA-256 over the file's bytes.
    pub hash_prefix: String,
    /// Snippet length in bytes (used by [`crate::cluster`] to pick
    /// the shortest member as the representative repro).
    pub byte_len: usize,
}

/// Errors returned by [`load_snippets`].
#[derive(Debug)]
pub enum SnippetError {
    /// A row carries an expectation that requires a snippet but
    /// `snippet_path` is `None` in the sidecar.
    MissingSnippetPath {
        /// Owning row ID.
        row_id: String,
    },
    /// The snippet file could not be read.
    Io {
        /// Owning row ID.
        row_id: String,
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// A row whose expectation is `validator-failing` carries a
    /// snippet path; this is out of scope for F1.
    ValidatorFailingNotSupported {
        /// Owning row ID.
        row_id: String,
    },
}

impl std::fmt::Display for SnippetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnippetError::MissingSnippetPath { row_id } => {
                write!(
                    f,
                    "row {row_id}: positive/rejected expectation requires snippet_path"
                )
            }
            SnippetError::Io { row_id, err } => {
                write!(f, "row {row_id}: snippet I/O error: {err}")
            }
            SnippetError::ValidatorFailingNotSupported { row_id } => {
                write!(
                    f,
                    "row {row_id}: validator-failing expectation is out of scope for F1"
                )
            }
        }
    }
}

impl std::error::Error for SnippetError {}

/// Load every snippet referenced by `rows`. The returned `Vec` is
/// parallel to `rows`.
pub fn load_snippets(snippets_dir: &Path, rows: &[Row]) -> Result<Vec<Snippet>, SnippetError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match row.expectation {
            Expectation::ValidatorFailing => {
                return Err(SnippetError::ValidatorFailingNotSupported {
                    row_id: row.id.clone(),
                });
            }
            Expectation::Positive | Expectation::Rejected => {}
        }
        let rel = row
            .snippet_path
            .as_ref()
            .ok_or_else(|| SnippetError::MissingSnippetPath {
                row_id: row.id.clone(),
            })?;
        let path = snippets_dir.join(rel);
        let bytes = fs::read(&path).map_err(|err| SnippetError::Io {
            row_id: row.id.clone(),
            err,
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let hash_prefix: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        out.push(Snippet {
            path,
            hash_prefix,
            byte_len: bytes.len(),
        });
    }
    Ok(out)
}
