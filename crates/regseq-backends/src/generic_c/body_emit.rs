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
//! Task d2 adds the non-enum structured-data layer: section 4 place
//! projections (`Field` / `Index` / `ConstantIndex` / `Subslice`) + section
//! 14 field access, the section 10 `Aggregate` / `Repeat` rvalues, the
//! `PtrMetadata` slot read + non-enum `Discriminant`, section 11 slice/str
//! fat-pointer construction (`Unsize` + `Ref`/`RawPtr`-to-unsized), and the
//! section 10.1 `*WithOverflow` result `(T, bool)`. Task d2.1 adds the
//! section 13 enum tag-ops over the c2 union-of-views (`Downcast`+`Field`,
//! enum `Discriminant`, `SetDiscriminant`, `Aggregate::Enum`, single-member
//! `Const::EnumValue`). The remaining deferrals still fail loud via
//! [`EmitError::Unsupported`]: all non-scalar `Const` (`Bytes` / `Aggregate`)
//! plus the multi-member `Const::EnumValue` are task d2.2; `dyn` / vtable
//! construction is the vtable task. Diag-log call-site interception is
//! **task e**.

use regseq_model::{
    AggregateKind, AssertKind, AttrData, BinOp, CallCallee, CastKind, Const, ConstId, EnumShape,
    FloatKind, InstanceId, InstanceKind, InstanceRef, IntKind, IntrinsicKind, LocalId,
    NonDivergingIntrinsic, Operand, Place, PlaceElem, PointerCoercion, RegseqModel, Rvalue,
    StatementKind, SwitchTargets, TagEncoding, TerminatorKind, Ty, TyId, UintKind, UnOp,
    VariantIdx, VariantLayout,
};

use super::{
    enum_emit, layout_emit, naming, slice_data_const, types, EmitError, InstanceSymbols, NameTable,
};

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

/// The result of walking a place's projection chain ([`BodyEmit::place_walk`]):
/// the C lvalue and the running (section 19.4 narrowed) type, plus -- when the
/// chain crossed a fat (slice/str) `Deref` -- the lvalue of that fat pointer
/// base (for a trailing `from_end` index `.len` or a `Subslice` re-borrow).
struct PlaceWalk {
    /// The C lvalue expression.
    lvalue: String,
    /// The running narrowed type after the full projection chain.
    ty: TyId,
    /// The most recent fat-pointer base lvalue, when a fat `Deref` was crossed.
    fat_base: Option<String>,
    /// Accumulated `from` offset from array `Subslice` projections not yet
    /// consumed by an `Index` / `ConstantIndex`.
    pending_from: u64,
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

    /// The resolved type of a place: the head local's type narrowed through
    /// the full section 19.4 projection chain (shared with [`Self::place_lvalue`]
    /// via [`Self::place_walk`], so the lvalue and type cannot drift).
    /// Infallible: on a deferred construct (a `dyn` `Deref`, or a malformed
    /// `Downcast`) [`Self::place_walk`] errors and [`Self::place_lvalue`]
    /// (also via `place_walk`) fails loud at the use site, so the head-local
    /// fallback type returned here is never consumed for emission.
    fn place_ty(&self, place: &Place) -> TyId {
        self.place_walk(place)
            .map(|w| w.ty)
            .unwrap_or_else(|_| self.body.local_decls[place.local].ty)
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
            StatementKind::SetDiscriminant(place, variant) => {
                self.set_discriminant(place, variant.index())
            }
            StatementKind::Intrinsic(intr) => Ok(Some(self.intrinsic(intr)?)),
        }
    }

    /// `Assign(place, rvalue)` -> `<place> = <rvalue>;`. A ZST destination
    /// elides the statement (the local has no C declaration). Some rvalues
    /// cannot be a plain `dst = <expr>;` and lower at the assign site:
    /// `Transmute` (a `memcpy` reinterpret, section 11), an **array**
    /// destination (`Aggregate(Array)` / `Repeat` -- C forbids array
    /// assignment, section 14 / boundary 6), and `*WithOverflow` (a
    /// multi-field `(T, bool)` write, section 10.1 / Change G).
    fn assign(&self, place: &Place, rvalue: &Rvalue) -> Result<Option<String>, EmitError> {
        // Validate the destination place first (rejects d2.1 / vtable
        // projections) and recover its lvalue + type from the single walk.
        let walk = self.place_walk(place)?;
        let dst = walk.lvalue;
        let dst_ty = walk.ty;
        let zst = self.is_zst(dst_ty);

        match rvalue {
            Rvalue::Cast(CastKind::Transmute, op, _) => {
                // Bit reinterpret via memcpy between equal-sizeof objects.
                // Validate the source operand first, then elide the write for
                // a ZST destination (a 0-byte transmute is a no-op, and the ZST
                // destination local is never declared so it cannot be addressed).
                let src_addr = self.operand_addr(op)?;
                if zst {
                    return Ok(None);
                }
                return Ok(Some(format!(
                    "memcpy(&({dst}), {src_addr}, sizeof({dst}));"
                )));
            }
            Rvalue::Aggregate(AggregateKind::Array(_), ops) => {
                return self.assign_array(&dst, dst_ty, ops);
            }
            Rvalue::Aggregate(AggregateKind::Enum(enum_ty, variant), ops) => {
                // Enum construction is a tag write + disjoint payload-field
                // writes through the variant views (C forbids a single
                // compound literal designating two union members), so it
                // lowers at the assign site like the array cases (Change D).
                return self.assign_enum(&dst, *enum_ty, variant.index(), ops, zst);
            }
            Rvalue::Discriminant(place) if self.is_niche_discriminant_place(place) => {
                // The niche cell has no `_tag` view (section 13); the read is
                // a section 14.7 byte-storage memcpy into a temp then a
                // ternary, so it lowers at the assign site (Change C).
                return self.assign_niche_discriminant(&dst, dst_ty, place, zst);
            }
            Rvalue::Aggregate(kind, ops)
                if Self::is_struct_like(kind) && self.has_array_operand(ops) =>
            {
                // A struct / tuple / closure / union with an array field cannot
                // be a compound-literal expression (C forbids an array
                // initializer expression); lower it per-field here.
                return self.assign_struct_fields(&dst, kind, ops, zst);
            }
            Rvalue::Repeat(op, n) => {
                return self.assign_repeat(&dst, dst_ty, op, *n);
            }
            Rvalue::BinaryOp(
                op @ (BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow),
                a,
                b,
            ) => {
                return self.assign_with_overflow(&dst, op, a, b, zst);
            }
            _ => {}
        }

        // Lower the rvalue first so a deferred construct fails loud even when
        // the destination is ZST, rather than being silently dropped. Only
        // after the rvalue is a recognized form do we elide the assignment for
        // a ZST destination (the local is never declared; a ZST write is a
        // no-op).
        let value = self.rvalue_expr(rvalue, dst_ty)?;
        if zst {
            return Ok(None);
        }
        Ok(Some(format!("{dst} = {value};")))
    }

    /// The element type of an array `ty`, when it is one.
    fn array_elem(&self, ty: TyId) -> Option<TyId> {
        match &self.model.types[ty] {
            Ty::Array(elem, _) => Some(*elem),
            _ => None,
        }
    }

    /// A temporary C identifier that cannot collide with any body-local C name
    /// (so the `Repeat` loop variable / `*WithOverflow` temp are sound even
    /// when a debug-named local sanitizes to the same base). Deterministic: a
    /// pure function of the body's local names, so emission stays byte-stable.
    fn fresh_temp_name(&self, base: &str) -> String {
        let taken: std::collections::HashSet<String> = (0..self.body.local_decls.len())
            .map(|i| local_c_name(self.model, self.body, LocalId::from_u32(i as u32)))
            .collect();
        if !taken.contains(base) {
            return base.to_string();
        }
        let mut n = 1usize;
        loop {
            let candidate = format!("{base}_{n}");
            if !taken.contains(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    /// Whether any non-ZST operand is an array value (a place of `Ty::Array`).
    /// A struct / tuple / closure / union aggregate with such a field cannot
    /// use a designated-initializer compound literal (C forbids an array
    /// expression as an initializer), so it lowers per-field at the assign site.
    fn has_array_operand(&self, ops: &[Operand]) -> bool {
        ops.iter().any(|op| {
            !self.is_zst_operand(op)
                && self
                    .operand_ty(op)
                    .is_some_and(|t| matches!(self.model.types[t], Ty::Array(_, _)))
        })
    }

    /// A struct / tuple / closure / union aggregate **with an array-typed
    /// field**, lowered per-field at the assign site (a compound literal cannot
    /// initialize an array member from an array expression): a scalar /
    /// aggregate field is `dst.f = <op>;`, an array field is
    /// `memcpy(&dst.f, &<op>, sizeof dst.f);` (C array no-assignment, boundary 6).
    fn assign_struct_fields(
        &self,
        dst: &str,
        kind: &AggregateKind,
        ops: &[Operand],
        zst: bool,
    ) -> Result<Option<String>, EmitError> {
        let (ty, union_field) = match kind {
            AggregateKind::Struct(ty) | AggregateKind::Tuple(ty) | AggregateKind::Closure(ty) => {
                (*ty, None)
            }
            AggregateKind::Union(ty, field) => (*ty, Some(field.index())),
            _ => unreachable!("assign_struct_fields on a non-struct-like aggregate"),
        };
        let mut stmts = Vec::new();
        for (i, op) in ops.iter().enumerate() {
            if self.is_zst_operand(op) {
                continue;
            }
            let field_idx = union_field.unwrap_or(i);
            let (name, fty) = layout_emit::field_c_name_ty(self.model, ty, field_idx).ok_or(
                EmitError::Unsupported("Aggregate field of a non-aggregate type (task d2)"),
            )?;
            if matches!(self.model.types[fty], Ty::Array(_, _)) {
                let src = self.operand_addr(op)?;
                stmts.push(format!(
                    "memcpy(&{dst}.{name}, {src}, sizeof({dst}.{name}));"
                ));
            } else {
                let e = self.operand_expr(op, Some(fty))?;
                stmts.push(format!("{dst}.{name} = {e};"));
            }
        }
        if zst {
            return Ok(None);
        }
        Ok(Some(stmts.join(" ")))
    }

    /// Whether an `AggregateKind` is a struct / tuple / closure / union (the
    /// compound-literal kinds, as opposed to `Array` / `RawPtr` / `Enum`).
    fn is_struct_like(kind: &AggregateKind) -> bool {
        matches!(
            kind,
            AggregateKind::Struct(_)
                | AggregateKind::Tuple(_)
                | AggregateKind::Closure(_)
                | AggregateKind::Union(_, _)
        )
    }

    /// `Aggregate(Array, ops)` at the assign site (C forbids array
    /// assignment, boundary 6): per-element `dst[i] = <opi>;` for a scalar /
    /// non-array element, or `memcpy(&dst[i], &<opi>, ...)` for an array
    /// element (which is itself not assignable). A ZST array writes nothing.
    fn assign_array(
        &self,
        dst: &str,
        dst_ty: TyId,
        ops: &[Operand],
    ) -> Result<Option<String>, EmitError> {
        let elem_ty = self.array_elem(dst_ty);
        if self.is_zst(dst_ty) {
            // A ZST array ([ZST; n] or [T; 0]): validate each operand, emit
            // nothing (the destination local is never declared).
            for op in ops.iter() {
                let _ = self.operand_expr(op, elem_ty)?;
            }
            return Ok(None);
        }
        let elem_ty = elem_ty.ok_or(EmitError::Unsupported(
            "Array aggregate into a non-array destination (task d2)",
        ))?;
        let elem_is_array = matches!(self.model.types[elem_ty], Ty::Array(_, _));
        let mut stmts = Vec::new();
        for (i, op) in ops.iter().enumerate() {
            if elem_is_array {
                let src = self.operand_addr(op)?;
                stmts.push(format!("memcpy(&{dst}[{i}], {src}, sizeof({dst}[{i}]));"));
            } else {
                let e = self.operand_expr(op, Some(elem_ty))?;
                stmts.push(format!("{dst}[{i}] = {e};"));
            }
        }
        Ok(Some(stmts.join(" ")))
    }

    /// `Repeat(op, n)` at the assign site (array fill, boundary 6): a
    /// per-element fill loop. The loop variable ([`Self::fresh_temp_name`],
    /// `_ri` by default) is collision-proofed against the body's local names
    /// and lives in the `for`'s own scope, so sibling loops in the same block
    /// do not collide. A ZST array writes nothing.
    fn assign_repeat(
        &self,
        dst: &str,
        dst_ty: TyId,
        op: &Operand,
        n: u64,
    ) -> Result<Option<String>, EmitError> {
        let elem_ty = self.array_elem(dst_ty);
        if self.is_zst(dst_ty) {
            let _ = self.operand_expr(op, elem_ty)?;
            return Ok(None);
        }
        let elem_ty = elem_ty.ok_or(EmitError::Unsupported(
            "Repeat into a non-array destination (task d2)",
        ))?;
        let iv = self.fresh_temp_name("_ri");
        let store = if matches!(self.model.types[elem_ty], Ty::Array(_, _)) {
            let src = self.operand_addr(op)?;
            format!("memcpy(&{dst}[{iv}], {src}, sizeof({dst}[{iv}]));")
        } else {
            let e = self.operand_expr(op, Some(elem_ty))?;
            format!("{dst}[{iv}] = {e};")
        };
        Ok(Some(format!(
            "for (size_t {iv} = 0; {iv} < {n}; {iv}++) {store}"
        )))
    }

    /// `*WithOverflow` at the assign site (section 10.1 / Change G): call the
    /// prelude `regseq_{add,sub,mul}_<T>` helper once into a temporary, then
    /// write the destination `(T, bool)` tuple's two fields (`_0` = the value
    /// `v`, `_1` = the `overflow` flag). The temp ([`Self::fresh_temp_name`],
    /// `_ov` by default) is collision-proofed against the body's local names
    /// and lives in a fresh block scope, so it cannot collide with a body local
    /// or a sibling site.
    fn assign_with_overflow(
        &self,
        dst: &str,
        op: &BinOp,
        a: &Operand,
        b: &Operand,
        zst: bool,
    ) -> Result<Option<String>, EmitError> {
        let x = self.operand_expr(a, None)?;
        let y = self.operand_expr(b, None)?;
        let info = self.operand_int_info(a).ok_or(EmitError::Unsupported(
            "*WithOverflow on a non-integer operand (task d2)",
        ))?;
        // (T, bool) is never zero-sized; the guard is defensive.
        if zst {
            return Ok(None);
        }
        let helper = match op {
            BinOp::AddWithOverflow => "add",
            BinOp::SubWithOverflow => "sub",
            BinOp::MulWithOverflow => "mul",
            _ => unreachable!("assign_with_overflow on a non-overflow BinOp"),
        };
        let ct = info.ctype;
        let tmp = self.fresh_temp_name("_ov");
        Ok(Some(format!(
            "{{ struct regseq_overflow_{ct} {tmp} = regseq_{helper}_{ct}(({x}), ({y})); \
             {dst}._0 = {tmp}.v; {dst}._1 = {tmp}.overflow; }}"
        )))
    }

    // -- section 13 enum tag-ops ---------------------------------------------

    /// Whether a `Discriminant(place)` reads a `Multiple { Niche }` enum (the
    /// niche cell has no `_tag` view, so the read lowers at the assign site).
    fn is_niche_discriminant_place(&self, place: &Place) -> bool {
        matches!(
            enum_emit::enum_shape(self.model, self.place_ty(place)),
            Some(EnumShape::Multiple {
                encoding: TagEncoding::Niche { .. },
                ..
            })
        )
    }

    /// `Discriminant(place)` as a pure C expression (section 13 / 21.2.8). A
    /// non-enum predecessor yields the constant `0`; an enum yields the
    /// **matched variant's `discriminant`** (not the raw cell, not
    /// `tag_value`). The `Multiple { Niche }` case has no `_tag` view and is
    /// handled at the assign site ([`Self::assign_niche_discriminant`]), so it
    /// is defensive here. `result_ty` renders the discriminant literal.
    fn discriminant_expr(&self, place: &Place, result_ty: TyId) -> Result<String, EmitError> {
        let ty = self.place_ty(place);
        let Some(shape) = enum_emit::enum_shape(self.model, ty) else {
            // Section 21.2.8: a non-enum predecessor's discriminant is 0.
            return Ok("0".to_string());
        };
        match shape {
            // An uninhabited enum has no runtime value, so `Discriminant` on
            // it is statically unreachable; a defensive `0` keeps the (dead)
            // expression well-typed.
            EnumShape::Empty => Ok("0".to_string()),
            EnumShape::Single { variant, .. } => {
                render_discriminant(self.model, result_ty, variant.discriminant)
            }
            EnumShape::Multiple {
                encoding: TagEncoding::Direct,
                variants,
                ..
            } => {
                let enum_lv = self.place_lvalue(place)?;
                let (member, scalar_ty) = match enum_emit::enum_tag(self.model, shape) {
                    enum_emit::EnumTag::View {
                        member_path,
                        scalar_ty,
                    } => (member_path, scalar_ty),
                    _ => {
                        return Err(EmitError::Unsupported(
                            "Direct enum without a tag view (malformed layout)",
                        ))
                    }
                };
                self.direct_discriminant_ternary(&enum_lv, &member, scalar_ty, variants, result_ty)
            }
            EnumShape::Multiple {
                encoding: TagEncoding::Niche { .. },
                ..
            } => Err(EmitError::Unsupported(
                "niche Discriminant outside an assignment (assign-site only)",
            )),
        }
    }

    /// The `Multiple { Direct }` discriminant ternary over the `_tag` read:
    /// `(<read> == <tv0> ? <d0> : ... : <d_last>)`. `tvN` is rendered per the
    /// tag `scalar_ty`, `dN` per `result_ty`; the last variant is the
    /// fallthrough. Deterministic in variant order.
    fn direct_discriminant_ternary(
        &self,
        enum_lv: &str,
        tag_member: &str,
        scalar_ty: TyId,
        variants: &[VariantLayout],
        result_ty: TyId,
    ) -> Result<String, EmitError> {
        if variants.is_empty() {
            return Err(EmitError::Unsupported("Multiple enum with no variants"));
        }
        let tag_read = format!("{enum_lv}.{tag_member}");
        let n = variants.len();
        let mut expr = render_discriminant(self.model, result_ty, variants[n - 1].discriminant)?;
        for v in variants[..n - 1].iter().rev() {
            let tv = v.tag_value.ok_or(EmitError::Unsupported(
                "Direct variant without a tag_value (malformed layout)",
            ))?;
            let tvr = render_tag_value(self.model, self.names, scalar_ty, tv)?;
            let d = render_discriminant(self.model, result_ty, v.discriminant)?;
            expr = format!("({tag_read} == {tvr} ? {d} : {expr})");
        }
        Ok(expr)
    }

    /// `Discriminant` of a `Multiple { Niche }` enum at the assign site: the
    /// niche cell has no `_tag` view, so read it via the section 14.7 byte-
    /// storage technique (memcpy into a typed temp, sound under `-std=gnu11`
    /// with no `-fno-strict-aliasing`), then a ternary over the **niched**
    /// variants (`tag_value: Some`) falling through to the `untagged_variant`'s
    /// `discriminant` (section 13 "no match -> untagged_variant"). The temp
    /// ([`Self::fresh_temp_name`]) lives in its own block scope.
    fn assign_niche_discriminant(
        &self,
        dst: &str,
        dst_ty: TyId,
        enum_place: &Place,
        zst: bool,
    ) -> Result<Option<String>, EmitError> {
        let ty = self.place_ty(enum_place);
        let shape = enum_emit::enum_shape(self.model, ty).ok_or(EmitError::Unsupported(
            "niche Discriminant on a non-enum place",
        ))?;
        let (untagged, variants) = match shape {
            EnumShape::Multiple {
                encoding:
                    TagEncoding::Niche {
                        untagged_variant, ..
                    },
                variants,
                ..
            } => (untagged_variant.index(), variants.as_slice()),
            _ => {
                return Err(EmitError::Unsupported(
                    "assign_niche_discriminant on a non-niche enum",
                ))
            }
        };
        let (offset, scalar_ty) = match enum_emit::enum_tag(self.model, shape) {
            enum_emit::EnumTag::Niche { offset, scalar_ty } => (offset, scalar_ty),
            _ => {
                return Err(EmitError::Unsupported(
                    "niche enum without a niche cell (malformed layout)",
                ))
            }
        };
        let enum_lv = self.place_lvalue(enum_place)?;
        // The discriminant result is an integer, never ZST; defensive guard.
        if zst {
            return Ok(None);
        }
        let untagged_disc = variants
            .get(untagged)
            .ok_or(EmitError::Unsupported(
                "niche untagged variant index out of range",
            ))?
            .discriminant;
        let mut expr = render_discriminant(self.model, dst_ty, untagged_disc)?;
        let tmp = self.fresh_temp_name("_nd");
        for v in variants.iter().rev() {
            if let Some(tv) = v.tag_value {
                let tvr = render_tag_value(self.model, self.names, scalar_ty, tv)?;
                let d = render_discriminant(self.model, dst_ty, v.discriminant)?;
                expr = format!("({tmp} == {tvr} ? {d} : {expr})");
            }
        }
        let tmp_cty = types::c_type(self.model, self.names, scalar_ty);
        Ok(Some(format!(
            "{{ {tmp_cty} {tmp}; memcpy(&{tmp}, (const unsigned char *)&({enum_lv}) + {offset}, \
             sizeof({tmp})); {dst} = {expr}; }}"
        )))
    }

    /// `SetDiscriminant(place, v)` (section 13): resolve the enum lvalue, then
    /// write `VariantLayout[v].tag_value` to the tag cell -- the `_tag` view
    /// member (`Direct` / `Single { scalar: Some }`), or the niche cell via a
    /// section 14.7 memcpy (`Niche` niched). A `Niche` untagged variant /
    /// `Single { scalar: None }` is a no-op (the statement is elided).
    fn set_discriminant(&self, place: &Place, v: usize) -> Result<Option<String>, EmitError> {
        let walk = self.place_walk(place)?;
        let shape = enum_emit::enum_shape(self.model, walk.ty).ok_or(EmitError::Unsupported(
            "SetDiscriminant on a non-enum place",
        ))?;
        self.emit_set_discriminant(&walk.lvalue, shape, v)
    }

    /// The tag-write statement for variant `v` of `shape` at the (already
    /// resolved) enum lvalue `enum_lv`, or `None` when the variant writes no
    /// tag bytes. Shared by [`Self::set_discriminant`] and the construction
    /// path ([`Self::assign_enum`]).
    fn emit_set_discriminant(
        &self,
        enum_lv: &str,
        shape: &EnumShape,
        v: usize,
    ) -> Result<Option<String>, EmitError> {
        let tag_value = variant_layout(shape, v)
            .ok_or(EmitError::Unsupported(
                "SetDiscriminant variant index out of range",
            ))?
            .tag_value;
        match enum_emit::enum_tag(self.model, shape) {
            enum_emit::EnumTag::View {
                member_path,
                scalar_ty,
            } => match tag_value {
                Some(tv) => {
                    let tvr = render_tag_value(self.model, self.names, scalar_ty, tv)?;
                    Ok(Some(format!("{enum_lv}.{member_path} = {tvr};")))
                }
                // A tag-view variant always carries a `tag_value`; defensive.
                None => Ok(None),
            },
            enum_emit::EnumTag::Niche { offset, scalar_ty } => match tag_value {
                Some(tv) => {
                    let scalar_cty = types::c_type(self.model, self.names, scalar_ty);
                    let tvr = render_tag_value(self.model, self.names, scalar_ty, tv)?;
                    Ok(Some(format!(
                        "memcpy((unsigned char *)&({enum_lv}) + {offset}, &({scalar_cty}){{{tvr}}}, \
                         sizeof({scalar_cty}));"
                    )))
                }
                // Niche untagged variant: the payload is the tag -> no-op.
                None => Ok(None),
            },
            // Single { scalar: None } / Empty: no tag bytes -> no-op.
            enum_emit::EnumTag::None => Ok(None),
        }
    }

    /// `Aggregate::Enum(ty, v)` at the assign site (section 13): the tag write
    /// (per the variant's encoding) followed by each non-ZST payload field,
    /// written through the variant view at its naturally-aligned offset (a
    /// direct member access; an array-typed field via `memcpy`, the d2 array
    /// rule). Tag and payload occupy disjoint byte ranges aliased through
    /// different `union` members; tag-first for determinism.
    fn assign_enum(
        &self,
        dst: &str,
        enum_ty: TyId,
        v: usize,
        ops: &[Operand],
        zst: bool,
    ) -> Result<Option<String>, EmitError> {
        let shape = enum_emit::enum_shape(self.model, enum_ty)
            .ok_or(EmitError::Unsupported("Aggregate::Enum on a non-enum type"))?;
        let mut stmts = Vec::new();
        if let Some(tag_stmt) = self.emit_set_discriminant(dst, shape, v)? {
            stmts.push(tag_stmt);
        }
        for (i, op) in ops.iter().enumerate() {
            if self.is_zst_operand(op) {
                continue;
            }
            match enum_emit::enum_variant_field(self.model, enum_ty, v, i) {
                Some((member_path, fty)) => {
                    if matches!(self.model.types[fty], Ty::Array(_, _)) {
                        let src = self.operand_addr(op)?;
                        stmts.push(format!(
                            "memcpy(&{dst}.{member_path}, {src}, sizeof({dst}.{member_path}));"
                        ));
                    } else {
                        let e = self.operand_expr(op, Some(fty))?;
                        stmts.push(format!("{dst}.{member_path} = {e};"));
                    }
                }
                None => {
                    // A non-ZST operand with no view field is a navigation
                    // mismatch (a ZST operand was skipped above). Validate the
                    // operand, then fail loud rather than drop the write.
                    let _ = self.operand_expr(op, None)?;
                    return Err(EmitError::Unsupported(
                        "enum construction payload field navigation failed",
                    ));
                }
            }
        }
        if zst {
            return Ok(None);
        }
        Ok(Some(stmts.join(" ")))
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

    /// The C lvalue of a place (section 4). Delegates to the single shared
    /// [`Self::place_walk`] so the lvalue and the running type cannot drift.
    fn place_lvalue(&self, place: &Place) -> Result<String, EmitError> {
        Ok(self.place_walk(place)?.lvalue)
    }

    /// Walk a place's projection chain once, building the C lvalue and the
    /// running section 19.4 narrowed type together (the single shared walk).
    ///
    /// - `Deref` (always index 0 per I-Deref-first): a **thin** pointer (to a
    ///   sized pointee) -> `(*lv)`; a **fat** pointer (to `Slice` / `Str`) ->
    ///   the `data` slot `lv.data`, remembering the fat base for a trailing
    ///   `.len`; a `dyn` pointer -> the vtable task.
    /// - `Field` (struct / tuple / closure / union predecessor) -> `lv.fname`
    ///   (the section 14 name, [`layout_emit::field_c_name_ty`]); narrow to the
    ///   inline post-narrow field type (place.rs). An enum predecessor is
    ///   unreachable here (a prior `Downcast` errors first).
    /// - `Index(local)` -> `lv[<local>]` (plus a pending array-`Subslice`
    ///   `from` offset); narrow to the element type.
    /// - `ConstantIndex` -> `[offset]` (`from_end:false`, plus a pending
    ///   `from`) or `[<base>.len - offset]` (`from_end:true`, the remembered
    ///   fat base); narrow to the element type.
    /// - `Subslice` as a place lvalue: a sized array base (`from_end:false`)
    ///   records the `from` offset + sub-array element so a follow-on index
    ///   offsets by `from`; the lvalue base is unchanged (a slice `Subslice`
    ///   is unsized and is materialised only under a `Ref`, [`Self::ref_expr`]).
    /// - `Downcast(v)` -> records a pending variant the following `Field`
    ///   consumes to spell the variant payload member path (section 13).
    fn place_walk(&self, place: &Place) -> Result<PlaceWalk, EmitError> {
        let mut lv = local_c_name(self.model, self.body, place.local);
        let mut ty = self.body.local_decls[place.local].ty;
        let mut fat_base: Option<String> = None;
        let mut pending_from: Option<u64> = None;
        let mut sub_array_elem: Option<TyId> = None;
        // Set by a `Downcast(v)`; consumed by the following `Field` to spell
        // the variant payload member path (I-Downcast-followed-by-Field).
        let mut pending_variant: Option<usize> = None;

        for elem in place.projection.iter() {
            match elem {
                PlaceElem::Deref => {
                    let pointee = match &self.model.types[ty] {
                        Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                        _ => {
                            // Not a pointer (malformed); preserve the d1 form.
                            lv = format!("(*{lv})");
                            continue;
                        }
                    };
                    match &self.model.types[pointee] {
                        Ty::Slice(_) | Ty::Str => {
                            // Fat pointer: the data slot; remember the base.
                            fat_base = Some(lv.clone());
                            lv = format!("{lv}.data");
                        }
                        Ty::Dynamic { .. } => {
                            return Err(EmitError::Unsupported(
                                "Deref of a dyn pointer (vtable task)",
                            ));
                        }
                        _ => {
                            // Thin pointer.
                            lv = format!("(*{lv})");
                            fat_base = None;
                        }
                    }
                    ty = pointee;
                    pending_from = None;
                    sub_array_elem = None;
                }
                PlaceElem::Field(idx, field_ty) => {
                    if let Some(v) = pending_variant.take() {
                        // Enum variant payload field: spell the variant view
                        // member path via the shared `enum_emit` navigator
                        // (Change A), so it matches the emitted definition.
                        // Payload views are naturally aligned, so this is a
                        // direct member access (no section 14.7 memcpy).
                        match enum_emit::enum_variant_field(self.model, ty, v, idx.index()) {
                            Some((member_path, _)) => lv = format!("{lv}.{member_path}"),
                            None => {
                                // The navigator omits ZST payload fields from
                                // the view; a ZST field's lvalue is never
                                // materialised (ZST-elision drops its
                                // assignment), so leave `lv` unchanged. A
                                // non-ZST `None` is a real navigation failure.
                                if !self.is_zst(*field_ty) {
                                    return Err(EmitError::Unsupported(
                                        "enum variant payload field navigation failed",
                                    ));
                                }
                            }
                        }
                    } else {
                        let (name, _) = layout_emit::field_c_name_ty(self.model, ty, idx.index())
                            .ok_or(EmitError::Unsupported(
                            "Field projection of a non-aggregate predecessor",
                        ))?;
                        lv = format!("{lv}.{name}");
                    }
                    ty = *field_ty;
                    fat_base = None;
                    pending_from = None;
                    sub_array_elem = None;
                }
                PlaceElem::Index(local) => {
                    let idx_name = local_c_name(self.model, self.body, *local);
                    let index_expr = match pending_from.take() {
                        Some(from) => format!("{from} + {idx_name}"),
                        None => idx_name,
                    };
                    lv = format!("{lv}[{index_expr}]");
                    ty = self.index_elem_ty(ty, sub_array_elem.take())?;
                    fat_base = None;
                }
                PlaceElem::ConstantIndex {
                    offset, from_end, ..
                } => {
                    if *from_end {
                        let base = fat_base.clone().ok_or(EmitError::Unsupported(
                            "ConstantIndex from_end on a non-slice place (task d2)",
                        ))?;
                        lv = format!("{lv}[{base}.len - {offset}]");
                    } else {
                        let index_expr = match pending_from.take() {
                            Some(from) => format!("{}", from + offset),
                            None => format!("{offset}"),
                        };
                        lv = format!("{lv}[{index_expr}]");
                    }
                    ty = self.index_elem_ty(ty, sub_array_elem.take())?;
                    fat_base = None;
                }
                PlaceElem::Subslice { from, to, from_end } => {
                    if *from_end {
                        // A slice Subslice is unsized; it is materialised only
                        // under a Ref ([`Self::ref_expr`]), never as a bare lvalue.
                        return Err(EmitError::Unsupported(
                            "Subslice from_end as a place lvalue (task d2)",
                        ));
                    }
                    let elem = match &self.model.types[ty] {
                        Ty::Array(elem, _) => *elem,
                        _ => {
                            return Err(EmitError::Unsupported(
                                "Subslice of a non-array place lvalue (task d2)",
                            ))
                        }
                    };
                    // Record the `from` offset + sub-array element so a
                    // follow-on index offsets by `from`; the lvalue is
                    // unchanged (section 19.4: Array -> Array(elem, to - from)).
                    pending_from = Some(pending_from.unwrap_or(0) + *from);
                    sub_array_elem = Some(elem);
                    let _ = to;
                    fat_base = None;
                }
                PlaceElem::Downcast(v) => {
                    // Record the active variant; the running `ty` stays the
                    // enum (the section 19.4 narrowing is realised by the
                    // following `Field`'s carried type). By
                    // I-Downcast-followed-by-Field the next elem is a `Field`.
                    pending_variant = Some(v.index());
                    fat_base = None;
                    pending_from = None;
                    sub_array_elem = None;
                }
            }
        }
        // A `Downcast` is never last (I-Downcast-followed-by-Field); a still-
        // pending variant means a malformed place.
        if pending_variant.is_some() {
            return Err(EmitError::Unsupported(
                "Downcast not followed by a Field (malformed place)",
            ));
        }
        Ok(PlaceWalk {
            lvalue: lv,
            ty,
            fat_base,
            pending_from: pending_from.unwrap_or(0),
        })
    }

    /// The element type reached by an `Index` / `ConstantIndex`: the tracked
    /// sub-array element (from a prior array `Subslice`, boundary 7) if set,
    /// else the `Array` / `Slice` element of the running type.
    fn index_elem_ty(&self, ty: TyId, sub_array_elem: Option<TyId>) -> Result<TyId, EmitError> {
        if let Some(elem) = sub_array_elem {
            return Ok(elem);
        }
        match &self.model.types[ty] {
            Ty::Array(elem, _) | Ty::Slice(elem) => Ok(*elem),
            _ => Err(EmitError::Unsupported(
                "Index of a non-array / non-slice place (task d2)",
            )),
        }
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
            Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) => self.ref_expr(place, dest_ty),
            Rvalue::Cast(kind, op, ty) => self.cast_expr(kind, op, *ty),
            Rvalue::BinaryOp(op, a, b) => self.binary_op(op, a, b),
            Rvalue::UnaryOp(op, a) => self.unary_op(op, a),
            // Array fill (Repeat) is handled at the assign site (C forbids
            // array assignment); reaching here is defensive.
            Rvalue::Repeat(_, _) => Err(EmitError::Unsupported(
                "Rvalue::Repeat outside an assignment (task d2)",
            )),
            Rvalue::Discriminant(place) => self.discriminant_expr(place, dest_ty),
            Rvalue::Aggregate(kind, ops) => self.aggregate_expr(kind, ops, dest_ty),
        }
    }

    /// `Ref(_, place)` / `RawPtr(_, place)` (section 10): the address of a
    /// sized place (`&<place>`), or -- for an unsized slice/str place -- the
    /// section 3.2 fat pointer built per section 11 (the same construction
    /// reached from the place side, spec line 853). `dest_ty` is the
    /// reference / raw-pointer destination type.
    fn ref_expr(&self, place: &Place, dest_ty: TyId) -> Result<String, EmitError> {
        match place.projection.last() {
            Some(PlaceElem::Subslice { from, to, from_end }) => {
                self.subslice_ref(place, *from, *to, *from_end, dest_ty)
            }
            Some(PlaceElem::Deref) => {
                let pointee = self.place_ty(place);
                if self.is_unsized(pointee) {
                    // A re-borrow of a whole unsized place (`&(*p)`).
                    self.fat_reborrow(place, pointee)
                } else {
                    Ok(format!("&({})", self.place_lvalue(place)?))
                }
            }
            _ => {
                if self.is_unsized(self.place_ty(place)) {
                    return Err(EmitError::Unsupported(
                        "Ref / RawPtr to an unsized non-slice place (task d2)",
                    ));
                }
                // Taking the address of a field reached through a packed
                // aggregate yields a possibly-misaligned pointer (section 14.7):
                // a plain value load/store of a packed member is sound directly,
                // but its address is not, so fail loud rather than emit UB.
                if self.ref_crosses_packed_field(place) {
                    return Err(EmitError::Unsupported(
                        "address of a misaligned packed field (section 14.7)",
                    ));
                }
                Ok(format!("&({})", self.place_lvalue(place)?))
            }
        }
    }

    /// Whether the address of `place` is taken through a `Field` projection
    /// into a `repr(packed)` aggregate (so the resulting pointer may be
    /// misaligned, section 14.7). A `Deref` resets alignment (the pointee is a
    /// properly-aligned object); array / slice indexing keeps element alignment.
    fn ref_crosses_packed_field(&self, place: &Place) -> bool {
        let mut ty = self.body.local_decls[place.local].ty;
        for elem in place.projection.iter() {
            match elem {
                PlaceElem::Deref => {
                    ty = match &self.model.types[ty] {
                        Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                        _ => return false,
                    };
                }
                PlaceElem::Field(_, field_ty) => {
                    let packed = matches!(
                        &self.model.types[ty],
                        Ty::Struct { layout, .. }
                            | Ty::Union { layout, .. }
                            | Ty::Tuple { layout, .. }
                            if layout.pack.is_some()
                    );
                    if packed {
                        return true;
                    }
                    ty = *field_ty;
                }
                PlaceElem::Index(_)
                | PlaceElem::ConstantIndex { .. }
                | PlaceElem::Subslice { .. } => {
                    ty = match &self.model.types[ty] {
                        Ty::Array(elem, _) | Ty::Slice(elem) => *elem,
                        _ => return false,
                    };
                }
                PlaceElem::Downcast(_) => return false,
            }
        }
        false
    }

    /// A re-borrow of a whole unsized place (`&(*p)` where `p` is a fat
    /// pointer): the existing fat pointer copied (`{ .data, .len }` identity),
    /// i.e. the fat base itself. A `dyn` pointee is the vtable task.
    fn fat_reborrow(&self, place: &Place, pointee_ty: TyId) -> Result<String, EmitError> {
        if matches!(self.model.types[pointee_ty], Ty::Dynamic { .. }) {
            return Err(EmitError::Unsupported("Ref of a dyn deref (vtable task)"));
        }
        let walk = self.place_walk(place)?;
        walk.fat_base.ok_or(EmitError::Unsupported(
            "re-borrow of an unsized place without a fat base (task d2)",
        ))
    }

    /// A `Subslice` under a `Ref` / `RawPtr`: a **slice** base builds a new
    /// section 3.2 fat pointer (`{ .data = base.data + from, .len }`, the
    /// `.data` cast to the element pointer type); a **sized array** base
    /// builds a thin pointer to the sub-array (`(<dest>)&base[from]`, where
    /// `<dest>` is the destination `&[elem; to - from]` C spelling). `dest_ty`
    /// is the reference type.
    fn subslice_ref(
        &self,
        place: &Place,
        from: u64,
        to: u64,
        from_end: bool,
        dest_ty: TyId,
    ) -> Result<String, EmitError> {
        let n = place.projection.len();
        let base_place = Place {
            local: place.local,
            projection: place.projection[..n - 1].to_vec().into_boxed_slice(),
        };
        let base = self.place_walk(&base_place)?;
        let effective_from = base.pending_from + from;
        match &self.model.types[base.ty] {
            Ty::Array(_, _) => {
                if from_end {
                    return Err(EmitError::Unsupported(
                        "Subslice from_end on an array base (task d2)",
                    ));
                }
                // `&base[from]` takes the address of the array base, so a base
                // reached through a packed field is a possibly-misaligned
                // pointer (section 14.7); fail loud rather than emit UB. (The
                // slice branch below reads `.data` by value, so it is exempt.)
                if self.ref_crosses_packed_field(&base_place) {
                    return Err(EmitError::Unsupported(
                        "address of a misaligned packed field (section 14.7)",
                    ));
                }
                // dest_ty is `&[elem; to - from]`; its C spelling is exactly
                // the pointer-to-array cast (with the correct const-ness).
                let cast = types::c_type(self.model, self.names, dest_ty);
                Ok(format!("({cast})&({}[{effective_from}])", base.lvalue))
            }
            Ty::Slice(elem) => {
                let fat = types::unsized_pointee_struct_name(self.model, self.names, dest_ty)
                    .ok_or(EmitError::Unsupported(
                        "Subslice ref with no fat-struct destination (task d2)",
                    ))?;
                let fbase = base.fat_base.clone().ok_or(EmitError::Unsupported(
                    "Subslice of a slice without a fat base (task d2)",
                ))?;
                let data_ty = types::slice_data_ptr_type(
                    self.model,
                    self.names,
                    *elem,
                    slice_data_const(self.model, dest_ty),
                );
                let len = if from_end {
                    // section 19.4 `[from .. len - to]`.
                    format!("({fbase}.len - {to} - {effective_from})")
                } else {
                    let len_val = to
                        .checked_sub(from)
                        .ok_or(EmitError::Unsupported("Subslice with to < from (task d2)"))?;
                    format!("{len_val}")
                };
                Ok(format!(
                    "(struct {fat}){{ .data = ({data_ty})(({}) + {effective_from}), .len = {len} }}",
                    base.lvalue
                ))
            }
            _ => Err(EmitError::Unsupported(
                "Subslice ref of a non-array / non-slice base (task d2)",
            )),
        }
    }

    /// `Aggregate(kind, ops)` (section 10) for the non-enum, expression-shaped
    /// kinds (struct / tuple / union / closure compound literals + raw
    /// pointers). Array and `Enum` aggregates are handled at the assign site
    /// (boundary 6 / section 13); a `dyn` raw pointer is the vtable task.
    fn aggregate_expr(
        &self,
        kind: &AggregateKind,
        ops: &[Operand],
        dest_ty: TyId,
    ) -> Result<String, EmitError> {
        match kind {
            AggregateKind::Struct(ty) | AggregateKind::Tuple(ty) | AggregateKind::Closure(ty) => {
                self.struct_like_literal(*ty, ops)
            }
            AggregateKind::Union(ty, field) => self.union_literal(*ty, field.index(), ops),
            AggregateKind::RawPtr(pointee, _) => self.raw_ptr_aggregate(*pointee, ops, dest_ty),
            AggregateKind::Array(_) => Err(EmitError::Unsupported(
                "Array aggregate outside an assignment (task d2)",
            )),
            // Enum construction lowers at the assign site (tag + disjoint
            // payload writes), mirroring `Array`; reaching here is defensive.
            AggregateKind::Enum(_, _) => Err(EmitError::Unsupported(
                "Aggregate::Enum outside an assignment (assign-site only)",
            )),
        }
    }

    /// A struct / tuple / closure compound literal: a designated initializer
    /// over the emittable (non-ZST) fields in MIR order, each mapped to its
    /// section 14 C name (so field order is irrelevant + padding is
    /// zero-filled deterministically).
    fn struct_like_literal(&self, ty: TyId, ops: &[Operand]) -> Result<String, EmitError> {
        let agg = self.names.agg_name(ty);
        let mut inits = Vec::new();
        for (i, op) in ops.iter().enumerate() {
            if self.is_zst_operand(op) {
                continue;
            }
            let (name, fty) = layout_emit::field_c_name_ty(self.model, ty, i).ok_or(
                EmitError::Unsupported("Aggregate field of a non-aggregate type (task d2)"),
            )?;
            let e = self.operand_expr(op, Some(fty))?;
            inits.push(format!(".{name} = {e}"));
        }
        Ok(format!("(struct {agg}){{ {} }}", inits.join(", ")))
    }

    /// A union compound literal: the single active field.
    fn union_literal(
        &self,
        ty: TyId,
        field_idx: usize,
        ops: &[Operand],
    ) -> Result<String, EmitError> {
        let agg = self.names.agg_name(ty);
        let op = ops.first().ok_or(EmitError::Unsupported(
            "Union aggregate without an operand (task d2)",
        ))?;
        if self.is_zst_operand(op) {
            return Ok(format!("(union {agg}){{0}}"));
        }
        let (name, fty) = layout_emit::field_c_name_ty(self.model, ty, field_idx).ok_or(
            EmitError::Unsupported("Union field of a non-aggregate type (task d2)"),
        )?;
        let e = self.operand_expr(op, Some(fty))?;
        Ok(format!("(union {agg}){{ .{name} = {e} }}"))
    }

    /// `Aggregate(RawPtr(pointee, _), ops)`: a **slice/str** pointee builds the
    /// section 3.2 fat pointer (`{ .data, .len }`, the `.data` cast to the
    /// element pointer type); a **sized** pointee is a thin pointer whose value
    /// is the data operand (the metadata operand is unit / ZST, dropped); a
    /// `dyn` pointee is the vtable task.
    fn raw_ptr_aggregate(
        &self,
        pointee: TyId,
        ops: &[Operand],
        dest_ty: TyId,
    ) -> Result<String, EmitError> {
        let data_op = || {
            ops.first().ok_or(EmitError::Unsupported(
                "RawPtr aggregate missing the data operand (task d2)",
            ))
        };
        match &self.model.types[pointee] {
            Ty::Dynamic { .. } => Err(EmitError::Unsupported(
                "RawPtr aggregate to a dyn pointer (vtable task)",
            )),
            Ty::Slice(elem) => {
                let fat = types::unsized_pointee_struct_name(self.model, self.names, dest_ty)
                    .ok_or(EmitError::Unsupported(
                        "RawPtr aggregate with no fat-struct destination (task d2)",
                    ))?;
                let data = self.operand_expr(data_op()?, None)?;
                let len = self.operand_expr(
                    ops.get(1).ok_or(EmitError::Unsupported(
                        "RawPtr aggregate missing the metadata operand (task d2)",
                    ))?,
                    None,
                )?;
                let data_ty = types::slice_data_ptr_type(
                    self.model,
                    self.names,
                    *elem,
                    slice_data_const(self.model, dest_ty),
                );
                Ok(format!(
                    "(struct {fat}){{ .data = ({data_ty})({data}), .len = {len} }}"
                ))
            }
            Ty::Str => {
                let data = self.operand_expr(data_op()?, None)?;
                let len = self.operand_expr(
                    ops.get(1).ok_or(EmitError::Unsupported(
                        "RawPtr aggregate missing the metadata operand (task d2)",
                    ))?,
                    None,
                )?;
                Ok(format!(
                    "(struct rust_str){{ .data = (const uint8_t *)({data}), .len = {len} }}"
                ))
            }
            _ => {
                // Sized (thin) pointee: the raw pointer is the data pointer;
                // the metadata operand is unit / ZST and is dropped.
                if let Some(metadata) = ops.get(1) {
                    let _ = self.operand_expr(metadata, None)?;
                }
                self.operand_expr(data_op()?, Some(dest_ty))
            }
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
            UnOp::PtrMetadata => {
                // The section 3.2 metadata slot of a fat pointer: `.len` for a
                // slice / str; `.vtable` (a `dyn` pointer) is the vtable task; a
                // thin pointer's metadata is () (ZST), elided at the assign site.
                let pointee = self.operand_ty(a).and_then(|t| match &self.model.types[t] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => Some(*p),
                    _ => None,
                });
                match pointee.map(|p| &self.model.types[p]) {
                    Some(Ty::Slice(_) | Ty::Str) => Ok(format!("({e}).len")),
                    Some(Ty::Dynamic { .. }) => Err(EmitError::Unsupported(
                        "PtrMetadata of a dyn pointer (vtable task)",
                    )),
                    _ => Ok(String::new()),
                }
            }
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
                // The `(T, bool)` result is a multi-field write built at the
                // assign site ([`Self::assign_with_overflow`], Change G); a
                // `*WithOverflow` never appears as a nested rvalue expression,
                // so reaching here is defensive.
                Err(EmitError::Unsupported(
                    "*WithOverflow as a nested rvalue (assign-site only)",
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
            CastKind::PointerCoercion(PointerCoercion::Unsize) => self.unsize_cast(op, dest_ty),
            CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer) => Err(
                EmitError::Unsupported("PointerCoercion::ClosureFnPointer (vtable task)"),
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

    /// `PointerCoercion::Unsize` (section 11): build the section 3.2 fat
    /// pointer from a thin pointer to a sized source. The `.data` slot is cast
    /// to the **element pointer type** (`const T *` per [`types::emit_slice_struct`]),
    /// not the fat struct type; the `.len` is the source array length. An
    /// already-fat source being re-unsized is an identity copy; a `dyn`
    /// destination is the vtable task. `dest_ty` is the unsized reference type.
    fn unsize_cast(&self, op: &Operand, dest_ty: TyId) -> Result<String, EmitError> {
        let op_expr = self.operand_expr(op, None)?;
        let pointee = match &self.model.types[dest_ty] {
            Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
            _ => {
                return Err(EmitError::Unsupported(
                    "Unsize cast to a non-pointer destination (task d2)",
                ))
            }
        };
        // `Some(n)` for a thin pointer to `[T; n]`; `None` for an already-fat
        // source (a slice/str re-unsize -> identity copy).
        let src_len = self.unsize_source_len(op);
        match &self.model.types[pointee] {
            Ty::Dynamic { .. } => Err(EmitError::Unsupported(
                "Unsize to a dyn pointer (vtable task)",
            )),
            Ty::Slice(elem) => match src_len {
                Some(n) => {
                    let fat = types::unsized_pointee_struct_name(self.model, self.names, dest_ty)
                        .ok_or(EmitError::Unsupported(
                        "Unsize to a slice with no fat-struct destination (task d2)",
                    ))?;
                    let data_ty = types::slice_data_ptr_type(
                        self.model,
                        self.names,
                        *elem,
                        slice_data_const(self.model, dest_ty),
                    );
                    Ok(format!(
                        "(struct {fat}){{ .data = ({data_ty})({op_expr}), .len = {n} }}"
                    ))
                }
                None => Ok(op_expr),
            },
            Ty::Str => match src_len {
                Some(n) => Ok(format!(
                    "(struct rust_str){{ .data = (const uint8_t *)({op_expr}), .len = {n} }}"
                )),
                None => Ok(op_expr),
            },
            _ => Err(EmitError::Unsupported(
                "Unsize to a sized pointee (task d2)",
            )),
        }
    }

    /// The source array length for an `Unsize`: the `n` of a thin pointer to
    /// `[T; n]`, or `None` when the source is already a fat pointer (a
    /// slice/str re-unsize, which is an identity copy).
    fn unsize_source_len(&self, op: &Operand) -> Option<u64> {
        let ty = self.operand_ty(op)?;
        let pointee = match &self.model.types[ty] {
            Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
            _ => return None,
        };
        match &self.model.types[pointee] {
            Ty::Array(_, n) => Some(*n),
            _ => None,
        }
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
            // A by-value array argument would decay to a pointer in C, losing
            // Rust's by-value (copied) array semantics. This is a rare Rust-ABI
            // edge (boundary 6); fail loud rather than mis-emit.
            if self
                .operand_ty(arg)
                .is_some_and(|t| matches!(self.model.types[t], Ty::Array(_, _)))
            {
                return Err(EmitError::Unsupported(
                    "by-value array call argument (C array decay)",
                ));
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
/// scalar subset plus the section 13 single-member `EnumValue` literal.
/// `consumer_ty` supplies the pointer type for a `Reloc` to a static, the
/// exact pointer spelling for a `NullPtr`, and the enum type for an
/// `EnumValue`. The non-scalar forms are still deferred: `Bytes` / `Aggregate`
/// and the multi-member `EnumValue` are task d2.2.
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
        Const::Bytes(_) => Err(EmitError::Unsupported("Const::Bytes (task d2.2)")),
        Const::Aggregate(_) => Err(EmitError::Unsupported("Const::Aggregate (task d2.2)")),
        Const::EnumValue {
            variant_idx,
            payload,
        } => enum_value_expr(model, names, syms, consumer_ty, *variant_idx, payload),
    }
}

/// `Const::EnumValue` (section 13 / 12): the **single-member** cases lower to
/// one C compound literal; everything requiring two members relabels to task
/// d2.2.
///
/// - inline enum (<= 1 view): a fieldless-collapsed `Direct` / `Single {
///   scalar: Some }` tag view -> `(struct E){ .tag = <tag_value> }`; a
///   `Single { scalar: None }` / niche **untagged** payload -> the payload
///   children (ZST omitted); a niche **niched** variant -> `(struct E){ .<F>
///   = <tag_value> }` **only** when its niche cell coincides exactly with one
///   inline scalar field `F` (else task d2.2);
/// - multi-view enum: a **fieldless** `Direct` variant -> `(struct E){
///   .u._tag = { .tag = <tag_value> } }`; anything else (a payload-bearing
///   `Direct` variant -> two members; any niche-cell write in a multi-view
///   enum) -> `EmitError::Unsupported` labelled task d2.2.
///
/// A non-scalar payload child (`Bytes` / `Aggregate` / a deferred
/// `EnumValue`) **transitively defers** when [`const_expr`] recurses into it.
fn enum_value_expr(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    consumer_ty: Option<TyId>,
    variant_idx: VariantIdx,
    payload: &[ConstId],
) -> Result<String, EmitError> {
    let enum_ty = consumer_ty.ok_or(EmitError::Unsupported(
        "Const::EnumValue without a known enum type (task d2.2)",
    ))?;
    let shape = enum_emit::enum_shape(model, enum_ty).ok_or(EmitError::Unsupported(
        "Const::EnumValue with a non-enum consumer (task d2.2)",
    ))?;
    let v = variant_idx.index();
    let agg = names.agg_name(enum_ty).to_string();

    // Lower EVERY payload child through `const_expr` up front (whatever the
    // single-member shape turns out to be), so a non-scalar child -- including
    // one of an omitted ZST field -- transitively defers to its own d2.2 arm
    // before any designator is built. `kept` is the designators for the
    // non-ZST (in-view) fields; the omitted ZST fields contribute none.
    let kept = lower_enum_payload(model, names, syms, enum_ty, v, payload)?;

    if enum_emit::enum_is_inline(model, shape) {
        match enum_emit::enum_tag(model, shape) {
            // Inline tag view: all variants are fieldless (a payload would add
            // a second view), so the single view is `_tag`. The designator is
            // the shared navigator's tag member path (`tag` inline), never a
            // re-derived name.
            enum_emit::EnumTag::View {
                member_path,
                scalar_ty,
            } => enum_value_tag_literal(model, names, &agg, shape, v, &member_path, scalar_ty),
            // Inline, no tag (Single { scalar: None }): the payload children.
            enum_emit::EnumTag::None => Ok(enum_value_payload_literal(&agg, &kept)),
            // Inline niche.
            enum_emit::EnumTag::Niche { offset, scalar_ty } => {
                let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
                    "Const::EnumValue variant index out of range (task d2.2)",
                ))?;
                match variant.tag_value {
                    // A niched variant: write the niche cell, only when it
                    // coincides exactly with one inline scalar field.
                    Some(tv) => {
                        let (fname, _) =
                            niche_coinciding_field(model, enum_ty, shape, offset, scalar_ty)
                                .ok_or(EmitError::Unsupported(
                                    "Const::EnumValue (multi-member): task d2.2",
                                ))?;
                        let tvr = render_tag_value(model, names, scalar_ty, tv)?;
                        Ok(format!("(struct {agg}){{ .{fname} = {tvr} }}"))
                    }
                    // The untagged variant: the payload IS the value.
                    None => Ok(enum_value_payload_literal(&agg, &kept)),
                }
            }
        }
    } else {
        match enum_emit::enum_tag(model, shape) {
            // A multi-view `Direct` enum: only a fieldless variant (the `_tag`
            // member alone) is single-member; a payload-bearing variant needs
            // the `_tag` member and the `_v<v>` payload member -> task d2.2.
            // The designator is the navigator's nested tag member path
            // (`u._tag.tag`), a valid C99 designator-list.
            enum_emit::EnumTag::View {
                member_path,
                scalar_ty,
            } if payload_all_zst(model, shape, v) => {
                enum_value_tag_literal(model, names, &agg, shape, v, &member_path, scalar_ty)
            }
            // A payload-bearing `Direct` variant, or any niche-cell write in a
            // multi-view enum: two members.
            _ => Err(EmitError::Unsupported(
                "Const::EnumValue (multi-member): task d2.2",
            )),
        }
    }
}

/// Lower every `Const::EnumValue` payload child through [`const_expr`],
/// returning the designators for the non-ZST (in-view) fields. Children of
/// omitted (ZST / no-view) fields are still lowered -- so a non-scalar child
/// (`Bytes` / `Aggregate` / a deferred `EnumValue`) transitively defers to its
/// own d2.2 arm -- but contribute no designator (matching the view).
fn lower_enum_payload(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    enum_ty: TyId,
    v: usize,
    payload: &[ConstId],
) -> Result<Vec<String>, EmitError> {
    let mut inits = Vec::new();
    for (i, child_id) in payload.iter().enumerate() {
        let child = model.consts.resolve(*child_id);
        match enum_emit::enum_variant_field(model, enum_ty, v, i) {
            Some((member_path, fty)) => {
                let e = const_expr(model, names, syms, child, Some(fty))?;
                inits.push(format!(".{member_path} = {e}"));
            }
            None => {
                let _ = const_expr(model, names, syms, child, None)?;
            }
        }
    }
    Ok(inits)
}

/// The single-member tag designator literal for a fieldless variant `v`:
/// `(struct E){ .<tag_member_path> = <tag_value> }`, where `tag_member_path`
/// is the shared navigator's tag cell path (`tag` inline / `u._tag.tag`
/// multi-view, both valid C designator-lists) -- not a re-derived name.
fn enum_value_tag_literal(
    model: &RegseqModel,
    names: &NameTable,
    agg: &str,
    shape: &EnumShape,
    v: usize,
    tag_member_path: &str,
    scalar_ty: TyId,
) -> Result<String, EmitError> {
    let tv = variant_layout(shape, v)
        .and_then(|vl| vl.tag_value)
        .ok_or(EmitError::Unsupported(
            "Const::EnumValue (multi-member): task d2.2",
        ))?;
    let tvr = render_tag_value(model, names, scalar_ty, tv)?;
    Ok(format!("(struct {agg}){{ .{tag_member_path} = {tvr} }}"))
}

/// Assemble the payload-field compound literal from the already-lowered `kept`
/// designators (`Single { scalar: None }` or a niche untagged variant):
/// `(struct E){ .<field> = <child>, ... }`, or `{0}` when every field was a
/// ZST omitted from the view.
fn enum_value_payload_literal(agg: &str, kept: &[String]) -> String {
    if kept.is_empty() {
        format!("(struct {agg}){{0}}")
    } else {
        format!("(struct {agg}){{ {} }}", kept.join(", "))
    }
}

/// The inline view field whose niche cell coincides exactly with the niche
/// (`field.offset == TagScalar.offset` **and** `field.ty == scalar_ty`), as
/// the member path the navigator spells, or `None` when no single inline
/// scalar field coincides (the niche is a sub-byte of a nested aggregate
/// field -> task d2.2). The inline view is the niche untagged variant's
/// payload.
fn niche_coinciding_field(
    model: &RegseqModel,
    enum_ty: TyId,
    shape: &EnumShape,
    offset: u64,
    scalar_ty: TyId,
) -> Option<(String, TyId)> {
    let (untagged, variants) = match shape {
        EnumShape::Multiple {
            encoding: TagEncoding::Niche {
                untagged_variant, ..
            },
            variants,
            ..
        } => (untagged_variant.index(), variants),
        _ => return None,
    };
    let variant = variants.get(untagged)?;
    for (idx, f) in variant.fields.iter().enumerate() {
        if f.offset == offset && f.ty == scalar_ty {
            // Spell the member path through the navigator (no re-derivation).
            return enum_emit::enum_variant_field(model, enum_ty, untagged, idx);
        }
    }
    None
}

/// Whether variant `v` of `shape` is fieldless (every payload field is ZST),
/// so its construction writes only the tag.
fn payload_all_zst(model: &RegseqModel, shape: &EnumShape, v: usize) -> bool {
    match variant_layout(shape, v) {
        Some(vl) => vl.fields.iter().all(|f| layout_emit::is_zst(model, f.ty)),
        None => true,
    }
}

/// The [`VariantLayout`] for variant index `v` of `shape`, or `None` when out
/// of range (`Empty` has none; `Single` has only variant 0).
fn variant_layout(shape: &EnumShape, v: usize) -> Option<&VariantLayout> {
    match shape {
        EnumShape::Empty => None,
        EnumShape::Single { variant, .. } => (v == 0).then_some(variant),
        EnumShape::Multiple { variants, .. } => variants.get(v),
    }
}

/// Render an enum tag value of logical type `scalar_ty` (section 13 / 11.1).
/// Every admitted `TagScalar.scalar_ty` is covered: `Int` / `Uint` via the d1
/// signed / unsigned renderers; `Bool` / `Char` via their d1 const forms; a
/// `Ptr` / `Ref` tag (the niche tag *is* the pointer, e.g. `Option<&T>`) as
/// the integer bit pattern cast to the pointer's C type.
fn render_tag_value(
    model: &RegseqModel,
    names: &NameTable,
    scalar_ty: TyId,
    value: u128,
) -> Result<String, EmitError> {
    match &model.types[scalar_ty] {
        Ty::Bool => Ok(if value != 0 {
            "1".to_string()
        } else {
            "0".to_string()
        }),
        Ty::Char => Ok(format!("(uint32_t){}U", value as u32)),
        Ty::Int(kind) => Ok(render_int(*kind, value)),
        Ty::Uint(kind) => Ok(render_uint(*kind, value)),
        Ty::Ptr(_, _) | Ty::Ref(_, _) => {
            let cty = types::c_type(model, names, scalar_ty);
            Ok(format!("(({cty}){}ULL)", value as u64))
        }
        _ => Err(EmitError::Unsupported(
            "enum tag scalar of an unsupported type",
        )),
    }
}

/// Render a `Discriminant` result value per the `result_ty` integer type
/// (section 11.1 signed / unsigned literal emission).
fn render_discriminant(
    model: &RegseqModel,
    result_ty: TyId,
    value: u128,
) -> Result<String, EmitError> {
    match &model.types[result_ty] {
        Ty::Int(kind) => Ok(render_int(*kind, value)),
        Ty::Uint(kind) => Ok(render_uint(*kind, value)),
        _ => Err(EmitError::Unsupported(
            "Discriminant result of a non-integer type",
        )),
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
