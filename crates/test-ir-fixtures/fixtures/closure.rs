#![no_std]

// Phase-1a closure-family synthesis fixture.
//
// The `|x| x + 1` zero-capture closure expression is the source of
// the single synthesised ClosureCaptures discriminator Item plus the
// inherited {Call, CallMut, CallOnce} ClosureCall arms exercised by
// the `regseq-model-95` (P1a-E2E-03) integration test.
//
// `make_inc`'s body reifies the closure as a local and passes it to
// `core::mem::drop` to keep the closure type live through the bridge
// pipeline:
//   * The body's `Fn::call` path is not exercised, so the bridge
//     never has to translate the closure's `FnOnce::Output`
//     projection (which `translate_ty` rejects as a non-rigid
//     `TyKind::Alias`).
//   * `core::mem::drop::<F>(F)` is monomorphised against the closure
//     type, which forces `translate_ty` (called during body
//     materialisation) to intern the closure's `Ty::Closure` and the
//     parallel `Ty::Struct(ClosureCaptures, args, layout)` layout
//     view per stage-3 D6.3.
//   * The closure is `Copy` (no captures = no `Drop` impl), so the
//     `drop` call is a no-op at runtime; rustc emits a
//     `dropping_copy_types` lint we silence below.
//
// The single-function form keeps the spec's "exactly one
// ClosureCaptures" invariant intact and produces a zero-sized
// captures struct that exercises the canonical `UNIT_LAYOUT` path
// per `spec.md:3849-3854`.
#[allow(dropping_copy_types)]
pub fn make_inc() {
    let f = |x: u32| x + 1;
    core::mem::drop(f);
}
