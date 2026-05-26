#![no_std]
// FT-019: an associated const on an inherent `impl` block. `S::A`
// bridges as an `ItemKind::Const` item (value `Uint(U32, 7)`).
struct S;
impl S {
    const A: u32 = 7;
}
pub fn f() -> u32 {
    S::A
}
