//! Semantic lowering context.
//!
//! `SemanticLoweringContext` wraps the shared MIR interpreter context
//! with hierarchy and resolver. `SemanticFnContext` adds per-function
//! local environment.

use regseq_model::declaration::mir_body::MirFnBodyDecl;
use regseq_model::item::function::{FunctionDef, FunctionKind};
use regseq_model::item::identity::ItemId;

use super::local_env::LocalEnvironment;
use super::ref_resolver::ReferenceResolver;
use crate::lowering::hierarchy::ItemHierarchy;
use crate::lowering::mir_interpreter::context::{FnContext, MirInterpreterContext};

// ---------------------------------------------------------------------------
// Shared context
// ---------------------------------------------------------------------------

/// Shared context for semantic body lowering across all functions.
pub struct SemanticLoweringContext<'a> {
    /// MIR interpreter shared context (register metadata, resource fields, etc.)
    pub mir_ctx: &'a MirInterpreterContext,
    /// Item hierarchy for structured ref resolution and owner lookups.
    pub hierarchy: &'a ItemHierarchy,
    /// Canonical identity resolver.
    pub resolver: ReferenceResolver,
}

// ---------------------------------------------------------------------------
// Per-function context
// ---------------------------------------------------------------------------

/// Per-function context for semantic body lowering.
pub struct SemanticFnContext<'a> {
    /// MIR per-function context (local_decls, copy_sources, etc.)
    pub fn_ctx: FnContext<'a>,
    /// Semantic local environment.
    pub local_env: LocalEnvironment,
    /// ItemId of the function being lowered.
    pub func_id: ItemId,
    /// Canonical path of the function (for diagnostics).
    pub func_path: String,
    /// Item hierarchy for type lookups (e.g., deriving option payload types).
    pub hierarchy: &'a ItemHierarchy,
}

/// Build a per-function semantic context from the shared context and
/// a MIR function body declaration.
pub fn build_fn_context<'a>(
    shared: &'a SemanticLoweringContext<'a>,
    func: &'a MirFnBodyDecl,
    func_id: ItemId,
    func_def: &FunctionDef,
) -> SemanticFnContext<'a> {
    let fn_ctx = FnContext::new(
        shared.mir_ctx,
        &func.body.local_decls,
        func.body.arg_count,
        &func.receiver,
        &func.params,
        &func.body,
    );

    let has_self_receiver = func.receiver == "ref_mut" || func.receiver == "ref";

    // Determine owner ItemId from FunctionKind.
    let owner_id = match &func_def.kind {
        FunctionKind::InherentMethod { owner } => Some(*owner),
        FunctionKind::TraitMethod { owner, .. } => Some(*owner),
        FunctionKind::FreeFunction => None,
    };

    let param_names: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();

    let local_env = LocalEnvironment::build(
        &func.body.local_decls,
        func.body.arg_count,
        has_self_receiver,
        &param_names,
        owner_id,
        shared.hierarchy,
    );

    SemanticFnContext {
        fn_ctx,
        local_env,
        func_id,
        func_path: func_def.canonical_path.as_str().to_string(),
        hierarchy: shared.hierarchy,
    }
}
