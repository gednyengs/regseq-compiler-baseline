/// MIR pass whitelist for Regseq compilation.
///
/// Returns the `mir_enable_passes` value used by both
/// `RegseqCallbacks::config` and `RegseqAwareCallbacks::config`. The two
/// callback types MUST share this whitelist so the cleanup-pass set is
/// identical across the primary spec and any regseq-aware crate (e.g.,
/// the `regseq` facade). Drift between them would mean the bridge
/// consumes MIR with one shape from the spec and a different shape from
/// upstream regseq-aware crates -- a recipe for hard-to-debug
/// recognition mismatches.
///
/// Cleanup passes (force-on; off at `mir_opt_level=0` by default):
/// - `RemoveStorageMarkers`  -- strip `StorageLive` / `StorageDead`
/// - `RemoveZsts`            -- elide zero-sized-type writes
/// - `SimplifyLocals-final`  -- compact unused local decls
/// - `ReorderBasicBlocks`    -- reverse-postorder block IDs (RPO)
/// - `ReorderLocals`         -- renumber locals in encounter order
///
/// UB-check passes (force-off; driver specs do not get instrumentation):
/// - `CheckAlignment`, `CheckNull`, `CheckEnums`
///
/// This whitelist is the canonical "clean MIR" shape and matches the
/// flag set in `scripts/dump-fixture-mir.sh`. The bridge consumes MIR
/// in this shape. Per Q6.
pub fn regseq_mir_passes() -> Vec<(String, bool)> {
    vec![
        ("RemoveStorageMarkers".to_string(), true),
        ("RemoveZsts".to_string(), true),
        ("SimplifyLocals-final".to_string(), true),
        ("ReorderBasicBlocks".to_string(), true),
        ("ReorderLocals".to_string(), true),
        ("CheckAlignment".to_string(), false),
        ("CheckNull".to_string(), false),
        ("CheckEnums".to_string(), false),
    ]
}

/// Passthrough callbacks that leave rustc behavior unchanged. Used for
/// cargo probes (`--print*`, `-vV`), dependencies that are not regseq-aware,
/// and anything else outside the regseq pipeline.
pub struct PassthroughCallbacks;

impl rustc_driver::Callbacks for PassthroughCallbacks {}

/// Callbacks for regseq-aware crates (the `regseq` facade and any
/// future regseq-aware extensions). Applies the same MIR shape contract
/// as `RegseqCallbacks` so cross-crate MIR consumed by the bridge has a
/// known shape, but does NOT set `no_codegen` and does NOT override
/// `after_analysis` -- rustc must produce normal `rmeta` / `rlib`
/// outputs because downstream specs link against them.
///
/// This struct deliberately has no fields and no constructor: its
/// behavior is fully determined by the global MIR shape contract
/// (`regseq_mir_passes`), so there is nothing to parameterize. If a
/// future regseq-aware crate needs invocation-specific configuration,
/// add it here with a justification.
pub struct RegseqAwareCallbacks;

impl rustc_driver::Callbacks for RegseqAwareCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        // Same MIR shape contract as RegseqCallbacks. The whitelist is
        // sourced from the shared `regseq_mir_passes()` so it cannot
        // drift between the two callback types.
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.always_encode_mir = true;
        config.opts.unstable_opts.mir_enable_passes = regseq_mir_passes();
        config.opts.cg.overflow_checks = Some(false);
        config.opts.cg.debug_assertions = Some(false);
        config.opts.debug_assertions = false;

        // INTENTIONALLY NOT SET (symmetric register_tool rule):
        //   - config.opts.unstable_opts.no_codegen
        //     rustc must codegen a normal rlib for the facade so
        //     downstream specs can link against it.
        //   - feature(register_tool) / register_tool(rseq) crate_attr
        //     injection. The aware crate's source declares its own;
        //     without source-side declarations `cargo check` and
        //     `rust-analyzer` fail on the facade outside the wrapper.
        //     This is the symmetric "source declares its own
        //     compiler-relevant attributes" methodology rule.
    }
    // No `after_analysis` override: rustc proceeds to codegen normally.
}

/// Regseq callbacks that hook into the compilation pipeline for the
/// primary driver spec.
pub struct RegseqCallbacks {
    options: regseq_config::CompilerOptions,
}

impl RegseqCallbacks {
    /// Create callbacks configured for regseq mode.
    pub fn new(options: regseq_config::CompilerOptions) -> Self {
        Self { options }
    }

    /// Access the compiler options used to configure this callback.
    pub fn options(&self) -> &regseq_config::CompilerOptions {
        &self.options
    }
}

impl rustc_driver::Callbacks for RegseqCallbacks {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.opts.unstable_opts.no_codegen = true;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.always_encode_mir = true;
        config.opts.unstable_opts.mir_enable_passes = regseq_mir_passes();
        config.opts.cg.overflow_checks = Some(false);
        config.opts.cg.debug_assertions = Some(false);
        config.opts.debug_assertions = false;

        // INTENTIONALLY NOT SET (symmetric register_tool rule, mirrors
        // the matching note in `RegseqAwareCallbacks::config`):
        //   - feature(register_tool) / register_tool(rseq) crate_attr
        //     injection. The spec source declares its own (symmetric
        //     "source declares its own compiler-relevant attributes"
        //     methodology rule).
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        _tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        // Bridge entry point. Currently empty: the RegseqModel-producing
        // bridge is being built bottom-up. Until the bridge can produce
        // a RegseqModel for the configured emit specs, this method just
        // stops compilation cleanly.
        eprintln!(
            "regseq-driver: after_analysis reached for target_os={:?}; \
             bridge not yet wired -- skipping all extraction.",
            self.options.target_os,
        );
        rustc_driver::Compilation::Stop
    }
}
