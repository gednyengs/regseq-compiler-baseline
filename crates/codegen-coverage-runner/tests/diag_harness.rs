//! Task-e harness tests (Test Plan B4): Change E (`--bless` writes only
//! `golden.c` from the stage-2 emit, never `expected.stdout`; a mismatch is
//! reported; the non-bless path is unchanged) and Change F (entry-symbol
//! resolution mirrors `fn_cname` -- a rendered-suffix to `sanitize(emission_name)`;
//! missing / ambiguous suffix is a clear error).
//!
//! The on-disk corpus six-stage pass (B5, Change G) lives in its OWN test
//! binary (`corpus_e2e.rs`): its stage-3 byte-snapshot is reproducible only in
//! a process that has NOT accumulated unrelated rustc compilations (the emit's
//! MIR is sensitive to rustc's process-global interner history), so it must run
//! isolated -- exactly the canonical Mode-A context the runner uses to
//! regenerate the ledger. Mixing it with the bless / entry bridges here would
//! pollute that context.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary (stage 1 + entry resolution both
//! drive the real bridge).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{Expect, Fixture, FixtureKind, Manifest};
use codegen_coverage_runner::pipeline;
use regseq_backends::{entry_c_symbol, EntrySymbol};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A temp `.rs` snippet file removed on drop.
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("ccr-e-{label}-{}-{nanos}.rs", std::process::id()));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A throwaway fixture directory under the system temp dir, removed on drop.
struct TempFixtureDir {
    dir: PathBuf,
}

impl TempFixtureDir {
    fn new(label: &str) -> TempFixtureDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("ccr-e-fix-{label}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("must create temp fixture dir");
        TempFixtureDir { dir }
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let p = self.dir.join(name);
        std::fs::write(&p, content).expect("must write sibling file");
        p
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A minimal self-contained diag-log driver: one `info!` of a computed value.
const DIAG_INPUT: &str = r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> { I32(i32), Str(&'a str) }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }
            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {{ $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }}; }
        pub(crate) use info;
    }
}

#[inline(never)]
fn add(a: i32, b: i32) -> i32 { a + b }

#[no_mangle]
pub extern "C" fn regseq_main() {
    crate::regseq::log::info!("r={}", add(20, 22));
}
"#;

/// Build a `Fixture` over `dir` (kind=rust, entry=regseq_main, expect=pass).
fn fixture_in(dir: &TempFixtureDir, input: PathBuf) -> Fixture {
    Fixture {
        manifest: Manifest {
            name: "tmp".to_string(),
            kind: FixtureKind::Rust,
            entry: "regseq_main".to_string(),
            covers: vec!["DL-2".to_string()],
            expect: Expect::Pass,
            overflow_checks: None,
        },
        dir: dir.dir.clone(),
        input,
        golden_c: dir.dir.join("golden.c"),
        expected_stdout: dir.dir.join("expected.stdout"),
        expected_stderr: None,
    }
}

// ---- Change E: --bless honesty (B4) ---------------------------------------

#[test]
fn bless_writes_golden_c_from_emit_and_never_expected_stdout() {
    let td = TempFixtureDir::new("bless");
    let input = td.write("input.rs", DIAG_INPUT);
    let fixture = fixture_in(&td, input);
    assert!(!fixture.expected_stdout.exists());

    let outcome = pipeline::bless_fixture(&fixture, cc());
    // golden.c was written and equals the stage-2 emit (the section-8
    // byte-faithful fwrite sequence).
    assert!(fixture.golden_c.exists(), "golden.c must be written");
    assert!(outcome.blessed_bytes.is_some());
    let golden = std::fs::read_to_string(&fixture.golden_c).unwrap();
    assert!(
        golden.contains("fwrite(\"[INFO] r=\", 1, 9, stdout);")
            && golden.contains("snprintf(_n, sizeof _n, \"%d\", (int)("),
        "golden.c is the section-8 fwrite emit:\n{golden}"
    );
    // The honesty invariant: --bless NEVER writes expected.stdout.
    assert!(
        !fixture.expected_stdout.exists(),
        "--bless must never write expected.stdout"
    );
}

#[test]
fn bless_reports_mismatch_against_hand_authored_expected_stdout() {
    let td = TempFixtureDir::new("blessmiss");
    let input = td.write("input.rs", DIAG_INPUT);
    // Hand-author a WRONG expected.stdout.
    td.write("expected.stdout", "[INFO] r=999\n");
    let fixture = fixture_in(&td, input);

    let before = std::fs::read_to_string(&fixture.expected_stdout).unwrap();
    let outcome = pipeline::bless_fixture(&fixture, cc());
    // golden.c is still written (for diff review)...
    assert!(fixture.golden_c.exists());
    // ...the mismatch is reported (not a silent pass)...
    assert!(!outcome.ok, "a stdout mismatch is not ok");
    assert!(
        outcome.report.contains("MISMATCH") && outcome.report.contains("r=42"),
        "mismatch report shows the actual stdout: {}",
        outcome.report
    );
    // ...and the hand-authored expected.stdout is left UNTOUCHED.
    let after = std::fs::read_to_string(&fixture.expected_stdout).unwrap();
    assert_eq!(before, after, "--bless must not modify expected.stdout");
}

#[test]
fn bless_ok_when_expected_stdout_matches() {
    let td = TempFixtureDir::new("blessok");
    let input = td.write("input.rs", DIAG_INPUT);
    td.write("expected.stdout", "[INFO] r=42\n");
    let fixture = fixture_in(&td, input);

    let outcome = pipeline::bless_fixture(&fixture, cc());
    assert!(outcome.ok, "matching stdout is ok: {}", outcome.report);
    assert!(outcome.report.contains("matches expected.stdout"));
}

#[test]
fn non_bless_run_writes_no_files_and_compares_stage3() {
    // The non-bless path reads golden.c (stage-3 compare); it must NEVER write
    // golden.c or expected.stdout. With no golden.c present it fails at stage-3
    // (a snapshot read error), proving it does not regenerate the golden.
    let td = TempFixtureDir::new("nonbless");
    let input = td.write("input.rs", DIAG_INPUT);
    let fixture = fixture_in(&td, input);
    assert!(!fixture.golden_c.exists());

    let result = pipeline::run_fixture(&fixture, cc());
    // No golden.c was created by the non-bless run.
    assert!(
        !fixture.golden_c.exists(),
        "the non-bless run must not write golden.c"
    );
    assert!(
        !fixture.expected_stdout.exists(),
        "the non-bless run must not write expected.stdout"
    );
    // It fails at the snapshot stage because golden.c is absent.
    assert!(result.is_fail(), "missing golden.c -> a stage-3 failure");
}

// ---- Change F: entry-symbol resolution (B4) -------------------------------

#[test]
fn entry_resolves_to_sanitized_emission_name() {
    let snippet = TempSnippet::new("entry", DIAG_INPUT);
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    // The default suffix `regseq_main` resolves to exactly one symbol.
    let sym = match entry_c_symbol(&model, "regseq_main") {
        EntrySymbol::Resolved(s) => s,
        other => panic!("expected a resolved entry, got {other:?}"),
    };
    // The bridge mints emission_name = rendered path (e.g. crate::regseq_main);
    // the backend emits sanitize(emission_name), which the harness must link.
    assert!(
        sym.ends_with("regseq_main"),
        "entry symbol ends with the entry suffix: {sym}"
    );
    assert!(!sym.contains("::"), "entry symbol is sanitized: {sym}");
    // The emitted TU defines exactly that symbol, so the generated main.c links.
    let tu = regseq_backends::emit_generic_c(&model).unwrap();
    assert!(
        tu.contains(&format!("void {sym}(void)")),
        "the emitted TU defines the resolved entry symbol {sym}"
    );
}

#[test]
fn entry_missing_suffix_is_not_found() {
    let snippet = TempSnippet::new("entrymiss", DIAG_INPUT);
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    assert_eq!(
        entry_c_symbol(&model, "no_such_entry_xyz"),
        EntrySymbol::NotFound
    );
}

#[test]
fn entry_ambiguous_suffix_reports_all_matches() {
    // Two functions whose rendered paths both end with `helper`.
    let src = format!(
        "{}\n{}",
        DIAG_INPUT,
        r#"
#[no_mangle]
#[inline(never)]
pub extern "C" fn a_helper() {}
#[no_mangle]
#[inline(never)]
pub extern "C" fn b_helper() {}
"#
    );
    let snippet = TempSnippet::new("entryamb", &src);
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("snippet must bridge to Ok, got {other:?}"),
    };
    match entry_c_symbol(&model, "helper") {
        EntrySymbol::Ambiguous(syms) => {
            assert!(syms.len() >= 2, "at least two matches: {syms:?}");
        }
        other => panic!("expected ambiguous, got {other:?}"),
    }
}
