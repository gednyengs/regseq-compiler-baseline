//! Post-bridge inspection predicate enum + dispatch per strategy
//! section 7.
//!
//! F1 ships the complete 22-variant enum (20 leaf variants plus
//! the two composites `All` / `Any`) so the sidecar can be
//! authored against the full predicate vocabulary, but implements
//! dispatch only for the demo set's predicates (`TyVariant`,
//! `ItemKind`, `FunctionInstanceFlavor`, `All`, `Any`). F2 (per
//! `bridge-coverage-runner-strategy.md` section 11, mini-F3 slice)
//! adds three more dispatch arms (`SyntheticItemKind`,
//! `ConstVariant`, `CallDivergent`) required by the first snippet
//! batch's Section-5 rows. The remaining 14 leaf variants return
//! [`InspectError::Unimplemented`]; F3 per strategy section 11
//! hardens dispatch for the rest.
//!
//! Strategy section 7 deviation (implementation-level only): the
//! strategy text presents `All`/`Any` as struct variants carrying
//! a `predicates: Vec<InspectionPredicate>` field. F1 uses
//! newtype variants (`All(Vec<InspectionPredicate>)`) so the
//! adjacently-tagged serde representation produces a flat
//! `{ kind = "All", arg = [...] }` TOML shape instead of the
//! nested `{ kind = "All", arg = { predicates = [...] } }`
//! shape struct variants force. The semantic content (all-of vs
//! any-of composition) is identical.

use serde::Deserialize;

use regseq_model::{
    Const, InstanceKind, ItemKind, RegseqModel, SyntheticItemKind, TerminatorKind, Ty,
};

/// Closed inspection-predicate enum per strategy section 7.
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", content = "arg", rename_all = "PascalCase")]
pub enum InspectionPredicate {
    /// At least one `model.types` entry matches the named `Ty`
    /// variant (e.g., `"Enum"`, `"Bool"`, `"Dynamic"`).
    TyVariant(String),
    /// At least one Item's `kind` matches the named `ItemKind`
    /// variant (e.g., `"Static"`, `"Struct"`).
    ItemKind(String),
    /// At least one `SyntheticItemKind` entry matches.
    SyntheticItemKind(String),
    /// At least one Function Instance's flavor matches.
    FunctionInstanceFlavor(String),
    /// At least one body's statement matches the named variant.
    StatementKind(String),
    /// At least one body's terminator matches the named variant.
    TerminatorKind(String),
    /// At least one body's rvalue matches the named variant.
    RvalueVariant(String),
    /// At least one PlaceElem matches the named variant.
    PlaceElem(String),
    /// At least one Const matches the named variant.
    ConstVariant(String),
    /// At least one Rvalue::Aggregate uses the named AggregateKind.
    AggregateKind(String),
    /// At least one Rvalue::Cast uses the named CastKind.
    CastKind(String),
    /// At least one Rvalue::BinaryOp uses the named BinOp.
    BinOp(String),
    /// At least one Rvalue::UnaryOp uses the named UnOp.
    UnOp(String),
    /// At least one Terminator::Assert uses the named AssertKind.
    AssertKind(String),
    /// At least one NonDivergingIntrinsic sub-variant matches.
    NonDivergingIntrinsic(String),
    /// At least one Call terminator's callee matches.
    CallCallee(String),
    /// At least one Call terminator's divergence flag matches.
    CallDivergent(bool),
    /// At least one Function Instance has `body == None`.
    BodyAbsent,
    /// At least one Item carries the named Linkage variant.
    Linkage(String),
    /// At least one `Ty::Int`/`Ty::Uint`/`Ty::Float` carries the
    /// named scalar-kind variant.
    ScalarKind(String),
    /// Composite: ALL of the listed predicates hold.
    All(Vec<InspectionPredicate>),
    /// Composite: AT LEAST ONE of the listed predicates holds.
    Any(Vec<InspectionPredicate>),
}

/// Errors returned by [`check`].
#[derive(Debug, Clone)]
pub enum InspectError {
    /// The predicate's variant has no F1 dispatch arm; F3 per
    /// strategy section 11 fills these in.
    Unimplemented {
        /// Identifying tag for the unimplemented predicate variant.
        variant: &'static str,
        /// Rendered argument the unimplemented predicate carried,
        /// surfaced in the report so the F3 follow-up can scope
        /// to the exact sidecar entries the gap blocks.
        arg: String,
    },
    /// Dispatch ran but the construct was not found in the model.
    NotFound {
        /// Identifying tag for the predicate variant.
        variant: &'static str,
        /// Argument the predicate was searched for.
        arg: String,
    },
    /// `Any` composite ran but no nested predicate held; carries
    /// every nested error for diagnostics.
    AnyFailed {
        /// Per-child diagnostics.
        children: Vec<InspectError>,
    },
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InspectError::Unimplemented { variant, arg } => {
                write!(
                    f,
                    "inspection predicate {variant}({arg}) is not implemented in F1 (F3 follow-up)"
                )
            }
            InspectError::NotFound { variant, arg } => {
                write!(
                    f,
                    "inspection predicate {variant}({arg:?}) did not match any model entry"
                )
            }
            InspectError::AnyFailed { children } => {
                write!(
                    f,
                    "Any composite: no child predicate matched ({} child errors)",
                    children.len()
                )
            }
        }
    }
}

/// Returns the static tag for an `InspectionPredicate` variant.
fn variant_name(pred: &InspectionPredicate) -> &'static str {
    match pred {
        InspectionPredicate::TyVariant(_) => "TyVariant",
        InspectionPredicate::ItemKind(_) => "ItemKind",
        InspectionPredicate::SyntheticItemKind(_) => "SyntheticItemKind",
        InspectionPredicate::FunctionInstanceFlavor(_) => "FunctionInstanceFlavor",
        InspectionPredicate::StatementKind(_) => "StatementKind",
        InspectionPredicate::TerminatorKind(_) => "TerminatorKind",
        InspectionPredicate::RvalueVariant(_) => "RvalueVariant",
        InspectionPredicate::PlaceElem(_) => "PlaceElem",
        InspectionPredicate::ConstVariant(_) => "ConstVariant",
        InspectionPredicate::AggregateKind(_) => "AggregateKind",
        InspectionPredicate::CastKind(_) => "CastKind",
        InspectionPredicate::BinOp(_) => "BinOp",
        InspectionPredicate::UnOp(_) => "UnOp",
        InspectionPredicate::AssertKind(_) => "AssertKind",
        InspectionPredicate::NonDivergingIntrinsic(_) => "NonDivergingIntrinsic",
        InspectionPredicate::CallCallee(_) => "CallCallee",
        InspectionPredicate::CallDivergent(_) => "CallDivergent",
        InspectionPredicate::BodyAbsent => "BodyAbsent",
        InspectionPredicate::Linkage(_) => "Linkage",
        InspectionPredicate::ScalarKind(_) => "ScalarKind",
        InspectionPredicate::All(_) => "All",
        InspectionPredicate::Any(_) => "Any",
    }
}

/// Render the argument carried by an `InspectionPredicate` variant
/// for inclusion in [`InspectError::Unimplemented`].
fn arg_for_display(pred: &InspectionPredicate) -> String {
    match pred {
        InspectionPredicate::TyVariant(arg)
        | InspectionPredicate::ItemKind(arg)
        | InspectionPredicate::SyntheticItemKind(arg)
        | InspectionPredicate::FunctionInstanceFlavor(arg)
        | InspectionPredicate::StatementKind(arg)
        | InspectionPredicate::TerminatorKind(arg)
        | InspectionPredicate::RvalueVariant(arg)
        | InspectionPredicate::PlaceElem(arg)
        | InspectionPredicate::ConstVariant(arg)
        | InspectionPredicate::AggregateKind(arg)
        | InspectionPredicate::CastKind(arg)
        | InspectionPredicate::BinOp(arg)
        | InspectionPredicate::UnOp(arg)
        | InspectionPredicate::AssertKind(arg)
        | InspectionPredicate::NonDivergingIntrinsic(arg)
        | InspectionPredicate::CallCallee(arg)
        | InspectionPredicate::Linkage(arg)
        | InspectionPredicate::ScalarKind(arg) => arg.clone(),
        InspectionPredicate::CallDivergent(b) => b.to_string(),
        InspectionPredicate::BodyAbsent => String::new(),
        InspectionPredicate::All(_) | InspectionPredicate::Any(_) => "<composite>".to_string(),
    }
}

/// Variant tag for a `Ty` value.
fn ty_variant_tag(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::Char => "Char",
        Ty::Int(_) => "Int",
        Ty::Uint(_) => "Uint",
        Ty::Float(_) => "Float",
        Ty::Str => "Str",
        Ty::Never => "Never",
        Ty::Array(_, _) => "Array",
        Ty::Slice(_) => "Slice",
        Ty::Ref(_, _) => "Ref",
        Ty::Ptr(_, _) => "Ptr",
        Ty::FnDef(_, _) => "FnDef",
        Ty::FnPtr(_) => "FnPtr",
        Ty::Dynamic { .. } => "Dynamic",
        Ty::Closure(_, _) => "Closure",
        Ty::Tuple { .. } => "Tuple",
        Ty::Struct { .. } => "Struct",
        Ty::Enum { .. } => "Enum",
        Ty::Union { .. } => "Union",
    }
}

/// Variant tag for an `ItemKind` value.
fn item_kind_tag(kind: &ItemKind) -> &'static str {
    match kind {
        ItemKind::Function => "Function",
        ItemKind::Static { .. } => "Static",
        ItemKind::Const => "Const",
        ItemKind::Struct => "Struct",
        ItemKind::Enum => "Enum",
        ItemKind::Union => "Union",
    }
}

/// Variant tag for a `SyntheticItemKind` value.
fn synthetic_kind_variant_tag(sk: &SyntheticItemKind) -> &'static str {
    match sk {
        SyntheticItemKind::None => "None",
        SyntheticItemKind::ClosureCaptures { .. } => "ClosureCaptures",
        SyntheticItemKind::ClosureCall { .. } => "ClosureCall",
        SyntheticItemKind::ClosureDropGlue { .. } => "ClosureDropGlue",
        SyntheticItemKind::DropGlue { .. } => "DropGlue",
        SyntheticItemKind::VTableType { .. } => "VTableType",
        SyntheticItemKind::VTableInstance { .. } => "VTableInstance",
        SyntheticItemKind::VTableMethodShim { .. } => "VTableMethodShim",
        SyntheticItemKind::VTableDropShim { .. } => "VTableDropShim",
        SyntheticItemKind::ConstMemoryStatic { .. } => "ConstMemoryStatic",
    }
}

/// Variant tag for a `Const` value.
fn const_variant_tag(c: &Const) -> &'static str {
    match c {
        Const::Bool(_) => "Bool",
        Const::Char(_) => "Char",
        Const::Int(_, _) => "Int",
        Const::Uint(_, _) => "Uint",
        Const::Float(_, _) => "Float",
        Const::ZST => "ZST",
        Const::Bytes(_) => "Bytes",
        Const::Aggregate(_) => "Aggregate",
        Const::EnumValue { .. } => "EnumValue",
        Const::NullPtr(_) => "NullPtr",
        Const::Reloc(_) => "Reloc",
    }
}

/// Variant tag for a `FunctionInstanceFlavor` value. Mirrors the
/// `Debug` rendering.
fn flavor_tag(flavor: &regseq_model::FunctionInstanceFlavor) -> &'static str {
    use regseq_model::FunctionInstanceFlavor as F;
    match flavor {
        F::Item => "Item",
        F::Intrinsic => "Intrinsic",
        F::VTableShim => "VTableShim",
        F::ReifyShim => "ReifyShim",
        F::FnPtrShim => "FnPtrShim",
        F::Virtual => "Virtual",
        F::ClosureOnceShim => "ClosureOnceShim",
        F::ConstructCoroutineInClosureShim => "ConstructCoroutineInClosureShim",
        F::ThreadLocalShim => "ThreadLocalShim",
        F::FutureDropPollShim => "FutureDropPollShim",
        F::DropGlue => "DropGlue",
        F::CloneShim => "CloneShim",
        F::FnPtrAddrShim => "FnPtrAddrShim",
        F::AsyncDropGlueCtorShim => "AsyncDropGlueCtorShim",
        F::AsyncDropGlue => "AsyncDropGlue",
    }
}

/// Walk `model` and verify `pred` holds.
pub fn check(model: &RegseqModel, pred: &InspectionPredicate) -> Result<(), InspectError> {
    match pred {
        InspectionPredicate::TyVariant(arg) => {
            let hit = model
                .types
                .iter()
                .any(|(_, ty)| ty_variant_tag(ty) == arg.as_str());
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "TyVariant",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::ItemKind(arg) => {
            let hit = model
                .items
                .iter()
                .any(|item| item_kind_tag(&item.kind) == arg.as_str());
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "ItemKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::FunctionInstanceFlavor(arg) => {
            let hit = model.instances.iter().any(|inst| match &inst.kind {
                InstanceKind::Function(data) => flavor_tag(&data.flavor) == arg.as_str(),
                _ => false,
            });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "FunctionInstanceFlavor",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::SyntheticItemKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .item_synthetic_kinds
                .iter()
                .any(|sk| synthetic_kind_variant_tag(sk) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "SyntheticItemKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::ConstVariant(arg) => {
            let needle = arg.as_str();
            let hit = model
                .consts
                .iter()
                .any(|(_, c)| const_variant_tag(c) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "ConstVariant",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::CallDivergent(arg) => {
            let needs_divergent = *arg;
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| match &bb.terminator.kind {
                    TerminatorKind::Call { target, .. } => target.is_none() == needs_divergent,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "CallDivergent",
                    arg: needs_divergent.to_string(),
                })
            }
        }
        InspectionPredicate::All(children) => {
            for child in children {
                check(model, child)?;
            }
            Ok(())
        }
        InspectionPredicate::Any(children) => {
            let mut errors = Vec::with_capacity(children.len());
            for child in children {
                match check(model, child) {
                    Ok(()) => return Ok(()),
                    Err(err) => errors.push(err),
                }
            }
            Err(InspectError::AnyFailed { children: errors })
        }
        other => Err(InspectError::Unimplemented {
            variant: variant_name(other),
            arg: arg_for_display(other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use regseq_model::{
        ArgsKey, BasicBlock, Body, CallCallee, FloatKind, FnAbi, FnSig, FunctionInstanceData,
        FunctionInstanceFlavor, GenericArgs, IndexVec, InstanceId, InstanceKind, IntKind, ItemId,
        LocalDecl, LocalId, Mutability, Place, PrincipalTraitKey, ProjectionBindingsKey,
        RegseqModel, Safety, SourceFileId, Span, StableCrateId, Terminator, UNIT_LAYOUT,
    };

    fn empty_model() -> RegseqModel {
        RegseqModel::with_primary("inspect_test", StableCrateId(1))
    }

    fn dummy_span() -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        }
    }

    fn dummy_principal(local: u64) -> PrincipalTraitKey {
        PrincipalTraitKey {
            trait_path: regseq_model::DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: local,
            },
            trait_args: ArgsKey(0x1111),
            projection_bindings: ProjectionBindingsKey(0x2222),
        }
    }

    /// Build a `Body` whose basic-block terminators are exactly
    /// `terminators`, without statements.
    fn body_with_terminators(
        unit_ty: regseq_model::TyId,
        terminators: Vec<TerminatorKind>,
    ) -> Body {
        let mut basic_blocks: IndexVec<regseq_model::BlockId, BasicBlock> = IndexVec::new();
        for kind in terminators {
            basic_blocks.push(BasicBlock {
                statements: Box::new([]),
                terminator: Terminator {
                    kind,
                    span: dummy_span(),
                },
            });
        }
        let mut local_decls: IndexVec<LocalId, LocalDecl> = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit_ty,
            mutability: Mutability::Mut,
            span: dummy_span(),
            debug_name: None,
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: dummy_span(),
        }
    }

    /// Push a minimal `Function` instance carrying `body`. The
    /// instance back-references `ItemId::ZERO`; the dispatch arms
    /// under test never resolve item back-references, so a stub
    /// is sufficient.
    fn push_function_instance(model: &mut RegseqModel, body: Body) {
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let sig = model.sigs.intern(FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        let emission_name = model.strings.intern("inspect_test_fn");
        model.instances.push(regseq_model::Instance {
            item: ItemId::ZERO,
            generic_args: GenericArgs::new(),
            emission_name,
            span: dummy_span(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: sig,
                body: Some(body),
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        });
    }

    #[test]
    fn synthetic_item_kind_arm_matches_present_variant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        model
            .item_synthetic_kinds
            .push(SyntheticItemKind::VTableDropShim {
                principal: dummy_principal(0x7A17),
                self_ty: ArgsKey(0x3333),
            });

        assert!(check(
            &model,
            &InspectionPredicate::SyntheticItemKind("VTableDropShim".to_string()),
        )
        .is_ok());

        match check(
            &model,
            &InspectionPredicate::SyntheticItemKind("VTableType".to_string()),
        ) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "SyntheticItemKind");
                assert_eq!(arg, "VTableType");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn const_variant_arm_matches_present_variant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        model.consts.intern(Const::Float(FloatKind::F32, 0));
        model.consts.intern(Const::Int(IntKind::I32, 0));

        assert!(check(
            &model,
            &InspectionPredicate::ConstVariant("Float".to_string()),
        )
        .is_ok());
        assert!(check(
            &model,
            &InspectionPredicate::ConstVariant("Int".to_string()),
        )
        .is_ok());

        match check(
            &model,
            &InspectionPredicate::ConstVariant("Reloc".to_string()),
        ) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "ConstVariant");
                assert_eq!(arg, "Reloc");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn call_divergent_arm_matches_divergent_call_and_reports_not_found_otherwise() {
        let mut divergent_model = empty_model();
        let unit_ty = divergent_model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let body = body_with_terminators(
            unit_ty,
            vec![TerminatorKind::Call {
                callee: CallCallee::Direct(InstanceId::ZERO),
                args: Box::new([]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: None,
            }],
        );
        push_function_instance(&mut divergent_model, body);

        assert!(check(&divergent_model, &InspectionPredicate::CallDivergent(true),).is_ok());
        match check(&divergent_model, &InspectionPredicate::CallDivergent(false)) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "CallDivergent");
                assert_eq!(arg, "false");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }

        let mut returning_model = empty_model();
        let unit_ty = returning_model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let body = body_with_terminators(
            unit_ty,
            vec![
                TerminatorKind::Call {
                    callee: CallCallee::Direct(InstanceId::ZERO),
                    args: Box::new([]),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: Some(regseq_model::BlockId::from_u32(1)),
                },
                TerminatorKind::Return,
            ],
        );
        push_function_instance(&mut returning_model, body);

        assert!(check(&returning_model, &InspectionPredicate::CallDivergent(false),).is_ok());
        match check(&returning_model, &InspectionPredicate::CallDivergent(true)) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "CallDivergent");
                assert_eq!(arg, "true");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn unimplemented_arms_still_report_unimplemented() {
        let model = empty_model();
        match check(&model, &InspectionPredicate::PlaceElem("Deref".to_string())) {
            Err(InspectError::Unimplemented { variant, arg }) => {
                assert_eq!(variant, "PlaceElem");
                assert_eq!(arg, "Deref");
            }
            other => panic!("expected Unimplemented, got {other:?}"),
        }
    }
}
