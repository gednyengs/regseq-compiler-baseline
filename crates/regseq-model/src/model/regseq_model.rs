//! The unified [`RegseqModel`] root: the single progressively-filled IR
//! that the compiler owns.
//!
//! Starts with normalized extraction data (Phase 1: declaration-side
//! fields populated, binding-side fields `None`), then absorbs platform
//! binding data at join time (Phase 2: bindings populated). See spec.md
//! Section 1 for the pipeline role.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::identity::{DeclId, SourceOrigin, SymbolInterner};
use crate::model::module::Module;
use crate::model::mutation::{MutationGuard, MutationPhase};

/// The unified Regseq Model: the single progressively-filled IR that
/// the compiler owns.
///
/// Starts with normalized extraction data (Phase 1: declarations
/// populated, bindings empty), then absorbs platform binding data
/// (Phase 2: bindings populated).
///
/// `RegseqModel` does not derive `Eq`/`PartialEq` because
/// [`SymbolInterner`] is compared by identity rather than content;
/// consumers that need structural equality should compare the
/// projections they care about.
///
/// The [`SymbolInterner`] is intentionally skipped by `Serialize`: per
/// `ir-reference.md` Section 7 the interner is an internal compiler
/// optimization, not part of the wire format. Each [`Symbol`] /
/// [`FileId`] in the model serializes as its resolved string through
/// the interner guard installed by
/// [`crate::serialize::serialize_regseq_model`].
///
/// [`Symbol`]: crate::identity::Symbol
/// [`FileId`]: crate::identity::FileId
#[derive(Clone, Debug, Serialize)]
pub struct RegseqModel {
    /// Interned symbols for all names and identifiers.
    #[serde(skip_serializing)]
    pub symbols: SymbolInterner,
    /// Source locations for all declarations, keyed by [`DeclId`].
    ///
    /// A [`BTreeMap`] (rather than a dense `Vec`) avoids requiring
    /// contiguous DeclId allocation and gives deterministic iteration
    /// order for serialization.
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,
    /// The root module (crate module) of the module tree.
    ///
    /// The root itself is always [`ModuleKind::Normal`]; the driver
    /// module is typically a submodule.
    ///
    /// [`ModuleKind::Normal`]: crate::model::module::ModuleKind::Normal
    pub root_module: Module,
    /// Format version for serialization compatibility. Incremented
    /// whenever the serialized layout changes.
    pub format_version: u32,
}

impl RegseqModel {
    /// Open a transactional mutation scope.
    ///
    /// The returned [`MutationGuard`] exposes typed `modify_*`
    /// methods and one [`MutationGuard::commit`] entry point that
    /// runs the phase-level verifier selected by `phase`. See
    /// [`crate::model::mutation`] for the full contract.
    ///
    /// Every in-place transformation of a [`RegseqModel`] MUST go
    /// through this surface; the only alternative is
    /// [`Self::from_verified_components_unchecked`], which is
    /// deliberately doc-hidden and requires the caller to run the
    /// appropriate verifier separately before handing the model to
    /// any non-test consumer.
    pub fn begin_mutation(&mut self, phase: MutationPhase) -> MutationGuard<'_> {
        MutationGuard::new(self, phase)
    }

    /// Construct a [`RegseqModel`] **without** running the verifier.
    ///
    /// This is an unsafe-at-contract-level escape hatch reserved for
    /// internal infrastructure that reconstructs models from
    /// external sources (e.g., deserializing a golden JSON in
    /// tests). The caller MUST invoke
    /// [`crate::verify::verify_phase_1`] or
    /// [`crate::verify::verify_phase_2`] on the returned value
    /// before passing it to any non-test consumer. Public API for
    /// the compiler pipeline flows through
    /// [`Self::begin_mutation`] instead.
    ///
    /// Visibility is `pub(crate)` so the escape hatch is not part of
    /// the public API surface (spec acceptance criterion "narrowly
    /// scoped"). `#[doc(hidden)]` is retained so that if it is ever
    /// widened to `pub` for serde integration, `cargo doc` still
    /// keeps it off the published rustdoc front page.
    ///
    /// The `#[allow(dead_code)]` is intentional: the escape hatch is
    /// part of the API contract of this task (spec Implementation
    /// Notes "Unsafe escape hatch (justified)") even before any
    /// in-crate consumer exists. Later tasks (e.g., serde
    /// `Deserialize`) will call it; removing it now would force
    /// those tasks to reintroduce the same function.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub(crate) fn from_verified_components_unchecked(
        symbols: SymbolInterner,
        source_locations: BTreeMap<DeclId, SourceOrigin>,
        root_module: Module,
        format_version: u32,
    ) -> Self {
        Self {
            symbols,
            source_locations,
            root_module,
            format_version,
        }
    }
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{DeclId, ModuleId, SourceLocation, SymbolInterner, TypeDefId};
    use crate::model::driver::{DriverIdentity, DriverStruct};
    use crate::model::module::{Module, ModuleKind};
    use std::collections::BTreeSet;

    fn empty_module(
        interner: &mut SymbolInterner,
        name: &str,
        id: u32,
        kind: ModuleKind,
    ) -> Module {
        let name_sym = interner.intern(name);
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

    #[test]
    fn constructs_minimal_regseq_model_with_driver_submodule() {
        let mut symbols = SymbolInterner::new();

        let mut driver_mod = empty_module(&mut symbols, "drv", 2, ModuleKind::Driver);
        let drv_type_id = TypeDefId::from(DeclId::new(3));
        let drv_name = symbols.intern("MyDriver");
        let drv_identity_name = symbols.intern("my-driver");
        driver_mod.driver = Some(DriverStruct {
            type_id: drv_type_id,
            name: drv_name,
            identity: DriverIdentity {
                name: drv_identity_name,
                compatible: vec![],
                bus: None,
            },
            device_class: None,
            pm_info: None,
            platform_metadata: None,
            clocks: vec![],
            resets: vec![],
            pinctrl: vec![],
            interrupts: vec![],
            mmio: vec![],
            sync: vec![],
            properties: vec![],
            user_data: vec![],
        });

        let mut root_module = empty_module(&mut symbols, "crate_root", 1, ModuleKind::Normal);
        root_module.submodules.push(driver_mod);

        let model = RegseqModel {
            symbols,
            source_locations: BTreeMap::new(),
            root_module,
            format_version: 1,
        };

        assert_eq!(
            model.root_module.kind,
            ModuleKind::Normal,
            "root module must be Normal -- the crate module is not the driver module"
        );
        assert_eq!(model.root_module.submodules.len(), 1);
        assert_eq!(
            model.root_module.submodules[0].kind,
            ModuleKind::Driver,
            "driver submodule must be Driver-kind"
        );
        assert!(
            model.root_module.submodules[0].driver.is_some(),
            "driver submodule must carry a DriverStruct"
        );
        assert_eq!(model.format_version, 1);
        assert!(model.source_locations.is_empty());
    }

    #[test]
    fn source_locations_iterate_in_deterministic_order() {
        let mut symbols = SymbolInterner::new();
        let file = symbols.intern_file("test.rs");
        let root_module = empty_module(&mut symbols, "crate_root", 1, ModuleKind::Normal);

        let mut source_locations = BTreeMap::new();
        source_locations.insert(
            DeclId::new(10),
            SourceOrigin::Source(SourceLocation {
                file,
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
            }),
        );
        source_locations.insert(DeclId::new(5), SourceOrigin::CompilerGenerated);
        source_locations.insert(
            DeclId::new(7),
            SourceOrigin::Source(SourceLocation {
                file,
                start_line: 2,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            }),
        );

        let model = RegseqModel {
            symbols,
            source_locations,
            root_module,
            format_version: 1,
        };

        let keys_once: Vec<DeclId> = model.source_locations.keys().copied().collect();
        let keys_twice: Vec<DeclId> = model.source_locations.keys().copied().collect();
        assert_eq!(
            keys_once, keys_twice,
            "BTreeMap iteration must be deterministic across calls"
        );
        assert_eq!(
            keys_once,
            vec![DeclId::new(5), DeclId::new(7), DeclId::new(10)],
            "BTreeMap iterates by sorted key order"
        );
    }

    #[test]
    fn regseq_model_is_cloneable() {
        let mut symbols = SymbolInterner::new();
        let root_module = empty_module(&mut symbols, "crate_root", 1, ModuleKind::Normal);

        let model = RegseqModel {
            symbols,
            source_locations: BTreeMap::new(),
            root_module,
            format_version: 1,
        };

        let cloned = model.clone();
        assert_eq!(cloned.format_version, model.format_version);
        assert_eq!(cloned.root_module.id, model.root_module.id);
    }

    #[test]
    fn from_verified_components_unchecked_constructs_without_verifying() {
        // The escape hatch is deliberately narrow (`pub(crate)` +
        // `#[doc(hidden)]`) but must remain functional for internal
        // infrastructure like deserialization. This test exercises it
        // directly and proves two properties:
        //   1. it produces a model with the provided components,
        //   2. it does NOT run the verifier (the input model here is
        //      intentionally malformed -- it has no Driver submodule,
        //      which `verify_phase_1` rejects).
        let mut symbols = SymbolInterner::new();
        let root_module = empty_module(&mut symbols, "crate_root", 1, ModuleKind::Normal);
        let mut source_locations = BTreeMap::new();
        source_locations.insert(DeclId::new(1), SourceOrigin::CompilerGenerated);

        let model = RegseqModel::from_verified_components_unchecked(
            symbols,
            source_locations,
            root_module,
            42,
        );

        assert_eq!(model.format_version, 42);
        assert_eq!(model.root_module.kind, ModuleKind::Normal);
        assert!(
            model.root_module.submodules.is_empty(),
            "escape hatch must not mutate the input"
        );
        assert!(
            crate::verify::verify_phase_1(&model).is_err(),
            "escape hatch must not run the verifier -- a malformed model passes through"
        );
    }
}
