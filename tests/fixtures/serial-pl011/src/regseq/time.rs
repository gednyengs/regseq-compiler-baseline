/// Compile-time duration type for delay and timeout surfaces.
///
/// Storage is in microseconds. The type is opaque; only the three stable
/// constructors are part of the public surface.
#[rseq::framework_type("duration")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    us: u64,
}

impl Duration {
    /// Create a duration from a microsecond count.
    pub const fn from_us(us: u64) -> Self {
        Self { us }
    }

    /// Create a duration from a millisecond count.
    pub const fn from_ms(ms: u64) -> Self {
        Self { us: ms * 1_000 }
    }

    /// Create a duration from a second count.
    pub const fn from_s(s: u64) -> Self {
        Self { us: s * 1_000_000 }
    }
}
