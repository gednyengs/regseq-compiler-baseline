//! Bridge-side validation diagnostics for diagnostic-logging Calls
//! per spec.md section 26.8.
//!
//! Surface layout:
//!
//! - `validate_log_call(cx, parsed_template, arg_count)` -- the
//!   public template-vs-argcount validator. Drives D1 (template /
//!   argument arity parity) and D7 (trailing-newline ban) on the
//!   parsed template + peeled arg count. D3 (closed-subset
//!   enforcement) is discharged at parse time by
//!   `format::parse_format_template`. The public signature does not
//!   carry operand types or per-target context; richer diagnostics
//!   are exposed through the per-rule helpers below.
//!
//! - `validate_per_position_modifier_compat(specifier, operand_ty)`
//!   (`pub(crate)`) -- D2 (per-position modifier vs operand-type
//!   compatibility) per spec.md section 26.7 modifier table.
//!
//! - `validate_operand_type_admissibility(operand_ty)` (`pub(crate)`)
//!   -- D4 (operand source type admissibility) per spec.md section
//!   26.4 `From`-closed set.
//!
//! D6 (`args[0]` literal-ness at the MIR shape) is checked during
//! DiagLogPeel step 1 against the **pre-Regseq-const-lowering** rustc
//! MIR Constant value per spec.md section 26.8 D6; the
//! post-materialisation surface visible here serialises both
//! literal and const-reference forms to the same `Const::Aggregate`
//! shape per spec.md section 9.7, so D6 is intentionally not
//! re-checkable from this surface.
//!
//! D8 (`&str` operand count vs the selected binding target's
//! `max_str_args_per_call`) requires per-target binding state that
//! flows in via the build's target-selection flag; the helper
//! signature carries the bound as a `Option<usize>` so the
//! per-target wiring can compose D8 without re-deriving the bound
//! from a different surface.
//!
//! D5 (atomic-context reachability) is deferred to phase-2 stage 17
//! per spec.md section 26.8 "D5 deferred".

use regseq_model::{IntKind, Ty, UintKind};

use crate::bridge::{BridgeCtxt, BridgeError, DiagLogValidationDiagnostic};

use super::format::{FormatSpecifier, ParsedFormatTemplate};

/// Validate a peeled diagnostic-logging Call's call-site fingerprint
/// per spec.md section 26.8.
///
/// Returns `BridgeError::DiagLogValidation` if the peeled Call's
/// fingerprint violates D1 (specifier / argument arity) or D7
/// (trailing-newline ban). `arg_count` is the count of peeled
/// argument operands (i.e., `call.args.len() - 1`, excluding the
/// format-template Operand at index 0) per the `I-DiagLog-CallArity`
/// invariant at spec.md section 23.6.2 B-4 / B-6 exceptions.
pub fn validate_log_call(
    _cx: &BridgeCtxt<'_>,
    parsed_template: &ParsedFormatTemplate,
    arg_count: usize,
) -> Result<(), BridgeError> {
    validate_template_against_arg_count(parsed_template, arg_count)
}

/// Internal helper: drives D1 (specifier / argument arity) + D7
/// (trailing newline) on the parsed template, independent of any
/// `BridgeCtxt`. Split from the public surface so unit tests can
/// exercise the validation logic without constructing a rustc-backed
/// `BridgeCtxt`. The public `validate_log_call` is exercised
/// end-to-end through the integration-test harness at
/// `crates/regseq-driver/tests/bridge_diag_log_peel.rs`.
pub(crate) fn validate_template_against_arg_count(
    parsed_template: &ParsedFormatTemplate,
    arg_count: usize,
) -> Result<(), BridgeError> {
    // D1 -- specifier count vs argument count parity per spec.md
    // section 26.8 D1.
    if parsed_template.specifiers.len() != arg_count {
        return Err(BridgeError::DiagLogValidation {
            diagnostic: DiagLogValidationDiagnostic::D1,
            detail: format!(
                "format template has {} placeholder(s) but the peeled call provides {} argument(s)",
                parsed_template.specifiers.len(),
                arg_count,
            ),
        });
    }

    // D7 -- trailing newline forbidden per spec.md section 26.7 +
    // section 26.8 D7. The trailing literal segment (the last entry
    // of `literal_segments`) is the substring after the final
    // placeholder, or the entire template if there are no
    // placeholders.
    if let Some(tail) = parsed_template.literal_segments.last() {
        if ends_with_forbidden_newline(tail) {
            return Err(BridgeError::DiagLogValidation {
                diagnostic: DiagLogValidationDiagnostic::D7,
                detail: "format template ends in a newline; backends append the target-appropriate terminator (spec.md section 26.7)"
                    .to_string(),
            });
        }
    }

    Ok(())
}

/// D2 -- per-position modifier vs operand-type compatibility per
/// spec.md section 26.7 modifier table.
///
/// - `Display` specifier accepts any admissible operand type.
/// - `Hex` / `HexUpper` / `HexFormatted` specifiers accept only
///   integer-width operand types (`Ty::Uint(U8..U64)` /
///   `Ty::Int(I8..I64)`) per the modifier table.
///
/// `operand_kind` is the resolved `Ty` for the argument operand; the
/// caller resolves `body.local_decls[operand_local].ty` for
/// Move/Copy operands and the const's type for Constant operands.
///
pub(crate) fn validate_per_position_modifier_compat(
    specifier: &FormatSpecifier,
    operand_kind: &Ty,
) -> Result<(), BridgeError> {
    let requires_integer = matches!(
        specifier,
        FormatSpecifier::Hex | FormatSpecifier::HexUpper | FormatSpecifier::HexFormatted { .. },
    );
    if requires_integer && !is_integer_ty(operand_kind) {
        return Err(BridgeError::DiagLogValidation {
            diagnostic: DiagLogValidationDiagnostic::D2,
            detail: format!(
                "hex modifier requires an integer-width operand per spec.md section 26.7 modifier table, but operand resolves to {:?}",
                operand_kind,
            ),
        });
    }
    Ok(())
}

/// D4 -- per-operand source-Rust `Ty` admissibility per spec.md
/// section 26.4 `From`-closed set. The admissible value-typed shapes
/// after DiagLogPeel are `Ty::Uint(U8..U64)`, `Ty::Int(I8..I64)`,
/// `Ty::Bool`, and `Ty::Ref(Ty::Str, _)`. The reference-companion
/// `From<&T>` / `From<&mut T>` impls produce post-peel value-typed
/// loads via DiagLogPeel step 2 (spec.md section 25.7.3), so the
/// reference-companion source shapes never reach this validator.
///
/// `Ty::Ref` admissibility requires the pointee to be `Ty::Str`;
/// the helper resolves the pointee through the model's type
/// interner.
pub(crate) fn validate_operand_type_admissibility(
    model: &regseq_model::RegseqModel,
    operand_kind: &Ty,
) -> Result<(), BridgeError> {
    if is_admissible_logarg_ty(model, operand_kind) {
        return Ok(());
    }
    Err(BridgeError::DiagLogValidation {
        diagnostic: DiagLogValidationDiagnostic::D4,
        detail: format!(
            "operand type {:?} is outside the LogArg admissible set (spec.md section 26.4)",
            operand_kind,
        ),
    })
}

/// D8 -- `&str` operand count in args[1..] vs the selected binding
/// target's declared `max_str_args_per_call` per spec.md section
/// 26.8 D8. The bound is per-target; the helper accepts a
/// `Option<usize>` so callers without a target-binding context
/// (e.g., unit tests, or builds that haven't selected a target yet)
/// skip D8 enforcement until the target-binding wiring lands.
///
/// The helper is reachable through the target-binding integration
/// surface; the per-target bound is declared in the corresponding
/// `<target>-backend.md` artifact per spec.md section 26.9.
pub(crate) fn validate_str_operand_count_against_target_bound(
    str_operand_count: usize,
    target_bound: Option<usize>,
) -> Result<(), BridgeError> {
    let Some(bound) = target_bound else {
        return Ok(());
    };
    if str_operand_count <= bound {
        return Ok(());
    }
    Err(BridgeError::DiagLogValidation {
        diagnostic: DiagLogValidationDiagnostic::D8,
        detail: format!(
            "the call passes {str_operand_count} `&str` operand(s), exceeding the selected target's max_str_args_per_call bound of {bound}",
        ),
    })
}

/// Returns `true` if `ty` is one of the integer-width admissible
/// operand shapes (`Ty::Uint(U8..U64)` or `Ty::Int(I8..I64)`). Used
/// by D2 modifier-vs-operand-type compatibility checks.
fn is_integer_ty(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Uint(UintKind::U8)
            | Ty::Uint(UintKind::U16)
            | Ty::Uint(UintKind::U32)
            | Ty::Uint(UintKind::U64)
            | Ty::Int(IntKind::I8)
            | Ty::Int(IntKind::I16)
            | Ty::Int(IntKind::I32)
            | Ty::Int(IntKind::I64)
    )
}

/// Returns `true` if `ty` is in the post-peel LogArg admissible set
/// per spec.md section 26.4 / 26.6: value-typed scalar (integer or
/// bool) or `Ty::Ref(Ty::Str, _)`.
fn is_admissible_logarg_ty(model: &regseq_model::RegseqModel, ty: &Ty) -> bool {
    if is_integer_ty(ty) {
        return true;
    }
    if matches!(ty, Ty::Bool) {
        return true;
    }
    if let Ty::Ref(pointee_id, _) = ty {
        let pointee = model.types.resolve(*pointee_id);
        return matches!(pointee, Ty::Str);
    }
    false
}

/// Returns `true` if `s` ends in `"\n"`, `"\r"`, or `"\r\n"`. The
/// three sequences are the spec.md section 26.7 forbidden trailing
/// newline forms.
fn ends_with_forbidden_newline(s: &str) -> bool {
    s.ends_with('\n') || s.ends_with('\r')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::diag_log::format::{parse_format_template, FormatSpecifier};

    fn dummy_template(specifier_count: usize) -> ParsedFormatTemplate {
        let mut literal_segments = Vec::with_capacity(specifier_count + 1);
        for _ in 0..=specifier_count {
            literal_segments.push(String::new());
        }
        ParsedFormatTemplate {
            literal_segments,
            specifiers: vec![FormatSpecifier::Display; specifier_count],
        }
    }

    #[test]
    fn matching_specifier_and_argument_counts_validate() {
        let parsed = dummy_template(2);
        let result = validate_template_against_arg_count(&parsed, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn extra_argument_rejected_with_d1() {
        let parsed = dummy_template(1);
        let err = validate_template_against_arg_count(&parsed, 2).expect_err("D1 should fire");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D1);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn missing_argument_rejected_with_d1() {
        let parsed = dummy_template(2);
        let err = validate_template_against_arg_count(&parsed, 1).expect_err("D1 should fire");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D1);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn trailing_newline_rejected_with_d7() {
        let parsed = parse_format_template("hello\n").expect("template parses");
        let err = validate_template_against_arg_count(&parsed, 0)
            .expect_err("D7 should fire on trailing newline");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D7);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn embedded_newline_mid_template_accepted() {
        let parsed = parse_format_template("hello\nworld").expect("template parses");
        let result = validate_template_against_arg_count(&parsed, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn trailing_carriage_return_rejected_with_d7() {
        let parsed = parse_format_template("hello\r").expect("template parses");
        let err = validate_template_against_arg_count(&parsed, 0)
            .expect_err("D7 should fire on trailing carriage return");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D7);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn hex_modifier_rejects_bool_operand_per_d2() {
        let err = validate_per_position_modifier_compat(&FormatSpecifier::Hex, &Ty::Bool)
            .expect_err("D2 should fire on hex modifier vs bool operand");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D2);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn hex_modifier_accepts_uint32_operand_per_d2() {
        let result =
            validate_per_position_modifier_compat(&FormatSpecifier::Hex, &Ty::Uint(UintKind::U32));
        assert!(result.is_ok());
    }

    #[test]
    fn display_modifier_accepts_bool_operand() {
        let result = validate_per_position_modifier_compat(&FormatSpecifier::Display, &Ty::Bool);
        assert!(result.is_ok());
    }

    fn empty_model() -> regseq_model::RegseqModel {
        regseq_model::RegseqModel::with_primary("test", regseq_model::StableCrateId(1))
    }

    #[test]
    fn d4_admits_value_typed_integer() {
        let model = empty_model();
        let result = validate_operand_type_admissibility(&model, &Ty::Int(IntKind::I64));
        assert!(result.is_ok());
    }

    #[test]
    fn d4_admits_bool() {
        let model = empty_model();
        let result = validate_operand_type_admissibility(&model, &Ty::Bool);
        assert!(result.is_ok());
    }

    #[test]
    fn d4_admits_ref_to_str() {
        let mut model = empty_model();
        let str_ty = model.types.intern(Ty::Str);
        let ref_ty = Ty::Ref(str_ty, regseq_model::Provenance::SharedBorrow);
        let result = validate_operand_type_admissibility(&model, &ref_ty);
        assert!(result.is_ok());
    }

    #[test]
    fn d4_rejects_ref_to_non_str_pointee() {
        let mut model = empty_model();
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
        let ref_ty = Ty::Ref(u32_ty, regseq_model::Provenance::SharedBorrow);
        let err = validate_operand_type_admissibility(&model, &ref_ty)
            .expect_err("D4 should fire on Ref(u32)");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D4);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn d4_rejects_unit_tuple() {
        let model = empty_model();
        let unit = Ty::Tuple {
            fields: Vec::new(),
            layout: regseq_model::UNIT_LAYOUT,
        };
        let err =
            validate_operand_type_admissibility(&model, &unit).expect_err("D4 should fire on unit");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D4);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn d8_under_bound_accepts() {
        let result = validate_str_operand_count_against_target_bound(2, Some(4));
        assert!(result.is_ok());
    }

    #[test]
    fn d8_over_bound_rejects() {
        let err = validate_str_operand_count_against_target_bound(5, Some(4))
            .expect_err("D8 should fire when count exceeds bound");
        match err {
            BridgeError::DiagLogValidation { diagnostic, .. } => {
                assert_eq!(diagnostic, DiagLogValidationDiagnostic::D8);
            }
            other => panic!("expected DiagLogValidation, got {other:?}"),
        }
    }

    #[test]
    fn d8_no_bound_skips_check() {
        let result = validate_str_operand_count_against_target_bound(100, None);
        assert!(result.is_ok());
    }
}
