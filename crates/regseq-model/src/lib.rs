//! RegseqModel: the internal IR between rustc's `optimized_mir` and
//! the C backends.
//!
//! Stage 1 (vocabulary) types live here. The model is owned (no
//! `'tcx`), serializable, and rustc-free per Q1; the bridge in
//! `regseq-driver` builds a `RegseqModel` from `tcx` and hands it
//! off.

mod crate_info;
mod def_path;
mod idx;
mod ids;
mod index_vec;
mod instance;
mod model;
mod placeholders;
mod pretty;
mod source;
mod string_interner;

pub use crate_info::{CrateInfo, StableCrateId};
pub use def_path::{
    def_path_data_sort_rank, render_def_path, DefPath, DefPathData, DefPathHash, DefPathSegment,
};
pub use idx::Idx;
pub use instance::{
    regseq_instance_kind_rank, rustc_instance_kind_sort_rank, GenericArg, GenericArgs, Instance,
    InstanceKind, RustcInstanceKindTag,
};
pub use model::{InvariantViolation, RegseqModel};
pub use placeholders::{Body, Const, FnSig, Item, Ty};
pub use pretty::PrettyPrinter;
pub use ids::{
    BodyId, ConstId, CrateId, FnSigId, InstanceId, ItemId, SourceFileId, StringId, TyId,
};
pub use index_vec::IndexVec;
pub use source::{Position, SourceFile, Span};
pub use string_interner::StringInterner;

const _: () = assert!(core::mem::size_of::<CrateId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<CrateId>>() == 4);
const _: () = assert!(core::mem::size_of::<StringId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<StringId>>() == 4);
const _: () = assert!(core::mem::size_of::<SourceFileId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<SourceFileId>>() == 4);
const _: () = assert!(core::mem::size_of::<ItemId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<ItemId>>() == 4);
const _: () = assert!(core::mem::size_of::<BodyId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<BodyId>>() == 4);
const _: () = assert!(core::mem::size_of::<TyId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<TyId>>() == 4);
const _: () = assert!(core::mem::size_of::<ConstId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<ConstId>>() == 4);
const _: () = assert!(core::mem::size_of::<FnSigId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<FnSigId>>() == 4);
const _: () = assert!(core::mem::size_of::<InstanceId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<InstanceId>>() == 4);
const _: () = assert!(core::mem::size_of::<Span>() == 12);
