//! Read-only body visitor for joint validation.
//!
//! The visitor surface is the shared walk hook set for the phase-1a
//! validator tasks. B-Body-1 through B-Body-10 from spec.md section
//! 25.9 are enforced by later P1a-VAL tasks through these hooks:
//! local type closure at `visit_local_decl` / `visit_ty`, callee
//! closure at `visit_terminator` / `visit_call_callee`, place
//! validation at `visit_place`, operand validation at
//! `visit_operand`, rvalue validation at `visit_rvalue`, statement
//! validation at `visit_statement`, terminator validation at
//! `visit_terminator`, and CFG ordering through `visit_body`.

use crate::basic_block::BasicBlock;
use crate::body::{Body, LocalDecl};
use crate::ids::{BlockId, TyId};
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, CastKind, PointerCoercion, Rvalue};
use crate::statement::{CopyNonOverlapping, NonDivergingIntrinsic, Statement, StatementKind};
use crate::terminator::{AssertKind, CallCallee, SwitchTargets, Terminator, TerminatorKind};

/// Body-level validation error emitted by default `visit_body`
/// pre-checks.
///
/// These correspond to the B-NonEmpty, B-Successors-Valid,
/// B-Reachable, and B-RPO checks absorbed from spec.md section
/// 25.6.2. Concrete validators can override `emit_body_error` to
/// translate these markers into their diagnostic representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyValidationError {
    /// `Body.basic_blocks` is empty.
    BNonEmpty,
    /// A terminator successor references a missing basic block.
    BSuccessorsValid,
    /// At least one basic block is unreachable from
    /// `BlockId::START_BLOCK`.
    BReachable,
    /// Basic blocks are not stored in DFS reverse-postorder.
    BRpo,
}

/// Read-only visitor over MIR body content.
///
/// The ten `visit_X` / `super_X` hook pairs match spec.md section
/// 25.6.1. Default `super_X` methods use exhaustive destructuring,
/// so adding a new IR variant forces this traversal to be updated.
pub trait Visitor {
    /// Emits a body-level validation error discovered by the default
    /// `visit_body` DFS-RPO pre-checks.
    fn emit_body_error(&mut self, _error: BodyValidationError) {}

    /// Visits a body after running the DFS-RPO pre-check procedure
    /// from spec.md section 25.6.2.
    fn visit_body(&mut self, body: &Body) {
        if body.basic_blocks.is_empty() {
            self.emit_body_error(BodyValidationError::BNonEmpty);
            return;
        }

        let block_len = body.basic_blocks.len();
        for bb in body.basic_blocks.iter() {
            for succ in bb.terminator.successors() {
                if succ.index() >= block_len {
                    self.emit_body_error(BodyValidationError::BSuccessorsValid);
                    return;
                }
            }
        }

        let rpo = dfs_rpo_from(BlockId::START_BLOCK, body);
        if rpo.len() != block_len {
            self.emit_body_error(BodyValidationError::BReachable);
        }
        for (idx, bb_id) in rpo.iter().copied().enumerate() {
            if bb_id != BlockId::from_u32(idx as u32) {
                self.emit_body_error(BodyValidationError::BRpo);
                break;
            }
        }

        self.super_body(body)
    }

    /// Recurses into local declarations and basic blocks.
    fn super_body(&mut self, body: &Body) {
        let Body {
            basic_blocks,
            local_decls,
            arg_count: _arg_count,
            span: _span,
        } = body;
        for decl in local_decls.iter() {
            self.visit_local_decl(decl);
        }
        for bb in basic_blocks.iter() {
            self.visit_basic_block(bb);
        }
    }

    /// Visits a basic block.
    fn visit_basic_block(&mut self, bb: &BasicBlock) {
        self.super_basic_block(bb)
    }

    /// Recurses into statements and the terminator.
    fn super_basic_block(&mut self, bb: &BasicBlock) {
        let BasicBlock {
            statements,
            terminator,
        } = bb;
        for stmt in statements.iter() {
            self.visit_statement(stmt);
        }
        self.visit_terminator(terminator);
    }

    /// Visits a local declaration.
    fn visit_local_decl(&mut self, decl: &LocalDecl) {
        self.super_local_decl(decl)
    }

    /// Recurses into the local type.
    fn super_local_decl(&mut self, decl: &LocalDecl) {
        let LocalDecl {
            ty,
            mutability: _mutability,
            span: _span,
            debug_name: _debug_name,
        } = decl;
        self.visit_ty(*ty);
    }

    /// Visits a statement.
    fn visit_statement(&mut self, stmt: &Statement) {
        self.super_statement(stmt)
    }

    /// Recurses into statement child constructs.
    fn super_statement(&mut self, stmt: &Statement) {
        let Statement { kind, span: _span } = stmt;
        match kind {
            StatementKind::Assign(place, rvalue) => {
                self.visit_place(place);
                self.visit_rvalue(rvalue);
            }
            StatementKind::SetDiscriminant(place, _variant) => {
                self.visit_place(place);
            }
            StatementKind::Intrinsic(intrinsic) => match intrinsic {
                NonDivergingIntrinsic::Assume(operand) => {
                    self.visit_operand(operand);
                }
                NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                    let CopyNonOverlapping { src, dst, count } = payload;
                    self.visit_operand(src);
                    self.visit_operand(dst);
                    self.visit_operand(count);
                }
            },
        }
    }

    /// Visits a terminator.
    fn visit_terminator(&mut self, term: &Terminator) {
        self.super_terminator(term)
    }

    /// Recurses into terminator child constructs.
    fn super_terminator(&mut self, term: &Terminator) {
        let Terminator { kind, span: _span } = term;
        match kind {
            TerminatorKind::Goto(_target) => {}
            TerminatorKind::SwitchInt { discr, targets } => {
                self.visit_operand(discr);
                let SwitchTargets {
                    cases: _cases,
                    otherwise: _otherwise,
                } = targets;
            }
            TerminatorKind::Return => {}
            TerminatorKind::Unreachable => {}
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target: _target,
            } => {
                self.visit_call_callee(callee);
                for arg in args.iter() {
                    self.visit_operand(arg);
                }
                self.visit_place(destination);
            }
            TerminatorKind::Assert {
                cond,
                expected: _expected,
                msg,
                target: _target,
            } => {
                self.visit_operand(cond);
                match msg {
                    AssertKind::BoundsCheck { len, index } => {
                        self.visit_operand(len);
                        self.visit_operand(index);
                    }
                    AssertKind::Overflow(_op, left, right) => {
                        self.visit_operand(left);
                        self.visit_operand(right);
                    }
                    AssertKind::OverflowNeg(operand) => {
                        self.visit_operand(operand);
                    }
                    AssertKind::DivisionByZero(operand) => {
                        self.visit_operand(operand);
                    }
                    AssertKind::RemainderByZero(operand) => {
                        self.visit_operand(operand);
                    }
                    AssertKind::MisalignedPointerDereference { required, found } => {
                        self.visit_operand(required);
                        self.visit_operand(found);
                    }
                    AssertKind::NullPointerDereference => {}
                    AssertKind::InvalidEnumConstruction(operand) => {
                        self.visit_operand(operand);
                    }
                }
            }
        }
    }

    /// Visits a place.
    fn visit_place(&mut self, place: &Place) {
        self.super_place(place)
    }

    /// Recurses into projection-carried type IDs.
    fn super_place(&mut self, place: &Place) {
        let Place {
            local: _local,
            projection,
        } = place;
        for elem in projection.iter() {
            match elem {
                PlaceElem::Deref => {}
                PlaceElem::Field(_field, ty) => {
                    self.visit_ty(*ty);
                }
                PlaceElem::Index(_local) => {}
                PlaceElem::ConstantIndex {
                    offset: _offset,
                    min_length: _min_length,
                    from_end: _from_end,
                } => {}
                PlaceElem::Subslice {
                    from: _from,
                    to: _to,
                    from_end: _from_end,
                } => {}
                PlaceElem::Downcast(_variant) => {}
            }
        }
    }

    /// Visits an rvalue.
    fn visit_rvalue(&mut self, rvalue: &Rvalue) {
        self.super_rvalue(rvalue)
    }

    /// Recurses into rvalue child constructs.
    fn super_rvalue(&mut self, rvalue: &Rvalue) {
        match rvalue {
            Rvalue::Use(operand) => {
                self.visit_operand(operand);
            }
            Rvalue::Repeat(operand, _count) => {
                self.visit_operand(operand);
            }
            Rvalue::Ref(_provenance, place) => {
                self.visit_place(place);
            }
            Rvalue::RawPtr(_mutability, place) => {
                self.visit_place(place);
            }
            Rvalue::Cast(kind, operand, ty) => {
                match kind {
                    CastKind::PointerExposeProvenance => {}
                    CastKind::PointerWithExposedProvenance => {}
                    CastKind::PointerCoercion(coercion) => match coercion {
                        PointerCoercion::ReifyFnPointer => {}
                        PointerCoercion::UnsafeFnPointer => {}
                        PointerCoercion::ClosureFnPointer => {}
                        PointerCoercion::Unsize => {}
                    },
                    CastKind::IntToInt => {}
                    CastKind::FloatToInt => {}
                    CastKind::FloatToFloat => {}
                    CastKind::IntToFloat => {}
                    CastKind::PtrToPtr => {}
                    CastKind::FnPtrToPtr => {}
                    CastKind::Transmute => {}
                    CastKind::Subtype => {}
                }
                self.visit_operand(operand);
                self.visit_ty(*ty);
            }
            Rvalue::BinaryOp(_op, left, right) => {
                self.visit_operand(left);
                self.visit_operand(right);
            }
            Rvalue::UnaryOp(_op, operand) => {
                self.visit_operand(operand);
            }
            Rvalue::Discriminant(place) => {
                self.visit_place(place);
            }
            Rvalue::Aggregate(kind, operands) => {
                match kind {
                    AggregateKind::Array(ty) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::Tuple(ty) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::Struct(ty) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::Enum(ty, _variant) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::Union(ty, _field) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::Closure(ty) => {
                        self.visit_ty(*ty);
                    }
                    AggregateKind::RawPtr(ty, _mutability) => {
                        self.visit_ty(*ty);
                    }
                }
                for operand in operands.iter() {
                    self.visit_operand(operand);
                }
            }
        }
    }

    /// Visits an operand.
    fn visit_operand(&mut self, op: &Operand) {
        self.super_operand(op)
    }

    /// Recurses into operand child constructs.
    fn super_operand(&mut self, op: &Operand) {
        match op {
            Operand::Copy(place) => {
                self.visit_place(place);
            }
            Operand::Move(place) => {
                self.visit_place(place);
            }
            Operand::Constant(_constant) => {}
        }
    }

    /// Visits a type ID.
    fn visit_ty(&mut self, ty: TyId) {
        self.super_ty(ty)
    }

    /// Type IDs have no structural children at this layer.
    fn super_ty(&mut self, _ty: TyId) {}

    /// Visits a call callee.
    fn visit_call_callee(&mut self, callee: &CallCallee) {
        self.super_call_callee(callee)
    }

    /// Recurses into indirect callee operands.
    fn super_call_callee(&mut self, callee: &CallCallee) {
        match callee {
            CallCallee::Direct(_instance) => {}
            CallCallee::Indirect(operand) => {
                self.visit_operand(operand);
            }
        }
    }
}

fn dfs_rpo_from(start: BlockId, body: &Body) -> Vec<BlockId> {
    fn dfs(current: BlockId, body: &Body, seen: &mut [bool], postorder: &mut Vec<BlockId>) {
        let idx = current.index();
        if seen[idx] {
            return;
        }
        seen[idx] = true;
        for succ in body.basic_blocks[current].terminator.successors() {
            dfs(succ, body, seen, postorder);
        }
        postorder.push(current);
    }

    let mut seen = vec![false; body.basic_blocks.len()];
    let mut postorder = Vec::new();
    dfs(start, body, &mut seen, &mut postorder);
    postorder.reverse();
    postorder
}

#[cfg(test)]
mod tests {
    use super::{BodyValidationError, Visitor};
    use crate::{
        BasicBlock, BlockId, Body, ConstId, FieldIdx, IndexVec, LocalDecl, LocalId, Mutability,
        Operand, Place, PlaceElem, Rvalue, SourceFileId, Span, Statement, StatementKind,
        Terminator, TerminatorKind, TyId,
    };

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn local_decl(ty: u32) -> LocalDecl {
        LocalDecl {
            ty: TyId::from_u32(ty),
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }
    }

    fn block(terminator: TerminatorKind, statements: Box<[Statement]>) -> BasicBlock {
        BasicBlock {
            statements,
            terminator: Terminator {
                kind: terminator,
                span: span(90, 91),
            },
        }
    }

    fn const_operand(index: u32) -> Operand {
        Operand::Constant(ConstId::from_u32(index))
    }

    fn place(local: u32) -> Place {
        Place::from(LocalId::from_u32(local))
    }

    fn valid_body(statements: Box<[Statement]>) -> Body {
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(block(TerminatorKind::Return, statements));
        let mut local_decls = IndexVec::new();
        local_decls.push(local_decl(0));
        local_decls.push(local_decl(1));
        Body {
            basic_blocks,
            local_decls,
            arg_count: 1,
            span: span(0, 1),
        }
    }

    #[derive(Default)]
    struct CountingVisitor {
        blocks: usize,
        locals: usize,
        statements: usize,
        terminators: usize,
        places: usize,
        rvalues: usize,
        operands: usize,
        tys: usize,
        callees: usize,
        errors: Vec<BodyValidationError>,
    }

    impl Visitor for CountingVisitor {
        fn emit_body_error(&mut self, error: BodyValidationError) {
            self.errors.push(error);
        }

        fn visit_basic_block(&mut self, bb: &BasicBlock) {
            self.blocks += 1;
            self.super_basic_block(bb);
        }

        fn visit_local_decl(&mut self, decl: &LocalDecl) {
            self.locals += 1;
            self.super_local_decl(decl);
        }

        fn visit_statement(&mut self, stmt: &Statement) {
            self.statements += 1;
            self.super_statement(stmt);
        }

        fn visit_terminator(&mut self, term: &Terminator) {
            self.terminators += 1;
            self.super_terminator(term);
        }

        fn visit_place(&mut self, place: &Place) {
            self.places += 1;
            self.super_place(place);
        }

        fn visit_rvalue(&mut self, rvalue: &Rvalue) {
            self.rvalues += 1;
            self.super_rvalue(rvalue);
        }

        fn visit_operand(&mut self, op: &Operand) {
            self.operands += 1;
            self.super_operand(op);
        }

        fn visit_ty(&mut self, ty: TyId) {
            self.tys += 1;
            self.super_ty(ty);
        }

        fn visit_call_callee(&mut self, callee: &crate::CallCallee) {
            self.callees += 1;
            self.super_call_callee(callee);
        }
    }

    #[test]
    fn default_visit_body_walks_blocks_locals_and_terminators() {
        let body = valid_body(Box::new([]));
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &body);

        assert_eq!(visitor.blocks, body.basic_blocks.len());
        assert_eq!(visitor.locals, body.local_decls.len());
        assert_eq!(visitor.terminators, body.basic_blocks.len());
        assert!(visitor.errors.is_empty());
    }

    #[test]
    fn visit_statement_triggers_once_per_statement() {
        let statements = Box::new([
            Statement {
                kind: StatementKind::Assign(place(0), Rvalue::Use(const_operand(1))),
                span: span(10, 11),
            },
            Statement {
                kind: StatementKind::Assign(place(1), Rvalue::Use(const_operand(2))),
                span: span(11, 12),
            },
        ]);
        let body = valid_body(statements);
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &body);

        assert_eq!(visitor.statements, 2);
    }

    #[test]
    fn visit_place_reaches_assign_lhs_and_rvalue_operand_place() {
        let body = valid_body(Box::new([Statement {
            kind: StatementKind::Assign(
                place(0),
                Rvalue::Use(Operand::Copy(Place::from(LocalId::from_u32(1)))),
            ),
            span: span(10, 11),
        }]));
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &body);

        assert!(visitor.places >= 2);
    }

    #[test]
    fn default_visit_body_reports_cfg_precheck_errors() {
        let empty = Body {
            basic_blocks: IndexVec::new(),
            local_decls: IndexVec::new(),
            arg_count: 0,
            span: Span::dummy(),
        };
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &empty);

        assert_eq!(visitor.errors, vec![BodyValidationError::BNonEmpty]);

        let mut invalid_successor = valid_body(Box::new([]));
        invalid_successor.basic_blocks[BlockId::START_BLOCK]
            .terminator
            .kind = TerminatorKind::Goto(BlockId::from_u32(9));
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &invalid_successor);

        assert_eq!(visitor.errors, vec![BodyValidationError::BSuccessorsValid]);
    }

    #[test]
    fn default_visit_body_reports_reachability_and_rpo_errors() {
        let mut unreachable = valid_body(Box::new([]));
        unreachable
            .basic_blocks
            .push(block(TerminatorKind::Return, Box::new([])));
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &unreachable);

        assert_eq!(visitor.errors, vec![BodyValidationError::BReachable]);

        let mut out_of_rpo = Body {
            basic_blocks: IndexVec::new(),
            local_decls: valid_body(Box::new([])).local_decls,
            arg_count: 1,
            span: Span::dummy(),
        };
        out_of_rpo.basic_blocks.push(block(
            TerminatorKind::Goto(BlockId::from_u32(2)),
            Box::new([]),
        ));
        out_of_rpo
            .basic_blocks
            .push(block(TerminatorKind::Return, Box::new([])));
        out_of_rpo.basic_blocks.push(block(
            TerminatorKind::Goto(BlockId::from_u32(1)),
            Box::new([]),
        ));
        let mut visitor = CountingVisitor::default();

        Visitor::visit_body(&mut visitor, &out_of_rpo);

        assert_eq!(visitor.errors, vec![BodyValidationError::BRpo]);
    }

    #[test]
    fn override_hook_replaces_place_recursion() {
        #[derive(Default)]
        struct PlaceOnly {
            places: usize,
            tys: usize,
        }

        impl Visitor for PlaceOnly {
            fn visit_place(&mut self, _place: &Place) {
                self.places += 1;
            }

            fn visit_ty(&mut self, _ty: TyId) {
                self.tys += 1;
            }
        }

        let projected = Place {
            local: LocalId::from_u32(0),
            projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(0), TyId::from_u32(7))]),
        };
        let mut visitor = PlaceOnly::default();

        visitor.visit_place(&projected);

        assert_eq!(visitor.places, 1);
        assert_eq!(visitor.tys, 0);
    }

    #[test]
    fn exhaustive_destructuring_defaults_compile() {
        // The default `super_X` implementations above intentionally
        // avoid `..` and catch-all match arms. This test documents the
        // compile-time convention: adding a variant to the walked IR
        // makes the visitor defaults fail to compile until updated.
        let body = valid_body(Box::new([]));
        let mut visitor = CountingVisitor::default();
        Visitor::visit_body(&mut visitor, &body);
        assert!(visitor.errors.is_empty());
    }
}
