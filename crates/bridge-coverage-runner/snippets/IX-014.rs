#![no_std]
pub struct Captured(i32);
pub fn run(f: impl FnOnce(i32) -> i32) -> i32 {
    f(0)
}
pub fn entry() -> i32 {
    let c = Captured(5);
    run(move |n| c.0 + n)
}
