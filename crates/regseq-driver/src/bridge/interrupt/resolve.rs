//! Phase-5 `#[rseq::isr]` resolution + post-drain finalisation (construct #6).
//!
//! Two passes, mirroring the `device_lifecycle::resolve_drivers` seam:
//!
//!   * **[`resolve_isrs`]** runs after `resolve_drivers` and BEFORE the
//!     Phase-5 body drain. It discovers the `#[rseq::isr]` inherent methods on
//!     recognized drivers, validates the C04 handler signature (`&mut self ->
//!     IrqResult`, `index: usize` for array-bind, never `Result`), resolves
//!     each handler to its `Interrupt` resource (bare = exactly-one rule,
//!     `field = "…"` = explicit), FORCE-MATERIALIZES the handler instance (so
//!     the drain gives it a body the backend can emit), and enforces the D13
//!     exactly-one-handler discipline per non-optional resource.
//!   * **[`finalise_isrs`]** runs after the drain + the late-item barriers,
//!     when every handler body exists and `ItemId`s are final. It records the
//!     [`AttrData::IsrBinding`] marker onto each handler's function Item and
//!     computes the `IrqResult` arm-reachability facts from the materialized
//!     body (tri-state; `Unknown` is the conservative floor — the R11 warning
//!     over-warns and the R09 `DeferWork` guard fail-louds on it, never the
//!     reverse).

use std::collections::HashMap;

use regseq_model::{
    AttrData, Body, BlockId, Const, EnumShape, FrameworkType, InstanceId, InstanceKind,
    InstanceRef, InterruptForm, IrqArmReachability, ItemId, LayoutKind, LocalId, Operand, Place,
    RegseqModel, ArmReachability, Rvalue, StatementKind, StringId, TerminatorKind, Ty,
};
use rustc_ast::token::{Delimiter, LitKind as TokenLitKind, TokenKind};
use rustc_ast::tokenstream::TokenTree;
use rustc_hir::def::DefKind;
use rustc_hir::{AttrArgs, Attribute};
use rustc_middle::ty::{self, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::const_::instance_ref_for_function_provenance;
use super::super::framework_type::{recognise_framework_type_attr, recover_source_def_id};
use super::super::{BridgeCtxt, BridgeError, InterruptDiagnostic};

/// A handler's authored-attribute facts, carried from [`resolve_isrs`] (which
/// knows the authored form) to [`finalise_isrs`] (which runs after `ItemId`s
/// are final and can record the marker onto the function Item).
pub(crate) struct PendingIsrAttr {
    /// The force-materialized handler instance.
    iref: InstanceRef,
    /// The explicit `field = "…"` target, `None` for the bare form.
    explicit_field: Option<StringId>,
    /// Whether the handler is array-bound.
    array_bind: bool,
}

/// One discovered `#[rseq::isr]` method, pre-validation.
struct IsrSite {
    method: DefId,
    self_adt: DefId,
    /// `Some(field)` for `#[rseq::isr(field = "…")]`, `None` for bare.
    explicit_field: Option<String>,
}

/// Discover + validate + bind + force-materialize every `#[rseq::isr]`
/// handler. Returns the pending attribute records for [`finalise_isrs`].
pub(crate) fn resolve_isrs(cx: &mut BridgeCtxt<'_>) -> Result<Vec<PendingIsrAttr>, BridgeError> {
    let tcx = cx.input.tcx;

    // Driver-struct def-id -> model ItemId, for every recognized driver (a
    // driver with zero interrupt resources still participates: its bare ISR
    // is the ambiguous-with-zero case).
    let driver_items: HashMap<DefId, ItemId> = cx
        .model
        .device_lifecycle
        .drivers
        .keys()
        .copied()
        .filter_map(|item_id| {
            let def_id = recover_source_def_id(tcx, cx.model.item_def_paths[item_id])?;
            Some((def_id, item_id))
        })
        .collect();

    let mut pending = Vec::new();
    for site in isr_sites(tcx) {
        let Some(&driver_item) = driver_items.get(&site.self_adt) else {
            // An ISR on a non-driver impl is outside the recognized surface
            // this slice; the trait-ISR / excluded-primitive rejection is the
            // T08 IRQ-ERR-EXCLUDED-PRIMITIVE check.
            continue;
        };
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&driver_item]
                .descriptor
                .name;
            cx.model.strings.resolve(name_id).to_string()
        };
        let method_name = tcx.item_name(site.method).as_str().to_string();

        // C04 signature validation.
        let array_bind = validate_isr_signature(tcx, &site, &method_name)?;

        // Resource binding: explicit `field = "…"` or the bare exactly-one
        // rule (IRQ-ERR-BARE-ISR-AMBIGUOUS).
        let resources: Vec<(StringId, InterruptForm)> = cx
            .model
            .interrupt
            .drivers
            .get(&driver_item)
            .map(|d| d.resources.iter().map(|r| (r.name, r.form)).collect())
            .unwrap_or_default();
        let resource_idx = match &site.explicit_field {
            Some(field) => {
                let target = resources
                    .iter()
                    .position(|(name, _)| cx.model.strings.resolve(*name) == field);
                match target {
                    Some(idx) => idx,
                    None => {
                        return Err(BridgeError::InterruptValidation(
                            InterruptDiagnostic::UnknownField {
                                method: method_name,
                                field: field.clone(),
                            },
                        ))
                    }
                }
            }
            None => {
                if resources.len() != 1 {
                    return Err(BridgeError::InterruptValidation(
                        InterruptDiagnostic::BareIsrAmbiguous {
                            driver: driver_name,
                            method: method_name,
                            resource_count: resources.len(),
                        },
                    ));
                }
                0
            }
        };

        // Force-materialize the handler (inherent, non-generic) so the
        // Phase-5 drain gives it a body, exactly like the lifecycle
        // callbacks.
        let instance = ty::Instance::mono(tcx, site.method);
        let iref = instance_ref_for_function_provenance(cx, instance)?;

        let method_name_id = cx.model.strings.intern(&method_name);
        let explicit_field_id = site
            .explicit_field
            .as_deref()
            .map(|f| cx.model.strings.intern(f));
        if let Some(driver) = cx.model.interrupt.drivers.get_mut(&driver_item) {
            driver.handlers.push(regseq_model::IsrHandler {
                method_name: method_name_id,
                resource: resource_idx as u32,
                array_bind,
                body: Some(iref),
                arms: IrqArmReachability::default(),
            });
        }
        pending.push(PendingIsrAttr {
            iref,
            explicit_field: explicit_field_id,
            array_bind,
        });
    }

    // D13: exactly one handler per non-optional resource (0 on Optional is
    // fine — registration is skipped on absence; >1 is a duplicate anywhere).
    let driver_keys: Vec<ItemId> = cx.model.interrupt.drivers.keys().copied().collect();
    for item_id in driver_keys {
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(name_id).to_string()
        };
        let tables = &cx.model.interrupt.drivers[&item_id];
        for (idx, resource) in tables.resources.iter().enumerate() {
            let count = tables
                .handlers
                .iter()
                .filter(|h| h.resource as usize == idx)
                .count();
            let resource_name = cx.model.strings.resolve(resource.name).to_string();
            if count > 1 {
                return Err(BridgeError::InterruptValidation(
                    InterruptDiagnostic::DuplicateHandler {
                        driver: driver_name,
                        resource: resource_name,
                    },
                ));
            }
            if count == 0 && !matches!(resource.form, InterruptForm::Optional) {
                return Err(BridgeError::InterruptValidation(
                    InterruptDiagnostic::MissingHandler {
                        driver: driver_name,
                        resource: resource_name,
                    },
                ));
            }
        }
    }
    Ok(pending)
}

/// Walk the crate for `#[rseq::isr]` inherent methods: each site's method
/// def-id, its impl's Self ADT, and the parsed attribute form.
fn isr_sites(tcx: TyCtxt<'_>) -> Vec<IsrSite> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("isr")];
    let mut sites = Vec::new();
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let def_id = local_def_id.to_def_id();
        if !matches!(tcx.def_kind(def_id), DefKind::AssocFn) {
            continue;
        }
        let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
            continue;
        };
        let impl_def_id = tcx.parent(def_id);
        // Inherent impls only this slice; a trait-provided ISR is the T08
        // excluded-primitive rejection.
        if !matches!(tcx.def_kind(impl_def_id), DefKind::Impl { of_trait: false }) {
            continue;
        }
        let self_ty = tcx.type_of(impl_def_id).instantiate_identity();
        let Some(adt) = self_ty.ty_adt_def() else {
            continue;
        };
        sites.push(IsrSite {
            method: def_id,
            self_adt: adt.did(),
            explicit_field: parse_isr_attr_field(attr),
        });
    }
    // `definitions()` order is deterministic per compilation; sort by def
    // index for belt-and-braces determinism of diagnostic order.
    sites.sort_by_key(|s| (s.method.krate.as_u32(), s.method.index.as_u32()));
    sites
}

/// Parse `#[rseq::isr]` / `#[rseq::isr(field = "…")]` into the optional
/// explicit field. Any other argument shape reads as bare (the unknown-key
/// hardening is part of the T08 diagnostic completion).
fn parse_isr_attr_field(attr: &Attribute) -> Option<String> {
    let Attribute::Unparsed(item) = attr else {
        return None;
    };
    let AttrArgs::Delimited(delim) = &item.args else {
        return None;
    };
    if delim.delim != Delimiter::Parenthesis {
        return None;
    }
    let mut iter = delim.tokens.iter();
    let key = match iter.next() {
        Some(TokenTree::Token(tok, _)) => tok.ident().map(|(ident, _)| ident.name)?,
        _ => return None,
    };
    if key.as_str() != "field" {
        return None;
    }
    match iter.next() {
        Some(TokenTree::Token(tok, _)) if matches!(tok.kind, TokenKind::Eq) => {}
        _ => return None,
    }
    let TokenTree::Token(tok, _) = iter.next()? else {
        return None;
    };
    let TokenKind::Literal(lit) = &tok.kind else {
        return None;
    };
    if !matches!(lit.kind, TokenLitKind::Str | TokenLitKind::StrRaw(_)) {
        return None;
    }
    Some(lit.symbol.as_str().to_string())
}

/// C04 signature validation: `fn(&mut self) -> IrqResult` for the plain form,
/// `fn(&mut self, index: usize) -> IrqResult` for array-bind. Returns whether
/// the handler is array-bound. The return type must be the
/// `#[rseq::framework_type("irq-result")]`-tagged enum — a `Result<…>` return
/// (or anything else) fails this check (C04: dispatch result, never
/// `Result<_, RegseqError>`).
fn validate_isr_signature(
    tcx: TyCtxt<'_>,
    site: &IsrSite,
    method_name: &str,
) -> Result<bool, BridgeError> {
    let err = |detail: &str| {
        Err(BridgeError::InterruptValidation(
            InterruptDiagnostic::IsrSignature {
                method: method_name.to_string(),
                detail: detail.to_string(),
            },
        ))
    };
    let sig = tcx.fn_sig(site.method).instantiate_identity().skip_binder();
    let inputs = sig.inputs();
    let Some(receiver) = inputs.first() else {
        return err("missing `&mut self` receiver");
    };
    let receiver_ok = matches!(
        receiver.kind(),
        TyKind::Ref(_, inner, rustc_middle::ty::Mutability::Mut)
            if inner.ty_adt_def().is_some_and(|adt| adt.did() == site.self_adt)
    );
    if !receiver_ok {
        return err("receiver must be `&mut self`");
    }
    let array_bind = match inputs.len() {
        1 => false,
        2 => {
            if !matches!(inputs[1].kind(), TyKind::Uint(rustc_middle::ty::UintTy::Usize)) {
                return err("second parameter must be `index: usize` (array-bind form)");
            }
            true
        }
        _ => return err("expected `(&mut self)` or `(&mut self, index: usize)`"),
    };
    let output = sig.output();
    let output_ok = output.ty_adt_def().is_some_and(|adt| {
        adt.did().is_local()
            && recognise_framework_type_attr(tcx, adt.did()) == Some(FrameworkType::IrqResult)
    });
    if !output_ok {
        return err(
            "return type must be the `irq-result` framework type (a dispatch result, \
             never `Result<_, RegseqError>`)",
        );
    }
    Ok(array_bind)
}

/// Post-drain finalisation: record each handler's [`AttrData::IsrBinding`]
/// onto its (now final) function Item and compute the `IrqResult`
/// arm-reachability facts from the materialized body.
pub(crate) fn finalise_isrs(
    cx: &mut BridgeCtxt<'_>,
    pending: Vec<PendingIsrAttr>,
) -> Result<(), BridgeError> {
    for attr in &pending {
        let Some(instance_id) = resolve_instance(&cx.model, attr.iref) else {
            continue;
        };
        let item = cx.model.instances[instance_id].item;
        let attrs = &mut cx.model.domain_attrs.item_attrs[item];
        if !attrs
            .iter()
            .any(|a| matches!(a, AttrData::IsrBinding { .. }))
        {
            attrs.push(AttrData::IsrBinding {
                field: attr.explicit_field,
                array_bind: attr.array_bind,
            });
        }
    }

    // Arm reachability per handler, from the drained body.
    let driver_keys: Vec<ItemId> = cx.model.interrupt.drivers.keys().copied().collect();
    for item_id in driver_keys {
        let handler_irefs: Vec<(usize, InstanceRef)> = cx.model.interrupt.drivers[&item_id]
            .handlers
            .iter()
            .enumerate()
            .filter_map(|(idx, h)| h.body.map(|iref| (idx, iref)))
            .collect();
        for (handler_idx, iref) in handler_irefs {
            let arms = resolve_instance(&cx.model, iref)
                .and_then(|id| match &cx.model.instances[id].kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .map(|body| compute_arm_reachability(&cx.model, body))
                .unwrap_or_default();
            if let Some(driver) = cx.model.interrupt.drivers.get_mut(&item_id) {
                driver.handlers[handler_idx].arms = arms;
            }
        }
    }
    Ok(())
}

/// Resolve a content-addressed `InstanceRef` to its `InstanceId` (linear scan,
/// mirroring the backend's `resolve_reloc`).
fn resolve_instance(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|idx| InstanceId::from_u32(idx as u32))
        .find(|&id| model.instance_ref_of(id) == target)
}

/// A small variant-set lattice for the reachability analysis: a bitmask over
/// `VariantIdx` (the `IrqResult` enum has 3 variants) plus a `top` flag for
/// "any value / not understood".
#[derive(Clone, Copy, Default, PartialEq)]
struct VariantSet {
    mask: u8,
    top: bool,
}

impl VariantSet {
    fn add_variant(&mut self, v: u32) {
        if v < 8 {
            self.mask |= 1 << v;
        } else {
            self.top = true;
        }
    }
    fn union(&mut self, other: VariantSet) -> bool {
        let before = (self.mask, self.top);
        self.mask |= other.mask;
        self.top |= other.top;
        before != (self.mask, self.top)
    }
    fn exactly(&self, v: u32) -> bool {
        !self.top && v < 8 && self.mask == (1 << v)
    }
    fn contains(&self, v: u32) -> bool {
        self.top || (v < 8 && self.mask & (1 << v) != 0)
    }
}

/// Compute the handler's `IrqResult` arm-reachability facts from its
/// materialized body.
///
/// Proof depth (design-intended; the contract floor is warn-on-unknown):
///
///   * **ProvenReachable** — a construction of the arm's variant flows into
///     the return place in a basic block that is CFG-reachable from entry and
///     CFG-reaches a `Return`, in the OPTIMIZED MIR (rustc has already pruned
///     constant-false branches, so this is post-dead-code liveness — the
///     distinction the R11 contract draws against syntactic presence).
///     Path-predicate satisfiability beyond CFG shape is out of scope.
///   * **ProvenUnreachable** — the return place's may-value set is bounded
///     (no unanalyzed source feeds it, transitively) and excludes the
///     variant: no execution can return that arm.
///   * **Unknown** — everything else. Consumers treat it conservatively
///     (R11 warns; the R09 `DeferWork` guard rejects).
fn compute_arm_reachability(model: &RegseqModel, body: &Body) -> IrqArmReachability {
    let unknown = IrqArmReachability::default();
    // The return local's type must be the IrqResult enum; map its variant
    // indices to arms by variant NAME (declaration order is authored, the
    // names are the contract).
    let ret_ty = body.local_decls[LocalId::RETURN_PLACE].ty;
    let Ty::Enum { layout, .. } = model.types.resolve(ret_ty) else {
        return unknown;
    };
    let LayoutKind::Enum { shape } = &layout.kind else {
        return unknown;
    };
    let variant_names: Vec<&str> = match shape {
        EnumShape::Empty => Vec::new(),
        EnumShape::Single { variant, .. } => vec![model.strings.resolve(variant.name)],
        EnumShape::Multiple { variants, .. } => variants
            .iter()
            .map(|v| model.strings.resolve(v.name))
            .collect(),
    };
    let arm_variant = |name: &str| -> Option<u32> {
        variant_names
            .iter()
            .position(|n| *n == name)
            .map(|i| i as u32)
    };
    let (Some(handled_v), Some(not_handled_v), Some(defer_v)) = (
        arm_variant("Handled"),
        arm_variant("NotHandled"),
        arm_variant("DeferWork"),
    ) else {
        // Not the three-arm IrqResult shape: leave everything Unknown.
        return unknown;
    };

    // Locals of the IrqResult type (by TyId identity — same interner).
    let is_irq_local = |local: LocalId| -> bool {
        local.index() < body.local_decls.len() && body.local_decls[local].ty == ret_ty
    };
    let bare_local = |place: &Place| -> Option<LocalId> {
        place.projection.is_empty().then_some(place.local)
    };

    // Per-local may-value sets + flow edges, then a fixpoint.
    let n_locals = body.local_decls.len();
    let mut sets: Vec<VariantSet> = vec![VariantSet::default(); n_locals];
    let mut edges: Vec<(usize, usize)> = Vec::new(); // (from, to)
    // Direct construction sites: (block, variant) writing the return place.
    let mut ret_constructions: Vec<(BlockId, RetSource)> = Vec::new();

    enum RetSource {
        Variant(u32),
        FromLocal(LocalId),
    }

    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        let block_id = BlockId::from_u32(block_idx as u32);
        for stmt in block.statements.iter() {
            match &stmt.kind {
                StatementKind::Assign(place, rvalue) => {
                    let Some(local) = bare_local(place) else {
                        // A projected write into an IrqResult local is not
                        // understood: taint the head local.
                        if is_irq_local(place.local) {
                            sets[place.local.index()].top = true;
                        }
                        continue;
                    };
                    if !is_irq_local(local) {
                        continue;
                    }
                    match rvalue {
                        Rvalue::Use(Operand::Constant(cid)) => {
                            match model.consts.resolve(*cid) {
                                Const::EnumValue { variant_idx, .. } => {
                                    let v = variant_idx.as_u32();
                                    sets[local.index()].add_variant(v);
                                    if local == LocalId::RETURN_PLACE {
                                        ret_constructions
                                            .push((block_id, RetSource::Variant(v)));
                                    }
                                }
                                _ => sets[local.index()].top = true,
                            }
                        }
                        // A fieldless variant construction survives as an
                        // `Aggregate` at the bridge's mir-opt level.
                        Rvalue::Aggregate(regseq_model::AggregateKind::Enum(_, variant_idx), _) => {
                            let v = variant_idx.as_u32();
                            sets[local.index()].add_variant(v);
                            if local == LocalId::RETURN_PLACE {
                                ret_constructions.push((block_id, RetSource::Variant(v)));
                            }
                        }
                        Rvalue::Use(Operand::Copy(src)) | Rvalue::Use(Operand::Move(src)) => {
                            match bare_local(src) {
                                Some(src_local) if is_irq_local(src_local) => {
                                    edges.push((src_local.index(), local.index()));
                                    if local == LocalId::RETURN_PLACE {
                                        ret_constructions
                                            .push((block_id, RetSource::FromLocal(src_local)));
                                    }
                                }
                                _ => sets[local.index()].top = true,
                            }
                        }
                        _ => sets[local.index()].top = true,
                    }
                }
                StatementKind::SetDiscriminant(place, variant_idx) => {
                    if let Some(local) = bare_local(place) {
                        if is_irq_local(local) {
                            let v = variant_idx.as_u32();
                            sets[local.index()].add_variant(v);
                            if local == LocalId::RETURN_PLACE {
                                ret_constructions.push((block_id, RetSource::Variant(v)));
                            }
                        }
                    } else if is_irq_local(place.local) {
                        sets[place.local.index()].top = true;
                    }
                }
                StatementKind::Intrinsic(_) => {}
            }
        }
        if let TerminatorKind::Call { destination, .. } = &block.terminator.kind {
            // A call writing an IrqResult local is an unanalyzed source.
            if is_irq_local(destination.local) {
                sets[destination.local.index()].top = true;
                if bare_local(destination) == Some(LocalId::RETURN_PLACE) {
                    // The return value comes from a call: nothing provable.
                    return unknown;
                }
            }
        }
    }

    // Fixpoint over the copy edges.
    loop {
        let mut changed = false;
        for &(from, to) in &edges {
            let src = sets[from];
            changed |= sets[to].union(src);
        }
        if !changed {
            break;
        }
    }

    // Block liveness: forward-reachable from entry AND reaching a Return.
    let n_blocks = body.basic_blocks.len();
    let mut fwd = vec![false; n_blocks];
    let mut stack = vec![BlockId::START_BLOCK];
    while let Some(b) = stack.pop() {
        if std::mem::replace(&mut fwd[b.index()], true) {
            continue;
        }
        for succ in body.basic_blocks[b].terminator.successors() {
            if !fwd[succ.index()] {
                stack.push(succ);
            }
        }
    }
    let mut reverse_edges: Vec<Vec<usize>> = vec![Vec::new(); n_blocks];
    let mut bwd = vec![false; n_blocks];
    let mut stack: Vec<usize> = Vec::new();
    for (idx, block) in body.basic_blocks.iter().enumerate() {
        for succ in block.terminator.successors() {
            reverse_edges[succ.index()].push(idx);
        }
        if matches!(block.terminator.kind, TerminatorKind::Return) {
            stack.push(idx);
        }
    }
    while let Some(b) = stack.pop() {
        if std::mem::replace(&mut bwd[b], true) {
            continue;
        }
        for &pred in &reverse_edges[b] {
            if !bwd[pred] {
                stack.push(pred);
            }
        }
    }
    let live = |b: BlockId| fwd[b.index()] && bwd[b.index()];

    // Per-arm verdicts.
    let ret_set = sets[LocalId::RETURN_PLACE.index()];
    let verdict = |v: u32| -> ArmReachability {
        let proven_reachable = ret_constructions.iter().any(|(block, source)| {
            live(*block)
                && match source {
                    RetSource::Variant(sv) => *sv == v,
                    RetSource::FromLocal(l) => sets[l.index()].exactly(v),
                }
        });
        if proven_reachable {
            ArmReachability::ProvenReachable
        } else if !ret_set.top && !ret_set.contains(v) {
            ArmReachability::ProvenUnreachable
        } else {
            ArmReachability::Unknown
        }
    };
    IrqArmReachability {
        handled: verdict(handled_v),
        not_handled: verdict(not_handled_v),
        defer_work: verdict(defer_v),
    }
}
