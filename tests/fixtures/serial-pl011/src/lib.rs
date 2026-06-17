#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq;

mod driver;
pub mod registers;

pub use driver::SerialPl011;
pub use registers::pl011;
