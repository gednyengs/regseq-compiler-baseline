//! Unified RegseqModel types.
//!
//! This module hosts the rustc-independent data model that consolidates
//! declaration, binding, and lowering facts produced by the extraction
//! pipeline. Types are added incrementally as Phase 2 of the regseq-model
//! execution plan progresses.

pub mod callback;
pub mod constant;
pub mod driver;
pub mod framework;
pub mod function;
pub mod impl_block;
pub mod module;
pub mod mutation;
pub mod provenance;
pub mod register;
pub mod regseq_model;
pub mod resources;
pub mod type_def;

pub use callback::{CallbackContext, CallbackFamily};
pub use constant::Constant;
pub use driver::{
    BusKind, DeviceClass, DeviceClassKind, DriverIdentity, DriverStruct, PlatformBusBinding,
    PlatformMetadata, PmInfo,
};
pub use framework::{FrameworkTypeKind, TypeDefClassification};
pub use function::{Function, Param, ParamKind};
pub use impl_block::ImplBlock;
pub use module::{Module, ModuleKind};
pub use mutation::{MutationGuard, MutationPhase, ResourceBinding};
pub use provenance::Provenance;
pub use register::{
    AccessKind, BitfieldDef, NamedValue, RegisterBlock, RegisterBlockAttributes,
    RegisterBlockField, RegisterDescriptor, WrapperMethod,
};
pub use regseq_model::RegseqModel;
pub use resources::{
    ClockBinding, ClockDecl, ClockResource, InterruptBinding, InterruptDecl, InterruptResource,
    MmioBinding, MmioDecl, MmioResource, PinCtrlBinding, PinCtrlDecl, PinCtrlResource,
    PropertyField, ResetBinding, ResetDecl, ResetResource, ResourceKind, Shape, SyncFieldKind,
    SyncKind, SyncResource, UserDataField, UserDataKind,
};
pub use type_def::{AttrEntry, AttrValue, Attrs, FieldDef, TypeDef, TypeDefKind, VariantDef};
