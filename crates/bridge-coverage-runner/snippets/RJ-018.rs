#![no_std]
#![feature(unsafe_binders)]
// RJ-018: `UnsafeBinder`. An unsafe-binder type (`unsafe<'a> &'a u32`,
// via the incomplete `unsafe_binders` feature) is rejected by design ->
// `BridgeError::UnsafeBinder { ty_repr: "unsafe<'a> &'a u32" }`
// (bridge/ty.rs). Empirically PRODUCIBLE at HEAD: the witness compiles,
// reaches the bridge, and yields a clean Rejected (distinct from the
// non-producible feature-gated RJ-017/020/021). Expected by-design
// rejection (expectation = rejected).
use core::unsafe_binder::wrap_binder;
pub fn f() {
    let x: u32 = 5;
    let _b: unsafe<'a> &'a u32 = unsafe { wrap_binder!(&x) };
}
