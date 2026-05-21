//! Post-bridge inspection predicate enum + dispatch per strategy
//! section 7.
//!
//! F1 shipped the complete 22-variant enum (20 leaf variants plus
//! the two composites `All` / `Any`) so the sidecar could be
//! authored against the full predicate vocabulary, while
//! implementing dispatch only for the demo set's predicates
//! (`TyVariant`, `ItemKind`, `FunctionInstanceFlavor`, `All`,
//! `Any`). F2 (per `bridge-coverage-runner-strategy.md` section 11,
//! mini-F3 slice) added three more dispatch arms
//! (`SyntheticItemKind`, `ConstVariant`, `CallDivergent`). F3
//! closes out the remaining 14 leaf arms (`StatementKind`,
//! `TerminatorKind`, `RvalueVariant`, `PlaceElem`, `AggregateKind`,
//! `CastKind`, `BinOp`, `UnOp`, `AssertKind`,
//! `NonDivergingIntrinsic`, `CallCallee`, `BodyAbsent`, `Linkage`,
//! `ScalarKind`), bringing dispatch coverage to 20/20 leaves + 2/2
//! composites = the full 22-variant predicate vocabulary. Because
//! dispatch is now complete, [`InspectError`] no longer carries an
//! `Unimplemented` variant; the surviving variants are
//! [`InspectError::NotFound`] and [`InspectError::AnyFailed`].
//!
//! Strategy section 7 deviation (implementation-level only): the
//! strategy text presents `All`/`Any` as struct variants carrying
//! a `predicates: Vec<InspectionPredicate>` field. F1 uses
//! newtype variants (`All(Vec<InspectionPredicate>)`) so the
//! adjacently-tagged serde representation produces a flat
//! `{ kind = "All", arg = [...] }` TOML shape instead of the
//! nested `{ kind = "All", arg = { predicates = [...] } }`
//! shape struct variants force. The semantic content (all-of vs
//! any-of composition) is identical.

use serde::Deserialize;

use regseq_model::{
    AggregateKind, AssertKind, BinOp, CallCallee, CastKind, Const, InstanceKind, ItemKind, Linkage,
    NonDivergingIntrinsic, PlaceElem, PointerCoercion, RegseqModel, Rvalue, StatementKind,
    SyntheticItemKind, TerminatorKind, Ty, UnOp,
};

/// Closed inspection-predicate enum per strategy section 7.
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", content = "arg", rename_all = "PascalCase")]
pub enum InspectionPredicate {
    /// At least one `model.types` entry matches the named `Ty`
    /// variant (e.g., `"Enum"`, `"Bool"`, `"Dynamic"`).
    TyVariant(String),
    /// At least one Item's `kind` matches the named `ItemKind`
    /// variant (e.g., `"Static"`, `"Struct"`).
    ItemKind(String),
    /// At least one `SyntheticItemKind` entry matches.
    SyntheticItemKind(String),
    /// At least one Function Instance's flavor matches.
    FunctionInstanceFlavor(String),
    /// At least one body's statement matches the named variant.
    StatementKind(String),
    /// At least one body's terminator matches the named variant.
    TerminatorKind(String),
    /// At least one body's rvalue matches the named variant.
    RvalueVariant(String),
    /// At least one PlaceElem matches the named variant.
    PlaceElem(String),
    /// At least one Const matches the named variant.
    ConstVariant(String),
    /// At least one Rvalue::Aggregate uses the named AggregateKind.
    AggregateKind(String),
    /// At least one Rvalue::Cast uses the named CastKind.
    CastKind(String),
    /// At least one Rvalue::BinaryOp uses the named BinOp.
    BinOp(String),
    /// At least one Rvalue::UnaryOp uses the named UnOp.
    UnOp(String),
    /// At least one Terminator::Assert uses the named AssertKind.
    AssertKind(String),
    /// At least one NonDivergingIntrinsic sub-variant matches.
    NonDivergingIntrinsic(String),
    /// At least one Call terminator's callee matches.
    CallCallee(String),
    /// At least one Call terminator's divergence flag matches.
    CallDivergent(bool),
    /// At least one Function Instance has `body == None`.
    BodyAbsent,
    /// At least one Item carries the named Linkage variant.
    Linkage(String),
    /// At least one `Ty::Int`/`Ty::Uint`/`Ty::Float` carries the
    /// named scalar-kind variant.
    ScalarKind(String),
    /// Composite: ALL of the listed predicates hold.
    All(Vec<InspectionPredicate>),
    /// Composite: AT LEAST ONE of the listed predicates holds.
    Any(Vec<InspectionPredicate>),
}

/// Errors returned by [`check`].
#[derive(Debug, Clone)]
pub enum InspectError {
    /// Dispatch ran but the construct was not found in the model.
    NotFound {
        /// Identifying tag for the predicate variant.
        variant: &'static str,
        /// Argument the predicate was searched for.
        arg: String,
    },
    /// `Any` composite ran but no nested predicate held; carries
    /// every nested error for diagnostics.
    AnyFailed {
        /// Per-child diagnostics.
        children: Vec<InspectError>,
    },
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InspectError::NotFound { variant, arg } => {
                write!(
                    f,
                    "inspection predicate {variant}({arg:?}) did not match any model entry"
                )
            }
            InspectError::AnyFailed { children } => {
                write!(
                    f,
                    "Any composite: no child predicate matched ({} child errors)",
                    children.len()
                )
            }
        }
    }
}

/// Variant tag for a `Ty` value.
fn ty_variant_tag(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::Char => "Char",
        Ty::Int(_) => "Int",
        Ty::Uint(_) => "Uint",
        Ty::Float(_) => "Float",
        Ty::Str => "Str",
        Ty::Never => "Never",
        Ty::Array(_, _) => "Array",
        Ty::Slice(_) => "Slice",
        Ty::Ref(_, _) => "Ref",
        Ty::Ptr(_, _) => "Ptr",
        Ty::FnDef(_, _) => "FnDef",
        Ty::FnPtr(_) => "FnPtr",
        Ty::Dynamic { .. } => "Dynamic",
        Ty::Closure(_, _) => "Closure",
        Ty::Tuple { .. } => "Tuple",
        Ty::Struct { .. } => "Struct",
        Ty::Enum { .. } => "Enum",
        Ty::Union { .. } => "Union",
    }
}

/// Variant tag for an `ItemKind` value.
fn item_kind_tag(kind: &ItemKind) -> &'static str {
    match kind {
        ItemKind::Function => "Function",
        ItemKind::Static { .. } => "Static",
        ItemKind::Const => "Const",
        ItemKind::Struct => "Struct",
        ItemKind::Enum => "Enum",
        ItemKind::Union => "Union",
    }
}

/// Variant tag for a `SyntheticItemKind` value.
fn synthetic_kind_variant_tag(sk: &SyntheticItemKind) -> &'static str {
    match sk {
        SyntheticItemKind::None => "None",
        SyntheticItemKind::ClosureCaptures { .. } => "ClosureCaptures",
        SyntheticItemKind::ClosureCall { .. } => "ClosureCall",
        SyntheticItemKind::ClosureDropGlue { .. } => "ClosureDropGlue",
        SyntheticItemKind::DropGlue { .. } => "DropGlue",
        SyntheticItemKind::VTableType { .. } => "VTableType",
        SyntheticItemKind::VTableInstance { .. } => "VTableInstance",
        SyntheticItemKind::VTableMethodShim { .. } => "VTableMethodShim",
        SyntheticItemKind::VTableDropShim { .. } => "VTableDropShim",
        SyntheticItemKind::ConstMemoryStatic { .. } => "ConstMemoryStatic",
    }
}

/// Variant tag for a `Const` value.
fn const_variant_tag(c: &Const) -> &'static str {
    match c {
        Const::Bool(_) => "Bool",
        Const::Char(_) => "Char",
        Const::Int(_, _) => "Int",
        Const::Uint(_, _) => "Uint",
        Const::Float(_, _) => "Float",
        Const::ZST => "ZST",
        Const::Bytes(_) => "Bytes",
        Const::Aggregate(_) => "Aggregate",
        Const::EnumValue { .. } => "EnumValue",
        Const::NullPtr(_) => "NullPtr",
        Const::Reloc(_) => "Reloc",
    }
}

/// Variant tag for a `FunctionInstanceFlavor` value. Mirrors the
/// `Debug` rendering.
fn flavor_tag(flavor: &regseq_model::FunctionInstanceFlavor) -> &'static str {
    use regseq_model::FunctionInstanceFlavor as F;
    match flavor {
        F::Item => "Item",
        F::Intrinsic => "Intrinsic",
        F::VTableShim => "VTableShim",
        F::ReifyShim => "ReifyShim",
        F::FnPtrShim => "FnPtrShim",
        F::Virtual => "Virtual",
        F::ClosureOnceShim => "ClosureOnceShim",
        F::ConstructCoroutineInClosureShim => "ConstructCoroutineInClosureShim",
        F::ThreadLocalShim => "ThreadLocalShim",
        F::FutureDropPollShim => "FutureDropPollShim",
        F::DropGlue => "DropGlue",
        F::CloneShim => "CloneShim",
        F::FnPtrAddrShim => "FnPtrAddrShim",
        F::AsyncDropGlueCtorShim => "AsyncDropGlueCtorShim",
        F::AsyncDropGlue => "AsyncDropGlue",
    }
}

/// Variant tag for a `StatementKind` value (3 variants).
fn statement_kind_tag(kind: &StatementKind) -> &'static str {
    match kind {
        StatementKind::Assign(_, _) => "Assign",
        StatementKind::SetDiscriminant(_, _) => "SetDiscriminant",
        StatementKind::Intrinsic(_) => "Intrinsic",
    }
}

/// Variant tag for a `TerminatorKind` value (6 variants).
fn terminator_kind_tag(kind: &TerminatorKind) -> &'static str {
    match kind {
        TerminatorKind::Goto(_) => "Goto",
        TerminatorKind::SwitchInt { .. } => "SwitchInt",
        TerminatorKind::Return => "Return",
        TerminatorKind::Unreachable => "Unreachable",
        TerminatorKind::Call { .. } => "Call",
        TerminatorKind::Assert { .. } => "Assert",
    }
}

/// Variant tag for an `Rvalue` value (9 variants).
fn rvalue_variant_tag(rvalue: &Rvalue) -> &'static str {
    match rvalue {
        Rvalue::Use(_) => "Use",
        Rvalue::Repeat(_, _) => "Repeat",
        Rvalue::Ref(_, _) => "Ref",
        Rvalue::RawPtr(_, _) => "RawPtr",
        Rvalue::Cast(_, _, _) => "Cast",
        Rvalue::BinaryOp(_, _, _) => "BinaryOp",
        Rvalue::UnaryOp(_, _) => "UnaryOp",
        Rvalue::Discriminant(_) => "Discriminant",
        Rvalue::Aggregate(_, _) => "Aggregate",
    }
}

/// Variant tag for a `PlaceElem` value (6 variants).
fn place_elem_tag(elem: &PlaceElem) -> &'static str {
    match elem {
        PlaceElem::Deref => "Deref",
        PlaceElem::Field(_, _) => "Field",
        PlaceElem::Index(_) => "Index",
        PlaceElem::ConstantIndex { .. } => "ConstantIndex",
        PlaceElem::Subslice { .. } => "Subslice",
        PlaceElem::Downcast(_) => "Downcast",
    }
}

/// Variant tag for an `AggregateKind` value (7 variants).
fn aggregate_kind_tag(kind: &AggregateKind) -> &'static str {
    match kind {
        AggregateKind::Array(_) => "Array",
        AggregateKind::Tuple(_) => "Tuple",
        AggregateKind::Struct(_) => "Struct",
        AggregateKind::Enum(_, _) => "Enum",
        AggregateKind::Union(_, _) => "Union",
        AggregateKind::Closure(_) => "Closure",
        AggregateKind::RawPtr(_, _) => "RawPtr",
    }
}

/// Variant tag for a `CastKind` value. The nested
/// `CastKind::PointerCoercion(sub)` case is rendered as a dotted
/// compound (`"PointerCoercion::ReifyFnPointer"` etc.) so the
/// inspection-predicate `arg` strings can distinguish the four
/// nested sub-variants; the flat `"PointerCoercion"` tag would
/// collapse them ambiguously. All other variants use their flat
/// variant tag. The dotted-compound convention is also recorded in
/// `crates/bridge-coverage-runner/snippets/README.md` so sidecar
/// authors can target the right encoding.
fn cast_kind_tag(kind: &CastKind) -> &'static str {
    match kind {
        CastKind::PointerExposeProvenance => "PointerExposeProvenance",
        CastKind::PointerWithExposedProvenance => "PointerWithExposedProvenance",
        CastKind::PointerCoercion(sub) => match sub {
            PointerCoercion::ReifyFnPointer => "PointerCoercion::ReifyFnPointer",
            PointerCoercion::UnsafeFnPointer => "PointerCoercion::UnsafeFnPointer",
            PointerCoercion::ClosureFnPointer => "PointerCoercion::ClosureFnPointer",
            PointerCoercion::Unsize => "PointerCoercion::Unsize",
        },
        CastKind::IntToInt => "IntToInt",
        CastKind::FloatToInt => "FloatToInt",
        CastKind::FloatToFloat => "FloatToFloat",
        CastKind::IntToFloat => "IntToFloat",
        CastKind::PtrToPtr => "PtrToPtr",
        CastKind::FnPtrToPtr => "FnPtrToPtr",
        CastKind::Transmute => "Transmute",
        CastKind::Subtype => "Subtype",
    }
}

/// Variant tag for a `BinOp` value (26 variants).
fn bin_op_tag(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "Add",
        BinOp::Sub => "Sub",
        BinOp::Mul => "Mul",
        BinOp::Div => "Div",
        BinOp::Rem => "Rem",
        BinOp::AddUnchecked => "AddUnchecked",
        BinOp::SubUnchecked => "SubUnchecked",
        BinOp::MulUnchecked => "MulUnchecked",
        BinOp::AddWithOverflow => "AddWithOverflow",
        BinOp::SubWithOverflow => "SubWithOverflow",
        BinOp::MulWithOverflow => "MulWithOverflow",
        BinOp::BitXor => "BitXor",
        BinOp::BitAnd => "BitAnd",
        BinOp::BitOr => "BitOr",
        BinOp::Shl => "Shl",
        BinOp::ShlUnchecked => "ShlUnchecked",
        BinOp::Shr => "Shr",
        BinOp::ShrUnchecked => "ShrUnchecked",
        BinOp::Eq => "Eq",
        BinOp::Lt => "Lt",
        BinOp::Le => "Le",
        BinOp::Ne => "Ne",
        BinOp::Ge => "Ge",
        BinOp::Gt => "Gt",
        BinOp::Cmp => "Cmp",
        BinOp::Offset => "Offset",
    }
}

/// Variant tag for a `UnOp` value (3 variants).
fn un_op_tag(op: &UnOp) -> &'static str {
    match op {
        UnOp::Not => "Not",
        UnOp::Neg => "Neg",
        UnOp::PtrMetadata => "PtrMetadata",
    }
}

/// Variant tag for an `AssertKind` value (8 variants).
fn assert_kind_tag(kind: &AssertKind) -> &'static str {
    match kind {
        AssertKind::BoundsCheck { .. } => "BoundsCheck",
        AssertKind::Overflow(_, _, _) => "Overflow",
        AssertKind::OverflowNeg(_) => "OverflowNeg",
        AssertKind::DivisionByZero(_) => "DivisionByZero",
        AssertKind::RemainderByZero(_) => "RemainderByZero",
        AssertKind::MisalignedPointerDereference { .. } => "MisalignedPointerDereference",
        AssertKind::NullPointerDereference => "NullPointerDereference",
        AssertKind::InvalidEnumConstruction(_) => "InvalidEnumConstruction",
    }
}

/// Variant tag for a `NonDivergingIntrinsic` value (2 variants).
fn non_diverging_intrinsic_tag(intrinsic: &NonDivergingIntrinsic) -> &'static str {
    match intrinsic {
        NonDivergingIntrinsic::Assume(_) => "Assume",
        NonDivergingIntrinsic::CopyNonOverlapping(_) => "CopyNonOverlapping",
    }
}

/// Variant tag for a `CallCallee` value (2 variants).
fn call_callee_tag(callee: &CallCallee) -> &'static str {
    match callee {
        CallCallee::Direct(_) => "Direct",
        CallCallee::Indirect(_) => "Indirect",
    }
}

/// Variant tag for a `Linkage` value (4 variants).
fn linkage_tag(linkage: &Linkage) -> &'static str {
    match linkage {
        Linkage::Default => "Default",
        Linkage::NoMangle => "NoMangle",
        Linkage::Foreign => "Foreign",
        Linkage::Weak => "Weak",
    }
}

/// Walk every `Place::projection` element that appears anywhere in
/// `bb` (statement places + terminator places + assert/switch/call
/// operand-bearing places, as covered by the basic-block walker
/// below), returning true if any projection element's variant tag
/// matches `needle`.
fn any_place_elem_in_block(bb: &regseq_model::BasicBlock, needle: &str) -> bool {
    for stmt in bb.statements.iter() {
        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                if any_place_elem_in_place(place, needle) {
                    return true;
                }
                if any_place_elem_in_rvalue(rvalue, needle) {
                    return true;
                }
            }
            StatementKind::SetDiscriminant(place, _) => {
                if any_place_elem_in_place(place, needle) {
                    return true;
                }
            }
            StatementKind::Intrinsic(intrinsic) => {
                if any_place_elem_in_intrinsic(intrinsic, needle) {
                    return true;
                }
            }
        }
    }
    match &bb.terminator.kind {
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        TerminatorKind::SwitchInt { discr, .. } => {
            if any_place_elem_in_operand(discr, needle) {
                return true;
            }
        }
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            if any_place_elem_in_place(destination, needle) {
                return true;
            }
            if let CallCallee::Indirect(operand) = callee {
                if any_place_elem_in_operand(operand, needle) {
                    return true;
                }
            }
            for arg in args.iter() {
                if any_place_elem_in_operand(arg, needle) {
                    return true;
                }
            }
        }
        TerminatorKind::Assert { cond, msg, .. } => {
            if any_place_elem_in_operand(cond, needle) {
                return true;
            }
            if any_place_elem_in_assert_kind(msg, needle) {
                return true;
            }
        }
    }
    false
}

fn any_place_elem_in_place(place: &regseq_model::Place, needle: &str) -> bool {
    place
        .projection
        .iter()
        .any(|elem| place_elem_tag(elem) == needle)
}

fn any_place_elem_in_operand(operand: &regseq_model::Operand, needle: &str) -> bool {
    match operand {
        regseq_model::Operand::Copy(place) | regseq_model::Operand::Move(place) => {
            any_place_elem_in_place(place, needle)
        }
        regseq_model::Operand::Constant(_) => false,
    }
}

fn any_place_elem_in_rvalue(rvalue: &Rvalue, needle: &str) -> bool {
    match rvalue {
        Rvalue::Use(op) => any_place_elem_in_operand(op, needle),
        Rvalue::Repeat(op, _) => any_place_elem_in_operand(op, needle),
        Rvalue::Ref(_, place) => any_place_elem_in_place(place, needle),
        Rvalue::RawPtr(_, place) => any_place_elem_in_place(place, needle),
        Rvalue::Cast(_, op, _) => any_place_elem_in_operand(op, needle),
        Rvalue::BinaryOp(_, lhs, rhs) => {
            any_place_elem_in_operand(lhs, needle) || any_place_elem_in_operand(rhs, needle)
        }
        Rvalue::UnaryOp(_, op) => any_place_elem_in_operand(op, needle),
        Rvalue::Discriminant(place) => any_place_elem_in_place(place, needle),
        Rvalue::Aggregate(_, ops) => ops.iter().any(|op| any_place_elem_in_operand(op, needle)),
    }
}

fn any_place_elem_in_intrinsic(intrinsic: &NonDivergingIntrinsic, needle: &str) -> bool {
    match intrinsic {
        NonDivergingIntrinsic::Assume(op) => any_place_elem_in_operand(op, needle),
        NonDivergingIntrinsic::CopyNonOverlapping(c) => {
            any_place_elem_in_operand(&c.src, needle)
                || any_place_elem_in_operand(&c.dst, needle)
                || any_place_elem_in_operand(&c.count, needle)
        }
    }
}

fn any_place_elem_in_assert_kind(kind: &AssertKind, needle: &str) -> bool {
    match kind {
        AssertKind::BoundsCheck { len, index } => {
            any_place_elem_in_operand(len, needle) || any_place_elem_in_operand(index, needle)
        }
        AssertKind::Overflow(_, lhs, rhs) => {
            any_place_elem_in_operand(lhs, needle) || any_place_elem_in_operand(rhs, needle)
        }
        AssertKind::OverflowNeg(op)
        | AssertKind::DivisionByZero(op)
        | AssertKind::RemainderByZero(op)
        | AssertKind::InvalidEnumConstruction(op) => any_place_elem_in_operand(op, needle),
        AssertKind::MisalignedPointerDereference { required, found } => {
            any_place_elem_in_operand(required, needle) || any_place_elem_in_operand(found, needle)
        }
        AssertKind::NullPointerDereference => false,
    }
}

/// Inner scalar-kind variant tag for a `Ty` value if it is
/// `Ty::Int(_)` / `Ty::Uint(_)` / `Ty::Float(_)`. Returns `None`
/// for every other `Ty` variant. The returned tag is the flat
/// variant tag of the inner `IntKind` / `UintKind` / `FloatKind`
/// (e.g., `"I8"`, `"U16"`, `"F128"`).
fn scalar_kind_tag_from_ty(ty: &Ty) -> Option<&'static str> {
    use regseq_model::{FloatKind, IntKind, UintKind};
    match ty {
        Ty::Int(ik) => Some(match ik {
            IntKind::I8 => "I8",
            IntKind::I16 => "I16",
            IntKind::I32 => "I32",
            IntKind::I64 => "I64",
            IntKind::I128 => "I128",
            IntKind::Isize => "Isize",
        }),
        Ty::Uint(uk) => Some(match uk {
            UintKind::U8 => "U8",
            UintKind::U16 => "U16",
            UintKind::U32 => "U32",
            UintKind::U64 => "U64",
            UintKind::U128 => "U128",
            UintKind::Usize => "Usize",
        }),
        Ty::Float(fk) => Some(match fk {
            FloatKind::F16 => "F16",
            FloatKind::F32 => "F32",
            FloatKind::F64 => "F64",
            FloatKind::F128 => "F128",
        }),
        _ => None,
    }
}

/// Walk `model` and verify `pred` holds.
pub fn check(model: &RegseqModel, pred: &InspectionPredicate) -> Result<(), InspectError> {
    match pred {
        InspectionPredicate::TyVariant(arg) => {
            let hit = model
                .types
                .iter()
                .any(|(_, ty)| ty_variant_tag(ty) == arg.as_str());
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "TyVariant",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::ItemKind(arg) => {
            let hit = model
                .items
                .iter()
                .any(|item| item_kind_tag(&item.kind) == arg.as_str());
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "ItemKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::FunctionInstanceFlavor(arg) => {
            let hit = model.instances.iter().any(|inst| match &inst.kind {
                InstanceKind::Function(data) => flavor_tag(&data.flavor) == arg.as_str(),
                _ => false,
            });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "FunctionInstanceFlavor",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::SyntheticItemKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .item_synthetic_kinds
                .iter()
                .any(|sk| synthetic_kind_variant_tag(sk) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "SyntheticItemKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::ConstVariant(arg) => {
            let needle = arg.as_str();
            let hit = model
                .consts
                .iter()
                .any(|(_, c)| const_variant_tag(c) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "ConstVariant",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::CallDivergent(arg) => {
            let needs_divergent = *arg;
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| match &bb.terminator.kind {
                    TerminatorKind::Call { target, .. } => target.is_none() == needs_divergent,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "CallDivergent",
                    arg: needs_divergent.to_string(),
                })
            }
        }
        InspectionPredicate::StatementKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| statement_kind_tag(&stmt.kind) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "StatementKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::TerminatorKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| terminator_kind_tag(&bb.terminator.kind) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "TerminatorKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::RvalueVariant(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Assign(_, rvalue) => rvalue_variant_tag(rvalue) == needle,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "RvalueVariant",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::PlaceElem(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| any_place_elem_in_block(bb, needle));
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "PlaceElem",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::AggregateKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Assign(_, Rvalue::Aggregate(kind, _)) => {
                        aggregate_kind_tag(kind) == needle
                    }
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "AggregateKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::CastKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Assign(_, Rvalue::Cast(kind, _, _)) => {
                        cast_kind_tag(kind) == needle
                    }
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "CastKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::BinOp(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Assign(_, Rvalue::BinaryOp(op, _, _)) => {
                        bin_op_tag(op) == needle
                    }
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "BinOp",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::UnOp(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Assign(_, Rvalue::UnaryOp(op, _)) => un_op_tag(op) == needle,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "UnOp",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::AssertKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| match &bb.terminator.kind {
                    TerminatorKind::Assert { msg, .. } => assert_kind_tag(msg) == needle,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "AssertKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::NonDivergingIntrinsic(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .flat_map(|bb| bb.statements.iter())
                .any(|stmt| match &stmt.kind {
                    StatementKind::Intrinsic(intrinsic) => {
                        non_diverging_intrinsic_tag(intrinsic) == needle
                    }
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "NonDivergingIntrinsic",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::CallCallee(arg) => {
            let needle = arg.as_str();
            let hit = model
                .instances
                .iter()
                .filter_map(|inst| match &inst.kind {
                    InstanceKind::Function(data) => data.body.as_ref(),
                    _ => None,
                })
                .flat_map(|body| body.basic_blocks.iter())
                .any(|bb| match &bb.terminator.kind {
                    TerminatorKind::Call { callee, .. } => call_callee_tag(callee) == needle,
                    _ => false,
                });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "CallCallee",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::BodyAbsent => {
            let hit = model.instances.iter().any(|inst| match &inst.kind {
                InstanceKind::Function(data) => data.body.is_none(),
                _ => false,
            });
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "BodyAbsent",
                    arg: String::new(),
                })
            }
        }
        InspectionPredicate::Linkage(arg) => {
            let needle = arg.as_str();
            let hit = model
                .items
                .iter()
                .any(|item| linkage_tag(&item.linkage) == needle);
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "Linkage",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::ScalarKind(arg) => {
            let needle = arg.as_str();
            let hit = model
                .types
                .iter()
                .any(|(_, ty)| scalar_kind_tag_from_ty(ty) == Some(needle));
            if hit {
                Ok(())
            } else {
                Err(InspectError::NotFound {
                    variant: "ScalarKind",
                    arg: arg.clone(),
                })
            }
        }
        InspectionPredicate::All(children) => {
            for child in children {
                check(model, child)?;
            }
            Ok(())
        }
        InspectionPredicate::Any(children) => {
            let mut errors = Vec::with_capacity(children.len());
            for child in children {
                match check(model, child) {
                    Ok(()) => return Ok(()),
                    Err(err) => errors.push(err),
                }
            }
            Err(InspectError::AnyFailed { children: errors })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use regseq_model::{
        ArgsKey, BasicBlock, Body, CallCallee, FloatKind, FnAbi, FnSig, FunctionInstanceData,
        FunctionInstanceFlavor, GenericArgs, IndexVec, InstanceId, InstanceKind, IntKind, ItemId,
        LocalDecl, LocalId, Mutability, Place, PrincipalTraitKey, ProjectionBindingsKey,
        RegseqModel, Safety, SourceFileId, Span, StableCrateId, Statement, Terminator, UNIT_LAYOUT,
    };

    fn empty_model() -> RegseqModel {
        RegseqModel::with_primary("inspect_test", StableCrateId(1))
    }

    fn dummy_span() -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo: 0,
            hi: 0,
        }
    }

    fn dummy_principal(local: u64) -> PrincipalTraitKey {
        PrincipalTraitKey {
            trait_path: regseq_model::DefPathHash {
                stable_crate_id: StableCrateId(1),
                local_hash: local,
            },
            trait_args: ArgsKey(0x1111),
            projection_bindings: ProjectionBindingsKey(0x2222),
        }
    }

    /// Build a `Body` whose basic-block terminators are exactly
    /// `terminators`, without statements.
    fn body_with_terminators(
        unit_ty: regseq_model::TyId,
        terminators: Vec<TerminatorKind>,
    ) -> Body {
        let mut basic_blocks: IndexVec<regseq_model::BlockId, BasicBlock> = IndexVec::new();
        for kind in terminators {
            basic_blocks.push(BasicBlock {
                statements: Box::new([]),
                terminator: Terminator {
                    kind,
                    span: dummy_span(),
                },
            });
        }
        let mut local_decls: IndexVec<LocalId, LocalDecl> = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: unit_ty,
            mutability: Mutability::Mut,
            span: dummy_span(),
            debug_name: None,
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: dummy_span(),
        }
    }

    /// Push a minimal `Function` instance carrying `body`. The
    /// instance back-references `ItemId::ZERO`; the dispatch arms
    /// under test never resolve item back-references, so a stub
    /// is sufficient.
    fn push_function_instance(model: &mut RegseqModel, body: Body) {
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let sig = model.sigs.intern(FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        let emission_name = model.strings.intern("inspect_test_fn");
        model.instances.push(regseq_model::Instance {
            item: ItemId::ZERO,
            generic_args: GenericArgs::new(),
            emission_name,
            span: dummy_span(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: sig,
                body: Some(body),
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        });
    }

    #[test]
    fn synthetic_item_kind_arm_matches_present_variant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        model
            .item_synthetic_kinds
            .push(SyntheticItemKind::VTableDropShim {
                principal: dummy_principal(0x7A17),
                self_ty: ArgsKey(0x3333),
            });

        assert!(check(
            &model,
            &InspectionPredicate::SyntheticItemKind("VTableDropShim".to_string()),
        )
        .is_ok());

        match check(
            &model,
            &InspectionPredicate::SyntheticItemKind("VTableType".to_string()),
        ) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "SyntheticItemKind");
                assert_eq!(arg, "VTableType");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn const_variant_arm_matches_present_variant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        model.consts.intern(Const::Float(FloatKind::F32, 0));
        model.consts.intern(Const::Int(IntKind::I32, 0));

        assert!(check(
            &model,
            &InspectionPredicate::ConstVariant("Float".to_string()),
        )
        .is_ok());
        assert!(check(
            &model,
            &InspectionPredicate::ConstVariant("Int".to_string()),
        )
        .is_ok());

        match check(
            &model,
            &InspectionPredicate::ConstVariant("Reloc".to_string()),
        ) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "ConstVariant");
                assert_eq!(arg, "Reloc");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn call_divergent_arm_matches_divergent_call_and_reports_not_found_otherwise() {
        let mut divergent_model = empty_model();
        let unit_ty = divergent_model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let body = body_with_terminators(
            unit_ty,
            vec![TerminatorKind::Call {
                callee: CallCallee::Direct(InstanceId::ZERO),
                args: Box::new([]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: None,
            }],
        );
        push_function_instance(&mut divergent_model, body);

        assert!(check(&divergent_model, &InspectionPredicate::CallDivergent(true),).is_ok());
        match check(&divergent_model, &InspectionPredicate::CallDivergent(false)) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "CallDivergent");
                assert_eq!(arg, "false");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }

        let mut returning_model = empty_model();
        let unit_ty = returning_model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let body = body_with_terminators(
            unit_ty,
            vec![
                TerminatorKind::Call {
                    callee: CallCallee::Direct(InstanceId::ZERO),
                    args: Box::new([]),
                    destination: Place::from(LocalId::RETURN_PLACE),
                    target: Some(regseq_model::BlockId::from_u32(1)),
                },
                TerminatorKind::Return,
            ],
        );
        push_function_instance(&mut returning_model, body);

        assert!(check(&returning_model, &InspectionPredicate::CallDivergent(false),).is_ok());
        match check(&returning_model, &InspectionPredicate::CallDivergent(true)) {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, "CallDivergent");
                assert_eq!(arg, "true");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    fn assert_not_found(result: Result<(), InspectError>, expect_variant: &str, expect_arg: &str) {
        match result {
            Err(InspectError::NotFound { variant, arg }) => {
                assert_eq!(variant, expect_variant);
                assert_eq!(arg, expect_arg);
            }
            other => panic!("expected NotFound({expect_variant}, {expect_arg}), got {other:?}"),
        }
    }

    fn statement(kind: StatementKind) -> Statement {
        Statement {
            kind,
            span: dummy_span(),
        }
    }

    fn body_with(statements: Vec<Statement>, terminator: TerminatorKind) -> Body {
        let mut basic_blocks: IndexVec<regseq_model::BlockId, BasicBlock> = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: statements.into_boxed_slice(),
            terminator: Terminator {
                kind: terminator,
                span: dummy_span(),
            },
        });
        let mut local_decls: IndexVec<LocalId, LocalDecl> = IndexVec::new();
        local_decls.push(LocalDecl {
            ty: regseq_model::TyId::from_u32(0),
            mutability: Mutability::Mut,
            span: dummy_span(),
            debug_name: None,
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: dummy_span(),
        }
    }

    fn const_op(model: &mut RegseqModel) -> regseq_model::Operand {
        let c = model.consts.intern(regseq_model::Const::Bool(false));
        regseq_model::Operand::Constant(c)
    }

    /// Push a Function instance carrying `body` with the given flavor.
    fn push_function_with_flavor(
        model: &mut RegseqModel,
        body: Option<Body>,
        flavor: regseq_model::FunctionInstanceFlavor,
    ) {
        let unit_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let sig = model.sigs.intern(FnSig {
            inputs: vec![],
            output: unit_ty,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        let emission_name = model.strings.intern("inspect_test_fn");
        model.instances.push(regseq_model::Instance {
            item: ItemId::ZERO,
            generic_args: GenericArgs::new(),
            emission_name,
            span: dummy_span(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig: sig,
                body,
                flavor,
                virtual_idx: None,
            }),
        });
    }

    fn push_item(model: &mut RegseqModel, kind: ItemKind, linkage: regseq_model::Linkage) {
        use regseq_model::{DefPath, DefPathData, DefPathSegment, InlineHint};
        let rendered = model.strings.intern("test::item");
        model.items.push(regseq_model::Item {
            def_path: DefPath {
                crate_id: regseq_model::CrateId::ZERO,
                segments: vec![
                    DefPathSegment {
                        data: DefPathData::CrateRoot,
                        disambiguator: 0,
                    },
                    DefPathSegment {
                        data: DefPathData::ValueNs(rendered),
                        disambiguator: 0,
                    },
                ],
                rendered,
            },
            kind,
            span: dummy_span(),
            linkage,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        });
        model.item_def_paths.push(regseq_model::DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: model.items.len() as u64,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
    }

    #[test]
    fn statement_kind_arm_matches_set_discriminant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let body = body_with(
            vec![statement(StatementKind::SetDiscriminant(
                Place::from(LocalId::RETURN_PLACE),
                regseq_model::VariantIdx::from_u32(0),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::StatementKind("SetDiscriminant".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::StatementKind("Intrinsic".to_string()),
            ),
            "StatementKind",
            "Intrinsic",
        );
    }

    #[test]
    fn terminator_kind_arm_matches_return_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let body = body_with(vec![], TerminatorKind::Return);
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::TerminatorKind("Return".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::TerminatorKind("Unreachable".to_string()),
            ),
            "TerminatorKind",
            "Unreachable",
        );
    }

    #[test]
    fn rvalue_variant_arm_matches_discriminant_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let body = body_with(
            vec![statement(StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
                Rvalue::Discriminant(Place::from(LocalId::RETURN_PLACE)),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::RvalueVariant("Discriminant".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::RvalueVariant("Aggregate".to_string()),
            ),
            "RvalueVariant",
            "Aggregate",
        );
    }

    #[test]
    fn place_elem_arm_matches_deref_in_assign_lhs_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let body = body_with(
            vec![statement(StatementKind::Assign(
                Place {
                    local: LocalId::RETURN_PLACE,
                    projection: Box::new([PlaceElem::Deref]),
                },
                Rvalue::Discriminant(Place::from(LocalId::RETURN_PLACE)),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(&model, &InspectionPredicate::PlaceElem("Deref".to_string()),).is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::PlaceElem("Subslice".to_string()),
            ),
            "PlaceElem",
            "Subslice",
        );
    }

    #[test]
    fn aggregate_kind_arm_matches_tuple_aggregate_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let tuple_ty = model.types.intern(Ty::Tuple {
            fields: vec![],
            layout: UNIT_LAYOUT,
        });
        let body = body_with(
            vec![statement(StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
                Rvalue::Aggregate(AggregateKind::Tuple(tuple_ty), Box::new([])),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::AggregateKind("Tuple".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::AggregateKind("Array".to_string()),
            ),
            "AggregateKind",
            "Array",
        );
    }

    #[test]
    fn cast_kind_arm_matches_pointer_coercion_dotted_compound_and_flat_variants() {
        let mut model = empty_model();
        let cop = const_op(&mut model);
        let dest_ty = regseq_model::TyId::from_u32(0);
        let body = body_with(
            vec![
                statement(StatementKind::Assign(
                    Place::from(LocalId::RETURN_PLACE),
                    Rvalue::Cast(
                        CastKind::PointerCoercion(PointerCoercion::Unsize),
                        cop.clone(),
                        dest_ty,
                    ),
                )),
                statement(StatementKind::Assign(
                    Place::from(LocalId::RETURN_PLACE),
                    Rvalue::Cast(CastKind::IntToInt, cop.clone(), dest_ty),
                )),
            ],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::CastKind("PointerCoercion::Unsize".to_string()),
        )
        .is_ok());
        assert!(check(
            &model,
            &InspectionPredicate::CastKind("IntToInt".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::CastKind("PointerCoercion::ReifyFnPointer".to_string()),
            ),
            "CastKind",
            "PointerCoercion::ReifyFnPointer",
        );
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::CastKind("Transmute".to_string()),
            ),
            "CastKind",
            "Transmute",
        );
    }

    #[test]
    fn bin_op_arm_matches_add_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let cop = const_op(&mut model);
        let body = body_with(
            vec![statement(StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
                Rvalue::BinaryOp(BinOp::Add, cop.clone(), cop),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(&model, &InspectionPredicate::BinOp("Add".to_string())).is_ok());
        assert_not_found(
            check(&model, &InspectionPredicate::BinOp("Offset".to_string())),
            "BinOp",
            "Offset",
        );
    }

    #[test]
    fn un_op_arm_matches_neg_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let cop = const_op(&mut model);
        let body = body_with(
            vec![statement(StatementKind::Assign(
                Place::from(LocalId::RETURN_PLACE),
                Rvalue::UnaryOp(UnOp::Neg, cop),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(&model, &InspectionPredicate::UnOp("Neg".to_string())).is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::UnOp("PtrMetadata".to_string()),
            ),
            "UnOp",
            "PtrMetadata",
        );
    }

    #[test]
    fn assert_kind_arm_matches_null_pointer_dereference_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let cop = const_op(&mut model);
        let body = body_with(
            vec![],
            TerminatorKind::Assert {
                cond: cop,
                expected: true,
                msg: AssertKind::NullPointerDereference,
                target: regseq_model::BlockId::from_u32(1),
            },
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::AssertKind("NullPointerDereference".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::AssertKind("BoundsCheck".to_string()),
            ),
            "AssertKind",
            "BoundsCheck",
        );
    }

    #[test]
    fn non_diverging_intrinsic_arm_matches_assume_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let cop = const_op(&mut model);
        let body = body_with(
            vec![statement(StatementKind::Intrinsic(
                NonDivergingIntrinsic::Assume(cop),
            ))],
            TerminatorKind::Return,
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::NonDivergingIntrinsic("Assume".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::NonDivergingIntrinsic("CopyNonOverlapping".to_string()),
            ),
            "NonDivergingIntrinsic",
            "CopyNonOverlapping",
        );
    }

    #[test]
    fn call_callee_arm_matches_direct_call_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        let body = body_with(
            vec![],
            TerminatorKind::Call {
                callee: CallCallee::Direct(InstanceId::ZERO),
                args: Box::new([]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: None,
            },
        );
        push_function_with_flavor(
            &mut model,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(
            &model,
            &InspectionPredicate::CallCallee("Direct".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(
                &model,
                &InspectionPredicate::CallCallee("Indirect".to_string()),
            ),
            "CallCallee",
            "Indirect",
        );
    }

    #[test]
    fn body_absent_arm_matches_function_without_body_and_reports_not_found_otherwise() {
        let mut model_with_absent = empty_model();
        push_function_with_flavor(
            &mut model_with_absent,
            None,
            regseq_model::FunctionInstanceFlavor::Item,
        );

        assert!(check(&model_with_absent, &InspectionPredicate::BodyAbsent).is_ok());

        let mut model_with_body = empty_model();
        let body = body_with(vec![], TerminatorKind::Return);
        push_function_with_flavor(
            &mut model_with_body,
            Some(body),
            regseq_model::FunctionInstanceFlavor::Item,
        );
        assert_not_found(
            check(&model_with_body, &InspectionPredicate::BodyAbsent),
            "BodyAbsent",
            "",
        );
    }

    #[test]
    fn linkage_arm_matches_no_mangle_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        push_item(
            &mut model,
            ItemKind::Function,
            regseq_model::Linkage::NoMangle,
        );

        assert!(check(
            &model,
            &InspectionPredicate::Linkage("NoMangle".to_string()),
        )
        .is_ok());
        assert_not_found(
            check(&model, &InspectionPredicate::Linkage("Weak".to_string())),
            "Linkage",
            "Weak",
        );
    }

    #[test]
    fn scalar_kind_arm_matches_int_kind_inner_and_reports_not_found_otherwise() {
        let mut model = empty_model();
        model.types.intern(Ty::Int(regseq_model::IntKind::I32));

        assert!(check(&model, &InspectionPredicate::ScalarKind("I32".to_string()),).is_ok());
        assert_not_found(
            check(&model, &InspectionPredicate::ScalarKind("F64".to_string())),
            "ScalarKind",
            "F64",
        );
    }
}
