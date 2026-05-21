#![no_std]
pub struct W<T, U> {
    pub a: T,
    pub b: U,
}
pub fn f() -> W<u32, u32> {
    W { a: 0, b: 0 }
}
