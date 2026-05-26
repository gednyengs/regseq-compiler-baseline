#![no_std]
// RV-006-I: `BinOp::AddWithOverflow`. Built with `overflow_checks = true`
// (the driver-69 `BuildOverrides` surface): a checked integer `a + b`
// lowers to `Rvalue::BinaryOp(AddWithOverflow, ..)` whose bool result
// feeds an overflow assert. The SAME witness surfaces the overflow
// assert covered by TM-006-B.
pub fn f(a: i32, b: i32) -> i32 {
    a + b
}
