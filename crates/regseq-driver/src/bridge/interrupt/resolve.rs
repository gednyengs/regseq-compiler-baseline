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
    ArmReachability, AttrData, BlockId, Body, Const, EnumShape, FrameworkType, InstanceId,
    InstanceKind, InstanceRef, InterruptForm, IrqArmReachability, ItemId, LayoutKind, LocalId,
    Operand, Place, RegseqModel, Rvalue, StatementKind, StringId, TerminatorKind, Ty,
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

    // C06/I02: the Interrupt shell must stay methodless — a driver-callable
    // line-control reintroduction is a frontend error.
    check_line_control_reintroduction(tcx)?;

    let mut pending = Vec::new();
    for site in isr_sites(tcx)? {
        let Some(&driver_item) = driver_items.get(&site.self_adt) else {
            return Err(BridgeError::InterruptValidation(
                InterruptDiagnostic::ExcludedPrimitive {
                    method: tcx.item_name(site.method).as_str().to_string(),
                    detail: "bound to a type that is not a recognized #[rseq::driver] \
                             struct (an ISR is an inherent driver method)"
                        .to_string(),
                },
            ));
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

        // C06 form↔signature coherence: an array resource demands the
        // array-bind signature (IRQ-ERR-ARRAY-BIND-NO-INDEX); the index
        // parameter is only valid on an array binding (C04).
        match resources[resource_idx].1 {
            InterruptForm::Array { .. } if !array_bind => {
                return Err(BridgeError::InterruptValidation(
                    InterruptDiagnostic::ArrayBindNoIndex {
                        method: method_name,
                        resource: cx
                            .model
                            .strings
                            .resolve(resources[resource_idx].0)
                            .to_string(),
                    },
                ));
            }
            InterruptForm::Single | InterruptForm::Optional if array_bind => {
                return Err(BridgeError::InterruptValidation(
                    InterruptDiagnostic::IsrSignature {
                        method: method_name,
                        detail: "the `index: usize` parameter is only valid for an \
                                 `[Interrupt; N]` array binding (C04)"
                            .to_string(),
                    },
                ));
            }
            _ => {}
        }

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

/// Walk the crate for `#[rseq::isr]` sites: each valid site's method def-id,
/// its inherent impl's Self ADT, and the parsed attribute form. An ISR
/// anchored anywhere else — a trait method declaration, a trait impl
/// (provided-ISR is in the taxonomy's excluded set), or a free function — is
/// the `IRQ-ERR-EXCLUDED-PRIMITIVE` frontend error (C06; never silently
/// skipped, I05).
fn isr_sites(tcx: TyCtxt<'_>) -> Result<Vec<IsrSite>, BridgeError> {
    let path = [RustcSymbol::intern("rseq"), RustcSymbol::intern("isr")];
    let excluded = |def_id: DefId, detail: &str| -> BridgeError {
        BridgeError::InterruptValidation(InterruptDiagnostic::ExcludedPrimitive {
            method: tcx.item_name(def_id).as_str().to_string(),
            detail: detail.to_string(),
        })
    };
    let mut sites = Vec::new();
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let def_id = local_def_id.to_def_id();
        let is_fn_like = matches!(tcx.def_kind(def_id), DefKind::AssocFn | DefKind::Fn);
        if !is_fn_like {
            continue;
        }
        let Some(attr) = tcx.get_attrs_by_path(def_id, &path).next() else {
            continue;
        };
        if matches!(tcx.def_kind(def_id), DefKind::Fn) {
            return Err(excluded(
                def_id,
                "a free function cannot be an ISR (an ISR is an inherent driver method)",
            ));
        }
        let impl_def_id = tcx.parent(def_id);
        match tcx.def_kind(impl_def_id) {
            DefKind::Impl { of_trait: false } => {}
            DefKind::Impl { of_trait: true } => {
                return Err(excluded(
                    def_id,
                    "a trait-provided ISR binding is excluded (taxonomy excluded set); \
                     bind an inherent driver method",
                ))
            }
            DefKind::Trait => {
                return Err(excluded(
                    def_id,
                    "a trait method declaration cannot be an ISR (taxonomy excluded set)",
                ))
            }
            _ => {
                return Err(excluded(
                    def_id,
                    "an ISR must be an inherent method on a #[rseq::driver] struct",
                ))
            }
        }
        let self_ty = tcx.type_of(impl_def_id).instantiate_identity();
        let Some(adt) = self_ty.ty_adt_def() else {
            return Err(excluded(
                def_id,
                "an ISR must be an inherent method on a #[rseq::driver] struct",
            ));
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
    Ok(sites)
}

/// C06/I02: reject a driver-callable line-control reintroduction — ANY
/// method declared in an inherent impl on the `Interrupt` resource shell
/// (name-anchored, like every recognition key). The shell is methodless by
/// contract: register/enable/disable/deregister are compiler-managed
/// lifecycle placement, never author calls.
fn check_line_control_reintroduction(tcx: TyCtxt<'_>) -> Result<(), BridgeError> {
    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let impl_def_id = local_def_id.to_def_id();
        if !matches!(tcx.def_kind(impl_def_id), DefKind::Impl { of_trait: false }) {
            continue;
        }
        let self_ty = tcx.type_of(impl_def_id).instantiate_identity();
        let Some(adt) = self_ty.ty_adt_def() else {
            continue;
        };
        if tcx.item_name(adt.did()).as_str() != "Interrupt" {
            continue;
        }
        if let Some(method) = tcx
            .associated_items(impl_def_id)
            .in_definition_order()
            .find(|item| item.is_fn())
        {
            return Err(BridgeError::InterruptValidation(
                InterruptDiagnostic::DriverCallableLineControl {
                    method: method.name().as_str().to_string(),
                },
            ));
        }
    }
    Ok(())
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
            if !matches!(
                inputs[1].kind(),
                TyKind::Uint(rustc_middle::ty::UintTy::Usize)
            ) {
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
/// Proof depth (the contract floor is warn-on-unknown — the analysis may
/// over-warn, NEVER under-warn, so `ProvenReachable` demands an actual
/// proof, not CFG-path existence; Codex M1 review MUST-FIX):
///
///   * **ProvenReachable** — the return place's may-value set is bounded and
///     is EXACTLY this arm's variant, AND the live CFG provably terminates
///     via `Return` (acyclic, and every live terminator is total control
///     flow — no calls or asserts, either of which may diverge): every
///     execution returns this arm, with NO feasibility assumption anywhere.
///     A construction that is merely on a CFG path (whose branch predicates
///     might be runtime-infeasible) is NOT proof — a conditional arm stays
///     `Unknown`, so the R11 warning over-warns rather than ever being
///     wrongly suppressed (re-review hardening: CFG-live-Return alone was
///     not proof — the lone Return could sit behind an infeasible branch
///     with every feasible path diverging).
///   * **ProvenUnreachable** — the return place's may-value set is bounded
///     (no unanalyzed source feeds it, transitively) and excludes the
///     variant: no execution can return that arm. This direction is
///     load-bearing for R09 (a wrong `ProvenUnreachable` would admit a
///     `DeferWork`-returning handler), so the set is a may-over-approximation
///     (dead-block constructions still inflate it).
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
    let bare_local =
        |place: &Place| -> Option<LocalId> { place.projection.is_empty().then_some(place.local) };

    // Per-local may-value sets + flow edges, then a fixpoint.
    let n_locals = body.local_decls.len();
    let mut sets: Vec<VariantSet> = vec![VariantSet::default(); n_locals];
    let mut edges: Vec<(usize, usize)> = Vec::new(); // (from, to)

    for block in body.basic_blocks.iter() {
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
                        Rvalue::Use(Operand::Constant(cid)) => match model.consts.resolve(*cid) {
                            Const::EnumValue { variant_idx, .. } => {
                                sets[local.index()].add_variant(variant_idx.as_u32());
                            }
                            _ => sets[local.index()].top = true,
                        },
                        // A fieldless variant construction survives as an
                        // `Aggregate` at the bridge's mir-opt level.
                        Rvalue::Aggregate(regseq_model::AggregateKind::Enum(_, variant_idx), _) => {
                            sets[local.index()].add_variant(variant_idx.as_u32());
                        }
                        Rvalue::Use(Operand::Copy(src)) | Rvalue::Use(Operand::Move(src)) => {
                            match bare_local(src) {
                                Some(src_local) if is_irq_local(src_local) => {
                                    edges.push((src_local.index(), local.index()));
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
                            sets[local.index()].add_variant(variant_idx.as_u32());
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

    // The termination proof for ProvenReachable: over the CFG-live blocks,
    // (a) every terminator is TOTAL control flow — Goto/SwitchInt/Return
    // only (a Call may diverge in its callee; an Assert may abort;
    // Unreachable is UB) — and (b) the live subgraph is acyclic. Then every
    // execution walks a finite path of total steps and MUST end at a
    // `Return`: no feasibility assumption is involved.
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
    let total_flow = body.basic_blocks.iter().enumerate().all(|(idx, b)| {
        !fwd[idx]
            || matches!(
                b.terminator.kind,
                TerminatorKind::Goto(_) | TerminatorKind::SwitchInt { .. } | TerminatorKind::Return
            )
    });
    // Acyclicity of the live subgraph: Kahn peel — acyclic iff every live
    // node is peeled.
    let acyclic = {
        let mut indegree = vec![0usize; n_blocks];
        let mut live_count = 0usize;
        for (idx, b) in body.basic_blocks.iter().enumerate() {
            if !fwd[idx] {
                continue;
            }
            live_count += 1;
            for succ in b.terminator.successors() {
                if fwd[succ.index()] {
                    indegree[succ.index()] += 1;
                }
            }
        }
        let mut queue: Vec<usize> = (0..n_blocks)
            .filter(|&i| fwd[i] && indegree[i] == 0)
            .collect();
        let mut peeled = 0usize;
        while let Some(i) = queue.pop() {
            peeled += 1;
            for succ in body.basic_blocks[BlockId::from_u32(i as u32)]
                .terminator
                .successors()
            {
                let s = succ.index();
                if fwd[s] {
                    indegree[s] -= 1;
                    if indegree[s] == 0 {
                        queue.push(s);
                    }
                }
            }
        }
        peeled == live_count
    };
    let provably_returns = total_flow && acyclic;

    // Per-arm verdicts: ProvenReachable only on actual proof (the return
    // place can hold NOTHING but this arm and every execution provably
    // terminates via Return — so every execution returns it); conditional
    // arms stay Unknown so the R11 consumer over-warns, never under-warns.
    let ret_set = sets[LocalId::RETURN_PLACE.index()];
    let verdict = |v: u32| -> ArmReachability {
        if ret_set.exactly(v) && provably_returns {
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
