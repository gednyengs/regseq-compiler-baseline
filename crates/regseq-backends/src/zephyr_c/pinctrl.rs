//! Pin-controller-and-GPIO-configuration lowering (pinctrl state-selection) for
//! the zephyr_c backend (M4/T21, C04/C07/D10). Mirrors `clock.rs`: this module
//! owns the init-hub acquire gate + the build-time DT pin-config define; the
//! author-op lowering (`apply::<S>()`) lives in `body::emit_pinctrl`.
//!
//! Zephyr's pin control is DT-authoritative and build-time: `PINCTRL_DT_INST_DEFINE`
//! materializes the pin-state table from the devicetree, and
//! `pinctrl_apply_state(cfg, PINCTRL_STATE_<S>)` selects a state at runtime over
//! the build-time `struct pinctrl_dev_config *` (`PINCTRL_DT_INST_DEV_CONFIG_GET`).
//! There is no `devm`-style handle (unlike linux_c's required form) and no field
//! morph — the config is a DT constant, so REQUIRED and OPTIONAL share the same
//! API; they differ only in rc handling (required hard-fails, optional ignores,
//! C04). The "acquire" is the init-hub default-state apply, slotted BEFORE the
//! clock gate (pins-before-clock, C07).
//!
//! Only a RESOLVED resource emits any scaffolding (Codex M4 #5): an absent-only
//! `Option<PinCtrl>` TU emits no `PINCTRL_DT_INST_DEFINE`, no
//! `<zephyr/drivers/pinctrl.h>`, no acquire — its `apply` compile-time-folds to
//! `Err(PinctrlAbsent)` in `body::emit_pinctrl` (D06/C04). This is the real
//! PL011-Zephyr-qemu target's path witnessed SYNTHETICALLY (the clock-managed
//! precedent, D12): the `pinctrl_*` spellings are the Zephyr pinctrl contract
//! (D10/C04, spec-derived).

use regseq_model::{RegseqModel, Ty, TyId};

/// The build-time DT pin-config accessor (single instance, hardcoded index 0 —
/// the `DT_INST_*(0)` convention used throughout zephyr_c, e.g. the IRQ connect).
pub(crate) fn cfg_expr() -> &'static str {
    "PINCTRL_DT_INST_DEV_CONFIG_GET(0)"
}

/// Whether the driver has ≥1 RESOLVED pinctrl resource — the demand gate for the
/// `PINCTRL_DT_INST_DEFINE`, the `<zephyr/drivers/pinctrl.h>` include, the
/// binding-yaml `pinctrl-device.yaml`, and the init-hub acquire gate. Resolved-
/// only: an optional-ABSENT resource emits no scaffolding (Codex M4 #5), so an
/// absent-only / pinctrl-less TU stays byte-unchanged (I07).
pub(crate) fn has_resolved_pinctrl(model: &RegseqModel, driver_ty: TyId) -> bool {
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(p) = model.pinctrl.drivers.get(item) {
            return p.resources.iter().any(|r| r.binding.is_resolved());
        }
    }
    false
}

/// The init-hub pinctrl-acquire lines (C07/D10/C04): one default-state apply per
/// RESOLVED resource in declaration order, slotted AFTER the region map and
/// BEFORE the clock gate — pins-before-clock (C07). Over the build-time DT pin
/// config (`PINCTRL_DT_INST_DEV_CONFIG_GET(0)`):
///   * REQUIRED — the rc is checked and propagated NATIVE on failure (a hard init
///     error, I02 — abort before the clock gate / `on_init`; `pinctrl_apply_state`
///     returns `0` / `-ENOENT` / another negative errno, so the original rc is
///     preserved rather than flattened, Codex M4 #4).
///   * OPTIONAL — best-effort: the rc is ignored (`(void)`, C04).
/// Empty for a pinctrl-less / absent-only driver (no golden churn, I07).
pub(crate) fn acquire_lines(model: &RegseqModel, driver_ty: TyId) -> String {
    let mut out = String::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(p) = model.pinctrl.drivers.get(item) {
            let cfg = cfg_expr();
            for res in &p.resources {
                if !res.binding.is_resolved() {
                    continue;
                }
                if res.required {
                    out.push_str(&format!(
                        "\t/* preamble: apply the REQUIRED default pin state over the build-time DT\n\t * pin config — a hard init error if it fails (I02, C07/D10/C04); the\n\t * pinctrl_apply_state rc is propagated NATIVE, BEFORE the clock gate /\n\t * on_init */\n\t{{\n\t\tint __ret = pinctrl_apply_state({cfg}, PINCTRL_STATE_DEFAULT);\n\t\tif (__ret)\n\t\t\treturn __ret;\n\t}}\n\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "\t/* preamble: apply the OPTIONAL default pin state (best-effort — the rc\n\t * is ignored, C04/D10), BEFORE the clock gate (pins-before-clock, C07) */\n\t(void)pinctrl_apply_state({cfg}, PINCTRL_STATE_DEFAULT);\n\n"
                    ));
                }
            }
        }
    }
    out
}
