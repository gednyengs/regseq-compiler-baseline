//! SwitchInt target translation and loop recovery for Pass 2.
//!
//! Implemented by task regseq-model-44.
//!
//! This module extends the Pass 2 straight-line driver in
//! [`super::pass2`] with one translation rule: rustc
//! [`mir::TerminatorKind::SwitchInt`] maps directly onto Regseq IR's
//! [`regseq_model_old::ir::TerminatorKind::SwitchInt`]. The mapping follows
//! `docs/discussions/models/regseq-model/ir-reference.md` Section 5:
//!
//! - The discriminant operand is mapped through the shared task-42
//!   helper [`super::mir_mapping::map_operand`].
//! - The `(value, target)` pairs from the MIR
//!   [`mir::SwitchTargets`] are copied verbatim, preserving order.
//! - The MIR `otherwise()` fallback is appended as the last entry of
//!   [`regseq_model_old::ir::SwitchTargets::targets`], so the Regseq IR
//!   invariant `targets.len() == values.len() + 1` holds by
//!   construction.
//!
//! Target indices flow through Pass 2's block-remap table
//! ([`BasicBlockId`] lookups), which already handles cleanup-block
//! drops and contiguous renumbering. When a SwitchInt target was
//! dropped (i.e., the original MIR block was a cleanup block), the
//! translation raises [`ConstructionError::SwitchTargetIntoCleanup`]
//! rather than the generic "unsupported terminator" error used by
//! fallible-call targets.
//!
//! ### Loops
//!
//! There is no dedicated loop construct in the Regseq IR.
//! `while`/`for`/`loop` in source lower to a header block whose
//! terminator either is a `Goto` back onto itself (`loop {}`) or
//! ends with a `SwitchInt` over the condition, plus a trailing body
//! block whose `Goto` targets an earlier `BasicBlockId`. That
//! back-edge surfaces naturally once SwitchInt translation lands --
//! no loop-detection code is introduced. See
//! `ir-reference.md` Section 5 design rationale ("Loops ... are
//! represented as natural CFG back-edges.").

use rustc_hir::def_id::DefId;
use rustc_middle::mir;
use rustc_middle::ty::TyCtxt;

use regseq_model_old::identity::{BasicBlockId, SymbolInterner};
use regseq_model_old::ir::{SwitchTargets, TerminatorKind};

use super::mir_mapping::{map_operand, ConstantIndex, LocalIdMap, MapOperandResult, TypeDefIndex};
use super::{ConstructionError, ErrorProvenance};

/// Translate a rustc [`mir::TerminatorKind::SwitchInt`] into a Regseq
/// IR [`TerminatorKind::SwitchInt`].
///
/// `block_map` is the Pass 2 MIR-block-to-[`BasicBlockId`] table:
/// `Some` for preserved blocks, `None` for cleanup blocks that were
/// dropped. A `None` lookup for a SwitchInt target produces
/// [`ConstructionError::SwitchTargetIntoCleanup`] -- driver control
/// flow must not route into the cleanup subtree.
#[allow(clippy::too_many_arguments)]
pub(crate) fn translate_switch_int<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner_def_id: DefId,
    term: &mir::Terminator<'tcx>,
    discr: &mir::Operand<'tcx>,
    targets: &mir::SwitchTargets,
    block_map: &[Option<BasicBlockId>],
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<TerminatorKind, ConstructionError> {
    let discriminant = match map_operand(tcx, discr, locals, constants, type_def_index, interner)? {
        MapOperandResult::Mapped(o) => o,
        MapOperandResult::Drop => {
            return Err(ConstructionError::UnsupportedRvalue {
                provenance: ErrorProvenance {
                    item_path: tcx.def_path_str(owner_def_id),
                    span: term.source_info.span,
                },
                rendered: "SwitchInt discriminant dropped by map_operand".to_owned(),
            });
        }
    };

    let value_target_count = targets.iter().len();
    let mut values: Vec<u128> = Vec::with_capacity(value_target_count);
    let mut mapped_targets: Vec<BasicBlockId> = Vec::with_capacity(value_target_count + 1);
    for (value, target) in targets.iter() {
        values.push(value);
        mapped_targets.push(require_non_cleanup_target(
            tcx,
            owner_def_id,
            term,
            block_map,
            target,
        )?);
    }
    mapped_targets.push(require_non_cleanup_target(
        tcx,
        owner_def_id,
        term,
        block_map,
        targets.otherwise(),
    )?);

    Ok(TerminatorKind::SwitchInt {
        discriminant,
        targets: SwitchTargets {
            values,
            targets: mapped_targets,
        },
    })
}

fn require_non_cleanup_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner_def_id: DefId,
    term: &mir::Terminator<'tcx>,
    block_map: &[Option<BasicBlockId>],
    target: mir::BasicBlock,
) -> Result<BasicBlockId, ConstructionError> {
    match block_map.get(target.as_usize()).copied().flatten() {
        Some(bb) => Ok(bb),
        None => Err(ConstructionError::SwitchTargetIntoCleanup {
            provenance: ErrorProvenance {
                item_path: tcx.def_path_str(owner_def_id),
                span: term.source_info.span,
            },
            block_index: target.as_u32(),
        }),
    }
}
