#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f() {
    unsafe { core::intrinsics::unreachable() }
}
