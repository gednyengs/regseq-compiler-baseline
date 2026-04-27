//! Transactional mutation API for [`RegseqModel`].
//!
//! Every in-place transformation of a [`RegseqModel`] routes through a
//! [`MutationGuard`] which runs the appropriate phase-level verifier on
//! [`MutationGuard::commit`]. There is no path through this guarded
//! surface that bypasses verification. Accidental corruption at the API
//! surface is impossible: the only escape hatch is
//! [`RegseqModel::from_verified_components_unchecked`], which is
//! `#[doc(hidden)]` and must be followed by an explicit verifier call.
//!
//! # Semantics
//!
//! - [`RegseqModel::begin_mutation`] takes a [`MutationPhase`] (the
//!   caller's declared post-mutation phase), clones the current model
//!   as a rollback snapshot, and returns a [`MutationGuard`].
//! - Mutations happen **in place** on the live model. The
//!   `modify_*` methods hand out `&mut` access to the targeted field.
//! - [`MutationGuard::commit`] runs the phase-level verifier
//!   ([`verify_phase_1`] or [`verify_phase_2`]) against the mutated
//!   model. On `Ok`, the snapshot is dropped and the mutation sticks.
//!   On `Err`, the model is rolled back to the snapshot and the error
//!   is returned.
//! - Dropping the guard **without** calling [`MutationGuard::commit`]
//!   rolls the model back to the snapshot. This gives callers a
//!   panic-safe way to abandon a partial mutation: either the commit
//!   succeeds and the model is verified, or the model is restored to
//!   its pre-mutation state.
//!
//! # Cost model
//!
//! `begin_mutation` clones the entire [`RegseqModel`]. The cost is
//! acceptable at Phase 6 because joins happen once per driver, not in
//! hot loops. An optimization pass that snapshots only the affected
//! subtree is deliberate future work, not something to attempt here --
//! the goal of this task is the verification contract, not the
//! allocation profile.
//!
//! [`RegseqModel`]: crate::model::RegseqModel
//! [`verify_phase_1`]: crate::verify::verify_phase_1
//! [`verify_phase_2`]: crate::verify::verify_phase_2

// Task 62 changes `MutationGuard::commit` to return
// `Result<VerifyReport, VerifierError>` so non-fatal lint findings flow
// through alongside structural rollback. `VerifierError` remains a
// first-class value to match the rest of the verifier surface --
// callers match on `.kind` and `.level`. Boxing here would force every
// commit site to dereference first. See the matching allow in
// `crate::verify`.
#![allow(clippy::result_large_err)]

use crate::identity::{DeclId, FunctionId, ModuleId};
use crate::ir::{Body, LiteralValue};
use crate::model::driver::DriverStruct;
use crate::model::function::Function;
use crate::model::module::{Module, ModuleKind};
use crate::model::regseq_model::RegseqModel;
use crate::model::resources::{
    ClockBinding, InterruptBinding, MmioBinding, PinCtrlBinding, ResetBinding,
};
use crate::verify::{verify_phase_1, verify_phase_2, VerifierError, VerifyReport};

/// Which phase-level verifier runs on [`MutationGuard::commit`].
///
/// This enum records the phase whose invariants the caller claims the
/// post-mutation model must satisfy. It is distinct from
/// [`crate::verify::VerifierLevel`] on purpose: `VerifierLevel` names a
/// rule bucket ("module-level", "body-level", ...), while
/// `MutationPhase` names a phase of the pipeline whose full invariant
/// set applies.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MutationPhase {
    /// Post-mutation model must satisfy [`verify_phase_1`].
    ///
    /// [`verify_phase_1`]: crate::verify::verify_phase_1
    Phase1,
    /// Post-mutation model must satisfy [`verify_phase_2`].
    ///
    /// [`verify_phase_2`]: crate::verify::verify_phase_2
    Phase2,
}

/// Platform binding payload for a single resource slot.
///
/// [`MutationGuard::insert_binding`] accepts one of these and routes it
/// to the matching resource (`clocks`, `resets`, `pinctrl`,
/// `interrupts`, `mmio`) identified by its [`DeclId`]. Synchronization
/// resources carry no binding data and are not represented here.
#[derive(Clone, Debug)]
pub enum ResourceBinding {
    /// Clock binding payload.
    Clock(ClockBinding),
    /// Reset binding payload.
    Reset(ResetBinding),
    /// PinCtrl binding payload.
    PinCtrl(PinCtrlBinding),
    /// Interrupt binding payload.
    Interrupt(InterruptBinding),
    /// MMIO binding payload.
    Mmio(MmioBinding),
}

/// Transactional mutation guard for a [`RegseqModel`].
///
/// Constructed by [`RegseqModel::begin_mutation`]. Mutations happen in
/// place on the referenced model; [`Self::commit`] verifies the
/// mutated model and commits or rolls back. Dropping the guard without
/// calling `commit` rolls back as well, so a panic or early-return
/// between `begin_mutation` and `commit` cannot leave the model in an
/// unverified state.
///
/// See the module-level rustdoc for the verification contract.
#[must_use = "MutationGuard must be committed or dropped to apply/discard mutations"]
pub struct MutationGuard<'m> {
    /// The live model being mutated.
    model: &'m mut RegseqModel,
    /// Pre-mutation snapshot. `Some` until `commit` succeeds.
    snapshot: Option<RegseqModel>,
    /// Which phase verifier runs on commit.
    phase: MutationPhase,
}

impl<'m> MutationGuard<'m> {
    /// Create a guard. Private -- use [`RegseqModel::begin_mutation`].
    pub(crate) fn new(model: &'m mut RegseqModel, phase: MutationPhase) -> Self {
        let snapshot = model.clone();
        Self {
            model,
            snapshot: Some(snapshot),
            phase,
        }
    }

    /// Which phase verifier will run on [`Self::commit`].
    pub fn phase(&self) -> MutationPhase {
        self.phase
    }

    /// Apply `f` to the module identified by `id`, searching the
    /// full module tree (including the root).
    ///
    /// If no module matches, `f` is not invoked. The absence of a
    /// match does not fail the guard directly; it surfaces as a
    /// downstream verifier error on [`Self::commit`] only if the
    /// model is actually invalid. Callers that need an error on a
    /// missing id should check themselves before invoking.
    pub fn modify_module<F>(&mut self, id: ModuleId, f: F)
    where
        F: FnOnce(&mut Module),
    {
        if let Some(m) = find_module_mut(&mut self.model.root_module, id) {
            f(m);
        }
    }

    /// Apply `f` to the sole [`DriverStruct`] in the model.
    ///
    /// A valid [`RegseqModel`] has exactly one driver module
    /// (enforced by [`verify_phase_1`]), so no disambiguating id is
    /// needed. In a transient invalid state with zero or multiple
    /// driver modules, this method operates on the first
    /// [`ModuleKind::Driver`] module encountered in document order;
    /// a subsequent [`Self::commit`] will surface the structural
    /// violation through the module-level verifier bucket.
    ///
    /// [`verify_phase_1`]: crate::verify::verify_phase_1
    pub fn modify_driver_struct<F>(&mut self, f: F)
    where
        F: FnOnce(&mut DriverStruct),
    {
        if let Some(driver) = find_driver_mut(&mut self.model.root_module) {
            f(driver);
        }
    }

    /// Apply `f` to the body of the function identified by `id`.
    ///
    /// Searches free functions, driver impl block methods, and trait
    /// impl methods in document order. If no function matches, `f`
    /// is not invoked.
    pub fn modify_body<F>(&mut self, id: FunctionId, f: F)
    where
        F: FnOnce(&mut Body),
    {
        if let Some(func) = find_function_mut(&mut self.model.root_module, id) {
            f(&mut func.body);
        }
    }

    /// Populate a resource's binding slot.
    ///
    /// Walks the sole driver struct's resource lists (matching on
    /// [`DeclId`]) and sets the matching resource's `binding` field.
    /// The variant of `binding` must match the resource kind;
    /// mismatches are a caller error and are silently ignored here
    /// (a later [`Self::commit`] will typically reject the model
    /// because the slot is still `None`, surfacing as
    /// `RequiredBindingMissing` from the Phase-2 boundary rule).
    pub fn insert_binding(&mut self, resource: DeclId, binding: ResourceBinding) {
        let Some(driver) = find_driver_mut(&mut self.model.root_module) else {
            return;
        };
        match binding {
            ResourceBinding::Clock(b) => {
                if let Some(c) = driver
                    .clocks
                    .iter_mut()
                    .find(|c| c.id.decl_id() == resource)
                {
                    c.binding = Some(b);
                }
            }
            ResourceBinding::Reset(b) => {
                if let Some(r) = driver
                    .resets
                    .iter_mut()
                    .find(|r| r.id.decl_id() == resource)
                {
                    r.binding = Some(b);
                }
            }
            ResourceBinding::PinCtrl(b) => {
                if let Some(p) = driver
                    .pinctrl
                    .iter_mut()
                    .find(|p| p.id.decl_id() == resource)
                {
                    p.binding = Some(b);
                }
            }
            ResourceBinding::Interrupt(b) => {
                if let Some(i) = driver
                    .interrupts
                    .iter_mut()
                    .find(|i| i.id.decl_id() == resource)
                {
                    i.binding = Some(b);
                }
            }
            ResourceBinding::Mmio(b) => {
                if let Some(m) = driver.mmio.iter_mut().find(|m| m.id.decl_id() == resource) {
                    m.binding = Some(b);
                }
            }
        }
    }

    /// Populate a property's `bound_value` slot.
    pub fn insert_property_value(&mut self, property: DeclId, value: LiteralValue) {
        let Some(driver) = find_driver_mut(&mut self.model.root_module) else {
            return;
        };
        if let Some(p) = driver
            .properties
            .iter_mut()
            .find(|p| p.id.decl_id() == property)
        {
            p.bound_value = Some(value);
        }
    }

    /// Commit the mutation.
    ///
    /// Runs [`verify_phase_1`] or [`verify_phase_2`] (per the phase
    /// set at [`RegseqModel::begin_mutation`]) against the mutated
    /// model. On structural success, the mutation sticks, the
    /// snapshot is dropped, and any non-fatal [`LintFinding`]s flow
    /// out on the `Ok` arm inside a [`VerifyReport`]. On structural
    /// failure, the model is rolled back to the pre-mutation
    /// snapshot and the verifier error is returned.
    ///
    /// Rollback is gated on the `Err(VerifierError)` arm alone: a
    /// committed mutation whose resulting model triggers a lint rule
    /// is still committed, and the returned report's `lints` vector
    /// is the caller's opt-in visibility into those findings. Every
    /// existing caller that previously treated `commit` as pass/fail
    /// simply ignores the returned `VerifyReport`.
    ///
    /// `commit` consumes the guard, so the caller cannot continue
    /// mutating after a commit attempt: either the mutation is
    /// accepted or it is fully discarded.
    ///
    /// [`verify_phase_1`]: crate::verify::verify_phase_1
    /// [`verify_phase_2`]: crate::verify::verify_phase_2
    /// [`LintFinding`]: crate::verify::LintFinding
    pub fn commit(mut self) -> Result<VerifyReport, VerifierError> {
        // Take the snapshot out so the Drop impl does not roll back
        // after a successful commit.
        let snapshot = self
            .snapshot
            .take()
            .expect("snapshot was set by begin_mutation");
        let result = match self.phase {
            MutationPhase::Phase1 => verify_phase_1(self.model),
            MutationPhase::Phase2 => verify_phase_2(self.model),
        };
        match result {
            Ok(report) => Ok(report),
            Err(e) => {
                *self.model = snapshot;
                Err(e)
            }
        }
    }
}

impl Drop for MutationGuard<'_> {
    /// Roll back on drop if the guard was not committed.
    ///
    /// `commit` clears `snapshot` on both the success and failure
    /// paths, so the Drop path only fires when the caller abandoned
    /// the guard (explicitly via `drop` or implicitly via an early
    /// return or panic). Rolling back here is what makes the
    /// verification contract airtight: there is no way for a caller
    /// to walk away from a mutation and leave the live model in an
    /// unverified state.
    fn drop(&mut self) {
        if let Some(snap) = self.snapshot.take() {
            *self.model = snap;
        }
    }
}

// --- Tree walkers ----------------------------------------------------

/// Locate the first module (in document order, starting at `root`)
/// whose id matches `target`.
fn find_module_mut(root: &mut Module, target: ModuleId) -> Option<&mut Module> {
    if root.id == target {
        return Some(root);
    }
    for sub in &mut root.submodules {
        if let Some(m) = find_module_mut(sub, target) {
            return Some(m);
        }
    }
    None
}

/// Locate the first [`DriverStruct`] in document order.
fn find_driver_mut(root: &mut Module) -> Option<&mut DriverStruct> {
    if root.kind == ModuleKind::Driver {
        if let Some(driver) = root.driver.as_mut() {
            return Some(driver);
        }
    }
    for sub in &mut root.submodules {
        if let Some(driver) = find_driver_mut(sub) {
            return Some(driver);
        }
    }
    None
}

/// Locate the first function (in document order, across free
/// functions, driver impl blocks, trait impl blocks, and plain impl
/// blocks) whose id matches `target`.
fn find_function_mut(root: &mut Module, target: FunctionId) -> Option<&mut Function> {
    if let Some(f) = root.free_functions.iter_mut().find(|f| f.id == target) {
        return Some(f);
    }
    for block in &mut root.impl_blocks {
        if let Some(f) = block.methods.iter_mut().find(|f| f.id == target) {
            return Some(f);
        }
    }
    for block in &mut root.trait_impls {
        if let Some(f) = block.methods.iter_mut().find(|f| f.id == target) {
            return Some(f);
        }
    }
    for block in &mut root.driver_impl_blocks {
        if let Some(f) = block.methods.iter_mut().find(|f| f.id == target) {
            return Some(f);
        }
    }
    for sub in &mut root.submodules {
        if let Some(f) = find_function_mut(sub, target) {
            return Some(f);
        }
    }
    None
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{DeclId, ModuleId, SymbolInterner};
    use crate::model::module::ModuleKind;
    use std::collections::{BTreeMap, BTreeSet};

    fn empty_module(symbols: &mut SymbolInterner, name: &str, id: u32, kind: ModuleKind) -> Module {
        let name_sym = symbols.intern(name);
        Module {
            id: ModuleId::from(DeclId::new(id)),
            name: name_sym,
            kind,
            submodules: vec![],
            constants: vec![],
            free_functions: vec![],
            type_defs: vec![],
            register_descriptors: vec![],
            register_blocks: vec![],
            impl_blocks: vec![],
            driver: None,
            trait_impls: vec![],
            driver_impl_blocks: vec![],
            abstractions_used: BTreeSet::new(),
            intrinsics_used: BTreeSet::new(),
        }
    }

    fn empty_model() -> RegseqModel {
        let mut symbols = SymbolInterner::new();
        let root = empty_module(&mut symbols, "crate_root", 1, ModuleKind::Normal);
        RegseqModel {
            symbols,
            source_locations: BTreeMap::new(),
            root_module: root,
            format_version: 1,
        }
    }

    #[test]
    fn find_module_mut_locates_root_by_id() {
        let mut m = empty_model();
        let id = m.root_module.id;
        let found = find_module_mut(&mut m.root_module, id).expect("root found");
        assert_eq!(found.id, id);
    }

    #[test]
    fn find_module_mut_locates_nested_submodule() {
        let mut m = empty_model();
        let sub = empty_module(&mut m.symbols, "child", 2, ModuleKind::Normal);
        let sub_id = sub.id;
        m.root_module.submodules.push(sub);

        let found = find_module_mut(&mut m.root_module, sub_id).expect("submodule found");
        assert_eq!(found.id, sub_id);
    }

    #[test]
    fn find_module_mut_returns_none_for_unknown_id() {
        let mut m = empty_model();
        let bogus = ModuleId::from(DeclId::new(999));
        assert!(find_module_mut(&mut m.root_module, bogus).is_none());
    }
}
