//! Driver-module-level topology summaries derived from a [`RegseqModel`].
//!
//! These helpers answer "what does this driver declare?" without
//! requiring a pre-computed side table. Each walker runs in time
//! proportional to the number of driver fields or impl blocks.

use std::collections::BTreeSet;

use crate::declaration::callbacks::CallbackFamily;
use crate::identity::{FunctionId, Symbol, TypeDefId};
use crate::model::driver::{DriverStruct, PlatformBusBinding};
use crate::model::module::{Module, ModuleKind};
use crate::model::regseq_model::RegseqModel;
use crate::model::resources::ResourceKind;

/// A callback method projection.
///
/// Points to a single method of a Regseq trait-impl block on the
/// driver, tagged with the callback family the impl participates in
/// and the driver type the impl targets. The `'a` lifetime borrows
/// from the underlying [`RegseqModel`]; the struct owns no data.
#[derive(Clone, Copy, Debug)]
pub struct CallbackImpl<'a> {
    /// Which callback family the impl's trait belongs to.
    pub family: CallbackFamily,
    /// The method's function id within the model.
    pub function_id: FunctionId,
    /// The driver type the impl targets.
    pub driver_type: TypeDefId,
    /// Placeholder marker for the borrow of `model`.
    ///
    /// Held so the struct tracks the model's lifetime even though it
    /// currently only exposes owned copies of its other fields. Kept
    /// as a private zero-sized marker to match the spec's "no owned
    /// strings; everything borrowed from model" direction -- future
    /// fields that borrow directly (method name, provenance) can drop
    /// in without changing the public type signature.
    _marker: std::marker::PhantomData<&'a RegseqModel>,
}

/// Return every resource field on the driver struct, paired with its
/// family tag.
///
/// Output is sorted by `(ResourceKind, field_name)` so two calls on
/// the same model return byte-identical vectors. Properties are
/// reported under [`ResourceKind::Property`] even though the model
/// stores them in a separate `properties` slot.
///
/// **Sync fields are intentionally omitted.** The canonical
/// [`ResourceKind`] taxonomy has no `Sync` variant; synchronization
/// primitives are tracked on [`DriverStruct::sync`] separately, and
/// exposing them here would require widening the enum and the
/// downstream lowering match arms that consume it. Callers that need
/// sync inventory should read `DriverStruct.sync` directly. This is
/// the single deliberate deviation from the legacy
/// `DeclarationModel::all_resource_field_names`, which reported sync
/// fields under a string tag rather than a typed variant.
///
/// Returns an empty vector when the model has no driver module (e.g.,
/// a pathological pre-verification state).
pub fn resource_field_names(model: &RegseqModel) -> Vec<(ResourceKind, &str)> {
    let Some(driver) = find_driver_struct(&model.root_module) else {
        return Vec::new();
    };
    let mut out: Vec<(ResourceKind, &str)> = Vec::new();

    for r in &driver.clocks {
        out.push((ResourceKind::Clock, model.symbols.resolve(r.field_name)));
    }
    for r in &driver.resets {
        out.push((ResourceKind::Reset, model.symbols.resolve(r.field_name)));
    }
    for r in &driver.pinctrl {
        out.push((ResourceKind::PinCtrl, model.symbols.resolve(r.field_name)));
    }
    for r in &driver.interrupts {
        out.push((ResourceKind::Interrupt, model.symbols.resolve(r.field_name)));
    }
    for r in &driver.mmio {
        out.push((ResourceKind::Mmio, model.symbols.resolve(r.field_name)));
    }
    for p in &driver.properties {
        out.push((ResourceKind::Property, model.symbols.resolve(p.field_name)));
    }

    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    out
}

/// Return the set of callback families actually implemented by the
/// driver module.
///
/// A family is "present" if the driver's trait-impl set contains at
/// least one impl whose trait name maps to that family (see
/// [`classify_trait_name`]). Impl blocks whose trait name does not
/// match any known Regseq callback trait are ignored.
pub fn callback_families_present(model: &RegseqModel) -> BTreeSet<CallbackFamily> {
    let mut families: BTreeSet<CallbackFamily> = BTreeSet::new();
    if let Some(driver_mod) = find_driver_module(&model.root_module) {
        for impl_block in &driver_mod.trait_impls {
            if let Some(family) = classify_trait_name(model, impl_block.trait_name) {
                families.insert(family);
            }
        }
    }
    families
}

/// Return every callback impl method defined on the driver.
///
/// One entry per method on every Regseq trait-impl block of the
/// driver module. Impl blocks whose trait name is not recognized as
/// a callback family are skipped.
///
/// Output is sorted by `(family, function_id)` so iteration is
/// deterministic.
pub fn callback_impls(model: &RegseqModel) -> Vec<CallbackImpl<'_>> {
    let mut out: Vec<CallbackImpl<'_>> = Vec::new();
    if let Some(driver_mod) = find_driver_module(&model.root_module) {
        for impl_block in &driver_mod.trait_impls {
            let Some(family) = classify_trait_name(model, impl_block.trait_name) else {
                continue;
            };
            for method in &impl_block.methods {
                out.push(CallbackImpl {
                    family,
                    function_id: method.id,
                    driver_type: impl_block.target_type,
                    _marker: std::marker::PhantomData,
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.family
            .cmp(&b.family)
            .then(a.function_id.cmp(&b.function_id))
    });
    out
}

/// Return the Layer-2 platform bus binding recorded on the driver.
///
/// Reads `DriverStruct.platform_metadata.bus`. Returns `None` if the
/// driver has no `[bus]` section in the platform file, if the model
/// has not yet undergone Phase-2 join, or if the model is missing a
/// driver struct entirely.
pub fn bus_binding(model: &RegseqModel) -> Option<&PlatformBusBinding> {
    find_driver_struct(&model.root_module)
        .and_then(|driver| driver.platform_metadata.as_ref())
        .and_then(|metadata| metadata.bus.as_ref())
}

/// Return the Layer-1 authored bus identity declared by the driver.
///
/// Reads `DriverStruct.identity.bus`. Returns `None` when the
/// driver's `#[rseq::driver(..)]` attribute omits the `bus = "..."`
/// key, or when the model has no driver module. The returned
/// [`Symbol`] resolves through `model.symbols` to the authored bus
/// family name (e.g., `"amba"`, `"pci"`, `"integrated"`).
///
/// Counterpart to the legacy `AnalysisInput::bus_identity`, which
/// returned a typed `BusType` enum. The new model stores the raw
/// symbol; typed validation happens in `verify_phase_1`.
pub fn bus_identity(model: &RegseqModel) -> Option<Symbol> {
    find_driver_struct(&model.root_module).and_then(|driver| driver.identity.bus)
}

// -- Internal helpers --------------------------------------------------

/// Return the driver submodule, if one exists.
fn find_driver_module(module: &Module) -> Option<&Module> {
    if module.kind == ModuleKind::Driver {
        return Some(module);
    }
    for submodule in &module.submodules {
        if let Some(found) = find_driver_module(submodule) {
            return Some(found);
        }
    }
    None
}

/// Return the driver struct inside the driver submodule, if both
/// exist.
fn find_driver_struct(module: &Module) -> Option<&DriverStruct> {
    find_driver_module(module).and_then(|m| m.driver.as_ref())
}

/// Map a Regseq trait name to its [`CallbackFamily`].
///
/// Unknown trait names -- including inherent impls whose
/// [`crate::model::impl_block::ImplBlock::trait_name`] is `None` --
/// return `None`. The mapping matches the legacy
/// `CallbackDeclSet` family tags.
fn classify_trait_name(
    model: &RegseqModel,
    trait_name: Option<crate::identity::Symbol>,
) -> Option<CallbackFamily> {
    let symbol = trait_name?;
    match model.symbols.resolve(symbol) {
        "Lifecycle" => Some(CallbackFamily::Lifecycle),
        "Serial" => Some(CallbackFamily::Serial),
        "SerialConsole" => Some(CallbackFamily::SerialConsole),
        "PowerManagement" => Some(CallbackFamily::PowerManagement),
        _ => None,
    }
}
