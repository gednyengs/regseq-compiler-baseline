//! Timed-CPU-delay-and-sleep (TCD construct) frontend validation.
//!
//! The construct-neutral intrinsic recognition (`bridge::intrinsic`) maps the
//! `regseq::delay::{busy_wait,sleep}` facade to `IntrinsicKind::BusyWait`/
//! `Sleep` and records the marker. This module adds the family-specific
//! signature obligation, `TCD-ERR-02` (C03/R13/D11): a recognized
//! `busy_wait`/`sleep` intrinsic must take exactly one **by-value**
//! `regseq::Duration` framework-type argument.
//!
//! Rust's own type-checker subsumes this for the *canonical* typed facade — a
//! mistyped *call* fails before the bridge runs (which executes in
//! `after_analysis`). The residual reachable case is a **mis-declared** facade:
//! a function tagged `#[rseq::intrinsic("busy_wait")]` whose parameter is not a
//! `Duration`. Validating the recognized callee's signature mirrors
//! device-interrupt's `IRQ-ERR-ISR-SIGNATURE` check (`bridge/interrupt`).

use regseq_model::{
    AttrData, Body, CallCallee, FrameworkType, Instance, InstanceKind, IntrinsicKind, ItemId,
    RegseqModel, SyntheticItemKind, TerminatorKind,
};

use super::framework_type::{recognise_framework_type_attr, recover_source_def_id};
use super::{BridgeCtxt, BridgeError, TimedCpuDelayDiagnostic};

/// The author-facing facade path for a recognized delay intrinsic kind, used
/// in the `TCD-ERR-02` rendering.
fn facade_path(kind: &IntrinsicKind) -> &'static str {
    match kind {
        IntrinsicKind::BusyWait => "regseq::delay::busy_wait",
        IntrinsicKind::Sleep => "regseq::delay::sleep",
        // Only the two delay kinds reach this module.
        _ => "regseq::delay::<intrinsic>",
    }
}

/// Validate the signature of every recognized `busy_wait`/`sleep` intrinsic
/// facade: exactly one by-value `Duration` framework-type parameter
/// (`TCD-ERR-02`).
///
/// Runs in Phase 4 after `intrinsic::materialise_intrinsics` has recorded the
/// `IntrinsicKind::BusyWait`/`Sleep` markers, so it walks the recorded markers
/// rather than re-discovering callees. Recognition is call-driven, so a
/// mis-declared facade that is never called is never recognized and never
/// validated — harmless, since it has no call site to lower.
pub(crate) fn validate_delay_signatures(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        let Some(kind) = attrs.iter().find_map(|a| match a {
            AttrData::Intrinsic(k @ (IntrinsicKind::BusyWait | IntrinsicKind::Sleep)) => Some(k),
            _ => None,
        }) else {
            continue;
        };
        let item_id = ItemId::from_u32(idx as u32);
        // Synthetic Items carry the reserved StableCrateId(0) whose hash panics
        // inside `def_path_hash_to_def_id`; a delay facade is always a source
        // function, so skip synthetics defensively before recovery.
        if !matches!(
            cx.model.item_synthetic_kinds[item_id],
            SyntheticItemKind::None
        ) {
            continue;
        }
        let Some(def_id) = recover_source_def_id(tcx, cx.model.item_def_paths[item_id]) else {
            continue;
        };

        let path = facade_path(kind);
        let sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        // Arity: exactly one parameter. A bogus `fn busy_wait()` or
        // `fn busy_wait(Duration, u32)` is Rust-callable against its own
        // signature, so the count is part of the contract.
        if inputs.len() != 1 {
            return Err(BridgeError::TimedCpuDelayValidation(
                TimedCpuDelayDiagnostic::ArgTypeMismatch {
                    intrinsic: path.to_string(),
                    detail: format!("found {} parameters, expected 1", inputs.len()),
                },
            ));
        }
        // Type: a by-value `Duration` framework type. `ty_adt_def()` is `None`
        // for a reference (`&Duration`), a primitive (`u64`), or a generic
        // param, and `Option<Duration>`'s ADT is `Option` (whose attribute is
        // not `duration`), so each of those is rejected.
        let is_duration = inputs[0].ty_adt_def().is_some_and(|adt| {
            recognise_framework_type_attr(tcx, adt.did()) == Some(FrameworkType::Duration)
        });
        if !is_duration {
            return Err(BridgeError::TimedCpuDelayValidation(
                TimedCpuDelayDiagnostic::ArgTypeMismatch {
                    intrinsic: path.to_string(),
                    detail: format!("parameter type `{}` is not `regseq::Duration`", inputs[0]),
                },
            ));
        }
    }
    Ok(())
}

/// Whether `item` carries the attribute `pred` selects (bounds-checked, so a
/// late callee ItemId beyond the attr table is treated as unmarked).
fn item_attr_matches(model: &RegseqModel, item: ItemId, pred: impl Fn(&AttrData) -> bool) -> bool {
    item.index() < model.domain_attrs.item_attrs.len()
        && model.domain_attrs.item_attrs[item].iter().any(pred)
}

/// The materialized body of a function instance, if any.
fn function_body(instance: &Instance) -> Option<&Body> {
    match &instance.kind {
        InstanceKind::Function(data) => data.body.as_ref(),
        _ => None,
    }
}

/// Frontend context-legality check `TCD-ERR-01` (C05/D05): reject a
/// `sleep`-kind intrinsic call that occurs in the body of an `#[rseq::isr]`-
/// marked handler (sleeping in interrupt context). `busy_wait` is never
/// rejected (I07).
///
/// Runs **after** `interrupt::finalise_isrs` — the only point where both the
/// handler bodies are materialized (the Phase-5 drain) AND the
/// `AttrData::IsrBinding` markers are recorded — and reads the `Sleep`
/// intrinsic markers recorded in Phase 4. Rejecting here aborts the bridge, so
/// no IR reaches a backend for the offending spec (I06).
///
/// Phase-1 enforcement is **marking-based**: it covers a `sleep` call
/// lexically within the handler body. A `sleep` hidden one helper-call deeper
/// (a callee the marking does not yet cover) escapes until the full-callgraph
/// reachability hardening lands (GRILL O1) — the thesis text must not claim
/// full static sleep-in-ISR enforcement before then.
pub(crate) fn validate_isr_no_sleep(model: &RegseqModel) -> Result<(), BridgeError> {
    for instance in model.instances.iter() {
        // Only `#[rseq::isr]`-marked handler functions with a materialized body.
        if !item_attr_matches(model, instance.item, |a| {
            matches!(a, AttrData::IsrBinding { .. })
        }) {
            continue;
        }
        let Some(body) = function_body(instance) else {
            continue;
        };
        for block in body.basic_blocks.iter() {
            // A directly-resolved call to a `Sleep`-marked intrinsic facade.
            let TerminatorKind::Call {
                callee: CallCallee::Direct(callee_id),
                ..
            } = &block.terminator.kind
            else {
                continue;
            };
            let callee_item = model.instances[*callee_id].item;
            if item_attr_matches(model, callee_item, |a| {
                matches!(a, AttrData::Intrinsic(IntrinsicKind::Sleep))
            }) {
                return Err(BridgeError::TimedCpuDelayValidation(
                    TimedCpuDelayDiagnostic::SleepInIsr {
                        intrinsic: "regseq::delay::sleep".to_string(),
                        context_kind: "rseq::isr".to_string(),
                    },
                ));
            }
        }
    }
    Ok(())
}
