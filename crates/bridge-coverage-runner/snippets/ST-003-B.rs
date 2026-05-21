#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub unsafe fn f(src: *const u8, dst: *mut u8, n: usize) {
    unsafe { core::intrinsics::copy_nonoverlapping(src, dst, n) };
}
