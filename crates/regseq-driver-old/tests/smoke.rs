#![feature(rustc_private)]

extern crate rustc_driver;

#[test]
fn run_function_symbol_is_exposed() {
    let run_fn: fn() -> i32 = regseq_driver_old::run;
    let _ = run_fn;
}
