#![no_std]
// RV-006-A: `BinOp::Add`. With overflow checks OFF (the default build),
// integer `a + b` lowers to a plain `Rvalue::BinaryOp(Add, ..)` with no
// overflow assert. (Contrast RV-006-I, which builds with overflow checks
// ON to surface `AddWithOverflow`.)
pub fn f(a: u32, b: u32) -> u32 {
    a + b
}
