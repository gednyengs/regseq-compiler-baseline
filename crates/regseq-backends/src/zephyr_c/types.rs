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
            Ty::Tuple { fields, .. } => {
                let mut parts = vec!["rseq_tuple".to_string()];
                for f in fields {
                    parts.push(c_ident(scalar_spelling_or_err(model, *f)?));
                }
                parts.join("_")
            }
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

    /// Every named aggregate, in BTreeMap (TyId-keyed) order — deterministic
    /// per model.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&TyId, &String)> {
        self.names.iter()
    }
}

/// Emit the C definition for one named aggregate. Structs get real members
/// (resource fields substituted by the caller via `field_override`); enums
/// become opaque layout-faithful byte carriers (`uint8_t _raw[N]`) whose
/// tag/payload bytes the body emitter writes per the model's resolved
/// `Layout` — the byte image, not a C view structure, is the contract (the
/// carrier crosses the scaffold boundary only through the errno adapter's
/// discriminant read).
pub(crate) fn emit_aggregate_def(
    model: &RegseqModel,
    names: &mut TypeNames,
    ty: TyId,
    field_override: &dyn Fn(&str) -> Option<String>,
) -> Result<String, EmitError> {
    let name = names.name_of(model, ty)?;
    let layout = layout_of(model, ty)
        .ok_or_else(|| EmitError::Unsupported("definition of a non-aggregate".into()))?;
    match (&model.types[ty], &layout.kind) {
        (Ty::Enum { .. }, _) => Ok(format!(
            "/* {} - layout-faithful byte carrier (size {}, align {}) */\nstruct {} {{\n\tuint8_t _raw[{}];\n}} __attribute__((aligned({})));\n",
            name, layout.size, layout.align, name, layout.size, layout.align
        )),
        (Ty::Struct { .. } | Ty::Tuple { .. }, regseq_model::LayoutKind::Struct { fields }) => {
            // Emit fields in MEMORY order (by offset) so the C layout matches
            // the model layout without padding directives in the common case.
            let mut ordered: Vec<_> = fields.iter().collect();
            ordered.sort_by_key(|f| f.offset);
            let mut out = format!("struct {name} {{\n");
            let mut any = false;
            for f in &ordered {
                let fname = c_ident(model.strings.resolve(f.name));
                if let Some(subst) = field_override(&fname) {
                    out.push_str(&format!("\t{subst};\n"));
                    any = true;
                    continue;
                }
                if is_zst(model, f.ty) {
                    continue;
                }
                out.push_str(&format!("\t{} {};\n", c_type(model, names, f.ty)?, fname));
                any = true;
            }
            if !any {
                out.push_str("\t/* fieldless spec aggregate */\n");
            }
            out.push_str("};\n");
            Ok(out)
        }
        other => Err(EmitError::Unsupported(format!(
            "aggregate definition for {other:?} (first-light subset)"
        ))),
    }
}

/// The scalar spelling for a type when it has one (used for tuple name
/// mangling); errors on non-scalars.
fn scalar_spelling_or_err(model: &RegseqModel, ty: TyId) -> Result<&'static str, EmitError> {
    match &model.types[ty] {
        Ty::Bool => Ok("bool"),
        Ty::Int(k) => int_spelling(*k),
        Ty::Uint(k) => uint_spelling(*k),
        other => Err(EmitError::Unsupported(format!(
            "tuple over non-scalar field {other:?}"
        ))),
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
