//! Semantic type universe for the Regseq internal representation.
//!
//! `SemanticType` covers all types that can appear in function signatures,
//! local declarations, struct fields, enum variants, and constant definitions
//! within the semantic model.
//!
//! `ResourceKind` is the shared resource-family discriminant used by both
//! the semantic IR and the portable lowering plan.

use serde::{Deserialize, Serialize};

use crate::item::ItemId;

/// The semantic type model for the Regseq internal representation.
///
/// This is the type universe of the semantic model. It covers all types
/// that can appear in function signatures, local declarations, struct
/// fields, enum variants, and constant definitions.
///
/// `Opaque` is a diagnostic escape hatch for types the compiler cannot yet
/// classify. Backends must not emit code from `Opaque` types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum SemanticType {
    // --- Scalar types ---
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,

    // --- Unit / void ---
    /// Unit type, corresponding to C `void`.
    Void,

    // --- Compound types ---
    /// Named struct type (by item reference).
    Struct {
        id: ItemId,
        name: String,
    },
    /// Named enum type (by item reference).
    Enum {
        id: ItemId,
        name: String,
    },
    /// Fixed-size array.
    Array {
        element: Box<SemanticType>,
        len: usize,
    },

    // --- Reference types ---
    /// Shared reference.
    Ref {
        inner: Box<SemanticType>,
    },
    /// Mutable reference.
    RefMut {
        inner: Box<SemanticType>,
    },

    // --- Carrier types ---
    /// `Option<T>` -- may or may not hold a value.
    Option {
        inner: Box<SemanticType>,
    },
    /// `Result<T, E>` -- success or error.
    Result {
        ok: Box<SemanticType>,
        err: Box<SemanticType>,
    },

    // --- Domain-specific types ---
    /// Opaque driver data (the `self` pointer, `struct driver_data *`).
    DriverData,
    /// Register block reference.
    RegisterBlock {
        name: String,
    },
    /// Register value with known access width.
    RegisterValue {
        width: u32,
    },
    /// Resource handle (clock, reset, pinctrl).
    ResourceHandle {
        kind: ResourceKind,
    },

    // --- Deferred / opaque ---
    /// Type that the semantic model does not yet fully represent.
    /// Carries a description for diagnostics, not for emission.
    Opaque {
        description: String,
    },
}

/// Resource family discriminant.
///
/// Re-exported from [`crate::model::resources::ResourceKind`], its
/// canonical home. Kept here for transitional compatibility while
/// `semantic/` is in the process of being deleted.
pub use crate::model::resources::ResourceKind;
