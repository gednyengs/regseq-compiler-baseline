#![no_std]
pub fn invoke(f: impl Fn(i32) -> i32) -> i32 {
    f(5)
}
pub fn entry() -> i32 {
    let a: i32 = 10;
    let b: i32 = 20;
    invoke(|x| x + a + b)
}
