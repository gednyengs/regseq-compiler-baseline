#![no_std]

mod driver;
pub mod registers;

pub use driver::Pl011SpecSlice;
pub use registers::Pl011Registers;
