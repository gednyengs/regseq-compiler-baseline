//! Phase-5 `#[rseq::work_handler]` discovery, validation, binding, and
//! force-materialization (deferred-work-item-scheduling construct).
//!
//! Mirrors `bridge::interrupt::resolve` (`resolve_isrs`/`finalise_isrs`), but
//! strictly simpler: a deferred-work handler is `fn(&mut self) -> ()` (no
//! `IrqResult`, no array-bind `index` parameter), and a `Work` resource has
//! exactly one handler (GRILL D10) bound by the bare `#[rseq::work_handler]`
//! against the sole `Work` (the explicit `field = "…"` form + the ≥2-`Work`
//! ambiguity are design-in, NOT built).
//!
//! Two passes:
//!   * `resolve_work_handlers` (Phase 5, before the body drain) — discovery,
//!     signature validation, handler↔`Work` binding, the duplicate/zero/
//!     no-handler discipline, and force-materialization of the handler body
//!     `InstanceRef` (so the Phase-5 drain gives it a body the backend emits).
//!   * `finalise_work_handlers` (post-drain, after the late-item barriers) —
//!     records the `AttrData::WorkHandler { field: None }` marker onto the
//!     handler's now-final function `ItemId`.

use std::collections::HashMap;

use regseq_model::{AttrData, InstanceId, InstanceRef, ItemId, RegseqModel};
use rustc_hir::def::DefKind;
use rustc_middle::ty::{self, TyCtxt, TyKind};
use rustc_span::{def_id::DefId, Symbol as RustcSymbol};

use super::super::const_::instance_ref_for_function_provenance;
use super::super::framework_type::recover_source_def_id;
use super::super::{BridgeCtxt, BridgeError, DeferredWorkDiagnostic};

/// A pending `AttrData::WorkHandler` record, carried from `resolve_work_handlers`
/// (which knows the authored form) to [`finalise_work_handlers`] (which runs
/// after `ItemId`s are final and can record the marker onto the function Item).
pub(crate) struct PendingWorkHandlerAttr {
    /// The force-materialized handler instance.
    iref: InstanceRef,
}

/// One discovered `#[rseq::work_handler]` method, pre-validation.
struct WorkHandlerSite {
    method: DefId,
    self_adt: DefId,
}

/// Discover + validate + bind + force-materialize every `#[rseq::work_handler]`
/// handler. Returns the pending attribute records for [`finalise_work_handlers`].
pub(crate) fn resolve_work_handlers(
    cx: &mut BridgeCtxt<'_>,
) -> Result<Vec<PendingWorkHandlerAttr>, BridgeError> {
    let tcx = cx.input.tcx;

    // Driver-struct def-id -> model ItemId, for every recognized driver (a
    // driver with zero `Work` resources still participates: its bare handler is
    // the zero-`Work` error case).
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
    for site in work_handler_sites(tcx)? {
        let method_name = tcx.item_name(site.method).as_str().to_string();
        let Some(&driver_item) = driver_items.get(&site.self_adt) else {
            // A `#[rseq::work_handler]` on a type that is not a recognized
            // `#[rseq::driver]` struct: there is no `Work` to bind (a handler is
            // an inherent driver method).
            return Err(BridgeError::DeferredWorkValidation(
                DeferredWorkDiagnostic::BareHandlerZeroWork {
                    driver: tcx.item_name(site.self_adt).as_str().to_string(),
                    method: method_name,
                },
            ));
        };
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&driver_item]
                .descriptor
                .name;
            cx.model.strings.resolve(name_id).to_string()
        };

        // Signature validation: `fn(&mut self) -> ()`.
        validate_work_handler_signature(tcx, &site, &driver_name, &method_name)?;

        // Resource binding: the bare handler resolves to the sole `Work` (zero
        // `Work` -> error; the ≥2-`Work` ambiguity is design-in, not built, so
        // v1 binds the sole/first resource — the single-resource scope, GRILL
        // D01/D10).
        let resource_count = cx
            .model
            .deferred_work
            .drivers
            .get(&driver_item)
            .map(|d| d.resources.len())
            .unwrap_or(0);
        if resource_count == 0 {
            return Err(BridgeError::DeferredWorkValidation(
                DeferredWorkDiagnostic::BareHandlerZeroWork {
                    driver: driver_name,
                    method: method_name,
                },
            ));
        }
        let resource_idx = 0usize;

        // D10: exactly one handler per `Work` — a second binding is a duplicate.
        let already_bound = cx.model.deferred_work.drivers[&driver_item].resources[resource_idx]
            .handler
            .is_some();
        if already_bound {
            let field = {
                let name_id =
                    cx.model.deferred_work.drivers[&driver_item].resources[resource_idx].name;
                cx.model.strings.resolve(name_id).to_string()
            };
            return Err(BridgeError::DeferredWorkValidation(
                DeferredWorkDiagnostic::DuplicateHandler {
                    driver: driver_name,
                    field,
                },
            ));
        }

        // Force-materialize the handler (inherent, non-generic) so the Phase-5
        // drain gives it a body, exactly like the lifecycle callbacks / ISRs.
        let instance = ty::Instance::mono(tcx, site.method);
        let iref = instance_ref_for_function_provenance(cx, instance)?;

        if let Some(driver) = cx.model.deferred_work.drivers.get_mut(&driver_item) {
            driver.resources[resource_idx].handler = Some(iref);
        }
        pending.push(PendingWorkHandlerAttr { iref });
    }

    // Every required `Work` must have a bound handler (a `Work` you can never run
    // is broken — GRILL G5).
    let driver_keys: Vec<ItemId> = cx.model.deferred_work.drivers.keys().copied().collect();
    for item_id in driver_keys {
        let driver_name = {
            let name_id = cx.model.device_lifecycle.drivers[&item_id].descriptor.name;
            cx.model.strings.resolve(name_id).to_string()
        };
        let resources = &cx.model.deferred_work.drivers[&item_id].resources;
        for resource in resources {
            if resource.required && resource.handler.is_none() {
                let field = cx.model.strings.resolve(resource.name).to_string();
                return Err(BridgeError::DeferredWorkValidation(
                    DeferredWorkDiagnostic::RequiredWorkNoHandler {
                        driver: driver_name,
                        field,
                    },
                ));
            }
        }
    }
    Ok(pending)
}

/// Discover every `#[rseq::work_handler]` inherent driver method. Mirrors
/// `interrupt::resolve::isr_sites`: a handler must be an inherent method on a
/// struct (a free function / trait method is rejected as a signature error this
/// slice — the dedicated excluded-primitive diagnostic is design-in).
fn work_handler_sites(tcx: TyCtxt<'_>) -> Result<Vec<WorkHandlerSite>, BridgeError> {
    let path = [
        RustcSymbol::intern("rseq"),
        RustcSymbol::intern("work_handler"),
    ];
    let bad_placement = |def_id: DefId, detail: &str| -> BridgeError {
        BridgeError::DeferredWorkValidation(DeferredWorkDiagnostic::HandlerSignature {
            driver: "<none>".to_string(),
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
        let handler_attrs: Vec<_> = tcx.get_attrs_by_path(def_id, &path).collect();
        if handler_attrs.is_empty() {
            continue;
        }
        // SF-1: the bare `#[rseq::work_handler]` is the ONLY built form. EVERY
        // non-bare spelling — the list `#[rseq::work_handler(field = "…")]`, the
        // name-value `#[rseq::work_handler = "…"]` (string OR any other literal),
        // even an empty `()` — is the design-in multi-resource binding, NOT built
        // in v1. `Attribute::is_word()` is true only for the bare path form
        // (`AttrArgs::Empty`), so `!is_word()` rejects all of them uniformly.
        // Checked across ALL work_handler attrs (a later `field = …` must not be
        // hidden by an earlier bare one), rather than silently treating an
        // argument/value as bare — which would mis-bind a `field = "typo"` to the
        // sole `Work` with no error. Mirrors the UNSUPPORTED-FORM discipline (a
        // design-in FORM gets a clean v1 rejection); reuses HANDLER-SIGNATURE (the
        // attribute is part of the handler declaration), so it adds no new
        // `DWS-ERR-*` id — C06 stays locked (GRILL D17).
        if handler_attrs.iter().any(|attr| !attr.is_word()) {
            return Err(bad_placement(
                def_id,
                "carries an explicit attribute argument or value (the `field = \"…\"` form); that \
                 form is design-in, not built in v1 — use the bare `#[rseq::work_handler]`",
            ));
        }
        if matches!(tcx.def_kind(def_id), DefKind::Fn) {
            return Err(bad_placement(
                def_id,
                "a free function cannot be a work handler (a handler is an inherent \
                 driver method)",
            ));
        }
        let impl_def_id = tcx.parent(def_id);
        match tcx.def_kind(impl_def_id) {
            DefKind::Impl { of_trait: false } => {}
            _ => {
                return Err(bad_placement(
                    def_id,
                    "a work handler must be an inherent method on a #[rseq::driver] struct",
                ))
            }
        }
        let self_ty = tcx.type_of(impl_def_id).instantiate_identity();
        let Some(adt) = self_ty.ty_adt_def() else {
            return Err(bad_placement(
                def_id,
                "a work handler must be an inherent method on a #[rseq::driver] struct",
            ));
        };
        sites.push(WorkHandlerSite {
            method: def_id,
            self_adt: adt.did(),
        });
    }
    // `definitions()` order is deterministic per compilation; sort by def index
    // for belt-and-braces determinism of diagnostic order.
    sites.sort_by_key(|s| (s.method.krate.as_u32(), s.method.index.as_u32()));
    Ok(sites)
}

/// Validate the `fn(&mut self) -> ()` handler signature (GRILL G2/I01).
fn validate_work_handler_signature(
    tcx: TyCtxt<'_>,
    site: &WorkHandlerSite,
    driver_name: &str,
    method_name: &str,
) -> Result<(), BridgeError> {
    let err = |detail: &str| {
        Err(BridgeError::DeferredWorkValidation(
            DeferredWorkDiagnostic::HandlerSignature {
                driver: driver_name.to_string(),
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
    if inputs.len() != 1 {
        return err("expected exactly `(&mut self)` (no extra parameters)");
    }
    if !sig.output().is_unit() {
        return err("return type must be `()` (a deferred-work handler returns nothing)");
    }
    Ok(())
}

/// Post-drain finalisation: record each handler's [`AttrData::WorkHandler`]
/// marker onto its (now final) function Item. Mirrors `interrupt::finalise_isrs`
/// (minus the arm-reachability pass — deferred-work handlers have no
/// `IrqResult`). Only the bare `field: None` form is built.
pub(crate) fn finalise_work_handlers(
    cx: &mut BridgeCtxt<'_>,
    pending: Vec<PendingWorkHandlerAttr>,
) -> Result<(), BridgeError> {
    for attr in &pending {
        let Some(instance_id) = resolve_instance(&cx.model, attr.iref) else {
            continue;
        };
        let item = cx.model.instances[instance_id].item;
        let attrs = &mut cx.model.domain_attrs.item_attrs[item];
        if !attrs
            .iter()
            .any(|a| matches!(a, AttrData::WorkHandler { .. }))
        {
            attrs.push(AttrData::WorkHandler { field: None });
        }
    }
    Ok(())
}

/// Resolve a content-addressed `InstanceRef` to its `InstanceId` (linear scan,
/// mirroring `interrupt::resolve::resolve_instance` / the backend's
/// `resolve_reloc`).
fn resolve_instance(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|idx| InstanceId::from_u32(idx as u32))
        .find(|&id| model.instance_ref_of(id) == target)
}
