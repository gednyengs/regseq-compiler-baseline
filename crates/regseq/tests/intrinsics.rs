use regseq::intrinsics::register_access::poll_until;
use regseq::intrinsics::sync::{memory_barrier, BarrierKind};
use regseq::Duration;

#[test]
fn canonical_poll_until_is_accessible() {
    // Verify the canonical intrinsic path compiles and has the expected signature.
    let _: fn(fn() -> bool, Duration, Duration) -> Result<(), regseq::RegseqError> = poll_until;
}

#[test]
fn canonical_memory_barrier_is_accessible() {
    // Verify the canonical intrinsic path compiles and has the expected signature.
    let _: fn(BarrierKind) = memory_barrier;
}

#[test]
fn compatibility_poll_until_same_signature() {
    // The compatibility path should resolve to the same function.
    let canonical: fn(fn() -> bool, Duration, Duration) -> Result<(), regseq::RegseqError> =
        poll_until;
    let compat: fn(fn() -> bool, Duration, Duration) -> Result<(), regseq::RegseqError> =
        regseq::poll::until;
    assert!(std::ptr::fn_addr_eq(canonical, compat));
}

#[test]
fn barrier_kind_variants_usable() {
    // Verify all BarrierKind variants are publicly accessible.
    let kinds = [
        BarrierKind::ReadRead,
        BarrierKind::ReadWrite,
        BarrierKind::WriteRead,
        BarrierKind::WriteWrite,
        BarrierKind::Full,
    ];
    assert_eq!(kinds.len(), 5);
    // Verify Debug, Clone, Copy, PartialEq, Eq
    let a = BarrierKind::Full;
    let b = a;
    assert_eq!(a, b);
    let _ = format!("{:?}", a);
}
