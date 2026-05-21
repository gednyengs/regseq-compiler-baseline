#![no_std]
pub static X: u32 = 42;
pub const P: &'static u32 = &X;
pub fn entry() -> u32 {
    *P
}
