//! Leaf rustc-MIR-to-Regseq-IR mapping helpers for Pass 2.
//!
//! Implemented by task regseq-model-42.
//!
//! This module provides the five pure mapping functions Pass 2 uses to
//! translate live rustc MIR into the Regseq IR ([`map_ty`],
//! [`map_const`], [`map_operand`], [`map_place`], [`map_rvalue`]) plus
//! the supporting newtype wrappers ([`TypeDefIndex`], [`ConstantIndex`],
//! [`LocalIdMap`]) that carry the Pass 1 cross-references into each
//! helper.
//!
//! Every helper input is a live rustc value (`ty::Ty<'tcx>`,
//! `mir::Operand<'tcx>`, ...). This pass maps directly into the
//! model-level Regseq IR.

use std::collections::HashMap;

use rustc_hir::def_id::DefId;
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use regseq_model::identity::{ConstantId, FunctionId, LocalId, SymbolInterner, TypeDefId};
use regseq_model::ir::{
    AggregateKind, BinOp, BorrowKind, CastKind, LiteralValue, Mutability, Operand, Place,
    Projection, RegseqType, Rvalue, ScalarType, UnOp,
};

use super::{ConstructionError, ErrorProvenance};

// ---------------------------------------------------------------------------
// Newtype wrappers
// ---------------------------------------------------------------------------

/// Pass 1 cross-reference index from rustc [`DefId`] to Regseq
/// [`TypeDefId`]. Populated by `walk::populate_module_items_and_submodules`
/// during Pass 1 and exposed as `CrossRefMaps::type_defs`.
#[derive(Copy, Clone)]
pub struct TypeDefIndex<'a>(&'a HashMap<DefId, TypeDefId>);

impl<'a> TypeDefIndex<'a> {
    /// Wrap the Pass 1 `type_defs` map.
    pub fn new(inner: &'a HashMap<DefId, TypeDefId>) -> Self {
        Self(inner)
    }

    /// Resolve a rustc ADT to its assigned [`TypeDefId`], if any.
    pub fn lookup(&self, did: DefId) -> Option<TypeDefId> {
        self.0.get(&did).copied()
    }
}

/// Pass 1 cross-reference index from rustc [`DefId`] to Regseq
/// [`ConstantId`]. Populated by `walk::build_constant` /
/// `walk::build_associated_constant` and exposed as
/// `CrossRefMaps::constants`.
#[derive(Copy, Clone)]
pub struct ConstantIndex<'a>(&'a HashMap<DefId, ConstantId>);

impl<'a> ConstantIndex<'a> {
    /// Wrap the Pass 1 `constants` map.
    pub fn new(inner: &'a HashMap<DefId, ConstantId>) -> Self {
        Self(inner)
    }

    /// Resolve an unevaluated MIR constant's `DefId` to its assigned
    /// [`ConstantId`], if any.
    pub fn lookup(&self, did: DefId) -> Option<ConstantId> {
        self.0.get(&did).copied()
    }
}

/// Pass 1 cross-reference index from rustc [`DefId`] to Regseq
/// [`FunctionId`]. Populated by `walk::populate_module_items_and_submodules`
/// during Pass 1 and exposed as `CrossRefMaps::functions`. Pass 2
/// [`build_body`](super::build_body) consults it to resolve `Call`
/// targets without re-walking the HIR.
#[derive(Copy, Clone)]
pub struct FunctionIndex<'a>(&'a HashMap<DefId, FunctionId>);

impl<'a> FunctionIndex<'a> {
    /// Wrap the Pass 1 `functions` map.
    pub fn new(inner: &'a HashMap<DefId, FunctionId>) -> Self {
        Self(inner)
    }

    /// Resolve a callee's rustc `DefId` to its assigned [`FunctionId`],
    /// if any.
    pub fn lookup(&self, did: DefId) -> Option<FunctionId> {
        self.0.get(&did).copied()
    }
}

/// MIR-local-to-Regseq-LocalId assignment built incrementally during
/// Pass 2 body construction. The map additionally borrows the MIR
/// body's `local_decls` so the place-mapping helper can read each
/// local's rustc type without threading an extra parameter through
/// every call site.
pub struct LocalIdMap<'a, 'tcx> {
    map: HashMap<mir::Local, LocalId>,
    local_decls: &'a mir::LocalDecls<'tcx>,
}

impl<'a, 'tcx> LocalIdMap<'a, 'tcx> {
    /// Build an empty [`LocalIdMap`] anchored on `local_decls`. Pass 2
    /// populates it as each MIR local is assigned a Regseq [`LocalId`].
    pub fn new(local_decls: &'a mir::LocalDecls<'tcx>) -> Self {
        Self {
            map: HashMap::new(),
            local_decls,
        }
    }

    /// Record the [`LocalId`] for a MIR local.
    pub fn insert(&mut self, local: mir::Local, id: LocalId) {
        self.map.insert(local, id);
    }

    /// Look up the [`LocalId`] assigned to a MIR local.
    pub fn lookup(&self, local: mir::Local) -> Option<LocalId> {
        self.map.get(&local).copied()
    }

    /// Return the rustc type of a MIR local (from the borrowed
    /// `local_decls` table).
    pub fn local_ty(&self, local: mir::Local) -> ty::Ty<'tcx> {
        self.local_decls[local].ty
    }

    /// Borrow the underlying [`mir::LocalDecls`] so Pass 2 can delegate
    /// operand-type queries through `mir::Operand::ty(local_decls, tcx)`
    /// without re-plumbing the body reference.
    pub fn local_decls(&self) -> &'a mir::LocalDecls<'tcx> {
        self.local_decls
    }
}

/// Outcome of mapping a single MIR [`mir::Operand`]. Most operands map
/// to an owned Regseq [`Operand`]; a handful (notably any future
/// runtime-checks operand shape rustc may introduce) are dropped
/// entirely by the caller.
pub enum MapOperandResult {
    /// Successful mapping to a Regseq IR [`Operand`].
    Mapped(Operand),
    /// Caller drops this operand entirely.
    Drop,
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

/// Map a rustc [`ty::Ty`] to a Regseq [`RegseqType`].
///
/// Covers every [`RegseqType`] variant via `ty.kind()`. Returns
/// [`ConstructionError::StdTypeNotPermitted`] for any ADT whose crate
/// is `std`, and [`ConstructionError::UnsupportedRustType`] for type
/// kinds outside the driver-spec grammar (closures, coroutines, raw
/// pointers, function pointers, trait objects, non-unit tuples).
pub fn map_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    type_def_index: &TypeDefIndex<'_>,
) -> Result<RegseqType, ConstructionError> {
    match ty.kind() {
        ty::TyKind::Bool => Ok(RegseqType::Bool),
        ty::TyKind::Tuple(elems) if elems.is_empty() => Ok(RegseqType::Unit),
        ty::TyKind::Never => Ok(RegseqType::Never),
        ty::TyKind::Uint(u) => uint_to_scalar(*u).map(RegseqType::Scalar).ok_or_else(|| {
            ConstructionError::UnsupportedRustType {
                provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                rendered: format!("{ty}"),
            }
        }),
        ty::TyKind::Int(i) => int_to_scalar(*i).map(RegseqType::Scalar).ok_or_else(|| {
            ConstructionError::UnsupportedRustType {
                provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                rendered: format!("{ty}"),
            }
        }),
        ty::TyKind::Array(inner, len_const) => {
            let inner_mapped = map_ty(tcx, *inner, type_def_index)?;
            let len = len_const.try_to_target_usize(tcx).ok_or_else(|| {
                ConstructionError::UnsupportedRustType {
                    provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                    rendered: format!("array-length `{len_const:?}` on `{ty}`"),
                }
            })?;
            Ok(RegseqType::Array(Box::new(inner_mapped), len as usize))
        }
        ty::TyKind::Ref(_, inner, mutbl) => {
            if let ty::TyKind::Slice(slice_inner) = inner.kind() {
                let mapped = map_ty(tcx, *slice_inner, type_def_index)?;
                Ok(RegseqType::Slice(Box::new(mapped)))
            } else {
                let mapped = map_ty(tcx, *inner, type_def_index)?;
                Ok(RegseqType::Ref(Box::new(mapped), mutability(*mutbl)))
            }
        }
        ty::TyKind::Adt(adt_def, args) => {
            let did = adt_def.did();
            if is_option_adt(tcx, did) {
                let Some(t) = args.types().next() else {
                    return Err(ConstructionError::UnsupportedRustType {
                        provenance: ErrorProvenance::for_def_id(tcx, did),
                        rendered: format!("Option without type argument: `{ty}`"),
                    });
                };
                let mapped = map_ty(tcx, t, type_def_index)?;
                return Ok(RegseqType::Option(Box::new(mapped)));
            }
            if is_result_adt(tcx, did) {
                let mut it = args.types();
                let (Some(t), Some(e)) = (it.next(), it.next()) else {
                    return Err(ConstructionError::UnsupportedRustType {
                        provenance: ErrorProvenance::for_def_id(tcx, did),
                        rendered: format!("Result without both type arguments: `{ty}`"),
                    });
                };
                let mapped_t = map_ty(tcx, t, type_def_index)?;
                let mapped_e = map_ty(tcx, e, type_def_index)?;
                return Ok(RegseqType::Result(Box::new(mapped_t), Box::new(mapped_e)));
            }
            // `def_path_str` canonicalizes item paths to their public
            // re-export location (e.g. `alloc::string::String` renders
            // as `std::string::String` when compiled in a std-aware
            // build). Check that prefix rather than the raw crate name
            // so drivers can't sneak in std types through re-exports.
            let path_str = tcx.def_path_str(did);
            if path_str.starts_with("std::") {
                return Err(ConstructionError::StdTypeNotPermitted {
                    provenance: ErrorProvenance::for_def_id(tcx, did),
                    path: path_str,
                });
            }
            match type_def_index.lookup(did) {
                Some(tid) => Ok(RegseqType::Defined(tid)),
                None => Err(ConstructionError::UnsupportedRustType {
                    provenance: ErrorProvenance::for_def_id(tcx, did),
                    rendered: format!("ADT `{}` missing from TypeDefIndex", tcx.def_path_str(did)),
                }),
            }
        }
        _ => Err(ConstructionError::UnsupportedRustType {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: format!("{ty}"),
        }),
    }
}

fn uint_to_scalar(u: ty::UintTy) -> Option<ScalarType> {
    Some(match u {
        ty::UintTy::U8 => ScalarType::U8,
        ty::UintTy::U16 => ScalarType::U16,
        ty::UintTy::U32 => ScalarType::U32,
        ty::UintTy::U64 => ScalarType::U64,
        ty::UintTy::Usize => ScalarType::Usize,
        // 128-bit integers have no representation in the Regseq scalar
        // space; reject rather than silently narrow.
        ty::UintTy::U128 => return None,
    })
}

fn int_to_scalar(i: ty::IntTy) -> Option<ScalarType> {
    Some(match i {
        ty::IntTy::I8 => ScalarType::I8,
        ty::IntTy::I16 => ScalarType::I16,
        ty::IntTy::I32 => ScalarType::I32,
        ty::IntTy::I64 => ScalarType::I64,
        ty::IntTy::Isize => ScalarType::Isize,
        ty::IntTy::I128 => return None,
    })
}

fn mutability(m: rustc_ast::Mutability) -> Mutability {
    match m {
        rustc_ast::Mutability::Not => Mutability::Shared,
        rustc_ast::Mutability::Mut => Mutability::Mutable,
    }
}

fn is_option_adt(tcx: TyCtxt<'_>, did: DefId) -> bool {
    tcx.get_diagnostic_name(did) == Some(rustc_span::sym::Option)
}

fn is_result_adt(tcx: TyCtxt<'_>, did: DefId) -> bool {
    tcx.get_diagnostic_name(did) == Some(rustc_span::sym::Result)
}

// ---------------------------------------------------------------------------
// Const mapping
// ---------------------------------------------------------------------------

/// Map a rustc [`ty::Const`] to a Regseq [`LiteralValue`], dispatching
/// by `expected_ty`. Returns
/// [`ConstructionError::UnsupportedConstShape`] if the constant cannot
/// be evaluated to a scalar of a width compatible with `expected_ty`.
pub fn map_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    c: ty::Const<'tcx>,
    expected_ty: ty::Ty<'tcx>,
) -> Result<LiteralValue, ConstructionError> {
    let typing_env = ty::TypingEnv::fully_monomorphized();
    let scalar_int = resolve_ty_const_to_scalar_int(tcx, c, typing_env).ok_or_else(|| {
        ConstructionError::UnsupportedConstShape {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: format!("non-scalar ty::Const `{c:?}`"),
        }
    })?;
    let bits: u128 = scalar_int.to_bits(scalar_int.size());
    scalar_to_literal(expected_ty, bits).ok_or_else(|| ConstructionError::UnsupportedConstShape {
        provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
        rendered: format!(
            "scalar const of type `{expected_ty}` (kind `{:?}`)",
            expected_ty.kind()
        ),
    })
}

/// Reduce a `ty::Const` to a `ScalarInt` leaf, driving the rustc
/// const-eval query system when the input is an `Unevaluated`
/// [`ty::ConstKind`] rather than a pre-evaluated value.
fn resolve_ty_const_to_scalar_int<'tcx>(
    tcx: TyCtxt<'tcx>,
    c: ty::Const<'tcx>,
    typing_env: ty::TypingEnv<'tcx>,
) -> Option<ty::ScalarInt> {
    // Fast path: the const is already a concrete valtree leaf.
    if let Some(si) = c.try_to_leaf() {
        return Some(si);
    }
    // Slow path: evaluate an unevaluated generic or anon constant
    // through the const-eval query system.
    if let ty::ConstKind::Unevaluated(uv) = c.kind() {
        let valtree = tcx
            .const_eval_resolve_for_typeck(typing_env, uv, rustc_span::DUMMY_SP)
            .ok()?
            .ok()?;
        return valtree.try_to_leaf();
    }
    None
}

fn scalar_to_literal(ty: ty::Ty<'_>, bits: u128) -> Option<LiteralValue> {
    match ty.kind() {
        ty::TyKind::Uint(u) => Some(match u {
            ty::UintTy::U8 => LiteralValue::U8(bits as u8),
            ty::UintTy::U16 => LiteralValue::U16(bits as u16),
            ty::UintTy::U32 => LiteralValue::U32(bits as u32),
            ty::UintTy::U64 => LiteralValue::U64(bits as u64),
            ty::UintTy::Usize => LiteralValue::Usize(bits as u64),
            // 128-bit integers have no Regseq `LiteralValue` variant;
            // reject instead of narrowing.
            ty::UintTy::U128 => return None,
        }),
        ty::TyKind::Int(i) => Some(match i {
            ty::IntTy::I8 => LiteralValue::I8(bits as i8),
            ty::IntTy::I16 => LiteralValue::I16(bits as i16),
            ty::IntTy::I32 => LiteralValue::I32(bits as i32),
            ty::IntTy::I64 => LiteralValue::I64(bits as i64),
            ty::IntTy::Isize => LiteralValue::Isize(bits as i64),
            ty::IntTy::I128 => return None,
        }),
        ty::TyKind::Bool => Some(LiteralValue::Bool(bits != 0)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Operand mapping
// ---------------------------------------------------------------------------

/// Map a rustc [`mir::Operand`] to a Regseq [`Operand`], deferring
/// place translation to [`map_place`] and scalar-constant evaluation
/// to [`map_const`]. Returns [`MapOperandResult::Drop`] for operand
/// shapes that the caller drops entirely (e.g., a future runtime-checks
/// operand).
pub fn map_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: &mir::Operand<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    constants: &ConstantIndex<'_>,
    type_def_index: &TypeDefIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<MapOperandResult, ConstructionError> {
    match op {
        mir::Operand::Copy(place) => {
            let p = map_place(tcx, place, locals, type_def_index, interner)?;
            Ok(MapOperandResult::Mapped(Operand::Copy(p)))
        }
        mir::Operand::Move(place) => {
            let p = map_place(tcx, place, locals, type_def_index, interner)?;
            Ok(MapOperandResult::Mapped(Operand::Move(p)))
        }
        mir::Operand::Constant(const_op) => {
            let span = const_op.span;
            match const_op.const_ {
                mir::Const::Unevaluated(uneval, _eval_ty) => match constants.lookup(uneval.def) {
                    Some(cid) => Ok(MapOperandResult::Mapped(Operand::Constant(cid))),
                    None => Err(ConstructionError::UnsupportedConstShape {
                        provenance: ErrorProvenance::for_span(span),
                        rendered: format!(
                            "unevaluated constant `{}` missing from ConstantIndex",
                            tcx.def_path_str(uneval.def)
                        ),
                    }),
                },
                mir::Const::Val(val, ty) => match val {
                    mir::ConstValue::Scalar(mir::interpret::Scalar::Int(int)) => {
                        let bits = int.to_bits(int.size());
                        match scalar_to_literal(ty, bits) {
                            Some(lit) => Ok(MapOperandResult::Mapped(Operand::Literal(lit))),
                            None => Err(ConstructionError::UnsupportedConstShape {
                                provenance: ErrorProvenance::for_span(span),
                                rendered: format!(
                                    "scalar const of type `{ty}` (kind `{:?}`)",
                                    ty.kind()
                                ),
                            }),
                        }
                    }
                    other => Err(ConstructionError::UnsupportedConstShape {
                        provenance: ErrorProvenance::for_span(span),
                        rendered: format!("{other:?}"),
                    }),
                },
                mir::Const::Ty(ty_of, ty_const) => {
                    let lit = map_const(tcx, ty_const, ty_of)?;
                    Ok(MapOperandResult::Mapped(Operand::Literal(lit)))
                }
            }
        }
        mir::Operand::RuntimeChecks(_) => Ok(MapOperandResult::Drop),
    }
}

// ---------------------------------------------------------------------------
// Place mapping
// ---------------------------------------------------------------------------

/// Map a rustc [`mir::Place`] to a Regseq [`Place`]. Resolves field
/// names against the parent place's rustc type (tracked incrementally
/// across each projection) and interns them. Returns
/// [`ConstructionError::UnsupportedProjection`] for MIR projections
/// that do not appear in the Regseq IR (`OpaqueCast`,
/// `UnwrapUnsafeBinder`).
pub fn map_place<'tcx>(
    tcx: TyCtxt<'tcx>,
    place: &mir::Place<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<Place, ConstructionError> {
    let local =
        locals
            .lookup(place.local)
            .ok_or_else(|| ConstructionError::UnsupportedProjection {
                provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                rendered: format!(
                    "MIR local _{} not assigned a Regseq LocalId",
                    place.local.as_u32()
                ),
            })?;

    // Walk projections; `parent_ty` is the rustc type of the place's
    // current prefix (before the next projection is applied). Seeded
    // from the local's decl and advanced by `project_ty` for each step.
    // `pending_variant` tracks the most recent `Downcast` so the next
    // `Field` can resolve its name from the correct enum variant.
    let mut parent_ty = locals.local_ty(place.local);
    let mut pending_variant: Option<rustc_abi::VariantIdx> = None;
    let mut projection = Vec::with_capacity(place.projection.len());
    for elem in place.projection.iter() {
        let mapped = match elem {
            mir::ProjectionElem::Deref => Projection::Deref,
            mir::ProjectionElem::Field(idx, field_ty) => {
                let index = idx.as_u32();
                let name = field_name(tcx, parent_ty, pending_variant, index, interner);
                let ty = map_ty(tcx, field_ty, type_def_index)?;
                Projection::Field { index, name, ty }
            }
            mir::ProjectionElem::Index(mir_local) => {
                let id = locals.lookup(mir_local).ok_or_else(|| {
                    ConstructionError::UnsupportedProjection {
                        provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                        rendered: format!(
                            "MIR index-local _{} not assigned a Regseq LocalId",
                            mir_local.as_u32()
                        ),
                    }
                })?;
                Projection::Index(id)
            }
            mir::ProjectionElem::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => Projection::ConstantIndex {
                offset,
                min_length,
                from_end,
            },
            mir::ProjectionElem::Subslice { from, to, from_end } => {
                Projection::Subslice { from, to, from_end }
            }
            mir::ProjectionElem::Downcast(name_opt, variant_idx) => {
                let resolved_name = downcast_name(tcx, parent_ty, variant_idx, name_opt, interner);
                Projection::Downcast {
                    name: resolved_name,
                    variant_index: variant_idx.as_u32(),
                }
            }
            mir::ProjectionElem::OpaqueCast(_) => {
                return Err(ConstructionError::UnsupportedProjection {
                    provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                    rendered: "OpaqueCast".to_owned(),
                });
            }
            mir::ProjectionElem::UnwrapUnsafeBinder(_) => {
                return Err(ConstructionError::UnsupportedProjection {
                    provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                    rendered: "UnwrapUnsafeBinder".to_owned(),
                });
            }
        };
        // Update the variant latch after emitting the projection: a
        // `Downcast` sets the variant; any subsequent projection
        // (`Field`, `Deref`, `Index`, ...) invalidates it, since all
        // other projections change the place type away from the
        // downcasted enum variant before another field access could
        // land on it.
        pending_variant = match &elem {
            mir::ProjectionElem::Downcast(_, v) => Some(*v),
            _ => None,
        };
        projection.push(mapped);
        parent_ty = project_ty(tcx, parent_ty, &elem);
    }

    Ok(Place { local, projection })
}

/// Advance `parent_ty` by the effect of applying `elem`. Rustc has the
/// same utility on [`mir::tcx::PlaceTy`]; this helper mirrors its
/// projection semantics for the projections the Regseq IR keeps.
fn project_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    parent: ty::Ty<'tcx>,
    elem: &mir::PlaceElem<'tcx>,
) -> ty::Ty<'tcx> {
    match elem {
        mir::ProjectionElem::Deref => parent.builtin_deref(true).unwrap_or(parent),
        mir::ProjectionElem::Field(_, ty) => *ty,
        mir::ProjectionElem::Index(_) | mir::ProjectionElem::ConstantIndex { .. } => {
            parent.builtin_index().unwrap_or(parent)
        }
        mir::ProjectionElem::Subslice { .. } => parent,
        mir::ProjectionElem::Downcast(_, _) => parent,
        mir::ProjectionElem::OpaqueCast(ty) | mir::ProjectionElem::UnwrapUnsafeBinder(ty) => {
            let _ = tcx;
            *ty
        }
    }
}

fn field_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    parent: ty::Ty<'tcx>,
    downcast_variant: Option<rustc_abi::VariantIdx>,
    index: u32,
    interner: &mut SymbolInterner,
) -> Option<regseq_model::identity::Symbol> {
    let _ = tcx;
    let ty::TyKind::Adt(adt, _) = parent.kind() else {
        return None;
    };
    // Enum field access must be preceded by `Downcast(variant)` in
    // MIR; we look up the field in that variant's table. Non-enum
    // ADTs (structs, unions) use the sole `non_enum_variant()`.
    let variant = if adt.is_enum() {
        let v = downcast_variant?;
        if v.as_usize() >= adt.variants().len() {
            return None;
        }
        adt.variant(v)
    } else {
        adt.non_enum_variant()
    };
    let field_def = variant.fields.iter().nth(index as usize)?;
    // rustc `Symbol` is an interned identifier; tuple-struct fields
    // carry names `0`, `1`, ... in the `FieldDef` table. The Regseq IR
    // wants `None` for positional fields so backends emit `.0` /
    // `field_0` instead of `.0_name`.
    let name_str = field_def.name.as_str().to_owned();
    if name_str.parse::<u32>().is_ok() {
        None
    } else {
        Some(interner.intern(&name_str))
    }
}

fn downcast_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    parent: ty::Ty<'tcx>,
    variant_idx: rustc_abi::VariantIdx,
    rustc_name: Option<rustc_span::Symbol>,
    interner: &mut SymbolInterner,
) -> Option<regseq_model::identity::Symbol> {
    // Prefer the AdtDef lookup so the name is always canonical even when
    // MIR dropped the symbol (older rustc builds sometimes emit
    // `Downcast(None, idx)`).
    if let ty::TyKind::Adt(adt, _) = parent.kind() {
        if variant_idx.as_usize() < adt.variants().len() {
            let name = adt.variant(variant_idx).name.as_str().to_owned();
            return Some(interner.intern(&name));
        }
    }
    let _ = tcx;
    rustc_name.map(|n| interner.intern(n.as_str()))
}

// ---------------------------------------------------------------------------
// Rvalue mapping
// ---------------------------------------------------------------------------

/// Map a rustc [`mir::Rvalue`] to a Regseq [`Rvalue`]. Simplifies
/// MIR's `BorrowKind`, `CastKind`, and `AggregateKind` to the Regseq
/// IR's reduced surface. Returns
/// [`ConstructionError::UnsupportedRvalue`] for MIR rvalue kinds the
/// Regseq IR does not model (`ThreadLocalRef`, `RawPtr`,
/// `ShallowInitBox`, `WrapUnsafeBinder`, and any `Aggregate(kind)`
/// whose `kind` falls outside Array/Tuple/Adt).
pub fn map_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    rv: &mir::Rvalue<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    type_def_index: &TypeDefIndex<'_>,
    constants: &ConstantIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<Rvalue, ConstructionError> {
    match rv {
        mir::Rvalue::Use(op) => {
            // MIR emits `_0 = const ()` at function exit for `()`-returning
            // bodies, with the operand being a `ZeroSized` constant of
            // type `()`. The Regseq `LiteralValue` enum does not model
            // `()` directly, so map the use to the equivalent empty
            // tuple aggregate; this preserves the unit-assignment
            // semantics without requiring a new literal variant.
            if is_unit_constant(op) {
                return Ok(Rvalue::Aggregate(AggregateKind::Tuple, Vec::new()));
            }
            let mapped =
                map_operand_required(tcx, op, locals, constants, type_def_index, interner)?;
            Ok(Rvalue::Use(mapped))
        }
        mir::Rvalue::Repeat(op, ct) => {
            let mapped =
                map_operand_required(tcx, op, locals, constants, type_def_index, interner)?;
            let count = ct.try_to_target_usize(tcx).ok_or_else(|| {
                ConstructionError::UnsupportedConstShape {
                    provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                    rendered: format!("repeat-count const `{ct:?}`"),
                }
            })?;
            Ok(Rvalue::Repeat(mapped, count as usize))
        }
        mir::Rvalue::Ref(_region, borrow_kind, place) => {
            let bk = reduce_borrow_kind(borrow_kind);
            let mapped_place = map_place(tcx, place, locals, type_def_index, interner)?;
            Ok(Rvalue::Ref(bk, mapped_place))
        }
        mir::Rvalue::Cast(cast_kind, operand, target_ty) => {
            let ck = reduce_cast_kind(cast_kind).ok_or_else(|| {
                ConstructionError::UnsupportedRvalue {
                    provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                    rendered: format!("Cast({cast_kind:?})"),
                }
            })?;
            let mapped_op =
                map_operand_required(tcx, operand, locals, constants, type_def_index, interner)?;
            let mapped_ty = map_ty(tcx, *target_ty, type_def_index)?;
            Ok(Rvalue::Cast(ck, mapped_op, mapped_ty))
        }
        mir::Rvalue::BinaryOp(op, operands) => {
            let lhs = map_operand_required(
                tcx,
                &operands.0,
                locals,
                constants,
                type_def_index,
                interner,
            )?;
            let rhs = map_operand_required(
                tcx,
                &operands.1,
                locals,
                constants,
                type_def_index,
                interner,
            )?;
            Ok(Rvalue::BinaryOp(reduce_bin_op(op), lhs, rhs))
        }
        mir::Rvalue::UnaryOp(op, operand) => {
            let uo = reduce_un_op(op).ok_or_else(|| ConstructionError::UnsupportedRvalue {
                provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
                rendered: format!("UnaryOp({op:?})"),
            })?;
            let mapped =
                map_operand_required(tcx, operand, locals, constants, type_def_index, interner)?;
            Ok(Rvalue::UnaryOp(uo, mapped))
        }
        mir::Rvalue::Discriminant(place) => {
            let mapped = map_place(tcx, place, locals, type_def_index, interner)?;
            Ok(Rvalue::Discriminant(mapped))
        }
        mir::Rvalue::Aggregate(kind, ops) => {
            let agg = reduce_aggregate_kind(tcx, kind, type_def_index)?;
            let mut mapped_ops = Vec::with_capacity(ops.len());
            for op in ops {
                mapped_ops.push(map_operand_required(
                    tcx,
                    op,
                    locals,
                    constants,
                    type_def_index,
                    interner,
                )?);
            }
            Ok(Rvalue::Aggregate(agg, mapped_ops))
        }
        mir::Rvalue::CopyForDeref(place) => {
            let mapped = map_place(tcx, place, locals, type_def_index, interner)?;
            Ok(Rvalue::Use(Operand::Copy(mapped)))
        }
        mir::Rvalue::ThreadLocalRef(_)
        | mir::Rvalue::RawPtr(_, _)
        | mir::Rvalue::ShallowInitBox(_, _)
        | mir::Rvalue::WrapUnsafeBinder(_, _) => Err(ConstructionError::UnsupportedRvalue {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: format!("{rv:?}"),
        }),
    }
}

/// True when `op` is a MIR `ConstValue::ZeroSized` of type `()`.
fn is_unit_constant<'tcx>(op: &mir::Operand<'tcx>) -> bool {
    let mir::Operand::Constant(const_op) = op else {
        return false;
    };
    let mir::Const::Val(mir::ConstValue::ZeroSized, ty) = const_op.const_ else {
        return false;
    };
    matches!(ty.kind(), ty::TyKind::Tuple(elems) if elems.is_empty())
}

fn map_operand_required<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: &mir::Operand<'tcx>,
    locals: &LocalIdMap<'_, 'tcx>,
    constants: &ConstantIndex<'_>,
    type_def_index: &TypeDefIndex<'_>,
    interner: &mut SymbolInterner,
) -> Result<Operand, ConstructionError> {
    match map_operand(tcx, op, locals, constants, type_def_index, interner)? {
        MapOperandResult::Mapped(o) => Ok(o),
        MapOperandResult::Drop => Err(ConstructionError::UnsupportedRvalue {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: "dropped operand in required position".to_owned(),
        }),
    }
}

fn reduce_borrow_kind(k: &mir::BorrowKind) -> BorrowKind {
    match k {
        mir::BorrowKind::Shared => BorrowKind::Shared,
        mir::BorrowKind::Mut { .. } => BorrowKind::Mutable,
        mir::BorrowKind::Fake(_) => BorrowKind::Shared,
    }
}

fn reduce_cast_kind(k: &mir::CastKind) -> Option<CastKind> {
    Some(match k {
        mir::CastKind::IntToInt => CastKind::IntToInt,
        mir::CastKind::IntToFloat => CastKind::IntToFloat,
        mir::CastKind::FloatToInt => CastKind::FloatToInt,
        mir::CastKind::FloatToFloat => CastKind::FloatToFloat,
        mir::CastKind::PtrToPtr => CastKind::PtrToPtr,
        mir::CastKind::Transmute => CastKind::Transmute,
        mir::CastKind::PointerCoercion(_, _) => CastKind::PtrToPtr,
        mir::CastKind::PointerExposeProvenance
        | mir::CastKind::PointerWithExposedProvenance
        | mir::CastKind::Subtype
        | mir::CastKind::FnPtrToPtr => return None,
    })
}

fn reduce_bin_op(op: &mir::BinOp) -> BinOp {
    match op {
        mir::BinOp::Add => BinOp::Add,
        mir::BinOp::Sub => BinOp::Sub,
        mir::BinOp::Mul => BinOp::Mul,
        mir::BinOp::Div => BinOp::Div,
        mir::BinOp::Rem => BinOp::Rem,
        mir::BinOp::AddUnchecked => BinOp::AddUnchecked,
        mir::BinOp::SubUnchecked => BinOp::SubUnchecked,
        mir::BinOp::MulUnchecked => BinOp::MulUnchecked,
        mir::BinOp::AddWithOverflow => BinOp::AddWithOverflow,
        mir::BinOp::SubWithOverflow => BinOp::SubWithOverflow,
        mir::BinOp::MulWithOverflow => BinOp::MulWithOverflow,
        mir::BinOp::BitXor => BinOp::BitXor,
        mir::BinOp::BitAnd => BinOp::BitAnd,
        mir::BinOp::BitOr => BinOp::BitOr,
        mir::BinOp::Shl => BinOp::Shl,
        mir::BinOp::Shr => BinOp::Shr,
        mir::BinOp::ShlUnchecked => BinOp::ShlUnchecked,
        mir::BinOp::ShrUnchecked => BinOp::ShrUnchecked,
        mir::BinOp::Eq => BinOp::Eq,
        mir::BinOp::Ne => BinOp::Ne,
        mir::BinOp::Lt => BinOp::Lt,
        mir::BinOp::Le => BinOp::Le,
        mir::BinOp::Gt => BinOp::Gt,
        mir::BinOp::Ge => BinOp::Ge,
        mir::BinOp::Cmp => BinOp::Cmp,
        mir::BinOp::Offset => BinOp::Offset,
    }
}

fn reduce_un_op(op: &mir::UnOp) -> Option<UnOp> {
    Some(match op {
        mir::UnOp::Not => UnOp::Not,
        mir::UnOp::Neg => UnOp::Neg,
        mir::UnOp::PtrMetadata => return None,
    })
}

fn reduce_aggregate_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &mir::AggregateKind<'tcx>,
    type_def_index: &TypeDefIndex<'_>,
) -> Result<AggregateKind, ConstructionError> {
    match kind {
        mir::AggregateKind::Array(ty) => {
            let mapped = map_ty(tcx, *ty, type_def_index)?;
            Ok(AggregateKind::Array(mapped))
        }
        mir::AggregateKind::Tuple => Ok(AggregateKind::Tuple),
        mir::AggregateKind::Adt(def_id, variant_idx, _args, _user_ty, _active) => {
            let tid = type_def_index.lookup(*def_id).ok_or_else(|| {
                ConstructionError::UnsupportedRustType {
                    provenance: ErrorProvenance::for_def_id(tcx, *def_id),
                    rendered: format!(
                        "ADT `{}` missing from TypeDefIndex",
                        tcx.def_path_str(*def_id)
                    ),
                }
            })?;
            Ok(AggregateKind::Adt {
                type_id: tid,
                variant_index: variant_idx.as_u32(),
            })
        }
        mir::AggregateKind::Closure(_, _)
        | mir::AggregateKind::Coroutine(_, _)
        | mir::AggregateKind::CoroutineClosure(_, _)
        | mir::AggregateKind::RawPtr(_, _) => Err(ConstructionError::UnsupportedRvalue {
            provenance: ErrorProvenance::for_span(rustc_span::DUMMY_SP),
            rendered: format!("Aggregate({kind:?})"),
        }),
    }
}
