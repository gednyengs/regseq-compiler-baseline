//! RegseqModel: the internal IR between rustc's `optimized_mir` and
//! the C backends.
//!
//! Stage 1 (vocabulary) types live here. See
//! `docs/discussions/regseq-model/stage-1-vocabulary.md` for the
//! spec and `status.md` in the same folder for the per-step plan.

mod idx;
mod ids;
mod index_vec;

pub use idx::Idx;
pub use ids::CrateId;
pub use index_vec::IndexVec;

const _: () = assert!(core::mem::size_of::<CrateId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<CrateId>>() == 4);
