#![no_std]
pub trait G {
    fn greet(&self) -> u32 { 0 }
}
pub struct S;
impl G for S {}
pub fn f(s: &S) -> u32 {
    s.greet()
}
