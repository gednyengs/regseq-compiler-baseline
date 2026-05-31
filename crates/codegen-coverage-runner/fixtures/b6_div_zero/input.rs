#![no_std]

// B6 panic -- AK-4 (DivisionByZero). The divisor is read from a `static` so the
// division is a genuine runtime operation: a literal `0` divisor would be
// const-evaluated into a compile-time `unconditional_panic` lint error, whereas a
// static read is opaque to const-eval, leaving the always-inserted
// `Assert { DivisionByZero }` in the bridged MIR. At runtime the divisor is 0, so
// the assert fails and the host panic routine aborts with the section-16 message.
// `expect = "panic"` (see manifest); expected.stderr carries the rendered text.

// The zero divisor lives in a `static` (runtime value, not a const) so `a / b` is
// a runtime division rather than a const-folded compile error.
static ZERO: i32 = 0;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let a: i32 = 10;
    let b: i32 = ZERO;
    let _q = a / b; // Assert { DivisionByZero } -> AK-4 on the b == 0 path.
}
