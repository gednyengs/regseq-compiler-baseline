//! Compat adapter for Phase-7 consumer migrations.
//!
//! This module is the single bridge that Phase-7 consumer migration
//! tasks (`regseq-analysis-06`, `regseq-lowering-10`,
//! `regseq-linux-backend-03`, `regseq-driver-35`) import from. It has
//! two responsibilities:
//!
//! 1. **Section A** re-exports every type consumers currently import
//!    from `regseq_model::{declaration, binding, joined, semantic,
//!    item}` via `pub use`, so consumer migrations become
//!    import-path-only diffs. `compat::DeclarationModel` IS the legacy
//!    [`crate::declaration::DeclarationModel`] via `pub use` -- there
//!    are no type aliases in this module.
//!
//! 2. **Section B** defines read-only view facades
//!    (`DeclarationView`, `BindingView`, `JoinedView`, `SemanticView`,
//!    `ItemView`) that each carry two constructors:
//!    - `new_legacy(&legacy)` -- implemented here; the `Legacy` arm
//!      delegates every accessor to the underlying legacy type.
//!    - `new_from_regseq_model(&RegseqModel)` -- stubbed with
//!      `unimplemented!("regseq-driver-35 ...")`. Task
//!      `regseq-driver-35` fills this in during the runtime cutover.
//!
//! During Phase-7 tasks 2-4 (the consumer migrations) the views are
//! not exercised on any hot path; they exist to compile and to carry
//! the stubbed constructors so the handoff contract is explicit.
//!
//! # Surface completeness rule
//!
//! The re-export surface in Section A is derived from an audit grep
//! across the consumer files named in the Phase-7 brief:
//!
//! ```text
//! rg -n 'regseq_model::(declaration|binding|joined|semantic|item)::[A-Za-z0-9_:]+' \
//!     crates/regseq-analysis/src/ \
//!     crates/regseq-backends/src/lowering/planner.rs \
//!     crates/regseq-backends/src/linux/planner.rs \
//!     crates/regseq-backends/src/linux/plan.rs \
//!     crates/regseq-driver/src/emit_driver.rs \
//!     tests/e2e/tests/
//! ```
//!
//! Every symbol that appears in the audit output is re-exported
//! below, merged with the Phase-7 baseline surface spelled out in the
//! task spec. Task `regseq-analysis-06` re-runs the grep at its own
//! acceptance gate; a symbol missing from `compat::*` will fail that
//! gate.

use crate::model::RegseqModel;

// ============================================================================
// Section A: Legacy type re-exports (by name)
// ============================================================================
//
// Consumer migrations substitute `regseq_model::compat::X` for
// `regseq_model::<old_module>::X` with zero body changes; because
// every `compat::X` re-exports the legacy type via `pub use`, owned
// field types in `AnalysisInput` and elsewhere stay structurally
// identical.

// ----- Structural + value types from `crate::declaration` -----
pub use crate::declaration::{
    BodyDeclSet, BusType, CallbackDeclSet, CallbackFamily, CallbackImplDecl, ClockFieldDecl,
    CompletionOperationDecl, DeclarationModel, DeviceClass, DriverDecl, DurationCtorSiteDecl,
    FamilyCallSite, FnBodyDecl, GuardScopeDecl, LockOperationDecl, LogCallSiteDecl, LoggingDeclSet,
    PinCtrlFieldDecl, PmGateDecl, PmGateDeclSet, PollCallSiteDecl, PropertyDeclSet,
    PropertyLintDecl, RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl, ResetFieldDecl,
    ResourceDeclSet, SyncFieldDecl, SynchronizationDeclSet, TimedDelayCallSiteDecl,
    TimedDelayDeclSet, WrapperCallSiteDecl,
};
// `MmioFieldDecl` lives under `declaration::resources` and is not
// re-exported from `declaration/mod.rs`, but it appears as the
// element type of `JoinedArtifact::mmio_decl_only` and therefore on
// `JoinedView::mmio_decl_only`'s return type. Expose it here so
// consumers can name it without reaching into the private submodule
// path.
pub use crate::declaration::resources::MmioFieldDecl;

// ----- Nested module: `crate::declaration::mir_body` -----
//
// Note: `body_readiness.rs` imports `FnKind` from
// `declaration::mir_body` (the MIR flavor). We deliberately re-export
// that MIR `FnKind` under the unqualified `FnKind` name so consumer
// imports (`use regseq_model::compat::FnKind;`) resolve to the same
// type they do today. The authored-body `FnKind` living at
// `declaration::FnKind` is NOT re-exported here to avoid the
// collision; if a future consumer needs it, it should be re-exported
// under a distinct name (e.g. `AuthoredFnKind`).
pub use crate::declaration::mir_body::{
    FnKind, MirBody, MirBodyDeclSet, MirFnBodyDecl, MirOperand, MirPlace, MirProjectionElem,
    MirQueryFlavor, MirRvalue, MirStatementKind, MirTerminatorKind,
};

// ----- Binding types from `crate::binding` -----
pub use crate::binding::{
    BindingHeader, BindingModel, BindingProvenance, BusBinding, ClockBinding, InterruptBinding,
    JoinKey, MmioBinding, PinCtrlBinding, PropertyBinding, ResetBinding,
};

// ----- Joined artifact types from `crate::joined` -----
pub use crate::joined::{
    ClockJoinEntry, InterruptJoinEntry, JoinOutcome, JoinedArtifact, JoinedHeader, MmioJoinEntry,
    PinCtrlJoinEntry, PropertyJoinEntry, ResetJoinEntry,
};
// The `*_decl_only` lists on `JoinedArtifact` carry local entry
// types that are not re-exported from `crate::joined::mod.rs`.
// Expose them through compat so `JoinedView`'s delegating accessors
// return a public type from every external caller's perspective.
pub use crate::joined::artifact::{InterruptDeclOnly, PropertyDeclOnly};
// `perform_join` is invoked directly by
// `crates/regseq-driver/src/emit_driver.rs:23` and by
// `tests/e2e/tests/pl011_{bus_identity_proof,lowering_input_stability}.rs`.
// Re-export so consumer migrations can rewrite
// `use regseq_model::joined::perform_join` to
// `use regseq_model::compat::perform_join` in one pass.
pub use crate::joined::{perform_join, JoinError};

// ----- Semantic types from `crate::semantic` -----
pub use crate::semantic::{
    BinOp, Block, ConstantRef, DriverFieldRef, Expr, FieldWriteValue, FunctionRef,
    InterruptClassRef, LocalId, LocalOrigin, RegisterFieldRef, RegisterRef, ResourceOperation,
    ResourceRef, SemanticBody, SemanticLocal, SemanticModel, SemanticType, Stmt,
    SubsystemActionRef, UnaryOp,
};

// ----- Item-tree types from `crate::item` -----
pub use crate::item::{
    CanonicalPath, ConstantDef, DeviceCallbackRole, DeviceClassKind, EnumDef, EnumVariantDef,
    FieldAttribute, FunctionDef, FunctionKind, FunctionRole, ImplDef, InterruptClassDef, Item,
    ItemAttribute, ItemId, ItemStore, LifecycleRole, Module, PrimitiveType, StructDef,
    StructFieldDef, TypeRef,
};

// ----- Free helper functions consumers import -----
//
// `collect_call_targets` is imported by
// `crates/regseq-analysis/src/input.rs` via its true path
// `regseq_model::declaration::bodies::collect_call_targets`. We
// re-export that path directly so consumer migrations become a pure
// path rewrite with no churn in `declaration/mod.rs`.
pub use crate::declaration::bodies::collect_call_targets;

// ============================================================================
// Section B: Read-only view facades
// ============================================================================
//
// Each view wraps either a legacy structural type OR a `RegseqModel`.
// Tasks 2-4 only migrate consumer imports, so the Legacy arm is the
// only one exercised during Phase-7. Task `regseq-driver-35` fills in
// the `RegseqModel` arm when `AnalysisInput::from_regseq_model` is
// implemented and the runtime cutover happens.
//
// View types intentionally use no type aliases:
// `compat::DeclarationModel` (re-exported in Section A) is the legacy
// struct, NOT `DeclarationView`. This separation avoids any
// field-type / borrow-lifetime collision that would block
// `AnalysisInput` from keeping its current storage shape.

/// Panic message emitted by every `new_from_regseq_model` stub and
/// every `RegseqBacking` match arm on the view surface. Task
/// `regseq-driver-35` replaces these `unimplemented!()` sites with
/// real implementations; the identical message string makes those
/// sites greppable in a single pass.
const PHASE_7_HANDOFF_MSG: &str = "regseq-driver-35 fills this in during the runtime cutover";

// ----------------------------------------------------------------------------
// DeclarationView
// ----------------------------------------------------------------------------

enum DeclBacking<'a> {
    Legacy(&'a DeclarationModel),
    RegseqModel(&'a RegseqModel),
}

/// Read-only facade over declaration-side facts.
///
/// Backed by either a legacy [`DeclarationModel`] (Phase-7 tasks 2-4)
/// or a [`RegseqModel`] (Phase-7 task `regseq-driver-35`). Every
/// accessor on the legacy arm delegates to the equivalent method on
/// [`DeclarationModel`]; the [`RegseqModel`] arm is stubbed until the
/// runtime cutover.
pub struct DeclarationView<'a> {
    backing: DeclBacking<'a>,
}

impl<'a> DeclarationView<'a> {
    /// Construct a view over a legacy [`DeclarationModel`].
    pub fn new_legacy(m: &'a DeclarationModel) -> Self {
        Self {
            backing: DeclBacking::Legacy(m),
        }
    }

    /// Construct a view over a [`RegseqModel`].
    ///
    /// Stubbed with `unimplemented!` until task `regseq-driver-35`
    /// fills it in during the runtime cutover. The
    /// `DeclBacking::RegseqModel` variant is constructed on the
    /// preceding line so the enum variant is live in the Phase-7
    /// build; task `regseq-driver-35` replaces the construction plus
    /// panic with the real initializer.
    pub fn new_from_regseq_model(m: &'a RegseqModel) -> Self {
        let _phase_7_stub = DeclBacking::RegseqModel(m);
        unimplemented!("{}", PHASE_7_HANDOFF_MSG)
    }

    /// Iterate all resource field names across all sub-families.
    pub fn all_resource_field_names(&self) -> Vec<(&'static str, &str)> {
        match &self.backing {
            DeclBacking::Legacy(m) => m.all_resource_field_names(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Iterate all call-site evidence across all construct families.
    pub fn all_call_sites(&self) -> Vec<FamilyCallSite<'_>> {
        match &self.backing {
            DeclBacking::Legacy(m) => m.all_call_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all callback impl declarations across all families.
    pub fn all_callback_impls(&self) -> Vec<&CallbackImplDecl> {
        match &self.backing {
            DeclBacking::Legacy(m) => m.all_callback_impls(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all register wrapper call sites.
    pub fn wrapper_call_sites(&self) -> &[WrapperCallSiteDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.wrapper_call_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all poll call sites.
    pub fn poll_call_sites(&self) -> &[PollCallSiteDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.poll_call_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all clock field declarations.
    pub fn clock_fields(&self) -> &[ClockFieldDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.clock_fields(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all reset field declarations.
    pub fn reset_fields(&self) -> &[ResetFieldDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.reset_fields(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all pinctrl field declarations.
    pub fn pinctrl_fields(&self) -> &[PinCtrlFieldDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.pinctrl_fields(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all synchronization field declarations.
    pub fn sync_fields(&self) -> &[SyncFieldDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.sync_fields(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all lock operation declarations.
    pub fn lock_operations(&self) -> &[LockOperationDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.lock_operations(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all completion operation declarations.
    pub fn completion_operations(&self) -> &[CompletionOperationDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.completion_operations(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all guard scope declarations.
    pub fn guard_scopes(&self) -> &[GuardScopeDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.guard_scopes(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all timed-delay call sites.
    pub fn timed_delay_call_sites(&self) -> &[TimedDelayCallSiteDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.timed_delay_call_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all duration constructor sites.
    pub fn duration_ctor_sites(&self) -> &[DurationCtorSiteDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.duration_ctor_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all logging call sites.
    pub fn log_call_sites(&self) -> &[LogCallSiteDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.log_call_sites(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all PM gate declarations.
    pub fn pm_gates(&self) -> &[PmGateDecl] {
        match &self.backing {
            DeclBacking::Legacy(m) => m.pm_gates(),
            DeclBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// BindingView
// ----------------------------------------------------------------------------

enum BindingBacking<'a> {
    Legacy(&'a BindingModel),
    RegseqModel(&'a RegseqModel),
}

/// Read-only facade over binding-side facts.
///
/// Consumers in the Phase-7 audit set do not call methods on
/// [`BindingModel`] directly; they consume the owned
/// [`BusBinding`] out of the parent [`AnalysisInput`]. The view is
/// still defined so task `regseq-driver-35` has a consistent bridge
/// shape across all five legacy IR families.
pub struct BindingView<'a> {
    backing: BindingBacking<'a>,
}

impl<'a> BindingView<'a> {
    /// Construct a view over a legacy [`BindingModel`].
    pub fn new_legacy(m: &'a BindingModel) -> Self {
        Self {
            backing: BindingBacking::Legacy(m),
        }
    }

    /// Construct a view over a [`RegseqModel`].
    ///
    /// Stubbed with `unimplemented!` until task `regseq-driver-35`
    /// fills it in during the runtime cutover. See
    /// [`DeclarationView::new_from_regseq_model`] for the
    /// `_phase_7_stub` convention.
    pub fn new_from_regseq_model(m: &'a RegseqModel) -> Self {
        let _phase_7_stub = BindingBacking::RegseqModel(m);
        unimplemented!("{}", PHASE_7_HANDOFF_MSG)
    }

    /// Return the platform header.
    pub fn header(&self) -> &BindingHeader {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.header,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return the bus binding, if any.
    pub fn bus(&self) -> Option<&BusBinding> {
        match &self.backing {
            BindingBacking::Legacy(m) => m.bus.as_ref(),
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all property bindings.
    pub fn properties(&self) -> &[PropertyBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.properties,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all MMIO bindings.
    pub fn mmio(&self) -> &[MmioBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.mmio,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all interrupt bindings.
    pub fn interrupts(&self) -> &[InterruptBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.interrupts,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all clock bindings.
    pub fn clocks(&self) -> &[ClockBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.clocks,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all reset bindings.
    pub fn resets(&self) -> &[ResetBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.resets,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Return all pinctrl bindings.
    pub fn pinctrl(&self) -> &[PinCtrlBinding] {
        match &self.backing {
            BindingBacking::Legacy(m) => &m.pinctrl,
            BindingBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// JoinedView
// ----------------------------------------------------------------------------

enum JoinedBacking<'a> {
    Legacy(&'a JoinedArtifact),
    RegseqModel(&'a RegseqModel),
}

/// Read-only facade over joined-artifact facts.
///
/// Mirrors the `{family}_{joined,decl_only,binding_only}` accessor
/// family that consumers currently read as fields on
/// [`JoinedArtifact`]. The view exposes them as methods so task
/// `regseq-driver-35` can swap the backing storage without touching
/// call sites.
pub struct JoinedView<'a> {
    backing: JoinedBacking<'a>,
}

impl<'a> JoinedView<'a> {
    /// Construct a view over a legacy [`JoinedArtifact`].
    pub fn new_legacy(a: &'a JoinedArtifact) -> Self {
        Self {
            backing: JoinedBacking::Legacy(a),
        }
    }

    /// Construct a view over a [`RegseqModel`].
    ///
    /// Stubbed with `unimplemented!` until task `regseq-driver-35`
    /// fills it in during the runtime cutover. See
    /// [`DeclarationView::new_from_regseq_model`] for the
    /// `_phase_7_stub` convention.
    pub fn new_from_regseq_model(m: &'a RegseqModel) -> Self {
        let _phase_7_stub = JoinedBacking::RegseqModel(m);
        unimplemented!("{}", PHASE_7_HANDOFF_MSG)
    }

    /// Return the joined header.
    pub fn header(&self) -> &JoinedHeader {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.header,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined property entries.
    pub fn properties_joined(&self) -> &[PropertyJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.properties_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined MMIO entries.
    pub fn mmio_joined(&self) -> &[MmioJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.mmio_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined interrupt entries.
    pub fn interrupts_joined(&self) -> &[InterruptJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.interrupts_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined clock entries.
    pub fn clocks_joined(&self) -> &[ClockJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.clocks_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined reset entries.
    pub fn resets_joined(&self) -> &[ResetJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.resets_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Joined pinctrl entries.
    pub fn pinctrl_joined(&self) -> &[PinCtrlJoinEntry] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.pinctrl_joined,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only property entries (no matching binding).
    pub fn properties_decl_only(&self) -> &[PropertyDeclOnly] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.properties_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only property entries (no matching declaration).
    pub fn properties_binding_only(&self) -> &[PropertyBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.properties_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only MMIO entries.
    pub fn mmio_decl_only(&self) -> &[MmioFieldDecl] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.mmio_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only MMIO entries.
    pub fn mmio_binding_only(&self) -> &[MmioBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.mmio_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only interrupt entries.
    pub fn interrupts_decl_only(&self) -> &[InterruptDeclOnly] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.interrupts_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only interrupt entries.
    pub fn interrupts_binding_only(&self) -> &[InterruptBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.interrupts_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only clock entries.
    pub fn clocks_decl_only(&self) -> &[ClockFieldDecl] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.clocks_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only clock entries.
    pub fn clocks_binding_only(&self) -> &[ClockBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.clocks_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only reset entries.
    pub fn resets_decl_only(&self) -> &[ResetFieldDecl] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.resets_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only reset entries.
    pub fn resets_binding_only(&self) -> &[ResetBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.resets_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Declaration-only pinctrl entries.
    pub fn pinctrl_decl_only(&self) -> &[PinCtrlFieldDecl] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.pinctrl_decl_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Binding-only pinctrl entries.
    pub fn pinctrl_binding_only(&self) -> &[PinCtrlBinding] {
        match &self.backing {
            JoinedBacking::Legacy(a) => &a.pinctrl_binding_only,
            JoinedBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// SemanticView
// ----------------------------------------------------------------------------

enum SemanticBacking<'a> {
    Legacy(&'a SemanticModel),
    RegseqModel(&'a RegseqModel),
}

/// Read-only facade over semantic-model facts.
///
/// Mirrors the `functions` / `constants` / `interrupt_classes` field
/// surface that consumers read from [`SemanticModel`].
pub struct SemanticView<'a> {
    backing: SemanticBacking<'a>,
}

impl<'a> SemanticView<'a> {
    /// Construct a view over a legacy [`SemanticModel`].
    pub fn new_legacy(m: &'a SemanticModel) -> Self {
        Self {
            backing: SemanticBacking::Legacy(m),
        }
    }

    /// Construct a view over a [`RegseqModel`].
    ///
    /// Stubbed with `unimplemented!` until task `regseq-driver-35`
    /// fills it in during the runtime cutover. See
    /// [`DeclarationView::new_from_regseq_model`] for the
    /// `_phase_7_stub` convention.
    pub fn new_from_regseq_model(m: &'a RegseqModel) -> Self {
        let _phase_7_stub = SemanticBacking::RegseqModel(m);
        unimplemented!("{}", PHASE_7_HANDOFF_MSG)
    }

    /// Per-function semantic bodies.
    pub fn functions(&self) -> &[FunctionDef] {
        match &self.backing {
            SemanticBacking::Legacy(m) => &m.functions,
            SemanticBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Constants referenced during backend planning.
    pub fn constants(&self) -> &[ConstantDef] {
        match &self.backing {
            SemanticBacking::Legacy(m) => &m.constants,
            SemanticBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Interrupt class definitions.
    pub fn interrupt_classes(&self) -> &[InterruptClassDef] {
        match &self.backing {
            SemanticBacking::Legacy(m) => &m.interrupt_classes,
            SemanticBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// ItemView
// ----------------------------------------------------------------------------

enum ItemBacking<'a> {
    Legacy(&'a ItemStore),
    RegseqModel(&'a RegseqModel),
}

/// Read-only facade over item-tree facts.
///
/// Wraps an [`ItemStore`] or a [`RegseqModel`]. The current audit
/// set calls [`ItemStore::get`] and [`ItemStore::len`]; those are
/// exposed here directly.
pub struct ItemView<'a> {
    backing: ItemBacking<'a>,
}

impl<'a> ItemView<'a> {
    /// Construct a view over a legacy [`ItemStore`].
    pub fn new_legacy(s: &'a ItemStore) -> Self {
        Self {
            backing: ItemBacking::Legacy(s),
        }
    }

    /// Construct a view over a [`RegseqModel`].
    ///
    /// Stubbed with `unimplemented!` until task `regseq-driver-35`
    /// fills it in during the runtime cutover. See
    /// [`DeclarationView::new_from_regseq_model`] for the
    /// `_phase_7_stub` convention.
    pub fn new_from_regseq_model(m: &'a RegseqModel) -> Self {
        let _phase_7_stub = ItemBacking::RegseqModel(m);
        unimplemented!("{}", PHASE_7_HANDOFF_MSG)
    }

    /// Look up an item by [`ItemId`].
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        match &self.backing {
            ItemBacking::Legacy(s) => s.get(id),
            ItemBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Total item count.
    pub fn len(&self) -> usize {
        match &self.backing {
            ItemBacking::Legacy(s) => s.len(),
            ItemBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }

    /// Whether the item store is empty.
    pub fn is_empty(&self) -> bool {
        match &self.backing {
            ItemBacking::Legacy(s) => s.is_empty(),
            ItemBacking::RegseqModel(m) => {
                let _ = m;
                unimplemented!("{}", PHASE_7_HANDOFF_MSG)
            }
        }
    }
}
