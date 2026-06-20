//! Peripheral-clock-management acquire preamble for the zephyr_c backend
//! (M3/T15, C07/D12).
//!
//! The author-op lowering lives in `body::emit_clock`; this module owns the
//! init-hub acquire gate, mirroring the linux_c `clock::acquire_lines`. Zephyr
//! has no `devm`-style runtime acquire — the clock controller is a build-time DT
//! constant (`DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, idx))`) — so "acquire"
//! is the readiness gate: a managed clock whose controller is not ready is a hard
//! init error (I02), `-ENODEV` before the interrupt connect / `on_init` (C07).
//! The L2 `provider` is ignored here: the DT `clocks` property is authoritative
//! (D12), exactly as the IRQ number is DT-authoritative on Zephyr.

use regseq_model::{ClockMode, RegseqModel, Ty, TyId};

use super::body::clock_ctlr_expr;

/// Whether the driver has any MANAGED clock (the demand gate for the
/// `<zephyr/drivers/clock_control.h>` / `<errno.h>` includes + the acquire gate).
pub(crate) fn has_managed_clock(model: &RegseqModel, driver_ty: TyId) -> bool {
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(clocks) = model.clock.drivers.get(item) {
            return clocks
                .resources
                .iter()
                .any(|r| r.binding.mode == ClockMode::Managed);
        }
    }
    false
}

/// The init-hub clock-acquire lines (C07/D12): one `device_is_ready` gate per
/// MANAGED clock controller, in declaration order, `-ENODEV` on failure (a hard
/// init error, I02). Empty for a clock-less / static-only driver (no golden
/// churn, I07).
pub(crate) fn acquire_lines(model: &RegseqModel, driver_ty: TyId) -> String {
    let mut out = String::new();
    if let Ty::Struct { item, .. } = &model.types[driver_ty] {
        if let Some(clocks) = model.clock.drivers.get(item) {
            for (idx, res) in clocks.resources.iter().enumerate() {
                if res.binding.mode != ClockMode::Managed {
                    continue;
                }
                let ctlr = clock_ctlr_expr(idx);
                out.push_str(&format!(
                    "\t/* preamble: the managed clock controller must be ready before any\n\t * clock op — a hard init error if not (I02, C07/D12); DT-authoritative,\n\t * the L2 provider is advisory on Zephyr */\n\tif (!device_is_ready({ctlr}))\n\t\treturn -ENODEV;\n\n"
                ));
            }
        }
    }
    out
}
