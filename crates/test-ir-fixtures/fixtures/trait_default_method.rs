// Default trait method body coverage fixture for regseq-driver-59
// (Test Plan B9). Declares a trait `Greeter` with a default method
// body `greet` returning `42_u32`, a struct `S`, and an
// `impl Greeter for S {}` that does NOT override `greet`. A top-level
// `entry` fn calls `s.greet()` so the caller-side Instance reaches
// the default body via the regseq-driver-56 call-site
// canonicalization.
//
// Per the regseq-driver-58 Change H amendment recorded at
// `docs/discussions/regseq-model/spec.md:2351-2358`, the default
// body must surface as an `ItemKind::Function` Item at the trait's
// DefPath (`...::Greeter::greet`), and the trait container itself
// remains erased -- no `ItemKind::Trait` variant exists.
//
// Driven by `crates/regseq-model/tests/e2e_trait_default_method.rs`,
// which bridges the fixture end-to-end and asserts both the
// structural Item/Instance shape and the rendered snapshot.

#![no_std]

pub trait Greeter {
    fn greet(&self) -> u32 {
        42
    }
}

pub struct S;
impl Greeter for S {}

pub fn entry(s: &S) -> u32 {
    s.greet()
}
