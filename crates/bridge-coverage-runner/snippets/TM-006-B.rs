#![no_std]
// TM-006-B: `AssertKind::Overflow(BinOp, ..)`. Built with
// `overflow_checks = true` (the driver-69 `BuildOverrides` surface): a
// checked integer `a + b` lowers to an `AddWithOverflow` followed by an
// `Assert` terminator whose `msg` is `AssertKind::Overflow(Add, ..)`.
// (Same witness as RV-006-I, which covers the `AddWithOverflow` BinOp.)
pub fn f(a: i32, b: i32) -> i32 {
    a + b
}
