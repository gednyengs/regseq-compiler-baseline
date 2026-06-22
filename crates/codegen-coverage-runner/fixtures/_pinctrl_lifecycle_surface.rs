// Pin-controller-and-GPIO-configuration OPTIONAL-form lifecycle surface — the
// 3-variant `RegseqError` variant of `_lifecycle_surface.rs`, used by the
// `_pinctrl/*` fixtures whose optional `Option<PinCtrl>` absent path folds to
// `Err(RegseqError::PinctrlAbsent)` (D06/D12). The required-form fixtures keep
// the shared 2-variant `_lifecycle_surface.rs` (no golden churn); the optional
// fixtures pull THIS one instead:
//   include!("../../_pinctrl_lifecycle_surface.rs");
//   include!("../../_pinctrl_surface.rs");
// (declaring exactly one `RegseqError` + `Lifecycle` per fixture). The including
// `input.rs` owns the crate attrs (`#![no_std]`, `#![feature(register_tool)]`,
// `#![register_tool(rseq)]`).
//
// WHY A SEPARATE SURFACE (the M1 niche-isolation finding). The shared
// `_lifecycle_surface.rs` `RegseqError` has exactly TWO fieldless variants so
// `Result<(), RegseqError>` is niche-encoded (the entry-shim classifier requires
// the niche-encoded `Result<(), E>` form). Adding a third variant THERE would
// shift the niche layout and churn every shared lifecycle golden. A THREE-variant
// fieldless enum still leaves `Result<(), E>` niche-encoded (3 of 256 byte values
// used), so the entry-shim is satisfied here too — the isolation is purely to
// keep the unrelated 2-variant goldens byte-stable.

// -- RegseqError (3-variant: adds PinctrlAbsent) -------------------------------
// Identity is the `#[rseq::framework_type]` attribute (CR-2), not a facade crate.
// `PinctrlAbsent` is the optional-absent fold target (D06/C04); the generic-c
// errno adapter maps each Err variant to its `<errno.h>` symbol —
// `PinctrlAbsent` -> `ENODEV` (D12). The first two variants keep their
// `_lifecycle_surface.rs` names/order so any shared-name reasoning carries over.
#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    // errno-mappable (generic_c/errno.rs): HardwareError -> EIO, DeviceBusy ->
    // EBUSY, PinctrlAbsent -> ENODEV.
    HardwareError,
    DeviceBusy,
    PinctrlAbsent,
}

// -- the Lifecycle trait (identical to _lifecycle_surface.rs) -------------------
// Five callbacks, each `&mut self -> Result<(), RegseqError>` with a default
// no-op body. The bridge override-check reads which methods the `impl` provides;
// the four "glued" callbacks always emit their `LC call <cb>` marker, while
// `on_system_shutdown` is omit-when-absent.
pub trait Lifecycle {
    /// After preamble (bring-up). `&mut self`: may touch resources + user-data.
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    /// Orderly teardown; HW still reachable (HW-touching postambles run).
    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    /// Teardown after the device is gone; no HW access (postambles skipped).
    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    /// `on_init` returned `Err`; clean up partial state.
    fn on_init_failure(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    /// System going down; slot is omitted entirely when not overridden.
    fn on_system_shutdown(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}
