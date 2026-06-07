// Device-lifecycle AUTHOR surface — shared across the `lifecycle-*` fixtures.
//
// Just the author-facing pieces: the optional `Lifecycle` trait (up to five
// callbacks) and an inline `RegseqError`. The harness verbs
// (`lifecycle::{bind,teardown,shutdown}`) and the opaque `DriverHandle<D>` were
// removed (RD-1): the generic-c backend now SYNTHESIZES the driving `main` from
// the model, so a fixture's `input.rs` declares only its `#[rseq::driver]` +
// `impl Lifecycle`, and its `manifest.toml` selects scenarios via the
// `backend.lifecycle.scenario` param.
//
// Declared **inline per fixture** (PLAN CR-2): identity is the attribute marker
// (`#[rseq::framework_type]`), recognized exactly as register-access fixtures
// inline `Mmio`/`Field`/`RegseqError` — the near-empty `crates/regseq` facade is
// deliberately NOT the home (framework-type materialization skips
// dependency-crate items). To avoid per-fixture drift (PLAN W6) it lives in this
// single file and each fixture pulls it in with
//   include!("../_lifecycle_surface.rs");
// The including `input.rs` owns the crate attrs (`#![no_std]`,
// `#![feature(register_tool)]`, `#![register_tool(rseq)]`).

// -- RegseqError ---------------------------------------------------------------
// Inline framework type (CR-2; mirrors the `regseq_error_*` fixtures). Its
// identity is the `#[rseq::framework_type]` attribute, not a facade crate. TWO
// variants is deliberate: a single-variant error enum has no niche, so
// `Result<(), RegseqError>` would be Direct- (tag-) encoded and the entry-shim
// classifier (which requires the niche-encoded `Result<(), E>` form) would
// reject the `regseq_main` return type. `core::result::Result` is used
// throughout — no `Display`/`std::error::Error`.
#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    // Variant names must be C02 errno-mappable (generic_c/errno.rs): the
    // entry-shim exit-code adapter maps each Err variant to its `<errno.h>`
    // symbol. `HardwareError` -> EIO, `DeviceBusy` -> EBUSY.
    HardwareError,
    DeviceBusy,
}

// -- the Lifecycle trait (C09) -------------------------------------------------
// Five callbacks, each `&mut self -> Result<(), RegseqError>` with a default
// no-op body so a driver compiles with an empty `impl` (or, at the bridge seam,
// none at all). The bridge override-check (D07/D10) reads which methods the
// `impl` actually provides and drives register-or-omit; the four "glued"
// callbacks always emit their `LC call <cb>` marker (an absent override means
// the default no-op ran — still "invoked", CR-3), while `on_system_shutdown` is
// omit-when-absent. Defining a sixth method in an `impl` is rustc E0407 — no
// device-lifecycle diagnostic is needed for that.
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

// The harness verbs (`lifecycle::{bind,teardown,shutdown}`) and the opaque
// `DriverHandle<D>` were removed (RD-1). The backend synthesizes the driving
// `main` from the model; a fixture declares only its `#[rseq::driver]` +
// `impl Lifecycle`, with scenarios selected by the manifest's
// `backend.lifecycle.scenario` param.
