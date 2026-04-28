//! Lint rule: a property marked required also declares a default.
//!
//! A `required` property must be supplied by the platform file, so
//! the declaration-side default can never be consumed. Surfacing this
//! as an [`Advisory`] lint alerts reviewers to either drop the
//! default or flip the field to `Option<T>`.
//!
//! [`Advisory`]: super::super::LintLevel::Advisory

use crate::identity::SourceOrigin;
use crate::model::driver::DriverStruct;
use crate::model::module::Module;
use crate::model::regseq_model::RegseqModel;

use super::super::{LintFinding, LintKind, LintLevel};
use crate::verify::VerifierProvenance;

/// Append a [`LintFinding`] for every property that is both marked
/// `required` and declares a non-`None` `default_value`.
///
/// Properties are walked in per-driver declaration order (the index
/// order of `DriverStruct.properties`), which makes the rule's
/// append order deterministic on its own; the aggregating sorter
/// re-orders across rules.
pub fn check(model: &RegseqModel, out: &mut Vec<LintFinding>) {
    walk_drivers(&model.root_module, &mut |driver| {
        for p in &driver.properties {
            if p.required && p.default_value.is_some() {
                let decl_id = p.id.decl_id();
                let origin = model
                    .source_locations
                    .get(&decl_id)
                    .cloned()
                    .unwrap_or(SourceOrigin::CompilerGenerated);
                out.push(LintFinding {
                    level: LintLevel::Advisory,
                    kind: LintKind::PropertyRequiredWithDefault { property: decl_id },
                    provenance: VerifierProvenance::for_item(
                        format!("decl#{}", decl_id.index()),
                        origin,
                    ),
                    details: format!(
                        "property (DeclId {}) is marked `required` yet declares a default \
                         value; the default cannot be consumed because required properties \
                         must be supplied by the platform file",
                        decl_id.index()
                    ),
                });
            }
        }
    });
}

fn walk_drivers<F>(root: &Module, f: &mut F)
where
    F: FnMut(&DriverStruct),
{
    if let Some(d) = &root.driver {
        f(d);
    }
    for s in &root.submodules {
        walk_drivers(s, f);
    }
}
