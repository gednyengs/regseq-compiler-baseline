#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f(x: u32, y: u32) -> (u32, bool) {
    unsafe { core::intrinsics::mul_with_overflow(x, y) }
}
