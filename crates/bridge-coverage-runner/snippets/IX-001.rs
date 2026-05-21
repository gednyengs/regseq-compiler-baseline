#![no_std]
pub fn id<T>(x: T) -> T {
    x
}
pub fn entry() -> u32 {
    id::<u32>(0)
}
