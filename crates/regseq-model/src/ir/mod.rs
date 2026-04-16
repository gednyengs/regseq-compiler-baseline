//! CFG-based IR types for the Regseq Model.

pub mod intrinsic;
pub mod operand;
pub mod ops;
pub mod place;
pub mod rvalue;
pub mod types;

pub use intrinsic::{
    FormatKind, FormatPlaceholder, IntrinsicMetadata, RegisterFieldMeta, RegseqAbstraction,
    RegseqIntrinsic,
};
pub use operand::{LiteralValue, Operand};
pub use ops::{BinOp, UnOp};
pub use place::{Place, Projection};
pub use rvalue::{AggregateKind, BorrowKind, CastKind, Rvalue};
pub use types::{Mutability, RegseqType, ScalarType};
