//! Helpers for serializing RegseqModel IR to JSON with resolved
//! [`Symbol`]/[`FileId`] strings.
//!
//! The IR derives `serde::Serialize` throughout, but [`Symbol`] and
//! [`FileId`] carry the raw interner index on the wire by default so
//! in-tree round-trip tests can deserialize them back to `u32`. The
//! JSON-golden format documented in `ir-reference.md` Section 7
//! requires the resolved strings instead. Callers stage this switch
//! by installing a per-thread interner via
//! [`install_interner_for_serialization`](crate::identity::install_interner_for_serialization)
//! before handing the value to `serde_json`; the helpers here bundle
//! that dance into a single call.
//!
//! The helpers intentionally accept any `T: Serialize`. [`Body`] is
//! the primary use case (task `regseq-model-54`) but Phase 6 will
//! apply the same pattern to the full `RegseqModel`.

use serde::Serialize;

use crate::identity::{install_interner_for_serialization, SymbolInterner};
use crate::ir::Body;
use crate::model::RegseqModel;

/// Serialize `value` to a pretty-printed JSON string, resolving every
/// [`Symbol`](crate::identity::Symbol) and [`FileId`](crate::identity::FileId)
/// through `interner`. The output is newline-terminated so it matches
/// what text-editor saves produce -- golden files committed to the
/// repository should include the trailing newline.
///
/// Returns a `serde_json::Error` if the underlying serializer fails;
/// this should only happen for types that intentionally fail
/// serialization (the Regseq IR does not).
pub fn to_json_pretty<T: Serialize>(
    value: &T,
    interner: &SymbolInterner,
) -> Result<String, serde_json::Error> {
    let _guard = install_interner_for_serialization(interner);
    let mut json = serde_json::to_string_pretty(value)?;
    if !json.ends_with('\n') {
        json.push('\n');
    }
    Ok(json)
}

/// Convenience wrapper around [`to_json_pretty`] for [`Body`].
///
/// Takes ownership of the reference pair so the call site reads the
/// way the Phase 5 task spec describes the contract: "the
/// serialization helper takes a `(Body, &SymbolInterner)` pair so
/// symbol resolution happens at write time without embedding the
/// interner in `Body`."
pub fn body_to_json_pretty(
    body: &Body,
    interner: &SymbolInterner,
) -> Result<String, serde_json::Error> {
    to_json_pretty(body, interner)
}

/// Pretty-print a full [`RegseqModel`] to the wire format described by
/// `ir-reference.md` Section 7.
///
/// Uses the model's own [`SymbolInterner`] to resolve every
/// [`Symbol`](crate::identity::Symbol) and
/// [`FileId`](crate::identity::FileId) embedded in the IR to its
/// string form. The returned string is newline-terminated so it
/// matches committed golden files on disk.
///
/// The helper panics only if `serde_json` itself fails, which should
/// not happen for the Regseq IR.
pub fn serialize_regseq_model(model: &RegseqModel) -> String {
    to_json_pretty(model, &model.symbols)
        .expect("RegseqModel serialization must never fail for the Regseq IR")
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::SourceOrigin;
    use crate::identity::{BasicBlockId, LocalId, Symbol, SymbolInterner};
    use crate::ir::{
        BasicBlock, BasicBlockSourceMap, BodySourceMap, LocalDecl, RegseqType, ScalarType,
        Terminator, TerminatorKind,
    };

    fn dummy_body(interner: &mut SymbolInterner) -> Body {
        let name = interner.intern("count");
        Body {
            local_decls: vec![
                LocalDecl {
                    ty: RegseqType::Unit,
                    mutable: true,
                    name: None,
                },
                LocalDecl {
                    ty: RegseqType::Scalar(ScalarType::U32),
                    mutable: false,
                    name: Some(name),
                },
            ],
            arg_count: 1,
            basic_blocks: vec![BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                },
            }],
            source_map: BodySourceMap {
                body_span: SourceOrigin::CompilerGenerated,
                locals: vec![
                    SourceOrigin::CompilerGenerated,
                    SourceOrigin::CompilerGenerated,
                ],
                blocks: vec![BasicBlockSourceMap {
                    statements: vec![],
                    terminator: SourceOrigin::CompilerGenerated,
                }],
            },
            ordered_regions: vec![],
        }
    }

    #[test]
    fn body_to_json_resolves_symbol_fields_as_strings() {
        let mut interner = SymbolInterner::new();
        let body = dummy_body(&mut interner);
        let json = body_to_json_pretty(&body, &interner).expect("serialize body");
        assert!(
            json.contains("\"count\""),
            "Symbol must emit resolved string, got: {json}",
        );
        // Raw u32 index for `count` should NOT appear as a bare value.
        assert!(
            !json.contains("\"name\": 0"),
            "Symbol must not emit raw index, got: {json}",
        );
    }

    #[test]
    fn to_json_pretty_is_deterministic_across_runs() {
        let mut a = SymbolInterner::new();
        let body_a = dummy_body(&mut a);
        let first = body_to_json_pretty(&body_a, &a).unwrap();

        let mut b = SymbolInterner::new();
        let body_b = dummy_body(&mut b);
        let second = body_to_json_pretty(&body_b, &b).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn output_is_newline_terminated() {
        let mut interner = SymbolInterner::new();
        let body = dummy_body(&mut interner);
        let json = body_to_json_pretty(&body, &interner).unwrap();
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn installed_interner_is_cleared_after_helper_returns() {
        let mut interner = SymbolInterner::new();
        let body = dummy_body(&mut interner);
        let _ = body_to_json_pretty(&body, &interner).unwrap();
        // After the guard from `to_json_pretty` is dropped, a subsequent
        // raw Symbol serialization must fall back to the u32 encoding
        // (no TLS leakage).
        let sym: Symbol = interner.intern("another");
        let raw = serde_json::to_string(&sym).unwrap();
        assert!(
            raw.parse::<u32>().is_ok(),
            "Symbol must fall back to u32 when no guard is active, got {raw}",
        );
        // Mention an otherwise unused body-level ID to keep the imports
        // exercised by this test.
        let _ = BasicBlockId::new(0);
        let _ = LocalId::new(0);
    }
}
