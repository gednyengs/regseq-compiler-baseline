//! Interrupt class definition for the semantic model.
//!
//! This is the model-layer home for interrupt class definitions, shared
//! between `regseq-model` and `regseq-backends`. It replaces the previous
//! backend-local `InterruptClassDef` in `regseq-backends::lowering::behavior`.

use serde::{Deserialize, Serialize};

/// A top-level interrupt class definition for the semantic model.
///
/// Interrupt classes are derived from the authored ISR body's branch
/// conditions. They carry symbolic names and mask values so that backends
/// do not hardcode device-specific interrupt mask literals.
///
/// Symbolic names are resolved from the authored source, not from
/// positional ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptClassDef {
    /// Symbolic class name (e.g. "rx", "tx", "error").
    pub name: String,
    /// Bitmask value for this class.
    pub mask_value: u32,
}
