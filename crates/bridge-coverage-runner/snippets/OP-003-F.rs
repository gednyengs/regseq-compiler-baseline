#![no_std]
// Const::ZST coverage: the unit value `()` passed as a CALL OPERAND lowers to
// `Operand::Constant(Const::ZST)` (bridge const_.rs `ConstValue::ZeroSized ->
// Const::ZST`). Unlike a bare `let _u = ()` (elided by the RemoveZsts pass), a
// ZST call argument survives as a real const operand. Previously this row used a
// fn-item -> fn-pointer coercion, but that operand now carries function
// provenance as Const::Reloc (regseq-backends-20); the zero-sized-value variant
// is exercised here instead.
fn take(_z: ()) {}
pub fn f() {
    take(());
}
