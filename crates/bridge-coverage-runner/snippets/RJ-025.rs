#![no_std]
#![feature(unsized_fn_params)]
// RJ-025: `UnsupportedTy`. A bare unsized type (`str`) in argument
// position (via the incomplete `unsized_fn_params` feature) hits the
// sized-context type translation -> `BridgeError::UnsupportedTy
// { ty_repr: "str" }` (bridge/ty.rs). Empirically PRODUCIBLE at HEAD
// (compiles, reaches the bridge, clean Rejected). Expected by-design
// rejection (expectation = rejected).
pub fn f(_x: str) {}
