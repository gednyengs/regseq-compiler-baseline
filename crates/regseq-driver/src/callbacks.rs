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
        // panic=abort so MIR drop/closure-call sites have their unwind
        // cleanup blocks pruned before the bridge consumes them; the body
        // translator rejects any surviving `is_cleanup` block (see
        // `bridge/body/basic_block.rs`). Part of the shared MIR-shape
        // contract: this MUST match `RegseqCallbacks::config` (and the
        // `-C panic=abort` flag in the test-ir-fixtures harness), or
        // cross-crate MIR consumed by the bridge would drift in shape.
        config.opts.cg.panic = Some(rustc_target::spec::PanicStrategy::Abort);
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
        // panic=abort so MIR drop/closure-call sites have their unwind
        // cleanup blocks pruned before the bridge consumes them; the body
        // translator rejects any surviving `is_cleanup` block (see
        // `bridge/body/basic_block.rs`). MUST match `RegseqAwareCallbacks`
        // and the `-C panic=abort` flag in the test-ir-fixtures harness.
        config.opts.cg.panic = Some(rustc_target::spec::PanicStrategy::Abort);
        config.opts.cg.overflow_checks = Some(false);
        config.opts.cg.debug_assertions = Some(false);
        config.opts.debug_assertions = false;

        // Force `output_types.should_codegen() == true` so
        // `reachable_non_generics_provider` in rustc populates the
        // reachable set, which is what the Phase-1 mono-item
        // collection seeds from. Without this, `cargo check` passes
        // `--emit=dep-info,metadata` and rustc's
        // `reachable_non_generics_provider` (rustc_codegen_ssa/src/
        // back/symbol_export.rs:49-52) returns the empty map,
        // `is_reachable_non_generic` returns false for ordinary
        // non-entry roots (entry fns and items with special codegen
        // flags like `RUSTC_STD_INTERNAL_SYMBOL` are admitted via a
        // separate predicate in `RootCollector::is_root` at
        // `rustc_monomorphize/src/collector.rs:1649-1670`), the Lazy
        // strategy in `collect_roots`
        // (`rustc_monomorphize/src/collector.rs:1462-1499`) emits
        // zero roots for a `pub fn`-only library crate, and the
        // bridge's `collect_and_partition_mono_items` query
        // (`crates/regseq-driver/src/bridge/discovery.rs:12`) returns
        // an empty `MonoItem` set. Result: pretty-printed model is
        // empty even though the bridge appeared to run.
        //
        // `OutputType::Mir` is the lightest member of the set
        // `should_codegen` accepts (rustc_session/src/config.rs:881-892);
        // `Compilation::Stop` in `after_analysis` halts rustc before
        // the MIR-emit phase runs (`rustc_driver_impl/src/lib.rs`
        // honours `Stop` between analysis and codegen), so no
        // `.mir` file is written.
        //
        // Side effect (positive for the bridge, but a real
        // user-visible behavior change): `should_codegen() == true`
        // also triggers extra MIR validation passes that don't run
        // in pure metadata mode. Specifically,
        // `mir_drops_elaborated_and_const_checked` runs eagerly for
        // every body owner before `after_analysis`
        // (`rustc_interface/src/passes.rs:1111`). This is desirable
        // for us -- the bridge consumes drop-elaborated MIR -- but
        // means `cargo regseq` may surface const-check diagnostics
        // that `cargo check` alone would not. Other
        // `should_codegen()` gates downstream
        // (`rustc_metadata/src/rmeta/encoder.rs:1107`,
        // `rustc_codegen_ssa/src/back/symbol_export.rs:171`,
        // `rustc_metadata/src/locator.rs:312`) run after
        // `after_analysis` and are suppressed by `Compilation::Stop`.
        let mut output_entries: Vec<_> = config
            .opts
            .output_types
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        if !config
            .opts
            .output_types
            .contains_key(&rustc_session::config::OutputType::Mir)
        {
            output_entries.push((rustc_session::config::OutputType::Mir, None));
        }
        config.opts.output_types = rustc_session::config::OutputTypes::new(&output_entries);

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
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        // 1. Bridge rustc HIR/MIR into a RegseqModel.
        let input = crate::bridge::BridgeInput::new(tcx);
        let mut model = match crate::bridge::run_bridge(input) {
            Ok(model) => model,
            Err(err) => {
                tcx.dcx()
                    .fatal(format!("regseq-driver: bridge failed: {err}"));
            }
        };

        // 1b. Bus-resource-management construct (#3) + platform identity (T7):
        // parse the platform file ONCE (Codex A1). Join `[mmio.<id>]` to the
        // model's resource fields (resolving the effective BAR + `(base, size)`),
        // and resolve the `[platform]` identity (os/board/arch) — the single
        // source of target identity (D4), carried to the backend via the
        // `BackendContext` below. Not rustc-driven — runs here, post bridge,
        // parallel to the model. A platform file that is absent or empty yields no
        // bindings and the default (empty) identity; a malformed file is a hard
        // error.
        let mut platform_identity = regseq_model::PlatformIdentity::default();
        match regseq_config::parse_platform_file_path(&self.options.platform_file) {
            Ok(file) => {
                if let Err(err) = regseq_l2::bind_all(&mut model, &file.bindings) {
                    tcx.dcx().fatal(format!("regseq-driver: bus-resource {err}"));
                }
                match regseq_l2::resolve_platform_identity(&file.identity) {
                    Ok(identity) => platform_identity = identity,
                    Err(err) => {
                        tcx.dcx().fatal(format!(
                            "regseq-driver: platform identity [{}] {err}",
                            err.code()
                        ));
                    }
                }
            }
            Err(regseq_config::PlatformParseError::Io { .. }) => {
                // An unreadable/absent platform file is non-fatal: no L2 bindings
                // to join (the faked-base path is used) and the default identity.
                // The bus-resource `BRM-006` unresolvable diagnostic (construct #3
                // M6) is what fails a region that actually needs a binding.
            }
            Err(err) => {
                tcx.dcx()
                    .fatal(format!("regseq-driver: platform file error: {err}"));
            }
        }

        // Bus-resource-management surface validation (construct #3 M6, T19/T20):
        // the BRM-002/003/005/006 + unknown-`bus` diagnostics over the joined
        // L1+L2 view. Runs for every bus-declared driver regardless of whether a
        // platform file was present (an absent file still leaves a bus-managed
        // region unresolvable -> BRM-006). A non-bus driver is never affected.
        if let Err(err) = regseq_l2::validate_l2_surface(&model) {
            tcx.dcx()
                .fatal(format!("regseq-driver: bus-resource {err}"));
        }

        // 2. Joint validate. `RegseqModel::validate` takes `&mut self`
        // because it rebuilds the vtables side table (per
        // `crates/regseq-model/src/model.rs:91,3207`); subsequent
        // steps only need an immutable borrow.
        let bridge_input = regseq_model::BridgeInput::matching(&model);
        if let Err(err) = model.validate(&bridge_input) {
            tcx.dcx()
                .fatal(format!("regseq-driver: model.validate failed: {err:?}"));
        }

        // 3. Honor `--check`: validate-only path skips ALL emission
        // (per `crates/cargo-regseq/src/cli.rs:40`).
        if self.options.check {
            return rustc_driver::Compilation::Stop;
        }

        // 4. Warn on unsupported emit specs (`Driver` / `Analysis`
        // are documented but phase-1b territory per
        // `pipeline-refactor.md:879`). The warning logic lives in
        // `emit::collect_unsupported_warnings` so it is unit-testable
        // without a `TyCtxt`.
        for warning in crate::emit::collect_unsupported_warnings(&self.options.emit) {
            tcx.dcx().warn(warning);
        }

        // 5. Resolve backend codegen options from the `backend.*` compiler
        // params (RD-5/N1): the device-lifecycle scenario selection travels
        // here, parallel to the model, never through the bridge. An invalid
        // value (e.g. an unknown `backend.lifecycle.scenario` token) is fatal.
        let backend_options = match regseq_backends::BackendOptions::from_backend_params(
            self.options
                .params
                .iter()
                .filter(|p| p.scope == regseq_config::ParamScope::Backend)
                .map(|p| (p.name.as_str(), p.value.as_str())),
        ) {
            Ok(options) => options,
            Err(err) => {
                tcx.dcx()
                    .fatal(format!("regseq-driver: backend param error: {err}"));
            }
        };

        // The pure-emission context (T7): backend options + the platform identity
        // resolved at the 1b seam. generic-C traces the identity (D7); future
        // Zephyr/Linux backends select a framework from it.
        let backend_context =
            regseq_backends::BackendContext::new(backend_options, platform_identity);

        // 6. Dispatch each emit spec. Any emit failure is fatal.
        for spec in &self.options.emit {
            if let Err(err) =
                crate::emit::emit_one_with(&model, spec, &self.options.output_dir, &backend_context)
            {
                tcx.dcx()
                    .fatal(format!("regseq-driver: emit {spec} failed: {err}"));
            }
        }

        rustc_driver::Compilation::Stop
    }
}
