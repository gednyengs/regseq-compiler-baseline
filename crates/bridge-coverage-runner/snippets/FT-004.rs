#![no_std]
// FT-004: `ManuallyDrop<T>` over a `Drop` type suppresses the drop. The
// `D` value wrapped in `ManuallyDrop` is NOT dropped, so the bridge
// synthesises NO drop glue -- the runner predicate is the negative
// `Not(SyntheticItemKind = DropGlue)` (a positive witness of the
// drop-suppression guarantee). Contrast IX-020, where a normally-dropped
// struct DOES produce DropGlue.
use core::mem::ManuallyDrop;
struct D;
impl Drop for D {
    fn drop(&mut self) {}
}
pub fn f() {
    let _m = ManuallyDrop::new(D);
}
