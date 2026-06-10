//! Licensed-region strip for the composed-fixture identity invariant
//! (backend-zephyr-c GRILL C02, resolving O2).
//!
//! The two composed `input.rs` files (`pl011-composed`, `pl011-composed-zephyr`)
//! must be byte-identical after stripping exactly two **marked licensed
//! regions** per file — (i) `match-transport`, (ii) `deinit` — porting the L3
//! cited-deviation-whitelist discipline from traces to spec sources: every
//! OS-conditional source region is explicit, cited, and machine-checked, and
//! same-spec drift is mechanically impossible.
//!
//! Marker syntax (O2 resolution; whole-line comments, legal inside the
//! `#[rseq::driver(...)]` token tree because the lexer strips them):
//!
//! ```text
//! // RSEQ-LICENSED-REGION-BEGIN: <region-name> [ratified: <citation>]
//! ...OS-conditional content (may be empty)...
//! // RSEQ-LICENSED-REGION-END: <region-name>
//! ```
//!
//! The strip is a LINE-BASED text operation, independent of Rust parsing
//! (C02): a marker line is recognized by its trimmed prefix; marker lines and
//! every line between a begin/end pair are dropped. A begin marker with an
//! empty (or missing) `[ratified: ...]` payload is an error — the L3
//! empty-`ratified` rule, ported to sources.

/// Trimmed-line prefix opening a licensed region.
pub const BEGIN_MARKER: &str = "// RSEQ-LICENSED-REGION-BEGIN:";
/// Trimmed-line prefix closing a licensed region.
pub const END_MARKER: &str = "// RSEQ-LICENSED-REGION-END:";

/// One parsed licensed region (markers + contents already dropped from the
/// remainder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicensedRegion {
    /// Region name from the begin marker (must match the end marker's).
    pub name: String,
    /// The non-empty `[ratified: ...]` payload.
    pub citation: String,
    /// 1-based line number of the begin marker in the original text.
    pub begin_line: usize,
}

/// A malformed marker structure (positions are 1-based source lines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripError {
    /// `BEGIN` while a region is already open.
    NestedBegin { line: usize },
    /// `END` with no open region.
    UnmatchedEnd { line: usize },
    /// `END` naming a different region than the open `BEGIN`.
    NameMismatch {
        line: usize,
        begin: String,
        end: String,
    },
    /// EOF with a region still open.
    UnterminatedRegion { name: String, begin_line: usize },
    /// Begin marker without a non-empty `[ratified: ...]` citation.
    MissingCitation { name: String, line: usize },
    /// Begin marker with an empty region name.
    MissingName { line: usize },
}

impl std::fmt::Display for StripError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StripError::NestedBegin { line } => {
                write!(f, "line {line}: BEGIN marker inside an open region")
            }
            StripError::UnmatchedEnd { line } => {
                write!(f, "line {line}: END marker with no open region")
            }
            StripError::NameMismatch { line, begin, end } => write!(
                f,
                "line {line}: END marker names '{end}' but the open region is '{begin}'"
            ),
            StripError::UnterminatedRegion { name, begin_line } => write!(
                f,
                "region '{name}' opened at line {begin_line} is never closed"
            ),
            StripError::MissingCitation { name, line } => write!(
                f,
                "line {line}: region '{name}' lacks a non-empty [ratified: ...] citation \
                 (the L3 empty-ratified rule, ported to sources)"
            ),
            StripError::MissingName { line } => {
                write!(f, "line {line}: BEGIN marker with an empty region name")
            }
        }
    }
}

impl std::error::Error for StripError {}

/// Strip marker lines + licensed-region contents from `text` (line-based).
///
/// Returns the remainder (the shared spec text, with original line endings
/// preserved for the lines kept) and the parsed regions in source order.
pub fn strip_licensed_regions(text: &str) -> Result<(String, Vec<LicensedRegion>), StripError> {
    let mut remainder = String::with_capacity(text.len());
    let mut regions: Vec<LicensedRegion> = Vec::new();
    // (name, begin_line) of the currently open region.
    let mut open: Option<(String, usize)> = None;

    for (idx, line) in text.split_inclusive('\n').enumerate() {
        let lineno = idx + 1;
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(BEGIN_MARKER) {
            if let Some((name, begin_line)) = &open {
                let _ = (name, begin_line);
                return Err(StripError::NestedBegin { line: lineno });
            }
            let (name, citation) = parse_begin_payload(rest);
            if name.is_empty() {
                return Err(StripError::MissingName { line: lineno });
            }
            let citation = citation.ok_or(StripError::MissingCitation {
                name: name.clone(),
                line: lineno,
            })?;
            regions.push(LicensedRegion {
                name: name.clone(),
                citation,
                begin_line: lineno,
            });
            open = Some((name, lineno));
        } else if let Some(rest) = trimmed.strip_prefix(END_MARKER) {
            let Some((name, _)) = open.take() else {
                return Err(StripError::UnmatchedEnd { line: lineno });
            };
            let end_name = rest.trim();
            if end_name != name {
                return Err(StripError::NameMismatch {
                    line: lineno,
                    begin: name,
                    end: end_name.to_string(),
                });
            }
        } else if open.is_none() {
            remainder.push_str(line);
        }
        // else: inside an open region — dropped.
    }

    if let Some((name, begin_line)) = open {
        return Err(StripError::UnterminatedRegion { name, begin_line });
    }
    Ok((remainder, regions))
}

/// Parse `<region-name> [ratified: <citation>]` after a begin-marker prefix.
/// Returns `(name, Some(citation))` only when the citation payload is present
/// and non-empty.
fn parse_begin_payload(rest: &str) -> (String, Option<String>) {
    let rest = rest.trim();
    match rest.find("[ratified:") {
        Some(pos) => {
            let name = rest[..pos].trim().to_string();
            let after = &rest[pos + "[ratified:".len()..];
            let citation = match after.find(']') {
                Some(end) => after[..end].trim(),
                None => "",
            };
            if citation.is_empty() {
                (name, None)
            } else {
                (name, Some(citation.to_string()))
            }
        }
        None => (rest.to_string(), None),
    }
}

/// First line (1-based, in stripped-remainder coordinates) where the two
/// remainders diverge, with the diverging content — the C02 "failure names the
/// first diverging line" obligation.
pub fn first_divergence(a: &str, b: &str) -> Option<(usize, String, String)> {
    let mut a_lines = a.split_inclusive('\n');
    let mut b_lines = b.split_inclusive('\n');
    let mut lineno = 0;
    loop {
        lineno += 1;
        match (a_lines.next(), b_lines.next()) {
            (None, None) => return None,
            (la, lb) if la == lb => {}
            (la, lb) => {
                return Some((
                    lineno,
                    la.unwrap_or("<EOF>").trim_end().to_string(),
                    lb.unwrap_or("<EOF>").trim_end().to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CITED: &str =
        "// RSEQ-LICENSED-REGION-BEGIN: match-transport [ratified: A-13/B-5 via D01/D04]\n";
    const CITED_END: &str = "// RSEQ-LICENSED-REGION-END: match-transport\n";

    #[test]
    fn strips_region_contents_and_markers() {
        let text = format!("shared a\n{CITED}licensed\n{CITED_END}shared b\n");
        let (remainder, regions) = strip_licensed_regions(&text).expect("strip");
        assert_eq!(remainder, "shared a\nshared b\n");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "match-transport");
        assert_eq!(regions[0].citation, "A-13/B-5 via D01/D04");
        assert_eq!(regions[0].begin_line, 2);
    }

    #[test]
    fn empty_region_is_legal() {
        // The zephyr deinit region is empty by design (C02 region ii).
        let text = format!(
            "a\n// RSEQ-LICENSED-REGION-BEGIN: deinit [ratified: B-6/C-3 via D06]\n// RSEQ-LICENSED-REGION-END: deinit\nb\n"
        );
        let (remainder, regions) = strip_licensed_regions(&text).expect("strip");
        assert_eq!(remainder, "a\nb\n");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "deinit");
    }

    #[test]
    fn citationless_marker_fails() {
        // The L3 empty-`ratified` rule ported to sources (C02).
        let text = "// RSEQ-LICENSED-REGION-BEGIN: match-transport\nx\n// RSEQ-LICENSED-REGION-END: match-transport\n";
        match strip_licensed_regions(text) {
            Err(StripError::MissingCitation { name, line }) => {
                assert_eq!(name, "match-transport");
                assert_eq!(line, 1);
            }
            other => panic!("expected MissingCitation, got {other:?}"),
        }
    }

    #[test]
    fn empty_citation_payload_fails() {
        let text = "// RSEQ-LICENSED-REGION-BEGIN: deinit [ratified: ]\n// RSEQ-LICENSED-REGION-END: deinit\n";
        assert!(matches!(
            strip_licensed_regions(text),
            Err(StripError::MissingCitation { .. })
        ));
    }

    #[test]
    fn indented_markers_are_recognized() {
        // Markers inside the attribute token tree are indented; recognition is
        // on the TRIMMED line.
        let text = format!("    {CITED}    licensed\n    {CITED_END}");
        let (remainder, regions) = strip_licensed_regions(&text).expect("strip");
        assert_eq!(remainder, "");
        assert_eq!(regions.len(), 1);
    }

    #[test]
    fn nested_begin_fails() {
        let text = format!("{CITED}{CITED}{CITED_END}");
        assert!(matches!(
            strip_licensed_regions(&text),
            Err(StripError::NestedBegin { line: 2 })
        ));
    }

    #[test]
    fn unmatched_end_fails() {
        assert!(matches!(
            strip_licensed_regions(CITED_END),
            Err(StripError::UnmatchedEnd { line: 1 })
        ));
    }

    #[test]
    fn unterminated_region_fails() {
        assert!(matches!(
            strip_licensed_regions(CITED),
            Err(StripError::UnterminatedRegion { begin_line: 1, .. })
        ));
    }

    #[test]
    fn end_name_mismatch_fails() {
        let text = format!("{CITED}// RSEQ-LICENSED-REGION-END: deinit\n");
        assert!(matches!(
            strip_licensed_regions(&text),
            Err(StripError::NameMismatch { line: 2, .. })
        ));
    }

    #[test]
    fn divergence_names_first_differing_line() {
        let a = "same\nleft\nsame\n";
        let b = "same\nright\nsame\n";
        let (line, la, lb) = first_divergence(a, b).expect("diverges");
        assert_eq!(line, 2);
        assert_eq!(la, "left");
        assert_eq!(lb, "right");
    }

    #[test]
    fn identical_remainders_have_no_divergence() {
        assert_eq!(first_divergence("x\ny\n", "x\ny\n"), None);
    }

    #[test]
    fn shorter_file_diverges_at_eof() {
        let (line, la, lb) = first_divergence("x\n", "x\ny\n").expect("diverges");
        assert_eq!(line, 2);
        assert_eq!(la, "<EOF>");
        assert_eq!(lb, "y");
    }
}
