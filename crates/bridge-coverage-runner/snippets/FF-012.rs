#![no_std]
pub fn g() {}
pub fn f() -> fn() {
    let p: fn() = g;
    p.clone()
}
