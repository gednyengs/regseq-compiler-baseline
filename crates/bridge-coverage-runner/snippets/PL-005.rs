#![no_std]
pub fn f(a: [u32; 4]) -> u32 {
    let [first, ..] = a;
    first
}
