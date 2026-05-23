#![feature(core_intrinsics)]

pub fn raw_sized(data: *const u32) -> *const u32 {
    core::intrinsics::aggregate_raw_ptr(data, ())
}
