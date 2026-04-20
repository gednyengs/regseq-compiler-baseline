//! Phase-7 compat scaffold tests for `regseq_model::compat`.
//!
//! These tests live outside the crate under `tests/` so they also
//! prove the `compat` module's public API is reachable from
//! downstream crates that will migrate via import-path-only diffs
//! (tasks `regseq-analysis-06`, `regseq-lowering-10`,
//! `regseq-linux-backend-03`, `regseq-driver-35`).

use std::collections::{BTreeMap, BTreeSet};

use regseq_model::compat;
use regseq_model::identity::{DeclId, ModuleId, SymbolInterner};
use regseq_model::ir::{RegseqAbstraction, RegseqIntrinsic};
use regseq_model::model::module::{Module as RegseqModule, ModuleKind};
use regseq_model::model::RegseqModel;

// ---------------------------------------------------------------------------
// compat_reexports_compile
// ---------------------------------------------------------------------------
//
// Name every symbol that the Phase-7 consumer migration tasks will
// import from `regseq_model::compat`. If any symbol vanishes from
// Section A of `compat.rs`, this test fails at compile time, which
// is exactly the grep gate the consumer migration tasks rely on.

#[test]
fn compat_reexports_compile() {
    // Declaration re-exports.
    use compat::{
        BodyDeclSet, BusType, CallbackDeclSet, CallbackFamily, CallbackImplDecl, ClockFieldDecl,
        CompletionOperationDecl, DeclarationModel, DeviceClass, DriverDecl, DurationCtorSiteDecl,
        FamilyCallSite, FnBodyDecl, GuardScopeDecl, LockOperationDecl, LogCallSiteDecl,
        LoggingDeclSet, MmioFieldDecl, PinCtrlFieldDecl, PmGateDecl, PmGateDeclSet,
        PollCallSiteDecl, PropertyDeclSet, PropertyLintDecl, RegisterBlockDecl, RegisterDeclSet,
        RegisterDescriptorDecl, ResetFieldDecl, ResourceDeclSet, SyncFieldDecl,
        SynchronizationDeclSet, TimedDelayCallSiteDecl, TimedDelayDeclSet, WrapperCallSiteDecl,
    };
    // MIR body re-exports.
    use compat::{
        FnKind, MirBody, MirBodyDeclSet, MirFnBodyDecl, MirOperand, MirPlace, MirProjectionElem,
        MirQueryFlavor, MirRvalue, MirStatementKind, MirTerminatorKind,
    };
    // Binding re-exports.
    use compat::{
        BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
        JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
    };
    // Joined re-exports.
    use compat::{
        perform_join, ClockJoinEntry, InterruptDeclOnly, InterruptJoinEntry, JoinError,
        JoinOutcome, JoinedArtifact, JoinedHeader, MmioJoinEntry, PinCtrlJoinEntry,
        PropertyDeclOnly, PropertyJoinEntry, ResetJoinEntry,
    };
    // Semantic re-exports.
    use compat::{
        BinOp, Block, ConstantRef, DriverFieldRef, Expr, FieldWriteValue, FunctionRef,
        InterruptClassRef, LocalId, LocalOrigin, RegisterFieldRef, RegisterRef, ResourceOperation,
        ResourceRef, SemanticBody, SemanticLocal, SemanticModel, SemanticType, Stmt,
        SubsystemActionRef, UnaryOp,
    };
    // Item re-exports.
    use compat::{
        CanonicalPath, ConstantDef, DeviceCallbackRole, DeviceClassKind, EnumDef, EnumVariantDef,
        FieldAttribute, FunctionDef, FunctionKind, FunctionRole, ImplDef, InterruptClassDef, Item,
        ItemAttribute, ItemId, ItemStore, LifecycleRole, Module, PrimitiveType, StructDef,
        StructFieldDef, TypeRef,
    };
    // Free-helper re-export.
    use compat::collect_call_targets;

    // Enumerate each symbol at least once in a `PhantomData<fn() -> T>`
    // position so the compiler refuses to skip over any name. No
    // values are constructed.
    fn _phantom<T: ?Sized>() -> core::marker::PhantomData<fn() -> *const T> {
        core::marker::PhantomData
    }

    let _a = (
        _phantom::<BodyDeclSet>(),
        _phantom::<BusType>(),
        _phantom::<CallbackDeclSet>(),
        _phantom::<CallbackFamily>(),
        _phantom::<CallbackImplDecl>(),
        _phantom::<ClockFieldDecl>(),
        _phantom::<CompletionOperationDecl>(),
        _phantom::<DeclarationModel>(),
        _phantom::<DeviceClass>(),
        _phantom::<DriverDecl>(),
        _phantom::<DurationCtorSiteDecl>(),
        _phantom::<FamilyCallSite<'_>>(),
        _phantom::<FnBodyDecl>(),
        _phantom::<GuardScopeDecl>(),
        _phantom::<LockOperationDecl>(),
        _phantom::<LogCallSiteDecl>(),
        _phantom::<LoggingDeclSet>(),
        _phantom::<MmioFieldDecl>(),
        _phantom::<PinCtrlFieldDecl>(),
    );
    let _b = (
        _phantom::<PmGateDecl>(),
        _phantom::<PmGateDeclSet>(),
        _phantom::<PollCallSiteDecl>(),
        _phantom::<PropertyDeclSet>(),
        _phantom::<PropertyLintDecl>(),
        _phantom::<RegisterBlockDecl>(),
        _phantom::<RegisterDeclSet>(),
        _phantom::<RegisterDescriptorDecl>(),
        _phantom::<ResetFieldDecl>(),
        _phantom::<ResourceDeclSet>(),
        _phantom::<SyncFieldDecl>(),
        _phantom::<SynchronizationDeclSet>(),
        _phantom::<TimedDelayCallSiteDecl>(),
        _phantom::<TimedDelayDeclSet>(),
        _phantom::<WrapperCallSiteDecl>(),
    );
    let _c = (
        _phantom::<FnKind>(),
        _phantom::<MirBody>(),
        _phantom::<MirBodyDeclSet>(),
        _phantom::<MirFnBodyDecl>(),
        _phantom::<MirOperand>(),
        _phantom::<MirPlace>(),
        _phantom::<MirProjectionElem>(),
        _phantom::<MirQueryFlavor>(),
        _phantom::<MirRvalue>(),
        _phantom::<MirStatementKind>(),
        _phantom::<MirTerminatorKind>(),
    );
    let _d = (
        _phantom::<BindingHeader>(),
        _phantom::<BindingModel>(),
        _phantom::<BindingProvenance>(),
        _phantom::<BusBinding>(),
        _phantom::<ClockBinding>(),
        _phantom::<InterruptBinding>(),
        _phantom::<JoinKey>(),
        _phantom::<MmioBinding>(),
        _phantom::<PinCtrlBinding>(),
        _phantom::<PropertyBinding>(),
        _phantom::<ResetBinding>(),
    );
    let _e = (
        _phantom::<ClockJoinEntry>(),
        _phantom::<InterruptDeclOnly>(),
        _phantom::<InterruptJoinEntry>(),
        _phantom::<JoinError>(),
        _phantom::<JoinOutcome>(),
        _phantom::<JoinedArtifact>(),
        _phantom::<JoinedHeader>(),
        _phantom::<MmioJoinEntry>(),
        _phantom::<PinCtrlJoinEntry>(),
        _phantom::<PropertyDeclOnly>(),
        _phantom::<PropertyJoinEntry>(),
        _phantom::<ResetJoinEntry>(),
    );
    let _f = (
        _phantom::<BinOp>(),
        _phantom::<Block>(),
        _phantom::<ConstantRef>(),
        _phantom::<DriverFieldRef>(),
        _phantom::<Expr>(),
        _phantom::<FieldWriteValue>(),
        _phantom::<FunctionRef>(),
        _phantom::<InterruptClassRef>(),
        _phantom::<LocalId>(),
        _phantom::<LocalOrigin>(),
        _phantom::<RegisterFieldRef>(),
        _phantom::<RegisterRef>(),
        _phantom::<ResourceOperation>(),
        _phantom::<ResourceRef>(),
        _phantom::<SemanticBody>(),
        _phantom::<SemanticLocal>(),
        _phantom::<SemanticModel>(),
        _phantom::<SemanticType>(),
        _phantom::<Stmt>(),
        _phantom::<SubsystemActionRef>(),
        _phantom::<UnaryOp>(),
    );
    let _g = (
        _phantom::<CanonicalPath>(),
        _phantom::<ConstantDef>(),
        _phantom::<DeviceCallbackRole>(),
        _phantom::<DeviceClassKind>(),
        _phantom::<EnumDef>(),
        _phantom::<EnumVariantDef>(),
        _phantom::<FieldAttribute>(),
        _phantom::<FunctionDef>(),
        _phantom::<FunctionKind>(),
        _phantom::<FunctionRole>(),
        _phantom::<ImplDef>(),
        _phantom::<InterruptClassDef>(),
        _phantom::<Item>(),
        _phantom::<ItemAttribute>(),
        _phantom::<ItemId>(),
        _phantom::<ItemStore>(),
        _phantom::<LifecycleRole>(),
        _phantom::<Module>(),
        _phantom::<PrimitiveType>(),
        _phantom::<StructDef>(),
        _phantom::<StructFieldDef>(),
        _phantom::<TypeRef>(),
    );

    // Free helper re-exports: reference each one by coercing to a
    // function pointer so the symbol resolution is forced at compile
    // time without invoking the function.
    let _collect: fn(&regseq_model::declaration::bodies::BodyExpr) -> Vec<String> =
        collect_call_targets;
    let _join: fn(
        &compat::DeclarationModel,
        &compat::BindingModel,
    ) -> Result<compat::JoinedArtifact, compat::JoinError> = perform_join;
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Minimal `DeclarationModel` built directly from public fields. All
/// family sets are empty; enough to prove accessor delegation works.
fn minimal_declaration_model() -> compat::DeclarationModel {
    use regseq_model::declaration::mir_body::{MirBodyDeclSet, MirQueryFlavor};
    use regseq_model::declaration::provenance::Provenance;
    use regseq_model::declaration::resources::{
        ClockDeclSet, ClockFieldDecl, InterruptDeclSet, PinCtrlDeclSet, PinCtrlFieldDecl,
        ResetDeclSet, ResetFieldDecl, ResourceDeclSet,
    };
    use regseq_model::declaration::{
        BodyDeclSet, BusType, CallbackDeclSet, DeclarationBuildContext, DeclarationModel,
        DeviceClass, DriverDecl, LoggingDeclSet, PmGateDeclSet, PropertyDeclSet, RegisterDeclSet,
        SynchronizationDeclSet, TimedDelayDeclSet,
    };

    DeclarationModel {
        driver: DriverDecl {
            driver_name: "CompatDriver".into(),
            compatible: vec!["test,compat".into()],
            device_class: DeviceClass::SerialUart,
            bus: Some(BusType::Amba),
            provenance: Provenance::new("identity"),
        },
        resources: ResourceDeclSet {
            mmio: vec![],
            interrupts: InterruptDeclSet {
                resources: vec![],
                isr_bindings: vec![],
            },
            clock: ClockDeclSet {
                fields: vec![ClockFieldDecl {
                    field_name: "clk".into(),
                    clock_id: "bus_clk".into(),
                    required: true,
                    provenance: Provenance::new("clock"),
                }],
                call_sites: vec![],
            },
            reset: ResetDeclSet {
                fields: vec![ResetFieldDecl {
                    field_name: "reset".into(),
                    required: false,
                    provenance: Provenance::new("reset"),
                }],
                call_sites: vec![],
            },
            pinctrl: PinCtrlDeclSet {
                fields: vec![PinCtrlFieldDecl {
                    field_name: "pins".into(),
                    required: false,
                    pinctrl_id: Some("bus_pins".into()),
                    provenance: Provenance::new("pinctrl"),
                }],
                call_sites: vec![],
            },
        },
        synchronization: SynchronizationDeclSet {
            sync_fields: vec![],
            lock_operations: vec![],
            completion_operations: vec![],
            guard_scopes: vec![],
        },
        callbacks: CallbackDeclSet {
            lifecycle_impls: vec![],
            serial_impls: vec![],
            serial_console_impls: vec![],
            power_management_impls: vec![],
        },
        registers: RegisterDeclSet {
            blocks: vec![],
            descriptors: vec![],
            wrapper_call_sites: vec![],
            poll_call_sites: vec![],
        },
        properties: PropertyDeclSet {
            fields: vec![],
            lints: vec![],
        },
        timed_delay: TimedDelayDeclSet {
            call_sites: vec![],
            duration_ctors: vec![],
        },
        logging: LoggingDeclSet { call_sites: vec![] },
        pm_gate: PmGateDeclSet { gates: vec![] },
        bodies: BodyDeclSet { functions: vec![] },
        mir_bodies: MirBodyDeclSet {
            functions: vec![],
            query_flavor: MirQueryFlavor::DropsElaborated,
        },
        build_context: DeclarationBuildContext {
            target_os: "linux".into(),
        },
    }
}

/// Minimal `BindingModel` with empty family sections.
fn minimal_binding_model() -> compat::BindingModel {
    use regseq_model::binding::{BindingHeader, BindingModel, BindingProvenance};

    BindingModel {
        header: BindingHeader {
            version: "1".into(),
            os: "linux".into(),
            arch: "aarch64".into(),
            board: "board".into(),
            provenance: BindingProvenance::new("platform"),
        },
        properties: vec![],
        mmio: vec![],
        interrupts: vec![],
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        bus: None,
    }
}

/// Minimal `JoinedArtifact` with empty family sections.
fn minimal_joined_artifact() -> compat::JoinedArtifact {
    use regseq_model::binding::{BindingHeader, BindingProvenance};
    use regseq_model::joined::JoinedHeader;

    let header = JoinedHeader::from(BindingHeader {
        version: "1".into(),
        os: "linux".into(),
        arch: "aarch64".into(),
        board: "board".into(),
        provenance: BindingProvenance::new("platform"),
    });

    compat::JoinedArtifact {
        header,
        properties_joined: vec![],
        properties_decl_only: vec![],
        properties_binding_only: vec![],
        mmio_joined: vec![],
        mmio_decl_only: vec![],
        mmio_binding_only: vec![],
        interrupts_joined: vec![],
        interrupts_decl_only: vec![],
        interrupts_binding_only: vec![],
        clocks_joined: vec![],
        clocks_decl_only: vec![],
        clocks_binding_only: vec![],
        resets_joined: vec![],
        resets_decl_only: vec![],
        resets_binding_only: vec![],
        pinctrl_joined: vec![],
        pinctrl_decl_only: vec![],
        pinctrl_binding_only: vec![],
    }
}

/// Minimal `SemanticModel` with empty collections.
fn minimal_semantic_model() -> compat::SemanticModel {
    compat::SemanticModel {
        functions: vec![],
        constants: vec![],
        interrupt_classes: vec![],
    }
}

/// Minimal `RegseqModel` with an empty root module. Used solely to
/// trigger the `unimplemented!` panic in every view's
/// `new_from_regseq_model`.
fn minimal_regseq_model() -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let name = symbols.intern("crate_root");

    let root_module = RegseqModule {
        id: ModuleId::from(DeclId::new(1)),
        name,
        kind: ModuleKind::Normal,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: None,
        trait_impls: vec![],
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::<RegseqAbstraction>::new(),
        intrinsics_used: BTreeSet::<RegseqIntrinsic>::new(),
    };

    RegseqModel {
        symbols,
        source_locations: BTreeMap::new(),
        root_module,
        format_version: 1,
    }
}

// ---------------------------------------------------------------------------
// *_legacy_round_trip tests
// ---------------------------------------------------------------------------

#[test]
fn declaration_view_legacy_round_trip() {
    let decl = minimal_declaration_model();
    let view = compat::DeclarationView::new_legacy(&decl);

    // Delegate each accessor and compare against the underlying
    // `DeclarationModel` surface directly.
    assert_eq!(
        view.all_resource_field_names(),
        decl.all_resource_field_names()
    );
    assert_eq!(view.all_call_sites(), decl.all_call_sites());

    assert_eq!(
        view.all_callback_impls().len(),
        decl.all_callback_impls().len()
    );
    assert_eq!(view.wrapper_call_sites(), decl.wrapper_call_sites());
    assert_eq!(view.poll_call_sites(), decl.poll_call_sites());
    assert_eq!(view.clock_fields(), decl.clock_fields());
    assert_eq!(view.reset_fields(), decl.reset_fields());
    assert_eq!(view.pinctrl_fields(), decl.pinctrl_fields());
    assert_eq!(view.sync_fields(), decl.sync_fields());
    assert_eq!(view.lock_operations(), decl.lock_operations());
    assert_eq!(view.completion_operations(), decl.completion_operations());
    assert_eq!(view.guard_scopes(), decl.guard_scopes());
    assert_eq!(view.timed_delay_call_sites(), decl.timed_delay_call_sites());
    assert_eq!(view.duration_ctor_sites(), decl.duration_ctor_sites());
    assert_eq!(view.log_call_sites(), decl.log_call_sites());
    assert_eq!(view.pm_gates(), decl.pm_gates());

    // Spot-check a specific field that should appear on the view:
    // the fixture has one clock field so the delegating accessor
    // should return a single entry.
    assert_eq!(view.clock_fields().len(), 1);
    assert_eq!(view.clock_fields()[0].clock_id, "bus_clk");
}

#[test]
fn binding_view_legacy_round_trip() {
    let binding = minimal_binding_model();
    let view = compat::BindingView::new_legacy(&binding);

    assert_eq!(view.header().version, binding.header.version);
    assert!(view.bus().is_none());
    assert!(view.properties().is_empty());
    assert!(view.mmio().is_empty());
    assert!(view.interrupts().is_empty());
    assert!(view.clocks().is_empty());
    assert!(view.resets().is_empty());
    assert!(view.pinctrl().is_empty());
}

#[test]
fn joined_view_legacy_round_trip() {
    let artifact = minimal_joined_artifact();
    let view = compat::JoinedView::new_legacy(&artifact);

    // Header delegation.
    assert_eq!(view.header().board, artifact.header.board);

    // Joined-entry slices: parity check against the underlying
    // `JoinedArtifact` fields (compared as slice lengths since most
    // entry types do not implement `PartialEq` on every field
    // combination the test cares about; the minimal fixture has zero
    // entries across every family so an equal-length check is
    // sufficient for the Phase-7 handoff contract).
    assert_eq!(
        view.properties_joined().len(),
        artifact.properties_joined.len()
    );
    assert_eq!(view.mmio_joined().len(), artifact.mmio_joined.len());
    assert_eq!(
        view.interrupts_joined().len(),
        artifact.interrupts_joined.len()
    );
    assert_eq!(view.clocks_joined().len(), artifact.clocks_joined.len());
    assert_eq!(view.resets_joined().len(), artifact.resets_joined.len());
    assert_eq!(view.pinctrl_joined().len(), artifact.pinctrl_joined.len());

    // Declaration-only / binding-only slice accessors: the full
    // `{family}_{decl_only,binding_only}` accessor family required by
    // Section B of the spec.
    assert_eq!(
        view.properties_decl_only().len(),
        artifact.properties_decl_only.len()
    );
    assert_eq!(
        view.properties_binding_only().len(),
        artifact.properties_binding_only.len()
    );
    assert_eq!(view.mmio_decl_only().len(), artifact.mmio_decl_only.len());
    assert_eq!(
        view.mmio_binding_only().len(),
        artifact.mmio_binding_only.len()
    );
    assert_eq!(
        view.interrupts_decl_only().len(),
        artifact.interrupts_decl_only.len()
    );
    assert_eq!(
        view.interrupts_binding_only().len(),
        artifact.interrupts_binding_only.len()
    );
    assert_eq!(
        view.clocks_decl_only().len(),
        artifact.clocks_decl_only.len()
    );
    assert_eq!(
        view.clocks_binding_only().len(),
        artifact.clocks_binding_only.len()
    );
    assert_eq!(
        view.resets_decl_only().len(),
        artifact.resets_decl_only.len()
    );
    assert_eq!(
        view.resets_binding_only().len(),
        artifact.resets_binding_only.len()
    );
    assert_eq!(
        view.pinctrl_decl_only().len(),
        artifact.pinctrl_decl_only.len()
    );
    assert_eq!(
        view.pinctrl_binding_only().len(),
        artifact.pinctrl_binding_only.len()
    );
}

#[test]
fn semantic_view_legacy_round_trip() {
    let sm = minimal_semantic_model();
    let view = compat::SemanticView::new_legacy(&sm);

    assert_eq!(view.functions().len(), 0);
    assert_eq!(view.constants().len(), 0);
    assert_eq!(view.interrupt_classes().len(), 0);
}

#[test]
fn item_view_legacy_round_trip() {
    let store = compat::ItemStore::new();
    let view = compat::ItemView::new_legacy(&store);

    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
    assert!(view.get(compat::ItemId(0)).is_none());
}

// ---------------------------------------------------------------------------
// new_from_regseq_model_stub_panics tests
// ---------------------------------------------------------------------------
//
// Per the spec's Section C handoff contract, every
// `new_from_regseq_model` must panic with the documented Phase-7
// handoff message. Task `regseq-driver-35` deletes these tests as
// part of its runtime cutover.

#[test]
#[should_panic(expected = "regseq-driver-35")]
fn declaration_view_new_from_regseq_model_stub_panics() {
    let model = minimal_regseq_model();
    let _ = compat::DeclarationView::new_from_regseq_model(&model);
}

#[test]
#[should_panic(expected = "regseq-driver-35")]
fn binding_view_new_from_regseq_model_stub_panics() {
    let model = minimal_regseq_model();
    let _ = compat::BindingView::new_from_regseq_model(&model);
}

#[test]
#[should_panic(expected = "regseq-driver-35")]
fn joined_view_new_from_regseq_model_stub_panics() {
    let model = minimal_regseq_model();
    let _ = compat::JoinedView::new_from_regseq_model(&model);
}

#[test]
#[should_panic(expected = "regseq-driver-35")]
fn semantic_view_new_from_regseq_model_stub_panics() {
    let model = minimal_regseq_model();
    let _ = compat::SemanticView::new_from_regseq_model(&model);
}

#[test]
#[should_panic(expected = "regseq-driver-35")]
fn item_view_new_from_regseq_model_stub_panics() {
    let model = minimal_regseq_model();
    let _ = compat::ItemView::new_from_regseq_model(&model);
}
