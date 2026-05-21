#![no_std]
pub fn f(_: for<'a> fn(&'a u32, &'a u32)) {}
