//! Conversion from rustc MIR types to owned [`crate::mir_repr`] types.
//!
//! All functions here take a `TyCtxt` and rustc MIR values, returning
//! fully-owned representations that can be serialized without any rustc
//! arena dependency.

use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use crate::mir_repr::*;

// ---------------------------------------------------------------------------
// Source info conversion
// ---------------------------------------------------------------------------

fn convert_source_info(info: &mir::SourceInfo) -> MirSourceInfo {
    MirSourceInfo {
        scope: info.scope.as_u32(),
        span: None,
    }
}

// ---------------------------------------------------------------------------
// Body-level conversion
// ---------------------------------------------------------------------------

/// Convert a borrowed post-drop-elaboration MIR body into an owned `MirBody`.
pub(crate) fn convert_body<'tcx>(tcx: TyCtxt<'tcx>, body: &mir::Body<'tcx>) -> MirBody {
    let local_decls = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| convert_local_decl(tcx, local, decl, body.arg_count))
        .collect();

    let basic_blocks = body
        .basic_blocks
        .iter()
        .map(|bb| convert_basic_block(tcx, bb))
        .collect();

    let source_scopes = body
        .source_scopes
        .iter_enumerated()
        .map(|(idx, _scope)| MirSourceScope {
            index: idx.as_u32(),
            name: None,
        })
        .collect();

    MirBody {
        local_decls,
        arg_count: body.arg_count,
        basic_blocks,
        source_scopes,
    }
}

fn convert_local_decl<'tcx>(
    tcx: TyCtxt<'tcx>,
    local: mir::Local,
    decl: &mir::LocalDecl<'tcx>,
    arg_count: usize,
) -> MirLocalDecl {
    let idx = local.as_u32();
    let name = if idx == 0 {
        Some("_0".to_string())
    } else if (idx as usize) <= arg_count {
        Some(format!("_arg{idx}"))
    } else {
        Some(format!("_{idx}"))
    };
    MirLocalDecl {
        index: idx,
        ty: convert_ty(tcx, decl.ty),
        mutability: convert_mutability(decl.mutability),
        name,
    }
}

// ---------------------------------------------------------------------------
// Basic block conversion
// ---------------------------------------------------------------------------

fn convert_basic_block<'tcx>(
    tcx: TyCtxt<'tcx>,
    bb: &mir::BasicBlockData<'tcx>,
) -> MirBasicBlockData {
    MirBasicBlockData {
        statements: bb
            .statements
            .iter()
            .map(|s| convert_statement(tcx, s))
            .collect(),
        terminator: bb.terminator.as_ref().map(|t| convert_terminator(tcx, t)),
        is_cleanup: bb.is_cleanup,
    }
}

// ---------------------------------------------------------------------------
// Statement conversion
// ---------------------------------------------------------------------------

fn convert_statement<'tcx>(tcx: TyCtxt<'tcx>, stmt: &mir::Statement<'tcx>) -> MirStatement {
    MirStatement {
        source_info: convert_source_info(&stmt.source_info),
        kind: convert_statement_kind(tcx, &stmt.kind),
    }
}

fn convert_statement_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &mir::StatementKind<'tcx>,
) -> MirStatementKind {
    match kind {
        mir::StatementKind::Assign(box (place, rvalue)) => MirStatementKind::Assign {
            place: convert_place(tcx, place),
            rvalue: Box::new(convert_rvalue(tcx, rvalue)),
        },
        mir::StatementKind::StorageLive(local) => MirStatementKind::StorageLive {
            local: local.as_u32(),
        },
        mir::StatementKind::StorageDead(local) => MirStatementKind::StorageDead {
            local: local.as_u32(),
        },
        mir::StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => MirStatementKind::SetDiscriminant {
            place: convert_place(tcx, place),
            variant_index: variant_index.as_u32(),
        },
        mir::StatementKind::Nop => MirStatementKind::Nop,
        other => MirStatementKind::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Terminator conversion
// ---------------------------------------------------------------------------

fn convert_terminator<'tcx>(tcx: TyCtxt<'tcx>, term: &mir::Terminator<'tcx>) -> MirTerminator {
    MirTerminator {
        source_info: convert_source_info(&term.source_info),
        kind: convert_terminator_kind(tcx, &term.kind),
    }
}

fn convert_terminator_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &mir::TerminatorKind<'tcx>,
) -> MirTerminatorKind {
    match kind {
        mir::TerminatorKind::Goto { target } => MirTerminatorKind::Goto {
            target: target.as_u32(),
        },
        mir::TerminatorKind::SwitchInt { discr, targets } => MirTerminatorKind::SwitchInt {
            discr: convert_operand(tcx, discr),
            targets: convert_switch_targets(targets),
        },
        mir::TerminatorKind::Return => MirTerminatorKind::Return,
        mir::TerminatorKind::Unreachable => MirTerminatorKind::Unreachable,
        mir::TerminatorKind::UnwindResume => MirTerminatorKind::UnwindResume,
        mir::TerminatorKind::UnwindTerminate(reason) => MirTerminatorKind::UnwindTerminate {
            reason: format!("{reason:?}"),
        },
        mir::TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } => MirTerminatorKind::Call {
            func: convert_operand(tcx, func),
            args: args.iter().map(|a| convert_operand(tcx, &a.node)).collect(),
            destination: convert_place(tcx, destination),
            target: target.map(|bb| bb.as_u32()),
            unwind: convert_unwind_action(unwind),
        },
        mir::TerminatorKind::Drop {
            place,
            target,
            unwind,
            replace,
            ..
        } => MirTerminatorKind::Drop {
            place: convert_place(tcx, place),
            target: target.as_u32(),
            unwind: convert_unwind_action(unwind),
            replace: *replace,
        },
        mir::TerminatorKind::Assert {
            cond,
            expected,
            msg,
            target,
            unwind,
        } => MirTerminatorKind::Assert {
            cond: convert_operand(tcx, cond),
            expected: *expected,
            msg: format!("{msg:?}"),
            target: target.as_u32(),
            unwind: convert_unwind_action(unwind),
        },
        other => MirTerminatorKind::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

fn convert_switch_targets(targets: &mir::SwitchTargets) -> MirSwitchTargets {
    let mut values = Vec::new();
    let mut bbs = Vec::new();
    for (val, target) in targets.iter() {
        values.push(val);
        bbs.push(target.as_u32());
    }
    bbs.push(targets.otherwise().as_u32());
    MirSwitchTargets {
        values,
        targets: bbs,
    }
}

fn convert_unwind_action(action: &mir::UnwindAction) -> MirUnwindAction {
    match action {
        mir::UnwindAction::Continue => MirUnwindAction::Continue,
        mir::UnwindAction::Unreachable => MirUnwindAction::Unreachable,
        mir::UnwindAction::Terminate(reason) => MirUnwindAction::Terminate {
            reason: format!("{reason:?}"),
        },
        mir::UnwindAction::Cleanup(bb) => MirUnwindAction::Cleanup {
            target: bb.as_u32(),
        },
    }
}

// ---------------------------------------------------------------------------
// Place conversion
// ---------------------------------------------------------------------------

pub(crate) fn convert_place<'tcx>(tcx: TyCtxt<'tcx>, place: &mir::Place<'tcx>) -> MirPlace {
    MirPlace {
        local: place.local.as_u32(),
        projection: place
            .projection
            .iter()
            .map(|elem| convert_projection_elem(tcx, &elem))
            .collect(),
    }
}

fn convert_projection_elem<'tcx>(
    tcx: TyCtxt<'tcx>,
    elem: &mir::PlaceElem<'tcx>,
) -> MirProjectionElem {
    match elem {
        mir::ProjectionElem::Deref => MirProjectionElem::Deref,
        mir::ProjectionElem::Field(idx, ty) => MirProjectionElem::Field {
            index: idx.as_u32(),
            ty: convert_ty(tcx, *ty),
        },
        mir::ProjectionElem::Index(local) => MirProjectionElem::Index {
            local: local.as_u32(),
        },
        mir::ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => MirProjectionElem::ConstantIndex {
            offset: *offset,
            min_length: *min_length,
            from_end: *from_end,
        },
        mir::ProjectionElem::Subslice { from, to, from_end } => MirProjectionElem::Subslice {
            from: *from,
            to: *to,
            from_end: *from_end,
        },
        mir::ProjectionElem::Downcast(name, variant_idx) => MirProjectionElem::Downcast {
            name: name.map(|s| s.to_string()),
            variant_index: variant_idx.as_u32(),
        },
        mir::ProjectionElem::OpaqueCast(ty) => MirProjectionElem::OpaqueCast {
            ty: convert_ty(tcx, *ty),
        },
        other => MirProjectionElem::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Operand conversion
// ---------------------------------------------------------------------------

pub(crate) fn convert_operand<'tcx>(tcx: TyCtxt<'tcx>, operand: &mir::Operand<'tcx>) -> MirOperand {
    match operand {
        mir::Operand::Copy(place) => MirOperand::Copy {
            place: convert_place(tcx, place),
        },
        mir::Operand::Move(place) => MirOperand::Move {
            place: convert_place(tcx, place),
        },
        mir::Operand::Constant(box const_op) => MirOperand::Constant {
            constant: convert_const_operand(tcx, const_op),
        },
        // RuntimeChecks variant -- treat as unsupported for now
        _ => MirOperand::Constant {
            constant: MirConstant {
                ty: MirTy::Unsupported {
                    description: "runtime_checks_operand".to_string(),
                },
                value: MirConstValue::Unsupported {
                    description: "runtime_checks".to_string(),
                },
            },
        },
    }
}

// ---------------------------------------------------------------------------
// Rvalue conversion
// ---------------------------------------------------------------------------

fn convert_rvalue<'tcx>(tcx: TyCtxt<'tcx>, rvalue: &mir::Rvalue<'tcx>) -> MirRvalue {
    match rvalue {
        mir::Rvalue::Use(op) => MirRvalue::Use {
            operand: convert_operand(tcx, op),
        },
        mir::Rvalue::Ref(_, borrow_kind, place) => MirRvalue::Ref {
            borrow_kind: convert_borrow_kind(borrow_kind),
            place: convert_place(tcx, place),
        },
        mir::Rvalue::BinaryOp(op, box (lhs, rhs)) => MirRvalue::BinaryOp {
            op: convert_bin_op(op),
            lhs: convert_operand(tcx, lhs),
            rhs: convert_operand(tcx, rhs),
        },
        mir::Rvalue::UnaryOp(op, operand) => MirRvalue::UnaryOp {
            op: convert_un_op(op),
            operand: convert_operand(tcx, operand),
        },
        mir::Rvalue::Discriminant(place) => MirRvalue::Discriminant {
            place: convert_place(tcx, place),
        },
        mir::Rvalue::Aggregate(box agg_kind, operands) => MirRvalue::Aggregate {
            agg_kind: convert_aggregate_kind(tcx, agg_kind),
            operands: operands.iter().map(|op| convert_operand(tcx, op)).collect(),
        },
        mir::Rvalue::Cast(cast_kind, operand, ty) => MirRvalue::Cast {
            cast_kind: convert_cast_kind(cast_kind),
            operand: convert_operand(tcx, operand),
            ty: convert_ty(tcx, *ty),
        },
        mir::Rvalue::CopyForDeref(place) => MirRvalue::CopyForDeref {
            place: convert_place(tcx, place),
        },
        other => MirRvalue::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

fn convert_borrow_kind(kind: &mir::BorrowKind) -> MirBorrowKind {
    match kind {
        mir::BorrowKind::Shared => MirBorrowKind::Shared,
        mir::BorrowKind::Mut { .. } => MirBorrowKind::Mut,
        other => MirBorrowKind::Other(format!("{other:?}")),
    }
}

fn convert_aggregate_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &mir::AggregateKind<'tcx>,
) -> MirAggregateKind {
    match kind {
        mir::AggregateKind::Array(ty) => MirAggregateKind::Array {
            ty: convert_ty(tcx, *ty),
        },
        mir::AggregateKind::Tuple => MirAggregateKind::Tuple,
        mir::AggregateKind::Adt(def_id, variant_idx, args, _, _) => MirAggregateKind::Adt {
            path: tcx.def_path_str(*def_id),
            variant_index: variant_idx.as_u32(),
            args: args.types().map(|t| convert_ty(tcx, t)).collect(),
        },
        mir::AggregateKind::Closure(def_id, args) => MirAggregateKind::Closure {
            path: tcx.def_path_str(*def_id),
            args: args.types().map(|t| convert_ty(tcx, t)).collect(),
        },
        other => MirAggregateKind::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

fn convert_cast_kind(kind: &mir::CastKind) -> MirCastKind {
    match kind {
        mir::CastKind::IntToInt => MirCastKind::IntToInt,
        mir::CastKind::FloatToInt => MirCastKind::FloatToInt,
        mir::CastKind::FloatToFloat => MirCastKind::FloatToFloat,
        mir::CastKind::IntToFloat => MirCastKind::IntToFloat,
        mir::CastKind::PtrToPtr => MirCastKind::PtrToPtr,
        mir::CastKind::FnPtrToPtr => MirCastKind::FnPtrToPtr,
        mir::CastKind::Transmute => MirCastKind::Transmute,
        mir::CastKind::Subtype => MirCastKind::Subtype,
        mir::CastKind::PointerCoercion(coercion, _) => {
            MirCastKind::PointerCoercion(format!("{coercion:?}"))
        }
        other => MirCastKind::Other(format!("{other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Type conversion
// ---------------------------------------------------------------------------

pub(crate) fn convert_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: ty::Ty<'tcx>) -> MirTy {
    match ty.kind() {
        ty::TyKind::Bool => MirTy::Bool,
        ty::TyKind::Char => MirTy::Char,
        ty::TyKind::Int(int_ty) => MirTy::Int {
            int_ty: convert_int_ty(*int_ty),
        },
        ty::TyKind::Uint(uint_ty) => MirTy::Uint {
            uint_ty: convert_uint_ty(*uint_ty),
        },
        ty::TyKind::Float(float_ty) => MirTy::Float {
            float_ty: convert_float_ty(*float_ty),
        },
        ty::TyKind::Ref(_, inner, mutbl) => MirTy::Ref {
            mutbl: mutbl.is_mut(),
            ty: Box::new(convert_ty(tcx, *inner)),
        },
        ty::TyKind::RawPtr(inner, mutbl) => MirTy::RawPtr {
            mutbl: mutbl.is_mut(),
            ty: Box::new(convert_ty(tcx, *inner)),
        },
        ty::TyKind::Adt(adt_def, args) => MirTy::Adt {
            path: tcx.def_path_str(adt_def.did()),
            args: args.types().map(|t| convert_ty(tcx, t)).collect(),
        },
        ty::TyKind::FnDef(def_id, args) => MirTy::FnDef {
            path: tcx.def_path_str(*def_id),
            args: args.types().map(|t| convert_ty(tcx, t)).collect(),
        },
        ty::TyKind::Tuple(elems) => MirTy::Tuple {
            elements: elems.iter().map(|t| convert_ty(tcx, t)).collect(),
        },
        ty::TyKind::Array(elem, len) => MirTy::Array {
            ty: Box::new(convert_ty(tcx, *elem)),
            len: len.try_to_target_usize(tcx).unwrap_or(0),
        },
        ty::TyKind::Slice(elem) => MirTy::Slice {
            ty: Box::new(convert_ty(tcx, *elem)),
        },
        ty::TyKind::Str => MirTy::Str,
        ty::TyKind::Never => MirTy::Never,
        ty::TyKind::Param(param) => MirTy::Param {
            name: param.name.to_string(),
        },
        _ => MirTy::Unsupported {
            description: format!("{ty}"),
        },
    }
}

fn convert_int_ty(ty: ty::IntTy) -> MirIntTy {
    match ty {
        ty::IntTy::Isize => MirIntTy::Isize,
        ty::IntTy::I8 => MirIntTy::I8,
        ty::IntTy::I16 => MirIntTy::I16,
        ty::IntTy::I32 => MirIntTy::I32,
        ty::IntTy::I64 => MirIntTy::I64,
        ty::IntTy::I128 => MirIntTy::I128,
    }
}

fn convert_uint_ty(ty: ty::UintTy) -> MirUintTy {
    match ty {
        ty::UintTy::Usize => MirUintTy::Usize,
        ty::UintTy::U8 => MirUintTy::U8,
        ty::UintTy::U16 => MirUintTy::U16,
        ty::UintTy::U32 => MirUintTy::U32,
        ty::UintTy::U64 => MirUintTy::U64,
        ty::UintTy::U128 => MirUintTy::U128,
    }
}

fn convert_float_ty(ty: ty::FloatTy) -> MirFloatTy {
    match ty {
        ty::FloatTy::F32 => MirFloatTy::F32,
        ty::FloatTy::F64 => MirFloatTy::F64,
        _ => MirFloatTy::F64,
    }
}

fn convert_mutability(m: mir::Mutability) -> MirMutability {
    match m {
        mir::Mutability::Not => MirMutability::Not,
        mir::Mutability::Mut => MirMutability::Mut,
    }
}

// ---------------------------------------------------------------------------
// Constant conversion
// ---------------------------------------------------------------------------

fn convert_const_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    const_op: &mir::ConstOperand<'tcx>,
) -> MirConstant {
    let ty = convert_ty(tcx, const_op.const_.ty());
    let value = match const_op.const_ {
        mir::Const::Val(val, _) => convert_const_value(&val),
        mir::Const::Ty(_, _) => MirConstValue::Unsupported {
            description: "ty_const".to_string(),
        },
        mir::Const::Unevaluated(uneval, _) => MirConstValue::Unsupported {
            description: format!("unevaluated:{}", tcx.def_path_str(uneval.def)),
        },
    };
    MirConstant { ty, value }
}

fn convert_const_value(val: &mir::ConstValue) -> MirConstValue {
    match val {
        mir::ConstValue::Scalar(scalar) => match scalar {
            mir::interpret::Scalar::Int(int) => MirConstValue::Scalar {
                bits: int.to_bits(int.size()),
                size: int.size().bytes() as u8,
            },
            mir::interpret::Scalar::Ptr(_, _) => MirConstValue::Unsupported {
                description: "scalar_ptr".to_string(),
            },
        },
        mir::ConstValue::ZeroSized => MirConstValue::ZeroSized,
        other => MirConstValue::Unsupported {
            description: format!("{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Operator conversion
// ---------------------------------------------------------------------------

fn convert_bin_op(op: &mir::BinOp) -> MirBinOp {
    match op {
        mir::BinOp::Add => MirBinOp::Add,
        mir::BinOp::AddUnchecked => MirBinOp::AddUnchecked,
        mir::BinOp::AddWithOverflow => MirBinOp::AddWithOverflow,
        mir::BinOp::Sub => MirBinOp::Sub,
        mir::BinOp::SubUnchecked => MirBinOp::SubUnchecked,
        mir::BinOp::SubWithOverflow => MirBinOp::SubWithOverflow,
        mir::BinOp::Mul => MirBinOp::Mul,
        mir::BinOp::MulUnchecked => MirBinOp::MulUnchecked,
        mir::BinOp::MulWithOverflow => MirBinOp::MulWithOverflow,
        mir::BinOp::Div => MirBinOp::Div,
        mir::BinOp::Rem => MirBinOp::Rem,
        mir::BinOp::BitAnd => MirBinOp::BitAnd,
        mir::BinOp::BitOr => MirBinOp::BitOr,
        mir::BinOp::BitXor => MirBinOp::BitXor,
        mir::BinOp::Shl => MirBinOp::Shl,
        mir::BinOp::ShlUnchecked => MirBinOp::ShlUnchecked,
        mir::BinOp::Shr => MirBinOp::Shr,
        mir::BinOp::ShrUnchecked => MirBinOp::ShrUnchecked,
        mir::BinOp::Eq => MirBinOp::Eq,
        mir::BinOp::Ne => MirBinOp::Ne,
        mir::BinOp::Lt => MirBinOp::Lt,
        mir::BinOp::Le => MirBinOp::Le,
        mir::BinOp::Gt => MirBinOp::Gt,
        mir::BinOp::Ge => MirBinOp::Ge,
        mir::BinOp::Cmp => MirBinOp::Cmp,
        mir::BinOp::Offset => MirBinOp::Offset,
    }
}

fn convert_un_op(op: &mir::UnOp) -> MirUnOp {
    match op {
        mir::UnOp::Not => MirUnOp::Not,
        mir::UnOp::Neg => MirUnOp::Neg,
        mir::UnOp::PtrMetadata => MirUnOp::PtrMetadata,
    }
}
