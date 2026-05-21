#![no_std]
#![feature(core_intrinsics)]
#![allow(internal_features)]
pub fn f(x: u32, y: u32) -> core::cmp::Ordering {
    core::intrinsics::three_way_compare(x, y)
}
