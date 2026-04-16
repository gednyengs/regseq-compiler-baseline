//! The unified [`RegseqModel`] root: the single progressively-filled IR
//! that the compiler owns.
//!
//! Starts with normalized extraction data (Phase 1: declaration-side
//! fields populated, binding-side fields `None`), then absorbs platform
//! binding data at join time (Phase 2: bindings populated). See spec.md
//! Section 1 for the pipeline role.

use std::collections::BTreeMap;

use crate::identity::{DeclId, SourceOrigin, SymbolInterner};
use crate::model::module::Module;

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
#[derive(Clone, Debug)]
pub struct RegseqModel {
    /// Interned symbols for all names and identifiers.
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

#[cfg(test)]
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
}
