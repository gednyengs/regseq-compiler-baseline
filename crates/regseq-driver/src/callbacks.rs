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

    fn wants_analysis(&self) -> bool {
        self.options
            .emit
            .iter()
            .any(|e| matches!(e, regseq_config::EmitSpec::Analysis { .. }))
    }

    fn wants_model(&self) -> bool {
        self.options
            .emit
            .iter()
            .any(|e| matches!(e, regseq_config::EmitSpec::Model { .. }))
    }

    /// Return the explicit `path` from an `EmitSpec::Model { path }`, if one
    /// was supplied. `None` means "use the default location under
    /// `<output_dir>/drivers/<driver_name>.model.json`."
    fn model_path_override(&self) -> Option<&std::path::Path> {
        self.options.emit.iter().find_map(|e| match e {
            regseq_config::EmitSpec::Model { path } => path.as_deref(),
            _ => None,
        })
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

        // --- attribute registries (read-only context for downstream passes) ---
        let intrinsic_registry = crate::extract::intrinsic_registry::IntrinsicRegistry::build(tcx);
        eprintln!(
            "regseq-driver: intrinsic registry: {} entry(ies)",
            intrinsic_registry.len(),
        );
        let framework_type_registry =
            crate::extract::framework_types::FrameworkTypeRegistry::build(tcx);
        eprintln!(
            "regseq-driver: framework type registry: {} entry(ies)",
            framework_type_registry.len(),
        );
        let abstraction_type_registry =
            crate::extract::abstraction_types::AbstractionTypeRegistry::build(tcx);
        eprintln!(
            "regseq-driver: abstraction type registry: {} entry(ies)",
            abstraction_type_registry.len(),
        );
        let pin_state_registry = crate::construction::PinStateRegistry::build(tcx);
        eprintln!(
            "regseq-driver: pin-state registry: {} entry(ies)",
            pin_state_registry.len(),
        );

        // --- source locations (runs after all items are known) ---
        let source_location_surface =
            crate::extract::source_locations::extract_source_locations(tcx);
        eprintln!(
            "regseq-driver: source location extraction: {} item(s)",
            source_location_surface.len(),
        );

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

        if let Ok(ref ids) = identities {
            if let Some(primary) = ids.first() {
                // --- raw artifact emission ---
                // Legacy path kept building until C4 removes RawArtifact
                // entirely from the production driver.
                if self.wants_raw_artifact() {
                    // Phase 0 contract: identity enrichment is required.
                    // Do not build or emit raw artifacts when enrichment failed.
                    if let Err(ref e) = enrichment_surface {
                        eprintln!(
                            "regseq-driver: FATAL: identity enrichment extraction failed: {e:?} \
                             — raw artifact emission skipped",
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
                            intrinsic_registry: &intrinsic_registry,
                            framework_type_registry: &framework_type_registry,
                            abstraction_type_registry: &abstraction_type_registry,
                            source_locations: &source_location_surface,
                        };
                        let artifact = build_raw_artifact(tcx, primary, &self.options, &ex);
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
                }
            }
        }

        // --- RegseqModel JSON emission ---
        if self.wants_model() {
            let registries = crate::construction::ExtractionRegistries {
                intrinsic: &intrinsic_registry,
                framework_types: &framework_type_registry,
                abstraction_types: &abstraction_type_registry,
                pin_states: &pin_state_registry,
            };
            match crate::construction::build_declaration_skeleton(tcx, &registries) {
                Ok(model) => match emit_regseq_model_json(
                    &model,
                    &self.options.output_dir,
                    self.model_path_override(),
                ) {
                    Ok(output_path) => eprintln!(
                        "regseq-driver: RegseqModel JSON emitted to {}",
                        output_path.display(),
                    ),
                    Err(e) => eprintln!("regseq-driver: RegseqModel JSON emission failed: {e}"),
                },
                Err(e) => {
                    eprintln!("regseq-driver: RegseqModel construction failed: {e}")
                }
            }
        }

        // --- driver emission (stubbed during RegseqModel-only rebuild) ---
        if self.wants_driver() {
            eprintln!(
                "regseq-driver: EmitSpec::Driver is not yet exercised in the \
                 RegseqModel-only rebuild. See \
                 docs/discussions/models/regseq-model/legacy-removal-plan.md \
                 (DL-3). Skipping driver emission.",
            );
        }

        // --- analysis emission (stubbed during RegseqModel-only rebuild) ---
        if self.wants_analysis() {
            eprintln!(
                "regseq-driver: EmitSpec::Analysis is not yet exercised in the \
                 RegseqModel-only rebuild. See \
                 docs/discussions/models/regseq-model/legacy-removal-plan.md \
                 (DL-3). Skipping analysis emission.",
            );
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
    intrinsic_registry: &'a crate::extract::intrinsic_registry::IntrinsicRegistry,
    framework_type_registry: &'a crate::extract::framework_types::FrameworkTypeRegistry,
    abstraction_type_registry: &'a crate::extract::abstraction_types::AbstractionTypeRegistry,
    source_locations: &'a crate::extract::source_locations::RawSourceLocationSurface,
}

fn build_raw_artifact<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    primary_id: &crate::extract::driver_identity::DriverIdentity,
    options: &regseq_config::CompilerOptions,
    ex: &ExtractionResults<'_>,
) -> crate::raw_artifact::RawFrontendArtifact {
    use crate::raw_artifact::*;

    let intrinsic_registry = if ex.intrinsic_registry.is_empty() {
        None
    } else {
        Some(RawIntrinsicRegistrySurface::from_registry(
            ex.intrinsic_registry,
            tcx,
        ))
    };
    let framework_types = if ex.framework_type_registry.is_empty() {
        None
    } else {
        Some(RawFrameworkTypeSurface::from_registry(
            ex.framework_type_registry,
            tcx,
        ))
    };
    let abstraction_types = if ex.abstraction_type_registry.is_empty() {
        None
    } else {
        Some(RawAbstractionTypeSurface::from_registry(
            ex.abstraction_type_registry,
            tcx,
        ))
    };
    let source_locations = if ex.source_locations.is_empty() {
        None
    } else {
        Some(ex.source_locations.clone())
    };

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
        intrinsic_registry,
        framework_types,
        abstraction_types,
        source_locations,
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

/// Serialize `model` to the JSON form documented in `ir-reference.md`
/// Section 7 and write it to disk. When `path_override` is `Some`, the
/// JSON is written there verbatim; when `None`, it lands under
/// `<output_dir>/drivers/<driver_name>.model.json`.
///
/// Returns the path the JSON was written to so the caller can log it.
fn emit_regseq_model_json(
    model: &regseq_model::model::RegseqModel,
    output_dir: &std::path::Path,
    path_override: Option<&std::path::Path>,
) -> Result<std::path::PathBuf, String> {
    let json = regseq_model::serialize::serialize_regseq_model(model);

    let path: std::path::PathBuf = match path_override {
        Some(p) => p.to_path_buf(),
        None => {
            let drivers_dir = output_dir.join("drivers");
            std::fs::create_dir_all(&drivers_dir)
                .map_err(|e| format!("failed to create drivers dir: {e}"))?;
            let driver_name = model_driver_name(model)
                .ok_or_else(|| "constructed RegseqModel has no driver name".to_string())?;
            drivers_dir.join(format!("{driver_name}.model.json"))
        }
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create parent dir for {}: {e}", path.display()))?;
    }

    std::fs::write(&path, json)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(path)
}

fn model_driver_name(model: &regseq_model::model::RegseqModel) -> Option<&str> {
    fn find_driver<'a>(
        module: &'a regseq_model::model::Module,
    ) -> Option<&'a regseq_model::model::DriverStruct> {
        if module.kind == regseq_model::model::ModuleKind::Driver {
            return module.driver.as_ref();
        }
        for submodule in &module.submodules {
            if let Some(driver) = find_driver(submodule) {
                return Some(driver);
            }
        }
        None
    }

    find_driver(&model.root_module).map(|driver| model.symbols.resolve(driver.identity.name))
}
