#![no_std]
pub fn f(a: [u32; 4]) -> u32 {
    let [head, middle @ .., tail] = a;
    head + middle[0] + tail
}
