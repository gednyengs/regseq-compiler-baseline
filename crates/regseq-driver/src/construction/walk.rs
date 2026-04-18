//! Pass 1 HIR traversal: walks the local crate's module tree and
//! populates a [`RegseqModel`] root module (and all submodules) with
//! declaration-level items.
//!
//! Every ID allocation goes through [`DeclIdAllocator`]; every name goes
//! through [`SymbolInterner`]; every source span is resolved by
//! [`record_span`]. Pass 1 does not touch function bodies -- they are
//! left as default empty placeholders.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rustc_hir::def_id::{DefId, LocalModDefId, CRATE_DEF_ID};
use rustc_hir::ItemKind;
use rustc_middle::ty::TyCtxt;

use regseq_model::identity::{
    ConstantId, DeclId, FunctionId, ModuleId, RegisterBlockId, RegisterDescriptorId, SourceOrigin,
    Symbol, SymbolInterner, TypeDefId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, Mutability,
    RegseqType, ScalarType, Terminator, TerminatorKind,
};
use regseq_model::model::{
    Attrs, Constant, FieldDef, Function, Module, ModuleKind, Param, ParamKind, RegisterDescriptor,
    TypeDef, TypeDefClassification, TypeDefKind, VariantDef,
};

use super::id_allocator::DeclIdAllocator;
use super::{ConstructionError, ErrorProvenance, ExtractionRegistries};
use crate::construction::record_span;

/// Traversal-wide context. Held as a single struct so the many helpers
/// don't have to pass the same 5 parameters everywhere.
pub struct BuildContext<'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub registries: &'a ExtractionRegistries<'a>,
    pub symbols: &'a mut SymbolInterner,
    pub source_locations: &'a mut BTreeMap<DeclId, SourceOrigin>,
    pub allocator: &'a mut DeclIdAllocator,
    pub driver_modules: Vec<ErrorProvenance>,
    /// Crate-wide `DefId -> TypeDefId` map built as type definitions are
    /// assigned. Cross-references (e.g., `Mmio<T>` → register block)
    /// query this map rather than re-walking modules.
    pub type_def_map: HashMap<DefId, TypeDefId>,
    /// Crate-wide `DefId -> RegisterBlockId` map, populated when each
    /// `#[rseq::register_block]` struct is encountered.
    pub register_block_map: HashMap<DefId, RegisterBlockId>,
    /// Crate-wide `DefId -> FunctionId` map, populated as free functions
    /// and methods are assigned IDs. Pass 2 uses this map to resolve
    /// call targets against the model's function surface.
    pub function_map: HashMap<DefId, FunctionId>,
    /// Descriptor-module [`DefId`] -> [`RegisterDescriptorId`] map.
    /// Populated during the descriptor-synthesis pre-pass so register-block
    /// fields can resolve `ReadWrite<T, DR_REG::Register>` descriptor
    /// references without deferring them past Pass 1.
    pub descriptor_map: HashMap<DefId, RegisterDescriptorId>,
    /// Crate-wide `DefId -> ConstantId` map, populated as each free or
    /// associated constant/static is assigned an ID. Pass 2 uses this to
    /// map MIR `Operand::Constant(Unevaluated { def, .. })` to the
    /// model's typed constant surface.
    pub constant_map: HashMap<DefId, ConstantId>,
}

impl<'a, 'tcx> BuildContext<'a, 'tcx> {
    fn alloc_and_record(&mut self, def_id: DefId) -> DeclId {
        let id = self.allocator.alloc();
        record_span(
            self.tcx,
            self.symbols,
            self.source_locations,
            id,
            self.tcx.def_span(def_id),
        );
        id
    }

    fn intern(&mut self, s: &str) -> Symbol {
        self.symbols.intern(s)
    }
}

/// Walk the module rooted at `module_def_id` and populate `target`.
pub fn populate_module_items_and_submodules(
    cx: &mut BuildContext<'_, '_>,
    target: &mut Module,
    module_def_id: DefId,
) -> Result<(), ConstructionError> {
    // Determine if this is a crate-root or submodule walk. Either way we
    // enumerate free items whose parent module is `module_def_id`.
    let items: Vec<rustc_hir::ItemId> = collect_items_in_module(cx.tcx, module_def_id);

    // --- driver discovery + module-kind determination ---
    for item_id in &items {
        let item = cx.tcx.hir_item(*item_id);
        let def_id = item.owner_id.def_id.to_def_id();
        if let ItemKind::Struct(..) = &item.kind {
            if has_rseq_attr(cx.tcx, def_id, "driver") {
                target.kind = ModuleKind::Driver;
                cx.driver_modules
                    .push(ErrorProvenance::for_def_id(cx.tcx, def_id));
            }
        }
    }

    // --- pre-pass A: register descriptors ---
    // Extract every `register_bitfields!`-style descriptor module so
    // descriptor IDs are available before register blocks are built.
    // Register-block fields then resolve their `ReadWrite<T, DR_REG::Register>`
    // descriptor references via `cx.descriptor_map` instead of leaving
    // them as `None`.
    build_register_descriptors_for_module(cx, target, &items);

    // --- pre-pass B: register blocks ---
    // Build every `#[rseq::register_block]` struct so later driver-struct
    // MMIO resolution can always find the block in the
    // `DefId -> RegisterBlockId` map regardless of source order.
    for item_id in &items {
        let item = cx.tcx.hir_item(*item_id);
        let def_id = item.owner_id.def_id.to_def_id();
        if let ItemKind::Struct(ident, _generics, variant_data) = &item.kind {
            if has_rseq_attr(cx.tcx, def_id, "register_block") {
                let block = super::registers::build_register_block(
                    cx,
                    def_id,
                    ident.name.as_str(),
                    variant_data,
                )?;
                cx.register_block_map.insert(def_id, block.id);
                target.register_blocks.push(block);
            }
        }
    }

    // --- item population ---
    // Submodule population requires a second pass because modules are
    // items themselves.
    let mut submodule_defs: Vec<DefId> = Vec::new();
    let mut impls_by_type: HashMap<DefId, Vec<DefId>> = HashMap::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for item_id in &items {
        let item = cx.tcx.hir_item(*item_id);
        let def_id = item.owner_id.def_id.to_def_id();

        match &item.kind {
            ItemKind::Mod(_ident, _module) => {
                submodule_defs.push(def_id);
            }
            ItemKind::Struct(ident, _generics, variant_data) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;

                if has_rseq_attr(cx.tcx, def_id, "register_block") {
                    // Already built in the pre-pass; only duplicate-name
                    // tracking needs to run for this item.
                    continue;
                }

                if has_rseq_attr(cx.tcx, def_id, "driver") {
                    // Driver struct: build a DriverStruct AND a TypeDef
                    // in the same module.
                    let type_def = super::driver::build_driver_struct(
                        cx,
                        target,
                        def_id,
                        ident.name.as_str(),
                        variant_data,
                    )?;
                    cx.type_def_map.insert(def_id, type_def.id);
                    target.type_defs.push(type_def);
                } else {
                    let type_def =
                        build_type_def_for_struct(cx, def_id, ident.name.as_str(), variant_data)?;
                    cx.type_def_map.insert(def_id, type_def.id);
                    target.type_defs.push(type_def);
                }
            }
            ItemKind::Enum(ident, _generics, enum_def) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let type_def = build_type_def_for_enum(cx, def_id, name_str, enum_def)?;
                cx.type_def_map.insert(def_id, type_def.id);
                target.type_defs.push(type_def);
            }
            ItemKind::Const(ident, _generics, _ty, _body_id) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let constant = build_constant(cx, def_id, name_str);
                target.constants.push(constant);
            }
            ItemKind::Static(_mutbl, ident, _ty, _body_id) => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let constant = build_constant(cx, def_id, name_str);
                target.constants.push(constant);
            }
            ItemKind::Fn { ident, .. } => {
                let name_str = ident.name.as_str();
                check_duplicate(cx.tcx, module_def_id, &mut seen_names, name_str)?;
                let func = build_function_stub(cx, def_id, name_str)?;
                target.free_functions.push(func);
            }
            ItemKind::Impl(_impl_block) => {
                // Collect impl; process after the type-def sweep has seen
                // everything so target_type resolution is possible.
                if let Some(target_def) = impl_self_def_id(cx.tcx, def_id) {
                    impls_by_type.entry(target_def).or_default().push(def_id);
                }
            }
            _ => {}
        }
    }

    // --- impl block processing (deferred until type defs are assigned) ---
    // Use the crate-wide `DefId -> TypeDefId` map that tracks every
    // type assigned so far. This lets `impl Foo` blocks resolve whether
    // `Foo` is declared in the same module, a sibling module, or a
    // submodule already processed earlier in the walk.
    for (target_def, impls) in &impls_by_type {
        for &impl_def in impls {
            let impl_block_info = super::impls::build_impl_block(
                cx,
                impl_def,
                *target_def,
                &cx.type_def_map.clone(),
            )?;
            match impl_block_info {
                super::impls::ImplBlockAttachment::NonDriverInherent(block)
                | super::impls::ImplBlockAttachment::NonDriverTrait(block) => {
                    target.impl_blocks.push(block);
                }
                super::impls::ImplBlockAttachment::DriverInherent(block) => {
                    target.driver_impl_blocks.push(block);
                }
                super::impls::ImplBlockAttachment::DriverTrait(block) => {
                    target.trait_impls.push(block);
                }
                super::impls::ImplBlockAttachment::Skipped => {}
            }
        }
    }

    // --- dependency sets ---
    // Backend-focused abstractions/intrinsics accounting runs later in
    // Pass 2 (body analysis). Pass 1 leaves the sets empty.

    // --- submodules ---
    for sub_def_id in submodule_defs {
        let mut submodule = empty_module_for(cx, sub_def_id);
        populate_module_items_and_submodules(cx, &mut submodule, sub_def_id)?;
        target.submodules.push(submodule);
    }

    // Register descriptors were built in pre-pass A above.

    Ok(())
}

fn collect_items_in_module(tcx: TyCtxt<'_>, module_def_id: DefId) -> Vec<rustc_hir::ItemId> {
    let local = module_def_id.as_local().unwrap_or(CRATE_DEF_ID);
    let mod_id = LocalModDefId::new_unchecked(local);
    let mut items: Vec<rustc_hir::ItemId> = Vec::new();
    let module_items = tcx.hir_module_items(mod_id);
    for item_id in module_items.free_items() {
        items.push(item_id);
    }
    items
}

fn check_duplicate(
    tcx: TyCtxt<'_>,
    module_def_id: DefId,
    seen: &mut std::collections::HashSet<String>,
    name: &str,
) -> Result<(), ConstructionError> {
    if !seen.insert(name.to_owned()) {
        return Err(ConstructionError::DuplicateItemName {
            module: ErrorProvenance::for_def_id(tcx, module_def_id),
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, name: &str) -> bool {
    def_has_rseq_attr(tcx, def_id, name)
}

/// Public-to-submodules helper: does `def_id` carry `#[rseq::<name>]`?
pub(crate) fn def_has_rseq_attr(tcx: TyCtxt<'_>, def_id: DefId, name: &str) -> bool {
    for attr in tcx.get_all_attrs(def_id) {
        if let rustc_hir::Attribute::Unparsed(item) = attr {
            let segs = &item.path.segments;
            if segs.len() == 2 && segs[0].as_str() == "rseq" && segs[1].as_str() == name {
                return true;
            }
        }
    }
    false
}

/// Public-to-submodules helper: resolve a field's [`RegseqType`] using the
/// field's middle-ty. Callers that have the field's [`DefId`] available
/// should prefer this over HIR-level rendering so locally-defined ADTs
/// become [`RegseqType::Defined`] rather than `Unit`.
pub(crate) fn resolve_field_ty(cx: &BuildContext<'_, '_>, field_def_id: DefId) -> RegseqType {
    ty_to_regseq(
        cx.tcx,
        &cx.type_def_map,
        cx.tcx.type_of(field_def_id).instantiate_identity(),
    )
    .unwrap_or(RegseqType::Unit)
}

/// Resolve every field in a [`rustc_hir::VariantData`] to a
/// [`FieldDef`], going through middle-ty so ADT fields point at the
/// correct [`TypeDefId`] via the crate-wide map.
pub(crate) fn resolve_fields(
    cx: &mut BuildContext<'_, '_>,
    variant_data: &rustc_hir::VariantData<'_>,
) -> Vec<FieldDef> {
    // Two-phase to avoid overlapping borrows: first compute the type of
    // each field against the shared map, then intern field names.
    let resolved: Vec<(String, RegseqType)> = variant_data
        .fields()
        .iter()
        .map(|f| {
            let ty = resolve_field_ty(cx, f.def_id.to_def_id());
            (f.ident.name.as_str().to_owned(), ty)
        })
        .collect();
    resolved
        .into_iter()
        .map(|(name, ty)| FieldDef {
            name: cx.symbols.intern(&name),
            ty,
        })
        .collect()
}

/// Public-to-submodules helper: render a HIR type as a human-readable
/// string for diagnostic payloads.
pub(crate) fn rendered_ty_string(ty: &rustc_hir::Ty<'_>) -> String {
    match &ty.kind {
        rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) => path
            .segments
            .iter()
            .map(|s| s.ident.name.as_str().to_owned())
            .collect::<Vec<_>>()
            .join("::"),
        rustc_hir::TyKind::Tup([]) => "()".to_owned(),
        rustc_hir::TyKind::Array(_, _) => "[..]".to_owned(),
        _ => "<unknown>".to_owned(),
    }
}

/// Public-to-submodules helper: build a Constant for an associated
/// const declaration.
pub(crate) fn build_associated_constant(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
) -> regseq_model::model::Constant {
    super::walk::build_constant(cx, def_id, name)
}

fn empty_module_for(cx: &mut BuildContext<'_, '_>, def_id: DefId) -> Module {
    let id = ModuleId::from(cx.alloc_and_record(def_id));
    let name = cx
        .tcx
        .opt_item_name(def_id)
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| format!("mod_{}", id.decl_id().index()));
    let name_sym = cx.symbols.intern(&name);
    Module {
        id,
        name: name_sym,
        kind: ModuleKind::Normal,
        submodules: Vec::new(),
        constants: Vec::new(),
        free_functions: Vec::new(),
        type_defs: Vec::new(),
        register_descriptors: Vec::new(),
        register_blocks: Vec::new(),
        impl_blocks: Vec::new(),
        driver: None,
        trait_impls: Vec::new(),
        driver_impl_blocks: Vec::new(),
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    }
}

/// Find every `register_bitfields!`-style module child of `module_def_id`
/// and build a [`RegisterDescriptor`] for each, populating
/// `cx.descriptor_map` so later register-block construction can resolve
/// descriptor references to real IDs. Runs before the register-block
/// pre-pass, so descriptors always exist by the time blocks are built.
fn build_register_descriptors_for_module(
    cx: &mut BuildContext<'_, '_>,
    target: &mut Module,
    items: &[rustc_hir::ItemId],
) {
    use crate::extract::register_access::try_extract_register_descriptor;
    use regseq_model::model::{BitfieldDef, NamedValue};

    for item_id in items {
        let item = cx.tcx.hir_item(*item_id);
        let rustc_hir::ItemKind::Mod(ident, hir_mod) = &item.kind else {
            continue;
        };
        let module_name = ident.name.as_str();
        let Some(fact) = try_extract_register_descriptor(cx.tcx, module_name, hir_mod) else {
            continue;
        };
        let descriptor_def_id = item.owner_id.def_id.to_def_id();
        let id = RegisterDescriptorId::from(cx.alloc_and_record(descriptor_def_id));
        let name_sym = cx.symbols.intern(module_name);

        // Pre-intern field names and named-value names in a batch so the
        // borrow on `cx.symbols` doesn't conflict with the immutable
        // borrow on `cx.tcx` during the outer iteration.
        let fields: Vec<BitfieldDef> = fact
            .fields
            .iter()
            .map(|f| {
                let field_name = cx.symbols.intern(&f.field_name);
                let named_values = f
                    .named_values
                    .iter()
                    .map(|nv| NamedValue {
                        name: cx.symbols.intern(&nv.name),
                        value: nv.value as u64,
                    })
                    .collect();
                BitfieldDef {
                    name: field_name,
                    offset: f.offset,
                    width: f.width,
                    named_values,
                }
            })
            .collect();

        let access_width = access_width_from_fields(&fields);
        target.register_descriptors.push(RegisterDescriptor {
            id,
            name: name_sym,
            access_width,
            fields,
        });
        cx.descriptor_map.insert(descriptor_def_id, id);
    }
}

/// Derive a register's access width from the highest bit touched by any of
/// its fields, rounded up to the nearest standard width. Descriptors
/// without fields (empty `struct Register`) fall back to 32-bit.
fn access_width_from_fields(fields: &[regseq_model::model::BitfieldDef]) -> u32 {
    // Descriptors with no usable field data fall back to 32-bit, matching
    // the default used in the `register_block` attribute.
    let Some(highest_bit) = fields.iter().map(|f| f.offset + f.width).max() else {
        return 32;
    };
    if highest_bit <= 8 {
        8
    } else if highest_bit <= 16 {
        16
    } else if highest_bit <= 32 {
        32
    } else {
        64
    }
}

fn impl_self_def_id(tcx: TyCtxt<'_>, impl_def: DefId) -> Option<DefId> {
    let self_ty = tcx.type_of(impl_def).instantiate_identity();
    self_ty.ty_adt_def().map(|adt| adt.did())
}

// ---------------------------------------------------------------------------
// Item builders (simple cases)
// ---------------------------------------------------------------------------

fn build_type_def_for_struct(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
    variant_data: &rustc_hir::VariantData<'_>,
) -> Result<TypeDef, ConstructionError> {
    let id = TypeDefId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let classification = classify(cx, def_id)?;
    let attrs = collect_rseq_attrs(cx.tcx, def_id)?;

    let fields = resolve_fields(cx, variant_data);

    Ok(TypeDef {
        id,
        name: name_sym,
        kind: TypeDefKind::Struct { fields },
        classification,
        attrs,
    })
}

fn build_type_def_for_enum(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
    enum_def: &rustc_hir::EnumDef<'_>,
) -> Result<TypeDef, ConstructionError> {
    let id = TypeDefId::from(cx.alloc_and_record(def_id));
    let name_sym = cx.intern(name);
    let classification = classify(cx, def_id)?;
    let attrs = collect_rseq_attrs(cx.tcx, def_id)?;

    let variants = enum_def
        .variants
        .iter()
        .map(|v| {
            let vname = cx.symbols.intern(v.ident.name.as_str());
            let fields = resolve_fields(cx, &v.data);
            VariantDef {
                name: vname,
                fields,
                discriminant: None,
            }
        })
        .collect();

    Ok(TypeDef {
        id,
        name: name_sym,
        kind: TypeDefKind::Enum { variants },
        classification,
        attrs,
    })
}

/// Collect every `#[rseq::<name>(...)]` attribute on `def_id` and emit
/// a corresponding [`AttrEntry`] in the returned [`Attrs`] bag. Argument
/// payloads are preserved: positional literals surface as [`AttrValue`]
/// scalars, keyword-arg lists as [`AttrValue::KeyValues`], and presence-only
/// markers as [`AttrValue::Unit`]. Downstream consumers (task 52 PinCtrl
/// metadata, task 43 MIR lowering) rely on these payloads surviving Pass 1.
/// Fails with [`ConstructionError::UnknownRseqAttribute`] on unknown
/// `#[rseq::<name>]` short names.
fn collect_rseq_attrs(tcx: TyCtxt<'_>, def_id: DefId) -> Result<Attrs, ConstructionError> {
    super::attrs::collect_rseq_attrs(tcx, def_id)
}

fn classify(
    cx: &BuildContext<'_, '_>,
    def_id: DefId,
) -> Result<TypeDefClassification, ConstructionError> {
    if let Some(name) = cx.registries.abstraction_types.lookup(def_id) {
        if let Some(ab) = abstraction_from_canonical(name) {
            return Ok(TypeDefClassification::Abstraction(ab));
        }
        return Err(ConstructionError::RegistryLookupMismatch {
            provenance: ErrorProvenance::for_def_id(cx.tcx, def_id),
            canonical: name.to_owned(),
        });
    }
    if let Some(name) = cx.registries.framework_types.lookup(def_id) {
        if let Some(ft) = framework_from_canonical(name) {
            return Ok(TypeDefClassification::Framework(ft));
        }
        return Err(ConstructionError::RegistryLookupMismatch {
            provenance: ErrorProvenance::for_def_id(cx.tcx, def_id),
            canonical: name.to_owned(),
        });
    }
    Ok(TypeDefClassification::UserDefined)
}

fn abstraction_from_canonical(s: &str) -> Option<regseq_model::ir::RegseqAbstraction> {
    use regseq_model::ir::RegseqAbstraction as A;
    Some(match s {
        "clock" => A::Clock,
        "reset" => A::Reset,
        "pinctrl" | "pin_ctrl" => A::PinCtrl,
        "interrupt" => A::Interrupt,
        "mmio" => A::Mmio,
        "mutex" => A::Mutex,
        "spin_lock" | "spinlock" => A::SpinLock,
        "irq_spin_lock" | "irq_spinlock" => A::IrqSpinLock,
        "completion" => A::Completion,
        _ => return None,
    })
}

fn framework_from_canonical(s: &str) -> Option<regseq_model::model::FrameworkTypeKind> {
    use regseq_model::model::FrameworkTypeKind as F;
    Some(match s {
        "regseq-error" | "regseq_error" => F::RegseqError,
        "serial-errors" | "serial_errors" => F::SerialErrors,
        "rx-char" | "rx_char" => F::RxChar,
        "serial-config" | "serial_config" => F::SerialConfig,
        "data-bits" | "data_bits" => F::DataBits,
        "parity" => F::Parity,
        "stop-bits" | "stop_bits" => F::StopBits,
        "flow-control" | "flow_control" => F::FlowControl,
        "duration" => F::Duration,
        "irq-result" | "irq_result" => F::IrqResult,
        _ => return None,
    })
}

pub(crate) fn build_constant(cx: &mut BuildContext<'_, '_>, def_id: DefId, name: &str) -> Constant {
    let id = ConstantId::from(cx.alloc_and_record(def_id));
    cx.constant_map.insert(def_id, id);
    let name_sym = cx.intern(name);
    let ty = ty_to_regseq(
        cx.tcx,
        &cx.type_def_map,
        cx.tcx.type_of(def_id).instantiate_identity(),
    )
    .unwrap_or(RegseqType::Scalar(ScalarType::U32));
    let value = eval_const(cx.tcx, def_id).unwrap_or_else(|| fallback_literal(&ty));
    Constant {
        id,
        name: name_sym,
        ty,
        value,
    }
}

fn eval_const(tcx: TyCtxt<'_>, def_id: DefId) -> Option<LiteralValue> {
    let value = tcx.const_eval_poly(def_id).ok()?;
    let scalar = value.try_to_scalar()?;
    let bits = scalar.to_bits(scalar.size()).discard_err()?;
    let ty = tcx.type_of(def_id).instantiate_identity();
    Some(scalar_to_literal(ty, bits))
}

fn scalar_to_literal(ty: rustc_middle::ty::Ty<'_>, bits: u128) -> LiteralValue {
    use rustc_middle::ty::TyKind;
    match ty.kind() {
        TyKind::Uint(uint) => match uint {
            rustc_middle::ty::UintTy::U8 => LiteralValue::U8(bits as u8),
            rustc_middle::ty::UintTy::U16 => LiteralValue::U16(bits as u16),
            rustc_middle::ty::UintTy::U32 => LiteralValue::U32(bits as u32),
            rustc_middle::ty::UintTy::U64 => LiteralValue::U64(bits as u64),
            rustc_middle::ty::UintTy::Usize => LiteralValue::Usize(bits as u64),
            rustc_middle::ty::UintTy::U128 => LiteralValue::U64(bits as u64),
        },
        TyKind::Int(int) => match int {
            rustc_middle::ty::IntTy::I8 => LiteralValue::I8(bits as i8),
            rustc_middle::ty::IntTy::I16 => LiteralValue::I16(bits as i16),
            rustc_middle::ty::IntTy::I32 => LiteralValue::I32(bits as i32),
            rustc_middle::ty::IntTy::I64 => LiteralValue::I64(bits as i64),
            rustc_middle::ty::IntTy::Isize => LiteralValue::Isize(bits as i64),
            rustc_middle::ty::IntTy::I128 => LiteralValue::I64(bits as i64),
        },
        TyKind::Bool => LiteralValue::Bool(bits != 0),
        _ => LiteralValue::U32(bits as u32),
    }
}

fn fallback_literal(ty: &RegseqType) -> LiteralValue {
    match ty {
        RegseqType::Bool => LiteralValue::Bool(false),
        RegseqType::Scalar(ScalarType::U8) => LiteralValue::U8(0),
        RegseqType::Scalar(ScalarType::U16) => LiteralValue::U16(0),
        RegseqType::Scalar(ScalarType::U64) => LiteralValue::U64(0),
        RegseqType::Scalar(ScalarType::I32) => LiteralValue::I32(0),
        _ => LiteralValue::U32(0),
    }
}

fn build_function_stub(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
    name: &str,
) -> Result<Function, ConstructionError> {
    let id = FunctionId::from(cx.alloc_and_record(def_id));
    cx.function_map.insert(def_id, id);
    let name_sym = cx.intern(name);
    let (params, return_type) = function_signature(cx, def_id);
    let body = empty_body(cx.tcx, cx.symbols, def_id, params.len());
    let attrs = super::attrs::collect_rseq_attrs(cx.tcx, def_id)?;
    Ok(Function {
        id,
        name: name_sym,
        params,
        return_type,
        body,
        attrs,
    })
}

pub(crate) fn function_signature(
    cx: &mut BuildContext<'_, '_>,
    def_id: DefId,
) -> (Vec<Param>, RegseqType) {
    let sig = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
    let mut params = Vec::new();
    let assoc = cx.tcx.opt_associated_item(def_id);
    let receiver_count = match assoc {
        Some(item) if item.is_method() => 1,
        _ => 0,
    };
    for (i, input) in sig.inputs().iter().enumerate() {
        let kind = if i < receiver_count {
            ParamKind::Receiver
        } else {
            ParamKind::Regular
        };
        let name = if i < receiver_count {
            "self".to_owned()
        } else {
            format!("arg{i}")
        };
        let ty = ty_to_regseq(cx.tcx, &cx.type_def_map, *input).unwrap_or(RegseqType::Unit);
        let name_sym = cx.symbols.intern(&name);
        params.push(Param {
            kind,
            name: name_sym,
            ty,
        });
    }
    let return_type =
        ty_to_regseq(cx.tcx, &cx.type_def_map, sig.output()).unwrap_or(RegseqType::Unit);
    (params, return_type)
}

pub(crate) fn empty_body(
    tcx: TyCtxt<'_>,
    symbols: &mut SymbolInterner,
    def_id: DefId,
    arg_count: usize,
) -> Body {
    let span = tcx.def_span(def_id);
    let origin = super::resolve_origin(tcx, symbols, span);
    let mut local_decls = Vec::with_capacity(arg_count + 1);
    local_decls.push(LocalDecl {
        ty: RegseqType::Unit,
        mutable: true,
        name: None,
    });
    for _ in 0..arg_count {
        local_decls.push(LocalDecl {
            ty: RegseqType::Unit,
            mutable: false,
            name: None,
        });
    }
    // Pass 1 placeholder bodies reuse the function's `def_span` for every
    // local and the synthetic `Unreachable` terminator. Pass 2 overwrites
    // both with the real MIR-derived spans. Using the function's own span
    // keeps Pass 1 free of `CompilerGenerated` provenance (spec
    // obligation 8) without pretending these synthetic placeholders have
    // distinct authored locations.
    let locals_source = (0..local_decls.len()).map(|_| origin.clone()).collect();
    Body {
        local_decls,
        arg_count,
        basic_blocks: vec![BasicBlock {
            statements: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
            },
        }],
        source_map: BodySourceMap {
            body_span: origin.clone(),
            locals: locals_source,
            blocks: vec![BasicBlockSourceMap {
                statements: Vec::new(),
                terminator: origin,
            }],
        },
        ordered_regions: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Type rendering helpers
// ---------------------------------------------------------------------------

pub(crate) fn ty_to_regseq<'tcx>(
    tcx: TyCtxt<'tcx>,
    type_def_map: &HashMap<DefId, TypeDefId>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Option<RegseqType> {
    use rustc_middle::ty::TyKind;
    Some(match ty.kind() {
        TyKind::Bool => RegseqType::Bool,
        TyKind::Tuple(list) if list.is_empty() => RegseqType::Unit,
        TyKind::Never => RegseqType::Never,
        TyKind::Uint(u) => RegseqType::Scalar(match u {
            rustc_middle::ty::UintTy::U8 => ScalarType::U8,
            rustc_middle::ty::UintTy::U16 => ScalarType::U16,
            rustc_middle::ty::UintTy::U32 => ScalarType::U32,
            rustc_middle::ty::UintTy::U64 => ScalarType::U64,
            rustc_middle::ty::UintTy::Usize => ScalarType::Usize,
            rustc_middle::ty::UintTy::U128 => ScalarType::U64,
        }),
        TyKind::Int(i) => RegseqType::Scalar(match i {
            rustc_middle::ty::IntTy::I8 => ScalarType::I8,
            rustc_middle::ty::IntTy::I16 => ScalarType::I16,
            rustc_middle::ty::IntTy::I32 => ScalarType::I32,
            rustc_middle::ty::IntTy::I64 => ScalarType::I64,
            rustc_middle::ty::IntTy::Isize => ScalarType::Isize,
            rustc_middle::ty::IntTy::I128 => ScalarType::I64,
        }),
        TyKind::Array(inner, len_const) => {
            let inner_ty = ty_to_regseq(tcx, type_def_map, *inner)?;
            let len = len_const
                .try_to_target_usize(tcx)
                .map(|n| n as usize)
                .unwrap_or(0);
            RegseqType::Array(Box::new(inner_ty), len)
        }
        TyKind::Slice(inner) => {
            let inner_ty = ty_to_regseq(tcx, type_def_map, *inner)?;
            RegseqType::Slice(Box::new(inner_ty))
        }
        TyKind::Ref(_, inner, mutbl) => {
            let inner_ty = ty_to_regseq(tcx, type_def_map, *inner)?;
            let m = match mutbl {
                rustc_ast::Mutability::Not => Mutability::Shared,
                rustc_ast::Mutability::Mut => Mutability::Mutable,
            };
            RegseqType::Ref(Box::new(inner_ty), m)
        }
        TyKind::Adt(adt, _) => {
            let did = adt.did();
            if let Some(&tid) = type_def_map.get(&did) {
                return Some(RegseqType::Defined(tid));
            }
            // Foreign (std, facade) or not-yet-registered ADT: caller
            // decides the fallback.
            return None;
        }
        _ => return None,
    })
}
