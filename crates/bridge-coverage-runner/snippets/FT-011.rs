#![no_std]
pub fn invoke(f: impl Fn(u32) -> u32) -> u32 {
    f(0)
}
pub fn entry() -> u32 {
    invoke(|n| n + 1)
}
