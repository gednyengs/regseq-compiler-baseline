#![no_std]
pub struct Payload {
    pub value: i32,
}
pub fn consume(f: impl FnOnce(i32) -> i32) -> i32 {
    f(1)
}
pub fn entry() -> i32 {
    let p = Payload { value: 99 };
    consume(move |n| p.value + n)
}
