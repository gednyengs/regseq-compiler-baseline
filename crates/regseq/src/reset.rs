use crate::{Duration, RegseqError};

/// Typed reset-line resource declaration surface.
///
/// `ResetLine` is the opaque facade type that driver authors use to declare
/// hardware reset-line fields. The compiler extracts these declarations and
/// joins them with Layer 2 reset resource data during lowering.
///
/// `ResetLine` vs `Option<ResetLine>` encodes whether the reset resource
/// is required or optional for the device.
pub struct ResetLine {
    _opaque: (),
}

impl ResetLine {
    /// Assert the reset line (drive device into reset).
    pub fn assert(&self) -> Result<(), RegseqError> {
        unimplemented!("compiler-managed: ResetLine::assert")
    }

    /// Deassert the reset line (release device from reset).
    pub fn deassert(&self) -> Result<(), RegseqError> {
        unimplemented!("compiler-managed: ResetLine::deassert")
    }

    /// Assert, wait for `delay`, then deassert the reset line.
    pub fn toggle(&self, _delay: Duration) -> Result<(), RegseqError> {
        unimplemented!("compiler-managed: ResetLine::toggle")
    }
}
