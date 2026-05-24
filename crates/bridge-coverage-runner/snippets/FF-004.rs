#![no_std]
#[track_caller]
pub fn t() {}
pub static FP: fn() = t;
