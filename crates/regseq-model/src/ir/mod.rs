//! CFG-based IR types for the Regseq Model.

pub mod body;
pub mod intrinsic;
pub mod operand;
pub mod ops;
pub mod place;
pub mod rvalue;
pub mod statement;
pub mod terminator;
pub mod types;

pub use body::{BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LocalDecl, OrderedRegion};
pub use intrinsic::{
    parse_canonical_intrinsic_name, FormatKind, FormatPlaceholder, IntrinsicMetadata,
    RegisterFieldMeta, RegseqAbstraction, RegseqIntrinsic,
};
pub use operand::{LiteralValue, Operand};
pub use ops::{BinOp, UnOp};
pub use place::{Place, Projection};
pub use rvalue::{AggregateKind, BorrowKind, CastKind, Rvalue};
pub use statement::{MirIntrinsic, Statement, StatementKind};
pub use terminator::{SwitchTargets, Terminator, TerminatorKind};
pub use types::{Mutability, RegseqType, ScalarType};
