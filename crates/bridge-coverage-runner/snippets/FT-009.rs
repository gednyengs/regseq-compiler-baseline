#![no_std]
// FT-009: a generic free fn called with concrete args. `id::<u32>` is
// monomorphised into an `Item`-flavored Function instance carrying
// `generic_args = [u32]` -- the runner predicate `MonomorphizedInstance`
// (a plain non-generic call also interns a `Ty::FnDef`, so `TyVariant`
// would not discriminate monomorphisation; the generic-args-bearing
// instance does).
fn id<T>(x: T) -> T {
    x
}
pub fn f() -> u32 {
    id(7u32)
}
