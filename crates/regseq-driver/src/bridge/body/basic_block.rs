//! MIR basic-block bridge translation.

use regseq_model::BasicBlock;

use crate::bridge::{BridgeCtxt, BridgeError, DslRejectionKind};

use super::{statement::translate_statement, terminator::translate_terminator, BodyCtxt};

/// Translate a rustc `mir::BasicBlockData` into a
/// `regseq_model::BasicBlock`. Per spec.md section 24.7. Iterates
/// statements, then translates the block terminator with a mutable
/// reference to the statement vector so terminator-side rewrites
/// (notably the Drop-as-Call collapse in spec.md section 23.4) can
/// append the spec-mandated `&raw mut place` synthetic Assign before
/// the Call terminator.
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
    let mut statements = bb
        .statements
        .iter()
        .cloned()
        .filter_map(|stmt| translate_statement(cx, body_cx, stmt).transpose())
        .collect::<Result<Vec<_>, _>>()?;
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
