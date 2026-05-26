#![no_std]
// TM-005-A: `CallCallee::Direct(InstanceId)`. A direct call to a named
// function lowers to a `Call` terminator whose callee is `Direct`. The
// SAME witness carries the `target == Some(bb)` (non-diverging) edge
// covered by TM-005-C.
fn g() {}
pub fn f() {
    g()
}
