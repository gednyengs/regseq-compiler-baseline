use crate::regseq::RegseqError;

/// Typed clock resource declaration surface.
///
/// `Clock` is the opaque facade type that driver authors use to declare
/// peripheral clock fields. The compiler extracts these declarations and
/// joins them with Layer 2 clock binding data during lowering.
///
/// `Clock` vs `Option<Clock>` encodes whether the clock resource is
/// required or optional for the device.
#[rseq::abstraction("clock")]
pub struct Clock {
    _opaque: (),
}

impl Clock {
    /// Enable the peripheral clock.
    ///
    /// Fallible: returns an error if the clock cannot be enabled.
    #[rseq::intrinsic("clock_enable")]
    pub fn enable(&self) -> Result<(), RegseqError> {
        unimplemented!("compiler-managed: Clock::enable")
    }

    /// Disable the peripheral clock.
    ///
    /// Infallible on the stable facade surface.
    #[rseq::intrinsic("clock_disable")]
    pub fn disable(&self) {
        unimplemented!("compiler-managed: Clock::disable")
    }

    /// Query the clock rate in Hz.
    ///
    /// Infallible on the stable facade surface.
    #[rseq::intrinsic("clock_get_rate")]
    pub fn get_rate(&self) -> u32 {
        unimplemented!("compiler-managed: Clock::get_rate")
    }
}
