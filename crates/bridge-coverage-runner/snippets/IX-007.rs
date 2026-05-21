#![no_std]
pub trait T {
    fn m(&self) -> u32;
}
pub struct S;
impl T for S {
    fn m(&self) -> u32 {
        0
    }
}
impl Drop for S {
    fn drop(&mut self) {}
}
pub fn f(s: &S) -> u32 {
    let d: &dyn T = s;
    d.m()
}
