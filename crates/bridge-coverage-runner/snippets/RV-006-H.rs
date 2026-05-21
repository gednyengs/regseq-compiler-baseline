#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f(x: u32, y: u32) -> u32 {
    unsafe { core::intrinsics::unchecked_mul(x, y) }
}
