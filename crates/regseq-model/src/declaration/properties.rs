//! Firmware-property declarations and property-side lints.
//!
//! Normalizes property field declarations and associated lints from the
//! raw artifact's `firmware_properties` family. Property-side lints have
//! an explicit home here rather than being mixed into a generic lint bag.

use serde::{Deserialize, Serialize};

use super::provenance::Provenance;

/// Set of firmware-property declarations and lints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDeclSet {
    /// Firmware property field declarations.
    pub fields: Vec<PropertyFieldDecl>,
    /// Property-side lints.
    pub lints: Vec<PropertyLintDecl>,
}

/// A normalized firmware-property field declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyFieldDecl {
    /// Driver type that declares this property.
    pub driver_type: String,
    /// Field name on the driver struct.
    pub field_name: String,
    /// Field type as declared (Rust type string).
    pub field_type: String,
    /// Property identifier matching `#[rseq::property(id = "...")]`.
    pub property_id: String,
    /// Default literal value, if declared.
    pub default_literal: Option<String>,
    /// Vendor prefix, if declared.
    pub vendor_prefix: Option<String>,
    /// Whether the property is required.
    pub required: bool,
    /// Provenance.
    pub provenance: Provenance,
}

/// A property-side lint finding.
///
/// These are declaration-model lints, not general-purpose diagnostics.
/// They have an explicit home here rather than being mixed into a generic
/// diagnostics bag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyLintDecl {
    /// A property marked as required also has a default value.
    RequiredWithDefault {
        /// Driver type that declares this property.
        driver_type: String,
        /// Field name.
        field_name: String,
        /// Property id.
        property_id: String,
        /// Provenance.
        provenance: Provenance,
    },
}
