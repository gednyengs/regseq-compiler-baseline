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

/// Backend codegen options. The second half of the pure-emission input pair
/// `(model, BackendOptions)` (RD-7/N2). Constructed from backend-scoped compiler
/// params; [`BackendOptions::default`] is the historical model-only behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendOptions {
    /// The device-lifecycle scenarios the synthesized `main` runs (RD-5), in
    /// canonical order. Default = all four. Consumed by the synthesizer (T28).
    lifecycle_scenarios: Vec<LifecycleScenario>,
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions {
            lifecycle_scenarios: LifecycleScenario::ALL.to_vec(),
        }
    }
}

impl BackendOptions {
    /// The selected lifecycle scenarios, in canonical order.
    pub fn lifecycle_scenarios(&self) -> &[LifecycleScenario] {
        &self.lifecycle_scenarios
    }

    /// Build from backend-scoped `(name, value)` param pairs — the `backend.`
    /// scope prefix already stripped by the caller (so `name` is e.g.
    /// `lifecycle.scenario`). Recognizes `lifecycle.scenario` = a comma-separated
    /// scenario-token list; an unknown token fails loud
    /// ([`EmitError::BackendOption`]). An empty / absent value keeps the default
    /// (all scenarios). An unknown param *name* is ignored (forward-compatible
    /// with other backends' params). Last writer wins for a repeated name.
    pub fn from_backend_params<'a>(
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<BackendOptions, EmitError> {
        let mut options = BackendOptions::default();
        for (name, value) in pairs {
            match name {
                "lifecycle.scenario" => {
                    options.lifecycle_scenarios = parse_scenarios(value)?;
                }
                _ => {} // not ours — another backend's param.
            }
        }
        Ok(options)
    }
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
    fn unknown_param_name_is_ignored() {
        let opts =
            BackendOptions::from_backend_params([("some.other.backend.knob", "whatever")]).unwrap();
        assert_eq!(opts, BackendOptions::default());
    }
}
