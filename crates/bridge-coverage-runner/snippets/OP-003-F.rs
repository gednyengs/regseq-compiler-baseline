#![no_std]
// OP-003-F: `Const::ZST`. Coercing a zero-sized `FnDef` to a `fn()`
// pointer interns the ZST `FnDef` value as the single canonical
// `Const::ZST`. (A bare unit value would intern no const; the ZST const
// arises from a zero-sized `FnDef` operand.)
fn g() {}
pub fn f() {
    let _p: fn() = g;
}
