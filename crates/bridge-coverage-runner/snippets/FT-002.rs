#![no_std]
pub struct W<T> {
    pub v: T,
}
impl<T> Drop for W<T> {
    fn drop(&mut self) {}
}
pub fn f() {
    let _w = W { v: 0u32 };
}
