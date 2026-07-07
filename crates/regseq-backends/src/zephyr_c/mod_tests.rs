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

use super::{emit_zephyr_c, errno_symbol, EmitError};
use crate::capability;

// -- T19: pinctrl errno mapping (GRILL D12/C08) ------------------------------

/// The pinctrl optional-absent fold emits `Err(RegseqError::PinctrlAbsent)`;
/// the zephyr_c errno adapter must map it to `ENODEV` (else the scaffold exit
/// adapter fails loud on the unmapped variant when a 3-variant pinctrl
/// `RegseqError` reaches a probe-return). Mirrors generic_c/errno.rs (M3) and
/// linux_c (T19). Shares the `ENODEV` symbol with `DeviceNotFound`
/// (name-keyed, order-independent); an unmapped name still yields `None` so the
/// caller fails loud rather than defaulting.
#[test]
fn pinctrl_absent_maps_to_enodev() {
    assert_eq!(errno_symbol("PinctrlAbsent"), Some("ENODEV"));
    assert_eq!(errno_symbol("DeviceNotFound"), Some("ENODEV"));
    assert_eq!(errno_symbol("HardwareError"), Some("EIO"));
    assert_eq!(errno_symbol("Frobnicate"), None);
}

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

/// A resolved PCIe MMIO region: dead base/size literals (A-10(a)) + the
/// LIVE effective BAR number feeding `pcie_get_mbar` (D04).
fn pcie_mmio(m: &mut RegseqModel, name: &str, bar: Option<u8>) -> ResourceField {
    ResourceField {
        name: m.strings.intern(name),
        kind: ResourceKind::Mmio,
        binding: MmioBinding {
            id: None,
            bar_index: None,
            bar_type: regseq_model::BarType::Mem,
            effective_bar: bar,
            base: Some(0xf000_0000),
            size: Some(0x2_0000),
        },
    }
}

/// The minimal emitting PCIe model (backend-zephyr-c-e1000 R01): one-region
/// PCIe driver, devicetree-matched, no callbacks.
fn pcie_dt_model(bar: Option<u8>) -> RegseqModel {
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,e1000");
    let region = pcie_mmio(&mut m, "regs", bar);
    driver_into(
        &mut m,
        Some(BusType::Pci),
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![region],
            user_data_fields: vec![],
        },
    );
    m
}

// -- the PCIe scaffold (backend-zephyr-c-e1000 R01/R02, C01): positive
//    structure tests replacing the lifted UnsupportedInUnit gate ------------

#[test]
fn pcie_scaffold_emits_complete_tu_with_c01_preamble_order() {
    let emit = emit_zephyr_c(&pcie_dt_model(Some(0))).expect("PCIe model must emit");
    let tu = &emit.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("landmark `{needle}` missing from TU:\n{tu}"))
    };
    // Scaffold blocks (R01): include, data field, TU-static carrier,
    // DECLARE/INIT in the instantiation block.
    pos("#include <zephyr/drivers/pcie/pcie.h>");
    let p_pcie_field = pos("struct pcie_dev *pcie;");
    let p_carrier_decl = pos("static struct pcie_dev *__rseq_pcie;");
    let p_declare = pos("DEVICE_PCIE_INST_DECLARE(n);");
    let p_init_macro = pos("DEVICE_PCIE_INST_INIT(n, pcie),");
    assert!(p_pcie_field < p_carrier_decl, "data field before carrier");
    assert!(p_carrier_decl < p_declare && p_declare < p_init_macro);
    // The C01 preamble, in emission order (R02): BDF rung -> carrier
    // install -> hoisted probe (deviation audit) -> claim elision ->
    // MEM-only enable -> map into the TU-static base.
    let p_rung1 = pos("if (data->pcie->bdf == PCIE_BDF_NONE)");
    let p_carrier = pos("__rseq_pcie = data->pcie;");
    let p_deviation = pos("RSEQ-DEVIATION: get_resource_handle hoisted before enable_device");
    let p_probe = pos("if (!pcie_get_mbar(data->pcie->bdf, 0, &mbar))");
    let p_claim = pos("claim_regions: zero-call elision");
    let p_enable = pos("pcie_set_cmd(data->pcie->bdf, PCIE_CONF_CMDSTAT_MEM, true);");
    let p_map = pos("device_map(&minimal_mmio_base, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE);");
    assert!(p_rung1 < p_carrier, "BDF rung before the carrier install");
    assert!(
        p_carrier < p_deviation && p_deviation < p_probe,
        "carrier, then hoisted probe"
    );
    assert!(
        p_probe < p_claim && p_claim < p_enable,
        "probe -> claim elision -> enable"
    );
    assert!(p_enable < p_map, "enable before map (D02 order)");
    // Both rungs return unwrapped -ENODEV (D11/I09).
    assert_eq!(
        tu.matches("return -ENODEV; /* native channel, unwrapped (D11) */")
            .count(),
        2
    );
    // The deviation audit cites its license (never silent, I08).
    pos("[license: backend-zephyr-c-e1000 GRILL D02]");
    // The guard wording covers BOTH TU-statics (I07).
    pos("\"TU-static MMIO base and __rseq_pcie carrier require a single status-okay instance\"");
    // No DT-reg machinery on the PCIe arm: no DEVICE_MMIO map/capture, no
    // A-1 cross-check (the C05 binding requires no `reg`; base/size dead).
    assert!(!tu.contains("DEVICE_MMIO_MAP"), "{tu}");
    assert!(!tu.contains("DEVICE_MMIO_ROM_INIT"), "{tu}");
    assert!(!tu.contains("DEVICE_MMIO_ROM;"), "{tu}");
    assert!(!tu.contains("DEVICE_MMIO_RAM;"), "{tu}");
    assert!(!tu.contains("DT_REG_ADDR"), "{tu}");
    // A-15/D07: init placement unchanged.
    pos("POST_KERNEL, CONFIG_KERNEL_INIT_PRIORITY_DEVICE");
}

#[test]
fn pcie_effective_bar_is_the_bar_number_fed_to_get_mbar() {
    let emit = emit_zephyr_c(&pcie_dt_model(Some(2))).expect("PCIe model must emit");
    assert!(
        emit.tu.contains("pcie_get_mbar(data->pcie->bdf, 2, &mbar)"),
        "{}",
        emit.tu
    );
}

#[test]
fn pcie_region_without_effective_bar_is_a_named_driver_error() {
    let err = emit_zephyr_c(&pcie_dt_model(None)).unwrap_err();
    let EmitError::Driver(s) = err else {
        panic!("expected Driver, got {err:?}");
    };
    assert!(s.contains("no BAR number") && s.contains("regs"), "{s}");
}

#[test]
fn pcie_without_region_is_rejected_not_partially_emitted() {
    // The C01 preamble is region-shaped: a PCIe driver with no MMIO region
    // has no preamble contract — D.2 fail-loud, never a partial preamble
    // (C1 review round-1 finding 1).
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,e1000");
    driver_into(
        &mut m,
        Some(BusType::Pci),
        match_set,
        LifecycleOps::default(),
        Default::default(),
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(
        s.contains("no MMIO resource region") && s.contains("region-shaped"),
        "{s}"
    );
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

// -- the C06 class disposition (backend-zephyr-c-e1000 R14/D14): audited
//    class-neutral emission replacing the former A-16 reject -----------------

#[test]
fn non_generic_class_emits_class_neutral_with_the_c06_audit_comment() {
    let mut m = minimal_dt_model();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.device_class = ClassTag::Network;
    let emit = emit_zephyr_c(&m).expect("non-Generic class must emit (D14)");
    let tu = &emit.tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("landmark `{needle}` missing from TU:\n{tu}"))
    };
    // The audit comment names the declared class + the D14 license, and
    // sits immediately above the instantiation define (C06 item 1).
    let p_comment = pos("RSEQ-DEFERRED-CLASS: device_class \"network\" declared by the spec");
    let p_license = pos("[license: backend-zephyr-c-e1000 GRILL D14]");
    let p_define = pos("DEVICE_DT_INST_DEFINE(n, minimal_init, NULL,");
    assert!(p_comment < p_license && p_license < p_define);
    assert!(
        !tu[p_license..p_define].contains(";\\"),
        "no macro statement between the audit comment and the define:\n{}",
        &tu[p_comment..p_define]
    );
    // The scaffold itself stays the class-neutral NULL-api form.
    pos("NULL /* api: ClassTag::Generic only (A-16) */");
}

#[test]
fn generic_class_tu_carries_no_deferred_class_comment() {
    // C06 item 3: the ClassTag::Generic emission is byte-frozen — locally,
    // no audit comment; globally, the pl011 zephyr snapshots stay green
    // with zero re-bless.
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("minimal model must emit");
    assert!(!emit.tu.contains("RSEQ-DEFERRED-CLASS"), "{}", emit.tu);
    assert!(!emit.tu.contains("__rseq_pcie"), "{}", emit.tu);
    assert!(
        !emit.tu.contains("<zephyr/drivers/pcie/pcie.h>"),
        "{}",
        emit.tu
    );
}

// -- T03 match consumption (backend-zephyr-c-e1000 R05/C04): native DT
//    entry + Pci rows triple-render-audited; the D10 PCIe binding form ----

#[test]
fn pcie_mixed_match_triple_renders_each_pci_row() {
    let mut m = model();
    let compatible = m.strings.intern("regseq,e1000");
    let match_set = vec![
        MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100e,
            subsystem: None,
            class: None,
        },
        MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100f,
            subsystem: None,
            class: None,
        },
        MatchEntry::Devicetree { compatible },
    ];
    let region = pcie_mmio(&mut m, "regs", Some(0));
    driver_into(
        &mut m,
        Some(BusType::Pci),
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![region],
            user_data_fields: vec![],
        },
    );
    let emit = emit_zephyr_c(&m).expect("mixed Pci+DT set must emit (C04)");
    let tu = &emit.tu;
    // (i) native consumption: the authored DT entry IS the compat pin (I05:
    // nothing synthesized).
    assert!(tu.contains("#define DT_DRV_COMPAT regseq_e1000"), "{tu}");
    // (ii) one C block per Pci row, adjacent to the pin.
    let b1 = tu
        .find("/* RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100e")
        .unwrap_or_else(|| panic!("100e audit block:\n{tu}"));
    let b2 = tu
        .find("/* RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100f")
        .unwrap_or_else(|| panic!("100f audit block:\n{tu}"));
    let pin = tu.find("#define DT_DRV_COMPAT").expect("compat pin");
    assert!(b1 < b2 && b2 < pin, "both blocks at the pin, set order");
    // (iii) conformance rows + the summary-line section, from the SAME
    // struct values.
    assert_eq!(emit.conformance.ignored_match_entries.len(), 2);
    assert!(
        emit.conformance.summary_line().contains(
            "ignored=[pci vendor=0x8086 device=0x100e;pci vendor=0x8086 device=0x100f]"
        ),
        "{}",
        emit.conformance.summary_line()
    );
    // (iv) the binding-yaml third render + the D10 PCIe binding form:
    // pcie-device.yaml include (witnessed intel,e1000.yaml idiom),
    // vendor-id/device-id required, NO reg requirement.
    let yaml = &emit.binding_yaml;
    assert!(
        yaml.contains("# RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100e"),
        "{yaml}"
    );
    assert!(
        yaml.contains("# RSEQ-IGNORED-MATCH: pci vendor=0x8086 device=0x100f"),
        "{yaml}"
    );
    assert!(yaml.contains("include: [base.yaml, pcie-device.yaml]"), "{yaml}");
    assert!(yaml.contains("vendor-id:\n    required: true"), "{yaml}");
    assert!(yaml.contains("device-id:\n    required: true"), "{yaml}");
    assert!(!yaml.contains("reg:"), "no reg requirement on the PCIe form:\n{yaml}");
    // The yaml audit comments sit adjacent to (above) the consumed
    // `compatible:` surface — the yaml analog of the C-block/pin adjacency.
    let ya = yaml.find("# RSEQ-IGNORED-MATCH").unwrap();
    let yc = yaml.find("compatible:").unwrap();
    assert!(ya < yc, "audit comments above the compatible line:\n{yaml}");
}

// -- T04 capability/conformance wiring (backend-zephyr-c-e1000 R06/R14):
//    the wildcard row over bus "pci"; the C06 deferred-class note ----------

#[test]
fn pcie_absent_surprise_remove_resolves_the_wildcard_row() {
    // R06: an authored on_surprise_remove on the PCIe arm queries
    // bus_family = "pci"; the existing CAP-ZEPHYR-DT-SURPRISE-1 wildcard
    // resolves (registration form unchanged: StaticDevicetree) and the
    // absent-slot render is exactly the platform path's. No sibling row.
    let mut m = pcie_dt_model(Some(0));
    let (driver_ty, result_ty) = pcie_driver_and_result(&mut m);
    let on_init = push_ok_callback(&mut m, driver_ty, result_ty, "on_init");
    let on_sr = push_ok_callback(&mut m, driver_ty, result_ty, "on_surprise_remove");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    driver.ops.on_surprise_remove = Some(on_sr);
    let emit = emit_zephyr_c(&m).expect("the PCIe absent slot must emit (R06)");
    let tu = &emit.tu;
    let row = &capability::CAP_ZEPHYR_DT_SURPRISE_1;
    assert_eq!(tu.matches("RSEQ-ABSENT-SLOT:").count(), 1, "{tu}");
    assert!(
        tu.contains(&format!("* disposition: AbsentDispatch row: {}", row.id)),
        "{tu}"
    );
    // Non-emission with the body still checked; the conformance row cites
    // the row ID + full citation (I06).
    assert!(!tu.contains("minimal_on_surprise_remove"), "{tu}");
    assert_eq!(
        emit.conformance.absent_dispatch_slots,
        vec![capability::SlotEntry {
            slot: "on_surprise_remove".into(),
            row: Some(row.id.into()),
            citation: Some(row.citation.into()),
        }]
    );
}

#[test]
fn non_generic_class_carries_the_conformance_note() {
    let mut m = minimal_dt_model();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.device_class = ClassTag::Network;
    let emit = emit_zephyr_c(&m).expect("non-Generic class must emit (D14)");
    assert_eq!(
        emit.conformance.deferred_class,
        Some(capability::DeferredClassNote {
            class: "network".into(),
            license: capability::DEFERRED_CLASS_LICENSE,
        })
    );
    assert!(
        emit.conformance
            .summary_line()
            .ends_with("deferred_class=[network]"),
        "{}",
        emit.conformance.summary_line()
    );
}

#[test]
fn generic_class_report_carries_no_note() {
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("must emit");
    assert!(emit.conformance.deferred_class.is_none());
    assert!(
        !emit.conformance.summary_line().contains("deferred_class"),
        "{}",
        emit.conformance.summary_line()
    );
}

#[test]
fn platform_binding_yaml_is_byte_frozen() {
    // The non-PCIe yaml form carries neither the PCIe include nor any
    // audit section (I08: the pl011 goldens stay green with zero re-bless).
    let emit = emit_zephyr_c(&minimal_dt_model()).expect("must emit");
    let yaml = &emit.binding_yaml;
    assert!(!yaml.contains("pcie-device.yaml"), "{yaml}");
    assert!(!yaml.contains("RSEQ-IGNORED-MATCH"), "{yaml}");
    assert!(yaml.contains("include: base.yaml"), "{yaml}");
}

#[test]
fn other_class_tag_renders_its_carried_name() {
    let mut m = minimal_dt_model();
    let tag = m.strings.intern("quantum");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.device_class = ClassTag::Other(tag);
    let emit = emit_zephyr_c(&m).expect("Other class must emit (D14)");
    assert!(
        emit.tu
            .contains("RSEQ-DEFERRED-CLASS: device_class \"quantum\""),
        "{}",
        emit.tu
    );
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
fn mixed_match_set_is_audited_at_the_dt_compat_surface() {
    // C06/C03+A1 (spec-convergence-pl011): foreign entries alongside the
    // native Devicetree entry EMIT — each audited with an
    // RSEQ-IGNORED-MATCH block adjacent to the consumed match surface
    // (`#define DT_DRV_COMPAT`; zephyr has no match table), carried in the
    // report and the summary line, all from one MatchAudit value (D01).
    // The former unratified-rule rejection is lifted by that unit.
    let mut m = emitting_model(&["HardwareError", "DeviceBusy"]);
    {
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set.insert(
            0,
            MatchEntry::Amba {
                id: 0x0004_1011,
                mask: 0x000f_ffff,
            },
        );
    }
    let emit = emit_zephyr_c(&m).expect("mixed set must emit (C06)");
    let tu = &emit.tu;
    // (i) the C block, adjacent to (immediately preceding) the compat pin.
    let block = tu
        .find("/* RSEQ-IGNORED-MATCH: amba id=0x00041011 mask=0x000fffff")
        .unwrap_or_else(|| panic!("audit block present:\n{tu}"));
    let pin = tu.find("#define DT_DRV_COMPAT").expect("compat pin");
    assert!(block < pin, "audit block adjacent to the DT_DRV_COMPAT pin (A1)");
    assert!(
        tu[block..pin].len() < 400,
        "the block sits AT the pin, not floated to the header"
    );
    assert!(tu.contains("not consumed by the ratified static-devicetree registration form"));
    assert!(tu.contains("consumed natively by linux_c."));
    // The consumed surface carries no amba rendering (the zephyr analog of
    // the A2 filtered-table negative: the foreign entry feeds only the audit).
    assert!(
        !tu[pin..].contains("0x00041011"),
        "foreign entry absent from the consumed surface"
    );
    // (ii) the report — and the block renders from the SAME value (I03).
    assert_eq!(emit.conformance.ignored_match_entries.len(), 1);
    let audit = &emit.conformance.ignored_match_entries[0];
    assert_eq!(audit.entry, "amba id=0x00041011 mask=0x000fffff");
    assert_eq!(audit.native_on, &["linux_c"]);
    assert!(tu.contains(&audit.c_block()), "same-struct render");
    // (iii) the summary line.
    assert!(
        emit.conformance
            .summary_line()
            .ends_with("ignored=[amba id=0x00041011 mask=0x000fffff]"),
        "{}",
        emit.conformance.summary_line()
    );
}

#[test]
fn deferred_family_entry_alongside_native_is_rejected_loud() {
    // C06 final-review hardening: the D03(a) license covers RECOGNIZED
    // foreign entries only — a deferred-family entry (no ratified
    // registration form on ANY backend) keeps the D.2 fail-loud default
    // even when the native DT entry exists, never audited-and-ignored.
    let mut m = emitting_model(&["HardwareError", "DeviceBusy"]);
    {
        let hid = m.strings.intern("ACPI1234");
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set.push(MatchEntry::Acpi { hid });
    }
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(
        s.contains("no ratified registration form") && s.contains("fail-loud"),
        "the deferred-family rejection names the rule boundary: {s}"
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

// -- the deinit lowering (spec-convergence-pl011 C04/D05): authored
//    on_deinit lowers a deinit fn + the conditional macro swap; unauthored
//    leaves the scaffold byte-identical (I05) ------------------------------

/// The emitting model plus a legal `Ok(())` `on_deinit` body (C04).
fn deinit_model() -> RegseqModel {
    let mut m = emitting_model(&["HardwareError", "DeviceBusy"]);
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let result_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Enum { item, .. } if m.items[*item].is_result))
        .map(|(id, _)| id)
        .unwrap();
    let on_deinit = push_ok_callback(&mut m, driver_ty, result_ty, "on_deinit");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_deinit = Some(on_deinit);
    m
}

#[test]
fn authored_on_deinit_lowers_the_full_c04_shape() {
    let emit = emit_zephyr_c(&deinit_model()).expect("authored on_deinit must emit (C04)");
    let tu = &emit.tu;
    // The deinit fn, C04 signature, after init and before the instantiation
    // block.
    let init = tu.find("static int minimal_init(").expect("init fn");
    let deinit = tu
        .find("static int minimal_deinit(const struct device *dev)")
        .unwrap_or_else(|| panic!("C04 deinit signature:\n{tu}"));
    let inst_block = tu
        .find("DEVICE_DT_INST_DEINIT_DEFINE(n, minimal_init, minimal_deinit, NULL,")
        .unwrap_or_else(|| panic!("the conditional macro swap:\n{tu}"));
    assert!(init < deinit && deinit < inst_block);
    assert!(
        !tu.contains("DEVICE_DT_INST_DEFINE(n,"),
        "swapped, not duplicated"
    );
    // The fused adapter inside the deinit fn: same nine-row negated-errno
    // shape as init, arms in deterministic variant order.
    let deinit_body = &tu[deinit..inst_block];
    assert!(deinit_body.contains("if (__tag != 0x2)"), "{deinit_body}");
    let eio = deinit_body.find("return -EIO;").expect("HardwareError arm");
    let ebusy = deinit_body.find("return -EBUSY;").expect("DeviceBusy arm");
    assert!(eio < ebusy, "deterministic variant order (C04)");
    assert!(deinit_body.contains("default:"), "corrupt-tag defense");
    assert!(deinit_body.contains("\treturn 0;"), "Ok(()) => return 0");
    // The explicit no-op postamble callout (the map_region mirror, C04).
    assert!(
        deinit_body.contains("deliberate NO-OP"),
        "the unmap mirror is an explicit callout, not silence: {deinit_body}"
    );
    // The TU-header Kconfig pin row naming the silent-drop hazard.
    assert!(tu.contains("CONFIG_DEVICE_DEINIT_SUPPORT=y"), "{tu}");
    assert!(tu.contains("device.h:1175-1180"), "{tu}");
    // Conformance: on_deinit delivered, citing the flipped row (D05).
    let entry = emit
        .conformance
        .delivered_slots
        .iter()
        .find(|e| e.slot == "on_deinit")
        .expect("on_deinit in delivered_slots");
    assert_eq!(entry.row.as_deref(), Some(capability::CAP_ZEPHYR_DT_DEINIT_1.id));
    assert!(
        emit.conformance
            .summary_line()
            .contains("on_deinit:CAP-ZEPHYR-DT-DEINIT-1"),
        "{}",
        emit.conformance.summary_line()
    );
}

#[test]
fn unauthored_on_deinit_leaves_the_scaffold_byte_identical() {
    // I05: the conditional swap — without on_deinit no deinit artifact
    // exists at all (the absentslot/init-fail goldens stay byte-frozen,
    // proven by their snapshot e2es needing no re-bless).
    let tu = emit_zephyr_c(&emitting_model(&["HardwareError", "DeviceBusy"]))
        .expect("must emit")
        .tu;
    assert!(tu.contains("DEVICE_DT_INST_DEFINE(n, minimal_init, NULL,"));
    assert!(!tu.contains("DEINIT"), "no deinit trace in the scaffold");
    assert!(!tu.contains("minimal_deinit"));
    assert!(!tu.contains("CONFIG_DEVICE_DEINIT_SUPPORT"));
}

// -- the ratified OS-absent slot (os-capability-disposition unit): authored
//    on_surprise_remove under a ratified AbsentDispatch row emits, audited;
//    the body is still fully checked (D06/D10, C04/C05) ----------------------

/// The emitting model plus a legal `Ok(())` `on_surprise_remove` body.
fn absent_slot_model() -> RegseqModel {
    let mut m = emitting_model(&["HardwareError", "DeviceBusy"]);
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let result_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Enum { item, .. } if m.items[*item].is_result))
        .map(|(id, _)| id)
        .unwrap();
    let on_sr = push_ok_callback(&mut m, driver_ty, result_ty, "on_surprise_remove");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_surprise_remove = Some(on_sr);
    m
}

#[test]
fn absent_surprise_remove_emits_with_exactly_one_audit_block() {
    let emit = emit_zephyr_c(&absent_slot_model()).expect("the absent slot must emit");
    let tu = &emit.tu;
    // Exactly one C04 block, grammar verbatim, citing the ratified row.
    assert_eq!(tu.matches("RSEQ-ABSENT-SLOT:").count(), 1, "{tu}");
    let row = &capability::CAP_ZEPHYR_DT_SURPRISE_1;
    assert!(
        tu.contains("/* RSEQ-ABSENT-SLOT: on_surprise_remove"),
        "{tu}"
    );
    assert!(
        tu.contains(&format!("* disposition: AbsentDispatch row: {}", row.id)),
        "{tu}"
    );
    // The [ratified: ...] payload is non-empty by construction (the row
    // citation, verbatim from the const).
    assert!(tu.contains("[ratified: Zephyr v4.4.0"), "{tu}");
    assert!(
        tu.contains("body checked, not delivered: dispatch cannot occur on this target."),
        "{tu}"
    );
    // Non-emission: no callback symbol, no call site.
    assert!(!tu.contains("minimal_on_surprise_remove"), "{tu}");
    // The report row cites the SAME const the block renders from (I03):
    // block row ID == report row ID.
    assert_eq!(
        emit.conformance.absent_dispatch_slots,
        vec![crate::capability::SlotEntry {
            slot: "on_surprise_remove".into(),
            row: Some(row.id.into()),
            citation: Some(row.citation.into()),
        }]
    );
    assert!(
        tu.contains(&format!(
            "row: {}",
            emit.conformance.absent_dispatch_slots[0]
                .row
                .as_deref()
                .unwrap()
        )),
        "{tu}"
    );
    // The R05 summary line, plan-fixed wording, from the same struct.
    assert_eq!(
        emit.conformance.summary_line(),
        "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[on_init] \
         absent_dispatch=[on_surprise_remove:CAP-ZEPHYR-DT-SURPRISE-1] structurally_absent=[] rejected=[] ignored=[]"
    );
    assert!(emit.conformance.rejected_slots.is_empty());
}

#[test]
fn absent_lowering_has_no_observable_effect_beyond_the_audit_block() {
    // D06: the discarded lowering must not perturb the TU — the absent-slot
    // emit equals the no-surprise emit plus EXACTLY the audit block.
    let with_absent = emit_zephyr_c(&absent_slot_model()).expect("must emit");
    let without =
        emit_zephyr_c(&emitting_model(&["HardwareError", "DeviceBusy"])).expect("must emit");
    let block = format!("{}\n", capability::CAP_ZEPHYR_DT_SURPRISE_1.audit_block());
    assert_eq!(with_absent.tu.replace(&block, ""), without.tu);
    assert_eq!(with_absent.binding_yaml, without.binding_yaml);
}

#[test]
fn absent_surprise_remove_body_is_still_checked_scan() {
    // The D12 frame scan runs over the absent closure: a poll_until body
    // under the absent slot is the D.2 EmitError, never silent emission
    // (closes the old mod_tests:518 hole).
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let poll = push_intrinsic(&mut m, "poll_until", IntrinsicKind::PollUntil, vec![], ut);
    let on_sr = push_callback_calling(&mut m, poll, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_surprise_remove = Some(on_sr);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("B-1"), "{s}");
}

#[test]
fn absent_surprise_remove_runs_full_lowering_not_just_the_scan() {
    // D06's mechanism guarantee: the absent body runs the COMPLETE pipeline
    // (fn_head/emit_fn), so a construct only the lowering rejects — a
    // foreign bodyless callee, invisible to the frame scan — still fails.
    let mut m = minimal_dt_model();
    let ut = unit_ty(&mut m);
    let fn_item = item(&mut m, ItemKind::Function, "foreign_helper");
    let sig = m.sigs.intern(FnSig {
        inputs: vec![],
        output: ut,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let emission = m.strings.intern("foreign_helper");
    let foreign = m.instances.push(Instance {
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
    });
    let on_sr = push_callback_calling(&mut m, foreign, ut);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_surprise_remove = Some(on_sr);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Unsupported(s) = err else {
        panic!("expected Unsupported, got {err:?}");
    };
    assert!(s.contains("foreign (bodyless) function"), "{s}");
}

#[test]
fn absent_lowering_runs_the_type_definition_pass() {
    // The type-defs bank is a legality check of its own: an aggregate whose
    // DEFINITION is outside the subset (here a u128 field) is named cleanly
    // during fn_head and rejected only by emit_type_defs. The absent path
    // must run that pass over the throwaway names too (D06; Codex M1
    // review finding 1).
    let mut m = minimal_dt_model();
    let u128t = m.types.intern(Ty::Uint(UintKind::U128));
    let field = m.strings.intern("wide");
    let carrier = push_struct(
        &mut m,
        "WideCarrier",
        struct_layout(
            16,
            16,
            vec![FieldLayout {
                name: field,
                ty: u128t,
                offset: 0,
            }],
        ),
    );
    let sig = m.sigs.intern(FnSig {
        inputs: vec![],
        output: carrier,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let mut body = Body::default();
    body.local_decls.push(LocalDecl {
        ty: carrier,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.arg_count = 0;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: span(),
        },
    });
    let fn_item = item(&mut m, ItemKind::Function, "wide_callback");
    let emission = m.strings.intern("wide_callback");
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
    let on_sr = m.instance_ref_of(id);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_surprise_remove = Some(on_sr);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Unsupported(s) = err else {
        panic!("expected Unsupported, got {err:?}");
    };
    // The rejection comes from the type-DEFINITION pass, context-named.
    assert!(s.contains("[while defining `struct WideCarrier`]"), "{s}");
}

#[test]
fn no_audit_block_when_no_absent_slot() {
    let emit = emit_zephyr_c(&emitting_model(&["HardwareError", "DeviceBusy"])).expect("must emit");
    assert!(!emit.tu.contains("RSEQ-ABSENT-SLOT"), "{}", emit.tu);
    assert!(emit.conformance.absent_dispatch_slots.is_empty());
    // Delivered slots in LifecycleOps declaration order, row None (no
    // ratified row exists for the zephyr bring-up slots).
    assert_eq!(emit.conformance.delivered_slots.len(), 1);
    assert_eq!(emit.conformance.delivered_slots[0].slot, "on_init");
    assert_eq!(emit.conformance.delivered_slots[0].row, None);
    assert_eq!(
        emit.conformance.summary_line(),
        "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[on_init] \
         absent_dispatch=[] structurally_absent=[] rejected=[] ignored=[]"
    );
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
    // D14: table-cited (rendered from the row const, never a literal).
    assert!(s.contains(capability::CAP_ZEPHYR_DT_SHUTDOWN_1.id), "{s}");
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
    // The driver struct carries the field in its LAYOUT (the type-defs bank
    // renders from the layout) and in the handle (the B-8 gate's input).
    let driver_ty = push_struct(
        &mut m,
        "Minimal",
        struct_layout(
            1,
            1,
            vec![FieldLayout {
                name: field_name,
                ty: opt,
                offset: 0,
            }],
        ),
    );
    let driver_item = match &m.types[driver_ty] {
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
                bus_type: None,
            },
            handle: regseq_model::DriverHandle {
                resource_fields: vec![],
                user_data_fields: vec![UserDataField {
                    name: field_name,
                    ty: opt,
                    kind: UserDataKind::OptionCopy,
                }],
            },
            ops: LifecycleOps::default(),
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

// -- T02 config-space lowering (backend-zephyr-c-e1000 C02/C03): reads with
//    the D06 word-0 cache, the D05 write RMW, retained D.2 rejections ------

/// Push an `Ok(())`-returning lifecycle callback over `&mut <driver_ty>`
/// whose first block CALLS `callee` with the given args into a scratch
/// local — the config-intrinsic positive-emission seam. `extra_locals` are
/// appended after the scratch destination (locals _3, _4, ... — for
/// runtime-operand shapes).
#[allow(clippy::too_many_arguments)]
fn push_ok_callback_calling(
    m: &mut RegseqModel,
    driver_ty: TyId,
    result_ty: TyId,
    leaf: &str,
    callee: regseq_model::InstanceId,
    args: Vec<Operand>,
    dest_ty: TyId,
    extra_locals: Vec<TyId>,
) -> regseq_model::InstanceRef {
    let self_ty = m
        .types
        .intern(Ty::Ref(driver_ty, regseq_model::Provenance::UniqueBorrow));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let unit_const = m.consts.intern(Const::ZST);
    let mut body = Body::default();
    for (ty, debug) in [(result_ty, None), (self_ty, Some("self")), (dest_ty, None)] {
        body.local_decls.push(LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: span(),
            debug_name: debug.map(|d| m.strings.intern(d)),
        });
    }
    for ty in extra_locals {
        body.local_decls.push(LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: span(),
            debug_name: None,
        });
    }
    body.arg_count = 1;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(callee),
                args: args.into_boxed_slice(),
                destination: Place::from(regseq_model::LocalId::from_u32(2)),
                target: Some(regseq_model::BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(regseq_model::LocalId::RETURN_PLACE),
                Rvalue::Aggregate(
                    regseq_model::AggregateKind::Enum(result_ty, VariantIdx::from_u32(0)),
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
    let fn_item = item(m, ItemKind::Function, leaf);
    let emission = m.strings.intern(leaf);
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

/// The PCIe model's driver struct ty + the certified Result carrier.
fn pcie_driver_and_result(m: &mut RegseqModel) -> (TyId, TyId) {
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(m, &["HardwareError"]);
    (driver_ty, result_ty)
}

/// A PCIe model whose on_init performs one config READ of `ret_kind` at
/// constant `off`; returns the emitted TU.
fn pcie_config_read_tu(ret_kind: UintKind, off: u128) -> String {
    let mut m = pcie_dt_model(Some(0));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let ret = m.types.intern(Ty::Uint(ret_kind));
    let rc = push_intrinsic(
        &mut m,
        "read_config",
        IntrinsicKind::Config(ConfigOp::Read),
        vec![u32t],
        ret,
    );
    let off_c = m.consts.intern(Const::Uint(UintKind::U32, off));
    let (driver_ty, result_ty) = pcie_driver_and_result(&mut m);
    let on_init = push_ok_callback_calling(
        &mut m,
        driver_ty,
        result_ty,
        "on_init",
        rc,
        vec![Operand::Constant(off_c)],
        ret,
        vec![],
    );
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    emit_zephyr_c(&m).expect("licensed config read must emit").tu
}

/// A PCIe model whose on_init performs one config WRITE of `val_kind` at
/// constant `off`; returns the emit result.
fn pcie_config_write_emit(val_kind: UintKind, off: u128, val: u128) -> Result<String, EmitError> {
    let mut m = pcie_dt_model(Some(0));
    let ut = unit_ty(&mut m);
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let vt = m.types.intern(Ty::Uint(val_kind));
    let wc = push_intrinsic(
        &mut m,
        "write_config",
        IntrinsicKind::Config(ConfigOp::Write),
        vec![u32t, vt],
        ut,
    );
    let off_c = m.consts.intern(Const::Uint(UintKind::U32, off));
    let val_c = m.consts.intern(Const::Uint(val_kind, val));
    let (driver_ty, result_ty) = pcie_driver_and_result(&mut m);
    let on_init = push_ok_callback_calling(
        &mut m,
        driver_ty,
        result_ty,
        "on_init",
        wc,
        vec![Operand::Constant(off_c), Operand::Constant(val_c)],
        ut,
        vec![],
    );
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    emit_zephyr_c(&m).map(|e| e.tu)
}

#[test]
fn config_word0_reads_are_served_from_the_id_cache() {
    // vendor@0x00 AND device@0x02 both live in containing word 0 (D06).
    let tu = pcie_config_read_tu(UintKind::U16, 0x00);
    assert!(tu.contains("uint32_t __cw = __rseq_pcie->id;"), "{tu}");
    assert!(tu.contains("(uint16_t)((__cw >> 0u) & 0xffffu)"), "{tu}");
    assert!(!tu.contains("pcie_conf_read"), "zero bus traffic (D06):\n{tu}");

    let tu = pcie_config_read_tu(UintKind::U16, 0x02);
    assert!(tu.contains("uint32_t __cw = __rseq_pcie->id;"), "{tu}");
    assert!(tu.contains("(uint16_t)((__cw >> 16u) & 0xffffu)"), "{tu}");
    assert!(!tu.contains("pcie_conf_read"), "zero bus traffic (D06):\n{tu}");
}

#[test]
fn config_reads_beyond_word0_issue_one_live_word_read() {
    // revision@0x08 -> word 2, byte lane 0.
    let tu = pcie_config_read_tu(UintKind::U8, 0x08);
    assert!(
        tu.contains("uint32_t __cw = pcie_conf_read(__rseq_pcie->bdf, 2u);"),
        "{tu}"
    );
    assert!(tu.contains("(uint8_t)((__cw >> 0u) & 0xffu)"), "{tu}");
    // BAR0@0x10 -> word 4, full width.
    let tu = pcie_config_read_tu(UintKind::U32, 0x10);
    assert!(
        tu.contains("uint32_t __cw = pcie_conf_read(__rseq_pcie->bdf, 4u);"),
        "{tu}"
    );
    assert!(tu.contains("(uint32_t)((__cw >> 0u) & 0xffffffffu)"), "{tu}");
}

#[test]
fn config_write_width4_is_a_single_native_word_write() {
    let tu = pcie_config_write_emit(UintKind::U32, 0x04, 0x1234).expect("width-4 write emits");
    assert!(
        tu.contains("pcie_conf_write(__rseq_pcie->bdf, 1u, (uint32_t)("),
        "{tu}"
    );
    assert!(!tu.contains("RSEQ-RMW"), "no RMW on the native width:\n{tu}");
}

#[test]
fn config_write_subword_lowers_to_the_audited_c02_rmw() {
    // The fixture's CLS shape: u8 write @0x0C (D05, the B-3 partial lift).
    let tu = pcie_config_write_emit(UintKind::U8, 0x0C, 0x10).expect("u8 write emits via RMW");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("landmark `{needle}` missing from TU:\n{tu}"))
    };
    let p_audit = pos("RSEQ-RMW: width-1 config write lowered to the containing-word RMW");
    pos("[license: backend-zephyr-c-e1000 GRILL D05]");
    let p_read = pos("uint32_t __cur = pcie_conf_read(__rseq_pcie->bdf, __w);");
    let p_mask = pos("uint32_t __msk = 0xFFu << __sh;");
    let p_write =
        pos("pcie_conf_write(__rseq_pcie->bdf, __w, (__cur & ~__msk) | (((uint32_t)(");
    assert!(p_audit < p_read && p_read < p_mask && p_mask < p_write, "RMW order");
    // Halfword lane mask for a u16 write.
    let tu = pcie_config_write_emit(UintKind::U16, 0x0E, 0xBEEF).expect("u16 write emits via RMW");
    assert!(tu.contains("uint32_t __msk = 0xFFFFu << __sh;"), "{tu}");
}

#[test]
fn config_word0_subword_write_rmw_reads_live_never_the_cache() {
    // A sub-word write INTO word 0: the RMW's internal read stays a live
    // pcie_conf_read (C03's explicit carve-out), never the D06 id cache.
    let tu = pcie_config_write_emit(UintKind::U8, 0x01, 0xAA).expect("u8 write emits via RMW");
    assert!(
        tu.contains("uint32_t __cur = pcie_conf_read(__rseq_pcie->bdf, __w);"),
        "{tu}"
    );
    assert!(!tu.contains("__rseq_pcie->id"), "never the cache:\n{tu}");
}

#[test]
fn config_misaligned_subword_access_is_rejected() {
    // Misaligned halfword write (odd offset).
    let err = pcie_config_write_emit(UintKind::U16, 0x0D, 0x1).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("misaligned") && s.contains("B-3"), "{s}");
    // Misaligned dword write.
    let err = pcie_config_write_emit(UintKind::U32, 0x02, 0x1).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("misaligned"), "{s}");
}

#[test]
fn config_width8_write_is_rejected_citing_b3() {
    let err = pcie_config_write_emit(UintKind::U64, 0x08, 0x1).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("two-word") && s.contains("B-3"), "{s}");
}

#[test]
fn config_offset_beyond_the_u32_domain_is_rejected_not_truncated() {
    // A u64 constant above u32::MAX in the offset operand: out of the
    // intrinsics' u32 offset domain — rejected, never silently truncated
    // into an unrelated in-range access (C1 review round-1 finding 2).
    let err = pcie_config_write_emit(UintKind::U32, 0x1_0000_0004, 0x1).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("u32 offset domain"), "{s}");
}

#[test]
fn config_nonconstant_offset_is_rejected() {
    // The offset is a RUNTIME place, not a constant: the word-0 cache and
    // alignment legality cannot be decided at emit time -> D.2 reject.
    let mut m = pcie_dt_model(Some(0));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let u16t = m.types.intern(Ty::Uint(UintKind::U16));
    let rc = push_intrinsic(
        &mut m,
        "read_config",
        IntrinsicKind::Config(ConfigOp::Read),
        vec![u32t],
        u16t,
    );
    let (driver_ty, result_ty) = pcie_driver_and_result(&mut m);
    let off_local = regseq_model::LocalId::from_u32(3);
    let on_init = push_ok_callback_calling(
        &mut m,
        driver_ty,
        result_ty,
        "on_init",
        rc,
        vec![Operand::Copy(Place::from(off_local))],
        u16t,
        vec![u32t],
    );
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("non-constant offset"), "{s}");
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
    // A width-2 READ is one of the SPECIFIED PCI shapes (§B.4) — under a
    // non-PCI bus it has no carrier (B-4; the PCIe scaffold carries the
    // only ratified contract).
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
    assert!(s.contains("BRM-001") && s.contains("B-4"), "{s}");
}

// -- T07: lifecycle/BRM lowering — MMIO scaffold, errno adapter,
//    on_init_failure ---------------------------------------------------------

/// Intern the marked RegseqError (fieldless variants, direct u8 tag) and
/// the niche-encoded `Result<(), RegseqError>` carrier; returns
/// `(result_ty, regseq_error_ty)`. `err_variant_names` lets the
/// ErrnoMapping negative test inject an unmapped variant and the arm-order
/// test permute the declaration order.
fn result_carrier(m: &mut RegseqModel, err_variant_names: &[&str]) -> (TyId, TyId) {
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));

    let err_item = item(m, ItemKind::Enum, "RegseqError");
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
    let end = err_variant_names.len() as u128 - 1;
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
                        valid_range: WrappingRange { start: 0, end },
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

/// Push a callback whose body is `_0 = Ok(()); return;` over
/// `&mut <driver_ty>`; returns its `InstanceRef` for `LifecycleOps`.
fn push_ok_callback(
    m: &mut RegseqModel,
    driver_ty: TyId,
    result_ty: TyId,
    leaf: &str,
) -> regseq_model::InstanceRef {
    let self_ty = m
        .types
        .intern(Ty::Ref(driver_ty, regseq_model::Provenance::UniqueBorrow));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let unit_const = m.consts.intern(Const::ZST);
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
                    regseq_model::AggregateKind::Enum(result_ty, VariantIdx::from_u32(0)),
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
    let fn_item = item(m, ItemKind::Function, leaf);
    let emission = m.strings.intern(leaf);
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

/// The minimal full-TU model: one-region DT driver + `Ok(())` on_init over
/// the given RegseqError variant set.
fn emitting_model(err_variants: &[&str]) -> RegseqModel {
    let mut m = minimal_dt_model();
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                let rendered = m.strings.resolve(m.items[*item].def_path.rendered);
                rendered == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, err_variants);
    let on_init = push_ok_callback(&mut m, driver_ty, result_ty, "on_init");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    m
}

#[test]
fn full_tu_lowers_authored_on_init_through_the_adapter() {
    let emit = emit_zephyr_c(&emitting_model(&["HardwareError", "DeviceBusy"]))
        .expect("the composed shape must emit");
    let tu = &emit.tu;
    // The init preamble: claim elision documented, map, TU-static base.
    assert!(
        tu.contains("DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);"),
        "{tu}"
    );
    assert!(
        tu.contains("minimal_mmio_base = DEVICE_MMIO_GET(dev); /* TU-static base (A-2) */"),
        "{tu}"
    );
    assert!(tu.contains("static mm_reg_t minimal_mmio_base;"), "{tu}");
    assert!(
        !tu.contains("claim_regions("),
        "claim_regions is the ratified ZERO-CALL elision: {tu}"
    );
    // The authored callback, lowered and called through the adapter.
    assert!(
        tu.contains("static struct Result minimal_on_init(struct Minimal * self)"),
        "{tu}"
    );
    assert!(
        tu.contains("struct Result __r = minimal_on_init(&data->inner);"),
        "{tu}"
    );
    // Ok -> return 0; Err arms negated through the C02 table.
    assert!(tu.contains("if (__tag != 0x2)"), "{tu}");
    assert!(
        tu.contains("return -EIO;") && tu.contains("return -EBUSY;"),
        "{tu}"
    );
    assert!(tu.contains("\treturn 0;\n}"), "{tu}");
    // <errno.h> rides the adapter.
    assert!(tu.contains("#include <errno.h>"), "{tu}");
}

#[test]
fn errno_arms_follow_variant_declaration_order() {
    // Deterministic arm order = the enum's variant order, whatever it is.
    let emit = emit_zephyr_c(&emitting_model(&["DeviceBusy", "Timeout", "HardwareError"]))
        .expect("must emit");
    let tu = &emit.tu;
    let p_busy = tu.find("return -EBUSY;").expect("EBUSY arm");
    let p_timeout = tu.find("return -ETIMEDOUT;").expect("ETIMEDOUT arm");
    let p_eio = tu.find("return -EIO;").expect("EIO arm");
    assert!(
        p_busy < p_timeout && p_timeout < p_eio,
        "arms must follow variant order: {tu}"
    );
    // Tags are the declaration-order discriminants.
    assert!(tu.contains("case 0x0:"), "{tu}");
    assert!(tu.contains("case 0x1:"), "{tu}");
    assert!(tu.contains("case 0x2:"), "{tu}");
}

#[test]
fn on_init_failure_has_one_call_site_before_the_errno_switch() {
    let mut m = emitting_model(&["HardwareError", "DeviceBusy"]);
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let result_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Enum { item, .. } if m.items[*item].is_result))
        .map(|(id, _)| id)
        .unwrap();
    let on_init_failure = push_ok_callback(&mut m, driver_ty, result_ty, "on_init_failure");
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init_failure = Some(on_init_failure);
    let emit = emit_zephyr_c(&m).expect("must emit");
    let tu = &emit.tu;
    // Exactly ONE call site (D12: non-vacuous, the synthetic seam).
    let call = "(void)minimal_on_init_failure(&data->inner);";
    assert_eq!(tu.matches(call).count(), 1, "{tu}");
    // Positioned on the error path, BEFORE the errno switch (the cleanup
    // slot is sequenced ahead of the error return).
    let p_call = tu.find(call).unwrap();
    let p_switch = tu.find("switch (__err)").expect("errno switch");
    let p_tag = tu.find("if (__tag != 0x2)").expect("err branch");
    assert!(p_tag < p_call && p_call < p_switch, "{tu}");
}

#[test]
fn unmapped_regseq_error_variant_fails_loud() {
    let err = emit_zephyr_c(&emitting_model(&["HardwareError", "Bogus"])).unwrap_err();
    assert_eq!(err, EmitError::ErrnoMapping("Bogus".into()));
}

#[test]
fn unresolved_binding_is_a_hard_preamble_gate() {
    // Even with NO MMIO op in any body, an unresolved region is the A-10
    // hard emit-time error (platform.toml is REQUIRED).
    let mut m = model();
    let match_set = dt_match_for(&mut m, "regseq,pl011");
    let unresolved = ResourceField {
        name: m.strings.intern("regs"),
        kind: ResourceKind::Mmio,
        binding: MmioBinding::default(),
    };
    driver_into(
        &mut m,
        None,
        match_set,
        LifecycleOps::default(),
        regseq_model::DriverHandle {
            resource_fields: vec![unresolved],
            user_data_fields: vec![],
        },
    );
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::UnresolvedBinding(s) = err else {
        panic!("expected UnresolvedBinding, got {err:?}");
    };
    assert!(
        s.contains("[mmio.regs] base/size") && s.contains("A-10"),
        "{s}"
    );
}

#[test]
fn a1_cross_check_build_assert_carries_the_platform_literals() {
    let emit = emit_zephyr_c(&emitting_model(&["HardwareError", "DeviceBusy"])).expect("must emit");
    let tu = &emit.tu;
    assert!(
        tu.contains("BUILD_ASSERT(DT_REG_ADDR(DT_DRV_INST(0)) == 0x9040000 &&"),
        "{tu}"
    );
    assert!(
        tu.contains("DT_REG_SIZE(DT_DRV_INST(0)) == 0x1000,"),
        "{tu}"
    );
    // D11: a NAMED message.
    assert!(
        tu.contains("\"DT reg disagrees with the resolved [mmio] platform literals (D11)\""),
        "{tu}"
    );
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

// -- T06: register-access lowering + MMIO width family ----------------------
//
// Synthetic-model unit tests at the body seam (LowerCx + emit_fn driven
// directly; the TU fold consumes the bank in T07): accessor selection per
// width 1/2/4/8, the sys_io VALUE-FIRST write arg order, mask-confinement +
// RMW shape, descriptor const-folding, and the A-10 unresolved-binding hard
// error at the op site.

use regseq_model::{Const, Operand, PlaceElem, Rvalue, Statement, StatementKind};

use super::body::{emit_fn, DriverFacts, LowerCx};
use super::types::TypeNames;

/// A register-block driver model: `Minimal { regs: Mmio<Block> }` where
/// `Block` has one register wrapper per width at fixed offsets:
/// w8@0x00, w16@0x04, w32@0x30, w64@0x38. Returns
/// `(model, driver_ty, self_ty, [w1, w2, w4, w8] wrapper TyIds)`.
fn mmio_block_model() -> (RegseqModel, TyId, TyId, [TyId; 4]) {
    let mut m = model();

    // Width-carrier wrapper structs (ReadWrite<W, R> mirrors: layout size =
    // the access width).
    let w1 = {
        let u8t = m.types.intern(Ty::Uint(UintKind::U8));
        let f = m.strings.intern("_v");
        push_struct(
            &mut m,
            "W8",
            struct_layout(
                1,
                1,
                vec![FieldLayout {
                    name: f,
                    ty: u8t,
                    offset: 0,
                }],
            ),
        )
    };
    let w2 = {
        let u16t = m.types.intern(Ty::Uint(UintKind::U16));
        let f = m.strings.intern("_v");
        push_struct(
            &mut m,
            "W16",
            struct_layout(
                2,
                2,
                vec![FieldLayout {
                    name: f,
                    ty: u16t,
                    offset: 0,
                }],
            ),
        )
    };
    let w4 = {
        let u32t = m.types.intern(Ty::Uint(UintKind::U32));
        let f = m.strings.intern("_v");
        push_struct(
            &mut m,
            "W32",
            struct_layout(
                4,
                4,
                vec![FieldLayout {
                    name: f,
                    ty: u32t,
                    offset: 0,
                }],
            ),
        )
    };
    let w8 = {
        let u64t = m.types.intern(Ty::Uint(UintKind::U64));
        let f = m.strings.intern("_v");
        push_struct(
            &mut m,
            "W64",
            struct_layout(
                8,
                8,
                vec![FieldLayout {
                    name: f,
                    ty: u64t,
                    offset: 0,
                }],
            ),
        )
    };

    // The register block: named registers at hardware offsets.
    let block = {
        let r8 = m.strings.intern("r8");
        let r16 = m.strings.intern("r16");
        let cr = m.strings.intern("cr");
        let r64 = m.strings.intern("r64");
        push_struct(
            &mut m,
            "Block",
            struct_layout(
                0x40,
                8,
                vec![
                    FieldLayout {
                        name: r8,
                        ty: w1,
                        offset: 0x00,
                    },
                    FieldLayout {
                        name: r16,
                        ty: w2,
                        offset: 0x04,
                    },
                    FieldLayout {
                        name: cr,
                        ty: w4,
                        offset: 0x30,
                    },
                    FieldLayout {
                        name: r64,
                        ty: w8,
                        offset: 0x38,
                    },
                ],
            ),
        )
    };
    // Mmio<Block>: one `inner` field.
    let mmio = {
        let inner = m.strings.intern("inner");
        push_struct(
            &mut m,
            "Mmio",
            struct_layout(
                0x40,
                8,
                vec![FieldLayout {
                    name: inner,
                    ty: block,
                    offset: 0,
                }],
            ),
        )
    };
    // The driver struct.
    let regs = m.strings.intern("regs");
    let driver_ty = push_struct(
        &mut m,
        "MmioDriver",
        struct_layout(
            0x40,
            8,
            vec![FieldLayout {
                name: regs,
                ty: mmio,
                offset: 0,
            }],
        ),
    );
    let self_ty = m
        .types
        .intern(Ty::Ref(driver_ty, regseq_model::Provenance::UniqueBorrow));
    (m, driver_ty, self_ty, [w1, w2, w4, w8])
}

/// Push a one-op fn: `_2 = &((*_1).regs.inner.<reg>); <mmio op>(_2, ...);
/// return;` — the hoisted-borrow receiver shape the recovery chase expects.
/// `reg_idx` selects the Block field; `extra_args` follow the receiver.
fn push_mmio_op_fn(
    m: &mut RegseqModel,
    self_ty: TyId,
    op: regseq_model::MmioOp,
    reg_idx: u32,
    reg_wrapper: TyId,
    extra_args: Vec<Operand>,
    dest_ty: TyId,
) -> regseq_model::InstanceId {
    // The intrinsic callee (foreign): receiver + optional value/descriptor.
    let recv_ref = m
        .types
        .intern(Ty::Ref(reg_wrapper, regseq_model::Provenance::SharedBorrow));
    let intrinsic = push_intrinsic(
        m,
        match op {
            regseq_model::MmioOp::Read => "mmio_read",
            regseq_model::MmioOp::ReadField => "mmio_read_field",
            regseq_model::MmioOp::Write => "mmio_write",
            regseq_model::MmioOp::WriteField => "mmio_write_field",
            regseq_model::MmioOp::Modify => "mmio_modify",
        },
        IntrinsicKind::Mmio(op),
        vec![recv_ref],
        dest_ty,
    );

    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: dest_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let mut body = Body::default();
    // _0: return place, _1: self, _2: receiver borrow temp.
    body.local_decls.push(LocalDecl {
        ty: dest_ty,
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
    body.local_decls.push(LocalDecl {
        ty: recv_ref,
        mutability: Mutability::Not,
        span: span(),
        debug_name: None,
    });
    body.arg_count = 1;

    // The driver struct projection: (*_1).regs.inner.<reg>.
    let driver_ty = match &m.types[self_ty] {
        Ty::Ref(p, _) => *p,
        _ => unreachable!(),
    };
    let (mmio_ty, block_ty) = {
        let LayoutKind::Struct { fields } = &super::types::layout_of(m, driver_ty).unwrap().kind
        else {
            unreachable!()
        };
        let mmio_ty = fields[0].ty;
        let LayoutKind::Struct { fields } = &super::types::layout_of(m, mmio_ty).unwrap().kind
        else {
            unreachable!()
        };
        (mmio_ty, fields[0].ty)
    };
    let recv_place = Place {
        local: regseq_model::LocalId::from_u32(1),
        projection: vec![
            PlaceElem::Deref,
            PlaceElem::Field(regseq_model::FieldIdx::from_u32(0), mmio_ty),
            PlaceElem::Field(regseq_model::FieldIdx::from_u32(0), block_ty),
            PlaceElem::Field(regseq_model::FieldIdx::from_u32(reg_idx), reg_wrapper),
        ]
        .into_boxed_slice(),
    };

    let mut args: Vec<Operand> = vec![Operand::Move(Place::from(regseq_model::LocalId::from_u32(
        2,
    )))];
    args.extend(extra_args);

    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(regseq_model::LocalId::from_u32(2)),
                Rvalue::Ref(regseq_model::Provenance::SharedBorrow, recv_place),
            ),
            span: span(),
        }]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(intrinsic),
                args: args.into_boxed_slice(),
                destination: Place::from(regseq_model::LocalId::RETURN_PLACE),
                target: Some(regseq_model::BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: span(),
        },
    });

    let fn_item = item(m, ItemKind::Function, "op_fn");
    let emission = m.strings.intern("op_fn");
    m.instances.push(Instance {
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
    })
}

fn lower_cx<'m>(m: &'m RegseqModel, driver_ty: TyId, resolved: bool) -> LowerCx<'m> {
    LowerCx {
        model: m,
        names: TypeNames::new(),
        fn_symbols: std::collections::BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: DriverFacts {
            driver_ty,
            resources: vec![("regs".to_string(), resolved, "regs".to_string())],
            reg_macros: vec![("MMIODRIVER_CR".to_string(), 0x30)],
            base_symbol: "mmiodriver_mmio_base".to_string(),
            pcie: false,
        },
        width8_used: false,
        uses_log: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
        elided_locals: std::collections::BTreeSet::new(),
    }
}

fn lower_one(
    m: &mut RegseqModel,
    driver_ty: TyId,
    fn_id: regseq_model::InstanceId,
    resolved: bool,
) -> Result<(String, bool), EmitError> {
    let mut cx = lower_cx(m, driver_ty, resolved);
    cx.fn_symbols.insert(fn_id, "op_fn".to_string());
    let text = emit_fn(&mut cx, fn_id)?;
    Ok((text, cx.width8_used))
}

#[test]
fn mmio_width_family_selects_sys_io_accessors() {
    // (reg_idx, wrapper_idx, expected read fn, expected offset token)
    let cases: [(u32, usize, &str, &str); 4] = [
        (0, 0, "sys_read8", "mmiodriver_mmio_base"),
        (1, 1, "sys_read16", "mmiodriver_mmio_base + 0x4"),
        (2, 2, "sys_read32", "mmiodriver_mmio_base + MMIODRIVER_CR"),
        (3, 3, "sys_read64", "mmiodriver_mmio_base + 0x38"),
    ];
    for (reg_idx, widx, read_fn, addr) in cases {
        let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
        let dest_ty = match widx {
            0 => m.types.intern(Ty::Uint(UintKind::U8)),
            1 => m.types.intern(Ty::Uint(UintKind::U16)),
            2 => m.types.intern(Ty::Uint(UintKind::U32)),
            _ => m.types.intern(Ty::Uint(UintKind::U64)),
        };
        let f = push_mmio_op_fn(
            &mut m,
            self_ty,
            regseq_model::MmioOp::Read,
            reg_idx,
            widths[widx],
            vec![],
            dest_ty,
        );
        let (text, width8) = lower_one(&mut m, driver_ty, f, true).expect("lower");
        assert!(
            text.contains(&format!("{read_fn}({addr})")),
            "expected `{read_fn}({addr})` in:\n{text}"
        );
        assert_eq!(width8, widx == 3, "width8_used iff the 64-bit register");
    }
}

#[test]
fn mmio_write_is_value_first() {
    let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
    let unit = unit_ty(&mut m);
    let val = m.consts.intern(Const::Uint(UintKind::U32, 0x301));
    let f = push_mmio_op_fn(
        &mut m,
        self_ty,
        regseq_model::MmioOp::Write,
        2,
        widths[2],
        vec![Operand::Constant(val)],
        unit,
    );
    let (text, _) = lower_one(&mut m, driver_ty, f, true).expect("lower");
    // sys_io convention: VALUE first, ADDRESS second.
    assert!(
        text.contains("sys_write32((uint32_t)(0x301u), mmiodriver_mmio_base + MMIODRIVER_CR);"),
        "{text}"
    );
}

#[test]
fn mmio_write_field_confines_value_to_mask() {
    let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
    let unit = unit_ty(&mut m);
    // FieldValue { mask: 0x301, value: 0x301 } as a const aggregate.
    let mask = m.consts.intern(Const::Uint(UintKind::U32, 0x301));
    let value = m.consts.intern(Const::Uint(UintKind::U32, 0x301));
    let fv = m.consts.intern(Const::Aggregate(vec![mask, value]));
    let f = push_mmio_op_fn(
        &mut m,
        self_ty,
        regseq_model::MmioOp::WriteField,
        2,
        widths[2],
        vec![Operand::Constant(fv)],
        unit,
    );
    let (text, _) = lower_one(&mut m, driver_ty, f, true).expect("lower");
    assert!(
        text.contains(
            "sys_write32((uint32_t)((0x301) & (0x301)), mmiodriver_mmio_base + MMIODRIVER_CR);"
        ),
        "{text}"
    );
}

#[test]
fn mmio_read_field_masks_then_shifts() {
    let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    // Field { mask: 1<<3, shift: 3 }.
    let mask = m.consts.intern(Const::Uint(UintKind::U32, 0x8));
    let shift = m.consts.intern(Const::Uint(UintKind::U32, 3));
    let fld = m.consts.intern(Const::Aggregate(vec![mask, shift]));
    let f = push_mmio_op_fn(
        &mut m,
        self_ty,
        regseq_model::MmioOp::ReadField,
        2,
        widths[2],
        vec![Operand::Constant(fld)],
        u32t,
    );
    let (text, _) = lower_one(&mut m, driver_ty, f, true).expect("lower");
    assert!(
        text.contains("((sys_read32(mmiodriver_mmio_base + MMIODRIVER_CR) & (0x8)) >> (0x3))"),
        "{text}"
    );
}

#[test]
fn mmio_modify_preserves_unmentioned_bits() {
    let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
    let unit = unit_ty(&mut m);
    let mask = m.consts.intern(Const::Uint(UintKind::U32, 0x10));
    let value = m.consts.intern(Const::Uint(UintKind::U32, 0x0));
    let fv = m.consts.intern(Const::Aggregate(vec![mask, value]));
    let f = push_mmio_op_fn(
        &mut m,
        self_ty,
        regseq_model::MmioOp::Modify,
        2,
        widths[2],
        vec![Operand::Constant(fv)],
        unit,
    );
    let (text, _) = lower_one(&mut m, driver_ty, f, true).expect("lower");
    // RMW: read-modify-write preserving bits outside the mask.
    assert!(text.contains("uint32_t __cur = sys_read32("), "{text}");
    assert!(
        text.contains("sys_write32((__cur & ~(uint32_t)(0x10)) | ((uint32_t)((0x0) & (0x10))),"),
        "{text}"
    );
}

#[test]
fn mmio_against_unresolved_binding_is_the_a10_hard_error() {
    let (mut m, driver_ty, self_ty, widths) = mmio_block_model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let f = push_mmio_op_fn(
        &mut m,
        self_ty,
        regseq_model::MmioOp::Read,
        2,
        widths[2],
        vec![],
        u32t,
    );
    let err = lower_one(&mut m, driver_ty, f, false).unwrap_err();
    let EmitError::UnresolvedBinding(s) = err else {
        panic!("expected UnresolvedBinding, got {err:?}");
    };
    assert!(
        s.contains("[mmio.regs] base/size") && s.contains("A-10"),
        "{s}"
    );
}

// -- T08: diagnostics-logging lowering — LOG_* reassembly --------------------

/// Intern a static `&str` carrier const: `Aggregate([Reloc -> CMS static,
/// Uint(len)])` with the CMS static holding the literal bytes.
fn str_carrier(m: &mut RegseqModel, text: &[u8]) -> regseq_model::ConstId {
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let bytes: Vec<regseq_model::Byte> =
        text.iter().map(|b| regseq_model::Byte::Value(*b)).collect();
    let bytes_cid = m.consts.intern(Const::Bytes(bytes));
    let st_item = item(
        m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "cms",
    );
    let emission = m.strings.intern("cms");
    let st_id = m.instances.push(Instance {
        item: st_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: u8t,
            init: Some(bytes_cid),
        },
    });
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(st_id)));
    let len = m
        .consts
        .intern(Const::Uint(UintKind::Usize, text.len() as u128));
    m.consts.intern(Const::Aggregate(vec![reloc, len]))
}

/// Push an on_init whose body logs once (the given level/template/args)
/// then returns `Ok(())`.
fn push_logging_on_init(
    m: &mut RegseqModel,
    driver_ty: TyId,
    result_ty: TyId,
    level: regseq_model::LogLevel,
    log_args: Vec<Operand>,
) -> regseq_model::InstanceRef {
    let ut = unit_ty(m);
    let log_intrinsic = push_intrinsic(m, "log_site", IntrinsicKind::Log(level), vec![], ut);
    let self_ty = m
        .types
        .intern(Ty::Ref(driver_ty, regseq_model::Provenance::UniqueBorrow));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let unit_const = m.consts.intern(Const::ZST);
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
    body.local_decls.push(LocalDecl {
        ty: ut,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.arg_count = 1;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(log_intrinsic),
                args: log_args.into_boxed_slice(),
                destination: Place::from(regseq_model::LocalId::from_u32(2)),
                target: Some(regseq_model::BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(regseq_model::LocalId::RETURN_PLACE),
                Rvalue::Aggregate(
                    regseq_model::AggregateKind::Enum(result_ty, VariantIdx::from_u32(0)),
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

/// Model with a logging on_init; returns the emit result.
fn emit_with_log(
    level: regseq_model::LogLevel,
    template: &[u8],
    extra_args: impl FnOnce(&mut RegseqModel) -> Vec<Operand>,
) -> Result<super::ZephyrEmit, EmitError> {
    let mut m = minimal_dt_model();
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, &["HardwareError", "DeviceBusy"]);
    let fmt = str_carrier(&mut m, template);
    let mut args: Vec<Operand> = vec![Operand::Constant(fmt)];
    args.extend(extra_args(&mut m));
    let on_init = push_logging_on_init(&mut m, driver_ty, result_ty, level, args);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    emit_zephyr_c(&m)
}

#[test]
fn log_levels_select_the_zephyr_macros() {
    use regseq_model::LogLevel;
    for (level, mac) in [
        (LogLevel::Debug, "LOG_DBG(\"probe start\");"),
        (LogLevel::Info, "LOG_INF(\"probe start\");"),
        (LogLevel::Warn, "LOG_WRN(\"probe start\");"),
        (LogLevel::Error, "LOG_ERR(\"probe start\");"),
    ] {
        let emit = emit_with_log(level, b"probe start", |_| vec![]).expect("must emit");
        assert!(emit.tu.contains(mac), "{}", emit.tu);
        // The [LEVEL] text tag is structural via the macro — never in fmt.
        assert!(
            !emit.tu.contains("[DEBUG]") && !emit.tu.contains("[INFO]"),
            "{}",
            emit.tu
        );
    }
}

#[test]
fn log_demand_wires_the_module_block_and_a9_pins() {
    let emit = emit_with_log(regseq_model::LogLevel::Info, b"up", |_| vec![]).expect("emit");
    let tu = &emit.tu;
    // Demand-driven module block (scaffold §1 item 2), A-14 name.
    assert!(tu.contains("#define LOG_MODULE_NAME minimal"), "{tu}");
    assert!(tu.contains("LOG_MODULE_REGISTER(LOG_MODULE_NAME);"), "{tu}");
    assert!(tu.contains("#include <zephyr/logging/log.h>"), "{tu}");
    // The A-9 Kconfig pins ride the TU header when the log block exists.
    assert!(tu.contains("CONFIG_CBPRINTF_FULL_INTEGRAL"), "{tu}");
}

#[test]
fn log_literal_percent_doubles_and_no_trailing_newline() {
    let emit = emit_with_log(
        regseq_model::LogLevel::Info,
        b"duty 50% reached",
        |_| vec![],
    )
    .expect("emit");
    assert!(
        emit.tu.contains("LOG_INF(\"duty 50%% reached\");"),
        "{}",
        emit.tu
    );
    assert!(!emit.tu.contains("reached\\n"), "{}", emit.tu);
}

#[test]
fn log_numeric_args_use_the_certified_mapping() {
    // u32 default + u32 alternate-hex: "{} {:#x}" -> "%u %#x".
    let emit = emit_with_log(regseq_model::LogLevel::Info, b"vals {} {:#x}", |m| {
        let a = m.consts.intern(Const::Uint(UintKind::U32, 13));
        let b = m.consts.intern(Const::Uint(UintKind::U32, 0x301));
        vec![Operand::Constant(a), Operand::Constant(b)]
    })
    .expect("emit");
    assert!(
        emit.tu
            .contains("LOG_INF(\"vals %u %#x\", (unsigned)(0xdu), (unsigned)(0x301u));"),
        "{}",
        emit.tu
    );
}

#[test]
fn log_static_nul_free_str_arg_is_delivered() {
    let emit = emit_with_log(regseq_model::LogLevel::Warn, b"mode {}", |m| {
        vec![Operand::Constant(str_carrier(m, b"polling"))]
    })
    .expect("emit");
    assert!(
        emit.tu.contains("LOG_WRN(\"mode %s\", \"polling\");"),
        "{}",
        emit.tu
    );
}

#[test]
fn log_static_str_with_interior_nul_is_rejected_citing_a6() {
    let err = emit_with_log(regseq_model::LogLevel::Warn, b"mode {}", |m| {
        vec![Operand::Constant(str_carrier(m, b"po\0lling"))]
    })
    .unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("A-6") && s.contains("interior NUL"), "{s}");
}

#[test]
fn log_non_static_str_arg_is_rejected_citing_a5() {
    // A runtime &str place as the log argument (the A-5 deferred-mode gap).
    // Through `emit_zephyr_c` the fat-pointer LOCAL already fails loud at
    // declaration; the A-5 arm is the bank's defense-in-depth — drive the
    // seam directly (the T06 pattern) so its citation is locked.
    let (mut m, driver_ty, _self_ty, _w) = mmio_block_model();
    let ut = unit_ty(&mut m);
    let fmt = str_carrier(&mut m, b"mode {}");
    let str_ty = m.types.intern(Ty::Str);
    let str_ref = m
        .types
        .intern(Ty::Ref(str_ty, regseq_model::Provenance::SharedBorrow));
    let mut body = Body::default();
    body.local_decls.push(LocalDecl {
        ty: ut,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.local_decls.push(LocalDecl {
        ty: str_ref,
        mutability: Mutability::Not,
        span: span(),
        debug_name: Some(m.strings.intern("s")),
    });
    body.arg_count = 1;
    let mut cx = lower_cx(&m, driver_ty, true);
    let mut out = String::new();
    let err = cx
        .emit_diag_log(
            &regseq_model::LogLevel::Info,
            &body,
            &[
                Operand::Constant(fmt),
                Operand::Copy(Place::from(regseq_model::LocalId::from_u32(1))),
            ],
            &mut out,
        )
        .unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("A-5") && s.contains("non-static"), "{s}");
}

#[test]
fn log_non_static_str_arg_is_rejected_citing_a5_on_the_public_path() {
    // The C2-review fix: the A-5 gate fires through `emit_zephyr_c` (the
    // closure-level frame scan), BEFORE any local/signature C-type emission
    // can trip over the fat pointer with a generic Unsupported.
    let mut m = minimal_dt_model();
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. } if {
                m.strings.resolve(m.items[*item].def_path.rendered) == "Minimal"
            })
        })
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, &["HardwareError", "DeviceBusy"]);
    let fmt = str_carrier(&mut m, b"mode {}");
    let str_ty = m.types.intern(Ty::Str);
    let str_ref = m
        .types
        .intern(Ty::Ref(str_ty, regseq_model::Provenance::SharedBorrow));
    let ut = unit_ty(&mut m);
    let log_intrinsic = push_intrinsic(
        &mut m,
        "log_site",
        IntrinsicKind::Log(regseq_model::LogLevel::Info),
        vec![],
        ut,
    );
    let self_ty = m
        .types
        .intern(Ty::Ref(driver_ty, regseq_model::Provenance::UniqueBorrow));
    let sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
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
    body.local_decls.push(LocalDecl {
        ty: str_ref,
        mutability: Mutability::Not,
        span: span(),
        debug_name: Some(m.strings.intern("s")),
    });
    body.local_decls.push(LocalDecl {
        ty: ut,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    body.arg_count = 1;
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(log_intrinsic),
                args: Box::new([
                    Operand::Constant(fmt),
                    Operand::Copy(Place::from(regseq_model::LocalId::from_u32(2))),
                ]),
                destination: Place::from(regseq_model::LocalId::from_u32(3)),
                target: Some(regseq_model::BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    let unit_const = m.consts.intern(Const::ZST);
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(regseq_model::LocalId::RETURN_PLACE),
                Rvalue::Aggregate(
                    regseq_model::AggregateKind::Enum(result_ty, VariantIdx::from_u32(0)),
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
    let fn_item = item(&mut m, ItemKind::Function, "on_init");
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
    let on_init = m.instance_ref_of(id);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    let err = emit_zephyr_c(&m).unwrap_err();
    let EmitError::Rejected(s) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(s.contains("A-5") && s.contains("non-static"), "{s}");
}

#[test]
fn delay_helpers_emit_zephyr_kernel_primitives_and_round_up() {
    // Timed-cpu-delay-and-sleep (TCD construct, M2/T09): busy_wait ->
    // k_busy_wait(us) (round up to us, no magnitude bands); sleep ->
    // k_msleep(ms) (round up to ms). Minimum-wait (R06) via the overflow-safe
    // `ns == 0 ? 0 : 1 + (ns-1)/scale` ceiling.
    let h = super::body::delay_helpers(true, true);
    assert!(h.contains("static void regseq_zephyr_busy_wait_ns(uint64_t ns)"));
    assert!(
        h.contains("k_busy_wait((uint32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000ULL))"),
        "k_busy_wait round-up: {h}"
    );
    assert!(h.contains("static void regseq_zephyr_sleep_ns(uint64_t ns)"));
    assert!(
        h.contains("k_msleep((int32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000000ULL))"),
        "k_msleep round-up: {h}"
    );
    // Demand-precise gating.
    let busy_only = super::body::delay_helpers(true, false);
    assert!(busy_only.contains("busy_wait_ns") && !busy_only.contains("sleep_ns"));
}
