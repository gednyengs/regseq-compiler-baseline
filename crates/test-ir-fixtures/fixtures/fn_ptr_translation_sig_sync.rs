// regseq-driver-61 Test Plan B3 fixture: `TyKind::FnPtr`-translation
// latent-site regression guard. Exercises the `TyKind::FnPtr` branch
// at `crates/regseq-driver/src/bridge/ty.rs:135-179` for both the
// `FnAbi::Rust` (`fn(u32) -> u32`) and the `FnAbi::C`
// (`extern "C" fn(u32) -> u32`) variants. The Rust-ABI FnPtr must
// land with `sig_c_abi_lowerings[id] == None`; the C-ABI FnPtr
// must land with `sig_c_abi_lowerings[id] == Some(_)` per spec.md
// section 12 step 5 "Some iff `FnAbi::C`" presence rule.

#![no_std]

pub fn helper_rust(x: u32) -> u32 {
    x
}
pub extern "C" fn helper_c(x: u32) -> u32 {
    x
}

pub fn entry() -> u32 {
    let p_rust: fn(u32) -> u32 = helper_rust;
    let p_c: extern "C" fn(u32) -> u32 = helper_c;
    p_rust(0) + p_c(0)
}
