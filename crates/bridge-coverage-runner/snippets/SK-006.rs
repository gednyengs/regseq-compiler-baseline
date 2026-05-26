#![no_std]
// SK-006: `SyntheticItemKind::ClosureDropGlue { closure }`. A closure
// captures a `Drop` type by move; dropping the closure synthesises a
// closure drop-glue Item.
struct D;
impl Drop for D {
    fn drop(&mut self) {}
}
pub fn f() {
    let d = D;
    let c = move || {
        let _ = &d;
    };
    core::mem::drop(c);
}
