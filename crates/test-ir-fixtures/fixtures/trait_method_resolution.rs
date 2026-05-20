// Static-dispatch trait-method callee-canonicalization fixture for
// regseq-driver-56. Mirrors the playground reproduction case
// (`tests/fixtures/playground/src/lib.rs`): a trait with one method,
// a concrete impl, and a top-level fn that invokes the trait method
// on a concrete receiver.
//
// Driven by
// `crates/regseq-driver/tests/e2e_trait_method_resolution.rs` which
// bridges the fixture end-to-end, pretty-prints the resulting model,
// and asserts the rendered call site resolves to the impl method's
// DefPath (terminal `m` segment under a `{impl#N}` ancestor).
//
// The fixture is `#![no_std]` and free of `regseq::*` macros so the
// bridge sees it as plain Rust source.

#![no_std]

pub trait T {
    fn m(&self) -> u32;
}
pub struct S;
impl T for S {
    fn m(&self) -> u32 {
        42
    }
}
pub fn caller(s: &S) -> u32 {
    s.m()
}
