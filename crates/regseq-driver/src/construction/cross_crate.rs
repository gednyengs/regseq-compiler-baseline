//! Rule 3 cross-crate function extraction for Pass 1.
//!
//! Implemented by task regseq-model-47.
//!
//! Spec code is `#![no_std]` -- only `core` is permitted -- and per
//! `spec.md` Section 1, "monomorphized `core` functions (e.g.,
//! `Option::map`, `Option::unwrap_or`) are available in MIR at the
//! extraction stage and are extracted as regular functions." This
//! module walks every `Call` terminator reachable from an
//! already-registered authored function body and registers every
//! referenced `core` or `regseq` callee as a regular [`Function`]
//! under a synthetic module chain rooted at the callee's defining
//! crate name (e.g., `core::option::Option::map` -> synthetic
//! `core` -> `core::option` -> `core::option::Option` -> function
//! `map`).
//!
//! Call sites whose callee belongs to a crate outside the allowlist
//! (`core`, `regseq`) raise
//! [`ConstructionError::StdFunctionNotPermitted`]. Transitive
//! traversal is bounded by [`MAX_CROSS_CRATE_DEPTH`]; exceeding the
//! bound raises [`ConstructionError::CrossCrateRecursionLimit`].
//!
//! Only identity-level work happens here: a [`FunctionId`] is
//! assigned and the function's signature + span are recorded, while
//! the Pass 1 placeholder body is left in place. Pass 2's
//! [`build_body`](super::build_body) later replaces the placeholder
//! with the translated MIR -- cross-crate functions go through the
//! exact same body-construction pipeline as authored bodies.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use rustc_hir::def_id::{CrateNum, DefId, LOCAL_CRATE};
use rustc_middle::mir;
use rustc_middle::ty::{self, Instance, TyCtxt, TypingEnv};

use regseq_model::identity::{DeclId, FunctionId, ModuleId, SourceOrigin, Symbol, SymbolInterner};
use regseq_model::ir::RegseqType;
use regseq_model::model::{Function, Module, ModuleKind};

use super::id_allocator::DeclIdAllocator;
use super::walk::{empty_body, function_signature, BuildContext};
use super::{ConstructionError, ErrorProvenance};

/// Upper bound on the depth of the cross-crate call-graph walk
/// (counted as allowlisted-edge traversals from any authored-body
/// seed). 16 is generous for Regseq specs -- in practice the
/// PL011 fixture reaches depth 2-3 -- and still catches runaway
/// recursion from malformed MIR.
pub const MAX_CROSS_CRATE_DEPTH: u32 = 16;

/// Crates allowed as cross-crate Call-target sources. Any referenced
/// callee whose defining crate name is outside this list raises
/// [`ConstructionError::StdFunctionNotPermitted`].
pub const ALLOWED_CROSS_CRATES: &[&str] = &["core", "regseq"];

/// Run the Rule 3 cross-crate call-graph walk.
///
/// Seed the worklist with every local-crate function already
/// registered in `cx.function_map`. For each, fetch
/// `tcx.optimized_mir(def_id)` and scan every `Call` terminator:
///
/// - Callees in the local crate are ignored (already handled by the
///   module walk).
/// - Callees whose defining crate is in [`ALLOWED_CROSS_CRATES`] are
///   registered as new [`Function`]s under a synthetic module chain
///   rooted at the callee's crate name, and appended to the
///   worklist (transitive walk).
/// - Callees from any other crate raise
///   [`ConstructionError::StdFunctionNotPermitted`].
///
/// Synthetic modules are attached as submodules of `root_module`.
/// Each synthetic module has `kind: ModuleKind::Normal` and
/// `source: SourceOrigin::CompilerGenerated`. Each cross-crate
/// function's own `source` is resolved from `tcx.def_span(def_id)`.
pub fn extract_cross_crate_functions(
    cx: &mut BuildContext<'_, '_>,
    root_module: &mut Module,
) -> Result<(), ConstructionError> {
    extract_cross_crate_functions_with_depth_bound(cx, root_module, MAX_CROSS_CRATE_DEPTH)
}

/// Variant of [`extract_cross_crate_functions`] that accepts an
/// explicit depth bound. The production entry point calls this
/// with [`MAX_CROSS_CRATE_DEPTH`]; integration tests use it to
/// exercise the [`ConstructionError::CrossCrateRecursionLimit`]
/// error path without synthesizing a 17-deep cross-crate call
/// chain through real `core` MIR (infeasible in a stand-alone
/// specimen).
pub fn extract_cross_crate_functions_with_depth_bound(
    cx: &mut BuildContext<'_, '_>,
    root_module: &mut Module,
    max_depth: u32,
) -> Result<(), ConstructionError> {
    // Seed the worklist with all currently-registered authored
    // functions. Depth 0 is the seed row; each cross-crate jump
    // increments depth by 1.
    //
    // Sort by `def_path_str` so the walk order is stable across
    // runs: `cx.function_map` is a `HashMap`, and feeding its raw
    // iteration order into the worklist lets the DeclIdAllocator
    // assign different synthetic-module IDs to the same set of
    // cross-crate callees on repeat runs. Phase 5's golden-file
    // contract requires byte-identical output across runs, so the
    // order must be deterministic here.
    let mut seed_ids: Vec<DefId> = cx.function_map.keys().copied().collect();
    seed_ids.sort_by_cached_key(|did| cx.tcx.def_path_str(*did));
    let mut seen: HashSet<DefId> = seed_ids.iter().copied().collect();
    let mut worklist: VecDeque<(DefId, u32)> = seed_ids.into_iter().map(|d| (d, 0)).collect();

    // Track already-materialized synthetic modules by their
    // (crate-name-relative) path of interned segments. Every
    // intermediate module node is reused across callees that share
    // a prefix.
    let mut synth_index: SyntheticModuleIndex = SyntheticModuleIndex::default();

    while let Some((caller_def_id, depth)) = worklist.pop_front() {
        if depth > max_depth {
            return Err(ConstructionError::CrossCrateRecursionLimit {
                provenance: ErrorProvenance::for_def_id(cx.tcx, caller_def_id),
                depth,
            });
        }
        if !cx.tcx.is_mir_available(caller_def_id) {
            continue;
        }
        let body = cx.tcx.optimized_mir(caller_def_id);
        for data in body.basic_blocks.iter() {
            let Some(term) = data.terminator.as_ref() else {
                continue;
            };
            let mir::TerminatorKind::Call { func, .. } = &term.kind else {
                continue;
            };
            let Some((raw_def_id, generic_args)) = extract_fn_def(func) else {
                // Pass 2 rejects non-FnDef Call targets via
                // `UnresolvedCallTarget`; Pass 1's call-graph walk
                // simply skips them (they don't pull in any new
                // Function entries).
                continue;
            };

            // `Instance::resolve` is the spec's prescribed way to
            // turn a MIR `FnDef(def_id, args)` into the concrete
            // instance a caller will dispatch to -- for trait
            // methods, that swaps the trait's DefId for the
            // applicable impl method's DefId. We run it here both
            // to check that the resolved instance is allowlisted
            // (so trait calls routed to a local impl are
            // correctly classified as local, not core) and to
            // produce a sharper error provenance when a std
            // function is reached through a trait.
            //
            // Uses the caller's `TypingEnv` (derived from
            // `tcx.param_env(caller_def_id)`) per the spec's
            // "`Instance::resolve(tcx, param_env, ...)`" step, so
            // a generic caller's trait-bound substitutions still
            // resolve.
            //
            // The registration key itself is the raw FnDef DefId:
            // Pass 2's terminator translation (see
            // `construction::pass2::resolve_callee_def_id`) also
            // works with the raw FnDef DefId, so both passes
            // agree on what the call's identity is.
            let resolved_def_id =
                resolve_instance_def_id(cx.tcx, caller_def_id, raw_def_id, generic_args)
                    .unwrap_or(raw_def_id);

            // If the resolved instance points at a local method,
            // the method was already registered by Pass 1's
            // impl-block walker. Record the raw DefId as an
            // alias so Pass 2's lookup by the trait DefId finds
            // the local impl's FunctionId.
            if resolved_def_id.krate == LOCAL_CRATE {
                if raw_def_id != resolved_def_id {
                    if let Some(local_fn_id) = cx.function_map.get(&resolved_def_id).copied() {
                        cx.function_map.insert(raw_def_id, local_fn_id);
                        seen.insert(raw_def_id);
                    }
                }
                continue;
            }

            // Otherwise: cross-crate callee. Check the allowlist
            // on the raw DefId's crate -- that's the one Pass 2
            // will ask about. `resolved_def_id.krate` may equal
            // `raw_def_id.krate` in practice (both in `core`),
            // but we intentionally don't couple the allowlist
            // check to instance resolution: a trait's DefId is
            // the callee surface Pass 2 sees.
            if !is_allowed_cross_crate(cx.tcx, raw_def_id.krate) {
                return Err(ConstructionError::StdFunctionNotPermitted {
                    provenance: ErrorProvenance::for_def_id(cx.tcx, raw_def_id),
                    path: cx.tcx.def_path_str(raw_def_id),
                    caller: cx.tcx.def_path_str(caller_def_id),
                });
            }
            if !seen.insert(raw_def_id) {
                continue;
            }

            register_cross_crate_function(cx, root_module, &mut synth_index, raw_def_id)?;

            worklist.push_back((raw_def_id, depth + 1));
        }
    }

    Ok(())
}

/// Tracks synthetic module nodes materialized under `root_module`.
/// Keyed by crate-name-relative path (interned segment names joined
/// by `::`) so later cross-crate functions reuse existing chains.
#[derive(Default)]
struct SyntheticModuleIndex {
    nodes: HashMap<Vec<Symbol>, ModuleId>,
}

impl SyntheticModuleIndex {
    fn get(&self, path: &[Symbol]) -> Option<ModuleId> {
        self.nodes.get(path).copied()
    }

    fn insert(&mut self, path: Vec<Symbol>, id: ModuleId) {
        self.nodes.insert(path, id);
    }
}

/// Register a single cross-crate callee by:
///
/// 1. Resolving its synthetic module path via [`synthetic_path_for`].
/// 2. Walking down `root_module.submodules` and creating any missing
///    synthetic modules along the path.
/// 3. Allocating a [`FunctionId`], recording the callee's span,
///    building a signature + placeholder body, and appending the
///    resulting [`Function`] to the leaf synthetic module's
///    `free_functions`.
/// 4. Inserting `(def_id -> function_id)` into `cx.function_map` so
///    Pass 2's [`super::FunctionIndex`] can resolve the callee.
fn register_cross_crate_function(
    cx: &mut BuildContext<'_, '_>,
    root_module: &mut Module,
    synth_index: &mut SyntheticModuleIndex,
    def_id: DefId,
) -> Result<(), ConstructionError> {
    let (module_segments, fn_name) = synthetic_path_for(cx.tcx, def_id);

    let parent_module = if module_segments.is_empty() {
        root_module
    } else {
        ensure_synthetic_module_chain(cx, root_module, synth_index, &module_segments)
    };

    let function = build_cross_crate_function_stub(cx, def_id, &fn_name)?;
    parent_module.free_functions.push(function);
    Ok(())
}

/// Navigate `root_module.submodules` along `segments`, creating any
/// missing synthetic [`Module`] nodes with
/// `kind: ModuleKind::Normal` and `source: CompilerGenerated`.
/// Returns a mutable reference to the leaf module so the caller can
/// append the cross-crate function to it.
fn ensure_synthetic_module_chain<'a>(
    cx: &mut BuildContext<'_, '_>,
    root_module: &'a mut Module,
    synth_index: &mut SyntheticModuleIndex,
    segments: &[String],
) -> &'a mut Module {
    let mut interned_path: Vec<Symbol> = Vec::with_capacity(segments.len());
    let mut current: &mut Module = root_module;

    for segment in segments {
        let segment_sym = cx.symbols.intern(segment);
        interned_path.push(segment_sym);

        let existing_id = synth_index.get(&interned_path);
        match existing_id {
            Some(id) => {
                let pos = current
                    .submodules
                    .iter()
                    .position(|m| m.id == id)
                    .expect("synth_index and submodules must agree on ModuleId");
                current = &mut current.submodules[pos];
            }
            None => {
                let module_id = ModuleId::from(cx.allocator.alloc());
                cx.source_locations
                    .insert(module_id.decl_id(), SourceOrigin::CompilerGenerated);

                let new_module = Module {
                    id: module_id,
                    name: segment_sym,
                    kind: ModuleKind::Normal,
                    submodules: Vec::new(),
                    constants: Vec::new(),
                    free_functions: Vec::new(),
                    type_defs: Vec::new(),
                    register_descriptors: Vec::new(),
                    register_blocks: Vec::new(),
                    impl_blocks: Vec::new(),
                    driver: None,
                    trait_impls: Vec::new(),
                    driver_impl_blocks: Vec::new(),
                    abstractions_used: std::collections::BTreeSet::new(),
                    intrinsics_used: std::collections::BTreeSet::new(),
                };
                current.submodules.push(new_module);
                synth_index.insert(interned_path.clone(), module_id);
                let pos = current.submodules.len() - 1;
                current = &mut current.submodules[pos];
            }
        }
    }

    current
}

/// Build a placeholder [`Function`] for a cross-crate callee.
///
/// Signature is derived via the same `function_signature` helper
/// used by authored bodies. Body is an `empty_body` placeholder
/// whose terminator is `Unreachable` -- Pass 2's
/// [`build_body`](super::build_body) replaces it with the
/// translated MIR.
fn build_cross_crate_function_stub(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
) -> Result<Function, ConstructionError> {
    let id = FunctionId::from(alloc_and_record_span(
        cx.tcx,
        cx.symbols,
        cx.source_locations,
        cx.allocator,
        def_id,
    ));
    cx.function_map.insert(def_id, id);

    let name_sym = cx.symbols.intern(name);
    let (params, return_type) = cross_crate_function_signature(cx, def_id);
    let body = empty_body(cx.tcx, cx.symbols, def_id, params.len());

    Ok(Function {
        id,
        name: name_sym,
        params,
        return_type,
        body,
        attrs: Default::default(),
    })
}

/// Resolve a cross-crate callee's signature. Mirrors
/// [`super::walk::function_signature`] for non-foreign items but
/// falls back to a best-effort `Unit`-only signature when rustc
/// refuses to give us a normal `fn_sig` (closures, shims, etc.).
fn cross_crate_function_signature(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
) -> (Vec<regseq_model::model::Param>, RegseqType) {
    // For ordinary free/associated functions `fn_sig` is available
    // cross-crate too. Reuse the walker's helper so receiver-kind
    // detection and type mapping stay in one place.
    let _ = def_id;
    function_signature(cx, def_id)
}

/// Sibling of [`BuildContext::alloc_and_record_public`] that does
/// not require holding a `&mut BuildContext` -- the caller already
/// has exclusive mutable access to each of its fields. Used when
/// allocating a [`DeclId`] for a synthetic cross-crate item.
fn alloc_and_record_span(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    source_locations: &mut BTreeMap<DeclId, SourceOrigin>,
    allocator: &mut DeclIdAllocator,
    def_id: DefId,
) -> DeclId {
    let id = allocator.alloc();
    super::record_span(tcx, symbols, source_locations, id, tcx.def_span(def_id));
    id
}

/// Extract the callee's [`DefId`] and [`ty::GenericArgsRef`] from a
/// MIR `Call::func` operand. Returns `None` for indirect calls
/// (function pointers) and other shapes that Pass 1 cross-crate
/// extraction cannot resolve statically -- Pass 2 will surface
/// those as [`ConstructionError::UnresolvedCallTarget`] at
/// translation time.
fn extract_fn_def<'tcx>(func: &mir::Operand<'tcx>) -> Option<(DefId, ty::GenericArgsRef<'tcx>)> {
    let mir::Operand::Constant(const_op) = func else {
        return None;
    };
    let mir::Const::Val(_, ty) = const_op.const_ else {
        return None;
    };
    if let ty::TyKind::FnDef(def_id, args) = ty.kind() {
        return Some((*def_id, *args));
    }
    None
}

/// Use [`Instance::resolve`] to map a MIR `FnDef(def_id, args)` to
/// its concrete instance's [`DefId`]. For trait methods, this
/// swaps the trait's `def_id` for the applicable impl method's
/// `def_id`; for inherent methods and free functions, the result
/// equals the input.
///
/// Uses the caller's [`TypingEnv`] (derived from
/// `tcx.param_env(caller_def_id)`) so that trait-bound
/// substitutions from the caller's generics are honored during
/// resolution -- matching the spec's
/// "`Instance::resolve(tcx, param_env, callee_def_id, generic_args)`"
/// wording. Returns `None` when rustc cannot resolve the instance
/// (open generics, too-indirect shapes); callers fall back to the
/// raw `def_id`.
fn resolve_instance_def_id<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller_def_id: DefId,
    def_id: DefId,
    args: ty::GenericArgsRef<'tcx>,
) -> Option<DefId> {
    // `TypingEnv::non_body_analysis` pairs the caller's
    // `ParamEnv` (obtained via `tcx.param_env(caller_def_id)`)
    // with a post-analysis typing mode. Using the caller's
    // param-env rather than `fully_monomorphized` honors any
    // trait bounds on the caller's own generics so
    // `Instance::try_resolve` can route trait methods to the
    // applicable impl.
    let typing_env = TypingEnv::non_body_analysis(tcx, caller_def_id);
    let result = Instance::try_resolve(tcx, typing_env, def_id, args).ok()??;
    Some(result.def_id())
}

/// Resolve a crate to whether it is in the cross-crate allowlist.
/// LOCAL_CRATE is excluded -- the walker short-circuits that case
/// before calling this helper.
fn is_allowed_cross_crate(tcx: TyCtxt<'_>, krate: CrateNum) -> bool {
    let name = tcx.crate_name(krate);
    ALLOWED_CROSS_CRATES.contains(&name.as_str())
}

/// Compute the synthetic module path (crate-relative segment names)
/// and function name for a cross-crate callee.
///
/// Starts from `def_path_str(def_id)`, which renders as a `::`
/// separated path with embedded generic-argument segments like
/// `"core::option::Option::<Duration>::map"`. The returned split
/// collapses generic-arg segments (`<...>`) into their preceding
/// segment's identifier form -- the spec stipulates that
/// `core::option::Option::<Duration>::map` lives under
/// `core::option::Option`, not `core::option::Option::<Duration>`,
/// because generic arguments are part of the function's identity
/// and not of its module placement.
///
/// The last remaining segment is the function name; all preceding
/// segments form the synthetic module chain.
fn synthetic_path_for(tcx: TyCtxt<'_>, def_id: DefId) -> (Vec<String>, String) {
    let raw = tcx.def_path_str(def_id);
    let mut segments: Vec<String> = split_path_segments(&raw);
    // Drop the last segment as the function name. A well-formed
    // def_path_str always has at least one non-angle-bracketed
    // segment (the item name); guard against an empty result by
    // synthesizing a placeholder name.
    let fn_name = segments
        .pop()
        .unwrap_or_else(|| format!("cross_crate_item_{}", def_id.index.as_u32()));

    // Canonicalize `alloc::*` or other re-export display forms
    // into the defining-crate prefix. `def_path_str` prefers the
    // public re-export site (e.g., `std::vec::Vec` for a crate
    // built with std). Replace the first segment with the actual
    // defining crate name so the synthetic tree mirrors the
    // allowlist decision above.
    if let Some(first) = segments.first_mut() {
        let defining = tcx.crate_name(def_id.krate).as_str().to_string();
        if *first != defining {
            *first = defining;
        }
    } else {
        // No module segments at all (e.g., a free function at the
        // crate root): synthesize the crate name as the sole
        // parent module so the function does not float into the
        // local crate's root_module.
        segments.push(tcx.crate_name(def_id.krate).as_str().to_string());
    }

    (segments, fn_name)
}

/// Split a `def_path_str`-rendered path into segments, collapsing
/// generic-argument fragments (those wholly wrapped in `<...>`)
/// into their preceding segment's textual form so downstream
/// segmentation sees only item-level names.
fn split_path_segments(raw: &str) -> Vec<String> {
    // Walk the input tracking angle-bracket depth. Split only when
    // we see `::` at depth 0. `::` inside `<Foo as Bar>::item`
    // remains inside the surrounding segment.
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '<' {
            depth += 1;
            current.push(c);
            i += 1;
        } else if c == '>' {
            depth -= 1;
            current.push(c);
            i += 1;
        } else if c == ':' && depth == 0 && i + 1 < chars.len() && chars[i + 1] == ':' {
            // Segment separator at depth 0.
            out.push(std::mem::take(&mut current));
            i += 2;
        } else {
            current.push(c);
            i += 1;
        }
    }
    if !current.is_empty() {
        out.push(current);
    }

    // Collapse any pure `<...>` segment (a bare generic-args slot)
    // into its preceding segment so `Option::<Duration>::map` does
    // not split into three tokens. In practice `def_path_str`
    // mainly emits these for explicit `<T>` annotations on type
    // paths.
    let mut collapsed: Vec<String> = Vec::with_capacity(out.len());
    for seg in out {
        let trimmed = seg.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            if let Some(prev) = collapsed.last_mut() {
                prev.push_str("::");
                prev.push_str(trimmed);
                continue;
            }
        }
        collapsed.push(seg);
    }

    // Finally strip the generic-arg tails from every segment so the
    // synthetic module name is a bare identifier (`Option`, not
    // `Option::<Duration>`). The function name at the tail
    // preserves the tail segment verbatim so callers can
    // distinguish monomorphizations by name when desired.
    collapsed
        .into_iter()
        .map(|mut s| {
            if let Some(idx) = s.find("::<") {
                s.truncate(idx);
            }
            // Also drop a leading generic-binding prefix like
            // `<FieldValue as Add>` that can introduce a stray
            // empty or noisy segment. For such shapes we keep
            // the segment as-is; downstream grouping tolerates
            // non-identifier characters since the name is only
            // used for interning.
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_plain_path() {
        let segs = split_path_segments("core::option::Option::map");
        assert_eq!(segs, vec!["core", "option", "Option", "map"]);
    }

    #[test]
    fn split_collapses_inline_generic_arg_on_final_segment() {
        // `"core::option::Option::<Duration>::map"`: the
        // `<Duration>` belongs to `Option`, and the result must
        // preserve a trailing `map` as the function name.
        let segs = split_path_segments("core::option::Option::<Duration>::map");
        assert_eq!(segs, vec!["core", "option", "Option", "map"]);
    }

    #[test]
    fn split_keeps_qualified_path_inside_angle_brackets_together() {
        // `<FieldValue as core::ops::Add>::add` must stay as two
        // segments: the `<...>` receiver-spec and the `add` method.
        let segs = split_path_segments("<FieldValue as core::ops::Add>::add");
        assert_eq!(segs, vec!["<FieldValue as core::ops::Add>", "add"]);
    }

    #[test]
    fn allowlist_contains_core_and_regseq() {
        assert!(ALLOWED_CROSS_CRATES.contains(&"core"));
        assert!(ALLOWED_CROSS_CRATES.contains(&"regseq"));
        assert!(!ALLOWED_CROSS_CRATES.contains(&"std"));
        assert!(!ALLOWED_CROSS_CRATES.contains(&"alloc"));
    }

    #[test]
    fn synthetic_index_reuses_existing_entries() {
        // Smoke check the path-indexing data structure so future
        // refactors cannot silently drop the de-duplication
        // property that makes synthetic module placement
        // deterministic.
        let mut idx = SyntheticModuleIndex::default();
        let mut interner = SymbolInterner::new();
        let core = interner.intern("core");
        let option = interner.intern("option");
        let path = vec![core, option];

        let id = ModuleId::from(DeclId::new(7));
        assert!(idx.get(&path).is_none());
        idx.insert(path.clone(), id);
        assert_eq!(idx.get(&path), Some(id));
    }

    #[test]
    fn depth_bound_is_at_least_sixteen() {
        // The spec fixes the bound at 16 cross-crate edges from a
        // seed. Guard the constant so future refactors don't
        // silently relax the limit.
        assert_eq!(MAX_CROSS_CRATE_DEPTH, 16);
    }
}
