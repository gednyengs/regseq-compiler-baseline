#![no_std]
pub fn drive(mut f: impl FnMut(i32) -> i32) -> i32 {
    f(10) + f(20)
}
pub fn entry() -> i32 {
    let mut acc: i32 = 0;
    drive(|x| {
        acc += x;
        acc
    })
}
