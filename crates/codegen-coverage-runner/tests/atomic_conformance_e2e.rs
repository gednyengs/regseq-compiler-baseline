//! Atomicity construct M4 per-target conformance (T14-T17): a driver whose
//! `on_init` drives `Atomic<T>` fields lowers, under the Linux backend, to the
//! kernel-atomic API (`atomic_t`/`atomic64_t` storage; `atomic_read_acquire` /
//! `atomic_set_release` / `atomic_fetch_*` / `atomic_fetch_or` bit ops /
//! `atomic_try_cmpxchg` / `smp_mb`), and under the Zephyr backend to the GCC
//! `__atomic_*` builtins (`__atomic_load_n`/`store_n`/`fetch_*`/
//! `compare_exchange_n` with the exact `__ATOMIC_*` order). The fixture lives in
//! a per-call temp corpus; the assertions are on the emitted C (string-level),
//! so they are interner-insensitive and need no kernel/Zephyr tree.
//!
//! The mapping follows the Codex M4 foundation review (uniform width-keyed
//! storage; bit ops via fetch_or/and; Linux SeqCst load/store -> acquire/release;
//! conservative smp_mb fence; Zephyr __atomic builtins for native dual-ordering
//! CAS + width parametricity).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

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
            "ccr-atomic-conf-{label}-{}-{nanos}",
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

fn write_fixture(corpus: &TempCorpus) -> PathBuf {
    let fixture_dir = corpus.dir.join("atomic-driver");
    std::fs::create_dir_all(&fixture_dir).expect("create fixture dir");
    let surface = format!(
        "{}/fixtures/_lifecycle_surface.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let input = format!(
        "#![no_std]\n#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         include!(\"{surface}\");\n\n\
         #[derive(Clone, Copy)] pub enum Ordering {{ Relaxed, Acquire, Release, AcqRel, SeqCst }}\n\
         pub struct Atomic<T> {{ value: T }}\n\
         impl<T: Copy> Atomic<T> {{\n\
         \tpub const fn new(value: T) -> Self {{ Self {{ value }} }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_load\")] pub fn load(&self, _o: Ordering) -> T {{ self.value }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_store\")] pub fn store(&self, _v: T, _o: Ordering) {{}}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_fetch_add\")] pub fn fetch_add(&self, _v: T, _o: Ordering) -> T {{ self.value }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_fetch_or\")] pub fn fetch_or(&self, _v: T, _o: Ordering) -> T {{ self.value }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_compare_exchange\")] pub fn compare_exchange(&self, _c: T, _n: T, _s: Ordering, _f: Ordering) -> Result<T, T> {{ Ok(self.value) }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_bit_test_set\")] pub fn bit_test_set(&self, _b: u32, _o: Ordering) -> bool {{ false }}\n\
         \t#[inline(never)] #[rseq::intrinsic(\"atomic_bit_test_clear\")] pub fn bit_test_clear(&self, _b: u32, _o: Ordering) -> bool {{ false }}\n}}\n\
         pub mod atomic {{ use super::Ordering; #[inline(never)] #[rseq::intrinsic(\"atomic_fence\")] pub fn fence(_o: Ordering) {{}} }}\n\n\
         #[rseq::driver(name = \"atomicdrv\", bus = \"amba\",\n\
         \tamba = [ {{ id = 0x00041011, mask = 0x000fffff }} ],\n\
         \tcompatible = [\"regseq,atomicdrv\"]\n)]\n\
         pub struct AtomicDrv {{\n\
         \t#[rseq::atomic(initial = 7)] counter: Atomic<u32>,\n\
         \tflags: Atomic<u32>,\n\
         \twide: Atomic<u64>,\n\
         \tsi: Atomic<i32>,\n\
         \twi: Atomic<i64>,\n\
         \tptr: Atomic<*mut u8>,\n}}\n\n\
         impl Lifecycle for AtomicDrv {{\n\
         \tfn on_init(&mut self) -> Result<(), RegseqError> {{\n\
         \t\tlet _ = self.counter.load(Ordering::Acquire);\n\
         \t\tself.flags.store(1, Ordering::Release);\n\
         \t\tlet _ = self.counter.fetch_add(5, Ordering::AcqRel);\n\
         \t\tlet _ = self.flags.bit_test_set(3, Ordering::SeqCst);\n\
         \t\tlet _ = self.flags.bit_test_clear(3, Ordering::Relaxed);\n\
         \t\tlet _ = self.counter.compare_exchange(12, 20, Ordering::SeqCst, Ordering::Relaxed);\n\
         \t\tlet _ = self.wide.fetch_or(0xff00, Ordering::SeqCst);\n\
         \t\tatomic::fence(Ordering::Release);\n\
         \t\tOk(())\n\t}}\n}}\n"
    );
    std::fs::write(fixture_dir.join("input.rs"), input).expect("write input.rs");
    std::fs::write(
        fixture_dir.join("manifest.toml"),
        "name = \"atomic-driver\"\nkind = \"rust\"\ncovers = [\"ATM-C03\"]\nlifecycle_scenario = \"orderly\"\nexpect = \"pass\"\n",
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

fn bound_model(corpus_dir: &Path, binding: manifest::OsBinding) -> regseq_model::RegseqModel {
    let fixtures = manifest::discover(corpus_dir).expect("discover temp corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "atomic-driver")
        .expect("atomic-driver discovered")
        .with_binding(binding)
        .expect("resolves the per-OS binding");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    codegen_coverage_runner::pipeline::bind_l2_platform(&mut model, &fixture).expect("L2 bind");
    model
}

#[test]
fn linux_atomic_api_selection() {
    let corpus = TempCorpus::new("linux");
    let dir = write_fixture(&corpus);
    let model = bound_model(&corpus.dir, manifest::OsBinding::Linux);
    let tu = regseq_backends::emit_linux_c(&model)
        .expect("linux_c emit")
        .tu;
    let _ = dir;

    // The ops live in the `atomicdrv_on_init(struct AtomicDrv *_1)` callback, so
    // the receiver renders as `&(*_1).field`; the init lives in the probe, so it
    // is `&priv->inner.field`.
    for needle in [
        "#include <linux/atomic.h>",
        // storage override per element/width/signedness (C07/T18): u32/i32 ->
        // atomic_t, u64/i64 -> atomic64_t, pointer -> atomic_long_t (LP64).
        "atomic_t counter",
        "atomic_t flags",
        "atomic64_t wide",
        "atomic_t si",
        "atomic64_t wi",
        "atomic_long_t ptr",
        // init (R10/S10): counter seeded to 7 (in the probe) before on_init.
        "atomic_set(&priv->inner.counter, (int)0x7ULL);",
        // load Acquire -> _acquire; store Release -> _release.
        "atomic_read_acquire(&(*_1).counter)",
        "atomic_set_release(&(*_1).flags,",
        // fetch_add AcqRel -> full (unsuffixed).
        "atomic_fetch_add((int)(0x5u), &(*_1).counter)",
        // bit ops via fetch_or/fetch_and; the mask is built in the UNSIGNED
        // width type (no signed-shift UB), cast to the API value type. Clear is
        // Relaxed -> _relaxed.
        "atomic_fetch_or((int)((u32)1 << ((0x3u) & 31)), &(*_1).flags)",
        "atomic_fetch_and_relaxed((int)(~((u32)1 << ((0x3u) & 31))),",
        // CAS: try_cmpxchg writeback (SeqCst lub Relaxed -> SeqCst -> unsuffixed).
        "atomic_try_cmpxchg(&(*_1).counter, &__cas_exp,",
        // 64-bit field uses the atomic64 API.
        "atomic64_fetch_or((s64)(0xff00ull), &(*_1).wide)",
        // fence: conservative full barrier.
        "smp_mb();",
    ] {
        assert!(
            tu.contains(needle),
            "linux emit must contain `{needle}`:\n{tu}"
        );
    }
    // The bit op must NOT use the non-idiomatic test_and_set_bit cast hack.
    assert!(
        !tu.contains("test_and_set_bit"),
        "must use fetch_or, not test_and_set_bit:\n{tu}"
    );
}

#[test]
fn zephyr_atomic_builtin_selection() {
    let corpus = TempCorpus::new("zephyr");
    let _dir = write_fixture(&corpus);
    let model = bound_model(&corpus.dir, manifest::OsBinding::Zephyr);
    let tu = regseq_backends::emit_zephyr_c(&model)
        .expect("zephyr_c emit")
        .tu;

    // Ops are in `atomicdrv_on_init(struct AtomicDrv *_1)` (receiver `&(*_1).f`);
    // the init is in the probe (`&data->inner.f`).
    for needle in [
        // storage override per element/width/signedness (C07/T18): plain
        // width-T cells; pointer -> uintptr_t (LP64).
        "uint32_t counter",
        "uint32_t flags",
        "uint64_t wide",
        "int32_t si",
        "int64_t wi",
        "uintptr_t ptr",
        // init (R10/S10) in the probe.
        "__atomic_store_n(&data->inner.counter, (uint32_t)0x7ULL, __ATOMIC_RELAXED);",
        // load Acquire / store Release with the exact order.
        "__atomic_load_n(&(*_1).counter, __ATOMIC_ACQUIRE)",
        "__atomic_store_n(&(*_1).flags,",
        "__ATOMIC_RELEASE",
        // fetch_add AcqRel native (no collapse).
        "__atomic_fetch_add(&(*_1).counter,",
        "__ATOMIC_ACQ_REL",
        // bit ops via fetch_or/and.
        "__atomic_fetch_or(&(*_1).flags,",
        "__atomic_fetch_and(&(*_1).flags,",
        // CAS: native DUAL ordering (SeqCst success, Relaxed failure — no collapse).
        "__atomic_compare_exchange_n(&(*_1).counter, &__cas_exp,",
        "false, __ATOMIC_SEQ_CST, __ATOMIC_RELAXED)",
        // 64-bit field is width-parametric (same builtin, uint64_t cell).
        "__atomic_fetch_or(&(*_1).wide,",
        // fence builtin with the exact order.
        "__atomic_thread_fence(__ATOMIC_RELEASE);",
    ] {
        assert!(
            tu.contains(needle),
            "zephyr emit must contain `{needle}`:\n{tu}"
        );
    }
}
