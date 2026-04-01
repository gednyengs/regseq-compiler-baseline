//! Extract simplified MIR from the compiler.
//!
//! Converts `rustc_middle::mir::Body` into the rustc-independent [`MirFunc`]
//! representation used by the structuring and emission passes.

use std::collections::HashMap;

use rustc_middle::mir;
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LocalDefId;

use super::types::{CExpr, LocalInfo, MirBlock, MirFunc, MirStmt, MirTerminator, Unwind};

/// Collect simplified MIR for all non-synthetic function bodies in the crate.
pub fn collect_mir(tcx: TyCtxt<'_>) -> Vec<MirFunc> {
    let mut funcs = Vec::new();

    for &local_def_id in tcx.mir_keys(()) {
        // Skip closures, promoted MIR, etc. -- only top-level fn items.
        if !tcx.def_kind(local_def_id).is_fn_like() {
            continue;
        }

        let body = tcx.optimized_mir(local_def_id.to_def_id());
        let name = tcx.def_path_str(local_def_id);
        funcs.push(lower_body(&name, body, local_def_id, tcx));
    }

    funcs
}

fn lower_body(
    name: &str,
    body: &mir::Body<'_>,
    _def_id: LocalDefId,
    tcx: TyCtxt<'_>,
) -> MirFunc {
    let locals: Vec<LocalInfo> = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| {
            let idx = local.as_usize();
            LocalInfo {
                index: idx,
                ty: format!("{}", decl.ty),
                is_arg: idx >= 1 && idx <= body.arg_count,
            }
        })
        .collect();

    let blocks: Vec<MirBlock> = body
        .basic_blocks
        .iter_enumerated()
        .map(|(bb, data)| lower_block(bb, data, body, tcx))
        .collect();

    let debug_names = extract_debug_names(body);

    MirFunc {
        name: name.to_string(),
        locals,
        arg_count: body.arg_count,
        blocks,
        debug_names,
    }
}

fn lower_block(
    bb: mir::BasicBlock,
    data: &mir::BasicBlockData<'_>,
    body: &mir::Body<'_>,
    tcx: TyCtxt<'_>,
) -> MirBlock {
    let statements = data
        .statements
        .iter()
        .map(|stmt| lower_statement(stmt, tcx))
        .collect();

    let terminator = data
        .terminator
        .as_ref()
        .map(|t| lower_terminator(t, body, tcx))
        .unwrap_or(MirTerminator::Unreachable);

    MirBlock {
        index: bb.as_usize(),
        statements,
        terminator,
        is_cleanup: data.is_cleanup,
    }
}

fn lower_statement(stmt: &mir::Statement<'_>, tcx: TyCtxt<'_>) -> MirStmt {
    match &stmt.kind {
        mir::StatementKind::Assign(box (place, rvalue)) => MirStmt::Assign {
            lhs: lower_place(place, tcx),
            rhs: lower_rvalue(rvalue, tcx),
        },
        mir::StatementKind::StorageLive(local) => MirStmt::StorageLive(local.as_usize()),
        mir::StatementKind::StorageDead(local) => MirStmt::StorageDead(local.as_usize()),
        other => MirStmt::Other(format!("{other:?}")),
    }
}

fn lower_terminator(
    term: &mir::Terminator<'_>,
    _body: &mir::Body<'_>,
    tcx: TyCtxt<'_>,
) -> MirTerminator {
    match &term.kind {
        mir::TerminatorKind::Goto { target } => MirTerminator::Goto(target.as_usize()),

        mir::TerminatorKind::Return => MirTerminator::Return,

        mir::TerminatorKind::Unreachable => MirTerminator::Unreachable,

        mir::TerminatorKind::SwitchInt { discr, targets } => {
            let discr_expr = lower_operand(discr, tcx);
            let cases: Vec<(u128, usize)> = targets
                .iter()
                .map(|(val, bb)| (val, bb.as_usize()))
                .collect();
            let otherwise = targets.otherwise().as_usize();
            MirTerminator::SwitchInt {
                discr: discr_expr,
                targets: cases,
                otherwise,
            }
        }

        mir::TerminatorKind::Drop {
            place,
            target,
            unwind,
            ..
        } => MirTerminator::Drop {
            place: lower_place(place, tcx),
            target: target.as_usize(),
            unwind: lower_unwind(unwind),
        },

        mir::TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } => MirTerminator::Call {
            func: format!("{}", lower_operand(func, tcx)),
            args: args.iter().map(|a| lower_operand(&a.node, tcx)).collect(),
            destination: lower_place(destination, tcx),
            target: target.map(|bb| bb.as_usize()),
            unwind: lower_unwind(unwind),
        },

        mir::TerminatorKind::Assert {
            cond,
            expected,
            target,
            unwind,
            ..
        } => MirTerminator::Assert {
            cond: lower_operand(cond, tcx),
            expected: *expected,
            target: target.as_usize(),
            unwind: lower_unwind(unwind),
        },

        // Catch-all for terminators we don't specially handle.
        _other => MirTerminator::Goto(0), // placeholder
    }
}

fn lower_unwind(unwind: &mir::UnwindAction) -> Unwind {
    match unwind {
        mir::UnwindAction::Cleanup(bb) => Unwind::Cleanup(bb.as_usize()),
        mir::UnwindAction::Continue => Unwind::Continue,
        mir::UnwindAction::Unreachable => Unwind::Unreachable,
        mir::UnwindAction::Terminate(_) => Unwind::Terminate,
    }
}

/// Build a map from local index to source-level variable name using
/// `VarDebugInfo`.
fn extract_debug_names(body: &mir::Body<'_>) -> HashMap<usize, String> {
    let mut names = HashMap::new();
    for info in &body.var_debug_info {
        if let mir::VarDebugInfoContents::Place(place) = &info.value {
            if place.projection.is_empty() {
                names
                    .entry(place.local.as_usize())
                    .or_insert_with(|| info.name.to_string());
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Lowering helpers -- convert MIR constructs to CExpr
// ---------------------------------------------------------------------------

fn lower_place(place: &mir::Place<'_>, _tcx: TyCtxt<'_>) -> CExpr {
    let mut expr = CExpr::Local(place.local.as_usize());
    for elem in place.projection.iter() {
        match elem {
            mir::PlaceElem::Field(field, _) => {
                expr = CExpr::Field {
                    base: Box::new(expr),
                    field: format!("{}", field.as_usize()),
                };
            }
            mir::PlaceElem::Deref => {
                expr = CExpr::Deref(Box::new(expr));
            }
            mir::PlaceElem::Index(local) => {
                expr = CExpr::Call {
                    func: "index".to_string(),
                    args: vec![expr, CExpr::Local(local.as_usize())],
                };
            }
            mir::PlaceElem::Downcast(Some(sym), _) => {
                expr = CExpr::Cast {
                    operand: Box::new(expr),
                    ty: sym.to_string(),
                };
            }
            mir::PlaceElem::Downcast(None, idx) => {
                expr = CExpr::Cast {
                    operand: Box::new(expr),
                    ty: format!("variant#{}", idx.as_usize()),
                };
            }
            _ => {
                expr = CExpr::Field {
                    base: Box::new(expr),
                    field: "<?>".to_string(),
                };
            }
        }
    }
    expr
}

fn lower_operand(op: &mir::Operand<'_>, tcx: TyCtxt<'_>) -> CExpr {
    match op {
        mir::Operand::Copy(place) | mir::Operand::Move(place) => lower_place(place, tcx),
        mir::Operand::Constant(constant) => CExpr::Literal(format!("{}", constant.const_)),
        _ => CExpr::Literal(format!("{op:?}")),
    }
}

fn lower_rvalue(rvalue: &mir::Rvalue<'_>, tcx: TyCtxt<'_>) -> CExpr {
    match rvalue {
        mir::Rvalue::Use(op) => lower_operand(op, tcx),
        mir::Rvalue::Ref(_, _bk, place) => CExpr::AddrOf(Box::new(lower_place(place, tcx))),
        mir::Rvalue::RawPtr(_kind, place) => CExpr::AddrOf(Box::new(lower_place(place, tcx))),
        mir::Rvalue::BinaryOp(op, box (lhs, rhs)) => CExpr::BinOp {
            op: fmt_binop(op).to_string(),
            lhs: Box::new(lower_operand(lhs, tcx)),
            rhs: Box::new(lower_operand(rhs, tcx)),
        },
        mir::Rvalue::UnaryOp(op, operand) => {
            let sym = match op {
                mir::UnOp::Not => "!",
                mir::UnOp::Neg => "-",
                mir::UnOp::PtrMetadata => "ptr_metadata",
            };
            CExpr::UnaryOp {
                op: sym.to_string(),
                operand: Box::new(lower_operand(operand, tcx)),
            }
        }
        mir::Rvalue::Cast(_, op, ty) => CExpr::Cast {
            operand: Box::new(lower_operand(op, tcx)),
            ty: format!("{ty}"),
        },
        mir::Rvalue::Aggregate(kind, fields) => {
            let field_exprs: Vec<CExpr> =
                fields.iter().map(|f| lower_operand(f, tcx)).collect();
            let name = match kind.as_ref() {
                mir::AggregateKind::Tuple => "tuple".to_string(),
                mir::AggregateKind::Adt(def_id, variant, _, _, _) => {
                    format!("{}::{}", tcx.def_path_str(*def_id), variant.as_usize())
                }
                mir::AggregateKind::Array(_) => "array".to_string(),
                _ => "aggregate".to_string(),
            };
            CExpr::Aggregate {
                name,
                fields: field_exprs,
            }
        }
        mir::Rvalue::Discriminant(place) => CExpr::Call {
            func: "discriminant".to_string(),
            args: vec![lower_place(place, tcx)],
        },
        _ => CExpr::Literal(format!("{rvalue:?}")),
    }
}

fn fmt_binop(op: &mir::BinOp) -> &'static str {
    match op {
        mir::BinOp::Add | mir::BinOp::AddWithOverflow | mir::BinOp::AddUnchecked => "+",
        mir::BinOp::Sub | mir::BinOp::SubWithOverflow | mir::BinOp::SubUnchecked => "-",
        mir::BinOp::Mul | mir::BinOp::MulWithOverflow | mir::BinOp::MulUnchecked => "*",
        mir::BinOp::Div => "/",
        mir::BinOp::Rem => "%",
        mir::BinOp::BitXor => "^",
        mir::BinOp::BitAnd => "&",
        mir::BinOp::BitOr => "|",
        mir::BinOp::Shl | mir::BinOp::ShlUnchecked => "<<",
        mir::BinOp::Shr | mir::BinOp::ShrUnchecked => ">>",
        mir::BinOp::Eq => "==",
        mir::BinOp::Lt => "<",
        mir::BinOp::Le => "<=",
        mir::BinOp::Ne => "!=",
        mir::BinOp::Ge => ">=",
        mir::BinOp::Gt => ">",
        mir::BinOp::Cmp => "<=>",
        mir::BinOp::Offset => "offset",
    }
}
