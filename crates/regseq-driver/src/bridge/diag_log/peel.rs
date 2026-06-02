//! DiagLogPeel body-level pass per spec.md section 25.7.3 and
//! section 26.6.
//!
//! Walks every Call terminator in the materialised Body whose callee
//! Item carries `AttrData::Intrinsic(IntrinsicKind::Log(_))` (set by
//! the regseq-driver-44 recognition layer). For each such Call,
//! locates the slice-aggregate LogArg carrier the macro expansion
//! materialised at `args[1]`, decomposes it into the per-specifier
//! argument operands, and replaces the Call's argument vector with
//! the flat-arg form `[fmt_literal, expr_1, ..., expr_N]` per the
//! post-peel `I-DiagLog-CallArity` shape at spec.md section 23.6.2
//! B-4 / B-6 exceptions.
//!
//! Runs during Phase 5 after the vtable-indirect rewrite per
//! spec.md section 25.7.4 and before the joint validation walk so
//! that B-4 / B-6 see the peeled flat-arg shape.

use std::collections::HashSet;

use regseq_model::{
    AggregateKind, AttrData, BlockId, Body, CallCallee, InstanceId, IntrinsicKind, LocalId,
    Operand, Place, PlaceElem, RegseqModel, Rvalue, Statement, StatementKind, TerminatorKind, Ty,
    TyId, UNIT_LAYOUT,
};

use super::validation::{
    validate_operand_type_admissibility, validate_per_position_modifier_compat,
};

use crate::bridge::{BridgeCtxt, BridgeError, DiagLogValidationDiagnostic};

/// Body-level pass that peels the slice-aggregate LogArg carrier
/// into a flat-arg Call per spec.md section 25.7.3 + section 26.6.
///
/// Returns `Ok(())` after the pass completes. Bridge-internal
/// shape mismatches on Log-intrinsic Calls surface
/// `BridgeError::Phase5(...)` per spec.md section 25.7.3 step 1
/// ("Shape mismatches ... are bridge-internal invariant violations:
/// panic with a `bug!`-style diagnostic citing the offending MIR
/// shape"). The bridge surfaces these as Phase5 errors rather than
/// panicking so the test harness can observe them.
///
/// After producing the post-peel flat-arg Call shape, the pass
/// recovers the call-site fingerprint per spec.md section 25.7.3
/// step 5 and validates it via
/// `validation::validate_log_call`. The fingerprint discharge
/// requires the format-template literal at args[0]; the literal is
/// recovered best-effort from `cx.const_memory_static_cache` (the
/// const-lowering side table that holds the literal bytes per
/// spec.md section 9.7 / 26.6 transient-fingerprint discipline).
/// When the literal cannot be recovered (cross-crate const
/// references, non-cached lowerings) the fingerprint validation
/// step is skipped; D4 (operand admissibility) still runs.
pub fn diag_log_peel_pass(cx: &mut BridgeCtxt<'_>, body: &mut Body) -> Result<(), BridgeError> {
    peel_body_against_model_with_cx(cx, body)
}

/// Public-surface entry that resolves the format-template literal
/// through `cx.const_memory_static_cache` so the per-call fingerprint
/// validation can run. Mirrors `peel_body_against_model` for the
/// inner mutation; adds the literal lookup + validation step. Pushes
/// every collapsed wrapper Call's callee `InstanceId` onto the
/// bridge-side `DiagLogPeelResidue` accumulator per spec.md section
/// 26.6 + regseq-driver-48 Change A.
fn peel_body_against_model_with_cx(
    cx: &mut BridgeCtxt<'_>,
    body: &mut Body,
) -> Result<(), BridgeError> {
    let collapsed = peel_body_against_model(&mut cx.model, body)?;
    cx.diag_log_peel_residue
        .collapsed_wrapper_instances
        .extend(collapsed);
    validate_log_call_fingerprints(cx, body)
}

/// Walk every post-peel Log-intrinsic Call and run
/// `validate_log_call` against the recovered template + peeled
/// arg count per spec.md section 25.7.3 step 5 + section 26.8 D1 /
/// D7. The template literal is recovered best-effort through the
/// `cx.const_memory_static_cache` side table; calls whose literal
/// cannot be recovered skip fingerprint validation (D4 has already
/// run at peel time).
fn validate_log_call_fingerprints(cx: &BridgeCtxt<'_>, body: &Body) -> Result<(), BridgeError> {
    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        let block_id = BlockId::from_u32(block_idx as u32);
        if !call_is_log_intrinsic(&cx.model, body, block_id) {
            continue;
        }
        let TerminatorKind::Call { args, .. } = &block.terminator.kind else {
            continue;
        };
        // D6 (partial) -- args[0] must be a `Operand::Constant`
        // per spec.md section 26.8 D6. The strict literal-vs-
        // const-ref distinction requires pre-Regseq-const-lowering
        // MIR access (the post-lowering surface visible here
        // serialises both forms to `Const::Aggregate([Reloc, Uint])`
        // per spec.md section 9.7); a runtime-computed format
        // operand (`Operand::Move` / `Operand::Copy`) is rejected
        // here as a trivial D6 violation. Strict pre-lowering D6
        // enforcement lives at the bridge's per-rustc-terminator
        // translation envelope (covered by the recognition layer's
        // attribute-population step).
        let template_const_id = match args.first() {
            Some(Operand::Constant(id)) => *id,
            Some(other) => {
                return Err(BridgeError::DiagLogValidation {
                    diagnostic: DiagLogValidationDiagnostic::D6,
                    detail: format!(
                        "Log-intrinsic Call at block {} has a non-Constant format-template operand {:?} (spec.md section 26.8 D6)",
                        block_id.as_u32(),
                        other,
                    ),
                });
            }
            None => continue,
        };
        let Some(template) = extract_format_template_literal(cx, template_const_id) else {
            continue;
        };
        let parsed = super::format::parse_format_template(&template)?;
        // Peeled argument count excludes args[0] (the format template).
        let peeled_arg_count = args.len().saturating_sub(1);
        super::validation::validate_log_call(cx, &parsed, peeled_arg_count)?;
        // D2 -- per-position modifier vs operand-type compatibility
        // per spec.md section 26.7 modifier table. Walks each
        // (specifier, args[i+1]) pair; skips operands whose type is
        // not recoverable.
        for (idx, specifier) in parsed.specifiers.iter().enumerate() {
            let Some(arg) = args.get(idx + 1) else {
                break;
            };
            let Some(resolved) = operand_ty(&cx.model, body, arg) else {
                continue;
            };
            validate_per_position_modifier_compat(specifier, &resolved)?;
        }

        // D8 -- string-arg bound per spec.md section 26.8 D8. Count
        // post-peel `Ty::Ref(Ty::Str, _)` operands in args[1..] and
        // validate against the selected target's
        // `max_str_args_per_call`. The target-binding wiring is
        // owned by phase-1b backends; until a binding target flows
        // through the build's target-selection flag, this call
        // passes `None` and the helper short-circuits to `Ok(())`.
        let str_operand_count = count_str_operands(&cx.model, body, &args[1..]);
        super::validation::validate_str_operand_count_against_target_bound(
            str_operand_count,
            target_str_arg_bound(cx),
        )?;
    }
    Ok(())
}

/// Returns the number of `Ty::Ref(Ty::Str, _)` operands among
/// `args` per spec.md section 26.8 D8. Operands whose type is not
/// recoverable are excluded from the count (a conservative undercount
/// is consistent with the spec's "args[1..] excluded args[0]"
/// clause).
fn count_str_operands(model: &RegseqModel, body: &Body, args: &[Operand]) -> usize {
    let mut count = 0;
    for operand in args.iter() {
        let Some(ty) = operand_ty(model, body, operand) else {
            continue;
        };
        if let regseq_model::Ty::Ref(pointee_id, _) = ty {
            if matches!(model.types.resolve(pointee_id), regseq_model::Ty::Str) {
                count += 1;
            }
        }
    }
    count
}

/// Resolve the selected binding target's `max_str_args_per_call`
/// bound per spec.md section 26.8 D8. Returns `None` until the
/// target-binding context flows in via the build's target-selection
/// flag (the per-target bound is declared in the corresponding
/// `<target>-backend.md` artifact per spec.md section 26.9).
fn target_str_arg_bound(_cx: &BridgeCtxt<'_>) -> Option<usize> {
    None
}

/// Best-effort recovery of the format-template UTF-8 literal from
/// args[0]'s ConstId. Walks the post-Regseq-const-lowering shape
/// `Const::Aggregate([Const::Reloc(InstanceRef), Const::Uint(len)])`
/// per spec.md section 9.7 and resolves the `InstanceRef`'s
/// `def_path_hash` through `cx.const_memory_static_cache` to the
/// originating `Const::Bytes(...)` pointee. Returns `None` if the
/// literal cannot be recovered (cross-crate const reference,
/// non-cached lowering, non-UTF-8 bytes).
fn extract_format_template_literal(
    cx: &BridgeCtxt<'_>,
    template_const_id: regseq_model::ConstId,
) -> Option<String> {
    use regseq_model::Const;
    let outer = cx.model.consts.resolve(template_const_id);
    let Const::Aggregate(children) = outer else {
        return None;
    };
    let reloc_id = *children.first()?;
    let inner = cx.model.consts.resolve(reloc_id);
    let Const::Reloc(instance_ref) = inner else {
        return None;
    };
    let reloc_def_path_hash = instance_ref.def_path_hash;
    for entry in cx.const_memory_static_cache.iter() {
        let kind = regseq_model::SyntheticItemKind::ConstMemoryStatic {
            content_hash: entry.content_hash.clone(),
        };
        if regseq_model::synthetic_def_path_hash(&kind) != reloc_def_path_hash {
            continue;
        }
        let pointee = cx.model.consts.resolve(entry.pointee_const);
        let Const::Bytes(bytes) = pointee else {
            continue;
        };
        let raw_bytes: Vec<u8> = bytes
            .iter()
            .filter_map(|byte| match byte {
                regseq_model::Byte::Value(b) => Some(*b),
                _ => None,
            })
            .collect();
        return String::from_utf8(raw_bytes).ok();
    }
    None
}

/// Model-only entry point that drives the peel logic against a
/// `RegseqModel` reference. Split from the public surface so unit
/// tests can exercise the pass without constructing a rustc-backed
/// `BridgeCtxt`. The public `diag_log_peel_pass` is the wiring point
/// invoked by `bridge/body/materialise.rs::run_phase5_body_passes`.
///
/// Returns the `InstanceId`s of every collapsed `<LogArg as
/// From<T>>::from(...)` wrapper Call. The `InstanceId` is captured
/// BEFORE `collapse_wrapper_calls` rewrites the terminator, so the
/// caller (`peel_body_against_model_with_cx`) can push it onto the
/// bridge-side `DiagLogPeelResidue` accumulator. The accumulated
/// seed set drives the post-DiagLogPeel residue-driven DCE per
/// spec.md section 26.6 + regseq-driver-48 Change A/B.
pub(crate) fn peel_body_against_model(
    model: &mut RegseqModel,
    body: &mut Body,
) -> Result<Vec<InstanceId>, BridgeError> {
    // Collect the indices of Call blocks whose callee is a Log
    // intrinsic. A two-pass shape (collect then mutate) avoids
    // overlapping borrows of `body.basic_blocks`.
    let log_call_blocks: Vec<BlockId> = (0..body.basic_blocks.len())
        .map(|idx| BlockId::from_u32(idx as u32))
        .filter(|block_id| call_is_log_intrinsic(model, body, *block_id))
        .collect();

    // The carrier ADT references must not survive into RegseqModel
    // per spec.md section 25.7.3 step 3 + section 26.6
    // `I-DiagLog-CallArity` ("the `LogArg<'_>` type and the
    // `&[LogArg<'_>]` slice type do not appear anywhere in the
    // resulting Body"). LocalDecls are never popped (LocalId
    // indexing is stable per spec.md section 25.8); the cleanup
    // instead overwrites each carrier-local's `LocalDecl.ty` with
    // unit so no surviving Place can resolve to a `LogArg`-typed
    // local. Intern the unit Ty once per peel pass.
    let unit_ty = model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });

    // Accumulator for every collapsed `<LogArg as From<T>>::from(...)`
    // wrapper Call's `InstanceId`. Populated BEFORE
    // `collapse_wrapper_calls` rewrites each wrapper Call terminator
    // to a `Goto`, so the seeds survive the collapse and can drive
    // the residue-driven DCE in `dead_item_elim` per spec.md section
    // 26.6 + regseq-driver-48 Change A.
    let mut collapsed_wrapper_instances: Vec<InstanceId> = Vec::new();

    for block_id in log_call_blocks {
        // Canonical pre-peel shape assertion per spec.md section
        // 25.7.3 step 1: a Log-intrinsic Call has exactly two args
        // (format-template Constant + slice-aggregate LogArg
        // carrier). Anything else is a bridge-internal invariant
        // violation per the spec's "bug!-style diagnostic" rule;
        // surface it as Phase5 error.
        let (fmt_operand, slice_operand) = match &body.basic_blocks[block_id].terminator.kind {
            TerminatorKind::Call { args, .. } if args.len() == 2 => {
                (args[0].clone(), args[1].clone())
            }
            TerminatorKind::Call { args, .. } => {
                return Err(BridgeError::Phase5(format!(
                    "DiagLogPeel: Log-intrinsic Call at block {} has args.len() = {} (expected 2 pre-peel per spec.md section 25.7.3 step 1 canonical shape)",
                    block_id.as_u32(),
                    args.len(),
                )));
            }
            _ => continue,
        };

        // Canonical-shape assertion per spec.md section 25.7.3 step
        // 1: locate the slice-aggregate LogArg carrier behind
        // `args[1]`. The carrier path is `slice_operand -> (optional
        // Ref / RawPtr / Cast(Unsize) indirection chain) ->
        // Rvalue::Aggregate(Array, inner_operands)` per the macro
        // expansion shape at spec.md section 26.5. For empty
        // carriers (`&[]`) rustc may promote the empty array into a
        // `&'static` const; the trace terminates at an
        // `Operand::Constant` and the peel produces 0 inner
        // operands per the spec.md section 26.7 empty-template
        // legality clause.
        let carrier = match locate_log_arg_carrier(body, &slice_operand) {
            CarrierTrace::Aggregate(carrier) => carrier,
            CarrierTrace::EmptyConstSlice(carrier_locals) => LogArgCarrier {
                inner_operands: Vec::new(),
                carrier_locals,
            },
            CarrierTrace::Untraceable => {
                return Err(BridgeError::Phase5(format!(
                    "DiagLogPeel: Log-intrinsic Call at block {} has a non-canonical args[1] carrier shape per spec.md section 25.7.3 step 1 (operand {:?})",
                    block_id.as_u32(),
                    slice_operand,
                )));
            }
        };

        // Per spec.md section 25.7.3 step 2: each inner element of
        // the aggregate is a `Call` to `<LogArg<'_> as From<T_i>>::
        // from(<expr_i>)`. The peel recovers `<expr_i>` from each
        // such Call and queues the wrapper block IDs so the loop
        // below can replace each wrapper Call terminator with a
        // straight Goto to the From's continuation block (the
        // wrappers and `LogArg` temporaries must not survive into
        // RegseqModel per spec.md sections 25.7.3 step 3 + 26.6).
        // Falls back to the inner operand verbatim when no
        // preceding LogArg::from Call is found (the simplified
        // hand-constructed body shape used by the unit tests).
        let mut wrapper_blocks_to_collapse: Vec<BlockId> = Vec::new();
        // The destination local of each collapsed wrapper Call holds
        // a `LogArg<'_>` value at its `LocalDecl.ty`. It is not part
        // of the slice-aggregate trace (which only touches the array
        // / reference / cast chain), so it would not otherwise land
        // in `carrier_locals`. Track it here so the ty-cleanup pass
        // below nulls its declared type per spec.md section 25.7.3
        // step 3 ("intermediate temporaries holding `LogArg<'_>`
        // values, the array itself, the slice reference ... are not
        // translated").
        let mut wrapper_destination_locals: HashSet<LocalId> = HashSet::new();
        let recovered_operands: Vec<Operand> = carrier
            .inner_operands
            .iter()
            .map(|inner| {
                let (peeled, wrapper_match) = peel_log_arg_from_wrapper(model, body, inner);
                if let Some((block, instance_id)) = wrapper_match {
                    wrapper_blocks_to_collapse.push(block);
                    // Capture the wrapper Call's callee InstanceId
                    // BEFORE `collapse_wrapper_calls` rewrites the
                    // terminator (the Goto terminator carries no
                    // callee reference per spec.md section 23.6).
                    collapsed_wrapper_instances.push(instance_id);
                    if let Operand::Move(p) | Operand::Copy(p) = inner {
                        if p.projection.is_empty() {
                            wrapper_destination_locals.insert(p.local);
                        }
                    }
                }
                peeled
            })
            .collect();

        // Reference-companion load synthesis per spec.md section
        // 25.7.3 step 2. For each recovered operand whose source
        // type is `Ty::Ref(value_typed_pointee, _)` (and the
        // pointee is not `Ty::Str` -- the `From<&'a str>` impl is
        // by-value, not a reference-companion), synthesise a load
        // statement that copies through the deref into a fresh
        // value-typed synthetic local. The flat call carries the
        // loaded value-typed Operand. The `From<&'a str>` carrier
        // path keeps the reference-typed Operand verbatim.
        let peeled_operands: Vec<Operand> =
            synthesise_reference_companion_loads(model, body, block_id, recovered_operands);

        // Synthesise the flat-arg Call shape per spec.md section
        // 25.7.3 step 4.
        let mut new_args: Vec<Operand> = Vec::with_capacity(1 + peeled_operands.len());
        new_args.push(fmt_operand);
        new_args.extend(peeled_operands.into_iter());

        // D4 -- operand admissibility per spec.md section 26.8 D4 /
        // section 26.4 `From`-closed set. Walk every peeled operand
        // whose type is recoverable from `body.local_decls` and
        // surface a `BridgeError::DiagLogValidation` when the
        // operand resolves to a non-admissible value-typed `Ty`.
        // Constant operands and operands whose type the body local
        // table does not pin (e.g., a fresh local introduced by an
        // earlier pass without a local-decl entry) are best-effort
        // skipped; integration coverage for those depends on the
        // model-side operand-ty recovery surface in flight at
        // regseq-model-84.
        for arg in new_args.iter().skip(1) {
            let Some(resolved) = operand_ty(model, body, arg) else {
                continue;
            };
            validate_operand_type_admissibility(model, &resolved)?;
        }

        // Drop the carrier statements per spec.md section 25.7.3
        // step 3.
        drop_log_arg_carrier_statements(body, &carrier.carrier_locals);

        // Collapse each `LogArg::from(<expr_i>)` wrapper Call: the
        // wrapper's destination local feeds the dropped carrier
        // aggregate and has no other reader after peel. Replace the
        // wrapper's Call terminator with a Goto to the wrapper's
        // continuation block so the wrappers do not survive in
        // RegseqModel per spec.md section 26.6 "LogArg type does
        // NOT appear in RegseqModel after the bridge runs".
        collapse_wrapper_calls(body, &wrapper_blocks_to_collapse);

        // Overwrite each carrier-local's `LocalDecl.ty` with unit so
        // the carrier ADT references do not survive into the
        // post-peel Body per spec.md section 25.7.3 step 3 + section
        // 26.6 `I-DiagLog-CallArity`. Covers both the slice-aggregate
        // path (array / reference / reborrow / unsizing-cast locals
        // in `carrier_locals`) and the `LogArg::from` wrapper Call
        // destinations (`wrapper_destination_locals`).
        null_carrier_local_decl_types(
            body,
            unit_ty,
            &carrier.carrier_locals,
            &wrapper_destination_locals,
        );

        // Replace the Call's argument vector with the flat-arg form.
        if let TerminatorKind::Call { args, .. } = &mut body.basic_blocks[block_id].terminator.kind
        {
            *args = new_args.into_boxed_slice();
        }
    }

    Ok(collapsed_wrapper_instances)
}

/// Replace each wrapper block's `LogArg::from` Call terminator with
/// a `Goto` to the Call's continuation block. The wrapper Call's
/// destination local feeds the dropped carrier aggregate; with the
/// carrier gone, the wrapper's destination has no readers and the
/// Call may be collapsed to a Goto. Divergent wrappers (whose
/// `target` is `None`) are left alone: they would never reach this
/// pass because `LogArg::from` impls are infallible and always
/// return.
fn collapse_wrapper_calls(body: &mut Body, wrapper_blocks: &[BlockId]) {
    for &block_id in wrapper_blocks {
        let TerminatorKind::Call { target, .. } = &body.basic_blocks[block_id].terminator.kind
        else {
            continue;
        };
        let Some(target_block) = *target else {
            continue;
        };
        let span = body.basic_blocks[block_id].terminator.span;
        body.basic_blocks[block_id].terminator = regseq_model::Terminator {
            kind: TerminatorKind::Goto(target_block),
            span,
        };
    }
}

/// Recover the source `<expr_i>` operand from a `LogArg::from(<expr_i>)`
/// wrapper Call per spec.md section 25.7.3 step 2. The `inner`
/// operand is the aggregate element produced by the macro: in real
/// rustc-emitted MIR, `inner = Operand::Move(_logarg_i)` where the
/// preceding block's terminator is a `Call` whose destination is
/// `Place::from(_logarg_i)` and whose args[0] is `<expr_i>`.
///
/// Returns `(peeled_operand, wrapper_match)`. The optional
/// `wrapper_match` carries the `(BlockId, InstanceId)` pair for the
/// wrapper Call whose terminator must be collapsed to a Goto so the
/// wrapper does not survive into RegseqModel per spec.md section
/// 26.6. The `InstanceId` is the wrapper Call's
/// `CallCallee::Direct(instance_id)` callee, captured here so the
/// caller can push it onto the bridge-side `DiagLogPeelResidue`
/// accumulator BEFORE `collapse_wrapper_calls` rewrites the
/// terminator (the post-collapse `Goto` carries no callee). If no
/// preceding `LogArg::from` Call is located (unit-test bodies that
/// hand-construct the aggregate's inner operands directly), the
/// inner operand is returned verbatim and `wrapper_match` is
/// `None`.
///
/// Reference-companion `From<&T>` / `From<&mut T>` source operands
/// require a load synthesis step per spec.md section 25.7.3 step 2
/// "Load synthesis"; that refinement depends on the model-side
/// type-recovery surface (`operand_ty` helper coordinated with
/// regseq-model-84) and is left for the integration-side coverage
/// when the full LogArg `From` impl matrix lands. By-value cases
/// (integer and `&str` From impls) are exercised verbatim by this
/// helper.
fn peel_log_arg_from_wrapper(
    model: &RegseqModel,
    body: &Body,
    inner: &Operand,
) -> (Operand, Option<(BlockId, InstanceId)>) {
    let target_local = match inner {
        Operand::Move(p) | Operand::Copy(p) if p.projection.is_empty() => p.local,
        _ => return (inner.clone(), None),
    };
    for (idx, block) in body.basic_blocks.iter().enumerate() {
        let TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if !(destination.local == target_local && destination.projection.is_empty()) {
            continue;
        }
        let CallCallee::Direct(instance_id) = callee else {
            continue;
        };
        if !super::is_from_logarg_instance(model, *instance_id) {
            continue;
        }
        if let Some(arg) = args.first() {
            return (
                arg.clone(),
                Some((BlockId::from_u32(idx as u32), *instance_id)),
            );
        }
    }
    (inner.clone(), None)
}

// Structural recognition of `<LogArg<'_> as From<T>>::from` Call
// callees per spec.md section 26.4 + section 26.5 is lifted to
// `super::is_from_logarg_instance` in `diag_log/mod.rs` so the
// post-peel `dead_item_elim` pass and this peel pass share a
// single source of truth (regseq-driver-48 Conventions).

/// Outcome of `locate_log_arg_carrier`: either an Aggregate carrier
/// was located (with inner operands), or an empty const-slice
/// carrier was located (no inner operands), or the carrier could
/// not be located.
enum CarrierTrace {
    /// The trace reached `Rvalue::Aggregate(Array, _)` -- the
    /// canonical shape.
    Aggregate(LogArgCarrier),
    /// The trace reached an `Operand::Constant(_)` (e.g., a
    /// promoted empty array `&[]`). Equivalent to a 0-element
    /// aggregate per spec.md section 26.7 empty-template legality.
    EmptyConstSlice(HashSet<LocalId>),
    /// The trace bottomed out at a shape outside the closed
    /// carrier vocabulary (e.g., a function return value or a
    /// projection chain that does not resolve to an aggregate /
    /// const slice).
    Untraceable,
}

/// Returns `true` if the block's terminator is a Call to an Instance
/// whose Item carries `AttrData::Intrinsic(IntrinsicKind::Log(_))`
/// (the regseq-driver-44 Phase-2 recognition output materialised in
/// Phase 4 into `domain_attrs.item_attrs`).
fn call_is_log_intrinsic(model: &RegseqModel, body: &Body, block_id: BlockId) -> bool {
    let TerminatorKind::Call { callee, .. } = &body.basic_blocks[block_id].terminator.kind else {
        return false;
    };
    let CallCallee::Direct(instance_id) = callee else {
        // Indirect callees cannot satisfy the predicate per the
        // `I-DiagLog-CallArity` invariant text at spec.md section
        // 26.6.
        return false;
    };
    if instance_id.index() >= model.instances.len() {
        return false;
    }
    let item = model.instances[*instance_id].item;
    if item.index() >= model.domain_attrs.item_attrs.len() {
        return false;
    }
    model.domain_attrs.item_attrs[item]
        .iter()
        .any(|attr| matches!(attr, AttrData::Intrinsic(IntrinsicKind::Log(_))))
}

/// Result of locating the slice-aggregate LogArg carrier behind a
/// Log-intrinsic Call's `args[1]` operand.
#[derive(Clone, Debug)]
struct LogArgCarrier {
    /// Inner element operands extracted from the
    /// `Rvalue::Aggregate(Array, _)` source. Each is the operand
    /// the LogArg::from inner-Call wrapped per spec.md section 26.4
    /// (in a real rustc-emitted body the inner operand would itself
    /// be the temporary that received the From result; this
    /// body-level pass treats whatever the aggregate carries as the
    /// peel-emitted operand directly per the simplified
    /// hand-constructed shape used by the per-task unit tests).
    inner_operands: Vec<Operand>,
    /// Locals that the macro materialised solely to construct the
    /// carrier (the array local, the slice reference local, and any
    /// chained indirections). These locals' defining Assign
    /// statements are removed per spec.md section 25.7.3 step 3.
    carrier_locals: HashSet<LocalId>,
}

/// Trace `slice_operand` back through the body to find the
/// `Rvalue::Aggregate(Array, _)` that materialised the LogArg
/// carrier. Walks the macro expansion shape:
/// `Operand::Move(_t_slice_ref)` -> `Rvalue::Cast(Unsize, ...)` ->
/// `Rvalue::Ref(_, _t_arr)` -> `Rvalue::Aggregate(Array, ...)`.
/// Empty slice carriers (`&[]`) terminate at a promoted
/// `Operand::Constant`, in which case the trace returns
/// `EmptyConstSlice` so the peel can emit a 0-inner-operand flat
/// shape per spec.md section 26.7 empty-template legality.
fn locate_log_arg_carrier(body: &Body, slice_operand: &Operand) -> CarrierTrace {
    let mut carrier_locals: HashSet<LocalId> = HashSet::new();
    let mut current_place = match slice_operand {
        Operand::Move(p) | Operand::Copy(p) => p.clone(),
        Operand::Constant(_) => {
            // Direct constant slice carrier (e.g., a const-promoted
            // empty slice). Per spec.md section 26.7 empty-template
            // legality this lowers to 0 inner operands.
            return CarrierTrace::EmptyConstSlice(carrier_locals);
        }
    };
    // Cap the walk depth so a malformed body cannot drive the
    // traversal into an infinite loop.
    for _ in 0..16 {
        if !current_place.projection.is_empty() {
            // Carrier locals are bare locals in the macro shape;
            // any non-empty projection means we've left the
            // carrier path.
            return CarrierTrace::Untraceable;
        }
        let local = current_place.local;
        carrier_locals.insert(local);
        let Some(rv) = find_latest_assign(body, local) else {
            return CarrierTrace::Untraceable;
        };
        match rv {
            Rvalue::Aggregate(AggregateKind::Array(_), operands) => {
                return CarrierTrace::Aggregate(LogArgCarrier {
                    inner_operands: operands.to_vec(),
                    carrier_locals,
                });
            }
            Rvalue::Ref(_, inner) | Rvalue::RawPtr(_, inner) => {
                // Collapse the canonical reborrow shape `&*x` into
                // chasing `x` directly. rustc emits this at
                // `mir_opt_level=0` for slice unsizing per
                // `regseq_mir_passes()` (`crates/regseq-driver/src/
                // callbacks.rs`): `_t = &(*_s)` is materialised as
                // `Rvalue::Ref(_, Place { local: _s, projection:
                // [Deref] })`. Treating `[Deref]` as a transparent
                // indirection preserves the loop invariant that
                // `current_place` is always a bare local (enforced
                // at the top of the loop body); any other
                // projection still bottoms out at `Untraceable`.
                current_place = match &*inner.projection {
                    [] => inner.clone(),
                    [PlaceElem::Deref] => Place::from(inner.local),
                    _ => return CarrierTrace::Untraceable,
                };
            }
            // Cast Rvalue: chase the operand. Covers
            // `PointerCoercion(Unsize, _)` (`&[T; N]` -> `&[T]`)
            // and other reference / pointer coercions the bridge
            // may introduce between the array materialisation and
            // the slice site.
            Rvalue::Cast(_, Operand::Move(p), _) | Rvalue::Cast(_, Operand::Copy(p), _) => {
                current_place = p.clone();
            }
            Rvalue::Cast(_, Operand::Constant(_), _) => {
                // The carrier was const-promoted: empty-slice
                // pattern (or a future surface that promotes a
                // non-empty array). Treat as empty-carrier so the
                // peel produces a 0-inner-operand flat shape.
                return CarrierTrace::EmptyConstSlice(carrier_locals);
            }
            Rvalue::Use(Operand::Move(p)) | Rvalue::Use(Operand::Copy(p)) => {
                current_place = p.clone();
            }
            Rvalue::Use(Operand::Constant(_)) => {
                return CarrierTrace::EmptyConstSlice(carrier_locals);
            }
            _ => return CarrierTrace::Untraceable,
        }
    }
    CarrierTrace::Untraceable
}

/// Synthesise reference-companion loads per spec.md section 25.7.3
/// step 2 for each recovered source operand whose `Ty` is
/// `Ty::Ref(value_typed_pointee, _)` where `value_typed_pointee` is
/// an integer-width scalar or `Ty::Bool` (a value-typed
/// reference-companion source). The synthesised load appends a
/// `Statement::Assign(Place::from(_t_val), Rvalue::Use(Copy(deref_source)))`
/// to the wrapping Log-intrinsic Call's block immediately before
/// the flat Call, and returns `Move(Place::from(_t_val))` as the
/// peeled operand. The `From<&'a str>` impl pointee resolves to
/// `Ty::Str` (not a value-typed scalar); reference-typed string
/// operands flow through verbatim per the spec.md section 25.7.3
/// "The `&str` companions ... are NOT reference-companions in the
/// normalisation sense" clause.
fn synthesise_reference_companion_loads(
    model: &RegseqModel,
    body: &mut Body,
    call_block: BlockId,
    operands: Vec<Operand>,
) -> Vec<Operand> {
    use regseq_model::{Place, PlaceElem, Span, Statement, StatementKind, Ty};
    let mut new_statements: Vec<Statement> = body.basic_blocks[call_block].statements.to_vec();
    let mut peeled = Vec::with_capacity(operands.len());
    for operand in operands.into_iter() {
        let normalised = match &operand {
            Operand::Move(p) | Operand::Copy(p) if p.projection.is_empty() => {
                let p_local = p.local;
                let ty_id = body.local_decls[p_local].ty;
                match model.types.resolve(ty_id) {
                    Ty::Ref(pointee_id, _) => {
                        let pointee_resolved = model.types.resolve(*pointee_id).clone();
                        if pointee_is_value_typed_scalar(&pointee_resolved) {
                            // Synthesise the load: fresh value-typed
                            // local + Assign(Use(Copy(deref_source))).
                            let pointee_ty = *pointee_id;
                            let val_local = body.push_synthetic_local(pointee_ty);
                            let deref_source = Place {
                                local: p_local,
                                projection: Box::new([PlaceElem::Deref]),
                            };
                            new_statements.push(Statement {
                                kind: StatementKind::Assign(
                                    Place::from(val_local),
                                    regseq_model::Rvalue::Use(Operand::Copy(deref_source)),
                                ),
                                span: Span::dummy(),
                            });
                            Operand::Move(Place::from(val_local))
                        } else {
                            operand
                        }
                    }
                    _ => operand,
                }
            }
            _ => operand,
        };
        peeled.push(normalised);
    }
    body.basic_blocks[call_block].statements = new_statements.into_boxed_slice();
    peeled
}

/// Returns `true` if `ty` is a value-typed scalar admissible as a
/// `LogArg` source per spec.md section 26.4 -- integer-width
/// (`Ty::Uint(U8..U64)` / `Ty::Int(I8..I64)`) or `Ty::Bool`.
/// `Ty::Str` is intentionally excluded: the `From<&'a str>` impl is
/// by-value (the `Str` carrier variant stores the reference
/// directly) per spec.md section 25.7.3 step 2.
fn pointee_is_value_typed_scalar(ty: &regseq_model::Ty) -> bool {
    use regseq_model::{IntKind, Ty, UintKind};
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
            | Ty::Bool
    )
}

/// Resolve the `TyId` carried by `operand` through the body's
/// `local_decls` table when the operand is a bare-local Move or
/// Copy. Returns `None` for projected places (the type after
/// projection needs the model-side type-recovery surface) and for
/// constants whose `Const` variant does not pin a `TyId` directly.
fn operand_local_ty(body: &Body, operand: &Operand) -> Option<regseq_model::TyId> {
    match operand {
        Operand::Move(p) | Operand::Copy(p) => {
            if !p.projection.is_empty() {
                return None;
            }
            Some(body.local_decls[p.local].ty)
        }
        Operand::Constant(_) => None,
    }
}

/// Resolve the `Ty` carried by a constant operand through the
/// model's const interner. Scalar consts (`Bool`, `Char`, `Int`,
/// `Uint`, `Float`, `NullPtr`) carry their type identity directly
/// in the variant; aggregate / byte / relocation / enum consts
/// require richer context (e.g., the originating
/// `Operand`/`Place` type) and return `None`.
fn operand_constant_ty(model: &RegseqModel, operand: &Operand) -> Option<regseq_model::Ty> {
    use regseq_model::{Const, Ty};
    let Operand::Constant(const_id) = operand else {
        return None;
    };
    match model.consts.resolve(*const_id) {
        Const::Bool(_) => Some(Ty::Bool),
        Const::Char(_) => Some(Ty::Char),
        Const::Int(kind, _) => Some(Ty::Int(*kind)),
        Const::Uint(kind, _) => Some(Ty::Uint(*kind)),
        Const::Float(kind, _) => Some(Ty::Float(*kind)),
        Const::NullPtr(ty_id) => {
            // The pinned pointee type is wrapped in a Ptr; assume
            // mutable provenance since `NullPtr` does not pin
            // mutability and the DSL accepts both for raw pointer
            // formatting.
            Some(Ty::Ptr(*ty_id, regseq_model::Mutability::Mut))
        }
        _ => None,
    }
}

/// Resolve the `Ty` for any operand. Returns `None` only when the
/// operand's type is unrecoverable (projected place, aggregate /
/// byte / reloc const, etc.).
fn operand_ty(model: &RegseqModel, body: &Body, operand: &Operand) -> Option<regseq_model::Ty> {
    if let Some(ty_id) = operand_local_ty(body, operand) {
        return Some(model.types.resolve(ty_id).clone());
    }
    operand_constant_ty(model, operand)
}

/// Find the most recent (by block + statement order) Assign whose
/// target is the bare local `target_local`. Returns the assigned
/// `Rvalue` clone, or `None` if no such Assign exists.
fn find_latest_assign(body: &Body, target_local: LocalId) -> Option<Rvalue> {
    let mut latest: Option<Rvalue> = None;
    for block in body.basic_blocks.iter() {
        for statement in block.statements.iter() {
            if let StatementKind::Assign(place, rv) = &statement.kind {
                if place.local == target_local && place.projection.is_empty() {
                    latest = Some(rv.clone());
                }
            }
        }
    }
    latest
}

/// Remove every Assign statement whose target is one of the
/// carrier_locals per spec.md section 25.7.3 step 3 ("Drop the slice
/// aggregate").
fn drop_log_arg_carrier_statements(body: &mut Body, carrier_locals: &HashSet<LocalId>) {
    for block_idx in 0..body.basic_blocks.len() {
        let block_id = BlockId::from_u32(block_idx as u32);
        let filtered: Vec<Statement> = body.basic_blocks[block_id]
            .statements
            .iter()
            .filter(|statement| match &statement.kind {
                StatementKind::Assign(place, _) => {
                    !(carrier_locals.contains(&place.local) && place.projection.is_empty())
                }
                _ => true,
            })
            .cloned()
            .collect();
        body.basic_blocks[block_id].statements = filtered.into_boxed_slice();
    }
}

/// Overwrite each carrier-local's `LocalDecl.ty` with `unit_ty` so
/// the carrier ADT (`LogArg<'_>`, `[LogArg<'_>; N]`, `&[LogArg<'_>]`,
/// any reborrow / unsizing-cast indirection in the slice-aggregate
/// path) is unreachable from `body.local_decls` after the peel
/// completes per spec.md section 25.7.3 step 3 + section 26.6
/// `I-DiagLog-CallArity` ("the `LogArg<'_>` type and the
/// `&[LogArg<'_>]` slice type do not appear anywhere in the
/// resulting Body").
///
/// LocalDecls are intentionally not popped: `LocalId`s are stable
/// positional indices per spec.md section 25.8 and any pop would
/// invalidate every other Place in the body. The nulling preserves
/// `local_decls.len()` while removing the carrier ADT references --
/// the carrier locals have no surviving readers after
/// `drop_log_arg_carrier_statements` + `collapse_wrapper_calls`, so
/// their declared type carries no semantic content.
fn null_carrier_local_decl_types(
    body: &mut Body,
    unit_ty: TyId,
    carrier_locals: &HashSet<LocalId>,
    wrapper_destination_locals: &HashSet<LocalId>,
) {
    for &local in carrier_locals
        .iter()
        .chain(wrapper_destination_locals.iter())
    {
        body.local_decls[local].ty = unit_ty;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::{
        AggregateKind, AttrData, BasicBlock, Body, FnAbi, FnSig, FunctionInstanceData,
        FunctionInstanceFlavor, IndexVec, InlineHint, Instance, InstanceKind, IntrinsicKind, Item,
        ItemId, ItemKind, Linkage, LocalDecl, LocalId, LogLevel, Mutability, Operand, Place,
        Provenance, RegseqModel, Rvalue, Safety, Span, StableCrateId, Statement, StatementKind,
        SyntheticItemKind, Terminator, TerminatorKind, Ty, TyId, UNIT_LAYOUT,
    };

    fn intern_unit(model: &mut RegseqModel) -> TyId {
        model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        })
    }

    fn intern_logarg_array(model: &mut RegseqModel) -> TyId {
        // The exact element type does not matter for the pass; pick
        // a unit-shaped tuple as a stand-in.
        let unit = intern_unit(model);
        model.types.intern(Ty::Array(unit, 0))
    }

    fn make_log_function_item(model: &mut RegseqModel, name: &str) -> ItemId {
        use regseq_model::{DefPath, DefPathData, DefPathHash, DefPathSegment};
        let name_id = model.strings.intern(name);
        let rendered = model.strings.intern(&format!("test::{name}"));
        let item = model.items.push(Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::ValueNs(name_id),
                    disambiguator: 0,
                }],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
            is_result: false,
        });
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: item.as_u32() as u64 + 100,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        item
    }

    fn make_function_instance(
        model: &mut RegseqModel,
        item: ItemId,
        input_count: usize,
    ) -> regseq_model::InstanceId {
        let unit = intern_unit(model);
        let inputs = vec![unit; input_count];
        let fn_sig = model.sigs.intern(FnSig {
            inputs,
            output: unit,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        model.instances.push(Instance {
            item,
            generic_args: Vec::new(),
            emission_name: model.items[item].def_path.rendered,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }

    fn local_decl(ty: TyId) -> LocalDecl {
        LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }
    }

    /// Build a body with the canonical pre-peel macro shape:
    ///
    /// ```text
    /// block 0:
    ///   _t_arr        = [<inner_operands>]              // Aggregate(Array)
    ///   _t_slice_ref  = &_t_arr                          // Ref
    ///   Call { callee, args: [fmt, Move(_t_slice_ref)] }
    /// ```
    fn build_pre_peel_body(
        model: &mut RegseqModel,
        instance_id: regseq_model::InstanceId,
        inner_operands: Vec<Operand>,
    ) -> Body {
        let unit = intern_unit(model);
        let array_ty = intern_logarg_array(model);
        let ref_ty = model
            .types
            .intern(Ty::Ref(array_ty, Provenance::SharedBorrow));

        let mut local_decls = IndexVec::new();
        let return_local = local_decls.push(local_decl(unit)); // _0
        debug_assert_eq!(return_local, LocalId::RETURN_PLACE);
        let arr_local = local_decls.push(local_decl(array_ty)); // _1
        let slice_ref_local = local_decls.push(local_decl(ref_ty)); // _2

        // Format-template Const::ZST stand-in (the peel pass does
        // not inspect the literal's content; the public signature
        // just preserves args[0] verbatim).
        let fmt_const = model.consts.intern(regseq_model::Const::ZST);
        let fmt_operand = Operand::Constant(fmt_const);
        let slice_operand = Operand::Move(Place::from(slice_ref_local));

        let aggregate_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(arr_local),
                Rvalue::Aggregate(
                    AggregateKind::Array(array_ty),
                    inner_operands.into_boxed_slice(),
                ),
            ),
            span: Span::dummy(),
        };
        let ref_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(slice_ref_local),
                Rvalue::Ref(Provenance::SharedBorrow, Place::from(arr_local)),
            ),
            span: Span::dummy(),
        };

        let block = BasicBlock {
            statements: vec![aggregate_statement, ref_statement].into_boxed_slice(),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(instance_id),
                    args: vec![fmt_operand, slice_operand].into_boxed_slice(),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: None,
                },
                span: Span::dummy(),
            },
        };

        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(block);
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    #[test]
    fn peels_log_intrinsic_carrier_into_flat_arg_call() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let log_item = make_log_function_item(&mut model, "log_info");
        // Decorate the callee Item with the Log intrinsic attribute
        // so the pass recognises it. Per the regseq-driver-44
        // Phase-4 materialisation step.
        model.domain_attrs.item_attrs[log_item]
            .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
        let instance_id = make_function_instance(&mut model, log_item, 2);

        // Two inner operands stand in for the
        // `LogArg::from(speed)` / `LogArg::from(retries)` carrier
        // elements; the peel treats them as the post-From operands
        // directly per the hand-constructed body shape.
        let inner_op_1 = Operand::Constant(model.consts.intern(regseq_model::Const::Bool(true)));
        let inner_op_2 = Operand::Constant(model.consts.intern(regseq_model::Const::Bool(false)));
        let mut body = build_pre_peel_body(
            &mut model,
            instance_id,
            vec![inner_op_1.clone(), inner_op_2.clone()],
        );

        // Capture the pre-peel local count so we can assert the
        // carrier statements were dropped (`local_decls` length is
        // preserved -- LocalIds are stable positional indices per
        // spec.md section 25.8 and any pop would invalidate every
        // other Place in the body; the cleanup instead nulls each
        // carrier-local's `ty` to unit so no LogArg-typed local
        // survives).
        let pre_peel_local_count = body.local_decls.len();
        // Snapshot the array + slice-ref locals' pre-peel tys so we
        // can prove the cleanup overwrote them.
        let arr_local = LocalId::from_u32(1); // matches build_pre_peel_body
        let slice_ref_local = LocalId::from_u32(2);
        let pre_peel_arr_ty = body.local_decls[arr_local].ty;
        let pre_peel_slice_ref_ty = body.local_decls[slice_ref_local].ty;

        peel_body_against_model(&mut model, &mut body).expect("peel succeeds");

        // After peel: Call.args.len() == 1 + N (template + 2 args).
        let TerminatorKind::Call { args, .. } =
            &body.basic_blocks[BlockId::from_u32(0)].terminator.kind
        else {
            panic!("expected Call terminator post-peel");
        };
        assert_eq!(args.len(), 3, "post-peel arg count = 1 + N");
        // args[0] is the format-template Operand, unchanged.
        assert!(matches!(args[0], Operand::Constant(_)));
        // args[1..] are the peeled inner operands in source order.
        assert_eq!(args[1], inner_op_1);
        assert_eq!(args[2], inner_op_2);

        // The aggregate + ref carrier statements were removed.
        assert!(
            body.basic_blocks[BlockId::from_u32(0)]
                .statements
                .is_empty(),
            "expected carrier statements to be dropped"
        );

        // Locals are still present (LocalDecls are never popped by
        // the peel pass per spec.md section 25.8 -- LocalIds are
        // stable positional indices and popping any would invalidate
        // every other Place in the body).
        assert_eq!(body.local_decls.len(), pre_peel_local_count);

        // The carrier locals' declared types were overwritten with
        // unit so the carrier ADT references (`[LogArg; N]` /
        // `&[LogArg; N]`) do not survive in `body.local_decls` per
        // spec.md section 25.7.3 step 3 + section 26.6
        // `I-DiagLog-CallArity`. The unit Ty interned by the pass
        // matches the empty-tuple form per `UNIT_LAYOUT`.
        let unit = intern_unit(&mut model);
        assert_ne!(
            body.local_decls[arr_local].ty, pre_peel_arr_ty,
            "carrier array local ty must be overwritten"
        );
        assert_ne!(
            body.local_decls[slice_ref_local].ty, pre_peel_slice_ref_ty,
            "carrier slice-ref local ty must be overwritten"
        );
        assert_eq!(body.local_decls[arr_local].ty, unit);
        assert_eq!(body.local_decls[slice_ref_local].ty, unit);
    }

    /// Mirror the canonical macro-expansion shape rustc emits at
    /// `mir_opt_level=0` per `regseq_mir_passes()`
    /// (`crates/regseq-driver/src/callbacks.rs`): the slice unsizing
    /// goes through a reborrow `_reborrow = &(*_slice_ref)` before
    /// the `Unsize` cast. The walker must traverse the
    /// `Place { local, projection: [Deref] }` shape transparently so
    /// the trace reaches the underlying `Aggregate(Array, _)`.
    ///
    /// Body laid out as:
    ///
    /// ```text
    /// block 0:
    ///   _arr      = [<inner_operands>]                       // Aggregate(Array)
    ///   _slice    = &_arr                                    // Ref(_, _arr)
    ///   _reborrow = &(*_slice)                               // Ref(_, *_slice)  -- the reborrow
    ///   _unsized  = move _reborrow as &[T]   (Unsize)        // Cast
    ///   Call { callee, args: [fmt, Move(_unsized)] }
    /// ```
    fn build_pre_peel_body_with_reborrow(
        model: &mut RegseqModel,
        instance_id: regseq_model::InstanceId,
        inner_operands: Vec<Operand>,
    ) -> Body {
        use regseq_model::{CastKind, PointerCoercion};
        let unit = intern_unit(model);
        let array_ty = intern_logarg_array(model);
        let ref_ty = model
            .types
            .intern(Ty::Ref(array_ty, Provenance::SharedBorrow));
        // The unsizing cast lowers `&[T; N]` to `&[T]`. The slice
        // element type is not material to the peel pass; reuse the
        // array's element-type stand-in via the same `array_ty`'s
        // referent for consistency.
        let slice_ref_ty = model
            .types
            .intern(Ty::Ref(array_ty, Provenance::SharedBorrow));

        let mut local_decls = IndexVec::new();
        let return_local = local_decls.push(local_decl(unit)); // _0
        debug_assert_eq!(return_local, LocalId::RETURN_PLACE);
        let arr_local = local_decls.push(local_decl(array_ty)); // _1
        let slice_local = local_decls.push(local_decl(ref_ty)); // _2
        let reborrow_local = local_decls.push(local_decl(ref_ty)); // _3
        let unsized_local = local_decls.push(local_decl(slice_ref_ty)); // _4

        let fmt_const = model.consts.intern(regseq_model::Const::ZST);
        let fmt_operand = Operand::Constant(fmt_const);
        let slice_operand = Operand::Move(Place::from(unsized_local));

        let aggregate_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(arr_local),
                Rvalue::Aggregate(
                    AggregateKind::Array(array_ty),
                    inner_operands.into_boxed_slice(),
                ),
            ),
            span: Span::dummy(),
        };
        let slice_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(slice_local),
                Rvalue::Ref(Provenance::SharedBorrow, Place::from(arr_local)),
            ),
            span: Span::dummy(),
        };
        // The load-bearing piece: the reborrow's inner Place has a
        // single `Deref` projection. The pre-fix walker rejected
        // this shape at the bare-local invariant; the post-fix
        // walker collapses `&*x` to chasing `x`.
        let reborrow_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(reborrow_local),
                Rvalue::Ref(
                    Provenance::SharedBorrow,
                    Place {
                        local: slice_local,
                        projection: Box::new([PlaceElem::Deref]),
                    },
                ),
            ),
            span: Span::dummy(),
        };
        let unsize_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(unsized_local),
                Rvalue::Cast(
                    CastKind::PointerCoercion(PointerCoercion::Unsize),
                    Operand::Move(Place::from(reborrow_local)),
                    slice_ref_ty,
                ),
            ),
            span: Span::dummy(),
        };

        let block = BasicBlock {
            statements: vec![
                aggregate_statement,
                slice_statement,
                reborrow_statement,
                unsize_statement,
            ]
            .into_boxed_slice(),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(instance_id),
                    args: vec![fmt_operand, slice_operand].into_boxed_slice(),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: None,
                },
                span: Span::dummy(),
            },
        };

        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(block);
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    /// Regression test for the canonical rustc-emitted reborrow
    /// shape (`_reborrow = &(*_slice)`) at `mir_opt_level=0`. Without
    /// the `Rvalue::Ref(_, Place { projection: [Deref] })` reborrow
    /// collapse, the carrier walker bottoms out at `Untraceable` and
    /// the peel pass surfaces a `BridgeError::Phase5(...)` error.
    /// With the collapse, the trace reaches the underlying
    /// `Aggregate(Array, _)` and the flat-arg Call is synthesised
    /// correctly. Also locks in the LocalDecl-ty nulling for every
    /// link in the carrier chain (array, slice-ref, reborrow,
    /// unsized-cast destination).
    #[test]
    fn peels_carrier_through_reborrow_and_unsize() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let log_item = make_log_function_item(&mut model, "log_info");
        model.domain_attrs.item_attrs[log_item]
            .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
        let instance_id = make_function_instance(&mut model, log_item, 2);

        let inner_op = Operand::Constant(model.consts.intern(regseq_model::Const::Bool(true)));
        let mut body =
            build_pre_peel_body_with_reborrow(&mut model, instance_id, vec![inner_op.clone()]);

        let pre_peel_local_count = body.local_decls.len();
        // _1=arr, _2=slice, _3=reborrow, _4=unsized. Snapshot every
        // carrier local's pre-peel ty so we can prove the cleanup
        // walked the whole chain (not just the leaf the trace
        // started from).
        let arr_local = LocalId::from_u32(1);
        let slice_local = LocalId::from_u32(2);
        let reborrow_local = LocalId::from_u32(3);
        let unsized_local = LocalId::from_u32(4);
        let pre_peel_tys = [
            body.local_decls[arr_local].ty,
            body.local_decls[slice_local].ty,
            body.local_decls[reborrow_local].ty,
            body.local_decls[unsized_local].ty,
        ];

        peel_body_against_model(&mut model, &mut body)
            .expect("peel succeeds through the reborrow shape");

        let TerminatorKind::Call { args, .. } =
            &body.basic_blocks[BlockId::from_u32(0)].terminator.kind
        else {
            panic!("expected Call terminator post-peel");
        };
        // Flat-arg shape: fmt + the one recovered inner operand.
        assert_eq!(args.len(), 2);
        assert!(matches!(args[0], Operand::Constant(_)));
        assert_eq!(args[1], inner_op);

        // Every carrier statement (aggregate, slice ref, reborrow,
        // unsize cast) was dropped.
        assert!(
            body.basic_blocks[BlockId::from_u32(0)]
                .statements
                .is_empty(),
            "expected all four carrier statements to be dropped"
        );

        // LocalIds are stable; the cleanup nulls types in place.
        assert_eq!(body.local_decls.len(), pre_peel_local_count);
        let unit = intern_unit(&mut model);
        for (local, pre_ty) in [
            (arr_local, pre_peel_tys[0]),
            (slice_local, pre_peel_tys[1]),
            (reborrow_local, pre_peel_tys[2]),
            (unsized_local, pre_peel_tys[3]),
        ] {
            assert_ne!(
                body.local_decls[local].ty, pre_ty,
                "carrier local {local:?} ty must be overwritten"
            );
            assert_eq!(
                body.local_decls[local].ty, unit,
                "carrier local {local:?} ty must be nulled to unit"
            );
        }
    }

    /// Build a nominal Enum Item whose rendered def_path terminal
    /// segment matches `terminal_segment` (intended to be `"LogArg"`
    /// for the carrier-recognition tests). Mirrors
    /// `make_log_function_item`'s scaffolding shape so the new test
    /// composes with the existing Item / Instance fixtures.
    fn make_named_enum_item(model: &mut RegseqModel, terminal_segment: &str) -> ItemId {
        use regseq_model::{DefPath, DefPathData, DefPathHash, DefPathSegment};
        let name_id = model.strings.intern(terminal_segment);
        let rendered = model.strings.intern(&format!("test::{terminal_segment}"));
        let item = model.items.push(Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::TypeNs(name_id),
                    disambiguator: 0,
                }],
                rendered,
            },
            kind: ItemKind::Enum,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
            is_result: false,
        });
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: item.as_u32() as u64 + 200,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        item
    }

    /// Build an Item whose def_path is the canonical impl-method
    /// shape `core::convert::{impl#0}::from` that the bridge interns
    /// for `<T as From<U>>::from` Call callees after the
    /// regseq-driver-56 static-dispatch canonicalization
    /// (spec.md:2354-2394). The segment list is
    /// `[TypeNs(core), TypeNs(convert), Impl, ValueNs(from)]`. The
    /// Item's `kind` is `Function` because the impl method itself
    /// is a function item.
    fn make_from_impl_method_item(model: &mut RegseqModel) -> ItemId {
        use regseq_model::{DefPath, DefPathData, DefPathHash, DefPathSegment};
        let rendered = model.strings.intern("core::convert::{impl#0}::from");
        let core_seg = model.strings.intern("core");
        let convert_seg = model.strings.intern("convert");
        let from_method = model.strings.intern("from");
        let item = model.items.push(Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::TypeNs(core_seg),
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::TypeNs(convert_seg),
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::Impl,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(from_method),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
            is_result: false,
        });
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: item.as_u32() as u64 + 300,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        item
    }

    /// Regression for the `callee_is_from_logarg` structural
    /// recognition. Constructs the canonical impl-method Instance
    /// shape the bridge produces for
    /// `<LogArg<'_> as From<u32>>::from(x)` post-regseq-driver-56
    /// (Item def_path =
    /// `[TypeNs(core), TypeNs(convert), Impl, ValueNs(from)]`,
    /// rendered `core::convert::{impl#0}::from`,
    /// `FnSig.output = LogArg-enum-ty`) and a two-block body:
    ///
    /// ```text
    /// block 0:
    ///   _t_src = copy _1
    ///   Call { callee = From::from, args: [move _t_src],
    ///          destination = _t_wrapper, target: bb1 }
    /// block 1:
    ///   _t_arr       = [move _t_wrapper]      // Aggregate(Array)
    ///   _t_slice_ref = &_t_arr                // Ref
    ///   Call { callee = log_intrinsic, args: [fmt, Move(_t_slice_ref)],
    ///          destination = _0, target: None }
    /// ```
    ///
    /// After peel:
    /// - block 0's wrapper Call terminator is collapsed to a
    ///   `Goto(block 1)` (the From wrapper does not survive).
    /// - block 1's carrier statements are dropped.
    /// - block 1's log Call args become `[fmt, move _t_src]` --
    ///   the inner u32 operand recovered through the wrapper.
    /// - The wrapper destination local's `LocalDecl.ty` is nulled
    ///   to unit (so the `LogArg` type does not survive in
    ///   `local_decls`).
    ///
    /// The canonical impl-method recognizer
    /// (`is_from_logarg_instance`) keys on the def_path segment
    /// structure (terminal `ValueNs("from")` + `DefPathData::Impl`
    /// ancestor) plus `FnSig.output = LogArg`, so the wrapper Call
    /// is recognised even though the rendered string contains no
    /// `LogArg` substring.
    #[test]
    fn peels_logarg_from_wrapper_via_structural_recognition() {
        use regseq_model::{CallCallee, GenericArg};

        let mut model = RegseqModel::with_primary("test", StableCrateId(1));

        // Item + Instance for `<LogArg<'_> as From<u32>>::from`.
        // Order matters: build LogArg first so the type interner
        // can resolve generic_args[0] to its Item handle.
        let logarg_item = make_named_enum_item(&mut model, "LogArg");
        let logarg_ty = model.types.intern(Ty::Enum {
            item: logarg_item,
            args: Vec::new(),
            layout: regseq_model::Layout {
                size: 0,
                align: 1,
                pack: None,
                uninhabited: false,
                kind: regseq_model::LayoutKind::Struct { fields: Vec::new() },
            },
        });
        let u32_ty = model.types.intern(Ty::Uint(regseq_model::UintKind::U32));
        let from_item = make_from_impl_method_item(&mut model);

        // The canonical impl-method From Instance carries the
        // dispatch info (Self, source-type) in `generic_args`.
        let unit = intern_unit(&mut model);
        let from_sig = model.sigs.intern(FnSig {
            inputs: vec![u32_ty],
            output: logarg_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        let from_instance = model.instances.push(Instance {
            item: from_item,
            generic_args: vec![GenericArg::Type(logarg_ty), GenericArg::Type(u32_ty)],
            emission_name: model.items[from_item].def_path.rendered,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: from_sig,
                body: None,
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        });

        // Item + Instance for `log_<sev>` (the actual diagnostic-
        // logging callee). Decorated with the Log intrinsic attr
        // so `call_is_log_intrinsic` recognises it.
        let log_item = make_log_function_item(&mut model, "log_info");
        model.domain_attrs.item_attrs[log_item]
            .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
        let log_instance = make_function_instance(&mut model, log_item, 2);

        // Body locals: _0 unit return, _1 u32 source arg, _2 u32
        // temp, _3 LogArg wrapper destination, _4 [LogArg; N] array,
        // _5 &[LogArg; N] slice reference.
        let array_ty = model.types.intern(Ty::Array(logarg_ty, 1));
        let slice_ref_ty = model
            .types
            .intern(Ty::Ref(array_ty, Provenance::SharedBorrow));
        let mut local_decls = IndexVec::new();
        let return_local = local_decls.push(local_decl(unit));
        debug_assert_eq!(return_local, LocalId::RETURN_PLACE);
        let src_arg_local = local_decls.push(local_decl(u32_ty)); // _1
        let src_temp_local = local_decls.push(local_decl(u32_ty)); // _2
        let wrapper_dest_local = local_decls.push(local_decl(logarg_ty)); // _3
        let arr_local = local_decls.push(local_decl(array_ty)); // _4
        let slice_ref_local = local_decls.push(local_decl(slice_ref_ty)); // _5

        let fmt_const = model.consts.intern(regseq_model::Const::ZST);

        // block 0 -- wrapper Call.
        let src_temp_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(src_temp_local),
                Rvalue::Use(Operand::Copy(Place::from(src_arg_local))),
            ),
            span: Span::dummy(),
        };
        let wrapper_block = BasicBlock {
            statements: vec![src_temp_statement].into_boxed_slice(),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(from_instance),
                    args: vec![Operand::Move(Place::from(src_temp_local))].into_boxed_slice(),
                    destination: Place::from(wrapper_dest_local),
                    target: Some(BlockId::from_u32(1)),
                },
                span: Span::dummy(),
            },
        };

        // block 1 -- carrier setup + log Call.
        let aggregate_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(arr_local),
                Rvalue::Aggregate(
                    AggregateKind::Array(array_ty),
                    vec![Operand::Move(Place::from(wrapper_dest_local))].into_boxed_slice(),
                ),
            ),
            span: Span::dummy(),
        };
        let ref_statement = Statement {
            kind: StatementKind::Assign(
                Place::from(slice_ref_local),
                Rvalue::Ref(Provenance::SharedBorrow, Place::from(arr_local)),
            ),
            span: Span::dummy(),
        };
        let log_block = BasicBlock {
            statements: vec![aggregate_statement, ref_statement].into_boxed_slice(),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: CallCallee::Direct(log_instance),
                    args: vec![
                        Operand::Constant(fmt_const),
                        Operand::Move(Place::from(slice_ref_local)),
                    ]
                    .into_boxed_slice(),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: None,
                },
                span: Span::dummy(),
            },
        };

        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(wrapper_block);
        basic_blocks.push(log_block);
        let mut body = Body {
            basic_blocks,
            local_decls,
            arg_count: 1,
            span: Span::dummy(),
        };

        let pre_peel_local_count = body.local_decls.len();
        let pre_peel_wrapper_ty = body.local_decls[wrapper_dest_local].ty;

        peel_body_against_model(&mut model, &mut body)
            .expect("peel succeeds through the structural From-recognition path");

        // block 1's log Call now carries the recovered u32 operand,
        // NOT the LogArg-typed wrapper destination.
        let TerminatorKind::Call { args, .. } =
            &body.basic_blocks[BlockId::from_u32(1)].terminator.kind
        else {
            panic!("expected log Call terminator post-peel");
        };
        assert_eq!(args.len(), 2, "post-peel arg count = 1 fmt + 1 inner");
        assert!(matches!(args[0], Operand::Constant(_)));
        assert_eq!(
            args[1],
            Operand::Move(Place::from(src_temp_local)),
            "the recovered operand must be the u32 source local that the From wrapper consumed, \
             NOT the LogArg-typed wrapper destination"
        );

        // block 0's wrapper Call terminator was collapsed to Goto(bb1).
        match &body.basic_blocks[BlockId::from_u32(0)].terminator.kind {
            TerminatorKind::Goto(target) => assert_eq!(*target, BlockId::from_u32(1)),
            other => panic!("expected Goto(bb1) post-peel, got {other:?}"),
        }

        // The carrier statements in block 1 are dropped.
        assert!(
            body.basic_blocks[BlockId::from_u32(1)]
                .statements
                .is_empty(),
            "block 1 carrier statements must be dropped"
        );

        // LocalDecls are stable in length and the wrapper destination's
        // ty was nulled to unit (the LogArg-typed local must not
        // survive in `body.local_decls`).
        assert_eq!(body.local_decls.len(), pre_peel_local_count);
        assert_ne!(
            body.local_decls[wrapper_dest_local].ty, pre_peel_wrapper_ty,
            "wrapper destination ty must be overwritten"
        );
        assert_eq!(
            body.local_decls[wrapper_dest_local].ty, unit,
            "wrapper destination ty must be nulled to unit"
        );
        // The carrier-path locals also nulled to unit.
        assert_eq!(body.local_decls[arr_local].ty, unit);
        assert_eq!(body.local_decls[slice_ref_local].ty, unit);
    }

    #[test]
    fn non_log_call_body_is_unchanged() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let plain_item = make_log_function_item(&mut model, "regular_function");
        // No AttrData::Intrinsic attribute on the Item: the pass
        // must leave this Call alone.
        let instance_id = make_function_instance(&mut model, plain_item, 1);

        let inner_op = Operand::Constant(model.consts.intern(regseq_model::Const::Bool(true)));
        let mut body = build_pre_peel_body(&mut model, instance_id, vec![inner_op]);
        let pre_peel_args_len = match &body.basic_blocks[BlockId::from_u32(0)].terminator.kind {
            TerminatorKind::Call { args, .. } => args.len(),
            _ => unreachable!(),
        };
        let pre_peel_statement_count = body.basic_blocks[BlockId::from_u32(0)].statements.len();

        peel_body_against_model(&mut model, &mut body).expect("peel runs as a no-op");

        let post_peel_args_len = match &body.basic_blocks[BlockId::from_u32(0)].terminator.kind {
            TerminatorKind::Call { args, .. } => args.len(),
            _ => unreachable!(),
        };
        let post_peel_statement_count = body.basic_blocks[BlockId::from_u32(0)].statements.len();

        assert_eq!(post_peel_args_len, pre_peel_args_len);
        assert_eq!(post_peel_statement_count, pre_peel_statement_count);
    }

    #[test]
    fn peels_zero_arg_log_intrinsic() {
        // `regseq::log::info!("connected")` with no arguments: the
        // macro expansion still materialises an empty `[LogArg; 0]`
        // carrier. The peel pass replaces the call with a single
        // format-template argument.
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let log_item = make_log_function_item(&mut model, "log_info");
        model.domain_attrs.item_attrs[log_item]
            .push(AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info)));
        let instance_id = make_function_instance(&mut model, log_item, 2);

        let mut body = build_pre_peel_body(&mut model, instance_id, Vec::new());

        peel_body_against_model(&mut model, &mut body).expect("peel succeeds on empty carrier");

        let TerminatorKind::Call { args, .. } =
            &body.basic_blocks[BlockId::from_u32(0)].terminator.kind
        else {
            panic!("expected Call terminator post-peel");
        };
        assert_eq!(args.len(), 1, "post-peel arg count = 1 + 0 (template only)");
    }
}
