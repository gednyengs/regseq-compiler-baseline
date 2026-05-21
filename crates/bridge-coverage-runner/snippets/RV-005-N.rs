#![no_std]
pub fn f(g: for<'a> fn(&'a u32)) -> fn(&'static u32) {
    g
}
