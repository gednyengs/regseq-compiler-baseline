#![no_std]
// OP-003-G-1: `Byte::Uninit` inside a `Const::Bytes`. A `#[repr(C)]`
// struct const with a `u8` followed by a `u32` has 3 padding bytes
// between the fields; the whole-struct const interns as
// `Const::Bytes([Value(1), Uninit, Uninit, Uninit, Value(2), ...])`.
// Returning the whole value (not a field) forces the full struct const
// into the pool rather than const-folding a single field.
#[repr(C)]
pub struct S {
    a: u8,
    b: u32,
}
pub const C: S = S { a: 1, b: 2 };
pub fn f() -> S {
    C
}
