#![no_std]
// IX-016: const-eval x a const item used in a body. `f` loads `const C`,
// so the evaluated value `Uint(U32, 7)` is interned into the const pool
// (predicate `ConstVariant = Uint`).
const C: u32 = 7;
pub fn f() -> u32 {
    C
}
