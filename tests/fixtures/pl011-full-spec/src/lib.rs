#![no_std]

mod driver;
pub mod registers;

pub use driver::Pl011FullSpec;
pub use registers::Pl011Registers;
