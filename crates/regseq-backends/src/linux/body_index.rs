//! Borrowed-body index keyed by `FunctionId`.
//!
//! `FunctionBodyIndex<'a>` is the single canonical body-access contract
//! between the Linux planner / emitter and the underlying
//! [`RegseqModel`](regseq_model::model::RegseqModel). It wraps a
//! [`BTreeMap`] of borrowed `ir::Body` references so the backend can
//! look up a function's CFG body by `FunctionId` without copying any
//! body data.
//!
//! The index is constructed once from a `&RegseqModel` via
//! [`FunctionBodyIndex::from_regseq_model`]. Future emitters can query
//! it through [`FunctionBodyIndex::get`]; the index does not mutate
//! and it never copies bodies.

use std::collections::BTreeMap;

use regseq_model::identity::FunctionId;
use regseq_model::ir::Body;
use regseq_model::model::{Module, RegseqModel};

/// Borrowed-body lookup by [`FunctionId`].
///
/// Holds `&'a ir::Body` references into an externally-owned
/// [`RegseqModel`]. The lifetime `'a` is the lifetime of the borrowed
/// model; the index itself owns no body storage.
#[derive(Debug, Clone)]
pub struct FunctionBodyIndex<'a> {
    inner: BTreeMap<FunctionId, &'a Body>,
}

impl<'a> FunctionBodyIndex<'a> {
    /// Walk a [`RegseqModel`] and index every function body by its
    /// [`FunctionId`].
    ///
    /// Visits free functions plus every method in impl blocks, trait
    /// impls, and driver impl blocks across the full module tree.
    /// The traversal is deterministic -- it follows the model's
    /// declaration order.
    pub fn from_regseq_model(model: &'a RegseqModel) -> Self {
        let mut inner = BTreeMap::new();
        walk_module(&model.root_module, &mut inner);
        Self { inner }
    }

    /// Look up the body for a given [`FunctionId`].
    ///
    /// Returns `Some(&Body)` if the index knows about the function,
    /// `None` otherwise (e.g. when the index was built from a model
    /// that does not carry that function, or the default empty index
    /// was used).
    pub fn get(&self, id: FunctionId) -> Option<&'a Body> {
        self.inner.get(&id).copied()
    }

    /// Number of indexed function bodies.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// True when the index has no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for FunctionBodyIndex<'_> {
    /// An empty index. Useful for call sites that do not yet have a
    /// [`RegseqModel`] available. Production call sites should
    /// construct the index via [`FunctionBodyIndex::from_regseq_model`].
    fn default() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }
}

fn walk_module<'a>(module: &'a Module, out: &mut BTreeMap<FunctionId, &'a Body>) {
    for func in &module.free_functions {
        out.insert(func.id, &func.body);
    }
    for ib in &module.impl_blocks {
        for func in &ib.methods {
            out.insert(func.id, &func.body);
        }
    }
    for ib in &module.trait_impls {
        for func in &ib.methods {
            out.insert(func.id, &func.body);
        }
    }
    for ib in &module.driver_impl_blocks {
        for func in &ib.methods {
            out.insert(func.id, &func.body);
        }
    }
    for submodule in &module.submodules {
        walk_module(submodule, out);
    }
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use regseq_model::identity::{DeclId, SourceLocation, SourceOrigin, SymbolInterner};
    use regseq_model::ir::{
        BasicBlock, BasicBlockSourceMap, BodySourceMap, LocalDecl, Mutability, RegseqType,
        ScalarType, Terminator, TerminatorKind,
    };
    use regseq_model::model::function::{Function, Param, ParamKind};
    use regseq_model::model::impl_block::ImplBlock;
    use regseq_model::model::module::{Module, ModuleKind};
    use regseq_model::model::type_def::Attrs;
    use std::collections::BTreeSet;

    fn mk_origin(interner: &mut SymbolInterner) -> SourceOrigin {
        let file = interner.intern_file("t.rs");
        SourceOrigin::Source(SourceLocation {
            file,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
        })
    }

    fn mk_body(origin: SourceOrigin) -> Body {
        Body {
            local_decls: vec![LocalDecl {
                ty: RegseqType::Unit,
                mutable: true,
                name: None,
            }],
            arg_count: 0,
            basic_blocks: vec![BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                },
            }],
            source_map: BodySourceMap {
                body_span: origin,
                locals: vec![SourceOrigin::CompilerGenerated],
                blocks: vec![BasicBlockSourceMap {
                    statements: vec![],
                    terminator: SourceOrigin::CompilerGenerated,
                }],
            },
            ordered_regions: vec![],
        }
    }

    fn mk_module(interner: &mut SymbolInterner, name: &str, id: u32, kind: ModuleKind) -> Module {
        let name_sym = interner.intern(name);
        Module {
            id: regseq_model::identity::ModuleId::from(DeclId::new(id)),
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

    fn mk_function(interner: &mut SymbolInterner, name: &str, id: u32) -> Function {
        let fn_name = interner.intern(name);
        let origin = mk_origin(interner);
        Function {
            id: FunctionId::from(DeclId::new(id)),
            name: fn_name,
            params: vec![],
            return_type: RegseqType::Unit,
            body: mk_body(origin),
            attrs: Attrs::default(),
        }
    }

    fn mk_receiver_param(interner: &mut SymbolInterner, struct_id: u32) -> Param {
        let self_name = interner.intern("self");
        Param {
            kind: ParamKind::Receiver,
            name: self_name,
            ty: RegseqType::Ref(
                Box::new(RegseqType::Defined(
                    regseq_model::identity::TypeDefId::from(DeclId::new(struct_id)),
                )),
                Mutability::Shared,
            ),
        }
    }

    fn mk_method(interner: &mut SymbolInterner, name: &str, id: u32, struct_id: u32) -> Function {
        let fn_name = interner.intern(name);
        let origin = mk_origin(interner);
        Function {
            id: FunctionId::from(DeclId::new(id)),
            name: fn_name,
            params: vec![mk_receiver_param(interner, struct_id)],
            return_type: RegseqType::Scalar(ScalarType::U32),
            body: mk_body(origin),
            attrs: Attrs::default(),
        }
    }

    #[test]
    fn default_is_empty() {
        let idx = FunctionBodyIndex::default();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert!(idx.get(FunctionId::from(DeclId::new(99))).is_none());
    }

    #[test]
    fn from_regseq_model_indexes_free_functions() {
        let mut interner = SymbolInterner::new();
        let mut root = mk_module(&mut interner, "crate_root", 1, ModuleKind::Normal);
        root.free_functions
            .push(mk_function(&mut interner, "f", 10));
        root.free_functions
            .push(mk_function(&mut interner, "g", 11));

        let model = RegseqModel {
            symbols: interner,
            source_locations: BTreeMap::new(),
            root_module: root,
            format_version: 1,
        };

        let idx = FunctionBodyIndex::from_regseq_model(&model);
        assert_eq!(idx.len(), 2);
        assert!(idx.get(FunctionId::from(DeclId::new(10))).is_some());
        assert!(idx.get(FunctionId::from(DeclId::new(11))).is_some());
        assert!(idx.get(FunctionId::from(DeclId::new(12))).is_none());
    }

    #[test]
    fn from_regseq_model_indexes_impl_methods_and_submodules() {
        let mut interner = SymbolInterner::new();
        let struct_id = regseq_model::identity::TypeDefId::from(DeclId::new(20));

        let mut sub = mk_module(&mut interner, "drv", 2, ModuleKind::Normal);
        sub.impl_blocks.push(ImplBlock {
            target_type: struct_id,
            trait_name: None,
            methods: vec![mk_method(&mut interner, "read", 40, 20)],
            associated_constants: vec![],
        });
        sub.driver_impl_blocks.push(ImplBlock {
            target_type: struct_id,
            trait_name: None,
            methods: vec![mk_method(&mut interner, "probe", 41, 20)],
            associated_constants: vec![],
        });

        let mut root = mk_module(&mut interner, "crate_root", 1, ModuleKind::Normal);
        root.submodules.push(sub);

        let model = RegseqModel {
            symbols: interner,
            source_locations: BTreeMap::new(),
            root_module: root,
            format_version: 1,
        };

        let idx = FunctionBodyIndex::from_regseq_model(&model);
        assert_eq!(idx.len(), 2);
        assert!(idx.get(FunctionId::from(DeclId::new(40))).is_some());
        assert!(idx.get(FunctionId::from(DeclId::new(41))).is_some());
    }
}
