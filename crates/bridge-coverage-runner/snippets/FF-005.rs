#![no_std]
fn g() -> u32 {
    7
}
fn call_it<F: Fn() -> u32>(f: F) -> u32 {
    f()
}
pub fn f() -> u32 {
    call_it(g as fn() -> u32)
}
