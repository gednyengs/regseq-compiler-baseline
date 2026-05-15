//! Source files, byte positions, and spans.
//!
//! `SourceFile` carries the per-file source text (when owned),
//! the file path (interned), and the precomputed `line_starts`
//! index for `byte -> Position` resolution. `Position` is the
//! resolved 1-based line / 1-based byte-column form.
//! `Span` is the file-local `[lo, hi)` byte range that appears
//! inline on every IR node carrying a source location.
//!
//! Per F1 the byte offsets in `Span` are *file-local*, not the
//! `SourceMap`-global `BytePos` rustc uses internally. Bridge code
//! is responsible for the global-to-file-local translation; that
//! work lands in a later step.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ids::{SourceFileId, StringId};
use crate::index_vec::IndexVec;

/// A source file in the model.
///
/// `content` is `Some` for crates Regseq extracts (the spec
/// crate, the `regseq` facade, regseq-aware crates per Q6) and
/// `None` for upstream-crate sources whose spans we surface but
/// whose source text we do not embed. `line_starts` is populated
/// in either case (rustc provides line-start info on a
/// `SourceFile` independent of whether we hold the text).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    /// Interned file path.
    pub path: StringId,
    /// File text, when this crate is one we own; `None` for
    /// upstream-crate sources we only refer to.
    pub content: Option<Arc<str>>,
    /// File-local byte offsets of each line start. Line `N`
    /// (1-based externally) is at `line_starts[N - 1]`. The table
    /// itself is zero-indexed.
    pub line_starts: Vec<u32>,
}

impl SourceFile {
    /// Resolves a file-local byte offset to a 1-based
    /// line / 1-based byte-column `Position`.
    ///
    /// Performs a binary search over `line_starts`. A `byte`
    /// beyond the last line start places the position on the
    /// final line at column `byte - last_line_start + 1`. When
    /// no recorded line start is `<= byte` (either `line_starts`
    /// is empty, or `line_starts[0] > byte`), the byte is
    /// reported on line 1 with column `byte + 1`.
    pub fn position(&self, byte: u32) -> Position {
        match self.line_starts.partition_point(|&start| start <= byte) {
            0 => Position {
                line: 1,
                col: byte + 1,
            },
            pp => {
                let line_idx = pp - 1;
                let line_start = self.line_starts[line_idx];
                Position {
                    line: line_idx as u32 + 1,
                    col: byte - line_start + 1,
                }
            }
        }
    }

    /// Returns the text of `line` (1-based) without its trailing
    /// terminator (`\n` or `\r\n`).
    ///
    /// Returns `None` when `content` is `None`, when `line` is
    /// `0` or out of range, or when the recorded line bounds fall
    /// outside the content (a corrupted SourceFile).
    pub fn line_text(&self, line: u32) -> Option<&str> {
        let content = self.content.as_ref()?;
        let line_idx = (line as usize).checked_sub(1)?;
        let start = (*self.line_starts.get(line_idx)?) as usize;
        let end = match self.line_starts.get(line_idx + 1) {
            Some(&next) => next as usize,
            None => content.len(),
        };
        if start > content.len() || end > content.len() || start > end {
            return None;
        }
        let raw = &content[start..end];
        let trimmed = raw
            .strip_suffix("\r\n")
            .or_else(|| raw.strip_suffix('\n'))
            .unwrap_or(raw);
        Some(trimmed)
    }
}

/// A resolved 1-based line / 1-based byte-column source position.
///
/// Stage-1 `Position` is byte-column. Display-column (which would
/// account for multi-byte and full-width characters) is deferred;
/// driver source code is overwhelmingly ASCII, where byte-col
/// matches display-col.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based byte column.
    pub col: u32,
}

/// A file-local source range `[lo, hi)`.
///
/// `lo` and `hi` are byte offsets into
/// `SourceFile.content`, *not* rustc's `SourceMap`-global
/// `BytePos`. Bridge code translates from global to file-local
/// (in a later step). Spans appear inline in IR nodes; there is
/// no span-deduplication table (per F2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file: SourceFileId,
    /// File-local byte offset, inclusive.
    pub lo: u32,
    /// File-local byte offset, exclusive.
    pub hi: u32,
}

impl Span {
    /// Returns the bridge-synthesized dummy span.
    ///
    /// Synthetic MIR locals introduced by bridge collapse or
    /// post-construction rewrites use this location when no source
    /// declaration exists (spec.md section 25.2).
    pub const fn dummy() -> Self {
        Self {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        }
    }

    /// Position of `lo` in the indexed `SourceFile`.
    pub fn start(&self, files: &IndexVec<SourceFileId, SourceFile>) -> Position {
        files[self.file].position(self.lo)
    }

    /// Position of `hi` in the indexed `SourceFile`. Note that
    /// `hi` is end-exclusive: `Span::end` returns the position
    /// one past the last byte of the span.
    pub fn end(&self, files: &IndexVec<SourceFileId, SourceFile>) -> Position {
        files[self.file].position(self.hi)
    }

    /// The source text covered by `[lo, hi)`, or `None` when the
    /// indexed `SourceFile` has `content == None`.
    ///
    /// Panics if `lo > hi` or if `hi` exceeds `content.len()` --
    /// these are bridge-invariant violations and surface via
    /// standard slice-bounds panics rather than defensive
    /// `Option`s (per methodology).
    pub fn slice<'a>(&self, files: &'a IndexVec<SourceFileId, SourceFile>) -> Option<&'a str> {
        let sf = &files[self.file];
        let content = sf.content.as_ref()?;
        Some(&content[self.lo as usize..self.hi as usize])
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
