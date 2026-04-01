use regseq::Duration;

#[test]
fn canonical_intrinsic_paths_are_public() {
    // Both canonical paths must be importable.
    use regseq::intrinsics::timed_delay::busy_wait;
    use regseq::intrinsics::timed_delay::sleep;

    let _ = busy_wait as fn(Duration);
    let _ = sleep as fn(Duration);
}

#[test]
fn compatibility_paths_are_public() {
    // The accepted authored surface must remain available.
    use regseq::delay::busy_wait;
    use regseq::delay::sleep;

    let _ = busy_wait as fn(Duration);
    let _ = sleep as fn(Duration);
}

#[test]
fn both_surfaces_use_shared_duration_type() {
    let d = Duration::from_ms(10);

    // Canonical path accepts Duration.
    let _: fn(Duration) = regseq::intrinsics::timed_delay::busy_wait;
    let _: fn(Duration) = regseq::intrinsics::timed_delay::sleep;

    // Compatibility path accepts the same Duration.
    let _: fn(Duration) = regseq::delay::busy_wait;
    let _: fn(Duration) = regseq::delay::sleep;

    let _ = d;
}

#[test]
fn canonical_and_compatibility_signatures_match() {
    // Both paths must resolve to the same function type.
    let canonical_bw: fn(Duration) = regseq::intrinsics::timed_delay::busy_wait;
    let compat_bw: fn(Duration) = regseq::delay::busy_wait;
    assert_eq!(canonical_bw as usize, compat_bw as usize);

    let canonical_sl: fn(Duration) = regseq::intrinsics::timed_delay::sleep;
    let compat_sl: fn(Duration) = regseq::delay::sleep;
    assert_eq!(canonical_sl as usize, compat_sl as usize);
}
