#![no_std]
pub struct R;
impl Drop for R {
    fn drop(&mut self) {}
}
pub fn f() {
    let r = R;
    core::mem::drop(r);
}
