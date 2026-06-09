//! Fixture manifest schema + corpus discovery (Doc 3 section 3).
//!
//! Each fixture is a directory
//! `crates/codegen-coverage-runner/fixtures/<name>/` carrying a
//! `manifest.toml` plus the sibling input / golden / expected files.
//! This module parses the manifest and resolves those sibling paths;
//! the actual file contents are consumed later by the pipeline stages.
//!
//! Phase-1 task a ships zero fixtures (Non-Goals): the corpus is empty
//! until task e authors the first one. [`discover`] therefore returns an
//! empty `Vec` for an empty (or fixtures-only-README) directory, which is
//! the honest Phase-1 baseline, not an error.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::pipeline::Stage;

/// The `manifest.toml` schema per Doc 3 section 3.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct Manifest {
    /// Fixture name (matches the fixture directory name by convention).
    pub name: String,
    /// Whether the input is a Rust source snippet or a hand-built IR model.
    pub kind: FixtureKind,
    /// `emission_name` of the entry instance (Doc 2 D3); default
    /// `regseq_main`.
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Doc-1 matrix row IDs this fixture exercises (the sole
    /// coverage-attribution source the ledger aggregates).
    pub covers: Vec<String>,
    /// Whether the fixture is expected to run cleanly or to panic.
    pub expect: Expect,
    /// Optional `-C overflow-checks` override threaded to the bridge
    /// (`BuildOverrides::overflow_checks`). Absent (the default) preserves
    /// the helper's hard-wired `overflow_checks = false`; `Some(true)` is
    /// the documented requirement for the arithmetic-overflow panic rows
    /// (AK-2 `Overflow`, AK-3 `OverflowNeg`), whose `Assert`s exist in the
    /// bridged MIR only when overflow checks are on.
    #[serde(default)]
    pub overflow_checks: Option<bool>,
    /// Expected-outcome configuration beyond the sibling `expected.*` files
    /// (the symbolic `expected.exit` oracle). Absent (the default) preserves
    /// the historical exit-0 pass semantics, so existing fixtures parse and
    /// run unchanged.
    #[serde(default)]
    pub expected: Expected,
    /// Optional device-lifecycle scenario selection (RD-5/RD-8), a CSV of
    /// scenario tokens (`orderly`, `surprise`, `init-fail`, `shutdown`) passed
    /// to the backend as the `backend.lifecycle.scenario` param. Absent (the
    /// default) selects all scenarios; for a no-lifecycle fixture it has no
    /// effect (the backend emits no scenarios). Maps to the same
    /// `BackendOptions` the CLI `--param backend.lifecycle.scenario` produces.
    #[serde(default)]
    pub lifecycle_scenario: Option<String>,
}

/// The `[expected]` manifest table: expected-outcome configuration resolved
/// at run time rather than from a sibling file. Carries the symbolic process
/// exit oracle (for `pass`/`panic`) and the expected-rejection localization
/// (for `reject`).
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct Expected {
    /// Symbolic expected process exit code: an errno identifier
    /// (e.g. `"ETIMEDOUT"`) or the literal `"0"`. Absent (the default)
    /// preserves exit-0 semantics. The identifier is resolved to a numeric
    /// on the fixture's own `cc`/libc at run time (contract C04); no numeric
    /// is ever stored here (invariant I03).
    #[serde(default)]
    pub exit: Option<String>,
    /// For `expect = "reject"`: the pipeline [`Stage`] the fixture MUST fail at
    /// (e.g. `stage = "bind"` for a join-time BRM). Absent → any stage is
    /// accepted as the rejection point (the diagnostic still gates it). Ignored
    /// for `pass`/`panic`.
    #[serde(default)]
    pub stage: Option<Stage>,
    /// For `expect = "reject"`: a substring the failure detail MUST contain
    /// (e.g. `diagnostic = "BRM-005"`). Absent → the failure detail is not
    /// checked (only that a failure occurred at the expected stage). Ignored for
    /// `pass`/`panic`.
    #[serde(default)]
    pub diagnostic: Option<String>,
}

/// Fixture input source kind.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FixtureKind {
    /// `input.rs` compiled through the real bridge (the supported path).
    Rust,
    /// `model.rs` hand-builds the `RegseqModel`; deferred this phase
    /// (the executor `Skip`s it with a recorded reason).
    Ir,
}

/// Expected runtime outcome of a fixture.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Expect {
    /// Clean exit; stdout is compared to `expected.stdout` (stage 6).
    Pass,
    /// Non-zero exit; stderr is compared to `expected.stderr` (stage 5
    /// inverted).
    Panic,
    /// The pipeline is EXPECTED to fail (reject the fixture) before it produces a
    /// running program — a compile-fail oracle. Green iff it fails at the
    /// `[expected].stage` (when set) with the `[expected].diagnostic` substring
    /// (when set); red if it is accepted (reaches emit) or fails differently. The
    /// end-to-end home for the join-time BRM diagnostics (T5).
    Reject,
}

/// Default entry instance name when the manifest omits `entry`.
fn default_entry() -> String {
    "regseq_main".to_string()
}

/// A discovered fixture: its parsed manifest plus the resolved on-disk
/// paths of its sibling files.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// The fixture directory.
    pub dir: PathBuf,
    /// `input.rs` (kind `rust`) or `model.rs` (kind `ir`).
    pub input: PathBuf,
    /// `golden.c` (stage-3 snapshot oracle).
    pub golden_c: PathBuf,
    /// `expected.stdout` (stage-6 behavioral oracle).
    pub expected_stdout: PathBuf,
    /// `expected.stderr`; `Some` iff `expect == Panic` (stage-5 oracle).
    pub expected_stderr: Option<PathBuf>,
    /// Sibling `platform.toml` (the L2 binding input), `Some` iff present in
    /// the fixture dir. Mirrors production's `--platform-file` and the
    /// `tests/fixtures/regaccess-*` convention. Absent = no L2 binding (the
    /// historical behavior; every existing fixture resolves to `None`). When
    /// `Some`, the pipeline parses it and runs the `regseq_l2` bind/validate
    /// over the bridged model before emit (T2).
    pub platform: Option<PathBuf>,
}

/// Errors from manifest parsing / corpus discovery.
#[derive(Debug)]
pub enum ManifestError {
    /// I/O error walking the corpus or reading a manifest.
    Io(std::io::Error),
    /// A `manifest.toml` failed to parse.
    Parse {
        /// The offending manifest path.
        path: PathBuf,
        /// The underlying TOML error.
        err: toml::de::Error,
    },
    /// A `manifest.toml` parsed but is not a valid committed fixture (a
    /// corpus-level lint beyond the schema, e.g. an under-specified `reject`).
    Invalid {
        /// The offending manifest path.
        path: PathBuf,
        /// What is wrong + how to fix it.
        detail: String,
    },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io(err) => write!(f, "fixture discovery I/O error: {err}"),
            ManifestError::Parse { path, err } => {
                write!(f, "failed to parse {}: {err}", path.display())
            }
            ManifestError::Invalid { path, detail } => {
                write!(f, "invalid fixture manifest {}: {detail}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<std::io::Error> for ManifestError {
    fn from(value: std::io::Error) -> Self {
        ManifestError::Io(value)
    }
}

/// Parse a `manifest.toml` body. Surfaced separately from [`discover`]
/// so tests can drive the schema with an in-memory string.
pub fn parse_manifest(text: &str) -> Result<Manifest, toml::de::Error> {
    toml::from_str(text)
}

/// Walk `fixtures_dir`, load each `<name>/manifest.toml`, resolve the
/// sibling input / golden / expected paths, and return the fixtures
/// sorted by manifest `name` (Doc 3 section 4 determinism).
///
/// An absent or empty directory (or one holding only the schema README)
/// returns an empty `Vec` -- the Phase-1 baseline -- not an error.
pub fn discover(fixtures_dir: &Path) -> Result<Vec<Fixture>, ManifestError> {
    if !fixtures_dir.exists() {
        return Ok(Vec::new());
    }

    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(fixtures_dir)? {
        let entry = entry?;
        let dir = entry.path();
        if !entry.file_type()?.is_dir() {
            // Skip non-directory entries (e.g. the schema README).
            continue;
        }
        let manifest_path = dir.join("manifest.toml");
        if !manifest_path.exists() {
            // A directory without a manifest is not a fixture.
            continue;
        }
        let text = std::fs::read_to_string(&manifest_path)?;
        let manifest = parse_manifest(&text).map_err(|err| ManifestError::Parse {
            path: manifest_path.clone(),
            err,
        })?;
        lint_manifest(&manifest, &manifest_path)?;
        fixtures.push(resolve(manifest, dir));
    }

    fixtures.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(fixtures)
}

/// Corpus-level lint for a committed fixture, beyond schema parsing
/// ([`parse_manifest`] stays lenient so unit tests can drive the raw schema).
///
/// A committed `expect = "reject"` fixture MUST localize its rejection precisely,
/// to avoid two footguns (T4/T5 Codex review):
/// - **No localization → masks regressions.** With neither `[expected].stage` nor
///   `[expected].diagnostic`, [`crate::pipeline`]'s `apply_expectation` greens ANY
///   failure — so the fixture later breaking for an unrelated reason still passes.
/// - **Post-emit stage → permanently red.** A `reject` fixture short-circuits
///   after emit, so only `acquire`/`bind`/`emit` can be the rejection point; a
///   `snapshot`/`compile`/`run`/`behavioral` stage could never be reached and the
///   fixture would never go green.
///
/// So a committed reject requires a pre-emit `[expected].stage` AND an
/// `[expected].diagnostic` substring. (`pass`/`panic` fixtures are unaffected.)
fn lint_manifest(manifest: &Manifest, path: &Path) -> Result<(), ManifestError> {
    if manifest.expect != Expect::Reject {
        return Ok(());
    }
    let invalid = |detail: String| ManifestError::Invalid {
        path: path.to_path_buf(),
        detail,
    };
    let stage = manifest.expected.stage.ok_or_else(|| {
        invalid(
            "expect = \"reject\" requires [expected].stage (the pre-emit stage \
             acquire/bind/emit it must fail at)"
                .to_string(),
        )
    })?;
    if !matches!(stage, Stage::Acquire | Stage::Bind | Stage::Emit) {
        return Err(invalid(format!(
            "[expected].stage = {stage:?} is post-emit; a reject fixture \
             short-circuits after emit, so only acquire/bind/emit are valid \
             rejection points"
        )));
    }
    if manifest.expected.diagnostic.is_none() {
        return Err(invalid(
            "expect = \"reject\" requires [expected].diagnostic (a substring of \
             the failure, e.g. a BRM code) so a wrong-reason failure cannot pass"
                .to_string(),
        ));
    }
    Ok(())
}

/// Resolve the sibling file paths for a parsed manifest in `dir`.
fn resolve(manifest: Manifest, dir: PathBuf) -> Fixture {
    let input = match manifest.kind {
        FixtureKind::Rust => dir.join("input.rs"),
        FixtureKind::Ir => dir.join("model.rs"),
    };
    let golden_c = dir.join("golden.c");
    let expected_stdout = dir.join("expected.stdout");
    let expected_stderr = match manifest.expect {
        Expect::Panic => Some(dir.join("expected.stderr")),
        // A reject fixture fails before it runs — no stderr oracle (the rejection
        // is localized by `[expected].stage`/`diagnostic`, not a sibling file).
        Expect::Pass | Expect::Reject => None,
    };
    // The platform file is discovered by presence (unlike the always-joined
    // golden/expected paths): absent = no L2 binding, so every L2-free fixture
    // keeps `None` and the pipeline's bind step is a no-op for it.
    let platform = {
        let p = dir.join("platform.toml");
        p.exists().then_some(p)
    };
    Fixture {
        manifest,
        dir,
        input,
        golden_c,
        expected_stdout,
        expected_stderr,
        platform,
    }
}
