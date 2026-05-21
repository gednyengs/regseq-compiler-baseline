#![no_std]
pub struct W<T> {
    pub x: T,
}
pub fn f<T>(w: W<T>) -> T {
    w.x
}
pub fn entry() -> u32 {
    f(W { x: 0_u32 })
}
