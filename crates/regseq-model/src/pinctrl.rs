//! Pin-controller-and-GPIO-configuration construct IR (pinctrl state-selection).
//!
//! The author declares a typed `PinCtrl` (or `Option<PinCtrl>`) field on the
//! `#[rseq::driver]` struct via `#[rseq::pinctrl(id = "…")]` and calls
//! `apply::<S>()` on it to select a named pin state (`default`/`sleep`). This
//! module is the owned/serializable model of that surface: per-resource facts
//! (`pinctrl_id` join key, declaration form via `required`, field name) and the
//! L2-resolved `[pinctrl.<id>]` binding (the `pin_states` inventory). It is
//! populated by the `regseq-driver` bridge's `pinctrl` recognition, joined by
//! the `regseq-l2` `pinctrl` tenant, and consumed by the backends' pinctrl
//! lifecycle glue + per-op lowering.
//!
//! It is the `clock.rs` mirror (a dedicated top-level `RegseqModel` table, GRILL
//! D09): pin muxing is a *capability layered on the device* (an input it
//! consumes), not part of the `DriverHandle`'s address footprint, so it gets its
//! own table rather than living on `DriverHandle`. Lifecycle adjacency
//! (`acquire_pinctrl` after `map_region`, before `acquire_clock` —
//! pins-before-clock, GRILL D08/C07) is handled by phase-ordering across tables,
//! not by IR co-location.
//!
//! Unlike `clock.rs`, the binding carries no mode/provider — pinctrl is
//! *state-selection only*: the contents of a state are firmware/board-owned and
//! the binding records just the joined `pin_states` inventory. Presence
//! (`resolved`) is the load-bearing distinction: an optional `Option<PinCtrl>`
//! with no `[pinctrl.<id>]` section binds to `None` (`resolved = false`,
//! empty `pin_states`) and its `apply` compile-time-folds to `Err(PinctrlAbsent)`
//! (GRILL D06).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, ItemId, StringId, VariantIdx};
use crate::{
    AttrData, Body, GenericArg, Operand, Place, PlaceElem, RegseqModel, Rvalue, StatementKind, Ty,
};

/// Sibling table on `RegseqModel` for recognized pinctrl surfaces, keyed by the
/// `#[rseq::driver]` struct's final `ItemId` (assigned at Phase 4, like
/// `clock` / `interrupt`). Sparse via map absence. `#[serde(default)]` on the
/// model field keeps pre-construct models deserializable (GRILL I06/D09).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinctrlTables {
    /// Recognized per-driver pinctrl surfaces, keyed by the driver-struct Item.
    #[serde(with = "ordered_pinctrl_map")]
    pub drivers: BTreeMap<ItemId, DriverPinctrls>,
}

/// One driver's pinctrl surface: its resources in declaration order (the
/// `acquire_pinctrl` preamble ops run in declaration order, GRILL D08). Like
/// `DriverClocks`, there are no handlers — `apply` is an author-called intrinsic,
/// not a callback binding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverPinctrls {
    /// `#[rseq::pinctrl(id = "…")]` resource fields, declaration order.
    pub resources: Vec<PinctrlResource>,
}

/// One recognized `PinCtrl`/`Option<PinCtrl>` resource field on the driver
/// struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinctrlResource {
    /// The struct field name.
    pub name: StringId,
    /// The mandatory L1 resource id (`#[rseq::pinctrl(id = "…")]`): the join key
    /// to the platform file's `[pinctrl.<id>]` section and the generic-c trace
    /// identity. Always present — a missing/empty `id` is a frontend error
    /// (no field-name fallback, GRILL D07).
    pub pinctrl_id: StringId,
    /// The declaration form: `true` for the required `PinCtrl` form, `false` for
    /// the optional `Option<PinCtrl>` form (GRILL D02/D03).
    pub required: bool,
    /// The L2-resolved `[pinctrl.<id>]` binding. Empty (`PinctrlBinding::default`)
    /// until the platform-file join; the `pinctrl` L2 tenant interns the inventory
    /// and fills it (M1), and validates its CONTENT — `"default"` membership +
    /// uniqueness/non-empty — at bind time in M2 (`PIN-L2-*`, T11).
    /// `#[serde(default)]` keeps earlier serialized models deserializable.
    #[serde(default)]
    pub binding: PinctrlBinding,
}

/// The L2-resolved `[pinctrl.<id>]` facts (contract C02). The tenant interns the
/// `pin_states` inventory and sets `resolved`; the `"default"`-membership
/// invariant (plus the rest of the CONTENT set) is enforced at bind time in M2
/// (`PIN-L2-NO-DEFAULT`, T11) — not yet in M1. Full validity is given by
/// [`PinctrlBinding::is_resolved`].
///
/// State-selection only: there is no mode/provider/frequency — the contents of a
/// pin state are firmware/board-owned, applied by the OS pin controller; the
/// binding records only the *names* the driver may select.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinctrlBinding {
    /// The joined pin-state-name inventory (interned), in platform-file order.
    /// Non-empty on every resolved binding; the `"default"`-membership invariant
    /// (GRILL D07) is enforced at bind time by the L2 tenant
    /// (`PIN-L2-NO-DEFAULT`, T11), so a resolved binding always contains
    /// `"default"`. Empty/ignored when `resolved == false` (optional-absent —
    /// GRILL D06).
    pub pin_states: Vec<StringId>,
    /// Whether the `[pinctrl.<id>]` section was present and joined. A missing
    /// section on a required `PinCtrl` is a binding error
    /// (`PIN-L2-NO-INVENTORY`, GRILL D03/C06); on an optional `Option<PinCtrl>`
    /// it is the allowed absent case (binds to `None`).
    pub resolved: bool,
}

impl PinctrlBinding {
    /// Full binding validity (not merely "the section was joined"): the section
    /// must have been joined (`resolved`) AND carry a non-empty `pin_states`
    /// inventory. The `"default"` membership invariant (a resolved binding
    /// contains `"default"`) is enforced at bind time by the L2 tenant
    /// (`PIN-L2-NO-DEFAULT`, T11), so a `resolved` binding is guaranteed to
    /// contain `"default"`; this predicate need only check non-emptiness. It is
    /// the single source of truth that the
    /// tenant's `bind`/`validate` (regseq-l2) and the backend emit gate
    /// (regseq-backends) agree with (GRILL C02): `acquire_pinctrl`/`apply`/
    /// `release` emit only for an `is_resolved()` binding. An optional-absent
    /// resource (`resolved = false`, empty `pin_states`) reads `!is_resolved()`
    /// and emits nothing — its `apply` compile-time-folds to `Err(PinctrlAbsent)`
    /// (D06). A required `PinCtrl` is NEVER `resolved = false` (a missing
    /// inventory is `PIN-L2-NO-INVENTORY`, D03/C06).
    pub fn is_resolved(&self) -> bool {
        self.resolved && !self.pin_states.is_empty()
        // The "default" membership invariant is enforced at bind time by the L2
        // tenant in M2 (PIN-L2-NO-DEFAULT, T11) — not yet checked here.
    }
}

/// Resolve the pin-state NAME selected by an `apply::<S>()` call from its
/// monomorphized `generic_args` (pinctrl construct, GRILL D04/C03). The concrete
/// marker ADT `S` appears among the args as a `Ty::Struct { item, .. }` whose
/// `item` carries the [`AttrData::PinStateMarker`] recorded by recognition; the
/// interned `id` is the resolved state name. There is exactly one marker among
/// the args (only a `#[rseq::pin_state]` marker carries that attr), so the first
/// match is the selected state.
///
/// Returns `None` when no arg resolves to a marker — a concrete `S` that is not a
/// `#[rseq::pin_state]` marker (e.g. a type with a hand-rolled `impl PinState`).
/// That is the *reachable* `PIN-ERR-UNRESOLVED-STATE-PARAM` case: the literal
/// generic param `S` is enforced-by-construction (the bridge rejects
/// `TyKind::Param`, so a post-monomorphization call always carries a concrete
/// `S`), and the residual reachable shape is "concrete `S`, no marker".
///
/// Shared by the generic-c backend's `apply` lowering (`resolve_apply_state`) and
/// the post-L2 `validate_apply_sites` membership pass so the two can never drift
/// in how they read the selected state (the single source of truth, GRILL C03).
pub fn resolve_pin_state(model: &RegseqModel, generic_args: &[GenericArg]) -> Option<StringId> {
    for ga in generic_args {
        let GenericArg::Type(ty_id) = ga else {
            continue;
        };
        let Ty::Struct { item, .. } = &model.types[*ty_id] else {
            continue;
        };
        let item = *item;
        if item.index() >= model.domain_attrs.item_attrs.len() {
            continue;
        }
        if let Some(id) = model.domain_attrs.item_attrs[item]
            .iter()
            .find_map(|a| match a {
                AttrData::PinStateMarker { id } => Some(*id),
                _ => None,
            })
        {
            return Some(id);
        }
    }
    None
}

/// Recover the `(driver item, pinctrl id, binding)` an `apply::<S>()` call's
/// receiver operand ultimately targets — the SINGLE source of truth shared by the
/// generic-c backend's `apply` lowering (`recover_pinctrl`) and the post-L2
/// `validate_apply_sites` membership pass, so the two can never drift in which
/// resource a receiver names (GRILL C03; the [`resolve_pin_state`] precedent).
///
/// Handles BOTH optional call paths (D02):
///   * the `OptionalPinCtrl`/`PinCtrl` direct call `(&)self.pinctrl.apply::<S>()`
///     — receiver `&self.pinctrl` resolves to `self.[Deref, Field(pinctrl)]`;
///   * the plain `if let Some(p) = &self.pinctrl { p.apply::<S>() }` local-tracing
///     path (D13/T15) — the unwrapped `p` chains back through the match binding
///     (`p = &(*_r as Some).0`, `_r = &self.pinctrl`), which this composes to
///     `self.[Deref, Field(pinctrl), Downcast(Some), Field(0)]`.
///
/// The projection walk returns at the FIRST field carrying an
/// [`AttrData::PinctrlResource`] marker (the driver-struct-level pinctrl field),
/// so the if-let path's trailing `Downcast(Some)/Field(0)` into the inner
/// `PinCtrl` is never reached — the recovered field is always the driver field
/// where the marker + binding live. Returns `None` for any shape it cannot resolve
/// (an ambiguous/missing reaching-def, a non-pinctrl receiver, a downcast/index
/// before the pinctrl field); callers either skip (the membership pass) or fail
/// loud (the backend).
pub fn recover_pinctrl_receiver(
    model: &RegseqModel,
    body: &Body,
    recv: &Operand,
) -> Option<(ItemId, StringId, PinctrlBinding)> {
    let place = canonical_receiver_place(model, body, recv, 0)?;
    walk_to_pinctrl_field(model, body, &place)
}

/// Resolve a receiver operand to the canonical place it names — rooted at a
/// non-borrow-temp local (`self` / a user local) with the full projection,
/// composing away every hoisted borrow temp (`_t = &place`) in the chain.
fn canonical_receiver_place(
    model: &RegseqModel,
    body: &Body,
    op: &Operand,
    depth: u32,
) -> Option<Place> {
    if depth > 16 {
        return None;
    }
    let place = match op {
        Operand::Copy(p) | Operand::Move(p) => p.clone(),
        Operand::Constant(_) => return None,
    };
    compose_place(model, body, place, depth)
}

/// Compose a place against the reaching definitions of its base local, folding a
/// chained borrow (`_t = &base; (*_t).rest` ⇒ `base.rest`, the leading `Deref`
/// cancelling the `&`) until the base local is a real local (no unique borrow
/// def). A bare local (no projection) is chased through its `Ref`/`RawPtr`/`Use`
/// definition.
fn compose_place(model: &RegseqModel, body: &Body, place: Place, depth: u32) -> Option<Place> {
    if depth > 16 {
        return None;
    }
    // Bare local: chase its unique definition (a hoisted borrow or a move).
    if place.projection.is_empty() {
        return match unique_assign(body, place.local)? {
            Rvalue::Ref(_, src) | Rvalue::RawPtr(_, src) => {
                compose_place(model, body, src.clone(), depth + 1)
            }
            Rvalue::Use(inner) => canonical_receiver_place(model, body, inner, depth + 1),
            _ => None,
        };
    }
    // Projected place whose base local is itself a borrow temp `_t = &base`: splice
    // `base.projection ++ place.projection[1..]` (drop the leading `Deref` that
    // cancels the `&`).
    if matches!(place.projection.first(), Some(PlaceElem::Deref)) {
        if let Some(Rvalue::Ref(_, base) | Rvalue::RawPtr(_, base)) =
            unique_assign(body, place.local)
        {
            let mut projection: Vec<PlaceElem> = base.projection.to_vec();
            projection.extend_from_slice(&place.projection[1..]);
            let spliced = Place {
                local: base.local,
                projection: projection.into_boxed_slice(),
            };
            return compose_place(model, body, spliced, depth + 1);
        }
    }
    Some(place)
}

/// The unique whole-local definition rvalue of `local` in `body` (the hoisted
/// borrow / move temp pattern), or `None` if there is no definition or more than
/// one (ambiguous).
fn unique_assign(body: &Body, local: crate::ids::LocalId) -> Option<&Rvalue> {
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
                return None; // ambiguous — bail rather than guess
            }
            def = Some(rhs);
        }
    }
    def
}

/// Walk a canonical place's projection to the FIRST field bearing an
/// [`AttrData::PinctrlResource`] marker, returning that driver field's
/// `(item, pinctrl id, binding)`. Stops at the marker field, so an if-let path's
/// trailing `Downcast(Some)/Field(0)` (into the inner `PinCtrl`) is never reached.
fn walk_to_pinctrl_field(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
) -> Option<(ItemId, StringId, PinctrlBinding)> {
    let mut ty = body.local_decls[place.local].ty;
    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => {
                ty = match &model.types[ty] {
                    Ty::Ref(p, _) | Ty::Ptr(p, _) => *p,
                    _ => return None,
                };
            }
            PlaceElem::Field(idx, field_ty) => {
                let Ty::Struct { item, .. } = &model.types[ty] else {
                    return None;
                };
                let item = *item;
                let fp = crate::FieldPath {
                    item,
                    variant: VariantIdx::ZERO,
                    field: FieldIdx::from_u32(idx.index() as u32),
                };
                if let Some((pinctrl_id, binding)) = pinctrl_marker_and_binding(model, &fp) {
                    return Some((item, pinctrl_id, binding));
                }
                ty = *field_ty;
            }
            // A downcast / index BEFORE the pinctrl field means the place does not
            // name a driver-level pinctrl resource field — unresolvable here.
            _ => return None,
        }
    }
    None
}

/// The `(pinctrl id, binding)` recorded for a driver field path, if it carries an
/// [`AttrData::PinctrlResource`] marker AND a matching resource record.
fn pinctrl_marker_and_binding(
    model: &RegseqModel,
    fp: &crate::FieldPath,
) -> Option<(StringId, PinctrlBinding)> {
    let pinctrl_id = model
        .domain_attrs
        .field_attrs
        .get(fp)?
        .iter()
        .find_map(|a| match a {
            AttrData::PinctrlResource { pinctrl_id, .. } => Some(*pinctrl_id),
            _ => None,
        })?;
    let binding = model
        .pinctrl
        .drivers
        .get(&fp.item)?
        .resources
        .iter()
        .find(|r| r.pinctrl_id == pinctrl_id)
        .map(|r| r.binding.clone())?;
    Some((pinctrl_id, binding))
}

/// Serde adapter: serialize the `ItemId`-keyed driver map as a JSON array of
/// `(key, value)` pairs (mirrors `clock::ordered_clock_map` /
/// `interrupt::ordered_interrupt_map`). `BTreeMap` iteration is key-sorted, so
/// the output is deterministic across runs regardless of insertion order.
mod ordered_pinctrl_map {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        map: &BTreeMap<ItemId, DriverPinctrls>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<(&ItemId, &DriverPinctrls)> = map.iter().collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<ItemId, DriverPinctrls>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(ItemId, DriverPinctrls)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    fn sid(n: u32) -> StringId {
        StringId::from_u32(n)
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, value);
    }

    fn sample_tables() -> PinctrlTables {
        let mut tables = PinctrlTables::default();
        tables.drivers.insert(
            ItemId::from_u32(3),
            DriverPinctrls {
                resources: vec![
                    // Optional, present (resolved) — the PL011 witness shape.
                    PinctrlResource {
                        name: sid(1),
                        pinctrl_id: sid(2),
                        required: false,
                        binding: PinctrlBinding {
                            pin_states: vec![sid(10), sid(11)],
                            resolved: true,
                        },
                    },
                    // Required, present (resolved).
                    PinctrlResource {
                        name: sid(5),
                        pinctrl_id: sid(6),
                        required: true,
                        binding: PinctrlBinding {
                            pin_states: vec![sid(10)],
                            resolved: true,
                        },
                    },
                    // Optional, absent (unresolved — binds to None).
                    PinctrlResource {
                        name: sid(7),
                        pinctrl_id: sid(8),
                        required: false,
                        binding: PinctrlBinding::default(),
                    },
                ],
            },
        );
        tables
    }

    #[test]
    fn pinctrl_tables_round_trip() {
        round_trip(&sample_tables());
    }

    #[test]
    fn pinctrl_map_json_is_deterministic_across_insert_order() {
        let mut a = PinctrlTables::default();
        let mut b = PinctrlTables::default();
        let mk = |n: u32| DriverPinctrls {
            resources: vec![PinctrlResource {
                name: sid(n),
                pinctrl_id: sid(n + 1),
                required: false,
                binding: PinctrlBinding::default(),
            }],
        };
        a.drivers.insert(ItemId::from_u32(1), mk(1));
        a.drivers.insert(ItemId::from_u32(2), mk(2));
        b.drivers.insert(ItemId::from_u32(2), mk(2));
        b.drivers.insert(ItemId::from_u32(1), mk(1));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn binding_defaults_are_unresolved() {
        let b = PinctrlBinding::default();
        assert!(b.pin_states.is_empty());
        assert!(!b.resolved);
        // Default binding is not yet a valid, fully-resolved binding.
        assert!(!b.is_resolved());
    }

    #[test]
    fn is_resolved_truth_table() {
        // resolved=false ⇒ never valid (optional-absent), regardless of states.
        assert!(!PinctrlBinding {
            pin_states: vec![],
            resolved: false,
        }
        .is_resolved());
        assert!(!PinctrlBinding {
            pin_states: vec![sid(10)],
            resolved: false,
        }
        .is_resolved());

        // resolved=true but empty pin_states ⇒ invalid (the L2 tenant rejects an
        // empty inventory as PIN-L2-EMPTY-STATES, so it must read
        // `!is_resolved()`).
        assert!(!PinctrlBinding {
            pin_states: vec![],
            resolved: true,
        }
        .is_resolved());

        // resolved=true with a non-empty inventory ⇒ valid. (The "default"
        // membership invariant is enforced at bind time in M2 (PIN-L2-NO-DEFAULT,
        // T11), so a later resolved binding contains it; here the interned ids
        // stand in for the inventory.)
        assert!(PinctrlBinding {
            pin_states: vec![sid(10)],
            resolved: true,
        }
        .is_resolved());
        assert!(PinctrlBinding {
            pin_states: vec![sid(10), sid(11)],
            resolved: true,
        }
        .is_resolved());
    }

    #[test]
    fn required_resource_round_trips() {
        // The build-wide required `PinCtrl` form (GRILL D03) round-trips with
        // `required = true` distinct from the optional form.
        let req = PinctrlResource {
            name: sid(1),
            pinctrl_id: sid(2),
            required: true,
            binding: PinctrlBinding {
                pin_states: vec![sid(10)],
                resolved: true,
            },
        };
        let opt = PinctrlResource {
            name: sid(1),
            pinctrl_id: sid(2),
            required: false,
            binding: PinctrlBinding {
                pin_states: vec![sid(10)],
                resolved: true,
            },
        };
        round_trip(&req);
        round_trip(&opt);
        assert_ne!(req, opt);
    }
}
