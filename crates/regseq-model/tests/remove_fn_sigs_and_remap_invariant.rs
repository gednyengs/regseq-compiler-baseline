//! regseq-driver-61 Test Plan B4 unit test: lock the named-invariant
//! `debug_assert_eq!` inside `RegseqModel::remove_fn_sigs_and_remap`
//! that pins the spec.md section 3 invariant
//! `sig_c_abi_lowerings.len() == sigs.len()`. A desynced model
//! passed through `remove_fn_sigs_and_remap` panics in debug builds
//! with the named-invariant message at the function's boundary,
//! NOT with the bare-numeric `permute_index_vec_with_keep` panic at
//! `crates/regseq-model/src/model.rs:3814`. Future desyncs surface
//! at the correct site for debuggers.

use regseq_model::{FnAbi, FnSig, RegseqModel, Safety, StableCrateId, Ty, TyId};

/// Construct a small `RegseqModel` with two interned `FnSig`s but
/// only one `sig_c_abi_lowerings` slot. Calling
/// `remove_fn_sigs_and_remap(&[true, true])` triggers the named
/// invariant in debug builds.
#[test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "named invariant fires only in debug builds"
)]
fn remove_fn_sigs_and_remap_panics_on_desynced_parallel_table() {
    let mut model = RegseqModel::with_primary("test_crate", StableCrateId(1));
    // Push a usable Ty for the FnSig's output (TyId::ZERO must exist
    // -- assertions in `model.validate()` are not invoked here, so a
    // bare `Ty::Bool` push suffices).
    model.types.push(Ty::Bool);

    // Intentionally intern two FnSigs without pushing parallel slots.
    let _ = model.sigs.intern(FnSig {
        inputs: vec![],
        output: TyId::ZERO,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    let _ = model.sigs.intern(FnSig {
        inputs: vec![TyId::ZERO],
        output: TyId::ZERO,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    });
    // sig_c_abi_lowerings is intentionally one entry short to trip
    // the spec.md section 3 invariant.
    model.sig_c_abi_lowerings.push(None);
    assert_eq!(model.sigs.len(), 2);
    assert_eq!(model.sig_c_abi_lowerings.len(), 1);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        model.remove_fn_sigs_and_remap(&[true, true])
    }));

    let payload = result.expect_err(
        "remove_fn_sigs_and_remap must panic in debug builds when sig_c_abi_lowerings is desynced from sigs",
    );
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|s| s.to_string())
        })
        .unwrap_or_default();
    assert!(
        message.contains("sig_c_abi_lowerings.len()") && message.contains("sigs.len()"),
        "panic message must reference the named invariant `sig_c_abi_lowerings.len() must equal sigs.len()`; got: {message:?}",
    );
}
