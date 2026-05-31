#![no_std]

// B6 panic -- AK-2 (Overflow). With `-C overflow-checks` on (manifest
// `overflow_checks = true`), `a + b` lowers to an `AddWithOverflow` followed by
// `Assert { Overflow(Add, a, b) }`. One addend is read from a `static` so the sum
// is a genuine runtime computation -- a literal `i32::MAX + 1` would be a
// compile-time `arithmetic_overflow` lint error. At runtime the sum overflows
// i32, the assert fails, and the host panic routine aborts with the section-16
// overflow message (the operator verb "add"). `expect = "panic"` (see manifest).

// `i32::MAX` lives in a `static` (runtime value, not a const) so the addition is
// not const-evaluated into a compile-time overflow error.
static AMAX: i32 = i32::MAX;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let a: i32 = AMAX;
    let b: i32 = 1;
    let _s = a + b; // AddWithOverflow + Assert { Overflow(Add) } -> AK-2.
}
