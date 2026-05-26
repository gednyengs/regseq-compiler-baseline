#![no_std]
// RJ-029: `UnsupportedConst`. A const-generic argument (`W<3>`) is
// rejected by design at `translate_generic_args` (bridge/ty.rs) ->
// `BridgeError::UnsupportedConst { const_repr: "3_usize" }`. Const-generic
// args are out of scope for driver specs (cross-ref C1 / driver-37).
// Expected by-design rejection (expectation = rejected).
pub struct W<const N: usize>;
pub fn f(_w: W<3>) {}
