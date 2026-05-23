#![no_std]
pub fn run(f: impl Fn(i32) -> i32) -> i32 {
    f(1) + f(2)
}
pub fn entry() -> i32 {
    run(|n| n * 3)
}
