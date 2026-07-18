//! Pin-controller-and-GPIO-configuration post-bridge SEMANTIC checks (pinctrl
//! state-selection construct, M2/T09–T10). These are pure model-level body walks
//! — no `TyCtxt` — so they live in `regseq-model` and run from BOTH production
//! (`regseq-driver`'s `callbacks.rs`) and the rustc-free codegen-coverage-runner
//! (`bind_l2_platform`), after the L2 join (`validate_l2_surface`):
//!
//!   * `PIN-ERR-AUTHOR-MANAGED-FIELD` (C06): author code must not write the
//!     compiler-managed `PinCtrl`/`Option<PinCtrl>` field. A legitimate driver
//!     never constructs its own struct (the compiler synthesizes it) and never
//!     writes the field, so any author `Assign`/`SetDiscriminant` to the field, or
//!     whole-struct aggregate construction of the driver, is a violation.
//!
//!   * `PIN-ERR-UNRESOLVED-STATE-PARAM` (C06/C03): an `apply::<S>()` call whose
//!     `S` is not a `#[rseq::pin_state]` marker (the literal generic param is
//!     enforced-by-construction — the bridge rejects `TyKind::Param` — so the
//!     reachable shape is a *concrete* non-marker `S`).
//!
//!   * `PIN-ERR-INVALID-STATE-MARKER` (C06/C03): an `apply::<S>()` selecting a
//!     state name not in the receiver resource's joined inventory. Scoped to a
//!     RESOLVED binding (required / optional-present); an optional-absent resource
//!     has no inventory to validate against (C03 B1).
//!
//! State resolution reuses the shared [`crate::resolve_pin_state`] so the
//! membership check can never drift from the backend's `apply` lowering. Receiver
//! recovery mirrors the generic-c backend's `resolve_receiver_src` +
//! `recover_pinctrl`; a shape this pass cannot resolve (the M3 `if let Some` local
//! tracing) is SKIPPED, never mis-reported. The tcx-dependent recognition checks
//! (`NON-ZST-MARKER`, `DUPLICATE-STATE-MARKER`, `SPOOFED-RECEIVER`) stay in the
//! bridge (`regseq-driver`), where the attribute/signature info lives.

use crate::ids::{ItemId, StringId, TyId, VariantIdx};
use crate::{
    resolve_pin_state, AggregateKind, AttrData, Body, CallCallee, FieldIdx, FieldPath,
    InstanceKind, IntrinsicKind, PinctrlBinding, Place, PlaceElem, RegseqModel, Rvalue,
    StatementKind, SyntheticItemKind, TerminatorKind, Ty,
};

/// A pinctrl post-bridge semantic violation (the body-walk `PIN-ERR-*` checks
/// that need a materialised model and the L2 join). Distinct from the
/// `regseq-driver` `PinctrlDiagnostic` (the tcx-dependent recognition family) and
/// the `regseq-l2` `PinctrlL2Error` (the `PIN-L2-*` binding family); the
/// `PIN-ERR-*` code in [`Self::code`] / [`std::fmt::Display`] is the contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinctrlSemanticError {
    /// `PIN-ERR-AUTHOR-MANAGED-FIELD`: author code writes the compiler-managed
    /// pinctrl field.
    AuthorManagedField {
        /// The offending driver's logical name.
        driver: String,
        /// The offending field name.
        field: String,
        /// How the field was written (e.g. `direct field assignment`).
        detail: String,
    },
    /// `PIN-ERR-UNRESOLVED-STATE-PARAM`: an `apply::<S>()` call selects a state
    /// type that is not a `#[rseq::pin_state]` marker.
    UnresolvedStateParam {
        /// The owning driver's logical name.
        driver: String,
        /// Detail (e.g. no marker found on the resolved type).
        detail: String,
    },
    /// `PIN-ERR-INVALID-STATE-MARKER`: an `apply::<S>()` selects a state name not
    /// in the resource's joined `[pinctrl.<id>].pin_states` inventory.
    InvalidStateMarker {
        /// The owning driver's logical name.
        driver: String,
        /// The pinctrl resource id (`#[rseq::pinctrl(id = "…")]`).
        id: String,
        /// The selected (out-of-inventory) state name.
        state: String,
        /// The joined inventory, comma-separated.
        inventory: String,
    },
}

impl PinctrlSemanticError {
    /// The locked `PIN-ERR-*` diagnostic code (GRILL D11/C06).
    pub fn code(&self) -> &'static str {
        match self {
            PinctrlSemanticError::AuthorManagedField { .. } => "PIN-ERR-AUTHOR-MANAGED-FIELD",
            PinctrlSemanticError::UnresolvedStateParam { .. } => "PIN-ERR-UNRESOLVED-STATE-PARAM",
            PinctrlSemanticError::InvalidStateMarker { .. } => "PIN-ERR-INVALID-STATE-MARKER",
        }
    }
}

impl std::fmt::Display for PinctrlSemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinctrlSemanticError::AuthorManagedField {
                driver,
                field,
                detail,
            } => write!(
                f,
                "PIN-ERR-AUTHOR-MANAGED-FIELD: driver `{driver}` writes the compiler-managed pinctrl \
                 field `{field}` ({detail}); the pinctrl resource is acquired/released by the \
                 synthesized lifecycle — author code may only call `apply::<S>()` on it"
            ),
            PinctrlSemanticError::UnresolvedStateParam { driver, detail } => write!(
                f,
                "PIN-ERR-UNRESOLVED-STATE-PARAM: driver `{driver}`: an `apply::<S>()` call selects a \
                 state type that is not a `#[rseq::pin_state]` marker ({detail})"
            ),
            PinctrlSemanticError::InvalidStateMarker {
                driver,
                id,
                state,
                inventory,
            } => write!(
                f,
                "PIN-ERR-INVALID-STATE-MARKER: driver `{driver}`: `apply::<{state}>()` selects state \
                 `{state}` which is not in pinctrl `{id}`'s inventory [{inventory}]"
            ),
        }
    }
}

/// Run all pinctrl post-bridge semantic checks over the bound model: the
/// author-managed-field body walk, then the apply-site state checks
/// (unresolved-param + inventory membership). A pinctrl-less model is a no-op.
/// Invoked post-L2 (after `validate_l2_surface`) so the inventory the membership
/// check needs is joined.
pub fn validate_pinctrl_semantics(model: &RegseqModel) -> Result<(), PinctrlSemanticError> {
    if model.pinctrl.drivers.is_empty() {
        return Ok(());
    }
    validate_author_managed_fields(model)?;
    validate_apply_sites(model)?;
    Ok(())
}

/// `PIN-ERR-AUTHOR-MANAGED-FIELD`: reject author writes to a compiler-managed
/// pinctrl field. Walks author (non-synthetic) function bodies; rejects a direct
/// `Assign`/`SetDiscriminant` to a recognized pinctrl field, or a whole-struct
/// `Aggregate(Struct(driver))` construction of a driver that carries one.
fn validate_author_managed_fields(model: &RegseqModel) -> Result<(), PinctrlSemanticError> {
    for instance in model.instances.iter() {
        // Walk AUTHOR bodies only: source items (`None`) AND author closure call
        // bodies (`ClosureCall`, materialised from the author's closure MIR) — an
        // author can write `self.pins = …` inside a closure called from `on_init`.
        // Compiler-owned glue (drop glue, vtable shims, const memory) is exempt:
        // it never assigns these fields, and is not author code.
        if !matches!(
            model.item_synthetic_kinds[instance.item],
            SyntheticItemKind::None | SyntheticItemKind::ClosureCall { .. }
        ) {
            continue;
        }
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            for stmt in bb.statements.iter() {
                match &stmt.kind {
                    StatementKind::Assign(dst, rvalue) => {
                        if let Some((driver, field)) = place_writes_pinctrl_field(model, body, dst)
                        {
                            return Err(PinctrlSemanticError::AuthorManagedField {
                                driver,
                                field,
                                detail: "direct field assignment".to_string(),
                            });
                        }
                        if let Rvalue::Aggregate(AggregateKind::Struct(ty_id), _) = rvalue {
                            if let Some((driver, field)) =
                                aggregate_constructs_pinctrl_driver(model, *ty_id)
                            {
                                return Err(PinctrlSemanticError::AuthorManagedField {
                                    driver,
                                    field,
                                    detail: "whole-struct construction of the driver".to_string(),
                                });
                            }
                        }
                    }
                    StatementKind::SetDiscriminant(dst, _) => {
                        if let Some((driver, field)) = place_writes_pinctrl_field(model, body, dst)
                        {
                            return Err(PinctrlSemanticError::AuthorManagedField {
                                driver,
                                field,
                                detail: "discriminant write to the field's Option tag".to_string(),
                            });
                        }
                    }
                    StatementKind::Intrinsic(_) => {}
                }
            }
        }
    }
    Ok(())
}

/// `PIN-ERR-UNRESOLVED-STATE-PARAM` + `PIN-ERR-INVALID-STATE-MARKER`: every
/// `apply::<S>()` call site must select a real marker whose name is in the
/// receiver resource's joined inventory. Membership is scoped to a RESOLVED
/// binding (C03 B1).
fn validate_apply_sites(model: &RegseqModel) -> Result<(), PinctrlSemanticError> {
    for instance in model.instances.iter() {
        let InstanceKind::Function(func) = &instance.kind else {
            continue;
        };
        let Some(body) = func.body() else {
            continue;
        };
        for bb in body.basic_blocks.iter() {
            let TerminatorKind::Call { callee, args, .. } = &bb.terminator.kind else {
                continue;
            };
            let CallCallee::Direct(callee_inst) = callee else {
                continue;
            };
            if !matches!(
                model.instance_intrinsic(*callee_inst),
                Some(IntrinsicKind::Pinctrl(_))
            ) {
                continue;
            }
            let Some(recv) = args.first() else {
                continue;
            };
            // Receiver recovery is the SHARED `recover_pinctrl_receiver` (the same
            // one the generic-c backend's `apply` lowering uses), so this membership
            // pass and the backend can never drift on which resource a receiver
            // names — including the plain `if let Some(p) = &self.pinctrl` path,
            // whose state marker must still be inventory-validated here (no
            // split-brain). A shape it cannot resolve is skipped, never mis-reported.
            let Some((driver_item, pinctrl_id, binding)) =
                crate::recover_pinctrl_receiver(model, body, recv)
            else {
                continue;
            };
            let driver = driver_name(model, driver_item);
            match resolve_pin_state(model, &model.instances[*callee_inst].generic_args) {
                None => {
                    return Err(PinctrlSemanticError::UnresolvedStateParam {
                        driver,
                        detail: "the selected state type carries no `#[rseq::pin_state]` marker"
                            .to_string(),
                    });
                }
                Some(state_id) => {
                    // Membership only on a resolved binding (C03): an
                    // optional-absent resource has no inventory.
                    if binding.is_resolved() {
                        let state = model.strings.resolve(state_id).to_string();
                        let in_inventory = binding
                            .pin_states
                            .iter()
                            .any(|s| model.strings.resolve(*s) == state);
                        if !in_inventory {
                            let inventory = binding
                                .pin_states
                                .iter()
                                .map(|s| model.strings.resolve(*s))
                                .collect::<Vec<_>>()
                                .join(", ");
                            return Err(PinctrlSemanticError::InvalidStateMarker {
                                driver,
                                id: model.strings.resolve(pinctrl_id).to_string(),
                                state,
                                inventory,
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Walk a place's projection to a recognized `PinctrlResource` field and return
/// `(driver item, pinctrl id, binding)`. Mirrors the backend's `recover_pinctrl`
/// projection walk.
fn recover_pinctrl_at_place(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
) -> Option<(ItemId, StringId, PinctrlBinding)> {
    let fp = pinctrl_field_path(model, body, place)?;
    let pinctrl_id = model.domain_attrs.field_attrs.get(&fp).and_then(|attrs| {
        attrs.iter().find_map(|a| match a {
            AttrData::PinctrlResource { pinctrl_id, .. } => Some(*pinctrl_id),
            _ => None,
        })
    })?;
    let binding = model
        .pinctrl
        .drivers
        .get(&fp.item)
        .and_then(|d| d.resources.iter().find(|r| r.pinctrl_id == pinctrl_id))
        .map(|r| r.binding.clone())?;
    Some((fp.item, pinctrl_id, binding))
}

/// The `FieldPath` a place's projection lands on, IF it ends at a struct field;
/// `None` for an array index / downcast / bare-local place.
fn pinctrl_field_path(model: &RegseqModel, body: &Body, place: &Place) -> Option<FieldPath> {
    let mut ty = body.local_decls[place.local].ty;
    let mut field_path: Option<FieldPath> = None;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => return None,
                };
            }
            PlaceElem::Field(idx, field_ty) => {
                let item = match &model.types[ty] {
                    Ty::Struct { item, .. } => *item,
                    _ => return None,
                };
                field_path = Some(FieldPath {
                    item,
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(idx.index() as u32),
                });
                ty = *field_ty;
            }
            _ => return None,
        }
    }
    field_path
}

/// If `place`'s projection writes a recognized pinctrl field, return the
/// `(driver name, field name)` for the diagnostic.
fn place_writes_pinctrl_field(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
) -> Option<(String, String)> {
    let (item, pinctrl_id, _) = recover_pinctrl_at_place(model, body, place)?;
    Some((
        driver_name(model, item),
        field_name(model, item, pinctrl_id),
    ))
}

/// If `ty_id` is a driver struct that carries a pinctrl field, return its
/// `(driver name, field name)` (the whole-struct aggregate case).
fn aggregate_constructs_pinctrl_driver(
    model: &RegseqModel,
    ty_id: TyId,
) -> Option<(String, String)> {
    let Ty::Struct { item, .. } = &model.types[ty_id] else {
        return None;
    };
    let item = *item;
    let resource = model.pinctrl.drivers.get(&item)?.resources.first()?;
    Some((
        driver_name(model, item),
        model.strings.resolve(resource.name).to_string(),
    ))
}

/// The driver's logical name (its device-lifecycle descriptor name), or a
/// placeholder if the item is not a recognized driver.
fn driver_name(model: &RegseqModel, item: ItemId) -> String {
    model
        .device_lifecycle
        .drivers
        .get(&item)
        .map(|d| model.strings.resolve(d.descriptor.name).to_string())
        .unwrap_or_else(|| "<driver>".to_string())
}

/// The struct field name of the pinctrl resource with `pinctrl_id` on `item`,
/// falling back to the id if the resource cannot be located.
fn field_name(model: &RegseqModel, item: ItemId, pinctrl_id: StringId) -> String {
    model
        .pinctrl
        .drivers
        .get(&item)
        .and_then(|d| d.resources.iter().find(|r| r.pinctrl_id == pinctrl_id))
        .map(|r| model.strings.resolve(r.name).to_string())
        .unwrap_or_else(|| model.strings.resolve(pinctrl_id).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DriverPinctrls, PinctrlResource, StableCrateId};

    fn model_with_pinctrl_driver() -> RegseqModel {
        let mut model = RegseqModel::with_primary("pinctrl-sem-test", StableCrateId(1));
        let name = model.strings.intern("pins");
        let pinctrl_id = model.strings.intern("uart_pins");
        model.pinctrl.drivers.insert(
            ItemId::from_u32(0),
            DriverPinctrls {
                resources: vec![PinctrlResource {
                    name,
                    pinctrl_id,
                    required: true,
                    binding: Default::default(),
                }],
            },
        );
        model
    }

    #[test]
    fn empty_model_is_a_noop() {
        let model = RegseqModel::with_primary("empty", StableCrateId(1));
        assert!(validate_pinctrl_semantics(&model).is_ok());
    }

    #[test]
    fn pinctrl_driver_with_no_instances_is_a_noop() {
        // A recognized pinctrl driver but no materialised bodies (no apply sites,
        // no author writes) is clean — the pass walks instances and finds nothing.
        let model = model_with_pinctrl_driver();
        assert!(validate_pinctrl_semantics(&model).is_ok());
    }

    #[test]
    fn error_codes_are_the_locked_pin_err_set() {
        assert_eq!(
            PinctrlSemanticError::AuthorManagedField {
                driver: "d".into(),
                field: "pins".into(),
                detail: "x".into(),
            }
            .code(),
            "PIN-ERR-AUTHOR-MANAGED-FIELD"
        );
        assert_eq!(
            PinctrlSemanticError::UnresolvedStateParam {
                driver: "d".into(),
                detail: "x".into(),
            }
            .code(),
            "PIN-ERR-UNRESOLVED-STATE-PARAM"
        );
        assert_eq!(
            PinctrlSemanticError::InvalidStateMarker {
                driver: "d".into(),
                id: "uart_pins".into(),
                state: "idle".into(),
                inventory: "default, sleep".into(),
            }
            .code(),
            "PIN-ERR-INVALID-STATE-MARKER"
        );
    }
}
