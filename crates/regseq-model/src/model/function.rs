//! Function, parameter, and parameter-kind model types.
//!
//! All callable entities in the Regseq Model share the same [`Function`]
//! representation: free functions, trait methods, helper methods, ISR
//! handlers, and abstraction wrapper methods. Domain-specific
//! classification comes from structural position in the model (which
//! section owns the function) and from the `attrs` field.
//!
//! A function with `params[0].kind == ParamKind::Receiver` is a method;
//! the receiver's type determines the target type.

use serde::{Deserialize, Serialize};

use crate::identity::{FunctionId, Symbol};
use crate::ir::{Body, RegseqType};
use crate::model::type_def::Attrs;

/// A callable entity: free function, trait method, helper method,
/// ISR handler, or abstraction wrapper method.
///
/// Domain-specific classification comes from structural position in
/// the model (which section owns the function) and from the [`Attrs`]
/// bag. The body is always present -- the spec does not have bodyless
/// function declarations, and functions are only extracted if they
/// have MIR bodies.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Function {
    /// The function's declaration identifier.
    pub id: FunctionId,
    /// The function's identifier as an interned symbol.
    pub name: Symbol,
    /// Ordered parameter list. If the first parameter is a
    /// [`ParamKind::Receiver`], the function is a method.
    pub params: Vec<Param>,
    /// Return type. Use [`RegseqType::Unit`] for functions returning `()`.
    pub return_type: RegseqType,
    /// Executable body.
    pub body: Body,
    /// Extensible attribute bag for semantic annotations.
    pub attrs: Attrs,
}

/// A function parameter.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Param {
    /// Semantic role of the parameter (receiver vs regular).
    pub kind: ParamKind,
    /// Parameter identifier.
    pub name: Symbol,
    /// Parameter type.
    pub ty: RegseqType,
}

/// The semantic role of a parameter.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ParamKind {
    /// `self` -- this parameter carries method semantics.
    Receiver,
    /// Normal parameter.
    Regular,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SourceLocation, SourceOrigin, SymbolInterner, TypeDefId};
    use crate::ir::{
        BasicBlock, BasicBlockSourceMap, BodySourceMap, LocalDecl, Mutability, ScalarType,
        Terminator, TerminatorKind,
    };

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    /// Build a minimal empty Body suitable for Phase 5 Pass 1 skeleton.
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

    // -- Free function ---------------------------------------------------

    #[test]
    fn constructs_free_function_with_no_receiver() {
        let mut interner = SymbolInterner::new();
        let fn_name = sym(&mut interner, "do_thing");
        let body = empty_body(&mut interner);

        let f = Function {
            id: FunctionId::from(DeclId::new(1)),
            name: fn_name,
            params: vec![],
            return_type: RegseqType::Unit,
            body,
            attrs: Attrs::default(),
        };

        assert!(f.params.is_empty());
        assert!(!f.params.iter().any(|p| p.kind == ParamKind::Receiver));
    }

    // -- Method function -------------------------------------------------

    #[test]
    fn constructs_method_function_with_receiver_as_first_param() {
        let mut interner = SymbolInterner::new();
        let fn_name = sym(&mut interner, "read");
        let self_name = sym(&mut interner, "self");
        let body = empty_body(&mut interner);

        let target_type_id = TypeDefId::from(DeclId::new(10));
        let receiver_ty = RegseqType::Ref(
            Box::new(RegseqType::Defined(target_type_id)),
            Mutability::Shared,
        );

        let f = Function {
            id: FunctionId::from(DeclId::new(2)),
            name: fn_name,
            params: vec![Param {
                kind: ParamKind::Receiver,
                name: self_name,
                ty: receiver_ty,
            }],
            return_type: RegseqType::Scalar(ScalarType::U32),
            body,
            attrs: Attrs::default(),
        };

        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].kind, ParamKind::Receiver);
    }

    // -- Multiple regular parameters -------------------------------------

    #[test]
    fn constructs_function_with_multiple_regular_params() {
        let mut interner = SymbolInterner::new();
        let fn_name = sym(&mut interner, "add3");
        let p0 = sym(&mut interner, "a");
        let p1 = sym(&mut interner, "b");
        let p2 = sym(&mut interner, "c");
        let body = empty_body(&mut interner);

        let f = Function {
            id: FunctionId::from(DeclId::new(3)),
            name: fn_name,
            params: vec![
                Param {
                    kind: ParamKind::Regular,
                    name: p0,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
                Param {
                    kind: ParamKind::Regular,
                    name: p1,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
                Param {
                    kind: ParamKind::Regular,
                    name: p2,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
            ],
            return_type: RegseqType::Scalar(ScalarType::U32),
            body,
            attrs: Attrs::default(),
        };

        assert_eq!(f.params.len(), 3);
        assert!(f.params.iter().all(|p| p.kind == ParamKind::Regular));
    }

    // -- ParamKind serde round-trip --------------------------------------

    #[test]
    fn param_kind_round_trips_through_json_for_every_variant() {
        for variant in [ParamKind::Receiver, ParamKind::Regular] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: ParamKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    #[test]
    fn param_kind_uses_adjacently_tagged_format() {
        let json = serde_json::to_string(&ParamKind::Receiver).unwrap();
        assert!(
            json.contains("\"kind\""),
            "adjacently tagged variants must emit a 'kind' field: {json}",
        );
    }

    // -- Method recognition helper behavior ------------------------------

    #[test]
    fn method_function_first_param_kind_is_receiver() {
        let mut interner = SymbolInterner::new();
        let fn_name = sym(&mut interner, "helper");
        let self_name = sym(&mut interner, "self");
        let other = sym(&mut interner, "x");
        let body = empty_body(&mut interner);

        let target_type_id = TypeDefId::from(DeclId::new(11));
        let receiver_ty = RegseqType::Ref(
            Box::new(RegseqType::Defined(target_type_id)),
            Mutability::Mutable,
        );

        let f = Function {
            id: FunctionId::from(DeclId::new(4)),
            name: fn_name,
            params: vec![
                Param {
                    kind: ParamKind::Receiver,
                    name: self_name,
                    ty: receiver_ty,
                },
                Param {
                    kind: ParamKind::Regular,
                    name: other,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
            ],
            return_type: RegseqType::Unit,
            body,
            attrs: Attrs::default(),
        };

        assert_eq!(
            f.params[0].kind,
            ParamKind::Receiver,
            "method's first parameter must be a Receiver"
        );
    }
}
