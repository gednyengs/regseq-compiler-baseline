//! MIR basic-block bridge translation.

use regseq_model::{BasicBlock, Statement};

use crate::bridge::{BridgeCtxt, BridgeError, DslRejectionKind};

use super::{statement::translate_statement, terminator::translate_terminator, BodyCtxt};

/// Translate a rustc `mir::BasicBlockData` into a
/// `regseq_model::BasicBlock`. Per spec.md section 24.7.
///
/// Audit-finding regseq-driver-42-1 (spec section 3.4 Phase 5): the
/// statements list is built into a mutable `Vec<Statement>` and
/// threaded through `translate_terminator` so the Drop-as-Call
/// collapse can append the `&raw mut place` structural prelude per
/// spec section 23.4 step 4 before the statement list is finalised
/// into a `Box<[Statement]>`.
pub fn translate_basic_block<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    bb: &rustc_middle::mir::BasicBlockData<'tcx>,
) -> Result<regseq_model::BasicBlock, BridgeError> {
    if bb.is_cleanup {
        return rejected_basic_block(
            "internal error: is_cleanup=true block survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy",
        );
    }
    let Some(terminator) = bb.terminator.clone() else {
        return rejected_basic_block(
            "internal error: terminator: None survived MIR construction -- pipeline misconfiguration or post-construction state corruption",
        );
    };
    let mut statements: Vec<Statement> = Vec::with_capacity(bb.statements.len());
    for stmt in bb.statements.iter().cloned() {
        if let Some(translated) = translate_statement(cx, body_cx, stmt)? {
            statements.push(translated);
        }
    }
    let terminator = translate_terminator(cx, body_cx, &mut statements, terminator)?;
    Ok(BasicBlock {
        statements: statements.into_boxed_slice(),
        terminator,
    })
}

fn rejected_basic_block<T>(construct: impl Into<String>) -> Result<T, BridgeError> {
    Err(BridgeError::DslRejection {
        kind: DslRejectionKind::BasicBlock,
        ty_repr: construct.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::rejected_basic_block;
    use crate::bridge::{BridgeError, DslRejectionKind};

    #[test]
    fn rejected_basic_block_uses_basic_block_rejection_kind() {
        let err = rejected_basic_block::<()>("bad block").unwrap_err();
        assert!(matches!(
            err,
            BridgeError::DslRejection {
                kind: DslRejectionKind::BasicBlock,
                ..
            }
        ));
    }
}
