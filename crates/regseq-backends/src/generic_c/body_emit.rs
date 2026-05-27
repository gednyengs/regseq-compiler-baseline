//! The section 7 body-to-C lowering (generic-c-backend.md sections 4 / 5 /
//! 6 / 7 / 7.1 / 10 / 11 / 12 / 16) for the **scalar / pointer /
//! control-flow** subset (Phase-1 task d1).
//!
//! [`emit_body`] builds the `{ ... }` block of a function definition: the
//! section 7 envelope (return-place + inner-local decls, `bb_N:` labels in
//! RPO storage order), the section 7.1 terminators, and the value layer
//! (`place_lvalue` / `operand_expr` / `rvalue_expr` / `cast_expr` /
//! `const_expr`). The signature is spelled by the caller ([`super`] banks
//! 4 / 5); only the body block lives here.
//!
//! Structured data -- `Field` / `Index` / `Downcast` projections,
//! `Aggregate` / `Repeat` / `Discriminant` rvalues, `SetDiscriminant`, the
//! aggregate / `Bytes` / `EnumValue` `Const` forms, fat-pointer
//! construction -- is **deferred to task d2** and each site returns
//! [`EmitError::Unsupported`] (fail loud). Diag-log call-site interception
//! is **task e**; d1 emits the ordinary / divergent call and errors on a
//! diag-log call.

use regseq_model::{
    AssertKind, AttrData, BinOp, CallCallee, CastKind, Const, FloatKind, InstanceId, InstanceKind,
    InstanceRef, IntKind, IntrinsicKind, LocalId, NonDivergingIntrinsic, Operand, Place, PlaceElem,
    PointerCoercion, RegseqModel, Rvalue, StatementKind, SwitchTargets, TerminatorKind, Ty, TyId,
    UintKind, UnOp,
};

use super::{naming, types, EmitError, InstanceSymbols, NameTable};

/// The C identifier for a body-local `local`. The return place is always
/// `_0`; otherwise the sanitised `debug_name` when `Some`, else the
/// synthetic `_N` (`N == LocalId`). Declaration and use sites both route
/// through this function, so they always agree.
pub(super) fn local_c_name(
    model: &RegseqModel,
    body: &regseq_model::Body,
    local: LocalId,
) -> String {
    if local == LocalId::RETURN_PLACE {
        return "_0".to_string();
    }
    match body.local_decls[local].debug_name {
        Some(s) => naming::sanitize(model.strings.resolve(s)),
        None => format!("_{}", local.as_u32()),
    }
}

/// Build the section 7 body block (`{ locals; bb_N: {...} ... }`) for a
/// function `instance` with materialised `body`. The owning signature is
/// emitted by the caller; this returns the brace-delimited block.
pub(super) fn emit_body(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    instance: &regseq_model::Instance,
    body: &regseq_model::Body,
) -> Result<String, EmitError> {
    let data = instance
        .kind
        .as_function()
        .expect("emit_body called on a non-function instance");
    let output_ty = model.sigs.resolve(data.fn_sig).output;
    let emitter = BodyEmit {
        model,
        names,
        syms,
        body,
        output_ty,
    };

    let mut out = String::from(" {\n");

    // Return place `_0`: declared iff the output is non-ZST (section 7).
    if !emitter.is_zst(output_ty) {
        out.push_str(&format!(
            "    {};\n",
            types::declare(model, names, output_ty, "_0")
        ));
    }
    // Inner locals (arg_count+1..); a ZST local declares nothing. Argument
    // locals are the C formal parameters, declared by the caller.
    for local in body.inner_locals() {
        let decl = &body.local_decls[local];
        if emitter.is_zst(decl.ty) {
            continue;
        }
        let name = local_c_name(model, body, local);
        out.push_str(&format!(
            "    {};\n",
            types::declare(model, names, decl.ty, &name)
        ));
    }

    // Basic blocks in RPO storage order.
    for (idx, bb) in body.basic_blocks.iter().enumerate() {
        out.push_str(&format!("    bb_{idx}: {{\n"));
        for stmt in bb.statements.iter() {
            if let Some(line) = emitter.statement(&stmt.kind)? {
                out.push_str(&format!("        {line}\n"));
            }
        }
        out.push_str(&format!(
            "        {}\n",
            emitter.terminator(&bb.terminator.kind)?
        ));
        out.push_str("    }\n");
    }

    out.push_str("}\n");
    Ok(out)
}

/// Integer-scalar facts used by shift masking, `*WithOverflow` helper
/// naming, and the `Cmp` / cast spellings.
struct IntInfo {
    /// Bit width.
    bits: u32,
    /// The C integer type spelling (matches the prelude helper suffix).
    ctype: &'static str,
    /// The unsigned C type of the same width (shift compute type).
    uctype: &'static str,
}

/// The per-statement / per-value lowering context: a pure read view of the
/// model plus the body being lowered (for local types / names).
struct BodyEmit<'a> {
    model: &'a RegseqModel,
    names: &'a NameTable,
    syms: &'a InstanceSymbols,
    body: &'a regseq_model::Body,
    output_ty: TyId,
}

impl BodyEmit<'_> {
    /// Whether `ty` is zero-sized (so it declares no local / is elided as a
    /// value). A bare unsized marker is treated as non-ZST to avoid
    /// `size_align_of`'s panic; such a type never reaches a scalar site.
    fn is_zst(&self, ty: TyId) -> bool {
        match &self.model.types[ty] {
            Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } => false,
            _ => self.model.size_align_of(ty).size == 0,
        }
    }

    /// The resolved type of a (d1) place: the head local's type, narrowed
    /// through `Deref` to the pointee. Field / index / downcast projections
    /// are rejected upstream by [`Self::place_lvalue`].
    fn place_ty(&self, place: &Place) -> TyId {
        let mut ty = self.body.local_decls[place.local].ty;
        for elem in place.projection.iter() {
            if let PlaceElem::Deref = elem {
                ty = match &self.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => ty,
                };
            }
        }
        ty
    }

    /// The resolved type of an operand, when statically known: the place
    /// type for a load; `None` for a constant (the const variant is read
    /// directly where a type is needed).
    fn operand_ty(&self, op: &Operand) -> Option<TyId> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => Some(self.place_ty(p)),
            Operand::Constant(_) => None,
        }
    }

    /// Whether an operand is statically a zero-sized value (so it is elided
    /// at a call-argument site).
    fn is_zst_operand(&self, op: &Operand) -> bool {
        match op {
            Operand::Copy(p) | Operand::Move(p) => self.is_zst(self.place_ty(p)),
            Operand::Constant(id) => matches!(self.model.consts.resolve(*id), Const::ZST),
        }
    }

    /// The integer facts of an operand, when it is an integer scalar.
    fn operand_int_info(&self, op: &Operand) -> Option<IntInfo> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => self.ty_int_info(self.place_ty(p)),
            Operand::Constant(id) => match self.model.consts.resolve(*id) {
                Const::Int(kind, _) => Some(self.int_int_info(*kind)),
                Const::Uint(kind, _) => Some(self.uint_int_info(*kind)),
                _ => None,
            },
        }
    }

    fn ty_int_info(&self, ty: TyId) -> Option<IntInfo> {
        match &self.model.types[ty] {
            Ty::Int(kind) => Some(self.int_int_info(*kind)),
            Ty::Uint(kind) => Some(self.uint_int_info(*kind)),
            _ => None,
        }
    }

    /// The float kind of an operand, when it is a float scalar (a place of
    /// float type or a float constant).
    fn operand_float_kind(&self, op: &Operand) -> Option<FloatKind> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => match &self.model.types[self.place_ty(p)] {
                Ty::Float(kind) => Some(*kind),
                _ => None,
            },
            Operand::Constant(id) => match self.model.consts.resolve(*id) {
                Const::Float(kind, _) => Some(*kind),
                _ => None,
            },
        }
    }

    fn ptr_bits(&self) -> u32 {
        u32::from(self.model.target.pointer_width)
    }

    fn int_int_info(&self, kind: IntKind) -> IntInfo {
        match kind {
            IntKind::I8 => IntInfo {
                bits: 8,
                ctype: "int8_t",
                uctype: "uint8_t",
            },
            IntKind::I16 => IntInfo {
                bits: 16,
                ctype: "int16_t",
                uctype: "uint16_t",
            },
            IntKind::I32 => IntInfo {
                bits: 32,
                ctype: "int32_t",
                uctype: "uint32_t",
            },
            IntKind::I64 => IntInfo {
                bits: 64,
                ctype: "int64_t",
                uctype: "uint64_t",
            },
            IntKind::I128 => IntInfo {
                bits: 128,
                ctype: "__int128",
                uctype: "__uint128_t",
            },
            IntKind::Isize => IntInfo {
                bits: self.ptr_bits(),
                ctype: "intptr_t",
                uctype: "uintptr_t",
            },
        }
    }

    fn uint_int_info(&self, kind: UintKind) -> IntInfo {
        match kind {
            UintKind::U8 => IntInfo {
                bits: 8,
                ctype: "uint8_t",
                uctype: "uint8_t",
            },
            UintKind::U16 => IntInfo {
                bits: 16,
                ctype: "uint16_t",
                uctype: "uint16_t",
            },
            UintKind::U32 => IntInfo {
                bits: 32,
                ctype: "uint32_t",
                uctype: "uint32_t",
            },
            UintKind::U64 => IntInfo {
                bits: 64,
                ctype: "uint64_t",
                uctype: "uint64_t",
            },
            UintKind::U128 => IntInfo {
                bits: 128,
                ctype: "__uint128_t",
                uctype: "__uint128_t",
            },
            UintKind::Usize => IntInfo {
                bits: self.ptr_bits(),
                ctype: "size_t",
                uctype: "size_t",
            },
        }
    }

    // -- section 7 statement dispatch ---------------------------------------

    /// Lower one statement. `None` means the statement is elided (a ZST
    /// assignment, whose destination local was never declared).
    fn statement(&self, kind: &StatementKind) -> Result<Option<String>, EmitError> {
        match kind {
            StatementKind::Assign(place, rvalue) => self.assign(place, rvalue),
            StatementKind::SetDiscriminant(_, _) => {
                Err(EmitError::Unsupported("SetDiscriminant (task d2)"))
            }
            StatementKind::Intrinsic(intr) => Ok(Some(self.intrinsic(intr)?)),
        }
    }

    /// `Assign(place, rvalue)` -> `<place> = <rvalue>;`. A ZST destination
    /// elides the statement (the local has no C declaration); `Transmute`
    /// lowers to a `memcpy` rather than an assignment (section 11).
    fn assign(&self, place: &Place, rvalue: &Rvalue) -> Result<Option<String>, EmitError> {
        // Validate the destination place first (rejects d2 projections).
        let dst = self.place_lvalue(place)?;
        let dst_ty = self.place_ty(place);
        let zst = self.is_zst(dst_ty);

        if let Rvalue::Cast(CastKind::Transmute, op, _) = rvalue {
            // Bit reinterpret via memcpy between equal-sizeof objects. Validate
            // the source operand first, then -- as for any rvalue -- elide the
            // write for a ZST destination (a 0-byte transmute is a no-op, and
            // the ZST destination local is never declared so it cannot be
            // addressed).
            let src_addr = self.operand_addr(op)?;
            if zst {
                return Ok(None);
            }
            return Ok(Some(format!(
                "memcpy(&({dst}), {src_addr}, sizeof({dst}));"
            )));
        }

        // Lower the rvalue first so a deferred construct (e.g. an `Aggregate`
        // / `Repeat` rvalue) fails loud even when the destination is ZST,
        // rather than being silently dropped. Only after the rvalue is a
        // recognized d1 form do we elide the assignment for a ZST destination
        // (the local is never declared; a ZST write is a no-op).
        let value = self.rvalue_expr(rvalue, dst_ty)?;
        if zst {
            return Ok(None);
        }
        Ok(Some(format!("{dst} = {value};")))
    }

    /// `Intrinsic(..)` (section 6): the portable `Assume` form and the
    /// named-field `CopyNonOverlapping` -> `memcpy`.
    fn intrinsic(&self, intr: &NonDivergingIntrinsic) -> Result<String, EmitError> {
        match intr {
            NonDivergingIntrinsic::Assume(op) => {
                let e = self.operand_expr(op, None)?;
                Ok(format!("if (!({e})) __builtin_unreachable();"))
            }
            NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                // C `memcpy(dst, src, count * sizeof *dst)`: read the named
                // fields so the argument order is correct without relying on
                // rustc's `(src, dst, count)` positional order.
                let dst = self.operand_expr(&copy.dst, None)?;
                let src = self.operand_expr(&copy.src, None)?;
                let count = self.operand_expr(&copy.count, None)?;
                Ok(format!(
                    "memcpy({dst}, {src}, ({count}) * sizeof(*({dst})));"
                ))
            }
        }
    }

    // -- section 4 places ----------------------------------------------------

    /// The C lvalue of a place: the bare local, or `(*p)` for `Deref`.
    /// Field / index / downcast projections are deferred to d2.
    fn place_lvalue(&self, place: &Place) -> Result<String, EmitError> {
        let mut lv = local_c_name(self.model, self.body, place.local);
        for elem in place.projection.iter() {
            match elem {
                PlaceElem::Deref => lv = format!("(*{lv})"),
                PlaceElem::Field(_, _) => {
                    return Err(EmitError::Unsupported("Field place projection (task d2)"))
                }
                PlaceElem::Index(_) => {
                    return Err(EmitError::Unsupported("Index place projection (task d2)"))
                }
                PlaceElem::ConstantIndex { .. } => {
                    return Err(EmitError::Unsupported(
                        "ConstantIndex place projection (task d2)",
                    ))
                }
                PlaceElem::Subslice { .. } => {
                    return Err(EmitError::Unsupported(
                        "Subslice place projection (task d2)",
                    ))
                }
                PlaceElem::Downcast(_) => {
                    return Err(EmitError::Unsupported(
                        "Downcast place projection (task d2)",
                    ))
                }
            }
        }
        Ok(lv)
    }

    /// The address-of expression for a Transmute source operand: `&(lvalue)`
    /// for a place, or `&(T){const}` (a gnu11 compound literal) for a
    /// constant.
    fn operand_addr(&self, op: &Operand) -> Result<String, EmitError> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => Ok(format!("&({})", self.place_lvalue(p)?)),
            Operand::Constant(id) => {
                let c = self.model.consts.resolve(*id);
                let value = const_expr(self.model, self.names, self.syms, c, None)?;
                let cty = const_c_type(self.model, c);
                Ok(format!("&({cty}){{{value}}}"))
            }
        }
    }

    // -- section 5 operands --------------------------------------------------

    /// The C value-expression of an operand (section 5). `consumer_ty` is
    /// the destination type, consulted only for the const forms that need it
    /// (`Reloc` to a static).
    fn operand_expr(&self, op: &Operand, consumer_ty: Option<TyId>) -> Result<String, EmitError> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => self.place_lvalue(p),
            Operand::Constant(id) => const_expr(
                self.model,
                self.names,
                self.syms,
                self.model.consts.resolve(*id),
                consumer_ty,
            ),
        }
    }

    // -- section 10 rvalues --------------------------------------------------

    /// The C expression of an rvalue (section 10) for the d1 subset.
    /// `dest_ty` is the assignment destination type (the operand consumer).
    fn rvalue_expr(&self, rvalue: &Rvalue, dest_ty: TyId) -> Result<String, EmitError> {
        match rvalue {
            Rvalue::Use(op) => self.operand_expr(op, Some(dest_ty)),
            Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) => {
                // Building a reference / raw pointer to a sized place is
                // `&place`; an unsized pointee builds a fat pointer (d2).
                if self.is_unsized(self.place_ty(place)) {
                    return Err(EmitError::Unsupported(
                        "Ref / RawPtr to an unsized pointee (task d2)",
                    ));
                }
                Ok(format!("&({})", self.place_lvalue(place)?))
            }
            Rvalue::Cast(kind, op, ty) => self.cast_expr(kind, op, *ty),
            Rvalue::BinaryOp(op, a, b) => self.binary_op(op, a, b),
            Rvalue::UnaryOp(op, a) => self.unary_op(op, a),
            Rvalue::Repeat(_, _) => Err(EmitError::Unsupported("Rvalue::Repeat (task d2)")),
            Rvalue::Discriminant(_) => {
                Err(EmitError::Unsupported("Rvalue::Discriminant (task d2)"))
            }
            Rvalue::Aggregate(_, _) => Err(EmitError::Unsupported("Rvalue::Aggregate (task d2)")),
        }
    }

    /// Whether a type is a bare unsized marker (`str` / `[T]` / `dyn`).
    fn is_unsized(&self, ty: TyId) -> bool {
        matches!(
            &self.model.types[ty],
            Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. }
        )
    }

    /// `UnaryOp` (section 10): `Not` is `!` for `bool`, `~` for integers;
    /// `Neg` is `-`; `PtrMetadata` is d2.
    fn unary_op(&self, op: &UnOp, a: &Operand) -> Result<String, EmitError> {
        let e = self.operand_expr(a, None)?;
        match op {
            UnOp::Not => {
                let is_bool = match a {
                    Operand::Copy(p) | Operand::Move(p) => {
                        matches!(&self.model.types[self.place_ty(p)], Ty::Bool)
                    }
                    Operand::Constant(id) => {
                        matches!(self.model.consts.resolve(*id), Const::Bool(_))
                    }
                };
                Ok(if is_bool {
                    format!("(!({e}))")
                } else {
                    format!("(~({e}))")
                })
            }
            UnOp::Neg => Ok(format!("(-({e}))")),
            UnOp::PtrMetadata => Err(EmitError::Unsupported("UnaryOp::PtrMetadata (task d2)")),
        }
    }

    /// `BinaryOp` (section 10.1). `-fwrapv` covers signed `+ - *`; shifts
    /// mask the count explicitly (`-fwrapv` does not cover shift UB).
    fn binary_op(&self, op: &BinOp, a: &Operand, b: &Operand) -> Result<String, EmitError> {
        let x = self.operand_expr(a, None)?;
        let y = self.operand_expr(b, None)?;
        let simple = |sym: &str| format!("(({x}) {sym} ({y}))");
        match op {
            BinOp::Add | BinOp::AddUnchecked => Ok(simple("+")),
            BinOp::Sub | BinOp::SubUnchecked => Ok(simple("-")),
            BinOp::Mul | BinOp::MulUnchecked => Ok(simple("*")),
            BinOp::Div => Ok(simple("/")),
            BinOp::Rem => Ok(simple("%")),
            BinOp::BitXor => Ok(simple("^")),
            BinOp::BitAnd => Ok(simple("&")),
            BinOp::BitOr => Ok(simple("|")),
            BinOp::Eq => Ok(simple("==")),
            BinOp::Ne => Ok(simple("!=")),
            BinOp::Lt => Ok(simple("<")),
            BinOp::Le => Ok(simple("<=")),
            BinOp::Ge => Ok(simple(">=")),
            BinOp::Gt => Ok(simple(">")),
            BinOp::Offset => Ok(simple("+")),
            BinOp::Cmp => Ok(format!("((int8_t)((({x}) > ({y})) - (({x}) < ({y}))))")),
            BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow => {
                // The prelude `regseq_{add,sub,mul}_<T>` helpers return a
                // `struct regseq_overflow_<T>`, but the rvalue's destination
                // is the Rust tuple `(T, bool)`, whose C struct type is
                // distinct and not assignment-compatible -- and consuming the
                // result requires `Field` projections (`.0` / `.1`). Both the
                // result-aggregate construction and its field access are d2,
                // so the `*WithOverflow` rvalue fails loud here rather than
                // mis-emitting an incompatible assignment.
                Err(EmitError::Unsupported(
                    "*WithOverflow result aggregate (T, bool) (task d2)",
                ))
            }
            BinOp::Shl | BinOp::ShlUnchecked => {
                let info = self.operand_int_info(a).ok_or(EmitError::Unsupported(
                    "shift on a non-integer operand (task d2)",
                ))?;
                let count = if matches!(op, BinOp::Shl) {
                    format!("(({y}) & {})", info.bits - 1)
                } else {
                    format!("({y})")
                };
                Ok(format!(
                    "(({})(({})({x}) << {count}))",
                    info.ctype, info.uctype
                ))
            }
            BinOp::Shr | BinOp::ShrUnchecked => {
                let info = self.operand_int_info(a).ok_or(EmitError::Unsupported(
                    "shift on a non-integer operand (task d2)",
                ))?;
                let count = if matches!(op, BinOp::Shr) {
                    format!("(({y}) & {})", info.bits - 1)
                } else {
                    format!("({y})")
                };
                // Signed `a` -> arithmetic, unsigned `a` -> logical; the C
                // `>>` on the operand's own type already selects this.
                Ok(format!("(({x}) >> {count})"))
            }
        }
    }

    // -- section 11 casts ----------------------------------------------------

    /// `Cast` (section 11) as a C expression. `Transmute` is handled at the
    /// assignment site (it is a `memcpy`, not an expression).
    fn cast_expr(&self, kind: &CastKind, op: &Operand, dest_ty: TyId) -> Result<String, EmitError> {
        let e = self.operand_expr(op, None)?;
        let dest = types::c_type(self.model, self.names, dest_ty);
        match kind {
            CastKind::IntToInt | CastKind::FloatToFloat | CastKind::IntToFloat => {
                Ok(format!("(({dest})({e}))"))
            }
            CastKind::FloatToInt => {
                // Saturating prelude helper, never a bare cast (C is UB
                // out-of-range; Rust `as` saturates, NaN -> 0). The source
                // float kind comes from a place operand's type or a float
                // constant operand.
                let prefix = match self.operand_float_kind(op) {
                    Some(FloatKind::F16) => "f16",
                    Some(FloatKind::F32) => "f32",
                    Some(FloatKind::F64) => "f64",
                    Some(FloatKind::F128) => "f128",
                    None => {
                        return Err(EmitError::Unsupported(
                            "FloatToInt from a non-float operand (task d2)",
                        ))
                    }
                };
                Ok(format!("regseq_{prefix}_to_{dest}({e})"))
            }
            CastKind::PtrToPtr | CastKind::FnPtrToPtr | CastKind::PointerWithExposedProvenance => {
                Ok(format!("(({dest})({e}))"))
            }
            CastKind::PointerExposeProvenance => Ok(format!("((uintptr_t)({e}))")),
            CastKind::Subtype => Ok(e),
            CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer) => Ok(e),
            CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer) => self.reify_fn_pointer(op),
            CastKind::PointerCoercion(PointerCoercion::Unsize) => {
                Err(EmitError::Unsupported("PointerCoercion::Unsize (task d2)"))
            }
            CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer) => Err(
                EmitError::Unsupported("PointerCoercion::ClosureFnPointer (task d2)"),
            ),
            CastKind::Transmute => {
                // Reached only if a Transmute appears outside the Assign
                // special case; bridged MIR always has it as a top-level
                // Assign rvalue, so this is a defensive guard.
                Err(EmitError::Unsupported(
                    "Transmute outside an assignment (task d2)",
                ))
            }
        }
    }

    /// `ReifyFnPointer` (section 11): the function designator `&<fn>`. The
    /// function is recovered from a `Reloc` constant operand or a place of
    /// `FnDef` type.
    fn reify_fn_pointer(&self, op: &Operand) -> Result<String, EmitError> {
        if let Operand::Constant(id) = op {
            if let Const::Reloc(target) = self.model.consts.resolve(*id) {
                return Ok(reloc_designator(
                    self.model, self.names, self.syms, *target, None,
                ));
            }
        }
        if let Some(ty) = self.operand_ty(op) {
            if let Ty::FnDef(item, args) = &self.model.types[ty] {
                for idx in 0..self.model.instances.len() {
                    let id = InstanceId::from_u32(idx as u32);
                    let inst = &self.model.instances[id];
                    if matches!(inst.kind, InstanceKind::Function(_))
                        && inst.item == *item
                        && &inst.generic_args == args
                    {
                        let name = self.syms.symbol(id);
                        return Ok(format!("(&{name})"));
                    }
                }
            }
        }
        Err(EmitError::Unsupported(
            "ReifyFnPointer of an unresolved function (task d2)",
        ))
    }

    // -- section 7.1 terminators --------------------------------------------

    /// Lower one terminator (section 7.1).
    fn terminator(&self, kind: &TerminatorKind) -> Result<String, EmitError> {
        match kind {
            TerminatorKind::Goto(t) => Ok(format!("goto bb_{};", t.as_u32())),
            TerminatorKind::Return => {
                if self.is_zst(self.output_ty) {
                    Ok("return;".to_string())
                } else {
                    Ok("return _0;".to_string())
                }
            }
            TerminatorKind::Unreachable => Ok("regseq_unreachable();".to_string()),
            TerminatorKind::SwitchInt { discr, targets } => self.switch_int(discr, targets),
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target,
            } => self.call(callee, args, destination, *target),
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
            } => self.assert(cond, *expected, msg, target.as_u32()),
        }
    }

    /// `SwitchInt` (section 7.1): a C `switch` for a <=64-bit discriminant,
    /// an if-else chain for a 128-bit discriminant.
    fn switch_int(&self, discr: &Operand, targets: &SwitchTargets) -> Result<String, EmitError> {
        let d = self.operand_expr(discr, None)?;
        let wide = self
            .operand_int_info(discr)
            .map(|i| i.bits > 64)
            .unwrap_or(false);
        let otherwise = targets.otherwise.as_u32();
        if wide {
            // The case values are `u128`; compare the discriminant's bit
            // pattern as `__uint128_t` against each case rendered from its
            // 64-bit halves (no C 128-bit integer literal exists, and a
            // u128 case can exceed `i128::MAX`).
            let mut out = String::new();
            for (v, t) in targets.cases.iter() {
                out.push_str(&format!(
                    "if ((__uint128_t)({d}) == {}) goto bb_{}; else ",
                    render_u128_literal(*v),
                    t.as_u32()
                ));
            }
            out.push_str(&format!("goto bb_{otherwise};"));
            Ok(out)
        } else {
            let mut out = format!("switch ({d}) {{ ");
            for (v, t) in targets.cases.iter() {
                out.push_str(&format!("case {v}ULL: goto bb_{}; ", t.as_u32()));
            }
            out.push_str(&format!("default: goto bb_{otherwise}; }}"));
            Ok(out)
        }
    }

    /// `Call` (section 7.1): the ordinary / divergent call. Diag-log
    /// interception is task e; a diag-log callee errors here.
    fn call(
        &self,
        callee: &CallCallee,
        args: &[Operand],
        destination: &Place,
        target: Option<regseq_model::BlockId>,
    ) -> Result<String, EmitError> {
        let callee_expr = match callee {
            CallCallee::Direct(inst) => {
                let instance = &self.model.instances[*inst];
                if self.is_diag_log(instance.item) {
                    return Err(EmitError::Unsupported("diag-log call (task e)"));
                }
                self.syms.symbol(*inst).to_string()
            }
            CallCallee::Indirect(op) => format!("({})", self.operand_expr(op, None)?),
        };

        let mut arg_strs = Vec::new();
        for arg in args.iter() {
            // Lower (validate) every argument first so a deferred place
            // projection reached through an argument fails loud, then drop the
            // ZST arguments from the C call (a ZST value carries no bytes and
            // is omitted from the prototype's parameter list, section 15).
            let expr = self.operand_expr(arg, None)?;
            if self.is_zst_operand(arg) {
                continue;
            }
            arg_strs.push(expr);
        }
        let call = format!("{callee_expr}({})", arg_strs.join(", "));

        match target {
            None => Ok(format!("{call}; __builtin_unreachable();")),
            Some(t) => {
                let dst = self.place_lvalue(destination)?;
                let assign = if self.is_zst(self.place_ty(destination)) {
                    format!("{call};")
                } else {
                    format!("{dst} = {call};")
                };
                Ok(format!("{assign} goto bb_{};", t.as_u32()))
            }
        }
    }

    /// Whether the instance's source item carries a diag-log recognition tag
    /// (section 15 diag-log exclusion / interception).
    fn is_diag_log(&self, item: regseq_model::ItemId) -> bool {
        let attrs = &self.model.domain_attrs.item_attrs;
        if item.index() >= attrs.len() {
            return false;
        }
        attrs[item]
            .iter()
            .any(|a| matches!(a, AttrData::Intrinsic(IntrinsicKind::Log(_))))
    }

    /// `Assert` (section 7.1 + section 16): branch on success, panic on
    /// failure with the `AssertKind`'s message (operands interpolated by
    /// their `Ty`).
    fn assert(
        &self,
        cond: &Operand,
        expected: bool,
        msg: &AssertKind,
        target: u32,
    ) -> Result<String, EmitError> {
        let c = self.operand_expr(cond, None)?;
        let want = if expected { "1" } else { "0" };
        let panic = self.assert_panic(msg)?;
        Ok(format!(
            "if (({c}) == {want}) goto bb_{target}; else {{ {panic} }}"
        ))
    }

    /// The section 16 panic block for an `AssertKind`: a fixed-message kind
    /// calls `regseq_panic(REGSEQ_MSG_*)`; an operand-bearing kind formats
    /// the operand(s) into the template's `%s` slots first.
    fn assert_panic(&self, msg: &AssertKind) -> Result<String, EmitError> {
        match msg {
            AssertKind::OverflowNeg(_) => Ok("regseq_panic(REGSEQ_MSG_OVERFLOW_NEG);".to_string()),
            AssertKind::DivisionByZero(_) => {
                Ok("regseq_panic(REGSEQ_MSG_DIVISION_BY_ZERO);".to_string())
            }
            AssertKind::RemainderByZero(_) => {
                Ok("regseq_panic(REGSEQ_MSG_REMAINDER_BY_ZERO);".to_string())
            }
            AssertKind::NullPointerDereference => {
                Ok("regseq_panic(REGSEQ_MSG_NULL_POINTER_DEREFERENCE);".to_string())
            }
            AssertKind::Overflow(op, _, _) => {
                // Only the operator verb is in the section 16 message; the
                // operands are not interpolated.
                let verb = binop_verb(op);
                Ok(format!(
                    "{{ char _m[256]; snprintf(_m, sizeof _m, REGSEQ_MSG_OVERFLOW, \"{verb}\"); regseq_panic(_m); }}"
                ))
            }
            AssertKind::BoundsCheck { len, index } => {
                self.format_panic("REGSEQ_MSG_BOUNDS_CHECK", &[len, index])
            }
            AssertKind::MisalignedPointerDereference { required, found } => {
                self.format_panic("REGSEQ_MSG_MISALIGNED", &[required, found])
            }
            AssertKind::InvalidEnumConstruction(v) => {
                self.format_panic("REGSEQ_MSG_INVALID_ENUM_CONSTRUCTION", &[v])
            }
        }
    }

    /// Emit a `snprintf`-into-stack-buffer panic for an operand-bearing
    /// `AssertKind`: render each operand to a small buffer by its `Ty`, then
    /// the `REGSEQ_MSG_*` template, then `regseq_panic`.
    fn format_panic(&self, template: &str, operands: &[&Operand]) -> Result<String, EmitError> {
        let mut block = String::from("{ ");
        let mut buf_names = Vec::new();
        for (i, op) in operands.iter().enumerate() {
            let (spec, cast) = self.printf_spec(op);
            let e = self.operand_expr(op, None)?;
            let buf = format!("_b{i}");
            block.push_str(&format!(
                "char {buf}[40]; snprintf({buf}, sizeof {buf}, \"{spec}\", {cast}({e})); "
            ));
            buf_names.push(buf);
        }
        block.push_str(&format!(
            "char _m[256]; snprintf(_m, sizeof _m, {template}, {}); regseq_panic(_m); }}",
            buf_names.join(", ")
        ));
        Ok(block)
    }

    /// The `printf` conversion specifier + value cast for an operand
    /// rendered by its `Ty`. Defaults to a signed wide form for operands of
    /// unknown type (e.g. a pointer-as-usize).
    fn printf_spec(&self, op: &Operand) -> (&'static str, &'static str) {
        match self.operand_ty(op).map(|t| &self.model.types[t]) {
            Some(Ty::Bool) | Some(Ty::Int(IntKind::I8 | IntKind::I16 | IntKind::I32)) => {
                ("%d", "(int)")
            }
            Some(Ty::Char) | Some(Ty::Uint(UintKind::U8 | UintKind::U16 | UintKind::U32)) => {
                ("%u", "(unsigned)")
            }
            Some(Ty::Int(IntKind::I64)) => ("%lld", "(long long)"),
            Some(Ty::Uint(UintKind::U64)) => ("%llu", "(unsigned long long)"),
            Some(Ty::Int(IntKind::Isize)) => ("%lld", "(long long)"),
            Some(Ty::Uint(UintKind::Usize)) => ("%zu", "(size_t)"),
            _ => ("%lld", "(long long)"),
        }
    }
}

/// `Const` -> a C static / `const` initializer (section 12), the d1 subset.
/// Reached from bank 5 for `InstanceKind::Static.init`; `consumer_ty` is the
/// static's declared type (the pointer type for a `Reloc`). Shares the value
/// lowering with operand-site constants.
pub(super) fn static_initializer(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    c: &Const,
    consumer_ty: TyId,
) -> Result<String, EmitError> {
    const_expr(model, names, syms, c, Some(consumer_ty))
}

/// `Const` -> C value expression / initializer (section 12), for the d1
/// subset. `consumer_ty` supplies the pointer type for a `Reloc` to a static
/// and the exact pointer spelling for a `NullPtr`. Aggregate / `Bytes` /
/// `EnumValue` forms are deferred to d2.
fn const_expr(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    c: &Const,
    consumer_ty: Option<TyId>,
) -> Result<String, EmitError> {
    match c {
        Const::Bool(b) => Ok(if *b { "1".to_string() } else { "0".to_string() }),
        Const::Char(u) => Ok(format!("(uint32_t){u}U")),
        Const::Int(kind, v) => Ok(render_int(*kind, *v)),
        Const::Uint(kind, v) => Ok(render_uint(*kind, *v)),
        Const::Float(kind, bits) => Ok(render_float(*kind, *bits)),
        Const::ZST => Ok(String::new()),
        // `NullPtr(pointee)` carries the *pointee* type (the consumer Ty
        // supplies mutability / borrow-flavor; the payload is for canonical
        // equality only -- pretty.rs render_const_null_ptr). Spell the null
        // *pointer*: the consumer's exact pointer type when known, else a
        // bare `(pointee *)0`.
        Const::NullPtr(pointee) => match consumer_ty {
            Some(t) if is_pointer(model, t) => {
                Ok(format!("(({})0)", types::c_type(model, names, t)))
            }
            _ => Ok(format!("(({} *)0)", types::c_type(model, names, *pointee))),
        },
        Const::Reloc(target) => Ok(reloc_designator(model, names, syms, *target, consumer_ty)),
        Const::Bytes(_) => Err(EmitError::Unsupported("Const::Bytes (task d2)")),
        Const::Aggregate(_) => Err(EmitError::Unsupported("Const::Aggregate (task d2)")),
        Const::EnumValue { .. } => Err(EmitError::Unsupported("Const::EnumValue (task d2)")),
    }
}

/// The C type spelling for a constant's compound literal / cast (used by the
/// Transmute source-address path). Only the scalar forms occur there.
fn const_c_type(_model: &RegseqModel, c: &Const) -> &'static str {
    match c {
        Const::Bool(_) => "_Bool",
        Const::Char(_) => "uint32_t",
        Const::Int(kind, _) => int_ctype(*kind),
        Const::Uint(kind, _) => uint_ctype(*kind),
        Const::Float(FloatKind::F16, _) => "_Float16",
        Const::Float(FloatKind::F32, _) => "float",
        Const::Float(FloatKind::F64, _) => "double",
        Const::Float(FloatKind::F128, _) => "_Float128",
        _ => "uint64_t",
    }
}

/// `Reloc(InstanceRef)` (section 12, boundary 6): resolve to the target
/// instance and emit its address. Admissible only to `Function` (`&<name>`)
/// and `Static` (`&<name>`, cast to the consumer pointer type); any other
/// instance kind is model corruption (mirrors `pretty.rs` section 10.1.7).
fn reloc_designator(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    target: InstanceRef,
    consumer_ty: Option<TyId>,
) -> String {
    let id = resolve_reloc(model, target).unwrap_or_else(|| {
        panic!(
            "generic-C emission: Const::Reloc does not resolve to any instance \
             (bridge-validated F4-2 invariant violated)"
        )
    });
    let inst = &model.instances[id];
    match &inst.kind {
        InstanceKind::Function(_) => format!("(&{})", syms.symbol(id)),
        InstanceKind::Static { .. } => {
            let name = syms.symbol(id);
            match consumer_ty {
                Some(t) if is_pointer(model, t) => {
                    format!("(({})&{name})", types::c_type(model, names, t))
                }
                _ => format!("(&{name})"),
            }
        }
        InstanceKind::Const { .. }
        | InstanceKind::Struct { .. }
        | InstanceKind::Enum { .. }
        | InstanceKind::Union { .. } => panic!(
            "generic-C emission: Const::Reloc resolved to a Const / Struct / Enum / Union \
             instance; only Function and Static are admissible (generic-c-backend.md section 12 \
             boundary 6 / pretty-printer.md section 10.1.7)"
        ),
    }
}

/// Whether `ty` is a pointer-flavored type (so a `Reloc`-to-static cast
/// applies).
fn is_pointer(model: &RegseqModel, ty: TyId) -> bool {
    matches!(
        &model.types[ty],
        Ty::Ref(_, _) | Ty::Ptr(_, _) | Ty::FnPtr(_)
    )
}

/// O(N) scan for the `InstanceId` whose canonical `InstanceRef` equals
/// `target`, mirroring the sibling pretty-printer's resolution.
fn resolve_reloc(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    for idx in 0..model.instances.len() {
        let id = InstanceId::from_u32(idx as u32);
        if model.instance_ref_of(id) == target {
            return Some(id);
        }
    }
    None
}

/// The section 16 operator verb for an `Overflow` `BinOp` (the `%s` slot in
/// `"attempt to <op> with overflow"`).
fn binop_verb(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add | BinOp::AddUnchecked | BinOp::AddWithOverflow => "add",
        BinOp::Sub | BinOp::SubUnchecked | BinOp::SubWithOverflow => "subtract",
        BinOp::Mul | BinOp::MulUnchecked | BinOp::MulWithOverflow => "multiply",
        BinOp::Div => "divide",
        BinOp::Rem => "calculate the remainder",
        BinOp::Shl | BinOp::ShlUnchecked => "shift left",
        BinOp::Shr | BinOp::ShrUnchecked => "shift right",
        BinOp::BitXor => "bitxor",
        BinOp::BitAnd => "bitand",
        BinOp::BitOr => "bitor",
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Ge | BinOp::Gt | BinOp::Cmp => {
            "compare"
        }
        BinOp::Offset => "offset",
    }
}

/// The C integer type spelling for a signed kind.
fn int_ctype(kind: IntKind) -> &'static str {
    match kind {
        IntKind::I8 => "int8_t",
        IntKind::I16 => "int16_t",
        IntKind::I32 => "int32_t",
        IntKind::I64 => "int64_t",
        IntKind::I128 => "__int128",
        IntKind::Isize => "intptr_t",
    }
}

/// The C integer type spelling for an unsigned kind.
fn uint_ctype(kind: UintKind) -> &'static str {
    match kind {
        UintKind::U8 => "uint8_t",
        UintKind::U16 => "uint16_t",
        UintKind::U32 => "uint32_t",
        UintKind::U64 => "uint64_t",
        UintKind::U128 => "__uint128_t",
        UintKind::Usize => "size_t",
    }
}

/// Render a 128-bit value as a C `__uint128_t` constant expression built
/// from its 64-bit halves (no C 128-bit integer literal exists, and a value
/// can exceed `u64::MAX`).
fn render_u128_literal(v: u128) -> String {
    let hi = (v >> 64) as u64;
    let lo = v as u64;
    if hi == 0 {
        format!("(__uint128_t){lo}ULL")
    } else {
        format!("(((__uint128_t){hi}ULL << 64) | (__uint128_t){lo}ULL)")
    }
}

/// Render a signed integer constant (section 12). The unsigned low-`w`-bit
/// pattern is emitted and cast to the signed C type, sidestepping the
/// `INT_MIN` literal-overflow problem (two's-complement host assumption). The
/// 128-bit value is built from halves (no C 128-bit literal exists).
fn render_int(kind: IntKind, v: u128) -> String {
    let cty = int_ctype(kind);
    match kind {
        IntKind::I8 => format!("(({cty})({}U))", v as u8),
        IntKind::I16 => format!("(({cty})({}U))", v as u16),
        IntKind::I32 => format!("(({cty})({}U))", v as u32),
        IntKind::I64 | IntKind::Isize => format!("(({cty})({}ULL))", v as u64),
        IntKind::I128 => format!("(({cty}){})", render_u128_literal(v)),
    }
}

/// Render an unsigned integer constant (section 12). The 128-bit value is
/// built from halves (no C 128-bit literal exists).
fn render_uint(kind: UintKind, v: u128) -> String {
    let cty = uint_ctype(kind);
    match kind {
        UintKind::U8 => format!("(({cty})({}U))", v as u8),
        UintKind::U16 => format!("(({cty})({}U))", v as u16),
        UintKind::U32 => format!("(({cty})({}U))", v as u32),
        UintKind::U64 | UintKind::Usize => format!("(({cty})({}ULL))", v as u64),
        UintKind::U128 => format!("(({cty}){})", render_u128_literal(v)),
    }
}

/// Render a floating-point constant (section 12) as a bit-faithful C
/// constant expression: a C99 hex-float literal for finite values,
/// `__builtin_inf*` for infinities, `__builtin_nan*` / `__builtin_nans*`
/// for NaNs (quiet / signaling, payload preserved).
fn render_float(kind: FloatKind, bits: u128) -> String {
    let (exp_bits, mant_bits, suffix) = match kind {
        FloatKind::F16 => (5u32, 10u32, "f16"),
        FloatKind::F32 => (8, 23, "f"),
        FloatKind::F64 => (11, 52, ""),
        FloatKind::F128 => (15, 112, "f128"),
    };
    let (inf, nan, nans) = match kind {
        FloatKind::F16 => (
            "__builtin_inff16()",
            "__builtin_nanf16",
            "__builtin_nansf16",
        ),
        FloatKind::F32 => ("__builtin_inff()", "__builtin_nanf", "__builtin_nansf"),
        FloatKind::F64 => ("__builtin_inf()", "__builtin_nan", "__builtin_nans"),
        FloatKind::F128 => (
            "__builtin_inff128()",
            "__builtin_nanf128",
            "__builtin_nansf128",
        ),
    };

    let total = 1 + exp_bits + mant_bits;
    let sign = (bits >> (total - 1)) & 1;
    let exp_mask: u128 = (1u128 << exp_bits) - 1;
    let exp = (bits >> mant_bits) & exp_mask;
    let mant_mask: u128 = (1u128 << mant_bits) - 1;
    let mant = bits & mant_mask;
    let neg = sign == 1;
    let bias: i64 = (1i64 << (exp_bits - 1)) - 1;

    let signed = |s: String| if neg { format!("(-{s})") } else { s };

    if exp == exp_mask {
        if mant == 0 {
            return signed(inf.to_string());
        }
        let quiet = (mant >> (mant_bits - 1)) & 1 == 1;
        let payload = mant & ((1u128 << (mant_bits - 1)) - 1);
        let builtin = if quiet { nan } else { nans };
        return signed(format!("{builtin}(\"0x{payload:x}\")"));
    }

    // Finite: build the hex-float mantissa digits (left-aligned to a hex
    // boundary, trailing zeros stripped).
    let pad = (4 - (mant_bits % 4)) % 4;
    let hex_digits = ((mant_bits + pad) / 4) as usize;
    let mant_aligned = mant << pad;
    let mut hex = format!("{mant_aligned:0hex_digits$x}");
    while hex.len() > 1 && hex.ends_with('0') {
        hex.pop();
    }
    let frac = if mant == 0 {
        String::new()
    } else {
        format!(".{hex}")
    };

    let body = if exp == 0 && mant == 0 {
        "0x0p+0".to_string()
    } else if exp == 0 {
        // Subnormal: leading digit 0, exponent 1 - bias.
        format!("0x0{frac}p{}", 1 - bias)
    } else {
        // Normal: leading digit 1, exponent exp - bias.
        format!("0x1{frac}p{}", exp as i64 - bias)
    };
    signed(format!("{body}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::{render_float, render_int, render_uint};
    use regseq_model::{FloatKind, IntKind, UintKind};

    #[test]
    fn render_int_uses_twos_complement_bit_pattern() {
        assert_eq!(render_int(IntKind::I32, 2), "((int32_t)(2U))");
        // -1 sign-extended to u128 is all ones.
        assert_eq!(
            render_int(IntKind::I32, u128::MAX),
            "((int32_t)(4294967295U))"
        );
        assert_eq!(render_int(IntKind::I8, u128::MAX), "((int8_t)(255U))");
    }

    #[test]
    fn render_uint_appends_width_suffix() {
        assert_eq!(render_uint(UintKind::U32, 5), "((uint32_t)(5U))");
        assert_eq!(render_uint(UintKind::U64, 5), "((uint64_t)(5ULL))");
    }

    #[test]
    fn render_float_finite_is_a_hex_float() {
        // 1.0, 2.0, 1.5, 0.0 (f64).
        assert_eq!(
            render_float(FloatKind::F64, 1.0f64.to_bits() as u128),
            "0x1p0"
        );
        assert_eq!(
            render_float(FloatKind::F64, 2.0f64.to_bits() as u128),
            "0x1p1"
        );
        assert_eq!(
            render_float(FloatKind::F64, 1.5f64.to_bits() as u128),
            "0x1.8p0"
        );
        assert_eq!(
            render_float(FloatKind::F64, 0.0f64.to_bits() as u128),
            "0x0p+0"
        );
        // f32 carries the `f` suffix.
        assert_eq!(
            render_float(FloatKind::F32, 1.0f32.to_bits() as u128),
            "0x1p0f"
        );
    }

    #[test]
    fn render_float_special_values_use_builtins() {
        assert_eq!(
            render_float(FloatKind::F64, f64::INFINITY.to_bits() as u128),
            "__builtin_inf()"
        );
        assert_eq!(
            render_float(FloatKind::F64, f64::NEG_INFINITY.to_bits() as u128),
            "(-__builtin_inf())"
        );
        assert!(render_float(FloatKind::F64, f64::NAN.to_bits() as u128).contains("__builtin_nan"));
    }
}
