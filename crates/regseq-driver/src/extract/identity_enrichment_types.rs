//! Types for the identity enrichment extraction surface.
//!
//! These types form the canonical Phase 0 handoff: canonical identity,
//! ownership, module containment, field identity, and ISR markers.
//! They derive `Serialize` and `Deserialize` so they can be included
//! directly in the raw frontend artifact JSON.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Canonical path
// ---------------------------------------------------------------------------

/// Fully-qualified item identity derived from `tcx.def_path_str(DefId)`.
///
/// Format: `crate::module::Type`, `crate::module::<impl Trait for Type>::method`.
/// Field paths extend parent: `crate::module::Struct::field_name`.
/// Deterministic within a compilation. Not backend-specific.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanonicalPath(pub String);

// ---------------------------------------------------------------------------
// Top-level enrichment surface
// ---------------------------------------------------------------------------

/// Identity and ownership facts for all items in the driver crate that the
/// semantic IR plan requires. This is the canonical Phase 0 handoff: later
/// tasks consume this surface directly to build `Module`, `ItemStore`, and
/// `CrossLinkTable` without inspecting extraction internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityEnrichmentSurface {
    /// Module containment facts: every discovered module and its direct children.
    pub modules: Vec<ModuleFact>,
    /// All structs with identity and field-level facts.
    pub structs: Vec<StructFact>,
    /// All enums with identity.
    pub enums: Vec<EnumFact>,
    /// All impl blocks with method/constant membership.
    pub impl_blocks: Vec<ImplBlockFact>,
    /// All free (non-method) functions.
    pub free_functions: Vec<FunctionFact>,
}

// ---------------------------------------------------------------------------
// Module containment
// ---------------------------------------------------------------------------

/// A module in the driver crate with its direct children.
///
/// Module facts are extracted from `ItemKind::Mod` in HIR. The crate root
/// is included as a module. Children are identified by canonical path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleFact {
    /// Canonical path of the module (e.g., `"pl011_full_spec::driver"`).
    pub canonical_path: CanonicalPath,
    /// Short module name.
    pub name: String,
    /// Canonical paths of items directly contained in this module.
    /// Includes structs, enums, functions, impl blocks, and sub-modules.
    pub child_items: Vec<CanonicalPath>,
}

// ---------------------------------------------------------------------------
// Struct and field identity
// ---------------------------------------------------------------------------

/// A struct with field-level identity and attribute-based resource classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructFact {
    /// Canonical path of the struct.
    pub canonical_path: CanonicalPath,
    /// Short struct name.
    pub name: String,
    /// Canonical path of the containing module.
    pub module_path: CanonicalPath,
    /// Whether this struct carries `#[rseq::driver]`.
    pub is_driver: bool,
    /// Whether this struct carries `#[rseq::register_block]`.
    pub is_register_block: bool,
    /// Field-level identity facts.
    pub fields: Vec<FieldFact>,
}

/// Field-level identity for cross-link resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldFact {
    /// Short field name.
    pub name: String,
    /// Canonical path: parent struct path + `::` + field name.
    pub canonical_path: CanonicalPath,
    /// Resource classification from `#[rseq::*]` attributes on this field.
    /// `None` if no rseq attribute is present.
    pub rseq_kind: Option<RseqFieldKind>,
}

/// Resource classification derived from `#[rseq::*]` attributes on struct fields.
///
/// This is NOT derived from type-name inspection. Only fields carrying an
/// explicit `#[rseq::*]` attribute receive a classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RseqFieldKind {
    /// Field carries `#[rseq::mmio(...)]`.
    Mmio { id: String },
    /// Field carries `#[rseq::clock(...)]`.
    Clock { id: String },
    /// Field carries `#[rseq::pinctrl(...)]`.
    PinCtrl { id: String },
    /// Field carries `#[rseq::interrupt(...)]`.
    Interrupt { id: String },
    /// Field carries `#[rseq::property(...)]`.
    Property { id: String },
}

// ---------------------------------------------------------------------------
// Enum identity
// ---------------------------------------------------------------------------

/// An enum with canonical identity. Relevant for interrupt-class enums
/// and declaration-identity enums in the driver crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumFact {
    /// Canonical path of the enum.
    pub canonical_path: CanonicalPath,
    /// Short enum name.
    pub name: String,
    /// Canonical path of the containing module.
    pub module_path: CanonicalPath,
    /// Variant names for cross-reference.
    pub variants: Vec<String>,
}

// ---------------------------------------------------------------------------
// Impl block ownership
// ---------------------------------------------------------------------------

/// An impl block with structural membership facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplBlockFact {
    /// Canonical path of the impl block.
    pub canonical_path: CanonicalPath,
    /// Canonical path of the self type.
    pub self_type_path: CanonicalPath,
    /// Short self type name.
    pub self_type_name: String,
    /// If trait impl: the trait's canonical path.
    pub trait_path: Option<CanonicalPath>,
    /// If trait impl: short trait name.
    pub trait_name: Option<String>,
    /// Whether this is an inherent impl (no trait).
    pub is_inherent: bool,
    /// Methods in this impl block, in source order.
    pub methods: Vec<ImplMethodFact>,
    /// Associated constants in this impl block, in source order.
    pub constants: Vec<AssociatedConstantFact>,
}

/// A method within an impl block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplMethodFact {
    /// Short method name.
    pub name: String,
    /// Canonical path of the method.
    pub canonical_path: CanonicalPath,
    /// Whether this method carries `#[rseq::isr]`.
    pub is_isr: bool,
}

/// An associated constant within an impl block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssociatedConstantFact {
    /// Short constant name.
    pub name: String,
    /// Canonical path of the constant.
    pub canonical_path: CanonicalPath,
    /// Type as rendered from HIR.
    pub ty: String,
    /// Canonical path of the owning impl block.
    pub owner_impl_path: CanonicalPath,
    /// Evaluated scalar value of the constant, if it evaluates to a scalar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluated_value: Option<u128>,
    /// HIR initializer expression rendered as a string, preserving authored
    /// definitional relationships (e.g., `Self::ERROR_INTERRUPT_MASK`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initializer_expr: Option<String>,
}

// ---------------------------------------------------------------------------
// Free function identity
// ---------------------------------------------------------------------------

/// A free (non-method) function with signature-level facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionFact {
    /// Short function name.
    pub name: String,
    /// Canonical path of the function.
    pub canonical_path: CanonicalPath,
    /// Canonical path of the containing module.
    pub module_path: CanonicalPath,
    /// Type parameter names (e.g., `["R"]` for `fn foo<R>(...)`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<String>,
    /// Parameters (name and HIR-rendered type).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<FnParamFact>,
    /// Return type as rendered from HIR. `None` for unit return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
}

/// A parameter of a free function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnParamFact {
    /// Parameter name from the HIR pattern.
    pub name: String,
    /// Parameter type as rendered from HIR.
    pub ty: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from identity enrichment extraction.
///
/// Required extraction failures are explicit hard errors. The Phase 0 contract
/// forbids silently degrading to `None` or omitting facts that later phases
/// need.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum IdentityEnrichmentError {
    /// No `#[rseq::driver]`-annotated struct found.
    NoDriverType,
    /// An impl block's self type could not be resolved to a canonical path.
    UnresolvedSelfType { impl_span: String },
    /// A trait impl's trait reference could not be resolved to a canonical path.
    UnresolvedTraitPath { impl_span: String },
    /// A `#[rseq::*]` attribute on a field is present but missing the required
    /// `id = "..."` argument.
    MalformedRseqAttribute {
        field_name: String,
        attr_name: String,
        reason: String,
    },
    /// A struct field's parent struct canonical path could not be resolved.
    UnresolvedFieldParent { field_name: String },
    /// An impl item's canonical path could not be resolved.
    UnresolvedImplItemPath { item_name: String },
}

impl std::fmt::Display for IdentityEnrichmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDriverType => write!(f, "no #[rseq::driver]-annotated struct found"),
            Self::UnresolvedSelfType { impl_span } => {
                write!(f, "unresolved self type in impl block at {impl_span}")
            }
            Self::UnresolvedTraitPath { impl_span } => {
                write!(f, "unresolved trait path in impl block at {impl_span}")
            }
            Self::MalformedRseqAttribute {
                field_name,
                attr_name,
                reason,
            } => write!(
                f,
                "malformed #[rseq::{attr_name}] on field `{field_name}`: {reason}"
            ),
            Self::UnresolvedFieldParent { field_name } => {
                write!(f, "unresolved parent struct for field `{field_name}`")
            }
            Self::UnresolvedImplItemPath { item_name } => {
                write!(f, "unresolved canonical path for impl item `{item_name}`")
            }
        }
    }
}
