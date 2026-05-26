#![no_std]
extern crate alloc;
// RJ-001: `DslRejectionKind::Box`. The bridge rejects `alloc::boxed::Box`
// by design (a `#![no_std]` driver target has no global allocator). A
// `Box<u32>` in a signature -> `BridgeError::DslRejection { kind: Box }`
// (bridge/ty.rs). Expected by-design rejection (expectation = rejected).
pub fn f(_x: alloc::boxed::Box<u32>) {}
