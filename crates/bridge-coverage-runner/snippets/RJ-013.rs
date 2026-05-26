#![no_std]
// Body-context use of a return-position `impl Trait` (RPIT) opaque from
// a callee. `mk`'s `-> impl Fn() -> u32` return is an `Alias(Opaque)`.
// At HEAD the bridge had no opaque-reveal arm, so `f`'s body -- which
// binds and invokes a value of that opaque type -- rejected with
// `NonRigidTy { kind: Alias, ty_repr: "impl Fn() -> u32" }`.
//
// Per regseq-driver-82 the bridge reveals body-context `Alias(Opaque)`
// nodes to their concrete hidden type (here a closure) during
// per-Instance body materialisation, so `f` now bridges `Ok`: the
// revealed closure is invoked through a `SyntheticItemKind::ClosureCall`
// shim. The opaque bridges in its defining fn (`mk`) as before; only the
// using body needed the reveal.
pub fn mk() -> impl Fn() -> u32 {
    || 0
}
pub fn f() -> u32 {
    let g = mk();
    g()
}
