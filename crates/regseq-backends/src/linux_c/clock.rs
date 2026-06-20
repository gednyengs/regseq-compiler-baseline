//! Peripheral-clock-management field morph + acquire preamble for the linux_c
//! backend (M3/T14, C04/C07/D08).
//!
//! The author declares a `#[rseq::clock(id="…")]` field (a ZST `Clock`) and calls
//! `enable()`/`disable()`/`get_rate()`; the L2 `ClockBinder` resolves it to a
//! **managed** (real Linux clk framework) or **static** (fixed `default_frequency`)
//! `ClockBinding`. This module owns the two scaffold-facing pieces (the author-op
//! lowering lives in `body::emit_clock`), mirroring `sync`:
//!
//!  * a MANAGED clock FIELD morphs to a `struct clk *` member in the priv `inner`
//!    struct ([`field_decls`], spliced by `emit_type_defs`'s `field_override`,
//!    exactly as a lock field morphs to `spinlock_t`); a STATIC clock keeps the
//!    ZST drop (nothing references it — enable/disable no-op, get_rate const-folds).
//!  * [`acquire_lines`] emits the probe-preamble `devm_clk_get` + `PTR_ERR`
//!    propagation per managed clock, slotted AFTER the region map and BEFORE the
//!    interrupt request (C07/D16); a get failure is a hard init error (I02).
//!    Release is automatic (devm — no postamble, A-2).
//!
//! The `clk_*` API spellings are the Linux clk-framework contract (D08, spec-derived).

use std::collections::BTreeMap;

use regseq_model::{ClockMode, RegseqModel, Ty, TyId};

use super::naming::c_ident;
use super::EmitError;

/// The `struct clk *` field declarations for each MANAGED clock on the driver
/// struct (the field_override map merged into `emit_type_defs`). A STATIC clock
/// is deliberately ABSENT — its ZST `Clock` field is dropped (no op references
/// it: enable/disable no-op, get_rate const-folds to `default_frequency`).
pub(crate) fn field_decls(model: &RegseqModel, driver_ty: TyId) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(clocks) = model.clock.drivers.get(item) {
            for res in &clocks.resources {
                if res.binding.mode == ClockMode::Managed {
                    let f = c_ident(model.strings.resolve(res.name));
                    m.insert(f.clone(), format!("struct clk *{f}"));
                }
            }
        }
    }
    m
}

/// The probe-preamble clock-acquire lines (C07/D08): one `devm_clk_get` per
/// MANAGED clock in declaration order, with `PTR_ERR` propagation (a hard init
/// error, I02 — the acquisition aborts the preamble before the interrupt request
/// or `on_init` runs). `dev_addr` is the `struct device *` address expression
/// (`&adev->dev` AMBA / `&pdev->dev` PCI). Empty for a clock-less / static-only
/// driver (no golden churn, I07). A managed clock with no resolved provider is a
/// loud error (CLK-L2-004 guarantees one upstream — defense in depth).
pub(crate) fn acquire_lines(
    model: &RegseqModel,
    driver_ty: TyId,
    dev_addr: &str,
) -> Result<String, EmitError> {
    let mut out = String::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(clocks) = model.clock.drivers.get(item) {
            for res in &clocks.resources {
                if res.binding.mode != ClockMode::Managed {
                    continue;
                }
                let f = c_ident(model.strings.resolve(res.name));
                let con = res.binding.provider.ok_or_else(|| {
                    EmitError::Unsupported(
                        "managed clock with no resolved provider reached linux_c emission \
                         (CLK-L2-004 should have rejected it)"
                            .into(),
                    )
                })?;
                let con_lit = format!("{:?}", model.strings.resolve(con));
                out.push_str(&format!(
                    "\n\t/* preamble: acquire the managed clock (devm: released automatically\n\t * on unbind / probe-fail, A-2); con_id = the bound provider. A get\n\t * failure is a hard init error (I02), propagated native (PTR_ERR —\n\t * -EPROBE_DEFER survives) BEFORE the interrupt request / on_init\n\t * (C07/D05b) */\n\tpriv->inner.{f} = devm_clk_get({dev_addr}, {con_lit});\n\tif (IS_ERR(priv->inner.{f}))\n\t\treturn PTR_ERR(priv->inner.{f});\n"
                ));
            }
        }
    }
    Ok(out)
}
