//! MIR type to semantic type conversion.
//!
//! Maps `MirTy` to `SemanticType` using the hierarchy for nominal ADT
//! resolution. Carrier types (`Option`, `Result`) are handled structurally.
//! All other nominal ADTs are resolved through the `CrossLinkTable` and
//! `ItemStore` to produce `SemanticType::Struct` or `SemanticType::Enum`
//! with real `ItemId`s.
//!
//! `Opaque` is a diagnostic fallback for genuinely unrecognized types.
//! It is NOT an accepted success mode for representative-set bodies.

use regseq_model::declaration::mir_body::{IntWidth, MirTy};
use regseq_model::item::cross_link::CrossLinkTable;
use regseq_model::item::identity::{CanonicalPath, ItemStore};
use regseq_model::item::types::Item;
use regseq_model::semantic::types::{ResourceKind, SemanticType};

use super::external_items::normalize_external_alias;

/// Convert a MIR type to a semantic type.
///
/// Requires the augmented hierarchy (with external items materialized)
/// for nominal ADT resolution. Carrier types are handled structurally.
pub fn lower_mir_ty(ty: &MirTy, cross_link: &CrossLinkTable, store: &ItemStore) -> SemanticType {
    match ty {
        // --- Scalars ---
        MirTy::Bool => SemanticType::Bool,
        MirTy::Int { signed, width } => lower_int(*signed, *width),

        // --- Unit ---
        MirTy::Tuple { elements } if elements.is_empty() => SemanticType::Void,

        // --- References ---
        MirTy::Ref {
            mutbl: false,
            ty: inner,
        } => SemanticType::Ref {
            inner: Box::new(lower_mir_ty(inner, cross_link, store)),
        },
        MirTy::Ref {
            mutbl: true,
            ty: inner,
        } => SemanticType::RefMut {
            inner: Box::new(lower_mir_ty(inner, cross_link, store)),
        },

        // --- ADTs: carrier types and nominal types ---
        MirTy::Struct { path, fields } => lower_adt(path, Some(fields), cross_link, store),
        MirTy::Enum { path, variants } => lower_adt_enum(path, variants, cross_link, store),

        // --- Never type ---
        MirTy::Never => SemanticType::Void,

        // --- Everything else is opaque ---
        _ => SemanticType::Opaque {
            description: format_mir_ty_description(ty),
        },
    }
}

fn lower_int(signed: bool, width: IntWidth) -> SemanticType {
    match (signed, width) {
        (false, IntWidth::U8) => SemanticType::U8,
        (false, IntWidth::U16) => SemanticType::U16,
        (false, IntWidth::U32) => SemanticType::U32,
        (false, IntWidth::U64) => SemanticType::U64,
        (false, IntWidth::Usize) => SemanticType::U64,
        (false, IntWidth::U128) => SemanticType::Opaque {
            description: "u128".into(),
        },
        (true, IntWidth::I8) => SemanticType::I8,
        (true, IntWidth::I16) => SemanticType::I16,
        (true, IntWidth::I32) => SemanticType::I32,
        (true, IntWidth::I64) => SemanticType::I64,
        (true, IntWidth::Isize) => SemanticType::I64,
        (true, IntWidth::I128) => SemanticType::Opaque {
            description: "i128".into(),
        },
        // Mismatched sign/width (e.g. signed=true with U32) — treat as opaque.
        _ => SemanticType::Opaque {
            description: format!("int(signed={signed}, width={width:?})"),
        },
    }
}

/// Lower a struct ADT type.
///
/// Domain infrastructure types (register wrappers, resource handles) are
/// handled structurally. All other nominal structs are resolved through
/// the hierarchy.
fn lower_adt(
    path: &str,
    _fields: Option<&Vec<regseq_model::declaration::mir_body::MirFieldTy>>,
    cross_link: &CrossLinkTable,
    store: &ItemStore,
) -> SemanticType {
    // --- Domain infrastructure types (register/resource wrappers) ---

    // Register wrappers (ReadWrite, ReadOnly, WriteOnly)
    if path.starts_with("regseq::ReadWrite")
        || path.starts_with("regseq::ReadOnly")
        || path.starts_with("regseq::WriteOnly")
    {
        return SemanticType::RegisterValue { width: 32 };
    }

    // Mmio wrapper
    if path.starts_with("regseq::Mmio") {
        return SemanticType::Opaque {
            description: format!("Mmio wrapper: {path}"),
        };
    }

    // FieldValue / Field -- register field value constructor types
    if path.contains("FieldValue") || path.contains("Field<") {
        return SemanticType::RegisterValue { width: 32 };
    }

    // Resource handles
    if path.starts_with("regseq::Clock") {
        return SemanticType::ResourceHandle {
            kind: ResourceKind::Clock,
        };
    }
    if path.starts_with("regseq::ResetLine") {
        return SemanticType::ResourceHandle {
            kind: ResourceKind::Reset,
        };
    }
    if path.starts_with("regseq::PinCtrl") {
        return SemanticType::ResourceHandle {
            kind: ResourceKind::PinCtrl,
        };
    }

    // Duration -- timing type (structural)
    if path.contains("Duration") {
        return SemanticType::Opaque {
            description: "Duration (structural)".into(),
        };
    }

    // --- Hierarchy resolution for all other nominal types ---
    resolve_nominal(path, cross_link, store)
}

/// Lower an enum ADT type.
///
/// Carrier types (Option, Result, ControlFlow) are handled structurally
/// by extracting inner types from variant fields. All other nominal enums
/// are resolved through the hierarchy.
fn lower_adt_enum(
    path: &str,
    variants: &[regseq_model::declaration::mir_body::MirVariantTy],
    cross_link: &CrossLinkTable,
    store: &ItemStore,
) -> SemanticType {
    // --- Carrier types (structural, not hierarchy-resolved) ---

    // Option<T>
    if path.contains("Option") && !path.contains("ControlFlow") {
        // Extract T from the Some variant's first field.
        let inner = variants
            .iter()
            .find(|v| v.name == "Some")
            .and_then(|v| v.fields.first())
            .map(|f| lower_mir_ty(&f.ty, cross_link, store))
            .unwrap_or(SemanticType::Opaque {
                description: "Option<?>".into(),
            });
        return SemanticType::Option {
            inner: Box::new(inner),
        };
    }

    // Result<T, E>
    if path.contains("Result") && !path.contains("ControlFlow") {
        let ok = variants
            .iter()
            .find(|v| v.name == "Ok")
            .and_then(|v| v.fields.first())
            .map(|f| lower_mir_ty(&f.ty, cross_link, store))
            .unwrap_or(SemanticType::Void);
        let err = variants
            .iter()
            .find(|v| v.name == "Err")
            .and_then(|v| v.fields.first())
            .map(|f| lower_mir_ty(&f.ty, cross_link, store))
            .unwrap_or(SemanticType::Opaque {
                description: "Result error type".into(),
            });
        return SemanticType::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        };
    }

    // ControlFlow<B, C> -- structural MIR type from ? desugaring
    if path.contains("ControlFlow") {
        return SemanticType::Opaque {
            description: "ControlFlow (structural)".into(),
        };
    }

    // --- Hierarchy resolution for all other nominal types ---
    resolve_nominal(path, cross_link, store)
}

/// Resolve a nominal type through the hierarchy.
///
/// This handles both local authored types and discovered external items.
fn resolve_nominal(path: &str, cross_link: &CrossLinkTable, store: &ItemStore) -> SemanticType {
    // Normalize aliases before lookup.
    let normalized = normalize_external_alias(path);
    let canonical = CanonicalPath::new(normalized);
    if let Some(id) = cross_link.resolve_item(&canonical) {
        if let Some(item) = store.get(id) {
            return match item {
                Item::Struct(s) => SemanticType::Struct {
                    id,
                    name: s.name.clone(),
                },
                Item::Enum(e) => SemanticType::Enum {
                    id,
                    name: e.name.clone(),
                },
                _ => SemanticType::Opaque {
                    description: format!("non-type item: {path}"),
                },
            };
        }
    }

    // Truly unrecognized — diagnostic fallback only.
    // NOT an accepted success mode for representative-set bodies.
    SemanticType::Opaque {
        description: format!("unrecognized ADT: {path}"),
    }
}

/// Produce a diagnostic description of a MIR type for `Opaque`.
fn format_mir_ty_description(ty: &MirTy) -> String {
    match ty {
        MirTy::Char => "char".into(),
        MirTy::Float { .. } => "float".into(),
        MirTy::RawPtr { .. } => "raw pointer".into(),
        MirTy::Fn { path, .. } => format!("fn: {path}"),
        MirTy::Tuple { elements } => format!("tuple({} elements)", elements.len()),
        MirTy::Array { element, len } => {
            format!("[{}; {len}]", format_mir_ty_description(element))
        }
        MirTy::Slice { .. } => "slice".into(),
        MirTy::Str => "str".into(),
        MirTy::Bytes => "bytes".into(),
        MirTy::ZeroSizedLike { description } => format!("zero-sized: {description}"),
        MirTy::NominalRef { path } => format!("nominal ref: {path}"),
        MirTy::Unsupported { description } => format!("unsupported: {description}"),
        _ => "unknown MIR type".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::item::identity::ItemId;

    fn empty_cross_link() -> CrossLinkTable {
        CrossLinkTable::new()
    }

    fn empty_store() -> ItemStore {
        ItemStore::new()
    }

    #[test]
    fn scalar_types() {
        let cl = empty_cross_link();
        let st = empty_store();
        assert_eq!(lower_mir_ty(&MirTy::Bool, &cl, &st), SemanticType::Bool);
        assert_eq!(
            lower_mir_ty(
                &MirTy::Int {
                    signed: false,
                    width: IntWidth::U32,
                },
                &cl,
                &st
            ),
            SemanticType::U32
        );
    }

    #[test]
    fn unit_tuple_is_void() {
        let cl = empty_cross_link();
        let st = empty_store();
        assert_eq!(
            lower_mir_ty(&MirTy::Tuple { elements: vec![] }, &cl, &st),
            SemanticType::Void
        );
    }

    #[test]
    fn option_type() {
        use regseq_model::declaration::mir_body::{MirFieldTy, MirVariantTy};
        let cl = empty_cross_link();
        let st = empty_store();
        let ty = MirTy::Enum {
            path: "core::option::Option".into(),
            variants: vec![
                MirVariantTy {
                    name: "None".into(),
                    fields: vec![],
                },
                MirVariantTy {
                    name: "Some".into(),
                    fields: vec![MirFieldTy {
                        index: 0,
                        name: "0".into(),
                        ty: MirTy::Int {
                            signed: false,
                            width: IntWidth::U8,
                        },
                    }],
                },
            ],
        };
        assert_eq!(
            lower_mir_ty(&ty, &cl, &st),
            SemanticType::Option {
                inner: Box::new(SemanticType::U8)
            }
        );
    }

    #[test]
    fn result_type_with_hierarchy() {
        use regseq_model::declaration::mir_body::{MirFieldTy, MirVariantTy};
        // RegseqError should resolve through hierarchy if materialized.
        let mut cl = CrossLinkTable::new();
        let mut st = ItemStore::new();
        let id = st.push(Item::Enum(regseq_model::item::types::EnumDef {
            id: ItemId(0),
            canonical_path: CanonicalPath::new("regseq::RegseqError"),
            name: "RegseqError".into(),
            variants: vec![],
            provenance: regseq_model::declaration::Provenance::new("external:regseq"),
        }));
        cl.insert_item(CanonicalPath::new("regseq::RegseqError"), id);

        let ty = MirTy::Enum {
            path: "core::result::Result".into(),
            variants: vec![
                MirVariantTy {
                    name: "Ok".into(),
                    fields: vec![MirFieldTy {
                        index: 0,
                        name: "0".into(),
                        ty: MirTy::Tuple { elements: vec![] },
                    }],
                },
                MirVariantTy {
                    name: "Err".into(),
                    fields: vec![MirFieldTy {
                        index: 0,
                        name: "0".into(),
                        ty: MirTy::Enum {
                            path: "regseq::RegseqError".into(),
                            variants: vec![],
                        },
                    }],
                },
            ],
        };
        match lower_mir_ty(&ty, &cl, &st) {
            SemanticType::Result { ok, err } => {
                assert_eq!(*ok, SemanticType::Void);
                assert!(
                    matches!(*err, SemanticType::Enum { ref name, .. } if name == "RegseqError")
                );
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn external_type_resolves_through_hierarchy() {
        let mut cl = CrossLinkTable::new();
        let mut st = ItemStore::new();
        let id = st.push(Item::Struct(regseq_model::item::types::StructDef {
            id: ItemId(0),
            canonical_path: CanonicalPath::new("regseq::RxChar"),
            name: "RxChar".into(),
            fields: vec![],
            attributes: vec![],
            provenance: regseq_model::declaration::Provenance::new("external:regseq"),
        }));
        cl.insert_item(CanonicalPath::new("regseq::RxChar"), id);

        let ty = MirTy::Struct {
            path: "regseq::RxChar".into(),
            fields: vec![],
        };
        match lower_mir_ty(&ty, &cl, &st) {
            SemanticType::Struct { name, .. } => assert_eq!(name, "RxChar"),
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn unrecognized_adt_is_opaque() {
        let cl = empty_cross_link();
        let st = empty_store();
        let ty = MirTy::Struct {
            path: "some::unknown::Type".into(),
            fields: vec![],
        };
        assert!(matches!(
            lower_mir_ty(&ty, &cl, &st),
            SemanticType::Opaque { .. }
        ));
    }

    #[test]
    fn serial_errors_resolves_directly() {
        let mut cl = CrossLinkTable::new();
        let mut st = ItemStore::new();
        let id = st.push(Item::Struct(regseq_model::item::types::StructDef {
            id: ItemId(0),
            canonical_path: CanonicalPath::new("regseq::SerialErrors"),
            name: "SerialErrors".into(),
            fields: vec![],
            attributes: vec![],
            provenance: regseq_model::declaration::Provenance::new("external:regseq"),
        }));
        cl.insert_item(CanonicalPath::new("regseq::SerialErrors"), id);

        let ty = MirTy::Struct {
            path: "regseq::SerialErrors".into(),
            fields: vec![],
        };
        match lower_mir_ty(&ty, &cl, &st) {
            SemanticType::Struct { name, .. } => assert_eq!(name, "SerialErrors"),
            other => panic!("expected Struct (SerialErrors), got {other:?}"),
        }
    }
}
