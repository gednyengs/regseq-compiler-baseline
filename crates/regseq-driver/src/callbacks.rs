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

    fn wants_raw_artifact(&self) -> bool {
        self.options
            .emit
            .iter()
            .any(|e| matches!(e, regseq_config::EmitSpec::RawArtifact))
    }

    fn wants_driver(&self) -> bool {
        self.options
            .emit
            .iter()
            .any(|e| matches!(e, regseq_config::EmitSpec::Driver { .. }))
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
        config.opts.cg.overflow_checks = Some(false);
        config.opts.cg.debug_assertions = Some(false);
        config.opts.debug_assertions = false;
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

        // --- cross-crate attribute validation (gate test, Phase 4) ---
        if std::env::var("REGSEQ_VALIDATE_ATTRS").is_ok() {
            let attr_report = crate::extract::attr_validation::validate_cross_crate_attrs(tcx);
            attr_report.log();
        }

        // --- identity ---
        let identities = crate::extract::driver_identity::extract_driver_identities(tcx);
        eprintln!("regseq-driver: identity extraction: {identities:?}");

        // --- register access ---
        let register_surface =
            crate::extract::register_access::extract_register_access_surface(tcx);
        eprintln!("regseq-driver: register access extraction: {register_surface:?}");

        // --- interrupts ---
        let interrupt_surface = crate::extract::interrupts::extract_interrupt_surface(tcx);
        eprintln!("regseq-driver: interrupt extraction: {interrupt_surface:?}");

        let isr_handler_names: std::collections::HashSet<String> = match &interrupt_surface {
            Ok(surface) => surface
                .isr_bindings
                .iter()
                .map(|b| b.handler_name.clone())
                .collect(),
            Err(_) => std::collections::HashSet::new(),
        };

        // --- synchronization ---
        let sync_surface = crate::extract::synchronization::extract_synchronization_surface(
            tcx,
            &isr_handler_names,
        );
        eprintln!("regseq-driver: synchronization extraction: {sync_surface:?}");

        // --- trait callbacks ---
        let trait_surface = crate::extract::traits::extract_trait_surface(tcx);
        eprintln!("regseq-driver: trait extraction: {trait_surface:?}");

        // --- firmware properties ---
        let property_surface = crate::extract::property::extract_property_surface(tcx);
        eprintln!("regseq-driver: property extraction: {property_surface:?}");

        // --- clock ---
        let clock_surface = crate::extract::clock::extract_clock_surface(tcx);
        eprintln!("regseq-driver: clock extraction: {clock_surface:?}");

        // --- reset ---
        let reset_surface = crate::extract::reset::extract_reset_surface(tcx);
        eprintln!("regseq-driver: reset extraction: {reset_surface:?}");

        // --- pinctrl ---
        let pinctrl_surface = crate::extract::pinctrl::extract_pinctrl_surface(tcx);
        eprintln!("regseq-driver: pinctrl extraction: {pinctrl_surface:?}");

        // --- timed delay ---
        let timed_delay_surface = crate::extract::timed_delay::extract_timed_delay_surface(tcx);
        eprintln!("regseq-driver: timed_delay extraction: {timed_delay_surface:?}");

        // --- logging ---
        let logging_surface = crate::extract::logging::extract_logging_surface(tcx);
        eprintln!("regseq-driver: logging extraction: {logging_surface:?}");

        // --- body extraction ---
        let body_surface = crate::extract::body::extract_body_surface(tcx);
        eprintln!("regseq-driver: body extraction: {body_surface:?}");

        // --- identity enrichment ---
        let enrichment_surface =
            crate::extract::identity_enrichment::extract_identity_enrichment(tcx);
        eprintln!("regseq-driver: identity enrichment extraction: {enrichment_surface:?}");

        // --- PM gate (requires trait surface for PM impl types) ---
        let pm_impl_types: std::collections::HashSet<String> = trait_surface
            .as_ref()
            .map(|ts| {
                ts.power_management_impls
                    .iter()
                    .map(|i| i.driver_type.clone())
                    .collect()
            })
            .unwrap_or_default();
        let pm_surface = crate::extract::pm::extract_pm_surface(tcx, &pm_impl_types);
        eprintln!("regseq-driver: pm extraction: {pm_surface:?}");

        // --- build raw artifact (shared by raw-artifact and driver emission) ---
        let needs_artifact = self.wants_raw_artifact() || self.wants_driver();
        if needs_artifact {
            if let Ok(ref ids) = identities {
                if let Some(primary) = ids.first() {
                    // Phase 0 contract: identity enrichment is required.
                    // Do not build or emit artifacts when enrichment failed.
                    if let Err(ref e) = enrichment_surface {
                        eprintln!(
                            "regseq-driver: FATAL: identity enrichment extraction failed: {e:?} \
                             — artifact and driver emission skipped",
                        );
                    } else {
                        let ex = ExtractionResults {
                            register: &register_surface,
                            traits: &trait_surface,
                            property: &property_surface,
                            clock: &clock_surface,
                            reset: &reset_surface,
                            pinctrl: &pinctrl_surface,
                            interrupt: &interrupt_surface,
                            sync: &sync_surface,
                            timed_delay: &timed_delay_surface,
                            logging: &logging_surface,
                            pm: &pm_surface,
                            body: &body_surface,
                            enrichment: &enrichment_surface,
                        };
                        let artifact = build_raw_artifact(primary, &self.options, &ex);

                        // --- raw artifact emission ---
                        if self.wants_raw_artifact() {
                            if let Err(e) = emit_raw_artifact(&artifact, &self.options.output_dir) {
                                eprintln!("regseq-driver: raw artifact emission failed: {e}");
                            } else {
                                eprintln!(
                                    "regseq-driver: raw artifact emitted to {}/drivers/{}.raw.json",
                                    self.options.output_dir.display(),
                                    primary.driver_name,
                                );
                            }
                        }

                        // --- driver emission ---
                        if self.wants_driver() {
                            match crate::emit_driver::emit_driver(&artifact, &self.options) {
                                Ok(result) => {
                                    eprintln!(
                                        "regseq-driver: driver emission complete: {} artifacts under {}",
                                        result.artifact_set.artifacts.len(),
                                        result.artifact_set.output_root.display(),
                                    );
                                    for a in &result.artifact_set.artifacts {
                                        eprintln!(
                                            "regseq-driver:   {} ({:?}, {} bytes)",
                                            a.path.display(),
                                            a.role,
                                            a.size_bytes,
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("regseq-driver: driver emission failed: {e}");
                                }
                            }
                        }
                    }
                }
            }
        }

        rustc_driver::Compilation::Stop
    }
}

/// Collected extraction results passed to raw artifact construction.
struct ExtractionResults<'a> {
    register: &'a Result<
        crate::extract::register_access::RegisterAccessSurface,
        crate::extract::register_access::RegisterAccessError,
    >,
    traits: &'a Result<crate::extract::traits::TraitSurface, crate::extract::traits::TraitError>,
    property: &'a Result<
        crate::extract::property::PropertySurface,
        crate::extract::property::PropertyError,
    >,
    clock: &'a Result<crate::extract::clock::ClockSurface, crate::extract::clock::ClockError>,
    reset: &'a Result<crate::extract::reset::ResetSurface, crate::extract::reset::ResetError>,
    pinctrl:
        &'a Result<crate::extract::pinctrl::PinCtrlSurface, crate::extract::pinctrl::PinCtrlError>,
    interrupt: &'a Result<
        crate::extract::interrupts::InterruptSurface,
        crate::extract::interrupts::InterruptError,
    >,
    sync: &'a Result<
        crate::extract::synchronization::SynchronizationSurface,
        crate::extract::synchronization::SynchronizationError,
    >,
    timed_delay: &'a Result<
        crate::extract::timed_delay::TimedDelaySurface,
        crate::extract::timed_delay::TimedDelayError,
    >,
    logging:
        &'a Result<crate::extract::logging::LoggingSurface, crate::extract::logging::LoggingError>,
    pm: &'a Result<crate::extract::pm::PmGateSurface, crate::extract::pm::PmError>,
    body: &'a Result<crate::mir_repr::MirBodySurface, crate::extract::body::BodyError>,
    enrichment: &'a Result<
        crate::extract::identity_enrichment::IdentityEnrichmentSurface,
        crate::extract::identity_enrichment::IdentityEnrichmentError,
    >,
}

fn build_raw_artifact(
    primary_id: &crate::extract::driver_identity::DriverIdentity,
    options: &regseq_config::CompilerOptions,
    ex: &ExtractionResults<'_>,
) -> crate::raw_artifact::RawFrontendArtifact {
    use crate::raw_artifact::*;

    RawFrontendArtifact {
        driver_name: primary_id.driver_name.clone(),
        target_os: format!("{:?}", options.target_os).to_lowercase(),
        identity: Some(RawDriverIdentity::from(primary_id)),
        register_access: ex
            .register
            .as_ref()
            .ok()
            .map(RawRegisterAccessSurface::from),
        trait_callbacks: ex.traits.as_ref().ok().map(RawTraitSurface::from),
        firmware_properties: ex.property.as_ref().ok().map(RawPropertySurface::from),
        clock: ex.clock.as_ref().ok().map(RawClockSurface::from),
        reset: ex.reset.as_ref().ok().map(RawResetSurface::from),
        pinctrl: ex.pinctrl.as_ref().ok().map(RawPinCtrlSurface::from),
        interrupts: ex.interrupt.as_ref().ok().map(RawInterruptSurface::from),
        synchronization: ex.sync.as_ref().ok().map(RawSynchronizationSurface::from),
        timed_delay: ex.timed_delay.as_ref().ok().map(RawTimedDelaySurface::from),
        logging: ex.logging.as_ref().ok().map(RawLoggingSurface::from),
        pm_gate: ex.pm.as_ref().ok().map(RawPmGateSurface::from),
        bodies: ex.body.as_ref().ok().cloned(),
        identity_enrichment: ex.enrichment.as_ref().ok().cloned(),
    }
}

fn emit_raw_artifact(
    artifact: &crate::raw_artifact::RawFrontendArtifact,
    output_dir: &std::path::Path,
) -> Result<(), String> {
    let drivers_dir = output_dir.join("drivers");
    std::fs::create_dir_all(&drivers_dir)
        .map_err(|e| format!("failed to create drivers dir: {e}"))?;

    let path = drivers_dir.join(format!("{}.raw.json", artifact.driver_name));
    let json = serde_json::to_string_pretty(artifact)
        .map_err(|e| format!("failed to serialize raw artifact: {e}"))?;

    std::fs::write(&path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))
}
