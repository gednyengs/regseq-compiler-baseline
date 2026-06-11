//! The linux_c composed-E1000 L0 gate (backend-linux-c-e1000 T09, C3 close).
//!
//! The five-construct + config-ops `e1000-composed` fixture — the
//! device-complexity keystone — through the C02 linux row: bridge → L2 bind
//! → `emit_linux_c` → byte-exact `golden.linux.c`, plus the L0 STRUCTURE
//! suite enumerated by C02 item 1, named so a regression is diagnosed, not
//! just byte-diffed:
//!   - the PCI scaffold shape (probe ladder + the T03-pinned managed
//!     preamble, ledger C-7),
//!   - the ≥2-row pci_device_id table + zero sentinel + MODULE_DEVICE_TABLE,
//!   - the foreign-transport match-entry rejection under a PCI driver,
//!   - the per-site container-recovery expression (C01),
//!   - the driver-receiver-restriction rejection,
//!   - the non-PCI config rejection,
//!   - the >1-region rejection naming B-5,
//!   - the surprise-arm present predicate (D06(3)),
//! plus the carried I02 gates (A-10 negative leg, A-12 polarity, A-3
//! placement).
//!
//! `BLESS=1` writes `golden.linux.c` from a single clean bridge+emit — for
//! THIS binary's fixture only (I04). SOLE test in this binary (interner
//! isolation, the established sole-binary pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use regseq_model::{BusType, MatchEntry, Provenance, Ty, UintKind};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn linux_e1000_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "e1000-composed")
        .expect("e1000-composed fixture discovered");

    if std::env::var_os("BLESS").is_some() {
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let emit = regseq_backends::emit_linux_c(&model).expect("emit_linux_c must succeed");
        std::fs::write(pipeline::linux_golden_path(fixture), &emit.tu)
            .expect("write golden.linux.c");
        std::fs::write(
            pipeline::linux_conformance_path(fixture),
            codegen_coverage_runner::conformance::render_json(&emit.conformance),
        )
        .expect("write conformance.linux.json");
        return;
    }

    // The C02 linux row (stage-attributed), byte-locking the snapshot.
    let result = pipeline::run_linux_fixture(fixture);
    assert!(
        result.is_green(),
        "e1000-composed must pass bridge -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::linux_golden_path(fixture))
        .expect("read golden.linux.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // -- the conformance JSON (amendment 6): the e1000 surprise delivery
    //    cites its ratified row — the same SLOT the zephyr artifacts cite
    //    absent, each tied to its own OS-specific ratified row (I03: every
    //    artifact renders its row ID from the one table const).
    let conf = std::fs::read_to_string(pipeline::linux_conformance_path(fixture))
        .expect("read conformance.linux.json");
    assert!(
        conf.contains("{\"slot\": \"on_surprise_remove\", \"row\": \"CAP-LINUX-PCI-SURPRISE-1\"}"),
        "{conf}"
    );

    // -- the PCI scaffold shape: probe ladder in canonical order with the
    //    T03-pinned managed preamble (ledger C-7) and the native channel.
    let create = pos("devm_kzalloc(&pdev->dev, sizeof(*priv), GFP_KERNEL)");
    let associate = pos("pci_set_drvdata(pdev, priv);");
    let enable = pos("ret = pcim_enable_device(pdev);");
    let map = pos("priv->inner.regs = pcim_iomap_region(pdev, 0, \"e1000\");");
    let native = pos("return PTR_ERR(priv->inner.regs);");
    let on_init_call = pos("e1000_on_init(&priv->inner)");
    assert!(
        create < associate && associate < enable && enable < map && map < native
            && native < on_init_call,
        "probe ladder must run create -> associate -> pcim enable -> fused \
         request+map (native channel) -> on_init"
    );
    // The pin REPLACES the unmanaged-era calls; no release ladder exists.
    assert!(!tu.contains("pci_request_selected_regions"));
    assert!(!tu.contains("pci_ioremap_bar"));
    assert!(!tu.contains("pci_release_region") && !tu.contains("pci_iounmap"));
    pos("devres-managed resources release automatically");

    // -- the errno adapter at the probe return, with the on_init_failure
    //    hook INSIDE the authored-Err arm before the negated-errno switch.
    let adapter = pos("if (__tag != 0x2)");
    let hook = pos("(void)e1000_on_init_failure(&priv->inner);");
    let errno_switch = pos("switch (__err)");
    assert!(on_init_call < adapter && adapter < hook && hook < errno_switch);
    pos("return -EIO;");
    pos("return -EBUSY;");

    // -- the surprise-arm present predicate (D06(3)): one void .remove
    //    branching on pci_device_is_present; orderly arm on_deinit, surprise
    //    arm on_surprise_remove.
    let remove = pos("static void e1000_remove(struct pci_dev *pdev)");
    let predicate = pos("if (pci_device_is_present(pdev)) {");
    let orderly = pos("(void)e1000_on_deinit(&priv->inner);");
    let surprise = pos("(void)e1000_on_surprise_remove(&priv->inner);");
    assert!(remove < predicate && predicate < orderly && orderly < surprise);
    // on_system_shutdown is NOT overridden: the slot must be omitted.
    assert!(
        !tu.contains(".shutdown"),
        "omit-when-absent .shutdown must not appear"
    );

    // -- the per-site container-recovery expression (C01): every config op
    //    carries its own braces block with its own recovery — 4 reads + 1
    //    write = 5 sites, never hoisted.
    assert_eq!(
        tu.matches("struct e1000_priv *__rseq_priv = container_of(_1, struct e1000_priv, inner);")
            .count(),
        5,
        "five config sites, each with its own per-site recovery"
    );
    pos("(void)pci_read_config_word(__rseq_priv->pdev, (int)(0x0u), &__cv);");
    pos("(void)pci_read_config_word(__rseq_priv->pdev, (int)(0x2u), &__cv);");
    pos("(void)pci_read_config_byte(__rseq_priv->pdev, (int)(0x8u), &__cv);");
    pos("(void)pci_read_config_dword(__rseq_priv->pdev, (int)(0x10u), &__cv);");
    pos("(void)pci_write_config_byte(__rseq_priv->pdev, (int)(0xcu), (u8)(0x10u));");
    pos("#include <linux/container_of.h>");

    // -- register access: width-switched accessors over the iomem cookie,
    //    the const bank from the recovered block layout, and the runtime
    //    register-array index terms (base + i*stride).
    pos("#define E1000_CTRL 0x000");
    pos("#define E1000_STATUS 0x008");
    pos("#define E1000_ICR 0x0c0");
    pos("#define E1000_IMC 0x0d8");
    pos("#define E1000_TCTL 0x400");
    pos("#define E1000_TDBAL 0x3800");
    pos("#define E1000_TDBAH 0x3804");
    pos("#define E1000_TDLEN 0x3808");
    pos("#define E1000_TDT 0x3818");
    pos("#define E1000_MTA 0x5200");
    pos("#define E1000_RA 0x5400");
    pos("#define E1000_VFTA 0x5600");
    assert!(tu.contains("readl(") && tu.contains("writel("));
    assert!(!tu.contains("readq(") && !tu.contains("writeq("));
    assert!(
        !tu.contains("CONFIG_64BIT"),
        "a width-4-only TU must not carry the A-12 guard"
    );
    // Runtime indices render as MIR temps; the structure fact is the
    // `base-macro + (index) * stride` form over the iomem cookie (the
    // array-of-struct rah lands at RA+4, a non-macro static offset).
    for needle in [
        "_1->regs + E1000_MTA + (",
        ") * 4);",
        "_1->regs + E1000_VFTA + (",
        "_1->regs + E1000_RA + (",
        ") * 8);",
        "_1->regs + 0x5404 + (",
    ] {
        pos(needle);
    }

    // -- A-3 placement: lowered-body logs are pr_* under the pr_fmt module
    //    prefix; no dev_* anywhere.
    let prfmt = pos("#define pr_fmt(fmt) KBUILD_MODNAME \": \" fmt");
    assert!(prfmt < tu.find("#include").unwrap());
    pos("pr_debug(\"e1000 probe start\\n\");");
    pos("pr_info(\"e1000 up\\n\");");
    pos("pr_warn(\"e1000 deinit: masking interrupts\\n\");");
    pos("pr_err(");
    assert!(
        !tu.contains("dev_dbg(") && !tu.contains("dev_info(") && !tu.contains("dev_err("),
        "A-3: no dev_* in lowered bodies"
    );

    // -- module tail: FULL 2-row match table + sentinel + device table +
    //    module_pci_driver (B-6 never-truncate).
    let row1 = pos(".device\t\t= 0x100e,");
    let row2 = pos(".device\t\t= 0x100f,");
    let sentinel = pos("{ 0, },");
    let devtable = pos("MODULE_DEVICE_TABLE(pci, e1000_ids);");
    let drvstruct = pos("static struct pci_driver e1000_driver = {");
    let macros = pos("module_pci_driver(e1000_driver);");
    assert!(remove < row1 && row1 < row2 && row2 < sentinel && sentinel < devtable);
    assert!(devtable < drvstruct && drvstruct < macros);

    // ======================== negative structure legs ======================

    // -- A-10: stripping the L2 binding turns the SAME model into the hard
    //    unresolved-binding diagnostic naming the missing literals.
    let unbound = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    let err = regseq_backends::emit_linux_c(&unbound).expect_err("unresolved binding must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("A-10") && msg.contains("base/size"),
        "the A-10 diagnostic must name the missing L2 literals: {msg}"
    );

    // -- >1-region: a second resource field on the bound model fails loud
    //    naming B-5 (the model+oracle successor unit).
    let mut multi = bound_model(fixture);
    {
        let extra = multi.strings.intern("regs2");
        let (_, driver) = multi.device_lifecycle.drivers.iter_mut().next().unwrap();
        let mut second = driver.handle.resource_fields[0].clone();
        second.name = extra;
        driver.handle.resource_fields.push(second);
    }
    let msg = regseq_backends::emit_linux_c(&multi)
        .expect_err(">1 region must fail")
        .to_string();
    assert!(msg.contains("B-5"), "the >1-region rejection names B-5: {msg}");

    // -- foreign-transport match entry under a PCI driver: audited and
    //    filtered out of the table, never silently dropped (C06/C03/A2 —
    //    the spec-convergence-pl011 lift replaced the former loud reject).
    let mut foreign = bound_model(fixture);
    {
        let (_, driver) = foreign.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.match_set.push(MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        });
    }
    let emit = regseq_backends::emit_linux_c(&foreign).expect("mixed set must emit (C06)");
    assert!(
        emit.tu
            .contains("/* RSEQ-IGNORED-MATCH: amba id=0x00041011 mask=0x000fffff"),
        "the foreign entry is audited in the TU"
    );
    let table = emit
        .tu
        .find("static const struct pci_device_id")
        .expect("pci table");
    let table_body = &emit.tu[table..emit.tu.find("{ 0, },").expect("sentinel")];
    assert!(
        !table_body.contains("0x00041011"),
        "the foreign entry is absent from the emitted table (A2)"
    );
    assert_eq!(emit.conformance.ignored_match_entries.len(), 1);

    // -- non-PCI config rejection: the SAME bodies under an AMBA descriptor
    //    hit the C01 legality (a) diagnostic at the eclipsed config sites.
    let mut amba = bound_model(fixture);
    {
        let (_, driver) = amba.device_lifecycle.drivers.iter_mut().next().unwrap();
        driver.descriptor.bus_type = Some(BusType::Amba);
        driver.descriptor.match_set = vec![MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        }];
    }
    let msg = regseq_backends::emit_linux_c(&amba)
        .expect_err("config under a non-PCI driver must fail")
        .to_string();
    assert!(
        msg.contains("non-PCI"),
        "the non-PCI config rejection names the restriction: {msg}"
    );

    // -- driver-receiver restriction: rewriting on_init's receiver type away
    //    from the driver struct hits the C01 legality (b) diagnostic.
    let mut alien = bound_model(fixture);
    {
        let u32t = alien.types.intern(Ty::Uint(UintKind::U32));
        let alien_recv = alien.types.intern(Ty::Ref(u32t, Provenance::SharedBorrow));
        let on_init_ref = alien
            .device_lifecycle
            .drivers
            .values()
            .next()
            .unwrap()
            .ops
            .on_init
            .expect("on_init overridden");
        let id = (0..alien.instances.len())
            .map(|i| regseq_model::InstanceId::from_u32(i as u32))
            .find(|id| alien.instance_ref_of(*id) == on_init_ref)
            .expect("on_init instance");
        let regseq_model::InstanceKind::Function(data) = &mut alien.instances[id].kind else {
            panic!("on_init is a function");
        };
        let body = data.body.as_mut().expect("on_init has a body");
        body.local_decls[regseq_model::LocalId::from_u32(1)].ty = alien_recv;
    }
    let msg = regseq_backends::emit_linux_c(&alien)
        .expect_err("config outside a driver-receiver body must fail")
        .to_string();
    assert!(
        msg.contains("receiver is not the driver type"),
        "the receiver restriction names itself: {msg}"
    );
}

/// Bridge + bind the fixture (a fresh model for each mutation leg; these run
/// strictly AFTER the snapshot verdict, so the extra bridges cannot perturb
/// the interner history the snapshot depends on).
fn bound_model(fixture: &codegen_coverage_runner::manifest::Fixture) -> regseq_model::RegseqModel {
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    model
}
