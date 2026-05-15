use regseq_model::{
    validate_body_places, BasicBlock, Body, BridgeInput, DefPath, DefPathData, DefPathHash,
    DefPathSegment, EnumShape, FieldIdx, FieldLayout, FnAbi, FnSig, FunctionInstanceData,
    FunctionInstanceFlavor, GenericArgs, IndexVec, InlineHint, Instance, InstanceKind,
    InvariantViolation, Item, ItemId, ItemKind, Layout, LayoutKind, Linkage, LocalDecl, LocalId,
    Mutability, Operand, Place, PlaceElem, PlaceLocation, PlaceSite, PlaceValidationErrorKind,
    Provenance, Rvalue, Safety, Span, StableCrateId, Statement, StatementKind, SyntheticItemKind,
    Terminator, TerminatorKind, Ty, TyId, UintKind, VariantIdx, VariantLayout,
};

struct Fixture {
    model: regseq_model::RegseqModel,
    bool_ty: TyId,
    u32_ty: TyId,
}

impl Fixture {
    fn new() -> Self {
        let mut model = regseq_model::RegseqModel::with_primary("spec", StableCrateId(7));
        let bool_ty = model.types.intern(Ty::Bool);
        let u32_ty = model.types.intern(Ty::Uint(UintKind::U32));
        Self {
            model,
            bool_ty,
            u32_ty,
        }
    }

    fn item(&mut self, kind: ItemKind, name: &str) -> ItemId {
        let name_id = self.model.strings.intern(name);
        let rendered = self.model.strings.intern(&format!("spec::{name}"));
        let item = Item {
            def_path: DefPath {
                crate_id: self.model.primary_crate,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(name_id),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        };
        let id = self.model.items.push(item);
        self.model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(7),
            local_hash: id.index() as u64,
        });
        self.model
            .item_synthetic_kinds
            .push(SyntheticItemKind::None);
        id
    }

    fn layout(&mut self, kind: LayoutKind) -> Layout {
        Layout {
            size: 8,
            align: 4,
            pack: None,
            uninhabited: false,
            kind,
        }
    }

    fn fields(&mut self, tys: &[TyId]) -> Vec<FieldLayout> {
        tys.iter()
            .enumerate()
            .map(|(idx, ty)| FieldLayout {
                name: self.model.strings.intern(&format!("field{idx}")),
                ty: *ty,
                offset: (idx as u64) * 4,
            })
            .collect()
    }

    fn struct_ty(&mut self, fields: &[TyId]) -> TyId {
        let item = self.item(ItemKind::Struct, "Struct");
        let field_layouts = self.fields(fields);
        let layout = self.layout(LayoutKind::Struct {
            fields: field_layouts,
        });
        self.model.types.intern(Ty::Struct {
            item,
            args: GenericArgs::new(),
            layout,
        })
    }

    fn union_ty(&mut self, fields: &[TyId]) -> TyId {
        let item = self.item(ItemKind::Union, "Union");
        let field_layouts = self.fields(fields);
        let layout = self.layout(LayoutKind::Union {
            fields: field_layouts,
        });
        self.model.types.intern(Ty::Union {
            item,
            args: GenericArgs::new(),
            layout,
        })
    }

    fn tuple_ty(&mut self, fields: &[TyId]) -> TyId {
        let field_layouts = self.fields(fields);
        let layout = self.layout(LayoutKind::Struct {
            fields: field_layouts,
        });
        self.model.types.intern(Ty::Tuple {
            fields: fields.to_vec(),
            layout,
        })
    }

    fn closure_ty(&mut self, fields: &[TyId]) -> TyId {
        let item = self.item(ItemKind::Struct, "Closure");
        let args = GenericArgs::new();
        let field_layouts = self.fields(fields);
        let layout = self.layout(LayoutKind::Struct {
            fields: field_layouts,
        });
        self.model.types.intern(Ty::Struct {
            item,
            args: args.clone(),
            layout,
        });
        self.model.types.intern(Ty::Closure(item, args))
    }

    fn enum_ty(&mut self, variants: Vec<Vec<TyId>>) -> TyId {
        let item = self.item(ItemKind::Enum, "Enum");
        let layouts = variants
            .into_iter()
            .enumerate()
            .map(|(idx, fields)| VariantLayout {
                name: self.model.strings.intern(&format!("Variant{idx}")),
                fields: self.fields(&fields),
                uninhabited: false,
                discriminant: idx as u128,
                tag_value: Some(idx as u128),
            })
            .collect::<Vec<_>>();
        let shape = match layouts.as_slice() {
            [] => EnumShape::Empty,
            [single] => EnumShape::Single {
                variant: single.clone(),
                scalar: None,
            },
            _ => EnumShape::Multiple {
                tag: regseq_model::TagScalar {
                    offset: 0,
                    scalar_ty: self.u32_ty,
                    valid_range: regseq_model::WrappingRange { start: 0, end: 255 },
                },
                encoding: regseq_model::TagEncoding::Direct,
                variants: layouts,
            },
        };
        let layout = self.layout(LayoutKind::Enum { shape });
        self.model.types.intern(Ty::Enum {
            item,
            args: GenericArgs::new(),
            layout,
        })
    }

    fn body_with_statements(&self, local_tys: &[TyId], statements: Box<[Statement]>) -> Body {
        let mut local_decls = IndexVec::new();
        for ty in local_tys {
            local_decls.push(LocalDecl {
                ty: *ty,
                mutability: Mutability::Mut,
                span: Span::dummy(),
                debug_name: None,
            });
        }
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements,
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    fn body_with_place(&self, local_ty: TyId, place: Place) -> Body {
        self.body_with_statements(
            &[local_ty],
            Box::new([Statement {
                kind: StatementKind::Assign(
                    place,
                    Rvalue::Use(Operand::Constant(regseq_model::ConstId::ZERO)),
                ),
                span: Span::dummy(),
            }]),
        )
    }

    fn push_function(&mut self, body: Body) -> regseq_model::InstanceId {
        let unit = self.tuple_ty(&[]);
        let sig = self.model.sigs.intern(FnSig {
            inputs: vec![],
            output: unit,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        let item = self.item(ItemKind::Function, "function");
        let name = self.model.strings.intern("function");
        self.model.instances.push(Instance {
            item,
            generic_args: GenericArgs::new(),
            emission_name: name,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: sig,
                body: Some(body),
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }
}

fn place(local: u32, projection: impl Into<Box<[PlaceElem]>>) -> Place {
    Place {
        local: LocalId::from_u32(local),
        projection: projection.into(),
    }
}

fn field(idx: u32, ty: TyId) -> PlaceElem {
    PlaceElem::Field(FieldIdx::from_u32(idx), ty)
}

fn validate(f: &Fixture, body: &Body) -> Result<(), PlaceValidationErrorKind> {
    validate_body_places(&f.model, body).map_err(|err| err.kind)
}

fn assert_err(f: &Fixture, body: &Body, expected: PlaceValidationErrorKind) {
    let err = validate_body_places(&f.model, body).unwrap_err();
    assert_eq!(err.kind, expected);
}

#[test]
fn i_deref_first_pass() {
    let mut f = Fixture::new();
    let ptr = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Not));
    let body = f.body_with_place(ptr, place(0, [PlaceElem::Deref]));

    assert_eq!(validate(&f, &body), Ok(()));
}

#[test]
fn i_deref_first_reject() {
    let mut f = Fixture::new();
    let ty = f.struct_ty(&[f.u32_ty]);
    let body = f.body_with_place(ty, place(0, [field(0, f.u32_ty), PlaceElem::Deref]));

    assert_err(
        &f,
        &body,
        PlaceValidationErrorKind::DerefNotFirst { index: 1 },
    );
}

#[test]
fn i_downcast_followed_by_field_pass() {
    let mut f = Fixture::new();
    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    let body = f.body_with_place(
        enum_ty,
        place(
            0,
            [
                PlaceElem::Downcast(VariantIdx::from_u32(0)),
                field(0, f.u32_ty),
            ],
        ),
    );

    assert_eq!(validate(&f, &body), Ok(()));
}

#[test]
fn i_downcast_followed_by_field_reject_last() {
    let mut f = Fixture::new();
    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    let body = f.body_with_place(
        enum_ty,
        place(0, [PlaceElem::Downcast(VariantIdx::from_u32(0))]),
    );

    assert_err(
        &f,
        &body,
        PlaceValidationErrorKind::DowncastNotFollowedByField { index: 0 },
    );
}

#[test]
fn i_downcast_followed_by_field_reject_non_field() {
    let mut f = Fixture::new();
    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    let body = f.body_with_place(
        enum_ty,
        place(
            0,
            [
                PlaceElem::Downcast(VariantIdx::from_u32(0)),
                PlaceElem::Deref,
            ],
        ),
    );

    assert_err(
        &f,
        &body,
        PlaceValidationErrorKind::DowncastNotFollowedByField { index: 0 },
    );
}

#[test]
fn type_narrowing_rows_pass() {
    let mut f = Fixture::new();
    let ptr = f.model.types.intern(Ty::Ptr(f.u32_ty, Mutability::Not));
    let ref_ty = f
        .model
        .types
        .intern(Ty::Ref(f.u32_ty, Provenance::SharedBorrow));
    assert_eq!(
        validate(&f, &f.body_with_place(ptr, place(0, [PlaceElem::Deref]))),
        Ok(())
    );
    assert_eq!(
        validate(&f, &f.body_with_place(ref_ty, place(0, [PlaceElem::Deref]))),
        Ok(())
    );

    for aggregate in [
        f.struct_ty(&[f.u32_ty]),
        f.tuple_ty(&[f.u32_ty]),
        f.union_ty(&[f.u32_ty]),
        f.closure_ty(&[f.u32_ty]),
    ] {
        assert_eq!(
            validate(
                &f,
                &f.body_with_place(aggregate, place(0, [field(0, f.u32_ty)]))
            ),
            Ok(())
        );
    }

    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                enum_ty,
                place(
                    0,
                    [
                        PlaceElem::Downcast(VariantIdx::from_u32(0)),
                        field(0, f.u32_ty)
                    ]
                ),
            ),
        ),
        Ok(())
    );

    let array = f.model.types.intern(Ty::Array(f.u32_ty, 4));
    let slice = f.model.types.intern(Ty::Slice(f.u32_ty));
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(array, place(0, [PlaceElem::Index(LocalId::from_u32(0))]))
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(slice, place(0, [PlaceElem::Index(LocalId::from_u32(0))]))
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                array,
                place(
                    0,
                    [PlaceElem::ConstantIndex {
                        offset: 0,
                        min_length: 4,
                        from_end: false
                    }]
                ),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                slice,
                place(
                    0,
                    [PlaceElem::ConstantIndex {
                        offset: 0,
                        min_length: 4,
                        from_end: true
                    }]
                ),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                slice,
                place(
                    0,
                    [PlaceElem::ConstantIndex {
                        offset: 0,
                        min_length: 4,
                        from_end: false
                    }]
                ),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                array,
                place(
                    0,
                    [PlaceElem::Subslice {
                        from: 0,
                        to: 2,
                        from_end: false
                    }]
                ),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                slice,
                place(
                    0,
                    [PlaceElem::Subslice {
                        from: 0,
                        to: 2,
                        from_end: true
                    }]
                ),
            ),
        ),
        Ok(())
    );
}

#[test]
fn type_narrowing_rows_reject() {
    let mut f = Fixture::new();
    let array = f.model.types.intern(Ty::Array(f.u32_ty, 4));
    let slice = f.model.types.intern(Ty::Slice(f.u32_ty));

    assert_err(
        &f,
        &f.body_with_place(f.u32_ty, place(0, [PlaceElem::Deref])),
        PlaceValidationErrorKind::DerefOnNonPointer { ty: f.u32_ty },
    );
    assert_err(
        &f,
        &f.body_with_place(f.u32_ty, place(0, [field(0, f.u32_ty)])),
        PlaceValidationErrorKind::FieldOnNonAggregate {
            ty: f.u32_ty,
            active_variant: None,
        },
    );
    assert_err(
        &f,
        &f.body_with_place(f.u32_ty, place(0, [PlaceElem::Index(LocalId::from_u32(0))])),
        PlaceValidationErrorKind::IndexOnNonIndexable { ty: f.u32_ty },
    );
    assert_err(
        &f,
        &f.body_with_place(
            f.u32_ty,
            place(
                0,
                [PlaceElem::ConstantIndex {
                    offset: 0,
                    min_length: 1,
                    from_end: false,
                }],
            ),
        ),
        PlaceValidationErrorKind::IndexOnNonIndexable { ty: f.u32_ty },
    );
    assert_err(
        &f,
        &f.body_with_place(
            array,
            place(
                0,
                [PlaceElem::ConstantIndex {
                    offset: 0,
                    min_length: 1,
                    from_end: true,
                }],
            ),
        ),
        PlaceValidationErrorKind::ConstantIndexFromEndOnArray { ty: array },
    );
    assert_err(
        &f,
        &f.body_with_place(
            slice,
            place(
                0,
                [PlaceElem::Subslice {
                    from: 0,
                    to: 1,
                    from_end: false,
                }],
            ),
        ),
        PlaceValidationErrorKind::SubsliceFromStartOnSlice { ty: slice },
    );
    assert_err(
        &f,
        &f.body_with_place(
            array,
            place(
                0,
                [PlaceElem::Subslice {
                    from: 0,
                    to: 1,
                    from_end: true,
                }],
            ),
        ),
        PlaceValidationErrorKind::SubsliceFromEndOnArray { ty: array },
    );
    assert_err(
        &f,
        &f.body_with_place(
            f.u32_ty,
            place(
                0,
                [
                    PlaceElem::Downcast(VariantIdx::from_u32(0)),
                    field(0, f.u32_ty),
                ],
            ),
        ),
        PlaceValidationErrorKind::DowncastOnNonEnum { ty: f.u32_ty },
    );
}

#[test]
fn field_idx_in_range_all_aggregate_shapes() {
    let mut f = Fixture::new();
    for ty in [
        f.struct_ty(&[f.u32_ty]),
        f.tuple_ty(&[f.u32_ty]),
        f.union_ty(&[f.u32_ty]),
        f.closure_ty(&[f.u32_ty]),
    ] {
        assert_eq!(
            validate(&f, &f.body_with_place(ty, place(0, [field(0, f.u32_ty)]))),
            Ok(())
        );
    }
    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                enum_ty,
                place(
                    0,
                    [
                        PlaceElem::Downcast(VariantIdx::from_u32(0)),
                        field(0, f.u32_ty)
                    ]
                ),
            ),
        ),
        Ok(())
    );
}

#[test]
fn field_idx_out_of_range_and_type_mismatch_reject() {
    let mut f = Fixture::new();
    let ty = f.struct_ty(&[f.u32_ty]);
    assert_err(
        &f,
        &f.body_with_place(ty, place(0, [field(1, f.u32_ty)])),
        PlaceValidationErrorKind::FieldIdxOutOfRange { actual: 1, max: 1 },
    );
    assert_err(
        &f,
        &f.body_with_place(ty, place(0, [field(0, f.bool_ty)])),
        PlaceValidationErrorKind::FieldTypeMismatch {
            expected: f.u32_ty,
            actual: f.bool_ty,
        },
    );
}

#[test]
fn field_idx_out_of_range_tuple_union_and_closure_reject() {
    let mut f = Fixture::new();
    for ty in [
        f.tuple_ty(&[f.u32_ty]),
        f.union_ty(&[f.u32_ty]),
        f.closure_ty(&[f.u32_ty]),
    ] {
        assert_err(
            &f,
            &f.body_with_place(ty, place(0, [field(1, f.u32_ty)])),
            PlaceValidationErrorKind::FieldIdxOutOfRange { actual: 1, max: 1 },
        );
    }
}

#[test]
fn field_idx_out_of_range_enum_after_downcast_rejects() {
    let mut f = Fixture::new();
    let enum_ty = f.enum_ty(vec![vec![f.u32_ty]]);
    assert_err(
        &f,
        &f.body_with_place(
            enum_ty,
            place(
                0,
                [
                    PlaceElem::Downcast(VariantIdx::from_u32(0)),
                    field(1, f.u32_ty),
                ],
            ),
        ),
        PlaceValidationErrorKind::FieldIdxOutOfRange { actual: 1, max: 1 },
    );
}

#[test]
fn variant_idx_range_accepts_single_and_multiple() {
    let mut f = Fixture::new();
    let single = f.enum_ty(vec![vec![f.u32_ty]]);
    let multiple = f.enum_ty(vec![vec![f.u32_ty], vec![f.u32_ty]]);

    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                single,
                place(
                    0,
                    [
                        PlaceElem::Downcast(VariantIdx::from_u32(0)),
                        field(0, f.u32_ty)
                    ]
                )
            )
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            &f,
            &f.body_with_place(
                multiple,
                place(
                    0,
                    [
                        PlaceElem::Downcast(VariantIdx::from_u32(1)),
                        field(0, f.u32_ty)
                    ]
                )
            )
        ),
        Ok(())
    );
}

#[test]
fn variant_idx_out_of_range_rejects_empty_single_and_multiple() {
    let mut f = Fixture::new();
    let empty = f.enum_ty(vec![]);
    let single = f.enum_ty(vec![vec![f.u32_ty]]);
    let multiple = f.enum_ty(vec![vec![f.u32_ty], vec![f.u32_ty]]);

    assert_err(
        &f,
        &f.body_with_place(
            empty,
            place(
                0,
                [
                    PlaceElem::Downcast(VariantIdx::from_u32(0)),
                    field(0, f.u32_ty),
                ],
            ),
        ),
        PlaceValidationErrorKind::VariantIdxOutOfRange { actual: 0, max: 0 },
    );
    assert_err(
        &f,
        &f.body_with_place(
            single,
            place(
                0,
                [
                    PlaceElem::Downcast(VariantIdx::from_u32(1)),
                    field(0, f.u32_ty),
                ],
            ),
        ),
        PlaceValidationErrorKind::VariantIdxOutOfRange { actual: 1, max: 1 },
    );
    assert_err(
        &f,
        &f.body_with_place(
            multiple,
            place(
                0,
                [
                    PlaceElem::Downcast(VariantIdx::from_u32(2)),
                    field(0, f.u32_ty),
                ],
            ),
        ),
        PlaceValidationErrorKind::VariantIdxOutOfRange { actual: 2, max: 2 },
    );
}

#[test]
fn head_local_out_of_range_reject() {
    let f = Fixture::new();
    let body = f.body_with_place(f.u32_ty, place(1, []));

    assert_err(
        &f,
        &body,
        PlaceValidationErrorKind::HeadLocalOutOfRange {
            local: LocalId::from_u32(1),
            locals_len: 1,
        },
    );
}

#[test]
fn multi_block_body_pass() {
    let mut f = Fixture::new();
    let struct_ty = f.struct_ty(&[f.u32_ty]);
    let good = Statement {
        kind: StatementKind::Assign(
            place(0, [field(0, f.u32_ty)]),
            Rvalue::Use(Operand::Constant(regseq_model::ConstId::ZERO)),
        ),
        span: Span::dummy(),
    };
    let mut blocks = IndexVec::new();
    blocks.push(BasicBlock {
        statements: Box::new([good.clone()]),
        terminator: Terminator {
            kind: TerminatorKind::Goto(regseq_model::BlockId::from_u32(1)),
            span: Span::dummy(),
        },
    });
    blocks.push(BasicBlock {
        statements: Box::new([good]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    });
    let mut local_decls = IndexVec::new();
    local_decls.push(LocalDecl {
        ty: struct_ty,
        mutability: Mutability::Mut,
        span: Span::dummy(),
        debug_name: None,
    });
    let body = Body {
        basic_blocks: blocks,
        local_decls,
        arg_count: 0,
        span: Span::dummy(),
    };

    assert_eq!(validate(&f, &body), Ok(()));
}

#[test]
fn bad_place_mid_block_reject_location() {
    let mut f = Fixture::new();
    let struct_ty = f.struct_ty(&[f.u32_ty]);
    let good = Statement {
        kind: StatementKind::Assign(
            place(0, [field(0, f.u32_ty)]),
            Rvalue::Use(Operand::Constant(regseq_model::ConstId::ZERO)),
        ),
        span: Span::dummy(),
    };
    let bad = Statement {
        kind: StatementKind::Assign(
            place(0, [field(1, f.u32_ty)]),
            Rvalue::Use(Operand::Constant(regseq_model::ConstId::ZERO)),
        ),
        span: Span::dummy(),
    };
    let mut blocks = IndexVec::new();
    blocks.push(BasicBlock {
        statements: Box::new([good.clone()]),
        terminator: Terminator {
            kind: TerminatorKind::Goto(regseq_model::BlockId::from_u32(1)),
            span: Span::dummy(),
        },
    });
    blocks.push(BasicBlock {
        statements: Box::new([good.clone(), good.clone(), bad]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: Span::dummy(),
        },
    });
    let mut local_decls = IndexVec::new();
    local_decls.push(LocalDecl {
        ty: struct_ty,
        mutability: Mutability::Mut,
        span: Span::dummy(),
        debug_name: None,
    });
    let body = Body {
        basic_blocks: blocks,
        local_decls,
        arg_count: 0,
        span: Span::dummy(),
    };

    let err = validate_body_places(&f.model, &body).unwrap_err();
    assert_eq!(
        err.location,
        PlaceLocation {
            block: regseq_model::BlockId::from_u32(1),
            site: PlaceSite::Statement(2),
            projection_step: Some(0),
        }
    );
}

#[test]
fn terminator_place_reject() {
    let mut f = Fixture::new();
    let struct_ty = f.struct_ty(&[f.u32_ty]);
    let body = {
        let mut local_decls = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: struct_ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        });
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: regseq_model::CallCallee::Direct(regseq_model::InstanceId::ZERO),
                    args: Box::new([]),
                    destination: place(0, [field(1, f.u32_ty)]),
                    target: None,
                },
                span: Span::dummy(),
            },
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    };

    let err = validate_body_places(&f.model, &body).unwrap_err();
    assert_eq!(err.location.site, PlaceSite::Terminator);
}

#[test]
fn joint_validator_integration_pass_and_reject() {
    let mut passing = Fixture::new();
    let body = passing.body_with_place(passing.u32_ty, place(0, []));
    passing.push_function(body);
    assert_eq!(
        passing
            .model
            .validate(&BridgeInput::matching(&passing.model)),
        Ok(())
    );

    let mut failing = Fixture::new();
    let body = failing.body_with_place(failing.u32_ty, place(1, []));
    let instance = failing.push_function(body);
    let err = failing
        .model
        .validate(&BridgeInput::matching(&failing.model))
        .unwrap_err();
    match err {
        InvariantViolation::PlaceWalk {
            instance: actual,
            error,
        } => {
            assert_eq!(actual, instance);
            assert_eq!(
                error.location,
                PlaceLocation {
                    block: regseq_model::BlockId::START_BLOCK,
                    site: PlaceSite::Statement(0),
                    projection_step: None,
                }
            );
        }
        other => panic!("unexpected validation error: {other:?}"),
    }
}

#[test]
fn joint_validator_rejects_dangling_direct_call_instance() {
    let mut failing = Fixture::new();
    let mut body = failing.body_with_statements(&[failing.u32_ty], Box::new([]));
    body.basic_blocks[regseq_model::BlockId::START_BLOCK].terminator = Terminator {
        kind: TerminatorKind::Call {
            callee: regseq_model::CallCallee::Direct(regseq_model::InstanceId::from_u32(99)),
            args: Box::new([]),
            destination: place(0, []),
            target: None,
        },
        span: Span::dummy(),
    };
    let owner = failing.push_function(body);
    let err = failing
        .model
        .validate(&BridgeInput::matching(&failing.model))
        .unwrap_err();
    match err {
        InvariantViolation::DirectCalleeOutOfRange {
            owner: actual_owner,
            callee,
            ..
        } => {
            assert_eq!(actual_owner, owner);
            assert_eq!(callee, regseq_model::InstanceId::from_u32(99));
        }
        other => panic!("expected DirectCalleeOutOfRange, got {other:?}"),
    }
}
