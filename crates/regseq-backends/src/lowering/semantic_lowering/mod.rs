//! Semantic body lowering (Phase 2B).
//!
//! Walks MIR CFGs and produces `SemanticBody` IR, populating the empty
//! bodies left by Phase 2A hierarchy construction. This is the canonical
//! semantic-body lowering path for the supported Regseq construct slice.
//!
//! The lowering reuses existing MIR infrastructure (`call_classifier`,
//! `place_resolver`, `cfg_helpers`) but targets the semantic IR types
//! (`Stmt`, `Expr`, `SemanticLocal`) rather than `BehaviorNode`.

pub mod call_lowering;
pub mod context;
pub mod expr_lowering;
pub mod external_items;
pub mod local_env;
pub mod option_lowering;
pub mod ref_resolver;
pub mod semantic_walker;
pub mod type_lowering;
pub mod validation;

use std::collections::{BTreeMap, BTreeSet};

use regseq_analysis::AnalysisInput;
use regseq_model::item::identity::{ItemId, ItemStore};
use regseq_model::item::types::Item;
use regseq_model::semantic::body::{Block, SemanticBody};

use self::context::{build_fn_context, SemanticLoweringContext};
use self::ref_resolver::ReferenceResolver;
use self::semantic_walker::walk_semantic_region;
use super::hierarchy::ItemHierarchy;
use super::mir_interpreter::context::MirInterpreterContext;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors during semantic body lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticLoweringError {
    /// No struct with `ItemAttribute::Driver` found in the hierarchy.
    DriverStructNotFound,
    /// No register block found for an MMIO field.
    RegisterBlockNotFound { mmio_field: String },
    /// A MIR function def_path could not be resolved to a FunctionDef.
    UnresolvedFunction { path: String, hint: String },
    /// A register name could not be resolved through the ownership chain.
    UnresolvedRegister { name: String, context: String },
    /// A driver field could not be resolved on the driver struct.
    UnresolvedDriverField { name: String },
    /// A constant canonical path could not be resolved.
    UnresolvedConstant { path: String },
    /// An interrupt class mask value could not be matched.
    UnresolvedInterruptClass { mask: u32, context: String },
    /// A resource field could not be resolved.
    UnresolvedResource { field_name: String },
    /// An unsupported MIR construct was encountered during lowering.
    UnsupportedConstruct {
        function: String,
        description: String,
    },
    /// A MIR body was missing for a function that requires lowering.
    MissingMirBody { function: String },
    /// A discovered external type is not in the supported external type set.
    UnsupportedExternalType { path: String, context: String },
    /// Field/resource resolution attempted on a non-driver owner (current
    /// slice limitation).
    UnsupportedOwnerType { owner_path: String, context: String },
}

impl std::fmt::Display for SemanticLoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DriverStructNotFound => write!(f, "no driver struct found in hierarchy"),
            Self::RegisterBlockNotFound { mmio_field } => {
                write!(f, "no register block found for MMIO field '{mmio_field}'")
            }
            Self::UnresolvedFunction { path, hint } => {
                write!(f, "unresolved function: {path} (hint: {hint})")
            }
            Self::UnresolvedRegister { name, context } => {
                write!(f, "unresolved register '{name}' in {context}")
            }
            Self::UnresolvedDriverField { name } => {
                write!(f, "unresolved driver field '{name}'")
            }
            Self::UnresolvedConstant { path } => {
                write!(f, "unresolved constant: {path}")
            }
            Self::UnresolvedInterruptClass { mask, context } => {
                write!(
                    f,
                    "unresolved interrupt class with mask {mask:#06x} in {context}"
                )
            }
            Self::UnresolvedResource { field_name } => {
                write!(f, "unresolved resource field '{field_name}'")
            }
            Self::UnsupportedConstruct {
                function,
                description,
            } => {
                write!(f, "unsupported construct in {function}: {description}")
            }
            Self::UnsupportedExternalType { path, context } => {
                write!(f, "unsupported external type '{path}': {context}")
            }
            Self::MissingMirBody { function } => {
                write!(f, "missing MIR body for function '{function}'")
            }
            Self::UnsupportedOwnerType {
                owner_path,
                context,
            } => {
                write!(
                    f,
                    "unsupported non-driver owner type '{owner_path}' in {context}"
                )
            }
        }
    }
}

impl std::error::Error for SemanticLoweringError {}

// ---------------------------------------------------------------------------
// Hierarchy population
// ---------------------------------------------------------------------------

/// Reconstruct the `ItemHierarchy` with populated `FunctionDef` bodies.
///
/// Iterates `0..store.len()`, clones each item, and replaces
/// `FunctionDef.body` where a lowered body exists in the `lowered` map.
/// Modules, cross-link table, and interrupt classes are carried over
/// unchanged. Item ordering is preserved so all existing `ItemId`
/// references remain valid.
pub fn populate_hierarchy(
    hierarchy: &ItemHierarchy,
    lowered: &BTreeMap<ItemId, SemanticBody>,
) -> ItemHierarchy {
    let mut new_store = ItemStore::new();
    for i in 0..hierarchy.store.len() {
        let id = ItemId(i as u32);
        let mut item = hierarchy
            .store
            .get(id)
            .expect("contiguous item store")
            .clone();
        if let Item::Function(ref mut func_def) = item {
            if let Some(body) = lowered.get(&id) {
                func_def.body = body.clone();
            }
        }
        new_store.push(item);
    }
    ItemHierarchy {
        store: new_store,
        modules: hierarchy.modules.clone(),
        cross_link: hierarchy.cross_link.clone(),
        interrupt_classes: hierarchy.interrupt_classes.clone(),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Lower MIR bodies and populate `FunctionDef.body` in the hierarchy.
///
/// This is the Phase 2B entry point. Takes a Phase 2A hierarchy (with empty
/// bodies) and the `AnalysisInput` (carrying MIR), and returns a new
/// `ItemHierarchy` with populated `SemanticBody` on each lowered
/// `FunctionDef`.
///
/// Non-lowered functions (those without MIR bodies in the input) retain
/// `SemanticBody::empty()`.
pub fn lower_and_populate_hierarchy(
    hierarchy: &ItemHierarchy,
    input: &AnalysisInput,
) -> Result<ItemHierarchy, SemanticLoweringError> {
    // Step 1: Discover and materialize external items.
    let augmented = external_items::discover_and_materialize(hierarchy, input)?;

    let mir_ctx = MirInterpreterContext::from_analysis(input);
    let resolver = ReferenceResolver::build(&augmented)?;
    let shared_ctx = SemanticLoweringContext {
        mir_ctx: &mir_ctx,
        hierarchy: &augmented,
        resolver,
    };

    let mut lowered: BTreeMap<ItemId, SemanticBody> = BTreeMap::new();

    // Collect all function bodies to lower.
    let reachable = input.mir_reachable_helper_names();
    let callbacks = input.mir_callback_bodies();
    let isrs = input.mir_isr_bodies();
    let helpers = input.mir_helper_bodies();
    let free_fns = input.mir_free_function_bodies();

    let all_bodies: Vec<_> = callbacks
        .iter()
        .chain(isrs.iter())
        .chain(helpers.iter().filter(|f| reachable.contains(&f.name)))
        .chain(free_fns.iter().filter(|f| reachable.contains(&f.name)))
        .collect();

    for func in &all_bodies {
        // Canonical path resolution only. No short-name fallback.
        let (func_id, func_def) = {
            let (id, def) = shared_ctx
                .resolver
                .resolve_function_by_def_path(&func.def_path)?;
            (id, def.clone())
        };

        let mut fn_ctx = build_fn_context(&shared_ctx, func, func_id, &func_def);
        let stmts = walk_semantic_region(
            &mut fn_ctx,
            &shared_ctx.resolver,
            &func.body,
            0,
            &BTreeSet::new(),
        )?;

        let body = SemanticBody {
            locals: fn_ctx.local_env.locals,
            params: fn_ctx.local_env.params,
            return_type: fn_ctx.local_env.return_type,
            body: Block { stmts },
        };

        lowered.insert(func_id, body);
    }

    Ok(populate_hierarchy(&augmented, &lowered))
}
