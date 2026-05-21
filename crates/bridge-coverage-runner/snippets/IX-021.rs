#![no_std]
pub trait T {}
pub fn f(_d: &(dyn T + Send)) {}
