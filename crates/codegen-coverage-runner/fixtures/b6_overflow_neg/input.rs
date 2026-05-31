#![no_std]

// B6 panic -- AK-3 (OverflowNeg). With `-C overflow-checks` on (manifest
// `overflow_checks = true`), unary `-x` lowers to `Assert { OverflowNeg(x) }`. The
// operand is `i32::MIN` read from a `static` so the negation is a genuine runtime
// operation -- a literal `-i32::MIN` would be a compile-time `arithmetic_overflow`
// lint error. At runtime `-i32::MIN` overflows i32, the assert fails, and the host
// panic routine aborts with the fixed section-16 message. `expect = "panic"`.

// `i32::MIN` lives in a `static` (runtime value, not a const) so the negation is
// not const-evaluated into a compile-time overflow error.
static IMIN: i32 = i32::MIN;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let x: i32 = IMIN;
    let _n = -x; // Assert { OverflowNeg(x) } -> AK-3 (negating i32::MIN overflows).
}
