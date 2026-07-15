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
    AggregateKind, AtomicElement, AtomicOp, AttrData, BinOp, CallCallee, CastKind, ClockBinding,
    ClockMode, ClockOp, Const, EnumShape, FetchOp, FieldIdx, FieldPath, InstanceId, InstanceKind,
    IntrinsicKind, ItemId, LayoutKind, LocalId, Operand, Place, PlaceElem, RegseqModel, Rvalue,
    StatementKind, TerminatorKind, Ty, TyId, UnOp, VariantIdx,
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
    /// Whether a `busy_wait` intrinsic was lowered (timed-cpu-delay-and-sleep
    /// construct): the TU then carries the `regseq_zephyr_busy_wait_ns`
    /// (k_busy_wait, round up to us) helper + `<zephyr/kernel.h>`.
    pub uses_delay_busy: bool,
    /// Whether a `sleep` intrinsic was lowered: the TU then carries the
    /// `regseq_zephyr_sleep_ns` (k_msleep, round up to ms) helper +
    /// `<zephyr/kernel.h>`.
    pub uses_delay_sleep: bool,
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
    /// C-sanitized driver name (`struct <drv>_data`).
    pub drv: String,
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
    // Synchronization (locks, T07/C02): the per-block C02 primitive statements,
    // the guard hidden-state locals (Zephyr `k_spinlock_key_t key`), and the dead
    // guard + receiver locals to elide. Built before the taint fixpoint so
    // guard-derived borrow temps are elided too.
    let sync_body = super::sync::build_for_body(cx, body, instance)?;
    for g in &sync_body.elide_locals {
        cx.elided_locals.insert(*g);
    }

    // Peripheral-clock-management (clock construct, M3/T15): a clock op's receiver
    // borrow temp (`_t = &self.<clock>`, the `&Clock` facade) is DEAD in the
    // emitted body — `emit_clock` lowers against the build-time DT clock node, not
    // the borrow — and the `Clock` field is a dropped ZST, so leaving it would not
    // compile. Elide it (mirrors the sync receiver elision); the taint fixpoint
    // then drops its derived borrow temps.
    for bb in body.basic_blocks.iter() {
        if let TerminatorKind::Call {
            callee: CallCallee::Direct(callee),
            args,
            ..
        } = &bb.terminator.kind
        {
            if matches!(
                cx.intrinsic_of(*callee),
                Some(IntrinsicKind::Clock(_) | IntrinsicKind::DeferredWork(_))
            ) {
                if let Some(Operand::Copy(p) | Operand::Move(p)) = args.first() {
                    if p.projection.is_empty() {
                        cx.elided_locals.insert(p.local);
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
    // Synchronization guard hidden state (T07/C02): Zephyr `k_spinlock_key_t key`
    // for each SpinLock/IrqSpinLock acquire, assigned from `k_spin_lock` and
    // consumed by `k_spin_unlock` (function-scoped so it spans acquire + release).
    for decl in &sync_body.state_locals {
        out.push_str(&format!("\t{decl}\n"));
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
        // A recognized sync acquire/release/completion block replaces its `Call`
        // terminator with the C02 primitive + a goto to the call's successor.
        let block_id = regseq_model::BlockId::from_u32(idx as u32);
        if let Some(stmt) = sync_body.stmts.get(&block_id) {
            emit_sync_terminator(&bb.terminator.kind, stmt, &mut out)?;
        } else {
            emit_terminator(cx, body, &bb.terminator.kind, ret_zst, &mut out)?;
        }
    }

    out.push_str("}\n");
    Ok(out)
}

/// The C name of a body local: `_0` for the return place, the sanitized
/// debug name when present, else `_<n>`.
pub(crate) fn local_name(model: &RegseqModel, body: &regseq_model::Body, local: LocalId) -> String {
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
        Rvalue::Discriminant(src) => {
            // The discriminant of an enum `match` (`_t = Discriminant(e);
            // switchInt(_t)`). The SwitchInt compares against the Rust semantic
            // DISCRIMINANT, so the stored tag/niche byte must be DECODED to that
            // discriminant. When the stored tag IS the discriminant (a Direct
            // enum, e.g. the serial `irq_source_set`) the raw byte already is the
            // SwitchInt value — read it directly (the legacy form, no golden
            // churn). Otherwise (a niche-encoded `Result<(), E>`) emit the
            // tag→discriminant decode ternary — without it `enable()?` (the first
            // fallible author op `?`'d in a per-target body) mis-routes.
            let src_ty = place_ty(cx.model, body, src)?;
            let src_lv = place_lvalue(cx, body, src)?;
            let (offset, size) = types::enum_tag_cell(cx.model, src_ty)?;
            let dest_c = types::c_type(cx.model, &mut cx.names, dest_ty)?;
            let rd = match size {
                1 => "uint8_t",
                2 => "uint16_t",
                4 => "uint32_t",
                8 => "uint64_t",
                other => {
                    return Err(EmitError::Unsupported(format!(
                        "enum discriminant tag width {other} (first-light subset)"
                    )))
                }
            };
            let layout = types::layout_of(cx.model, src_ty).ok_or_else(|| {
                EmitError::Unsupported("discriminant of a non-aggregate".into())
            })?;
            let LayoutKind::Enum {
                shape: EnumShape::Multiple { variants, .. },
            } = &layout.kind
            else {
                return Err(EmitError::Unsupported(
                    "discriminant of an unsupported enum shape (first-light subset)".into(),
                ));
            };
            // The raw tag/niche byte equals the SwitchInt value (the Rust
            // DISCRIMINANT, not the variant index) exactly when every variant's
            // stored tag IS its discriminant — i.e. a Direct (tag-)encoded enum,
            // including explicit discriminants. Then read it directly (the legacy
            // form: the serial `irq_source_set` enum, no golden churn). A
            // niche-encoded enum (an untagged data variant, e.g. `Result<(), E>`)
            // needs the tag→discriminant decode.
            let is_identity = variants.iter().all(|v| v.tag_value == Some(v.discriminant));
            let to_u64 = |x: u128| -> Result<u64, EmitError> {
                (x <= u64::MAX as u128).then_some(x as u64).ok_or_else(|| {
                    EmitError::Unsupported("128-bit enum tag/discriminant (first-light subset)".into())
                })
            };
            if is_identity && size == 1 {
                out.push_str(&format!("\t{lv} = ({dest_c}){src_lv}._raw[{offset}];\n"));
            } else if is_identity {
                cx.extra_includes.insert("<string.h>");
                out.push_str(&format!(
                    "\t{{ {rd} __d; memcpy(&__d, &{src_lv}._raw[{offset}], sizeof(__d)); {lv} = ({dest_c})__d; }}\n"
                ));
            } else {
                cx.extra_includes.insert("<string.h>");
                // Decode the stored tag → the Rust discriminant (what SwitchInt
                // compares). The else is the untagged niche data variant, or the
                // last variant for a fully-tagged enum (dead).
                let untagged: Vec<usize> = variants
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.tag_value.is_none())
                    .map(|(i, _)| i)
                    .collect();
                if untagged.len() > 1 {
                    return Err(EmitError::Unsupported(
                        "enum with multiple untagged variants (first-light subset)".into(),
                    ));
                }
                let else_idx = untagged
                    .first()
                    .copied()
                    .unwrap_or(variants.len().saturating_sub(1));
                let mut ternary = format!("{:#x}", to_u64(variants[else_idx].discriminant)?);
                for (i, v) in variants.iter().enumerate().rev() {
                    if i == else_idx {
                        continue;
                    }
                    let tag = v.tag_value.ok_or_else(|| {
                        EmitError::Unsupported(
                            "untagged enum variant outside the niche-else position".into(),
                        )
                    })?;
                    ternary = format!(
                        "(__d == {:#x} ? {:#x} : {})",
                        to_u64(tag)?,
                        to_u64(v.discriminant)?,
                        ternary
                    );
                }
                out.push_str(&format!(
                    "\t{{ {rd} __d; memcpy(&__d, &{src_lv}._raw[{offset}], sizeof(__d)); {lv} = ({dest_c})({ternary}); }}\n"
                ));
            }
            Ok(())
        }
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
/// Emit a recognized synchronization site's C02 primitive in place of its `Call`
/// terminator, followed by a `goto` to the suppressed call's normal successor
/// (synchronization construct, T07/C02). Mirrors the linux_c twin.
fn emit_sync_terminator(
    kind: &TerminatorKind,
    primitive: &str,
    out: &mut String,
) -> Result<(), EmitError> {
    let TerminatorKind::Call { target, .. } = kind else {
        return Err(EmitError::Unsupported(
            "synchronization acquire/release/completion site terminator is not a Call".into(),
        ));
    };
    out.push_str(&format!("\t{primitive}\n"));
    match target {
        Some(t) => {
            out.push_str(&format!("\tgoto bb_{};\n", t.as_u32()));
            Ok(())
        }
        None => Err(EmitError::Unsupported(
            "diverging synchronization call (first-light subset)".into(),
        )),
    }
}

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
            // Timed-CPU-delay-and-sleep lowering (timed-cpu-delay-and-sleep
            // construct, C04/D09). `busy_wait` lowers to the
            // `regseq_zephyr_busy_wait_ns` helper (`k_busy_wait(us)`, round ns
            // up to us); `sleep` to `regseq_zephyr_sleep_ns` (`k_msleep(ms)`,
            // round ns up to ms — the kernel further rounds to the tick floor).
            // Zephyr needs no magnitude bands (k_busy_wait always takes us), so
            // the helper is just the overflow-safe ns->unit round-up.
            IntrinsicKind::BusyWait | IntrinsicKind::Sleep => {
                emit_delay(cx, kind, *instance, body, args, out)
            }
            // Atomic-memory-access-and-ordering lowering (atomicity construct):
            // the per-target Zephyr atomics emission lands in M4/T16. The
            // generic-c host oracle (M1) is the only backend exercised before
            // then; route here so the dispatch is exhaustive.
            IntrinsicKind::Atomic(op) => {
                emit_atomic(cx, op, *instance, body, args, destination, out)
            }
            // Peripheral-clock-management lowering (clock construct, M3/T15,
            // C04/D12): managed enable/disable/get_rate route to the Zephyr
            // clock_control API over the build-time DT clock node; static
            // enable/disable are no-ops and get_rate const-folds (D07).
            IntrinsicKind::Clock(op) => emit_clock(cx, op, body, args, destination, out),
            // Pin-controller-and-GPIO-configuration lowering (pinctrl
            // state-selection construct, M4/T21, C04/D10): `apply::<S>()` over a
            // RESOLVED binding lowers to `pinctrl_apply_state(cfg, PINCTRL_STATE_<S>)`
            // over the build-time DT pin config (required checks rc, optional
            // ignores). An optional-ABSENT binding compile-time-folds to a direct
            // `Err(PinctrlAbsent)`, no API call (D06/C04).
            IntrinsicKind::Pinctrl(op) => {
                emit_pinctrl(cx, op, *instance, body, args, destination, out)
            }
            IntrinsicKind::DeferredWork(_) => emit_deferred_work(cx, body, args, out),
            IntrinsicKind::CommandTransportSubmit => {
                emit_command_transport_submit(cx, body, args, destination, out)
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

/// Lower the locked command-transport operation to immediate Zephyr mailbox
/// admission.  Both operands remain opaque words owned by L2: the endpoint is
/// a pre-resolved `mbox_dt_spec`, and the payload points at the native message.
fn emit_command_transport_submit(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    args: &[Operand],
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    if args.len() != 2 {
        return Err(EmitError::Unsupported(
            "command_transport_submit requires endpoint and payload operands".into(),
        ));
    }
    cx.extra_includes.insert("<string.h>");
    cx.extra_includes.insert("<errno.h>");
    cx.extra_includes.insert("<zephyr/drivers/mbox.h>");

    let endpoint = operand_expr(cx, body, &args[0])?;
    let payload = operand_expr(cx, body, &args[1])?;
    let dst = place_lvalue(cx, body, destination)?;
    let dst_ty = place_ty(cx.model, body, destination)?;

    out.push_str(&format!(
        "\t{{ const struct mbox_dt_spec *__endpoint = (const struct mbox_dt_spec *)(uintptr_t)({endpoint});\n\t  int __reason = __endpoint && __endpoint->dev\n\t\t? mbox_send(__endpoint->dev, __endpoint->channel_id, (const struct mbox_msg *)(uintptr_t)({payload}))\n\t\t: -EINVAL;\n\t  if (__reason == 0) {{\n"
    ));
    out.push_str(&format!("\t\tmemset(&{dst}, 0, sizeof({dst}));\n"));
    emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(0), out)?;
    out.push_str("\t  } else {\n");
    cx_emit_cas_result(cx, &dst, dst_ty, 1, "__reason", out)?;
    out.push_str("\t  } }\n");
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
/// The build-time Zephyr clock-controller device expression for the clock at
/// declaration index `idx` (its position in the driver's DT `clocks` list):
/// `DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, idx))` (single-instance, the
/// `DT_INST_IRQN(0)` discipline). Authoritative on Zephyr (D12).
pub(crate) fn clock_ctlr_expr(idx: usize) -> String {
    format!("DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, {idx}))")
}

/// The clock subsystem (specifier) argument for the clock at index `idx`. The
/// real PL011-Zephyr-qemu target is STATIC, so the managed subsys is SYNTHETIC
/// (D12): the binding's clock-specifier cell, spelled via the standard
/// `DT_INST_CLOCKS_CELL_BY_IDX` accessor (the `id` cell name is the synthetic
/// stand-in for the per-SoC specifier — it is never built on a real board).
fn clock_subsys_expr(idx: usize) -> String {
    format!("(clock_control_subsys_t)DT_INST_CLOCKS_CELL_BY_IDX(0, {idx}, id)")
}

/// Recover the `(binding, declaration index)` named by a `Clock` wrapper-method
/// `&self.clk` receiver (peripheral-clock-management construct, M3/T15): chase the
/// receiver to the borrowed driver-struct field, read the `AttrData::ClockResource
/// { id }` marker for the L1 id, and find the L2-resolved `ClockBinding` plus the
/// resource's position (the DT clock index) in `model.clock`. Mirrors the linux_c
/// `recover_clock`; zephyr lowers against the build-time DT node, so no field
/// lvalue is returned.
fn recover_clock(
    cx: &LowerCx<'_>,
    body: &regseq_model::Body,
    recv: &Operand,
) -> Result<(ClockBinding, usize), EmitError> {
    let src = cx.resolve_receiver_src(body, recv, 0)?;
    let mut ty = body.local_decls[src.local].ty;
    let mut field: Option<(ItemId, FieldIdx)> = None;
    for elem in src.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &cx.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => {
                        return Err(EmitError::Unsupported(
                            "clock receiver deref of a non-pointer type".into(),
                        ))
                    }
                };
            }
            PlaceElem::Field(idx, fty) => {
                if let Ty::Struct { item, .. } = &cx.model.types[ty] {
                    field = Some((*item, *idx));
                }
                ty = *fty;
            }
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                return Err(EmitError::Unsupported(
                    "clock receiver indexes an array (the array form is not built in v1)".into(),
                ));
            }
            _ => {
                return Err(EmitError::Unsupported(
                    "clock receiver has an unsupported projection element".into(),
                ))
            }
        }
    }
    let (item, fidx) = field.ok_or_else(|| {
        EmitError::Unsupported("clock receiver does not resolve to a driver field".into())
    })?;
    let field_path = FieldPath {
        item,
        variant: VariantIdx::from_u32(0),
        field: fidx,
    };
    let id = cx
        .model
        .domain_attrs
        .field_attrs
        .get(&field_path)
        .and_then(|attrs| {
            attrs.iter().find_map(|a| match a {
                AttrData::ClockResource { id, .. } => Some(*id),
                _ => None,
            })
        })
        .ok_or_else(|| {
            EmitError::Unsupported("clock receiver field has no recorded ClockResource marker".into())
        })?;
    cx.model
        .clock
        .drivers
        .get(&item)
        .and_then(|d| {
            d.resources
                .iter()
                .enumerate()
                .find(|(_, r)| r.id == id)
                .map(|(i, r)| (r.binding.clone(), i))
        })
        .ok_or_else(|| {
            EmitError::Unsupported("clock receiver field has no resolved ClockResource binding".into())
        })
}

fn recover_deferred_work(
    cx: &LowerCx<'_>,
    body: &regseq_model::Body,
    recv: &Operand,
) -> Result<(usize, Place), EmitError> {
    let src = cx.resolve_receiver_src(body, recv, 0)?;
    let mut ty = body.local_decls[src.local].ty;
    let mut field = None;
    for elem in src.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &cx.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => return Err(EmitError::Unsupported("deferred-work receiver deref of a non-pointer type".into())),
                };
            }
            PlaceElem::Field(idx, fty) => {
                if let Ty::Struct { item, .. } = &cx.model.types[ty] {
                    field = Some((*item, *idx));
                }
                ty = *fty;
            }
            _ => return Err(EmitError::Unsupported("deferred-work receiver has an unsupported projection".into())),
        }
    }
    let (item, field) = field.ok_or_else(|| EmitError::Unsupported("deferred-work receiver does not resolve to a driver field".into()))?;
    let idx = cx
        .model
        .deferred_work
        .drivers
        .get(&item)
        .and_then(|d| d.resources.iter().position(|r| r.field == field))
        .ok_or_else(|| EmitError::Unsupported("deferred-work receiver field is not a recognized Work resource".into()))?;
    let parent = Place {
        local: src.local,
        projection: src.projection[..src.projection.len() - 1]
            .to_vec()
            .into_boxed_slice(),
    };
    Ok((idx, parent))
}

fn emit_deferred_work(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    args: &[Operand],
    out: &mut String,
) -> Result<(), EmitError> {
    let recv = args.first().ok_or_else(|| EmitError::Unsupported("deferred-work submit without a receiver".into()))?;
    let (idx, driver) = recover_deferred_work(cx, body, recv)?;
    let driver = place_lvalue(cx, body, &driver)?;
    cx.extra_includes.insert("<zephyr/kernel.h>");
    out.push_str(&format!(
        "\t{{ struct {}_data *__rseq_data = CONTAINER_OF(&{}, struct {}_data, inner);\n\t  (void)k_work_submit(&__rseq_data->__rseq_work_{}); }}\n",
        cx.driver.drv, driver, cx.driver.drv, idx
    ));
    Ok(())
}

/// Peripheral-clock-management author-op lowering for zephyr_c (clock construct,
/// M3/T15, C04/D12):
///   * MANAGED — `enable` → `clock_control_on`; `disable` → `clock_control_off`;
///     `get_rate` → `clock_control_get_rate` (raw, no fallback, D10/R10) over the
///     build-time DT clock node `DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, idx))`.
///     A `clock_control_on` failure PROPAGATES as `Err` (the author's `?`); a
///     `clock_control_get_rate` failure FAILS LOUD (`k_panic` — `get_rate` has no
///     error channel) — neither is silently swallowed. Witnessed synthetically
///     (the real PL011-Zephyr target is static).
///   * STATIC — `enable`/`disable` are no-ops; `get_rate` const-folds to the
///     resolved `default_frequency` (D07). No `clock_control_*` is emitted.
/// The caller (`emit_terminator`) appends the `goto bb_<target>`.
fn emit_clock(
    cx: &mut LowerCx<'_>,
    op: ClockOp,
    body: &regseq_model::Body,
    args: &[Operand],
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    let recv = args
        .first()
        .ok_or_else(|| EmitError::Unsupported("clock op call without a receiver".into()))?;
    let (binding, idx) = recover_clock(cx, body, recv)?;
    let is_static = binding.mode == ClockMode::Static;
    if !is_static {
        cx.extra_includes.insert("<zephyr/drivers/clock_control.h>");
    }
    let ctlr = clock_ctlr_expr(idx);
    let subsys = clock_subsys_expr(idx);
    match op {
        ClockOp::Enable => {
            let dst_ty = place_ty(cx.model, body, destination)?;
            let dst = place_lvalue(cx, body, destination)?;
            if is_static {
                // static: no-op enable → Ok(()) (variant 0, unit payload).
                emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(0), out)?;
            } else {
                // managed: a `clock_control_on` failure PROPAGATES as Err (the
                // author's `?` returns it; the scaffold adapter maps it to the
                // negated errno) — never silently swallowed. v1 maps any enable
                // failure to `RegseqError`'s first variant (→ -EIO); `memset`
                // zeroes the niche-encoded carrier to `Err(<first>)`.
                cx.extra_includes.insert("<string.h>");
                out.push_str(&format!("\tif (clock_control_on({ctlr}, {subsys}) != 0) {{\n"));
                out.push_str(&format!("\t\tmemset(&{dst}, 0, sizeof({dst}));\n"));
                emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(1), out)?;
                out.push_str("\t} else {\n");
                emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(0), out)?;
                out.push_str("\t}\n");
            }
        }
        ClockOp::Disable => {
            // Infallible (`-> ()`): no destination write. static → no-op.
            if !is_static {
                out.push_str(&format!("\tclock_control_off({ctlr}, {subsys});\n"));
            }
        }
        ClockOp::GetRate => {
            let dst = place_lvalue(cx, body, destination)?;
            let dst_ty = place_ty(cx.model, body, destination)?;
            let cty = types::c_type(cx.model, &mut cx.names, dst_ty)?;
            if is_static {
                let freq = binding.default_frequency.ok_or_else(|| {
                    EmitError::Unsupported(
                        "static clock get_rate without a resolved default_frequency".into(),
                    )
                })?;
                out.push_str(&format!("\t{dst} = ({cty}){freq}u;\n"));
            } else {
                // managed: the raw live rate query (no fallback, D10/R10). The
                // Zephyr API returns the rate through an out-pointer + a status;
                // `get_rate` has no error channel (returns `u32`), so a query
                // failure FAILS LOUD (`k_panic`) rather than silently returning a
                // fallback 0 — the codebase's unrecoverable-trap idiom.
                cx.extra_includes.insert("<zephyr/kernel.h>");
                out.push_str(&format!(
                    "\t{{ uint32_t __rate = 0; if (clock_control_get_rate({ctlr}, {subsys}, &__rate) != 0) k_panic(); {dst} = ({cty})__rate; }}\n"
                ));
            }
        }
    }
    Ok(())
}

/// Pin-controller-and-GPIO-configuration author-op lowering (pinctrl construct,
/// M4/T21, C04/D10): lower an `IntrinsicKind::Pinctrl(op)` `apply::<S>()` Call to
/// `pinctrl_apply_state(cfg, PINCTRL_STATE_<S>)` over the build-time DT pin config
/// (`PINCTRL_DT_INST_DEV_CONFIG_GET(0)`). REQUIRED checks the rc and constructs
/// `Err` on failure (v1 → RegseqError's first variant / -EIO, the clock-enable
/// precedent); OPTIONAL ignores the rc (best-effort, C04) and constructs `Ok(())`.
/// An optional-ABSENT (unresolved) binding compile-time-folds to a direct
/// `Err(RegseqError::PinctrlAbsent)`, no API call ([`emit_pinctrl_absent_fold`]).
/// Only the standard `default`/`sleep` states lower in M4 (D10); a non-standard
/// state is a loud error (a custom `PINCTRL_STATE_PRIV_START` id is design-in).
/// Unlike linux_c there is no handle/field morph and no `dev` recovery — the
/// config is a DT constant, so required and optional share the same API.
fn emit_pinctrl(
    cx: &mut LowerCx<'_>,
    op: regseq_model::PinctrlOp,
    instance: InstanceId,
    body: &regseq_model::Body,
    args: &[Operand],
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    let recv = args
        .first()
        .ok_or_else(|| EmitError::Unsupported("pinctrl op call without a receiver".into()))?;
    let (item, id, binding) = regseq_model::recover_pinctrl_receiver(cx.model, body, recv)
        .ok_or_else(|| {
            EmitError::Unsupported(
                "pinctrl receiver does not resolve to a single PinCtrl resource field".into(),
            )
        })?;
    // Optional-ABSENT ⇒ the D06 compile-time branch-fold: a direct
    // `Err(PinctrlAbsent)`, no API call (the absent-only TU stays
    // pinctrl-scaffold-free). Uniform across BOTH absent paths.
    if !binding.is_resolved() {
        return emit_pinctrl_absent_fold(cx, body, destination, out);
    }
    let required = cx
        .model
        .pinctrl
        .drivers
        .get(&item)
        .and_then(|d| d.resources.iter().find(|r| r.pinctrl_id == id))
        .map(|r| r.required)
        .ok_or_else(|| {
            EmitError::Unsupported(
                "pinctrl resolved receiver has no matching resource record".into(),
            )
        })?;
    // Resolve the selected state from `S` (the single source of truth shared with
    // the generic-c backend + the membership pass, C03). M4 lowers only the
    // standard `default`/`sleep` states (D10) — a non-standard state is a loud
    // error here (the custom-state path is design-in, Codex M4 #7).
    let regseq_model::PinctrlOp::ApplyState = op;
    let state =
        regseq_model::resolve_pin_state(cx.model, &cx.model.instances[instance].generic_args)
            .map(|s| cx.model.strings.resolve(s).to_string())
            .ok_or_else(|| {
                EmitError::Unsupported(
                    "pinctrl `apply::<S>()` generic arg `S` resolves to no `#[rseq::pin_state]` \
                     marker"
                        .into(),
                )
            })?;
    let state_macro = match state.as_str() {
        "default" => "PINCTRL_STATE_DEFAULT",
        "sleep" => "PINCTRL_STATE_SLEEP",
        other => {
            return Err(EmitError::Unsupported(format!(
                "pinctrl `apply::<{other}>()` — only the standard `default`/`sleep` states are \
                 lowered on the Zephyr backend in M4 (D10); a non-standard state needs a custom \
                 `PINCTRL_STATE_PRIV_START` id (design-in)"
            )))
        }
    };
    let dst_ty = place_ty(cx.model, body, destination)?;
    let dst = place_lvalue(cx, body, destination)?;
    cx.extra_includes.insert("<zephyr/drivers/pinctrl.h>");
    let cfg = super::pinctrl::cfg_expr();
    if required {
        // REQUIRED: check the rc; failure constructs `Err` (v1 → RegseqError's
        // first variant / -EIO; `memset` zeroes the niche carrier to Err(first),
        // the clock-enable precedent).
        cx.extra_includes.insert("<string.h>");
        out.push_str(&format!(
            "\tif (pinctrl_apply_state({cfg}, {state_macro}) != 0) {{\n"
        ));
        out.push_str(&format!("\t\tmemset(&{dst}, 0, sizeof({dst}));\n"));
        emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(1), out)?;
        out.push_str("\t} else {\n");
        emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(0), out)?;
        out.push_str("\t}\n");
    } else {
        // OPTIONAL: best-effort — the rc is ignored (C04). Then `Ok(())`.
        out.push_str(&format!(
            "\t(void)pinctrl_apply_state({cfg}, {state_macro});\n"
        ));
        emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(0), out)?;
    }
    Ok(())
}

/// Pin-controller-and-GPIO-configuration optional-ABSENT branch-fold (pinctrl
/// construct, M4/T21, D06/C04): the zephyr_c mirror of linux_c's fold — an
/// `apply::<S>()` on an UNRESOLVED binding compile-time-folds to a direct
/// `Err(RegseqError::PinctrlAbsent)`, NO `pinctrl_*` API call. Writes the inner
/// `RegseqError` tag for `PinctrlAbsent` (BY NAME) at the COMPOSITE offset (the
/// Result `Err` payload-field offset + the RegseqError tag offset), from the
/// resolved layout — robust to the niche encoding (Codex M4 #1). The scaffold
/// errno adapter maps `PinctrlAbsent` -> `ENODEV` (T19/D12).
fn emit_pinctrl_absent_fold(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    let dst_ty = place_ty(cx.model, body, destination)?;
    let dst = place_lvalue(cx, body, destination)?;
    let (composite_offset, tag_size, tag_value, err_idx) = {
        let result_layout = types::layout_of(cx.model, dst_ty).ok_or_else(|| {
            EmitError::Unsupported("pinctrl absent fold: Result destination has no layout".into())
        })?;
        let LayoutKind::Enum {
            shape: EnumShape::Multiple { variants, .. },
        } = &result_layout.kind
        else {
            return Err(EmitError::Unsupported(
                "pinctrl absent fold: destination is not a multi-variant enum (expected Result)"
                    .into(),
            ));
        };
        let err_idx = variants
            .iter()
            .position(|v| cx.model.strings.resolve(v.name) == "Err")
            .ok_or_else(|| {
                EmitError::Unsupported("pinctrl absent fold: Result has no Err variant".into())
            })?;
        let err_field = variants[err_idx].fields.first().ok_or_else(|| {
            EmitError::Unsupported(
                "pinctrl absent fold: Result Err variant has no payload field".into(),
            )
        })?;
        let err_payload_offset = err_field.offset;
        let regseqerror_ty = err_field.ty;
        let (re_tag_off, re_tag_size) = types::enum_tag_cell(cx.model, regseqerror_ty)?;
        let re_layout = types::layout_of(cx.model, regseqerror_ty).ok_or_else(|| {
            EmitError::Unsupported("pinctrl absent fold: RegseqError has no layout".into())
        })?;
        let LayoutKind::Enum {
            shape: EnumShape::Multiple { variants: re_vars, .. },
        } = &re_layout.kind
        else {
            return Err(EmitError::Unsupported(
                "pinctrl absent fold: RegseqError is not a multi-variant enum".into(),
            ));
        };
        let pa = re_vars
            .iter()
            .find(|v| cx.model.strings.resolve(v.name) == "PinctrlAbsent")
            .ok_or_else(|| {
                EmitError::Unsupported(
                    "pinctrl absent fold: RegseqError has no `PinctrlAbsent` variant (the optional \
                     error surface must declare it, D12)"
                        .into(),
                )
            })?;
        let tag = pa.tag_value.ok_or_else(|| {
            EmitError::Unsupported(
                "pinctrl absent fold: the PinctrlAbsent variant has no direct tag_value".into(),
            )
        })?;
        (err_payload_offset + re_tag_off, re_tag_size, tag, err_idx)
    };
    if tag_value > u64::MAX as u128 {
        return Err(EmitError::Unsupported(
            "pinctrl absent fold: 128-bit tag value (first-light subset)".into(),
        ));
    }
    out.push_str(
        "\t/* pinctrl optional-absent fold (D06/C04): Err(RegseqError::PinctrlAbsent) -> ENODEV; \
         no pinctrl API call */\n",
    );
    emit_enum_tag_write(cx, dst_ty, &dst, VariantIdx::from_u32(err_idx as u32), out)?;
    let tag = tag_value as u64;
    match tag_size {
        1 => out.push_str(&format!("\t{dst}._raw[{composite_offset}] = {tag:#x};\n")),
        2 | 4 | 8 => {
            let cty = match tag_size {
                2 => "uint16_t",
                4 => "uint32_t",
                _ => "uint64_t",
            };
            out.push_str(&format!(
                "\t{{ {cty} __tag = {tag:#x}; memcpy(&{dst}._raw[{composite_offset}], &__tag, \
                 sizeof(__tag)); }}\n"
            ));
            cx.extra_includes.insert("<string.h>");
        }
        other => {
            return Err(EmitError::Unsupported(format!(
                "pinctrl absent fold: enum tag width {other} (first-light subset)"
            )))
        }
    }
    Ok(())
}

pub(crate) fn operand_expr_typed(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    op: &Operand,
    consumer: Option<TyId>,
) -> Result<String, EmitError> {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            // An enum-variant payload READ (`(_e as Variant).0`) appears in the
            // `?`-operator desugar of a fallible author op (the clock construct's
            // `enable()?` is the first to reach zephyr_c). The opaque `_raw`
            // carrier has no union view, so read the variant field through an
            // alignment-/aliasing-safe memcpy statement-expression rather than
            // routing the `Downcast` through `place_lvalue` (which rejects it, so
            // a `Downcast` in a write / `&` position still fails loud).
            if place
                .projection
                .iter()
                .any(|e| matches!(e, PlaceElem::Downcast(_)))
            {
                read_downcast_payload(cx, body, place)
            } else {
                place_lvalue(cx, body, place)
            }
        }
        Operand::Constant(id) => const_expr(cx, *id, consumer),
    }
}

/// Read an enum-variant payload place (`<prefix> as Variant).<field>`) as a C
/// value expression: walk the prefix to the enum carrier lvalue, then read the
/// variant field from the opaque `_raw[..]` carrier at its layout offset via a
/// memcpy statement-expression (alignment-/aliasing-safe, the `Rvalue::Discriminant`
/// memcpy idiom). v1 supports the `?`-desugar shape — a single `Downcast`
/// immediately followed by its payload `Field`, with no trailing projection.
fn read_downcast_payload(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    place: &Place,
) -> Result<String, EmitError> {
    let mut expr = local_name(cx.model, body, place.local);
    let mut ty = body.local_decls[place.local].ty;
    let mut iter = place.projection.iter().peekable();
    while let Some(elem) = iter.next() {
        match elem {
            PlaceElem::Deref => {
                expr = format!("(*{expr})");
                ty = match &cx.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    other => {
                        return Err(EmitError::Unsupported(format!(
                            "deref of non-pointer {other:?} in a downcast read"
                        )))
                    }
                };
            }
            PlaceElem::Field(idx, field_ty) => {
                let fname = field_c_name(cx.model, ty, idx.as_u32() as usize)?;
                expr = format!("{expr}.{fname}");
                ty = *field_ty;
            }
            PlaceElem::Downcast(variant) => {
                let Some(PlaceElem::Field(fidx, field_ty)) = iter.next() else {
                    return Err(EmitError::Unsupported(
                        "Downcast not followed by a Field (malformed place)".into(),
                    ));
                };
                if iter.peek().is_some() {
                    return Err(EmitError::Unsupported(
                        "projection after a downcast variant field (first-light subset)".into(),
                    ));
                }
                let offset = {
                    let layout = types::layout_of(cx.model, ty).ok_or_else(|| {
                        EmitError::Unsupported("downcast read of a type with no layout".into())
                    })?;
                    let LayoutKind::Enum {
                        shape: EnumShape::Multiple { variants, .. },
                    } = &layout.kind
                    else {
                        return Err(EmitError::Unsupported(
                            "downcast read of a non-tagged enum (first-light subset)".into(),
                        ));
                    };
                    let vlayout = variants.get(variant.as_u32() as usize).ok_or_else(|| {
                        EmitError::Unsupported("downcast variant index out of range".into())
                    })?;
                    let f = vlayout.fields.get(fidx.as_u32() as usize).ok_or_else(|| {
                        EmitError::Unsupported("downcast variant field index out of range".into())
                    })?;
                    f.offset
                };
                let fty = types::c_type(cx.model, &mut cx.names, *field_ty)?;
                cx.extra_includes.insert("<string.h>");
                return Ok(format!(
                    "({{ {fty} __cv; memcpy(&__cv, &{expr}._raw[{offset}], sizeof(__cv)); __cv; }})"
                ));
            }
            other => {
                return Err(EmitError::Unsupported(format!(
                    "downcast-read projection {other:?} (first-light subset)"
                )))
            }
        }
    }
    place_lvalue(cx, body, place)
}

/// Render an operand with no consumer-type context.
fn operand_expr(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    op: &Operand,
) -> Result<String, EmitError> {
    operand_expr_typed(cx, body, op, None)
}

/// Timed-cpu-delay-and-sleep lowering (TCD construct): lower a `busy_wait`/
/// `sleep` Call to a call to the per-TU helper carrying the `Duration`
/// argument's u64 nanoseconds (`regseq_zephyr_busy_wait_ns` /
/// `regseq_zephyr_sleep_ns`). The `Duration` type comes from the callee's
/// first PARAMETER, and its sole field name is recovered from the layout.
/// Records the `<zephyr/kernel.h>` include and the `uses_delay_*` demand flags.
fn emit_delay(
    cx: &mut LowerCx<'_>,
    kind: IntrinsicKind,
    instance: InstanceId,
    body: &regseq_model::Body,
    args: &[Operand],
    out: &mut String,
) -> Result<(), EmitError> {
    let arg = args.first().ok_or_else(|| {
        EmitError::Unsupported("delay intrinsic call without a Duration argument".into())
    })?;
    let param_tys: Vec<TyId> = match &cx.model.instances[instance].kind {
        InstanceKind::Function(d) => cx.model.sigs.resolve(d.fn_sig).inputs.clone(),
        _ => Vec::new(),
    };
    let duration_ty = param_tys.first().copied().ok_or_else(|| {
        EmitError::Unsupported("delay intrinsic callee has no Duration parameter type".into())
    })?;
    let field = field_c_name(cx.model, duration_ty, 0)?;
    let dur_expr = operand_expr_typed(cx, body, arg, Some(duration_ty))?;
    let ns = format!("(uint64_t)({dur_expr}).{field}");
    cx.extra_includes.insert("<zephyr/kernel.h>");
    let helper = if matches!(kind, IntrinsicKind::Sleep) {
        cx.uses_delay_sleep = true;
        "regseq_zephyr_sleep_ns"
    } else {
        cx.uses_delay_busy = true;
        "regseq_zephyr_busy_wait_ns"
    };
    out.push_str(&format!("\t{helper}({ns});\n"));
    Ok(())
}

/// The plain width-`T` C storage type for an `Atomic<T>` element (atomicity
/// C07). Zephyr lowers the whole atomic surface to the GCC/Clang `__atomic_*`
/// builtins over this cell (Codex M4 review): width-parametric (no 32-bit
/// `atomic_t` ceiling), native dual-ordering CAS, and the exact memory order is
/// carried on every op — strictly more faithful than the full-order-only Zephyr
/// `atomic_t` API.
pub(crate) fn zephyr_atomic_cty(element: AtomicElement) -> &'static str {
    match element {
        AtomicElement::U32 => "uint32_t",
        AtomicElement::I32 => "int32_t",
        AtomicElement::U64 => "uint64_t",
        AtomicElement::I64 => "int64_t",
        AtomicElement::Ptr => "uintptr_t",
    }
}

/// The GCC `__ATOMIC_*` memory order for an `Ordering` discriminant.
fn zephyr_memorder(variant: u32) -> &'static str {
    match variant {
        0 => "__ATOMIC_RELAXED",
        1 => "__ATOMIC_ACQUIRE",
        2 => "__ATOMIC_RELEASE",
        3 => "__ATOMIC_ACQ_REL",
        _ => "__ATOMIC_SEQ_CST",
    }
}

/// Recover the `(field address lvalue, element)` named by an `Atomic<T>`
/// receiver (mirrors `linux_c::recover_atomic_field`): chase to the borrowed
/// driver-struct field place, take its address, read the element from the
/// `AttrData::Atomic` marker.
fn recover_atomic_field(
    cx: &mut LowerCx<'_>,
    body: &regseq_model::Body,
    recv: &Operand,
) -> Result<(String, AtomicElement), EmitError> {
    let src = cx.resolve_receiver_src(body, recv, 0)?;
    let mut ty = body.local_decls[src.local].ty;
    let mut field: Option<(ItemId, FieldIdx)> = None;
    for elem in src.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &cx.model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => return Err(EmitError::Unsupported("atomic receiver deref of non-pointer".into())),
                };
            }
            PlaceElem::Field(idx, fty) => {
                if let Ty::Struct { item, .. } = &cx.model.types[ty] {
                    field = Some((*item, *idx));
                }
                ty = *fty;
            }
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                ty = match &cx.model.types[ty] {
                    Ty::Array(elem, _) => *elem,
                    _ => return Err(EmitError::Unsupported("atomic receiver index of non-array".into())),
                };
                field = None;
            }
            _ => return Err(EmitError::Unsupported("atomic receiver unsupported projection".into())),
        }
    }
    let (item, fidx) = field.ok_or_else(|| {
        EmitError::Unsupported("atomic receiver does not resolve to a driver field".into())
    })?;
    // S01: atomic ops are supported only on a DIRECT driver-struct field; a
    // nested atomic field is rejected loud (the init seeds only the driver
    // struct).
    let driver_item = match &cx.model.types[cx.driver.driver_ty] {
        Ty::Struct { item, .. } => *item,
        _ => return Err(EmitError::Unsupported("driver type is not a struct".into())),
    };
    if item != driver_item {
        return Err(EmitError::Unsupported(
            "atomic op on a nested Atomic field is unsupported; place Atomic state \
             directly in the driver struct (S01)"
                .into(),
        ));
    }
    let (element, _) = cx.model.atomic_field_at(item, fidx).ok_or_else(|| {
        EmitError::Unsupported("atomic receiver field has no recorded Atomic marker".into())
    })?;
    let lv = place_lvalue(cx, body, &src)?;
    Ok((format!("&{lv}"), element))
}

/// The const-`Ordering` discriminant at `args[i]` (D14).
fn atomic_ordering(
    cx: &LowerCx<'_>,
    body: &regseq_model::Body,
    args: &[Operand],
    i: usize,
) -> Result<u32, EmitError> {
    let op = args
        .get(i)
        .ok_or_else(|| EmitError::Unsupported("atomic call missing the Ordering argument".into()))?;
    cx.model
        .resolve_operand_enum_variant(body, op)
        .ok_or_else(|| EmitError::Unsupported("atomic Ordering is not a compile-time constant".into()))
}

/// Atomic-memory-access-and-ordering lowering (atomicity construct) — Zephyr via
/// the GCC `__atomic_*` builtins (C03/C04/C07). Width-parametric over a plain
/// width-`T` cell; the exact `__ATOMIC_*` memory order rides every op; CAS is
/// native dual-ordering (no collapse, I07/C04); bit ops via `__atomic_fetch_or`/
/// `_and`; fence via `__atomic_thread_fence`.
fn emit_atomic(
    cx: &mut LowerCx<'_>,
    op: AtomicOp,
    _instance: InstanceId,
    body: &regseq_model::Body,
    args: &[Operand],
    destination: &Place,
    out: &mut String,
) -> Result<(), EmitError> {
    // `atomic::fence` — free fn, no receiver; order at args[0].
    if matches!(op, AtomicOp::Fence) {
        let v = atomic_ordering(cx, body, args, 0)?;
        out.push_str(&format!("\t__atomic_thread_fence({});\n", zephyr_memorder(v)));
        return Ok(());
    }
    let recv = args
        .first()
        .ok_or_else(|| EmitError::Unsupported("atomic op call without a receiver".into()))?;
    let (faddr, element) = recover_atomic_field(cx, body, recv)?;
    let cty = zephyr_atomic_cty(element);
    let dst_cty = |cx: &mut LowerCx<'_>| -> Result<String, EmitError> {
        let ty = place_ty(cx.model, body, destination)?;
        types::c_type(cx.model, &mut cx.names, ty)
    };
    match op {
        AtomicOp::Load => {
            let mo = zephyr_memorder(atomic_ordering(cx, body, args, 1)?);
            let dst = place_lvalue(cx, body, destination)?;
            let dcty = dst_cty(cx)?;
            out.push_str(&format!("\t{dst} = ({dcty})__atomic_load_n({faddr}, {mo});\n"));
        }
        AtomicOp::Store => {
            let mo = zephyr_memorder(atomic_ordering(cx, body, args, 2)?);
            let val = operand_expr(cx, body, &args[1])?;
            out.push_str(&format!("\t__atomic_store_n({faddr}, ({cty})({val}), {mo});\n"));
        }
        AtomicOp::FetchModify(fop) => {
            let mo = zephyr_memorder(atomic_ordering(cx, body, args, 2)?);
            let val = operand_expr(cx, body, &args[1])?;
            let dst = place_lvalue(cx, body, destination)?;
            let dcty = dst_cty(cx)?;
            let stmt = match fop {
                FetchOp::Add => format!("__atomic_fetch_add({faddr}, ({cty})({val}), {mo})"),
                FetchOp::Sub => format!("__atomic_fetch_sub({faddr}, ({cty})({val}), {mo})"),
                FetchOp::Or => format!("__atomic_fetch_or({faddr}, ({cty})({val}), {mo})"),
                FetchOp::And => format!("__atomic_fetch_and({faddr}, ({cty})({val}), {mo})"),
                FetchOp::Xor => format!("__atomic_fetch_xor({faddr}, ({cty})({val}), {mo})"),
                FetchOp::Swap => format!("__atomic_exchange_n({faddr}, ({cty})({val}), {mo})"),
            };
            out.push_str(&format!("\t{dst} = ({dcty}){stmt};\n"));
        }
        AtomicOp::BitTestSet | AtomicOp::BitTestClear => {
            let mo = zephyr_memorder(atomic_ordering(cx, body, args, 2)?);
            let bit = operand_expr(cx, body, &args[1])?;
            let dst = place_lvalue(cx, body, destination)?;
            let dcty = dst_cty(cx)?;
            // Mask the shift amount to width-1 so it is never UB for a runtime
            // out-of-range bit (the const out-of-range case is a frontend
            // reject; the runtime case is an author error the host oracle
            // catches). The cell cty is unsigned for the unsigned-only bit_test
            // element, so the shift itself is unsigned.
            let smask = if cty == "uint64_t" { 63 } else { 31 };
            let sh = format!("(({bit}) & {smask})");
            let rmw = match op {
                AtomicOp::BitTestSet => {
                    format!("__atomic_fetch_or({faddr}, ({cty})1 << {sh}, {mo})")
                }
                _ => format!("__atomic_fetch_and({faddr}, ~(({cty})1 << {sh}), {mo})"),
            };
            out.push_str(&format!("\t{dst} = ({dcty})((({rmw}) >> {sh}) & 1);\n"));
        }
        AtomicOp::CompareExchange => {
            // args = [recv, current, new, success, failure]. Native dual-ordering
            // (no collapse): __atomic_compare_exchange_n writes the observed prior
            // into `expected`, returns whether it committed. Build Result<T,T>.
            let succ = zephyr_memorder(atomic_ordering(cx, body, args, 3)?);
            let fail = zephyr_memorder(atomic_ordering(cx, body, args, 4)?);
            let cur = operand_expr(cx, body, &args[1])?;
            let new = operand_expr(cx, body, &args[2])?;
            let dst = place_lvalue(cx, body, destination)?;
            let dst_ty = place_ty(cx.model, body, destination)?;
            out.push_str(&format!("\t{{ {cty} __cas_exp = ({cty})({cur});\n"));
            out.push_str(&format!(
                "\t  bool __cas_ok = __atomic_compare_exchange_n({faddr}, &__cas_exp, ({cty})({new}), false, {succ}, {fail});\n"
            ));
            out.push_str("\t  if (__cas_ok) {\n");
            cx_emit_cas_result(cx, &dst, dst_ty, 0, "__cas_exp", out)?;
            out.push_str("\t  } else {\n");
            cx_emit_cas_result(cx, &dst, dst_ty, 1, "__cas_exp", out)?;
            out.push_str("\t  } }\n");
        }
        AtomicOp::Fence => unreachable!("fence handled before receiver recovery"),
    }
    Ok(())
}

/// Build a `Result<T,T>` variant (Ok=0/Err=1) carrying `payload_expr` into the
/// opaque enum carrier `dst` (the `emit_aggregate` enum pattern).
fn cx_emit_cas_result(
    cx: &mut LowerCx<'_>,
    dst: &str,
    dst_ty: TyId,
    variant_idx: u32,
    payload_expr: &str,
    out: &mut String,
) -> Result<(), EmitError> {
    use regseq_model::VariantIdx;
    cx.extra_includes.insert("<string.h>");
    let variant = VariantIdx::from_u32(variant_idx);
    out.push_str(&format!("\t\tmemset(&{dst}, 0, sizeof({dst}));\n"));
    emit_enum_tag_write(cx, dst_ty, dst, variant, out)?;
    let layout = types::layout_of(cx.model, dst_ty)
        .ok_or_else(|| EmitError::Unsupported("compare_exchange destination has no layout".into()))?;
    let LayoutKind::Enum {
        shape: EnumShape::Multiple { variants, .. },
    } = &layout.kind
    else {
        return Err(EmitError::Unsupported(
            "compare_exchange destination is not a tagged Result enum".into(),
        ));
    };
    let vlayout = &variants[variant_idx as usize];
    let f = vlayout
        .fields
        .first()
        .ok_or_else(|| EmitError::Unsupported("Result variant has no payload field".into()))?;
    let fty = types::c_type(cx.model, &mut cx.names, f.ty)?;
    out.push_str(&format!(
        "\t\t{{ {fty} __v = ({fty})({payload_expr}); memcpy(&{dst}._raw[{}], &__v, sizeof(__v)); }}\n",
        f.offset
    ));
    Ok(())
}

/// The timed-cpu-delay-and-sleep lowering helpers (TCD construct), emitted into
/// the TU before the bodies that call them (gated on `uses_delay_*`).
///
/// `regseq_zephyr_busy_wait_ns` rounds the ns request UP to microseconds and
/// calls `k_busy_wait(us)` (atomic-safe spin; no magnitude bands — k_busy_wait
/// always takes us). `regseq_zephyr_sleep_ns` rounds UP to milliseconds and
/// calls `k_msleep(ms)` (the kernel further rounds to the tick floor). Both
/// preserve the minimum-wait guarantee (R06) via the overflow-safe
/// `ns == 0 ? 0 : 1 + (ns-1)/scale` ceiling. The us/ms casts assume the
/// duration fits the kernel primitive's argument width (`k_busy_wait`'s
/// `uint32_t`, `k_msleep`'s `int32_t`) — true for any real driver delay.
pub(crate) fn delay_helpers(busy: bool, sleep: bool) -> String {
    let mut s = String::new();
    s.push_str("/* timed-cpu-delay-and-sleep (TCD construct) lowering helpers. */\n");
    if busy {
        s.push_str(
            "static void regseq_zephyr_busy_wait_ns(uint64_t ns)\n{\n\
             \tk_busy_wait((uint32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000ULL));\n}\n\n",
        );
    }
    if sleep {
        s.push_str(
            "static void regseq_zephyr_sleep_ns(uint64_t ns)\n{\n\
             \tk_msleep((int32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000000ULL));\n}\n\n",
        );
    }
    s
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
pub(crate) fn field_c_name(model: &RegseqModel, ty: TyId, idx: usize) -> Result<String, EmitError> {
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
