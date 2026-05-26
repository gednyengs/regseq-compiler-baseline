#![no_std]
// RV-003: `Rvalue::Ref(Provenance, Place)`. Taking a reference to a local
// (`&a`) lowers to an `Rvalue::Ref` in the body.
pub fn f() -> u32 {
    let a = 5u32;
    let r = &a;
    *r
}
