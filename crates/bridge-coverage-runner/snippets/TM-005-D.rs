#![no_std]
// TM-005-D: `Call.target == None` (a diverging call has no return edge).
// Calling a `-> !` function lowers to a `Call` terminator whose `target`
// is `None` -- the runner predicate `CallDivergent=true` asserts at least
// one such call exists.
fn diverge() -> ! {
    loop {}
}
pub fn f() {
    diverge()
}
