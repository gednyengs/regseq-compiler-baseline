#![no_std]
pub fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
pub fn entry() -> i32 {
    apply(|n| n + 7, 3)
}
