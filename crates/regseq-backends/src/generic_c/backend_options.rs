//! Backend codegen options derived from `backend.*` compiler params (RD-5/N1).
//!
//! Generic-C emission is a pure function of `(model, BackendOptions)` (RD-7/N2):
//! the same model + the same options always produce the same translation unit,
//! so the snapshot oracle's byte-stability holds. [`BackendOptions::default`]
//! reproduces the historical model-only emission byte-for-byte (every existing
//! golden), so a caller that passes the default sees no change.
//!
//! Today the only option is the device-lifecycle scenario selection (RD-5):
//! which of the canonical lifecycle scenarios the synthesized harness `main`
//! runs (consumed by the scenario synthesizer, T28). The selection arrives as a
//! `backend.lifecycle.scenario=<csv>` compiler param (the CLI `--param` path) or
//! the equivalent fixture-manifest field; the parser lives here so both paths
//! validate it identically.

use regseq_model::PlatformIdentity;

use super::EmitError;

/// A canonical device-lifecycle scenario the synthesized harness `main` runs.
/// The token spelling (see [`LifecycleScenario::token`]) is what the
/// `backend.lifecycle.scenario` CSV accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleScenario {
    /// Orderly teardown: `on_init` → … → `on_deinit` (device still present).
    Orderly,
    /// Surprise removal: `on_init` → … → `on_surprise_remove` (HW gone).
    Surprise,
    /// Init-failure rollback: `on_init` Errs → `on_init_failure` + unwind.
    InitFail,
    /// System shutdown: `on_init` → … → `on_system_shutdown` (iff overridden).
    Shutdown,
}

impl LifecycleScenario {
    /// All scenarios in canonical (emission) order — the default selection.
    pub const ALL: [LifecycleScenario; 4] = [
        LifecycleScenario::Orderly,
        LifecycleScenario::Surprise,
        LifecycleScenario::InitFail,
        LifecycleScenario::Shutdown,
    ];

    /// The `backend.lifecycle.scenario` CSV token for this scenario.
    pub fn token(self) -> &'static str {
        match self {
            LifecycleScenario::Orderly => "orderly",
            LifecycleScenario::Surprise => "surprise",
            LifecycleScenario::InitFail => "init-fail",
            LifecycleScenario::Shutdown => "shutdown",
        }
    }

    fn from_token(token: &str) -> Option<LifecycleScenario> {
        LifecycleScenario::ALL.into_iter().find(|s| s.token() == token)
    }

    /// A C-identifier-safe label (the synthesized scenario-helper fn name embeds
    /// it) — the [`token`](LifecycleScenario::token) with `-` mapped to `_`.
    pub fn ident(self) -> &'static str {
        match self {
            LifecycleScenario::Orderly => "orderly",
            LifecycleScenario::Surprise => "surprise",
            LifecycleScenario::InitFail => "init_fail",
            LifecycleScenario::Shutdown => "shutdown",
        }
    }
}

/// One device-interrupt harness fire step (construct #6): where in the
/// orderly scenario the synthesized harness calls the SYNCHRONOUS
/// `regseq_irq_fire` (GRILL D07), and the optional device-model cause poke
/// that precedes a fire. Parsed from the `backend.irq.fire` CSV; the steps
/// target the driver's FIRST interrupt resource (the lowered M1 surface is a
/// single `Interrupt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqFireStep {
    /// Fire after registration, before `on_init` — inert (line disabled).
    PreInit,
    /// Fire after the post-`on_init`-Ok enable — sources still masked.
    PostInit,
    /// Poke a cause value into the host device model (`regseq_irq_dev_poke`)
    /// then fire — the handled case.
    Cause {
        /// Regfile offset of the poked cell (e.g. the PL011 MIS).
        offset: u32,
        /// The 32-bit value latched before the fire.
        value: u32,
    },
    /// Fire after the deinit teardown — inert (line unregistered).
    PostDeinit,
}

/// Backend codegen options. The second half of the pure-emission input pair
/// `(model, BackendOptions)` (RD-7/N2). Constructed from backend-scoped compiler
/// params; [`BackendOptions::default`] is the historical model-only behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendOptions {
    /// The device-lifecycle scenarios the synthesized `main` runs (RD-5), in
    /// canonical order. Default = all four. Consumed by the synthesizer (T28).
    lifecycle_scenarios: Vec<LifecycleScenario>,
    /// The device-interrupt fire steps the orderly scenario runs (construct
    /// #6), in authored CSV order. Default = none (zero churn: an interrupt
    /// TU without the param registers/enables/tears down but never fires).
    irq_fire: Vec<IrqFireStep>,
    /// Pin-controller-and-GPIO-configuration acquire-FAILURE injection (pinctrl
    /// construct, T18): the `#[rseq::pinctrl(id = "…")]` resource id whose
    /// compiler-managed `acquire_pinctrl` the harness forces to FAIL, exercising
    /// the REQUIRED form's hard-fail (init aborts before `on_init`; the optional
    /// form ignores the rc, D03/C04). `None` = the default (acquire never fails).
    /// A generic-c-oracle test knob (not model/L2 truth), so it rides the same
    /// `backend.*` param path as `lifecycle.scenario`/`irq.fire`.
    pinctrl_fail_acquire: Option<String>,
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions {
            lifecycle_scenarios: LifecycleScenario::ALL.to_vec(),
            irq_fire: Vec::new(),
            pinctrl_fail_acquire: None,
        }
    }
}

impl BackendOptions {
    /// The selected lifecycle scenarios, in canonical order.
    pub fn lifecycle_scenarios(&self) -> &[LifecycleScenario] {
        &self.lifecycle_scenarios
    }

    /// The device-interrupt fire steps, in authored order.
    pub fn irq_fire(&self) -> &[IrqFireStep] {
        &self.irq_fire
    }

    /// The pinctrl resource id whose `acquire_pinctrl` the harness forces to fail
    /// (T18), or `None` for the default (acquire never fails).
    pub fn pinctrl_fail_acquire(&self) -> Option<&str> {
        self.pinctrl_fail_acquire.as_deref()
    }

    /// Build from backend-scoped `(name, value)` param pairs — the `backend.`
    /// scope prefix already stripped by the caller (so `name` is e.g.
    /// `lifecycle.scenario`). Recognizes `lifecycle.scenario` = a comma-separated
    /// scenario-token list and `irq.fire` = a comma-separated fire-step list
    /// (`pre-init` / `post-init` / `cause:0x<off>=0x<val>` / `post-deinit`); an
    /// unknown token fails loud ([`EmitError::BackendOption`]). An empty /
    /// absent value keeps the default. An unknown param *name* is ignored
    /// (forward-compatible with other backends' params). Last writer wins for a
    /// repeated name.
    pub fn from_backend_params<'a>(
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<BackendOptions, EmitError> {
        let mut options = BackendOptions::default();
        for (name, value) in pairs {
            match name {
                "lifecycle.scenario" => {
                    options.lifecycle_scenarios = parse_scenarios(value)?;
                }
                "irq.fire" => {
                    options.irq_fire = parse_irq_fire(value)?;
                }
                "pinctrl.fail-acquire" => {
                    // A single pinctrl id (trimmed); empty/absent keeps the default
                    // (no injected failure). The id must match a RESOLVED pinctrl
                    // resource at emit (the synthesizer fails loud otherwise, T18).
                    let id = value.trim();
                    options.pinctrl_fail_acquire = (!id.is_empty()).then(|| id.to_string());
                }
                _ => {} // not ours — another backend's param.
            }
        }
        Ok(options)
    }
}

/// The full pure-emission context for a generic-C emit: the backend codegen
/// [`BackendOptions`] (harness/codegen knobs — today the lifecycle scenario
/// selection) PLUS the target [`PlatformIdentity`] (os/board/arch, sourced from
/// the platform file's `[platform]` table — D4/D5).
///
/// Kept as two distinct fields rather than overloading [`BackendOptions`]: "how
/// the harness exercises this driver" and "what real platform we target" are
/// different kinds of input (Codex B). Emission stays pure over `(model,
/// BackendContext)`. generic-C currently only *traces* the identity (a one-line
/// header comment, emitted only when an identity is present — D7); future
/// Zephyr/Linux backends will select a framework from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendContext {
    options: BackendOptions,
    platform: PlatformIdentity,
}

impl BackendContext {
    /// Bundle backend options with a target platform identity.
    pub fn new(options: BackendOptions, platform: PlatformIdentity) -> BackendContext {
        BackendContext { options, platform }
    }

    /// The backend codegen options (lifecycle scenarios, …).
    pub fn options(&self) -> &BackendOptions {
        &self.options
    }

    /// The target platform identity (os/board/arch).
    pub fn platform(&self) -> &PlatformIdentity {
        &self.platform
    }
}

impl From<BackendOptions> for BackendContext {
    /// Wrap options with the default (empty) identity — the zero-churn path for
    /// callers with no platform identity (the `_with(&BackendOptions)` shims and
    /// existing tests). An empty identity emits no trace, so goldens are stable.
    fn from(options: BackendOptions) -> BackendContext {
        BackendContext {
            options,
            platform: PlatformIdentity::default(),
        }
    }
}

/// Parse an `irq.fire` CSV into the authored-order fire-step list. Unlike the
/// scenario CSV, AUTHORED ORDER IS THE CONTRACT (the steps are a script the
/// orderly scenario runs in sequence), so no canonicalization happens; a
/// repeated step fires repeatedly. An empty list means no fires.
fn parse_irq_fire(value: &str) -> Result<Vec<IrqFireStep>, EmitError> {
    let mut steps = Vec::new();
    for token in value.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let step = match token {
            "pre-init" => IrqFireStep::PreInit,
            "post-init" => IrqFireStep::PostInit,
            "post-deinit" => IrqFireStep::PostDeinit,
            cause if cause.starts_with("cause:") => {
                let spec = &cause["cause:".len()..];
                let (off, val) = spec.split_once('=').ok_or_else(|| {
                    EmitError::BackendOption(format!(
                        "malformed irq.fire cause step '{token}' \
                         (expected cause:0x<offset>=0x<value>)"
                    ))
                })?;
                let parse_hex = |s: &str| -> Option<u32> {
                    let s = s.trim();
                    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
                    u32::from_str_radix(hex, 16).ok()
                };
                let (Some(offset), Some(value)) = (parse_hex(off), parse_hex(val)) else {
                    return Err(EmitError::BackendOption(format!(
                        "malformed irq.fire cause step '{token}' \
                         (expected cause:0x<offset>=0x<value>)"
                    )));
                };
                IrqFireStep::Cause { offset, value }
            }
            other => {
                return Err(EmitError::BackendOption(format!(
                    "unknown irq.fire step '{other}' (expected one of: pre-init, \
                     post-init, cause:0x<offset>=0x<value>, post-deinit)"
                )))
            }
        };
        steps.push(step);
    }
    Ok(steps)
}

/// Parse a `lifecycle.scenario` CSV into a deduplicated, canonical-order
/// selection. An empty list (`""` / all-whitespace) means "all" — emission is
/// independent of CSV token order so the golden is stable regardless of how the
/// param is spelled.
fn parse_scenarios(value: &str) -> Result<Vec<LifecycleScenario>, EmitError> {
    let mut selected: Vec<LifecycleScenario> = Vec::new();
    for token in value.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let scenario = LifecycleScenario::from_token(token).ok_or_else(|| {
            EmitError::BackendOption(format!(
                "unknown lifecycle scenario '{token}' \
                 (expected one of: orderly, surprise, init-fail, shutdown)"
            ))
        })?;
        if !selected.contains(&scenario) {
            selected.push(scenario);
        }
    }
    if selected.is_empty() {
        return Ok(LifecycleScenario::ALL.to_vec());
    }
    Ok(LifecycleScenario::ALL
        .into_iter()
        .filter(|s| selected.contains(s))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_scenarios_in_canonical_order() {
        assert_eq!(
            BackendOptions::default().lifecycle_scenarios(),
            LifecycleScenario::ALL.as_slice()
        );
    }

    #[test]
    fn no_params_is_default() {
        let opts = BackendOptions::from_backend_params(std::iter::empty()).unwrap();
        assert_eq!(opts, BackendOptions::default());
    }

    #[test]
    fn explicit_subset_parses_in_canonical_order() {
        // CSV order is intentionally non-canonical; emission must not depend on it.
        let opts = BackendOptions::from_backend_params([("lifecycle.scenario", "shutdown,orderly")])
            .unwrap();
        assert_eq!(
            opts.lifecycle_scenarios(),
            &[LifecycleScenario::Orderly, LifecycleScenario::Shutdown]
        );
    }

    #[test]
    fn duplicate_and_whitespace_tokens_are_normalized() {
        let opts =
            BackendOptions::from_backend_params([("lifecycle.scenario", " orderly , orderly ,surprise ")])
                .unwrap();
        assert_eq!(
            opts.lifecycle_scenarios(),
            &[LifecycleScenario::Orderly, LifecycleScenario::Surprise]
        );
    }

    #[test]
    fn empty_value_keeps_all() {
        let opts = BackendOptions::from_backend_params([("lifecycle.scenario", "")]).unwrap();
        assert_eq!(opts.lifecycle_scenarios(), LifecycleScenario::ALL.as_slice());
    }

    #[test]
    fn unknown_scenario_token_fails_loud() {
        let err = BackendOptions::from_backend_params([("lifecycle.scenario", "orderly,bogus")])
            .unwrap_err();
        match err {
            EmitError::BackendOption(msg) => assert!(msg.contains("bogus"), "msg: {msg}"),
            other => panic!("expected BackendOption, got {other:?}"),
        }
    }

    #[test]
    fn irq_fire_default_is_empty() {
        assert!(BackendOptions::default().irq_fire().is_empty());
    }

    #[test]
    fn irq_fire_parses_in_authored_order_with_cause() {
        let opts = BackendOptions::from_backend_params([(
            "irq.fire",
            "pre-init, post-init, cause:0x040=0x00000010, post-deinit",
        )])
        .unwrap();
        assert_eq!(
            opts.irq_fire(),
            &[
                IrqFireStep::PreInit,
                IrqFireStep::PostInit,
                IrqFireStep::Cause {
                    offset: 0x40,
                    value: 0x10
                },
                IrqFireStep::PostDeinit,
            ]
        );
    }

    #[test]
    fn irq_fire_unknown_or_malformed_tokens_fail_loud() {
        for bad in ["mid-flight", "cause:0x40", "cause:40=0x10"] {
            let err = BackendOptions::from_backend_params([("irq.fire", bad)]).unwrap_err();
            assert!(
                matches!(err, EmitError::BackendOption(_)),
                "expected BackendOption for {bad}"
            );
        }
    }

    #[test]
    fn pinctrl_fail_acquire_defaults_none_and_parses_an_id() {
        // T18: default is no injected failure; a non-empty value is the id, an
        // empty/whitespace value keeps the default.
        assert_eq!(BackendOptions::default().pinctrl_fail_acquire(), None);
        let opts =
            BackendOptions::from_backend_params([("pinctrl.fail-acquire", " uart_pins ")]).unwrap();
        assert_eq!(opts.pinctrl_fail_acquire(), Some("uart_pins"));
        let empty = BackendOptions::from_backend_params([("pinctrl.fail-acquire", "  ")]).unwrap();
        assert_eq!(empty.pinctrl_fail_acquire(), None);
    }

    #[test]
    fn unknown_param_name_is_ignored() {
        let opts =
            BackendOptions::from_backend_params([("some.other.backend.knob", "whatever")]).unwrap();
        assert_eq!(opts, BackendOptions::default());
    }

    #[test]
    fn backend_context_exposes_options_and_platform() {
        let opts = BackendOptions::from_backend_params([("lifecycle.scenario", "orderly")]).unwrap();
        let platform = PlatformIdentity {
            board: Some("qemu-pc".to_string()),
            ..PlatformIdentity::default()
        };
        let ctx = BackendContext::new(opts.clone(), platform.clone());
        assert_eq!(ctx.options(), &opts);
        assert_eq!(ctx.platform(), &platform);
    }

    #[test]
    fn backend_context_from_options_has_default_identity() {
        let opts = BackendOptions::default();
        let ctx = BackendContext::from(opts.clone());
        assert_eq!(ctx.options(), &opts);
        assert_eq!(ctx.platform(), &PlatformIdentity::default());
    }

    #[test]
    fn backend_context_default_is_default_options_and_identity() {
        let ctx = BackendContext::default();
        assert_eq!(ctx.options(), &BackendOptions::default());
        assert_eq!(ctx.platform(), &PlatformIdentity::default());
    }
}
