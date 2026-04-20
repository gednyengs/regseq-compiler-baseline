//! Compile-only smoke test for the Phase-7 compat import surface.
//!
//! Proves that the public surface exercised by
//! `regseq_analysis::AnalysisInput` resolves through
//! `regseq_model::compat`. Failing to compile indicates an incomplete
//! import migration.

#[test]
fn uses_compat_only() {
    // Compile-only smoke: demonstrates that
    // AnalysisInput's public surface resolves through
    // regseq_model::compat. Failing to compile proves the
    // migration is incomplete. The re-exported types are the
    // legacy owned structs (not lifetime-parameterized views),
    // so no `<'_>` appears here.
    use regseq_model::compat::{
        BusBinding, CallbackFamily, DeclarationModel, JoinOutcome, JoinedArtifact,
    };
    let _ = std::any::type_name::<DeclarationModel>();
    let _ = std::any::type_name::<JoinedArtifact>();
    let _ = std::any::type_name::<BusBinding>();
    let _ = std::any::type_name::<CallbackFamily>();
    let _ = std::any::type_name::<JoinOutcome>();
}
