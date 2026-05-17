//! Phase-5 vtable-indirect body rewrite.

use regseq_model::{
    BlockId, Body, CallCallee, FieldIdx, FunctionInstanceFlavor, LocalId, Mutability, Operand,
    Place, PlaceElem, Rvalue, Statement, StatementKind, SyntheticItemKind, TerminatorKind, Ty,
    TyId, UnOp, UNIT_LAYOUT,
};

use crate::bridge::{BridgeCtxt, BridgeError};

struct VtableCallTarget {
    slot: u32,
    vtable_key: Option<(regseq_model::PrincipalTraitKey, regseq_model::ArgsKey)>,
}

/// Rewrite Direct Calls against `dyn Trait` shim placeholders into
/// Indirect Calls that read the function pointer from the
/// VTableInstance per spec.md section 25.7.2. Runs after
/// Drop-as-Call collapse and before joint validation.
pub fn rewrite_vtable_indirect(
    cx: &mut BridgeCtxt<'_>,
    body: &mut regseq_model::Body,
) -> Result<(), BridgeError> {
    rewrite_vtable_indirect_in_model(&mut cx.model, body)
}

fn rewrite_vtable_indirect_in_model(
    model: &mut regseq_model::RegseqModel,
    body: &mut regseq_model::Body,
) -> Result<(), BridgeError> {
    for block_idx in 0..body.basic_blocks.len() {
        let block_id = BlockId::from_u32(block_idx as u32);
        let Some((instance_id, target, receiver)) = virtual_call_site(model, body, block_id)?
        else {
            continue;
        };
        let receiver_ty = operand_ty(model, body, &receiver)?;
        let (dynamic_ty, data_mutability, needs_raw) = receiver_dynamic_ty(model, receiver_ty)?;
        consume_vtable_artifacts(model, dynamic_ty, target.vtable_key.as_ref(), target.slot)?;
        let fallback_fn_sig = model.instances[instance_id]
            .kind
            .as_function()
            .map(|function| function.fn_sig)
            .ok_or_else(|| {
                BridgeError::Phase5(format!(
                    "virtual call instance {} is not a function",
                    instance_id.as_u32()
                ))
            })?;

        let recv = body.push_synthetic_local(receiver_ty);
        let meta_ty = metadata_ty(model, dynamic_ty)?;
        let meta = body.push_synthetic_local(meta_ty);
        // Per spec section 25.7.2 step 4, the Field projection at
        // the vtable slot must declare the slot's actual layout-
        // declared type so the joint-validation place walker's
        // FieldTypeMismatch check (spec 19.7) succeeds. The synth
        // path may declare method slots with a canonical-vacant
        // FnPtr signature (`extern "C" fn(*mut ()) -> ()` per spec
        // section 13.2) rather than the Virtual instance's per-
        // monomorphisation signature; we therefore read the field's
        // declared `TyId` directly from the vtable struct layout
        // and fall back to the Virtual fn_sig only when no layout
        // is available (hand-built test fixtures).
        let fn_ptr_ty = vtable_slot_field_ty(model, dynamic_ty, target.slot)
            .unwrap_or_else(|| model.types.intern(Ty::FnPtr(fallback_fn_sig)));
        let func = body.push_synthetic_local(fn_ptr_ty);
        let unit = model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        });
        let data_ty = model.types.intern(Ty::Ptr(unit, data_mutability));
        let data = body.push_synthetic_local(data_ty);
        let raw = if needs_raw {
            Some(
                body.push_synthetic_local(model.types.intern(Ty::Ptr(dynamic_ty, data_mutability))),
            )
        } else {
            None
        };

        let mut statements = body.basic_blocks[block_id].statements.to_vec();
        statements.push(assign(recv, Rvalue::Use(receiver)));
        if let Some(raw) = raw {
            statements.push(assign(
                raw,
                Rvalue::RawPtr(
                    data_mutability,
                    Place {
                        local: recv,
                        projection: Box::new([PlaceElem::Deref]),
                    },
                ),
            ));
        }
        statements.push(assign(
            meta,
            Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(Place::from(recv))),
        ));
        statements.push(assign(
            func,
            Rvalue::Use(Operand::Copy(Place {
                local: meta,
                projection: Box::new([
                    PlaceElem::Deref,
                    PlaceElem::Field(FieldIdx::from_u32(target.slot), fn_ptr_ty),
                ]),
            })),
        ));
        let data_source = raw
            .map(|raw| Operand::Move(Place::from(raw)))
            .unwrap_or_else(|| Operand::Copy(Place::from(recv)));
        statements.push(assign(
            data,
            Rvalue::Cast(regseq_model::CastKind::PtrToPtr, data_source, data_ty),
        ));

        body.basic_blocks[block_id].statements = statements.into_boxed_slice();
        let TerminatorKind::Call { callee, args, .. } =
            &mut body.basic_blocks[block_id].terminator.kind
        else {
            unreachable!("call site was checked before rewrite")
        };
        *callee = CallCallee::Indirect(Operand::Move(Place::from(func)));
        args[0] = Operand::Move(Place::from(data));
    }

    Ok(())
}

fn virtual_call_site(
    model: &regseq_model::RegseqModel,
    body: &Body,
    block_id: BlockId,
) -> Result<Option<(regseq_model::InstanceId, VtableCallTarget, Operand)>, BridgeError> {
    let TerminatorKind::Call { callee, args, .. } = &body.basic_blocks[block_id].terminator.kind
    else {
        return Ok(None);
    };
    let CallCallee::Direct(instance_id) = *callee else {
        return Ok(None);
    };
    let Some(target) = virtual_call_slot(model, instance_id) else {
        return Ok(None);
    };
    let Some(receiver) = args.first() else {
        return Err(BridgeError::Phase5(format!(
            "virtual call instance {} has no receiver argument",
            instance_id.as_u32()
        )));
    };
    Ok(Some((instance_id, target, receiver.clone())))
}

fn virtual_call_slot(
    model: &regseq_model::RegseqModel,
    instance: regseq_model::InstanceId,
) -> Option<VtableCallTarget> {
    // Audit-finding regseq-driver-43-5: the rewrite predicate is
    // `FunctionInstanceFlavor::Virtual { virtual_idx: Some(slot) }`,
    // period. Earlier revisions also required a
    // `SyntheticItemKind::VTable*Shim` tag on the callee Item; per
    // spec section 25.7.2 the correct identity is the Instance's
    // flavor alone. The synthetic-kind tag (when present) still
    // carries the (principal, self_ty) used to validate the matching
    // VTableInstance / VTableShim Items, but it is no longer part of
    // the trigger predicate.
    if instance.index() >= model.instances.len() {
        return None;
    }
    let function = model.instances[instance].kind.as_function()?;
    if function.flavor != FunctionInstanceFlavor::Virtual {
        return None;
    }
    let slot = function.virtual_idx?;
    let vtable_key = match &model.item_synthetic_kinds[model.instances[instance].item] {
        SyntheticItemKind::VTableMethodShim {
            principal, self_ty, ..
        }
        | SyntheticItemKind::VTableDropShim { principal, self_ty } => {
            Some((principal.clone(), *self_ty))
        }
        _ => None,
    };
    Some(VtableCallTarget { slot, vtable_key })
}

fn operand_ty(
    model: &mut regseq_model::RegseqModel,
    body: &Body,
    operand: &Operand,
) -> Result<TyId, BridgeError> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place_ty(model, body, place),
        Operand::Constant(konst) => match model.consts.resolve(*konst) {
            regseq_model::Const::NullPtr(pointee) => {
                Ok(model.types.intern(Ty::Ptr(*pointee, Mutability::Not)))
            }
            other => Err(BridgeError::Phase5(format!(
                "constant virtual call receiver has no pointer type: {other:?}"
            ))),
        },
    }
}

fn place_ty(
    model: &mut regseq_model::RegseqModel,
    body: &Body,
    place: &Place,
) -> Result<TyId, BridgeError> {
    if place.local.index() >= body.local_decls.len() {
        return Err(BridgeError::Phase5(format!(
            "place local {} is out of range",
            place.local.as_u32()
        )));
    }
    let decl = &body.local_decls[place.local];
    let mut current = decl.ty;
    for elem in place.projection.iter() {
        current = match elem {
            PlaceElem::Deref => match model.types.resolve(current) {
                Ty::Ref(inner, _) | Ty::Ptr(inner, _) => *inner,
                other => {
                    return Err(BridgeError::Phase5(format!(
                        "receiver deref on non-pointer type {other:?}"
                    )));
                }
            },
            PlaceElem::Field(_, field_ty) => *field_ty,
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => {
                match model.types.resolve(current) {
                    Ty::Array(elem, _) | Ty::Slice(elem) => *elem,
                    other => {
                        return Err(BridgeError::Phase5(format!(
                            "receiver index on non-indexable type {other:?}"
                        )));
                    }
                }
            }
            PlaceElem::Subslice { from, to, from_end } => match model.types.resolve(current) {
                Ty::Array(elem, _) if !from_end => {
                    let len = to.checked_sub(*from).ok_or_else(|| {
                        BridgeError::Phase5("receiver subslice end precedes start".to_string())
                    })?;
                    model.types.intern(Ty::Array(*elem, len))
                }
                Ty::Slice(_) if *from_end => current,
                other => {
                    return Err(BridgeError::Phase5(format!(
                        "receiver subslice on unsupported type {other:?}"
                    )));
                }
            },
            PlaceElem::Downcast(_) => current,
        };
    }
    Ok(current)
}

fn receiver_dynamic_ty(
    model: &regseq_model::RegseqModel,
    receiver_ty: TyId,
) -> Result<(TyId, Mutability, bool), BridgeError> {
    // Per spec.md section 13.2 (lines 4607-4618), the erased
    // receiver type is uniformly `Ty::Ptr(ErasedPointee,
    // Mutability::Mut)` regardless of the source borrow flavor.
    // A shared-receiver erasure would not be sound across all
    // admissible impls (e.g. `UnsafeCell`-bearing impls that
    // legitimately mutate through `&self`). This collapse also
    // matches the canonical vacant FnSig's `inputs[0] = *mut ()`
    // so the joint-validator B-6 per-arg type-match check
    // (spec.md section 23.6.2) succeeds.
    match model.types.resolve(receiver_ty) {
        Ty::Ref(dynamic_ty, _) => {
            expect_dynamic(model, *dynamic_ty)?;
            Ok((*dynamic_ty, Mutability::Mut, true))
        }
        Ty::Ptr(dynamic_ty, _) => {
            expect_dynamic(model, *dynamic_ty)?;
            Ok((*dynamic_ty, Mutability::Mut, false))
        }
        other => Err(BridgeError::Phase5(format!(
            "virtual call receiver must be ref or ptr to dyn Trait, got {other:?}"
        ))),
    }
}

fn expect_dynamic(model: &regseq_model::RegseqModel, ty: TyId) -> Result<(), BridgeError> {
    if matches!(model.types.resolve(ty), Ty::Dynamic { .. }) {
        Ok(())
    } else {
        Err(BridgeError::Phase5(format!(
            "virtual call receiver pointee is not dyn Trait: {:?}",
            model.types.resolve(ty)
        )))
    }
}

/// Read the declared `TyId` of the vtable struct's slot at
/// `slot_idx` from the layout-bearing `Ty::Struct` view referenced
/// by `dynamic_ty.vtable_ty`. Returns `None` when the layout is
/// missing the requested slot, the vtable view is not a Struct, or
/// the layout's `kind` is not `LayoutKind::Struct`. Per spec section
/// 13.2 + 25.7.2 step 4.
fn vtable_slot_field_ty(
    model: &regseq_model::RegseqModel,
    dynamic_ty: TyId,
    slot_idx: u32,
) -> Option<TyId> {
    let Ty::Dynamic { vtable_ty, .. } = model.types.resolve(dynamic_ty) else {
        return None;
    };
    let Ty::Struct { layout, .. } = model.types.resolve(*vtable_ty) else {
        return None;
    };
    let regseq_model::LayoutKind::Struct { fields } = &layout.kind else {
        return None;
    };
    fields.get(slot_idx as usize).map(|field| field.ty)
}

fn metadata_ty(
    model: &mut regseq_model::RegseqModel,
    dynamic_ty: TyId,
) -> Result<TyId, BridgeError> {
    let Ty::Dynamic { vtable_ty, .. } = model.types.resolve(dynamic_ty) else {
        return Err(BridgeError::Phase5(
            "metadata source is not dyn Trait".to_string(),
        ));
    };
    Ok(model.types.intern(Ty::Ptr(*vtable_ty, Mutability::Not)))
}

fn consume_vtable_artifacts(
    model: &regseq_model::RegseqModel,
    dynamic_ty: TyId,
    vtable_key: Option<&(regseq_model::PrincipalTraitKey, regseq_model::ArgsKey)>,
    slot: u32,
) -> Result<(), BridgeError> {
    let Ty::Dynamic { vtable_ty, .. } = model.types.resolve(dynamic_ty) else {
        return Ok(());
    };
    let Ty::Struct { item, .. } = model.types.resolve(*vtable_ty) else {
        return Ok(());
    };
    let Some((principal, _)) = model.vtables.iter().find(|(_, mapped)| *mapped == item) else {
        Err(BridgeError::Phase5(format!(
            "dyn Trait vtable item {} is absent from RegseqModel.vtables",
            item.as_u32()
        )))?
    };
    let (principal, self_ty) = if let Some((candidate_principal, self_ty)) = vtable_key {
        if candidate_principal != principal {
            return Err(BridgeError::Phase5(
                "vtable shim principal does not match RegseqModel.vtables".to_string(),
            ));
        }
        (candidate_principal, *self_ty)
    } else {
        let Some(self_ty) = find_vtable_instance_self_ty(model, principal) else {
            return Err(BridgeError::Phase5(
                "matching VTableInstance item is absent for virtual call".to_string(),
            ));
        };
        (principal, self_ty)
    };
    consume_vtable_instance_item(model, principal, self_ty)?;
    consume_vtable_shim_item(model, principal, self_ty, slot)
}

fn consume_vtable_instance_item(
    model: &regseq_model::RegseqModel,
    principal: &regseq_model::PrincipalTraitKey,
    self_ty: regseq_model::ArgsKey,
) -> Result<(), BridgeError> {
    if model.item_synthetic_kinds.iter().any(|kind| {
        matches!(
            kind,
            SyntheticItemKind::VTableInstance {
                principal: candidate_principal,
                self_ty: candidate_self_ty,
            } if candidate_principal == principal && *candidate_self_ty == self_ty
        )
    }) {
        Ok(())
    } else {
        Err(BridgeError::Phase5(
            "matching VTableInstance item is absent for vtable shim call".to_string(),
        ))
    }
}

fn find_vtable_instance_self_ty(
    model: &regseq_model::RegseqModel,
    principal: &regseq_model::PrincipalTraitKey,
) -> Option<regseq_model::ArgsKey> {
    model
        .item_synthetic_kinds
        .iter()
        .find_map(|kind| match kind {
            SyntheticItemKind::VTableInstance {
                principal: candidate_principal,
                self_ty,
            } if candidate_principal == principal => Some(*self_ty),
            _ => None,
        })
}

fn consume_vtable_shim_item(
    model: &regseq_model::RegseqModel,
    principal: &regseq_model::PrincipalTraitKey,
    self_ty: regseq_model::ArgsKey,
    slot: u32,
) -> Result<(), BridgeError> {
    if model.item_synthetic_kinds.iter().any(|kind| match kind {
        SyntheticItemKind::VTableMethodShim {
            principal: candidate_principal,
            self_ty: candidate_self_ty,
            vtable_entry_index,
            ..
        } => {
            candidate_principal == principal
                && *candidate_self_ty == self_ty
                && *vtable_entry_index == slot
        }
        SyntheticItemKind::VTableDropShim {
            principal: candidate_principal,
            self_ty: candidate_self_ty,
        } => candidate_principal == principal && *candidate_self_ty == self_ty && slot == 0,
        _ => false,
    }) {
        Ok(())
    } else {
        Err(BridgeError::Phase5(
            "matching VTableShim item is absent for virtual call".to_string(),
        ))
    }
}

fn assign(local: LocalId, rvalue: Rvalue) -> Statement {
    Statement {
        kind: StatementKind::Assign(Place::from(local), rvalue),
        span: regseq_model::Span::dummy(),
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite_vtable_indirect_in_model;
    use regseq_model::{
        ArgsKey, BasicBlock, Body, DefPath, DefPathData, DefPathHash, DefPathSegment, FnAbi, FnSig,
        FunctionInstanceData, FunctionInstanceFlavor, GenericArg, IndexVec, InlineHint, Instance,
        InstanceKind, Item, ItemKind, Linkage, LocalDecl, Mutability, Operand, Place,
        PrincipalTraitKey, ProjectionBindingsKey, RegseqModel, Safety, Span, StableCrateId,
        Statement, SyntheticItemKind, Terminator, TerminatorKind, Ty, UNIT_LAYOUT,
    };

    fn item(model: &mut RegseqModel, name: &str, kind: SyntheticItemKind) -> regseq_model::ItemId {
        let name_id = model.strings.intern(name);
        let rendered = model.strings.intern(&format!("test::{name}"));
        let item = model.items.push(Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::ValueNs(name_id),
                    disambiguator: 0,
                }],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        });
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: item.as_u32() as u64 + 1,
        });
        model.item_synthetic_kinds.push(kind);
        model.domain_attrs.item_attrs.push(Vec::new());
        item
    }

    fn local(ty: regseq_model::TyId) -> LocalDecl {
        LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }
    }

    fn model_with_call(
        synthetic_kind: SyntheticItemKind,
        flavor: FunctionInstanceFlavor,
        virtual_idx: Option<u32>,
    ) -> (RegseqModel, Body, regseq_model::InstanceId) {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let unit = model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        });
        let fn_sig = model.sigs.intern(FnSig {
            inputs: vec![model.types.intern(Ty::Ptr(unit, Mutability::Mut))],
            output: unit,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        let fn_ptr = model.types.intern(Ty::FnPtr(fn_sig));
        let vtable_item = item(&mut model, "vtable", SyntheticItemKind::None);
        model.items[vtable_item].kind = ItemKind::Struct;
        let vtable_ty = model.types.intern(Ty::Struct {
            item: vtable_item,
            args: Vec::new(),
            layout: regseq_model::Layout {
                size: 8,
                align: 8,
                pack: None,
                uninhabited: false,
                kind: regseq_model::LayoutKind::Struct {
                    fields: vec![regseq_model::FieldLayout {
                        name: model.strings.intern("call"),
                        ty: fn_ptr,
                        offset: 0,
                    }],
                },
            },
        });
        let dynamic = model.types.intern(Ty::Dynamic {
            auto_traits: Vec::new(),
            vtable_ty,
        });
        let receiver = model
            .types
            .intern(Ty::Ref(dynamic, regseq_model::Provenance::SharedBorrow));
        let principal = PrincipalTraitKey {
            trait_path: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 99,
            },
            trait_args: ArgsKey::EMPTY,
            projection_bindings: ProjectionBindingsKey(0),
        };
        let vtable_instance = item(
            &mut model,
            "vtable_instance",
            SyntheticItemKind::VTableInstance {
                principal: principal.clone(),
                self_ty: ArgsKey::EMPTY,
            },
        );
        model.items[vtable_instance].kind = ItemKind::Static {
            mutability: Mutability::Not,
        };
        model.vtables.insert(principal.clone(), vtable_item);
        let callee_item = item(&mut model, "shim", synthetic_kind);
        let callee = model.instances.push(Instance {
            item: callee_item,
            generic_args: vec![GenericArg::Type(receiver)],
            emission_name: model.items[callee_item].def_path.rendered,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor,
                virtual_idx,
            }),
        });

        let mut local_decls = IndexVec::new();
        local_decls.push(local(unit));
        local_decls.push(local(receiver));
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::<[Statement]>::default(),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: regseq_model::CallCallee::Direct(callee),
                    args: Box::new([Operand::Copy(Place::from(regseq_model::LocalId::from_u32(
                        1,
                    )))]),
                    destination: Place::from(regseq_model::LocalId::RETURN_PLACE),
                    target: None,
                },
                span: Span::dummy(),
            },
        });

        (
            model,
            Body {
                basic_blocks,
                local_decls,
                arg_count: 1,
                span: Span::dummy(),
            },
            callee,
        )
    }

    fn method_shim_kind() -> SyntheticItemKind {
        SyntheticItemKind::VTableMethodShim {
            principal: PrincipalTraitKey {
                trait_path: DefPathHash {
                    stable_crate_id: StableCrateId(1),
                    local_hash: 99,
                },
                trait_args: ArgsKey::EMPTY,
                projection_bindings: ProjectionBindingsKey(0),
            },
            self_ty: ArgsKey::EMPTY,
            vtable_entry_index: 0,
            method_def_path: DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: 100,
            },
        }
    }

    #[test]
    fn rewrites_vtable_method_shim_call_to_indirect_and_adds_ref_locals() {
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        let before = body.local_decls.len();
        rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap();

        assert_eq!(body.local_decls.len(), before + 5);
        assert_eq!(
            body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .statements
                .len(),
            5
        );
        assert!(matches!(
            body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .terminator
                .kind,
            TerminatorKind::Call {
                callee: regseq_model::CallCallee::Indirect(_),
                ..
            }
        ));
    }

    #[test]
    fn rewrites_constant_null_pointer_receiver_after_materialisation() {
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        let receiver_ty = body.local_decls[regseq_model::LocalId::from_u32(1)].ty;
        let Ty::Ref(dynamic, _) = model.types.resolve(receiver_ty) else {
            panic!("test receiver should be a dyn reference");
        };
        let konst = model.consts.intern(regseq_model::Const::NullPtr(*dynamic));
        let TerminatorKind::Call { args, .. } = &mut body.basic_blocks
            [regseq_model::BlockId::START_BLOCK]
            .terminator
            .kind
        else {
            panic!("test body should contain a call");
        };
        args[0] = Operand::Constant(konst);
        let before = body.local_decls.len();

        rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap();

        assert_eq!(body.local_decls.len(), before + 4);
        assert!(matches!(
            body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .terminator
                .kind,
            TerminatorKind::Call {
                callee: regseq_model::CallCallee::Indirect(_),
                ..
            }
        ));
    }

    #[test]
    fn rewrites_projected_receiver_place() {
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        let receiver_ty = body.local_decls[regseq_model::LocalId::from_u32(1)].ty;
        let wrapped = model
            .types
            .intern(Ty::Ref(receiver_ty, regseq_model::Provenance::SharedBorrow));
        body.local_decls[regseq_model::LocalId::from_u32(1)].ty = wrapped;
        let TerminatorKind::Call { args, .. } = &mut body.basic_blocks
            [regseq_model::BlockId::START_BLOCK]
            .terminator
            .kind
        else {
            panic!("test body should contain a call");
        };
        args[0] = Operand::Copy(Place {
            local: regseq_model::LocalId::from_u32(1),
            projection: Box::new([regseq_model::PlaceElem::Deref]),
        });

        rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap();

        assert!(matches!(
            body.basic_blocks[regseq_model::BlockId::START_BLOCK]
                .terminator
                .kind,
            TerminatorKind::Call {
                callee: regseq_model::CallCallee::Indirect(_),
                ..
            }
        ));
    }

    #[test]
    fn rejects_virtual_rewrite_when_vtable_map_is_missing() {
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        model.vtables.clear();

        let err = rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap_err();

        assert!(format!("{err}").contains("RegseqModel.vtables"));
    }

    #[test]
    fn rejects_vtable_shim_when_matching_instance_item_is_missing() {
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        for kind in model.item_synthetic_kinds.iter_mut() {
            if matches!(kind, SyntheticItemKind::VTableInstance { .. }) {
                *kind = SyntheticItemKind::None;
            }
        }

        let err = rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap_err();

        assert!(format!("{err}").contains("VTableInstance"));
    }

    #[test]
    fn erased_receiver_t_data_is_uniformly_mut_for_shared_borrow_source() {
        // Per spec.md section 13.2 (lines 4607-4618), the rewrite's
        // erased receiver is uniformly `Ty::Ptr(ErasedPointee,
        // Mutability::Mut)` regardless of the source borrow flavor.
        // `model_with_call` constructs a `&dyn Trait` receiver with
        // `Provenance::SharedBorrow` (the flavor that previously
        // produced `Mutability::Not` and tripped the B-6 per-arg
        // type-match check at validation).
        let (mut model, mut body, _callee) =
            model_with_call(method_shim_kind(), FunctionInstanceFlavor::Virtual, Some(0));
        let receiver_ty = body.local_decls[regseq_model::LocalId::from_u32(1)].ty;
        assert!(
            matches!(
                model.types.resolve(receiver_ty),
                Ty::Ref(_, regseq_model::Provenance::SharedBorrow)
            ),
            "fixture sanity: receiver must be a shared borrow"
        );
        let before = body.local_decls.len();

        rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap();

        // Locals pushed in order: recv, meta, func, data (= _t_data),
        // raw. `_t_data` lives at index `before + 3`.
        let t_data = regseq_model::LocalId::from_u32((before + 3) as u32);
        let t_data_ty = body.local_decls[t_data].ty;
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        });
        let expected_data_ty = model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut));
        assert_eq!(
            t_data_ty, expected_data_ty,
            "_t_data must be Ty::Ptr(unit, Mutability::Mut) regardless of source borrow flavor per spec.md section 13.2 lines 4607-4618"
        );
        match model.types.resolve(t_data_ty) {
            Ty::Ptr(pointee, mutability) => {
                assert_eq!(
                    *mutability,
                    Mutability::Mut,
                    "_t_data mutability must be Mut"
                );
                assert!(
                    matches!(model.types.resolve(*pointee), Ty::Tuple { fields, .. } if fields.is_empty()),
                    "_t_data pointee must be unit per the erased-receiver rule"
                );
            }
            other => panic!("_t_data ty must be Ty::Ptr, got {other:?}"),
        }

        let TerminatorKind::Call { args, .. } = &body.basic_blocks
            [regseq_model::BlockId::START_BLOCK]
            .terminator
            .kind
        else {
            panic!("rewritten terminator must still be a Call");
        };
        let arg0 = args[0].clone();
        match &arg0 {
            Operand::Move(place) => {
                assert_eq!(place.local, t_data, "args[0] must move _t_data");
                assert!(
                    place.projection.is_empty(),
                    "args[0] place must be unprojected"
                );
            }
            other => panic!("args[0] must be Operand::Move(Place::from(_t_data)), got {other:?}"),
        }
        let arg0_ty = super::operand_ty(&mut model, &body, &arg0).unwrap();
        assert_eq!(
            arg0_ty, expected_data_ty,
            "operand_ty(args[0]) must be Ty::Ptr(unit, Mutability::Mut) to satisfy spec.md section 23.6.2 B-6"
        );
    }

    #[test]
    fn leaves_non_vtable_calls_unchanged() {
        let (model, mut body, _callee) =
            model_with_call(SyntheticItemKind::None, FunctionInstanceFlavor::Item, None);
        let before = body.clone();
        let mut model = model;
        rewrite_vtable_indirect_in_model(&mut model, &mut body).unwrap();

        assert_eq!(body, before);
    }
}
