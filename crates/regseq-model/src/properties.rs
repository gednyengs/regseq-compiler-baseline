//! Property-specific validation for the Layer 2 `[properties]` section.
//!
//! Contains parser-side and section-local validation rules for property
//! entries. This module does not perform cross-layer checks against Layer 1
//! declarations or Rust field types.

use crate::platform::PlatformValue;

/// Returns the human-readable kind name for a property value.
///
/// Useful for diagnostic messages and debug-friendly inspection.
pub fn value_kind_name(value: &PlatformValue) -> &'static str {
    match value {
        PlatformValue::Integer(_) => "integer",
        PlatformValue::Boolean(_) => "boolean",
        PlatformValue::String(_) => "string",
        PlatformValue::List(_) => "list",
        PlatformValue::Table(_) => "table",
    }
}

/// Check whether a property value is a scalar kind (integer, boolean, string).
pub fn is_scalar(value: &PlatformValue) -> bool {
    matches!(
        value,
        PlatformValue::Integer(_) | PlatformValue::Boolean(_) | PlatformValue::String(_)
    )
}

/// Check whether a property value is an aggregate kind (list, table).
pub fn is_aggregate(value: &PlatformValue) -> bool {
    matches!(value, PlatformValue::List(_) | PlatformValue::Table(_))
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn value_kind_names_are_correct() {
        assert_eq!(value_kind_name(&PlatformValue::Integer(1)), "integer");
        assert_eq!(value_kind_name(&PlatformValue::Boolean(true)), "boolean");
        assert_eq!(
            value_kind_name(&PlatformValue::String("x".into())),
            "string"
        );
        assert_eq!(value_kind_name(&PlatformValue::List(vec![])), "list");
        assert_eq!(
            value_kind_name(&PlatformValue::Table(BTreeMap::new())),
            "table"
        );
    }

    #[test]
    fn scalar_and_aggregate_classification() {
        assert!(is_scalar(&PlatformValue::Integer(1)));
        assert!(is_scalar(&PlatformValue::Boolean(false)));
        assert!(is_scalar(&PlatformValue::String("x".into())));
        assert!(!is_scalar(&PlatformValue::List(vec![])));
        assert!(!is_scalar(&PlatformValue::Table(BTreeMap::new())));

        assert!(!is_aggregate(&PlatformValue::Integer(1)));
        assert!(is_aggregate(&PlatformValue::List(vec![])));
        assert!(is_aggregate(&PlatformValue::Table(BTreeMap::new())));
    }
}
