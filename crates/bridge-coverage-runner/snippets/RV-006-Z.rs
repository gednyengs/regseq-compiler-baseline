#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f(p: *const u32, n: isize) -> *const u32 {
    unsafe { core::intrinsics::offset(p, n) }
}
