#![no_std]
// OP-003-K: `Const::Reloc(InstanceRef)`. A scalar function-pointer const
// item (`const P: fn() = g`) interns a top-level whole-pointer
// relocation `Const::Reloc(InstanceRef { .. })` referencing `g`'s
// instance. (NB: a `let p: fn() = g` local instead interns `Const::ZST`
// from the FnDef coercion -- the *const item* path produces the Reloc;
// see OP-003-F for the ZST shape.)
fn g() {}
pub const P: fn() = g;
pub fn f() {
    (P)()
}
