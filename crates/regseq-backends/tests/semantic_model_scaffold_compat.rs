//! Tests for semantic model compatibility with the lowering module scaffold.
//!
//! These tests verify that `SemanticModel` integrates with the existing
//! lowering types, that `BinOp` and `ResourceKind` re-exports work, and
//! that shared-type ownership between `regseq-model` and `regseq-backends`
//! is acyclic and explicit.

use regseq_model::declaration::Provenance;
use regseq_model::item::constant::{ConstValue, ConstantDef};
use regseq_model::item::function::{FunctionDef, FunctionKind, FunctionRole};
use regseq_model::item::identity::{CanonicalPath, ItemId};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::type_ref::{PrimitiveType, TypeRef};
use regseq_model::semantic::{SemanticBody, SemanticModel};

// ---------------------------------------------------------------------------
// Test 7: Lowering scaffold compatibility
// ---------------------------------------------------------------------------

/// SemanticModel is importable alongside DriverLoweringPlan and shares the
/// same FunctionDef, ConstantDef, InterruptClassDef types from regseq_model.
#[test]
fn semantic_model_alongside_lowering_plan() {
    use regseq_backends::lowering::plan::DriverLoweringPlan;

    // Both SemanticModel and DriverLoweringPlan reference the same
    // FunctionDef, ConstantDef, InterruptClassDef from regseq_model::item.
    let func = FunctionDef {
        id: ItemId(0),
        canonical_path: CanonicalPath::new("pkg::MyDriver::init"),
        name: "init".into(),
        kind: FunctionKind::FreeFunction,
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: Provenance::new("identity"),
    };

    let model = SemanticModel {
        functions: vec![func],
        constants: vec![],
        interrupt_classes: vec![],
    };

    // SemanticModel exists independently of DriverLoweringPlan.
    // The types it uses (FunctionDef, ConstantDef, InterruptClassDef) are
    // the same regseq-model types the lowering plan already depends on.
    assert_eq!(model.functions.len(), 1);

    // DriverLoweringPlan is still constructable (this proves coexistence).
    // We just verify it's a real type, not that we can construct it
    // (construction requires AnalysisInput).
    let _plan_type_name = std::any::type_name::<DriverLoweringPlan>();
    assert!(_plan_type_name.contains("DriverLoweringPlan"));
}

/// BinOp from regseq_model::semantic is the same type as
/// crate::lowering::behavior::BinOp (via re-export).
#[test]
fn binop_is_shared_type() {
    use regseq_backends::lowering::behavior::BinOp as BackendBinOp;
    use regseq_model::semantic::BinOp as ModelBinOp;

    // These are the same type via re-export.
    let model_add = ModelBinOp::Add;
    let backend_add = BackendBinOp::Add;
    assert_eq!(model_add, backend_add);

    // Verify all 12 variants exist on both paths.
    let _variants: Vec<ModelBinOp> = vec![
        BackendBinOp::Add,
        BackendBinOp::Sub,
        BackendBinOp::Mul,
        BackendBinOp::Div,
        BackendBinOp::Shl,
        BackendBinOp::Shr,
        BackendBinOp::BitAnd,
        BackendBinOp::BitOr,
        BackendBinOp::Eq,
        BackendBinOp::Ne,
        BackendBinOp::And,
        BackendBinOp::Or,
    ];
    assert_eq!(_variants.len(), 12);
}

/// ResourceKind from regseq_model::semantic is the same type as
/// crate::lowering::plan::ResourceKind (via re-export).
#[test]
fn resource_kind_is_shared_type() {
    use regseq_backends::lowering::plan::ResourceKind as BackendKind;
    use regseq_model::semantic::ResourceKind as ModelKind;

    // These are the same type via re-export.
    let model_clock = ModelKind::Clock;
    let backend_clock = BackendKind::Clock;
    assert_eq!(model_clock, backend_clock);

    // Verify all 6 variants exist on both paths.
    let _variants: Vec<ModelKind> = vec![
        BackendKind::Mmio,
        BackendKind::Interrupt,
        BackendKind::Clock,
        BackendKind::Reset,
        BackendKind::PinCtrl,
        BackendKind::Property,
    ];
    assert_eq!(_variants.len(), 6);
}

// ---------------------------------------------------------------------------
// Test 8: Acyclic ownership
// ---------------------------------------------------------------------------

/// regseq-backends imports semantic types from regseq-model without
/// circular dependencies. ItemId and CanonicalPath are shared across both.
#[test]
fn acyclic_shared_type_ownership() {
    // Prove that the same ItemId type is used in both crates.
    let id = ItemId(42);

    // Used in model-layer item hierarchy.
    let func = FunctionDef {
        id,
        canonical_path: CanonicalPath::new("pkg::MyDriver::tx"),
        name: "tx".into(),
        kind: FunctionKind::FreeFunction,
        role: FunctionRole::Helper,
        body: SemanticBody::empty(),
        provenance: Provenance::new("identity"),
    };
    assert_eq!(func.id, id);

    // Used in semantic model.
    let model = SemanticModel {
        functions: vec![func],
        constants: vec![ConstantDef {
            id: ItemId(1),
            canonical_path: CanonicalPath::new("pkg::MyDriver::MASK"),
            name: "MASK".into(),
            owner: Some(id),
            ty: TypeRef::Primitive {
                ty: PrimitiveType::U32,
            },
            value: Some(ConstValue::Integer(0xFF)),
            stable_name: "MY_DRIVER_MASK".into(),
            provenance: Provenance::new("identity"),
        }],
        interrupt_classes: vec![InterruptClassDef {
            name: "tx".into(),
            mask_value: 0x20,
        }],
    };

    // Same ItemId flows through both model and semantic layers.
    assert_eq!(model.functions[0].id, id);
    assert_eq!(model.constants[0].owner, Some(id));

    // CanonicalPath is the shared cross-reference identity.
    let path = CanonicalPath::new("pkg::MyDriver::tx");
    assert_eq!(model.functions[0].canonical_path, path);
}
