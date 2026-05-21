#![no_std]
#[repr(C)]
pub struct WithRustFn {
    pub f: fn() -> u32,
}
pub extern "C" fn boundary(_s: WithRustFn) {}
