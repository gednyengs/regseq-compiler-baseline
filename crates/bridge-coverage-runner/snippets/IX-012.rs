#![no_std]
pub struct Inner;
impl Drop for Inner {
    fn drop(&mut self) {}
}
pub struct Outer {
    pub i: Inner,
}
pub fn f() {
    let _o = Outer { i: Inner };
}
