#![no_std]
pub static X: u32 = 42;
pub const A: &'static u32 = &X;
pub fn f() -> u32 {
    *A
}
