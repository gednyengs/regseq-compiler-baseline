//! Doc-1 construct-surface matrix parser (row IDs + design status).
//!
//! The runner consumes Doc 1
//! (`01-construct-surface-and-matrix.md`) read-only: it parses every
//! family table into the (id, status) row universe the ledger aggregates
//! against. The `status` column is reflected verbatim as the current
//! design status (the Doc-1 status-flip workflow is out of scope here).
//!
//! Cell splitting is GFM-code-span aware: a `|` inside an inline-code
//! span (delimited by a backtick run) is literal, not a column delimiter.
//! Doc 1 contains exactly such a cell (`` C `^ & |` `` in the `BO-12..14`
//! row), so a naive split would misread its status column.

use std::path::Path;

/// One matrix row: its Doc-1 ID and design status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixRow {
    /// Doc-1 row ID, taken verbatim (grouped ranges like `BO-1..5` are
    /// a single ID -- no range expansion).
    pub id: String,
    /// Design status from the Doc-1 legend.
    pub status: Status,
}

/// Doc-1 coverage legend (lines 31-44 of the matrix doc).
#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// `generic-c-backend.md` pins this construct's C lowering generally.
    Covered,
    /// The spec explicitly defers it (not gated this phase).
    Deferred,
    /// No general C lowering pinned anywhere -- an open in-scope gap.
    Unspecced,
    /// Not lowerable / not emitted in Phase 1 by design (never gates).
    OutOfScope,
}

impl Status {
    /// Parse a Doc-1 legend status string, or `None` if not a legend value.
    fn from_legend(s: &str) -> Option<Status> {
        match s {
            "covered" => Some(Status::Covered),
            "deferred" => Some(Status::Deferred),
            "unspecced" => Some(Status::Unspecced),
            "out-of-scope" => Some(Status::OutOfScope),
            _ => None,
        }
    }
}

/// Errors from matrix parsing.
#[derive(Debug)]
pub enum MatrixError {
    /// The matrix file could not be read.
    Io(std::io::Error),
    /// A candidate matrix row carried a non-legend status (the matrix
    /// drifted from the legend).
    NonLegendStatus {
        /// The row ID whose status is off-legend.
        id: String,
        /// The offending status cell, verbatim.
        status: String,
    },
    /// Two rows share an ID (each Doc-1 row ID must be unique).
    DuplicateId {
        /// The duplicated ID.
        id: String,
    },
}

impl std::fmt::Display for MatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatrixError::Io(err) => write!(f, "matrix I/O error: {err}"),
            MatrixError::NonLegendStatus { id, status } => write!(
                f,
                "row {id}: status {status:?} is not a legend value \
                 (covered/deferred/unspecced/out-of-scope)"
            ),
            MatrixError::DuplicateId { id } => write!(f, "duplicate matrix row ID: {id}"),
        }
    }
}

impl std::error::Error for MatrixError {}

impl From<std::io::Error> for MatrixError {
    fn from(value: std::io::Error) -> Self {
        MatrixError::Io(value)
    }
}

/// Read and parse the Doc-1 matrix markdown at `matrix_md_path`.
pub fn parse(matrix_md_path: &Path) -> Result<Vec<MatrixRow>, MatrixError> {
    let text = std::fs::read_to_string(matrix_md_path)?;
    parse_str(&text)
}

/// Parse Doc-1 matrix markdown from an in-memory string. Returns rows
/// sorted by ID, duplicate-free.
pub fn parse_str(md: &str) -> Result<Vec<MatrixRow>, MatrixError> {
    let mut rows: Vec<MatrixRow> = Vec::new();
    for line in md.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let cells = split_cells(line);
        if cells.len() < 6 {
            continue;
        }
        let id = &cells[0];
        if !is_row_id(id) {
            // Header (`| id |`) and separator (`| -- |`) rows fail this.
            continue;
        }
        let status_cell = &cells[3];
        let status =
            Status::from_legend(status_cell).ok_or_else(|| MatrixError::NonLegendStatus {
                id: id.clone(),
                status: status_cell.clone(),
            })?;
        if rows.iter().any(|r| &r.id == id) {
            return Err(MatrixError::DuplicateId { id: id.clone() });
        }
        rows.push(MatrixRow {
            id: id.clone(),
            status,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

/// Split a markdown table row into trimmed cells on column-delimiter
/// `|` characters, treating a `|` inside an inline-code span (a run of
/// backticks of matching length, per GFM) as literal. The leading and
/// trailing boundary-pipe empties are dropped (one each).
fn split_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    // Length of the backtick run that opened the current code span; 0
    // means we are not inside a span.
    let mut span_run: usize = 0;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            let mut run = 1;
            while chars.peek() == Some(&'`') {
                chars.next();
                run += 1;
            }
            for _ in 0..run {
                cur.push('`');
            }
            if span_run == 0 {
                span_run = run;
            } else if span_run == run {
                span_run = 0;
            }
            continue;
        }
        if c == '|' && span_run == 0 {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    cells.push(cur.trim().to_string());

    if cells.first().is_some_and(|c| c.is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|c| c.is_empty()) {
        cells.pop();
    }
    cells
}

/// Returns true iff `s` matches a Doc-1 row-ID shape:
/// `[A-Z]{2,3}-\d+(\.\.\d+)?[a-z]?` -- a 2-3 letter family prefix, `-`,
/// one or more digits, an optional `..<digits>` grouped range, and an
/// optional single lowercase suffix letter. Accepts `IT-1`, `TY-18`,
/// `IN-5b`, and `BO-1..5`; rejects headers (`id`) and separators (`--`).
fn is_row_id(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;

    // Family prefix: 2-3 uppercase ASCII letters.
    let mut prefix_len = 0;
    while i < bytes.len() && bytes[i].is_ascii_uppercase() {
        i += 1;
        prefix_len += 1;
    }
    if !(2..=3).contains(&prefix_len) {
        return false;
    }

    // Separator.
    if i >= bytes.len() || bytes[i] != b'-' {
        return false;
    }
    i += 1;

    // One or more digits.
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return false;
    }

    // Optional grouped range: `..<digits>`.
    if i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b'.' {
        i += 2;
        let range_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == range_start {
            return false;
        }
    }

    // Optional single lowercase suffix letter.
    if i < bytes.len() && bytes[i].is_ascii_lowercase() {
        i += 1;
    }

    i == bytes.len()
}
