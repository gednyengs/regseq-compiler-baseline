#![no_std]
#![feature(extern_types)]
unsafe extern "C" {
    pub type Opaque;
}
pub fn f(p: *const Opaque) -> *const Opaque {
    p
}
