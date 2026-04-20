//! Module model type: structural container for all model items.
//!
//! A [`Module`] corresponds to a Rust module in the driver spec crate and
//! mirrors the spec's source organization (spec.md Section 2). Modules
//! nest arbitrarily through [`Module::submodules`]. Exactly one module
//! in the tree has kind [`ModuleKind::Driver`] and owns the driver
//! struct, Regseq trait impls, and driver impl blocks; all other
//! modules are [`ModuleKind::Normal`] and hold general-purpose items.
//!
//! Per-module dependency sets (`abstractions_used`, `intrinsics_used`)
//! are kept as [`BTreeSet`] values so traversal and emission are
//! deterministic across runs (spec.md Section 10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::identity::{ModuleId, Symbol};
use crate::ir::{RegseqAbstraction, RegseqIntrinsic};
use crate::model::constant::Constant;
use crate::model::driver::DriverStruct;
use crate::model::function::Function;
use crate::model::impl_block::ImplBlock;
use crate::model::register::{RegisterBlock, RegisterDescriptor};
use crate::model::type_def::TypeDef;

/// The kind of a module.
///
/// Exactly one module in the Regseq model tree has kind [`Driver`]
/// (spec.md Section 2). All other modules have kind [`Normal`].
///
/// [`Driver`]: ModuleKind::Driver
/// [`Normal`]: ModuleKind::Normal
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ModuleKind {
    /// Contains the driver struct, Regseq trait impls, and driver impl blocks.
    Driver,
    /// Contains only general-purpose items.
    Normal,
}

/// A module in the Regseq model tree.
///
/// Mirrors the spec source's module organization. Items are grouped by
/// kind (Sections 3.1 through 3.7); driver-only items (Sections 3.8,
/// 3.9, 3.10) are `None`/empty `Vec` in [`ModuleKind::Normal`] modules.
/// Submodules allow arbitrary nesting.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Module {
    /// The module's declaration identifier.
    pub id: ModuleId,
    /// The module's identifier as an interned symbol.
    pub name: Symbol,
    /// Whether this module owns the driver (Driver) or only general-purpose items (Normal).
    pub kind: ModuleKind,

    // --- Items (Section 3) ---
    /// Nested submodules (Section 3.1).
    pub submodules: Vec<Module>,
    /// Top-level constants and statics (Section 3.2).
    pub constants: Vec<Constant>,
    /// Free functions (Section 3.3).
    pub free_functions: Vec<Function>,
    /// Type definitions: structs and enums (Section 3.4).
    pub type_defs: Vec<TypeDef>,
    /// Register descriptors (Section 3.5).
    pub register_descriptors: Vec<RegisterDescriptor>,
    /// Register blocks (Section 3.6).
    pub register_blocks: Vec<RegisterBlock>,
    /// Impl blocks for non-driver types (Section 3.7).
    pub impl_blocks: Vec<ImplBlock>,

    // --- Driver-only items (present when kind == Driver) ---
    /// The driver struct declaration (Section 4). `None` for Normal modules.
    pub driver: Option<DriverStruct>,
    /// Regseq trait impls for the driver (Section 5). Empty for Normal modules.
    pub trait_impls: Vec<ImplBlock>,
    /// Driver impl blocks with helpers and ISRs (Section 6). Empty for Normal modules.
    pub driver_impl_blocks: Vec<ImplBlock>,

    // --- Dependency sets (Section 10) ---
    /// Regseq abstractions referenced within this module.
    pub abstractions_used: BTreeSet<RegseqAbstraction>,
    /// Regseq intrinsic functions invoked within this module.
    pub intrinsics_used: BTreeSet<RegseqIntrinsic>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{
        ConstantId, DeclId, FunctionId, ModuleId, SourceLocation, SourceOrigin, SymbolInterner,
        TypeDefId,
    };
    use crate::ir::{
        BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, RegseqType,
        ScalarType, Terminator, TerminatorKind,
    };
    use crate::model::driver::{DriverIdentity, DriverStruct};
    use crate::model::framework::TypeDefClassification;
    use crate::model::function::Function;
    use crate::model::type_def::{Attrs, TypeDef, TypeDefKind};

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    fn empty_body(interner: &mut SymbolInterner) -> Body {
        let file = interner.intern_file("test.rs");
        let origin = SourceOrigin::Source(SourceLocation {
            file,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
        });
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

    fn empty_module(
        interner: &mut SymbolInterner,
        name: &str,
        id: u32,
        kind: ModuleKind,
    ) -> Module {
        let name_sym = sym(interner, name);
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

    fn free_function(interner: &mut SymbolInterner, fn_id: u32, name: &str) -> Function {
        let fn_name = sym(interner, name);
        let body = empty_body(interner);
        Function {
            id: FunctionId::from(DeclId::new(fn_id)),
            name: fn_name,
            params: vec![],
            return_type: RegseqType::Unit,
            body,
            attrs: Attrs::default(),
        }
    }

    // -- Normal module ---------------------------------------------------

    #[test]
    fn constructs_normal_module_with_type_def_free_function_and_constant() {
        let mut interner = SymbolInterner::new();
        let mut m = empty_module(&mut interner, "util", 1, ModuleKind::Normal);

        let ty_name = sym(&mut interner, "Counter");
        m.type_defs.push(TypeDef {
            id: TypeDefId::from(DeclId::new(2)),
            name: ty_name,
            kind: TypeDefKind::Struct { fields: vec![] },
            classification: TypeDefClassification::UserDefined,
            attrs: Attrs::default(),
        });

        m.free_functions
            .push(free_function(&mut interner, 3, "noop"));

        let const_name = sym(&mut interner, "LIMIT");
        m.constants.push(Constant {
            id: ConstantId::from(DeclId::new(4)),
            name: const_name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(42),
        });

        assert_eq!(m.kind, ModuleKind::Normal);
        assert_eq!(m.type_defs.len(), 1);
        assert_eq!(m.free_functions.len(), 1);
        assert_eq!(m.constants.len(), 1);
        assert!(m.driver.is_none(), "Normal modules have no driver struct");
        assert!(m.trait_impls.is_empty());
        assert!(m.driver_impl_blocks.is_empty());
    }

    // -- Driver module ---------------------------------------------------

    #[test]
    fn constructs_driver_module_with_struct_trait_impl_and_dependencies() {
        let mut interner = SymbolInterner::new();
        let mut m = empty_module(&mut interner, "driver", 10, ModuleKind::Driver);

        let drv_type_id = TypeDefId::from(DeclId::new(11));
        let drv_name = sym(&mut interner, "MyDriver");
        let drv_identity_name = sym(&mut interner, "my-driver");
        m.driver = Some(DriverStruct {
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

        let lifecycle = sym(&mut interner, "Lifecycle");
        m.trait_impls.push(ImplBlock {
            target_type: drv_type_id,
            trait_name: Some(lifecycle),
            methods: vec![
                free_function(&mut interner, 12, "init"),
                free_function(&mut interner, 13, "deinit"),
            ],
            associated_constants: vec![],
        });

        m.driver_impl_blocks.push(ImplBlock {
            target_type: drv_type_id,
            trait_name: None,
            methods: vec![free_function(&mut interner, 14, "program_baud")],
            associated_constants: vec![],
        });

        m.abstractions_used.insert(RegseqAbstraction::Clock);
        m.abstractions_used.insert(RegseqAbstraction::Mmio);
        m.intrinsics_used.insert(RegseqIntrinsic::ClockGetRate);
        m.intrinsics_used.insert(RegseqIntrinsic::RegisterWrite);

        assert_eq!(m.kind, ModuleKind::Driver);
        assert!(m.driver.is_some());
        assert_eq!(m.trait_impls.len(), 1);
        assert_eq!(m.driver_impl_blocks.len(), 1);
        assert!(!m.abstractions_used.is_empty());
        assert!(!m.intrinsics_used.is_empty());
    }

    // -- Module tree: exactly-one-Driver invariant -----------------------

    fn count_driver_modules(m: &Module) -> usize {
        let mine = if m.kind == ModuleKind::Driver { 1 } else { 0 };
        mine + m.submodules.iter().map(count_driver_modules).sum::<usize>()
    }

    #[test]
    fn module_tree_has_exactly_one_driver_module() {
        let mut interner = SymbolInterner::new();

        let driver_child = empty_module(&mut interner, "drv", 101, ModuleKind::Driver);
        let normal_child = empty_module(&mut interner, "util", 102, ModuleKind::Normal);

        let mut root = empty_module(&mut interner, "crate_root", 100, ModuleKind::Normal);
        root.submodules.push(driver_child);
        root.submodules.push(normal_child);

        assert_eq!(root.submodules.len(), 2);
        assert_eq!(
            count_driver_modules(&root),
            1,
            "the tree must contain exactly one Driver module"
        );
    }

    // -- ModuleKind serde round-trip -------------------------------------

    #[test]
    fn module_kind_round_trips_through_json_for_every_variant() {
        for variant in [ModuleKind::Driver, ModuleKind::Normal] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: ModuleKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    #[test]
    fn module_kind_uses_adjacently_tagged_format() {
        let json = serde_json::to_string(&ModuleKind::Driver).unwrap();
        assert!(
            json.contains("\"kind\""),
            "adjacently tagged variants must emit a 'kind' field: {json}",
        );
    }

    // -- BTreeSet determinism --------------------------------------------

    #[test]
    fn abstractions_used_iterates_in_deterministic_order() {
        let mut interner = SymbolInterner::new();
        let mut m = empty_module(&mut interner, "driver", 1, ModuleKind::Driver);
        m.abstractions_used.insert(RegseqAbstraction::Mmio);
        m.abstractions_used.insert(RegseqAbstraction::Clock);
        m.abstractions_used.insert(RegseqAbstraction::Reset);
        m.abstractions_used.insert(RegseqAbstraction::Interrupt);

        let once: Vec<RegseqAbstraction> = m.abstractions_used.iter().copied().collect();
        let twice: Vec<RegseqAbstraction> = m.abstractions_used.iter().copied().collect();
        assert_eq!(
            once, twice,
            "BTreeSet iteration must be deterministic across calls"
        );
        assert_eq!(once.len(), 4);
    }

    #[test]
    fn intrinsics_used_iterates_in_deterministic_order() {
        let mut interner = SymbolInterner::new();
        let mut m = empty_module(&mut interner, "driver", 1, ModuleKind::Driver);
        m.intrinsics_used.insert(RegseqIntrinsic::RegisterWrite);
        m.intrinsics_used.insert(RegseqIntrinsic::ClockEnable);
        m.intrinsics_used.insert(RegseqIntrinsic::BusyWait);

        let once: Vec<RegseqIntrinsic> = m.intrinsics_used.iter().copied().collect();
        let twice: Vec<RegseqIntrinsic> = m.intrinsics_used.iter().copied().collect();
        assert_eq!(once, twice);
        assert_eq!(once.len(), 3);
    }
}
