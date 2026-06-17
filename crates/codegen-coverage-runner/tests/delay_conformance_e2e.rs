//! Timed-cpu-delay-and-sleep (TCD construct) per-OS conformance (M2/T08+T09):
//! a driver whose `on_init` calls `busy_wait` (const + runtime-valued) and a
//! process-context `sleep` lowers, under the Linux backend, to the
//! `regseq_linux_busy_wait_ns` magnitude-dispatch helper (ndelay/udelay/mdelay,
//! round up) + msleep-only `regseq_linux_sleep_ns`; and under the Zephyr
//! backend to `regseq_zephyr_busy_wait_ns` (k_busy_wait, round up to us) +
//! `regseq_zephyr_sleep_ns` (k_msleep, round up to ms). The fixture lives in a
//! per-call temp corpus (no committed golden, no top-level-corpus interaction);
//! the assertions are on the emitted C, so they are interner-insensitive.
//!
//! `sleep` is called in process-context `on_init`, NOT an ISR, so this is legal
//! independent of the TCD-ERR-01 context check (M3).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp single-fixture corpus removed on drop.
struct TempCorpus {
    dir: PathBuf,
}

impl TempCorpus {
    fn new(label: &str) -> TempCorpus {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ccr-delay-conf-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp corpus dir");
        TempCorpus { dir }
    }
}

impl Drop for TempCorpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Write the delay-driver fixture into the temp corpus and return its dir.
fn write_fixture(corpus: &TempCorpus) -> PathBuf {
    let fixture_dir = corpus.dir.join("delay-driver");
    std::fs::create_dir_all(&fixture_dir).expect("create fixture dir");

    // Include the committed lifecycle surface (Lifecycle trait + RegseqError) by
    // ABSOLUTE path: `include!` is resolved relative to the temp input.rs, and
    // `env!("CARGO_MANIFEST_DIR")` is not set when the bridge runs rustc, so the
    // path is baked in here at test-compile time.
    let surface = format!(
        "{}/fixtures/_lifecycle_surface.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let input = format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         include!(\"{surface}\");\n\n\
         #[rseq::framework_type(\"duration\")]\n#[derive(Clone, Copy)]\n\
         pub struct Duration {{ ns: u64 }}\n\
         impl Duration {{\n\
         \tpub const fn from_us(us: u64) -> Self {{ Self {{ ns: us * 1_000 }} }}\n\
         \tpub const fn from_ms(ms: u64) -> Self {{ Self {{ ns: ms * 1_000_000 }} }}\n}}\n\n\
         #[inline(never)]\n#[rseq::intrinsic(\"busy_wait\")]\npub fn busy_wait(_d: Duration) {{}}\n\
         #[inline(never)]\n#[rseq::intrinsic(\"sleep\")]\npub fn sleep(_d: Duration) {{}}\n\n\
         #[inline(never)]\nfn eeprom_delay_us() -> u64 {{ 500 }}\n\n\
         #[rseq::driver(name = \"delaydrv\", bus = \"amba\",\n\
         \tamba = [ {{ id = 0x00041011, mask = 0x000fffff }} ],\n\
         \tcompatible = [\"regseq,delaydrv\"]\n)]\n\
         pub struct DelayDrv {{ __pad: u8 }}\n\n\
         impl Lifecycle for DelayDrv {{\n\
         \tfn on_init(&mut self) -> Result<(), RegseqError> {{\n\
         \t\tbusy_wait(Duration::from_us(10));\n\
         \t\tbusy_wait(Duration::from_us(eeprom_delay_us()));\n\
         \t\tsleep(Duration::from_ms(5));\n\
         \t\tOk(())\n\t}}\n}}\n"
    );
    std::fs::write(fixture_dir.join("input.rs"), input).expect("write input.rs");
    std::fs::write(
        fixture_dir.join("manifest.toml"),
        "name = \"delay-driver\"\nkind = \"rust\"\ncovers = [\"TCD-C04\"]\nlifecycle_scenario = \"orderly\"\nexpect = \"pass\"\n",
    )
    .expect("write manifest.toml");
    for (suffix, os) in [("linux", "linux"), ("zephyr", "zephyr")] {
        std::fs::write(
            fixture_dir.join(format!("platform.{suffix}.toml")),
            format!("version = \"1\"\n[platform]\nos = \"{os}\"\nboard = \"qemu-virt\"\narch = \"aarch64\"\n"),
        )
        .expect("write platform toml");
    }
    fixture_dir
}

/// Bridge + bind the fixture under `binding`, returning the bound model.
fn bound_model(corpus_dir: &Path, binding: manifest::OsBinding) -> regseq_model::RegseqModel {
    let fixtures = manifest::discover(corpus_dir).expect("discover temp corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "delay-driver")
        .expect("delay-driver discovered")
        .with_binding(binding)
        .expect("resolves the per-OS binding");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, &fixture).expect("L2 bind");
    model
}

#[test]
fn linux_busy_wait_bands_and_msleep_only() {
    let corpus = TempCorpus::new("linux");
    let dir = write_fixture(&corpus);
    let model = bound_model(&corpus.dir, manifest::OsBinding::Linux);
    let tu = regseq_backends::emit_linux_c(&model)
        .expect("linux_c emit must succeed")
        .tu;
    let _ = dir;

    // The magnitude-dispatch helper, its include, and the three atomic-safe
    // bands with round-up.
    assert!(tu.contains("#include <linux/delay.h>"), "delay include:\n{tu}");
    assert!(
        tu.contains("static void regseq_linux_busy_wait_ns(uint64_t ns)"),
        "busy_wait helper:\n{tu}"
    );
    assert!(tu.contains("ndelay((unsigned long)ns)"), "ndelay band:\n{tu}");
    assert!(
        tu.contains("udelay((unsigned long)(1ULL + (ns - 1ULL) / 1000ULL))"),
        "udelay round-up:\n{tu}"
    );
    assert!(
        tu.contains("mdelay((unsigned long)(1ULL + (ns - 1ULL) / 1000000ULL))"),
        "mdelay round-up:\n{tu}"
    );
    // sleep: msleep-only.
    assert!(
        tu.contains("static void regseq_linux_sleep_ns(uint64_t ns)") && tu.contains("msleep("),
        "msleep-only sleep helper:\n{tu}"
    );
    // Both busy_wait call sites + the sleep call site, reading the Duration's
    // ns field. (The const 10us and the runtime eeprom value both lower through
    // the same helper — intent preserved as helper identity.)
    let busy_calls = tu.matches("regseq_linux_busy_wait_ns((uint64_t)").count();
    assert!(busy_calls >= 2, "two busy_wait call sites, found {busy_calls}:\n{tu}");
    assert!(
        tu.contains("regseq_linux_sleep_ns((uint64_t)"),
        "sleep call site:\n{tu}"
    );
    // The busy_wait helper never calls a sleeping primitive (I04 intent).
    let busy_helper = &tu[tu.find("regseq_linux_busy_wait_ns(uint64_t ns)").unwrap()..];
    let busy_helper = &busy_helper[..busy_helper.find('}').unwrap()];
    assert!(!busy_helper.contains("msleep"), "busy helper must not sleep");
}

#[test]
fn zephyr_k_busy_wait_and_k_msleep() {
    let corpus = TempCorpus::new("zephyr");
    let _dir = write_fixture(&corpus);
    let model = bound_model(&corpus.dir, manifest::OsBinding::Zephyr);
    let tu = regseq_backends::emit_zephyr_c(&model)
        .expect("zephyr_c emit must succeed")
        .tu;

    assert!(tu.contains("#include <zephyr/kernel.h>"), "kernel include:\n{tu}");
    assert!(
        tu.contains("static void regseq_zephyr_busy_wait_ns(uint64_t ns)"),
        "busy_wait helper:\n{tu}"
    );
    assert!(
        tu.contains("k_busy_wait((uint32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000ULL))"),
        "k_busy_wait round-up:\n{tu}"
    );
    assert!(
        tu.contains("static void regseq_zephyr_sleep_ns(uint64_t ns)")
            && tu.contains("k_msleep((int32_t)(ns == 0ULL ? 0ULL : 1ULL + (ns - 1ULL) / 1000000ULL))"),
        "k_msleep round-up:\n{tu}"
    );
    let busy_calls = tu.matches("regseq_zephyr_busy_wait_ns((uint64_t)").count();
    assert!(busy_calls >= 2, "two busy_wait call sites, found {busy_calls}:\n{tu}");
    assert!(
        tu.contains("regseq_zephyr_sleep_ns((uint64_t)"),
        "sleep call site:\n{tu}"
    );
}
