/// Parsed command-line arguments for the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverArgs {
    /// Path to the rustc binary (from RUSTC_WRAPPER invocation).
    /// None if invoked directly (not as a wrapper).
    pub rustc_path: Option<String>,
    /// Arguments to pass to rustc.
    pub rustc_args: Vec<String>,
}

impl DriverArgs {
    /// Parse process arguments (including argv\[0\]) into driver arguments.
    pub fn parse_from(args: &[String]) -> Self {
        let mut iter = args.iter();
        let _binary_name = iter.next();
        let Some(first) = iter.next() else {
            return Self {
                rustc_path: None,
                rustc_args: Vec::new(),
            };
        };

        if first.starts_with('-') {
            let mut rustc_args = Vec::with_capacity(args.len().saturating_sub(1));
            rustc_args.push(first.clone());
            rustc_args.extend(iter.cloned());
            Self {
                rustc_path: None,
                rustc_args,
            }
        } else {
            Self {
                rustc_path: Some(first.clone()),
                rustc_args: iter.cloned().collect(),
            }
        }
    }

    /// Determine whether the current invocation should run passthrough rustc
    /// behavior or the regseq pipeline.
    pub fn dispatch(&self) -> DriverMode {
        DispatchSignals::from_env(&self.rustc_args).mode()
    }
}

/// Driver execution mode selected by dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverMode {
    /// Forward to rustc unchanged (cargo probe, dependency, etc.).
    Passthrough,
    /// Apply regseq MIR shape settings, but let rustc proceed to codegen
    /// normally (no `no_codegen`, no `after_analysis` extraction).
    /// Used for the regseq facade and any future regseq-aware crates so
    /// cross-crate MIR consumed by the spec extractor has a known shape.
    RegseqAware,
    /// Run the regseq compilation pipeline.
    Regseq,
}

/// Signals used to select the driver execution mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchSignals {
    /// Whether arguments contain cargo probe flags (`--print*` or `-vV`).
    pub has_print_query: bool,
    /// Whether arguments contain `--target`.
    pub has_target: bool,
    /// Whether `CARGO_PRIMARY_PACKAGE` is present in the environment.
    pub has_cargo_primary_package: bool,
    /// Whether `REGSEQ_OPTIONS` is present in the environment.
    pub has_regseq_options: bool,
    /// Value of `CARGO_PKG_NAME` (set by cargo for every rustc invocation
    /// it spawns). `None` when the env var is missing — typically a
    /// direct, non-cargo rustc invocation.
    pub current_pkg_name: Option<String>,
    /// Comma-split values from `REGSEQ_AWARE_CRATES`. `None` when the
    /// env var is absent. An empty `Vec` (env present but empty / only
    /// commas) means "no aware crates declared", distinguishable from
    /// `None`.
    pub regseq_aware_crates: Option<Vec<String>>,
}

impl DispatchSignals {
    /// Build dispatch signals from rustc arguments and explicit env-presence values.
    pub fn from_rustc_args(
        rustc_args: &[String],
        has_cargo_primary_package: bool,
        has_regseq_options: bool,
        current_pkg_name: Option<String>,
        regseq_aware_crates: Option<Vec<String>>,
    ) -> Self {
        Self {
            has_print_query: has_print_query(rustc_args),
            has_target: has_arg(rustc_args, "--target"),
            has_cargo_primary_package,
            has_regseq_options,
            current_pkg_name,
            regseq_aware_crates,
        }
    }

    /// Build dispatch signals from rustc arguments and process environment.
    pub fn from_env(rustc_args: &[String]) -> Self {
        Self::from_rustc_args(
            rustc_args,
            std::env::var_os("CARGO_PRIMARY_PACKAGE").is_some(),
            std::env::var_os("REGSEQ_OPTIONS").is_some(),
            std::env::var("CARGO_PKG_NAME").ok(),
            std::env::var("REGSEQ_AWARE_CRATES")
                .ok()
                .map(|raw| parse_aware_crates(&raw)),
        )
    }

    /// Resolve the final driver mode from dispatch signals.
    pub fn mode(&self) -> DriverMode {
        if self.has_print_query {
            return DriverMode::Passthrough;
        }
        if !self.has_target {
            return DriverMode::Passthrough;
        }
        if self.has_cargo_primary_package && self.has_regseq_options {
            return DriverMode::Regseq;
        }
        if let (Some(name), Some(aware)) = (&self.current_pkg_name, &self.regseq_aware_crates) {
            if aware.iter().any(|n| n == name) {
                return DriverMode::RegseqAware;
            }
        }
        DriverMode::Passthrough
    }
}

/// Parse the comma-separated `REGSEQ_AWARE_CRATES` env value into a
/// list of crate names. Empty entries (e.g. trailing commas) are
/// dropped; whitespace around names is trimmed.
fn parse_aware_crates(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Check if an argument flag is present (e.g., "--target", "-vV").
pub fn has_arg(args: &[String], flag: &str) -> bool {
    let with_equals = format!("{flag}=");
    args.iter()
        .any(|arg| arg == flag || arg.starts_with(&with_equals))
}

/// Get the value of an argument (supports --arg=value and --arg value).
pub fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let with_equals = format!("{flag}=");
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&with_equals) {
            return Some(value);
        }
        if arg == flag {
            return args.get(index + 1).map(String::as_str);
        }
    }
    None
}

fn has_print_query(args: &[String]) -> bool {
    has_arg(args, "-vV") || has_arg(args, "--print")
}
