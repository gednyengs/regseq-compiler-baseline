//! Declaration IR: normalized Layer 1 declaration types.
//!
//! The submodules here host the per-construct extraction-surface types
//! (clocks, resets, register blocks, properties, etc.) that Phase 1
//! construction consumes. They are an intermediate layer between
//! `regseq-driver`'s rustc-linked extractors and the target
//! [`crate::model::RegseqModel`].
//!
//! The legacy `DeclarationModel` struct, the HIR-style `bodies` IR,
//! the `traversal` helpers, and the `compat` / `binding` / `joined` /
//! `item` / `semantic` sibling layers were removed in Wave B. See
//! `docs/discussions/models/regseq-model/legacy-removal-plan.md`.

pub mod build_context;
pub mod callbacks;
pub mod mir_body;
pub mod operations;
pub mod properties;
pub mod provenance;
pub mod registers;
pub mod resources;
pub mod synchronization;

pub use build_context::DeclarationBuildContext;
pub use callbacks::{CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl};
pub use driver::{BusType, DeviceClass, DriverDecl};
pub use operations::{
    DelayIntrinsic, DelayPathKind, DurationCtor, DurationCtorSiteDecl, LogArgKind, LogCallSiteDecl,
    LogLevel, LoggingDeclSet, PmGateDecl, PmGateDeclSet, PmGateProperties, TimedDelayCallSiteDecl,
    TimedDelayDeclSet,
};
pub use properties::{PropertyDeclSet, PropertyFieldDecl, PropertyLintDecl};
pub use provenance::Provenance;
pub use registers::{
    PollCallSiteDecl, RegisterBlockDecl, RegisterDeclSet, RegisterDescriptorDecl,
    RegisterDescriptorFieldDecl, RegisterFieldDecl, RegisterNamedValueDecl, WrapperCallSiteDecl,
    WrapperMethod,
};
pub use resources::{
    ClockCallSiteDecl, ClockDeclSet, ClockFieldDecl, ClockMethod, InterruptBindingHints,
    InterruptDeclSet, InterruptFieldShape, InterruptResourceDecl, IsrBindTarget, IsrBindingDecl,
    MmioFieldDecl, PinCtrlCallSiteDecl, PinCtrlDeclSet, PinCtrlFieldDecl, ResetCallSiteDecl,
    ResetDeclSet, ResetFieldDecl, ResetMethod, ResourceDeclSet,
};
pub use synchronization::{
    CompletionMethod, CompletionOperationDecl, GuardScopeDecl, LockOperationDecl, SyncFieldDecl,
    SyncFieldKind, SynchronizationDeclSet,
};

pub mod driver;
