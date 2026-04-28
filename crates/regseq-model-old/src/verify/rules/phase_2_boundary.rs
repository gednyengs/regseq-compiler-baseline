//! Phase-2 boundary rule bucket (task 56).
//!
//! Two checks, in canonical resource-then-property order:
//!
//! - **All required bindings populated** (spec.md Section 13 line
//!   1404-1406): every resource whose declared shape is
//!   [`Shape::Single`] must carry `binding == Some(...)` after Phase 2.
//!   `Shape::Optional` and `Shape::Array(_)` resources may stay
//!   `None`.
//! - **All required property `bound_value` populated** (spec.md
//!   Section 13 line 1407): every property whose `required` flag is
//!   `true` must have `bound_value == Some(...)` after Phase 2.
//!
//! Note on "platform file IDs must match declarations" (spec.md line
//! 1408): that rule is detected at join time by task 59 with a
//! `JoinError`, not re-detected here. Detecting it earlier with
//! proper platform-section provenance is strictly better than
//! re-detecting it from a side-record.

use crate::identity::{DeclId, SourceOrigin};
use crate::model::driver::DriverStruct;
use crate::model::module::Module;
use crate::model::regseq_model::RegseqModel;
use crate::model::resources::Shape;

use super::super::{VerifierError, VerifierErrorKind, VerifierLevel, VerifierProvenance};

/// Run Phase-2 boundary checks in deterministic order, returning the
/// first violation.
pub fn run(model: &RegseqModel) -> Result<(), VerifierError> {
    walk_drivers(model, &mut |d| required_bindings_populated(model, d))?;
    walk_drivers(model, &mut |d| required_properties_populated(model, d))?;
    Ok(())
}

fn required_bindings_populated(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<(), VerifierError> {
    for r in &driver.clocks {
        check_required(model, r.id.decl_id(), r.shape, r.binding.is_some(), "clock")?;
    }
    for r in &driver.resets {
        check_required(model, r.id.decl_id(), r.shape, r.binding.is_some(), "reset")?;
    }
    for r in &driver.pinctrl {
        check_required(
            model,
            r.id.decl_id(),
            r.shape,
            r.binding.is_some(),
            "pinctrl",
        )?;
    }
    for r in &driver.interrupts {
        check_required(
            model,
            r.id.decl_id(),
            r.shape,
            r.binding.is_some(),
            "interrupt",
        )?;
    }
    for r in &driver.mmio {
        check_required(model, r.id.decl_id(), r.shape, r.binding.is_some(), "mmio")?;
    }
    Ok(())
}

fn check_required(
    model: &RegseqModel,
    decl_id: DeclId,
    shape: Shape,
    populated: bool,
    slot: &'static str,
) -> Result<(), VerifierError> {
    if shape == Shape::Single && !populated {
        return Err(VerifierError {
            level: VerifierLevel::DriverStruct,
            kind: VerifierErrorKind::RequiredBindingMissing {
                resource: decl_id,
                slot,
            },
            provenance: VerifierProvenance::for_item(
                format!("decl#{}", decl_id.index()),
                lookup_origin(model, decl_id),
            ),
            details: format!(
                "{slot} resource (DeclId {}) is declared as required (shape Single) but \
                 binding == None after Phase 2",
                decl_id.index()
            ),
        });
    }
    Ok(())
}

fn required_properties_populated(
    model: &RegseqModel,
    driver: &DriverStruct,
) -> Result<(), VerifierError> {
    for p in &driver.properties {
        if p.required && p.bound_value.is_none() {
            let decl_id = p.id.decl_id();
            return Err(VerifierError {
                level: VerifierLevel::DriverStruct,
                kind: VerifierErrorKind::RequiredPropertyValueMissing { property: decl_id },
                provenance: VerifierProvenance::for_item(
                    format!("decl#{}", decl_id.index()),
                    lookup_origin(model, decl_id),
                ),
                details: format!(
                    "property (DeclId {}) is declared required but bound_value == None \
                     after Phase 2",
                    decl_id.index()
                ),
            });
        }
    }
    Ok(())
}

fn walk_drivers<F>(model: &RegseqModel, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&DriverStruct) -> Result<(), VerifierError>,
{
    walk_modules(&model.root_module, &mut |m| {
        if let Some(d) = &m.driver {
            f(d)?;
        }
        Ok(())
    })
}

fn walk_modules<F>(root: &Module, f: &mut F) -> Result<(), VerifierError>
where
    F: FnMut(&Module) -> Result<(), VerifierError>,
{
    f(root)?;
    for s in &root.submodules {
        walk_modules(s, f)?;
    }
    Ok(())
}

fn lookup_origin(model: &RegseqModel, decl_id: DeclId) -> SourceOrigin {
    model
        .source_locations
        .get(&decl_id)
        .cloned()
        .unwrap_or(SourceOrigin::CompilerGenerated)
}
