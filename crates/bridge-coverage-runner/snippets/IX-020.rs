#![no_std]
// IX-020: a `Drop` impl + an associated const + an inherent method on
// the SAME struct, all exercised together. The single `Drop` field makes
// the drop glue a single-block shim (NOT the FT-003 multi-Drop B-RPO
// path); the predicate `SyntheticItemKind = DropGlue` confirms the drop
// path is synthesised alongside the assoc const and method.
struct S(u32);
impl Drop for S {
    fn drop(&mut self) {}
}
impl S {
    const A: u32 = 7;
    fn m(&self) -> u32 {
        self.0
    }
}
pub fn f() -> u32 {
    let s = S(1);
    s.m() + S::A
}
