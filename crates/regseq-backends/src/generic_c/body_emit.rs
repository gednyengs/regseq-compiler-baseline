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
//! `Const::EnumValue`). Task d2.2 completes the non-scalar `Const` layer: the
//! `Const::Bytes` byte-image walker, the `Const::Aggregate` reference carrier
//! and typed valtree, and the multi-member `Const::EnumValue` (the `_raw`
//! byte image). A narrow set of d2.3 carve-outs still fail loud via
//! [`EmitError::Unsupported`] (a relocation in an enum / aggregate const, an
//! inline bool-niche, a `ConstMemoryStatic` at incompatible pointee types, a
//! union byte image / valtree); `dyn` / vtable construction is the vtable
//! task. Diag-log call-site interception lowers to the section 8 `printf`.

use regseq_model::{
    AggregateKind, AssertKind, BinOp, Byte, CallCallee, CastKind, Const, ConstId, Endian,
    EnumShape, FloatKind, InstanceId, InstanceKind, InstanceRef, IntKind, IntrinsicKind, LocalId,
    LogLevel, NonDivergingIntrinsic, Operand, Place, PlaceElem, PointerCoercion, RegseqModel,
    Rvalue, StatementKind, SwitchTargets, TagEncoding, TerminatorKind, Ty, TyId, UintKind, UnOp,
    VariantIdx, VariantLayout,
};

use super::{
    enum_emit, layout_emit, naming, slice_data_const, types, EmitError, InstanceSymbols, NameTable,
    StorageTy,
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
        // The niche cell is a raw bit pattern; read it as the scalar's raw
        // storage unsigned integer (not the logical C type, which may normalize
        // -- `_Bool` -- or constrain the value), so an invalid-bit-pattern niche
        // tag (e.g. `Option<bool>`'s `None == 2`) round-trips and compares
        // correctly against `tag_value` (also a raw bit pattern, mod 2^width).
        let storage_kind = niche_storage_kind(self.model, scalar_ty)?;
        let mut expr = render_discriminant(self.model, dst_ty, untagged_disc)?;
        let tmp = self.fresh_temp_name("_nd");
        for v in variants.iter().rev() {
            if let Some(tv) = v.tag_value {
                let tvr = render_uint(storage_kind, tv);
                let d = render_discriminant(self.model, dst_ty, v.discriminant)?;
                expr = format!("({tmp} == {tvr} ? {d} : {expr})");
            }
        }
        let tmp_cty = uint_ctype(storage_kind);
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
                    // Write the raw niche bit pattern through the scalar's raw
                    // storage unsigned integer (see `assign_niche_discriminant`):
                    // `(_Bool){2}` would normalize to 1, so a `_Bool` niche needs
                    // `(uint8_t){2}`.
                    let storage_kind = niche_storage_kind(self.model, scalar_ty)?;
                    let raw_cty = uint_ctype(storage_kind);
                    let tvr = render_uint(storage_kind, tv);
                    Ok(Some(format!(
                        "memcpy((unsigned char *)&({enum_lv}) + {offset}, &({raw_cty}){{{tvr}}}, \
                         sizeof({raw_cty}));"
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

    /// [`index_elem_ty`] over the emitter's model (a thin forwarder so the
    /// `place_walk` call sites read unchanged).
    fn index_elem_ty(&self, ty: TyId, sub_array_elem: Option<TyId>) -> Result<TyId, EmitError> {
        index_elem_ty(self.model, ty, sub_array_elem)
    }

    /// The address-of expression for a Transmute source operand: `&(lvalue)`
    /// for a place, or `&(T){const}` (a gnu11 compound literal) for a
    /// constant.
    fn operand_addr(&self, op: &Operand) -> Result<String, EmitError> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => Ok(format!("&({})", self.place_lvalue(p)?)),
            Operand::Constant(id) => {
                let c = self.model.consts.resolve(*id);
                let value = const_expr(self.model, self.names, self.syms, c, None, false)?;
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
                false,
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

    /// `Call` (section 7.1): the ordinary / divergent call. A diag-log callee
    /// is intercepted here and lowered to the section 8 `printf`.
    fn call(
        &self,
        callee: &CallCallee,
        args: &[Operand],
        destination: &Place,
        target: Option<regseq_model::BlockId>,
    ) -> Result<String, EmitError> {
        // Diag-log interception (section 7.1 / section 8): a `Direct` callee
        // whose source item carries `IntrinsicKind::Log(level)` lowers to the
        // section-8 byte-faithful `fwrite` statement sequence + `goto
        // bb_<target>` instead of a C call. Recognition is keyed by instance
        // identity, so an `Indirect` callee can never satisfy it.
        if let CallCallee::Direct(inst) = callee {
            if let Some(IntrinsicKind::Log(level)) = self.model.instance_intrinsic(*inst) {
                let level = level.clone();
                return self.emit_diag_log(level, args, target);
            }
        }

        let callee_expr = match callee {
            CallCallee::Direct(inst) => self.syms.symbol(*inst).to_string(),
            CallCallee::Indirect(op) => format!("({})", self.operand_expr(op, None)?),
        };

        // The callee's parameter types, threaded as the per-argument consumer
        // type so a non-scalar / reference const argument lowers against the
        // parameter type (a scalar argument ignores it). The recovery is the
        // single helper the Change-F pre-pass also uses, so emission + scan
        // cannot disagree on which arguments carry a known consumer type; an
        // unrecoverable signature lowers arguments at `None` (a non-scalar
        // const argument then fails loud).
        let param_tys = super::callee_param_tys(self.model, self.body, callee);

        let mut arg_strs = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            // Lower (validate) every argument first so a deferred place
            // projection reached through an argument fails loud, then drop the
            // ZST arguments from the C call (a ZST value carries no bytes and
            // is omitted from the prototype's parameter list, section 15).
            let expr = self.operand_expr(arg, param_tys.get(i).copied())?;
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

    // ---- section 8 diagnostic-logging -> stdio (fwrite) ----

    /// Lower a diag-log `Call` to its section-8 byte-faithful `fwrite`
    /// statement sequence + `goto bb_<target>` (Changes A-D). `args` is the
    /// post-DiagLogPeel flat operand slice `[fmt_literal, v1, ..., vN]`
    /// (Pipeline-A, section 8.5): the format template literal followed by the
    /// value operands. `level` selects the section-8.1 bracketed tag; `target`
    /// is the (always-`Some`) unit-return successor. Every byte that reaches
    /// stdout is written via `fwrite(ptr, 1, n, stdout)` with an explicit byte
    /// count, so an interior NUL in a length-prefixed `&str` / literal is
    /// reproduced faithfully (a NUL-terminated `printf` format + `%s` would
    /// silently truncate at the first NUL).
    fn emit_diag_log(
        &self,
        level: LogLevel,
        args: &[Operand],
        target: Option<regseq_model::BlockId>,
    ) -> Result<String, EmitError> {
        // A Log callee always returns unit and never diverges, so a `None`
        // target is model corruption (section 7.1 / I-DiagLog-CallArity).
        let target = target.ok_or(EmitError::Unsupported("diag-log call with no target"))?;

        // Pipeline-A: read `args[0]` as the format template literal and
        // `args[1..]` as the flat value operands. Never reconstruct a LogArg.
        let fmt_op = args.first().ok_or(EmitError::Unsupported(
            "diag-log call with no format operand",
        ))?;
        let template = self.read_template_bytes(fmt_op)?;
        let segments = parse_template(&template)?;
        let value_ops = &args[1..];

        // I-DiagLog-CallArity: placeholder count == args.len() - 1.
        let placeholders = segments
            .iter()
            .filter(|s| matches!(s, Segment::Placeholder(_)))
            .count();
        if placeholders != value_ops.len() {
            return Err(EmitError::Unsupported(
                "diag-log placeholder/argument arity mismatch",
            ));
        }

        // section 8.1 (byte-faithful fwrite redesign): every byte that reaches
        // stdout is written by `fwrite(ptr, 1, n, stdout)` with an explicit
        // byte count, so an interior NUL in a length-prefixed `&str` / literal
        // is reproduced. Accumulate literal bytes (the leading bracketed tag,
        // the template's literal text, the trailing `\n`) into `pending` and
        // flush each run as a single literal `fwrite`; render numerics via
        // `snprintf` into a fixed stack buffer then `fwrite`; `fwrite` each
        // `&str` value's full byte range. `k` is the per-call numeric-block
        // counter (independent `_n<k>` / `_l<k>` names).
        let mut pending: Vec<u8> = Vec::new();
        pending.push(b'[');
        pending.extend_from_slice(level_tag(level).as_bytes());
        pending.extend_from_slice(b"] ");
        let mut stmts: Vec<String> = Vec::new();
        let mut k = 0u32;
        let mut value_idx = 0usize;

        for seg in &segments {
            match seg {
                Segment::Literal(bytes) => {
                    // Defer: coalesce into the running literal run.
                    pending.extend_from_slice(bytes);
                }
                Segment::Placeholder(modifier) => {
                    // Flush the literal run accumulated before this placeholder
                    // (the tag merges into the first run), then emit the value.
                    flush_literal(&mut pending, &mut stmts);
                    let op = &value_ops[value_idx];
                    value_idx += 1;

                    // section 8.3 static `&str`: an `Operand::Constant` resolving
                    // to the `&str` carrier -> the CMS bytes as a C string
                    // literal `fwrite`n with their explicit decoded byte count
                    // (no buffer, no cap, no truncation, no NUL append; an
                    // interior NUL renders `\000` and is counted as one byte --
                    // faithful). A hex modifier on a `&str` is integer-only ->
                    // Unsupported (the bridge's D2 already forbids it).
                    if let Some(bytes) = self.read_static_str_bytes(op)? {
                        require_default_modifier(*modifier)?;
                        stmts.push(format!(
                            "fwrite(\"{}\", 1, {}, stdout);",
                            c_escape(&bytes),
                            bytes.len()
                        ));
                        continue;
                    }

                    // section 8.3 non-static `&str`: any other `&str`-typed place
                    // (a runtime-borrowed / sliced `&str`) -> `fwrite` of the
                    // section 3.2 fat pointer's full byte range. The `&str`
                    // lowers to the fat struct `rust_str { const uint8_t *data;
                    // size_t len; }`; `fwrite((op).data, 1, (op).len, stdout)`
                    // writes exactly `.len` bytes (the fat pointer guarantees
                    // `.len` valid bytes -> no over-read), reproducing any
                    // interior NUL. No buffer, no cap, no truncation, no NUL.
                    if self.is_str_ref_place(op) {
                        require_default_modifier(*modifier)?;
                        let expr = self.operand_expr(op, None)?;
                        stmts.push(format!("fwrite(({expr}).data, 1, ({expr}).len, stdout);"));
                        continue;
                    }

                    // section 8.2 numeric: the reused (specifier, cast) renders
                    // into a fixed [24] stack buffer via `snprintf`, then the
                    // returned length is `fwrite`n. The buffer is provably larger
                    // than the widest admissible rendering (20 bytes: `%lld` of
                    // i64::MIN / `%llu` of u64::MAX; any hex form <= 18;
                    // Isize/Usize are non-admissible -- rejected by the whitelist
                    // -- so never reach here), so `snprintf` never truncates ->
                    // `_l<k>` is the exact, non-negative length < 24 and `fwrite`
                    // reads only in-bounds bytes (no clamp needed). The operand
                    // `Ty` comes from a place's decl or a scalar constant.
                    let ty = self
                        .diag_numeric_ty(op)
                        .ok_or(EmitError::Unsupported("diag-log argument of unknown type"))?;
                    let (spec, cast) = diag_specifier_and_cast_for(*modifier, &ty)?;
                    let expr = self.operand_expr(op, None)?;
                    stmts.push(format!(
                        "{{ char _n{k}[24]; int _l{k} = snprintf(_n{k}, sizeof _n{k}, \"{spec}\", {cast}({expr})); fwrite(_n{k}, 1, _l{k}, stdout); }}"
                    ));
                    k += 1;
                }
            }
        }
        // The trailing `\n` merges into the final literal run, which is always
        // non-empty (it holds at least the tag and/or the `\n`).
        pending.push(b'\n');
        flush_literal(&mut pending, &mut stmts);

        // No destination assignment (unit return); the successor `goto` closes
        // the interception.
        stmts.push(format!("goto bb_{};", target.as_u32()));
        Ok(stmts.join(" "))
    }

    /// Read the format-template text bytes from a diag-log `args[0]` operand
    /// (Change B part 1). Mirrors the bridge's `extract_format_template_literal`
    /// (`peel.rs`) but walks `model.consts` + `resolve_reloc` + the CMS
    /// instance's `Static { init }` (the backend cannot see the bridge cache).
    /// D6 guarantees a literal, so a bridged model always succeeds; any other
    /// shape fails loud defensively.
    fn read_template_bytes(&self, op: &Operand) -> Result<Vec<u8>, EmitError> {
        self.read_str_carrier_bytes(op)
            .ok_or(EmitError::Unsupported(
                "diag-log format operand is not a string literal",
            ))
    }

    /// Read the bytes of a *static* `&str` argument carrier, or `None` if the
    /// operand is not a static-literal carrier (then it is a non-static `&str`,
    /// section 8.3). The static-vs-non-static discriminator is purely the
    /// operand form: an `Operand::Constant` resolving to the `&str` carrier is
    /// static; anything else of `&str` type is non-static.
    fn read_static_str_bytes(&self, op: &Operand) -> Result<Option<Vec<u8>>, EmitError> {
        Ok(self.read_str_carrier_bytes(op))
    }

    /// The shared `&str` carrier byte read: an `Operand::Constant` resolving to
    /// `Aggregate([Reloc -> CMS, Uint(len)])` -> the CMS `Const::Bytes` ->
    /// `bytes[0..len]`'s concrete `Byte::Value`s. `None` for any other shape.
    fn read_str_carrier_bytes(&self, op: &Operand) -> Option<Vec<u8>> {
        let cid = match op {
            Operand::Constant(cid) => *cid,
            _ => return None,
        };
        let (reloc_cid, len_cid) = match self.model.consts.resolve(cid) {
            Const::Aggregate(parts) if parts.len() == 2 => (parts[0], parts[1]),
            _ => return None,
        };
        let target = match self.model.consts.resolve(reloc_cid) {
            Const::Reloc(r) => *r,
            _ => return None,
        };
        let len = match self.model.consts.resolve(len_cid) {
            Const::Uint(_, v) => *v as usize,
            _ => return None,
        };
        let cms_id = resolve_reloc(self.model, target)?;
        let bytes_cid = match &self.model.instances[cms_id].kind {
            InstanceKind::Static { init: Some(c), .. } => *c,
            _ => return None,
        };
        let raw = match self.model.consts.resolve(bytes_cid) {
            Const::Bytes(bytes) => bytes,
            _ => return None,
        };
        // The carrier metadata `len` must not exceed the CMS byte image: reading
        // `bytes[0..len]` out of bounds is model corruption (a bridged carrier
        // never does this). Fail loud rather than silently read fewer bytes.
        if len > raw.len() {
            return None;
        }
        let mut out = Vec::with_capacity(len);
        for b in &raw[..len] {
            match b {
                Byte::Value(v) => out.push(*v),
                // A relocation or uninit byte inside a string literal is not a
                // literal text byte: a bridged template never carries one.
                Byte::Reloc(_) | Byte::Uninit => return None,
            }
        }
        Some(out)
    }

    /// Whether the operand is a place (`Copy` / `Move`) of `&str` type -- the
    /// section-8.3 non-static `&str` form (a static-literal carrier is an
    /// `Operand::Constant`, handled separately). The discriminator is purely
    /// the operand form, no provenance tracing.
    fn is_str_ref_place(&self, op: &Operand) -> bool {
        let place = match op {
            Operand::Copy(p) | Operand::Move(p) => p,
            Operand::Constant(_) => return false,
        };
        matches!(&self.model.types[self.place_ty(place)], Ty::Ref(pointee, _) if matches!(self.model.types[*pointee], Ty::Str))
    }

    /// The `Ty` of a numeric diag-log argument: a place's declared type, or a
    /// scalar constant's variant (`Uint` / `Int` / `Bool`). `None` for any
    /// other operand shape (the caller then fails loud). Returns an owned `Ty`
    /// so a constant -- which carries its kind, not a `TyId` -- maps uniformly
    /// with a place into [`diag_specifier_and_cast_for`].
    fn diag_numeric_ty(&self, op: &Operand) -> Option<Ty> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => Some(self.model.types[self.place_ty(p)].clone()),
            Operand::Constant(cid) => match self.model.consts.resolve(*cid) {
                Const::Uint(kind, _) => Some(Ty::Uint(*kind)),
                Const::Int(kind, _) => Some(Ty::Int(*kind)),
                Const::Bool(_) => Some(Ty::Bool),
                _ => None,
            },
        }
    }
}

/// The element type reached by an `Index` / `ConstantIndex`: the tracked
/// sub-array element (from a prior array `Subslice`, boundary 7) if set, else
/// the `Array` / `Slice` element of the running type. Shared by
/// [`BodyEmit::place_walk`] and the type-only [`place_ty`].
fn index_elem_ty(
    model: &RegseqModel,
    ty: TyId,
    sub_array_elem: Option<TyId>,
) -> Result<TyId, EmitError> {
    if let Some(elem) = sub_array_elem {
        return Ok(elem);
    }
    match &model.types[ty] {
        Ty::Array(elem, _) | Ty::Slice(elem) => Ok(*elem),
        _ => Err(EmitError::Unsupported(
            "Index of a non-array / non-slice place (task d2)",
        )),
    }
}

/// The section 19.4 narrowed type of a place -- **type only** (no lvalue, no
/// `NameTable` / `InstanceSymbols`). The single source of truth for a place's
/// running type, consumed by **both** the body emitter's
/// [`BodyEmit::place_walk`] (which lowers an assigned const against exactly
/// this type) and the task-d2.2 Change-F `ConstMemoryStatic` scan
/// (`super::const_memory_static_storage`). Mirrors `place_walk`'s type arms
/// exactly so the scan cannot drift from what the emitter lowers a const
/// against -- a drift would re-introduce the mismatched-type
/// `ConstMemoryStatic` cast Change F removes (the soundness bug the
/// `place_consumer_ty` shortcut caused). The two are pinned by the regression
/// fixtures that assign a CMS-backed reference const through a non-`Field`
/// projection.
pub(super) fn place_ty(
    model: &RegseqModel,
    body: &regseq_model::Body,
    place: &Place,
) -> Result<TyId, EmitError> {
    let mut ty = body.local_decls[place.local].ty;
    let mut sub_array_elem: Option<TyId> = None;
    let mut pending_variant = false;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                // Thin / fat / dyn all narrow to the pointee; a non-pointer
                // (malformed) leaves `ty` unchanged, mirroring `place_walk`.
                ty = match &model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => ty,
                };
                sub_array_elem = None;
            }
            PlaceElem::Field(_, field_ty) => {
                ty = *field_ty;
                sub_array_elem = None;
                pending_variant = false;
            }
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                ty = index_elem_ty(model, ty, sub_array_elem.take())?;
            }
            PlaceElem::Subslice { from_end, .. } => {
                if *from_end {
                    return Err(EmitError::Unsupported(
                        "Subslice from_end as a place lvalue (task d2)",
                    ));
                }
                let elem = match &model.types[ty] {
                    Ty::Array(elem, _) => *elem,
                    _ => {
                        return Err(EmitError::Unsupported(
                            "Subslice of a non-array place lvalue (task d2)",
                        ))
                    }
                };
                sub_array_elem = Some(elem);
            }
            PlaceElem::Downcast(_) => {
                pending_variant = true;
            }
        }
    }
    if pending_variant {
        return Err(EmitError::Unsupported(
            "Downcast not followed by a Field (malformed place)",
        ));
    }
    Ok(ty)
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
    // A static initializer is an init-list position (`T X = <here>;`), so an
    // array reconstruction omits the compound-literal cast (C forbids
    // initializing an array object from a compound-literal expression).
    const_expr(model, names, syms, c, Some(consumer_ty), true)
}

/// Reconstruct the initializer for a `ConstMemoryStatic` re-typed to its
/// consumer pointee `storage` (d2.2 Change F). `c` is the bridge's
/// `Const::Bytes` byte image; the value is reconstructed at the storage shape
/// (`Sized` walks the pointee type, `Array` walks `count` element slices), so
/// the emitted definition matches the re-typed declaration. Always an
/// init-list position.
pub(super) fn const_memory_static_initializer(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    c: &Const,
    storage: &StorageTy,
) -> Result<String, EmitError> {
    let Const::Bytes(bytes) = c else {
        return Err(EmitError::Unsupported(
            "ConstMemoryStatic initializer is not a byte image",
        ));
    };
    match storage {
        StorageTy::Sized(ty) => bytes_to_typed_literal(model, names, syms, bytes, *ty, true),
        StorageTy::Array { elem, count } => {
            let esize = model.size_align_of(*elem).size as usize;
            let mut parts = Vec::with_capacity(*count as usize);
            for i in 0..*count as usize {
                let lo = i * esize;
                let hi = lo + esize;
                if hi > bytes.len() {
                    return Err(EmitError::Unsupported(
                        "ConstMemoryStatic byte image shorter than its element array",
                    ));
                }
                parts.push(bytes_to_typed_literal(
                    model,
                    names,
                    syms,
                    &bytes[lo..hi],
                    *elem,
                    true,
                )?);
            }
            Ok(format!("{{ {} }}", parts.join(", ")))
        }
    }
}

/// `Const` -> C value expression / initializer (section 12). `consumer_ty`
/// supplies the pointer type for a `Reloc` to a static, the exact pointer
/// spelling for a `NullPtr`, the enum type for an `EnumValue`, and -- task
/// d2.2 -- the consumer `Ty` the non-scalar forms reconstruct against:
/// `Bytes` (the byte-image walker, Change B), `Aggregate` (the reference
/// carrier / typed valtree, Changes C/D), and multi-member `EnumValue` (the
/// `_raw` byte image, Change E). `at_init` is set at an init-list position (a
/// static initializer or a recursed aggregate member); it controls whether an
/// array reconstruction omits the compound-literal cast (C forbids
/// initializing an array object from a compound-literal expression).
fn const_expr(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    c: &Const,
    consumer_ty: Option<TyId>,
    at_init: bool,
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
        Const::Bytes(bytes) => {
            let ty = consumer_ty.ok_or(EmitError::Unsupported(
                "Const::Bytes without a known consumer type",
            ))?;
            bytes_to_typed_literal(model, names, syms, bytes, ty, at_init)
        }
        Const::Aggregate(children) => {
            aggregate_const_expr(model, names, syms, children, consumer_ty, at_init)
        }
        Const::EnumValue {
            variant_idx,
            payload,
        } => enum_value_expr(model, names, syms, consumer_ty, *variant_idx, payload),
    }
}

// -- d2.2 Change B: the `Const::Bytes` byte-image walker ---------------

/// Reconstruct a `Const::Bytes` byte image as a **typed** C initializer for
/// the consumer `Ty` (d2.2 Change B). Dispatches on `model.types[ty]`:
/// scalars decode `size_of(ty)` bytes per `model.target.endian`; pointers
/// match a complete `pointer_size` `Byte::Reloc` run / all-zero null;
/// `Tuple` / `Struct` / `Array` slice per `FieldLayout.offset` (source order)
/// and recurse; a `Ty::Union` byte image cannot be soundly reproduced (a
/// padded spanning member is not byte-preserving) -> task d2.3; an `Enum`
/// initializes the multi-view `_raw` member (no `Reloc`) or reconstructs the
/// inline fields. `at_init` controls the array cast omission. The walk is
/// purely positional + layout-driven; no tag / discriminant re-derivation.
pub(super) fn bytes_to_typed_literal(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    consumer_ty: TyId,
    at_init: bool,
) -> Result<String, EmitError> {
    match &model.types[consumer_ty] {
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) => {
            scalar_from_bytes(model, bytes, consumer_ty)
        }
        Ty::Ptr(_, _) | Ty::Ref(_, _) | Ty::FnPtr(_) => {
            pointer_from_bytes(model, names, syms, bytes, consumer_ty)
        }
        Ty::Tuple { .. } | Ty::Struct { .. } | Ty::Closure(_, _) => {
            aggregate_from_bytes(model, names, syms, bytes, consumer_ty)
        }
        Ty::Array(elem, n) => array_from_bytes(model, names, syms, bytes, *elem, *n, at_init),
        Ty::Union { .. } => Err(EmitError::Unsupported("Const union byte-image: task d2.3")),
        Ty::Enum { .. } => enum_from_bytes(model, names, syms, bytes, consumer_ty),
        _ => Err(EmitError::Unsupported(
            "Const::Bytes for an unsupported consumer type",
        )),
    }
}

/// Decode a scalar consumer from its `size_of` byte slice (per target endian)
/// into the d1 renderer (`Bytes` `Uninit` -> 0; a `Reloc` byte where a scalar
/// is expected is malformed).
fn scalar_from_bytes(model: &RegseqModel, bytes: &[Byte], ty: TyId) -> Result<String, EmitError> {
    let size = model.size_align_of(ty).size as usize;
    let bits = decode_scalar_bits(bytes, size, model.target.endian)?;
    match &model.types[ty] {
        Ty::Bool => Ok(if bits != 0 { "1" } else { "0" }.to_string()),
        Ty::Char => Ok(format!("(uint32_t){}U", bits as u32)),
        Ty::Int(kind) => Ok(render_int(*kind, bits)),
        Ty::Uint(kind) => Ok(render_uint(*kind, bits)),
        Ty::Float(kind) => Ok(render_float(*kind, bits)),
        _ => Err(EmitError::Unsupported(
            "scalar byte image over a non-scalar type",
        )),
    }
}

/// Assemble the `size`-byte scalar bit pattern from `bytes` per `endian`
/// (`Uninit` -> 0; a `Reloc` byte is malformed for a scalar).
fn decode_scalar_bits(bytes: &[Byte], size: usize, endian: Endian) -> Result<u128, EmitError> {
    if bytes.len() < size {
        return Err(EmitError::Unsupported(
            "byte image shorter than the scalar width",
        ));
    }
    let mut val: u128 = 0;
    for (i, b) in bytes.iter().take(size).enumerate() {
        let byte = match b {
            Byte::Value(v) => *v,
            Byte::Uninit => 0,
            Byte::Reloc(_) => {
                return Err(EmitError::Unsupported(
                    "relocation byte where a scalar value was expected",
                ))
            }
        };
        let shift = match endian {
            Endian::Little => 8 * i,
            Endian::Big => 8 * (size - 1 - i),
        };
        val |= u128::from(byte) << shift;
    }
    Ok(val)
}

/// Reconstruct a pointer consumer from its `pointer_size` byte slice: a
/// complete `Byte::Reloc(r)` run -> the d1 `&<target>` cast; all-zero (no
/// `Reloc`) -> the typed null pointer; anything else (mixed / partial reloc)
/// is malformed. A fat (unsized-pointee) reference is not a single pointer
/// slot, so its all-zero case is not a valid typed-null cast -> fail loud.
fn pointer_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    ty: TyId,
) -> Result<String, EmitError> {
    let size = model.size_align_of(ty).size as usize;
    if bytes.len() < size {
        return Err(EmitError::Unsupported(
            "byte image shorter than the pointer width",
        ));
    }
    if let Byte::Reloc(r) = bytes[0] {
        if bytes[..size].iter().all(|b| *b == Byte::Reloc(r)) {
            return Ok(reloc_designator(model, names, syms, r, Some(ty)));
        }
        return Err(EmitError::Unsupported(
            "partial pointer relocation in a byte image",
        ));
    }
    let all_zero = bytes[..size]
        .iter()
        .all(|b| matches!(b, Byte::Value(0) | Byte::Uninit));
    if all_zero && types::unsized_pointee_struct_name(model, names, ty).is_none() {
        return Ok(format!("(({})0)", types::c_type(model, names, ty)));
    }
    Err(EmitError::Unsupported(
        "non-null non-relocation pointer byte image",
    ))
}

/// Reconstruct a `Tuple` / `Struct` / `Closure` consumer from its byte image
/// (section 14): walk `FieldLayout` in source / MIR order, slice each non-ZST
/// field at its `offset`, recurse, and designate by the field's C name
/// ([`layout_emit::field_c_name_ty`]). ZST fields contribute no designator.
/// Emitted with the compound-literal cast (valid in both value + init
/// positions; nested array members brace-init via `at_init = true`).
fn aggregate_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    ty: TyId,
) -> Result<String, EmitError> {
    let fields = agg_field_layouts(model, ty).ok_or(EmitError::Unsupported(
        "byte image over an unresolved aggregate",
    ))?;
    let mut parts = Vec::new();
    for (idx, f) in fields.iter().enumerate() {
        if layout_emit::is_zst(model, f.ty) {
            continue;
        }
        let (fname, fty) = layout_emit::field_c_name_ty(model, ty, idx).ok_or(
            EmitError::Unsupported("aggregate field navigation failed in a byte image"),
        )?;
        let fsize = model.size_align_of(fty).size as usize;
        let off = f.offset as usize;
        if off + fsize > bytes.len() {
            return Err(EmitError::Unsupported(
                "aggregate field slice exceeds the byte image",
            ));
        }
        let recon =
            bytes_to_typed_literal(model, names, syms, &bytes[off..off + fsize], fty, true)?;
        parts.push(format!(".{fname} = {recon}"));
    }
    let agg = names.agg_name(ty);
    if parts.is_empty() {
        Ok(format!("(struct {agg}){{0}}"))
    } else {
        Ok(format!("(struct {agg}){{ {} }}", parts.join(", ")))
    }
}

/// The source / MIR-order `FieldLayout`s of a `Tuple` / `Struct` / `Closure`
/// (the same lookup [`layout_emit`] uses; a closure resolves through its
/// parallel capture `Ty::Struct`). `None` for any other type.
fn agg_field_layouts(model: &RegseqModel, ty: TyId) -> Option<&[regseq_model::FieldLayout]> {
    use regseq_model::LayoutKind;
    let layout = match &model.types[ty] {
        Ty::Struct { layout, .. } | Ty::Tuple { layout, .. } => layout,
        Ty::Closure(item, args) => {
            model
                .types
                .iter()
                .find_map(|(_, candidate)| match candidate {
                    Ty::Struct {
                        item: s_item,
                        args: s_args,
                        layout,
                    } if s_item == item && s_args == args => Some(layout),
                    _ => None,
                })?
        }
        _ => return None,
    };
    match &layout.kind {
        LayoutKind::Struct { fields } => Some(fields),
        _ => None,
    }
}

/// Reconstruct an `Array(elem, n)` consumer from its byte image: `n` element
/// slices of `size_of(elem)` bytes, each recursed at `elem`. The array
/// type-name is the **declarator-aware** `types::declare(elem, "[n]")` (valid
/// for an array-of-array / fn-pointer element); the compound-literal cast is
/// omitted at an init-list position (`at_init`), since C forbids initializing
/// an array object from a compound-literal expression.
fn array_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    elem: TyId,
    n: u64,
    at_init: bool,
) -> Result<String, EmitError> {
    let esize = model.size_align_of(elem).size as usize;
    let mut parts = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let lo = i * esize;
        let hi = lo + esize;
        if hi > bytes.len() {
            return Err(EmitError::Unsupported(
                "array element slice exceeds the byte image",
            ));
        }
        parts.push(bytes_to_typed_literal(
            model,
            names,
            syms,
            &bytes[lo..hi],
            elem,
            true,
        )?);
    }
    let inner = if parts.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{ {} }}", parts.join(", "))
    };
    if at_init {
        Ok(inner)
    } else {
        let tyname = types::declare(model, names, elem, &format!("[{n}]"));
        Ok(format!("({tyname}){inner}"))
    }
}

/// Reconstruct an `Enum` consumer from its byte image: a multi-view enum
/// initializes the section 13 `_raw` union member directly from the bytes
/// (sound iff the bytes carry no `Byte::Reloc` -- a reloc cannot be written
/// into `_raw`); an inline enum reconstructs its inline view fields.
fn enum_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    enum_ty: TyId,
) -> Result<String, EmitError> {
    let shape = enum_emit::enum_shape(model, enum_ty).ok_or(EmitError::Unsupported(
        "byte image over a non-enum consumer",
    ))?;
    if enum_emit::enum_is_inline(model, shape) {
        return reconstruct_inline_enum_from_bytes(model, names, syms, bytes, enum_ty, shape);
    }
    if bytes.iter().any(|b| matches!(b, Byte::Reloc(_))) {
        return Err(EmitError::Unsupported(
            "Const enum/aggregate with relocation payload: task d2.3",
        ));
    }
    let raw = enum_emit::enum_raw_member(model, shape).ok_or(EmitError::Unsupported(
        "multi-view enum byte image without a _raw union member",
    ))?;
    let agg = names.agg_name(enum_ty);
    Ok(format!(
        "(struct {agg}){{ .{raw} = {} }}",
        raw_byte_image(bytes)?
    ))
}

/// Render a raw byte image as an `unsigned char[]` initializer (`{ b0, ..,
/// bN-1 }`, `Uninit` -> 0). A `Reloc` byte cannot be written into `_raw`.
fn raw_byte_image(bytes: &[Byte]) -> Result<String, EmitError> {
    let mut parts = Vec::with_capacity(bytes.len());
    for b in bytes {
        match b {
            Byte::Value(v) => parts.push(v.to_string()),
            Byte::Uninit => parts.push("0".to_string()),
            Byte::Reloc(_) => {
                return Err(EmitError::Unsupported(
                    "Const enum/aggregate with relocation payload: task d2.3",
                ))
            }
        }
    }
    Ok(format!("{{ {} }}", parts.join(", ")))
}

/// Reconstruct an inline (<= 1 view, no `union`) enum from its byte image: a
/// tag-only view decodes the tag cell; a niche / single-variant payload view
/// reconstructs the untagged variant's inline fields (byte-faithful for both
/// the untagged and niched variants, since the niche is encoded in the payload
/// bytes). An inline bool-niche cannot round-trip a raw bit pattern through a
/// `_Bool` field -> task d2.3.
fn reconstruct_inline_enum_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    enum_ty: TyId,
    shape: &EnumShape,
) -> Result<String, EmitError> {
    let agg = names.agg_name(enum_ty);
    match enum_emit::enum_tag(model, shape) {
        enum_emit::EnumTag::View {
            member_path,
            scalar_ty,
        } => {
            let offset = direct_tag_offset(shape).ok_or(EmitError::Unsupported(
                "inline tag-view enum without a tag offset",
            ))? as usize;
            let size = model.size_align_of(scalar_ty).size as usize;
            if offset + size > bytes.len() {
                return Err(EmitError::Unsupported(
                    "inline enum tag slice exceeds the byte image",
                ));
            }
            let recon = scalar_from_bytes(model, &bytes[offset..offset + size], scalar_ty)?;
            Ok(format!("(struct {agg}){{ .{member_path} = {recon} }}"))
        }
        enum_emit::EnumTag::Niche { scalar_ty, .. } => {
            if matches!(model.types[scalar_ty], Ty::Bool) {
                return Err(EmitError::Unsupported(
                    "Const::EnumValue (inline bool-niche): task d2.3",
                ));
            }
            reconstruct_inline_variant_fields(model, names, syms, bytes, enum_ty, shape)
        }
        enum_emit::EnumTag::None => {
            reconstruct_inline_variant_fields(model, names, syms, bytes, enum_ty, shape)
        }
    }
}

/// Reconstruct the inline payload-bearing variant's non-ZST fields from a byte
/// image (a `Single { scalar: None }` variant, or a niche untagged variant),
/// designating by the navigator's member path. The byte image already encodes
/// the active variant, so this is byte-faithful for niched variants too.
fn reconstruct_inline_variant_fields(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    enum_ty: TyId,
    shape: &EnumShape,
) -> Result<String, EmitError> {
    let agg = names.agg_name(enum_ty);
    let v = inline_payload_variant(shape).ok_or(EmitError::Unsupported(
        "inline enum byte image without a payload variant",
    ))?;
    let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
        "inline enum payload variant out of range",
    ))?;
    let mut parts = Vec::new();
    for (idx, f) in variant.fields.iter().enumerate() {
        if layout_emit::is_zst(model, f.ty) {
            continue;
        }
        let (member_path, fty) = enum_emit::enum_variant_field(model, enum_ty, v, idx).ok_or(
            EmitError::Unsupported("inline enum field navigation failed in a byte image"),
        )?;
        let fsize = model.size_align_of(fty).size as usize;
        let off = f.offset as usize;
        if off + fsize > bytes.len() {
            return Err(EmitError::Unsupported(
                "inline enum field slice exceeds the byte image",
            ));
        }
        let recon =
            bytes_to_typed_literal(model, names, syms, &bytes[off..off + fsize], fty, true)?;
        parts.push(format!(".{member_path} = {recon}"));
    }
    if parts.is_empty() {
        Ok(format!("(struct {agg}){{0}}"))
    } else {
        Ok(format!("(struct {agg}){{ {} }}", parts.join(", ")))
    }
}

/// The variant index whose inline payload view an inline enum collapses to: a
/// `Single`'s only variant, or a niche enum's untagged variant. `None` for a
/// shape with no inline payload view.
fn inline_payload_variant(shape: &EnumShape) -> Option<usize> {
    match shape {
        EnumShape::Single { .. } => Some(0),
        EnumShape::Multiple {
            encoding: TagEncoding::Niche {
                untagged_variant, ..
            },
            ..
        } => Some(untagged_variant.index()),
        _ => None,
    }
}

// -- d2.2 Changes C / D: `Const::Aggregate` ----------------------------

/// Lower a `Const::Aggregate` (d2.2 Changes C / D): the `[Reloc,
/// Uint(meta)]` **reference carrier** when the consumer is `Ty::Ref(..)`, else
/// the typed valtree aggregate over a `Tuple` / `Struct` / `Array`.
fn aggregate_const_expr(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    children: &[ConstId],
    consumer_ty: Option<TyId>,
    at_init: bool,
) -> Result<String, EmitError> {
    let consumer = consumer_ty.ok_or(EmitError::Unsupported(
        "Const::Aggregate without a known consumer type",
    ))?;
    if let Ty::Ref(pointee, _) = &model.types[consumer] {
        let pointee = *pointee;
        reference_carrier(model, names, syms, children, consumer, pointee)
    } else {
        typed_valtree_aggregate(model, names, syms, children, consumer, at_init)
    }
}

/// The `Const::Aggregate([Reloc, Uint(meta)])` reference carrier (d2.2
/// Change C) for a `Ty::Ref(pointee, _)` consumer: a sized pointee -> the d1
/// reloc designator cast to the consumer pointer type (the `Uint` placeholder
/// dropped); `Ty::Str` / `Ty::Slice(elem)` -> the section 3.2 fat struct with
/// `.data` cast via [`types::slice_data_ptr_type`] (the mutability /
/// `UnsafeCell`-correct spelling) and `.len = (size_t)<meta>`; `Ty::Dynamic`
/// -> the vtable task. The carrier shape is enforced defensively.
fn reference_carrier(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    children: &[ConstId],
    consumer_ty: TyId,
    pointee: TyId,
) -> Result<String, EmitError> {
    // The carrier shape is exactly `[Reloc(_), Uint(Usize, meta)]` (the bridge
    // contract, const_.rs:390-405); any other shape is malformed.
    if children.len() != 2 {
        return Err(EmitError::Unsupported(
            "Const::Aggregate reference carrier with an unexpected child count",
        ));
    }
    let Const::Reloc(target) = model.consts.resolve(children[0]) else {
        return Err(EmitError::Unsupported(
            "Const::Aggregate reference carrier without a leading relocation",
        ));
    };
    let target = *target;
    let Const::Uint(UintKind::Usize, meta) = model.consts.resolve(children[1]) else {
        return Err(EmitError::Unsupported(
            "Const::Aggregate reference carrier metadata is not a usize",
        ));
    };
    // The fat-carrier `.len` (`size_t`); dropped for a thin (sized) carrier.
    let len_expr = render_uint(UintKind::Usize, *meta);
    match &model.types[pointee] {
        Ty::Dynamic { .. } => Err(EmitError::Unsupported(
            "Const::Aggregate reference carrier to a dyn pointee: vtable task",
        )),
        Ty::Str => {
            let fat = types::unsized_pointee_struct_name(model, names, consumer_ty).ok_or(
                EmitError::Unsupported("&str reference carrier without a fat struct name"),
            )?;
            let data = reloc_designator(model, names, syms, target, None);
            // `rust_str.data` is monomorphic `const uint8_t *` (bank 1).
            Ok(format!(
                "(struct {fat}){{ .data = (const uint8_t *){data}, .len = {len_expr} }}"
            ))
        }
        Ty::Slice(elem) => {
            let elem = *elem;
            let fat = types::unsized_pointee_struct_name(model, names, consumer_ty).ok_or(
                EmitError::Unsupported("&[T] reference carrier without a fat struct name"),
            )?;
            let const_data = slice_data_const(model, consumer_ty);
            let data_ty = types::slice_data_ptr_type(model, names, elem, const_data);
            let data = reloc_designator(model, names, syms, target, None);
            Ok(format!(
                "(struct {fat}){{ .data = ({data_ty}){data}, .len = {len_expr} }}"
            ))
        }
        // A sized pointee is a thin reference: the reloc cast to the consumer
        // pointer type. The `Uint` placeholder is dropped (already validated).
        _ => Ok(reloc_designator(
            model,
            names,
            syms,
            target,
            Some(consumer_ty),
        )),
    }
}

/// The typed valtree `Const::Aggregate` (d2.2 Change D) over a non-`Ref`
/// consumer: a `Tuple` / `Struct` / `Closure` pairs `children[i]` with
/// `FieldLayout[i]` (source order), designating by the field's C name; a
/// non-`u8` `Array(elem, n)` lowers each child at `elem` (the declarator-aware
/// array type-name, cast omitted at an init position). A `Ty::Union` consumer
/// carries no active `FieldIdx` -> task d2.3; an enum consumer is defensively
/// rejected (the bridge uses `EnumValue` for enums). ZST children are recursed
/// (fail-loud) but omitted from designators.
fn typed_valtree_aggregate(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    children: &[ConstId],
    consumer_ty: TyId,
    at_init: bool,
) -> Result<String, EmitError> {
    match &model.types[consumer_ty] {
        Ty::Tuple { .. } | Ty::Struct { .. } | Ty::Closure(_, _) => {
            let fields = agg_field_layouts(model, consumer_ty).ok_or(EmitError::Unsupported(
                "Const::Aggregate over an unresolved aggregate",
            ))?;
            if children.len() != fields.len() {
                return Err(EmitError::Unsupported(
                    "Const::Aggregate child count does not match the field count",
                ));
            }
            let mut parts = Vec::new();
            for (idx, child_id) in children.iter().enumerate() {
                let child = model.consts.resolve(*child_id);
                let (fname, fty) = layout_emit::field_c_name_ty(model, consumer_ty, idx).ok_or(
                    EmitError::Unsupported("Const::Aggregate field navigation failed"),
                )?;
                // A ZST child is still lowered (so a deferred shape fails loud)
                // but contributes no designator.
                if layout_emit::is_zst(model, fty) {
                    let _ = const_expr(model, names, syms, child, Some(fty), true)?;
                    continue;
                }
                let e = const_expr(model, names, syms, child, Some(fty), true)?;
                parts.push(format!(".{fname} = {e}"));
            }
            let agg = names.agg_name(consumer_ty);
            if parts.is_empty() {
                Ok(format!("(struct {agg}){{0}}"))
            } else {
                Ok(format!("(struct {agg}){{ {} }}", parts.join(", ")))
            }
        }
        Ty::Array(elem, n) => {
            let elem = *elem;
            let n = *n;
            if children.len() != n as usize {
                return Err(EmitError::Unsupported(
                    "Const::Aggregate array child count does not match the length",
                ));
            }
            let mut parts = Vec::with_capacity(n as usize);
            for child_id in children {
                let child = model.consts.resolve(*child_id);
                parts.push(const_expr(model, names, syms, child, Some(elem), true)?);
            }
            let inner = if parts.is_empty() {
                "{0}".to_string()
            } else {
                format!("{{ {} }}", parts.join(", "))
            };
            if at_init {
                Ok(inner)
            } else {
                let tyname = types::declare(model, names, elem, &format!("[{n}]"));
                Ok(format!("({tyname}){inner}"))
            }
        }
        Ty::Union { .. } => Err(EmitError::Unsupported(
            "Const::Aggregate over a union (no active field): task d2.3",
        )),
        Ty::Enum { .. } => Err(EmitError::Unsupported(
            "Const::Aggregate consumed by an enum (the bridge uses EnumValue)",
        )),
        _ => Err(EmitError::Unsupported(
            "Const::Aggregate for an unsupported consumer type",
        )),
    }
}

/// `Const::EnumValue` (section 13 / 12): lower a value-position enum const to
/// one C compound literal (d2.2 Change E completes the multi-member
/// cases d2.1 handed off).
///
/// - inline enum (<= 1 view, no `union`): a fieldless-collapsed `Direct` /
///   `Single { scalar: Some }` tag view -> `(struct E){ .tag = <tag_value> }`;
///   a `Single { scalar: None }` / niche **untagged** payload -> the payload
///   children (ZST omitted); a niche **niched** variant -> `(struct E){ .<F> =
///   <tag_value> }` when its niche cell coincides with one inline scalar field
///   `F`. An inline bool-niche / a niche cell inside a nested aggregate cannot
///   round-trip a raw bit pattern through a const literal -> task d2.3.
/// - multi-view enum (a `union u` with the Change-A `_raw` member): a
///   **fieldless** `Direct` variant stays the d2.1 single tag-member init
///   (`(struct E){ .u._tag.tag = <tv> }`); a fully byte-serializable value (a
///   payload-bearing `Direct` variant with a scalar payload, or any niched /
///   untagged variant -- including a **bool-niche**) serializes the tag +
///   payload to a `layout.size` byte image and initializes the union via
///   `(struct E){ .u._raw = { ... } }`. A pointer / `Reloc` / non-scalar
///   payload cannot be written into `_raw` -> task d2.3.
fn enum_value_expr(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    consumer_ty: Option<TyId>,
    variant_idx: VariantIdx,
    payload: &[ConstId],
) -> Result<String, EmitError> {
    let enum_ty = consumer_ty.ok_or(EmitError::Unsupported(
        "Const::EnumValue without a known enum type",
    ))?;
    let shape = enum_emit::enum_shape(model, enum_ty).ok_or(EmitError::Unsupported(
        "Const::EnumValue with a non-enum consumer",
    ))?;
    let v = variant_idx.index();
    let agg = names.agg_name(enum_ty).to_string();

    if enum_emit::enum_is_inline(model, shape) {
        // Inline: lower every payload child through `const_expr` up front, so a
        // non-scalar child (including one of an omitted ZST field) transitively
        // defers before any designator is built. `kept` holds the designators
        // for the non-ZST (in-view) fields.
        let kept = lower_enum_payload(model, names, syms, enum_ty, v, payload)?;
        match enum_emit::enum_tag(model, shape) {
            // Inline tag view: all variants are fieldless, so the single view is
            // `_tag`. The designator is the shared navigator's tag member path.
            enum_emit::EnumTag::View {
                member_path,
                scalar_ty,
            } => enum_value_tag_literal(model, names, &agg, shape, v, &member_path, scalar_ty),
            // Inline, no tag (Single { scalar: None }): the payload children.
            enum_emit::EnumTag::None => Ok(enum_value_payload_literal(&agg, &kept)),
            // Inline niche.
            enum_emit::EnumTag::Niche { offset, scalar_ty } => {
                let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
                    "Const::EnumValue variant index out of range",
                ))?;
                match variant.tag_value {
                    // A niched variant: write the niche cell, only when it
                    // coincides exactly with one inline scalar field (an inline
                    // enum has no `_raw` to write a byte image through).
                    Some(tv) => {
                        match niche_coinciding_field(model, enum_ty, shape, offset, scalar_ty) {
                            Some((fname, _)) => {
                                let tvr = render_tag_value(model, names, scalar_ty, tv)?;
                                Ok(format!("(struct {agg}){{ .{fname} = {tvr} }}"))
                            }
                            None if matches!(model.types[scalar_ty], Ty::Bool) => {
                                Err(EmitError::Unsupported(
                                    "Const::EnumValue (inline bool-niche): task d2.3",
                                ))
                            }
                            None => Err(EmitError::Unsupported(
                                "Const::EnumValue (inline niche cell not a top-level field): task d2.3",
                            )),
                        }
                    }
                    // The untagged variant: the payload IS the value.
                    None => Ok(enum_value_payload_literal(&agg, &kept)),
                }
            }
        }
    } else {
        // A fieldless `Direct` variant of a multi-view enum stays a single
        // tag-member init (the d2.1 path; one union member, no double-init).
        if let enum_emit::EnumTag::View {
            member_path,
            scalar_ty,
        } = enum_emit::enum_tag(model, shape)
        {
            if payload_all_zst(model, shape, v) {
                return enum_value_tag_literal(
                    model,
                    names,
                    &agg,
                    shape,
                    v,
                    &member_path,
                    scalar_ty,
                );
            }
        }
        // Every other multi-view value (a payload-bearing `Direct` variant, or
        // any niched / untagged niche variant) serializes to the `_raw` byte
        // image -- the union cannot be const-initialized through separate
        // `_tag` / `_v<v>` members (a second union designator overrides the
        // first).
        serialize_multiview_enum_value(model, &agg, enum_ty, shape, v, payload)
    }
}

/// Serialize a fully-known multi-view enum value to a `layout.size` byte image
/// and emit `(struct E){ .u._raw = { ... } }` (d2.2 Change E). Mirrors
/// `assign_enum`'s tag-then-payload write, but into bytes: the variant's
/// `tag_value` (when present) at the tag cell over its raw storage width, then
/// each non-ZST scalar payload child at its `FieldLayout.offset`, endian per
/// `model.target.endian`. A pointer / `Reloc` / non-scalar payload child
/// cannot be encoded into `_raw` -> task d2.3. `_raw` is written through the
/// `enum_raw_member` navigator path (the single source of truth).
fn serialize_multiview_enum_value(
    model: &RegseqModel,
    agg: &str,
    enum_ty: TyId,
    shape: &EnumShape,
    v: usize,
    payload: &[ConstId],
) -> Result<String, EmitError> {
    let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
        "Const::EnumValue variant index out of range",
    ))?;
    let size = model.size_align_of(enum_ty).size as usize;
    let mut image = vec![0u8; size];
    let endian = model.target.endian;

    // Tag write: a `Direct` / niched variant carries a `tag_value`; the
    // untagged niche variant has `None` (its payload is the tag).
    if let Some(tv) = variant.tag_value {
        let (offset, width) = enum_tag_location(model, shape).ok_or(EmitError::Unsupported(
            "multi-view enum value without a tag cell location",
        ))?;
        write_bits_into(&mut image, offset, width, tv, endian)?;
    }

    // Payload write: each non-ZST field's scalar bit pattern at its offset.
    for (i, child_id) in payload.iter().enumerate() {
        let field = variant.fields.get(i).ok_or(EmitError::Unsupported(
            "Const::EnumValue payload exceeds the variant field count",
        ))?;
        if layout_emit::is_zst(model, field.ty) {
            continue;
        }
        let child = model.consts.resolve(*child_id);
        let bits = const_scalar_bits(child).ok_or(EmitError::Unsupported(
            "Const enum/aggregate with relocation payload: task d2.3",
        ))?;
        let width = model.size_align_of(field.ty).size as usize;
        write_bits_into(&mut image, field.offset as usize, width, bits, endian)?;
    }

    let raw = enum_emit::enum_raw_member(model, shape).ok_or(EmitError::Unsupported(
        "multi-view enum value without a _raw union member",
    ))?;
    let img = image
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("(struct {agg}){{ .{raw} = {{ {img} }} }}"))
}

/// The (offset, raw storage width) of an enum's tag cell, for serializing a
/// value-position const into a byte image: the `_tag` view's `TagScalar`
/// offset + scalar width (`Direct`), or the niche cell's offset + scalar width
/// (`Niche`). `None` for an enum with no tag bytes. Reads `TagScalar` /
/// `enum_tag` as certificates -- no niche-formula re-derivation.
fn enum_tag_location(model: &RegseqModel, shape: &EnumShape) -> Option<(usize, usize)> {
    match enum_emit::enum_tag(model, shape) {
        enum_emit::EnumTag::View { scalar_ty, .. } => {
            let offset = direct_tag_offset(shape)?;
            Some((
                offset as usize,
                model.size_align_of(scalar_ty).size as usize,
            ))
        }
        enum_emit::EnumTag::Niche { offset, scalar_ty } => Some((
            offset as usize,
            model.size_align_of(scalar_ty).size as usize,
        )),
        enum_emit::EnumTag::None => None,
    }
}

/// The `TagScalar.offset` of a `_tag`-view enum (`Single { scalar: Some }` /
/// `Multiple { Direct }`), read directly as a layout certificate.
fn direct_tag_offset(shape: &EnumShape) -> Option<u64> {
    match shape {
        EnumShape::Single {
            scalar: Some(tag), ..
        } => Some(tag.offset),
        EnumShape::Multiple {
            tag,
            encoding: TagEncoding::Direct,
            ..
        } => Some(tag.offset),
        _ => None,
    }
}

/// The scalar bit pattern of a value-level scalar `Const` (for serializing an
/// enum payload into a byte image), or `None` for a non-scalar (`Reloc` /
/// `NullPtr` / `Aggregate` / `Bytes` / `EnumValue` / `ZST`), which cannot be
/// encoded into `_raw`. `Int` / `Float` already carry their (sign-extended /
/// IEEE) bit pattern; `Bool` / `Char` map to their integer value.
fn const_scalar_bits(c: &Const) -> Option<u128> {
    match c {
        Const::Bool(b) => Some(u128::from(*b)),
        Const::Char(u) => Some(u128::from(*u)),
        Const::Int(_, v) | Const::Uint(_, v) | Const::Float(_, v) => Some(*v),
        _ => None,
    }
}

/// Write the low `width` bytes of `bits` into `image[offset..]` in target byte
/// order. Fails loud if the write would run past the image (a layout / size
/// inconsistency).
fn write_bits_into(
    image: &mut [u8],
    offset: usize,
    width: usize,
    bits: u128,
    endian: Endian,
) -> Result<(), EmitError> {
    if offset + width > image.len() {
        return Err(EmitError::Unsupported(
            "enum value byte image write exceeds the layout size",
        ));
    }
    for i in 0..width {
        let byte = (bits >> (8 * i)) as u8;
        let pos = match endian {
            Endian::Little => offset + i,
            Endian::Big => offset + width - 1 - i,
        };
        image[pos] = byte;
    }
    Ok(())
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
                // An enum payload field sits in an init-list position inside
                // the enum compound literal, so children reconstruct with
                // `at_init = true` (an array field uses `{ ... }`, not a cast).
                let e = const_expr(model, names, syms, child, Some(fty), true)?;
                inits.push(format!(".{member_path} = {e}"));
            }
            None => {
                let _ = const_expr(model, names, syms, child, None, true)?;
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
            "tag-view enum variant without a tag value (model corruption)",
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
/// field -> task d2.3). The inline view is the niche untagged variant's
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
    // A `Const::EnumValue` writes the niche cell through the field's logical C
    // type (a const compound literal has no raw-byte/memcpy escape hatch). A
    // type that normalizes a raw bit pattern -- `_Bool`, where `(_Bool){2}`
    // stores 1 -- therefore cannot carry an arbitrary niche value as a const, so
    // defer it to d2.2. (The runtime niche path uses raw-integer byte storage
    // and is unaffected.)
    if matches!(model.types[scalar_ty], Ty::Bool) {
        return None;
    }
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

/// The raw storage unsigned-integer [`UintKind`] for a niche tag cell of
/// logical type `scalar_ty`. A niche tag is a raw bit pattern (`tag_value mod
/// 2^scalar_width`), so the cell is read / written / compared as an unsigned
/// integer of the scalar's storage width -- never the logical C type, which can
/// normalize (`_Bool`) or constrain (an invalid `char`) the value. The width is
/// the scalar's byte size; fails loud for a width outside {1,2,4,8,16}.
fn niche_storage_kind(model: &RegseqModel, scalar_ty: TyId) -> Result<UintKind, EmitError> {
    match model.size_align_of(scalar_ty).size {
        1 => Ok(UintKind::U8),
        2 => Ok(UintKind::U16),
        4 => Ok(UintKind::U32),
        8 => Ok(UintKind::U64),
        16 => Ok(UintKind::U128),
        _ => Err(EmitError::Unsupported(
            "niche tag cell of an unsupported storage width",
        )),
    }
}

/// Render an enum tag value of logical type `scalar_ty` (section 13 / 11.1) for
/// a `_tag`-view (`Direct` / `Single { scalar: Some }`) write or a non-`Bool`
/// inline-niche const field, where the value is in-range for the logical type.
/// Every admitted `TagScalar.scalar_ty` is covered: `Int` / `Uint` via the d1
/// signed / unsigned renderers; `Bool` / `Char` via their d1 const forms; a
/// `Ptr` / `Ref` tag (the niche tag *is* the pointer, e.g. `Option<&T>`) as
/// the integer bit pattern cast to the pointer's C type. (The runtime niche
/// cell path uses [`niche_storage_kind`] + `render_uint` for raw bit patterns.)
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

// ---- section 8 diagnostic-logging: format template parser + escaper ----

/// The section-8.2 / section-26.7 `(specifier, cast)` for a numeric operand
/// `Ty` under `modifier` (Change C). Module-level so [`mod_tests`] can pin the
/// mapping directly. Gates on the section-26.4-admissible numeric set: any
/// other `Ty` (incl. `Char` / `Isize` / `Usize` / `&str` -- the broader
/// `printf_spec` accepts the first three, but diag-log must not) fails loud.
/// For a hex modifier the cast is the two-step
/// `(varargs-unsigned)(unsigned-of-source-width)` so a negative signed value
/// renders its source-width two's-complement, and the `ll` length modifier is
/// inserted for 64-bit operands.
pub(crate) fn diag_specifier_and_cast_for(
    modifier: Modifier,
    ty: &Ty,
) -> Result<(String, String), EmitError> {
    match modifier {
        Modifier::None => {
            let (spec, cast) = match ty {
                Ty::Bool | Ty::Int(IntKind::I8 | IntKind::I16 | IntKind::I32) => ("%d", "(int)"),
                Ty::Uint(UintKind::U8 | UintKind::U16 | UintKind::U32) => ("%u", "(unsigned)"),
                Ty::Uint(UintKind::U64) => ("%llu", "(unsigned long long)"),
                Ty::Int(IntKind::I64) => ("%lld", "(long long)"),
                _ => {
                    return Err(EmitError::Unsupported(
                        "diag-log argument of non-admissible type",
                    ))
                }
            };
            Ok((spec.to_string(), cast.to_string()))
        }
        Modifier::Hex {
            upper,
            alternate,
            width,
        } => {
            // Source-width unsigned reinterpret + varargs-unsigned cast.
            let (src_cast, varargs, is_64) = match ty {
                Ty::Int(IntKind::I8) => ("(uint8_t)", "(unsigned)", false),
                Ty::Int(IntKind::I16) => ("(uint16_t)", "(unsigned)", false),
                Ty::Int(IntKind::I32) => ("(uint32_t)", "(unsigned)", false),
                Ty::Uint(UintKind::U8 | UintKind::U16 | UintKind::U32) => ("", "(unsigned)", false),
                Ty::Uint(UintKind::U64) | Ty::Int(IntKind::I64) => {
                    ("", "(unsigned long long)", true)
                }
                _ => {
                    return Err(EmitError::Unsupported(
                        "diag-log hex modifier on a non-integer operand",
                    ))
                }
            };
            let letter = if upper { "X" } else { "x" };
            let hash = if alternate { "#" } else { "" };
            let pad = if width > 0 {
                format!("0{width}")
            } else {
                String::new()
            };
            let length = if is_64 { "ll" } else { "" };
            let spec = format!("%{hash}{pad}{length}{letter}");
            let cast = format!("{varargs}{src_cast}");
            Ok((spec, cast))
        }
    }
}

/// A `&str` argument admits only the default `{}` placeholder: a hex modifier
/// is integer-only (section 26.8 D2). Fails loud on any non-default modifier
/// (the bridge already rejects a hex modifier on a `&str`, so a bridged model
/// never reaches this).
pub(crate) fn require_default_modifier(modifier: Modifier) -> Result<(), EmitError> {
    if matches!(modifier, Modifier::None) {
        Ok(())
    } else {
        Err(EmitError::Unsupported(
            "diag-log hex modifier on a non-integer operand",
        ))
    }
}

/// A parsed format-template placeholder modifier (section 26.7, the bridge's
/// actual accepted set). `None` is the default `{}`; `Hex` carries the bridge's
/// `{ upper, alternate, width }` flag tuple (mirroring the driver's
/// `FormatSpecifier`), covering the eight accepted forms `{x,X}` x
/// `{plain, #-alternate, 08-zero-pad, #010-zero-pad-alternate}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modifier {
    /// The default `{}` placeholder.
    None,
    /// A hex placeholder with its flag tuple.
    Hex {
        /// Uppercase conversion (`X` vs `x`).
        upper: bool,
        /// The `#` alternate flag.
        alternate: bool,
        /// The zero-pad width (`0` for none).
        width: u32,
    },
}

/// One segment of a parsed format template: literal text bytes, or a
/// placeholder carrying its modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// Literal text bytes (already brace-unescaped).
    Literal(Vec<u8>),
    /// A `{...}` placeholder.
    Placeholder(Modifier),
}

/// Parse a format-template byte string into an ordered segment sequence
/// (Change B part 2). Accepts the bridge's actual set: the default `{}`, the
/// eight hex forms, `{{`/`}}` brace escapes (-> a literal `{`/`}`), embedded
/// newlines (ordinary literal text), and the empty template (zero
/// placeholders). A malformed brace (an unclosed `{`, a stray unescaped `}`)
/// or any inner text outside the accepted set fails loud defensively (the
/// bridge already rejects these, so a bridged model never carries one).
pub(crate) fn parse_template(bytes: &[u8]) -> Result<Vec<Segment>, EmitError> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut literal: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'{' => {
                if bytes.get(i + 1) == Some(&b'{') {
                    // `{{` -> literal `{`.
                    literal.push(b'{');
                    i += 2;
                    continue;
                }
                // A real placeholder: scan to the closing `}`.
                let close = bytes[i + 1..]
                    .iter()
                    .position(|&c| c == b'}')
                    .map(|p| i + 1 + p)
                    .ok_or(EmitError::Unsupported("diag-log unclosed `{` in template"))?;
                let inner = &bytes[i + 1..close];
                let modifier = parse_placeholder(inner)?;
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(Segment::Placeholder(modifier));
                i = close + 1;
            }
            b'}' => {
                if bytes.get(i + 1) == Some(&b'}') {
                    // `}}` -> literal `}`.
                    literal.push(b'}');
                    i += 2;
                    continue;
                }
                return Err(EmitError::Unsupported("diag-log stray `}` in template"));
            }
            _ => {
                literal.push(b);
                i += 1;
            }
        }
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}

/// Parse a placeholder's inner text (between `{` and `}`) into a [`Modifier`].
/// Empty inner -> the default; otherwise it must be `:<hex-spec>` from the
/// bridge's accepted set. Anything else (positional / named / `:?` / `:b` /
/// `:o` / `:p` / precision / other widths) fails loud.
pub(crate) fn parse_placeholder(inner: &[u8]) -> Result<Modifier, EmitError> {
    if inner.is_empty() {
        return Ok(Modifier::None);
    }
    if inner[0] != b':' {
        return Err(EmitError::Unsupported(
            "diag-log non-empty placeholder spec",
        ));
    }
    let spec = &inner[1..];
    // The eight accepted forms, by exact byte match.
    let modifier = match spec {
        b"x" => Modifier::Hex {
            upper: false,
            alternate: false,
            width: 0,
        },
        b"X" => Modifier::Hex {
            upper: true,
            alternate: false,
            width: 0,
        },
        b"#x" => Modifier::Hex {
            upper: false,
            alternate: true,
            width: 0,
        },
        b"#X" => Modifier::Hex {
            upper: true,
            alternate: true,
            width: 0,
        },
        b"08x" => Modifier::Hex {
            upper: false,
            alternate: false,
            width: 8,
        },
        b"08X" => Modifier::Hex {
            upper: true,
            alternate: false,
            width: 8,
        },
        b"#010x" => Modifier::Hex {
            upper: false,
            alternate: true,
            width: 10,
        },
        b"#010X" => Modifier::Hex {
            upper: true,
            alternate: true,
            width: 10,
        },
        _ => {
            return Err(EmitError::Unsupported(
                "diag-log out-of-subset format modifier",
            ))
        }
    };
    Ok(modifier)
}

/// Render a byte run as the body of a C string literal (Change D escaper).
/// `"` -> `\"`, `\` -> `\\`, the C escapes `\n` / `\t` / `\r`, other
/// non-printable / non-ASCII bytes -> a 3-digit octal `\nnn` (fixed width so a
/// following digit cannot extend the escape), printable ASCII verbatim. Every
/// literal run and every static `&str` value is `fwrite`n raw (section 8.1),
/// never used as a `printf` format string, so a literal `%` needs no doubling.
pub(crate) fn c_escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out
}

/// Flush a pending literal byte run as a single `fwrite` statement (Change D):
/// `fwrite("<c_escape(run)>", 1, <decoded byte len>, stdout);`. The count is the
/// DECODED byte length (`run.len()`) -- an embedded `\n` is one byte and an
/// interior `\0` renders `\000` (three C-literal chars) but counts as one
/// decoded byte -- not the escaped C-literal length. An empty run emits nothing
/// (two adjacent placeholders leave no literal between them). Clears `run`.
fn flush_literal(run: &mut Vec<u8>, stmts: &mut Vec<String>) {
    if run.is_empty() {
        return;
    }
    stmts.push(format!(
        "fwrite(\"{}\", 1, {}, stdout);",
        c_escape(run),
        run.len()
    ));
    run.clear();
}

/// The section-8.1 bracketed-tag body (between `[` and `]`) for a `LogLevel`.
pub(crate) fn level_tag(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
    }
}
