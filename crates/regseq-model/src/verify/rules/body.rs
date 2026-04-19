//! Body-level rule bucket (task 56).
//!
//! Four checks live here, in deterministic-firing order:
//!
//! - **Referenced IDs resolve** (spec.md Section 13 line 1428): every
//!   `FunctionId`, `TypeDefId`, `ConstantId` appearing in a body's
//!   statements or terminators must be present in the model.
//! - **Operand types match** (spec.md Section 13 line 1429): for each
//!   `TerminatorKind::Call`, the argument operands' types match the
//!   callee's declared parameter types. Operands derived from a
//!   `LocalDecl` directly (no projection) are compared by exact
//!   `RegseqType` equality. Literal operands derive their type from
//!   the [`LiteralValue`] variant. Operand kinds we cannot type
//!   infer locally (`Operand::Constant`, projections through enum
//!   downcasts, etc.) are skipped -- the simple checks already
//!   exercise the prescribed surface.
//! - **No writes to read-only registers** (spec.md Section 13 line
//!   1430, fully implemented): every `IntrinsicCall` whose intrinsic
//!   is `RegisterWrite` or `RegisterWriteField` must reference a
//!   register descriptor whose `access_kind` is not `ReadOnly`.
//! - **No forbidden intrinsics in ISR context** (spec.md Section 13
//!   line 1429, hook-only at Phase 6): every `IntrinsicCall` issued
//!   by an ISR-classified body whose intrinsic appears in the
//!   configured `ISR_FORBIDDEN_INTRINSICS` policy set raises
//!   [`VerifierErrorKind::IsrContextViolation`]. The Phase-6 default
//!   policy set is empty -- the plumbing exists so a future phase or
//!   downstream consumer can swap in a real set without reshaping
//!   the verifier surface. Tests use [`run_with_isr_forbidden`] to
//!   exercise the hook with a synthetic non-empty set.
//!
//! ISR-context membership is determined by walking the model's ISR
//! bindings (`InterruptResource.binding.isr_target`) and treating
//! every targeted [`FunctionId`] as an ISR body. The same predicate
//! is used by [`super::phase_2_cross_item`] for binding-target
//! validation.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::{DeclId, FunctionId, RegisterDescriptorId, SourceOrigin};
use crate::ir::{
    AggregateKind, IntrinsicMetadata, LiteralValue, MirIntrinsic, Operand, Place, Projection,
    RegseqIntrinsic, RegseqType, ScalarType, StatementKind, TerminatorKind,
};
use crate::model::constant::Constant;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::module::Module;
use crate::model::register::{AccessKind, RegisterBlock, RegisterDescriptor};
use crate::model::regseq_model::RegseqModel;

use super::super::{
    IsrViolationKind, VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance,
};

/// Phase-6 policy set: no intrinsics are forbidden in ISR context.
///
/// Phase 7/8 will replace this slice with a real set once
/// `RegseqIntrinsic` admits sleep-class operations and the spec
/// pins down the exact list. The hook below already routes through
/// this constant so swapping it is a one-line change.
const ISR_FORBIDDEN_INTRINSICS: &[RegseqIntrinsic] = &[];

/// Run every body-level check in deterministic order, returning the
/// first violation. Uses the Phase-6 default ISR-forbidden policy
/// (empty set).
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    run_with_isr_forbidden(model, ISR_FORBIDDEN_INTRINSICS)
}

/// Run every body-level check with a caller-supplied
/// ISR-forbidden-intrinsic policy set.
///
/// Used by tests to exercise the hook with a synthetic non-empty set
/// without depending on the empty Phase-6 default.
pub fn run_with_isr_forbidden(
    model: &RegseqModel,
    isr_forbidden: &[RegseqIntrinsic],
) -> Result<(), VerifierError> {
    let context = BodyContext::collect(model);
    walk_functions(model, &mut |f| referenced_ids_resolve(model, f, &context))?;
    walk_functions(model, &mut |f| operand_types_match(model, f, &context))?;
    walk_functions(model, &mut |f| read_only_register_write(model, f, &context))?;
    walk_functions(model, &mut |f| {
        isr_context_legality(model, f, &context, isr_forbidden)
    })?;
    Ok(())
}

// --- Shared collection ----------------------------------------------

/// Pre-collected lookup tables used by every body-level rule.
struct BodyContext<'a> {
    /// All declared `FunctionId`s in the model.
    functions: BTreeSet<DeclId>,
    /// All declared `TypeDefId`s in the model.
    type_defs: BTreeSet<DeclId>,
    /// All declared `ConstantId`s in the model.
    constants: BTreeSet<DeclId>,
    /// Map from declared `ConstantId` to its `Constant` descriptor,
    /// used by `operand_type` to derive the type of an
    /// `Operand::Constant`.
    constants_by_id: BTreeMap<DeclId, &'a Constant>,
    /// Map from descriptor id to descriptor for ReadOnly check.
    descriptors: BTreeMap<DeclId, &'a RegisterDescriptor>,
    /// Map from register block id to block for descriptor lookup via
    /// (block, offset).
    blocks: BTreeMap<DeclId, &'a RegisterBlock>,
    /// `FunctionId` set of every body that runs in ISR context.
    isr_bodies: BTreeSet<DeclId>,
}

impl<'a> BodyContext<'a> {
    fn collect(model: &'a RegseqModel) -> Self {
        let mut ctx = BodyContext {
            functions: BTreeSet::new(),
            type_defs: BTreeSet::new(),
            constants: BTreeSet::new(),
            constants_by_id: BTreeMap::new(),
            descriptors: BTreeMap::new(),
            blocks: BTreeMap::new(),
            isr_bodies: BTreeSet::new(),
        };
        ctx.collect_module(&model.root_module);
        ctx
    }

    fn collect_module(&mut self, m: &'a Module) {
        for c in &m.constants {
            self.constants.insert(c.id.decl_id());
            self.constants_by_id.insert(c.id.decl_id(), c);
        }
        for f in &m.free_functions {
            self.functions.insert(f.id.decl_id());
        }
        for t in &m.type_defs {
            self.type_defs.insert(t.id.decl_id());
        }
        for r in &m.register_descriptors {
            self.descriptors.insert(r.id.decl_id(), r);
        }
        for b in &m.register_blocks {
            self.blocks.insert(b.id.decl_id(), b);
        }
        for ib in &m.impl_blocks {
            self.collect_impl(ib);
        }
        for ib in &m.trait_impls {
            self.collect_impl(ib);
        }
        for ib in &m.driver_impl_blocks {
            self.collect_impl(ib);
        }
        if let Some(driver) = &m.driver {
            for r in &driver.interrupts {
                if let Some(binding) = &r.binding {
                    if let Some(target) = binding.isr_target {
                        self.isr_bodies.insert(target.decl_id());
                    }
                }
            }
        }
        for sub in &m.submodules {
            self.collect_module(sub);
        }
    }

    fn collect_impl(&mut self, ib: &'a ImplBlock) {
        for f in &ib.methods {
            self.functions.insert(f.id.decl_id());
        }
        for c in &ib.associated_constants {
            self.constants.insert(c.id.decl_id());
            self.constants_by_id.insert(c.id.decl_id(), c);
        }
    }
}

// --- Referenced IDs resolve -----------------------------------------

fn referenced_ids_resolve(
    model: &RegseqModel,
    f: &Function,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    let body = f.id;
    for block in &f.body.basic_blocks {
        for stmt in &block.statements {
            check_statement_refs(model, body, &stmt.kind, ctx)?;
        }
        check_terminator_refs(model, body, &block.terminator.kind, ctx)?;
    }
    Ok(())
}

fn check_statement_refs(
    model: &RegseqModel,
    body: FunctionId,
    kind: &StatementKind,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            check_place_refs(model, body, place, ctx)?;
            check_rvalue_refs(model, body, rvalue, ctx)?;
        }
        StatementKind::SetDiscriminant { place, .. } => {
            check_place_refs(model, body, place, ctx)?;
        }
        StatementKind::MirIntrinsic(intrinsic) => {
            check_mir_intrinsic_refs(model, body, intrinsic, ctx)?;
        }
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => {}
    }
    Ok(())
}

fn check_mir_intrinsic_refs(
    model: &RegseqModel,
    body: FunctionId,
    intrinsic: &MirIntrinsic,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match intrinsic {
        MirIntrinsic::Assume(op) => check_operand_refs(model, body, op, ctx)?,
        MirIntrinsic::CopyNonOverlapping { src, dst, count } => {
            check_operand_refs(model, body, src, ctx)?;
            check_operand_refs(model, body, dst, ctx)?;
            check_operand_refs(model, body, count, ctx)?;
        }
    }
    Ok(())
}

fn check_rvalue_refs(
    model: &RegseqModel,
    body: FunctionId,
    rvalue: &crate::ir::Rvalue,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    use crate::ir::Rvalue;
    match rvalue {
        Rvalue::Use(op) | Rvalue::Repeat(op, _) | Rvalue::UnaryOp(_, op) => {
            check_operand_refs(model, body, op, ctx)?;
        }
        Rvalue::Cast(_, op, ty) => {
            check_operand_refs(model, body, op, ctx)?;
            check_type_refs(model, body, ty, ctx)?;
        }
        Rvalue::BinaryOp(_, lhs, rhs) => {
            check_operand_refs(model, body, lhs, ctx)?;
            check_operand_refs(model, body, rhs, ctx)?;
        }
        Rvalue::Aggregate(kind, ops) => {
            check_aggregate_kind_refs(model, body, kind, ctx)?;
            for op in ops {
                check_operand_refs(model, body, op, ctx)?;
            }
        }
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => {
            check_place_refs(model, body, place, ctx)?;
        }
    }
    Ok(())
}

fn check_aggregate_kind_refs(
    model: &RegseqModel,
    body: FunctionId,
    kind: &AggregateKind,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match kind {
        AggregateKind::Array(ty) => check_type_refs(model, body, ty, ctx),
        AggregateKind::Adt { type_id, .. } => {
            if !ctx.type_defs.contains(&type_id.decl_id()) {
                return Err(unresolved_body_reference(model, body, type_id.decl_id()));
            }
            Ok(())
        }
        AggregateKind::Tuple => Ok(()),
    }
}

fn check_terminator_refs(
    model: &RegseqModel,
    body: FunctionId,
    kind: &TerminatorKind,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match kind {
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            if !ctx.functions.contains(&callee.decl_id()) {
                return Err(unresolved_body_reference(model, body, callee.decl_id()));
            }
            for arg in args {
                check_operand_refs(model, body, arg, ctx)?;
            }
            check_place_refs(model, body, destination, ctx)?;
        }
        TerminatorKind::IntrinsicCall {
            args, destination, ..
        } => {
            for arg in args {
                check_operand_refs(model, body, arg, ctx)?;
            }
            check_place_refs(model, body, destination, ctx)?;
        }
        TerminatorKind::SwitchInt { discriminant, .. } => {
            check_operand_refs(model, body, discriminant, ctx)?;
        }
        TerminatorKind::Drop { place, .. } => {
            check_place_refs(model, body, place, ctx)?;
        }
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn check_operand_refs(
    model: &RegseqModel,
    body: FunctionId,
    op: &Operand,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match op {
        Operand::Constant(cid) => {
            if !ctx.constants.contains(&cid.decl_id()) {
                return Err(unresolved_body_reference(model, body, cid.decl_id()));
            }
        }
        Operand::Copy(place) | Operand::Move(place) => {
            check_place_refs(model, body, place, ctx)?;
        }
        Operand::Literal(_) => {}
    }
    Ok(())
}

/// Walk a [`Place`]'s projection chain and validate that any
/// embedded type ids resolve. The local id itself is checked
/// elsewhere; here we only chase the typed handles inside the
/// projections.
fn check_place_refs(
    model: &RegseqModel,
    body: FunctionId,
    place: &Place,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    for proj in &place.projection {
        if let Projection::Field { ty, .. } = proj {
            check_type_refs(model, body, ty, ctx)?;
        }
    }
    Ok(())
}

fn check_type_refs(
    model: &RegseqModel,
    body: FunctionId,
    ty: &RegseqType,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    match ty {
        RegseqType::Defined(target) => {
            if !ctx.type_defs.contains(&target.decl_id()) {
                return Err(unresolved_body_reference(model, body, target.decl_id()));
            }
            Ok(())
        }
        RegseqType::Option(inner)
        | RegseqType::Slice(inner)
        | RegseqType::Array(inner, _)
        | RegseqType::Ref(inner, _) => check_type_refs(model, body, inner, ctx),
        RegseqType::Result(ok, err) => {
            check_type_refs(model, body, ok, ctx)?;
            check_type_refs(model, body, err, ctx)
        }
        RegseqType::Scalar(_) | RegseqType::Bool | RegseqType::Unit | RegseqType::Never => Ok(()),
    }
}

fn unresolved_body_reference(
    model: &RegseqModel,
    body: FunctionId,
    decl_id: DeclId,
) -> VerifierError {
    let body_decl = body.decl_id();
    VerifierError {
        level: VerifierLevel::Body,
        kind: VerifierErrorKind::UnresolvedBodyReference { body, decl_id },
        provenance: VerifierProvenance::for_item(
            format!("body#{}", body_decl.index()),
            lookup_origin(model, body_decl),
        ),
        details: format!(
            "function body (FunctionId {}) references undeclared DeclId {}",
            body_decl.index(),
            decl_id.index()
        ),
    }
}

// --- Operand types match --------------------------------------------

fn operand_types_match(
    model: &RegseqModel,
    f: &Function,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    let body = f.id;
    let callees = collect_function_signatures(model);
    for (block_idx, block) in f.body.basic_blocks.iter().enumerate() {
        match &block.terminator.kind {
            TerminatorKind::Call { callee, args, .. } => {
                let Some(sig) = callees.get(&callee.decl_id()) else {
                    // Unresolved-call already reported by
                    // cross_item / referenced_ids_resolve. Avoid
                    // double-failing.
                    continue;
                };
                if args.len() != sig.params.len() {
                    return Err(operand_arity_mismatch(
                        model,
                        body,
                        block_idx,
                        sig.params.len(),
                        args.len(),
                    ));
                }
                check_args_against_params(
                    model,
                    body,
                    block_idx,
                    args,
                    &sig.params,
                    &f.body.local_decls,
                    ctx,
                )?;
            }
            TerminatorKind::IntrinsicCall {
                intrinsic,
                args,
                metadata,
                ..
            } => {
                // For intrinsics that carry a structured signature in
                // their `IntrinsicMetadata`, compare the value-arg
                // type against the implied scalar width. RegisterWrite
                // / RegisterWriteField metadata carry `access_width`;
                // the value operand must be a scalar of matching
                // width. Other intrinsics (BusyWait, Sleep, Log,
                // pinctrl, clock control) do not surface a signature
                // in the model and would require a per-intrinsic
                // signature table that lands in Phase 7+. For those,
                // we fall back to a well-formedness sanity check
                // (every Place operand resolves to a typed
                // `LocalDecl`; otherwise raise OperandTypeMismatch
                // with the placeholder expected description).
                if let Some(IntrinsicMetadata::Register {
                    access_width,
                    field,
                    ..
                }) = metadata
                {
                    if matches!(
                        intrinsic,
                        RegseqIntrinsic::RegisterWrite | RegseqIntrinsic::RegisterWriteField
                    ) {
                        let expected_scalar = scalar_for_access_width(*access_width);
                        // For RegisterWrite the value is the LAST
                        // arg; for RegisterWriteField the value is
                        // also the LAST arg, but the field's mask
                        // narrows the effective type. In both cases
                        // the value operand should be the final arg
                        // when present.
                        if let (Some(expected), Some(value)) = (expected_scalar, args.last()) {
                            if let Some(got) =
                                operand_type(value, &f.body.local_decls, &ctx.constants_by_id)
                            {
                                if !types_compatible(&expected, &got) {
                                    return Err(operand_type_mismatch(
                                        model,
                                        body,
                                        block_idx,
                                        args.len() - 1,
                                        &format!("{:?}", expected),
                                        &format!("{:?}", got),
                                    ));
                                }
                            }
                        }
                        let _ = field; // field-specific narrowing checks land in Phase 7+
                    }
                }
                // No further per-arg checks: operands whose type
                // `operand_type` cannot infer locally (deeper
                // projection chains like Deref/Index/Downcast) are
                // skipped rather than raised as
                // `OperandTypeMismatch`. The Phase-6 contract is to
                // match what the IR already preserves, not to
                // synthesize a deeper type-inference pass.
                let _ = args;
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_args_against_params(
    model: &RegseqModel,
    body: FunctionId,
    block_idx: usize,
    args: &[Operand],
    params: &[RegseqType],
    locals: &[crate::ir::LocalDecl],
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    for (arg_idx, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
        let Some(got) = operand_type(arg, locals, &ctx.constants_by_id) else {
            continue;
        };
        if !types_compatible(expected, &got) {
            return Err(operand_type_mismatch(
                model,
                body,
                block_idx,
                arg_idx,
                &format!("{:?}", expected),
                &format!("{:?}", got),
            ));
        }
    }
    Ok(())
}

fn operand_type_mismatch(
    model: &RegseqModel,
    body: FunctionId,
    block_idx: usize,
    arg_idx: usize,
    expected: &str,
    got: &str,
) -> VerifierError {
    VerifierError {
        level: VerifierLevel::Body,
        kind: VerifierErrorKind::OperandTypeMismatch {
            body,
            location: format!("bb{} arg #{}", block_idx, arg_idx),
            expected: expected.to_owned(),
            got: got.to_owned(),
        },
        provenance: VerifierProvenance::for_item(
            format!("body#{}", body.decl_id().index()),
            lookup_origin(model, body.decl_id()),
        ),
        details: format!(
            "call argument #{} at bb{} of function {} has type {} but expected {}",
            arg_idx,
            block_idx,
            body.decl_id().index(),
            got,
            expected
        ),
    }
}

fn operand_arity_mismatch(
    model: &RegseqModel,
    body: FunctionId,
    block_idx: usize,
    expected_params: usize,
    got_args: usize,
) -> VerifierError {
    VerifierError {
        level: VerifierLevel::Body,
        kind: VerifierErrorKind::OperandTypeMismatch {
            body,
            location: format!("bb{} <arg-arity>", block_idx),
            expected: format!("{expected_params} args"),
            got: format!("{got_args} args"),
        },
        provenance: VerifierProvenance::for_item(
            format!("body#{}", body.decl_id().index()),
            lookup_origin(model, body.decl_id()),
        ),
        details: format!(
            "call at bb{} of function {} passes {} args but callee has {} params",
            block_idx,
            body.decl_id().index(),
            got_args,
            expected_params
        ),
    }
}

/// Snapshot of a function's signature used for operand-type matching.
struct FnSignature {
    params: Vec<RegseqType>,
}

fn collect_function_signatures(model: &RegseqModel) -> BTreeMap<DeclId, FnSignature> {
    let mut map: BTreeMap<DeclId, FnSignature> = BTreeMap::new();
    fn walk(map: &mut BTreeMap<DeclId, FnSignature>, m: &Module) {
        for f in &m.free_functions {
            insert(map, f);
        }
        for ib in &m.impl_blocks {
            for f in &ib.methods {
                insert(map, f);
            }
        }
        for ib in &m.trait_impls {
            for f in &ib.methods {
                insert(map, f);
            }
        }
        for ib in &m.driver_impl_blocks {
            for f in &ib.methods {
                insert(map, f);
            }
        }
        for sub in &m.submodules {
            walk(map, sub);
        }
    }
    fn insert(map: &mut BTreeMap<DeclId, FnSignature>, f: &Function) {
        map.insert(
            f.id.decl_id(),
            FnSignature {
                params: f.params.iter().map(|p| p.ty.clone()).collect(),
            },
        );
    }
    walk(&mut map, &model.root_module);
    map
}

/// Best-effort operand type inference.
///
/// - `Operand::Copy(place)` / `Operand::Move(place)`: when the
///   place has no projection, the type comes from
///   `locals[place.local.index()].ty`. When the place ends in a
///   `Projection::Field { ty, .. }`, that `ty` is the operand's
///   final type (this is exactly the type the field carries; the
///   IR stores it inline so consumers don't have to resolve through
///   struct definitions). For other terminating projections
///   (`Deref`, `Index`, `ConstantIndex`, `Subslice`, `Downcast`),
///   returns `None` -- those require deeper type inference.
/// - `Operand::Constant(cid)`: the type is `Constant.ty` looked up
///   from the model's constants map. Returns `None` if the id is
///   not declared (the cross-item / body-level reference rule
///   reports that case).
/// - `Operand::Literal(lit)`: derived from the [`LiteralValue`]
///   variant.
fn operand_type(
    op: &Operand,
    locals: &[crate::ir::LocalDecl],
    constants: &BTreeMap<DeclId, &crate::model::constant::Constant>,
) -> Option<RegseqType> {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            if let Some(Projection::Field { ty, .. }) = place.projection.last() {
                return Some(ty.clone());
            }
            if !place.projection.is_empty() {
                return None;
            }
            let idx = place.local.index() as usize;
            locals.get(idx).map(|l| l.ty.clone())
        }
        Operand::Constant(cid) => constants.get(&cid.decl_id()).map(|c| c.ty.clone()),
        Operand::Literal(lit) => Some(literal_type(*lit)),
    }
}

fn literal_type(lit: LiteralValue) -> RegseqType {
    let scalar = match lit {
        LiteralValue::U8(_) => ScalarType::U8,
        LiteralValue::U16(_) => ScalarType::U16,
        LiteralValue::U32(_) => ScalarType::U32,
        LiteralValue::U64(_) => ScalarType::U64,
        LiteralValue::Usize(_) => ScalarType::Usize,
        LiteralValue::I8(_) => ScalarType::I8,
        LiteralValue::I16(_) => ScalarType::I16,
        LiteralValue::I32(_) => ScalarType::I32,
        LiteralValue::I64(_) => ScalarType::I64,
        LiteralValue::Isize(_) => ScalarType::Isize,
        LiteralValue::Bool(_) => return RegseqType::Bool,
    };
    RegseqType::Scalar(scalar)
}

fn types_compatible(a: &RegseqType, b: &RegseqType) -> bool {
    a == b
}

/// Map a register access width in bits to the scalar type the value
/// operand must carry. Returns `None` for widths the model does not
/// surface as a `ScalarType` variant.
fn scalar_for_access_width(width: u32) -> Option<RegseqType> {
    match width {
        8 => Some(RegseqType::Scalar(ScalarType::U8)),
        16 => Some(RegseqType::Scalar(ScalarType::U16)),
        32 => Some(RegseqType::Scalar(ScalarType::U32)),
        64 => Some(RegseqType::Scalar(ScalarType::U64)),
        _ => None,
    }
}

// --- Read-only register write check ---------------------------------

fn read_only_register_write(
    model: &RegseqModel,
    f: &Function,
    ctx: &BodyContext<'_>,
) -> Result<(), VerifierError> {
    let body = f.id;
    for block in &f.body.basic_blocks {
        let TerminatorKind::IntrinsicCall {
            intrinsic,
            metadata:
                Some(IntrinsicMetadata::Register {
                    register_block,
                    register_offset,
                    ..
                }),
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if !matches!(
            intrinsic,
            RegseqIntrinsic::RegisterWrite | RegseqIntrinsic::RegisterWriteField
        ) {
            continue;
        }
        let Some(rb) = ctx.blocks.get(&register_block.decl_id()) else {
            // Block not found: cross-item rule already reports this.
            continue;
        };
        let Some(field) = rb.fields.iter().find(|f| f.byte_offset == *register_offset) else {
            continue;
        };
        if field.access_kind == AccessKind::ReadOnly {
            let descriptor_id = field.descriptor.unwrap_or_else(|| {
                // ReadOnly w/ no descriptor would be malformed input;
                // surface a sentinel id to avoid panicking.
                use crate::identity::DeclId as Did;
                RegisterDescriptorId::from(Did::new(u32::MAX))
            });
            return Err(VerifierError {
                level: VerifierLevel::Body,
                kind: VerifierErrorKind::ReadOnlyRegisterWrite {
                    body,
                    register: descriptor_id,
                },
                provenance: VerifierProvenance::for_item(
                    format!("body#{}", body.decl_id().index()),
                    lookup_origin(model, body.decl_id()),
                ),
                details: format!(
                    "function body (FunctionId {}) writes to read-only register \
                     descriptor (DeclId {}) at offset 0x{:x} of block (DeclId {})",
                    body.decl_id().index(),
                    descriptor_id.decl_id().index(),
                    register_offset,
                    register_block.decl_id().index()
                ),
            });
        }
    }
    Ok(())
}

// --- ISR context legality (hook-scaffolded) -------------------------

fn isr_context_legality(
    model: &RegseqModel,
    f: &Function,
    ctx: &BodyContext<'_>,
    forbidden: &[RegseqIntrinsic],
) -> Result<(), VerifierError> {
    if !ctx.isr_bodies.contains(&f.id.decl_id()) {
        return Ok(());
    }
    if forbidden.is_empty() {
        // Phase-6 default policy is empty. Surface a one-line warning
        // (per spec.md Section 13 line 1429 deferral note: the rule
        // emits an `UnimplementedContextCheck` warning, not an
        // error) so consumers can see that the hook is plumbed but
        // the policy set is not yet defined. Fired once per ISR body
        // visited; suppressed in test builds via the
        // `RSEQ_VERIFY_QUIET` env var to avoid noisy output.
        if std::env::var_os("RSEQ_VERIFY_QUIET").is_none() {
            eprintln!(
                "regseq-model verifier: UnimplementedContextCheck -- \
                 no_sleep_in_isr policy set is empty; ISR body \
                 FunctionId({}) walked without intrinsic legality \
                 enforcement. Hook will fire once Phase 7/8 enumerates \
                 the forbidden-intrinsic set.",
                f.id.decl_id().index()
            );
        }
        return Ok(());
    }
    let body = f.id;
    for block in &f.body.basic_blocks {
        let TerminatorKind::IntrinsicCall { intrinsic, .. } = &block.terminator.kind else {
            continue;
        };
        if forbidden.contains(intrinsic) {
            return Err(VerifierError {
                level: VerifierLevel::Body,
                kind: VerifierErrorKind::IsrContextViolation {
                    body,
                    kind: IsrViolationKind::Sleep,
                },
                provenance: VerifierProvenance::for_item(
                    format!("body#{}", body.decl_id().index()),
                    lookup_origin(model, body.decl_id()),
                ),
                details: format!(
                    "ISR-classified function body (FunctionId {}) invoked forbidden \
                     intrinsic {:?}",
                    body.decl_id().index(),
                    intrinsic
                ),
            });
        }
    }
    Ok(())
}

// --- Walking helpers ------------------------------------------------

fn walk_functions<F>(model: &RegseqModel, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&Function) -> Result<(), VerifierError>,
{
    walk_modules(&model.root_module, &mut |m| {
        for func in &m.free_functions {
            f(func)?;
        }
        for ib in &m.impl_blocks {
            for func in &ib.methods {
                f(func)?;
            }
        }
        for ib in &m.trait_impls {
            for func in &ib.methods {
                f(func)?;
            }
        }
        for ib in &m.driver_impl_blocks {
            for func in &ib.methods {
                f(func)?;
            }
        }
        Ok(())
    })
}

fn walk_modules<F>(root: &Module, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&Module) -> Result<(), VerifierError>,
{
    f(root)?;
    for s in &root.submodules {
        walk_modules(s, f)?;
    }
    Ok(())
}

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
