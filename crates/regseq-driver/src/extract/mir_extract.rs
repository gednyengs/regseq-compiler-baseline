//! Conversion from rustc MIR types to owned [`crate::mir_repr`] types.
//!
//! All functions here take a `TyCtxt` and rustc MIR values, returning
//! fully-owned representations that can be serialized without any rustc
//! arena dependency.

use std::collections::{HashMap, HashSet};

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
    let mut local_decls: Vec<MirLocalDecl> = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| convert_local_decl(tcx, local, decl, body.arg_count))
        .collect();

    // Patch in authored debug variable names from MIR var_debug_info.
    // This preserves user-authored local names (e.g. `handled`, `clear_mask`)
    // so that downstream analysis can trace exact variable identity through
    // the CFG without relying on positional indices alone.
    for debug_info in &body.var_debug_info {
        if let mir::VarDebugInfoContents::Place(place) = &debug_info.value {
            if place.projection.is_empty() {
                let local_idx = place.local.as_u32() as usize;
                if let Some(decl) = local_decls.get_mut(local_idx) {
                    decl.name = Some(debug_info.name.to_string());
                }
            }
        }
    }

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

    let symbolic_consts = resolve_symbolic_consts(tcx, body);

    MirBody {
        local_decls,
        arg_count: body.arg_count,
        basic_blocks,
        source_scopes,
        symbolic_consts,
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
                    path: None,
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

/// Convert a rustc type into an owned `MirTy`.
///
/// This is the public entry point. It creates a fresh recursion-tracking
/// set and delegates to the inner converter. Recursive or mutually
/// recursive nominal types are represented via `MirTy::NominalRef`.
pub(crate) fn convert_ty<'tcx>(tcx: TyCtxt<'tcx>, ty: ty::Ty<'tcx>) -> MirTy {
    let mut expanding = HashSet::new();
    convert_ty_inner(tcx, ty, &mut expanding)
}

/// Inner type conversion with recursion tracking.
///
/// The `expanding` set tracks which ADT `DefId`s are currently being
/// resolved. When a type is encountered that is already in the set,
/// we emit `MirTy::NominalRef` to break the cycle.
fn convert_ty_inner<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    expanding: &mut HashSet<rustc_hir::def_id::DefId>,
) -> MirTy {
    match ty.kind() {
        ty::TyKind::Bool => MirTy::Bool,
        ty::TyKind::Char => MirTy::Char,
        ty::TyKind::Int(int_ty) => MirTy::Int {
            signed: true,
            width: convert_int_width(*int_ty),
        },
        ty::TyKind::Uint(uint_ty) => MirTy::Int {
            signed: false,
            width: convert_uint_width(*uint_ty),
        },
        ty::TyKind::Float(float_ty) => MirTy::Float {
            width: convert_float_width(*float_ty),
        },
        ty::TyKind::Ref(_, inner, mutbl) => MirTy::Ref {
            mutbl: mutbl.is_mut(),
            ty: Box::new(convert_ty_inner(tcx, *inner, expanding)),
        },
        ty::TyKind::RawPtr(inner, mutbl) => MirTy::RawPtr {
            mutbl: mutbl.is_mut(),
            ty: Box::new(convert_ty_inner(tcx, *inner, expanding)),
        },
        ty::TyKind::Adt(adt_def, args) => {
            let did = adt_def.did();
            let path = tcx.def_path_str(did);

            // Recursion guard: if this type is already being expanded,
            // emit a nominal reference instead of recursing.
            if !expanding.insert(did) {
                return MirTy::NominalRef { path };
            }

            let result = if adt_def.is_enum() {
                let variants = adt_def
                    .variants()
                    .iter()
                    .map(|variant| {
                        let fields = variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(i, field)| {
                                let field_ty = field.ty(tcx, args);
                                // Zero-sized fields (PhantomData, etc.)
                                let ty = if let Ok(layout) = tcx.layout_of(
                                    ty::TypingEnv::fully_monomorphized().as_query_input(field_ty),
                                ) {
                                    if layout.is_zst() {
                                        MirTy::ZeroSizedLike {
                                            description: format!("{field_ty}"),
                                        }
                                    } else {
                                        convert_ty_inner(tcx, field_ty, expanding)
                                    }
                                } else {
                                    convert_ty_inner(tcx, field_ty, expanding)
                                };
                                MirFieldTy {
                                    index: i as u32,
                                    name: field.name.to_string(),
                                    ty,
                                }
                            })
                            .collect();
                        MirVariantTy {
                            name: variant.name.to_string(),
                            fields,
                        }
                    })
                    .collect();
                MirTy::Enum { path, variants }
            } else if adt_def.is_struct() {
                let variant = adt_def.non_enum_variant();
                let fields = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        let field_ty = field.ty(tcx, args);
                        let ty = if let Ok(layout) = tcx.layout_of(
                            ty::TypingEnv::fully_monomorphized().as_query_input(field_ty),
                        ) {
                            if layout.is_zst() {
                                MirTy::ZeroSizedLike {
                                    description: format!("{field_ty}"),
                                }
                            } else {
                                convert_ty_inner(tcx, field_ty, expanding)
                            }
                        } else {
                            convert_ty_inner(tcx, field_ty, expanding)
                        };
                        MirFieldTy {
                            index: i as u32,
                            name: field.name.to_string(),
                            ty,
                        }
                    })
                    .collect();
                MirTy::Struct { path, fields }
            } else {
                // Union type - not supported in the MIR schema.
                MirTy::Unsupported {
                    description: format!("union: {path}"),
                }
            };

            expanding.remove(&did);
            result
        }
        ty::TyKind::FnDef(def_id, args) => {
            let base_path = tcx.def_path_str(*def_id);
            // Embed generic type arguments in the path so that downstream
            // consumers can discover type-parameter identities (e.g.
            // distinguishing `apply::<DefaultState>` from `apply::<SleepState>`)
            // without needing a separate generic-args field.
            let type_args: Vec<String> = args.types().map(|t| format!("{t}")).collect();
            let path = if type_args.is_empty() {
                base_path
            } else {
                format!("{base_path}::<{}>", type_args.join(", "))
            };
            let fn_sig = tcx.fn_sig(*def_id).instantiate(tcx, args).skip_binder();
            let params = fn_sig
                .inputs()
                .iter()
                .map(|t| convert_ty_inner(tcx, *t, expanding))
                .collect();
            let ret = Box::new(convert_ty_inner(tcx, fn_sig.output(), expanding));
            MirTy::Fn { path, params, ret }
        }
        ty::TyKind::Tuple(elems) => MirTy::Tuple {
            elements: elems
                .iter()
                .map(|t| convert_ty_inner(tcx, t, expanding))
                .collect(),
        },
        ty::TyKind::Array(elem, len) => MirTy::Array {
            element: Box::new(convert_ty_inner(tcx, *elem, expanding)),
            len: len.try_to_target_usize(tcx).unwrap_or(0),
        },
        ty::TyKind::Slice(elem) => MirTy::Slice {
            element: Box::new(convert_ty_inner(tcx, *elem, expanding)),
        },
        ty::TyKind::Str => MirTy::Str,
        ty::TyKind::Never => MirTy::Never,
        ty::TyKind::Closure(def_id, _) => MirTy::ZeroSizedLike {
            description: format!("closure:{}", tcx.def_path_str(*def_id)),
        },
        ty::TyKind::Param(param) => MirTy::Unsupported {
            description: format!("param:{}", param.name),
        },
        _ => MirTy::Unsupported {
            description: format!("{ty}"),
        },
    }
}

fn convert_int_width(ty: ty::IntTy) -> IntWidth {
    match ty {
        ty::IntTy::Isize => IntWidth::Isize,
        ty::IntTy::I8 => IntWidth::I8,
        ty::IntTy::I16 => IntWidth::I16,
        ty::IntTy::I32 => IntWidth::I32,
        ty::IntTy::I64 => IntWidth::I64,
        ty::IntTy::I128 => IntWidth::I128,
    }
}

fn convert_uint_width(ty: ty::UintTy) -> IntWidth {
    match ty {
        ty::UintTy::Usize => IntWidth::Usize,
        ty::UintTy::U8 => IntWidth::U8,
        ty::UintTy::U16 => IntWidth::U16,
        ty::UintTy::U32 => IntWidth::U32,
        ty::UintTy::U64 => IntWidth::U64,
        ty::UintTy::U128 => IntWidth::U128,
    }
}

fn convert_float_width(ty: ty::FloatTy) -> FloatWidth {
    match ty {
        ty::FloatTy::F32 => FloatWidth::F32,
        ty::FloatTy::F64 => FloatWidth::F64,
        _ => FloatWidth::F64,
    }
}

fn convert_mutability(m: mir::Mutability) -> MirMutability {
    match m {
        mir::Mutability::Not => MirMutability::Not,
        mir::Mutability::Mut => MirMutability::Mut,
    }
}

// ---------------------------------------------------------------------------
// Symbolic constant resolution
// ---------------------------------------------------------------------------

/// Scan a MIR body for scalar constant operands whose ADT type has named
/// associated constants. For each unique (type, scalar) pair that resolves
/// unambiguously to a single associated constant, emit a mapping.
fn resolve_symbolic_consts<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mir::Body<'tcx>,
) -> Vec<SymbolicConstMapping> {
    // Collect all (ty, scalar_bits) pairs seen in the body.
    let mut seen: HashMap<(String, u128), ()> = HashMap::new();

    for bb in body.basic_blocks.iter() {
        visit_operands_in_block(bb, |operand| {
            if let mir::Operand::Constant(box const_op) = operand {
                if let mir::Const::Val(
                    mir::ConstValue::Scalar(mir::interpret::Scalar::Int(int)),
                    const_ty,
                ) = const_op.const_
                {
                    if let ty::TyKind::Adt(adt_def, _) = const_ty.kind() {
                        if adt_def.is_struct() {
                            let path = tcx.def_path_str(adt_def.did());
                            let bits = int.to_bits(int.size());
                            seen.insert((path, bits), ());
                        }
                    }
                }
            }
        });
    }

    if seen.is_empty() {
        return Vec::new();
    }

    // Group by type path.
    let mut by_type: HashMap<String, Vec<u128>> = HashMap::new();
    for (ty_path, bits) in seen.keys() {
        by_type.entry(ty_path.clone()).or_default().push(*bits);
    }

    let mut result = Vec::new();
    let typing_env = ty::TypingEnv::fully_monomorphized();

    for (ty_path, values) in &by_type {
        // Find the ADT DefId for this type path. We need to look it up
        // from one of the constants we already saw.
        let adt_did = {
            let mut found = None;
            'outer: for bb in body.basic_blocks.iter() {
                visit_operands_in_block(bb, |operand| {
                    if found.is_some() {
                        return;
                    }
                    if let mir::Operand::Constant(box const_op) = operand {
                        if let mir::Const::Val(_, const_ty) = const_op.const_ {
                            if let ty::TyKind::Adt(adt_def, _) = const_ty.kind() {
                                if tcx.def_path_str(adt_def.did()) == *ty_path {
                                    found = Some(adt_def.did());
                                }
                            }
                        }
                    }
                });
                if found.is_some() {
                    break 'outer;
                }
            }
            match found {
                Some(did) => did,
                None => continue,
            }
        };

        // Collect all associated constants from inherent impls of this type.
        let inherent_impls = tcx.inherent_impls(adt_did);

        let mut const_map: HashMap<u128, Vec<String>> = HashMap::new();
        for &impl_did in inherent_impls.iter() {
            for item in tcx.associated_items(impl_did).in_definition_order() {
                if !matches!(item.kind, ty::AssocKind::Const { .. }) {
                    continue;
                }
                let uneval = mir::UnevaluatedConst {
                    def: item.def_id,
                    args: ty::List::empty(),
                    promoted: None,
                };
                let Ok(const_val) =
                    tcx.const_eval_resolve(typing_env, uneval, tcx.def_span(item.def_id))
                else {
                    continue;
                };
                let mir::ConstValue::Scalar(mir::interpret::Scalar::Int(const_int)) = const_val
                else {
                    // Also try Indirect for struct-wrapped scalars.
                    if let mir::ConstValue::Indirect { alloc_id, offset } = const_val {
                        if let Ok(layout) = tcx.layout_of(
                            typing_env
                                .as_query_input(tcx.type_of(item.def_id).instantiate_identity()),
                        ) {
                            let rustc_middle::mir::interpret::GlobalAlloc::Memory(alloc) =
                                tcx.global_alloc(alloc_id)
                            else {
                                continue;
                            };
                            let size = layout.size;
                            if size.bytes() > 0 {
                                let range = rustc_middle::mir::interpret::alloc_range(offset, size);
                                if let Ok(scalar) = alloc.inner().read_scalar(&tcx, range, false) {
                                    if let Ok(int) = scalar.try_to_scalar_int() {
                                        let bits = int.to_bits(int.size());
                                        let path = tcx.def_path_str(item.def_id);
                                        const_map.entry(bits).or_default().push(path);
                                    }
                                }
                            }
                        }
                    }
                    continue;
                };
                let bits = const_int.to_bits(const_int.size());
                let path = tcx.def_path_str(item.def_id);
                const_map.entry(bits).or_default().push(path);
            }
        }

        // For each value we saw in the body, find unambiguous matches.
        for bits in values {
            if let Some(paths) = const_map.get(bits) {
                if paths.len() == 1 {
                    result.push(SymbolicConstMapping {
                        ty_path: ty_path.clone(),
                        value: *bits,
                        path: paths[0].clone(),
                    });
                }
            }
        }
    }

    result
}

/// Visit all operands in a basic block (both statement and terminator operands).
fn visit_operands_in_block<'tcx, F>(bb: &mir::BasicBlockData<'tcx>, mut visit: F)
where
    F: FnMut(&mir::Operand<'tcx>),
{
    for stmt in &bb.statements {
        if let mir::StatementKind::Assign(box (_, ref rvalue)) = stmt.kind {
            match rvalue {
                mir::Rvalue::Use(op)
                | mir::Rvalue::UnaryOp(_, op)
                | mir::Rvalue::Cast(_, op, _) => visit(op),
                mir::Rvalue::BinaryOp(_, box (lhs, rhs)) => {
                    visit(lhs);
                    visit(rhs);
                }
                mir::Rvalue::Aggregate(_, ops) => {
                    for op in ops.iter() {
                        visit(op);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(ref term) = bb.terminator {
        match &term.kind {
            mir::TerminatorKind::Call { func, args, .. } => {
                visit(func);
                for arg in args {
                    visit(&arg.node);
                }
            }
            mir::TerminatorKind::SwitchInt { discr, .. } => {
                visit(discr);
            }
            mir::TerminatorKind::Assert { cond, .. } => {
                visit(cond);
            }
            _ => {}
        }
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
    match const_op.const_ {
        mir::Const::Val(val, _) => MirConstant {
            ty,
            value: convert_const_value(&val),
        },
        mir::Const::Ty(_, _) => MirConstant {
            ty,
            value: MirConstValue::Unsupported {
                path: None,
                description: "ty_const".to_string(),
            },
        },
        mir::Const::Unevaluated(uneval, eval_ty) => {
            let path = tcx.def_path_str(uneval.def);
            let resolved = try_resolve_const_fields(tcx, const_op, &uneval, eval_ty);
            // Always emit Unevaluated to preserve the constant's def-path
            // identity. When field resolution succeeds, encode the resolved
            // scalar fields in the path using bracket notation so downstream
            // consumers can extract them without losing the constant's name.
            let annotated_path = if resolved.is_empty() {
                path.clone()
            } else {
                let fields: Vec<String> = resolved
                    .iter()
                    .map(|f| {
                        let val = match f.value.as_ref() {
                            MirConstValue::Bits { value } => format!("{:#x}", value),
                            _ => "?".to_string(),
                        };
                        format!("{}={}", f.index, val)
                    })
                    .collect();
                format!("{}[{}]", path, fields.join(","))
            };
            MirConstant {
                ty,
                value: MirConstValue::Unevaluated {
                    path: annotated_path,
                },
            }
        }
    }
}

fn convert_const_value(val: &mir::ConstValue) -> MirConstValue {
    match val {
        mir::ConstValue::Scalar(scalar) => match scalar {
            mir::interpret::Scalar::Int(int) => MirConstValue::Bits {
                value: int.to_bits(int.size()),
            },
            mir::interpret::Scalar::Ptr(_, _) => MirConstValue::Unsupported {
                path: None,
                description: "scalar_ptr".to_string(),
            },
        },
        mir::ConstValue::ZeroSized => MirConstValue::ZeroSized,
        other => MirConstValue::Unsupported {
            path: None,
            description: format!("{other:?}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Unevaluated constant field resolution
// ---------------------------------------------------------------------------

/// Attempt to evaluate an unevaluated constant and extract its scalar struct
/// fields as structured `ConstField` values. Returns an empty vec if
/// evaluation fails or the constant is not a struct.
fn try_resolve_const_fields<'tcx>(
    tcx: TyCtxt<'tcx>,
    const_op: &mir::ConstOperand<'tcx>,
    uneval: &mir::UnevaluatedConst<'tcx>,
    eval_ty: ty::Ty<'tcx>,
) -> Vec<ConstField> {
    // Only attempt resolution for ADT (struct) types.
    let ty::TyKind::Adt(adt_def, args) = eval_ty.kind() else {
        return vec![];
    };

    let typing_env = ty::TypingEnv::fully_monomorphized();

    // Evaluate the constant.
    let Ok(val) = tcx.const_eval_resolve(typing_env, *uneval, const_op.span) else {
        return vec![];
    };

    // We need an indirect (allocation-backed) result for struct types.
    let mir::ConstValue::Indirect { alloc_id, offset } = val else {
        return vec![];
    };

    // Get the struct layout so we can read fields at correct offsets.
    let Ok(struct_layout) = tcx.layout_of(typing_env.as_query_input(eval_ty)) else {
        return vec![];
    };

    // Retrieve the backing allocation.
    let rustc_middle::mir::interpret::GlobalAlloc::Memory(const_alloc) = tcx.global_alloc(alloc_id)
    else {
        return vec![];
    };

    let mut fields = Vec::new();
    let variant = adt_def.non_enum_variant();

    for (i, field_def) in variant.fields.iter().enumerate() {
        let field_ty = field_def.ty(tcx, args);
        let Ok(field_layout) = tcx.layout_of(typing_env.as_query_input(field_ty)) else {
            continue;
        };
        let field_size = field_layout.size;

        // Skip ZST fields (e.g., PhantomData).
        if field_size.bytes() == 0 {
            continue;
        }

        let field_offset = struct_layout.layout.fields().offset(i);
        let read_offset = offset + field_offset;
        let range = rustc_middle::mir::interpret::alloc_range(read_offset, field_size);
        let Ok(scalar) = const_alloc
            .inner()
            .read_scalar(&tcx, range, /* read_provenance */ false)
        else {
            continue;
        };
        let Ok(int) = scalar.try_to_scalar_int() else {
            continue;
        };

        fields.push(ConstField {
            index: i as u32,
            name: Some(field_def.name.to_string()),
            value: Box::new(MirConstValue::Bits {
                value: int.to_bits(field_size),
            }),
        });
    }

    fields
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
