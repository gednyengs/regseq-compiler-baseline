// regseq-driver-61 Test Plan B2 fixture: closure-family FnSig-intern
// coverage guard. A zero-capture closure expression that is reified
// as a local and passed to `core::mem::drop`, mirroring the existing
// `closure.rs` fixture shape.
//
// The bare-intern closure-call `make_fn_sig` path at
// `crates/regseq-driver/src/bridge/body/terminator.rs:316-331` is
// currently structurally unreachable end-to-end: a direct `f(7)`
// call site at this fixture's `entry` would trip an upstream
// rustc-side ICE in
// `compiler/rustc_hir_analysis/src/collect.rs:1028` because the
// bridge's `callsite_fn_sig` shells out to `tcx.fn_sig(def_id)`
// which panics for closure DefIds with the message "to get the
// signature of a closure, use `args.as_closure().sig()` not
// `fn_sig()`". That is a separate (pre-existing) bridge gap, not
// covered by this task.
//
// What this fixture DOES exercise: the eager closure-family
// synthesis path at `crates/regseq-driver/src/bridge/mod.rs:2278+`
// (`ClosureCall` / `ClosureCaptures` / `ClosureDropGlue` Items)
// during Phase 2 of `run_bridge`. The eager synth has its own slot
// pairing at `bridge/mod.rs:2303-2305`. The integration test
// verifies that the parallel-table invariant
// `sig_c_abi_lowerings.len() == sigs.len()` holds AND that the
// eager synth produced at least one `SyntheticItemKind::ClosureCall`
// Item.

#![no_std]

#[allow(dropping_copy_types)]
pub fn entry() {
    let f = |x: u32| x + 1;
    core::mem::drop(f);
}
