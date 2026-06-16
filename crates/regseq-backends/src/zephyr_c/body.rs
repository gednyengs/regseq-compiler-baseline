//! Subset MIR-body lowering for the zephyr_c backend.
//!
//! Lowers a function instance's post-monomorphization body to Zephyr C:
//! locals declared up front, CFG-flattened basic blocks with `bb_<n>:`
//! labels and `goto` terminators (the same flattening discipline the
//! reference backend certifies), `<stdint.h>` type spelling throughout.
//!
//! The supported construct set grows task by task (T05 scaffold → T06
//! register access → T07 lifecycle bodies → T08 diag-log); anything outside
//! it is a loud [`EmitError::Unsupported`] naming the construct — never
//! wrong code (I06). Intrinsic-anchored calls (Log/Mmio/Config/PollUntil)
//! are intercepted by callee item attribute before ordinary call lowering;
//! the Config arm lowers the licensed C02/C03 shapes on the PCIe arm (the
//! `config_shape` classifier — shared with the D12 frame scan, which fires
//! first on every `emit_zephyr_c` path) and the PollUntil arm mirrors the
//! scan's D.2 rejection defensively.
//!
//! Structural sibling of `linux_c::body` (F-1: the lowering MECHANISM —
//! reaching-def receiver chase, descriptor const-folding, opaque enum
//! carriers — is the certified shape; no code is shared, and every
//! OS-facing bank is Zephyr-native: `sys_read*`/`sys_write*` value-first
//! accessors over the TU-static `mm_reg_t` base (A-2), `LOG_*` reassembly,
//! `k_panic()` traps).

use std::collections::BTreeMap;

use regseq_model::{
    AggregateKind, AttrData, BinOp, CallCallee, CastKind, Const, EnumShape, InstanceId,
    InstanceKind, IntrinsicKind, LayoutKind, LocalId, Operand, Place, PlaceElem, RegseqModel,
    Rvalue, StatementKind, TerminatorKind, Ty, TyId, UnOp,
};

use super::types::{self, TypeNames};
use super::EmitError;

/// Per-TU lowering context shared by every emitted function.
pub(crate) struct LowerCx<'m> {
    pub model: &'m RegseqModel,
    pub names: TypeNames,
    /// C symbol per lowered instance.
    pub fn_symbols: BTreeMap<InstanceId, String>,
    /// Demand-recorded extra includes (e.g. `<zephyr/kernel.h>` for
    /// `k_panic()`).
    pub extra_includes: std::collections::BTreeSet<&'static str>,
    /// Driver facts for register-access lowering (T06).
    pub driver: DriverFacts,
    /// Whether any width-8 MMIO op was lowered (D19/C-2: aarch64-validated;
    /// the TU carries the arch-conditionality note and a target arch lacking
    /// `sys_*64` fails the C compile loud).
    pub width8_used: bool,
    /// Whether any `LOG_*` call was lowered (the TU then carries the
    /// demand-driven log-module block, scaffold §1 item 2).
    pub uses_log: bool,
    /// Per-function: locals holding elided trivial-accessor results.
    pub elided_locals: std::collections::BTreeSet<LocalId>,
}

impl<'m> LowerCx<'m> {
    /// The first `AttrData::Intrinsic` marker on an instance's item, if any.
    pub(crate) fn intrinsic_of(&self, instance: InstanceId) -> Option<&'m IntrinsicKind> {
        let item = self.model.instances[instance].item;
        if item.as_u32() as usize >= self.model.domain_attrs.item_attrs.len() {
            return None;
        }
        self.model.domain_attrs.item_attrs[item]
            .iter()
            .find_map(|attr| match attr {
                AttrData::Intrinsic(kind) => Some(kind),
                _ => None,
            })
    }
}

/// The driver facts the mmio lowering needs, supplied by the TU fold.
pub(crate) struct DriverFacts {
    /// The driver struct type (the lowered `self` pointee).
    pub driver_ty: TyId,
    /// Per resource field: `(C field name, A-10 resolved?, L1 join id)`.
    pub resources: Vec<(String, bool, String)>,
    /// `(macro name, byte offset)` per named register of the single block,
    /// for the const bank + offset rendering.
    pub reg_macros: Vec<(String, u64)>,
    /// The TU-static `mm_reg_t` base symbol every MMIO op reads (A-2: the
    /// faithful first realization, paired with the emitted single-instance
    /// `BUILD_ASSERT` guard).
    pub base_symbol: String,
    /// Whether this TU is the PCIe arm (bus = "pci"): config-space ops are
    /// licensed (C02/C03) and read the TU-static `__rseq_pcie` carrier
    /// (D12); on every other arm they keep the D.2 rejection.
    pub pcie: bool,
}

/// The `static <ret> <name>(<params>)` head shared by the forward
/// prototype and the definition (callees may be emitted after callers, so
/// every lowered function gets a prototype before the body bank).
pub(crate) fn fn_head(cx: &mut LowerCx<'_>, instance: InstanceId) -> Result<String, EmitError> {
    let inst = &cx.model.instances[instance];
    let InstanceKind::Function(data) = &inst.kind else {
        return Err(EmitError::Unsupported(format!(
            "non-function instance {:?} in the lowered closure",
            cx.model.strings.resolve(inst.emission_name)
        )));
    };
    let Some(body) = &data.body else {
        return Err(EmitError::Unsupported(format!(
            "foreign (bodyless) function `{}`",
            cx.model.strings.resolve(inst.emission_name)
        )));
    };
    let sig = cx.model.sigs.resolve(data.fn_sig);
    let c_name = cx.fn_symbols[&instance].clone();

    // Signature: ZST returns lower to void; ZST params are dropped.
    let ret = if types::is_zst(cx.model, sig.output) {
        "void".to_string()
    } else {
        types::c_type(cx.model, &mut cx.names, sig.output)?
    };
    let mut params = Vec::new();
    for local in body.arg_locals() {
        let decl = &body.local_decls[local];
        if types::is_zst(cx.model, decl.ty) {
            continue;
        }
        params.push(format!(
            "{} {}",
            types::c_type(cx.model, &mut cx.names, decl.ty)?,
            local_name(cx.model, body, local)
        ));
    }
    let params = if params.is_empty() {
        "void".to_string()
    } else {
        params.join(", ")
    };
    Ok(format!("static {ret} {c_name}({params})"))
}

/// A trivial projection accessor: a one-block body of exactly
/// `_0 = &((*_1).<fields...>); return;` — the `Mmio::<T>::deref` shape. Such
/// callees are inlined ANALYTICALLY at mmio-receiver recovery (the borrow
/// projection splices into the receiver chain) and their calls are elided;
/// any other use of the elided destination fails loud (never wrong code).
pub(crate) fn trivial_accessor_tail(
    model: &RegseqModel,
    instance: InstanceId,
) -> Option<Vec<PlaceElem>> {
    let InstanceKind::Function(data) = &model.instances[instance].kind else {
        return None;
    };
    let body = data.body.as_ref()?;
    if body.arg_count != 1 || body.basic_blocks.len() != 1 {
        return None;
    }
    let bb = &body.basic_blocks[regseq_model::BlockId::from_u32(0)];
    if !matches!(bb.terminator.kind, TerminatorKind::Return) {
        return None;
    }
    // In-order symbolic evaluation over borrow chains: each known local maps
    // to "a borrow of arg-1's pointee at <tail>". Reborrows (`&(*t)`,
    // `&(*t).f`) extend the tail; bare moves/copies propagate. The return
    // place must end up known.
    let arg = regseq_model::LocalId::from_u32(1);
    let mut known: BTreeMap<regseq_model::LocalId, Vec<PlaceElem>> = BTreeMap::new();
    for stmt in bb.statements.iter() {
        let StatementKind::Assign(dst, rhs) = &stmt.kind else {
            return None;
        };
        if !dst.projection.is_empty() {
            return None;
        }
        let tail = match rhs {
            Rvalue::Ref(_, rp) => {
                if !matches!(rp.projection.first(), Some(PlaceElem::Deref)) {
                    return None;
                }
                let rest: Vec<PlaceElem> = rp.projection[1..].to_vec();
                if rp.local == arg {
                    rest
                } else if let Some(base) = known.get(&rp.local) {
                    let mut t = base.clone();
                    t.extend(rest);
                    t
                } else {
                    return None;
                }
            }
            Rvalue::Use(Operand::Copy(q) | Operand::Move(q)) if q.projection.is_empty() => {
                known.get(&q.local)?.clone()
            }
            _ => return None,
        };
        known.insert(dst.local, tail);
    }
    known.get(&body.return_local()).cloned()
}

/// Emit one lowered function definition.
pub(crate) fn emit_fn(cx: &mut LowerCx<'_>, instance: InstanceId) -> Result<String, EmitError> {
    let head = fn_head(cx, instance)?;
    let inst = &cx.model.instances[instance];
    let InstanceKind::Function(data) = &inst.kind else {
        unreachable!("fn_head certified the function shape");
    };
    let body = data.body.as_ref().expect("fn_head certified a body");
    let sig = cx.model.sigs.resolve(data.fn_sig);

    // Destinations of trivial-accessor calls: their calls are elided and the
    // locals are never declared; any non-receiver use fails loud.
    cx.elided_locals.clear();
    for bb in body.basic_blocks.iter() {
        if let TerminatorKind::Call {
            callee: CallCallee::Direct(callee),
            destination,
            ..
        } = &bb.terminator.kind
        {
            if cx.intrinsic_of(*callee).is_none()
                && trivial_accessor_tail(cx.model, *callee).is_some()
                && destination.projection.is_empty()
                // Never elide into the return place: `return _0;` bypasses
                // the place_lvalue guard, so elision there could return an
                // uninitialized value. Left un-elided, the call lowers as an
                // ordinary call to an un-lowered instance — a loud error.
                && destination.local != body.return_local()
            {
                cx.elided_locals.insert(destination.local);
                // The call's argument temps exist only to feed the elided
                // accessor; elide them too (a use elsewhere fails loud).
                if let TerminatorKind::Call { args, .. } = &bb.terminator.kind {
                    for arg in args.iter() {
                        if let Operand::Copy(p) | Operand::Move(p) = arg {
                            if p.projection.is_empty() {
                                cx.elided_locals.insert(p.local);
                            }
                        }
                    }
                }
            }
        }
    }
    // Taint to fixpoint: borrow/copy temps deriving from an elided local are
    // elided too (they exist only symbolically, for the receiver chase). A
    // non-elided use of any of them still fails loud in `place_lvalue`.
    loop {
        let mut grew = false;
        for bb in body.basic_blocks.iter() {
            for stmt in bb.statements.iter() {
                let StatementKind::Assign(dst, rhs) = &stmt.kind else {
                    continue;
                };
                if !dst.projection.is_empty() || cx.elided_locals.contains(&dst.local) {
                    continue;
                }
                let root = match rhs {
                    Rvalue::Ref(_, p) | Rvalue::RawPtr(_, p) => Some(p.local),
                    Rvalue::Use(Operand::Copy(p) | Operand::Move(p)) => Some(p.local),
                    _ => None,
                };
                if let Some(root) = root {
                    if cx.elided_locals.contains(&root) {
                        cx.elided_locals.insert(dst.local);
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    let mut out = format!("{head}\n{{\n");

    // Locals (return place + temporaries); ZSTs are dropped.
    let ret_zst = types::is_zst(cx.model, sig.output);
    if !ret_zst {
        out.push_str(&format!(
            "\t{} _0;\n",
            types::c_type(
                cx.model,
                &mut cx.names,
                body.local_decls[body.return_local()].ty
            )?
        ));
    }
    for local in body.inner_locals() {
        let decl = &body.local_decls[local];
        if types::is_zst(cx.model, decl.ty) || cx.elided_locals.contains(&local) {
            continue;
        }
        out.push_str(&format!(
            "\t{} {};\n",
            types::c_type(cx.model, &mut cx.names, decl.ty)?,
            local_name(cx.model, body, local)
        ));
    }
    out.push('\n');

    // Blocks, in table order (RPO from the bridge), each labelled.
    for (idx, bb) in body.basic_blocks.iter().enumerate() {
        if idx != 0 {
            out.push_str(&format!("bb_{idx}:\n"));
        }
        for stmt in &bb.statements {
            emit_statement(cx, body, &stmt.kind, &mut out)?;
        }
        emit_terminator(cx, body, &bb.terminator.kind, ret_zst, &mut out)?;
    }

    out.push_str("}\n");
    Ok(out)
}

/// The C name of a body local: `_0` for the return place, the sanitized
/// debug name when present, else `_<n>`.
fn local_name(model: &RegseqModel, body: &regseq_model::Body, local: LocalId) -> String {
    if local == body.return_local() {
        return "_0".into();
    }
    match &body.local_decls[local].debug_name {
        Some(name) => super::naming::c_ident(model.strings.resolve(*name)),
        None => format!("_{}", local.as_u32()),
    }
}

fn emit_statement(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    kind: &StatementKind,
    out: &mut String,
) -> Result<(), EmitError> {
    match kind {
        StatementKind::Assign(place, rvalue)
            if place.projection.is_empty() && cx.elided_locals.contains(&place.local) =>
        {
            // The value exists only symbolically (receiver-chase input).
            let _ = rvalue;
            Ok(())
        }
        StatementKind::Assign(place, rvalue) => emit_assign(cx, body, place, rvalue, out),
        StatementKind::SetDiscriminant(place, variant) => {
            let ty = place_ty(cx.model, body, place)?;
            let lv = place_lvalue(cx, body, place)?;
            emit_enum_tag_write(cx, ty, &lv, *variant, out)
        }
        StatementKind::Intrinsic(_) => Ok(()), // non-diverging hints: no C
    }
}

fn emit_assign(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    place: &Place,
    rvalue: &Rvalue,
    out: &mut String,
) -> Result<(), EmitError> {
    let dest_ty = place_ty(cx.model, body, place)?;
    if types::is_zst(cx.model, dest_ty) {
        return Ok(()); // ZST writes lower to nothing
    }
    let lv = place_lvalue(cx, body, place)?;
    match rvalue {
        Rvalue::Use(op) => {
            let expr = operand_expr(cx, body, op)?;
            out.push_str(&format!("\t{lv} = {expr};\n"));
            Ok(())
        }
        Rvalue::BinaryOp(op, a, b) => {
            let a = operand_expr(cx, body, a)?;
            let b = operand_expr(cx, body, b)?;
            let c_op = bin_op_token(op)?;
            out.push_str(&format!("\t{lv} = {a} {c_op} {b};\n"));
            Ok(())
        }
        Rvalue::UnaryOp(op, a) => {
            let a = operand_expr(cx, body, a)?;
            let tok = match op {
                UnOp::Not => {
                    // Rust `!` on bool is logical not, on integers bitwise.
                    let bool_operand = matches!(&cx.model.types[dest_ty], Ty::Bool);
                    if bool_operand {
                        "!"
                    } else {
                        "~"
                    }
                }
                UnOp::Neg => "-",
                UnOp::PtrMetadata => {
                    return Err(EmitError::Unsupported(
                        "fat-pointer metadata extraction (first-light subset)".into(),
                    ))
                }
            };
            out.push_str(&format!("\t{lv} = {tok}{a};\n"));
            Ok(())
        }
        Rvalue::Cast(kind, op, to_ty) => {
            match kind {
                CastKind::IntToInt => {}
                other => {
                    return Err(EmitError::Unsupported(format!(
                        "cast kind {other:?} (first-light subset)"
                    )))
                }
            }
            let expr = operand_expr(cx, body, op)?;
            let to = types::c_type(cx.model, &mut cx.names, *to_ty)?;
            out.push_str(&format!("\t{lv} = ({to}){expr};\n"));
            Ok(())
        }
        Rvalue::Ref(_, src) | Rvalue::RawPtr(_, src) => {
            let src_lv = place_lvalue(cx, body, src)?;
            out.push_str(&format!("\t{lv} = &{src_lv};\n"));
            Ok(())
        }
        Rvalue::Aggregate(kind, operands) => emit_aggregate(cx, body, &lv, kind, operands, out),
        other => Err(EmitError::Unsupported(format!(
            "rvalue {other:?} (first-light subset)"
        ))),
    }
}

fn bin_op_token(op: &BinOp) -> Result<&'static str, EmitError> {
    Ok(match op {
        BinOp::Add | BinOp::AddUnchecked => "+",
        BinOp::Sub | BinOp::SubUnchecked => "-",
        BinOp::Mul | BinOp::MulUnchecked => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::BitXor => "^",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::Shl | BinOp::ShlUnchecked => "<<",
        BinOp::Shr | BinOp::ShrUnchecked => ">>",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Gt => ">",
        other => {
            return Err(EmitError::Unsupported(format!(
                "binary op {other:?} (first-light subset)"
            )))
        }
    })
}

/// Aggregate construction. Structs/tuples assign memberwise; enum variants
/// write the layout-resolved tag + payload bytes into the opaque carrier.
fn emit_aggregate(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    lv: &str,
    kind: &AggregateKind,
    operands: &[Operand],
    out: &mut String,
) -> Result<(), EmitError> {
    match kind {
        AggregateKind::Struct(ty) | AggregateKind::Tuple(ty) => {
            let layout = types::layout_of(cx.model, *ty)
                .ok_or_else(|| EmitError::Unsupported("aggregate without layout".into()))?;
            let LayoutKind::Struct { fields } = &layout.kind else {
                return Err(EmitError::Unsupported(
                    "struct aggregate with non-struct layout".into(),
                ));
            };
            for (field, op) in fields.iter().zip(operands.iter()) {
                if types::is_zst(cx.model, field.ty) {
                    continue;
                }
                let fname = super::naming::c_ident(cx.model.strings.resolve(field.name));
                let expr = operand_expr(cx, body, op)?;
                out.push_str(&format!("\t{lv}.{fname} = {expr};\n"));
            }
            Ok(())
        }
        AggregateKind::Enum(ty, variant) => {
            // Zero the carrier, write the variant tag (when the layout says
            // the variant carries one), then copy non-ZST payload bytes to
            // their field offsets.
            cx.extra_includes.insert("<string.h>");
            out.push_str(&format!("\tmemset(&{lv}, 0, sizeof({lv}));\n"));
            emit_enum_tag_write(cx, *ty, lv, *variant, out)?;
            let layout = types::layout_of(cx.model, *ty)
                .ok_or_else(|| EmitError::Unsupported("enum aggregate without layout".into()))?;
            let LayoutKind::Enum {
                shape: EnumShape::Multiple { variants, .. },
            } = &layout.kind
            else {
                return Err(EmitError::Unsupported(
                    "enum aggregate with unsupported shape (first-light subset)".into(),
                ));
            };
            let vlayout = &variants[variant.as_u32() as usize];
            for (field, op) in vlayout.fields.iter().zip(operands.iter()) {
                if types::is_zst(cx.model, field.ty) {
                    continue;
                }
                let expr = operand_expr(cx, body, op)?;
                let fty = types::c_type(cx.model, &mut cx.names, field.ty)?;
                out.push_str(&format!(
                    "\t{{ {fty} __v = {expr}; memcpy(&{lv}._raw[{}], &__v, sizeof(__v)); }}\n",
                    field.offset
                ));
            }
            Ok(())
        }
        other => Err(EmitError::Unsupported(format!(
            "aggregate kind {other:?} (first-light subset)"
        ))),
    }
}

/// Write an enum variant's tag into an opaque carrier lvalue, per the
/// resolved layout (`tag_value: None` = untagged variant: payload is the
/// tag, nothing to write).
fn emit_enum_tag_write(
    cx: &mut LowerCx<'_>,
    ty: TyId,
    lv: &str,
    variant: regseq_model::VariantIdx,
    out: &mut String,
) -> Result<(), EmitError> {
    let layout = types::layout_of(cx.model, ty)
        .ok_or_else(|| EmitError::Unsupported("tag write on a non-aggregate".into()))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(EmitError::Unsupported(
            "tag write on unsupported enum shape (first-light subset)".into(),
        ));
    };
    let vlayout = &variants[variant.as_u32() as usize];
    if let Some(tag_value) = vlayout.tag_value {
        let (offset, size) = types::enum_tag_cell(cx.model, ty)?;
        if tag_value > u64::MAX as u128 {
            return Err(EmitError::Unsupported(
                "128-bit enum tag value (first-light subset)".into(),
            ));
        }
        match size {
            1 => out.push_str(&format!(
                "\t{lv}._raw[{offset}] = {:#x};\n",
                tag_value as u64
            )),
            2 | 4 | 8 => {
                cx.extra_includes.insert("<string.h>");
                let cty = match size {
                    2 => "uint16_t",
                    4 => "uint32_t",
                    _ => "uint64_t",
                };
                out.push_str(&format!(
                    "\t{{ {cty} __tag = {:#x}; memcpy(&{lv}._raw[{offset}], &__tag, sizeof(__tag)); }}\n",
                    tag_value as u64
                ));
            }
            other => {
                return Err(EmitError::Unsupported(format!(
                    "enum tag width {other} (first-light subset)"
                )))
            }
        }
    }
    Ok(())
}

fn emit_terminator(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    kind: &TerminatorKind,
    ret_zst: bool,
    out: &mut String,
) -> Result<(), EmitError> {
    match kind {
        TerminatorKind::Goto(target) => {
            out.push_str(&format!("\tgoto bb_{};\n", target.as_u32()));
            Ok(())
        }
        TerminatorKind::Return => {
            if cx.elided_locals.contains(&body.return_local()) {
                return Err(EmitError::Unsupported(
                    "function returns an elided accessor result (first-light subset)".into(),
                ));
            }
            if ret_zst {
                out.push_str("\treturn;\n");
            } else {
                out.push_str("\treturn _0;\n");
            }
            Ok(())
        }
        TerminatorKind::Unreachable => {
            // Defensive trap: unreachable-by-construction control flow —
            // `k_panic()` is the Zephyr analogue of the oracle's panic
            // (unrecoverable, fatal-error path).
            cx.extra_includes.insert("<zephyr/kernel.h>");
            out.push_str("\tk_panic();\n");
            Ok(())
        }
        TerminatorKind::SwitchInt { discr, targets } => {
            let expr = operand_expr(cx, body, discr)?;
            for (value, target) in targets.cases.iter() {
                if *value > u64::MAX as u128 {
                    return Err(EmitError::Unsupported(
                        "128-bit switch case (first-light subset)".into(),
                    ));
                }
                out.push_str(&format!(
                    "\tif ({expr} == {:#x}) goto bb_{};\n",
                    *value as u64,
                    target.as_u32()
                ));
            }
            out.push_str(&format!("\tgoto bb_{};\n", targets.otherwise.as_u32()));
            Ok(())
        }
        TerminatorKind::Call {
            callee,
            args,
            destination,
            target,
        } => {
            emit_call(cx, body, callee, args, destination, out)?;
            match target {
                Some(t) => {
                    out.push_str(&format!("\tgoto bb_{};\n", t.as_u32()));
                    Ok(())
                }
                None => Err(EmitError::Unsupported(
                    "diverging call (first-light subset)".into(),
                )),
            }
        }
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            ..
        } => {
            // The MIR integrity guard (div-by-zero etc.): branch on success,
            // trap on violation — `k_panic()` is the faithful analogue of
            // the oracle's panic (unrecoverable).
            cx.extra_includes.insert("<zephyr/kernel.h>");
            let c = operand_expr(cx, body, cond)?;
            let bang = if *expected { "!" } else { "" };
            out.push_str(&format!("\tif ({bang}({c}))\n\t\tk_panic();\n"));
            out.push_str(&format!("\tgoto bb_{};\n", target.as_u32()));
            Ok(())
        }
    }
}

/// Direct call emission: intrinsic-anchored callees dispatch to their banks
/// (Mmio T06, Log T08); Config/PollUntil mirror the D12 frame scan's D.2
/// rejections defensively.
fn emit_call(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    callee: &CallCallee,
    args: &[Operand],
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    let CallCallee::Direct(instance) = callee else {
        return Err(EmitError::Unsupported(
            "indirect (function-pointer) call (first-light subset)".into(),
        ));
    };
    if trivial_accessor_tail(cx.model, *instance).is_some()
        && destination.projection.is_empty()
        && cx.elided_locals.contains(&destination.local)
    {
        // Elided: the accessor is inlined analytically at the receiver
        // chase; the call lowers to nothing.
        return Ok(());
    }
    if let Some(kind) = cx.intrinsic_of(*instance).cloned() {
        return match kind {
            IntrinsicKind::Mmio(op) => cx.emit_mmio(op, *instance, body, args, destination, out),
            IntrinsicKind::Log(level) => cx.emit_diag_log(&level, body, args, out),
            IntrinsicKind::PollUntil => Err(EmitError::Rejected(
                "`poll_until` is recognized-not-lowered (D.2/B-1, parity with generic_c's \
                 as-built disposition; convergence §9 forbids claiming it) — no WAIT_FOR \
                 lowering is invented"
                    .into(),
            )),
            IntrinsicKind::Config(op) => {
                cx.emit_config(op, *instance, body, args, destination, out)
            }
        };
    }
    let Some(symbol) = cx.fn_symbols.get(instance).cloned() else {
        return Err(EmitError::Unsupported(format!(
            "call to un-lowered instance `{}`",
            cx.model
                .strings
                .resolve(cx.model.instances[*instance].emission_name)
        )));
    };
    let param_tys: Vec<TyId> = match &cx.model.instances[*instance].kind {
        InstanceKind::Function(d) => cx.model.sigs.resolve(d.fn_sig).inputs.clone(),
        _ => Vec::new(),
    };
    let mut rendered = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        if operand_is_zst(cx.model, body, arg) {
            continue;
        }
        rendered.push(operand_expr_typed(
            cx,
            body,
            arg,
            param_tys.get(i).copied(),
        )?);
    }
    let call = format!("{symbol}({})", rendered.join(", "));
    let dest_ty = place_ty(cx.model, body, destination)?;
    if types::is_zst(cx.model, dest_ty) {
        out.push_str(&format!("\t{call};\n"));
    } else {
        let lv = place_lvalue(cx, body, destination)?;
        out.push_str(&format!("\t{lv} = {call};\n"));
    }
    Ok(())
}

fn operand_is_zst(model: &RegseqModel, body: &regseq_model::Body, op: &Operand) -> bool {
    match op {
        Operand::Copy(p) | Operand::Move(p) => place_ty(model, body, p)
            .map(|t| types::is_zst(model, t))
            .unwrap_or(false),
        Operand::Constant(id) => matches!(&model.consts[*id], Const::ZST),
    }
}

/// Render an operand as a C expression. `consumer` types a constant operand
/// (a call's parameter type) so non-scalar constants can render.
fn operand_expr_typed(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    op: &Operand,
    consumer: Option<TyId>,
) -> Result<String, EmitError> {
    match op {
        Operand::Copy(place) | Operand::Move(place) => place_lvalue(cx, body, place),
        Operand::Constant(id) => const_expr(cx, *id, consumer),
    }
}

/// Render an operand with no consumer-type context.
fn operand_expr(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    op: &Operand,
) -> Result<String, EmitError> {
    operand_expr_typed(cx, body, op, None)
}

/// Render a constant as a C expression.
fn const_expr(
    cx: &mut LowerCx<'_>,
    id: regseq_model::ConstId,
    consumer: Option<TyId>,
) -> Result<String, EmitError> {
    // A struct-typed byte image renders as a compound literal against the
    // consumer type's layout (the const-propagated FieldValue shape).
    if let (Const::Bytes(bytes), Some(cty)) = (&cx.model.consts[id], consumer) {
        if let Some(layout) = types::layout_of(cx.model, cty) {
            if let LayoutKind::Struct { fields } = &layout.kind {
                let cname = types::c_type(cx.model, &mut cx.names, cty)?;
                let mut members = Vec::new();
                for f in fields {
                    if types::is_zst(cx.model, f.ty) {
                        continue;
                    }
                    let size = types::layout_of(cx.model, f.ty)
                        .map(|l| l.size)
                        .unwrap_or_else(|| scalar_size(cx.model, f.ty));
                    let mut v: u64 = 0;
                    for i in 0..size {
                        match bytes.get((f.offset + i) as usize) {
                            Some(regseq_model::Byte::Value(b)) => v |= (*b as u64) << (8 * i),
                            _ => {
                                return Err(EmitError::Unsupported(
                                    "byte-image constant with non-value bytes".into(),
                                ))
                            }
                        }
                    }
                    members.push(format!(
                        ".{} = {v:#x}",
                        super::naming::c_ident(cx.model.strings.resolve(f.name))
                    ));
                }
                return Ok(format!("({cname}){{ {} }}", members.join(", ")));
            }
        }
    }
    match &cx.model.consts[id] {
        Const::Bool(b) => Ok(if *b { "true" } else { "false" }.into()),
        Const::Uint(kind, bits) => {
            if *bits > u64::MAX as u128 {
                return Err(EmitError::Unsupported(
                    "128-bit constant (first-light subset)".into(),
                ));
            }
            let suffix = match kind {
                regseq_model::UintKind::U64 | regseq_model::UintKind::Usize => "ull",
                _ => "u",
            };
            Ok(format!("{:#x}{suffix}", *bits as u64))
        }
        Const::Int(_, bits) => {
            // Sign-extended storage; render through the signed reinterpretation.
            let v = *bits as i128;
            if v > i64::MAX as i128 || v < i64::MIN as i128 {
                return Err(EmitError::Unsupported(
                    "128-bit constant (first-light subset)".into(),
                ));
            }
            Ok(format!("{}", v as i64))
        }
        other => Err(EmitError::Unsupported(format!(
            "constant {other:?} (first-light subset)"
        ))),
    }
}

/// Render a place as a C lvalue, walking the projection chain.
fn place_lvalue(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    place: &Place,
) -> Result<String, EmitError> {
    if cx.elided_locals.contains(&place.local) {
        return Err(EmitError::Unsupported(
            "use of an elided accessor result outside an MMIO receiver position \
             (first-light subset)"
                .into(),
        ));
    }
    let mut expr = local_name(cx.model, body, place.local);
    let mut ty = body.local_decls[place.local].ty;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                expr = format!("(*{expr})");
                ty = match &cx.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    other => {
                        return Err(EmitError::Unsupported(format!(
                            "deref of non-pointer {other:?}"
                        )))
                    }
                };
            }
            PlaceElem::Field(idx, field_ty) => {
                let fname = field_c_name(cx.model, ty, idx.as_u32() as usize)?;
                expr = format!("{expr}.{fname}");
                ty = *field_ty;
            }
            other => {
                return Err(EmitError::Unsupported(format!(
                    "place projection {other:?} (first-light subset)"
                )))
            }
        }
    }
    Ok(expr)
}

/// The C member name for field `idx` of an aggregate type.
fn field_c_name(model: &RegseqModel, ty: TyId, idx: usize) -> Result<String, EmitError> {
    let layout = types::layout_of(model, ty)
        .ok_or_else(|| EmitError::Unsupported("field of a non-aggregate".into()))?;
    let LayoutKind::Struct { fields } = &layout.kind else {
        return Err(EmitError::Unsupported(
            "field projection into a non-struct layout (first-light subset)".into(),
        ));
    };
    let field = fields
        .get(idx)
        .ok_or_else(|| EmitError::Unsupported(format!("field index {idx} out of bounds")))?;
    Ok(super::naming::c_ident(model.strings.resolve(field.name)))
}

/// The type a place resolves to after its projection chain.
pub(crate) fn place_ty(
    model: &RegseqModel,
    body: &regseq_model::Body,
    place: &Place,
) -> Result<TyId, EmitError> {
    let mut ty = body.local_decls[place.local].ty;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    other => {
                        return Err(EmitError::Unsupported(format!(
                            "deref of non-pointer {other:?}"
                        )))
                    }
                };
            }
            PlaceElem::Field(_, field_ty) => ty = *field_ty,
            other => {
                return Err(EmitError::Unsupported(format!(
                    "place projection {other:?} (first-light subset)"
                )))
            }
        }
    }
    Ok(ty)
}

// ---------------------------------------------------------------------------
// Register-access lowering (T06): MmioOp interception per the traceability
// matrix section A — width-switched sys_read8/16/32/64 + sys_write8/16/32/64
// (VALUE-FIRST write args, the sys_io convention) over the TU-static
// `mm_reg_t` base (A-2), mask/shift RMW preserved; the ratified A-10
// resolved-binding gate; width-8 is aarch64-validated (D19) with the C-2
// arch-conditionality note — a target arch lacking `sys_*64` fails the C
// compile loud (the caller injects the note when `width8_used` is set).
// ---------------------------------------------------------------------------

impl LowerCx<'_> {
    /// Lower one `IntrinsicKind::Mmio(op)` call.
    pub(crate) fn emit_mmio(
        &mut self,
        op: regseq_model::MmioOp,
        callee: InstanceId,
        body: &regseq_model::Body,
        args: &[Operand],
        destination: &Place,
        out: &mut String,
    ) -> Result<(), EmitError> {
        let desc_ty = match &self.model.instances[callee].kind {
            InstanceKind::Function(d) => self.model.sigs.resolve(d.fn_sig).inputs.get(1).copied(),
            _ => None,
        };
        use regseq_model::MmioOp;
        let recv = args
            .first()
            .ok_or_else(|| EmitError::Unsupported("mmio call with no receiver".into()))?;
        let (base, offset, runtime, width) = self.recover_register(body, recv)?;
        self.extra_includes.insert("<zephyr/sys/sys_io.h>");
        let (read_fn, write_fn, cast) = match width {
            1 => ("sys_read8", "sys_write8", "uint8_t"),
            2 => ("sys_read16", "sys_write16", "uint16_t"),
            4 => ("sys_read32", "sys_write32", "uint32_t"),
            8 => {
                // D19: width-8 is aarch64-validated; the C-2 note rides the
                // TU and a target arch lacking sys_*64 fails the C compile.
                self.width8_used = true;
                ("sys_read64", "sys_write64", "uint64_t")
            }
            other => {
                return Err(EmitError::Unsupported(format!(
                    "MMIO access width {other} (no sys_io accessor)"
                )))
            }
        };
        let mut addr = if offset == 0 {
            base.clone()
        } else {
            format!("{base} + {}", self.offset_token(offset))
        };
        // Runtime register-array index terms (`base + i*stride` — the
        // certified reaching-def mechanism shared with the sibling
        // backend's recovery, F-1).
        for (idx, stride) in &runtime {
            addr = format!("{addr} + ({idx}) * {stride}");
        }
        // Field/FieldValue descriptors: a runtime place renders as member
        // reads; a const-propagated aggregate const-folds to literals.
        let stmt = match op {
            MmioOp::Write => {
                let val = operand_expr(self, body, &args[1])?;
                // sys_io convention: VALUE FIRST, address second.
                format!("\t{write_fn}(({cast})({val}), {addr});\n")
            }
            MmioOp::WriteField => {
                let fv = self.descriptor(body, &args[1], desc_ty)?;
                let (mask, value) = (fv.member("mask")?, fv.member("value")?);
                format!("\t{write_fn}(({cast})(({value}) & ({mask})), {addr});\n")
            }
            MmioOp::Read => {
                let dst = place_lvalue(self, body, destination)?;
                let dst_cty = {
                    let ty = place_ty(self.model, body, destination)?;
                    types::c_type(self.model, &mut self.names, ty)?
                };
                format!("\t{dst} = ({dst_cty}){read_fn}({addr});\n")
            }
            MmioOp::ReadField => {
                let fld = self.descriptor(body, &args[1], desc_ty)?;
                let (mask, shift) = (fld.member("mask")?, fld.member("shift")?);
                let dst = place_lvalue(self, body, destination)?;
                let dst_cty = {
                    let ty = place_ty(self.model, body, destination)?;
                    types::c_type(self.model, &mut self.names, ty)?
                };
                format!("\t{dst} = ({dst_cty})(({read_fn}({addr}) & ({mask})) >> ({shift}));\n")
            }
            MmioOp::Modify => {
                let fv = self.descriptor(body, &args[1], desc_ty)?;
                let (mask, value) = (fv.member("mask")?, fv.member("value")?);
                format!(
                    "\t{{\n\t\t{cast} __cur = {read_fn}({addr});\n\t\t{write_fn}((__cur & ~({cast})({mask})) | (({cast})(({value}) & ({mask}))), {addr});\n\t}}\n"
                )
            }
        };
        out.push_str(&stmt);
        Ok(())
    }

    /// Render a recovered register offset: the const-bank macro when one
    /// names it exactly, else a hex literal.
    fn offset_token(&self, offset: u64) -> String {
        if let Some((name, _)) = self
            .driver
            .reg_macros
            .iter()
            .find(|(_, off)| *off == offset)
        {
            name.clone()
        } else {
            format!("{offset:#x}")
        }
    }

    /// Resolve an mmio receiver to `(base expr, byte offset, runtime
    /// array-index terms, access width)`.
    ///
    /// The receiver borrows the register wrapper through the chain
    /// `self.<resource>.inner.<reg>` (`Mmio<T>: Deref` inlined by rustc).
    /// The chain VALIDATES through the driver struct's resource field (the
    /// A-10 gate fires here), but the rendered base is the TU-static
    /// `mm_reg_t` symbol (A-2) — the spec-view field is dead weight in the
    /// zero image; the kernel's mapped base flows through
    /// `DEVICE_MMIO_MAP`/`DEVICE_MMIO_GET` at init (platform arm) or the
    /// C01 `device_map` (PCIe arm).
    #[allow(clippy::type_complexity)]
    fn recover_register(
        &mut self,
        body: &regseq_model::Body,
        recv: &Operand,
    ) -> Result<(String, u64, Vec<(String, u64)>, u64), EmitError> {
        let mut src = self.resolve_receiver_src(body, recv, 0)?;
        // Normalize through intermediate borrow temps: while the root local
        // is not the driver borrow, splice in its unique `Ref` definition
        // (`(*(&P)).x` == `P.x`). Bounded so a cyclic/corrupt body fails
        // loud instead of spinning.
        for _ in 0..8 {
            let root_ty = body.local_decls[src.local].ty;
            let is_driver_root = matches!(
                &self.model.types[root_ty],
                Ty::Ref(p, _) | Ty::Ptr(p, _) if *p == self.driver.driver_ty
            );
            if is_driver_root {
                break;
            }
            if !matches!(src.projection.first(), Some(PlaceElem::Deref)) {
                break;
            }
            // The unique definition of the root local.
            let mut def: Option<&Rvalue> = None;
            for bb in body.basic_blocks.iter() {
                for stmt in bb.statements.iter() {
                    if let StatementKind::Assign(dst, rhs) = &stmt.kind {
                        if dst.local == src.local && dst.projection.is_empty() {
                            if def.is_some() {
                                def = None;
                                break;
                            }
                            def = Some(rhs);
                        }
                    }
                }
            }
            match def {
                Some(Rvalue::Ref(_, inner)) => {
                    let mut projection: Vec<PlaceElem> = inner.projection.to_vec();
                    projection.extend(src.projection.iter().skip(1).cloned());
                    src = Place {
                        local: inner.local,
                        projection: projection.into_boxed_slice(),
                    };
                }
                Some(Rvalue::Use(Operand::Copy(q) | Operand::Move(q)))
                    if q.projection.is_empty() =>
                {
                    src = Place {
                        local: q.local,
                        projection: src.projection.clone(),
                    };
                }
                _ => {
                    // Not Assign-defined: an elided trivial-accessor call
                    // destination — splice `arg0's pointee ++ accessor tail`.
                    let mut spliced = false;
                    for bb in body.basic_blocks.iter() {
                        let TerminatorKind::Call {
                            callee: CallCallee::Direct(callee),
                            args: cargs,
                            destination,
                            ..
                        } = &bb.terminator.kind
                        else {
                            continue;
                        };
                        if destination.local != src.local || !destination.projection.is_empty() {
                            continue;
                        }
                        let Some(tail) = trivial_accessor_tail(self.model, *callee) else {
                            continue;
                        };
                        let arg_place = self.resolve_receiver_src(body, &cargs[0], 0)?;
                        // `*(&P).tail.rest` == `P.tail.rest`.
                        let mut projection: Vec<PlaceElem> = arg_place.projection.to_vec();
                        projection.extend(tail);
                        projection.extend(src.projection.iter().skip(1).cloned());
                        src = Place {
                            local: arg_place.local,
                            projection: projection.into_boxed_slice(),
                        };
                        spliced = true;
                        break;
                    }
                    if !spliced {
                        break;
                    }
                }
            }
        }
        let root_ty = body.local_decls[src.local].ty;
        let pointee = match &self.model.types[root_ty] {
            Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
            _ => {
                return Err(EmitError::Unsupported(
                    "mmio receiver root is not a borrow of the driver struct".into(),
                ))
            }
        };
        if pointee != self.driver.driver_ty {
            return Err(EmitError::Unsupported(format!(
                "mmio receiver does not project from the driver struct's resource field \
                 (first-light subset); root local _{} type {:?}, projection {:?}",
                src.local.as_u32(),
                self.model.types[root_ty],
                src.projection,
            )));
        }
        let mut elems = src.projection.iter();
        match elems.next() {
            Some(PlaceElem::Deref) => {}
            _ => {
                return Err(EmitError::Unsupported(
                    "mmio receiver must start by dereferencing the driver borrow".into(),
                ))
            }
        }
        // The resource field: validate + A-10 gate, then substitute the
        // TU-static base (A-2).
        let mut ty = match elems.next() {
            Some(PlaceElem::Field(idx, field_ty)) => {
                let fname = field_c_name(self.model, pointee, idx.as_u32() as usize)?;
                let Some((_, resolved, join_id)) = self
                    .driver
                    .resources
                    .iter()
                    .find(|(name, _, _)| *name == fname)
                else {
                    return Err(EmitError::Unsupported(format!(
                        "mmio receiver projects through non-resource field `{fname}`"
                    )));
                };
                if !resolved {
                    return Err(EmitError::UnresolvedBinding(format!(
                        "MMIO op against resource `{fname}` whose binding is not resolved: \
                         the platform file must supply `[mmio.{join_id}] base/size` \
                         (the L2 literals are the resolution trigger, ratified A-10/D11)"
                    )));
                }
                *field_ty
            }
            _ => {
                return Err(EmitError::Unsupported(
                    "mmio receiver does not name a driver resource field".into(),
                ))
            }
        };
        let base = self.driver.base_symbol.clone();
        // The block-relative offset (Mmio.inner + register fields), plus the
        // runtime index terms of register-array accesses (the donor
        // `mta[m]`/`ra[r].ral` patterns): each `Index` contributes
        // `(<index local>) * <element stride>` — the offset is recovered as
        // `base + i*stride`, not a layout constant (the certified
        // reaching-def mechanism, F-1).
        let mut offset: u64 = 0;
        let mut runtime: Vec<(String, u64)> = Vec::new();
        for elem in elems {
            match elem {
                PlaceElem::Field(idx, field_ty) => {
                    let layout = types::layout_of(self.model, ty).ok_or_else(|| {
                        EmitError::Unsupported("mmio block projection without layout".into())
                    })?;
                    let LayoutKind::Struct { fields } = &layout.kind else {
                        return Err(EmitError::Unsupported(
                            "mmio block projection into a non-struct layout".into(),
                        ));
                    };
                    let f = fields.get(idx.as_u32() as usize).ok_or_else(|| {
                        EmitError::Unsupported("mmio register field index out of range".into())
                    })?;
                    if f.ty != *field_ty {
                        return Err(EmitError::Unsupported(
                            "mmio receiver field type disagrees with the block layout".into(),
                        ));
                    }
                    offset += f.offset;
                    ty = f.ty;
                }
                PlaceElem::Index(idx_local) => {
                    let Ty::Array(elem_ty, _) = &self.model.types[ty] else {
                        return Err(EmitError::Unsupported(
                            "mmio index projection into a non-array register field".into(),
                        ));
                    };
                    let elem_ty = *elem_ty;
                    let stride = self.model.size_align_of(elem_ty).size;
                    runtime.push((local_name(self.model, body, *idx_local), stride));
                    ty = elem_ty;
                }
                other => {
                    return Err(EmitError::Unsupported(format!(
                        "mmio receiver projection {other:?} (first-light subset)"
                    )))
                }
            }
        }
        let width = types::layout_of(self.model, ty)
            .map(|l| l.size)
            .ok_or_else(|| EmitError::Unsupported("mmio register wrapper has no layout".into()))?;
        Ok((base, offset, runtime, width))
    }

    /// Chase a bare-local receiver to the place it uniquely borrows
    /// (the hoisted `&self.regs.inner.<reg>` borrow).
    fn resolve_receiver_src(
        &self,
        body: &regseq_model::Body,
        op: &Operand,
        depth: u32,
    ) -> Result<Place, EmitError> {
        if depth > 8 {
            return Err(EmitError::Unsupported(
                "mmio receiver reaching-def chase exceeded depth bound".into(),
            ));
        }
        let place = match op {
            Operand::Copy(p) | Operand::Move(p) => p,
            Operand::Constant(_) => {
                return Err(EmitError::Unsupported("mmio receiver is a constant".into()))
            }
        };
        if matches!(place.projection.last(), Some(PlaceElem::Field(..))) {
            return Ok(place.clone());
        }
        if !place.projection.is_empty() {
            return Err(EmitError::Unsupported(
                "mmio receiver has an unexpected trailing projection".into(),
            ));
        }
        let local = place.local;
        let mut def: Option<&Rvalue> = None;
        for bb in body.basic_blocks.iter() {
            for stmt in bb.statements.iter() {
                let StatementKind::Assign(dst, rhs) = &stmt.kind else {
                    continue;
                };
                if dst.local != local || !dst.projection.is_empty() {
                    continue;
                }
                if def.is_some() {
                    return Err(EmitError::Unsupported(
                        "mmio receiver local has multiple definitions (ambiguous)".into(),
                    ));
                }
                def = Some(rhs);
            }
        }
        match def {
            Some(Rvalue::Ref(_, src) | Rvalue::RawPtr(_, src)) => Ok(src.clone()),
            Some(Rvalue::Use(inner)) => self.resolve_receiver_src(body, inner, depth + 1),
            Some(_) => Err(EmitError::Unsupported(
                "mmio receiver temp has an unexpected rvalue".into(),
            )),
            None => Err(EmitError::Unsupported(
                "no definition for mmio receiver local in body".into(),
            )),
        }
    }

    /// A `Field`/`FieldValue` descriptor argument: either a runtime place
    /// (members render as C member reads) or a const-propagated aggregate
    /// (members const-fold to hex literals).
    fn descriptor(
        &mut self,
        body: &regseq_model::Body,
        op: &Operand,
        desc_ty: Option<TyId>,
    ) -> Result<Descriptor, EmitError> {
        match op {
            Operand::Copy(place) | Operand::Move(place) => {
                let ty = place_ty(self.model, body, place)?;
                let expr = place_lvalue(self, body, place)?;
                Ok(Descriptor::Place {
                    expr,
                    fields: self.descriptor_field_names(ty)?,
                })
            }
            Operand::Constant(id) => match &self.model.consts[*id] {
                Const::Aggregate(children) => {
                    let mut values = Vec::new();
                    for child in children {
                        match &self.model.consts[*child] {
                            Const::Uint(_, bits) => values.push(*bits as u64),
                            Const::ZST => {}
                            other => {
                                return Err(EmitError::Unsupported(format!(
                                    "descriptor const child {other:?}"
                                )))
                            }
                        }
                    }
                    Ok(Descriptor::Const { values })
                }
                // A const-propagated descriptor arrives as its LE byte
                // image; decode the scalar fields through the descriptor
                // type's layout.
                Const::Bytes(bytes) => {
                    let ty = desc_ty.ok_or_else(|| {
                        EmitError::Unsupported(
                            "byte-image descriptor with no recoverable type".into(),
                        )
                    })?;
                    let layout = types::layout_of(self.model, ty).ok_or_else(|| {
                        EmitError::Unsupported("byte-image descriptor without layout".into())
                    })?;
                    let LayoutKind::Struct { fields } = &layout.kind else {
                        return Err(EmitError::Unsupported(
                            "byte-image descriptor with non-struct layout".into(),
                        ));
                    };
                    let mut values = Vec::new();
                    for f in fields {
                        if types::is_zst(self.model, f.ty) {
                            continue;
                        }
                        let size = types::layout_of(self.model, f.ty)
                            .map(|l| l.size)
                            .unwrap_or_else(|| scalar_size(self.model, f.ty));
                        let mut v: u64 = 0;
                        for i in 0..size {
                            let idx = (f.offset + i) as usize;
                            let byte = match bytes.get(idx) {
                                Some(regseq_model::Byte::Value(b)) => *b,
                                _ => {
                                    return Err(EmitError::Unsupported(
                                        "byte-image descriptor with non-value bytes".into(),
                                    ))
                                }
                            };
                            v |= (byte as u64) << (8 * i);
                        }
                        values.push(v);
                    }
                    Ok(Descriptor::Const { values })
                }
                other => Err(EmitError::Unsupported(format!(
                    "mmio descriptor constant {other:?}"
                ))),
            },
        }
    }

    /// The member-name order of a descriptor struct (mask first), from its
    /// layout — `["mask", "shift"]` / `["mask", "value"]`.
    fn descriptor_field_names(&self, ty: TyId) -> Result<Vec<String>, EmitError> {
        let layout = types::layout_of(self.model, ty)
            .ok_or_else(|| EmitError::Unsupported("descriptor without layout".into()))?;
        let LayoutKind::Struct { fields } = &layout.kind else {
            return Err(EmitError::Unsupported(
                "descriptor with non-struct layout".into(),
            ));
        };
        Ok(fields
            .iter()
            .filter(|f| !types::is_zst(self.model, f.ty))
            .map(|f| super::naming::c_ident(self.model.strings.resolve(f.name)))
            .collect())
    }

    // -----------------------------------------------------------------------
    // Config-space lowering (backend-zephyr-c-e1000 T02, contracts C02/C03):
    // word-index marshalling over the 32-bit `pcie_conf_read/write` pins
    // (pcie.h:176/:187 — reg is "the configuration word index (not
    // address)"), reads served from the D06 word-0 ID cache or one live
    // word read with shift/mask extraction, width-4 writes native, width-1/2
    // writes via the audited D05 containing-word RMW. Legality (bus, width,
    // constant offset, alignment) is [`super::config_shape`] — the SAME
    // classifier the D12 frame scan runs first, so the op site can only see
    // licensed shapes.
    // -----------------------------------------------------------------------

    /// Lower one `IntrinsicKind::Config(op)` call per C02/C03 against the
    /// TU-static `__rseq_pcie` carrier (D12).
    pub(crate) fn emit_config(
        &mut self,
        op: regseq_model::ConfigOp,
        callee: InstanceId,
        body: &regseq_model::Body,
        args: &[Operand],
        destination: &Place,
        out: &mut String,
    ) -> Result<(), EmitError> {
        use regseq_model::ConfigOp;
        let shape = super::config_shape(self.model, callee, &op, args, self.driver.pcie)?;
        let word = shape.off >> 2;
        let sh = (shape.off & 0x3) * 8;
        match op {
            ConfigOp::Read => {
                let mask = match shape.width {
                    1 => "0xffu",
                    2 => "0xffffu",
                    _ => "0xffffffffu",
                };
                let dst = place_lvalue(self, body, destination)?;
                let dst_ty = place_ty(self.model, body, destination)?;
                let dst_cty = types::c_type(self.model, &mut self.names, dst_ty)?;
                let source = if word == 0 {
                    // The D06 cache: any read whose containing word is word
                    // 0 (vendor@0x00 AND device@0x02) is served from the
                    // pcie_dev ID snapshot — zero bus traffic.
                    "\t\t/* C03 word-0 cache (A-4/D06): VID|DID served from __rseq_pcie->id;\n\t\t * zero bus traffic */\n\t\tuint32_t __cw = __rseq_pcie->id;\n"
                        .to_string()
                } else {
                    format!(
                        "\t\tuint32_t __cw = pcie_conf_read(__rseq_pcie->bdf, {word}u); /* word index (\"not address\", pcie.h:176) */\n"
                    )
                };
                out.push_str(&format!(
                    "\t{{\n{source}\t\t{dst} = ({dst_cty})((__cw >> {sh}u) & {mask});\n\t}}\n"
                ));
            }
            ConfigOp::Write => {
                let val_arg = args.get(1).ok_or_else(|| {
                    EmitError::Unsupported("write_config with no value argument".into())
                })?;
                let val = operand_expr(self, body, val_arg)?;
                if shape.width == 4 {
                    out.push_str(&format!(
                        "\t{{\n\t\tpcie_conf_write(__rseq_pcie->bdf, {word}u, (uint32_t)({val})); /* word index (\"not address\", pcie.h:187) */\n\t}}\n"
                    ));
                } else {
                    // The D05 containing-word RMW (the ratified B-3 partial
                    // lift), C02 shape verbatim; its internal read is always
                    // the LIVE word, never the D06 cache.
                    let off_expr = operand_expr(
                        self,
                        body,
                        args.first().expect("config_shape validated the offset"),
                    )?;
                    let lane_mask = if shape.width == 1 { "0xFFu" } else { "0xFFFFu" };
                    out.push_str(&format!(
                        "\t{{\n\t\t/* RSEQ-RMW: width-{w} config write lowered to the containing-word RMW\n\t\t * (the ratified B-3 partial lift): one extra word read; adjacent bytes\n\t\t * rewritten with read-back values — benign per PCI config semantics\n\t\t * (QEMU e1000_write_config reacts only to PCI_COMMAND,\n\t\t * hw/net/e1000.c:1624). [license: backend-zephyr-c-e1000 GRILL D05] */\n\t\tunsigned int __w = ({off}) >> 2; /* pcie word index (\"not address\", pcie.h:187) */\n\t\tuint32_t __cur = pcie_conf_read(__rseq_pcie->bdf, __w); /* always the LIVE word, never the word-0 cache (C03) */\n\t\tuint32_t __sh = (({off}) & 0x3u) * 8u; /* byte lane */\n\t\tuint32_t __msk = {lane_mask} << __sh;\n\t\tpcie_conf_write(__rseq_pcie->bdf, __w, (__cur & ~__msk) | (((uint32_t)({val}) << __sh) & __msk));\n\t}}\n",
                        w = shape.width,
                        off = off_expr,
                        val = val
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A lowered descriptor argument.
pub(crate) enum Descriptor {
    /// Runtime struct value: members read as `(expr).member`.
    Place { expr: String, fields: Vec<String> },
    /// Const-folded: `[mask, shift-or-value]` literal values in declaration
    /// order.
    Const { values: Vec<u64> },
}

impl Descriptor {
    /// The C expression for a named member (`mask`/`shift`/`value`).
    fn member(&self, name: &str) -> Result<String, EmitError> {
        match self {
            Descriptor::Place { expr, fields } => {
                if fields.iter().any(|f| f == name) {
                    Ok(format!("({expr}).{name}"))
                } else {
                    Err(EmitError::Unsupported(format!(
                        "descriptor has no `{name}` member (fields: {fields:?})"
                    )))
                }
            }
            Descriptor::Const { values } => {
                // Declaration order: mask first, then shift (Field) or value
                // (FieldValue).
                let idx = match name {
                    "mask" => 0,
                    "shift" | "value" => 1,
                    _ => {
                        return Err(EmitError::Unsupported(format!(
                            "descriptor member `{name}`"
                        )))
                    }
                };
                values.get(idx).map(|v| format!("{v:#x}")).ok_or_else(|| {
                    EmitError::Unsupported(format!(
                        "descriptor const has no member {idx} (`{name}`)"
                    ))
                })
            }
        }
    }
}

/// The byte size of a scalar type (descriptor field decoding).
fn scalar_size(model: &RegseqModel, ty: TyId) -> u64 {
    match &model.types[ty] {
        Ty::Bool => 1,
        Ty::Uint(regseq_model::UintKind::U8) | Ty::Int(regseq_model::IntKind::I8) => 1,
        Ty::Uint(regseq_model::UintKind::U16) | Ty::Int(regseq_model::IntKind::I16) => 2,
        Ty::Uint(regseq_model::UintKind::U32) | Ty::Int(regseq_model::IntKind::I32) => 4,
        _ => 8,
    }
}

// ---------------------------------------------------------------------------
// Diagnostics-logging lowering (T08): each Log(level) call reassembles into
// ONE LOG_* macro invocation (the inverse of the oracle's fwrite
// decomposition): level -> LOG_DBG/LOG_INF/LOG_WRN/LOG_ERR (the `[LEVEL]`
// text tag is DROPPED — severity is structural via the macro), the template
// re-becomes a single cbprintf format literal (literal `%` doubled to `%%`,
// NO trailing newline — the Zephyr log core owns line termination), numeric
// placeholders reuse the certified (specifier, cast) mapping with stdint
// spelling. The bridge pre-flattens LogArg (peel): args = [template, v1,
// v2, ...].
//
// Argument-kind dispositions (GRILL D12/D18 — deliver-tested or reject
// fail-loud, never silent):
// - numeric/bool: delivered (the certified mapping).
// - static NUL-free `&str`: delivered as `%s` + the decoded literal (a
//   static literal is deferred-mode safe; byte-faithful because NUL-free).
// - static `&str` WITH interior NUL: rejected naming A-6 (the ratified
//   recommendation IS fail-loud; `%s` would truncate byte-faithfulness).
// - non-static `&str`: rejected naming A-5 (the bounce-buffer delivery is
//   designed but unexercised by any fixture; rejection names the gap).
// ---------------------------------------------------------------------------

/// A parsed template placeholder modifier (the bridge's accepted set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    None,
    Hex {
        upper: bool,
        alternate: bool,
        width: u32,
    },
}

/// One parsed template segment.
enum Segment {
    Literal(Vec<u8>),
    Placeholder(Modifier),
}

impl LowerCx<'_> {
    /// Lower one `IntrinsicKind::Log(level)` call to a single `LOG_*` call.
    pub(crate) fn emit_diag_log(
        &mut self,
        level: &regseq_model::LogLevel,
        body: &regseq_model::Body,
        args: &[Operand],
        out: &mut String,
    ) -> Result<(), EmitError> {
        use regseq_model::LogLevel;
        let macro_name = match level {
            LogLevel::Debug => "LOG_DBG",
            LogLevel::Info => "LOG_INF",
            LogLevel::Warn => "LOG_WRN",
            LogLevel::Error => "LOG_ERR",
        };
        let fmt_op = args
            .first()
            .ok_or_else(|| EmitError::Unsupported("diag-log call with no format operand".into()))?;
        let template = self.read_str_carrier_bytes(fmt_op).ok_or_else(|| {
            EmitError::Unsupported("diag-log format operand is not a string literal".into())
        })?;
        let segments = parse_template(&template)?;
        let value_ops = &args[1..];
        let placeholders = segments
            .iter()
            .filter(|s| matches!(s, Segment::Placeholder(_)))
            .count();
        if placeholders != value_ops.len() {
            return Err(EmitError::Unsupported(
                "diag-log placeholder/argument arity mismatch".into(),
            ));
        }

        let mut fmt = String::new();
        let mut call_args: Vec<String> = Vec::new();
        let mut value_idx = 0usize;
        for seg in &segments {
            match seg {
                Segment::Literal(bytes) => {
                    // The reassembled literal IS a cbprintf format string:
                    // double every literal `%`.
                    fmt.push_str(&c_escape_fmt(bytes));
                }
                Segment::Placeholder(modifier) => {
                    let op = &value_ops[value_idx];
                    value_idx += 1;
                    // Static `&str` value: NUL-free literals are delivered
                    // via `%s` (static lifetime — deferred-mode safe);
                    // interior NUL is the A-6 fail-loud rejection.
                    if let Some(bytes) = self.read_str_carrier_bytes(op) {
                        require_default_modifier(*modifier)?;
                        if bytes.contains(&0) {
                            return Err(EmitError::Rejected(
                                "diag-log static &str argument with an interior NUL: `%s` \
                                 truncates and byte-faithfulness is lost (ratified A-6: \
                                 fail-loud at emit time; LOG_HEXDUMP_* is the documented \
                                 opt-in fallback)"
                                    .into(),
                            ));
                        }
                        fmt.push_str("%s");
                        call_args.push(format!("\"{}\"", c_escape_arg(&bytes)));
                        continue;
                    }
                    // A non-static `&str` (a runtime place of &str type) is
                    // the A-5 gap: the bounce-buffer delivery is designed
                    // but unexercised — reject naming it.
                    if self.operand_is_str_ref(body, op) {
                        return Err(EmitError::Rejected(
                            "diag-log non-static &str argument under deferred mode: the \
                             call-site bounce-buffer delivery (ratified A-5 recommendation) \
                             is unexercised by this unit's corpus — rejected fail-loud, \
                             never silently over-read"
                                .into(),
                        ));
                    }
                    // Numeric: the certified (specifier, cast) mapping,
                    // stdint type spelling.
                    let ty = self.diag_numeric_ty(body, op)?;
                    let (spec, cast) = diag_specifier_and_cast(*modifier, &ty)?;
                    let expr = operand_expr(self, body, op)?;
                    fmt.push_str(&spec);
                    call_args.push(format!("{cast}({expr})"));
                }
            }
        }
        // NO trailing newline: the Zephyr log core terminates lines.

        self.uses_log = true;
        if call_args.is_empty() {
            out.push_str(&format!("\t{macro_name}(\"{fmt}\");\n"));
        } else {
            out.push_str(&format!(
                "\t{macro_name}(\"{fmt}\", {});\n",
                call_args.join(", ")
            ));
        }
        Ok(())
    }

    /// The `&str` carrier byte read: an `Operand::Constant` resolving to
    /// `Aggregate([Reloc -> CMS static, Uint(len)])` → the CMS bytes.
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
        let cms_id = (0..self.model.instances.len())
            .map(|i| InstanceId::from_u32(i as u32))
            .find(|id| self.model.instance_ref_of(*id) == target)?;
        let bytes_cid = match &self.model.instances[cms_id].kind {
            InstanceKind::Static { init: Some(c), .. } => *c,
            _ => return None,
        };
        let raw = match self.model.consts.resolve(bytes_cid) {
            Const::Bytes(bytes) => bytes,
            _ => return None,
        };
        if len > raw.len() {
            return None;
        }
        let mut out = Vec::with_capacity(len);
        for b in &raw[..len] {
            match b {
                regseq_model::Byte::Value(v) => out.push(*v),
                _ => return None,
            }
        }
        Some(out)
    }

    /// Whether an operand is a runtime `&str` place (the A-5 shape).
    fn operand_is_str_ref(&self, body: &regseq_model::Body, op: &Operand) -> bool {
        let place = match op {
            Operand::Copy(p) | Operand::Move(p) => p,
            Operand::Constant(_) => return false,
        };
        let Ok(ty) = place_ty(self.model, body, place) else {
            return false;
        };
        matches!(
            &self.model.types[ty],
            Ty::Ref(p, _) if matches!(&self.model.types[*p], Ty::Str)
        )
    }

    /// The numeric type of a diag-log argument (a place's declared type or a
    /// scalar constant's kind).
    fn diag_numeric_ty(&self, body: &regseq_model::Body, op: &Operand) -> Result<Ty, EmitError> {
        match op {
            Operand::Copy(p) | Operand::Move(p) => {
                let ty = place_ty(self.model, body, p)?;
                Ok(self.model.types[ty].clone())
            }
            Operand::Constant(id) => match &self.model.consts[*id] {
                Const::Bool(_) => Ok(Ty::Bool),
                Const::Uint(kind, _) => Ok(Ty::Uint(*kind)),
                Const::Int(kind, _) => Ok(Ty::Int(*kind)),
                other => Err(EmitError::Unsupported(format!(
                    "diag-log argument constant {other:?}"
                ))),
            },
        }
    }
}

/// The (cbprintf specifier, cast) for a numeric placeholder — the certified
/// mapping with stdint type spelling for the hex source-width reinterpret.
/// `%lld/%llu` require the A-9 `CONFIG_CBPRINTF_FULL_INTEGRAL` pin the TU
/// header documents.
fn diag_specifier_and_cast(modifier: Modifier, ty: &Ty) -> Result<(String, String), EmitError> {
    use regseq_model::{IntKind, UintKind};
    match modifier {
        Modifier::None => {
            let (spec, cast) = match ty {
                Ty::Bool | Ty::Int(IntKind::I8 | IntKind::I16 | IntKind::I32) => ("%d", "(int)"),
                Ty::Uint(UintKind::U8 | UintKind::U16 | UintKind::U32) => ("%u", "(unsigned)"),
                Ty::Uint(UintKind::U64) => ("%llu", "(unsigned long long)"),
                Ty::Int(IntKind::I64) => ("%lld", "(long long)"),
                _ => {
                    return Err(EmitError::Unsupported(
                        "diag-log argument of non-admissible type".into(),
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
                        "diag-log hex modifier on a non-integer operand".into(),
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
            Ok((
                format!("%{hash}{pad}{length}{letter}"),
                format!("{varargs}{src_cast}"),
            ))
        }
    }
}

/// A `&str` placeholder admits only the default `{}` modifier.
fn require_default_modifier(modifier: Modifier) -> Result<(), EmitError> {
    if matches!(modifier, Modifier::None) {
        Ok(())
    } else {
        Err(EmitError::Unsupported(
            "diag-log hex modifier on a non-integer operand".into(),
        ))
    }
}

/// Parse a format template into segments (the bridge's accepted grammar:
/// `{}`, the eight hex forms, `{{`/`}}` escapes).
fn parse_template(bytes: &[u8]) -> Result<Vec<Segment>, EmitError> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut literal: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                if bytes.get(i + 1) == Some(&b'{') {
                    literal.push(b'{');
                    i += 2;
                    continue;
                }
                let close = bytes[i + 1..]
                    .iter()
                    .position(|&c| c == b'}')
                    .map(|p| i + 1 + p)
                    .ok_or_else(|| {
                        EmitError::Unsupported("diag-log unclosed `{` in template".into())
                    })?;
                let modifier = parse_placeholder(&bytes[i + 1..close])?;
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(Segment::Placeholder(modifier));
                i = close + 1;
            }
            b'}' => {
                if bytes.get(i + 1) == Some(&b'}') {
                    literal.push(b'}');
                    i += 2;
                    continue;
                }
                return Err(EmitError::Unsupported(
                    "diag-log stray `}` in template".into(),
                ));
            }
            b => {
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

/// Parse a placeholder's inner text into a [`Modifier`] (exact-match set).
fn parse_placeholder(inner: &[u8]) -> Result<Modifier, EmitError> {
    if inner.is_empty() {
        return Ok(Modifier::None);
    }
    if inner[0] != b':' {
        return Err(EmitError::Unsupported(
            "diag-log non-empty placeholder spec".into(),
        ));
    }
    let m = |upper, alternate, width| Modifier::Hex {
        upper,
        alternate,
        width,
    };
    Ok(match &inner[1..] {
        b"x" => m(false, false, 0),
        b"X" => m(true, false, 0),
        b"#x" => m(false, true, 0),
        b"#X" => m(true, true, 0),
        b"08x" => m(false, false, 8),
        b"08X" => m(true, false, 8),
        b"#010x" => m(false, true, 10),
        b"#010X" => m(true, true, 10),
        _ => {
            return Err(EmitError::Unsupported(
                "diag-log out-of-subset format modifier".into(),
            ))
        }
    })
}

/// Escape a literal run for use INSIDE a cbprintf format string: C escapes +
/// `%` doubled to `%%` (the run is now format text, unlike the oracle's raw
/// fwrite).
fn c_escape_fmt(bytes: &[u8]) -> String {
    let mut out = c_escape_arg(bytes);
    out = out.replace('%', "%%");
    out
}

/// Escape a byte run as a C string-literal body (no `%` doubling: argument
/// strings are not format text).
fn c_escape_arg(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{:03o}", b)),
        }
    }
    out
}
