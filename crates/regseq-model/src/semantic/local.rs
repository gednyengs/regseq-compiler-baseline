//! Semantic local environment types.
//!
//! Every function body in the semantic IR has an explicit local environment.
//! Each local carries a stable name, a semantic type, and an origin that
//! records how the local was introduced.

use serde::{Deserialize, Serialize};

use super::types::SemanticType;

/// Unique identifier for a local within a single `SemanticBody`.
///
/// `LocalId` is function-local only. It must not be reused as a cross-function
/// or cross-layer identity key.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct LocalId(pub u32);

/// A semantic local variable with identity, name, type, and origin.
///
/// Backends never receive anonymous or typeless locals. The MIR interpreter
/// assigns all local names during lowering; backends use them directly for
/// C variable emission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticLocal {
    /// Unique identifier within the function.
    pub id: LocalId,
    /// Stable name -- always a valid C identifier.
    ///
    /// Assigned by the MIR interpreter using:
    /// - authored param names (`uart_clk_hz`, `baud_rate`)
    /// - MIR debug names for authored locals (`handled`, `clear_mask`)
    /// - semantic derivation for computation temps (`ibrd`, `fbrd`, `div64`)
    /// - deterministic fallback (`t6`, `t9`) for unnamed intermediates
    pub name: String,
    /// Semantic type.
    pub ty: SemanticType,
    /// How this local was introduced.
    pub origin: LocalOrigin,
}

/// How a semantic local was introduced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalOrigin {
    /// Function parameter (position in signature).
    /// For methods, `self` is always position 0.
    Param { position: usize },
    /// User-declared local variable with authored name.
    Authored,
    /// Compiler-derived intermediate (semantically meaningful temp).
    Derived,
    /// Return place.
    ReturnPlace,
}
