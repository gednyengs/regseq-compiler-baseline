#![no_std]
fn call_once<F: FnOnce(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}
pub fn entry() -> i32 {
    let c = |n: i32| n + 1;
    call_once(c, 5)
}
