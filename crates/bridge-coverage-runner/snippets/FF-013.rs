#![no_std]
#![feature(fn_ptr_trait)]
use core::marker::FnPtr;
pub fn f(p: fn()) -> *const () {
    p.addr()
}
