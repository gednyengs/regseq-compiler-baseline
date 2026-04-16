//! Framework type kind and type-definition classification.
//!
//! Every `TypeDef` in the Regseq Model carries a [`TypeDefClassification`]
//! that distinguishes user-authored types from Regseq abstraction types
//! (discovered via `#[rseq::abstraction("...")]`) and framework types
//! (discovered via `#[rseq::framework_type("...")]`).
//!
//! The 10 [`FrameworkTypeKind`] variants enumerate the framework-level
//! types defined in spec.md Section 3.4 and exported by the `regseq`
//! facade crate.

use serde::{Deserialize, Serialize};

use crate::ir::RegseqAbstraction;

/// Identifies a specific Regseq framework type exported by the `regseq`
/// facade crate.
///
/// The attribute string used by the facade (`#[rseq::framework_type("...")]`)
/// is listed in the doc comment for each variant. The compiler maps
/// attribute strings to these variants during Phase 4 extraction.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FrameworkTypeKind {
    /// `regseq::RegseqError` -- attribute string `"regseq-error"`.
    RegseqError,
    /// `regseq::SerialErrors` -- attribute string `"serial-errors"`.
    SerialErrors,
    /// `regseq::RxChar` -- attribute string `"rx-char"`.
    RxChar,
    /// `regseq::SerialConfig` -- attribute string `"serial-config"`.
    SerialConfig,
    /// `regseq::DataBits` -- attribute string `"data-bits"`.
    DataBits,
    /// `regseq::Parity` -- attribute string `"parity"`.
    Parity,
    /// `regseq::StopBits` -- attribute string `"stop-bits"`.
    StopBits,
    /// `regseq::FlowControl` -- attribute string `"flow-control"`.
    FlowControl,
    /// `regseq::Duration` -- attribute string `"duration"`.
    Duration,
    /// `regseq::IrqResult` -- attribute string `"irq-result"`.
    IrqResult,
}

/// Classification of a type definition.
///
/// Attached to every `TypeDef` to tell downstream passes how to lower
/// the type: user-defined types are authored by the spec and lowered
/// structurally; abstraction types are recognized by name and routed
/// through backend-specific code generation; framework types are
/// enumerated sentinels that analyses special-case directly.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum TypeDefClassification {
    /// Spec author's types with no special Regseq semantics.
    UserDefined,
    /// Regseq abstraction type discovered via `#[rseq::abstraction("...")]`.
    Abstraction(RegseqAbstraction),
    /// Regseq framework type discovered via `#[rseq::framework_type("...")]`.
    Framework(FrameworkTypeKind),
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- FrameworkTypeKind construction ----------------------------------

    #[test]
    fn constructs_all_ten_framework_type_variants() {
        let variants: [FrameworkTypeKind; 10] = [
            FrameworkTypeKind::RegseqError,
            FrameworkTypeKind::SerialErrors,
            FrameworkTypeKind::RxChar,
            FrameworkTypeKind::SerialConfig,
            FrameworkTypeKind::DataBits,
            FrameworkTypeKind::Parity,
            FrameworkTypeKind::StopBits,
            FrameworkTypeKind::FlowControl,
            FrameworkTypeKind::Duration,
            FrameworkTypeKind::IrqResult,
        ];
        assert_eq!(variants.len(), 10);
    }

    #[test]
    fn framework_type_kind_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<FrameworkTypeKind>();

        let a = FrameworkTypeKind::RegseqError;
        let b = a;
        assert_eq!(a, b);
    }

    // -- TypeDefClassification construction ------------------------------

    #[test]
    fn constructs_all_three_classification_variants() {
        let _user = TypeDefClassification::UserDefined;
        let _abstraction = TypeDefClassification::Abstraction(RegseqAbstraction::Clock);
        let _framework = TypeDefClassification::Framework(FrameworkTypeKind::SerialConfig);
    }

    // -- Serde round trip for FrameworkTypeKind --------------------------

    #[test]
    fn framework_type_kind_round_trips_through_json_for_every_variant() {
        for variant in [
            FrameworkTypeKind::RegseqError,
            FrameworkTypeKind::SerialErrors,
            FrameworkTypeKind::RxChar,
            FrameworkTypeKind::SerialConfig,
            FrameworkTypeKind::DataBits,
            FrameworkTypeKind::Parity,
            FrameworkTypeKind::StopBits,
            FrameworkTypeKind::FlowControl,
            FrameworkTypeKind::Duration,
            FrameworkTypeKind::IrqResult,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: FrameworkTypeKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    #[test]
    fn framework_type_kind_uses_adjacently_tagged_format() {
        let json = serde_json::to_string(&FrameworkTypeKind::RxChar).unwrap();
        assert!(
            json.contains("\"kind\""),
            "adjacently tagged variants must emit a 'kind' field: {json}",
        );
    }

    // -- Serde round trip for TypeDefClassification ----------------------

    #[test]
    fn type_def_classification_user_defined_round_trip() {
        let v = TypeDefClassification::UserDefined;
        let json = serde_json::to_string(&v).unwrap();
        let back: TypeDefClassification = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn type_def_classification_abstraction_round_trip_for_every_abstraction() {
        for a in [
            RegseqAbstraction::Clock,
            RegseqAbstraction::Reset,
            RegseqAbstraction::PinCtrl,
            RegseqAbstraction::Interrupt,
            RegseqAbstraction::Mmio,
            RegseqAbstraction::Mutex,
            RegseqAbstraction::SpinLock,
            RegseqAbstraction::IrqSpinLock,
            RegseqAbstraction::Completion,
        ] {
            let v = TypeDefClassification::Abstraction(a);
            let json = serde_json::to_string(&v).unwrap();
            let back: TypeDefClassification = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "round-trip mismatch for abstraction {a:?}");
        }
    }

    #[test]
    fn type_def_classification_framework_round_trip_for_every_framework_type() {
        for kind in [
            FrameworkTypeKind::RegseqError,
            FrameworkTypeKind::SerialErrors,
            FrameworkTypeKind::RxChar,
            FrameworkTypeKind::SerialConfig,
            FrameworkTypeKind::DataBits,
            FrameworkTypeKind::Parity,
            FrameworkTypeKind::StopBits,
            FrameworkTypeKind::FlowControl,
            FrameworkTypeKind::Duration,
            FrameworkTypeKind::IrqResult,
        ] {
            let v = TypeDefClassification::Framework(kind);
            let json = serde_json::to_string(&v).unwrap();
            let back: TypeDefClassification = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back, "round-trip mismatch for framework {kind:?}");
        }
    }

    #[test]
    fn type_def_classification_uses_adjacently_tagged_format() {
        let v = TypeDefClassification::UserDefined;
        let json = serde_json::to_string(&v).unwrap();
        assert!(
            json.contains("\"kind\""),
            "adjacently tagged variants must emit a 'kind' field: {json}",
        );
    }

    #[test]
    fn type_def_classification_variants_are_distinguishable_in_json() {
        let user_json = serde_json::to_string(&TypeDefClassification::UserDefined).unwrap();
        let abstraction_json = serde_json::to_string(&TypeDefClassification::Abstraction(
            RegseqAbstraction::Clock,
        ))
        .unwrap();
        let framework_json = serde_json::to_string(&TypeDefClassification::Framework(
            FrameworkTypeKind::RegseqError,
        ))
        .unwrap();

        assert_ne!(user_json, abstraction_json);
        assert_ne!(user_json, framework_json);
        assert_ne!(abstraction_json, framework_json);
    }
}
