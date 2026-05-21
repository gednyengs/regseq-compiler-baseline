#![no_std]
pub fn f(p: *const u32) -> usize {
    p as usize
}
