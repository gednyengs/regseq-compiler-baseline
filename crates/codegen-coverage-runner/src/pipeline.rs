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

/// Stage 1b (T2): bind the fixture's L2 platform surface into the bridged model
/// and validate the joined L1+L2 surface, mirroring the production post-bridge
/// "1b" seam in `regseq-driver`'s `callbacks.rs` (~L214–250).
///
/// Two phases, deliberately split so the harness matches production's semantics
/// for BOTH the platform-present and platform-absent cases:
/// - **Bind** (only when the fixture carries a sibling `platform.toml`): parse it
///   (`regseq_config::parse_platform_bindings_path`) → run every registered L2
///   binder over the model (`regseq_l2::bind_all`). An absent file (`None`) is
///   production's "unreadable/absent platform file" path (`callbacks.rs:220`):
///   no bindings to join.
/// - **Validate** (ALWAYS, both cases): `regseq_l2::validate_l2_surface`
///   (`callbacks.rs:237`) → re-run the core `model.validate` on the model
///   (`callbacks.rs:246`). Production runs these for *every* bus-declared driver
///   regardless of whether a platform file existed — so a bus-managed region with
///   no resolvable `(base, size)` is rejected (BRM-006) here too, instead of
///   false-greening. For a non-bus fixture `validate_l2_surface` is a no-op, so
///   this stays zero-churn for the L2-free corpus.
///
/// Re-validation note: the bridge helper validated the model *pre*-join
/// (`test-ir-fixtures:468`); this re-runs `model.validate` *post*-join for
/// fidelity with production. `BridgeInput::matching` reconstructs the validator
/// input from the model itself, exactly as production does. Idempotent on an
/// unbound model — the core validator checks no bus facts (those live in
/// `MmioBinding`, checked by `validate_l2_surface`).
///
/// On any failure returns the human-readable detail; the caller attributes it to
/// [`Stage::Bind`].
///
/// `pub` so a fixture's `BLESS` path can apply the identical bind before emit
/// (the keystone e2e gate), exactly as `run_fixture` does internally — keeping
/// the blessed golden and the verified golden on one bind implementation.
pub fn bind_l2_platform(
    model: &mut regseq_model::RegseqModel,
    fixture: &Fixture,
) -> Result<regseq_model::PlatformIdentity, String> {
    // Parse the platform file ONCE (Codex A1): bind `[mmio.<id>]` only when a file
    // is present (an absent file mirrors production's no-bindings path), and
    // resolve the `[platform]` identity (os/board/arch) — returned for the
    // `BackendContext` the caller builds at emit (T7). Absent file → default
    // (empty) identity.
    let mut identity = regseq_model::PlatformIdentity::default();
    if let Some(platform_path) = fixture.platform.as_deref() {
        let file = regseq_config::parse_platform_file_path(platform_path)
            .map_err(|err| format!("platform file {}: {err}", platform_path.display()))?;
        regseq_l2::bind_all(model, &file.bindings)
            .map_err(|err| format!("[{}] {err}", err.code()))?;
        identity = regseq_l2::resolve_platform_identity(&file.identity)
            .map_err(|err| format!("[{}] {err}", err.code()))?;
    }
    // Validate the L2 surface + re-validate the (possibly bound) model for EVERY
    // fixture, exactly as production does for every bus-declared driver.
    regseq_l2::validate_l2_surface(model).map_err(|err| format!("[{}] {err}", err.code()))?;
    let bridge_input = regseq_model::BridgeInput::matching(model);
    model
        .validate(&bridge_input)
        .map_err(|violation| format!("post-bind model.validate failed: {violation:?}"))?;
    Ok(identity)
}

/// The pipeline stage a fixture failure is attributed to. Deserializable so a
/// `reject` fixture's `[expected] stage = "bind"` can name the stage it must fail
/// at (the lowercase variant name).
#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Stage 1: acquire a validated `RegseqModel` (bridge).
    Acquire,
    /// Stage 1b: bind the L2 platform surface (`platform.toml`) into the model
    /// and validate the joined L1+L2 surface (T2). Only reached for a fixture
    /// that carries a sibling `platform.toml`; an L2-free fixture skips it.
    Bind,
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
    let verdict = apply_expectation(fixture, run_stages(fixture, cc));
    FixtureResult {
        name,
        covers,
        verdict,
    }
}

/// Fold a fixture's `expect` policy over the raw stage verdict (T4). For
/// `pass`/`panic` the raw verdict already encodes the outcome (the stage wiring
/// runs the behavioral oracle), so it passes through unchanged. For `reject` the
/// polarity inverts: the fixture is GREEN iff the pipeline FAILED at the expected
/// stage with the expected diagnostic substring, and RED if it was accepted
/// (raw `Pass`, i.e. reached emit) or failed at a different stage / without the
/// diagnostic. A `Skipped` raw verdict passes through (a width/kind gate is
/// orthogonal to the expectation).
fn apply_expectation(fixture: &Fixture, raw: FixtureVerdict) -> FixtureVerdict {
    if fixture.manifest.expect != Expect::Reject {
        return raw;
    }
    let expected = &fixture.manifest.expected;
    match raw {
        FixtureVerdict::Skipped { reason } => FixtureVerdict::Skipped { reason },
        // Accepted through emit — the rejection never happened.
        FixtureVerdict::Pass => fail(
            expected.stage.unwrap_or(Stage::Bind),
            format!(
                "expected rejection{}{}, but the fixture was accepted through emit",
                expected
                    .stage
                    .map(|s| format!(" at {s:?}"))
                    .unwrap_or_default(),
                expected
                    .diagnostic
                    .as_deref()
                    .map(|d| format!(" with {d}"))
                    .unwrap_or_default(),
            ),
        ),
        FixtureVerdict::Fail { stage, detail } => {
            // Wrong stage → the fixture failed, but not where it was meant to.
            if let Some(want) = expected.stage {
                if stage != want {
                    return fail(
                        stage,
                        format!("expected rejection at {want:?}, but failed at {stage:?}: {detail}"),
                    );
                }
            }
            // Missing diagnostic substring → failed at the right stage for the
            // wrong reason.
            if let Some(want) = expected.diagnostic.as_deref() {
                if !detail.contains(want) {
                    return fail(
                        stage,
                        format!("expected diagnostic {want:?} not found in failure: {detail}"),
                    );
                }
            }
            // Failed exactly as expected → a green reject.
            FixtureVerdict::Pass
        }
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
    let mut model = match fixture.manifest.kind {
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

    // Stage 1b: L2 platform binding (T2). Bind `platform.toml` into the bridged
    // model BEFORE emit, so the bound model is the artifact the backend consumes
    // (the goldens then capture L2 correctness for free). No-op for an L2-free
    // fixture; a parse / BRM / re-validate failure fails loud here, tagged Bind.
    let platform_identity = match bind_l2_platform(&mut model, fixture) {
        Ok(identity) => identity,
        Err(detail) => return fail(Stage::Bind, detail),
    };

    // Stage 2: Emit + entry-shim classification (the backend is the single
    // source of truth for the shim shape, contract C04; an unresolved /
    // ambiguous / unsupported entry shape fails loud here at emit, D08/D09).
    // The fixture's device-lifecycle scenario selection (RD-8) rides as a
    // backend param; the `[platform]` identity (T7) rides the BackendContext —
    // both parallel to the model, mirroring production's `callbacks.rs` seam.
    let backend_options = match backend_options_for(fixture) {
        Ok(options) => options,
        Err(err) => return fail(Stage::Emit, err.to_string()),
    };
    let backend_context = regseq_backends::BackendContext::new(backend_options, platform_identity);
    let (emitted, shim) = match regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &backend_context,
    ) {
        Ok(emit) => (emit.tu, emit.shim),
        Err(err) => return fail(Stage::Emit, err.to_string()),
    };

    // A `reject` fixture was EXPECTED to fail at acquire/bind/emit. Reaching here
    // means emit accepted it — the rejection did not happen. Stop before the
    // snapshot/compile/run stages (a reject fixture has no golden / behavioral
    // oracle); `apply_expectation` turns this "accepted through emit" `Pass` into
    // a Fail.
    if fixture.manifest.expect == Expect::Reject {
        return FixtureVerdict::Pass;
    }

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
        // A reject fixture never reaches stage 3+ (it short-circuits after emit
        // above), so this point is unreachable for it.
        Expect::Reject => unreachable!("reject fixtures short-circuit after emit"),
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
    let mut model = match fixture.manifest.kind {
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

    // Stage 1b: L2 platform binding (T2) — same join as run_stages, so the
    // blessed golden.c reflects the BOUND model (no hand-set binding).
    let platform_identity = match bind_l2_platform(&mut model, fixture) {
        Ok(identity) => identity,
        Err(detail) => {
            return BlessOutcome {
                name,
                blessed_bytes: None,
                report: format!("L2 bind failed: {detail}"),
                ok: false,
            };
        }
    };

    // Stage 2: Emit + entry-shim classification (the backend's single
    // source of truth for the shim shape, contract C04). The fixture's
    // device-lifecycle scenario selection (RD-8) rides as a backend param; the
    // `[platform]` identity (T7) rides the BackendContext, so the blessed
    // golden.c reflects the traced identity too.
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
    let backend_context = regseq_backends::BackendContext::new(backend_options, platform_identity);
    let (emitted, shim) = match regseq_backends::emit_generic_c_for_entry_with_context(
        &model,
        &fixture.manifest.entry,
        &backend_context,
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
