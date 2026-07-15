//! Locked device-command-transport validation.
//!
//! The construct is one call intrinsic, not a resource table: endpoint and
//! payload values are supplied by L2 and remain ordinary operands. The only
//! retained IR fact is `IntrinsicKind::CommandTransportSubmit` on the callee.

use regseq_model::{
    AttrData, CallCallee, IntrinsicKind, ItemId, LocalId, Place, RegseqModel, StatementKind,
    SyntheticItemKind, TerminatorKind, Visitor,
};
use rustc_middle::ty::{self, TyKind};

use super::framework_type::recover_source_def_id;
use super::{BridgeCtxt, BridgeError, TransportDiagnostic};

/// Validate the one locked facade signature:
///
/// `submit(resolved_endpoint: usize, payload: usize) -> SubmissionOutcome`
///
/// The two words are opaque L2 carriers. The compiler never interprets or
/// stores either value. `SubmissionOutcome` has exactly `Accepted` and
/// `Rejected(isize)`; the signed word preserves the backend-native reason
/// without creating a portable reason enumeration.
pub(crate) fn validate_submit_signatures(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    let tcx = cx.input.tcx;
    for (idx, attrs) in cx.model.domain_attrs.item_attrs.iter().enumerate() {
        if !attrs.iter().any(|attr| {
            matches!(
                attr,
                AttrData::Intrinsic(IntrinsicKind::CommandTransportSubmit)
            )
        }) {
            continue;
        }
        let item = ItemId::from_u32(idx as u32);
        if !matches!(cx.model.item_synthetic_kinds[item], SyntheticItemKind::None) {
            continue;
        }
        let Some(def_id) = recover_source_def_id(tcx, cx.model.item_def_paths[item]) else {
            continue;
        };
        let sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        if inputs.len() != 2
            || !matches!(inputs[0].kind(), TyKind::Uint(ty::UintTy::Usize))
            || !matches!(inputs[1].kind(), TyKind::Uint(ty::UintTy::Usize))
        {
            return bad_signature(format!(
                "expected two opaque word operands `(resolved_endpoint: usize, payload: usize)`, found `{sig}`"
            ));
        }

        let output = sig.output();
        let Some(adt) = output.ty_adt_def() else {
            return bad_signature(format!(
                "return type `{output}` is not `SubmissionOutcome`"
            ));
        };
        if !adt.is_enum() || tcx.item_name(adt.did()).as_str() != "SubmissionOutcome" {
            return bad_signature(format!(
                "return type `{output}` is not the locked `SubmissionOutcome` enum"
            ));
        }
        let variants = adt.variants();
        let accepted = variants.iter().next();
        let rejected = variants.iter().nth(1);
        let exact = variants.len() == 2
            && accepted.is_some_and(|v| v.name.as_str() == "Accepted" && v.fields.is_empty())
            && rejected.is_some_and(|v| {
                v.name.as_str() == "Rejected"
                    && v.fields.len() == 1
                    && v.fields.iter().next().is_some_and(|field| {
                        matches!(
                            tcx.type_of(field.did).instantiate_identity().kind(),
                            TyKind::Int(ty::IntTy::Isize)
                        )
                    })
            });
        if !exact {
            return bad_signature(
                "`SubmissionOutcome` must contain exactly `Accepted` and `Rejected(isize)`"
                    .into(),
            );
        }
    }
    Ok(())
}

fn bad_signature(detail: String) -> Result<(), BridgeError> {
    Err(BridgeError::TransportValidation(
        TransportDiagnostic::BadSignature { detail },
    ))
}

/// Reject a submit whose synchronous admission result is thrown away. This is
/// deliberately a use check, not a completion check: the consumed value is
/// only `Accepted | Rejected(native_reason)` at the call site.
pub(crate) fn validate_submit_results(model: &RegseqModel) -> Result<(), BridgeError> {
    for instance in model.instances.iter() {
        let Some(body) = instance.kind.as_function().and_then(|data| data.body.as_ref()) else {
            continue;
        };
        for block in body.basic_blocks.iter() {
            let TerminatorKind::Call {
                callee: CallCallee::Direct(callee),
                destination,
                ..
            } = &block.terminator.kind
            else {
                continue;
            };
            if !matches!(
                model.instance_intrinsic(*callee),
                Some(IntrinsicKind::CommandTransportSubmit)
            ) {
                continue;
            }
            let local = destination.local;
            if local != LocalId::RETURN_PLACE && !local_is_read(body, local) {
                return Err(BridgeError::TransportValidation(
                    TransportDiagnostic::DiscardedOutcome,
                ));
            }
        }
    }
    Ok(())
}

fn local_is_read(body: &regseq_model::Body, local: LocalId) -> bool {
    struct ReadsLocal {
        local: LocalId,
        read: bool,
    }
    impl Visitor for ReadsLocal {
        fn visit_place(&mut self, place: &Place) {
            if place.local == self.local {
                self.read = true;
            }
            self.super_place(place);
        }

        fn visit_statement(&mut self, statement: &regseq_model::Statement) {
            match &statement.kind {
                StatementKind::Assign(_, value) => self.visit_rvalue(value),
                StatementKind::SetDiscriminant(_, _) => {}
                StatementKind::Intrinsic(_) => self.super_statement(statement),
            }
        }

        fn visit_terminator(&mut self, terminator: &regseq_model::Terminator) {
            match &terminator.kind {
                TerminatorKind::Call { callee, args, .. } => {
                    self.visit_call_callee(callee);
                    for arg in args.iter() {
                        self.visit_operand(arg);
                    }
                }
                _ => self.super_terminator(terminator),
            }
        }
    }

    let mut reads = ReadsLocal { local, read: false };
    reads.visit_body(body);
    reads.read
}
