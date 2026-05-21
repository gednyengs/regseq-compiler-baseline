#![no_std]
#![feature(thread_local)]
#[thread_local]
pub static T: u32 = 0;
pub fn f() -> u32 {
    unsafe { T }
}
