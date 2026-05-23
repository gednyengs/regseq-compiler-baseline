#![no_std]
pub fn invoke(mut f: impl FnMut(i32)) {
    f(1);
    f(2);
}
pub fn entry() -> i32 {
    let mut total: i32 = 0;
    invoke(|x| total += x);
    total
}
