//! Device-lifecycle construct bridge (construct #2).
//!
//! Recognizes the authored device-lifecycle surface — a `#[rseq::driver]`
//! struct, its match sugar + `#[rseq::device_class]` tag, the `impl Lifecycle`
//! override set, and the driver-struct field split — and interns it into
//! `RegseqModel.device_lifecycle` for the generic-c lifecycle glue to consume.
//!
//! The recognition is **item-driven** (it walks the interned struct Items and
//! reads each struct's tool attributes), mirroring `bridge::framework_type`.
//! `materialise_drivers` interns the descriptors (Phase 4); `resolve_drivers`
//! (Phase 5) classifies fields for every driver and override-checks the
//! `impl Lifecycle` callbacks of those that have one (RD-2/RD-3). The harness
//! verbs were removed (RD-1), so there is no call-driven half.

mod recognition;
mod resolve;

pub(crate) use recognition::materialise_drivers;
pub(crate) use resolve::resolve_drivers;
