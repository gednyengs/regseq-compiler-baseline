//! Module-level and Phase-1 boundary rule bucket (task 55).
//!
//! The task 55 spec groups three kinds of checks under this bucket --
//! they are all triggered by walking the module tree and its
//! enclosed driver struct, so they share a single traversal:
//!
//! - **Module-tree invariants** (level [`VerifierLevel::Module`]):
//!   exactly one [`ModuleKind::Driver`] module in the tree; no
//!   duplicate item names within any module.
//! - **Declaration-side completeness** (level
//!   [`VerifierLevel::Module`]): every [`Function`] carries a
//!   non-placeholder body. Other declaration-side fields
//!   ([`TypeDefClassification`], resource taxonomic properties,
//!   property type) are already non-`Option` at the type level and
//!   therefore cannot be "missing".
//! - **Phase-1 boundary leakage** (level
//!   [`VerifierLevel::DriverStruct`]): every resource binding slot on
//!   the driver struct is `None`; every property `bound_value` is
//!   `None`.
//!
//! Error output is deterministic: modules are traversed in document
//! order (Vec order), driver resources in their spec-canonical field
//! order (clock, reset, pinctrl, interrupt, mmio), and within each
//! resource/property Vec in index order.
//!
//! [`VerifierLevel::Module`]: super::super::VerifierLevel::Module
//! [`VerifierLevel::DriverStruct`]: super::super::VerifierLevel::DriverStruct
//! [`ModuleKind::Driver`]: crate::model::module::ModuleKind::Driver
//! [`Function`]: crate::model::function::Function
//! [`TypeDefClassification`]: crate::model::framework::TypeDefClassification

use std::collections::BTreeSet;

use crate::identity::{DeclId, SourceOrigin, Symbol, SymbolInterner};
use crate::ir::{Body, TerminatorKind};
use crate::model::driver::DriverStruct;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::module::{Module, ModuleKind};
use crate::model::regseq_model::RegseqModel;

use super::super::{VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance};

/// Run every module-level and Phase-1 boundary check in deterministic
/// order, returning the first violation.
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    exactly_one_driver_module(model)?;
    no_duplicate_item_names(model)?;
    declaration_side_completeness(model)?;
    phase_1_leakage(model)?;
    Ok(())
}

/// Run the module-tree subset that still applies after Phase 2.
///
/// Excludes [`phase_1_leakage`] -- post-Phase-2 models are expected
/// to carry populated bindings and property values, so re-running
/// that check would be a false positive. The other three module-tree
/// invariants ("exactly one Driver module", "no duplicate item
/// names", "every Function has a real body") are still load-bearing
/// after the join.
pub fn run_phase_2_subset(model: &RegseqModel) -> Result<(), VerifierError> {
    exactly_one_driver_module(model)?;
    no_duplicate_item_names(model)?;
    declaration_side_completeness(model)?;
    Ok(())
}

// --- Exactly one Driver module ---------------------------------------

fn exactly_one_driver_module(model: &RegseqModel) -> Result<(), VerifierError> {
    let count = count_driver_modules(&model.root_module);
    match count {
        0 => Err(VerifierError {
            level: VerifierLevel::Module,
            kind: VerifierErrorKind::MissingDriverModule,
            provenance: VerifierProvenance::model_level(),
            details: "no module in the module tree has ModuleKind::Driver".into(),
        }),
        1 => Ok(()),
        n => Err(VerifierError {
            level: VerifierLevel::Module,
            kind: VerifierErrorKind::MultipleDriverModules { count: n },
            provenance: VerifierProvenance::model_level(),
            details: format!("expected exactly one Driver module, found {n}"),
        }),
    }
}

fn count_driver_modules(m: &Module) -> usize {
    let mine = if m.kind == ModuleKind::Driver { 1 } else { 0 };
    mine + m.submodules.iter().map(count_driver_modules).sum::<usize>()
}

// --- No duplicate item names ----------------------------------------

fn no_duplicate_item_names(model: &RegseqModel) -> Result<(), VerifierError> {
    walk_modules(&model.root_module, &mut |m| {
        check_duplicates(model, m)?;
        Ok(())
    })
}

fn check_duplicates(model: &RegseqModel, m: &Module) -> Result<(), VerifierError> {
    let module_name = resolve(&model.symbols, m.name);
    let module_origin = lookup_origin(model, m.id.decl_id());

    // Each iterator visits one item category's names in document order.
    // Vec order is stable, so iterating category-by-category, then
    // within-category by index, keeps the first-duplicate detection
    // deterministic across runs.
    let submodule_names = m.submodules.iter().map(|s| s.name);
    let constant_names = m.constants.iter().map(|c| c.name);
    let function_names = m.free_functions.iter().map(|f| f.name);
    let type_def_names = m.type_defs.iter().map(|t| t.name);
    let register_descriptor_names = m.register_descriptors.iter().map(|r| r.name);
    let register_block_names = m.register_blocks.iter().map(|r| r.name);

    let all_names = submodule_names
        .chain(constant_names)
        .chain(function_names)
        .chain(type_def_names)
        .chain(register_descriptor_names)
        .chain(register_block_names);

    let mut seen: BTreeSet<Symbol> = BTreeSet::new();
    for name in all_names {
        if !seen.insert(name) {
            let resolved = resolve(&model.symbols, name);
            let details = format!(
                "module '{}' has more than one item named '{}'",
                module_name, resolved
            );
            return Err(VerifierError {
                level: VerifierLevel::Module,
                kind: VerifierErrorKind::DuplicateItemName {
                    module: module_name.clone(),
                    name: resolved,
                },
                provenance: VerifierProvenance::for_item(module_name, module_origin),
                details,
            });
        }
    }
    Ok(())
}

// --- Declaration-side completeness ----------------------------------

fn declaration_side_completeness(model: &RegseqModel) -> Result<(), VerifierError> {
    walk_modules(&model.root_module, &mut |m| {
        for f in &m.free_functions {
            check_function_body(model, f)?;
        }
        for impl_block in &m.impl_blocks {
            check_impl_block(model, impl_block)?;
        }
        for impl_block in &m.trait_impls {
            check_impl_block(model, impl_block)?;
        }
        for impl_block in &m.driver_impl_blocks {
            check_impl_block(model, impl_block)?;
        }
        Ok(())
    })
}

fn check_impl_block(model: &RegseqModel, impl_block: &ImplBlock) -> Result<(), VerifierError> {
    for f in &impl_block.methods {
        check_function_body(model, f)?;
    }
    Ok(())
}

fn check_function_body(model: &RegseqModel, f: &Function) -> Result<(), VerifierError> {
    if is_placeholder_body(&f.body) {
        let decl_id = f.id.decl_id();
        let item_path = resolve(&model.symbols, f.name);
        return Err(VerifierError {
            level: VerifierLevel::Module,
            kind: VerifierErrorKind::DeclarationFieldUnpopulated {
                decl_id,
                field: "body",
            },
            provenance: VerifierProvenance::for_item(
                item_path.clone(),
                lookup_origin(model, decl_id),
            ),
            details: format!(
                "function '{}' has a placeholder body -- Phase-1 requires every \
                 function to carry a real (non-Pass-1-skeleton) body",
                item_path
            ),
        });
    }
    Ok(())
}

/// Return `true` if `body` is a Phase-5 Pass-1 skeleton or is
/// otherwise empty.
///
/// Phase-5 Pass-1 produces a synthetic body with exactly one basic
/// block whose only instruction is a `TerminatorKind::Unreachable`
/// (see `regseq-driver` `construction/walk.rs` -- the `Pass 1
/// placeholder bodies` comment). Phase 5 Pass 2 overwrites that
/// placeholder with the real CFG. Any function still carrying the
/// skeleton when Phase 1 completes is a Phase-boundary violation.
fn is_placeholder_body(body: &Body) -> bool {
    if body.basic_blocks.is_empty() {
        return true;
    }
    if body.basic_blocks.len() == 1 {
        let only = &body.basic_blocks[0];
        if only.statements.is_empty() && matches!(only.terminator.kind, TerminatorKind::Unreachable)
        {
            return true;
        }
    }
    false
}

// --- Phase-1 boundary leakage ---------------------------------------

fn phase_1_leakage(model: &RegseqModel) -> Result<(), VerifierError> {
    walk_modules(&model.root_module, &mut |m| {
        if let Some(driver) = &m.driver {
            check_driver_bindings(model, driver)?;
        }
        Ok(())
    })
}

fn check_driver_bindings(model: &RegseqModel, driver: &DriverStruct) -> Result<(), VerifierError> {
    // Slots are traversed in the canonical order defined by
    // DriverStruct's field declaration: clocks, resets, pinctrl,
    // interrupts, mmio. Iteration is index-stable.
    for r in &driver.clocks {
        check_binding(model, r.id.decl_id(), r.binding.is_some(), "clock")?;
    }
    for r in &driver.resets {
        check_binding(model, r.id.decl_id(), r.binding.is_some(), "reset")?;
    }
    for r in &driver.pinctrl {
        check_binding(model, r.id.decl_id(), r.binding.is_some(), "pinctrl")?;
    }
    for r in &driver.interrupts {
        check_binding(model, r.id.decl_id(), r.binding.is_some(), "interrupt")?;
    }
    for r in &driver.mmio {
        check_binding(model, r.id.decl_id(), r.binding.is_some(), "mmio")?;
    }
    for p in &driver.properties {
        if p.bound_value.is_some() {
            let decl_id = p.id.decl_id();
            let field_name = resolve(&model.symbols, p.field_name);
            return Err(VerifierError {
                level: VerifierLevel::DriverStruct,
                kind: VerifierErrorKind::UnexpectedPropertyValuePopulated { property: decl_id },
                provenance: VerifierProvenance::for_item(
                    field_name.clone(),
                    lookup_origin(model, decl_id),
                ),
                details: format!(
                    "property '{}' has bound_value populated after Phase 1; Phase 2 is the only populator",
                    field_name
                ),
            });
        }
    }
    Ok(())
}

fn check_binding(
    model: &RegseqModel,
    decl_id: DeclId,
    populated: bool,
    slot: &'static str,
) -> Result<(), VerifierError> {
    if populated {
        return Err(VerifierError {
            level: VerifierLevel::DriverStruct,
            kind: VerifierErrorKind::UnexpectedBindingPopulated {
                resource: decl_id,
                slot,
            },
            provenance: VerifierProvenance::for_item(
                slot.to_string(),
                lookup_origin(model, decl_id),
            ),
            details: format!(
                "{slot} resource (DeclId {}) has binding populated after Phase 1; \
                 Phase 2 is the only populator",
                decl_id.index()
            ),
        });
    }
    Ok(())
}

// --- Shared helpers -------------------------------------------------

/// Depth-first walk over the module tree, invoking `f` for each
/// module including the root.
///
/// Deterministic: each module is visited once, then its submodules in
/// Vec order. Stops at the first `Err` returned by `f`.
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

fn resolve(interner: &SymbolInterner, sym: Symbol) -> String {
    interner.resolve(sym).to_owned()
}

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
