#![no_std]
pub fn entry() -> fn() {
    let c = || {};
    let g: fn() = c;
    g
}
