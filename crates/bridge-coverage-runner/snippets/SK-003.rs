#![no_std]
pub fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}
pub fn entry() -> i32 {
    apply(|n| n + 1, 41)
}
