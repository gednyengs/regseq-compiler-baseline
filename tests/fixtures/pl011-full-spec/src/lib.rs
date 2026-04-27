#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

mod driver;
pub mod registers;

pub use driver::Pl011FullSpec;
pub use registers::pl011;
