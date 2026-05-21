#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f(cond: bool) {
    unsafe { core::intrinsics::assume(cond) };
}
