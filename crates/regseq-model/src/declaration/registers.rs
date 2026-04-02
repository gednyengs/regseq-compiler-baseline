//! Register block declarations, descriptor facts, and operation evidence.
//!
//! NOTE on extraction limitation: the raw artifact preserves wrapper method
//! and `containing_fn` for each wrapper call site, but does not preserve the
//! concrete target register for each wrapper call site. The declaration model
//! normalizes this honestly -- `WrapperCallSiteDecl` carries the method and
//! containing function, but not a target register identity.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Set of all register-related declarations and evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterDeclSet {
    /// Register block declarations.
    pub blocks: Vec<RegisterBlockDecl>,
    /// Register descriptor facts (bitfield metadata per register module).
    pub descriptors: Vec<RegisterDescriptorDecl>,
    /// Wrapper call-site evidence (construct-owned).
    pub wrapper_call_sites: Vec<WrapperCallSiteDecl>,
    /// Poll call-site evidence (construct-owned).
    pub poll_call_sites: Vec<PollCallSiteDecl>,
}

/// A normalized register block declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterBlockDecl {
    /// Block name (type name from the register block struct).
    pub name: String,
    /// Fields within this register block.
    pub fields: Vec<RegisterFieldDecl>,
    /// Provenance.
    pub provenance: Provenance,
}

/// A register field within a register block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterFieldDecl {
    /// Field name.
    pub name: String,
    /// Access width in bits, if declared.
    pub access_width: Option<u32>,
    /// Endianness, if declared.
    pub endian: Option<String>,
    /// Side-effect annotation, if declared.
    pub side_effects: Option<String>,
    /// Hardware-update annotation, if declared.
    pub hw_updates: Option<String>,
}

/// A register descriptor fact (bitfield metadata for a register module).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterDescriptorDecl {
    /// Register module name.
    pub register_module: String,
    /// Descriptor fields within this register module.
    pub fields: Vec<RegisterDescriptorFieldDecl>,
    /// Provenance.
    pub provenance: Provenance,
}

/// A single field within a register descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterDescriptorFieldDecl {
    /// Field name within the register.
    pub field_name: String,
    /// Bit offset within the register.
    pub offset: u32,
    /// Bit width of this field.
    pub width: u32,
    /// Named values (enum variants) for this field.
    pub named_values: Vec<RegisterNamedValueDecl>,
}

/// A named value (enum variant) within a register descriptor field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterNamedValueDecl {
    /// Enum variant name.
    pub name: String,
    /// Numeric value.
    pub value: u32,
}

/// A normalized wrapper call-site evidence record.
///
/// NOTE: The raw artifact does not preserve the concrete target register
/// for each wrapper call site. Only the wrapper method and containing
/// function are available. This is an honest representation of the current
/// extraction limitation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapperCallSiteDecl {
    /// Wrapper method kind invoked at this call site.
    pub method: WrapperMethod,
    /// Function containing this call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}

/// Typed register wrapper method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WrapperMethod {
    /// Raw read (no field masking).
    ReadRaw,
    /// Typed read.
    Read,
    /// Typed write.
    Write,
    /// Read-modify-write.
    Modify,
    /// Set (write with specific field bits).
    Set,
}

impl WrapperMethod {
    /// Parse from the raw string representation.
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "read_raw" => Some(Self::ReadRaw),
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "modify" => Some(Self::Modify),
            "set" => Some(Self::Set),
            _ => None,
        }
    }

    /// Return the canonical string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadRaw => "read_raw",
            Self::Read => "read",
            Self::Write => "write",
            Self::Modify => "modify",
            Self::Set => "set",
        }
    }
}

/// A normalized poll call-site evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollCallSiteDecl {
    /// Function containing this poll call site.
    pub containing_fn: String,
    /// Provenance.
    pub provenance: Provenance,
}
