#![no_std]
pub fn f(p: *const u8) -> *mut u8 {
    p as *mut u8
}
