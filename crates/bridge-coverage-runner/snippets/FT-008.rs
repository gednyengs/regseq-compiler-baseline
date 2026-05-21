#![no_std]
pub trait Tr {
    fn m(&self) -> u32 {
        0
    }
}
pub struct S1;
impl Tr for S1 {
    fn m(&self) -> u32 {
        42
    }
}
pub struct S2;
impl Tr for S2 {}
pub fn entry(s1: &S1, s2: &S2) -> u32 {
    s1.m() + s2.m()
}
