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

use super::{emit_linux_c, errno_symbol, EmitError};
use crate::capability;

// -- T19: pinctrl errno mapping (GRILL D12/C08) ------------------------------

/// The pinctrl optional-absent fold emits `Err(RegseqError::PinctrlAbsent)`;
/// the linux_c errno adapter must map it to `ENODEV` (else the scaffold exit
/// adapter fails loud on the unmapped variant when a 3-variant pinctrl
/// `RegseqError` reaches a probe-return). Mirrors generic_c/errno.rs (M3) and
/// zephyr_c (T19). Shares the `ENODEV` symbol with `DeviceNotFound`
/// (name-keyed, order-independent); an unmapped name still yields `None` so the
/// caller fails loud rather than defaulting.
#[test]
fn pinctrl_absent_maps_to_enodev() {
    assert_eq!(errno_symbol("PinctrlAbsent"), Some("ENODEV"));
    assert_eq!(errno_symbol("DeviceNotFound"), Some("ENODEV"));
    assert_eq!(errno_symbol("HardwareError"), Some("EIO"));
    assert_eq!(errno_symbol("Frobnicate"), None);
}

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
fn result_carrier(m: &mut RegseqModel, err_variant_names: [&str; 2]) -> (TyId, TyId) {
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

fn pci_match() -> Vec<MatchEntry> {
    vec![MatchEntry::Pci {
        vendor: 0x8086,
        device: 0x100e,
        subsystem: None,
        class: None,
    }]
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
fn all_foreign_match_set_under_amba_is_rejected_loud() {
    // C06 (spec-convergence-pl011): the D03(a) rule licenses ignoring
    // foreign entries only when >=1 native entry exists — an all-foreign
    // set rejects loudly naming the fix (the linux analog of zephyr A-13).
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
    let err = emit_linux_c(&m).unwrap_err();
    let EmitError::Driver(msg) = err else {
        panic!("expected Driver error, got {err:?}");
    };
    assert!(
        msg.contains("no native entries")
            && msg.contains("amba_driver")
            && msg.contains("amba = [{ id, mask }]"),
        "the all-foreign rejection names the rule and the fix: {msg}"
    );
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
    assert!(
        devtable < drvstruct,
        "match table precedes the driver struct"
    );
    assert!(drvstruct < macros, "driver struct precedes module macros");
    assert!(macros < license);
}

#[test]
fn probe_carries_create_associate_and_the_negated_adapter() {
    let tu = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    // create → associate (canonical order, §C.3).
    let create = tu
        .find("devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL)")
        .unwrap();
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
    let second = tu
        .find(".id\t= 0x00042222,")
        .expect("second entry (FULL match_set)");
    let sentinel = tu.find("{ 0, 0 },").expect("zero sentinel");
    assert!(first < second && second < sentinel);
}

#[test]
fn foreign_dt_entry_under_amba_is_audited_not_tabled() {
    // C06/C03/A2 (spec-convergence-pl011): a recognized foreign entry under
    // an amba driver EMITS — audited adjacent to the id table, ABSENT from
    // the emitted table, and carried in the report and the summary line, all
    // three renders from the one MatchAudit value (D01).
    let mut m = minimal_amba_model();
    {
        let compatible = m.strings.intern("regseq,pl011");
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver
            .descriptor
            .match_set
            .push(MatchEntry::Devicetree { compatible });
    }
    let emit = emit_linux_c(&m).expect("mixed set must emit (C06)");
    let tu = &emit.tu;
    // (i) the C block, adjacent to (immediately preceding) the id table.
    let block = tu
        .find("/* RSEQ-IGNORED-MATCH: devicetree compatible=\"regseq,pl011\"")
        .unwrap_or_else(|| panic!("audit block present:\n{tu}"));
    let table = tu
        .find("static const struct amba_id minimal_ids[]")
        .expect("table");
    assert!(block < table, "audit block precedes the table");
    assert!(tu.contains("not consumed by the ratified amba_driver registration form"));
    assert!(tu.contains("consumed natively by zephyr_c."));
    // A2: the foreign entry feeds only the audit — the table carries exactly
    // the native row; never-synthesize holds structurally.
    let table_body = &tu[table..tu.find("{ 0, 0 },").expect("sentinel")];
    assert_eq!(
        table_body.matches(".id\t=").count(),
        1,
        "exactly the native row in the table"
    );
    assert!(
        !table_body.contains("regseq,pl011"),
        "foreign entry absent from the emitted table (A2)"
    );
    assert!(
        !tu.contains("of_match_table"),
        "never-synthesize: no of_match_table minted from the compatible"
    );
    // (ii) the report section — and the block rendered from the SAME value.
    assert_eq!(emit.conformance.ignored_match_entries.len(), 1);
    let audit = &emit.conformance.ignored_match_entries[0];
    assert_eq!(audit.entry, "devicetree compatible=\"regseq,pl011\"");
    assert_eq!(audit.native_on, &["zephyr_c"]);
    assert!(tu.contains(&audit.c_block()), "same-struct render (I03)");
    // (iii) the summary line section.
    assert!(
        emit.conformance
            .summary_line()
            .ends_with("ignored=[devicetree compatible=\"regseq,pl011\"]"),
        "{}",
        emit.conformance.summary_line()
    );
}

/// Build a minimal model with one extra callback (besides on_init) bound.
fn model_with_callback(cb: &str) -> RegseqModel {
    let mut m = driver_model(Some(BusType::Amba), amba_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let on_init = push_ok_on_init(&mut m, driver_ty, result_ty);
    let extra = push_ok_on_init(&mut m, driver_ty, result_ty);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    match cb {
        "on_surprise_remove" => driver.ops.on_surprise_remove = Some(extra),
        "on_init_failure" => driver.ops.on_init_failure = Some(extra),
        "on_system_shutdown" => driver.ops.on_system_shutdown = Some(extra),
        _ => unreachable!(),
    }
    m
}

#[test]
fn amba_surprise_remove_fails_loud() {
    // No ratified AMBA present predicate: fail loud, naming the PCI unit.
    let err = emit_linux_c(&model_with_callback("on_surprise_remove")).unwrap_err();
    assert!(
        matches!(err, EmitError::Unsupported(ref s) if s.contains("on_surprise_remove")),
        "got {err:?}"
    );
}

#[test]
fn on_init_failure_hook_runs_on_the_authored_err_arm_only() {
    let tu = emit_linux_c(&model_with_callback("on_init_failure"))
        .unwrap()
        .tu;
    let hook = tu
        .find("(void)minimal_on_init_failure(&priv->inner);")
        .expect("the failure hook is emitted");
    let err_branch = tu.find("if (__tag != 0x2)").expect("the Err branch");
    let switch = tu.find("switch (__err)").expect("the errno switch");
    assert!(
        err_branch < hook && hook < switch,
        "the hook must run inside the authored Err arm, before the errno return"
    );
}

#[test]
fn shutdown_slot_is_omit_when_absent() {
    let with = emit_linux_c(&model_with_callback("on_system_shutdown"))
        .unwrap()
        .tu;
    assert!(with.contains("static void minimal_shutdown(struct device *dev)"));
    assert!(with.contains(".shutdown = minimal_shutdown,"));
    assert!(with.contains("(void)minimal_on_system_shutdown(&priv->inner);"));

    let without = emit_linux_c(&minimal_amba_model()).unwrap().tu;
    assert!(
        !without.contains(".shutdown"),
        "the .shutdown initializer must be omitted when on_system_shutdown is not overridden"
    );
}

#[test]
fn resolved_region_probe_carries_the_devm_claim_map_and_native_channel() {
    let tu = emit_linux_c(&mmio_write_model(4, true)).unwrap().tu;
    // Canonical order: create -> associate -> preamble -> on_init.
    let create = tu.find("devm_kzalloc").unwrap();
    let associate = tu.find("amba_set_drvdata").unwrap();
    let map = tu
        .find("priv->inner.regs = devm_ioremap_resource(&adev->dev, &adev->res);")
        .expect("devm claim+map fused acquisition");
    let native = tu
        .find("return PTR_ERR(priv->inner.regs);")
        .expect("the native error channel propagates directly");
    let on_init = tu.find("mmiodrv_on_init(&priv->inner)").unwrap();
    assert!(create < associate && associate < map && map < native && native < on_init);
    // The postamble emits nothing (devm, A-2) and remove stays void.
    assert!(tu.contains("devm-managed resources release automatically"));
}

// ---------------------------------------------------------------------------
// T06: register-access lowering — accessor selection (width-switched),
// value-first arg order, const bank, A-12 guard both polarities, and the
// ratified A-10 unresolved-binding hard error.
// ---------------------------------------------------------------------------

/// Build a driver model whose `on_init` performs one whole-register write
/// of `0x40` through a `Mmio<Regs>`-shaped resource field whose single
/// register has the given byte `width`. `resolved` controls the L2 binding
/// (the A-10 trigger).
fn mmio_write_model(width: u64, resolved: bool) -> RegseqModel {
    mmio_write_model_for(width, resolved, BusType::Amba, amba_match(), None)
}

/// The PCI flavor of [`mmio_write_model`] (backend-linux-c-e1000 T04):
/// width-4 write, PCI bus + match, with the given `effective_bar`.
fn pci_mmio_write_model(resolved: bool, effective_bar: Option<u8>) -> RegseqModel {
    mmio_write_model_for(4, resolved, BusType::Pci, pci_match(), effective_bar)
}

fn mmio_write_model_for(
    width: u64,
    resolved: bool,
    bus: BusType,
    match_set: Vec<MatchEntry>,
    effective_bar: Option<u8>,
) -> RegseqModel {
    let mut m = model();
    let store_kind = match width {
        1 => UintKind::U8,
        2 => UintKind::U16,
        4 => UintKind::U32,
        _ => UintKind::U64,
    };
    let store = m.types.intern(Ty::Uint(store_kind));

    // The register wrapper (repr(transparent) over the storage width).
    let wrapper_ty = {
        let f = FieldLayout {
            name: m.strings.intern("_v"),
            ty: store,
            offset: 0,
        };
        push_struct(&mut m, "ReadWrite", struct_layout(width, width, vec![f]))
    };
    // The block: one register `ctrl` at offset 0.
    let block_ty = {
        let f = FieldLayout {
            name: m.strings.intern("ctrl"),
            ty: wrapper_ty,
            offset: 0,
        };
        push_struct(&mut m, "Regs", struct_layout(width, width, vec![f]))
    };
    // The Mmio resource carrier.
    let mmio_ty = {
        let f = FieldLayout {
            name: m.strings.intern("inner"),
            ty: block_ty,
            offset: 0,
        };
        push_struct(&mut m, "Mmio", struct_layout(width, width, vec![f]))
    };
    // The driver struct with the `regs` resource field.
    let driver_ty = {
        let f = FieldLayout {
            name: m.strings.intern("regs"),
            ty: mmio_ty,
            offset: 0,
        };
        push_struct(&mut m, "MmioDrv", struct_layout(width, width, vec![f]))
    };
    let driver_item = match &m.types[driver_ty] {
        Ty::Struct { item, .. } => *item,
        _ => unreachable!(),
    };

    // The intrinsic-anchored `mmio_write` callee (no body; intercepted).
    let recv_ty = m
        .types
        .intern(Ty::Ref(wrapper_ty, Provenance::SharedBorrow));
    let unit_out = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let write_sig = m.sigs.intern(FnSig {
        inputs: vec![recv_ty, store],
        output: unit_out,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let write_item = item(&mut m, ItemKind::Function, "write");
    while m.domain_attrs.item_attrs.len() <= write_item.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[write_item].push(regseq_model::AttrData::Intrinsic(
        regseq_model::IntrinsicKind::Mmio(regseq_model::MmioOp::Write),
    ));
    let write_emission = m.strings.intern("write");
    let write_id = m.instances.push(Instance {
        item: write_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: write_emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: write_sig,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });

    // on_init: borrow self.regs.inner.ctrl, call write(recv, 0x40), Ok(()).
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let self_ty = m.types.intern(Ty::Ref(driver_ty, Provenance::UniqueBorrow));
    let on_init_sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: result_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let val_const = m.consts.intern(Const::Uint(store_kind, 0x40));
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
        debug_name: None,
    });
    body.arg_count = 1;
    let recv_local = body.push_synthetic_local(recv_ty);
    use regseq_model::{BlockId, FieldIdx, LocalId, PlaceElem};
    let recv_place = Place {
        local: LocalId::from_u32(1),
        projection: Box::new([
            PlaceElem::Deref,
            PlaceElem::Field(FieldIdx::from_u32(0), mmio_ty),
            PlaceElem::Field(FieldIdx::from_u32(0), block_ty),
            PlaceElem::Field(FieldIdx::from_u32(0), wrapper_ty),
        ]),
    };
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(recv_local),
                Rvalue::Ref(Provenance::SharedBorrow, recv_place),
            ),
            span: span(),
        }]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(write_id),
                args: Box::new([
                    Operand::Move(Place::from(recv_local)),
                    Operand::Constant(val_const),
                ]),
                destination: Place::from(LocalId::from_u32(0)),
                target: Some(BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
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
    // Patch: the Call writes a unit destination; use a dedicated ZST local.
    let unit_local = body.push_synthetic_local(unit_out);
    if let TerminatorKind::Call { destination, .. } =
        &mut body.basic_blocks[BlockId::from_u32(0)].terminator.kind
    {
        *destination = Place::from(unit_local);
    }

    let on_init_item = item(&mut m, ItemKind::Function, "on_init");
    let emission = m.strings.intern("on_init");
    let on_init_id = m.instances.push(Instance {
        item: on_init_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: on_init_sig,
            body: Some(body),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let on_init_ref = m.instance_ref_of(on_init_id);

    let name = m.strings.intern("mmiodrv");
    let regs_name = m.strings.intern("regs");
    m.device_lifecycle.drivers.insert(
        driver_item,
        regseq_model::DriverLifecycle {
            descriptor: regseq_model::DriverDescriptor {
                name,
                match_set,
                device_class: regseq_model::ClassTag::Generic,
                bus_type: Some(bus),
                dma_present: false,
            },
            handle: regseq_model::DriverHandle {
                resource_fields: vec![regseq_model::ResourceField {
                    name: regs_name,
                    kind: regseq_model::ResourceKind::Mmio,
                    binding: regseq_model::MmioBinding {
                        id: Some(regs_name),
                        bar_index: None,
                        bar_type: regseq_model::BarType::Mem,
                        effective_bar,
                        base: resolved.then_some(0x0900_0000),
                        size: resolved.then_some(0x1000),
                    },
                }],
                user_data_fields: vec![],
            },
            ops: regseq_model::LifecycleOps {
                on_init: Some(on_init_ref),
                ..Default::default()
            },
        },
    );
    m
}

#[test]
fn mmio_write_selects_the_width_accessor_value_first() {
    for (width, accessor, cast) in [
        (1, "writeb", "u8"),
        (2, "writew", "u16"),
        (4, "writel", "u32"),
        (8, "writeq", "u64"),
    ] {
        let emit = emit_linux_c(&mmio_write_model(width, true))
            .unwrap_or_else(|e| panic!("width {width}: {e}"));
        let needle = format!("{accessor}(({cast})(0x40u");
        assert!(
            emit.tu.contains(&needle) || emit.tu.contains(&format!("{accessor}(({cast})(0x40")),
            "width {width} must lower to value-first {accessor}; TU:\n{}",
            emit.tu
        );
        // The iomem cookie is the base; the register is at offset 0.
        assert!(
            emit.tu.contains("_1->regs)"),
            "the access must address the resource cookie; TU:\n{}",
            emit.tu
        );
    }
}

#[test]
fn width8_injects_the_a12_guard_and_width4_does_not() {
    let with = emit_linux_c(&mmio_write_model(8, true)).unwrap().tu;
    assert!(with.contains("#ifndef CONFIG_64BIT"));
    assert!(with.contains("#error"));
    let without = emit_linux_c(&mmio_write_model(4, true)).unwrap().tu;
    assert!(
        !without.contains("CONFIG_64BIT"),
        "width-4 TU must carry no guard"
    );
}

#[test]
fn mmio_tu_carries_the_register_const_bank_and_io_include() {
    let tu = emit_linux_c(&mmio_write_model(4, true)).unwrap().tu;
    assert!(tu.contains("#define MMIODRV_CTRL 0x000"));
    assert!(tu.contains("#include <linux/io.h>"));
}

#[test]
fn unresolved_binding_is_the_a10_hard_error() {
    let err = emit_linux_c(&mmio_write_model(4, false)).unwrap_err();
    let EmitError::UnresolvedBinding(msg) = err else {
        panic!("expected the A-10 UnresolvedBinding error, got {err:?}");
    };
    assert!(
        msg.contains("[mmio.regs]") && msg.contains("base/size"),
        "the A-10 error must name the missing L2 literals: {msg}"
    );
}

// ---------------------------------------------------------------------------
// T08 (ratified A-11): the config-intrinsic body-lowering seam — explicit
// `(void)` status discards in BOTH directions, proven by driving
// `emit_config` directly with synthetic config-op bodies. The full-TU PCI
// path stays fail-loud (D02), asserted at the TU interception.
// ---------------------------------------------------------------------------

/// A bare lowering context + body for driving the seam directly.
fn seam_cx_and_body(m: &RegseqModel, dst_ty: TyId) -> (super::body::LowerCx<'_>, Body) {
    let cx = super::body::LowerCx {
        model: m,
        names: super::types::TypeNames::new(),
        fn_symbols: std::collections::BTreeMap::new(),
        extra_includes: std::collections::BTreeSet::new(),
        driver: super::body::DriverFacts {
            driver_ty: dst_ty, // unused by the config seam
            resources: vec![],
            reg_macros: vec![],
            bus_type: Some(BusType::Pci),
            drv: "seam".into(),
        },
        width8_used: false,
        uses_pr: false,
        uses_delay_busy: false,
        uses_delay_sleep: false,
        elided_locals: std::collections::BTreeSet::new(),
    };
    let body = Body::default();
    (cx, body)
}

#[test]
fn a11_read_config_carries_the_void_discard() {
    let mut m = model();
    let u16t = m.types.intern(Ty::Uint(UintKind::U16));
    let off = m.consts.intern(Const::Uint(UintKind::U16, 0x6));
    let (mut cx, mut body) = seam_cx_and_body(&m, u16t);
    let dst = body.push_synthetic_local(u16t);
    let mut out = String::new();
    cx.emit_config(
        regseq_model::ConfigOp::Read,
        &body,
        &[Operand::Constant(off)],
        &Place::from(dst),
        super::body::ConfigDevice {
            pdev: "pdev",
            recovery: None,
        },
        &mut out,
    )
    .expect("read_config lowers");
    assert!(
        out.contains("(void)pci_read_config_word(pdev, (int)(0x6u), &__cv);"),
        "the read status must be explicitly discarded: {out}"
    );
    assert!(out.contains("= (u16)__cv;"), "out-param marshalling: {out}");
}

#[test]
fn a11_write_config_carries_the_void_discard() {
    let mut m = model();
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let off = m.consts.intern(Const::Uint(UintKind::U16, 0x4));
    let val = m.consts.intern(Const::Uint(UintKind::U32, 0x146));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let (mut cx, mut body) = seam_cx_and_body(&m, u32t);
    let dst = body.push_synthetic_local(unit);
    let mut out = String::new();
    cx.emit_config(
        regseq_model::ConfigOp::Write,
        &body,
        &[Operand::Constant(off), Operand::Constant(val)],
        &Place::from(dst),
        super::body::ConfigDevice {
            pdev: "pdev",
            recovery: None,
        },
        &mut out,
    )
    .expect("write_config lowers");
    // The bracketed write shape (C01: the braces block scopes the per-site
    // recovery when present).
    assert!(
        out.contains(
            "\t{\n\t\t(void)pci_write_config_dword(pdev, (int)(0x4u), (u32)(0x146u));\n\t}\n"
        ),
        "the write must be bracketed with an explicit status discard: {out}"
    );
}

#[test]
fn a11_width8_config_fails_loud() {
    let mut m = model();
    let u64t = m.types.intern(Ty::Uint(UintKind::U64));
    let off = m.consts.intern(Const::Uint(UintKind::U16, 0x0));
    let (mut cx, mut body) = seam_cx_and_body(&m, u64t);
    let dst = body.push_synthetic_local(u64t);
    let mut out = String::new();
    let err = cx
        .emit_config(
            regseq_model::ConfigOp::Read,
            &body,
            &[Operand::Constant(off)],
            &Place::from(dst),
            super::body::ConfigDevice {
                pdev: "pdev",
                recovery: None,
            },
            &mut out,
        )
        .unwrap_err();
    assert!(
        matches!(err, EmitError::Unsupported(ref s) if s.contains("B-3")),
        "width-8 config must fail loud naming the ledger gap: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// T09: diagnostics-logging — printk reassembly (one pr_* call per Log,
// level -> macro, literal % doubling, certified specifier mapping, pr_fmt
// module prefix; A-3: pr_* in lowered bodies).
// ---------------------------------------------------------------------------

/// Intern a `&str` carrier constant for `text` (Aggregate[Reloc -> CMS
/// static Bytes, Uint(len)]).
fn str_carrier(m: &mut RegseqModel, text: &str) -> regseq_model::ConstId {
    let bytes: Vec<regseq_model::Byte> = text.bytes().map(regseq_model::Byte::Value).collect();
    let bytes_cid = m.consts.intern(Const::Bytes(bytes));
    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let cms_item = item(
        m,
        ItemKind::Static {
            mutability: Mutability::Not,
        },
        "fmt_cms",
    );
    let emission = m.strings.intern("fmt_cms");
    let cms_id = m.instances.push(Instance {
        item: cms_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Static {
            ty: u8t,
            init: Some(bytes_cid),
        },
    });
    let reloc = m.consts.intern(Const::Reloc(m.instance_ref_of(cms_id)));
    let len = m
        .consts
        .intern(Const::Uint(UintKind::Usize, text.len() as u128));
    m.consts.intern(Const::Aggregate(vec![reloc, len]))
}

/// A minimal AMBA model whose on_init performs one Log call at `level` with
/// `template` and the given value operands, then returns Ok(()).
fn log_model(
    level: regseq_model::LogLevel,
    template: &str,
    extra_args: impl Fn(&mut RegseqModel) -> Vec<Operand>,
) -> RegseqModel {
    let mut m = driver_model(Some(BusType::Amba), amba_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);

    // The intrinsic-anchored log callee.
    let unit_out = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let log_sig = m.sigs.intern(FnSig {
        inputs: vec![],
        output: unit_out,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let log_item = item(&mut m, ItemKind::Function, "log_anchor");
    while m.domain_attrs.item_attrs.len() <= log_item.as_u32() as usize {
        m.domain_attrs.item_attrs.push(Vec::new());
    }
    m.domain_attrs.item_attrs[log_item].push(regseq_model::AttrData::Intrinsic(
        regseq_model::IntrinsicKind::Log(level),
    ));
    let log_emission = m.strings.intern("log_anchor");
    let log_id = m.instances.push(Instance {
        item: log_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: log_emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: log_sig,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });

    // on_init: log(...) -> Ok(()).
    let fmt = str_carrier(&mut m, template);
    let mut call_args = vec![Operand::Constant(fmt)];
    call_args.extend(extra_args(&mut m));
    let self_ty = m.types.intern(Ty::Ref(driver_ty, Provenance::UniqueBorrow));
    let on_init_sig = m.sigs.intern(FnSig {
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
        debug_name: None,
    });
    body.arg_count = 1;
    use regseq_model::{BlockId, LocalId};
    let unit_out2 = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });
    let unit_local = body.push_synthetic_local(unit_out2);
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(log_id),
                args: call_args.into_boxed_slice(),
                destination: Place::from(unit_local),
                target: Some(BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
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
    let on_init_item = item(&mut m, ItemKind::Function, "on_init");
    let emission = m.strings.intern("on_init");
    let on_init_id = m.instances.push(Instance {
        item: on_init_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: on_init_sig,
            body: Some(body),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    let on_init_ref = m.instance_ref_of(on_init_id);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init_ref);
    m
}

#[test]
fn log_reassembles_one_pr_call_with_doubled_percent_and_specs() {
    let m = log_model(regseq_model::LogLevel::Info, "duty 50% {} {:#x}", |m| {
        vec![
            Operand::Constant(m.consts.intern(Const::Uint(UintKind::U32, 13))),
            Operand::Constant(m.consts.intern(Const::Uint(UintKind::U32, 0x301))),
        ]
    });
    let tu = emit_linux_c(&m).unwrap().tu;
    assert!(
        tu.contains("pr_info(\"duty 50%% %u %#x\\n\", (unsigned)(0xdu), (unsigned)(0x301u));"),
        "one reassembled pr_info with doubled %% and certified specs; TU:\n{tu}"
    );
    // pr_fmt module prefix precedes every include (A-3).
    let prfmt = tu
        .find("#define pr_fmt(fmt) KBUILD_MODNAME \": \" fmt")
        .expect("pr_fmt define");
    let first_inc = tu.find("#include").unwrap();
    assert!(prfmt < first_inc);
    assert!(tu.contains("#include <linux/printk.h>"));
    // The [LEVEL] text tag is dropped (severity is the macro).
    assert!(!tu.contains("[INFO]"));
    // A-3: no dev_* in lowered bodies (no scaffold-synthesized logs exist).
    assert!(!tu.contains("dev_info("));
}

#[test]
fn log_levels_map_to_the_printk_macro_family() {
    use regseq_model::LogLevel;
    for (level, macro_name) in [
        (LogLevel::Debug, "pr_debug"),
        (LogLevel::Info, "pr_info"),
        (LogLevel::Warn, "pr_warn"),
        (LogLevel::Error, "pr_err"),
    ] {
        let m = log_model(level, "tick", |_| vec![]);
        let tu = emit_linux_c(&m).unwrap().tu;
        assert!(
            tu.contains(&format!("{macro_name}(\"tick\\n\");")),
            "{macro_name} expected; TU:\n{tu}"
        );
    }
}

#[test]
fn static_str_value_lowers_to_dot_star_s() {
    let m = log_model(regseq_model::LogLevel::Info, "dev={}", |m| {
        vec![Operand::Constant(str_carrier(m, "uart0"))]
    });
    let tu = emit_linux_c(&m).unwrap().tu;
    assert!(
        tu.contains("pr_info(\"dev=%.*s\\n\", (int)5, \"uart0\");"),
        "static &str -> %.*s with decoded literal + len; TU:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// C3-close P1 regression: a function returning a trivial-accessor result is
// never elided into the return place — it fails loud (the accessor is not a
// lowered instance), instead of returning an uninitialized value.
// ---------------------------------------------------------------------------

#[test]
fn returning_an_accessor_result_fails_loud() {
    let mut m = mmio_write_model(4, true);
    // The accessor: &Mmio<Regs> -> &Regs (`_0 = &((*_1).inner); return;`).
    let mmio_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. }
            if m.strings.resolve(m.items[*item].def_path.rendered) == "Mmio")
        })
        .map(|(id, _)| id)
        .unwrap();
    let block_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. }
            if m.strings.resolve(m.items[*item].def_path.rendered) == "Regs")
        })
        .map(|(id, _)| id)
        .unwrap();
    let mmio_ref = m.types.intern(Ty::Ref(mmio_ty, Provenance::SharedBorrow));
    let block_ref = m.types.intern(Ty::Ref(block_ty, Provenance::SharedBorrow));
    use regseq_model::{BlockId, FieldIdx, LocalId, PlaceElem};
    let deref_sig = m.sigs.intern(FnSig {
        inputs: vec![mmio_ref],
        output: block_ref,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let mut deref_body = Body::default();
    deref_body.local_decls.push(LocalDecl {
        ty: block_ref,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    deref_body.local_decls.push(LocalDecl {
        ty: mmio_ref,
        mutability: Mutability::Not,
        span: span(),
        debug_name: None,
    });
    deref_body.arg_count = 1;
    deref_body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
                Rvalue::Ref(
                    Provenance::SharedBorrow,
                    Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Deref,
                            PlaceElem::Field(FieldIdx::from_u32(0), block_ty),
                        ]),
                    },
                ),
            ),
            span: span(),
        }]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: span(),
        },
    });
    let deref_item = item(&mut m, ItemKind::Function, "deref");
    let deref_emission = m.strings.intern("deref");
    let deref_id = m.instances.push(Instance {
        item: deref_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: deref_emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: deref_sig,
            body: Some(deref_body),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });

    // A wrapper whose RETURN PLACE is the accessor call's destination.
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| {
            matches!(t, Ty::Struct { item, .. }
            if m.strings.resolve(m.items[*item].def_path.rendered) == "MmioDrv")
        })
        .map(|(id, _)| id)
        .unwrap();
    let self_ty = m.types.intern(Ty::Ref(driver_ty, Provenance::UniqueBorrow));
    let wrapper_sig = m.sigs.intern(FnSig {
        inputs: vec![self_ty],
        output: block_ref,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let mut wbody = Body::default();
    wbody.local_decls.push(LocalDecl {
        ty: block_ref,
        mutability: Mutability::Mut,
        span: span(),
        debug_name: None,
    });
    wbody.local_decls.push(LocalDecl {
        ty: self_ty,
        mutability: Mutability::Not,
        span: span(),
        debug_name: None,
    });
    wbody.arg_count = 1;
    let arg_tmp = wbody.push_synthetic_local(mmio_ref);
    wbody.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(arg_tmp),
                Rvalue::Ref(
                    Provenance::SharedBorrow,
                    Place {
                        local: LocalId::from_u32(1),
                        projection: Box::new([
                            PlaceElem::Deref,
                            PlaceElem::Field(FieldIdx::from_u32(0), mmio_ty),
                        ]),
                    },
                ),
            ),
            span: span(),
        }]),
        terminator: Terminator {
            kind: TerminatorKind::Call {
                callee: regseq_model::CallCallee::Direct(deref_id),
                args: Box::new([Operand::Move(Place::from(arg_tmp))]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: Some(BlockId::from_u32(1)),
            },
            span: span(),
        },
    });
    wbody.basic_blocks.push(BasicBlock {
        statements: Box::new([]),
        terminator: Terminator {
            kind: TerminatorKind::Return,
            span: span(),
        },
    });
    let wrapper_item = item(&mut m, ItemKind::Function, "get_regs");
    let wrapper_emission = m.strings.intern("get_regs");
    let wrapper_id = m.instances.push(Instance {
        item: wrapper_item,
        generic_args: regseq_model::GenericArgs::new(),
        emission_name: wrapper_emission,
        span: span(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: wrapper_sig,
            body: Some(wbody),
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    // Wire the wrapper in as on_deinit so it joins the lowered closure.
    let wrapper_ref = m.instance_ref_of(wrapper_id);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_deinit = Some(wrapper_ref);

    let err = emit_linux_c(&m).unwrap_err();
    assert!(
        matches!(&err, EmitError::Unsupported(s) if s.contains("un-lowered instance") || s.contains("elided accessor result")),
        "returning an accessor result must fail loud, never emit uninitialized C: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// backend-linux-c-e1000 T04: the PCI dispatch arm + scaffold — ordering
// landmarks, the T03-pinned managed preamble (ledger C-7), adapter polarity,
// and one negative test per new diagnostic (>1 region B-5, resolved-no-BAR).
// ---------------------------------------------------------------------------

/// The minimal emitting PCI model: fieldless PCI driver, `on_init` = `Ok(())`.
fn minimal_pci_model() -> RegseqModel {
    let mut m = driver_model(Some(BusType::Pci), pci_match(), LifecycleOps::default());
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

/// Like [`model_with_callback`], on the PCI arm.
fn pci_model_with_callback(cb: &str) -> RegseqModel {
    let mut m = driver_model(Some(BusType::Pci), pci_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let on_init = push_ok_on_init(&mut m, driver_ty, result_ty);
    let extra = push_ok_on_init(&mut m, driver_ty, result_ty);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(on_init);
    match cb {
        "on_surprise_remove" => driver.ops.on_surprise_remove = Some(extra),
        "on_init_failure" => driver.ops.on_init_failure = Some(extra),
        "on_system_shutdown" => driver.ops.on_system_shutdown = Some(extra),
        _ => unreachable!(),
    }
    m
}

#[test]
fn minimal_pci_model_emits_the_artifact_set() {
    let emit = emit_linux_c(&minimal_pci_model()).expect("emit");
    assert_eq!(emit.module_name, "minimal");
    assert_eq!(emit.kbuild, "obj-m += minimal.o\n");
    assert!(!emit.tu.is_empty());
}

#[test]
fn pci_tu_carries_the_d04_ordering_landmarks() {
    let tu = emit_linux_c(&minimal_pci_model()).unwrap().tu;
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`:\n{tu}"))
    };
    let includes = pos("#include <linux/pci.h>");
    let container = pos("struct minimal_priv {");
    let pdev_member = pos("struct pci_dev *pdev;");
    let body = pos("static struct Result minimal_on_init(");
    let probe =
        pos("static int minimal_probe(struct pci_dev *pdev, const struct pci_device_id *id)");
    let remove = pos("static void minimal_remove(struct pci_dev *pdev)");
    let table = pos("static const struct pci_device_id minimal_ids[]");
    let devtable = pos("MODULE_DEVICE_TABLE(pci, minimal_ids);");
    let drvstruct = pos("static struct pci_driver minimal_driver = {");
    let macros = pos("module_pci_driver(minimal_driver);");
    let license = pos("MODULE_LICENSE(\"GPL\");");
    assert!(includes < container, "includes precede driver state");
    assert!(container < pdev_member && pdev_member < body);
    assert!(body < probe, "lowered bodies precede callbacks");
    assert!(probe < remove);
    assert!(remove < table, "callbacks precede the match table");
    assert!(table < devtable);
    assert!(
        devtable < drvstruct,
        "match table precedes the driver struct"
    );
    assert!(drvstruct < macros && macros < license);
    // The PCI TU carries no AMBA scaffolding.
    assert!(!tu.contains("amba"), "no AMBA residue in a PCI TU:\n{tu}");
    assert!(tu.contains("(PCI match)"), "the header names the PCI match");
}

#[test]
fn pci_probe_carries_create_associate_and_the_negated_adapter() {
    let tu = emit_linux_c(&minimal_pci_model()).unwrap().tu;
    // create → associate (canonical order) over the PCI device.
    let create = tu
        .find("devm_kzalloc(&pdev->dev, sizeof(*priv), GFP_KERNEL)")
        .unwrap();
    let associate = tu.find("pci_set_drvdata(pdev, priv);").unwrap();
    let call = tu.find("minimal_on_init(&priv->inner)").unwrap();
    assert!(create < associate && associate < call);
    // The adapter: Ok tag compare + per-variant NEGATED errno arms +
    // corrupt-discriminant default (shared with the AMBA arm).
    assert!(tu.contains("if (__tag != 0x2)"));
    assert!(tu.contains("return -EIO;"), "HardwareError -> -EIO");
    assert!(tu.contains("return -EBUSY;"), "DeviceBusy -> -EBUSY");
    assert!(tu.contains("default:"), "corrupt-discriminant defense");
    assert!(!tu.contains("int main("));
}

#[test]
fn pci_resolved_region_probe_carries_the_t03_pinned_preamble() {
    let tu = emit_linux_c(&pci_mmio_write_model(true, Some(0)))
        .unwrap()
        .tu;
    // Canonical order: create -> associate -> enable -> fused request+map ->
    // on_init, with the native status declared up front.
    assert!(tu.contains("\tint ret;\n"), "native status declared:\n{tu}");
    let create = tu.find("devm_kzalloc").unwrap();
    let associate = tu.find("pci_set_drvdata(pdev, priv);").unwrap();
    let enable = tu
        .find("ret = pcim_enable_device(pdev);")
        .expect("the T03-pinned managed enable");
    let enable_check = tu
        .find("if (ret)\n\t\treturn ret;")
        .expect("native ret channel");
    let map = tu
        .find("priv->inner.regs = pcim_iomap_region(pdev, 0, \"mmiodrv\");")
        .expect("the T03-pinned fused request+map of BAR0");
    let native = tu
        .find("return PTR_ERR(priv->inner.regs);")
        .expect("the native error channel propagates directly");
    let on_init = tu.find("mmiodrv_on_init(&priv->inner)").unwrap();
    assert!(create < associate && associate < enable);
    assert!(enable < enable_check && enable_check < map && map < native && native < on_init);
    // The pin REPLACES the unmanaged-era calls: no plain request/map, no AMBA
    // devm fusion call, and no explicit release ladder in .remove (A-2).
    assert!(!tu.contains("pci_request_selected_regions"));
    assert!(!tu.contains("pci_iomap("), "the unfused map never appears");
    assert!(!tu.contains("pci_ioremap_bar"));
    assert!(!tu.contains("devm_ioremap_resource"));
    assert!(!tu.contains("pci_release_region"));
    assert!(!tu.contains("pci_iounmap"));
    assert!(tu.contains("devres-managed resources release automatically"));
}

#[test]
fn pci_resolved_region_without_a_bar_fails_loud() {
    let err = emit_linux_c(&pci_mmio_write_model(true, None)).unwrap_err();
    assert!(
        matches!(&err, EmitError::Driver(s) if s.contains("BAR index")),
        "a resolved PCI region with no effective BAR must fail loud: {err:?}"
    );
}

#[test]
fn pci_unresolved_binding_is_the_a10_hard_error() {
    let err = emit_linux_c(&pci_mmio_write_model(false, Some(0))).unwrap_err();
    let EmitError::UnresolvedBinding(msg) = err else {
        panic!("expected the A-10 UnresolvedBinding error, got {err:?}");
    };
    assert!(
        msg.contains("[mmio.regs]") && msg.contains("base/size"),
        "the A-10 error must name the missing L2 literals: {msg}"
    );
}

#[test]
fn pci_more_than_one_region_fails_loud_naming_b5() {
    let mut m = minimal_pci_model();
    let regs = m.strings.intern("regs");
    let regs2 = m.strings.intern("regs2");
    let field = |name| regseq_model::ResourceField {
        name,
        kind: regseq_model::ResourceKind::Mmio,
        binding: regseq_model::MmioBinding {
            id: None,
            bar_index: None,
            bar_type: regseq_model::BarType::Mem,
            effective_bar: Some(0),
            base: Some(0xf000_0000),
            size: Some(0x1000),
        },
    };
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.handle.resource_fields = vec![field(regs), field(regs2)];
    let err = emit_linux_c(&m).unwrap_err();
    assert!(
        matches!(&err, EmitError::Unsupported(s) if s.contains("B-5")),
        ">1 MMIO region must fail loud naming B-5: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// backend-linux-c-e1000 T05: pci_device_id[] match-table emission — the FULL
// match_set (B-6 never-truncate), zero sentinel, MODULE_DEVICE_TABLE, and the
// polarity pin: the lift covers `Pci` entries under a PCI driver ONLY.
// ---------------------------------------------------------------------------

#[test]
fn pci_full_match_set_renders_in_order_with_zero_sentinel() {
    let mut m = minimal_pci_model();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.match_set.push(MatchEntry::Pci {
        vendor: 0x8086,
        device: 0x100f,
        subsystem: None,
        class: None,
    });
    let tu = emit_linux_c(&m).unwrap().tu;
    let first = tu.find(".device\t\t= 0x100e,").expect("first row");
    let second = tu
        .find(".device\t\t= 0x100f,")
        .expect("second row (FULL match_set, B-6)");
    let sentinel = tu.find("{ 0, },").expect("zero sentinel");
    let devtable = tu.find("MODULE_DEVICE_TABLE(pci, minimal_ids);").unwrap();
    assert!(first < second && second < sentinel && sentinel < devtable);
    assert_eq!(
        tu.matches(".vendor\t\t= 0x8086,").count(),
        2,
        "every row carries its vendor"
    );
    assert_eq!(
        tu.matches(".subvendor\t= PCI_ANY_ID,").count(),
        2,
        "vendor/device rows wildcard the subsystem ids"
    );
}

#[test]
fn foreign_amba_entry_under_pci_is_audited_not_tabled() {
    // C06 applied uniformly to the PCI scaffold (D08: nativeness is per
    // ratified registration form — linux_c+pci consumes Pci entries; an
    // Amba entry is foreign HERE even though linux_c's amba scaffold would
    // consume it). The former loud reject naming this unit is lifted by
    // this unit; the audit replaces it.
    let mut m = minimal_pci_model();
    {
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set.push(MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        });
    }
    let emit = emit_linux_c(&m).expect("mixed set must emit (C06)");
    let tu = &emit.tu;
    let block = tu
        .find("/* RSEQ-IGNORED-MATCH: amba id=0x00041011 mask=0x000fffff")
        .unwrap_or_else(|| panic!("audit block present:\n{tu}"));
    let table = tu
        .find("static const struct pci_device_id")
        .expect("pci table");
    assert!(block < table, "audit block precedes the table");
    assert!(tu.contains("not consumed by the ratified pci_driver registration form"));
    // A2: the foreign entry is absent from the emitted table.
    let table_body = &tu[table..tu.find("{ 0, },").expect("sentinel")];
    assert!(
        !table_body.contains("0x00041011"),
        "foreign entry absent from the emitted pci table (A2)"
    );
    assert_eq!(emit.conformance.ignored_match_entries.len(), 1);
    let audit = &emit.conformance.ignored_match_entries[0];
    assert_eq!(audit.entry, "amba id=0x00041011 mask=0x000fffff");
    assert!(tu.contains(&audit.c_block()), "same-struct render (I03)");
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
    // even when a native entry exists, never audited-and-ignored.
    let mut m = minimal_amba_model();
    {
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set.push(MatchEntry::Usb {
            vendor: 0x1234,
            product: 0x5678,
        });
    }
    let err = emit_linux_c(&m).unwrap_err();
    let EmitError::Driver(msg) = err else {
        panic!("expected Driver error, got {err:?}");
    };
    assert!(
        msg.contains("no ratified registration form") && msg.contains("fail-loud"),
        "the deferred-family rejection names the rule boundary: {msg}"
    );
}

#[test]
fn all_foreign_match_set_under_pci_is_rejected_loud() {
    // The PCI-scaffold analog of the A-13 all-foreign reject.
    let mut m = minimal_pci_model();
    {
        let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set = vec![MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        }];
    }
    let err = emit_linux_c(&m).unwrap_err();
    let EmitError::Driver(msg) = err else {
        panic!("expected Driver error, got {err:?}");
    };
    assert!(
        msg.contains("no native entries")
            && msg.contains("pci_driver")
            && msg.contains("pci = [{ vendor, device }]"),
        "the all-foreign rejection names the rule and the fix: {msg}"
    );
}

#[test]
fn pci_match_entry_with_qualifiers_fails_loud() {
    let mut m = minimal_pci_model();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.descriptor.match_set[0] = MatchEntry::Pci {
        vendor: 0x8086,
        device: 0x100e,
        subsystem: Some(0x1028_0001),
        class: None,
    };
    let err = emit_linux_c(&m).unwrap_err();
    assert!(
        matches!(&err, EmitError::Unsupported(s) if s.contains("subsystem/class")),
        "qualified rows have no witness this unit and must fail loud: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// backend-linux-c-e1000 T06: the surprise-remove present-predicate `.remove`
// (GRILL D06(3)) — both polarities; the AMBA rejection unchanged.
// ---------------------------------------------------------------------------

#[test]
fn pci_surprise_remove_emits_the_present_predicate_branch() {
    // Overridden ⇒ ONE void .remove branching on pci_device_is_present:
    // orderly arm = on_deinit... wired in via a second callback so both arms
    // carry real calls.
    let mut m = pci_model_with_callback("on_surprise_remove");
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let on_deinit = push_ok_on_init(&mut m, driver_ty, result_ty);
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_deinit = Some(on_deinit);
    let tu = emit_linux_c(&m).unwrap().tu;
    let remove = tu
        .find("static void minimal_remove(struct pci_dev *pdev)")
        .unwrap();
    let predicate = tu
        .find("if (pci_device_is_present(pdev)) {")
        .expect("the ratified present predicate");
    let orderly = tu
        .find("(void)minimal_on_deinit(&priv->inner);")
        .expect("orderly arm runs on_deinit");
    let surprise = tu
        .find("(void)minimal_on_surprise_remove(&priv->inner);")
        .expect("surprise arm runs on_surprise_remove");
    assert!(remove < predicate && predicate < orderly && orderly < surprise);
    // The surprise arm runs no MMIO-touching releases — and the remove emits
    // no release ladder at all (A-2 devres).
    assert!(!tu.contains("pci_iounmap"));
    assert!(!tu.contains("pci_release_region"));
}

#[test]
fn pci_remove_without_surprise_override_is_plain_orderly() {
    let tu = emit_linux_c(&minimal_pci_model()).unwrap().tu;
    assert!(
        !tu.contains("pci_device_is_present"),
        "no predicate without the override:\n{tu}"
    );
}

// ---------------------------------------------------------------------------
// os-capability-disposition T04: the per-target conformance report (C05) —
// every authored slot delivered in LifecycleOps declaration order; the PCI
// surprise delivery carries its ratified row ID (D16(b)); rejected empty.
// ---------------------------------------------------------------------------

#[test]
fn pci_conformance_report_delivers_all_authored_slots_with_the_surprise_row() {
    let mut m = driver_model(Some(BusType::Pci), pci_match(), LifecycleOps::default());
    let driver_ty = m
        .types
        .iter()
        .find(|(_, t)| matches!(t, Ty::Struct { .. }))
        .map(|(id, _)| id)
        .unwrap();
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let cbs: Vec<_> = (0..5)
        .map(|_| push_ok_on_init(&mut m, driver_ty, result_ty))
        .collect();
    let (_, driver) = m.device_lifecycle.drivers.iter_mut().next().unwrap();
    driver.ops.on_init = Some(cbs[0]);
    driver.ops.on_deinit = Some(cbs[1]);
    driver.ops.on_surprise_remove = Some(cbs[2]);
    driver.ops.on_init_failure = Some(cbs[3]);
    driver.ops.on_system_shutdown = Some(cbs[4]);
    let emit = emit_linux_c(&m).expect("must emit");
    let report = &emit.conformance;
    assert_eq!(report.target, "linux_c");
    assert_eq!(report.os_version_pin, capability::LINUX_PIN);
    // Declaration order (the LifecycleOps field order).
    let slots: Vec<&str> = report
        .delivered_slots
        .iter()
        .map(|e| e.slot.as_str())
        .collect();
    assert_eq!(
        slots,
        vec![
            "on_init",
            "on_deinit",
            "on_surprise_remove",
            "on_init_failure",
            "on_system_shutdown"
        ]
    );
    // D16(b): row ID iff a ratified row exists — only the PCI surprise
    // delivery is table-ratified.
    for e in &report.delivered_slots {
        if e.slot == "on_surprise_remove" {
            assert_eq!(
                e.row.as_deref(),
                Some(capability::CAP_LINUX_PCI_SURPRISE_1.id)
            );
        } else {
            assert_eq!(e.row, None, "slot {}", e.slot);
        }
        assert_eq!(e.citation, None, "delivered entries carry no citation");
    }
    assert!(report.absent_dispatch_slots.is_empty());
    assert!(report.rejected_slots.is_empty(), "I07");
    assert!(
        report
            .summary_line()
            .contains("on_surprise_remove:CAP-LINUX-PCI-SURPRISE-1"),
        "{}",
        report.summary_line()
    );
}

#[test]
fn amba_conformance_report_carries_no_row_ids() {
    // The AMBA arm has no table-ratified deliveries: every delivered entry
    // is row None; the AMBA surprise rejection itself is untouched (I01,
    // asserted by amba_surprise_remove_fails_loud above).
    let emit = emit_linux_c(&model_with_callback("on_init_failure")).expect("must emit");
    let report = &emit.conformance;
    assert_eq!(report.target, "linux_c");
    let slots: Vec<(&str, Option<&str>)> = report
        .delivered_slots
        .iter()
        .map(|e| (e.slot.as_str(), e.row.as_deref()))
        .collect();
    assert_eq!(slots, vec![("on_init", None), ("on_init_failure", None)]);
    assert!(report.absent_dispatch_slots.is_empty());
    assert!(report.rejected_slots.is_empty());
}

// ---------------------------------------------------------------------------
// backend-linux-c-e1000 T07: config-space activation per C01 — the TU
// interception path: legal sites lower with the per-site container recovery;
// non-PCI TUs and non-driver-receiver bodies are distinct loud diagnostics;
// sibling ops each carry their own block (the A-3 preservation property).
// ---------------------------------------------------------------------------

/// A driver model whose `on_init` performs the given config ops in sequence
/// (`Read` = u16 @0x6 into a local; `Write` = u8 0x10 @0xc), through
/// intrinsic-anchored `read_config`/`write_config` callees. `bus`/`match_set`
/// parametrize the legality tests; `receiver_is_driver` controls C01 (b).
fn config_model(
    ops: &[regseq_model::ConfigOp],
    bus: BusType,
    match_set: Vec<MatchEntry>,
    receiver_is_driver: bool,
) -> RegseqModel {
    use regseq_model::{BlockId, ConfigOp, LocalId};
    let mut m = model();
    let driver_ty = push_struct(&mut m, "CfgDrv", struct_layout(0, 1, vec![]));
    let driver_item = match &m.types[driver_ty] {
        Ty::Struct { item, .. } => *item,
        _ => unreachable!(),
    };
    // A decoy receiver type for the C01 (b) negative.
    let other_ty = push_struct(&mut m, "NotTheDriver", struct_layout(0, 1, vec![]));

    let u8t = m.types.intern(Ty::Uint(UintKind::U8));
    let u16t = m.types.intern(Ty::Uint(UintKind::U16));
    let u32t = m.types.intern(Ty::Uint(UintKind::U32));
    let unit = m.types.intern(Ty::Tuple {
        fields: vec![],
        layout: struct_layout(0, 1, vec![]),
    });

    // The intrinsic-anchored config callees.
    let anchor =
        |m: &mut RegseqModel, name: &str, op: ConfigOp, sig_in: Vec<TyId>, sig_out: TyId| {
            let sig = m.sigs.intern(FnSig {
                inputs: sig_in,
                output: sig_out,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Safe,
            });
            let it = item(m, ItemKind::Function, name);
            while m.domain_attrs.item_attrs.len() <= it.as_u32() as usize {
                m.domain_attrs.item_attrs.push(Vec::new());
            }
            m.domain_attrs.item_attrs[it].push(regseq_model::AttrData::Intrinsic(
                regseq_model::IntrinsicKind::Config(op),
            ));
            let emission = m.strings.intern(name);
            m.instances.push(Instance {
                item: it,
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
        };
    let read_id = anchor(&mut m, "read_config", ConfigOp::Read, vec![u32t], u16t);
    let write_id = anchor(
        &mut m,
        "write_config",
        ConfigOp::Write,
        vec![u32t, u8t],
        unit,
    );

    // on_init(&mut <recv>) -> Result, performing the ops in sequence.
    let (result_ty, _) = result_carrier(&mut m, ["HardwareError", "DeviceBusy"]);
    let recv_pointee = if receiver_is_driver {
        driver_ty
    } else {
        other_ty
    };
    let self_ty = m
        .types
        .intern(Ty::Ref(recv_pointee, Provenance::UniqueBorrow));
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
    body.arg_count = 1;
    let off_read = m.consts.intern(Const::Uint(UintKind::U32, 0x6));
    let off_write = m.consts.intern(Const::Uint(UintKind::U32, 0xc));
    let val_write = m.consts.intern(Const::Uint(UintKind::U8, 0x10));
    let unit_const = m.consts.intern(Const::ZST);
    for (i, op) in ops.iter().enumerate() {
        let next = BlockId::from_u32(i as u32 + 1);
        let term = match op {
            ConfigOp::Read => {
                let dst = body.push_synthetic_local(u16t);
                TerminatorKind::Call {
                    callee: regseq_model::CallCallee::Direct(read_id),
                    args: Box::new([Operand::Constant(off_read)]),
                    destination: Place::from(dst),
                    target: Some(next),
                }
            }
            ConfigOp::Write => {
                let dst = body.push_synthetic_local(unit);
                TerminatorKind::Call {
                    callee: regseq_model::CallCallee::Direct(write_id),
                    args: Box::new([Operand::Constant(off_write), Operand::Constant(val_write)]),
                    destination: Place::from(dst),
                    target: Some(next),
                }
            }
        };
        body.basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: term,
                span: span(),
            },
        });
    }
    body.basic_blocks.push(BasicBlock {
        statements: Box::new([Statement {
            kind: StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
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
    let on_init_item = item(&mut m, ItemKind::Function, "on_init");
    let emission = m.strings.intern("on_init");
    let on_init_id = m.instances.push(Instance {
        item: on_init_item,
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
    let on_init_ref = m.instance_ref_of(on_init_id);

    let name = m.strings.intern("cfgdrv");
    m.device_lifecycle.drivers.insert(
        driver_item,
        regseq_model::DriverLifecycle {
            descriptor: regseq_model::DriverDescriptor {
                name,
                match_set,
                device_class: regseq_model::ClassTag::Generic,
                bus_type: Some(bus),
                dma_present: false,
            },
            handle: regseq_model::DriverHandle::default(),
            ops: regseq_model::LifecycleOps {
                on_init: Some(on_init_ref),
                ..Default::default()
            },
        },
    );
    m
}

#[test]
fn config_ops_lower_with_the_per_site_container_recovery() {
    use regseq_model::ConfigOp;
    let m = config_model(
        &[ConfigOp::Read, ConfigOp::Write],
        BusType::Pci,
        pci_match(),
        true,
    );
    let tu = emit_linux_c(&m).unwrap().tu;
    // The C01 read shape: recovery + carrier + (void) accessor + marshal,
    // one braces block.
    assert!(
        tu.contains(
            "\t{\n\t\tstruct cfgdrv_priv *__rseq_priv = container_of(self, struct cfgdrv_priv, inner);\n\t\tu16 __cv;\n\t\t(void)pci_read_config_word(__rseq_priv->pdev, (int)(0x6u), &__cv);\n"
        ),
        "the C01 read shape with per-site recovery; TU:\n{tu}"
    );
    // The C01 write shape: recovery + (void) accessor, its own braces block.
    assert!(
        tu.contains(
            "\t{\n\t\tstruct cfgdrv_priv *__rseq_priv = container_of(self, struct cfgdrv_priv, inner);\n\t\t(void)pci_write_config_byte(__rseq_priv->pdev, (int)(0xcu), (u8)(0x10u));\n\t}\n"
        ),
        "the C01 bracketed write shape with per-site recovery; TU:\n{tu}"
    );
    // The includes ride the helper-usage mechanism.
    assert!(tu.contains("#include <linux/container_of.h>"));
    assert!(tu.contains("#include <linux/pci.h>"));
}

#[test]
fn sibling_config_ops_each_carry_their_own_recovery_block() {
    use regseq_model::ConfigOp;
    let m = config_model(
        &[ConfigOp::Read, ConfigOp::Read],
        BusType::Pci,
        pci_match(),
        true,
    );
    let tu = emit_linux_c(&m).unwrap().tu;
    // Strictly per-site, never hoisted (the A-3 preservation property): TWO
    // sibling ops -> TWO recovery declarations, each inside its own block;
    // no function-scope device pointer exists.
    assert_eq!(
        tu.matches("__rseq_priv = container_of(self, struct cfgdrv_priv, inner);")
            .count(),
        2,
        "each sibling op carries its own per-site recovery; TU:\n{tu}"
    );
    assert_eq!(
        tu.matches("\t{\n\t\tstruct cfgdrv_priv *__rseq_priv")
            .count(),
        2,
        "every recovery lives inside its own braces block (never hoisted); TU:\n{tu}"
    );
}

#[test]
fn config_op_in_a_non_pci_tu_is_a_loud_diagnostic() {
    use regseq_model::ConfigOp;
    let m = config_model(&[ConfigOp::Read], BusType::Amba, amba_match(), true);
    let err = emit_linux_c(&m).unwrap_err();
    assert!(
        matches!(&err, EmitError::Unsupported(s) if s.contains("non-PCI")),
        "config in a non-PCI TU must fail loud naming the restriction: {err:?}"
    );
}

#[test]
fn config_op_with_a_non_driver_receiver_is_a_loud_diagnostic() {
    use regseq_model::ConfigOp;
    let m = config_model(&[ConfigOp::Read], BusType::Pci, pci_match(), false);
    let err = emit_linux_c(&m).unwrap_err();
    assert!(
        matches!(&err, EmitError::Unsupported(s) if s.contains("receiver is not the driver type")),
        "config outside a driver-receiver body must fail loud: {err:?}"
    );
}

#[test]
fn amba_surprise_remove_rejection_is_unchanged() {
    // The AMBA arm still has no ratified present predicate (T06 delivers the
    // PCI arm only).
    let err = emit_linux_c(&model_with_callback("on_surprise_remove")).unwrap_err();
    assert!(
        matches!(err, EmitError::Unsupported(ref s) if s.contains("on_surprise_remove") && s.contains("AMBA")),
        "got {err:?}"
    );
}

#[test]
fn pci_on_init_failure_hook_runs_on_the_authored_err_arm_only() {
    let tu = emit_linux_c(&pci_model_with_callback("on_init_failure"))
        .unwrap()
        .tu;
    let hook = tu
        .find("(void)minimal_on_init_failure(&priv->inner);")
        .expect("the failure hook is emitted");
    let err_branch = tu.find("if (__tag != 0x2)").expect("the Err branch");
    let switch = tu.find("switch (__err)").expect("the errno switch");
    assert!(
        err_branch < hook && hook < switch,
        "the hook must run inside the authored Err arm, before the errno return"
    );
}

#[test]
fn pci_shutdown_slot_is_omit_when_absent() {
    let with = emit_linux_c(&pci_model_with_callback("on_system_shutdown"))
        .unwrap()
        .tu;
    assert!(with.contains("static void minimal_shutdown(struct pci_dev *pdev)"));
    assert!(with.contains(".shutdown\t= minimal_shutdown,"));
    assert!(with.contains("(void)minimal_on_system_shutdown(&priv->inner);"));

    let without = emit_linux_c(&minimal_pci_model()).unwrap().tu;
    assert!(
        !without.contains(".shutdown"),
        "the .shutdown initializer must be omitted when on_system_shutdown is not overridden"
    );
}

#[test]
fn delay_helpers_select_linux_bands_and_round_up() {
    // Timed-cpu-delay-and-sleep (TCD construct, M2/T08): the busy_wait helper
    // selects the atomic-safe *delay family by runtime magnitude
    // (ndelay <1us / udelay <1ms / mdelay >=1ms) and rounds every conversion
    // UP (minimum-wait, R06); sleep is msleep-only (D09).
    let h = super::body::delay_helpers(true, true);
    assert!(h.contains("static void regseq_linux_busy_wait_ns(uint64_t ns)"));
    // The three magnitude bands at the spec-pinned boundaries.
    assert!(h.contains("ns < 1000ULL"), "ndelay band cutoff: {h}");
    assert!(h.contains("ndelay((unsigned long)ns)"), "ndelay arm: {h}");
    assert!(h.contains("ns < 1000000ULL"), "udelay band cutoff: {h}");
    // Overflow-safe round-UP ceilings (the naive `ns + scale-1` overflows near
    // Duration::MAX = u64::MAX).
    assert!(
        h.contains("udelay((unsigned long)(1ULL + (ns - 1ULL) / 1000ULL))"),
        "udelay round-up: {h}"
    );
    assert!(
        h.contains("mdelay((unsigned long)(1ULL + (ns - 1ULL) / 1000000ULL))"),
        "mdelay round-up: {h}"
    );
    // sleep: msleep-only, round up to ms, ns==0 guard.
    assert!(h.contains("static void regseq_linux_sleep_ns(uint64_t ns)"));
    assert!(
        h.contains("msleep((unsigned int)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000000ULL))"),
        "msleep round-up: {h}"
    );
    // Demand-precise: each helper is emitted only when its kind is present.
    let busy_only = super::body::delay_helpers(true, false);
    assert!(busy_only.contains("busy_wait_ns") && !busy_only.contains("sleep_ns"));
    let sleep_only = super::body::delay_helpers(false, true);
    assert!(sleep_only.contains("sleep_ns") && !sleep_only.contains("busy_wait_ns"));
}
