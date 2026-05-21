#![no_std]
pub fn f(addr: usize) -> *const u32 {
    addr as *const u32
}
