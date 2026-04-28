//! Impl block model type for inherent and trait impl blocks.
//!
//! A single [`ImplBlock`] type represents all four impl-block use sites:
//! - Inherent impl blocks on non-driver types (spec.md Section 3.7).
//! - Trait impl blocks on non-driver types (spec.md Section 3.7).
//! - Regseq trait impls for the driver struct (spec.md Section 5).
//! - Driver impl blocks providing helpers and ISRs (spec.md Section 6).
//!
//! The distinction between these use sites comes from structural position
//! in the containing [`Module`], not from the [`ImplBlock`] type itself.

use serde::Serialize;

use crate::identity::{Symbol, TypeDefId};
use crate::model::constant::Constant;
use crate::model::function::Function;

/// An impl block: inherent or trait implementation.
///
/// Used for:
/// - Inherent impl blocks on non-driver types (Section 3.7)
/// - Trait impl blocks on non-driver types (Section 3.7)
/// - Regseq trait impls for the driver struct (Section 5)
/// - Driver impl blocks with helpers/ISRs (Section 6)
///
/// The distinction between these use sites comes from structural
/// position in the Module, not from this type.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ImplBlock {
    /// The [`TypeDefId`] of the type being implemented.
    pub target_type: TypeDefId,
    /// The trait being implemented, if this is a trait impl.
    /// `None` for inherent impls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trait_name: Option<Symbol>,
    /// Methods defined in this impl block.
    pub methods: Vec<Function>,
    /// Associated constants defined in this impl block.
    pub associated_constants: Vec<Constant>,
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{
        ConstantId, DeclId, FunctionId, SourceLocation, SourceOrigin, Symbol, SymbolInterner,
        TypeDefId,
    };
    use crate::ir::{
        BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, Mutability,
        RegseqType, ScalarType, Terminator, TerminatorKind,
    };
    use crate::model::constant::Constant;
    use crate::model::function::{Function, Param, ParamKind};
    use crate::model::type_def::{AttrEntry, AttrValue, Attrs};

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

    fn method(
        interner: &mut SymbolInterner,
        fn_id: u32,
        name: &str,
        target: TypeDefId,
        receiver_mut: Mutability,
        return_type: RegseqType,
        attrs: Attrs,
    ) -> Function {
        let fn_name = sym(interner, name);
        let self_name = sym(interner, "self");
        let body = empty_body(interner);
        let receiver_ty = RegseqType::Ref(Box::new(RegseqType::Defined(target)), receiver_mut);
        Function {
            id: FunctionId::from(DeclId::new(fn_id)),
            name: fn_name,
            params: vec![Param {
                kind: ParamKind::Receiver,
                name: self_name,
                ty: receiver_ty,
            }],
            return_type,
            body,
            attrs,
        }
    }

    // -- Inherent impl block ---------------------------------------------

    #[test]
    fn constructs_inherent_impl_block_with_methods_and_associated_constant() {
        let mut interner = SymbolInterner::new();
        let target = TypeDefId::from(DeclId::new(100));
        let const_name = sym(&mut interner, "LIMIT");

        let m0 = method(
            &mut interner,
            1,
            "count",
            target,
            Mutability::Shared,
            RegseqType::Scalar(ScalarType::U32),
            Attrs::default(),
        );
        let m1 = method(
            &mut interner,
            2,
            "reset",
            target,
            Mutability::Mutable,
            RegseqType::Unit,
            Attrs::default(),
        );

        let c = Constant {
            id: ConstantId::from(DeclId::new(3)),
            name: const_name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(16),
        };

        let block = ImplBlock {
            target_type: target,
            trait_name: None,
            methods: vec![m0, m1],
            associated_constants: vec![c],
        };

        assert!(block.trait_name.is_none(), "inherent impls carry no trait");
        assert_eq!(block.target_type, target);
        assert_eq!(block.methods.len(), 2);
        assert_eq!(block.associated_constants.len(), 1);
    }

    // -- Trait impl block (Regseq trait) ---------------------------------

    #[test]
    fn constructs_trait_impl_block_with_init_deinit_methods() {
        let mut interner = SymbolInterner::new();
        let target = TypeDefId::from(DeclId::new(200));
        let trait_sym = sym(&mut interner, "Lifecycle");

        let init = method(
            &mut interner,
            10,
            "init",
            target,
            Mutability::Mutable,
            RegseqType::Unit,
            Attrs::default(),
        );
        let deinit = method(
            &mut interner,
            11,
            "deinit",
            target,
            Mutability::Mutable,
            RegseqType::Unit,
            Attrs::default(),
        );

        let block = ImplBlock {
            target_type: target,
            trait_name: Some(trait_sym),
            methods: vec![init, deinit],
            associated_constants: vec![],
        };

        assert_eq!(
            block.trait_name,
            Some(trait_sym),
            "trait impls must record the trait's name symbol"
        );
        assert_eq!(block.methods.len(), 2);
        assert!(
            block.associated_constants.is_empty(),
            "Regseq trait impls typically carry no associated constants"
        );
    }

    // -- Driver impl block with ISR handler ------------------------------

    #[test]
    fn constructs_driver_impl_block_with_helpers_and_isr_handler() {
        let mut interner = SymbolInterner::new();
        let driver_type = TypeDefId::from(DeclId::new(300));

        let helper = method(
            &mut interner,
            20,
            "program_baud",
            driver_type,
            Mutability::Mutable,
            RegseqType::Unit,
            Attrs::default(),
        );

        let isr_attrs = Attrs {
            entries: vec![AttrEntry {
                key: "rseq::isr".to_string(),
                value: AttrValue::Unit,
            }],
        };

        let isr = method(
            &mut interner,
            21,
            "on_irq",
            driver_type,
            Mutability::Mutable,
            RegseqType::Unit,
            isr_attrs,
        );

        let block = ImplBlock {
            target_type: driver_type,
            trait_name: None,
            methods: vec![helper, isr],
            associated_constants: vec![],
        };

        assert_eq!(block.target_type, driver_type);
        assert!(
            block.trait_name.is_none(),
            "driver impl blocks are inherent (Section 6)"
        );
        assert_eq!(block.methods.len(), 2);
        let isr_has_attrs = block.methods.iter().any(|m| !m.attrs.entries.is_empty());
        assert!(
            isr_has_attrs,
            "ISR handler should carry #[rseq::isr] metadata in attrs"
        );
    }

    // -- trait_name semantics --------------------------------------------

    #[test]
    fn trait_name_distinguishes_inherent_from_trait_impl() {
        let mut interner = SymbolInterner::new();
        let target = TypeDefId::from(DeclId::new(400));
        let trait_sym = sym(&mut interner, "Serial");

        let inherent = ImplBlock {
            target_type: target,
            trait_name: None,
            methods: vec![],
            associated_constants: vec![],
        };
        let trait_impl = ImplBlock {
            target_type: target,
            trait_name: Some(trait_sym),
            methods: vec![],
            associated_constants: vec![],
        };

        assert!(inherent.trait_name.is_none());
        assert!(trait_impl.trait_name.is_some());
        assert_ne!(inherent, trait_impl);
    }
}
