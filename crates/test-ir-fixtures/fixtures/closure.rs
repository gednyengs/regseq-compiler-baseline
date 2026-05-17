#![no_std]
pub fn make_inc() -> impl Fn(u32) -> u32 {
    |x| x + 1
}
