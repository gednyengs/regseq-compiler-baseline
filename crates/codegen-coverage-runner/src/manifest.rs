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
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io(err) => write!(f, "fixture discovery I/O error: {err}"),
            ManifestError::Parse { path, err } => {
                write!(f, "failed to parse {}: {err}", path.display())
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
        fixtures.push(resolve(manifest, dir));
    }

    fixtures.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(fixtures)
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
        Expect::Pass => None,
    };
    Fixture {
        manifest,
        dir,
        input,
        golden_c,
        expected_stdout,
        expected_stderr,
    }
}
