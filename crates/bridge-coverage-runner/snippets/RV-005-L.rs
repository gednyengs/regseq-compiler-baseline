#![no_std]
pub fn f(fp: fn()) -> *const () {
    fp as *const ()
}
