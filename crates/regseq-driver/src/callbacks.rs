/// Passthrough callbacks that leave rustc behavior unchanged.
pub struct PassthroughCallbacks;

impl rustc_driver::Callbacks for PassthroughCallbacks {}

/// Regseq callbacks that hook into the compilation pipeline.
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
        config
            .opts
            .unstable_opts
            .crate_attr
            .push("feature(register_tool)".to_string());
        config
            .opts
            .unstable_opts
            .crate_attr
            .push("register_tool(rseq)".to_string());
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.always_encode_mir = true;
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        eprintln!(
            "regseq-driver: reached after_analysis for target_os={:?}",
            self.options.target_os,
        );
        let identities = crate::extract::extract_driver_identities(tcx);
        eprintln!("regseq-driver: identity extraction: {identities:?}");
        rustc_driver::Compilation::Stop
    }
}
