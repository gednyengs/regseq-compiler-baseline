//! MIR body-content bridge translators.

use rustc_middle::mir;

pub mod basic_block;
pub mod materialise;
pub mod operand;
pub mod place;
pub mod rvalue;
pub mod statement;
pub mod terminator;
pub mod vtable_indirect;

pub use basic_block::translate_basic_block;
pub use materialise::{materialise_function_body, run_phase5_body_passes, InstanceHandle};
pub use operand::{lower_runtime_check_operand, translate_operand};
pub use place::translate_place;
pub use rvalue::translate_rvalue;
pub use statement::translate_statement;
pub use terminator::translate_terminator;
pub use vtable_indirect::rewrite_vtable_indirect;

/// Per-body translation context shared by Place, Operand, Rvalue, and Statement translation.
///
/// `model_body` is set by `materialise_function_body` so the
/// terminator-side Drop-as-Call prelude can allocate synthetic locals
/// in the model body (`Body::push_synthetic_local`, spec.md section
/// 25.7.1). Test entry points that exercise individual translators
/// (`translate_terminator`, `translate_statement`, etc.) leave it
/// unset; Drop-as-Call returns an error when the model body is
/// missing.
pub struct BodyCtxt<'tcx, 'mb> {
    local_decls: Option<&'tcx mir::LocalDecls<'tcx>>,
    model_body: Option<&'mb mut regseq_model::Body>,
}

impl<'tcx, 'mb> BodyCtxt<'tcx, 'mb> {
    /// Creates a body context without local declarations.
    pub fn empty() -> Self {
        Self {
            local_decls: None,
            model_body: None,
        }
    }

    /// Creates a body context over rustc MIR local declarations.
    pub fn new(local_decls: &'tcx mir::LocalDecls<'tcx>) -> Self {
        Self {
            local_decls: Some(local_decls),
            model_body: None,
        }
    }

    /// Creates a body context over rustc MIR local declarations with a
    /// mutable model `Body` for synthetic-local allocation.
    pub fn new_with_model_body(
        local_decls: &'tcx mir::LocalDecls<'tcx>,
        model_body: &'mb mut regseq_model::Body,
    ) -> Self {
        Self {
            local_decls: Some(local_decls),
            model_body: Some(model_body),
        }
    }

    pub(crate) fn local_decls(&self) -> Option<&'tcx mir::LocalDecls<'tcx>> {
        self.local_decls
    }

    pub(crate) fn model_body_mut(&mut self) -> Option<&mut regseq_model::Body> {
        self.model_body.as_deref_mut()
    }
}
