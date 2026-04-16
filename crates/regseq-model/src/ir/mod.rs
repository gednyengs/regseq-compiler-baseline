//! CFG-based IR types for the Regseq Model.

pub mod ops;
pub mod place;
pub mod types;

pub use ops::{BinOp, UnOp};
pub use place::{Place, Projection};
pub use types::{Mutability, RegseqType, ScalarType};
