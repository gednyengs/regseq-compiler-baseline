//! Declaration IR: normalized Layer 1 declaration types.
//!
//! This module defines the first normalized declaration-model types for the
//! Regseq compiler. It coexists with later binding, join, and lowering-oriented
//! IR families and preserves construct ownership, typed distinctions, and
//! per-entity provenance.
//!
//! The declaration model starts from the current `RawFrontendArtifact` shape
//! as-is and does not assume Layer 2 binding data, join results, or backend
//! lowering strategies.

pub mod build_context;
pub mod callbacks;
pub mod operations;
pub mod properties;
pub mod provenance;
pub mod registers;
pub mod resources;
pub mod synchronization;

// Re-export top-level types for convenience.
pub use build_context::DeclarationBuildContext;
pub use callbacks::{CallbackContext, CallbackDeclSet, CallbackFamily, CallbackImplDecl};
pub use driver::{DeviceClass, DriverDecl};
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

/// Top-level normalized declaration model.
///
/// Contains all declaration-side facts for a single driver spec, organized
/// by construct family. Build-context metadata is kept separate from
/// declaration identity. An optional operation evidence index provides
/// supplementary cross-referencing for traversal convenience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationModel {
    /// Driver identity declaration.
    pub driver: DriverDecl,
    /// Resource declarations (mmio, interrupts, clock, reset, pinctrl).
    pub resources: ResourceDeclSet,
    /// Synchronization declarations (kept structurally distinct from resources).
    pub synchronization: SynchronizationDeclSet,
    /// Callback-family impl declarations.
    pub callbacks: CallbackDeclSet,
    /// Register block declarations and operation evidence.
    pub registers: RegisterDeclSet,
    /// Firmware-property declarations and lints.
    pub properties: PropertyDeclSet,
    /// Timed-delay declaration-side evidence.
    pub timed_delay: TimedDelayDeclSet,
    /// Logging declaration-side evidence.
    pub logging: LoggingDeclSet,
    /// PM gate declaration-side evidence.
    pub pm_gate: PmGateDeclSet,
    /// Build-context metadata (target_os etc.), separate from declaration identity.
    pub build_context: DeclarationBuildContext,
}

use serde::{Deserialize, Serialize};
