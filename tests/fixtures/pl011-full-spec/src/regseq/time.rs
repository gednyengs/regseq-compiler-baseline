/// Compile-time duration type for delay and timeout surfaces.
///
/// Storage is in nanoseconds. The type is opaque; only the three stable
/// constructors and `MAX` are part of the public surface. Constructors
/// **saturate** to [`Duration::MAX`] on multiply overflow (never wrap), so a
/// const literal can never silently alias a tiny duration.
#[rseq::framework_type("duration")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    ns: u64,
}

impl Duration {
    /// The maximum representable duration; the saturation target on overflow.
    pub const MAX: Duration = Duration { ns: u64::MAX };

    /// Create a duration from a microsecond count.
    pub const fn from_us(us: u64) -> Self {
        Self { ns: us.saturating_mul(1_000) }
    }

    /// Create a duration from a millisecond count.
    pub const fn from_ms(ms: u64) -> Self {
        Self { ns: ms.saturating_mul(1_000_000) }
    }

    /// Create a duration from a second count.
    pub const fn from_s(s: u64) -> Self {
        Self { ns: s.saturating_mul(1_000_000_000) }
    }
}

// Compile-time conformance (timed-cpu-delay-and-sleep M0): the constructors
// convert exactly and **saturate** to `MAX` on multiply overflow rather than
// wrapping. This static assertion is evaluated whenever the fixture is compiled
// (the bridge's analysis pass, like `cargo check`), so a regression in the
// conversion math is a hard compile error — and it needs no public getter, so
// the opaque surface (I02) is preserved.
const _: () = {
    assert!(Duration::from_us(10).ns == 10_000);
    assert!(Duration::from_ms(1).ns == 1_000_000);
    assert!(Duration::from_s(1).ns == 1_000_000_000);
    // The largest exact corpus value (a 4 s settle) is well within range.
    assert!(Duration::from_s(4).ns == 4_000_000_000);
    // Multiply overflow saturates to `MAX` instead of wrapping.
    assert!(Duration::from_s(u64::MAX).ns == u64::MAX);
    assert!(Duration::from_ms(u64::MAX).ns == u64::MAX);
    assert!(Duration::from_us(u64::MAX).ns == u64::MAX);
    assert!(Duration::MAX.ns == u64::MAX);
};
