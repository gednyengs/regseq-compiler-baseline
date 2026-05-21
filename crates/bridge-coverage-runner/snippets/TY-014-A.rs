#![no_std]
pub trait R {
    fn r(&self) -> u32;
}
pub struct P;
impl R for P {
    fn r(&self) -> u32 { 0 }
}
pub fn f(p: &P) -> u32 {
    let d: &dyn R = p;
    d.r()
}
