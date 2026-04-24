//! Per-family helpers that enumerate `IntrinsicCall` terminators.
//!
//! Each helper walks the full module tree + every function body, and
//! collects `(FunctionId, BasicBlockId)` pairs whose terminator is an
//! `IntrinsicCall` matching the family predicate. Results are sorted
//! by `(FunctionId, BasicBlockId)` so iteration is deterministic.
//!
//! Complexity is O(F * B) where F is the number of functions in the
//! model and B is the number of basic blocks per function. The PL011
//! corpus has well under 10K blocks total; for the Phase-7 target
//! workload this is adequate without further indexing.

use crate::identity::{BasicBlockId, FunctionId};
use crate::ir::intrinsic::{IntrinsicMetadata, RegseqIntrinsic};
use crate::ir::terminator::TerminatorKind;
use crate::model::regseq_model::RegseqModel;

use super::reachability::collect_functions;

/// A located intrinsic call site.
///
/// Records the terminator's intrinsic kind, the function and basic
/// block it lives in, and a borrow of any attached [`IntrinsicMetadata`].
/// The `'a` lifetime ties the metadata borrow to the model the site
/// was collected from.
///
/// Helpers produce `CallSite` values in deterministic order, sorted
/// by `(function_id, block_id)`.
#[derive(Debug, Clone)]
pub struct CallSite<'a> {
    /// Function containing the terminator.
    pub function_id: FunctionId,
    /// Basic block whose terminator is the intrinsic call.
    pub block_id: BasicBlockId,
    /// Optional structured metadata carried by the terminator.
    pub metadata: Option<&'a IntrinsicMetadata>,
    /// The intrinsic invoked at this call site.
    pub intrinsic: RegseqIntrinsic,
}

/// Return all register-wrapper call sites in the model.
///
/// A register-wrapper call site is any `IntrinsicCall` terminator whose
/// intrinsic is a member of the register-access family
/// ([`RegseqIntrinsic::RegisterRead`], [`RegseqIntrinsic::RegisterReadField`],
/// [`RegseqIntrinsic::RegisterWrite`], [`RegseqIntrinsic::RegisterWriteField`]).
pub fn wrapper_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, is_register_access)
}

/// Return all poll call sites in the model.
///
/// Matches `IntrinsicCall` terminators whose intrinsic is
/// [`RegseqIntrinsic::PollUntil`].
pub fn poll_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |k| matches!(k, RegseqIntrinsic::PollUntil))
}

/// Return all logging call sites in the model.
///
/// Matches `IntrinsicCall` terminators whose intrinsic is any of the
/// four logging variants ([`RegseqIntrinsic::LogError`],
/// [`RegseqIntrinsic::LogWarn`], [`RegseqIntrinsic::LogInfo`],
/// [`RegseqIntrinsic::LogDebug`]).
pub fn log_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, is_log)
}

/// Return all timed-delay call sites in the model.
///
/// Matches `IntrinsicCall` terminators whose intrinsic is
/// [`RegseqIntrinsic::BusyWait`] or [`RegseqIntrinsic::Sleep`].
pub fn timed_delay_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |k| {
        matches!(k, RegseqIntrinsic::BusyWait | RegseqIntrinsic::Sleep)
    })
}

/// Return all completion-operation call sites in the model.
///
/// The CFG IR does not currently model completion operations as
/// first-class intrinsics (no matching [`RegseqIntrinsic`] variant
/// exists). Today this helper returns an empty vector; when a
/// `Completion*` family is added to [`RegseqIntrinsic`], this helper
/// will extend its predicate to cover those variants without any
/// caller-visible change.
pub fn completion_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |_| false)
}

/// Return all lock-operation call sites in the model.
///
/// Like [`completion_call_sites`], the CFG IR does not yet model lock
/// primitives as first-class intrinsics. This helper returns an empty
/// vector today; predicate coverage will expand when `Lock*` variants
/// are added to [`RegseqIntrinsic`].
pub fn lock_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |_| false)
}

/// Return all duration-constructor call sites in the model.
///
/// Duration constructors (e.g., `Duration::from_secs(..)`) lower to
/// ordinary [`TerminatorKind::Call`] invocations of user-visible
/// constructor functions rather than to [`TerminatorKind::IntrinsicCall`]
/// terminators -- the CFG IR has no `DurationCtor*` variant in
/// [`RegseqIntrinsic`]. This helper therefore returns an empty vector
/// today. Deferred to a follow-on task if a first-class intrinsic is
/// introduced.
pub fn duration_ctor_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |_| false)
}

/// Return all PM-gate call sites in the model.
///
/// The CFG IR has no `PmGate*` variant in [`RegseqIntrinsic`], so this
/// helper returns an empty vector today. Deferred to a follow-on task
/// if the family is re-expressed as an intrinsic-call site.
pub fn pm_gate_call_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, |_| false)
}

/// Return all register-access call sites in the model.
///
/// Structurally equivalent to [`wrapper_call_sites`] -- both report the
/// four register-access intrinsic variants. Exposed under a second name
/// for backends that prefer the `register_access_sites` vocabulary over
/// the lower-level "wrapper" terminology.
pub fn register_access_sites(model: &RegseqModel) -> Vec<CallSite<'_>> {
    collect_call_sites(model, is_register_access)
}

// -- Internal helpers --------------------------------------------------

fn is_register_access(kind: RegseqIntrinsic) -> bool {
    matches!(
        kind,
        RegseqIntrinsic::RegisterRead
            | RegseqIntrinsic::RegisterReadField
            | RegseqIntrinsic::RegisterWrite
            | RegseqIntrinsic::RegisterWriteField
    )
}

fn is_log(kind: RegseqIntrinsic) -> bool {
    matches!(
        kind,
        RegseqIntrinsic::LogError
            | RegseqIntrinsic::LogWarn
            | RegseqIntrinsic::LogInfo
            | RegseqIntrinsic::LogDebug
    )
}

/// Walk every function in the module tree and collect the
/// `IntrinsicCall` terminators whose intrinsic passes `predicate`.
///
/// The result is sorted by `(FunctionId, BasicBlockId)` before
/// returning so two calls on the same model produce byte-identical
/// output (spec "determinism rule").
fn collect_call_sites<P>(model: &RegseqModel, predicate: P) -> Vec<CallSite<'_>>
where
    P: Fn(RegseqIntrinsic) -> bool,
{
    let mut out: Vec<CallSite<'_>> = Vec::new();
    for function in collect_functions(&model.root_module) {
        for (block_index, block) in function.body.basic_blocks.iter().enumerate() {
            if let TerminatorKind::IntrinsicCall {
                intrinsic,
                metadata,
                ..
            } = &block.terminator.kind
            {
                if predicate(*intrinsic) {
                    out.push(CallSite {
                        function_id: function.id,
                        block_id: BasicBlockId(block_index as u32),
                        metadata: metadata.as_ref(),
                        intrinsic: *intrinsic,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.function_id
            .cmp(&b.function_id)
            .then(a.block_id.cmp(&b.block_id))
    });
    out
}
