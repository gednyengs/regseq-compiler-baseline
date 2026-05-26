#![no_std]
// FT-017: a `const fn` evaluated in a const context. `k()` runs at
// const-eval time to initialise `const C`, which bridges as an
// `ItemKind::Const` item (value `Uint(U32, 5)`).
const fn k() -> u32 {
    5
}
pub const C: u32 = k();
pub fn f() -> u32 {
    C
}
