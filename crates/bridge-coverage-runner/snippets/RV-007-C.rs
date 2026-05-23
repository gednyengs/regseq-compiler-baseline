#![feature(core_intrinsics)]

pub fn meta_slice(p: *const [u32]) -> usize {
    core::intrinsics::ptr_metadata(p)
}

pub fn meta_sized(p: *const u32) {
    let _m: () = core::intrinsics::ptr_metadata(p);
}
