use regseq::{Duration, RegseqError};

#[test]
fn poll_until_is_publicly_available() {
    // Verify the path regseq::poll::until resolves.
    let _f: fn(fn() -> bool, Duration, Duration) -> Result<(), RegseqError> = regseq::poll::until;
}

#[test]
fn poll_until_signature_compiles() {
    // Verify the accepted argument and return shape compiles with a closure.
    fn assert_compiles() {
        let _: Result<(), RegseqError> =
            regseq::poll::until(|| false, Duration::from_us(10), Duration::from_ms(100));
    }
    let _ = assert_compiles;
}

#[test]
fn poll_until_uses_shared_facade_types() {
    // Duration and RegseqError are the shared facade types, not local aliases.
    // Verify type compatibility without invoking the stub.
    fn assert_types(_delay: Duration, _timeout: Duration) {
        let _: fn(fn() -> bool, Duration, Duration) -> Result<(), RegseqError> =
            regseq::poll::until;
    }
    let _ = assert_types;
}
