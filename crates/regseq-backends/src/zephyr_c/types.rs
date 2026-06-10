//! Zephyr-C type lowering for the zephyr_c backend.
//!
//! Zephyr-native spelling (`<stdint.h>`, the vendored-driver idiom):
//! `uint8_t..uint64_t`, `int8_t..int64_t`, `bool`. Nominal aggregates become
//! `struct <name>`. Everything outside the current lowered subset is a loud
//! [`EmitError::Unsupported`] that shrinks as the T06–T08 banks land.
//! Independent of `generic_c`/`linux_c` type lowering (F-1: no shared core).

use std::collections::BTreeMap;

use regseq_model::{IntKind, RegseqModel, Ty, TyId, UintKind};

use super::naming::{c_ident, path_leaf};
use super::EmitError;

/// Resolved C names for the nominal aggregates a TU emits, keyed by `TyId`.
pub(crate) struct TypeNames {
    names: BTreeMap<TyId, String>,
}

impl TypeNames {
    pub(crate) fn new() -> Self {
        Self {
            names: BTreeMap::new(),
        }
    }

    /// The C tag name for an aggregate type, deriving and caching it on
    /// first use: `<leaf-of-def-path>` sanitized, with a deterministic
    /// `_<n>` suffix on clashes between distinct types.
    pub(crate) fn name_of(&mut self, model: &RegseqModel, ty: TyId) -> Result<String, EmitError> {
        if let Some(name) = self.names.get(&ty) {
            return Ok(name.clone());
        }
        let base = match &model.types[ty] {
            Ty::Struct { item, .. } | Ty::Enum { item, .. } | Ty::Union { item, .. } => c_ident(
                path_leaf(model.strings.resolve(model.items[*item].def_path.rendered)),
            ),
            other => {
                return Err(EmitError::Unsupported(format!(
                    "C name for non-nominal type {other:?}"
                )))
            }
        };
        let mut name = base.clone();
        let mut n = 1;
        while self.names.values().any(|v| v == &name) {
            n += 1;
            name = format!("{base}_{n}");
        }
        self.names.insert(ty, name.clone());
        Ok(name)
    }
}

pub(crate) fn int_spelling(kind: IntKind) -> Result<&'static str, EmitError> {
    Ok(match kind {
        IntKind::I8 => "int8_t",
        IntKind::I16 => "int16_t",
        IntKind::I32 => "int32_t",
        IntKind::I64 => "int64_t",
        _ => {
            return Err(EmitError::Unsupported(format!(
                "integer kind {kind:?} (first-light scalar subset)"
            )))
        }
    })
}

pub(crate) fn uint_spelling(kind: UintKind) -> Result<&'static str, EmitError> {
    Ok(match kind {
        UintKind::U8 => "uint8_t",
        UintKind::U16 => "uint16_t",
        UintKind::U32 => "uint32_t",
        UintKind::U64 => "uint64_t",
        UintKind::Usize => "size_t",
        _ => {
            return Err(EmitError::Unsupported(format!(
                "unsigned kind {kind:?} (first-light scalar subset)"
            )))
        }
    })
}

/// Whether `ty` lowers to nothing (a ZST: unit, fieldless-zero-size
/// aggregates, FnDef).
pub(crate) fn is_zst(model: &RegseqModel, ty: TyId) -> bool {
    match &model.types[ty] {
        Ty::FnDef(_, _) => true,
        Ty::Tuple { layout, .. }
        | Ty::Struct { layout, .. }
        | Ty::Enum { layout, .. }
        | Ty::Union { layout, .. } => layout.size == 0,
        _ => false,
    }
}

/// The enum tag facts the body emitter and the errno adapter need:
/// `(tag_offset, tag_size_bytes)`.
pub(crate) fn enum_tag_cell(model: &RegseqModel, ty: TyId) -> Result<(u64, u64), EmitError> {
    let layout = layout_of(model, ty)
        .ok_or_else(|| EmitError::Unsupported("enum tag of a non-aggregate".into()))?;
    let regseq_model::LayoutKind::Enum { shape } = &layout.kind else {
        return Err(EmitError::Unsupported(
            "enum tag of a non-enum layout".into(),
        ));
    };
    match shape {
        regseq_model::EnumShape::Multiple { tag, .. } => {
            let size = match &model.types[tag.scalar_ty] {
                Ty::Bool => 1,
                Ty::Int(IntKind::I8) | Ty::Uint(UintKind::U8) => 1,
                Ty::Int(IntKind::I16) | Ty::Uint(UintKind::U16) => 2,
                Ty::Int(IntKind::I32) | Ty::Uint(UintKind::U32) => 4,
                Ty::Int(IntKind::I64) | Ty::Uint(UintKind::U64) => 8,
                other => {
                    return Err(EmitError::Unsupported(format!(
                        "enum tag scalar {other:?} (first-light subset)"
                    )))
                }
            };
            Ok((tag.offset, size))
        }
        _ => Err(EmitError::Unsupported(
            "single-variant enum tag (first-light subset)".into(),
        )),
    }
}

/// The embedded `Layout` of an aggregate type (`None` for scalars).
pub(crate) fn layout_of(model: &RegseqModel, ty: TyId) -> Option<&regseq_model::Layout> {
    match &model.types[ty] {
        Ty::Tuple { layout, .. }
        | Ty::Struct { layout, .. }
        | Ty::Enum { layout, .. }
        | Ty::Union { layout, .. } => Some(layout),
        _ => None,
    }
}

/// The bare C type spelling for `ty` (no declarator). Grows with the
/// T06–T08 lowering banks; every unhandled shape is a loud error.
pub(crate) fn c_type(
    model: &RegseqModel,
    names: &mut TypeNames,
    ty: TyId,
) -> Result<String, EmitError> {
    Ok(match &model.types[ty] {
        Ty::Bool => "bool".into(),
        Ty::Int(k) => int_spelling(*k)?.into(),
        Ty::Uint(k) => uint_spelling(*k)?.into(),
        Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => {
            if !is_sized_supported(model, *pointee) {
                return Err(EmitError::Unsupported(format!(
                    "pointer to unsized/unsupported pointee {:?} (first-light subset)",
                    model.types[*pointee]
                )));
            }
            format!("{} *", c_type(model, names, *pointee)?)
        }
        Ty::Struct { .. } | Ty::Enum { .. } | Ty::Union { .. } | Ty::Tuple { .. } => {
            format!("struct {}", names.name_of(model, ty)?)
        }
        other => {
            return Err(EmitError::Unsupported(format!(
                "C spelling for type {other:?} (first-light subset)"
            )))
        }
    })
}

/// Whether `ty` is a sized type the subset can point at.
fn is_sized_supported(model: &RegseqModel, ty: TyId) -> bool {
    !matches!(
        &model.types[ty],
        Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. } | Ty::Never
    )
}
