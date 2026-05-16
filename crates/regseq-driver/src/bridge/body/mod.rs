//! MIR body-content bridge translators.

use regseq_model::Body;
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
/// The `'tcx` lifetime parameter is the rustc context lifetime
/// (carrying the borrow of `LocalDecls`). The `'b` lifetime is the
/// shorter borrow of the in-flight `regseq_model::Body` that
/// terminators populate with synthetic locals per audit finding
/// regseq-driver-42-2. The body slot is populated only during Phase-5
/// body materialisation; terminator translators reach the
/// materialised body through [`BodyCtxt::model_body_mut`] when they
/// need to allocate synthetic locals (e.g., the Drop-as-Call
/// structural prelude per spec section 23.4 / 25.7.1).
pub struct BodyCtxt<'tcx, 'b> {
    local_decls: Option<&'tcx mir::LocalDecls<'tcx>>,
    model_body: Option<&'b mut Body>,
}

impl<'tcx, 'b> BodyCtxt<'tcx, 'b> {
    /// Creates a body context without local declarations or a mutable
    /// model body. Intended for translators (e.g., constant lowering)
    /// that have no need for body-local context.
    pub fn empty() -> Self {
        Self {
            local_decls: None,
            model_body: None,
        }
    }

    /// Creates a body context over rustc MIR local declarations
    /// without a mutable model body. Suitable for unit tests and
    /// translators that do not need to allocate synthetic locals.
    pub fn new(local_decls: &'tcx mir::LocalDecls<'tcx>) -> Self {
        Self {
            local_decls: Some(local_decls),
            model_body: None,
        }
    }

    /// Creates a body context with both rustc local declarations and a
    /// mutable model body. Audit-finding regseq-driver-42-2: the
    /// Drop-as-Call collapse and any other terminator that needs to
    /// allocate synthetic locals goes through this mutable Body
    /// reference. Phase-5 body materialisation constructs the
    /// `BodyCtxt` via this constructor so synthetic-local allocations
    /// land in the actual materialised body.
    pub fn new_with_model_body(
        local_decls: &'tcx mir::LocalDecls<'tcx>,
        body: &'b mut Body,
    ) -> Self {
        Self {
            local_decls: Some(local_decls),
            model_body: Some(body),
        }
    }

    pub(crate) fn local_decls(&self) -> Option<&'tcx mir::LocalDecls<'tcx>> {
        self.local_decls
    }

    /// Returns a mutable reference to the model `Body` when the
    /// context was constructed via [`Self::new_with_model_body`].
    /// `None` for contexts that have no mutable body slot (per-test
    /// stubs, constant lowering, etc.).
    pub(crate) fn model_body_mut(&mut self) -> Option<&mut Body> {
        self.model_body.as_deref_mut()
    }
}
