#![no_std]

mod driver;
pub mod registers;

pub use driver::Pl011FullSpec;
pub use registers::pl011;
