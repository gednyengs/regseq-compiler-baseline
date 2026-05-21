#![no_std]
pub fn target() {}
pub fn entry(g: fn()) -> unsafe fn() {
    let h: unsafe fn() = g;
    let _ = target;
    h
}
