//! Wrapper module to include body and MIR extraction modules with correct
//! module hierarchy for `super::mir_extract` resolution.

#[path = "../../src/extract/mir_extract.rs"]
pub(crate) mod mir_extract;

#[path = "../../src/extract/body.rs"]
pub(crate) mod body;

#[path = "../../src/extract/identity_enrichment_types.rs"]
pub(crate) mod identity_enrichment_types;

#[path = "../../src/extract/identity_enrichment.rs"]
pub(crate) mod identity_enrichment;
