//! The linux_c composed-PL011 L0 gate (backend-linux-c T10, C3 close).
//!
//! The five-construct `pl011-composed` fixture — THE first-light spec —
//! through the C02 linux row: bridge → L2 bind → `emit_linux_c` → byte-exact
//! `golden.linux.c`, plus the L0 STRUCTURE suite: named assertions from the
//! Linux traceability matrix and the four ratified gates (probe-ladder
//! order, A-12 guard polarity, A-10 diagnostic, A-3 placement, A-11 is
//! locked at the unit seam in `linux_c::mod_tests`), so a regression is
//! diagnosed by name, not just byte-diffed (R08).
//!
//! `BLESS=1` writes `golden.linux.c` from a single clean bridge+emit — for
//! THIS binary's fixture only (I04). SOLE test in this binary (interner
//! isolation, the established sole-binary pattern).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

#[test]
fn linux_pl011_snapshot_and_l0_structure() {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed")
        .expect("pl011-composed fixture discovered");

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
        "pl011-composed must pass bridge -> emit_linux_c -> snapshot: {:?}",
        result.verdict
    );

    let tu = std::fs::read_to_string(pipeline::linux_golden_path(fixture))
        .expect("read golden.linux.c");
    let pos = |needle: &str| {
        tu.find(needle)
            .unwrap_or_else(|| panic!("TU must contain `{needle}`"))
    };

    // -- probe-ladder order (matrix §C: create -> associate -> preamble ->
    //    on_init; the canonical order is load-bearing, integration doc §8.1).
    let create = pos("devm_kzalloc(&adev->dev, sizeof(*priv), GFP_KERNEL)");
    let associate = pos("amba_set_drvdata(adev, priv);");
    let map = pos("priv->inner.regs = devm_ioremap_resource(&adev->dev, &adev->res);");
    let native = pos("return PTR_ERR(priv->inner.regs);");
    let on_init_call = pos("pl011_on_init(&priv->inner)");
    assert!(
        create < associate && associate < map && map < native && native < on_init_call,
        "probe ladder must run create -> associate -> devm claim+map (native \
         channel) -> on_init"
    );

    // -- the errno adapter at the probe return (negated C02 symbols; the
    //    composed RegseqError carries HardwareError -> -EIO and
    //    DeviceBusy -> -EBUSY).
    let adapter = pos("if (__tag != 0x2)");
    assert!(on_init_call < adapter);
    pos("return -EIO;");
    pos("return -EBUSY;");

    // -- A-12 polarity: every PL011 register is 32-bit, so the TU must NOT
    //    carry the CONFIG_64BIT guard.
    assert!(
        !tu.contains("CONFIG_64BIT"),
        "a width-4-only TU must not carry the A-12 guard"
    );

    // -- register access (matrix §A): width-switched accessors over the
    //    iomem cookie + the const bank from the recovered block layout.
    pos("#define PL011_DR 0x000");
    pos("#define PL011_FR 0x018");
    pos("#define PL011_IBRD 0x024");
    pos("#define PL011_CR 0x030");
    pos("#define PL011_ICR 0x044");
    assert!(
        tu.contains("readl(") && tu.contains("writel("),
        "32-bit registers must use readl/writel"
    );
    assert!(
        !tu.contains("readq(") && !tu.contains("writeq("),
        "no 64-bit accessor may appear in a 32-bit-only TU"
    );

    // -- A-3 placement: every lowered-body Log is pr_* with the pr_fmt
    //    module prefix; no dev_* outside scaffold-synthesized statements
    //    (none of which log today).
    let prfmt = pos("#define pr_fmt(fmt) KBUILD_MODNAME \": \" fmt");
    assert!(prfmt < tu.find("#include").unwrap());
    pos("pr_debug(\"pl011 probe start\\n\");");
    pos("pr_info(\"pl011 baud divisors: %u %u\\n\"");
    pos("pr_info(\"pl011 up\\n\");");
    pos("pr_warn(\"pl011 deinit: masking interrupts\\n\");");
    pos("pr_err(");
    assert!(
        !tu.contains("dev_dbg(") && !tu.contains("dev_info(") && !tu.contains("dev_err("),
        "A-3: no dev_* in lowered bodies"
    );

    // -- teardown: void remove, Result visibly discarded, devm postamble
    //    elision (feeds the C04 whitelist).
    let remove = pos("static void pl011_remove(struct amba_device *adev)");
    pos("(void)pl011_on_deinit(&priv->inner);");
    pos("devm-managed resources release automatically");
    assert!(adapter < remove);

    // -- module tail: full match set + sentinel + MODULE_DEVICE_TABLE +
    //    module_amba_driver (scaffold doc §1).
    let table = pos(".id\t= 0x00041011,");
    let second = pos(".id\t= 0x00abc123,");
    let sentinel = pos("{ 0, 0 },");
    assert!(table < second && second < sentinel, "full multi-row match table");
    let devtable = pos("MODULE_DEVICE_TABLE(amba, pl011_ids);");
    let macros = pos("module_amba_driver(pl011_driver);");
    assert!(remove < table && table < sentinel && sentinel < devtable && devtable < macros);

    // -- A-10 negative leg: stripping the L2 binding must turn the SAME
    //    model into the hard unresolved-binding diagnostic naming the
    //    missing literals (the op-site gate; R03).
    let mut unbound = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    // No bind_l2_platform: the L1 model alone has no (base, size).
    let err = regseq_backends::emit_linux_c(&unbound).expect_err("unresolved binding must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("A-10") && msg.contains("base/size"),
        "the A-10 diagnostic must name the missing L2 literals: {msg}"
    );
    // Keep the borrow checker honest about the mutable model.
    unbound.device_lifecycle.drivers.clear();
}
