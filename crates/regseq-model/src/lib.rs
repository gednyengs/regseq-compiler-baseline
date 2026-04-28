//! RegseqModel: the internal IR between rustc's `optimized_mir` and
//! the C backends.
//!
//! Stage 1 (vocabulary) types live here. See
//! `docs/discussions/regseq-model/stage-1-vocabulary.md` for the
//! spec and `status.md` in the same folder for the per-step plan.

mod idx;
mod ids;
mod index_vec;
mod source;
mod string_interner;

pub use idx::Idx;
pub use ids::{CrateId, SourceFileId, StringId};
pub use index_vec::IndexVec;
pub use source::{Position, SourceFile, Span};
pub use string_interner::StringInterner;

const _: () = assert!(core::mem::size_of::<CrateId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<CrateId>>() == 4);
const _: () = assert!(core::mem::size_of::<StringId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<StringId>>() == 4);
const _: () = assert!(core::mem::size_of::<SourceFileId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<SourceFileId>>() == 4);
const _: () = assert!(core::mem::size_of::<Span>() == 12);
