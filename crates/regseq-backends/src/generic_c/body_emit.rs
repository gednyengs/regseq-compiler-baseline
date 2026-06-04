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
//! byte image). Task d2.3a closes the cheap const carve-outs: a `Const::Bytes`
//! union byte image (the union `_raw` member), a no-relocation structured
//! multi-view enum payload (the recursive `serialize_const_into`), and a
//! multi-view niche-untagged relocation payload (the `u._v<untagged>` typed
//! view). Task d2.3b closes the const exit gate for every reachable enum/union
//! shape: an inline enum's anonymous-union `<raw>` serves the inline niche
//! const uniformly (closing the bool-niche + nested-niche cases), and a typed
//! full-object view `_c<idx>` routes a multi-view tag-view (`Direct`) enum
//! const whose payload carries a relocation (both the `Const::EnumValue` and
//! the `Const::Bytes`-with-`Byte::Reloc` forms). The remaining fail-louds are
//! external-rustc-invariant defensive guards, not shipped gaps: a niche enum
//! const with a relocation payload in a niched (non-untagged) variant (rustc
//! niche-fills only fieldless variants), a `Const::Aggregate` over a union
//! (rustc emits no union valtree), and a `ConstMemoryStatic` deduped at
//! incompatible pointee types (a bridge content-hash dedup follow-up); `dyn` /
//! vtable construction is the vtable task. Diag-log call-site interception
//! lowers to the section 8 `printf`.

use std::cell::RefCell;

use regseq_model::{
    args_key_for_generic_args, AggregateKind, AssertKind, BinOp, Byte, CallCallee, CastKind, Const,
    ConstId, Endian, EnumShape, FloatKind, GenericArg, InstanceId, InstanceKind, InstanceRef,
    IntKind, IntrinsicKind, ItemId, LocalId, LogLevel, MmioOp, NonDivergingIntrinsic, Operand, Place,
    PlaceElem, PointerCoercion, RegseqModel, Rvalue, StatementKind, SwitchTargets,
    SyntheticItemKind, TagEncoding, TerminatorKind, Ty, TyId, UintKind, UnOp, VariantIdx,
    VariantLayout,
};

use super::errno::errno_symbol;
use super::prelude::HelperUsage;
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
    helper_use: &RefCell<HelperUsage>,
) -> Result<String, EmitError> {
    let data = instance
        .kind
        .as_function()
        .expect("emit_body called on a non-function instance");
    let fn_sig = model.sigs.resolve(data.fn_sig);
    let output_ty = fn_sig.output;
    let lowering = super::c_abi_lowering(model, data.fn_sig);
    let emitter = BodyEmit {
        model,
        names,
        syms,
        body,
        output_ty,
        fn_sig,
        lowering,
        helper_use,
    };

    let mut out = String::from(" {\n");

    // Locals whose own storage is bare-borrowed (`&local`): a ZST in this set
    // needs real zero-sized storage to point at, overriding the ZST
    // declare-nothing rule applied in each phase below.
    let bare_zst = emitter.bare_borrowed_zst_locals();

    // Return place `_0`: declared iff the output is non-ZST (section 7), or a
    // ZST `_0` whose address is bare-borrowed.
    if !emitter.is_zst(output_ty) || bare_zst.contains(&LocalId::RETURN_PLACE) {
        out.push_str(&format!(
            "    {};\n",
            types::declare(model, names, output_ty, "_0")
        ));
    }
    // C-ABI `Substituted` argument locals: the C formal parameter carries the
    // ABI (`emit_as`) type under a distinct `<local>__abi` name, so the
    // original-typed body local must be declared HERE and filled from the ABI
    // formal by a representation-identical copy (the prologue marshalling). A
    // `Direct` argument's formal *is* the body local (declared by the caller),
    // so it is skipped.
    let prologue = emitter.input_marshalling()?;
    for local in body.arg_locals() {
        let i = local.as_u32() as usize - 1;
        let decl = &body.local_decls[local];
        if emitter.is_zst(decl.ty) {
            // A ZST argument is never a C formal (elided from the signature,
            // `mod.rs:fn_signature`) and normally declares nothing. Give it
            // real storage only if its address is bare-borrowed; this adds a
            // declaration alone -- no prologue marshalling (a ZST carries no
            // bytes, so `input_marshalling` already skips it).
            if bare_zst.contains(&local) {
                let name = local_c_name(model, body, local);
                out.push_str(&format!(
                    "    {};\n",
                    types::declare(model, names, decl.ty, &name)
                ));
            }
            continue;
        }
        if !super::input_needs_abi_formal(model, body, fn_sig, lowering, i) {
            continue;
        }
        let name = local_c_name(model, body, local);
        out.push_str(&format!(
            "    {};\n",
            types::declare(model, names, decl.ty, &name)
        ));
    }
    // Inner locals (arg_count+1..); a ZST local declares nothing (unless its
    // address is bare-borrowed). Direct argument locals are the C formal
    // parameters, declared by the caller.
    for local in body.inner_locals() {
        let decl = &body.local_decls[local];
        if emitter.is_zst(decl.ty) {
            if bare_zst.contains(&local) {
                let name = local_c_name(model, body, local);
                out.push_str(&format!(
                    "    {};\n",
                    types::declare(model, names, decl.ty, &name)
                ));
            }
            continue;
        }
        let name = local_c_name(model, body, local);
        out.push_str(&format!(
            "    {};\n",
            types::declare(model, names, decl.ty, &name)
        ));
    }
    // The prologue marshalling statements (copying each ABI formal into its
    // original-typed body local) run before the first basic block.
    for stmt in prologue {
        out.push_str(&format!("    {stmt}\n"));
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
    /// The owning function's signature: needed for the C-ABI `Substituted`
    /// return-position marshalling (the body's `_0` keeps `output_ty`; the
    /// `return` marshals it to the ABI `emit_as`).
    fn_sig: &'a regseq_model::FnSig,
    /// The owning function's C-ABI lowering side-table entry, when present.
    /// Drives the `Substituted` input prologue + return marshalling.
    lowering: Option<&'a regseq_model::CAbiLowering>,
    /// Demand-driven helper accounting (plan 10): the `*WithOverflow` /
    /// `FloatToInt` sites record the exact helper name they emit here, so
    /// `helper_block` emits only the helpers this TU's bodies use.
    helper_use: &'a RefCell<HelperUsage>,
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

    /// ZST locals whose own storage is borrowed by a **bare-local** `&local`
    /// (a `Ref` / `RawPtr` with an empty projection). A ZST normally declares
    /// no C object (a ZST read/write is a no-op), but `&local` needs a real,
    /// zero-sized object to point at -- otherwise the emitted `&(_n)` names a
    /// local that was never declared (and, for a ZST argument, also elided from
    /// the C signature, `mod.rs:fn_signature`). The borrowed empty struct is
    /// zero-sized under gnu11, so the resulting pointer is valid and (matching
    /// Rust's `&zst`) never dereferenced for data.
    ///
    /// Bare-projection scope is deliberate: a borrow through a `Deref` is
    /// re-rooted at a (declared, non-ZST) pointee, and a borrow of a *projected*
    /// ZST place (`&local.zst_field`) is a separate, pre-existing concern
    /// (a ZST field is omitted from the emitted aggregate) that declaring the
    /// base local would not fix. `Ref` / `RawPtr` only appear in `Assign`
    /// rvalues, so statements are the complete scan surface.
    fn bare_borrowed_zst_locals(&self) -> std::collections::HashSet<LocalId> {
        let mut set = std::collections::HashSet::new();
        for bb in self.body.basic_blocks.iter() {
            for stmt in bb.statements.iter() {
                if let StatementKind::Assign(_, Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p)) =
                    &stmt.kind
                {
                    if p.projection.is_empty() && self.is_zst(self.body.local_decls[p.local].ty) {
                        set.insert(p.local);
                    }
                }
            }
        }
        set
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

    /// The pointee type of an operand of `Ref`/`Ptr` type, when statically
    /// known. Used to detect a fat source pointer (an unsized pointee) at a
    /// `PtrToPtr` cast.
    fn operand_pointee(&self, op: &Operand) -> Option<TyId> {
        match &self.model.types[self.operand_ty(op)?] {
            Ty::Ref(p, _) | Ty::Ptr(p, _) => Some(*p),
            _ => None,
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
            Rvalue::BinaryOp(BinOp::Cmp, a, b) => {
                return self.assign_cmp(&dst, dst_ty, a, b, zst);
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

    /// The C-ABI `Substituted`-input prologue: for each substituted argument
    /// position, a representation-identical `memcpy` copying the ABI formal
    /// (`<local>__abi`, the `emit_as` type) into the original-typed body local
    /// (`<local>`). A `repr(transparent)` chain is layout-identical, so the
    /// `memcpy` is a reinterpret with no value change. A ZST argument carries
    /// no bytes and is skipped (it has no formal parameter). Runs before the
    /// first basic block so the body's first read of `<local>` sees the value.
    fn input_marshalling(&self) -> Result<Vec<String>, EmitError> {
        let mut out = Vec::new();
        for local in self.body.arg_locals() {
            let i = local.as_u32() as usize - 1;
            if !super::input_needs_abi_formal(self.model, self.body, self.fn_sig, self.lowering, i)
            {
                continue;
            }
            let body_ty = self.body.local_decls[local].ty;
            if self.is_zst(body_ty) {
                continue;
            }
            let name = local_c_name(self.model, self.body, local);
            out.push(format!("memcpy(&{name}, &{name}__abi, sizeof({name}));"));
        }
        Ok(out)
    }

    /// A representation-identical reinterpret of `src` (an addressable lvalue
    /// of the body value's original type) to `to_ty`, as a GNU
    /// statement-expression: a fresh temp of `to_ty` filled by `memcpy` from
    /// `src`. Used at C-ABI `Substituted`
    /// call-argument positions, where the body value is the original type but
    /// the prototype expects `emit_as` (a `repr(transparent)` chain is
    /// layout-identical, so the copy changes no bytes). The temp lives in its
    /// own block scope ([`Self::fresh_temp_name`]).
    fn reinterpret_to(&self, src: &str, to_ty: TyId) -> String {
        let tmp = self.fresh_temp_name("_ab");
        let tmp_cty = types::c_type(self.model, self.names, to_ty);
        format!("({{ {tmp_cty} {tmp}; memcpy(&{tmp}, &({src}), sizeof({tmp})); {tmp}; }})")
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
        // Demand-driven helper accounting (plan 10): record the exact wrapper
        // name this site emits so `helper_block` defines it (and its result
        // struct `regseq_overflow_<ct>`).
        self.helper_use
            .borrow_mut()
            .record_wrapper(format!("regseq_{helper}_{ct}"));
        let tmp = self.fresh_temp_name("_ov");
        Ok(Some(format!(
            "{{ struct regseq_overflow_{ct} {tmp} = regseq_{helper}_{ct}(({x}), ({y})); \
             {dst}._0 = {tmp}.v; {dst}._1 = {tmp}.overflow; }}"
        )))
    }

    /// `BinOp::Cmp` at the assign site (section 10.1, matrix row BO-25): the
    /// three-way compare's MIR result type is always `core::cmp::Ordering`, a
    /// `repr(i8)` fieldless enum (`Less = -1`, `Equal = 0`, `Greater = 1`) the
    /// backend lowers to `struct core_cmp_Ordering`. The scalar
    /// `(x > y) - (x < y)` is an i8 in `{-1, 0, 1}` -- exactly the matched
    /// variant's discriminant, which for a `Direct`-encoded fieldless enum is
    /// its tag value -- so it is written into the destination's tag cell using
    /// the same construction the backend already uses for a fieldless-enum
    /// discriminant write ([`Self::emit_set_discriminant`]'s `_tag` view), not
    /// emitted as a bare `int8_t` rvalue (which would mismatch the enum-struct
    /// place). A non-`Ordering` (non-`Direct`-tag-view) destination is rejected;
    /// `Cmp` is always assign-site (like `*WithOverflow`), so the nested-rvalue
    /// arm ([`Self::binary_op`]) is defensive `Unsupported`.
    fn assign_cmp(
        &self,
        dst: &str,
        dst_ty: TyId,
        a: &Operand,
        b: &Operand,
        zst: bool,
    ) -> Result<Option<String>, EmitError> {
        let x = self.operand_expr(a, None)?;
        let y = self.operand_expr(b, None)?;
        let shape = enum_emit::enum_shape(self.model, dst_ty).ok_or(EmitError::Unsupported(
            "Cmp into a non-enum destination (result is core::cmp::Ordering)",
        ))?;
        let (member_path, scalar_ty) = match enum_emit::enum_tag(self.model, shape) {
            enum_emit::EnumTag::View {
                member_path,
                scalar_ty,
            } => (member_path, scalar_ty),
            _ => {
                return Err(EmitError::Unsupported(
                    "Cmp into an enum without a Direct tag view (not core::cmp::Ordering)",
                ))
            }
        };
        // `Ordering` is never zero-sized; the guard is defensive (and validates
        // the operands above so a ZST destination still fails loud on a bad op).
        if zst {
            return Ok(None);
        }
        // The tag value is the i8 three-way scalar; cast it to the tag's C type
        // so the write matches `emit_set_discriminant`'s `<enum>.tag = ...;`.
        let tag_cty = types::c_type(self.model, self.names, scalar_ty);
        Ok(Some(format!(
            "{dst}.{member_path} = ({tag_cty})((({x}) > ({y})) - (({x}) < ({y})));"
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
        let enum_lv = self.place_lvalue(enum_place)?;
        // The discriminant result is an integer, never ZST; defensive guard.
        if zst {
            return Ok(None);
        }
        let tmp = self.fresh_temp_name("_nd");
        Ok(Some(niche_discriminant_read_block(
            self.model, ty, &enum_lv, dst, dst_ty, &tmp,
        )?))
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
                        Ty::Slice(_) | Ty::Str | Ty::Dynamic { .. } => {
                            // Fat pointer (slice/str `{data,len}` or dyn
                            // `{data,vtable}`): the data slot; remember the base
                            // so a trailing `.len`/`.vtable` metadata read or a
                            // whole-place re-borrow can recover the fat pointer.
                            fat_base = Some(lv.clone());
                            lv = format!("{lv}.data");
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
    /// pointer): the existing fat pointer copied -- the fat base itself
    /// (`{ .data, .len }` for slice/str, `{ .data, .vtable }` for dyn; both are
    /// recovered by `place_walk`'s shared fat-base tracking). `pointee_ty` is
    /// unused now that dyn flows through the same fat-base path.
    fn fat_reborrow(&self, place: &Place, _pointee_ty: TyId) -> Result<String, EmitError> {
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
                    Some(Ty::Dynamic { .. }) => Ok(format!("({e}).vtable")),
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
            BinOp::Cmp => {
                // The three-way result type is always `core::cmp::Ordering` (a
                // fieldless enum -> `struct core_cmp_Ordering`), a multi-field
                // place written at the assign site ([`Self::assign_cmp`], matrix
                // row BO-25); a bare `int8_t` nested rvalue would mismatch the
                // enum-struct destination, so reaching here is defensive.
                Err(EmitError::Unsupported(
                    "Cmp as a nested rvalue (assign-site only; result is core::cmp::Ordering)",
                ))
            }
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
                // Demand-driven helper accounting (plan 10): record the exact
                // saturating-cast name this site emits.
                self.helper_use
                    .borrow_mut()
                    .record_cast(format!("regseq_{prefix}_to_{dest}"));
                Ok(format!("regseq_{prefix}_to_{dest}({e})"))
            }
            CastKind::PtrToPtr => {
                // A fat source pointer (dyn `{data,vtable}` / slice/str
                // `{data,len}`) cast to a thin pointer takes the `data` slot --
                // casting the whole fat struct would be a C type error. The
                // bridged dyn-dispatch lowering emits `data = Cast(PtrToPtr,
                // &dyn, *mut ())` (the erased receiver), which hits this.
                if self.operand_pointee(op).is_some_and(|p| self.is_unsized(p)) {
                    Ok(format!("(({dest})(({e}).data))"))
                } else {
                    Ok(format!("(({dest})({e}))"))
                }
            }
            CastKind::FnPtrToPtr | CastKind::PointerWithExposedProvenance => {
                Ok(format!("(({dest})({e}))"))
            }
            CastKind::PointerExposeProvenance => Ok(format!("((uintptr_t)({e}))")),
            CastKind::Subtype => Ok(e),
            CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer) => Ok(e),
            CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer) => self.reify_fn_pointer(op),
            CastKind::PointerCoercion(PointerCoercion::Unsize) => self.unsize_cast(op, dest_ty),
            // `ClosureFnPointer` (`let f: fn(..) = |..| ..;`): the bridge
            // recovers the non-capturing closure's `CallOnce` shim at the cast
            // site and lowers the operand to a `Const::Reloc` (the closure value
            // is otherwise a type-erased `Const::ZST`), so this shares the
            // `ReifyFnPointer` `&<fn>` designator path.
            CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer) => {
                self.reify_fn_pointer(op)
            }
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
            Ty::Dynamic { .. } => {
                let src_pointee = self.operand_pointee(op).ok_or(EmitError::Unsupported(
                    "Unsize to dyn from a non-pointer source (vtable task)",
                ))?;
                if matches!(self.model.types[src_pointee], Ty::Dynamic { .. }) {
                    // An already-fat source (`&dyn -> &dyn` re-unsize / identity):
                    // copy the existing fat pointer; the vtable is unchanged.
                    return Ok(op_expr);
                }
                // `&T as &dyn Tr` (07 plan D4): the destination is a `Ty::Ref`
                // fat pointer `{ void *data; const VTable *vtable }`. Build it
                // directly (NOT a `RawPtr` aggregate, which is `Ty::Ptr`). The
                // `data` slot is the thin concrete source erased to `void *`; the
                // `vtable` slot is the address of the concrete impl's
                // `VTableInstance` static, resolved from the source pointee.
                let fat = types::unsized_pointee_struct_name(self.model, self.names, dest_ty)
                    .ok_or(EmitError::Unsupported(
                        "Unsize to a dyn with no trait-object struct destination (vtable task)",
                    ))?;
                let vtable = self.dyn_vtable_designator(pointee, src_pointee)?;
                Ok(format!(
                    "(struct {fat}){{ .data = (void *)({op_expr}), .vtable = {vtable} }}"
                ))
            }
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

    /// The `&<VTableInstance>` designator for the `.vtable` slot of an
    /// `&T as &dyn Tr` unsize (07 plan D4, Option A: the backend resolves the
    /// concrete impl's vtable static). `dyn_ty` is the `Ty::Dynamic` pointee;
    /// `src_pointee` is the concrete `Self` type. The instance is identified by
    /// `(principal, self_ty)`: the principal is recovered from the dyn's
    /// `vtable_ty` struct item via `RegseqModel.vtables`, and `self_ty` is the
    /// canonical args key of `[Type(src_pointee)]` -- the same key the bridge
    /// minted for the `VTableInstance` synthetic item.
    fn dyn_vtable_designator(&self, dyn_ty: TyId, src_pointee: TyId) -> Result<String, EmitError> {
        let Ty::Dynamic { vtable_ty, .. } = &self.model.types[dyn_ty] else {
            return Err(EmitError::Unsupported(
                "dyn vtable designator on a non-dyn type",
            ));
        };
        let Ty::Struct {
            item: vtable_type_item,
            ..
        } = &self.model.types[*vtable_ty]
        else {
            return Err(EmitError::Unsupported(
                "dyn vtable_ty is not a struct (vtable task)",
            ));
        };
        let principal = self
            .model
            .vtables
            .iter()
            .find(|(_, mapped)| *mapped == vtable_type_item)
            .map(|(p, _)| p)
            .ok_or(EmitError::Unsupported(
                "dyn vtable type item absent from RegseqModel.vtables (vtable task)",
            ))?;
        let self_ty = args_key_for_generic_args(self.model, &vec![GenericArg::Type(src_pointee)]);
        // The `VTableInstance` synthetic item for this (principal, self_ty).
        let instance_item = self
            .model
            .item_synthetic_kinds
            .iter()
            .position(|kind| {
                matches!(
                    kind,
                    SyntheticItemKind::VTableInstance { principal: p, self_ty: s }
                        if p == principal && *s == self_ty
                )
            })
            .map(|idx| ItemId::from_u32(idx as u32))
            .ok_or(EmitError::Unsupported(
                "no VTableInstance for the unsized concrete type (vtable task)",
            ))?;
        // The static instance backing that item; its symbol is the C global.
        let id = (0..self.model.instances.len())
            .map(|i| InstanceId::from_u32(i as u32))
            .find(|id| {
                self.model.instances[*id].item == instance_item
                    && matches!(self.model.instances[*id].kind, InstanceKind::Static { .. })
            })
            .ok_or(EmitError::Unsupported(
                "VTableInstance item has no static instance (vtable task)",
            ))?;
        Ok(format!("(&{})", self.syms.symbol(id)))
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
                } else if super::output_is_substituted(self.fn_sig, self.lowering) {
                    // The body's `_0` keeps `output_ty`, but the C return type is
                    // the ABI `emit_as`. Marshal `_0` back to the ABI type by a
                    // representation-identical copy into a fresh temp, then
                    // return the temp (a `repr(transparent)` chain changes no
                    // bytes).
                    let abi_ty = super::effective_output_ty(self.fn_sig, self.lowering);
                    let tmp = self.fresh_temp_name("_ar");
                    let tmp_cty = types::c_type(self.model, self.names, abi_ty);
                    Ok(format!(
                        "{{ {tmp_cty} {tmp}; memcpy(&{tmp}, &_0, sizeof({tmp})); return {tmp}; }}"
                    ))
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
            // Register-access interception (register-access construct): a Direct
            // callee whose source item carries `IntrinsicKind::Mmio(op)` lowers to
            // a host device-model hook call instead of a C call. The descriptor
            // argument (RegisterId / Field / FieldValue) carries the register
            // offset/width (+ mask/shift/value) as ordinary runtime struct fields.
            if let Some(IntrinsicKind::Mmio(op)) = self.model.instance_intrinsic(*inst) {
                let op = *op;
                return self.emit_mmio(op, callee, args, destination, target);
            }
        }

        let callee_expr = match callee {
            CallCallee::Direct(inst) => self.syms.symbol(*inst).to_string(),
            CallCallee::Indirect(op) => format!("({})", self.operand_expr(op, None)?),
        };

        // The callee's parameter types, threaded as the per-argument consumer
        // type so a non-scalar / reference const argument lowers against the
        // parameter type (a scalar argument ignores it). For a C-ABI
        // `Substituted` position these are the `emit_as` ABI types. The recovery
        // is the single helper the Change-F pre-pass also uses, so emission +
        // scan cannot disagree on which arguments carry a known consumer type;
        // an unrecoverable signature lowers arguments at `None` (a non-scalar
        // const argument then fails loud).
        let param_tys = super::callee_param_tys(self.model, self.body, callee);
        // The callee's C-ABI lowering, used to reinterpret a `Substituted`
        // argument from the body value's original type to the ABI `emit_as` the
        // prototype expects (and to marshal a `Substituted` return back into the
        // original-typed destination below). `None` when the signature is not
        // recoverable -- then no position is substituted and no marshalling runs.
        let callee_sig_id = super::callee_fn_sig_id(self.model, self.body, callee);
        let callee_fn_sig = callee_sig_id.map(|s| self.model.sigs.resolve(s));
        let callee_lowering = callee_sig_id.and_then(|s| super::c_abi_lowering(self.model, s));

        let mut arg_strs = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            // Lower (validate) every argument first so a deferred place
            // projection reached through an argument fails loud, then drop the
            // ZST arguments from the C call (a ZST value carries no bytes and
            // is omitted from the prototype's parameter list, section 15).
            let mut expr = self.operand_expr(arg, param_tys.get(i).copied())?;
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
            // A C-ABI `Substituted` position: a body *place* value is the
            // original type, but the prototype expects `emit_as`, so reinterpret
            // it to the ABI type (a `repr(transparent)` chain is
            // layout-identical). A constant argument is already lowered at the
            // `emit_as` consumer type by `operand_expr`, so it needs no copy.
            if matches!(arg, Operand::Copy(_) | Operand::Move(_)) {
                if let (Some(fs), Some(eff)) = (callee_fn_sig, param_tys.get(i).copied()) {
                    if super::input_is_substituted(fs, callee_lowering, i) {
                        expr = self.reinterpret_to(&expr, eff);
                    }
                }
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
                } else if callee_fn_sig
                    .is_some_and(|fs| super::output_is_substituted(fs, callee_lowering))
                {
                    // The call yields the ABI `emit_as`, but `destination` is the
                    // original type. Hold the (single-evaluated) ABI result in a
                    // fresh temp, then marshal it back into the destination by a
                    // representation-identical copy (a `repr(transparent)` chain
                    // changes no bytes).
                    let abi_ty = super::effective_output_ty(
                        callee_fn_sig.expect("checked output_is_substituted"),
                        callee_lowering,
                    );
                    let tmp = self.fresh_temp_name("_ac");
                    let tmp_cty = types::c_type(self.model, self.names, abi_ty);
                    format!(
                        "{{ {tmp_cty} {tmp} = {call}; memcpy(&{dst}, &{tmp}, sizeof({dst})); }}"
                    )
                } else {
                    format!("{dst} = {call};")
                };
                Ok(format!("{assign} goto bb_{};", t.as_u32()))
            }
        }
    }

    /// Register-access MMIO lowering (register-access construct): lower an
    /// `IntrinsicKind::Mmio(op)` Call to a host device-model hook call against
    /// the Bank-1 register file (`regseq_mmio_read` / `regseq_mmio_write`).
    ///
    /// The register offset/width (+ field mask/shift/value) ride in the
    /// descriptor argument (`RegisterId` / `Field` / `FieldValue`) as ordinary
    /// runtime struct fields the `register_block!` macro populates, so lowering
    /// reads `(desc).offset` / `.width` / `.mask` / `.shift` / `.value` directly
    /// — no type-level offset recovery needed.
    ///
    /// Argument layout (the `&self` receiver is `args[0]`; the host device model
    /// is a single global register file, so the receiver is ignored):
    /// - `mmio_read(reg)`         : args = [recv, reg]            -> dest = read
    /// - `mmio_read_field(field)`  : args = [recv, field]          -> dest = read
    /// - `mmio_write(reg, val)`   : args = [recv, reg, val]
    /// - `mmio_write_field(fv)`   : args = [recv, fv]
    fn emit_mmio(
        &self,
        op: MmioOp,
        callee: &CallCallee,
        args: &[Operand],
        destination: &Place,
        target: Option<regseq_model::BlockId>,
    ) -> Result<String, EmitError> {
        let target = target.ok_or(EmitError::Unsupported("mmio call with no target"))?;
        // A descriptor argument (RegisterId / Field / FieldValue) is an
        // aggregate const, so it must be rendered against its parameter type (a
        // `None` consumer type fails to emit a `Const::Bytes`). `operand_ty`
        // yields `None` for constants, so recover the type from the callee's
        // parameter signature, exactly as the ordinary-call path does.
        let param_tys = super::callee_param_tys(self.model, self.body, callee);
        let arg = |this: &Self, i: usize| -> Result<String, EmitError> {
            this.operand_expr(&args[i], param_tys.get(i).copied())
        };
        let desc_arg = arg;
        let stmt = match op {
            MmioOp::Write => {
                let desc = desc_arg(self, 1)?;
                let val = arg(self, 2)?;
                format!(
                    "regseq_mmio_write(({desc}).offset, ({desc}).width, (uint64_t)({val}));"
                )
            }
            MmioOp::WriteField => {
                let fv = desc_arg(self, 1)?;
                format!(
                    "regseq_mmio_write(({fv}).offset, ({fv}).width, (uint64_t)(({fv}).value));"
                )
            }
            MmioOp::Read => {
                let desc = desc_arg(self, 1)?;
                let dst = self.place_lvalue(destination)?;
                let dst_cty = types::c_type(self.model, self.names, self.place_ty(destination));
                format!("{dst} = ({dst_cty})regseq_mmio_read(({desc}).offset, ({desc}).width);")
            }
            MmioOp::ReadField => {
                let fld = desc_arg(self, 1)?;
                let dst = self.place_lvalue(destination)?;
                let dst_cty = types::c_type(self.model, self.names, self.place_ty(destination));
                format!(
                    "{dst} = ({dst_cty})((regseq_mmio_read(({fld}).offset, ({fld}).width) & ({fld}).mask) >> ({fld}).shift);"
                )
            }
            MmioOp::Modify => {
                // RMW: read, clear the field mask, OR the value, write back.
                // Emitted in C so it needs no generic Rust bit-op lowering.
                // args = [recv, reg (RegisterId), values (FieldValue)].
                let reg = desc_arg(self, 1)?;
                let values = arg(self, 2)?;
                let off = self.fresh_temp_name("_rmw_off");
                let w = self.fresh_temp_name("_rmw_w");
                let cur = self.fresh_temp_name("_rmw_cur");
                format!(
                    "{{ uint32_t {off} = ({reg}).offset; uint32_t {w} = ({reg}).width; \
                     uint64_t {cur} = regseq_mmio_read({off}, {w}); \
                     regseq_mmio_write({off}, {w}, ({cur} & ~(uint64_t)(({values}).mask)) | (uint64_t)(({values}).value)); }}"
                )
            }
        };
        Ok(format!("{stmt} goto bb_{};", target.as_u32()))
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
        // `&str` value's full byte range. The numeric scratch buffer / length
        // names come from `fresh_temp_name`, so they cannot collide with (and
        // thus shadow) a body-local C name -- e.g. a debug-named local `_n0`
        // used as the operand (every other backend temp follows this same
        // discipline). Each numeric block is its own `{ }` scope, so one
        // `(nbuf, nlen)` pair is reused across all numeric placeholders.
        let nbuf = self.fresh_temp_name("_n");
        let nlen = self.fresh_temp_name("_l");
        let mut pending: Vec<u8> = Vec::new();
        pending.push(b'[');
        pending.extend_from_slice(level_tag(level).as_bytes());
        pending.extend_from_slice(b"] ");
        let mut stmts: Vec<String> = Vec::new();
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
                    // `{nlen}` is the exact, non-negative length < 24 and `fwrite`
                    // reads only in-bounds bytes (no clamp needed). The operand
                    // `Ty` comes from a place's decl or a scalar constant.
                    let ty = self
                        .diag_numeric_ty(op)
                        .ok_or(EmitError::Unsupported("diag-log argument of unknown type"))?;
                    let (spec, cast) = diag_specifier_and_cast_for(*modifier, &ty)?;
                    let expr = self.operand_expr(op, None)?;
                    stmts.push(format!(
                        "{{ char {nbuf}[24]; int {nlen} = snprintf({nbuf}, sizeof {nbuf}, \"{spec}\", {cast}({expr})); fwrite({nbuf}, 1, {nlen}, stdout); }}"
                    ));
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
            let mut covered = Vec::with_capacity(*count as usize);
            for i in 0..*count as usize {
                let lo = i * esize;
                let hi = lo + esize;
                if hi > bytes.len() {
                    return Err(EmitError::Unsupported(
                        "ConstMemoryStatic byte image shorter than its element array",
                    ));
                }
                covered.push((lo, hi));
                parts.push(bytes_to_typed_literal(
                    model,
                    names,
                    syms,
                    &bytes[lo..hi],
                    *elem,
                    true,
                )?);
            }
            // A relocation in a trailing byte beyond the `count * esize` element
            // region (or anywhere when `elem` is ZST) would be silently dropped
            // from the array initializer -> fail loud.
            reject_uncovered_relocs(bytes, &covered)?;
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
/// and recurse; a `Ty::Union` byte image initializes the always-emitted
/// byte-covering `_raw` union member (d2.3a Change B; a `Byte::Reloc` in the
/// image is a Phase-1 limitation -- a `char[]` cannot hold an address
/// constant); an `Enum` initializes the multi-view `_raw` member (no `Reloc`)
/// or reconstructs the inline fields. `at_init` controls the array cast
/// omission. The walk is purely positional + layout-driven; no tag /
/// discriminant re-derivation.
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
        Ty::Union { .. } => union_from_bytes(model, names, bytes, consumer_ty),
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
    // `decode_scalar_bits` rejects a `Byte::Reloc` inside the `size`-byte prefix;
    // a trailing reloc (an overlong image) would otherwise be ignored -> fail
    // loud so no relocation is ever silently dropped (a scalar holds no address).
    reject_uncovered_relocs(bytes, &[(0, size)])?;
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
    // The pointer occupies `bytes[..size]`; a `Byte::Reloc` in a trailing byte
    // (an overlong image) would be ignored -> fail loud so no relocation is
    // silently dropped (the relocation must be the single pointer-width run).
    reject_uncovered_relocs(bytes, &[(0, size)])?;
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
    let mut covered = Vec::new();
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
        covered.push((off, off + fsize));
        let recon =
            bytes_to_typed_literal(model, names, syms, &bytes[off..off + fsize], fty, true)?;
        parts.push(format!(".{fname} = {recon}"));
    }
    // A relocation in inter-field / trailing padding (or a ZST field) is outside
    // every typed field slice -> it would be silently dropped from the
    // reconstructed literal. A relocation occupies a pointer slot (never
    // padding), so fail loud instead (soundness; a relocation in a covered slice
    // is resolved by that field's recursion).
    reject_uncovered_relocs(bytes, &covered)?;
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
    let mut covered = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let lo = i * esize;
        let hi = lo + esize;
        if hi > bytes.len() {
            return Err(EmitError::Unsupported(
                "array element slice exceeds the byte image",
            ));
        }
        covered.push((lo, hi));
        parts.push(bytes_to_typed_literal(
            model,
            names,
            syms,
            &bytes[lo..hi],
            elem,
            true,
        )?);
    }
    // A relocation in a byte beyond the `n * esize` element region (an overlong
    // image), or anywhere when `elem` is ZST (no element slice covers a byte),
    // would be silently dropped -> fail loud (a reloc must land in a typed slot).
    reject_uncovered_relocs(bytes, &covered)?;
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

/// Reconstruct a `Ty::Union` consumer from its byte image (d2.3a Change B):
/// `(union U){ .<raw> = { b0, .., bN-1 } }`, initializing the always-emitted
/// byte-covering `_raw` member ([`layout_emit::union_raw_member`], the single
/// source of truth for the member name). Sound gnu11 union type-punning under
/// the pinned `-std=gnu11` (no `-fno-strict-aliasing`): runtime reads go
/// through a typed member of the same union object, never an
/// inactive-member-address deref. A `Byte::Reloc` in the image is a Phase-1
/// limitation ([`raw_byte_image`] fails loud) -- a `char[]` cannot hold an
/// address constant. The compound-literal form is valid at both value and
/// static-init positions, so no `at_init` special-casing.
fn union_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    bytes: &[Byte],
    union_ty: TyId,
) -> Result<String, EmitError> {
    let raw = layout_emit::union_raw_member(model, union_ty).ok_or(EmitError::Unsupported(
        "Const::Bytes union image without a _raw union member",
    ))?;
    let agg = names.agg_name(union_ty);
    Ok(format!(
        "(union {agg}){{ .{raw} = {} }}",
        raw_byte_image(bytes)?
    ))
}

/// The niche-tagged-relocation residual message (d2.3b Change E): an
/// external-rustc-invariant DEFENSIVE guard, not a shipped gap. rustc
/// niche-fills only fieldless variants, so a niched (non-untagged) variant
/// carries no payload, let alone a relocation; the validator does not enforce
/// this (`layout_validator.rs:554`), so the byte-image serializer keeps a hard
/// fail-loud rather than a silent miscompile. A multi-view tag-view (`Direct`)
/// reloc uses the `_c<idx>` typed full view instead.
const NICHE_TAGGED_RELOC_MSG: &str = "niche enum const with a relocation payload in a niched (non-untagged) variant: not expected (rustc niche-fills only fieldless variants); a Direct reloc uses the _c<idx> view";

/// Reconstruct an `Enum` consumer from its byte image. An inline enum routes a
/// NO-relocation image to its anonymous-union `<raw>` uniformly (d2.3b Change B;
/// closes the inline bool-niche / nested-niche cases) and a `Byte::Reloc`-
/// bearing image to the typed single-view reconstruction (a reloc cannot enter
/// `<raw>`). A multi-view enum initializes the section 13 `_raw` member directly
/// for a no-relocation image; a `Byte::Reloc`-bearing image of a tag-view
/// (`Direct`) enum routes its tag + relocation through the `_c<idx>` typed full
/// view (d2.3b Change D); a `Byte::Reloc`-bearing image of a multi-view NICHE
/// enum routes the **untagged** variant's reloc through its `u._v<untagged>`
/// typed view (d2.3b Fix #2, via `reconstruct_payload_variant_fields`), and only
/// a reloc in a genuinely *niched* (non-untagged) variant's slot is the
/// niche-tagged-reloc defensive guard.
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
    let has_reloc = bytes.iter().any(|b| matches!(b, Byte::Reloc(_)));
    if enum_emit::enum_is_inline(model, shape) {
        // Inline: a no-relocation byte image initializes `<raw>` uniformly
        // (Change B); a reloc-bearing image keeps the typed single-view
        // reconstruction (the single view IS the whole object). A zero-view
        // inline enum has no `<raw>` -> the typed path handles it.
        if !has_reloc {
            if let Some(raw) = enum_emit::enum_raw_member(model, shape) {
                let agg = names.agg_name(enum_ty);
                return Ok(format!(
                    "(struct {agg}){{ .{raw} = {} }}",
                    raw_byte_image(bytes)?
                ));
            }
        }
        return reconstruct_inline_enum_from_bytes(model, names, syms, bytes, enum_ty, shape);
    }
    if has_reloc {
        // Change D: a multi-view tag-view (`Direct`) reloc byte image routes its
        // tag + relocation through the `_c<idx>` typed full view.
        if matches!(
            enum_emit::enum_tag(model, shape),
            enum_emit::EnumTag::View { .. }
        ) {
            return reconstruct_multiview_reloc_from_bytes(
                model, names, syms, bytes, enum_ty, shape,
            );
        }
        // A multi-view NICHE enum carrying a relocation (d2.3b Fix #2): route the
        // UNTAGGED variant's reloc through its `u._v<untagged>` typed view -- the
        // bytes analog of the value-position M1-lite (d2.3a). The single source of
        // truth `reconstruct_payload_variant_fields` reconstructs the untagged
        // variant's fields (a reloc lands in a typed pointer field) and its own
        // `reject_uncovered_relocs` fails loud if the reloc instead lands in a
        // *niched* (non-untagged) variant's slot -- the niche-tagged-reloc
        // external-invariant residual (rustc niche-fills only fieldless variants).
        return reconstruct_payload_variant_fields(model, names, syms, bytes, enum_ty, shape);
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

/// Reconstruct a multi-view tag-view (`Direct`) enum from a `Byte::Reloc`-
/// bearing byte image (d2.3b Change D, the `static` form of #2-reloc). The
/// active variant is determined by DECODING the `Direct` tag slice
/// ([`enum_tag_location`]'s offset + width) with the target-endian scalar-byte
/// decoder and matching it against each `variant.tag_value` -- a `Byte::Reloc`
/// inside the tag slice, a slice shorter than the tag width, or no matching
/// `tag_value` is a malformed image (fail loud, never a silent default). Each
/// non-ZST payload field is reconstructed via [`bytes_to_typed_literal`] (a
/// `Byte::Reloc` field -> the reference carrier -> `&Y`, landing in the typed
/// pointer slot), spelled through the [`enum_emit::enum_variant_typed_view`]
/// navigator: `(struct E){ .u._c<v> = { .<tag> = <tv>, .<field> = .. } }` --
/// one union member, no double-init.
fn reconstruct_multiview_reloc_from_bytes(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    enum_ty: TyId,
    shape: &EnumShape,
) -> Result<String, EmitError> {
    let agg = names.agg_name(enum_ty);
    // Determine the active variant the same way the CMS re-typing scan does
    // (the shared [`direct_enum_active_variant`], so they cannot drift), then
    // write the variant's `tag_value` CERTIFICATE (not the decoded bits) -- the
    // same source `lower_direct_reloc_to_typed_view` (the value-position form)
    // uses.
    let v = direct_enum_active_variant(model, shape, bytes).ok_or(EmitError::Unsupported(
        "multi-view reloc byte image: could not determine the active variant from the tag (malformed byte image)",
    ))?;
    let tv = variant_layout(shape, v)
        .and_then(|vl| vl.tag_value)
        .ok_or(EmitError::Unsupported(
            "multi-view reloc byte image variant without a tag value",
        ))?;
    let view = enum_emit::enum_variant_typed_view(model, enum_ty, v).ok_or(
        EmitError::Unsupported("multi-view reloc byte image variant without a typed full view"),
    )?;
    let tvr = render_tag_value(model, names, view.tag_ty, tv)?;
    let (toff, twidth) = enum_tag_location(model, shape).ok_or(EmitError::Unsupported(
        "multi-view reloc byte image without a tag cell location",
    ))?;
    let mut inner = vec![format!(".{} = {tvr}", view.tag_name)];
    let mut covered = vec![(toff, toff + twidth)];
    for f in &view.fields {
        if layout_emit::is_zst(model, f.ty) {
            continue;
        }
        let fsize = model.size_align_of(f.ty).size as usize;
        let off = f.offset as usize;
        if off + fsize > bytes.len() {
            return Err(EmitError::Unsupported(
                "multi-view enum payload field slice exceeds the byte image",
            ));
        }
        covered.push((off, off + fsize));
        let recon =
            bytes_to_typed_literal(model, names, syms, &bytes[off..off + fsize], f.ty, true)?;
        inner.push(format!(".{} = {recon}", f.name));
    }
    // A relocation in inter-field / trailing padding (or a ZST payload field) is
    // outside every typed slice -> fail loud rather than silently drop it.
    reject_uncovered_relocs(bytes, &covered)?;
    Ok(format!(
        "(struct {agg}){{ .u.{} = {{ {} }} }}",
        view.member,
        inner.join(", ")
    ))
}

/// The variant index whose `tag_value` matches the decoded `Direct` tag bits
/// (`tag_value mod 2^(8*twidth) == tag_bits`), for the Change-D bytes path.
/// `None` for no match (a malformed byte image). The tag bits are already
/// masked to the tag width by [`decode_scalar_bits`].
fn match_variant_by_tag(shape: &EnumShape, tag_bits: u128, twidth: usize) -> Option<usize> {
    let mask = if twidth >= 16 {
        u128::MAX
    } else {
        (1u128 << (8 * twidth)) - 1
    };
    let variants: &[VariantLayout] = match shape {
        EnumShape::Multiple { variants, .. } => variants,
        EnumShape::Single { variant, .. } => std::slice::from_ref(variant),
        EnumShape::Empty => return None,
    };
    variants
        .iter()
        .position(|v| matches!(v.tag_value, Some(tv) if (tv & mask) == tag_bits))
}

/// The active variant index of a multi-view tag-view (`Direct`) enum byte image:
/// decode the tag slice ([`enum_tag_location`], target-endian) and match it to a
/// variant's `tag_value`. `None` if not a tag-view enum, the tag slice is
/// reloc-bearing or shorter than the tag width, or no `tag_value` matches (a
/// malformed image). The **single source of truth** both the bytes-form reloc
/// emitter ([`reconstruct_multiview_reloc_from_bytes`]) and the `ConstMemoryStatic`
/// re-typing scan (`mod.rs`'s `scan_bytes`) select the active variant through, so
/// the scan re-types exactly the CMS the emission casts.
pub(super) fn direct_enum_active_variant(
    model: &RegseqModel,
    shape: &EnumShape,
    bytes: &[Byte],
) -> Option<usize> {
    let (toff, twidth) = enum_tag_location(model, shape)?;
    if toff + twidth > bytes.len() {
        return None;
    }
    // The tag is a scalar; a relocation in its slice is a malformed image.
    if bytes[toff..toff + twidth]
        .iter()
        .any(|b| matches!(b, Byte::Reloc(_)))
    {
        return None;
    }
    let tag_bits =
        decode_scalar_bits(&bytes[toff..toff + twidth], twidth, model.target.endian).ok()?;
    match_variant_by_tag(shape, tag_bits, twidth)
}

/// Render a raw byte image as an `unsigned char[]` initializer (`{ b0, ..,
/// bN-1 }`, `Uninit` -> 0). Shared by the enum `_raw` and the union `_raw`
/// no-relocation paths. A `Reloc` byte cannot be written into a `char[]` (it
/// cannot hold an address constant) -> fail loud. Reachable only via a union
/// byte image holding a reference (a union has no typed full view to route a
/// reloc through; the enum `_raw` callers pass only no-relocation images, an
/// enum reloc using the `_c<idx>` typed view instead).
fn raw_byte_image(bytes: &[Byte]) -> Result<String, EmitError> {
    let mut parts = Vec::with_capacity(bytes.len());
    for b in bytes {
        match b {
            Byte::Value(v) => parts.push(v.to_string()),
            Byte::Uninit => parts.push("0".to_string()),
            Byte::Reloc(_) => return Err(EmitError::Unsupported(
                "relocation byte in a union byte image: a char[] cannot hold an address constant",
            )),
        }
    }
    Ok(format!("{{ {} }}", parts.join(", ")))
}

/// Fail loud if any `Byte::Reloc` in `bytes` lies outside every `covered`
/// `[start, end)` byte range -- the non-ZST field / payload slices a byte-image
/// reconstruction emitted a typed designator for. A relocation occupies a
/// pointer slot, so a `Byte::Reloc` in inter-field / trailing padding or a ZST
/// region is never produced by a well-formed (rustc) image; left unchecked it
/// would be SILENTLY DROPPED from the reconstructed initializer (a soundness
/// hole). A relocation INSIDE a covered slice is resolved by that slice's
/// recursive reconstruction (`pointer_from_bytes` consumes a complete
/// pointer-width run; a scalar field rejects a stray reloc; a nested aggregate
/// re-checks its own coverage), so this per-level guard composes into a complete
/// "every reloc lands in a typed pointer slot, or fail loud" invariant.
fn reject_uncovered_relocs(bytes: &[Byte], covered: &[(usize, usize)]) -> Result<(), EmitError> {
    for (i, b) in bytes.iter().enumerate() {
        if matches!(b, Byte::Reloc(_)) && !covered.iter().any(|&(lo, hi)| i >= lo && i < hi) {
            return Err(EmitError::Unsupported(
                "relocation byte outside every typed field slice in a byte image (padding / ZST region): a relocation must land in a typed pointer slot",
            ));
        }
    }
    Ok(())
}

/// Reconstruct an inline (<= 1 view, no `union`) enum from a byte image
/// [`enum_from_bytes`] routed here -- a zero-view inline enum (no `<raw>`), or a
/// `Byte::Reloc`-bearing inline image (a reloc cannot enter `<raw>`, so the
/// typed single view carries it). A no-relocation inline image already
/// initializes `<raw>` uniformly in [`enum_from_bytes`] (Change B). A tag-only
/// view decodes the tag cell; a niche / single-variant payload view
/// reconstructs the untagged variant's inline fields (a `Byte::Reloc` lands in
/// the typed pointer field).
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
            // A fieldless tag-view inline enum has no pointer field, so a
            // `Byte::Reloc` anywhere outside the tag slice (padding / trailing)
            // is malformed -> fail loud rather than silently drop it
            // (`scalar_from_bytes` already rejects a reloc inside the tag slice).
            reject_uncovered_relocs(bytes, &[(offset, offset + size)])?;
            let recon = scalar_from_bytes(model, &bytes[offset..offset + size], scalar_ty)?;
            Ok(format!("(struct {agg}){{ .{member_path} = {recon} }}"))
        }
        enum_emit::EnumTag::Niche { .. } | enum_emit::EnumTag::None => {
            reconstruct_payload_variant_fields(model, names, syms, bytes, enum_ty, shape)
        }
    }
}

/// Reconstruct the payload-bearing untagged variant's non-ZST fields from a byte
/// image (a `Single { scalar: None }` variant, or a niche **untagged** variant),
/// designating by the navigator's member path -- `<field>` for an inline enum,
/// `u._v<untagged>.<field>` for a **multi-view** niche enum (d2.3b Fix #2: the
/// bytes analog of the value-position M1-lite). The byte image already encodes
/// the active variant, so this is byte-faithful; a `Byte::Reloc` in an untagged
/// field lands in that field's typed pointer slot, while a reloc OUTSIDE the
/// untagged variant's field slices (a *niched* variant's slot, or padding) is
/// rejected by the trailing `reject_uncovered_relocs` (the niche-tagged-reloc
/// external-invariant residual: rustc niche-fills only fieldless variants).
fn reconstruct_payload_variant_fields(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    bytes: &[Byte],
    enum_ty: TyId,
    shape: &EnumShape,
) -> Result<String, EmitError> {
    let agg = names.agg_name(enum_ty);
    let v = inline_payload_variant(shape).ok_or(EmitError::Unsupported(
        "enum byte image without an untagged payload variant",
    ))?;
    let variant = variant_layout(shape, v)
        .ok_or(EmitError::Unsupported("enum payload variant out of range"))?;
    let mut parts = Vec::new();
    let mut covered = Vec::new();
    for (idx, f) in variant.fields.iter().enumerate() {
        if layout_emit::is_zst(model, f.ty) {
            continue;
        }
        let (member_path, fty) = enum_emit::enum_variant_field(model, enum_ty, v, idx).ok_or(
            EmitError::Unsupported("enum payload field navigation failed in a byte image"),
        )?;
        let fsize = model.size_align_of(fty).size as usize;
        let off = f.offset as usize;
        if off + fsize > bytes.len() {
            return Err(EmitError::Unsupported(
                "inline enum field slice exceeds the byte image",
            ));
        }
        covered.push((off, off + fsize));
        let recon =
            bytes_to_typed_literal(model, names, syms, &bytes[off..off + fsize], fty, true)?;
        parts.push(format!(".{member_path} = {recon}"));
    }
    // A relocation in inter-field / trailing padding (or a ZST field) would be
    // silently dropped -> fail loud (a reloc must land in a typed pointer slot).
    reject_uncovered_relocs(bytes, &covered)?;
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
/// carries no active `FieldIdx` -> a defensive guard (#5: rustc emits no union
/// valtree); an enum consumer is defensively rejected (the bridge uses
/// `EnumValue` for enums). ZST children are recursed (fail-loud) but omitted
/// from designators.
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
            "Const::Aggregate over a union: not expected (rustc emits no union valtree; unions arrive as Const::Bytes)",
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
/// one C compound literal (d2.2 Change E + d2.3a/d2.3b complete every reachable
/// shape d2.1 handed off).
///
/// - inline enum (<= 1 view): a fieldless-collapsed `Direct` /
///   `Single { scalar: Some }` tag view -> `(struct E){ .tag = <tag_value> }`;
///   a `Single { scalar: None }` / niche **untagged** payload -> the payload
///   children (ZST omitted; the typed single view, so a relocation lands in the
///   typed field); a niche **niched** variant -> the anonymous-union `<raw>`
///   byte image (`serialize_enum_value_to_raw`, d2.3b Change B), so a bool-niche
///   `2` or a niche cell buried in a nested aggregate round-trips with no C-type
///   normalization.
/// - multi-view enum (a `union u` with the `_raw` + `_c<idx>` members): a
///   **fieldless** `Direct` variant stays the d2.1 single tag-member init
///   (`(struct E){ .u._tag.tag = <tv> }`); a multi-view niche **untagged**
///   variant (no `tag_value` to co-write) routes through the existing
///   `u._v<untagged>` typed view (M1-lite, d2.3a); a tag-view (`Direct`)
///   payload-bearing variant whose payload carries a **relocation** routes the
///   tag + reloc through the `_c<idx>` typed full view (d2.3b Change D,
///   `lower_direct_reloc_to_typed_view`); every other value (a no-relocation
///   payload-bearing `Direct` variant, or a niched-tagged variant) serializes
///   the tag + a **no-relocation** payload (scalar / aggregate / array /
///   `Bytes` / `NullPtr` / nested `EnumValue`) to a `layout.size` byte image via
///   `serialize_const_into` and initializes the union via
///   `(struct E){ .u._raw = { ... } }`. A relocation in a niched (non-untagged)
///   variant fails loud (the niche-tagged-reloc defensive guard).
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
            enum_emit::EnumTag::Niche { .. } => {
                let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
                    "Const::EnumValue variant index out of range",
                ))?;
                match variant.tag_value {
                    // A niched variant serializes uniformly to the
                    // anonymous-union `<raw>` byte image (Change B): the niche
                    // value at the niche cell + any payload, written as raw
                    // bytes -- so an arbitrary bit pattern (a bool-niche `2`, a
                    // niche cell buried in a nested aggregate) round-trips with
                    // no C-type normalization (`(_Bool){2}` could not). A
                    // relocation in the payload still fails loud via
                    // `serialize_const_into` (the niche-tagged-reloc guard).
                    Some(_tv) => {
                        serialize_enum_value_to_raw(model, &agg, enum_ty, shape, v, payload)
                    }
                    // The untagged variant: the payload IS the value (the typed
                    // single view; a relocation lands in the typed field).
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
                // Still lower the (all-ZST) payload children to fail loud on any
                // deferred / relocation-bearing shape wrongly attached to a ZST
                // field (a certificate violation) -- the same ZST discipline
                // `serialize_const_into` applies; all-ZST -> no designators kept.
                let _ = lower_enum_payload(model, names, syms, enum_ty, v, payload)?;
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
        // M1-lite (d2.3a Change D): a multi-view niche **untagged** variant has
        // no tag to co-write, so its whole value is its payload -- route it
        // through the EXISTING `u._v<untagged>` typed view via
        // `lower_enum_payload`. A relocation payload then lands in a typed
        // pointer field (`.u._v<v>.<f> = &Y`) -- one union member, no
        // double-init, an address constant in a pointer slot (which `_raw`, a
        // `char[]`, cannot hold). Keyed on the authoritative `untagged_variant`
        // certificate -- the same field the runtime niche read (`discriminant`)
        // and `inline_payload_variant` read -- NOT a `tag_value.is_none()` proxy:
        // `is_none()` is equivalent only under the (validator-unenforced)
        // invariant that exactly one variant is tagless, so a malformed niched
        // variant carrying `tag_value: None` must not slip into the typed view
        // (it would emit an in-range payload with no niche tag, which the runtime
        // read mis-resolves to the untagged variant). A non-untagged tagless
        // variant instead falls through and fails loud in
        // `serialize_multiview_enum_value`.
        if let EnumShape::Multiple {
            encoding: TagEncoding::Niche {
                untagged_variant, ..
            },
            ..
        } = shape
        {
            if v == untagged_variant.index() {
                let kept = lower_enum_payload(model, names, syms, enum_ty, v, payload)?;
                return Ok(enum_value_payload_literal(&agg, &kept));
            }
        }
        // Change D (d2.3b / M1): a multi-view tag-view (`Direct`) payload-bearing
        // variant whose payload (transitively) carries a relocation routes the
        // tag + the relocation through the typed full view `_c<v>` -- the reloc
        // lands in a typed pointer slot (`_raw`, a `char[]`, cannot hold an
        // address constant), in ONE union designator with no double-init. A
        // fieldless variant with a reloc on a ZST field was already caught (and
        // failed loud) by the tag-literal shortcut above. A no-relocation
        // payload still serializes to `_raw` (the fall-through, d2.3a).
        if matches!(
            enum_emit::enum_tag(model, shape),
            enum_emit::EnumTag::View { .. }
        ) && payload
            .iter()
            .any(|id| const_carries_reloc(model, model.consts.resolve(*id)))
        {
            return lower_direct_reloc_to_typed_view(
                model, names, syms, enum_ty, shape, v, payload,
            );
        }
        // Every other multi-view value (a no-relocation payload-bearing
        // `Direct` variant, or a niched-tagged niche variant) serializes to the
        // `_raw` byte image -- the union cannot be const-initialized through
        // separate `_tag` / `_v<v>` members (a second union designator
        // overrides the first).
        serialize_enum_value_to_raw(model, &agg, enum_ty, shape, v, payload)
    }
}

/// Lower a multi-view tag-view (`Direct`) enum const whose payload carries a
/// relocation through the `_c<v>` typed full view (d2.3b Change D, the
/// `Const::EnumValue` form of #2-reloc): `(struct E){ .u._c<v> = { .<tag> =
/// <render_tag_value(tv)>, .<field> = <const_expr(child)> } }`. The tag + each
/// non-ZST payload field are spelled through the
/// [`enum_emit::enum_variant_typed_view`] navigator (never re-derived); a
/// relocation child lowers via `const_expr`'s reference-carrier / reloc arm
/// (-> `&Y` / the fat carrier), landing in the TYPED field -- nested relocs land
/// via nested designators. A relocation wrongly attached to a ZST / omitted
/// field is a layout certificate violation (fail loud, never silently dropped),
/// matching [`lower_enum_payload`].
fn lower_direct_reloc_to_typed_view(
    model: &RegseqModel,
    names: &NameTable,
    syms: &InstanceSymbols,
    enum_ty: TyId,
    shape: &EnumShape,
    v: usize,
    payload: &[ConstId],
) -> Result<String, EmitError> {
    let agg = names.agg_name(enum_ty);
    let tv = variant_layout(shape, v)
        .and_then(|vl| vl.tag_value)
        .ok_or(EmitError::Unsupported(
            "Direct reloc enum const for a variant without a tag value",
        ))?;
    let view = enum_emit::enum_variant_typed_view(model, enum_ty, v).ok_or(
        EmitError::Unsupported("Direct reloc enum const for a variant without a typed full view"),
    )?;
    let tvr = render_tag_value(model, names, view.tag_ty, tv)?;
    let mut inner = vec![format!(".{} = {tvr}", view.tag_name)];
    for (i, child_id) in payload.iter().enumerate() {
        let field = view.fields.get(i).ok_or(EmitError::Unsupported(
            "Direct reloc enum const payload exceeds the variant field count",
        ))?;
        let child = model.consts.resolve(*child_id);
        if layout_emit::is_zst(model, field.ty) {
            // A relocation occupies a pointer slot, never a zero-sized field;
            // fail loud rather than drop the address (matching lower_enum_payload).
            if const_carries_reloc(model, child) {
                return Err(EmitError::Unsupported(
                    "relocation attached to a zero-sized enum payload field (layout certificate violation)",
                ));
            }
            let _ = const_expr(model, names, syms, child, Some(field.ty), true)?;
            continue;
        }
        let e = const_expr(model, names, syms, child, Some(field.ty), true)?;
        inner.push(format!(".{} = {e}", field.name));
    }
    Ok(format!(
        "(struct {agg}){{ .u.{} = {{ {} }} }}",
        view.member,
        inner.join(", ")
    ))
}

/// Serialize a no-relocation `Const` of type `ty` into `image[offset..]` as its
/// target byte representation (d2.3a Change C) -- the const -> byte-image
/// inverse of [`bytes_to_typed_literal`], purely positional + layout-driven:
///
/// - scalar (`Bool`/`Char`/`Int`/`Uint`/`Float`) -> [`const_scalar_bits`] +
///   [`write_bits_into`] over `size_of(ty)`, endian per `endian`.
/// - `NullPtr` -> the zero bit pattern over `size_of(ty)` (a null pointer).
/// - `ZST` -> no bytes (a zero-sized value).
/// - `Bytes` -> the bytes at `offset..` (`Value(v)`->`v`, `Uninit`->`0`,
///   `Reloc` -> fail loud #2-reloc), bounds-checked against the field width.
/// - `Aggregate` over `Tuple`/`Struct`/`Closure` -> each child at
///   `offset + FieldLayout.offset` in source order; over `Array(elem, n)` ->
///   each child at `offset + i*size_of(elem)`. ZST fields recurse (so a
///   deferred shape fails loud) but write zero bytes. Over `Ty::Ref` (a
///   reference carrier -> reloc, #2-reloc) / `Ty::Union` (a `Const::Aggregate`
///   over a union is #5 -- defensive) -> fail loud.
/// - `EnumValue` (a nested enum payload) -> the nested enum's `tag_value` (when
///   `Some`) at its [`enum_tag_location`] over the tag width, then each payload
///   field via `serialize_const_into` at its `FieldLayout.offset` (all
///   `+ offset`; a ZST field recurses with zero width). Inline vs. multi-view is
///   irrelevant -- the byte image is representation-independent.
/// - `Reloc` -> fail loud (#2-reloc): a byte image cannot hold an address.
///
/// All offsets are absolute into `image`; every write is bounds-checked
/// ([`write_bits_into`] / the `Bytes` arm), so a layout / size inconsistency
/// fails loud rather than silently miscompiling.
fn serialize_const_into(
    model: &RegseqModel,
    image: &mut [u8],
    offset: usize,
    c: &Const,
    ty: TyId,
    endian: Endian,
) -> Result<(), EmitError> {
    match c {
        Const::Bool(_)
        | Const::Char(_)
        | Const::Int(_, _)
        | Const::Uint(_, _)
        | Const::Float(_, _) => {
            let bits = const_scalar_bits(c).ok_or(EmitError::Unsupported(
                "scalar const without a bit pattern (serialize_const_into)",
            ))?;
            let width = model.size_align_of(ty).size as usize;
            write_bits_into(image, offset, width, bits, endian)
        }
        Const::NullPtr(_) => {
            let width = model.size_align_of(ty).size as usize;
            write_bits_into(image, offset, width, 0, endian)
        }
        Const::ZST => Ok(()),
        Const::Bytes(bytes) => {
            let width = model.size_align_of(ty).size as usize;
            if offset + width > image.len() || bytes.len() > width {
                return Err(EmitError::Unsupported(
                    "Const::Bytes payload exceeds the field width (serialize_const_into)",
                ));
            }
            for (i, b) in bytes.iter().enumerate() {
                image[offset + i] = match b {
                    Byte::Value(v) => *v,
                    Byte::Uninit => 0,
                    Byte::Reloc(_) => return Err(EmitError::Unsupported(NICHE_TAGGED_RELOC_MSG)),
                };
            }
            Ok(())
        }
        Const::Aggregate(children) => {
            serialize_aggregate_into(model, image, offset, children, ty, endian)
        }
        Const::EnumValue {
            variant_idx,
            payload,
        } => {
            let shape = enum_emit::enum_shape(model, ty).ok_or(EmitError::Unsupported(
                "nested Const::EnumValue payload over a non-enum type",
            ))?;
            let v = variant_idx.index();
            let variant = variant_layout(shape, v).ok_or(EmitError::Unsupported(
                "nested Const::EnumValue variant index out of range",
            ))?;
            if let Some(tv) = variant.tag_value {
                let (toff, twidth) = enum_tag_location(model, shape).ok_or(
                    EmitError::Unsupported("nested enum value without a tag cell location"),
                )?;
                write_bits_into(image, offset + toff, twidth, tv, endian)?;
            }
            for (i, child_id) in payload.iter().enumerate() {
                let field = variant.fields.get(i).ok_or(EmitError::Unsupported(
                    "nested Const::EnumValue payload exceeds the variant field count",
                ))?;
                // Recurse every payload child (ZST included: it writes zero
                // bytes via its zero width, but a deferred / reloc-bearing shape
                // wrongly attached to a ZST field still fails loud).
                let (foff, fty) = (field.offset as usize, field.ty);
                let child = model.consts.resolve(*child_id);
                serialize_const_into(model, image, offset + foff, child, fty, endian)?;
            }
            Ok(())
        }
        Const::Reloc(_) => Err(EmitError::Unsupported(NICHE_TAGGED_RELOC_MSG)),
    }
}

/// Serialize a `Const::Aggregate` of type `ty` into `image[offset..]` (the
/// aggregate arm of [`serialize_const_into`]): a `Tuple` / `Struct` / `Closure`
/// recurses each child at its `FieldLayout.offset` in source / MIR order; an
/// `Array(elem, n)` recurses each child at `offset + i*size_of(elem)`. A
/// `Ty::Ref` is a reference carrier (a relocation a `char[]` cannot hold ->
/// fail loud; a `Direct` reloc uses the `_c<idx>` typed view, so this is the
/// niche-tagged-reloc defensive guard); a `Ty::Union` is the #5 defensive guard
/// (a `Const::Aggregate` carries no active field).
fn serialize_aggregate_into(
    model: &RegseqModel,
    image: &mut [u8],
    offset: usize,
    children: &[ConstId],
    ty: TyId,
    endian: Endian,
) -> Result<(), EmitError> {
    match &model.types[ty] {
        Ty::Tuple { .. } | Ty::Struct { .. } | Ty::Closure(_, _) => {
            let fields = agg_field_layouts(model, ty).ok_or(EmitError::Unsupported(
                "Const::Aggregate over an unresolved aggregate (serialize_const_into)",
            ))?;
            if children.len() != fields.len() {
                return Err(EmitError::Unsupported(
                    "Const::Aggregate child count does not match the field count (serialize_const_into)",
                ));
            }
            let positions: Vec<(usize, TyId)> =
                fields.iter().map(|f| (f.offset as usize, f.ty)).collect();
            for (child_id, (foff, fty)) in children.iter().zip(positions) {
                let child = model.consts.resolve(*child_id);
                serialize_const_into(model, image, offset + foff, child, fty, endian)?;
            }
            Ok(())
        }
        Ty::Array(elem, n) => {
            let elem = *elem;
            let n = *n;
            if children.len() != n as usize {
                return Err(EmitError::Unsupported(
                    "Const::Aggregate array child count does not match the length (serialize_const_into)",
                ));
            }
            let esize = model.size_align_of(elem).size as usize;
            for (i, child_id) in children.iter().enumerate() {
                let child = model.consts.resolve(*child_id);
                serialize_const_into(model, image, offset + i * esize, child, elem, endian)?;
            }
            Ok(())
        }
        Ty::Ref(_, _) => Err(EmitError::Unsupported(NICHE_TAGGED_RELOC_MSG)),
        Ty::Union { .. } => Err(EmitError::Unsupported(
            "Const::Aggregate over a union: not expected (rustc emits no union valtree; unions arrive as Const::Bytes)",
        )),
        _ => Err(EmitError::Unsupported(
            "Const::Aggregate for an unsupported byte-image consumer type (serialize_const_into)",
        )),
    }
}

/// Serialize a fully-known enum value to a `layout.size` byte image and emit
/// `(struct E){ .<raw> = { ... } }` (d2.2 Change E; d2.3a Change C; generalized
/// d2.3b Change B to any enum with a `<raw>` member + a tag location -- a
/// multi-view enum's `u._raw`, or a one-view inline enum's anonymous-union
/// `<raw>`, e.g. an inline niched variant). Mirrors `assign_enum`'s
/// tag-then-payload write, but into bytes: the variant's `tag_value` (when
/// present) at the tag cell over its raw storage width, then each payload child
/// recursively byte-serialized at its `FieldLayout.offset` via
/// [`serialize_const_into`] (a ZST field recurses with zero width; so a
/// no-relocation aggregate / array / `Bytes` / `NullPtr` / nested `EnumValue`
/// child also lands in `<raw>`), endian per `model.target.endian`. Any
/// relocation in the payload (a `Reloc` / reference carrier / `Byte::Reloc`,
/// possibly nested) cannot be encoded into a `char[]` -> fail loud (the
/// niche-tagged-reloc defensive guard; a multi-view `Direct` reloc uses the
/// `_c<idx>` typed view instead). `<raw>` is written through the
/// [`enum_emit::enum_raw_member`] navigator path (the single source of truth).
fn serialize_enum_value_to_raw(
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

    // Tag write: every multi-view value reaching the `_raw` serializer carries a
    // `tag_value`. A fieldless `Direct` variant is the tag-literal path; the
    // niche **untagged** variant (the only tagless multi-view value) is routed to
    // the typed view by M1-lite (keyed on `untagged_variant`). So a tagless value
    // here is a malformed niche shape (a non-untagged variant with
    // `tag_value: None`) -> fail loud, rather than emit a tagless `_raw` image
    // that the runtime niche read would mis-resolve to the untagged variant.
    let tv = variant.tag_value.ok_or(EmitError::Unsupported(
        "multi-view enum _raw serialization for a tagless non-untagged variant (malformed niche shape)",
    ))?;
    let (offset, width) = enum_tag_location(model, shape).ok_or(EmitError::Unsupported(
        "multi-view enum value without a tag cell location",
    ))?;
    write_bits_into(&mut image, offset, width, tv, endian)?;

    // Payload write: each field's value byte-serialized at its offset
    // (recursive: a no-reloc aggregate / array / Bytes / NullPtr / nested
    // EnumValue child also serializes into the image; any reloc fails loud). A
    // ZST field recurses too (zero width writes nothing, but a deferred /
    // reloc-bearing shape wrongly attached to it still fails loud).
    for (i, child_id) in payload.iter().enumerate() {
        let field = variant.fields.get(i).ok_or(EmitError::Unsupported(
            "Const::EnumValue payload exceeds the variant field count",
        ))?;
        let child = model.consts.resolve(*child_id);
        serialize_const_into(
            model,
            &mut image,
            field.offset as usize,
            child,
            field.ty,
            endian,
        )?;
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
/// own d2.2 arm -- but contribute no designator (matching the view). A
/// **relocation** wrongly attached to an omitted (ZST) field is a certificate
/// violation (a relocation occupies a pointer slot, never a zero-sized field):
/// it fails loud rather than being silently discarded with no designator
/// (d2.3a soundness; `const_expr` already fails loud on a `Bytes` / `Aggregate`
/// / `EnumValue` child with no consumer type, but a bare `Reloc` / `NullPtr`
/// would otherwise succeed and be dropped).
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
                if const_carries_reloc(model, child) {
                    return Err(EmitError::Unsupported(
                        "relocation attached to a zero-sized enum payload field (layout certificate violation)",
                    ));
                }
                let _ = const_expr(model, names, syms, child, None, true)?;
            }
        }
    }
    Ok(inits)
}

/// Whether `c` is or (transitively) contains a relocation (`Reloc` or a
/// `Byte::Reloc`). A relocation occupies a pointer slot, so it can never be
/// soundly discarded when attached to a ZST / omitted enum payload field --
/// [`lower_enum_payload`] uses this to keep that discard fail-loud rather than
/// dropping an address constant. The const graph is acyclic (`const_value.rs`),
/// so the recursion terminates.
fn const_carries_reloc(model: &RegseqModel, c: &Const) -> bool {
    match c {
        Const::Reloc(_) => true,
        Const::Bytes(bytes) => bytes.iter().any(|b| matches!(b, Byte::Reloc(_))),
        Const::Aggregate(children)
        | Const::EnumValue {
            payload: children, ..
        } => children
            .iter()
            .any(|id| const_carries_reloc(model, model.consts.resolve(*id))),
        _ => false,
    }
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

/// Emit the section 13 / 14.7 niche-`Discriminant` read of the enum at C lvalue
/// `enum_lv` (logical type `enum_ty`) into `dst` (a C lvalue of type `dst_ty`),
/// using `tmp` as the raw-storage scratch name. Returns the brace-delimited
/// statement block `{ <tmp_cty> <tmp>; memcpy(...); <dst> = <ternary>; }`.
///
/// The single source of truth for the niche read, shared by the assign-site
/// `Discriminant` lowering ([`BodyEmit::assign_niche_discriminant`]) and the
/// regseq-backends-28 errno boundary adapter ([`emit_exit_adapter`], which reads
/// the `Result<(), RegseqError>` discriminant here). The niche cell is read by a
/// memcpy into a raw unsigned temp (so an invalid-bit-pattern niche tag such as
/// `Option<bool>`'s `None == 2` round-trips and compares correctly against
/// `tag_value`, also a raw bit pattern mod `2^width`), then a ternary over the
/// niched variants falls through to the untagged variant's `discriminant`.
pub(super) fn niche_discriminant_read_block(
    model: &RegseqModel,
    enum_ty: TyId,
    enum_lv: &str,
    dst: &str,
    dst_ty: TyId,
    tmp: &str,
) -> Result<String, EmitError> {
    let shape = enum_emit::enum_shape(model, enum_ty).ok_or(EmitError::Unsupported(
        "niche Discriminant on a non-enum place",
    ))?;
    let (untagged, variants) = match shape {
        EnumShape::Multiple {
            encoding: TagEncoding::Niche {
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
    let (offset, scalar_ty) = match enum_emit::enum_tag(model, shape) {
        enum_emit::EnumTag::Niche { offset, scalar_ty } => (offset, scalar_ty),
        _ => {
            return Err(EmitError::Unsupported(
                "niche enum without a niche cell (malformed layout)",
            ))
        }
    };
    let untagged_disc = variants
        .get(untagged)
        .ok_or(EmitError::Unsupported(
            "niche untagged variant index out of range",
        ))?
        .discriminant;
    let storage_kind = niche_storage_kind(model, scalar_ty)?;
    let mut expr = render_discriminant(model, dst_ty, untagged_disc)?;
    for v in variants.iter().rev() {
        if let Some(tv) = v.tag_value {
            let tvr = render_uint(storage_kind, tv);
            let d = render_discriminant(model, dst_ty, v.discriminant)?;
            expr = format!("({tmp} == {tvr} ? {d} : {expr})");
        }
    }
    let tmp_cty = uint_ctype(storage_kind);
    Ok(format!(
        "{{ {tmp_cty} {tmp}; memcpy(&{tmp}, (const unsigned char *)&({enum_lv}) + {offset}, \
         sizeof({tmp})); {dst} = {expr}; }}"
    ))
}

/// Emit the hand-authored `RegseqError` -> libc errno boundary adapter
/// (regseq-error contract C03), reusing the section 13 discriminant-extraction
/// machinery for both reads. This is the regseq-backends-28 mechanism resolved
/// by the open-item O3 spike: the symbolic errno `switch` is hand-emitted C
/// (model constants are numeric / relocs, with no symbolic-macro form, and
/// lowered body returns emit `_0` / `return;`, so a synthesized model-level
/// adapter body could not emit `return EINVAL;`), while the two discriminant
/// reads route through the shared section 13 helpers.
///
/// The emitted adapter `<adapter>(void)`:
/// 1. calls the faithfully-emitted entry, capturing the section 13
///    `Result<(), RegseqError>` C struct in `__r`;
/// 2. reads the `Result` niche discriminant ([`niche_discriminant_read_block`])
///    and, on the `Ok` discriminant, `return 0;` (R04);
/// 3. otherwise reads the inner `RegseqError` Direct tag (the section 13 tag
///    view) and `switch`es it, emitting `return <SYMBOL>;` for every variant
///    keyed by `VariantLayout.name` through the C02 table ([`errno_symbol`]),
///    with `default: return EIO;` as corrupt-discriminant defense only (I05).
///
/// A marked variant absent from the C02 table fails loud with
/// [`EmitError::ErrnoMapping`] (never a silent `EIO`, R08 / I05). Emitted codes
/// are symbolic (`return EINVAL;`); the `<errno.h>` include is recorded as
/// demand-driven by the caller ([`super::emit_generic_c`]).
pub(super) fn emit_exit_adapter(
    model: &RegseqModel,
    names: &NameTable,
    adapter: &str,
    entry: &str,
    result_ty: TyId,
    err_enum_ty: TyId,
) -> Result<String, EmitError> {
    // The `Result<(), RegseqError>` niche enum: the niche cell scalar (for the
    // discriminant temp's C type + the `Ok` test) and the `Ok` variant (index 0,
    // rustc's `Result` order) discriminant.
    let result_shape = enum_emit::enum_shape(model, result_ty).ok_or(EmitError::Unsupported(
        "errno adapter: the entry return type is not an enum",
    ))?;
    let niche_scalar_ty = match enum_emit::enum_tag(model, result_shape) {
        enum_emit::EnumTag::Niche { scalar_ty, .. } => scalar_ty,
        _ => {
            return Err(EmitError::Unsupported(
                "errno adapter: the Result entry return is not niche-encoded",
            ))
        }
    };
    let ok_disc = match result_shape {
        EnumShape::Multiple { variants, .. } if !variants.is_empty() => variants[0].discriminant,
        _ => {
            return Err(EmitError::Unsupported(
                "errno adapter: the Result return is not a multi-variant enum",
            ))
        }
    };

    // The Err payload (variant index 1, rustc's `Result` order) -> the inner
    // RegseqError lvalue, then its Direct tag view (section 13).
    let (err_member, _err_field_ty) = enum_emit::enum_variant_field(model, result_ty, 1, 0).ok_or(
        EmitError::Unsupported("errno adapter: the Result Err payload field was not found"),
    )?;
    let err_shape = enum_emit::enum_shape(model, err_enum_ty).ok_or(EmitError::Unsupported(
        "errno adapter: RegseqError is not an enum",
    ))?;
    let (tag_member, tag_scalar_ty) = match enum_emit::enum_tag(model, err_shape) {
        enum_emit::EnumTag::View {
            member_path,
            scalar_ty,
        } => (member_path, scalar_ty),
        _ => {
            return Err(EmitError::Unsupported(
                "errno adapter: RegseqError is not a Direct (tag-view) enum",
            ))
        }
    };
    let err_variants = match err_shape {
        EnumShape::Multiple { variants, .. } => variants.as_slice(),
        _ => {
            return Err(EmitError::Unsupported(
                "errno adapter: RegseqError is not a multi-variant enum",
            ))
        }
    };

    let result_decl = types::declare(model, names, result_ty, "__r");
    let disc_cty = types::c_type(model, names, niche_scalar_ty);
    let niche_block =
        niche_discriminant_read_block(model, result_ty, "__r", "__disc", niche_scalar_ty, "__nd")?;
    let ok_disc_render = render_discriminant(model, niche_scalar_ty, ok_disc)?;
    let tag_read = format!("__r.{err_member}.{tag_member}");

    // One `case <tag_value>: return <SYMBOL>;` per RegseqError variant, in variant
    // order (deterministic). An unknown marked variant fails loud (R08 / I05).
    let mut cases = String::new();
    for v in err_variants {
        let tv = v.tag_value.ok_or(EmitError::Unsupported(
            "errno adapter: RegseqError variant without a tag_value (malformed Direct layout)",
        ))?;
        let case_lit = render_tag_value(model, names, tag_scalar_ty, tv)?;
        let name = model.strings.resolve(v.name);
        let symbol = errno_symbol(name).ok_or_else(|| EmitError::ErrnoMapping(name.to_string()))?;
        cases.push_str(&format!("    case {case_lit}: return {symbol};\n"));
    }

    Ok(format!(
        "\n/* ---------------------------------------------------------------------------\n\
         \x20* RegseqError -> errno boundary adapter (regseq-error C02 / C03).\n\
         \x20* --------------------------------------------------------------------------- */\n\
         int {adapter}(void) {{\n\
         \x20\x20\x20\x20{result_decl} = {entry}();\n\
         \x20\x20\x20\x20{disc_cty} __disc;\n\
         \x20\x20\x20\x20{niche_block}\n\
         \x20\x20\x20\x20if (__disc == {ok_disc_render}) {{\n\
         \x20\x20\x20\x20\x20\x20\x20\x20return 0;\n\
         \x20\x20\x20\x20}}\n\
         \x20\x20\x20\x20switch ({tag_read}) {{\n\
         {cases}\
         \x20\x20\x20\x20default: return EIO;\n\
         \x20\x20\x20\x20}}\n\
         }}\n"
    ))
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
pub(super) fn render_tag_value(
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
pub(super) fn render_discriminant(
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
