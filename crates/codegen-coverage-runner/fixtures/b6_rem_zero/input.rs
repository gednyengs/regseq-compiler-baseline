#![no_std]

// B6 panic -- AK-5 (RemainderByZero). The divisor is read from a `static` so the
// `%` is a genuine runtime operation (a literal `0` divisor would be a
// compile-time `unconditional_panic` lint error); the always-inserted
// `Assert { RemainderByZero }` survives in the bridged MIR. At runtime the divisor
// is 0, so the assert fails and the host panic routine aborts with the section-16
// message. `expect = "panic"` (see manifest); expected.stderr carries the text.

// The zero divisor lives in a `static` (runtime value, not a const) so `a % b` is
// a runtime remainder rather than a const-folded compile error.
static ZERO: i32 = 0;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let a: i32 = 10;
    let b: i32 = ZERO;
    let _r = a % b; // Assert { RemainderByZero } -> AK-5 on the b == 0 path.
}
