#![no_std]
pub const P: *const u32 = core::ptr::null();
pub fn f() -> *const u32 {
    P
}
