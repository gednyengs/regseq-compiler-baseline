#![no_std]
pub struct R;
impl Drop for R {
    fn drop(&mut self) {}
}
pub fn f() {
    let _x = R;
    let _y = R;
}
