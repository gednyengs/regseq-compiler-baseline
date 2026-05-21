#![no_std]
pub fn f(x: u32) -> i32 {
    unsafe { core::mem::transmute::<u32, i32>(x) }
}
