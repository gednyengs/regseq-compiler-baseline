//! Manifest schema unit tests (Doc 3 section 3): parse valid rust / ir /
//! panic manifests, the `entry` default, and reject a malformed one.
//!
//! The `rustc_private` anchor is required because the crate under test
//! transitively links the rustc-backed bridge.

#![feature(rustc_private)]

extern crate rustc_driver;

use codegen_coverage_runner::manifest::{self, discover, parse_manifest, Expect, FixtureKind};
use codegen_coverage_runner::pipeline::Stage;

#[test]
fn parses_rust_pass_manifest_with_default_entry() {
    let text = r#"
name = "example_add"
kind = "rust"
covers = ["BO-1..5", "RV-6"]
expect = "pass"
"#;
    let m = parse_manifest(text).expect("valid rust manifest must parse");
    assert_eq!(m.name, "example_add");
    assert_eq!(m.kind, FixtureKind::Rust);
    assert_eq!(m.entry, "regseq_main", "entry must default to regseq_main");
    assert_eq!(m.covers, vec!["BO-1..5".to_string(), "RV-6".to_string()]);
    assert_eq!(m.expect, Expect::Pass);
}

#[test]
fn parses_ir_manifest() {
    let text = r#"
name = "niche_enum"
kind = "ir"
covers = ["TY-18"]
expect = "pass"
"#;
    let m = parse_manifest(text).expect("valid ir manifest must parse");
    assert_eq!(m.kind, FixtureKind::Ir);
}

#[test]
fn parses_panic_manifest_with_explicit_entry() {
    let text = r#"
name = "div_by_zero"
kind = "rust"
entry = "driver_main"
covers = ["AK-3"]
expect = "panic"
"#;
    let m = parse_manifest(text).expect("valid panic manifest must parse");
    assert_eq!(m.entry, "driver_main");
    assert_eq!(m.expect, Expect::Panic);
}

#[test]
fn parses_symbolic_expected_exit_and_defaults_to_absent() {
    // The `[expected]` table carries the symbolic errno identifier; no numeric
    // is ever stored (contract C04).
    let with_exit = r#"
name = "errno_return"
kind = "rust"
covers = ["RE-1"]
expect = "pass"
[expected]
exit = "ETIMEDOUT"
"#;
    let m = parse_manifest(with_exit).expect("manifest with [expected].exit must parse");
    assert_eq!(m.expected.exit.as_deref(), Some("ETIMEDOUT"));

    // The literal "0" is accepted (exit-0 semantics, made explicit).
    let zero = r#"
name = "ok_return"
kind = "rust"
covers = ["RE-2"]
expect = "pass"
[expected]
exit = "0"
"#;
    assert_eq!(
        parse_manifest(zero)
            .expect("explicit exit = \"0\" must parse")
            .expected
            .exit
            .as_deref(),
        Some("0")
    );

    // Absent `[expected]` defaults to no symbolic exit -- existing fixtures
    // parse unchanged with exit-0 semantics (invariant I02).
    let absent = r#"
name = "legacy"
kind = "rust"
covers = ["RE-3"]
expect = "pass"
"#;
    assert_eq!(
        parse_manifest(absent)
            .expect("manifest without [expected] must parse")
            .expected
            .exit,
        None,
        "absent expected.exit must default to None (exit-0 semantics)"
    );
}

#[test]
fn rejects_manifest_missing_required_field() {
    // `kind` is required and absent.
    let text = r#"
name = "broken"
covers = ["IT-1"]
expect = "pass"
"#;
    assert!(
        parse_manifest(text).is_err(),
        "manifest missing `kind` must fail to parse"
    );
}

#[test]
fn rejects_manifest_with_unknown_kind() {
    let text = r#"
name = "broken"
kind = "wasm"
covers = ["IT-1"]
expect = "pass"
"#;
    assert!(
        parse_manifest(text).is_err(),
        "unknown kind value must fail to parse"
    );
}

#[test]
fn parses_reject_manifest_with_stage_and_diagnostic() {
    // T4: a compile-fail oracle. `expect = "reject"` + the `[expected]`
    // stage/diagnostic localize WHERE and WHY the pipeline must reject it.
    let text = r#"
name = "brm-halfspec-reject"
kind = "rust"
covers = ["BRM-005"]
expect = "reject"
[expected]
stage = "bind"
diagnostic = "BRM-005"
"#;
    let m = parse_manifest(text).expect("valid reject manifest must parse");
    assert_eq!(m.expect, Expect::Reject);
    assert_eq!(m.expected.stage, Some(Stage::Bind));
    assert_eq!(m.expected.diagnostic.as_deref(), Some("BRM-005"));

    // The localization is optional: `expect = "reject"` alone parses, with both
    // fields absent (any-stage / any-detail rejection).
    let bare = r#"
name = "bare-reject"
kind = "rust"
covers = ["X-1"]
expect = "reject"
"#;
    let b = parse_manifest(bare).expect("bare reject manifest must parse");
    assert_eq!(b.expect, Expect::Reject);
    assert_eq!(b.expected.stage, None);
    assert_eq!(b.expected.diagnostic, None);
}

// ---- T1: sibling `platform.toml` discovery -------------------------------

/// A temp fixtures-corpus dir removed on drop. `discover` walks a real
/// directory, so the platform-discovery contract needs an on-disk fixture.
struct TempCorpus {
    dir: std::path::PathBuf,
}

impl TempCorpus {
    fn new(label: &str) -> TempCorpus {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ccr-manifest-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("must create temp corpus dir");
        TempCorpus { dir }
    }

    /// Write a one-fixture corpus: `<corpus>/<name>/{manifest.toml,input.rs}`
    /// plus an optional sibling `platform.toml`. Returns the fixture dir.
    fn fixture(&self, name: &str, with_platform: bool) -> std::path::PathBuf {
        let fixture_dir = self.dir.join(name);
        std::fs::create_dir_all(&fixture_dir).expect("must create fixture dir");
        std::fs::write(
            fixture_dir.join("manifest.toml"),
            format!("name = \"{name}\"\nkind = \"rust\"\ncovers = [\"BRM-1\"]\nexpect = \"pass\"\n"),
        )
        .expect("must write manifest.toml");
        std::fs::write(fixture_dir.join("input.rs"), "#![no_std]\n").expect("must write input.rs");
        if with_platform {
            std::fs::write(
                fixture_dir.join("platform.toml"),
                "version = \"1\"\n[mmio.regs]\nbar = 0\n",
            )
            .expect("must write platform.toml");
        }
        fixture_dir
    }

    /// Write a fixture with a caller-supplied `manifest.toml` body (+ an
    /// `input.rs`), for driving the corpus-level lint via `discover`.
    fn manifest_fixture(&self, name: &str, manifest_body: &str) -> std::path::PathBuf {
        let fixture_dir = self.dir.join(name);
        std::fs::create_dir_all(&fixture_dir).expect("must create fixture dir");
        std::fs::write(fixture_dir.join("manifest.toml"), manifest_body)
            .expect("must write manifest.toml");
        std::fs::write(fixture_dir.join("input.rs"), "#![no_std]\n").expect("must write input.rs");
        fixture_dir
    }
}

impl Drop for TempCorpus {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn discover_resolves_sibling_platform_toml_when_present() {
    let corpus = TempCorpus::new("with-platform");
    let fixture_dir = corpus.fixture("brm_keystone", true);

    let fixtures = discover(&corpus.dir).expect("discovery must succeed");
    assert_eq!(fixtures.len(), 1, "one fixture in the temp corpus");
    let f = &fixtures[0];
    assert_eq!(
        f.platform.as_deref(),
        Some(fixture_dir.join("platform.toml").as_path()),
        "a fixture with a sibling platform.toml resolves platform = Some(<dir>/platform.toml)"
    );
}

#[test]
fn discover_lints_committed_reject_fixtures() {
    // A committed reject with a pre-emit stage + a diagnostic is VALID.
    let ok = TempCorpus::new("reject-ok");
    ok.manifest_fixture(
        "good_reject",
        "name = \"good_reject\"\nkind = \"rust\"\ncovers = [\"BRM-005\"]\n\
         expect = \"reject\"\n[expected]\nstage = \"bind\"\ndiagnostic = \"BRM-005\"\n",
    );
    assert!(
        discover(&ok.dir).is_ok(),
        "a reject fixture with a pre-emit stage + diagnostic must discover cleanly"
    );

    // A bare reject (no stage/diagnostic) is REJECTED — it would green any failure.
    let bare = TempCorpus::new("reject-bare");
    bare.manifest_fixture(
        "bare_reject",
        "name = \"bare_reject\"\nkind = \"rust\"\ncovers = [\"X\"]\nexpect = \"reject\"\n",
    );
    assert!(
        discover(&bare.dir).is_err(),
        "a committed reject with no [expected].stage must be rejected by the lint"
    );

    // A post-emit stage is REJECTED — reject short-circuits after emit, so it
    // could never be reached.
    let post = TempCorpus::new("reject-postemit");
    post.manifest_fixture(
        "postemit_reject",
        "name = \"postemit_reject\"\nkind = \"rust\"\ncovers = [\"X\"]\n\
         expect = \"reject\"\n[expected]\nstage = \"compile\"\ndiagnostic = \"x\"\n",
    );
    assert!(
        discover(&post.dir).is_err(),
        "a reject fixture with a post-emit stage must be rejected by the lint"
    );

    // Stage present but diagnostic missing is REJECTED (wrong-reason green guard).
    let nodiag = TempCorpus::new("reject-nodiag");
    nodiag.manifest_fixture(
        "nodiag_reject",
        "name = \"nodiag_reject\"\nkind = \"rust\"\ncovers = [\"X\"]\n\
         expect = \"reject\"\n[expected]\nstage = \"bind\"\n",
    );
    assert!(
        discover(&nodiag.dir).is_err(),
        "a committed reject with no [expected].diagnostic must be rejected by the lint"
    );
}

#[test]
fn discover_leaves_platform_none_when_absent() {
    let corpus = TempCorpus::new("no-platform");
    corpus.fixture("plain_fixture", false);

    let fixtures = discover(&corpus.dir).expect("discovery must succeed");
    assert_eq!(fixtures.len(), 1, "one fixture in the temp corpus");
    assert_eq!(
        fixtures[0].platform, None,
        "a fixture with no sibling platform.toml resolves platform = None (the L2-free baseline)"
    );
}

// -- the explicit OsBinding selector (spec-convergence-pl011 C05, plan
//    decision 1) -------------------------------------------------------------

/// Write the two C05 per-OS platform files into a fixture dir.
fn write_suffixed_platforms(fixture_dir: &std::path::Path) {
    for name in ["platform.linux.toml", "platform.zephyr.toml"] {
        std::fs::write(
            fixture_dir.join(name),
            "version = \"1\"\n[mmio.regs]\nbar = 0\n",
        )
        .expect("must write per-OS platform file");
    }
}

#[test]
fn with_binding_resolves_the_c05_fixed_names_per_os() {
    let corpus = TempCorpus::new("osbinding");
    let fixture_dir = corpus.fixture("converged", false);
    write_suffixed_platforms(&fixture_dir);

    let fixtures = discover(&corpus.dir).expect("discovery must succeed");
    let f = &fixtures[0];
    assert_eq!(f.platform, None, "no legacy platform.toml discovered");

    let linux = f
        .with_binding(manifest::OsBinding::Linux)
        .expect("linux binding resolves");
    assert_eq!(
        linux.platform.as_deref(),
        Some(fixture_dir.join("platform.linux.toml").as_path())
    );
    assert_eq!(linux.golden_c, fixture_dir.join("golden.c"));
    assert_eq!(linux.expected_stdout, fixture_dir.join("expected.stdout"));

    let zephyr = f
        .with_binding(manifest::OsBinding::Zephyr)
        .expect("zephyr binding resolves");
    assert_eq!(
        zephyr.platform.as_deref(),
        Some(fixture_dir.join("platform.zephyr.toml").as_path())
    );
    assert_eq!(zephyr.golden_c, fixture_dir.join("golden.zephyr-oracle.c"));
    assert_eq!(
        zephyr.expected_stdout,
        fixture_dir.join("expected.zephyr.stdout")
    );
}

#[test]
fn with_binding_is_loud_when_the_selected_platform_file_is_missing() {
    // No silent fallback to the legacy platform.toml (plan decision 1).
    let corpus = TempCorpus::new("osbinding-missing");
    corpus.fixture("legacy_only", true);
    let fixtures = discover(&corpus.dir).expect("discovery must succeed");
    let err = fixtures[0]
        .with_binding(manifest::OsBinding::Zephyr)
        .expect_err("missing platform.zephyr.toml must be loud");
    let msg = err.to_string();
    assert!(
        msg.contains("platform.zephyr.toml") && msg.contains("no fallback"),
        "{msg}"
    );
}

#[test]
fn discover_rejects_a_fixture_carrying_both_platform_conventions() {
    // The collapse must not half-apply: legacy platform.toml + a suffixed
    // file in one fixture is a LOUD loader error, not a preference.
    let corpus = TempCorpus::new("both-conventions");
    let fixture_dir = corpus.fixture("half_applied", true);
    write_suffixed_platforms(&fixture_dir);
    let err = discover(&corpus.dir).expect_err("dual-convention fixture must fail discovery");
    let msg = err.to_string();
    assert!(
        msg.contains("BOTH") && msg.contains("mutually exclusive"),
        "{msg}"
    );
}
