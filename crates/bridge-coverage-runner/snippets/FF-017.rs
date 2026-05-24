#![no_std]
pub fn f() -> u32 {
    let x = 5u32;
    let c = move || x;
    c()
}
