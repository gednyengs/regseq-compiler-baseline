//! MIR body-content bridge translators.

use rustc_middle::mir;

pub mod operand;
pub mod place;
pub mod rvalue;
pub mod statement;

pub use operand::{lower_runtime_check_operand, translate_operand};
pub use place::translate_place;
pub use rvalue::translate_rvalue;
pub use statement::translate_statement;

/// Per-body translation context shared by Place, Operand, Rvalue, and Statement translation.
pub struct BodyCtxt<'tcx> {
    local_decls: Option<&'tcx mir::LocalDecls<'tcx>>,
}

impl<'tcx> BodyCtxt<'tcx> {
    /// Creates a body context without local declarations.
    pub fn empty() -> Self {
        Self { local_decls: None }
    }

    /// Creates a body context over rustc MIR local declarations.
    pub fn new(local_decls: &'tcx mir::LocalDecls<'tcx>) -> Self {
        Self {
            local_decls: Some(local_decls),
        }
    }

    pub(crate) fn local_decls(&self) -> Option<&'tcx mir::LocalDecls<'tcx>> {
        self.local_decls
    }
}
