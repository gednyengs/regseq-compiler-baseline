#![no_std]
pub fn target() {}
pub fn entry() -> fn() {
    let g: fn() = target;
    g
}
