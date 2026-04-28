//! Per-module dependency-set accounting (task regseq-model-53).
//!
//! This module hosts the Pass-3 derivation step. It is invoked after
//! Pass 2 body construction completes for every body in the model;
//! the construction module re-exports [`derive_module_dependencies`]
//! so callers that orchestrate Pass 1 + Pass 2 invoke it as the
//! terminal construction step (no Pass 2 orchestrator exists yet --
//! bodies are attached per-function via
//! [`build_body`](super::build_body), so the Pass-3 hook is a
//! publicly-invoked function rather than an embedded stage).
//!
//! [`derive_module_dependencies`] walks the populated module tree
//! bottom-up and fills each module's
//! [`Module::abstractions_used`](regseq_model_old::model::Module::abstractions_used)
//! and
//! [`Module::intrinsics_used`](regseq_model_old::model::Module::intrinsics_used)
//! sets. See spec.md Section 2 (module definition) and Section 10
//! (derivation rule).
//!
//! The derivation is purely observational: it inspects the populated
//! model and never reports errors. A parent module's sets do NOT
//! include the sets of its submodules (per spec.md Section 2).

use std::collections::{BTreeSet, HashMap};

use regseq_model_old::identity::TypeDefId;
use regseq_model_old::ir::{
    MirIntrinsic, Operand, Place, Projection, RegseqAbstraction, RegseqIntrinsic, RegseqType,
    Rvalue, StatementKind, Terminator, TerminatorKind,
};
use regseq_model_old::model::{
    Function, Module, ModuleKind, RegseqModel, SyncKind, TypeDefClassification, TypeDefKind,
};

/// Derive per-module dependency sets for every module in `model`.
///
/// Walks the module tree bottom-up (leaf modules first, parents last)
/// and overwrites each module's `abstractions_used` and
/// `intrinsics_used` sets with the freshly-derived contents. The
/// previous contents are discarded.
pub fn derive_module_dependencies(model: &mut RegseqModel) {
    let classifications = collect_classifications(&model.root_module);
    derive_in_module(&mut model.root_module, &classifications);
}

/// Build a model-wide `TypeDefId -> TypeDefClassification` lookup.
/// The lookup spans every module in the tree so an abstraction type
/// defined in one module can be detected in another module's item
/// that references it.
fn collect_classifications(root: &Module) -> HashMap<TypeDefId, TypeDefClassification> {
    let mut out = HashMap::new();
    collect_classifications_inner(root, &mut out);
    out
}

fn collect_classifications_inner(
    module: &Module,
    out: &mut HashMap<TypeDefId, TypeDefClassification>,
) {
    for td in &module.type_defs {
        out.insert(td.id, td.classification.clone());
    }
    for sub in &module.submodules {
        collect_classifications_inner(sub, out);
    }
}

fn derive_in_module(
    module: &mut Module,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
) {
    // Leaf modules first so parents see their own content only.
    for sub in &mut module.submodules {
        derive_in_module(sub, classifications);
    }

    let mut abstractions: BTreeSet<RegseqAbstraction> = BTreeSet::new();
    let mut intrinsics: BTreeSet<RegseqIntrinsic> = BTreeSet::new();

    if module.kind == ModuleKind::Driver {
        if let Some(driver) = &module.driver {
            if !driver.clocks.is_empty() {
                abstractions.insert(RegseqAbstraction::Clock);
            }
            if !driver.resets.is_empty() {
                abstractions.insert(RegseqAbstraction::Reset);
            }
            if !driver.pinctrl.is_empty() {
                abstractions.insert(RegseqAbstraction::PinCtrl);
            }
            if !driver.interrupts.is_empty() {
                abstractions.insert(RegseqAbstraction::Interrupt);
            }
            if !driver.mmio.is_empty() {
                abstractions.insert(RegseqAbstraction::Mmio);
            }
            for s in &driver.sync {
                abstractions.insert(sync_kind_to_abstraction(s.sync_kind));
            }
        }
    }

    for td in &module.type_defs {
        match &td.kind {
            TypeDefKind::Struct { fields } => {
                for f in fields {
                    scan_type(&f.ty, classifications, &mut abstractions);
                }
            }
            TypeDefKind::Enum { variants } => {
                for v in variants {
                    for f in &v.fields {
                        scan_type(&f.ty, classifications, &mut abstractions);
                    }
                }
            }
        }
    }

    for f in &module.free_functions {
        scan_function(f, classifications, &mut abstractions, &mut intrinsics);
    }
    for b in &module.impl_blocks {
        for m in &b.methods {
            scan_function(m, classifications, &mut abstractions, &mut intrinsics);
        }
    }
    for b in &module.trait_impls {
        for m in &b.methods {
            scan_function(m, classifications, &mut abstractions, &mut intrinsics);
        }
    }
    for b in &module.driver_impl_blocks {
        for m in &b.methods {
            scan_function(m, classifications, &mut abstractions, &mut intrinsics);
        }
    }

    module.abstractions_used = abstractions;
    module.intrinsics_used = intrinsics;
}

fn scan_function(
    f: &Function,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
    intrinsics: &mut BTreeSet<RegseqIntrinsic>,
) {
    for p in &f.params {
        scan_type(&p.ty, classifications, abstractions);
    }
    scan_type(&f.return_type, classifications, abstractions);
    for local in &f.body.local_decls {
        scan_type(&local.ty, classifications, abstractions);
    }
    for bb in &f.body.basic_blocks {
        for stmt in &bb.statements {
            scan_statement_kind(&stmt.kind, classifications, abstractions);
        }
        scan_terminator(&bb.terminator, classifications, abstractions, intrinsics);
    }
}

fn scan_statement_kind(
    kind: &StatementKind,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
) {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            scan_place(place, classifications, abstractions);
            scan_rvalue(rvalue, classifications, abstractions);
        }
        StatementKind::SetDiscriminant { place, .. } => {
            scan_place(place, classifications, abstractions);
        }
        StatementKind::MirIntrinsic(mi) => match mi {
            MirIntrinsic::Assume(op) => scan_operand(op, classifications, abstractions),
            MirIntrinsic::CopyNonOverlapping { src, dst, count } => {
                scan_operand(src, classifications, abstractions);
                scan_operand(dst, classifications, abstractions);
                scan_operand(count, classifications, abstractions);
            }
        },
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => {}
    }
}

fn scan_rvalue(
    rv: &Rvalue,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
) {
    match rv {
        Rvalue::Use(op) | Rvalue::Cast(_, op, _) | Rvalue::UnaryOp(_, op) => {
            scan_operand(op, classifications, abstractions);
        }
        Rvalue::Repeat(op, _) => {
            scan_operand(op, classifications, abstractions);
        }
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => {
            scan_place(place, classifications, abstractions);
        }
        Rvalue::BinaryOp(_, l, r) => {
            scan_operand(l, classifications, abstractions);
            scan_operand(r, classifications, abstractions);
        }
        Rvalue::Aggregate(_, ops) => {
            for op in ops {
                scan_operand(op, classifications, abstractions);
            }
        }
    }
}

fn scan_operand(
    op: &Operand,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
) {
    if let Operand::Copy(p) | Operand::Move(p) = op {
        scan_place(p, classifications, abstractions);
    }
}

fn scan_terminator(
    term: &Terminator,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
    intrinsics: &mut BTreeSet<RegseqIntrinsic>,
) {
    match &term.kind {
        TerminatorKind::Call {
            args, destination, ..
        } => {
            for a in args {
                scan_operand(a, classifications, abstractions);
            }
            scan_place(destination, classifications, abstractions);
        }
        TerminatorKind::IntrinsicCall {
            intrinsic,
            args,
            destination,
            ..
        } => {
            intrinsics.insert(*intrinsic);
            for a in args {
                scan_operand(a, classifications, abstractions);
            }
            scan_place(destination, classifications, abstractions);
        }
        TerminatorKind::Drop { place, .. } => {
            scan_place(place, classifications, abstractions);
        }
        TerminatorKind::SwitchInt { discriminant, .. } => {
            scan_operand(discriminant, classifications, abstractions);
        }
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

fn scan_place(
    place: &Place,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
) {
    for proj in &place.projection {
        if let Projection::Field { ty, .. } = proj {
            scan_type(ty, classifications, abstractions);
        }
    }
}

fn scan_type(
    ty: &RegseqType,
    classifications: &HashMap<TypeDefId, TypeDefClassification>,
    abstractions: &mut BTreeSet<RegseqAbstraction>,
) {
    match ty {
        RegseqType::Defined(id) => {
            if let Some(TypeDefClassification::Abstraction(a)) = classifications.get(id) {
                abstractions.insert(*a);
            }
        }
        RegseqType::Option(inner) | RegseqType::Slice(inner) => {
            scan_type(inner, classifications, abstractions);
        }
        RegseqType::Array(inner, _) => {
            scan_type(inner, classifications, abstractions);
        }
        RegseqType::Ref(inner, _) => {
            scan_type(inner, classifications, abstractions);
        }
        RegseqType::Result(ok, err) => {
            scan_type(ok, classifications, abstractions);
            scan_type(err, classifications, abstractions);
        }
        RegseqType::Scalar(_) | RegseqType::Bool | RegseqType::Unit | RegseqType::Never => {}
    }
}

fn sync_kind_to_abstraction(kind: SyncKind) -> RegseqAbstraction {
    match kind {
        SyncKind::Mutex => RegseqAbstraction::Mutex,
        SyncKind::SpinLock => RegseqAbstraction::SpinLock,
        SyncKind::IrqSpinLock => RegseqAbstraction::IrqSpinLock,
        SyncKind::Completion => RegseqAbstraction::Completion,
    }
}
