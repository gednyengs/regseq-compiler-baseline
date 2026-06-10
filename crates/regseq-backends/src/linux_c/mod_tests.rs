//! linux_c T04 unit seam: synthetic minimal models drive the API surface,
//! the D04 ordering landmarks, the artifact set, and one negative test per
//! fail-loud diagnostic (R01/R02/R04, S01/S02/S09/S10, I06).

use regseq_model::{
    AggregateKind, BasicBlock, Body, BusType, Const, CrateId, DefPath, EnumShape, FieldLayout,
    FnAbi, FnSig, FunctionInstanceData, FunctionInstanceFlavor, Instance, InstanceKind, Item,
    ItemKind, Layout, LayoutKind, LifecycleOps, LocalDecl, MatchEntry, Mutability, Operand, Place,
    Provenance, RegseqModel, Rvalue, Safety, ScalarLayouts, SizeAlign, SourceFileId, Span,
    StableCrateId, Statement, StatementKind, TagEncoding, TagScalar, TargetInfo, Terminator,
    TerminatorKind, Ty, TyId, UintKind, VariantIdx, VariantLayout, VariantRange, WrappingRange,
};

use super::{emit_linux_c, EmitError};

// -- model-construction helpers (local; mirrors the sibling generic_c test
//    helpers in shape, shares no code per F-1) ------------------------------

fn model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("t04", StableCrateId(1));
    let triple = model.strings.intern("aarch64");
    let data_layout = model.strings.intern("dl");
    let sa = |size, align| SizeAlign { size, align };
    model.target = TargetInfo {
        triple,
        pointer_width: 64,
        endian: regseq_model::Endian::Little,
        data_layout,
        scalar_layouts: ScalarLayouts {
            bool_: sa(1, 1),
            char_: sa(4, 4),
            int: [sa(1, 1), sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16), sa(8, 8)],
            uint: [sa(1, 1), sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16), sa(8, 8)],
            float: [sa(2, 2), sa(4, 4), sa(8, 8), sa(16, 16)],
            fn_ptr: sa(8, 8),
            data_ptr: sa(8, 8),
        },
    };
    model
}

fn span() -> Span {
    Span {
        file: SourceFileId::SYNTHESIZED,
        lo: 0,
        hi: 0,
    }
}

fn item(m: &mut RegseqModel, kind: ItemKind, rendered: &str) -> regseq_model::ItemId {
    let rid = m.strings.intern(rendered);
    let id = m.items.push(Item {
        def_path: DefPath {
            crate_id: CrateId::ZERO,
            segments: Vec::new(),
            rendered: rid,
        },
        kind,
        span: span(),
        linkage: regseq_model::Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
        is_result: false,
    });
    m.item_def_paths.push(regseq_model::DefPathHash {
        stable_crate_id: StableCrateId(1),
        local_hash: id.as_u32() as u64,
    });
    id
}

fn struct_layout(size: u64, align: u64, fields: Vec<FieldLayout>) -> Layout {
    Layout {
        size,
        align,
        pack: None,
        uninhabited: false,
        kind: LayoutKind::Struct { fields },
    }
}

fn push_struct(m: &mut RegseqModel, rendered: &str, layout: Layout) -> TyId {
    let it = item(m, ItemKind::Struct, rendered);
    m.types.intern(Ty::Struct {
        item: it,
        args: regseq_model::GenericArgs::new(),
        layout,
    })
}

/// A driver entry for `rendered` with the given bus/match/ops; returns the
/// model ready for `emit_linux_c`.
fn driver_model(
    bus: Option<BusType>,
    match_set: Vec<MatchEntry>,
    ops: LifecycleOps,
) -> RegseqModel {
    let mut m = model();
    let ty = push_struct(&mut m, "Minimal", struct_layout(0, 1, vec![]));
    let driver_item = match &m.types[ty] {
        Ty::Struct { item, .. } => *item,
        _ => unreachable!(),
    };
    let name = m.strings.intern("minimal");
    m.device_lifecycle.drivers.insert(
        driver_item,
        regseq_model::DriverLifecycle {
            descriptor: regseq_model::DriverDescriptor {
                name,
                match_set,
                device_class: regseq_model::ClassTag::Generic,
                bus_type: bus,
                dma_present: false,
            },
            handle: regseq_model::DriverHandle::default(),
            ops,
        },
    );
    m
}

fn amba_match() -> Vec<MatchEntry> {
    vec![MatchEntry::Amba {
        id: 0x0004_1011,
        mask: 0x000f_ffff,
    }]
}

/// Intern the marked RegseqError (2 fieldless variants, direct u8 tag) and
/// the niche-encoded `Result<(), RegseqError>` carrier; returns
/// `(result_ty, regseq_error_ty)`. `err_variant_names` lets the
/// ErrnoMapping negative test inject an unmapped variant.
fn result_carrier(
    m: &mut RegseqModel,
    err_variant_names: [&str; 2],
) -> (TyId, TyId) {
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));

    let err_item = item(m, ItemKind::Enum, "RegseqError");
    // Mark it as the RegseqError framework type (sparse table: fill up to
    // the item).
    while m.domain_attrs.item_attrs.len() <= err_item.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[err_item].push(regseq_model::AttrData::FrameworkType(
        regseq_model::FrameworkType::RegseqError,
    ));
    let err_variants: Vec<VariantLayout> = err_variant_names
        .iter()
        .enumerate()
        .map(|(i, n)| VariantLayout {
            name: m.strings.intern(n),
            fields: vec![],
            uninhabited: false,
            discriminant: i as u128,
            tag_value: Some(i as u128),
        })
        .collect();
    let err_ty = m.types.intern(Ty::Enum {
        item: err_item,
        args: regseq_model::GenericArgs::new(),
        layout: Layout {
            size: 1,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: EnumShape::Multiple {
                    tag: TagScalar {
                        offset: 0,
                        scalar_ty: u8t,
                        valid_range: WrappingRange { start: 0, end: 1 },
                    },
                    encoding: TagEncoding::Direct,
                    variants: err_variants,
                },
            },
        },
    });

    let result_item = item(m, ItemKind::Enum, "Result");
    m.items[result_item].is_result = true;
    let ok_name = m.strings.intern("Ok");
    let err_name = m.strings.intern("Err");
    let payload_name = m.strings.intern("0");
    let result_ty = m.types.intern(Ty::Enum {
        item: result_item,
        args: regseq_model::GenericArgs::new(),
        layout: Layout {
            size: 1,
            align: 1,
            pack: None,
            uninhabited: false,
            kind: LayoutKind::Enum {
                shape: EnumShape::Multiple {
                    tag: TagScalar {
                        offset: 0,
                        scalar_ty: u8t,
                        valid_range: WrappingRange { start: 0, end: 2 },
                    },
                    encoding: TagEncoding::Niche {
                        untagged_variant: VariantIdx::from_u32(1),
                        niche_variants: VariantRange {
                            start: VariantIdx::from_u32(0),
                            end: VariantIdx::from_u32(0),
                        },
                        niche_start: 2,
                    },
                    variants: vec![
                        VariantLayout {
                            name: ok_name,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 0,
                            tag_value: Some(2),
                        },
                        VariantLayout {
                            name: err_name,
                            fields: vec![FieldLayout {
                                name: payload_name,
                                ty: err_ty,
                                offset: 0,
                            }],
                            uninhabited: false,
                            discriminant: 1,
                            tag_value: None,
                        },
                    ],
                },
            },
        },
    });
    (result_ty, err_ty)
}

/// Push an `on_init` function instance whose body is `_0 = Ok(()); return;`
/// over `&mut <driver_ty>`; returns its `InstanceRef` for `LifecycleOps`.
fn push_ok_on_init(
    m: &mut RegseqModel,
    driver_ty: TyId,
    result_ty: TyId,
) -> regseq_model::InstanceRef {
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let self_ty = m.types.intern(Ty::Ref(driver_ty, Provenance::UniqueBorrow));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let unit_const = m.consts.intern(Const::ZST);
    let _ = unit; // the Ok payload is the ZST const, not a typed local
    let mut body = Body::default();
    body.local_decls.push(LocalDecl {
        ty: result_ty,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.local_decls.push(LocalDecl {
        ty: self_ty,
        mutability: Mutability::Not,
        span: span(),
        debug_name: Some(m.strings.intern("self")),
    });
    body.arg_count = 1;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(regseq_model::LocalId::RETURN_PLACE),
                Rvalue::Aggregate(
                    AggregateKind::Enum(result_ty, VariantIdx::from_u32(0)),
                    Box::new([Operand::Constant(unit_const)]),
                ),
            ),
            span: span(),
        }]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: span(),
        },
    });
    let fn_item = item(m, ItemKind::Function, "on_init");
    let emission = m.strings.intern("on_init");
    let id = m.instances.push(Instance {
        item: fn_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: Some(body),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    m.instance_ref_of(id)
}

// -- diagnostics frame (R04/S10/I06): one negative test per diagnostic ------

#[test]
fn no_driver_is_a_named_driver_error() {
    let err = emit_linux_c(&model()).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("no recognized")));
}

#[test]
fn pci_bus_names_the_delivering_unit() {
    let m = driver_model(Some(BusType::Pci), amba_match(), LifecycleOps::default());
    let err = emit_linux_c(&m).unwrap_err();
    let EmitError::UnsupportedInUnit { what, unit } = err else {
        panic!("expected UnsupportedInUnit, got {err:?}");
    };
    assert!(what.contains("PCI scaffold"));
    assert_eq!(unit, "backend/linux-c-e1000");
}

#[test]
fn non_amba_bus_is_unsupported_in_unit() {
    let m = driver_model(Some(BusType::I2c), amba_match(), LifecycleOps::default());
    assert!(matches!(
        emit_linux_c(&m).unwrap_err(),
        EmitError::UnsupportedInUnit { .. }
    ));
}

#[test]
fn missing_bus_is_a_named_driver_error() {
    let m = driver_model(None, amba_match(), LifecycleOps::default());
    let err = emit_linux_c(&m).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("bus = \"amba\"")));
}

#[test]
fn non_amba_match_entry_is_unsupported_in_unit() {
    let m = driver_model(
        Some(BusType::Amba),
        vec![MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100e,
            subsystem: None,
            class: None,
        }],
        LifecycleOps::default(),
    );
    assert!(matches!(
        emit_linux_c(&m).unwrap_err(),
        EmitError::UnsupportedInUnit { .. }
    ));
}

#[test]
fn empty_match_set_is_a_named_driver_error() {
    let m = driver_model(Some(BusType::Amba), vec![], LifecycleOps::default());
    let err = emit_linux_c(&m).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("match set")));
}

#[test]
fn unmapped_regseq_error_variant_fails_loud() {
    let mut m = driver_model(Some(BusType::Amba), amba_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "Bogus"]);
    let on_init = push_ok_on_init(&mut m, driver_ty, result_ty);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_linux_c(&m).unwrap_err();
    assert_eq!(err, EmitError::ErrnoMapping("Bogus".into()));
}

// -- the minimal AMBA scaffold (R01/R02/S01/S09) -----------------------------

/// The minimal emitting model: fieldless AMBA driver, `on_init` = `Ok(())`.
fn minimal_amba_model() -> RegseqModel {
    let mut m = driver_model(Some(BusType::Amba), amba_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let on_init = push_ok_on_init(&mut m, driver_ty, result_ty);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    m
}

#[test]
fn minimal_amba_model_emits_the_artifact_set() {
    let emit = emit_linux_c(&minimal_amba_model()).expect("emit");
    assert_eq!(emit.module_name, "minimal");
    assert_eq!(emit.kbuild, "obj-m += minimal.o\n");
    assert!(!emit.tu.is_empty());
}

#[test]
fn emission_is_byte_identical_across_two_calls() {
    let a = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    let b = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    assert_eq!(a, b);
}

#[test]
fn tu_carries_the_d04_ordering_landmarks() {
    let tu = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`:\n{tu}"))
    };
    // includes → driver state → lowered bodies → callbacks → match table →
    // driver struct → module macros (GRILL D04).
    let includes = pos("#include <linux/amba/bus.h>");
    let container = pos("struct minimal_priv {");
    let body = pos("static struct Result minimal_on_init(");
    let probe = pos("static int minimal_probe(struct amba_device *adev, const struct amba_id *id)");
    let remove = pos("static void minimal_remove(struct amba_device *adev)");
    let table = pos("static const struct amba_id minimal_ids[]");
    let devtable = pos("MODULE_DEVICE_TABLE(amba, minimal_ids);");
    let drvstruct = pos("static struct amba_driver minimal_driver = {");
    let macros = pos("module_amba_driver(minimal_driver);");
    let license = pos("MODULE_LICENSE(\"GPL\");");
    assert!(includes < container, "includes precede driver state");
    assert!(container < body, "driver state precedes lowered bodies");
    assert!(body < probe, "lowered bodies precede callbacks");
    assert!(probe < remove);
    assert!(remove < table, "callbacks precede the match table");
    assert!(table < devtable);
    assert!(devtable < drvstruct, "match table precedes the driver struct");
    assert!(drvstruct < macros, "driver struct precedes module macros");
    assert!(macros < license);
}

#[test]
fn probe_carries_create_associate_and_the_negated_adapter() {
    let tu = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    // create → associate (canonical order, §C.3).
    let create = tu.find("devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL)").unwrap();
    let associate = tu.find("amba_set_drvdata(adev, priv);").unwrap();
    let call = tu.find("minimal_on_init(&priv->inner)").unwrap();
    assert!(create < associate && associate < call);
    // The adapter: Ok tag (niche value 2) compare + per-variant NEGATED
    // errno arms + corrupt-discriminant default.
    assert!(tu.contains("if (__tag != 0x2)"));
    assert!(tu.contains("return -EIO;"), "HardwareError -> -EIO");
    assert!(tu.contains("return -EBUSY;"), "DeviceBusy -> -EBUSY");
    assert!(tu.contains("default:"), "corrupt-discriminant defense");
    // Registration is module-scoped (no scenario functions, no main).
    assert!(!tu.contains("int main("));
    assert!(!tu.contains("__rseq_lc_"));
}

#[test]
fn full_match_set_renders_with_zero_sentinel() {
    let mut m = minimal_amba_model();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.match_set.push(MatchEntry::Amba {
        id: 0x0004_2222,
        mask: 0x000f_ffff,
    });
    let tu = emit_linux_c(&m).unwrap().tu;
    let first = tu.find(".id\t= 0x00041011,").expect("first entry");
    let second = tu.find(".id\t= 0x00042222,").expect("second entry (FULL match_set)");
    let sentinel = tu.find("{ 0, 0 },").expect("zero sentinel");
    assert!(first < second && second < sentinel);
}

#[test]
fn deferred_callbacks_fail_loud_until_t07() {
    for cb in ["on_surprise_remove", "on_init_failure", "on_system_shutdown"] {
        let mut m = driver_model(Some(BusType::Amba), amba_match(), LifecycleOps::default());
        let driver_ty = m
            .types
            .iter()
            .find(|(_, t)| matches!(t, Ty::Struct { .. }))
            .map(|(id, _)| id)
            .unwrap();
        let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
        let r = push_ok_on_init(&mut m, driver_ty, result_ty);
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        match cb {
            "on_surprise_remove" => driver.ops.on_surprise_remove = Some(r),
            "on_init_failure" => driver.ops.on_init_failure = Some(r),
            _ => driver.ops.on_system_shutdown = Some(r),
        }
        let err = emit_linux_c(&m).unwrap_err();
        assert!(
            matches!(err, EmitError::Unsupported(ref s) if s.contains(cb) && s.contains("T07")),
            "{cb} must fail loud naming itself and T07; got {err:?}"
        );
    }
}
