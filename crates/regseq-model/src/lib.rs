//! RegseqModel: the internal IR between rustc's `optimized_mir` and
//! the C backends.
//!
//! Stage 1 (vocabulary) types live here. The model is owned (no
//! `'tcx`), serializable, and rustc-free per Q1; the bridge in
//! `regseq-driver` builds a `RegseqModel` from `tcx` and hands it
//! off.

mod args_key;
mod basic_block;
mod body;
mod body_ty;
mod bridge_input;
mod build;
mod cabi;
mod clock;
mod const_interner;
mod const_value;
mod crate_info;
mod def_path;
mod deferred_work;
mod device_lifecycle;
mod dma;
mod domain_attr;
mod fn_sig;
mod fn_sig_interner;
mod ids;
mod idx;
mod index_vec;
mod instance;
mod instance_ref;
mod interrupt;
mod item;
mod layout;
mod layout_validator;
mod model;
pub mod operand;
mod place;
mod place_validator;
mod pinctrl;
mod pinctrl_semantics;
mod pretty;
mod serial;
pub mod rvalue;
mod source;
mod statement;
mod string_interner;
mod synchronization;
mod synthetic;
mod synthetic_def_path;
mod target;
mod terminator;
mod ty;
mod ty_interner;
mod visit;

pub use args_key::{
    args_key_for_generic_args, args_key_for_generic_args_with_pending, canonical_args_bytes,
    canonical_args_bytes_with_pending, canonical_ty_bytes, canonical_ty_bytes_with_pending,
    ArgsKey, DefPathHashLookup, GenericArg, GenericArgs,
};
pub use basic_block::BasicBlock;
pub use body::{Body, LocalDecl};
pub use body_ty::{
    operand_ty, place_enum_variant_count, place_ty, pointee_ty, rvalue_ty, TypeRecovery,
    VariantRecovery,
};
pub use bridge_input::BridgeInput;
pub use build::BuildInfo;
pub use cabi::{CAbiLowering, CAbiShape, SubstReason};
pub use const_interner::ConstInterner;
pub use const_value::{canonical_const_bytes, Byte, Const};
pub use crate_info::{CrateInfo, StableCrateId};
pub use def_path::{
    def_path_data_sort_rank, render_def_path, DefPath, DefPathData, DefPathHash, DefPathSegment,
};
pub use clock::{ClockBinding, ClockMode, ClockResource, ClockTables, DriverClocks};
pub use deferred_work::{
    DeferredWorkTables, DriverDeferredWork, SubmitSite, SubmitVariant, WorkResource,
};
pub use device_lifecycle::{
    BarType, BusType, ClassTag, DeviceLifecycleTables, DriverDescriptor, DriverHandle,
    DriverLifecycle, LifecycleOps, MatchEntry, MmioBinding, ResourceField, ResourceKind,
    UserDataField, UserDataKind,
};
pub use dma::{
    AsyncTransferChannel, DmaBufferOps, DmaChannelOps, DmaTables, DriverDma, OwnedDmaBuffer,
};
pub use domain_attr::{
    AtomicElement, AtomicOp, AttrData, ClockOp, ConfigOp, DeferredWorkOp, DomainAttrTables, FetchOp,
    FieldPath, FrameworkType, IntrinsicKind, LogLevel, MmioOp, PinctrlOp,
};
pub use fn_sig::{canonical_fn_sig, FnSig};
pub use fn_sig_interner::FnSigInterner;
pub use ids::{
    BlockId, ConstId, CrateId, FieldIdx, FnSigId, InstanceId, ItemId, LocalId, SourceFileId,
    StringId, Symbol, TyId, VariantIdx,
};
pub use idx::Idx;
pub use index_vec::IndexVec;
pub use instance::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, FunctionInstanceData,
    FunctionInstanceFlavor, Instance, InstanceKind, RegseqInstanceKey,
};
pub use instance_ref::InstanceRef;
pub use interrupt::{
    ArmReachability, DriverInterrupts, InterruptBinding, InterruptForm, InterruptResource,
    InterruptTables, IrqArmReachability, IsrHandler, TriggerPolarity, TriggerType,
};
pub use item::{BridgeItemFlags, InlineHint, Item, ItemKind, Linkage};
pub use synchronization::{
    AcquireSite, Cardinality, CompletionOpKind, CompletionSite, DriverSynchronization,
    SpinlockHeldRegion, SyncKind, SyncOp, SyncResource, SynchronizationTables,
};
pub use layout::{
    EnumShape, FieldLayout, Layout, LayoutKind, TagEncoding, TagScalar, VariantLayout,
    VariantRange, WrappingRange, UNIT_LAYOUT,
};
pub use model::{invariant_descriptor, InvariantId, InvariantViolation, RegseqModel};
pub use operand::{LiteralValue, Operand};
pub use place::{Place, PlaceElem};
pub use place_validator::{
    validate_body_places, PlaceLocation, PlaceSite, PlaceValidationError, PlaceValidationErrorKind,
};
pub use pinctrl::{
    recover_pinctrl_receiver, resolve_pin_state, DriverPinctrls, PinctrlBinding, PinctrlResource,
    PinctrlTables,
};
pub use pinctrl_semantics::{validate_pinctrl_semantics, PinctrlSemanticError};
pub use pretty::{PrettyPrintOptions, PrettyPrinter};
pub use serial::{
    DriverSerialPort, SerialEnumBinding, SerialPortOps, SerialPortTables, SerialTypeBindings,
};
pub use rvalue::{AggregateKind, BinOp, CastKind, PointerCoercion, Rvalue, UnOp};
pub use source::{Position, SourceFile, Span};
pub use statement::{CopyNonOverlapping, NonDivergingIntrinsic, Statement, StatementKind};
pub use string_interner::StringInterner;
pub use synthetic::{
    ClosureCallKind, ContentHash, PrincipalTraitKey, ProjectionBindingsKey, SyntheticItemKind,
};
pub use synthetic_def_path::{synthetic_def_path_hash, synthetic_def_path_segments};
pub use target::{
    default_scalar_layouts, Endian, PlatformIdentity, ScalarLayouts, SizeAlign, TargetInfo, TargetOs,
};
pub use terminator::{AssertKind, CallCallee, SwitchTargets, Terminator, TerminatorKind};
pub use ty::{
    AutoTrait, ClosedEnumDiscriminantError, FloatKind, FnAbi, IntKind, Mutability, Provenance,
    Safety, Ty, UintKind, FLOAT_KIND_COUNT, INT_KIND_COUNT, UINT_KIND_COUNT,
};
pub use ty_interner::{NominalKind, TyInterner};
pub use visit::{BodyValidationError, Visitor};

const _: () = assert!(core::mem::size_of::<CrateId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<CrateId>>() == 4);
const _: () = assert!(core::mem::size_of::<StringId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<StringId>>() == 4);
const _: () = assert!(core::mem::size_of::<SourceFileId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<SourceFileId>>() == 4);
const _: () = assert!(core::mem::size_of::<ItemId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<ItemId>>() == 4);
const _: () = assert!(core::mem::size_of::<TyId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<TyId>>() == 4);
const _: () = assert!(core::mem::size_of::<ConstId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<ConstId>>() == 4);
const _: () = assert!(core::mem::size_of::<FnSigId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<FnSigId>>() == 4);
const _: () = assert!(core::mem::size_of::<InstanceId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<InstanceId>>() == 4);
const _: () = assert!(core::mem::size_of::<LocalId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<LocalId>>() == 4);
const _: () = assert!(core::mem::size_of::<BlockId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<BlockId>>() == 4);
const _: () = assert!(core::mem::size_of::<VariantIdx>() == 4);
const _: () = assert!(core::mem::size_of::<Option<VariantIdx>>() == 4);
const _: () = assert!(core::mem::size_of::<FieldIdx>() == 4);
const _: () = assert!(core::mem::size_of::<Option<FieldIdx>>() == 4);
const _: () = assert!(core::mem::size_of::<Span>() == 12);
