//! The 6-stage per-fixture pipeline (Doc 3 section 1) with per-stage
//! failure attribution: a failure is tagged to the stage it occurred in
//! so a snapshot drift is distinguishable from a behavioral miscompute,
//! a compile error, or an acquire failure.
//!
//! Stages: 1 Acquire (bridge -> validated `RegseqModel`), 2 Emit +
//! entry-shim classification (`regseq_backends::emit_generic_c_for_entry`),
//! 3 Snapshot (byte-exact vs golden), 4 Compile + 5 Run (host `cc`),
//! 6 Behavioral (stdout for a pass fixture, stderr + non-zero exit for a
//! panic fixture).
//!
//! Until tasks b-e implement emit, every `kind = rust` fixture stops at
//! stage 2 with `Fail { Emit, .. }` -- the honest Phase-1 baseline.

use std::path::Path;

use test_ir_fixtures::{try_bridge_fixture_path_with_overrides, BridgeOutcome, BuildOverrides};

use crate::manifest::{Expect, Fixture, FixtureKind};
use crate::{oracle, toolchain};

/// Build the bridge `BuildOverrides` for a fixture from its manifest. Only
/// `overflow_checks` is exposed (absent -> `None` -> the helper's hard-wired
/// `false`, so a fixture that omits it bridges bit-for-bit as before).
fn bridge_overrides(fixture: &Fixture) -> BuildOverrides {
    BuildOverrides {
        overflow_checks: fixture.manifest.overflow_checks,
        ..BuildOverrides::default()
    }
}

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

/// Build the backend codegen options for a fixture from its manifest (RD-8/T27):
/// the manifest `lifecycle_scenario` CSV maps to the `backend.lifecycle.scenario`
/// param, validated by the same [`regseq_backends::BackendOptions`] parser the
/// CLI uses. Absent → the default (all scenarios; no effect on a no-lifecycle
/// fixture).
pub fn backend_options_for(
    fixture: &Fixture,
) -> Result<regseq_backends::BackendOptions, regseq_backends::EmitError> {
    regseq_backends::BackendOptions::from_backend_params(
        fixture
            .manifest
            .lifecycle_scenario
            .iter()
            .map(|csv| ("lifecycle.scenario", csv.as_str())),
    )
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
        FixtureKind::Rust => {
            match try_bridge_fixture_path_with_overrides(&fixture.input, &bridge_overrides(fixture))
            {
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
            }
        }
    };

    // Stage 2: Emit + entry-shim classification (the backend is the single
    // source of truth for the shim shape, contract C04; an unresolved /
    // ambiguous / unsupported entry shape fails loud here at emit, D08/D09).
    // The fixture's device-lifecycle scenario selection (RD-8) rides as a
    // backend param, parallel to the model.
    let backend_options = match backend_options_for(fixture) {
        Ok(options) => options,
        Err(err) => return fail(Stage::Emit, err.to_string()),
    };
    let (emitted, shim) = match regseq_backends::emit_generic_c_for_entry_with(
        &model,
        &fixture.manifest.entry,
        &backend_options,
    ) {
        Ok(emit) => (emit.tu, emit.shim),
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

    // Stages 4-5: Compile + Run (the backend-resolved shim shape, not re-derived).
    let run = match toolchain::compile_and_run(&emitted, &shim, cc) {
        Ok(run) => run,
        Err(toolchain::ToolchainError::CompileFailed { stderr }) => {
            return fail(Stage::Compile, stderr);
        }
        // `compile_and_run` only ever yields `CompileFailed` or `Io`; the `Io`
        // Display is "toolchain I/O error: ...", identical to the prior arm.
        Err(err) => {
            return fail(Stage::Compile, err.to_string());
        }
    };

    // Stage 6: Behavioral oracle.
    match fixture.manifest.expect {
        Expect::Pass => {
            // Stage 5 boundary for a pass fixture: the process must exit with
            // the resolved `expected.exit` (default 0) AND with empty stderr.
            // The symbol is resolved on the SAME cc/libc that ran the fixture,
            // so no numeric lives in the manifest (contract C04); an absent or
            // "0" field resolves to 0 with no probe, preserving exit-0
            // semantics for existing fixtures (invariant I02).
            let exit_symbol = fixture.manifest.expected.exit.as_deref().unwrap_or("0");
            let expected_exit = match toolchain::resolve_errno(exit_symbol, cc) {
                Ok(code) => code,
                Err(err) => {
                    return fail(
                        Stage::Run,
                        format!("cannot resolve expected.exit {exit_symbol:?}: {err}"),
                    );
                }
            };
            if let Err(mismatch) = oracle::behavioral_exit(&run, expected_exit) {
                return fail(Stage::Run, mismatch.summary);
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

/// The outcome of blessing one fixture (Change E): the emitted C was written
/// to `golden.c` (always, when stages 1-2 succeed), plus a report of whether
/// the freshly-blessed emit compiled, ran, and matched the hand-authored
/// `expected.stdout` -- printed for a human, never auto-blessed.
#[derive(Debug)]
pub struct BlessOutcome {
    /// Fixture name.
    pub name: String,
    /// Bytes written to `golden.c`, or `None` if stages 1-2 failed (nothing
    /// was written).
    pub blessed_bytes: Option<usize>,
    /// A human-readable line per fixture (the bless report).
    pub report: String,
    /// Whether the fixture emitted AND (where `expected.stdout` exists)
    /// matched -- the bless run's per-fixture success bit.
    pub ok: bool,
}

/// Bless one fixture (Change E): run stage-1 (acquire) + stage-2 (emit), write
/// the emitted C to `golden.c` (creating / overwriting it), **skip** the
/// stage-3 snapshot compare, then run stages 4-6 in **report mode** (print the
/// actual stdout on a miss so a human can author `expected.stdout`). NEVER
/// writes `expected.stdout` / `expected.stderr` (the honesty invariant): the
/// behavioral oracle stays hand-authored.
pub fn bless_fixture(fixture: &Fixture, cc: &Path) -> BlessOutcome {
    let name = fixture.manifest.name.clone();

    // Stage 1: Acquire.
    let model = match fixture.manifest.kind {
        FixtureKind::Ir => {
            return BlessOutcome {
                name,
                blessed_bytes: None,
                report: "skipped (kind=ir)".to_string(),
                ok: false,
            };
        }
        FixtureKind::Rust => {
            match try_bridge_fixture_path_with_overrides(&fixture.input, &bridge_overrides(fixture))
            {
                BridgeOutcome::Ok(model) => model,
                other => {
                    return BlessOutcome {
                        name,
                        blessed_bytes: None,
                        report: format!("acquire failed: {other:?}"),
                        ok: false,
                    };
                }
            }
        }
    };

    // Stage 2: Emit + entry-shim classification (the backend's single
    // source of truth for the shim shape, contract C04). The fixture's
    // device-lifecycle scenario selection (RD-8) rides as a backend param.
    let backend_options = match backend_options_for(fixture) {
        Ok(options) => options,
        Err(err) => {
            return BlessOutcome {
                name,
                blessed_bytes: None,
                report: format!("backend param error: {err}"),
                ok: false,
            };
        }
    };
    let (emitted, shim) = match regseq_backends::emit_generic_c_for_entry_with(
        &model,
        &fixture.manifest.entry,
        &backend_options,
    ) {
        Ok(emit) => (emit.tu, emit.shim),
        Err(err) => {
            return BlessOutcome {
                name,
                blessed_bytes: None,
                report: format!("emit failed: {err}"),
                ok: false,
            };
        }
    };

    // Write golden.c (the ONLY file --bless ever writes). Stage 3 is skipped.
    if let Err(err) = std::fs::write(&fixture.golden_c, &emitted) {
        return BlessOutcome {
            name,
            blessed_bytes: None,
            report: format!("could not write golden.c: {err}"),
            ok: false,
        };
    }
    let blessed_bytes = Some(emitted.len());

    // Stages 4-6 in REPORT mode. A compile/run miss is reported, never fatal;
    // golden.c is already written for diff review.
    let run = match toolchain::compile_and_run(&emitted, &shim, cc) {
        Ok(run) => run,
        Err(err) => {
            return BlessOutcome {
                name,
                blessed_bytes,
                report: format!(
                    "blessed golden.c ({} bytes); compile/run failed: {err}",
                    emitted.len()
                ),
                ok: false,
            };
        }
    };

    // Stage-5 boundary (report mode): the run must exit with the resolved
    // `expected.exit` (default 0) AND with empty stderr; otherwise it is a
    // broken-run miss -- report it (so a broken emit is not silently blessed),
    // but golden.c is still written for diff review. Honoring `expected.exit`
    // (mirroring `run_fixture`) lets a nonzero-errno fixture -- e.g. the EINVAL
    // RegseqError boundary fixture -- bless cleanly instead of tripping a
    // spurious not-clean-exit miss; the symbol is resolved on the SAME cc/libc
    // that ran the fixture (contract C04, no manifest numeric).
    let exit_symbol = fixture.manifest.expected.exit.as_deref().unwrap_or("0");
    let expected_exit = match toolchain::resolve_errno(exit_symbol, cc) {
        Ok(code) => Some(code),
        Err(err) => {
            return BlessOutcome {
                name,
                blessed_bytes,
                report: format!(
                    "blessed golden.c ({} bytes); cannot resolve expected.exit {exit_symbol:?}: {err}",
                    emitted.len()
                ),
                ok: false,
            };
        }
    };
    if run.exit_code != expected_exit || !run.stderr.is_empty() {
        return BlessOutcome {
            name,
            blessed_bytes,
            report: format!(
                "blessed golden.c ({} bytes); run did not exit as expected (exit={:?}, expected={:?}, stderr={:?}) -- actual stdout:\n{}",
                emitted.len(),
                run.exit_code,
                expected_exit,
                run.stderr,
                run.stdout
            ),
            ok: false,
        };
    }

    // Stage-6 report (do not certify) against the hand-authored expected.stdout.
    // `ok` is true only when the run exited cleanly AND a hand-authored
    // expected.stdout exists AND matches: a missing expected.stdout (a human
    // must still author + verify it) or a mismatch is reported, never blessed.
    let (report, ok) = match std::fs::read_to_string(&fixture.expected_stdout) {
        Ok(expected) if run.stdout == expected => (
            format!(
                "blessed golden.c ({} bytes); stdout matches expected.stdout",
                emitted.len()
            ),
            true,
        ),
        Ok(_) => (
            format!(
                "blessed golden.c ({} bytes); stdout MISMATCH -- actual stdout:\n{}",
                emitted.len(),
                run.stdout
            ),
            false,
        ),
        Err(_) => (
            format!(
                "blessed golden.c ({} bytes); no expected.stdout yet -- actual stdout:\n{}",
                emitted.len(),
                run.stdout
            ),
            false,
        ),
    };
    BlessOutcome {
        name,
        blessed_bytes,
        report,
        ok,
    }
}
