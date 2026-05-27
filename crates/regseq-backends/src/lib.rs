//! Regseq codegen backends (Linux, Zephyr). Generic-C backend entry point.

use regseq_model::RegseqModel;

/// Lower a post-mono `RegseqModel` to a single generic-C translation unit
/// (`generic-c-backend.md` section 15 five-bank order). Phase-1 tasks b-e
/// fill this in; until then it returns [`EmitError::NotImplemented`].
///
/// The contract is fixed by Doc 2 D3: one `.c` string per model (no `.h`),
/// with the harness generating a separate `main.c`. The codegen-coverage
/// harness is this seam's first consumer (its stage 2 calls this function
/// and surfaces `Err` as a stage-2 failure).
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    Err(EmitError::NotImplemented)
}

/// Errors from generic-C emission. Tasks b-e extend this; the only variant
/// this task ships is the not-yet-implemented marker.
///
/// `#[non_exhaustive]` lets later tasks add variants without a breaking
/// change, and keeps the harness decoupled from the error taxonomy (it
/// consumes only `Ok(String)` vs `Err(impl Display)`).
#[derive(Debug)]
#[non_exhaustive]
pub enum EmitError {
    /// The generic-C backend lowering is not yet implemented.
    NotImplemented,
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::NotImplemented => write!(f, "generic-C emission not implemented"),
        }
    }
}

impl std::error::Error for EmitError {}

// The seam (`emit_generic_c` + `EmitError`) is exercised by the
// codegen-coverage runner's `tests/emit_seam.rs`, which bridges a real
// snippet to a `RegseqModel` and asserts the stub returns `Err` and that
// the pipeline tags it as a stage-2 (`Emit`) failure. Covering it there
// (rather than here) keeps this crate the pure seam with no test-only
// model-construction dependency, per the task spec's Test-surface note.
