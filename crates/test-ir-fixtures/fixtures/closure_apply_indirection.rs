#![no_std]

pub fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 {
    f(x, y)
}

pub fn entry(x: i32, y: i32) -> i32 {
    apply(x, y, |x, y| x + y)
}
