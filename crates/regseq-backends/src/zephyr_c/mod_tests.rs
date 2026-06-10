//! zephyr_c T05 unit seam: synthetic minimal models drive the API surface,
//! the scaffold §1 ordering landmarks, the artifact set, and one negative
//! test per fail-loud diagnostic of the D12 frame (R01/R02/R04, S01/S02,
//! S06/S11, I06).
//!
//! Model-construction helpers are local copies (mirrors the sibling
//! linux_c/generic_c test helpers in shape, shares no code per F-1).

use regseq_model::{
    BasicBlock, Body, BusType, ClassTag, ConfigOp, CrateId, DefPath, EnumShape, FieldLayout, FnAbi,
    FnSig, FunctionInstanceData, FunctionInstanceFlavor, Instance, InstanceKind, IntrinsicKind,
    Item, ItemKind, Layout, LayoutKind, LifecycleOps, LocalDecl, MatchEntry, MmioBinding,
    Mutability, Place, RegseqModel, ResourceField, ResourceKind, Safety, ScalarLayouts, SizeAlign,
    SourceFileId, Span, StableCrateId, TagEncoding, TagScalar, TargetInfo, Terminator,
    TerminatorKind, Ty, TyId, UintKind, UserDataField, UserDataKind, VariantIdx, VariantLayout,
    VariantRange, WrappingRange,
};

use super::{emit_zephyr_c, EmitError};

// -- model-construction helpers ----------------------------------------------

fn model() -> RegseqModel {
    let mut model = RegseqModel::with_primary("t05", StableCrateId(1));
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

fn dt_match_for(m: &mut RegseqModel, compatible: &str) -> Vec<MatchEntry> {
    vec![MatchEntry::Devicetree {
        compatible: m.strings.intern(compatible),
    }]
}

fn resolved_mmio(m: &mut RegseqModel, name: &str) -> ResourceField {
    ResourceField {
        name: m.strings.intern(name),
        kind: ResourceKind::Mmio,
        binding: MmioBinding {
            id: None,
            bar_index: None,
            bar_type: regseq_model::BarType::Mem,
            effective_bar: None,
            base: Some(0x0904_0000),
            size: Some(0x1000),
        },
    }
}

/// Push a driver entry named `minimal` with the given bus/match/ops/handle.
fn driver_into(
    m: &mut RegseqModel,
    bus: Option<BusType>,
    match_set: Vec<MatchEntry>,
    ops: LifecycleOps,
    handle: regseq_model::DriverHandle,
) -> TyId {
    let ty = push_struct(m, "Minimal", struct_layout(0, 1, vec![]));
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
                device_class: ClassTag::Generic,
                bus_type: bus,
                dma_present: false,
            },
            handle,
            ops,
        },
    );
    ty
}

/// The minimal emitting model: one-region devicetree driver, no callbacks.
fn minimal_dt_model() -> RegseqModel {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    let region = resolved_mmio(&mut m, "regs");
    driver_into(
        &mut m,
        Some(BusType::Amba),
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![region],
            user_data_fields: vec![],
        },
    );
    m
}

fn unit_ty(m: &mut RegseqModel) -> TyId {
    m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    })
}

/// Push a foreign (body-less) intrinsic-anchored function instance.
fn push_intrinsic(
    m: &mut RegseqModel,
    rendered: &str,
    kind: IntrinsicKind,
    inputs: Vec<TyId>,
    output: TyId,
) -> regseq_model::InstanceId {
    let fn_item = item(m, ItemKind::Function, rendered);
    while m.domain_attrs.item_attrs.len() <= fn_item.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[fn_item].push(regseq_model::AttrData::Intrinsic(kind));
    let sig = m.sigs.intern(FnSig {
        inputs,
        output,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission = m.strings.intern(rendered);
    m.instances.push(Instance {
        item: fn_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    })
}

/// Push a callback fn whose single block calls `callee`; returns its
/// `InstanceRef` — drives the D12 frame scan (the call site is reachable
/// from the lifecycle root).
fn push_callback_calling(
    m: &mut RegseqModel,
    callee: regseq_model::InstanceId,
    local_ty: TyId,
) -> regseq_model::InstanceRef {
    let sig = m.sigs.intern(FnSig {
        inputs: vec![],
        output: local_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let mut body = Body::default();
    body.local_decls.push(LocalDecl {
        ty: local_ty,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.arg_count = 0;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                args: Box::new([]),
                destination: Place::from(regseq_model::LocalId::RETURN_PLACE),
                target: None,
            },
            span: span(),
        },
    });
    let fn_item = item(m, ItemKind::Function, "callback");
    let emission = m.strings.intern("callback");
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

/// An `Option<T>`-shaped enum whose `None` variant carries the given niche
/// tag value — `0` models the inherited zero-image shapes
/// (Option<NonZeroU8>), `2` models a beyond-the-guarantee shape
/// (Option<bool>).
fn option_enum(m: &mut RegseqModel, rendered: &str, none_tag: u128) -> TyId {
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let none_name = m.strings.intern("None");
    let some_name = m.strings.intern("Some");
    let payload_name = m.strings.intern("0");
    let it = item(m, ItemKind::Enum, rendered);
    m.types.intern(Ty::Enum {
        item: it,
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
                        niche_start: none_tag,
                    },
                    variants: vec![
                        VariantLayout {
                            name: none_name,
                            fields: vec![],
                            uninhabited: false,
                            discriminant: 0,
                            tag_value: Some(none_tag),
                        },
                        VariantLayout {
                            name: some_name,
                            fields: vec![FieldLayout {
                                name: payload_name,
                                ty: u8t,
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
    })
}

// -- diagnostics frame (R04/S11/I06): one negative test per diagnostic ------

#[test]
fn no_driver_is_a_named_driver_error() {
    let err = emit_zephyr_c(&model()).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("no recognized")));
}

#[test]
fn two_drivers_is_a_named_driver_error() {
    let mut m = model();
    let match_a = dt_match_for(&mut m, "a,b");
    let match_b = dt_match_for(&mut m, "c,d");
    driver_into(
        &mut m,
        None,
        match_a,
        LifecycleOps::default(),
        Default::default(),
    );
    driver_into(
        &mut m,
        None,
        match_b,
        LifecycleOps::default(),
        Default::default(),
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("more than one")));
}

#[test]
fn pci_bus_names_the_e1000_unit() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    driver_into(
        &mut m,
        Some(BusType::Pci),
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::UnsupportedInUnit { what, unit } = err else {
        panic!("expected UnsupportedInUnit, got {err:?}");
    };
    assert!(what.contains("PCIe scaffold"));
    assert_eq!(unit, "backend/zephyr-c-e1000");
}

#[test]
fn deferred_bus_is_unsupported_in_unit() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    driver_into(
        &mut m,
        Some(BusType::I2c),
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    assert!(matches!(
        emit_zephyr_c(&m).unwrap_err(),
        EmitError::UnsupportedInUnit { .. }
    ));
}

#[test]
fn non_generic_class_is_rejected_citing_a16() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.device_class = ClassTag::Network;
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-9") && s.contains("A-16"), "{s}");
}

#[test]
fn empty_match_set_is_a_named_driver_error() {
    let m = {
        let mut m = model();
        driver_into(
            &mut m,
            None,
            vec![],
            LifecycleOps::default(),
            Default::default(),
        );
        m
    };
    let err = emit_zephyr_c(&m).unwrap_err();
    assert!(matches!(err, EmitError::Driver(ref s) if s.contains("match set")));
}

#[test]
fn amba_only_match_set_is_rejected_naming_the_fix() {
    let mut m = model();
    driver_into(
        &mut m,
        Some(BusType::Amba),
        vec![MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        }],
        LifecycleOps::default(),
        Default::default(),
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    // A-13: the diagnostic must NAME THE FIX.
    assert!(
        s.contains("B-5") && s.contains("companion") && s.contains("Devicetree"),
        "{s}"
    );
}

#[test]
fn mixed_match_set_defers_to_the_convergence_unit() {
    let mut m = model();
    let mut match_set = dt_match_for(&mut m, "regseq,pl011");
    match_set.push(MatchEntry::Amba {
        id: 0x0004_1011,
        mask: 0x000f_ffff,
    });
    driver_into(
        &mut m,
        Some(BusType::Amba),
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    // The consumption rule is unratified (GRILL D03a): defer loudly, do not
    // decide.
    assert!(
        s.contains("unratified") && s.contains("spec-convergence-pl011"),
        "{s}"
    );
}

#[test]
fn multi_compatible_match_set_is_unsupported() {
    let mut m = model();
    let mut match_set = dt_match_for(&mut m, "regseq,pl011");
    match_set.extend(dt_match_for(&mut m, "regseq,pl011-alt"));
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    assert!(matches!(
        emit_zephyr_c(&m).unwrap_err(),
        EmitError::Unsupported(ref s) if s.contains("multi-compatible")
    ));
}

#[test]
fn on_deinit_is_rejected_citing_d06_not_unsupported() {
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let cb = push_intrinsic(&mut m, "dummy", IntrinsicKind::PollUntil, vec![], ut);
    let on_deinit = push_callback_calling(&mut m, cb, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_deinit = Some(on_deinit);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    // D06 re-cite: names the v4.4 fork state AND the delivering unit; the
    // wording is a scheduling statement, not a capability denial.
    assert!(
        s.contains("device_deinit()") && s.contains("spec-convergence-pl011"),
        "{s}"
    );
    assert!(!s.to_lowercase().contains("unsupported"), "{s}");
}

#[test]
fn on_surprise_remove_is_rejected_citing_b7() {
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let cb = push_intrinsic(&mut m, "dummy", IntrinsicKind::PollUntil, vec![], ut);
    let on_sr = push_callback_calling(&mut m, cb, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_surprise_remove = Some(on_sr);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-7"), "{s}");
}

#[test]
fn on_system_shutdown_is_designed_and_scheduled_per_a18() {
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let cb = push_intrinsic(&mut m, "dummy", IntrinsicKind::PollUntil, vec![], ut);
    let on_sd = push_callback_calling(&mut m, cb, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_system_shutdown = Some(on_sd);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    // D12: the exact disposition is a scheduling statement — never
    // "unsupported".
    assert!(s.contains("designed and scheduled per A-18"), "{s}");
    assert!(!s.to_lowercase().contains("unsupported"), "{s}");
}

#[test]
fn port_io_region_is_rejected_citing_b2() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    let mut region = resolved_mmio(&mut m, "ports");
    region.binding.bar_type = regseq_model::BarType::Io;
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![region],
            user_data_fields: vec![],
        },
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-2"), "{s}");
}

#[test]
fn two_resolved_regions_are_rejected_citing_b10() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    let a = resolved_mmio(&mut m, "regs_a");
    let b = resolved_mmio(&mut m, "regs_b");
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![a, b],
            user_data_fields: vec![],
        },
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(
        s.contains("B-10") && s.contains("DEVICE_MMIO_NAMED_"),
        "{s}"
    );
}

#[test]
fn option_copy_beyond_the_zero_image_is_rejected_citing_b8() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    // Option<bool>-shaped: None is niched at tag value 2 — the all-zero
    // image decodes to Some(false), NOT None.
    let opt = option_enum(&mut m, "OptBool", 2);
    let field_name = m.strings.intern("flag");
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![],
            user_data_fields: vec![UserDataField {
                name: field_name,
                ty: opt,
                kind: UserDataKind::OptionCopy,
            }],
        },
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-8") && s.contains("zero-image"), "{s}");
}

#[test]
fn option_copy_within_the_zero_image_passes_through() {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    // Option<NonZeroU8>-shaped: None IS the zero niche — the inherited
    // guarantee holds and the field rides the zero-filled static image.
    let opt = option_enum(&mut m, "OptNz", 0);
    let field_name = m.strings.intern("count");
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![],
            user_data_fields: vec![UserDataField {
                name: field_name,
                ty: opt,
                kind: UserDataKind::OptionCopy,
            }],
        },
    );
    let emit = emit_zephyr_c(&m).expect("zero-image OptionCopy must emit");
    assert!(emit.tu.contains("struct OptNz count;"), "{}", emit.tu);
}

#[test]
fn poll_until_is_rejected_citing_b1() {
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let poll = push_intrinsic(&mut m, "poll_until", IntrinsicKind::PollUntil, vec![], ut);
    let on_init = push_callback_calling(&mut m, poll, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(
        s.contains("B-1") && s.contains("recognized-not-lowered"),
        "{s}"
    );
}

#[test]
fn config_write_subword_is_rejected_citing_b3() {
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let u16t = m.types.intern(Ty::Uint(UintKind::U16));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let wc = push_intrinsic(
        &mut m,
        "write_config",
        IntrinsicKind::Config(ConfigOp::Write),
        vec![u32t, u16t],
        ut,
    );
    let on_init = push_callback_calling(&mut m, wc, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-3") && s.contains("RMW"), "{s}");
}

#[test]
fn config_width8_is_rejected_citing_b3() {
    let mut m = minimal_dt_model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let u64t = m.types.intern(Ty::Uint(UintKind::U64));
    let rc = push_intrinsic(
        &mut m,
        "read_config",
        IntrinsicKind::Config(ConfigOp::Read),
        vec![u32t],
        u64t,
    );
    let on_init = push_callback_calling(&mut m, rc, u64t);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-3") && s.contains("two-word"), "{s}");
}

#[test]
fn specified_config_shape_on_non_pci_is_rejected_citing_b4() {
    let mut m = minimal_dt_model();
    let u16t = m.types.intern(Ty::Uint(UintKind::U16));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    // A width-2 READ is one of the SPECIFIED PCI shapes (§B.4) — here it
    // still has no carrier because the unit has no PCIe scaffold.
    let rc = push_intrinsic(
        &mut m,
        "read_config",
        IntrinsicKind::Config(ConfigOp::Read),
        vec![u32t],
        u16t,
    );
    let on_init = push_callback_calling(&mut m, rc, u16t);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(
        s.contains("BRM-001") && s.contains("backend/zephyr-c-e1000"),
        "{s}"
    );
}

#[test]
fn authored_body_lowering_is_the_shrinking_placeholder() {
    // The T06-T08 placeholder: a clean on_init (no frame-scan hits) is
    // still Unsupported until the body banks land — fail-loud, named.
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let helper = push_intrinsic(
        &mut m,
        "mmio_read",
        IntrinsicKind::Mmio(regseq_model::MmioOp::Read),
        vec![],
        ut,
    );
    let on_init = push_callback_calling(&mut m, helper, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    assert!(matches!(
        err,
        EmitError::Unsupported(ref s) if s.contains("T06-T08")
    ));
}

// -- the minimal scaffold (R01/R02, S01/S02): artifact set + §1 landmarks ----

#[test]
fn artifact_set_is_exactly_tu_plus_binding_yaml() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    assert_eq!(emit.driver_name, "minimal");
    assert!(!emit.tu.is_empty());
    assert!(!emit.binding_yaml.is_empty());
}

#[test]
fn tu_skeleton_landmarks_appear_in_scaffold_order() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    let tu = &emit.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("landmark `{needle}` missing from TU:\n{tu}"))
    };
    // §1 order, top to bottom.
    let p_compat = pos("#define DT_DRV_COMPAT regseq_pl011");
    let p_include = pos("#include <zephyr/device.h>");
    let p_inner = pos("struct Minimal {");
    let p_config = pos("struct minimal_config {");
    let p_data = pos("struct minimal_data {");
    let p_init = pos("static int minimal_init(const struct device *dev)");
    let p_inst = pos("#define RSEQ_INIT(n)");
    let p_foreach = pos("DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)");
    let p_guard = pos("BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1");
    assert!(p_compat < p_include, "compat pin first");
    assert!(p_include < p_inner, "includes before state");
    assert!(
        p_inner < p_config && p_config < p_data,
        "inner, config, data"
    );
    assert!(p_data < p_init, "state before the init fn");
    assert!(p_init < p_inst, "init fn before instantiation");
    assert!(p_inst < p_foreach, "macro before foreach");
    assert!(p_foreach < p_guard, "single-instance guard last");
}

#[test]
fn mmio_macros_are_first_members_and_inner_is_embedded() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    let tu = &emit.tu;
    // DEVICE_MMIO_ROM is the config struct's FIRST member.
    let config_body = tu
        .split("struct minimal_config {")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap();
    assert!(
        config_body.trim_start().starts_with("DEVICE_MMIO_ROM;"),
        "DEVICE_MMIO_ROM must be the FIRST config member:\n{config_body}"
    );
    // DEVICE_MMIO_RAM is the data struct's FIRST member; the spec driver is
    // embedded (A-17) with the retained state word.
    let data_body = tu
        .split("struct minimal_data {")
        .nth(1)
        .unwrap()
        .split("};")
        .next()
        .unwrap();
    assert!(
        data_body.trim_start().starts_with("DEVICE_MMIO_RAM;"),
        "DEVICE_MMIO_RAM must be the FIRST data member:\n{data_body}"
    );
    assert!(data_body.contains("struct Minimal inner;"), "{data_body}");
    assert!(data_body.contains("uint32_t __rseq_state;"), "{data_body}");
}

#[test]
fn instantiation_block_carries_the_d13_hardcoded_placement() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    let tu = &emit.tu;
    assert!(tu.contains("POST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE"));
    assert!(
        tu.contains("hardcoded (ratified D13: no BackendOptions knob this unit)"),
        "the D13 decision must be documented in the TU"
    );
    assert!(tu.contains("NULL /* api: ClassTag::Generic only (A-16) */"));
    assert!(tu.contains("DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))"));
    assert!(tu.contains("DEVICE_DT_INST_DEFINE(n, minimal_init, NULL,"));
}

#[test]
fn no_main_no_runner_and_log_block_is_demand_driven() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    let tu = &emit.tu;
    assert!(!tu.contains("int main("), "a Zephyr driver TU has no main");
    // No log site reachable -> no module block, no log include (scaffold §1
    // item 2 demand discipline).
    assert!(!tu.contains("LOG_MODULE_REGISTER"), "{tu}");
    assert!(!tu.contains("<zephyr/logging/log.h>"), "{tu}");
}

#[test]
fn binding_yaml_carries_compatible_description_and_base_include() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    let yaml = &emit.binding_yaml;
    assert!(yaml.contains("compatible: \"regseq,pl011\""), "{yaml}");
    assert!(
        yaml.contains("description: minimal (Regseq-generated driver)"),
        "{yaml}"
    );
    assert!(yaml.contains("include: base.yaml"), "{yaml}");
}
