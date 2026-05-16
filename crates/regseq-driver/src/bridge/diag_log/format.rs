//! Diagnostic-logging format-template grammar parser per spec.md
//! section 26.7 closed subset. Debug formatters (`{:?}`) are parsed
//! only to reject per D3 at spec.md line 12840.

use crate::bridge::{BridgeError, DiagLogFormatReason};

/// Parsed format template per the closed-subset grammar at spec.md
/// section 26.7.
///
/// `literal_segments` contains `specifiers.len() + 1` entries that
/// alternate with the specifiers: text before the first specifier,
/// text between adjacent specifiers, and the trailing tail. Escape
/// sequences `{{` and `}}` are rendered as the literal `{` and `}`
/// per spec.md section 26.7 ("Escape sequences").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedFormatTemplate {
    /// Literal text segments between (and around) specifiers.
    pub literal_segments: Vec<String>,
    /// Specifiers in source order.
    pub specifiers: Vec<FormatSpecifier>,
}

/// One format specifier per spec.md section 26.7 closed subset.
/// Debug formatters (`{:?}`) parse-and-reject per D3 at spec.md
/// line 12840 and never appear as a parsed specifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatSpecifier {
    /// `{}` -- the default Display specifier.
    Display,
    /// `{:x}` -- lowercase hex.
    Hex,
    /// `{:X}` -- uppercase hex.
    HexUpper,
    /// `{:#x}` / `{:#X}` / `{:08x}` / `{:#010x}` and analogous
    /// explicit-modifier forms with optional alternate-form `#`
    /// flag, optional zero-fill, and optional width.
    HexFormatted {
        /// Fill character (`' '` default, `'0'` when the zero-fill
        /// flag is set).
        fill: char,
        /// Minimum field width in characters, or `0` when no width
        /// is specified.
        width: u32,
        /// Alternate-form flag (`#`): emit the `0x` / `0X` prefix.
        alternate: bool,
        /// `true` for the `X` (uppercase) type character, `false`
        /// for the `x` (lowercase) type character.
        upper: bool,
    },
}

/// Parse a format template string per spec.md section 26.7 closed
/// subset.
///
/// Returns `BridgeError::DiagLogFormat` if the template contains a
/// Debug formatter (`{:?}`) per D3 at spec.md line 12840, a
/// positional argument (`{0}`), a named argument (`{name}`), or any
/// other non-subset construct (e.g., precision, binary, octal,
/// pointer formatting).
pub fn parse_format_template(template: &str) -> Result<ParsedFormatTemplate, BridgeError> {
    let mut literal_segments: Vec<String> = Vec::new();
    let mut specifiers: Vec<FormatSpecifier> = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = template.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let c = chars[idx];
        match c {
            '{' => {
                // Escape sequence `{{` -> literal `{`.
                if idx + 1 < chars.len() && chars[idx + 1] == '{' {
                    current.push('{');
                    idx += 2;
                    continue;
                }
                // Find the closing `}`. The closed subset has no
                // nested braces inside a specifier body.
                let close = find_closing_brace(&chars, idx + 1).ok_or_else(|| {
                    BridgeError::DiagLogFormat {
                        reason: DiagLogFormatReason::UnclosedBrace,
                        template: template.to_string(),
                    }
                })?;
                let spec_body: String = chars[idx + 1..close].iter().collect();
                let specifier = parse_specifier(&spec_body, template)?;
                literal_segments.push(std::mem::take(&mut current));
                specifiers.push(specifier);
                idx = close + 1;
            }
            '}' => {
                // Escape sequence `}}` -> literal `}`.
                if idx + 1 < chars.len() && chars[idx + 1] == '}' {
                    current.push('}');
                    idx += 2;
                    continue;
                }
                return Err(BridgeError::DiagLogFormat {
                    reason: DiagLogFormatReason::UnmatchedClosingBrace,
                    template: template.to_string(),
                });
            }
            other => {
                current.push(other);
                idx += 1;
            }
        }
    }
    literal_segments.push(current);

    debug_assert_eq!(literal_segments.len(), specifiers.len() + 1);
    Ok(ParsedFormatTemplate {
        literal_segments,
        specifiers,
    })
}

/// Locate the closing `}` for a placeholder that opens at `chars[start - 1] == '{'`.
/// Returns the index of the closing `}`, or `None` if the placeholder
/// is unterminated. The closed subset has no nested braces; any `{`
/// encountered inside a placeholder body is treated as an unclosed
/// placeholder.
fn find_closing_brace(chars: &[char], start: usize) -> Option<usize> {
    let mut idx = start;
    while idx < chars.len() {
        match chars[idx] {
            '}' => return Some(idx),
            '{' => return None,
            _ => idx += 1,
        }
    }
    None
}

/// Parse the body of a single placeholder (`{<body>}` -- without the
/// braces). `<body>` is either empty (`{}` -> Display) or starts with
/// `:` followed by the format specifier per spec.md section 26.7
/// closed subset.
fn parse_specifier(body: &str, template: &str) -> Result<FormatSpecifier, BridgeError> {
    // `{}` -> Display.
    if body.is_empty() {
        return Ok(FormatSpecifier::Display);
    }

    // Positional / named arguments are outside the closed subset
    // per spec.md section 26.7.
    let first = body.chars().next().unwrap();
    if first != ':' {
        // `{0}` (positional) or `{name}` (named) or similar.
        let reason = if first.is_ascii_digit() {
            DiagLogFormatReason::PositionalArgRejected
        } else if first.is_alphabetic() || first == '_' {
            DiagLogFormatReason::NamedArgRejected
        } else {
            DiagLogFormatReason::NonSubsetModifier
        };
        return Err(BridgeError::DiagLogFormat {
            reason,
            template: template.to_string(),
        });
    }

    // Body starts with `:`; parse the format spec.
    let spec = &body[1..];

    // Debug formatter `{:?}` parses-and-rejects per D3 at
    // spec.md line 12840.
    if spec == "?" {
        return Err(BridgeError::DiagLogFormat {
            reason: DiagLogFormatReason::DebugFormatterRejected,
            template: template.to_string(),
        });
    }

    // `{:}` -- not in the closed v1 subset per spec.md section
    // 26.7. Authors must write `{}` for Display; the `:`-prefixed
    // empty body is rejected as a non-subset modifier.
    if spec.is_empty() {
        return Err(BridgeError::DiagLogFormat {
            reason: DiagLogFormatReason::NonSubsetModifier,
            template: template.to_string(),
        });
    }

    parse_hex_spec(spec, template)
}

/// Parse the closed v1 subset hex specifier body (the text after
/// `:`). Per spec.md section 26.7 the closed v1 subset is exactly:
///
/// ```text
/// :x      :X      // plain hex
/// :#x     :#X     // alternate-form hex with 0x / 0X prefix
/// :08x    :08X    // zero-padded 8-wide hex
/// :#010x  :#010X  // alternate-form zero-padded 10-wide hex
/// ```
///
/// Wider modifier coverage (other widths, decimal padding, alignment,
/// sign, precision, binary / octal / pointer / Debug / etc.) belongs
/// in the experiments track at `docs/discussions/logging-and-printing/`
/// per spec.md section 26.7 and may extend the subset in a future
/// revision. Any specifier outside the seven leading forms above
/// surfaces `DiagLogFormatReason::NonSubsetModifier`.
fn parse_hex_spec(spec: &str, template: &str) -> Result<FormatSpecifier, BridgeError> {
    let (upper, payload) = match spec {
        // Plain hex.
        "x" => return Ok(FormatSpecifier::Hex),
        "X" => return Ok(FormatSpecifier::HexUpper),
        // Alternate-form, no width.
        "#x" => (false, (false, true, 0u32, ' ')),
        "#X" => (true, (false, true, 0u32, ' ')),
        // Zero-padded width 8.
        "08x" => (false, (true, false, 8u32, '0')),
        "08X" => (true, (true, false, 8u32, '0')),
        // Alternate-form, zero-padded width 10.
        "#010x" => (false, (true, true, 10u32, '0')),
        "#010X" => (true, (true, true, 10u32, '0')),
        _ => {
            return Err(BridgeError::DiagLogFormat {
                reason: DiagLogFormatReason::NonSubsetModifier,
                template: template.to_string(),
            });
        }
    };
    let (_zero_fill, alternate, width, fill) = payload;
    Ok(FormatSpecifier::HexFormatted {
        fill,
        width,
        alternate,
        upper,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_template_has_one_empty_literal_and_no_specifiers() {
        let parsed = parse_format_template("").expect("empty template parses");
        assert_eq!(parsed.literal_segments, vec![String::new()]);
        assert!(parsed.specifiers.is_empty());
    }

    #[test]
    fn display_only_template() {
        let parsed = parse_format_template("x = {}").expect("display parses");
        assert_eq!(parsed.specifiers, vec![FormatSpecifier::Display]);
        assert_eq!(
            parsed.literal_segments,
            vec!["x = ".to_string(), String::new()]
        );
    }

    #[test]
    fn lowercase_hex_specifier() {
        let parsed = parse_format_template("x = {:x}").expect("hex parses");
        assert_eq!(parsed.specifiers, vec![FormatSpecifier::Hex]);
    }

    #[test]
    fn uppercase_hex_specifier() {
        let parsed = parse_format_template("{:X}").expect("hex-upper parses");
        assert_eq!(parsed.specifiers, vec![FormatSpecifier::HexUpper]);
    }

    #[test]
    fn formatted_hex_with_alternate_zero_fill_width() {
        let parsed = parse_format_template("{:#010x}").expect("formatted hex parses");
        assert_eq!(
            parsed.specifiers,
            vec![FormatSpecifier::HexFormatted {
                fill: '0',
                width: 10,
                alternate: true,
                upper: false,
            }]
        );
    }

    #[test]
    fn formatted_hex_zero_padded_no_alternate() {
        let parsed = parse_format_template("{:08x}").expect("zero-padded hex parses");
        assert_eq!(
            parsed.specifiers,
            vec![FormatSpecifier::HexFormatted {
                fill: '0',
                width: 8,
                alternate: false,
                upper: false,
            }]
        );
    }

    #[test]
    fn formatted_hex_alternate_no_width() {
        let parsed = parse_format_template("{:#x}").expect("alternate-only hex parses");
        assert_eq!(
            parsed.specifiers,
            vec![FormatSpecifier::HexFormatted {
                fill: ' ',
                width: 0,
                alternate: true,
                upper: false,
            }]
        );
    }

    #[test]
    fn multiple_specifiers_in_order() {
        let parsed = parse_format_template("a = {}, b = {:x}").expect("two specifiers parse");
        assert_eq!(
            parsed.specifiers,
            vec![FormatSpecifier::Display, FormatSpecifier::Hex]
        );
        assert_eq!(
            parsed.literal_segments,
            vec!["a = ".to_string(), ", b = ".to_string(), String::new()]
        );
    }

    #[test]
    fn debug_formatter_rejected_per_d3() {
        let err = parse_format_template("{:?}").expect_err("debug formatter rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::DebugFormatterRejected);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn positional_argument_rejected() {
        let err = parse_format_template("{0}").expect_err("positional rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::PositionalArgRejected);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn named_argument_rejected() {
        let err = parse_format_template("{name}").expect_err("named rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::NamedArgRejected);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn unclosed_brace_rejected() {
        let err = parse_format_template("{").expect_err("unclosed brace rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::UnclosedBrace);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn escape_open_brace_renders_to_literal() {
        let parsed = parse_format_template("{{").expect("escape parses");
        assert!(parsed.specifiers.is_empty());
        assert_eq!(parsed.literal_segments, vec!["{".to_string()]);
    }

    #[test]
    fn escape_close_brace_renders_to_literal() {
        let parsed = parse_format_template("}}").expect("escape parses");
        assert!(parsed.specifiers.is_empty());
        assert_eq!(parsed.literal_segments, vec!["}".to_string()]);
    }

    #[test]
    fn stray_close_brace_rejected() {
        let err = parse_format_template("}").expect_err("stray close brace rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::UnmatchedClosingBrace);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn binary_specifier_rejected_as_non_subset() {
        let err = parse_format_template("{:b}").expect_err("binary rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::NonSubsetModifier);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn octal_specifier_rejected_as_non_subset() {
        let err = parse_format_template("{:o}").expect_err("octal rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::NonSubsetModifier);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn pointer_specifier_rejected_as_non_subset() {
        let err = parse_format_template("{:p}").expect_err("pointer rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::NonSubsetModifier);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn precision_rejected_as_non_subset() {
        let err = parse_format_template("{:.5}").expect_err("precision rejected");
        match err {
            BridgeError::DiagLogFormat { reason, .. } => {
                assert_eq!(reason, DiagLogFormatReason::NonSubsetModifier);
            }
            other => panic!("expected DiagLogFormat, got {other:?}"),
        }
    }

    #[test]
    fn arbitrary_width_outside_closed_v1_subset_rejected() {
        // Per spec.md section 26.7 the closed v1 leading set lists
        // widths 8 (`:08x`) and 10 (`:#010x`) only. Wider modifier
        // coverage is deferred to a future revision; widths 5, 12,
        // etc. surface NonSubsetModifier here.
        for tpl in ["{:05x}", "{:5x}", "{:12x}", "{:#08x}"] {
            let err = parse_format_template(tpl).expect_err("non-subset width rejected");
            match err {
                BridgeError::DiagLogFormat { reason, .. } => {
                    assert_eq!(
                        reason,
                        DiagLogFormatReason::NonSubsetModifier,
                        "tpl `{tpl}` should reject as NonSubsetModifier",
                    );
                }
                other => panic!("expected DiagLogFormat, got {other:?}"),
            }
        }
    }

    #[test]
    fn uppercase_alternate_and_zero_padded_forms_accepted() {
        for (tpl, expected) in [
            (
                "{:#X}",
                FormatSpecifier::HexFormatted {
                    fill: ' ',
                    width: 0,
                    alternate: true,
                    upper: true,
                },
            ),
            (
                "{:08X}",
                FormatSpecifier::HexFormatted {
                    fill: '0',
                    width: 8,
                    alternate: false,
                    upper: true,
                },
            ),
            (
                "{:#010X}",
                FormatSpecifier::HexFormatted {
                    fill: '0',
                    width: 10,
                    alternate: true,
                    upper: true,
                },
            ),
        ] {
            let parsed = parse_format_template(tpl).expect("uppercase formatted hex parses");
            assert_eq!(parsed.specifiers, vec![expected]);
        }
    }
}
