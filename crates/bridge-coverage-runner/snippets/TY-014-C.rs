#![no_std]
pub trait Trait {}
pub fn f(_d: &(dyn Trait + Sync)) {}
