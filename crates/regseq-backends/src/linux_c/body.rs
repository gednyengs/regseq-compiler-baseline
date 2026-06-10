//! Subset MIR-body lowering for the linux_c backend.
//!
//! Lowers a function instance's post-monomorphization body to kernel C:
//! locals declared up front, CFG-flattened basic blocks with `bb_<n>:`
//! labels and `goto` terminators (the same flattening discipline the
//! reference backend certifies), kernel type spelling throughout.
//!
//! The supported construct set grows task by task (T04 minimal scaffold →
//! T06 register access → T08 config → T09 diag-log); anything outside it is
//! a loud [`EmitError::Unsupported`] naming the construct — never wrong code
//! (I06). Intrinsic-anchored calls (Log/Mmio/Config/PollUntil) are
//! intercepted by callee item attribute before ordinary call lowering.

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
    /// Demand-recorded extra includes (e.g. `<linux/bug.h>` for `BUG()`).
    pub extra_includes: std::collections::BTreeSet<&'static str>,
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

/// The `static <ret> <name>(<params>)` head shared by the forward
/// prototype and the definition (P1: callees may be emitted after callers,
/// so every lowered function gets a prototype before the body bank).
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

/// Emit one lowered function definition.
pub(crate) fn emit_fn(cx: &mut LowerCx<'_>, instance: InstanceId) -> Result<String, EmitError> {
    let head = fn_head(cx, instance)?;
    let inst = &cx.model.instances[instance];
    let InstanceKind::Function(data) = &inst.kind else {
        unreachable!("fn_head certified the function shape");
    };
    let body = data.body.as_ref().expect("fn_head certified a body");
    let sig = cx.model.sigs.resolve(data.fn_sig);

    let mut out = format!("{head}\n{{\n");

    // Locals (return place + temporaries); ZSTs are dropped.
    let ret_zst = types::is_zst(cx.model, sig.output);
    if !ret_zst {
        out.push_str(&format!(
            "\t{} _0;\n",
            types::c_type(cx.model, &mut cx.names, body.local_decls[body.return_local()].ty)?
        ));
    }
    for local in body.inner_locals() {
        let decl = &body.local_decls[local];
        if types::is_zst(cx.model, decl.ty) {
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
            1 => out.push_str(&format!("\t{lv}._raw[{offset}] = {:#x};\n", tag_value as u64)),
            2 | 4 | 8 => {
                let cty = match size {
                    2 => "u16",
                    4 => "u32",
                    _ => "u64",
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
            if ret_zst {
                out.push_str("\treturn;\n");
            } else {
                out.push_str("\treturn _0;\n");
            }
            Ok(())
        }
        TerminatorKind::Unreachable => {
            // Defensive trap: unreachable-by-construction control flow.
            cx.extra_includes.insert("<linux/bug.h>");
            out.push_str("\tBUG();\n");
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
        TerminatorKind::Assert { .. } => Err(EmitError::Unsupported(
            "Assert terminator (first-light subset: build fixtures with overflow checks off)"
                .into(),
        )),
    }
}

/// Ordinary (non-intrinsic) direct call emission. Intrinsic-anchored callees
/// are dispatched by the caller-provided hooks as the lowering grows
/// (T06/T08/T09); in the T04 subset every intrinsic is a loud diagnostic.
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
    if let Some(kind) = cx.intrinsic_of(*instance) {
        return Err(EmitError::Unsupported(format!(
            "intrinsic {kind:?} (not yet lowered in this revision of linux_c)"
        )));
    }
    let Some(symbol) = cx.fn_symbols.get(instance).cloned() else {
        return Err(EmitError::Unsupported(format!(
            "call to un-lowered instance `{}`",
            cx.model
                .strings
                .resolve(cx.model.instances[*instance].emission_name)
        )));
    };
    let mut rendered = Vec::new();
    for arg in args.iter() {
        if operand_is_zst(cx.model, body, arg) {
            continue;
        }
        rendered.push(operand_expr(cx, body, arg)?);
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
        Operand::Copy(p) | Operand::Move(p) => {
            place_ty(model, body, p).map(|t| types::is_zst(model, t)).unwrap_or(false)
        }
        Operand::Constant(id) => matches!(&model.consts[*id], Const::ZST),
    }
}

/// Render an operand as a C expression.
fn operand_expr(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    op: &Operand,
) -> Result<String, EmitError> {
    match op {
        Operand::Copy(place) | Operand::Move(place) => place_lvalue(cx, body, place),
        Operand::Constant(id) => const_expr(cx, *id),
    }
}

/// Render a scalar constant as a C literal.
fn const_expr(cx: &mut LowerCx<'_>, id: regseq_model::ConstId) -> Result<String, EmitError> {
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
    let field = fields.get(idx).ok_or_else(|| {
        EmitError::Unsupported(format!("field index {idx} out of bounds"))
    })?;
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
