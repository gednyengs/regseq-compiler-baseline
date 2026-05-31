#![no_std]

// B6 panic -- AK-1 (BoundsCheck) + TM-6 (the Assert terminator). Indexing a fixed
// array with a runtime index sourced from a `static` inserts the always-on
// `Assert { BoundsCheck { len, index } }` (TM-6). The index is read from a static
// (not a literal) so rustc cannot prove it out-of-bounds at compile time -- a
// literal out-of-range index would be a compile-time `unconditional_panic` lint
// error instead of a runtime assert. At runtime index 5 >= len 3, so the assert
// fails and the host panic routine aborts with the section-16 bounds message
// (AK-1). `expect = "panic"` (see manifest); expected.stderr carries the rendered
// `len` / `index` values.

// The out-of-bounds index lives in a `static` so it is a runtime value.
static IDX: usize = 5;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let arr: [u8; 3] = [10, 20, 30];
    let i: usize = IDX;
    // arr[i]: Assert { BoundsCheck { len: 3, index: i } } (TM-6); on the
    // out-of-bounds path (i == 5) it renders AK-1 and aborts.
    let _v = arr[i];
}
