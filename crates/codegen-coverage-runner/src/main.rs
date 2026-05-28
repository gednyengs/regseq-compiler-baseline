//! `codegen-coverage-runner` binary entry point.
//!
//! Mode-A CLI per Doc 3 section 4:
//!
//! ```text
//! codegen-coverage-runner \
//!   --fixtures <dir>      (default crates/codegen-coverage-runner/fixtures/)
//!   --matrix <path>       (default the Doc-1 matrix md)
//!   --ledger-out <dir>    (default .harness/generic-c/)
//!   --cc <path>           (default cc)
//!   --bless               (off; human-invoked golden regen, inert this task)
//! ```
//!
//! Run loop: parse the matrix, discover fixtures, run each through the
//! 6-stage pipeline, aggregate into the ledger, write the ledger, print a
//! summary, and exit 0 iff the verdict is `pass` (and no fixture FAILed),
//! else 1. Config errors (bad matrix / discovery / ledger write) exit 2.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` are
//! inherited from the bridge-runner precedent: stage 1 links the
//! rustc-backed bridge, so the final binary must link the rustc dylibs
//! (the sysroot rpath is supplied by `build.rs`).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;
use std::process::ExitCode;

use codegen_coverage_runner::ledger::{self, PhaseVerdict};
use codegen_coverage_runner::manifest;
use codegen_coverage_runner::matrix;
use codegen_coverage_runner::pipeline;

/// Parsed CLI arguments with Doc-3 defaults applied.
struct Args {
    fixtures: PathBuf,
    matrix: PathBuf,
    ledger_out: PathBuf,
    cc: PathBuf,
    bless: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            fixtures: PathBuf::from("crates/codegen-coverage-runner/fixtures/"),
            matrix: PathBuf::from(
                "docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md",
            ),
            ledger_out: PathBuf::from(".harness/generic-c/"),
            cc: PathBuf::from("cc"),
            bless: false,
        }
    }
}

/// Hand-rolled arg parser (no `clap`, per the bridge-runner precedent).
fn parse_args<I: IntoIterator<Item = String>>(iter: I) -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = iter.into_iter();
    let _exe = iter.next();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fixtures" => args.fixtures = PathBuf::from(value(&mut iter, "--fixtures")?),
            "--matrix" => args.matrix = PathBuf::from(value(&mut iter, "--matrix")?),
            "--ledger-out" => args.ledger_out = PathBuf::from(value(&mut iter, "--ledger-out")?),
            "--cc" => args.cc = PathBuf::from(value(&mut iter, "--cc")?),
            "--bless" => args.bless = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(args)
}

/// Pull the next value for a flag, or report a missing-value error.
fn value<I: Iterator<Item = String>>(iter: &mut I, flag: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args()) {
        Ok(a) => a,
        Err(err) => {
            eprintln!("argument error: {err}");
            return ExitCode::from(2);
        }
    };

    let rows = match matrix::parse(&args.matrix) {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("failed to parse matrix: {err}");
            return ExitCode::from(2);
        }
    };

    let fixtures = match manifest::discover(&args.fixtures) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("failed to discover fixtures: {err}");
            return ExitCode::from(2);
        }
    };

    if args.bless {
        // --bless regenerates `golden.c` from the actual emit (Change E):
        // per fixture run stages 1-2, (over)write golden.c, then run stages
        // 4-6 in report mode. It NEVER writes expected.stdout/stderr -- the
        // behavioral oracle stays hand-authored (Doc 3 section 2 honesty
        // invariant). No ledger is written in bless mode.
        let mut all_ok = true;
        for fixture in &fixtures {
            let outcome = pipeline::bless_fixture(fixture, &args.cc);
            println!("{}: {}", outcome.name, outcome.report);
            all_ok &= outcome.ok;
        }
        return if all_ok {
            ExitCode::from(0)
        } else {
            ExitCode::from(1)
        };
    }

    let toolchain = ledger::probe_toolchain(&args.cc);

    let results: Vec<pipeline::FixtureResult> = fixtures
        .iter()
        .map(|fixture| pipeline::run_fixture(fixture, &args.cc))
        .collect();

    let ledger = match ledger::build(&rows, &results, toolchain) {
        Ok(l) => l,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(2);
        }
    };

    if let Err(err) = ledger::write_ledger(&ledger, &args.ledger_out) {
        eprintln!("failed to write ledger: {err}");
        return ExitCode::from(2);
    }

    let any_fail = results.iter().any(|r| r.is_fail());
    print_summary(&ledger, &results, &args.ledger_out);

    match ledger.verdict {
        PhaseVerdict::Pass if !any_fail => ExitCode::from(0),
        _ => ExitCode::from(1),
    }
}

/// Print a one-screen run summary to stdout.
fn print_summary(
    ledger: &ledger::Ledger,
    results: &[pipeline::FixtureResult],
    ledger_out: &std::path::Path,
) {
    let verdict = match ledger.verdict {
        PhaseVerdict::Pass => "pass",
        PhaseVerdict::NotPass => "not-pass",
    };
    let green = results.iter().filter(|r| r.is_green()).count();
    let failed = results.iter().filter(|r| r.is_fail()).count();
    println!("codegen-coverage-runner");
    println!("  matrix rows:   {}", ledger.rows.len());
    println!(
        "  fixtures:      {} ({green} green, {failed} failed)",
        results.len()
    );
    println!(
        "  drift:         {} covered-without-green, {} green-without-covered",
        ledger.drift.covered_without_green.len(),
        ledger.drift.green_without_covered.len()
    );
    println!(
        "  toolchain:     {} ({})",
        ledger.toolchain.cc, ledger.toolchain.cc_version
    );
    println!("  ledger:        {}", ledger_out.display());
    println!("  verdict:       {verdict}");
}
