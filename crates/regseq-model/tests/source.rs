//! Integration tests for stage-1 step 4: `SourceFile`,
//! `Position`, `Span` (and the third concrete ID, `SourceFileId`).
//!
//! Validates the spec requirements from the "Source files and
//! spans" section of `docs/discussions/regseq-model/stage-1-vocabulary.md`:
//! - 1-based line, 1-based byte column for `Position`;
//! - `[lo, hi)` end-exclusive, file-local byte offsets for `Span`;
//! - `position` is a binary search over `line_starts`;
//! - `line_text` is 1-based, strips `\n` and `\r\n`, returns
//!   `None` when content is absent or out of range;
//! - `Span::start`/`end`/`slice` resolve via the `SourceFile`
//!   table.

use std::sync::Arc;

use regseq_model::{
    IndexVec, Position, SourceFile, SourceFileId, Span, StringId,
};

fn make_files(content: &str) -> IndexVec<SourceFileId, SourceFile> {
    let mut line_starts: Vec<u32> = vec![0];
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push((i + 1) as u32);
        }
    }
    let sf = SourceFile {
        path: StringId::ZERO,
        content: Some(Arc::from(content)),
        line_starts,
    };
    let mut files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
    files.push(sf);
    files
}

// ---------------------------------------------------------------
// Position from byte (binary search over line_starts)
// ---------------------------------------------------------------

#[test]
fn position_byte_zero_is_line_one_col_one() {
    let files = make_files("abc\ndef\n");
    let pos = files[SourceFileId::ZERO].position(0);
    assert_eq!(pos, Position { line: 1, col: 1 });
}

#[test]
fn position_within_first_line() {
    let files = make_files("hello world\nsecond line\n");
    let pos = files[SourceFileId::ZERO].position(6); // "w" of "world"
    assert_eq!(pos, Position { line: 1, col: 7 });
}

#[test]
fn position_at_line_boundary_belongs_to_next_line() {
    // line_starts = [0, 4, 8] for "abc\ndef\nghi"
    let files = make_files("abc\ndef\nghi");
    let pos = files[SourceFileId::ZERO].position(4); // 'd'
    assert_eq!(pos, Position { line: 2, col: 1 });
}

#[test]
fn position_on_third_line() {
    let files = make_files("abc\ndef\nghi");
    let pos = files[SourceFileId::ZERO].position(10); // 'i'
    assert_eq!(pos, Position { line: 3, col: 3 });
}

#[test]
fn position_past_end_extends_last_line() {
    // For byte beyond all line_starts, position falls on the last line.
    let files = make_files("ab\ncd"); // line_starts = [0, 3]
    let pos = files[SourceFileId::ZERO].position(5); // one past 'd'
    assert_eq!(pos, Position { line: 2, col: 3 });
}

#[test]
fn position_with_empty_line_starts_treats_byte_zero_as_one_one() {
    let sf = SourceFile {
        path: StringId::ZERO,
        content: Some(Arc::from("anything")),
        line_starts: Vec::new(),
    };
    assert_eq!(sf.position(0), Position { line: 1, col: 1 });
    assert_eq!(sf.position(5), Position { line: 1, col: 6 });
}

#[test]
fn position_when_first_line_start_is_after_byte_falls_back_to_line_one() {
    // Pathological / corrupted input: line_starts[0] > byte. There
    // is no recorded line start <= byte, so the resolver must NOT
    // index line_starts[0] (its value is post-`byte`); it must fall
    // back to line 1 with column byte + 1.
    let sf = SourceFile {
        path: StringId::ZERO,
        content: Some(Arc::from("anything")),
        line_starts: vec![10],
    };
    assert_eq!(sf.position(0), Position { line: 1, col: 1 });
    assert_eq!(sf.position(5), Position { line: 1, col: 6 });
    // At byte == 10 we cross into line 1's recorded start; that's
    // the first line, column 1.
    assert_eq!(sf.position(10), Position { line: 1, col: 1 });
}

// ---------------------------------------------------------------
// line_text: 1-based, terminator-stripped, content-aware
// ---------------------------------------------------------------

#[test]
fn line_text_returns_first_line_without_terminator() {
    let files = make_files("hello\nworld\n");
    let sf = &files[SourceFileId::ZERO];
    assert_eq!(sf.line_text(1), Some("hello"));
    assert_eq!(sf.line_text(2), Some("world"));
}

#[test]
fn line_text_strips_crlf() {
    let files = make_files("hello\r\nworld\r\n");
    let sf = &files[SourceFileId::ZERO];
    assert_eq!(sf.line_text(1), Some("hello"));
    assert_eq!(sf.line_text(2), Some("world"));
}

#[test]
fn line_text_last_line_without_terminator() {
    let files = make_files("hello\nworld"); // no trailing \n
    let sf = &files[SourceFileId::ZERO];
    assert_eq!(sf.line_text(2), Some("world"));
}

#[test]
fn line_text_returns_none_for_zero_or_out_of_range() {
    let files = make_files("only\n");
    let sf = &files[SourceFileId::ZERO];
    assert_eq!(sf.line_text(0), None);
    assert_eq!(sf.line_text(99), None);
}

#[test]
fn line_text_returns_none_when_content_absent() {
    let sf = SourceFile {
        path: StringId::ZERO,
        content: None,
        line_starts: vec![0, 6, 12],
    };
    assert_eq!(sf.line_text(1), None);
    assert_eq!(sf.line_text(2), None);
}

// ---------------------------------------------------------------
// Span resolution
// ---------------------------------------------------------------

#[test]
fn span_start_and_end_resolve_via_files() {
    let files = make_files("abc\ndef\nghi");
    let span = Span {
        file: SourceFileId::ZERO,
        lo: 4, // 'd'
        hi: 7, // one past 'f'
    };
    assert_eq!(span.start(&files), Position { line: 2, col: 1 });
    assert_eq!(span.end(&files), Position { line: 2, col: 4 });
}

#[test]
fn span_slice_returns_text_range() {
    let files = make_files("abc\ndef\nghi");
    let span = Span {
        file: SourceFileId::ZERO,
        lo: 4,
        hi: 7,
    };
    assert_eq!(span.slice(&files), Some("def"));
}

#[test]
fn span_slice_returns_none_when_content_absent() {
    let mut files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
    files.push(SourceFile {
        path: StringId::ZERO,
        content: None,
        line_starts: vec![0, 4],
    });
    let span = Span {
        file: SourceFileId::ZERO,
        lo: 0,
        hi: 3,
    };
    assert_eq!(span.slice(&files), None);
}

#[test]
fn span_empty_range_slices_empty_str() {
    let files = make_files("abc");
    let span = Span {
        file: SourceFileId::ZERO,
        lo: 1,
        hi: 1,
    };
    assert_eq!(span.slice(&files), Some(""));
}

// ---------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------

#[test]
fn position_round_trips_through_json() {
    let p = Position { line: 7, col: 12 };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, r#"{"line":7,"col":12}"#);
    let back: Position = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}

#[test]
fn span_round_trips_through_json() {
    let span = Span {
        file: SourceFileId::from_u32(2),
        lo: 4,
        hi: 7,
    };
    let json = serde_json::to_string(&span).unwrap();
    assert_eq!(json, r#"{"file":2,"lo":4,"hi":7}"#);
    let back: Span = serde_json::from_str(&json).unwrap();
    assert_eq!(back, span);
}

#[test]
fn source_file_with_content_round_trips() {
    let sf = SourceFile {
        path: StringId::from_u32(3),
        content: Some(Arc::from("ab\ncd\n")),
        line_starts: vec![0, 3],
    };
    let json = serde_json::to_string(&sf).unwrap();
    let back: SourceFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.path, sf.path);
    assert_eq!(back.line_starts, sf.line_starts);
    assert_eq!(back.content.as_deref(), sf.content.as_deref());
}

#[test]
fn source_file_with_no_content_round_trips() {
    let sf = SourceFile {
        path: StringId::from_u32(5),
        content: None,
        line_starts: vec![0, 4, 8],
    };
    let json = serde_json::to_string(&sf).unwrap();
    let back: SourceFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back, sf);
}

// ---------------------------------------------------------------
// SourceFileId direct serde smoke tests (parallel to StringId/CrateId)
// ---------------------------------------------------------------

#[test]
fn source_file_id_round_trips_through_json() {
    for raw in [0u32, 1, 7, SourceFileId::MAX_AS_U32] {
        let id = SourceFileId::from_u32(raw);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, raw.to_string());
        let back: SourceFileId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}

#[test]
fn source_file_id_rejects_max_u32_on_deserialize() {
    let json = u32::MAX.to_string();
    let result: Result<SourceFileId, _> = serde_json::from_str(&json);
    let err = result.expect_err("u32::MAX must be rejected (niche-reserved)");
    assert!(err.to_string().contains("SourceFileId"));
}

#[test]
fn source_file_id_synthesized_alias_equals_zero() {
    // `SourceFileId::SYNTHESIZED` is a documentary alias for
    // `SourceFileId::ZERO`, used by bridge code when constructing
    // spans for items that have no source location.
    assert_eq!(SourceFileId::SYNTHESIZED, SourceFileId::ZERO);
    assert_eq!(SourceFileId::SYNTHESIZED.as_u32(), 0);
}

#[test]
fn synthesized_span_slice_returns_none_via_sentinel_file() {
    // The bridge populates source_files[0] as the synthesized
    // sentinel: content=None, so any Span pointing at
    // SourceFileId::SYNTHESIZED slices to None regardless of
    // lo/hi -- consumers do not need Option<Span> checks.
    let mut files: IndexVec<SourceFileId, SourceFile> = IndexVec::new();
    files.push(SourceFile {
        path: StringId::ZERO,
        content: None,
        line_starts: vec![0],
    });
    let span = Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    };
    assert_eq!(span.slice(&files), None);
}
