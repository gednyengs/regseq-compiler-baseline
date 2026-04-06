//! Canonical identity resolution from the item hierarchy.
//!
//! Resolves MIR-derived identities to structured semantic refs through
//! canonical paths and hierarchy ownership structure. The resolver is
//! keyed by canonical paths, not by short names.
//!
//! **Current-slice limitation:** field and resource resolution assumes
//! driver-struct ownership. Helper-type field access (methods on non-driver
//! structs) is not yet supported. Unsupported ownership patterns produce
//! hard errors.
//!
//! **Transitional bridge:** interrupt class identity is bridged through
//! mask-value matching, not canonical paths. This is acceptable for the
//! PL011 slice but not the long-term semantic identity contract.

use std::collections::BTreeMap;

use regseq_model::item::attributes::{FieldAttribute, ItemAttribute};
use regseq_model::item::constant::{ConstValue, ConstantDef};
use regseq_model::item::function::FunctionDef;
use regseq_model::item::identity::ItemId;
use regseq_model::item::interrupt::InterruptClassDef;
use regseq_model::item::types::{Item, StructDef, StructFieldDef};
use regseq_model::semantic::refs::{
    ConstantRef, DriverFieldRef, FunctionRef, InterruptClassId, InterruptClassRef, RegisterRef,
    ResourceRef,
};
use regseq_model::semantic::types::ResourceKind;

use super::SemanticLoweringError;
use crate::lowering::hierarchy::ItemHierarchy;

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// Canonical identity resolver built from the item hierarchy.
///
/// All lowering-critical resolution goes through canonical paths and
/// ownership structure. Short names are for diagnostics only.
pub struct ReferenceResolver {
    /// Driver struct ItemId.
    driver_id: ItemId,
    /// Driver struct field list (cached for field lookups).
    driver_fields: Vec<StructFieldDef>,
    /// Register block ItemId (first MMIO block in current slice).
    register_block_id: Option<ItemId>,
    /// Register block field list (cached for register lookups).
    register_block_fields: Vec<StructFieldDef>,
    /// Interrupt class definitions (from hierarchy).
    interrupt_classes: Vec<InterruptClassDef>,
    /// Item hierarchy reference data: function ItemIds indexed by canonical path.
    function_index: BTreeMap<String, (ItemId, FunctionDef)>,
    /// Constant ItemIds indexed by canonical path.
    constant_index: BTreeMap<String, (ItemId, ConstantDef)>,
    /// MIR def_path -> canonical path normalization index.
    ///
    /// MIR uses shortened def_paths (e.g., `driver::Pl011FullSpec::init`)
    /// while the hierarchy uses full canonical paths
    /// (e.g., `pl011_full_spec::driver::Pl011FullSpec::init`).
    /// This is a deterministic prefix normalization, not a name-based fallback.
    mir_path_to_canonical: BTreeMap<String, String>,
}

impl ReferenceResolver {
    /// Build the resolver from an item hierarchy.
    ///
    /// Finds the driver struct and register blocks by walking `Module.items`
    /// and checking `ItemAttribute`. Caches structural data for fast lookups.
    pub fn build(hierarchy: &ItemHierarchy) -> Result<Self, SemanticLoweringError> {
        let mut driver_id = None;
        let mut driver_fields = Vec::new();
        let mut register_block_id = None;
        let mut register_block_fields = Vec::new();
        let mut function_index = BTreeMap::new();
        let mut constant_index = BTreeMap::new();

        // Walk all items reachable through modules.
        let all_ids: Vec<ItemId> = hierarchy
            .modules
            .iter()
            .flat_map(|m| m.items.iter().copied())
            .collect();

        for id in &all_ids {
            let Some(item) = hierarchy.store.get(*id) else {
                continue;
            };
            match item {
                Item::Struct(s) => {
                    if s.attributes.contains(&ItemAttribute::Driver) {
                        driver_id = Some(s.id);
                        driver_fields = s.fields.clone();
                    }
                    if s.attributes.contains(&ItemAttribute::RegisterBlock) {
                        register_block_id = Some(s.id);
                        register_block_fields = s.fields.clone();
                    }
                }
                Item::Impl(impl_def) => {
                    // Index methods and constants from this impl.
                    for method_id in &impl_def.methods {
                        if let Some(Item::Function(func)) = hierarchy.store.get(*method_id) {
                            function_index.insert(
                                func.canonical_path.as_str().to_string(),
                                (*method_id, func.clone()),
                            );
                        }
                    }
                    for const_id in &impl_def.constants {
                        if let Some(Item::Constant(c)) = hierarchy.store.get(*const_id) {
                            constant_index.insert(
                                c.canonical_path.as_str().to_string(),
                                (*const_id, c.clone()),
                            );
                        }
                    }
                }
                Item::Function(func) => {
                    // Free functions.
                    function_index.insert(
                        func.canonical_path.as_str().to_string(),
                        (*id, func.clone()),
                    );
                }
                Item::Constant(c) => {
                    // Module-level constants.
                    constant_index.insert(c.canonical_path.as_str().to_string(), (*id, c.clone()));
                }
                _ => {}
            }
        }

        let driver_id = driver_id.ok_or(SemanticLoweringError::DriverStructNotFound)?;

        // Build MIR def_path normalization index.
        // MIR def_paths strip the crate prefix from canonical paths and use
        // `<Type as Trait>::method` syntax for trait impls. We build both
        // directions: stripped canonical -> canonical, and trait-style -> canonical.
        let crate_prefix = hierarchy
            .modules
            .first()
            .map(|m| format!("{}::", m.canonical_path.as_str()))
            .unwrap_or_default();

        let mut mir_path_to_canonical = BTreeMap::new();
        // Strip crate prefix for functions.
        for canonical in function_index.keys() {
            if let Some(stripped) = canonical.strip_prefix(&crate_prefix) {
                mir_path_to_canonical.insert(stripped.to_string(), canonical.clone());
            }
        }
        // Strip crate prefix for constants.
        for canonical in constant_index.keys() {
            if let Some(stripped) = canonical.strip_prefix(&crate_prefix) {
                mir_path_to_canonical.insert(stripped.to_string(), canonical.clone());
            }
        }
        // Also add trait-style paths for trait impl methods.
        // E.g., canonical: "pl011_full_spec::driver::Pl011FullSpec::init"
        //        MIR:      "<driver::Pl011FullSpec as regseq::Lifecycle>::init"
        for id in &all_ids {
            if let Some(Item::Impl(impl_def)) = hierarchy.store.get(*id) {
                if let Some(ref trait_ref) = impl_def.trait_ref {
                    // Get the self type name.
                    let self_type_name = match &impl_def.self_type {
                        regseq_model::item::type_ref::TypeRef::Item { id: self_id } => {
                            hierarchy.store.get(*self_id).and_then(|item| match item {
                                Item::Struct(s) => Some(s.name.clone()),
                                _ => None,
                            })
                        }
                        _ => None,
                    };
                    if let Some(type_name) = self_type_name {
                        // Get the module-relative type path by stripping crate prefix.
                        let self_canonical = impl_def
                            .canonical_path
                            .as_str()
                            .strip_prefix(&crate_prefix)
                            .unwrap_or(impl_def.canonical_path.as_str());
                        // Find the module path for the self type.
                        let type_module = self_canonical
                            .strip_suffix(&format!("::<impl {} for {type_name}>", trait_ref.name))
                            .or_else(|| {
                                self_canonical.strip_suffix(&format!("::<impl {type_name}>"))
                            })
                            .unwrap_or("");

                        for method_id in &impl_def.methods {
                            if let Some(Item::Function(func)) = hierarchy.store.get(*method_id) {
                                let canonical = func.canonical_path.as_str().to_string();

                                // Build MIR-style trait impl path:
                                // <module::Type as trait_path>::method_name
                                let module_type = if type_module.is_empty() {
                                    type_name.clone()
                                } else {
                                    format!("{type_module}::{type_name}")
                                };
                                let mir_impl_path =
                                    format!("<{module_type} as {}>::{}", trait_ref.path, func.name);
                                mir_path_to_canonical.insert(mir_impl_path, canonical.clone());

                                // Also index the simple trait-level path:
                                // trait_path::method_name (e.g., regseq::Serial::tx_ready)
                                let trait_path = format!("{}::{}", trait_ref.path, func.name);
                                mir_path_to_canonical.insert(trait_path, canonical);
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            driver_id,
            driver_fields,
            register_block_id,
            register_block_fields,
            interrupt_classes: hierarchy.interrupt_classes.clone(),
            function_index,
            constant_index,
            mir_path_to_canonical,
        })
    }

    /// The driver struct ItemId.
    pub fn driver_id(&self) -> ItemId {
        self.driver_id
    }

    /// The register block ItemId, if one exists.
    pub fn register_block_id(&self) -> Option<ItemId> {
        self.register_block_id
    }

    // -----------------------------------------------------------------------
    // Function resolution
    // -----------------------------------------------------------------------

    /// Resolve a function by its canonical path or MIR def_path.
    ///
    /// Tries direct canonical path lookup first, then normalizes MIR
    /// def_paths through a deterministic prefix normalization index.
    /// No short-name or final-segment fallback.
    pub fn resolve_function_by_def_path(
        &self,
        def_path: &str,
    ) -> Result<(ItemId, &FunctionDef), SemanticLoweringError> {
        // Direct canonical path lookup.
        if let Some((id, func)) = self.function_index.get(def_path) {
            return Ok((*id, func));
        }

        // MIR def_path normalization: deterministic prefix mapping.
        if let Some(canonical) = self.mir_path_to_canonical.get(def_path) {
            if let Some((id, func)) = self.function_index.get(canonical) {
                return Ok((*id, func));
            }
        }

        Err(SemanticLoweringError::UnresolvedFunction {
            path: def_path.into(),
            hint: "not found via canonical path or MIR def_path normalization".into(),
        })
    }

    /// Resolve a MIR call path to a `FunctionRef`.
    ///
    /// `mir_fn_path` is the full path from `MirTy::FnDef { path }`.
    /// Canonical identity must match exactly. No short-name fallback.
    pub fn resolve_call(&self, mir_fn_path: &str) -> Result<FunctionRef, SemanticLoweringError> {
        let (id, func) = self.resolve_function_by_def_path(mir_fn_path)?;
        Ok(FunctionRef {
            item: id,
            canonical_path: func.canonical_path.clone(),
            name: func.name.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // Register resolution
    // -----------------------------------------------------------------------

    /// Resolve a register short name to a `RegisterRef` through the
    /// ownership chain: driver -> MMIO field -> register block -> field.
    pub fn resolve_register(
        &self,
        register_name: &str,
    ) -> Result<RegisterRef, SemanticLoweringError> {
        let block_id =
            self.register_block_id
                .ok_or(SemanticLoweringError::RegisterBlockNotFound {
                    mmio_field: "unknown".into(),
                })?;

        // Find the matching register field in the register block.
        // MIR type modules produce names like "lcrh" (from LCRH_REG) while
        // register block fields may use "lcr_h". We match using underscore-
        // stripped normalization as a deterministic convention bridge.
        let normalized_query = register_name.replace('_', "");
        for field in &self.register_block_fields {
            if field.name == register_name || field.name.replace('_', "") == normalized_query {
                return Ok(RegisterRef {
                    block: block_id,
                    canonical_path: field.canonical_path.clone(),
                    name: field.name.clone(),
                });
            }
        }

        Err(SemanticLoweringError::UnresolvedRegister {
            name: register_name.into(),
            context: "register block field lookup".into(),
        })
    }

    // -----------------------------------------------------------------------
    // Driver field resolution
    // -----------------------------------------------------------------------

    /// Resolve a driver field name to a `DriverFieldRef`.
    pub fn resolve_driver_field(
        &self,
        field_name: &str,
    ) -> Result<DriverFieldRef, SemanticLoweringError> {
        for field in &self.driver_fields {
            if field.name == field_name {
                return Ok(DriverFieldRef {
                    owner: self.driver_id,
                    canonical_path: field.canonical_path.clone(),
                    name: field.name.clone(),
                });
            }
        }

        Err(SemanticLoweringError::UnresolvedDriverField {
            name: field_name.into(),
        })
    }

    /// Check if a driver field is Option-typed (has no resource attribute
    /// but is known to be optional state, like `pending_tx: Option<u8>`).
    ///
    /// For the current slice this is heuristic: fields without resource
    /// attributes that are not MMIO, interrupt, or property fields.
    pub fn is_option_state_field(&self, field_name: &str) -> bool {
        for field in &self.driver_fields {
            if field.name == field_name {
                // If the field has no resource/structural attribute, it is
                // likely an Option state field.
                return field.attributes.is_empty();
            }
        }
        false
    }

    /// Resolve a driver field by its struct field index.
    ///
    /// Uses the enrichment-derived field list which includes all driver
    /// fields (resources, properties, state fields, etc.).
    pub fn resolve_driver_field_by_index(
        &self,
        index: usize,
    ) -> Result<DriverFieldRef, SemanticLoweringError> {
        if let Some(field) = self.driver_fields.get(index) {
            return Ok(DriverFieldRef {
                owner: self.driver_id,
                canonical_path: field.canonical_path.clone(),
                name: field.name.clone(),
            });
        }
        Err(SemanticLoweringError::UnresolvedDriverField {
            name: format!("field index {index}"),
        })
    }

    // -----------------------------------------------------------------------
    // Resource resolution
    // -----------------------------------------------------------------------

    /// Resolve a resource field to a `ResourceRef`.
    ///
    /// The resource kind is determined from the field's `FieldAttribute`.
    pub fn resolve_resource(&self, field_name: &str) -> Result<ResourceRef, SemanticLoweringError> {
        for field in &self.driver_fields {
            if field.name == field_name {
                if let Some(kind) = resource_kind_from_attributes(&field.attributes) {
                    let field_ref = DriverFieldRef {
                        owner: self.driver_id,
                        canonical_path: field.canonical_path.clone(),
                        name: field.name.clone(),
                    };
                    return Ok(ResourceRef {
                        kind,
                        field: field_ref,
                    });
                }
            }
        }

        Err(SemanticLoweringError::UnresolvedResource {
            field_name: field_name.into(),
        })
    }

    /// Check if a driver field is a known resource field.
    pub fn is_resource_field(&self, field_name: &str) -> bool {
        self.driver_fields
            .iter()
            .any(|f| f.name == field_name && resource_kind_from_attributes(&f.attributes).is_some())
    }

    /// Get the resource kind for a driver field, if it is a resource.
    pub fn resource_kind_for_field(&self, field_name: &str) -> Option<ResourceKind> {
        self.driver_fields
            .iter()
            .find(|f| f.name == field_name)
            .and_then(|f| resource_kind_from_attributes(&f.attributes))
    }

    /// Return the unique driver resource field name for a given kind.
    ///
    /// If zero or multiple resource fields of the requested kind exist,
    /// returns `None`. This is useful for current-slice lowering bridges
    /// that can be made structural only when the ownership is unambiguous.
    pub fn unique_resource_field_name(&self, kind: ResourceKind) -> Option<String> {
        let mut matches = self
            .driver_fields
            .iter()
            .filter(|f| resource_kind_from_attributes(&f.attributes) == Some(kind))
            .map(|f| f.name.clone());

        let first = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(first)
        }
    }

    // -----------------------------------------------------------------------
    // Constant resolution
    // -----------------------------------------------------------------------

    /// Resolve a constant by canonical path to a `ConstantRef`.
    ///
    /// Accepts paths with or without `"unevaluated:"` prefix.
    pub fn resolve_constant(&self, path: &str) -> Result<ConstantRef, SemanticLoweringError> {
        let clean_path = path.strip_prefix("unevaluated:").unwrap_or(path);

        // Canonical path lookup.
        if let Some((id, c)) = self.constant_index.get(clean_path) {
            return Ok(build_constant_ref(*id, c));
        }

        // MIR def_path normalization (deterministic prefix mapping).
        if let Some(canonical) = self.mir_path_to_canonical.get(clean_path) {
            if let Some((id, c)) = self.constant_index.get(canonical) {
                return Ok(build_constant_ref(*id, c));
            }
        }

        Err(SemanticLoweringError::UnresolvedConstant { path: path.into() })
    }

    // -----------------------------------------------------------------------
    // Interrupt class resolution (transitional bridge)
    // -----------------------------------------------------------------------

    /// Resolve an interrupt class by mask value.
    ///
    /// This is a transitional implementation bridge: class identity is
    /// matched by mask value extracted from the `"class_0x..."` encoding
    /// used by the existing MIR interpreter. Not the long-term identity
    /// contract.
    pub fn resolve_interrupt_class_by_mask(
        &self,
        mask: u32,
    ) -> Result<InterruptClassRef, SemanticLoweringError> {
        for (i, class) in self.interrupt_classes.iter().enumerate() {
            if class.mask_value == mask && class.name != "all" {
                return Ok(InterruptClassRef {
                    id: InterruptClassId(i as u32),
                    name: class.name.clone(),
                });
            }
        }

        Err(SemanticLoweringError::UnresolvedInterruptClass {
            mask,
            context: "mask-value matching against hierarchy interrupt classes".into(),
        })
    }

    /// Resolve an interrupt class by the `"class_0xNNNN"` name encoding.
    ///
    /// Extracts the mask value from the name and delegates to
    /// `resolve_interrupt_class_by_mask`.
    pub fn resolve_interrupt_class_by_name(
        &self,
        class_name: &str,
    ) -> Result<InterruptClassRef, SemanticLoweringError> {
        // Try to extract mask from "class_0xNNNN" encoding.
        if let Some(hex_part) = class_name
            .strip_prefix("class_0x")
            .or_else(|| class_name.strip_prefix("class_"))
        {
            if let Ok(mask) = u32::from_str_radix(hex_part.trim_start_matches("0x"), 16) {
                return self.resolve_interrupt_class_by_mask(mask);
            }
        }

        // Try direct name matching against hierarchy classes.
        for (i, class) in self.interrupt_classes.iter().enumerate() {
            if class.name == class_name {
                return Ok(InterruptClassRef {
                    id: InterruptClassId(i as u32),
                    name: class.name.clone(),
                });
            }
        }

        Err(SemanticLoweringError::UnresolvedInterruptClass {
            mask: 0,
            context: format!("name matching for '{class_name}'"),
        })
    }

    // -----------------------------------------------------------------------
    // Owner type queries
    // -----------------------------------------------------------------------

    /// Check if an ItemId is the driver struct.
    pub fn is_driver_struct(&self, id: ItemId) -> bool {
        id == self.driver_id
    }

    /// Look up a StructDef by ItemId from the hierarchy.
    pub fn lookup_struct<'a>(
        &self,
        hierarchy: &'a ItemHierarchy,
        id: ItemId,
    ) -> Option<&'a StructDef> {
        match hierarchy.store.get(id)? {
            Item::Struct(s) => Some(s),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resource_kind_from_attributes(attrs: &[FieldAttribute]) -> Option<ResourceKind> {
    for attr in attrs {
        match attr {
            FieldAttribute::Clock { .. } => return Some(ResourceKind::Clock),
            FieldAttribute::PinCtrl { .. } => return Some(ResourceKind::PinCtrl),
            FieldAttribute::Mmio { .. } => return Some(ResourceKind::Mmio),
            FieldAttribute::Interrupt { .. } => return Some(ResourceKind::Interrupt),
            FieldAttribute::Property { .. } => return Some(ResourceKind::Property),
            _ => {}
        }
    }
    None
}

fn build_constant_ref(id: ItemId, c: &ConstantDef) -> ConstantRef {
    let known_value = c.value.as_ref().and_then(|v| match v {
        ConstValue::Integer(i) => Some(*i as i128),
        ConstValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        ConstValue::Str(_) => None,
    });

    ConstantRef {
        id: Some(id),
        canonical_path: c.canonical_path.clone(),
        name: c.name.clone(),
        stable_name: c.stable_name.clone(),
        known_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lowering::hierarchy::{build_item_hierarchy, CallbackTraitPaths, ItemHierarchy};
    use regseq_model::declaration::raw_input::{
        RawAssociatedConstantFactInput, RawFieldFactInput, RawIdentityEnrichmentInput,
        RawImplBlockFactInput, RawImplMethodFactInput, RawModuleFactInput, RawRseqFieldKindInput,
        RawStructFactInput,
    };
    use regseq_model::item::identity::CanonicalPath;
    use regseq_model::item::interrupt::InterruptClassDef;

    fn minimal_pl011_hierarchy() -> ItemHierarchy {
        let enrichment = RawIdentityEnrichmentInput {
            structs: vec![
                RawStructFactInput {
                    name: "Pl011FullSpec".into(),
                    canonical_path: "pl011::driver::Pl011FullSpec".into(),
                    module_path: "pl011::driver".into(),
                    is_driver: true,
                    is_register_block: false,
                    fields: vec![
                        RawFieldFactInput {
                            name: "regs".into(),
                            canonical_path: "pl011::driver::Pl011FullSpec::regs".into(),
                            rseq_kind: Some(RawRseqFieldKindInput::Mmio {
                                id: "uart_regs".into(),
                            }),
                        },
                        RawFieldFactInput {
                            name: "clock".into(),
                            canonical_path: "pl011::driver::Pl011FullSpec::clock".into(),
                            rseq_kind: Some(RawRseqFieldKindInput::Clock {
                                id: "uart_clk".into(),
                            }),
                        },
                        RawFieldFactInput {
                            name: "pending_tx".into(),
                            canonical_path: "pl011::driver::Pl011FullSpec::pending_tx".into(),
                            rseq_kind: None,
                        },
                    ],
                },
                RawStructFactInput {
                    name: "Pl011Registers".into(),
                    canonical_path: "pl011::registers::Pl011Registers".into(),
                    module_path: "pl011::registers".into(),
                    is_driver: false,
                    is_register_block: true,
                    fields: vec![
                        RawFieldFactInput {
                            name: "cr".into(),
                            canonical_path: "pl011::registers::Pl011Registers::cr".into(),
                            rseq_kind: None,
                        },
                        RawFieldFactInput {
                            name: "ibrd".into(),
                            canonical_path: "pl011::registers::Pl011Registers::ibrd".into(),
                            rseq_kind: None,
                        },
                    ],
                },
            ],
            enums: vec![],
            impl_blocks: vec![RawImplBlockFactInput {
                canonical_path: "pl011::driver::<impl Pl011FullSpec>".into(),
                self_type_path: "pl011::driver::Pl011FullSpec".into(),
                self_type_name: "Pl011FullSpec".into(),
                trait_path: None,
                trait_name: None,
                is_inherent: true,
                methods: vec![
                    RawImplMethodFactInput {
                        name: "set_baud_rate".into(),
                        canonical_path: "pl011::driver::Pl011FullSpec::set_baud_rate".into(),
                        is_isr: false,
                    },
                    RawImplMethodFactInput {
                        name: "uart_clock_hz".into(),
                        canonical_path: "pl011::driver::Pl011FullSpec::uart_clock_hz".into(),
                        is_isr: false,
                    },
                ],
                constants: vec![RawAssociatedConstantFactInput {
                    name: "DEFAULT_UART_CLOCK_HZ".into(),
                    canonical_path: "pl011::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ".into(),
                    ty: "u32".into(),
                    owner_impl_path: "pl011::driver::<impl Pl011FullSpec>".into(),
                }],
            }],
            free_functions: vec![],
            modules: vec![RawModuleFactInput {
                name: "driver".into(),
                canonical_path: "pl011::driver".into(),
                child_items: vec![
                    "pl011::driver::Pl011FullSpec".into(),
                    "pl011::registers::Pl011Registers".into(),
                    "pl011::driver::<impl Pl011FullSpec>".into(),
                ],
            }],
        };

        let constant_values = vec![(
            CanonicalPath::new("pl011::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"),
            ConstValue::Integer(48_000_000),
        )];

        let interrupt_classes = vec![
            InterruptClassDef {
                name: "rx".into(),
                mask_value: 0x0050,
            },
            InterruptClassDef {
                name: "tx".into(),
                mask_value: 0x0020,
            },
        ];

        let trait_paths = CallbackTraitPaths {
            lifecycle: "regseq::Lifecycle".into(),
            serial: "regseq::Serial".into(),
        };

        build_item_hierarchy(
            &enrichment,
            &constant_values,
            &interrupt_classes,
            &BTreeMap::new(),
            &trait_paths,
        )
        .expect("hierarchy build should succeed")
    }

    #[test]
    fn resolver_finds_driver_struct() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        assert!(resolver.driver_id().0 < h.store.len() as u32);
    }

    #[test]
    fn resolver_resolves_function_by_path() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let func_ref = resolver
            .resolve_call("pl011::driver::Pl011FullSpec::set_baud_rate")
            .unwrap();
        assert_eq!(func_ref.name, "set_baud_rate");
        assert!(!func_ref.canonical_path.as_str().is_empty());
    }

    #[test]
    fn resolver_resolves_register() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let reg = resolver.resolve_register("ibrd").unwrap();
        assert_eq!(reg.name, "ibrd");
        assert!(reg.block == resolver.register_block_id().unwrap());
    }

    #[test]
    fn resolver_resolves_driver_field() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let field = resolver.resolve_driver_field("clock").unwrap();
        assert_eq!(field.name, "clock");
        assert_eq!(field.owner, resolver.driver_id());
    }

    #[test]
    fn resolver_resolves_resource() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let resource = resolver.resolve_resource("clock").unwrap();
        assert_eq!(resource.kind, ResourceKind::Clock);
        assert_eq!(resource.field.name, "clock");
    }

    #[test]
    fn resolver_resolves_constant() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let c = resolver
            .resolve_constant("pl011::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ")
            .unwrap();
        assert_eq!(c.name, "DEFAULT_UART_CLOCK_HZ");
        assert_eq!(c.known_value, Some(48_000_000));
    }

    #[test]
    fn resolver_resolves_interrupt_class_by_mask() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        let class = resolver.resolve_interrupt_class_by_mask(0x0050).unwrap();
        assert_eq!(class.name, "rx");
    }

    #[test]
    fn resolver_rejects_unknown_function() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        assert!(resolver.resolve_call("no::such::function").is_err());
    }

    #[test]
    fn resolver_rejects_unknown_register() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        assert!(resolver.resolve_register("nonexistent").is_err());
    }

    #[test]
    fn option_state_field_detection() {
        let h = minimal_pl011_hierarchy();
        let resolver = ReferenceResolver::build(&h).unwrap();
        assert!(resolver.is_option_state_field("pending_tx"));
        assert!(!resolver.is_option_state_field("regs"));
        assert!(!resolver.is_option_state_field("clock"));
    }
}
