#![no_std]
// Private, uncalled function with a non-rigid RPIT signature. It is
// never reached by rustc's mono-item collector (not a pub root, no
// caller), so the bridge creates no reactive Instance for it. During
// Phase-4 `construct_function_instances` its identity signature is
// translated; the `-> impl Fn(i32) -> i32` RPIT return type is an
// `Alias(Opaque)` that `translate_fn_sig` rejects with
// `BridgeError::NonRigidTy`, which the bucket-4 carve-out absorbs by
// recording `BridgeItemFlags.signature_non_rigid = true` and pushing
// no Instance. The `SignatureNonRigid` inspection predicate observes
// that flag.
fn opaque_adder() -> impl Fn(i32) -> i32 {
    |x| x + 1
}
pub fn entry() -> i32 {
    0
}
