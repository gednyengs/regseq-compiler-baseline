#![no_std]
// SK-011: `SyntheticItemKind::VTableDropShim { principal, self_ty }`. A
// `dyn T` over a concrete type that also implements `Drop` synthesises a
// vtable drop shim (the vtable's drop slot).
trait T {
    fn m(&self);
}
struct S;
impl Drop for S {
    fn drop(&mut self) {}
}
impl T for S {
    fn m(&self) {}
}
fn take(x: &dyn T) {
    x.m()
}
pub fn g() {
    let s = S;
    take(&s);
}
