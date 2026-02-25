#![feature(rustc_private)]

extern crate rustc_driver;

fn main() {
    std::process::exit(regseq_driver::run())
}
