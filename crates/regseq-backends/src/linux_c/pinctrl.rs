//! Pin-controller-and-GPIO-configuration lowering (pinctrl state-selection) for
//! the linux_c backend (M4/T20, C04/C07/D10). Mirrors `clock.rs`: this module
//! owns the two scaffold-facing pieces (the REQUIRED-form field morph + the
//! probe-preamble acquire); the author-op lowering (`apply::<S>()`) lives in
//! `body::emit_pinctrl`.
//!
//!  * a REQUIRED `PinCtrl` FIELD morphs to a `struct pinctrl *` member in the
//!    priv `inner` struct ([`field_decls`], spliced by `emit_type_defs`'s
//!    `field_override`, exactly as a managed clock field morphs to `struct clk
//!    *`); the acquire preamble is `devm_pinctrl_get` + `IS_ERR` + select the
//!    `"default"` state ([`acquire_lines`]). Release is devm-automatic.
//!  * an OPTIONAL `Option<PinCtrl>` keeps NO field (the handle-less
//!    `pinctrl_pm_select_*_state(dev)` PM helpers resolve from the device); the
//!    kernel auto-selects the `"default"` pin state during probe, so the acquire
//!    preamble is a no-op (a comment only).
//!
//! Only a RESOLVED resource emits any scaffolding (Codex M4 #5): an absent-only
//! `Option<PinCtrl>` TU is byte-unchanged (no `<linux/pinctrl/consumer.h>`, no
//! field, no preamble) — its `apply` compile-time-folds to `Err(PinctrlAbsent)`
//! in `body::emit_pinctrl` (D06/C04), with no pinctrl API call.
//!
//! The `pinctrl_*` API spellings are the Linux pinctrl-consumer contract
//! (D10/C04, spec-derived — PL011's reference path).

use std::collections::BTreeMap;

use regseq_model::{RegseqModel, Ty, TyId};

use super::naming::c_ident;
use super::EmitError;

/// Whether the driver has ≥1 RESOLVED pinctrl resource — the demand gate for the
/// `<linux/pinctrl/consumer.h>` include, the field morph, and the acquire
/// preamble. Resolved-only: an optional-ABSENT resource emits no scaffolding
/// (Codex M4 #5), so an absent-only / pinctrl-less TU stays byte-unchanged (I07).
pub(crate) fn has_resolved_pinctrl(model: &RegseqModel, driver_ty: TyId) -> bool {
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(p) = model.pinctrl.drivers.get(item) {
            return p.resources.iter().any(|r| r.binding.is_resolved());
        }
    }
    false
}

/// The `struct pinctrl *` field declarations for each RESOLVED REQUIRED pinctrl
/// resource on the driver struct (the field_override map merged into
/// `emit_type_defs`). An OPTIONAL resource is deliberately ABSENT — no field
/// morph (the handle-less PM helpers resolve from the device); an unresolved
/// (optional-absent) resource emits nothing.
pub(crate) fn field_decls(model: &RegseqModel, driver_ty: TyId) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(p) = model.pinctrl.drivers.get(item) {
            for res in &p.resources {
                if res.required && res.binding.is_resolved() {
                    let f = c_ident(model.strings.resolve(res.name));
                    m.insert(f.clone(), format!("struct pinctrl *{f}"));
                }
            }
        }
    }
    m
}

/// The probe-preamble pinctrl-acquire lines (C07/D10/C04): one block per RESOLVED
/// resource in declaration order, slotted AFTER the region map and BEFORE the
/// clock acquire — pins-before-clock (C07). `dev_addr` is the `struct device *`
/// address expression (`&adev->dev` AMBA / `&pdev->dev` PCI), used by the
/// required form only.
///
///  * REQUIRED — `devm_pinctrl_get(dev)` + `IS_ERR`/`PTR_ERR`, then look up + the
///    select the `"default"` state, propagating the NATIVE errno on failure (a
///    hard init error — abort before the clock acquire / `on_init`, I02;
///    `PTR_ERR` preserves `-EPROBE_DEFER`, the original `pinctrl_select_state` rc
///    is propagated rather than flattened, Codex M4 #2). devm releases on
///    unbind/probe-fail (A-2 — no postamble).
///  * OPTIONAL — auto-bind no-op: the kernel selects the `"default"` pin state
///    during probe (handle-less PM path, C04/D10). A comment only.
///
/// Empty for a pinctrl-less / absent-only driver (no golden churn, I07).
pub(crate) fn acquire_lines(
    model: &RegseqModel,
    driver_ty: TyId,
    dev_addr: &str,
) -> Result<String, EmitError> {
    let mut out = String::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(p) = model.pinctrl.drivers.get(item) {
            for res in &p.resources {
                if !res.binding.is_resolved() {
                    continue;
                }
                let f = c_ident(model.strings.resolve(res.name));
                if res.required {
                    out.push_str(&format!(
                        "\n\t/* preamble: acquire the REQUIRED pin controller (devm: released\n\t * automatically on unbind / probe-fail, A-2) + select the \"default\"\n\t * state. A get / lookup / select failure is a hard init error (I02),\n\t * propagated NATIVE (PTR_ERR — -EPROBE_DEFER survives; the select rc is\n\t * preserved, not flattened) BEFORE the clock acquire / on_init\n\t * (C07/D10/C04) */\n\tpriv->inner.{f} = devm_pinctrl_get({dev_addr});\n\tif (IS_ERR(priv->inner.{f}))\n\t\treturn PTR_ERR(priv->inner.{f});\n\t{{\n\t\tstruct pinctrl_state *__ps_{f} = pinctrl_lookup_state(priv->inner.{f}, \"default\");\n\t\tint __ret_{f};\n\t\tif (IS_ERR(__ps_{f}))\n\t\t\treturn PTR_ERR(__ps_{f});\n\t\t__ret_{f} = pinctrl_select_state(priv->inner.{f}, __ps_{f});\n\t\tif (__ret_{f})\n\t\t\treturn __ret_{f};\n\t}}\n"
                    ));
                } else {
                    out.push_str(
                        "\n\t/* preamble: OPTIONAL pin controller — the kernel auto-selects the\n\t * \"default\" pin state during probe (handle-less PM path, C04/D10);\n\t * nothing to acquire here */\n",
                    );
                }
            }
        }
    }
    Ok(out)
}
