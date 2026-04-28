//! Phase-2 cross-item rule bucket (task 56).
//!
//! Two checks (spec.md Section 13 line 1432, "Interrupt ISR bindings
//! match declared interrupt resources"):
//!
//! - **ISR binding target resolves**: every populated
//!   `InterruptResource.binding.isr_target` `FunctionId` MUST appear
//!   in the model's function index. A dangling binding raises
//!   [`VerifierErrorKind::UnknownIsrTarget`].
//! - **ISR binding target is ISR-classified**: the resolved
//!   [`Function`] MUST carry the `#[rseq::isr]` attribute marker that
//!   Phase 5 records on `Attrs.entries` (key `"rseq::isr"`). A
//!   non-ISR function bound as an ISR target raises
//!   [`VerifierErrorKind::NonIsrFunctionBoundAsIsr`].
//!
//! These rules only run at Phase 2 because the binding (and therefore
//! the ISR target) is populated post-join. The Phase-1 cross-item
//! bucket skips them for that reason.

use std::collections::BTreeMap;

use crate::identity::{DeclId, FunctionId, SourceOrigin};
use crate::model::driver::DriverStruct;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::module::Module;
use crate::model::regseq_model::RegseqModel;

use super::super::{VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance};

/// The Attrs entry key Phase 5 uses to record `#[rseq::isr]`.
const ISR_ATTR_KEY: &str = "rseq::isr";

/// Run Phase-2 cross-item checks in deterministic order, returning
/// the first violation.
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    let functions = collect_functions(model);
    walk_drivers(model, &mut |d| isr_targets_resolve(model, d, &functions))
}

fn isr_targets_resolve(
    model: &RegseqModel,
    driver: &DriverStruct,
    functions: &BTreeMap<DeclId, &Function>,
) -> Result<(), VerifierError> {
    for r in &driver.interrupts {
        let Some(binding) = &r.binding else {
            continue;
        };
        let Some(target) = binding.isr_target else {
            continue;
        };
        let interrupt_decl = r.id.decl_id();
        let Some(func) = functions.get(&target.decl_id()) else {
            return Err(unknown_isr_target(model, interrupt_decl, target));
        };
        if !is_isr_classified(func) {
            return Err(non_isr_function(model, interrupt_decl, target));
        }
    }
    Ok(())
}

fn is_isr_classified(f: &Function) -> bool {
    f.attrs
        .entries
        .iter()
        .any(|entry| entry.key == ISR_ATTR_KEY)
}

fn unknown_isr_target(model: &RegseqModel, interrupt: DeclId, target: FunctionId) -> VerifierError {
    VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::UnknownIsrTarget { interrupt, target },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", interrupt.index()),
            lookup_origin(model, interrupt),
        ),
        details: format!(
            "interrupt resource (DeclId {}) binds ISR target FunctionId {} which is not \
             declared in the model",
            interrupt.index(),
            target.decl_id().index()
        ),
    }
}

fn non_isr_function(model: &RegseqModel, interrupt: DeclId, target: FunctionId) -> VerifierError {
    VerifierError {
        level: VerifierLevel::CrossItem,
        kind: VerifierErrorKind::NonIsrFunctionBoundAsIsr { interrupt, target },
        provenance: VerifierProvenance::for_item(
            format!("decl#{}", interrupt.index()),
            lookup_origin(model, interrupt),
        ),
        details: format!(
            "interrupt resource (DeclId {}) binds FunctionId {} as ISR target, but the \
             function does not carry the #[rseq::isr] attribute marker",
            interrupt.index(),
            target.decl_id().index()
        ),
    }
}

fn collect_functions(model: &RegseqModel) -> BTreeMap<DeclId, &Function> {
    let mut map: BTreeMap<DeclId, &Function> = BTreeMap::new();
    fn walk<'a>(map: &mut BTreeMap<DeclId, &'a Function>, m: &'a Module) {
        for f in &m.free_functions {
            map.insert(f.id.decl_id(), f);
        }
        for ib in &m.impl_blocks {
            visit_impl(map, ib);
        }
        for ib in &m.trait_impls {
            visit_impl(map, ib);
        }
        for ib in &m.driver_impl_blocks {
            visit_impl(map, ib);
        }
        for sub in &m.submodules {
            walk(map, sub);
        }
    }
    fn visit_impl<'a>(map: &mut BTreeMap<DeclId, &'a Function>, ib: &'a ImplBlock) {
        for f in &ib.methods {
            map.insert(f.id.decl_id(), f);
        }
    }
    walk(&mut map, &model.root_module);
    map
}

fn walk_drivers<F>(model: &RegseqModel, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&DriverStruct) -> Result<(), VerifierError>,
{
    walk_modules(&model.root_module, &mut |m| {
        if let Some(d) = &m.driver {
            f(d)?;
        }
        Ok(())
    })
}

fn walk_modules<F>(root: &Module, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&Module) -> Result<(), VerifierError>,
{
    f(root)?;
    for s in &root.submodules {
        walk_modules(s, f)?;
    }
    Ok(())
}

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
