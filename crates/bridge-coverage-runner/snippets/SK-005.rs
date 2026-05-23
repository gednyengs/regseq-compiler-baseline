#![no_std]
pub struct Holder(i32);
pub fn finish(f: impl FnOnce(i32) -> i32) -> i32 {
    f(7)
}
pub fn entry() -> i32 {
    let h = Holder(33);
    finish(move |n| h.0 + n)
}
