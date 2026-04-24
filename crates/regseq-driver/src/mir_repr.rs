//! Thin re-export shim over the driver-owned MIR extraction schema.
//!
//! All MIR body types are defined once in
//! `crate::mir_body` and re-exported here for backward-compatibility
//! with existing driver code and test harnesses.
//! Driver-specific surface types (`MirBodySurface`, `MirExtractedFn`)
//! remain defined locally.

// Re-export driver-owned MIR extraction types.
pub use crate::mir_body::{
    ConstField, FloatWidth, IntWidth, MirAggregateKind, MirBasicBlockData, MirBinOp, MirBody,
    MirBorrowKind, MirCastKind, MirConstValue, MirConstant, MirFieldTy, MirLocalDecl,
    MirMutability, MirOperand, MirPlace, MirProjectionElem, MirRvalue, MirSourceInfo,
    MirSourceScope, MirStatement, MirStatementKind, MirSwitchTargets, MirTerminator,
    MirTerminatorKind, MirTy, MirUnOp, MirUnwindAction, MirVariantTy, MonomorphizationKey,
};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Driver-specific body surface types
// ---------------------------------------------------------------------------

/// All extracted MIR function bodies for a driver crate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirBodySurface {
    /// Extracted function bodies in discovery order.
    pub functions: Vec<MirExtractedFn>,
}

/// A single extracted function with MIR body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MirExtractedFn {
    /// Function name as authored.
    pub name: String,
    /// Stable def-path string for provenance.
    pub def_path: String,
    /// Function classification.
    pub kind: MirFnKind,
    /// Receiver style.
    pub receiver: String,
    /// Parameters (excluding self).
    pub params: Vec<MirFnParam>,
    /// Return type, if non-unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<MirTy>,
    /// The MIR body.
    pub body: MirBody,
    /// Names of helpers called from this function's MIR.
    pub helper_calls: Vec<String>,
    /// Monomorphization key for generic-instantiated functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monomorphization: Option<MonomorphizationKey>,
}

/// Function classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum MirFnKind {
    /// Trait callback method.
    Callback {
        trait_name: String,
        method_name: String,
    },
    /// ISR handler.
    Isr,
    /// Helper on the driver type.
    Helper,
    /// Free function.
    FreeFunction,
}

/// Function parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MirFnParam {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: MirTy,
}
