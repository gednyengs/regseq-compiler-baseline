//! Call-graph reachability helpers over a [`RegseqModel`].
//!
//! The call graph is formed by `TerminatorKind::Call { callee, .. }`
//! edges; `IntrinsicCall` terminators are **not** edges because
//! intrinsics are not user-defined functions.
//!
//! Both helpers return [`BTreeSet`] values so iteration order is
//! deterministic (`FunctionId` and `Symbol` both derive `Ord`).
//!
//! Complexity is O(F + E) for the closure, where F is the number of
//! functions and E is the number of `Call` terminator edges -- a
//! standard BFS.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::identity::{FunctionId, Symbol};
use crate::ir::terminator::TerminatorKind;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::module::{Module, ModuleKind};
use crate::model::regseq_model::RegseqModel;

/// Collect every [`Function`] declared in `module` and its submodules.
///
/// Reaches free functions, non-driver impl-block methods, the driver's
/// Regseq trait-impl methods, and driver impl-block methods (helpers +
/// ISRs). Returns a `Vec<&Function>` rather than exposing a callback
/// interface so callers can hold the borrow without running into
/// higher-ranked-lifetime issues at the closure boundary.
pub(super) fn collect_functions(module: &Module) -> Vec<&Function> {
    let mut out: Vec<&Function> = Vec::new();
    push_functions(module, &mut out);
    out
}

fn push_functions<'a>(module: &'a Module, out: &mut Vec<&'a Function>) {
    for function in &module.free_functions {
        out.push(function);
    }
    for impl_block in &module.impl_blocks {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for impl_block in &module.trait_impls {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for impl_block in &module.driver_impl_blocks {
        for method in &impl_block.methods {
            out.push(method);
        }
    }
    for submodule in &module.submodules {
        push_functions(submodule, out);
    }
}

/// Return the set of [`FunctionId`]s transitively reachable from
/// `seeds` via `TerminatorKind::Call` edges.
///
/// The returned set **includes** each seed so callers who only want
/// the closure excluding the seeds themselves should remove them
/// explicitly.
///
/// Determinism: the BFS walks successors in sorted order, and the
/// output is a [`BTreeSet`] (natural order).
pub fn transitively_reachable_from(
    model: &RegseqModel,
    seeds: &BTreeSet<FunctionId>,
) -> BTreeSet<FunctionId> {
    let call_graph = build_call_graph(model);

    let mut reachable: BTreeSet<FunctionId> = BTreeSet::new();
    let mut queue: VecDeque<FunctionId> = VecDeque::new();
    for seed in seeds {
        if reachable.insert(*seed) {
            queue.push_back(*seed);
        }
    }

    while let Some(current) = queue.pop_front() {
        if let Some(callees) = call_graph.get(&current) {
            for callee in callees {
                if reachable.insert(*callee) {
                    queue.push_back(*callee);
                }
            }
        }
    }

    reachable
}

/// Return the set of helper-function names transitively reachable from
/// any Regseq-trait callback method on the driver.
///
/// Seeds are every method defined on a trait-impl block of the driver
/// module (Lifecycle, Serial, SerialConsole, PowerManagement, etc.).
/// The result set is the transitive closure through
/// `TerminatorKind::Call` edges, **excluding the seed callbacks
/// themselves**, projected to each function's [`Symbol`] name.
///
/// Callers that need ISRs or other seed sets should call
/// [`transitively_reachable_from`] directly.
pub fn reachable_helper_names(model: &RegseqModel) -> BTreeSet<Symbol> {
    let seeds = callback_seed_set(model);
    let reachable = transitively_reachable_from(model, &seeds);

    let functions_by_id = index_functions_by_id(model);

    let mut names = BTreeSet::new();
    for id in &reachable {
        if seeds.contains(id) {
            continue;
        }
        if let Some(function) = functions_by_id.get(id) {
            names.insert(function.name);
        }
    }
    names
}

// -- Internal helpers --------------------------------------------------

/// Build the call graph: `caller -> sorted callees`.
fn build_call_graph(model: &RegseqModel) -> BTreeMap<FunctionId, Vec<FunctionId>> {
    let mut graph: BTreeMap<FunctionId, Vec<FunctionId>> = BTreeMap::new();
    for function in collect_functions(&model.root_module) {
        let mut callees: BTreeSet<FunctionId> = BTreeSet::new();
        for block in &function.body.basic_blocks {
            if let TerminatorKind::Call { callee, .. } = &block.terminator.kind {
                callees.insert(*callee);
            }
        }
        graph.insert(function.id, callees.into_iter().collect());
    }
    graph
}

/// Return every function in the model indexed by its [`FunctionId`].
fn index_functions_by_id(model: &RegseqModel) -> BTreeMap<FunctionId, &Function> {
    let mut out: BTreeMap<FunctionId, &Function> = BTreeMap::new();
    for function in collect_functions(&model.root_module) {
        out.insert(function.id, function);
    }
    out
}

/// Return the seed set of callback method ids for
/// [`reachable_helper_names`].
///
/// Seeds are every method on every Regseq trait-impl block
/// ([`Module::trait_impls`]) of the sole driver module.
fn callback_seed_set(model: &RegseqModel) -> BTreeSet<FunctionId> {
    let mut seeds: BTreeSet<FunctionId> = BTreeSet::new();
    collect_driver_callbacks(&model.root_module, &mut seeds);
    seeds
}

fn collect_driver_callbacks(module: &Module, out: &mut BTreeSet<FunctionId>) {
    if module.kind == ModuleKind::Driver {
        for impl_block in &module.trait_impls {
            append_method_ids(impl_block, out);
        }
    }
    for submodule in &module.submodules {
        collect_driver_callbacks(submodule, out);
    }
}

fn append_method_ids(impl_block: &ImplBlock, out: &mut BTreeSet<FunctionId>) {
    for method in &impl_block.methods {
        out.insert(method.id);
    }
}
