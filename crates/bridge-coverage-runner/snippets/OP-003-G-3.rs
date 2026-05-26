#![no_std]
// OP-003-G-3: `Byte::Reloc(InstanceRef)` inside a `Const::Bytes`. A
// `#[repr(C)]` struct const with a raw-pointer field pointing at a
// static interns as `Const::Bytes([Reloc(..) x ptr-width, Value(7),
// Uninit x padding])` -- the pointer slot becomes per-byte relocation
// cells. Distinct from OP-003-K's top-level `Const::Reloc` (whole
// pointer): here the reloc is a byte cell inside a larger Bytes value.
#[repr(C)]
pub struct S {
    p: *const u32,
    x: u8,
}
pub static Q: u32 = 9;
pub const C: S = S {
    p: &Q as *const u32,
    x: 7,
};
pub fn f() -> S {
    C
}
