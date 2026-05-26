#![no_std]
// OP-003-C: `Const::Int(IntKind, u128)`. A signed-integer literal in a
// body interns a `Const::Int(I32, 7)` scalar into the const pool.
pub fn f() -> i32 {
    7
}
