#![no_std]
pub struct Inner;
impl Drop for Inner {
    fn drop(&mut self) {}
}
pub struct Outer {
    pub a: Inner,
    pub b: Inner,
}
pub fn f() {
    let _o = Outer { a: Inner, b: Inner };
}
