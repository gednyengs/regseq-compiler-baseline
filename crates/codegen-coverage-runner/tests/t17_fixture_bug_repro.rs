//! T17 (PLAN PLAN-2026-06-18-135408, gaps G-T1): fresh reproduction + fix
//! guard for the four `pl011-full-spec` construction-pass bugs.
//!
//! Each bug is a body-construction pattern that an earlier compiler could not
//! bridge. This test extracts each pattern into a minimal self-contained
//! `#![no_std]` snippet and runs it through the same single-file bridge the
//! `b*` coverage fixtures use (`try_bridge_fixture_path`). The runnable
//! snippets (no libcore-method dependency) are additionally emitted to C and
//! executed: the snippet aborts (via an extern fail callback) on a wrong
//! result, so a clean exit is a runtime proof the lowering computes correctly.
//!
//! The four bug sites in `tests/fixtures/pl011-full-spec/src/driver.rs`, with
//! their fresh verdicts against the current compiler:
//!   1. tuple-destructure from a match   (`line_control_value`, ~L188) -- STALE (bridges)
//!   2. ZST method-ref into `Option::map` (`uart_clock_hz`,     ~L332) -- STALE (bridges)
//!   3. struct-typed assoc-const + BitOr  (`rx_char`,            ~L523) -- FIXED (was 2 coupled
//!      bugs: scalar-ABI struct const extraction + a `BitOr::Output` projection
//!      that survived signature translation; see `bridge/const_.rs` and
//!      `bridge/ty.rs::normalize_monomorphic_fn_sig`)
//!   4. runtime division -> Assert{DivByZero} (`set_baud_rate`, ~L146) -- STALE (bridges)

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::toolchain::{self, EntryShim};
use regseq_backends::emit_generic_c;
use regseq_model::{InstanceKind, RegseqModel};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// Map a rendered `DefPath` to the C identifier the backend emits for it
/// (mirrors `generic_c_bodies.rs`'s `c_ident`/`fn_cname`: the bridge mints a
/// crate-prefixed `emission_name`, so the entry symbol is NOT bare
/// `regseq_main`).
fn c_ident(s: &str) -> String {
    s.replace("::", "_")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The emitted C name of the function instance whose rendered path ends in
/// `suffix`.
fn fn_cname(model: &RegseqModel, suffix: &str) -> String {
    for inst in model.instances.iter() {
        if matches!(inst.kind, InstanceKind::Function(_)) {
            let rendered = model.strings.resolve(inst.emission_name);
            if rendered.ends_with(suffix) {
                return c_ident(rendered);
            }
        }
    }
    panic!("no function instance whose path ends in {suffix:?}");
}

fn write_tmp(label: &str, src: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("t17-{label}-{}-{nanos}.rs", std::process::id()));
    std::fs::write(&path, src).expect("write tmp snippet");
    path
}

/// Bridge a snippet and assert it reaches `BridgeOutcome::Ok`, returning the
/// model so runnable cases can additionally emit + execute.
fn bridge_ok(label: &str, src: &str) -> RegseqModel {
    let path = write_tmp(label, src);
    let outcome = try_bridge_fixture_path(&path);
    let _ = std::fs::remove_file(&path);
    match outcome {
        BridgeOutcome::Ok(model) => model,
        BridgeOutcome::ValidatorFailed { violation, .. } => {
            panic!("T17 {label}: bridge produced an invalid model: {violation:?}")
        }
        BridgeOutcome::Rejected(err) => panic!("T17 {label}: bridge REJECTED: {err}"),
        BridgeOutcome::RustcRejected { stderr } => {
            panic!("T17 {label}: rustc rejected the snippet: {stderr:?}")
        }
    }
}

/// Emit the model to generic-C, append a real aborting `regseq_test_fail`
/// callback, compile, and run `regseq_main`. A clean exit (0) proves the
/// lowered body computed the expected result (the snippet calls the callback
/// on a wrong value). Only valid for snippets with no libcore-method call
/// (those surface as body-less externs that would not link).
fn assert_runs_clean(label: &str, model: &RegseqModel) {
    let tu = emit_generic_c(model).unwrap_or_else(|e| panic!("T17 {label}: emit failed: {e:?}"));
    let entry = fn_cname(model, "regseq_main");
    let fail = fn_cname(model, "regseq_test_fail");
    let tu = format!(
        "{tu}\nvoid {fail}(void) {{ fprintf(stderr, \"wrong result\\n\"); abort(); }}\n"
    );
    let run = toolchain::compile_and_run(&tu, &EntryShim::unit_void(&entry), cc())
        .unwrap_or_else(|e| panic!("T17 {label}: TU must compile + run: {e}"));
    assert_eq!(
        run.exit_code,
        Some(0),
        "T17 {label}: a correct lowering exits 0; stderr: {}",
        run.stderr
    );
}

// Bug 1: `let (pen, eps) = match { .. => (A, B) }` with struct-valued tuple
// elements (the real site builds a `(FieldValue, FieldValue)` tuple).
const BUG1_TUPLE: &str = r#"#![no_std]
extern "C" { fn regseq_test_fail(); }

pub enum Parity { None, Odd, Even }

#[derive(Clone, Copy)]
pub struct Fv(u32);

#[no_mangle]
pub extern "C" fn regseq_main() {
    let p = Parity::Even;
    let (pen, eps): (Fv, Fv) = match p {
        Parity::None => (Fv(0), Fv(0)),
        Parity::Odd => (Fv(1), Fv(0)),
        Parity::Even => (Fv(1), Fv(1)),
    };
    if pen.0 + eps.0 != 2 { unsafe { regseq_test_fail(); } }
}
"#;

// Bug 2: `self.clock.as_ref().map(Clock::get_rate).unwrap_or(default)` -- a
// bare fn-item (ZST FnDef) passed as the `FnOnce` argument to `Option::map`.
// Bridge-only (uses libcore `Option::map`/`unwrap_or`, which are body-less
// externs in the bridge and would not link if run).
const BUG2_ZST_METHODREF: &str = r#"#![no_std]
extern "C" { fn regseq_test_fail(); }

pub struct Clock(u32);
impl Clock { pub fn get_rate(&self) -> u32 { self.0 } }

#[no_mangle]
pub extern "C" fn regseq_main() {
    let clock: Option<Clock> = Some(Clock(48));
    let r = clock.as_ref().map(Clock::get_rate).unwrap_or(24);
    if r != 48 { unsafe { regseq_test_fail(); } }
}
"#;

// Bug 3: struct-typed associated consts (`const OVERRUN: Self = Self { bits: 1
// << 0 }`) plus a `BitOr` impl -- the exact `SerialErrors` carrier shape. The
// single-field struct gets a scalar ABI, so the consts are scalar-int values
// with an Adt type (the const-extraction half) and `errors | OVERRUN` returns
// `<SerialErrors as BitOr>::Output` (the projection-normalization half).
const BUG3_BITFLAGS: &str = r#"#![no_std]
use core::ops::BitOr;
extern "C" { fn regseq_test_fail(); }

#[derive(Clone, Copy)]
pub struct SerialErrors { bits: u8 }

impl SerialErrors {
    pub const NONE: Self = Self { bits: 0 };
    pub const OVERRUN: Self = Self { bits: 1 << 0 };
    pub const PARITY: Self = Self { bits: 1 << 1 };
    pub const FRAMING: Self = Self { bits: 1 << 2 };
    pub const BREAK: Self = Self { bits: 1 << 3 };
    pub const fn empty() -> Self { Self::NONE }
    pub const fn bits(self) -> u8 { self.bits }
}

impl BitOr for SerialErrors {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output { Self { bits: self.bits | rhs.bits } }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let mut errors = SerialErrors::empty();
    errors = errors | SerialErrors::OVERRUN;
    errors = errors | SerialErrors::BREAK;
    if errors.bits() != 0b1001 { unsafe { regseq_test_fail(); } }
}
"#;

// Bug 3b: the same carrier, but marked `#[rseq::framework_type]` exactly as the
// real `SerialErrors` is -- guards that the fix is NOT framework-type-specific.
const BUG3_BITFLAGS_FRAMEWORK: &str = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
use core::ops::BitOr;
extern "C" { fn regseq_test_fail(); }

#[rseq::framework_type("serial-errors")]
#[derive(Clone, Copy)]
pub struct SerialErrors { bits: u8 }

impl SerialErrors {
    pub const NONE: Self = Self { bits: 0 };
    pub const OVERRUN: Self = Self { bits: 1 << 0 };
    pub const BREAK: Self = Self { bits: 1 << 3 };
    pub const fn empty() -> Self { Self::NONE }
    pub const fn bits(self) -> u8 { self.bits }
}

impl BitOr for SerialErrors {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output { Self { bits: self.bits | rhs.bits } }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let mut errors = SerialErrors::empty();
    errors = errors | SerialErrors::OVERRUN;
    errors = errors | SerialErrors::BREAK;
    if errors.bits() != 0b1001 { unsafe { regseq_test_fail(); } }
}
"#;

// Bug 4: the baud-divisor division, computed from fn args so the divisor is a
// genuine runtime value -> rustc inserts `Assert{DivisionByZero}` at
// mir_opt_level=0 (the terminator the old construction pass rejected).
const BUG4_DIVISION: &str = r#"#![no_std]
extern "C" { fn regseq_test_fail(); }

#[no_mangle]
#[inline(never)]
pub extern "C" fn compute(uart_clk_hz: u32, baud_rate: u32) -> u64 {
    let clk = uart_clk_hz as u64;
    let baud = baud_rate as u64;
    (clk * 8 + baud) / (baud * 2)
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let d = compute(48_000_000, 115200);
    let ibrd = (d >> 6) as u32;
    let fbrd = (d & 0x3f) as u32;
    if ibrd != 26 || fbrd != 3 { unsafe { regseq_test_fail(); } }
}
"#;

#[test]
fn t17_bug1_tuple_destructure_bridges_and_runs() {
    let model = bridge_ok("bug1-tuple-destructure", BUG1_TUPLE);
    assert_runs_clean("bug1-tuple-destructure", &model);
}

#[test]
fn t17_bug2_zst_methodref_bridges() {
    // Bridge-only: the body calls libcore `Option::map`/`unwrap_or`.
    let _ = bridge_ok("bug2-zst-methodref", BUG2_ZST_METHODREF);
}

#[test]
fn t17_bug3_struct_scalar_const_and_bitor_bridges_and_runs() {
    let model = bridge_ok("bug3-bitflags-const", BUG3_BITFLAGS);
    assert_runs_clean("bug3-bitflags-const", &model);
}

#[test]
fn t17_bug3b_framework_type_carrier_bridges() {
    // The fix is generic, not framework-type-specific: the `#[rseq::framework_type]`
    // marker must not change the verdict.
    let _ = bridge_ok("bug3b-bitflags-framework", BUG3_BITFLAGS_FRAMEWORK);
}

#[test]
fn t17_bug4_division_assert_bridges_and_runs() {
    let model = bridge_ok("bug4-division-assert", BUG4_DIVISION);
    assert_runs_clean("bug4-division-assert", &model);
}
