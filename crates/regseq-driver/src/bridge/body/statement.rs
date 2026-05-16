//! MIR `Statement` bridge translation.

use regseq_model::{
    CopyNonOverlapping, NonDivergingIntrinsic, SourceFile, SourceFileId, Span, Statement,
    StatementKind, VariantIdx,
};
use rustc_middle::mir;
use rustc_span::Pos;

use crate::bridge::{BridgeCtxt, BridgeError, DslRejectionKind};

use super::{
    operand::translate_operand, place::translate_place, rvalue::translate_rvalue, BodyCtxt,
};

/// Translate a rustc `mir::Statement` into a `regseq_model::Statement`.
/// Per spec.md section 22.6 and section 22.5.1 statement-site rejections.
pub fn translate_statement<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    stmt: mir::Statement<'tcx>,
) -> Result<Option<Statement>, BridgeError> {
    let kind = match stmt.kind {
        mir::StatementKind::Assign(box (place, rvalue)) => {
            if matches!(rvalue, mir::Rvalue::Ref(_, mir::BorrowKind::Fake(_), _)) {
                return rejected_statement("Assign with fake borrow");
            }
            StatementKind::Assign(
                translate_place(cx, body_cx, place)?,
                translate_rvalue(cx, body_cx, rvalue)?,
            )
        }
        mir::StatementKind::SetDiscriminant {
            box place,
            variant_index,
        } => StatementKind::SetDiscriminant(
            translate_place(cx, body_cx, place)?,
            VariantIdx::from_u32(variant_index.as_u32()),
        ),
        mir::StatementKind::Intrinsic(box intrinsic) => {
            StatementKind::Intrinsic(translate_non_diverging_intrinsic(cx, body_cx, intrinsic)?)
        }
        mir::StatementKind::FakeRead(_) => {
            return rejected_statement(
                "FakeRead: NOPed by CleanupPostBorrowck and swept by required cleanup passes",
            );
        }
        mir::StatementKind::AscribeUserType(_, _) => {
            return rejected_statement("AscribeUserType: NOPed by CleanupPostBorrowck");
        }
        mir::StatementKind::BackwardIncompatibleDropHint { .. } => {
            return rejected_statement(
                "BackwardIncompatibleDropHint: lint-only Nop swept by cleanup passes",
            );
        }
        mir::StatementKind::Coverage(_) => {
            return rejected_statement(
                "Coverage: cleanup-discharged marker or unsupported instrumentation statement",
            );
        }
        mir::StatementKind::PlaceMention(_) => {
            return rejected_statement("PlaceMention: removed by required RemovePlaceMention");
        }
        mir::StatementKind::Nop => {
            return rejected_statement("Nop: swept by required cleanup passes");
        }
        mir::StatementKind::StorageLive(_) => {
            return rejected_statement("StorageLive: removed by whitelisted RemoveStorageMarkers");
        }
        mir::StatementKind::StorageDead(_) => {
            return rejected_statement("StorageDead: removed by whitelisted RemoveStorageMarkers");
        }
        mir::StatementKind::Retag(_, _) => {
            return rejected_statement(
                "Retag: only emitted with -Z mir-emit-retag, outside bridge compilation contract",
            );
        }
        mir::StatementKind::ConstEvalCounter => {
            return rejected_statement(
                "ConstEvalCounter: only injected for CTFE bodies, not optimized runtime MIR",
            );
        }
    };

    Ok(Some(Statement {
        kind,
        span: translate_span(cx, stmt.source_info.span),
    }))
}

fn translate_non_diverging_intrinsic<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    intrinsic: mir::NonDivergingIntrinsic<'tcx>,
) -> Result<NonDivergingIntrinsic, BridgeError> {
    match intrinsic {
        mir::NonDivergingIntrinsic::Assume(operand) => Ok(NonDivergingIntrinsic::Assume(
            translate_operand(cx, body_cx, operand)?,
        )),
        mir::NonDivergingIntrinsic::CopyNonOverlapping(copy) => Ok(
            NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping {
                src: translate_operand(cx, body_cx, copy.src)?,
                dst: translate_operand(cx, body_cx, copy.dst)?,
                count: translate_operand(cx, body_cx, copy.count)?,
            }),
        ),
    }
}

pub(crate) fn translate_span(cx: &mut BridgeCtxt<'_>, span: rustc_span::Span) -> Span {
    if span.is_dummy() {
        return Span::dummy();
    }

    let source_map = cx.input.tcx.sess.source_map();
    let file = source_map.lookup_source_file(span.lo());
    let path = file.name.prefer_local_unconditionally().to_string();
    let file_id = intern_source_file(cx, &file, &path);
    let lo = file.relative_position(span.lo()).to_u32();
    let hi = file.relative_position(span.hi()).to_u32();

    Span {
        file: file_id,
        lo,
        hi,
    }
}

fn intern_source_file(
    cx: &mut BridgeCtxt<'_>,
    file: &rustc_span::SourceFile,
    path: &str,
) -> SourceFileId {
    for (idx, source_file) in cx.model.source_files.iter().enumerate() {
        if cx.model.strings.resolve(source_file.path) == path {
            return SourceFileId::from_u32(idx as u32);
        }
    }

    let path = cx.model.strings.intern(path);
    let content = file
        .src
        .as_ref()
        .map(|src| std::sync::Arc::<str>::from(src.as_str()));
    let line_starts = file.lines().iter().map(|pos| pos.to_u32()).collect();
    cx.model.source_files.push(SourceFile {
        path,
        content,
        line_starts,
    })
}

fn rejected_statement<T>(construct: impl Into<String>) -> Result<T, BridgeError> {
    Err(BridgeError::DslRejection {
        kind: DslRejectionKind::Statement,
        ty_repr: construct.into(),
    })
}
