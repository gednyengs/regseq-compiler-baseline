//! Sort-before-serialize helpers used by the section joiners.
//!
//! Phase 2 is deterministic: the same (`RegseqModel`, `PlatformFile`)
//! pair must produce a byte-identical serialization after the join.
//! This module centralizes the canonical order policy.
//!
//! # Order policy
//!
//! - [`sorted_construct_sections`]: yield every keyed
//!   [`ConstructSection`] in **global** `(kind, id)` lexicographic
//!   order. This is the task-spec canonical iteration order for
//!   construct sections (`.harness/tasks/regseq-model/59-populate-bindings.md`,
//!   Phase-2 determinism bullet #2). Section dispatch inside the
//!   join logic consumes this single sorted stream rather than
//!   filtering per-kind, so the global ordering is reflected in
//!   every binding write.
//! - [`sorted_properties`]: yield [`PropertyEntry`] references
//!   sorted by `key` (stable, lexicographic).
//!
//! # Why not rely on the parser?
//!
//! The TOML parser preserves input order (`Vec<ConstructSection>`);
//! that is not guaranteed to be deterministic across TOML
//! serializers or hand-edited files. Sorting here makes the join
//! independent of the `PlatformFile` input order.

use crate::platform::{ConstructSection, PropertyEntry};

/// Return every keyed construct section in `(kind, id)` lexicographic
/// order. Sections with no `section_key` are skipped (the canonical
/// platform-file schema mandates `[kind.id]` for every construct
/// section).
///
/// Each element is `(kind, id, &ConstructSection)`. Callers dispatch
/// on `kind` to route to the per-kind binding-build logic.
pub(crate) fn sorted_construct_sections(
    sections: &[ConstructSection],
) -> Vec<(&str, &str, &ConstructSection)> {
    let mut out: Vec<(&str, &str, &ConstructSection)> = sections
        .iter()
        .filter_map(|s| {
            s.section_key
                .as_deref()
                .map(|k| (s.section_kind.as_str(), k, s))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)));
    out
}

/// Return `[properties]` entries sorted by `key`.
pub(crate) fn sorted_properties(properties: &[PropertyEntry]) -> Vec<&PropertyEntry> {
    let mut out: Vec<&PropertyEntry> = properties.iter().collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}
