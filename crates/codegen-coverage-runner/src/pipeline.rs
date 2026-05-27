//! The 6-stage per-fixture pipeline (Doc 3 section 1) with per-stage
//! failure attribution: a failure is tagged to the stage it occurred in
//! so a snapshot drift is distinguishable from a behavioral miscompute,
//! a compile error, or an acquire failure.
//!
//! Stages: 1 Acquire (bridge -> validated `RegseqModel`), 2 Emit
//! (`regseq_backends::emit_generic_c`), 3 Snapshot (byte-exact vs
//! golden), 4 Compile + 5 Run (host `cc`), 6 Behavioral (stdout for a
//! pass fixture, stderr + non-zero exit for a panic fixture).
//!
//! Until tasks b-e implement emit, every `kind = rust` fixture stops at
//! stage 2 with `Fail { Emit, .. }` -- the honest Phase-1 baseline.

use std::path::Path;

use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

use crate::manifest::{Expect, Fixture, FixtureKind};
use crate::{oracle, toolchain};

/// The pipeline stage a fixture failure is attributed to.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Stage {
    /// Stage 1: acquire a validated `RegseqModel` (bridge).
    Acquire,
    /// Stage 2: emit the generic-C translation unit.
    Emit,
    /// Stage 3: snapshot oracle (emitted C vs golden C).
    Snapshot,
    /// Stage 4: host `cc` compile.
    Compile,
    /// Stage 5: run the linked binary.
    Run,
    /// Stage 6: behavioral oracle (stdout, or panic stderr).
    Behavioral,
}

/// Per-fixture verdict with stage-tagged failure attribution.
#[derive(Debug)]
pub enum FixtureVerdict {
    /// All in-scope stages held.
    Pass,
    /// A stage failed; `stage` localizes it and `detail` explains.
    Fail {
        /// The stage that failed.
        stage: Stage,
        /// Human-readable failure detail.
        detail: String,
    },
    /// The fixture was not executed (e.g. `kind = ir`, or a width gate);
    /// `reason` records why.
    Skipped {
        /// Why the fixture was skipped.
        reason: String,
    },
}

/// The outcome of running one fixture through the pipeline.
#[derive(Debug)]
pub struct FixtureResult {
    /// Fixture name (from the manifest).
    pub name: String,
    /// Doc-1 row IDs this fixture covers (from the manifest).
    pub covers: Vec<String>,
    /// The per-fixture verdict.
    pub verdict: FixtureVerdict,
}

impl FixtureResult {
    /// Whether this fixture passed (a "green" fixture for ledger
    /// aggregation).
    pub fn is_green(&self) -> bool {
        matches!(self.verdict, FixtureVerdict::Pass)
    }

    /// Whether this fixture FAILed (as opposed to passing or skipping).
    pub fn is_fail(&self) -> bool {
        matches!(self.verdict, FixtureVerdict::Fail { .. })
    }
}

/// Run a single fixture through stages 1-6, returning a stage-attributed
/// [`FixtureResult`]. `cc` is the host C compiler for stages 4-5.
pub fn run_fixture(fixture: &Fixture, cc: &Path) -> FixtureResult {
    let name = fixture.manifest.name.clone();
    let covers = fixture.manifest.covers.clone();
    let verdict = run_stages(fixture, cc);
    FixtureResult {
        name,
        covers,
        verdict,
    }
}

/// The stage wiring proper, returning the [`FixtureVerdict`].
fn run_stages(fixture: &Fixture, cc: &Path) -> FixtureVerdict {
    // Stage 1: Acquire.
    let model = match fixture.manifest.kind {
        FixtureKind::Ir => {
            return FixtureVerdict::Skipped {
                reason: "kind=ir unsupported in Phase-1 task a".to_string(),
            };
        }
        FixtureKind::Rust => match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(model) => model,
            BridgeOutcome::ValidatorFailed { violation, .. } => {
                return fail(Stage::Acquire, format!("validator failed: {violation:?}"));
            }
            BridgeOutcome::Rejected(err) => {
                return fail(Stage::Acquire, format!("bridge rejected: {err}"));
            }
            BridgeOutcome::RustcRejected { stderr } => {
                return fail(
                    Stage::Acquire,
                    format!("rustc rejected before after_analysis: {stderr}"),
                );
            }
        },
    };

    // Stage 2: Emit.
    let emitted = match regseq_backends::emit_generic_c(&model) {
        Ok(c) => c,
        Err(err) => return fail(Stage::Emit, err.to_string()),
    };

    // Stage 3: Snapshot oracle (emitted C vs golden C).
    let golden_c = match std::fs::read_to_string(&fixture.golden_c) {
        Ok(text) => text,
        Err(err) => {
            return fail(
                Stage::Snapshot,
                format!("cannot read golden {}: {err}", fixture.golden_c.display()),
            );
        }
    };
    if let Err(mismatch) = oracle::snapshot(&emitted, &golden_c) {
        return fail(Stage::Snapshot, mismatch.summary);
    }

    // Stages 4-5: Compile + Run.
    let run = match toolchain::compile_and_run(&emitted, &fixture.manifest.entry, cc) {
        Ok(run) => run,
        Err(toolchain::ToolchainError::CompileFailed { stderr }) => {
            return fail(Stage::Compile, stderr);
        }
        Err(toolchain::ToolchainError::Io(err)) => {
            return fail(Stage::Compile, format!("toolchain I/O error: {err}"));
        }
    };

    // Stage 6: Behavioral oracle.
    match fixture.manifest.expect {
        Expect::Pass => {
            // Stage 5 boundary for a pass fixture: clean exit, no panic.
            if run.exit_code != Some(0) {
                return fail(
                    Stage::Run,
                    format!("expected clean exit 0, got exit_code {:?}", run.exit_code),
                );
            }
            if !run.stderr.is_empty() {
                return fail(
                    Stage::Run,
                    format!("expected empty stderr, got {:?}", run.stderr),
                );
            }
            let golden_stdout = match std::fs::read_to_string(&fixture.expected_stdout) {
                Ok(text) => text,
                Err(err) => {
                    return fail(
                        Stage::Behavioral,
                        format!("cannot read {}: {err}", fixture.expected_stdout.display()),
                    );
                }
            };
            if let Err(mismatch) = oracle::behavioral_stdout(&run.stdout, &golden_stdout) {
                return fail(Stage::Behavioral, mismatch.summary);
            }
        }
        Expect::Panic => {
            let stderr_path = fixture
                .expected_stderr
                .as_ref()
                .expect("panic fixture must resolve an expected.stderr path");
            let golden_stderr = match std::fs::read_to_string(stderr_path) {
                Ok(text) => text,
                Err(err) => {
                    return fail(
                        Stage::Behavioral,
                        format!("cannot read {}: {err}", stderr_path.display()),
                    );
                }
            };
            if let Err(mismatch) = oracle::behavioral_panic(&run, &golden_stderr) {
                return fail(Stage::Behavioral, mismatch.summary);
            }
        }
    }

    FixtureVerdict::Pass
}

/// Build a `Fail` verdict for `stage` with `detail`.
fn fail(stage: Stage, detail: String) -> FixtureVerdict {
    FixtureVerdict::Fail { stage, detail }
}
