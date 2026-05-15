//! RegseqModel: the internal IR between rustc's `optimized_mir` and
//! the C backends.
//!
//! Stage 1 (vocabulary) types live here. The model is owned (no
//! `'tcx`), serializable, and rustc-free per Q1; the bridge in
//! `regseq-driver` builds a `RegseqModel` from `tcx` and hands it
//! off.

mod bridge_input;
mod build;
mod crate_info;
mod def_path;
mod domain_attr;
mod ids;
mod idx;
mod index_vec;
mod instance;
mod model;
mod placeholders;
mod pretty;
mod source;
mod string_interner;
mod target;
mod ty;

pub use bridge_input::BridgeInput;
pub use build::BuildInfo;
pub use crate_info::{CrateInfo, StableCrateId};
pub use def_path::{
    def_path_data_sort_rank, render_def_path, DefPath, DefPathData, DefPathHash, DefPathSegment,
};
pub use domain_attr::DomainAttrTables;
pub use ids::{
    BlockId, ConstId, CrateId, FnSigId, InstanceId, ItemId, LocalId, SourceFileId, StringId, TyId,
    VariantIdx,
};
pub use idx::Idx;
pub use index_vec::IndexVec;
pub use instance::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, GenericArg, GenericArgs, Instance,
    InstanceKind, RustcInstanceKindTag,
};
pub use model::{invariant_descriptor, InvariantId, InvariantViolation, RegseqModel};
pub use placeholders::{
    Body, CAbiLowering, Const, ConstInterner, FnSig, FnSigInterner, InlineHint, IntrinsicKind,
    Item, Linkage, PrincipalTraitKey, SyntheticItemKind,
};
pub use pretty::PrettyPrinter;
pub use source::{Position, SourceFile, Span};
pub use string_interner::StringInterner;
pub use target::{default_scalar_layouts, Endian, ScalarLayouts, SizeAlign, TargetInfo};
pub use ty::{
    AutoTrait, ClosedEnumDiscriminantError, FloatKind, FnAbi, IntKind, Layout, Mutability,
    Provenance, Safety, Ty, TyInterner, UintKind, FLOAT_KIND_COUNT, INT_KIND_COUNT,
    UINT_KIND_COUNT,
};

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
const _: () = assert!(core::mem::size_of::<Span>() == 12);
