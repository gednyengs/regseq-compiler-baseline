//! Item hierarchy construction from enriched extraction.
//!
//! Builds the Layer 2 item hierarchy from Phase 0 identity enrichment facts.
//! This is Phase 2A: hierarchy construction only. No MIR-derived semantic
//! body content is populated. All `FunctionDef` bodies are
//! `SemanticBody::empty()` per the accepted regseq-lowering-05 type boundary.
//!
//! The builder is deterministic: given the same enrichment input and
//! configuration, it produces the same `ItemHierarchy` output.

use std::collections::BTreeMap;

use regseq_model::declaration::raw_input::{
    RawFieldFactInput, RawIdentityEnrichmentInput, RawImplBlockFactInput, RawImplMethodFactInput,
    RawRseqFieldKindInput,
};
use regseq_model::declaration::Provenance;
use regseq_model::item::attributes::{FieldAttribute, ItemAttribute};
use regseq_model::item::constant::{ConstValue, ConstantDef};
use regseq_model::item::cross_link::CrossLinkTable;
use regseq_model::item::function::{
    DeviceCallbackRole, DeviceClassKind, FunctionDef, FunctionKind, FunctionRole, LifecycleRole,
};
use regseq_model::item::identity::{CanonicalPath, ItemId, ItemStore};
use regseq_model::item::impl_def::{ImplDef, TraitRef};
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::module::Module;
use regseq_model::item::type_ref::{PrimitiveType, TypeRef};
use regseq_model::item::types::{EnumDef, EnumVariantDef, Item, StructDef, StructFieldDef};
use regseq_model::semantic::SemanticBody;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Known canonical trait paths for callback classification.
///
/// The caller provides exact canonical trait identities as canonicalized
/// by the enrichment extractor (`extract_trait_identity` in
/// `crates/regseq-driver/src/extract/traits.rs`). The builder matches
/// `impl_block.trait_path` exactly against these fields -- no suffix,
/// prefix, or substring heuristics.
///
/// `CallbackTraitPaths` is the authoritative source for trait-path
/// matching in the hierarchy builder.
#[derive(Debug, Clone)]
pub struct CallbackTraitPaths {
    /// Exact canonical path for the Lifecycle trait.
    pub lifecycle: String,
    /// Exact canonical path for the Serial trait.
    pub serial: String,
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// The complete populated item hierarchy from a single construction pass.
///
/// Produced by [`build_item_hierarchy`]. Individual fields are serializable
/// via `regseq-model`'s serde derives for hierarchy dump artifacts and
/// reviewability.
#[derive(Debug, Clone)]
pub struct ItemHierarchy {
    /// Canonical item payload store.
    pub store: ItemStore,
    /// Logical modules (not pushed into `ItemStore`; `Module` is not an
    /// `Item` variant).
    pub modules: Vec<Module>,
    /// Cross-reference resolution table (item-level and field-level).
    pub cross_link: CrossLinkTable,
    /// Interrupt class definitions for the ISR model.
    pub interrupt_classes: Vec<InterruptClassDef>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors during hierarchy construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HierarchyBuildError {
    /// A canonical path appeared more than once in the item store.
    DuplicateCanonicalPath { path: String },
    /// An impl block references a self type not found in the enrichment.
    UnresolvedSelfType {
        impl_path: String,
        self_type_path: String,
    },
}

impl std::fmt::Display for HierarchyBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateCanonicalPath { path } => {
                write!(f, "duplicate canonical path: {path}")
            }
            Self::UnresolvedSelfType {
                impl_path,
                self_type_path,
            } => {
                write!(
                    f,
                    "unresolved self type for impl {impl_path}: {self_type_path}"
                )
            }
        }
    }
}

impl std::error::Error for HierarchyBuildError {}

// ---------------------------------------------------------------------------
// Public builder
// ---------------------------------------------------------------------------

/// Build the Layer 2 item hierarchy from enriched extraction data.
///
/// `FunctionDef` bodies use `SemanticBody::empty()` per the accepted
/// lowering-05 boundary and amended 06 spec. No MIR-derived body content
/// is populated.
///
/// # Errors
///
/// Returns `HierarchyBuildError::DuplicateCanonicalPath` if two items share
/// the same canonical path. Returns `HierarchyBuildError::UnresolvedSelfType`
/// if an impl block's self type cannot be resolved to an existing struct or
/// enum in the enrichment.
pub fn build_item_hierarchy(
    enrichment: &RawIdentityEnrichmentInput,
    constant_values: &[(CanonicalPath, ConstValue)],
    interrupt_classes: &[InterruptClassDef],
    isr_bindings: &BTreeMap<CanonicalPath, String>,
    trait_paths: &CallbackTraitPaths,
) -> Result<ItemHierarchy, HierarchyBuildError> {
    let mut store = ItemStore::new();
    let mut cross_link = CrossLinkTable::new();

    // Index constant values by canonical path for O(1) lookup.
    let const_map: BTreeMap<&str, &ConstValue> = constant_values
        .iter()
        .map(|(p, v)| (p.as_str(), v))
        .collect();

    // Phase A: Structs and Enums (leaf items, no outgoing ItemId refs).
    build_structs(enrichment, &mut store, &mut cross_link)?;
    build_enums(enrichment, &mut store, &mut cross_link)?;

    // Phase B: Impl blocks with methods and constants.
    build_impl_blocks(
        enrichment,
        &mut store,
        &mut cross_link,
        isr_bindings,
        trait_paths,
        &const_map,
    )?;

    // Phase C: Free functions.
    build_free_functions(enrichment, &mut store, &mut cross_link)?;

    // Phase D: Modules.
    let modules = build_modules(enrichment, &cross_link);

    // Phase E: Copy interrupt classes.
    let interrupt_classes = interrupt_classes.to_vec();

    Ok(ItemHierarchy {
        store,
        modules,
        cross_link,
        interrupt_classes,
    })
}

// ---------------------------------------------------------------------------
// Phase A: Structs and Enums
// ---------------------------------------------------------------------------

fn build_structs(
    enrichment: &RawIdentityEnrichmentInput,
    store: &mut ItemStore,
    cross_link: &mut CrossLinkTable,
) -> Result<(), HierarchyBuildError> {
    for s in &enrichment.structs {
        let cp = CanonicalPath::new(&s.canonical_path);
        check_duplicate(cross_link, &cp)?;

        let fields: Vec<StructFieldDef> = s.fields.iter().map(build_struct_field).collect();

        let mut attributes = Vec::new();
        if s.is_driver {
            attributes.push(ItemAttribute::Driver);
        }
        if s.is_register_block {
            attributes.push(ItemAttribute::RegisterBlock);
        }

        let struct_def = StructDef {
            id: ItemId(0), // overwritten by store.push()
            canonical_path: cp.clone(),
            name: s.name.clone(),
            fields: fields.clone(),
            attributes,
            provenance: Provenance::new("enrichment"),
        };

        let struct_id = store.push(Item::Struct(struct_def));
        cross_link.insert_item(cp, struct_id);

        // Insert field-level cross-links.
        for (idx, field) in fields.iter().enumerate() {
            check_duplicate(cross_link, &field.canonical_path)?;
            cross_link.insert_field(field.canonical_path.clone(), struct_id, idx);
        }
    }
    Ok(())
}

fn build_enums(
    enrichment: &RawIdentityEnrichmentInput,
    store: &mut ItemStore,
    cross_link: &mut CrossLinkTable,
) -> Result<(), HierarchyBuildError> {
    for e in &enrichment.enums {
        let cp = CanonicalPath::new(&e.canonical_path);
        check_duplicate(cross_link, &cp)?;

        let variants: Vec<EnumVariantDef> = e
            .variants
            .iter()
            .map(|name| EnumVariantDef {
                name: name.clone(),
                discriminant: None,
                fields: Vec::new(),
            })
            .collect();

        let enum_def = EnumDef {
            id: ItemId(0),
            canonical_path: cp.clone(),
            name: e.name.clone(),
            variants,
            provenance: Provenance::new("enrichment"),
        };

        let enum_id = store.push(Item::Enum(enum_def));
        cross_link.insert_item(cp, enum_id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase B: Impl blocks
// ---------------------------------------------------------------------------

fn build_impl_blocks(
    enrichment: &RawIdentityEnrichmentInput,
    store: &mut ItemStore,
    cross_link: &mut CrossLinkTable,
    isr_bindings: &BTreeMap<CanonicalPath, String>,
    trait_paths: &CallbackTraitPaths,
    const_map: &BTreeMap<&str, &ConstValue>,
) -> Result<(), HierarchyBuildError> {
    for ib in &enrichment.impl_blocks {
        let impl_cp = CanonicalPath::new(&ib.canonical_path);
        check_duplicate(cross_link, &impl_cp)?;

        // Resolve self type -- hard error if unresolved.
        let self_type_cp = CanonicalPath::new(&ib.self_type_path);
        let self_type_id = cross_link.resolve_item(&self_type_cp).ok_or(
            HierarchyBuildError::UnresolvedSelfType {
                impl_path: ib.canonical_path.clone(),
                self_type_path: ib.self_type_path.clone(),
            },
        )?;

        let self_type = TypeRef::Item { id: self_type_id };

        let trait_ref = match (&ib.trait_path, &ib.trait_name) {
            (Some(path), Some(name)) => Some(TraitRef {
                name: name.clone(),
                path: path.clone(),
            }),
            _ => None,
        };

        // Build methods.
        let mut method_ids = Vec::with_capacity(ib.methods.len());
        for method in &ib.methods {
            let method_cp = CanonicalPath::new(&method.canonical_path);
            check_duplicate(cross_link, &method_cp)?;

            let kind = if trait_ref.is_some() {
                FunctionKind::TraitMethod {
                    trait_ref: trait_ref.clone().unwrap(),
                    owner: self_type_id,
                }
            } else {
                FunctionKind::InherentMethod {
                    owner: self_type_id,
                }
            };

            let role = classify_function_role(ib, method, isr_bindings, trait_paths);

            let func_def = FunctionDef {
                id: ItemId(0),
                canonical_path: method_cp.clone(),
                name: method.name.clone(),
                kind,
                role,
                body: SemanticBody::empty(),
                provenance: Provenance::new("enrichment"),
            };

            let func_id = store.push(Item::Function(func_def));
            cross_link.insert_item(method_cp, func_id);
            method_ids.push(func_id);
        }

        // Build associated constants.
        let mut constant_ids = Vec::with_capacity(ib.constants.len());
        for c in &ib.constants {
            let const_cp = CanonicalPath::new(&c.canonical_path);
            check_duplicate(cross_link, &const_cp)?;

            let value = const_map
                .get(c.canonical_path.as_str())
                .map(|v| (*v).clone());
            let ty = parse_type_ref(&c.ty);
            let stable_name = derive_stable_name(&ib.self_type_name, &c.name);

            let const_def = ConstantDef {
                id: ItemId(0),
                canonical_path: const_cp.clone(),
                name: c.name.clone(),
                owner: Some(self_type_id),
                ty,
                value,
                stable_name,
                provenance: Provenance::new("enrichment"),
            };

            let const_id = store.push(Item::Constant(const_def));
            cross_link.insert_item(const_cp, const_id);
            constant_ids.push(const_id);
        }

        // Create ImplDef.
        let impl_def = ImplDef {
            id: ItemId(0),
            canonical_path: impl_cp.clone(),
            self_type,
            trait_ref,
            methods: method_ids,
            constants: constant_ids,
            provenance: Provenance::new("enrichment"),
        };

        let impl_id = store.push(Item::Impl(impl_def));
        cross_link.insert_item(impl_cp, impl_id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase C: Free functions
// ---------------------------------------------------------------------------

fn build_free_functions(
    enrichment: &RawIdentityEnrichmentInput,
    store: &mut ItemStore,
    cross_link: &mut CrossLinkTable,
) -> Result<(), HierarchyBuildError> {
    for f in &enrichment.free_functions {
        let cp = CanonicalPath::new(&f.canonical_path);
        check_duplicate(cross_link, &cp)?;

        let func_def = FunctionDef {
            id: ItemId(0),
            canonical_path: cp.clone(),
            name: f.name.clone(),
            kind: FunctionKind::FreeFunction,
            role: FunctionRole::Helper,
            body: SemanticBody::empty(),
            provenance: Provenance::new("enrichment"),
        };

        let func_id = store.push(Item::Function(func_def));
        cross_link.insert_item(cp, func_id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase D: Modules
// ---------------------------------------------------------------------------

fn build_modules(
    enrichment: &RawIdentityEnrichmentInput,
    cross_link: &CrossLinkTable,
) -> Vec<Module> {
    enrichment
        .modules
        .iter()
        .map(|m| {
            let items: Vec<ItemId> = m
                .child_items
                .iter()
                .filter_map(|path| cross_link.resolve_item(&CanonicalPath::new(path)))
                .collect();

            Module {
                name: m.name.clone(),
                canonical_path: CanonicalPath::new(&m.canonical_path),
                items,
                provenance: Provenance::new("enrichment"),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Function role classification
// ---------------------------------------------------------------------------

fn classify_function_role(
    impl_block: &RawImplBlockFactInput,
    method: &RawImplMethodFactInput,
    isr_bindings: &BTreeMap<CanonicalPath, String>,
    trait_paths: &CallbackTraitPaths,
) -> FunctionRole {
    // 1. ISR check (structural, from enrichment's HIR-derived is_isr flag).
    if method.is_isr {
        let binding = isr_bindings
            .get(&CanonicalPath::new(&method.canonical_path))
            .cloned()
            .unwrap_or_else(|| "auto".into());
        return FunctionRole::Isr { binding };
    }

    // 2. Trait callback (exact canonical trait identity matching).
    if let Some(ref trait_path) = impl_block.trait_path {
        if trait_path == &trait_paths.lifecycle {
            return classify_lifecycle_method(&method.name);
        }
        if trait_path == &trait_paths.serial {
            return classify_serial_method(&method.name);
        }
    }

    // 3. Inherent impl fallback.
    FunctionRole::Helper
}

fn classify_lifecycle_method(name: &str) -> FunctionRole {
    match name {
        "init" => FunctionRole::Lifecycle {
            role: LifecycleRole::Init,
        },
        "deinit" => FunctionRole::Lifecycle {
            role: LifecycleRole::Deinit,
        },
        _ => FunctionRole::Helper,
    }
}

fn classify_serial_method(name: &str) -> FunctionRole {
    let callback = match name {
        "port_open" => DeviceCallbackRole::PortOpen,
        "port_close" => DeviceCallbackRole::PortClose,
        "configure" => DeviceCallbackRole::Configure,
        "tx_byte" => DeviceCallbackRole::TxByte,
        "rx_char" => DeviceCallbackRole::RxChar,
        "tx_ready" => DeviceCallbackRole::TxReady,
        "tx_empty" => DeviceCallbackRole::TxEmpty,
        "flush_rx_fifo" => DeviceCallbackRole::FlushRxFifo,
        "flush_tx_fifo" => DeviceCallbackRole::FlushTxFifo,
        "send_break" => DeviceCallbackRole::SendBreak,
        other => DeviceCallbackRole::Named(other.to_string()),
    };
    FunctionRole::DeviceClassCallback {
        class: DeviceClassKind::Serial,
        callback,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_struct_field(f: &RawFieldFactInput) -> StructFieldDef {
    let attributes = f
        .rseq_kind
        .as_ref()
        .map(|k| vec![map_rseq_field_kind(k)])
        .unwrap_or_default();

    StructFieldDef {
        canonical_path: CanonicalPath::new(&f.canonical_path),
        name: f.name.clone(),
        ty: TypeRef::Named {
            name: "<unresolved>".into(),
        },
        attributes,
    }
}

fn map_rseq_field_kind(kind: &RawRseqFieldKindInput) -> FieldAttribute {
    match kind {
        RawRseqFieldKindInput::Mmio { id } => FieldAttribute::Mmio { id: id.clone() },
        RawRseqFieldKindInput::Clock { id } => FieldAttribute::Clock { id: id.clone() },
        RawRseqFieldKindInput::PinCtrl { id } => FieldAttribute::PinCtrl { id: id.clone() },
        RawRseqFieldKindInput::Interrupt { id } => FieldAttribute::Interrupt { id: id.clone() },
        RawRseqFieldKindInput::Property { id } => FieldAttribute::Property { id: id.clone() },
    }
}

fn parse_type_ref(ty: &str) -> TypeRef {
    match ty {
        "bool" => TypeRef::Primitive {
            ty: PrimitiveType::Bool,
        },
        "u8" => TypeRef::Primitive {
            ty: PrimitiveType::U8,
        },
        "u16" => TypeRef::Primitive {
            ty: PrimitiveType::U16,
        },
        "u32" => TypeRef::Primitive {
            ty: PrimitiveType::U32,
        },
        "u64" => TypeRef::Primitive {
            ty: PrimitiveType::U64,
        },
        "i8" => TypeRef::Primitive {
            ty: PrimitiveType::I8,
        },
        "i16" => TypeRef::Primitive {
            ty: PrimitiveType::I16,
        },
        "i32" => TypeRef::Primitive {
            ty: PrimitiveType::I32,
        },
        "i64" => TypeRef::Primitive {
            ty: PrimitiveType::I64,
        },
        "usize" => TypeRef::Primitive {
            ty: PrimitiveType::Usize,
        },
        other => TypeRef::Named {
            name: other.to_string(),
        },
    }
}

fn derive_stable_name(owner_name: &str, const_name: &str) -> String {
    let owner_upper = to_screaming_snake(owner_name);
    format!("{owner_upper}_{const_name}")
}

/// Convert a CamelCase identifier to SCREAMING_SNAKE_CASE.
///
/// Runs of uppercase characters followed by a lowercase character are
/// treated as acronyms (e.g., "FooBar" -> "FOO_BAR").
/// Digit-to-uppercase transitions also get an underscore
/// (e.g., "Pl011FullSpec" -> "PL011_FULL_SPEC").
fn to_screaming_snake(name: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            if ch.is_uppercase() {
                // Insert underscore before uppercase after lowercase or digit,
                // or before uppercase-before-lowercase in an acronym run.
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && i + 1 < chars.len() && chars[i + 1].is_lowercase())
                {
                    result.push('_');
                }
            } else if ch.is_ascii_digit() && prev.is_lowercase() {
                // Insert underscore before digit after lowercase (optional, not needed for PL011).
            }
        }
        result.push(ch.to_uppercase().next().unwrap());
    }
    result
}

fn check_duplicate(
    cross_link: &CrossLinkTable,
    path: &CanonicalPath,
) -> Result<(), HierarchyBuildError> {
    if cross_link.resolve_item(path).is_some() || cross_link.resolve_field(path).is_some() {
        return Err(HierarchyBuildError::DuplicateCanonicalPath {
            path: path.as_str().to_string(),
        });
    }
    Ok(())
}
