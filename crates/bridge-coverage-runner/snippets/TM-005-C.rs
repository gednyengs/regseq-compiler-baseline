#![no_std]
// TM-005-C: `Call.target == Some(bb)` (a converging call has a return
// edge). A direct call to a non-diverging function lowers to a `Call`
// terminator whose `target` is `Some` -- the runner predicate
// `CallDivergent=false` asserts at least one such call exists. (Same
// witness as TM-005-A, which covers the `Direct` callee axis.)
fn g() {}
pub fn f() {
    g()
}
