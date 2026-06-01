use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Parser;
use regseq_config::{
    validate_dotted_ident, ColorChoice, CompilerOptions, DumpStage, EmitSpec, LogLevel,
    RegseqParam, TargetOs,
};

use crate::Error;

/// Cargo wrapper parser for `cargo regseq`.
#[derive(Debug, Parser)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum CargoCli {
    /// Regseq compiler driver.
    Regseq(Cli),
}

/// `cargo regseq` command-line options.
#[derive(Debug, Parser)]
pub struct Cli {
    /// Target OS for code generation.
    #[arg(long = "target-os")]
    pub target_os: TargetOs,

    /// Path to the platform file (.toml).
    #[arg(long = "platform-file")]
    pub platform_file: PathBuf,

    /// Output directory for generated artifacts (default: ./regseq-out/).
    #[arg(long = "output-dir", default_value = "./regseq-out/")]
    pub output_dir: PathBuf,

    /// What to emit (repeatable; default: model). Supported specs:
    /// model[=PATH], json[=PATH], generic-c[=PATH], bin[=NAME],
    /// driver[=MATCH_ID], analysis[.NAME][=PATH].
    #[arg(long = "emit", value_name = "SPEC")]
    pub emit: Vec<EmitSpec>,

    /// Validate without writing output files.
    #[arg(long = "check")]
    pub check: bool,

    /// Additional analyses to run (repeatable).
    #[arg(short = 'a', long = "add-analysis", value_name = "NAME")]
    pub add_analysis: Vec<String>,

    /// Analyses to skip from the default set (repeatable).
    #[arg(short = 's', long = "skip-analysis", value_name = "NAME")]
    pub skip_analysis: Vec<String>,

    /// Treat this diagnostic as an error (repeatable).
    #[arg(long = "deny", value_name = "NAME")]
    pub deny: Vec<String>,

    /// Suppress this diagnostic (repeatable).
    #[arg(long = "allow", value_name = "NAME")]
    pub allow: Vec<String>,

    /// Key-value parameters for compiler and backends (repeatable).
    #[arg(short = 'P', long = "param", value_name = "KEY=VALUE")]
    pub params: Vec<RegseqParam>,

    /// Set log level (default: warn).
    #[arg(long = "log-level", default_value_t = LogLevel::Warn)]
    pub log_level: LogLevel,

    /// Enable verbose output (shorthand for --log-level debug).
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Dump compiler state at a pipeline stage.
    #[arg(long = "dump-stage")]
    pub dump_stage: Option<DumpStage>,

    /// Color output (always, auto, never; default: auto).
    #[arg(long = "color", default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,

    /// Extra arguments passed through to cargo check.
    #[arg(last = true)]
    pub cargo_args: Vec<String>,
}

impl Cli {
    /// Validate CLI structure and argument relationships.
    pub fn validate(&self) -> Result<(), Error> {
        if !self.platform_file.exists() || !self.platform_file.is_file() {
            return Err(Error::Validation(format!(
                "platform file '{}' does not exist or is not a regular file",
                self.platform_file.display()
            )));
        }

        if self.output_dir.exists() && self.output_dir.is_file() {
            return Err(Error::Validation(format!(
                "output directory '{}' points to a regular file",
                self.output_dir.display()
            )));
        }

        let conflicting_analyses =
            conflicting_names(self.add_analysis.iter(), self.skip_analysis.iter());
        if !conflicting_analyses.is_empty() {
            return Err(Error::Validation(format!(
                "analysis names cannot appear in both --add-analysis and --skip-analysis: {}",
                conflicting_analyses.join(", ")
            )));
        }

        let conflicting_diagnostics = conflicting_names(self.deny.iter(), self.allow.iter());
        if !conflicting_diagnostics.is_empty() {
            return Err(Error::Validation(format!(
                "diagnostic names cannot appear in both --deny and --allow: {}",
                conflicting_diagnostics.join(", ")
            )));
        }

        for name in self
            .add_analysis
            .iter()
            .chain(self.skip_analysis.iter())
            .chain(self.deny.iter())
            .chain(self.allow.iter())
        {
            if let Err(reason) = validate_dotted_ident(name) {
                return Err(Error::Validation(format!(
                    "invalid dotted identifier '{}': {}",
                    name, reason
                )));
            }
        }

        for spec in &self.emit {
            if let EmitSpec::Analysis {
                analysis_name: Some(name),
                ..
            } = spec
            {
                if let Err(reason) = validate_dotted_ident(name) {
                    return Err(Error::Validation(format!(
                        "invalid analysis name in --emit '{}': {}",
                        name, reason
                    )));
                }
            }
        }

        Ok(())
    }

    /// Resolve effective log level from `--log-level` and `--verbose`.
    pub fn resolved_log_level(&self) -> LogLevel {
        if self.verbose && self.log_level == LogLevel::Warn {
            LogLevel::Debug
        } else {
            self.log_level
        }
    }

    /// Convert parsed CLI arguments into serializable compiler options.
    pub fn into_compiler_options(self) -> CompilerOptions {
        let Cli {
            target_os,
            platform_file,
            output_dir,
            mut emit,
            check,
            add_analysis,
            skip_analysis,
            deny,
            allow,
            params,
            log_level,
            verbose,
            dump_stage,
            color,
            cargo_args: _cargo_args,
        } = self;

        if emit.is_empty() {
            // Default emit during the RegseqModel-only rebuild: produce
            // the pretty-printed RegseqModel text. Pass `--emit json`
            // explicitly for the machine-readable form. Driver and
            // Analysis emit are stubbed in regseq-driver until backends
            // and analyses are wired in.
            emit.push(EmitSpec::Model { path: None });
        }
        emit.dedup();

        let resolved_log_level = if verbose && log_level == LogLevel::Warn {
            LogLevel::Debug
        } else {
            log_level
        };

        CompilerOptions {
            target_os,
            platform_file: std::fs::canonicalize(&platform_file).unwrap_or(platform_file),
            output_dir: std::path::absolute(&output_dir).unwrap_or(output_dir),
            emit,
            check,
            add_analysis,
            skip_analysis,
            deny,
            allow,
            params,
            log_level: resolved_log_level,
            dump_stage,
            color,
        }
    }
}

fn conflicting_names<'a, I, J>(lhs: I, rhs: J) -> Vec<String>
where
    I: IntoIterator<Item = &'a String>,
    J: IntoIterator<Item = &'a String>,
{
    let left: BTreeSet<&str> = lhs.into_iter().map(String::as_str).collect();
    let right: BTreeSet<&str> = rhs.into_iter().map(String::as_str).collect();
    left.intersection(&right)
        .map(|name| (*name).to_string())
        .collect()
}
